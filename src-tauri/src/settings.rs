//! What the person left switched on, kept between runs.
//!
//! Three booleans, next to the session token. The folder starts with a dot, so
//! `vault::is_ignored` already keeps it out of the tree and away from the
//! watcher — no new rule needed anywhere.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const DIR: &str = ".server";
const FILE: &str = "settings.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// The server itself. Off means the port is closed, here included.
    pub serving: bool,
    /// Listen on every interface rather than only on this machine.
    pub local_network: bool,
    /// Keep a `tailscale serve` mapping pointing at the app.
    pub tailscale: bool,
    /// Path prefixes a question may never carry to a provider. Empty by
    /// default — the person decides what is private, not the app.
    #[serde(default)]
    pub excluded: Vec<String>,
    /// Free-tier model names change without notice; a rename must not need a
    /// rebuild, so they live here rather than in the code.
    #[serde(default = "default_groq_model")]
    pub groq_model: String,
    #[serde(default = "default_gemini_model")]
    pub gemini_model: String,
    /// Turning a question into search terms is keyword work, and the flash-lite
    /// tier carries 500 requests a day against the 20 of the model that writes
    /// the answer. Spending the scarce one on that would be spending it wrong.
    #[serde(default = "default_gemini_expand_model")]
    pub gemini_expand_model: String,
    /// Each Gemini model has its own daily bucket — 20 for the flash ones, 500
    /// for the lite — so an exhausted model is a reason to try the next, not to
    /// give up on the provider.
    #[serde(default = "default_gemini_ladder")]
    pub gemini_ladder: Vec<String>,
    /// Gemini sends no rate-limit header, so the total has to be written down.
    /// 20 a day is what the free tier shows for the flash models.
    #[serde(default = "default_gemini_daily_limit")]
    pub gemini_daily_limit: u32,
    /// Searching by meaning runs on this machine, with no provider and no key.
    /// Only one model is valid — the stored vectors are its width — but the
    /// name is kept so a future change of model has somewhere to be recorded.
    #[serde(default = "default_embedding_model")]
    pub embedding_model: String,
    /// Vectors are a derived index, like a screenshot's OCR and the full-text
    /// index — both of which the app keeps current without being asked. Nothing
    /// in the vault is rewritten, so the rule about not touching your notes
    /// unbidden does not reach this.
    #[serde(default = "yes")]
    pub auto_embed: bool,
}

impl Settings {
    /// Replaces model names the provider has retired.
    ///
    /// Without this, changing a default fixes nothing for anyone who already
    /// ran the app: `serde(default)` only fills a field that is *missing*, and
    /// a saved settings file has the dead name written in it. Healing on read
    /// rather than migrating on write means it also fixes a file someone
    /// restored from a backup.
    fn heal(&mut self) {
        for model in [
            &mut self.groq_model,
            &mut self.gemini_model,
            &mut self.gemini_expand_model,
            &mut self.embedding_model,
        ] {
            if crate::provider::RETIRED_MODELS.contains(&model.as_str()) {
                *model = String::new();
            }
        }
        if self.groq_model.is_empty() {
            self.groq_model = default_groq_model();
        }
        if self.gemini_model.is_empty() {
            self.gemini_model = default_gemini_model();
        }
        if self.gemini_expand_model.is_empty() {
            self.gemini_expand_model = default_gemini_expand_model();
        }
        if self.embedding_model.is_empty() {
            self.embedding_model = default_embedding_model();
        }
    }
}

fn default_groq_model() -> String {
    crate::provider::DEFAULT_GROQ_MODEL.to_string()
}

fn default_gemini_model() -> String {
    crate::provider::DEFAULT_GEMINI_MODEL.to_string()
}

fn default_gemini_expand_model() -> String {
    crate::provider::DEFAULT_GEMINI_EXPAND_MODEL.to_string()
}

fn default_gemini_daily_limit() -> u32 {
    20
}

fn yes() -> bool {
    true
}

fn default_embedding_model() -> String {
    "multilingual-e5-small".to_string()
}

fn default_gemini_ladder() -> Vec<String> {
    ["gemini-flash-latest", "gemini-3.6-flash", "gemini-3.5-flash", "gemini-flash-lite-latest"]
        .iter()
        .map(|model| model.to_string())
        .collect()
}

/// Nothing is exposed until it is asked for once: the first run serves only to
/// this machine, and the switches are remembered from then on.
impl Default for Settings {
    fn default() -> Self {
        Self {
            serving: true,
            local_network: false,
            tailscale: false,
            excluded: Vec::new(),
            groq_model: default_groq_model(),
            gemini_model: default_gemini_model(),
            gemini_expand_model: default_gemini_expand_model(),
            gemini_ladder: default_gemini_ladder(),
            gemini_daily_limit: default_gemini_daily_limit(),
            embedding_model: default_embedding_model(),
            auto_embed: true,
        }
    }
}

pub fn path(root: &Path) -> PathBuf {
    root.join(DIR).join(FILE)
}

pub fn load(root: &Path) -> Settings {
    let mut settings: Settings = std::fs::read_to_string(path(root))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default();
    settings.heal();
    settings
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
        let wanted = Settings { local_network: true, tailscale: true, ..Settings::default() };
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

    /// The case that actually bit: an install from before the model was retired
    /// keeps working after an update, without anyone editing a file by hand.
    #[test]
    fn a_retired_model_is_replaced_when_the_settings_are_read() {
        let dir = TempDir::new().unwrap();
        let mut stored = Settings::default();
        stored.groq_model = "llama-3.3-70b-versatile".to_string();
        save(dir.path(), &stored).unwrap();

        let read = load(dir.path());

        assert_eq!(read.groq_model, crate::provider::DEFAULT_GROQ_MODEL);
        assert!(!crate::provider::RETIRED_MODELS.contains(&read.groq_model.as_str()));
    }

    /// A name nobody has retired is left alone: healing must not undo a choice.
    #[test]
    fn a_model_the_user_picked_survives_the_reading() {
        let dir = TempDir::new().unwrap();
        let mut stored = Settings::default();
        stored.groq_model = "openai/gpt-oss-20b".to_string();
        save(dir.path(), &stored).unwrap();

        assert_eq!(load(dir.path()).groq_model, "openai/gpt-oss-20b");
    }
}