use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

pub const VAULT_DIR: &str = "Cortex";
pub const INDEX_DIR: &str = ".index";
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

/// Everything under `.index/` is ours, and dotfiles are someone else's business
/// (`.git`, `.DS_Store`, editor swap files).
pub fn is_ignored(root: &Path, path: &Path) -> bool {
    let Ok(rel) = path.strip_prefix(root) else {
        return true;
    };
    rel.components().any(|c| {
        c.as_os_str()
            .to_str()
            .is_some_and(|s| s.starts_with('.') && s != ".")
    })
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
