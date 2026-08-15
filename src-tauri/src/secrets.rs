//! Reading API keys without ever handling their value.
//!
//! Nothing here logs, returns, or serialises a key. The only thing that leaves
//! this module for the interface is whether one is configured — `Presence`.

use std::path::Path;

/// Where the installed app looks. An app opened from the Finder inherits no
/// shell environment and has no file beside the `.app`, the same trap that hid
/// the Tailscale binary. Starts with a dot, so `vault::is_ignored` keeps it out
/// of the tree and away from the watcher.
const VAULT_ENV: &str = ".server/env";

/// Reads a key: the process environment first, so a terminal launch uses
/// whatever that terminal has, then the file in the vault.
pub fn key(root: &Path, name: &str) -> Option<String> {
    if let Ok(value) = std::env::var(name) {
        let value = value.trim().to_string();
        if !value.is_empty() {
            return Some(value);
        }
    }
    from_file(root, name)
}

fn from_file(root: &Path, name: &str) -> Option<String> {
    let raw = std::fs::read_to_string(root.join(VAULT_ENV)).ok()?;

    raw.lines()
        .map(str::trim)
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| line.split_once('='))
        .find(|(key, _)| key.trim() == name)
        .map(|(_, value)| unquote(value.trim()))
        .filter(|value| !value.is_empty())
}

/// `KEY="value"` and `KEY=value` both appear in the wild.
fn unquote(value: &str) -> String {
    let trimmed = value
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .or_else(|| value.strip_prefix('\'').and_then(|rest| rest.strip_suffix('\'')));
    trimmed.unwrap_or(value).to_string()
}

/// What the panel is allowed to know. Never the value.
#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Presence {
    pub groq: bool,
    pub gemini: bool,
}

pub fn presence(root: &Path) -> Presence {
    Presence {
        groq: key(root, "GROQ_API_KEY").is_some(),
        gemini: key(root, "GEMINI_API_KEY").is_some(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write(dir: &TempDir, body: &str) {
        let path = dir.path().join(".server");
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("env"), body).unwrap();
    }

    #[test]
    fn reads_a_key_from_the_vault_file() {
        let dir = TempDir::new().unwrap();
        write(&dir, "GROQ_API_KEY=abc123\n");
        assert_eq!(key(dir.path(), "GROQ_API_KEY").as_deref(), Some("abc123"));
    }

    #[test]
    fn quotes_and_spacing_are_tolerated() {
        let dir = TempDir::new().unwrap();
        write(&dir, "  GEMINI_API_KEY = \"xyz\"  \n");
        assert_eq!(key(dir.path(), "GEMINI_API_KEY").as_deref(), Some("xyz"));
    }

    #[test]
    fn comments_and_blanks_are_skipped() {
        let dir = TempDir::new().unwrap();
        write(&dir, "# GROQ_API_KEY=commented\n\nGROQ_API_KEY=real\n");
        assert_eq!(key(dir.path(), "GROQ_API_KEY").as_deref(), Some("real"));
    }

    /// An empty assignment is the shape the example file ships with — it must
    /// read as "not configured", not as a key of zero length.
    #[test]
    fn an_empty_value_counts_as_missing() {
        let dir = TempDir::new().unwrap();
        write(&dir, "GROQ_API_KEY=\n");
        assert!(key(dir.path(), "GROQ_API_KEY").is_none());
    }

    #[test]
    fn no_file_and_no_variable_means_missing() {
        let dir = TempDir::new().unwrap();
        let presence = presence(dir.path());
        // The process may legitimately carry one; the file path is what is
        // under test, so only the absence of a file is asserted here.
        assert!(!presence.groq || std::env::var("GROQ_API_KEY").is_ok());
    }

    /// The file lives where the watcher and the tree already refuse to look.
    #[test]
    fn the_key_file_is_invisible_to_the_vault() {
        let dir = TempDir::new().unwrap();
        assert!(crate::vault::is_ignored(dir.path(), &dir.path().join(VAULT_ENV)));
    }
}
