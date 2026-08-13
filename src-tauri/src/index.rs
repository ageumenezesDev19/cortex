use std::path::{Path, PathBuf};

use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use walkdir::WalkDir;

use crate::ocr::OcrResult;
use crate::{parse, vault};

pub struct Indexer {
    root: PathBuf,
    conn: Connection,
}

#[derive(Debug, Default, Clone, Copy, serde::Serialize)]
pub struct Counts {
    pub notes: i64,
    pub images: i64,
    pub chunks: i64,
}

impl Indexer {
    pub fn new(root: PathBuf, conn: Connection) -> Self {
        Self { root, conn }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns `false` when the file was already indexed unchanged — the hash
    /// comparison is what keeps a `git checkout` from reindexing the vault.
    pub fn index_note(&mut self, abs: &Path) -> Result<bool> {
        let Ok(source) = std::fs::read_to_string(abs) else {
            return Ok(false);
        };
        let rel = vault::relative(&self.root, abs)?;
        let hash = blake3::hash(source.as_bytes()).to_hex().to_string();

        let known: Option<String> = self
            .conn
            .query_row("SELECT content_hash FROM notes WHERE path = ?1", [&rel], |r| {
                r.get(0)
            })
            .optional()?;
        if known.as_deref() == Some(hash.as_str()) {
            return Ok(false);
        }

        let stem = abs
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| rel.clone());
        let title = parse::title_of(&source, &stem);
        let folder = vault::folder_of(&rel);
        let mtime = vault::mtime_of(abs);

        // Resolved before the transaction opens: `resolve_link` reads `self`,
        // and the transaction holds it mutably for as long as it lives.
        let linked_assets: Vec<String> = parse::image_links(&source)
            .iter()
            .filter_map(|dest| self.resolve_link(&rel, dest))
            .collect();

        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO notes (path, title, folder, mtime, content_hash)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(path) DO UPDATE SET
               title = excluded.title, folder = excluded.folder,
               mtime = excluded.mtime, content_hash = excluded.content_hash",
            params![rel, title, folder, mtime, hash],
        )?;
        let note_id: i64 = tx.query_row("SELECT id FROM notes WHERE path = ?1", [&rel], |r| r.get(0))?;

        tx.execute(
            "DELETE FROM chunks WHERE note_id = ?1 AND kind = 'text'",
            [note_id],
        )?;
        {
            let mut insert = tx.prepare(
                "INSERT INTO chunks (note_id, kind, heading_path, start_offset, end_offset, text)
                 VALUES (?1, 'text', ?2, ?3, ?4, ?5)",
            )?;
            for chunk in parse::chunk_markdown(&source) {
                insert.execute(params![
                    note_id,
                    chunk.heading_path,
                    chunk.start as i64,
                    chunk.end as i64,
                    chunk.text
                ])?;
            }
        }

        // Screenshots the note points at belong to it, so an image result can
        // say which note it came from.
        {
            let mut link = tx.prepare("UPDATE assets SET note_id = ?1 WHERE path = ?2")?;
            for asset_rel in &linked_assets {
                link.execute(params![note_id, asset_rel])?;
            }
        }
        tx.commit()?;
        Ok(true)
    }

    pub fn remove_note(&mut self, abs: &Path) -> Result<()> {
        let rel = vault::relative(&self.root, abs)?;
        self.conn
            .execute("DELETE FROM notes WHERE path = ?1", [rel])?;
        Ok(())
    }

    pub fn index_asset(&mut self, abs: &Path, ocr: &OcrResult) -> Result<()> {
        let rel = vault::relative(&self.root, abs)?;
        let mtime = vault::mtime_of(abs);
        let note_id = self.owning_note(abs)?;
        let label = abs
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| rel.clone());

        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO assets (path, ocr_json, mtime, note_id)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(path) DO UPDATE SET
               ocr_json = excluded.ocr_json, mtime = excluded.mtime,
               note_id = COALESCE(excluded.note_id, assets.note_id)",
            params![rel, serde_json::to_string(ocr)?, mtime, note_id],
        )?;
        let asset_id: i64 =
            tx.query_row("SELECT id FROM assets WHERE path = ?1", [&rel], |r| r.get(0))?;

        tx.execute("DELETE FROM chunks WHERE asset_id = ?1", [asset_id])?;
        if !ocr.text.trim().is_empty() {
            tx.execute(
                "INSERT INTO chunks (asset_id, note_id, kind, heading_path, text)
                 VALUES (?1, ?2, 'image', ?3, ?4)",
                params![asset_id, note_id, label, ocr.text],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn remove_asset(&mut self, abs: &Path) -> Result<()> {
        let rel = vault::relative(&self.root, abs)?;
        self.conn
            .execute("DELETE FROM assets WHERE path = ?1", [rel])?;
        Ok(())
    }

    pub fn asset_is_current(&self, abs: &Path) -> Result<bool> {
        let rel = vault::relative(&self.root, abs)?;
        let stored: Option<i64> = self
            .conn
            .query_row(
                "SELECT mtime FROM assets WHERE path = ?1 AND ocr_json IS NOT NULL",
                [&rel],
                |r| r.get(0),
            )
            .optional()?;
        Ok(stored == Some(vault::mtime_of(abs)))
    }

    /// Walks the vault and indexes every markdown file, returning the images
    /// that still need OCR.
    pub fn scan(&mut self) -> Result<Vec<PathBuf>> {
        let mut pending_images = Vec::new();
        let root = self.root.clone();
        for entry in WalkDir::new(&root).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if !entry.file_type().is_file() || vault::is_ignored(&root, path) {
                continue;
            }
            if vault::is_markdown(path) {
                let _ = self.index_note(path);
            } else if vault::is_image(path) && !self.asset_is_current(path).unwrap_or(false) {
                pending_images.push(path.to_path_buf());
            }
        }
        self.prune_missing()?;
        Ok(pending_images)
    }

    /// Drops rows whose file disappeared while the app was closed.
    fn prune_missing(&mut self) -> Result<()> {
        for table in ["notes", "assets"] {
            let paths: Vec<String> = {
                let mut stmt = self.conn.prepare(&format!("SELECT path FROM {table}"))?;
                let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
                rows.filter_map(|r| r.ok())
                    .filter(|rel| !vault::absolute(&self.root, rel).exists())
                    .collect()
            };
            for rel in paths {
                self.conn
                    .execute(&format!("DELETE FROM {table} WHERE path = ?1"), [rel])?;
            }
        }
        Ok(())
    }

    pub fn counts(&self) -> Result<Counts> {
        Ok(Counts {
            notes: self.conn.query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0))?,
            images: self
                .conn
                .query_row("SELECT COUNT(*) FROM assets WHERE ocr_json IS NOT NULL", [], |r| {
                    r.get(0)
                })?,
            chunks: self.conn.query_row("SELECT COUNT(*) FROM chunks", [], |r| r.get(0))?,
        })
    }

    /// Resolves a markdown link destination against the note that wrote it.
    fn resolve_link(&self, note_rel: &str, dest: &str) -> Option<String> {
        if dest.contains("://") {
            return None;
        }
        let base = vault::absolute(&self.root, note_rel);
        let joined = base.parent()?.join(dest);
        let canonical = joined.canonicalize().ok()?;
        vault::relative(&self.root, &canonical).ok()
    }

    /// An image has no frontmatter and no link back, so ownership is decided by
    /// asking the notes beside its `_assets/` folder which one mentions it.
    fn owning_note(&self, image: &Path) -> Result<Option<i64>> {
        let Some(assets_dir) = image.parent() else {
            return Ok(None);
        };
        let Some(note_dir) = assets_dir.parent() else {
            return Ok(None);
        };
        let Some(file_name) = image.file_name().and_then(|n| n.to_str()) else {
            return Ok(None);
        };

        let Ok(entries) = std::fs::read_dir(note_dir) else {
            return Ok(None);
        };
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if !vault::is_markdown(&path) {
                continue;
            }
            let Ok(source) = std::fs::read_to_string(&path) else {
                continue;
            };
            if !source.contains(file_name) {
                continue;
            }
            let rel = vault::relative(&self.root, &path)?;
            let id: Option<i64> = self
                .conn
                .query_row("SELECT id FROM notes WHERE path = ?1", [&rel], |r| r.get(0))
                .optional()?;
            if id.is_some() {
                return Ok(id);
            }
        }
        Ok(None)
    }
}
