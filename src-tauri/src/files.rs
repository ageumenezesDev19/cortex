use std::path::Path;

use anyhow::{anyhow, Result};

use crate::{ocr, vault};

/// Creates a note and returns its path relative to the vault. The title becomes
/// both the file name (slugged) and the first heading, which is the shape every
/// note already in the vault has.
pub fn create_note(root: &Path, folder: &str, title: &str) -> Result<String> {
    let title = vault::safe_name(title)?;
    let slug = vault::slugify(&title);
    if slug.is_empty() {
        return Err(anyhow!("esse nome não vira um arquivo"));
    }

    let folder_path = vault::resolve(root, folder)?;
    std::fs::create_dir_all(&folder_path)?;

    let mut destination = folder_path.join(format!("{slug}.md"));
    let mut attempt = 1;
    while destination.exists() {
        destination = folder_path.join(format!("{slug}-{attempt}.md"));
        attempt += 1;
    }

    std::fs::write(&destination, format!("# {title}\n\n"))?;
    vault::relative(root, &destination)
}

pub fn write_note(root: &Path, path: &str, source: &str) -> Result<()> {
    let abs = vault::resolve(root, path)?;
    if !vault::is_markdown(&abs) {
        return Err(anyhow!("só notas markdown podem ser escritas"));
    }
    std::fs::write(abs, source)?;
    Ok(())
}

pub fn create_folder(root: &Path, parent: &str, name: &str) -> Result<String> {
    let name = vault::safe_name(name)?;
    let destination = vault::resolve(root, parent)?.join(vault::slugify(&name));

    if destination.exists() {
        return Err(anyhow!("já existe algo chamado {name} aqui"));
    }
    std::fs::create_dir_all(&destination)?;
    vault::relative(root, &destination)
}

/// Returns the new relative path. Topic membership is the caller's problem —
/// see `topics::retarget`.
pub fn rename(root: &Path, path: &str, name: &str) -> Result<String> {
    let name = vault::safe_name(name)?;
    let abs = vault::resolve(root, path)?;
    let parent = abs.parent().ok_or_else(|| anyhow!("não dá para renomear a raiz"))?;

    // A note keeps whatever extension it had; a folder has none to keep.
    let file_name = if abs.is_dir() {
        vault::slugify(&name)
    } else {
        let extension = abs
            .extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_else(|| "md".into());
        format!("{}.{extension}", vault::slugify(&name))
    };

    let destination = parent.join(&file_name);
    if destination.exists() {
        return Err(anyhow!("já existe algo chamado {file_name} aqui"));
    }
    std::fs::rename(&abs, &destination)?;
    vault::relative(root, &destination)
}

pub fn delete(root: &Path, path: &str) -> Result<()> {
    let abs = vault::resolve(root, path)?;
    if abs == root {
        return Err(anyhow!("não dá para apagar a raiz do vault"));
    }

    if abs.is_dir() {
        std::fs::remove_dir_all(&abs)?;
    } else {
        std::fs::remove_file(&abs)?;
        // The OCR sidecar belongs to the image, so it leaves with it.
        let sidecar = ocr::json_path(&abs);
        if sidecar.exists() {
            std::fs::remove_file(sidecar)?;
        }
    }
    Ok(())
}

/// Writes a pasted screenshot beside its note and returns the markdown to
/// insert. OCR is not this function's business — the watcher picks the file up.
pub fn paste_image(root: &Path, note_path: &str, bytes: &[u8], extension: &str) -> Result<String> {
    let note = vault::resolve(root, note_path)?;
    let assets = note
        .parent()
        .ok_or_else(|| anyhow!("nota sem pasta"))?
        .join(vault::ASSETS_DIR);
    std::fs::create_dir_all(&assets)?;

    let extension = match extension.to_lowercase().as_str() {
        "jpeg" | "jpg" => "jpg",
        "webp" => "webp",
        _ => "png",
    };
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);

    let name = format!("print-{stamp}.{extension}");
    std::fs::write(assets.join(&name), bytes)?;
    Ok(format!("![]({}/{name})", vault::ASSETS_DIR))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn a_new_note_is_slugged_and_carries_its_title() {
        let dir = TempDir::new().unwrap();
        let path = create_note(dir.path(), "rocketseat", "Server Actions").unwrap();

        assert_eq!(path, "rocketseat/server-actions.md");
        let source = std::fs::read_to_string(dir.path().join(&path)).unwrap();
        assert!(source.starts_with("# Server Actions\n"));
    }

    #[test]
    fn a_second_note_with_the_same_title_does_not_overwrite_the_first() {
        let dir = TempDir::new().unwrap();
        let first = create_note(dir.path(), "", "Hooks").unwrap();
        let second = create_note(dir.path(), "", "Hooks").unwrap();

        assert_eq!(first, "hooks.md");
        assert_eq!(second, "hooks-1.md");
    }

    #[test]
    fn renaming_a_note_keeps_its_extension() {
        let dir = TempDir::new().unwrap();
        let path = create_note(dir.path(), "react", "Hooks").unwrap();
        let renamed = rename(dir.path(), &path, "Ganchos e Efeitos").unwrap();

        assert_eq!(renamed, "react/ganchos-e-efeitos.md");
        assert!(dir.path().join(&renamed).exists());
        assert!(!dir.path().join(&path).exists());
    }

    #[test]
    fn renaming_a_folder_adds_no_extension() {
        let dir = TempDir::new().unwrap();
        create_folder(dir.path(), "", "Trybe").unwrap();

        assert_eq!(rename(dir.path(), "trybe", "Cursos").unwrap(), "cursos");
    }

    #[test]
    fn renaming_onto_an_existing_name_is_refused() {
        let dir = TempDir::new().unwrap();
        create_note(dir.path(), "", "Hooks").unwrap();
        let other = create_note(dir.path(), "", "Estado").unwrap();

        assert!(rename(dir.path(), &other, "Hooks").is_err());
        assert!(dir.path().join("estado.md").exists());
    }

    #[test]
    fn deleting_an_image_takes_its_ocr_sidecar_along() {
        let dir = TempDir::new().unwrap();
        let assets = dir.path().join("react").join(vault::ASSETS_DIR);
        std::fs::create_dir_all(&assets).unwrap();
        std::fs::write(assets.join("print.png"), b"x").unwrap();
        std::fs::write(assets.join("print.ocr.json"), b"{}").unwrap();

        delete(dir.path(), "react/_assets/print.png").unwrap();

        assert!(!assets.join("print.png").exists());
        assert!(!assets.join("print.ocr.json").exists(), "sidecar ficou órfão");
    }

    #[test]
    fn a_pasted_image_lands_beside_its_note() {
        let dir = TempDir::new().unwrap();
        let note = create_note(dir.path(), "react", "Hooks").unwrap();
        let markdown = paste_image(dir.path(), &note, b"bytes", "png").unwrap();

        assert!(markdown.starts_with("![](_assets/print-"));
        assert!(markdown.ends_with(".png)"));
        assert_eq!(
            std::fs::read_dir(dir.path().join("react").join(vault::ASSETS_DIR))
                .unwrap()
                .count(),
            1
        );
    }

    #[test]
    fn writing_refuses_anything_that_is_not_a_note() {
        let dir = TempDir::new().unwrap();
        assert!(write_note(dir.path(), "config.json", "{}").is_err());
        assert!(write_note(dir.path(), "../fora.md", "oi").is_err());
    }
}