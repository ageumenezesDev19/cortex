//! Reading API keys without ever handling their value.
//!
//! Nothing here logs, returns, or serialises a key. The only thing that leaves
//! this module for the interface is whether one is configured — `Presence`.

use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// Where the installed app looks. An app opened from the Finder inherits no
/// shell environment and has no file beside the `.app`, the same trap that hid
/// the Tailscale binary. Starts with a dot, so `vault::is_ignored` keeps it out
/// of the tree, `vault::resolve` refuses to name it, and neither the watcher nor
/// `/asset/<path>` can reach it.
const VAULT_ENV: &str = ".server/env";

/// The only names that may be written. A caller cannot use `store` to put an
/// arbitrary line into the file.
pub const NAMES: [&str; 2] = ["GROQ_API_KEY", "GEMINI_API_KEY"];

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

pub fn path(root: &Path) -> PathBuf {
    root.join(VAULT_ENV)
}

/// Which of the two places the key in use came from. The panel needs this
/// because `key()` reads the environment first: a key typed in the app sits
/// behind an exported variable, and that has to be said out loud rather than
/// look like the app forgot what was typed.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    Environment,
    App,
}

pub fn source(root: &Path, name: &str) -> Option<Source> {
    if std::env::var(name).is_ok_and(|value| !value.trim().is_empty()) {
        return Some(Source::Environment);
    }
    from_file(root, name).map(|_| Source::App)
}

/// What the panel is allowed to know: whether a key is there and where it came
/// from. Never the value.
#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Presence {
    pub groq: Option<Source>,
    pub gemini: Option<Source>,
}

pub fn presence(root: &Path) -> Presence {
    Presence { groq: source(root, "GROQ_API_KEY"), gemini: source(root, "GEMINI_API_KEY") }
}

/// Writes a key into the vault file, or removes it when given nothing.
///
/// Every other line survives: the file is hand-editable and may carry comments
/// or variables this app knows nothing about. The value never passes through a
/// command line, a log, or a return value.
pub fn store(root: &Path, name: &str, value: Option<&str>) -> Result<(), String> {
    if !NAMES.contains(&name) {
        return Err(format!("chave desconhecida: {name}"));
    }

    let file = path(root);
    let previous = std::fs::read_to_string(&file).unwrap_or_default();
    let mut lines: Vec<String> =
        previous.lines().filter(|line| !assigns(line, name)).map(str::to_string).collect();

    if let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) {
        // A newline in the value would write a second assignment, which is how
        // a paste with a stray line break becomes a file nobody can explain.
        if value.chars().any(char::is_control) {
            return Err("a chave tem caracteres que não cabem em uma linha".into());
        }
        lines.push(format!("{name}={value}"));
    }

    write_private(&file, &lines)
}

/// `KEY=value` for this key, and not a comment about it.
fn assigns(line: &str, name: &str) -> bool {
    let line = line.trim();
    !line.starts_with('#') && line.split_once('=').is_some_and(|(key, _)| key.trim() == name)
}

/// Created readable only by this user — and the mode is set again afterwards,
/// because `mode` on the open only applies to a file that did not exist yet.
fn write_private(file: &Path, lines: &[String]) -> Result<(), String> {
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    let mut handle = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(file)
        .map_err(|error| error.to_string())?;

    let mut body = lines.join("\n");
    if !body.is_empty() {
        body.push('\n');
    }
    handle.write_all(body.as_bytes()).map_err(|error| error.to_string())?;

    std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o600))
        .map_err(|error| error.to_string())
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
        assert!(presence.groq.is_none() || std::env::var("GROQ_API_KEY").is_ok());
    }

    /// The file lives where the watcher and the tree already refuse to look.
    #[test]
    fn the_key_file_is_invisible_to_the_vault() {
        let dir = TempDir::new().unwrap();
        assert!(crate::vault::is_ignored(dir.path(), &dir.path().join(VAULT_ENV)));
        assert!(crate::vault::resolve(dir.path(), VAULT_ENV).is_err());
    }

    #[test]
    fn a_stored_key_reads_back() {
        let dir = TempDir::new().unwrap();
        store(dir.path(), "GROQ_API_KEY", Some("gsk_exemplo")).unwrap();
        assert_eq!(from_file(dir.path(), "GROQ_API_KEY").as_deref(), Some("gsk_exemplo"));
        assert_eq!(source(dir.path(), "GEMINI_API_KEY"), None);
    }

    /// The file is hand-editable, and a person's comments and unrelated
    /// variables must survive the app writing to it.
    #[test]
    fn storing_keeps_every_other_line() {
        let dir = TempDir::new().unwrap();
        write(&dir, "# minhas chaves\nOUTRA=coisa\nGROQ_API_KEY=antiga\n");
        store(dir.path(), "GROQ_API_KEY", Some("nova")).unwrap();

        let raw = std::fs::read_to_string(path(dir.path())).unwrap();
        assert!(raw.contains("# minhas chaves"));
        assert!(raw.contains("OUTRA=coisa"));
        assert!(!raw.contains("antiga"));
        assert_eq!(from_file(dir.path(), "GROQ_API_KEY").as_deref(), Some("nova"));
    }

    #[test]
    fn removing_a_key_leaves_nothing_behind() {
        let dir = TempDir::new().unwrap();
        store(dir.path(), "GEMINI_API_KEY", Some("segredo")).unwrap();
        store(dir.path(), "GEMINI_API_KEY", None).unwrap();

        let raw = std::fs::read_to_string(path(dir.path())).unwrap();
        assert!(!raw.contains("segredo"));
        assert!(from_file(dir.path(), "GEMINI_API_KEY").is_none());
    }

    #[test]
    fn the_file_is_readable_only_by_its_owner() {
        let dir = TempDir::new().unwrap();
        // Both paths to the file: created by the store, and pre-existing with
        // the permissions a hand-written file would have.
        store(dir.path(), "GROQ_API_KEY", Some("a")).unwrap();
        assert_eq!(mode(dir.path()), 0o600);

        std::fs::set_permissions(path(dir.path()), std::fs::Permissions::from_mode(0o644)).unwrap();
        store(dir.path(), "GROQ_API_KEY", Some("b")).unwrap();
        assert_eq!(mode(dir.path()), 0o600);
    }

    fn mode(root: &Path) -> u32 {
        std::fs::metadata(path(root)).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn a_name_the_app_does_not_own_is_refused() {
        let dir = TempDir::new().unwrap();
        assert!(store(dir.path(), "PATH", Some("/etc")).is_err());
    }

    /// A key pasted with a line break would otherwise write a second assignment.
    #[test]
    fn a_value_with_a_line_break_is_refused() {
        let dir = TempDir::new().unwrap();
        assert!(store(dir.path(), "GROQ_API_KEY", Some("abc\nOUTRA=coisa")).is_err());
    }
}
