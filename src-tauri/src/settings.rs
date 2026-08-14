//! What the person left switched on, kept between runs.
//!
//! Three booleans, next to the session token. The folder starts with a dot, so
//! `vault::is_ignored` already keeps it out of the tree and away from the
//! watcher — no new rule needed anywhere.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const DIR: &str = ".server";
const FILE: &str = "settings.json";

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// The server itself. Off means the port is closed, here included.
    pub serving: bool,
    /// Listen on every interface rather than only on this machine.
    pub local_network: bool,
    /// Keep a `tailscale serve` mapping pointing at the app.
    pub tailscale: bool,
}

/// Nothing is exposed until it is asked for once: the first run serves only to
/// this machine, and the switches are remembered from then on.
impl Default for Settings {
    fn default() -> Self {
        Self { serving: true, local_network: false, tailscale: false }
    }
}

pub fn path(root: &Path) -> PathBuf {
    root.join(DIR).join(FILE)
}

pub fn load(root: &Path) -> Settings {
    std::fs::read_to_string(path(root))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

pub fn save(root: &Path, settings: &Settings) -> Result<(), String> {
    let file = path(root);
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let body = serde_json::to_vec_pretty(settings).map_err(|error| error.to_string())?;
    std::fs::write(file, body).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault;
    use tempfile::TempDir;

    #[test]
    fn the_first_run_serves_only_to_this_machine() {
        let settings = Settings::default();
        assert!(settings.serving);
        assert!(!settings.local_network);
        assert!(!settings.tailscale);
    }

    #[test]
    fn what_was_switched_on_survives_a_restart() {
        let dir = TempDir::new().unwrap();
        let wanted = Settings { serving: true, local_network: true, tailscale: true };
        save(dir.path(), &wanted).unwrap();

        let read = load(dir.path());
        assert!(read.local_network && read.tailscale);
    }

    #[test]
    fn a_corrupt_file_falls_back_instead_of_failing_to_start() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(DIR)).unwrap();
        std::fs::write(path(dir.path()), "isto não é json").unwrap();
        assert!(!load(dir.path()).local_network);
    }

    #[test]
    fn the_settings_are_invisible_to_the_vault() {
        let dir = TempDir::new().unwrap();
        assert!(vault::is_ignored(dir.path(), &path(dir.path())));
    }
}