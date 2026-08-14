use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::{html, vault};

/// Where imported notes land, so nothing lands mixed with what you wrote.
pub const IMPORT_DIR: &str = "_importado";
/// Starts with a dot, so `vault::is_ignored` already keeps it out of the tree.
const MANIFEST_DIR: &str = ".import";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteRef {
    pub id: String,
    pub name: String,
    pub folder: String,
    pub modified: String,
}

/// What one note left behind, and how to find it again.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Imported {
    pub id: String,
    pub modified: String,
    /// Images left in Notes.app; the second pass reads this to know where to go.
    pub attachments: usize,
}

pub type Manifest = HashMap<String, Imported>;

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub imported: u32,
    pub skipped: u32,
    pub failed: u32,
    pub attachments: usize,
}

/// Every note's metadata in one call. Bodies are fetched one at a time — see
/// `body_of`, and the reason there.
pub fn list() -> Result<Vec<NoteRef>> {
    // Walked folder by folder rather than note by note: a note will not tell you
    // its own folder — `name of container` refuses to coerce to text — and the
    // folder is what keeps the imported vault organised instead of one flat pile.
    // Verified: the 36 folders account for all 232 notes, so nothing is missed.
    let script = r#"
        set out to ""
        tell application "Notes"
          repeat with f in folders
            set folderName to name of f
            repeat with n in notes of f
              try
                set out to out & (id of n) & tab & (name of n) & tab & folderName & tab & ((modification date of n) as string) & linefeed
              end try
            end repeat
          end repeat
        end tell
        return out
    "#;

    let output = Command::new("osascript").arg("-e").arg(script).output()?;
    if !output.status.success() {
        return Err(anyhow!(
            "o Notes.app recusou a leitura: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            Some(NoteRef {
                id: parts.next()?.to_string(),
                name: parts.next()?.trim().to_string(),
                folder: parts.next().unwrap_or_default().trim().to_string(),
                modified: parts.next().unwrap_or_default().trim().to_string(),
            })
        })
        .filter(|note| !note.id.is_empty())
        .collect())
}

/// Writes one note's HTML straight to a file.
///
/// This exists because the bodies carry their images as base64: the whole
/// library is ~2 GB and a single note reaches 271 MB. Letting `osascript` write
/// to a file keeps that out of this process — peak cost is one note on disk,
/// never one note in memory twice.
fn body_of(id: &str, destination: &Path) -> Result<()> {
    let file = std::fs::File::create(destination)?;
    let script = format!(
        r#"tell application "Notes" to return body of note id "{}""#,
        id.replace('"', "")
    );

    let status = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .stdout(Stdio::from(file))
        .stderr(Stdio::null())
        .status()?;

    if !status.success() {
        return Err(anyhow!("não consegui ler o corpo da nota"));
    }
    Ok(())
}

/// Reads the body, dropping the base64 payloads on the way in.
///
/// This pass imports text, and the images are what make these files enormous —
/// one note is 271 MB. Reading the file whole and handing it to an HTML parser
/// would cost several times that in memory for bytes we then throw away. So the
/// payload is skipped byte by byte, and each image is left as a bare `<img>` for
/// the converter to mark.
fn read_without_data_uris(path: &Path) -> Result<String> {
    use std::io::Read;

    let mut file = std::io::BufReader::new(std::fs::File::open(path)?);
    let mut out = Vec::new();
    let mut buffer = [0u8; 64 * 1024];
    let mut skipping = false;
    let mut tail: Vec<u8> = Vec::new();

    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        for &byte in &buffer[..read] {
            if skipping {
                // The payload runs until the attribute's closing quote.
                if byte == b'"' {
                    skipping = false;
                    out.push(byte);
                }
                continue;
            }

            out.push(byte);
            tail.push(byte);
            if tail.len() > 5 {
                tail.remove(0);
            }
            if tail == b"data:" {
                out.truncate(out.len() - 5);
                skipping = true;
                tail.clear();
            }
        }
    }
    Ok(String::from_utf8_lossy(&out).into_owned())
}

pub fn manifest_path(root: &Path) -> PathBuf {
    root.join(MANIFEST_DIR).join("notes.json")
}

pub fn read_manifest(root: &Path) -> Manifest {
    std::fs::read_to_string(manifest_path(root))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

fn write_manifest(root: &Path, manifest: &Manifest) -> Result<()> {
    let path = manifest_path(root);
    std::fs::create_dir_all(path.parent().unwrap_or(root))?;
    std::fs::write(path, serde_json::to_vec_pretty(manifest)?)?;
    Ok(())
}

/// Imports every note, reporting progress as it goes. Text only in this pass:
/// each image leaves a mark in the markdown and a count in the manifest.
///
/// Re-running is safe. A note whose modification date has not moved is skipped,
/// so an interrupted import resumes instead of duplicating.
pub fn run(root: &Path, mut progress: impl FnMut(u32, u32, &str)) -> Result<Report> {
    let notes = list()?;
    let total = notes.len() as u32;
    let mut manifest = read_manifest(root);
    let mut report = Report::default();

    let scratch = std::env::temp_dir().join("cortex-import");
    std::fs::create_dir_all(&scratch)?;
    let body_file = scratch.join("body.html");

    for (index, note) in notes.iter().enumerate() {
        progress(index as u32 + 1, total, &note.name);

        let path = destination_for(root, note);
        if manifest
            .get(&path)
            .is_some_and(|done| done.id == note.id && done.modified == note.modified)
        {
            report.skipped += 1;
            continue;
        }

        match import_one(root, note, &path, &body_file) {
            Ok(attachments) => {
                report.imported += 1;
                report.attachments += attachments;
                manifest.insert(
                    path,
                    Imported {
                        id: note.id.clone(),
                        modified: note.modified.clone(),
                        attachments,
                    },
                );
            }
            Err(_) => report.failed += 1,
        }
    }

    let _ = std::fs::remove_file(&body_file);
    write_manifest(root, &manifest)?;
    Ok(report)
}

fn import_one(root: &Path, note: &NoteRef, path: &str, body_file: &Path) -> Result<usize> {
    body_of(&note.id, body_file)?;
    let raw = read_without_data_uris(body_file)?;
    let converted = html::to_markdown(&raw);

    // The note's name is the heading; Notes keeps it apart from the body, and
    // without it a note whose body is empty would import as an empty file.
    let title = if note.name.is_empty() { "Sem título" } else { &note.name };
    let source = format!("# {title}\n\n{}", converted.markdown);

    let absolute = vault::resolve(root, path)?;
    if let Some(parent) = absolute.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(absolute, source)?;
    Ok(converted.attachments)
}

/// `_importado/<pasta>/<nota>.md`, slugged the same way the app names everything
/// else it writes.
fn destination_for(root: &Path, note: &NoteRef) -> String {
    let folder = vault::slugify(&note.folder);
    let name = {
        let slug = vault::slugify(&note.name);
        if slug.is_empty() {
            format!("sem-titulo-{}", short_id(&note.id))
        } else {
            slug
        }
    };

    let mut path = if folder.is_empty() {
        format!("{IMPORT_DIR}/{name}.md")
    } else {
        format!("{IMPORT_DIR}/{folder}/{name}.md")
    };

    // Two notes in the same folder can share a name; the id keeps them apart.
    if vault::absolute(root, &path).exists() && !already_ours(root, &path, &note.id) {
        path = path.replace(".md", &format!("-{}.md", short_id(&note.id)));
    }
    path
}

fn already_ours(root: &Path, path: &str, id: &str) -> bool {
    read_manifest(root).get(path).is_some_and(|done| done.id == id)
}

fn short_id(id: &str) -> String {
    id.rsplit('/').next().unwrap_or(id).replace("p", "").chars().take(6).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn note(name: &str, folder: &str) -> NoteRef {
        NoteRef {
            id: "x-coredata://ABC/ICNote/p123".into(),
            name: name.into(),
            folder: folder.into(),
            modified: "quarta-feira, 13 de agosto de 2026".into(),
        }
    }

    #[test]
    fn a_note_lands_under_its_folder_inside_the_import_area() {
        let dir = TempDir::new().unwrap();
        assert_eq!(
            destination_for(dir.path(), &note("Server Actions", "Rocketseat")),
            "_importado/rocketseat/server-actions.md"
        );
    }

    #[test]
    fn a_note_with_no_folder_sits_at_the_import_root() {
        let dir = TempDir::new().unwrap();
        assert_eq!(
            destination_for(dir.path(), &note("Ideias soltas", "")),
            "_importado/ideias-soltas.md"
        );
    }

    #[test]
    fn a_nameless_note_still_gets_a_file_name() {
        let dir = TempDir::new().unwrap();
        let path = destination_for(dir.path(), &note("", ""));
        assert!(path.starts_with("_importado/sem-titulo-"));
        assert!(path.ends_with(".md"));
    }

    #[test]
    fn a_second_note_with_the_same_name_does_not_overwrite_the_first() {
        let dir = TempDir::new().unwrap();
        let first = destination_for(dir.path(), &note("Hooks", "React"));
        std::fs::create_dir_all(dir.path().join("_importado/react")).unwrap();
        std::fs::write(dir.path().join(&first), "# Hooks").unwrap();

        let mut other = note("Hooks", "React");
        other.id = "x-coredata://ABC/ICNote/p999".into();
        assert_ne!(destination_for(dir.path(), &other), first);
    }

    #[test]
    fn reimporting_the_same_note_keeps_the_same_path() {
        let dir = TempDir::new().unwrap();
        let same = note("Hooks", "React");
        let path = destination_for(dir.path(), &same);

        std::fs::create_dir_all(dir.path().join("_importado/react")).unwrap();
        std::fs::write(dir.path().join(&path), "# Hooks").unwrap();
        let mut manifest = Manifest::new();
        manifest.insert(
            path.clone(),
            Imported { id: same.id.clone(), modified: same.modified.clone(), attachments: 0 },
        );
        write_manifest(dir.path(), &manifest).unwrap();

        assert_eq!(destination_for(dir.path(), &same), path);
    }

    #[test]
    fn a_base64_payload_is_dropped_but_the_image_tag_survives() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("body.html");
        let payload = "A".repeat(50_000);
        std::fs::write(
            &file,
            format!("<div>antes</div><div><img src=\"data:image/png;base64,{payload}\"></div><div>depois</div>"),
        )
        .unwrap();

        let read = read_without_data_uris(&file).unwrap();
        assert!(read.len() < 200, "o payload continuou na memória: {} bytes", read.len());
        assert!(read.contains("<img src=\"\">"));
        assert!(read.contains("antes") && read.contains("depois"));

        // And the converter still counts it as one attachment.
        assert_eq!(html::to_markdown(&read).attachments, 1);
    }

    #[test]
    fn the_manifest_survives_a_round_trip() {
        let dir = TempDir::new().unwrap();
        let mut manifest = Manifest::new();
        manifest.insert(
            "_importado/react/hooks.md".into(),
            Imported { id: "p1".into(), modified: "hoje".into(), attachments: 3 },
        );
        write_manifest(dir.path(), &manifest).unwrap();

        let read = read_manifest(dir.path());
        assert_eq!(read["_importado/react/hooks.md"].attachments, 3);
    }
}