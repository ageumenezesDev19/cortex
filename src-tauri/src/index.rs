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

    /// Lent out so the embedding can run on this thread with the connection
    /// that already serialises with the writes.
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    /// Reloads the topic manifests into their database mirror. Whole-file
    /// replacement, so an edit made in the editor cannot leave the index behind.
    pub fn sync_topics(&mut self) -> Result<()> {
        let topics = crate::topics::load_all(&self.root)?;
        crate::topics::sync_to_db(&mut self.conn, &topics)
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
            // The title and the path ride on the chunk so the search can weigh
            // them: the name of a note is often the only place a word appears.
            let mut insert = tx.prepare(
                "INSERT INTO chunks
                   (note_id, kind, heading_path, start_offset, end_offset, text, title, path)
                 VALUES (?1, 'text', ?2, ?3, ?4, ?5, ?6, ?7)",
            )?;
            for chunk in parse::chunk_markdown(&source) {
                insert.execute(params![
                    note_id,
                    chunk.heading_path,
                    chunk.start as i64,
                    chunk.end as i64,
                    chunk.text,
                    title,
                    rel
                ])?;
            }
        }

        // Screenshots the note points at belong to it, so an image result can
        // say which note it came from — and so the search shows the print inside
        // the note instead of a second time beside it.
        //
        // Cleared before being set, or an image taken out of a note would stay
        // filed under it forever. The chunk carries the same link because that
        // is the column the search groups by; leaving it behind is what used to
        // put one screenshot in two results.
        {
            tx.execute("UPDATE assets SET note_id = NULL WHERE note_id = ?1", [note_id])?;
            tx.execute(
                "UPDATE chunks SET note_id = NULL WHERE note_id = ?1 AND kind = 'image'",
                [note_id],
            )?;

            let mut link = tx.prepare("UPDATE assets SET note_id = ?1 WHERE path = ?2")?;
            let mut link_chunks = tx.prepare(
                "UPDATE chunks SET note_id = ?1
                 WHERE asset_id = (SELECT id FROM assets WHERE path = ?2)",
            )?;
            for asset_rel in &linked_assets {
                link.execute(params![note_id, asset_rel])?;
                link_chunks.execute(params![note_id, asset_rel])?;
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
        let note_id = self.owning_note(abs)?;
        let root = self.root.clone();
        index_asset_into(&mut self.conn, &root, abs, ocr, note_id)
    }
}

/// The write itself, on a plain connection. Split out so the classify-existing
/// pass can reindex a screenshot without owning the watcher's indexer — one
/// place still decides what an image chunk looks like.
pub fn index_asset_into(
    conn: &mut Connection,
    root: &Path,
    abs: &Path,
    ocr: &OcrResult,
    note_id: Option<i64>,
) -> Result<()> {
    {
        let rel = vault::relative(root, abs)?;
        let mtime = vault::mtime_of(abs);
        let label = abs
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| rel.clone());

        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO assets (path, ocr_json, mtime, note_id, content_hash)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(path) DO UPDATE SET
               ocr_json = excluded.ocr_json, mtime = excluded.mtime,
               note_id = COALESCE(excluded.note_id, assets.note_id),
               content_hash = excluded.content_hash",
            params![
                rel,
                serde_json::to_string(ocr)?,
                mtime,
                note_id,
                vault::content_hash(abs)
            ],
        )?;
        // Read back rather than reuse the parameter: the upsert above keeps the
        // owner this run did not find, and the chunk has to say the same thing
        // the asset does or the search splits the image off into its own result.
        let (asset_id, owner): (i64, Option<i64>) = tx.query_row(
            "SELECT id, note_id FROM assets WHERE path = ?1",
            [&rel],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;

        // The marks join the indexed text, so searching `css` finds a
        // screenshot of a stylesheet even though the word never appears in it.
        // They also join the heading, which the result card already shows —
        // that is the line telling you why this screenshot came back.
        let marks = ocr.marks.as_ref().map(|marks| marks.all()).unwrap_or_default();
        let heading = if marks.is_empty() {
            label.clone()
        } else {
            format!("{label} · {}", marks.join(" · "))
        };
        let text = if marks.is_empty() {
            ocr.text.clone()
        } else {
            format!("{}\n{}", marks.join(" "), ocr.text)
        };

        tx.execute("DELETE FROM chunks WHERE asset_id = ?1", [asset_id])?;
        if !ocr.text.trim().is_empty() {
            tx.execute(
                "INSERT INTO chunks (asset_id, note_id, kind, heading_path, text, title, path)
                 VALUES (?1, ?2, 'image', ?3, ?4, ?5, ?6)",
                params![asset_id, owner, heading, text, label, rel],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
}

impl Indexer {
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
        self.fill_missing_hashes()?;
        Ok(pending_images)
    }

    /// Screenshots indexed before the column existed. Reading the bytes is cheap
    /// next to the OCR they already went through, so they are hashed here rather
    /// than sent round the whole pipeline again.
    fn fill_missing_hashes(&mut self) -> Result<()> {
        let stale: Vec<String> = {
            let mut stmt = self
                .conn
                .prepare("SELECT path FROM assets WHERE content_hash IS NULL")?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
            rows.filter_map(|r| r.ok()).collect()
        };

        for rel in stale {
            let Some(hash) = vault::content_hash(&vault::absolute(&self.root, &rel)) else {
                continue;
            };
            self.conn.execute(
                "UPDATE assets SET content_hash = ?1 WHERE path = ?2",
                params![hash, rel],
            )?;
        }
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn ocr(text: &str) -> crate::ocr::OcrResult {
        crate::ocr::OcrResult {
            text: text.into(),
            blocks: Vec::new(),
            lang: "pt".into(),
            ocr_at: String::new(),
            marks: None,
        }
    }

    /// A note, a screenshot beside it, and control over which is read first.
    ///
    /// The root is canonical because `resolve_link` canonicalises what it
    /// resolves, and on macOS a temporary folder is reached through a symlink —
    /// an uncanonical root would make every link in the test fall outside it.
    fn vault() -> (TempDir, Indexer) {
        let dir = TempDir::new().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::create_dir_all(root.join("react").join("_assets")).unwrap();
        let indexer = Indexer::new(root, crate::db::open_in_memory().unwrap());
        (dir, indexer)
    }

    fn chunk_note_id(indexer: &Indexer) -> Option<i64> {
        indexer
            .conn
            .query_row("SELECT note_id FROM chunks WHERE kind = 'image'", [], |r| r.get(0))
            .unwrap()
    }

    /// The bug this covers: the image was read before the note pointed at it, so
    /// its chunk carried no note and the search showed the print a second time,
    /// loose, beside the note that already contained it.
    #[test]
    fn a_print_linked_after_it_was_read_belongs_to_the_note() {
        let (dir, mut indexer) = vault();
        let note = indexer.root().join("react").join("hooks.md");
        let print = indexer.root().join("react").join("_assets").join("print.png");

        std::fs::write(&note, "# Hooks\n\ntexto\n").unwrap();
        indexer.index_note(&note).unwrap();
        std::fs::write(&print, b"x").unwrap();
        indexer.index_asset(&print, &ocr("cache no print")).unwrap();
        assert_eq!(chunk_note_id(&indexer), None, "nada aponta para o print ainda");

        std::fs::write(&note, "# Hooks\n\ntexto\n\n![](_assets/print.png)\n").unwrap();
        indexer.index_note(&note).unwrap();

        assert!(chunk_note_id(&indexer).is_some(), "o print ficaria solto na busca");
    }

    #[test]
    fn a_print_taken_out_of_a_note_is_loose_again() {
        let (dir, mut indexer) = vault();
        let note = indexer.root().join("react").join("hooks.md");
        let print = indexer.root().join("react").join("_assets").join("print.png");

        std::fs::write(&note, "# Hooks\n\n![](_assets/print.png)\n").unwrap();
        std::fs::write(&print, b"x").unwrap();
        indexer.index_asset(&print, &ocr("cache no print")).unwrap();
        indexer.index_note(&note).unwrap();
        assert!(chunk_note_id(&indexer).is_some());

        std::fs::write(&note, "# Hooks\n\nsem o print\n").unwrap();
        indexer.index_note(&note).unwrap();

        assert_eq!(chunk_note_id(&indexer), None, "o vínculo sobreviveu à edição");
    }
}
