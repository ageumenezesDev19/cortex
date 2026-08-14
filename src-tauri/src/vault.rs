use std::path::{Component, Path, PathBuf};

use anyhow::{anyhow, Result};
use serde::Serialize;

pub const VAULT_DIR: &str = "Cortex";
pub const INDEX_DIR: &str = ".index";
pub const TOPICS_DIR: &str = ".topics";
pub const ASSETS_DIR: &str = "_assets";

const IMAGE_EXTENSIONS: [&str; 8] = ["png", "jpg", "jpeg", "heic", "heif", "webp", "tiff", "gif"];

pub fn root() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("no home directory"))?;
    Ok(home.join(VAULT_DIR))
}

/// Creates the vault on first run and guarantees `.index/` exists. The index is
/// derived and disposable — deleting it is a supported way to force a rebuild.
pub fn ensure() -> Result<PathBuf> {
    let root = root()?;
    std::fs::create_dir_all(root.join(INDEX_DIR))?;

    let welcome = root.join("bem-vindo.md");
    if !welcome.exists() && is_empty_vault(&root)? {
        std::fs::write(&welcome, WELCOME_NOTE)?;
    }
    Ok(root)
}

fn is_empty_vault(root: &Path) -> Result<bool> {
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        if entry.file_name() != INDEX_DIR {
            return Ok(false);
        }
    }
    Ok(true)
}

pub fn db_path(root: &Path) -> PathBuf {
    root.join(INDEX_DIR).join("cortex.db")
}

/// Dotfiles are someone else's business (`.git`, `.DS_Store`, editor swap
/// files) and `.index/` is ours to rebuild. `.topics/` is the exception: the
/// manifests live in a dot-folder but the watcher has to see them change.
pub fn is_ignored(root: &Path, path: &Path) -> bool {
    let Ok(rel) = path.strip_prefix(root) else {
        return true;
    };
    rel.components().any(|component| {
        component
            .as_os_str()
            .to_str()
            .is_some_and(|name| name.starts_with('.') && name != "." && name != TOPICS_DIR)
    })
}

pub fn is_topic_manifest(root: &Path, path: &Path) -> bool {
    path.strip_prefix(root)
        .is_ok_and(|rel| rel.starts_with(TOPICS_DIR))
}

pub fn is_markdown(path: &Path) -> bool {
    matches!(extension(path).as_deref(), Some("md") | Some("markdown"))
}

pub fn is_image(path: &Path) -> bool {
    extension(path).is_some_and(|ext| IMAGE_EXTENSIONS.contains(&ext.as_str()))
}

fn extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
}

/// Paths are stored relative to the vault root so the whole index survives the
/// vault being moved or synced to another machine.
pub fn relative(root: &Path, path: &Path) -> Result<String> {
    let rel = path.strip_prefix(root)?;
    Ok(rel.to_string_lossy().replace('\\', "/"))
}

pub fn absolute(root: &Path, relative: &str) -> PathBuf {
    root.join(relative)
}

/// The single gate every write goes through. Checked lexically rather than by
/// canonicalising, because the file being created does not exist yet — and a
/// `..` has to be refused before it is ever joined to the root.
pub fn resolve(root: &Path, relative: &str) -> Result<PathBuf> {
    let candidate = Path::new(relative);
    if candidate.is_absolute() {
        return Err(anyhow!("caminho absoluto não é aceito"));
    }
    for component in candidate.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            _ => return Err(anyhow!("caminho sai do vault: {relative}")),
        }
    }
    Ok(root.join(candidate))
}

/// File names have to survive git, the Finder and a URL one day, so accents are
/// folded rather than escaped.
pub fn slugify(name: &str) -> String {
    let mut slug = String::new();
    for character in name.trim().to_lowercase().chars() {
        let folded = fold(character);
        if folded.is_alphanumeric() {
            slug.push(folded);
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    slug.trim_matches('-').to_string()
}

fn fold(character: char) -> char {
    match character {
        'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'ç' => 'c',
        'ñ' => 'n',
        other => other,
    }
}

/// A file name that markdown can point at without ceremony. A space in a link
/// destination ends the URL there unless the whole thing is wrapped in `<>`, so
/// `Screenshot 2026-08-14 at 1.57.12 AM.png` silently breaks every note that
/// embeds it. The extension is left alone; only the name is folded.
pub fn safe_file_name(name: &str) -> String {
    let path = Path::new(name);
    let stem = path.file_stem().map(|s| s.to_string_lossy()).unwrap_or_default();
    let slug = slugify(&stem);
    let slug = if slug.is_empty() { "arquivo".to_string() } else { slug };

    match path.extension().map(|e| e.to_string_lossy().to_lowercase()) {
        Some(extension) => format!("{slug}.{extension}"),
        None => slug,
    }
}

/// A name typed by a person becomes a file name. Separators would silently
/// create folders, and a leading dot would hide the note from the indexer.
pub fn safe_name(name: &str) -> Result<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("o nome não pode ficar vazio"));
    }
    if trimmed.starts_with('.') {
        return Err(anyhow!("o nome não pode começar com ponto"));
    }
    if trimmed.contains(['/', '\\', ':']) {
        return Err(anyhow!("o nome não pode conter / \\ ou :"));
    }
    Ok(trimmed.to_string())
}

#[derive(Debug, Serialize)]
pub struct TreeEntry {
    pub path: String,
    pub name: String,
    /// `folder`, `note` or `image`.
    pub kind: String,
    pub children: Vec<TreeEntry>,
}

pub fn tree(root: &Path) -> Result<Vec<TreeEntry>> {
    read_level(root, root)
}

fn read_level(root: &Path, dir: &Path) -> Result<Vec<TreeEntry>> {
    let mut entries: Vec<TreeEntry> = Vec::new();

    for entry in std::fs::read_dir(dir)?.filter_map(|e| e.ok()) {
        let path = entry.path();
        // `is_ignored` lets `.topics/` through so the watcher can see topics
        // change. The tree is a different question — the app's own folders are
        // not the user's vault, and they have no business being browsable.
        if is_ignored(root, &path) || is_topic_manifest(root, &path) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);

        let kind = if is_dir {
            "folder"
        } else if is_markdown(&path) {
            "note"
        } else if is_image(&path) {
            "image"
        } else {
            // The `.ocr.json` sidecars are ours, not the user's — they belong to
            // the image they sit beside, not to the tree.
            continue;
        };

        entries.push(TreeEntry {
            path: relative(root, &path)?,
            name,
            kind: kind.to_string(),
            children: if is_dir {
                read_level(root, &path)?
            } else {
                Vec::new()
            },
        });
    }

    entries.sort_by(|a, b| match (a.kind.as_str(), b.kind.as_str()) {
        ("folder", "folder") | (_, _) if a.kind == b.kind => {
            a.name.to_lowercase().cmp(&b.name.to_lowercase())
        }
        ("folder", _) => std::cmp::Ordering::Less,
        (_, "folder") => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
    Ok(entries)
}

pub fn folder_of(relative: &str) -> String {
    match relative.rsplit_once('/') {
        Some((folder, _)) => folder.to_string(),
        None => String::new(),
    }
}

pub fn mtime_of(path: &Path) -> i64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

const WELCOME_NOTE: &str = r#"# Bem-vindo ao Cortex

Este é o seu vault. Tudo aqui é markdown em disco — você pode editar no VS Code,
versionar no git, ou abrir no Obsidian. O Cortex indexa sozinho.

## Como usar

Crie pastas por curso ou stack, escreva `.md` dentro delas, e jogue os prints numa
subpasta `_assets/`. Cada print novo passa por OCR automaticamente e vira texto
pesquisável — buscar por uma palavra que só existe dentro da imagem funciona.

## Onde fica o índice

Em `.index/cortex.db`. Ele é derivado e descartável: apagar a pasta faz o app
reconstruir tudo na próxima abertura.
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        PathBuf::from("/tmp/vault")
    }

    #[test]
    fn refuses_paths_that_climb_out_of_the_vault() {
        assert!(resolve(&root(), "../fora.md").is_err());
        assert!(resolve(&root(), "notas/../../fora.md").is_err());
        assert!(resolve(&root(), "/etc/passwd").is_err());
    }

    #[test]
    fn accepts_ordinary_paths_inside_the_vault() {
        assert_eq!(
            resolve(&root(), "react/hooks.md").unwrap(),
            root().join("react/hooks.md")
        );
    }

    #[test]
    fn rejects_names_that_would_become_folders_or_hide_the_file() {
        assert!(safe_name("react/hooks").is_err());
        assert!(safe_name(".oculta").is_err());
        assert!(safe_name("   ").is_err());
        assert_eq!(safe_name("  Server Actions  ").unwrap(), "Server Actions");
    }

    #[test]
    fn the_tree_never_shows_the_app_its_own_folders() {
        let dir = tempfile::TempDir::new().unwrap();
        for folder in [".index", ".topics", "react"] {
            std::fs::create_dir_all(dir.path().join(folder)).unwrap();
        }
        std::fs::write(dir.path().join(".topics/react.json"), "{}").unwrap();
        std::fs::write(dir.path().join("react/hooks.md"), "# Hooks").unwrap();
        std::fs::write(dir.path().join("react/print.png"), b"x").unwrap();
        std::fs::write(dir.path().join("react/print.ocr.json"), "{}").unwrap();

        let names: Vec<String> = tree(dir.path()).unwrap().into_iter().map(|e| e.name).collect();
        assert_eq!(names, ["react"], "vazou pasta interna para a árvore");

        let inside: Vec<String> = tree(dir.path()).unwrap()[0]
            .children
            .iter()
            .map(|e| e.name.clone())
            .collect();
        // The sidecar belongs to the image, not to the person browsing.
        assert_eq!(inside, ["hooks.md", "print.png"]);
    }

    #[test]
    fn folds_accents_and_collapses_separators() {
        assert_eq!(slugify("Programação   Funcional"), "programacao-funcional");
        assert_eq!(slugify("Next.js"), "next-js");
        assert_eq!(slugify("!!! C# !!!"), "c");
    }

    #[test]
    fn a_screenshot_name_loses_its_spaces_but_keeps_its_extension() {
        assert_eq!(
            safe_file_name("Screenshot 2026-08-14 at 1.57.12 AM.png"),
            "screenshot-2026-08-14-at-1-57-12-am.png"
        );
        assert_eq!(safe_file_name("WhatsApp Image.JPEG"), "whatsapp-image.jpeg");
        assert_eq!(safe_file_name("ja-seguro.png"), "ja-seguro.png");
        assert_eq!(safe_file_name("!!!.png"), "arquivo.png");
    }
}
