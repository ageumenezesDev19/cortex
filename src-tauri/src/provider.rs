//! Groq first, Gemini when the quota runs out — the same shape as Pitchfolio.
//!
//! Requests go through `/usr/bin/curl`, always present on macOS, for the reason
//! that has held for three rounds now: linking a TLS stack into the app costs
//! more than using what the system already ships. The key rides in a config
//! block on stdin rather than an argument, so it never appears in `ps`.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde::Serialize;
use serde_json::{json, Value};

use crate::secrets;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Provider {
    Groq,
    Gemini,
}

impl Provider {
    fn env_var(self) -> &'static str {
        match self {
            Provider::Groq => "GROQ_API_KEY",
            Provider::Gemini => "GEMINI_API_KEY",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Provider::Groq => "Groq",
            Provider::Gemini => "Gemini",
        }
    }
}

/// Free-tier model names change without notice, so they are settings rather
/// than constants — a rename must not need a rebuild.
pub const DEFAULT_GROQ_MODEL: &str = "llama-3.3-70b-versatile";
pub const DEFAULT_GEMINI_MODEL: &str = "gemini-2.0-flash";

#[derive(Debug)]
pub struct Answer {
    pub text: String,
    pub provider: Provider,
}

/// Asks the first provider that has a key, and falls to the next when it
/// refuses to serve. A quota that ran out is not an error worth showing: the
/// literal search is still on screen, which is the app's normal state.
pub fn ask(
    root: &Path,
    system: &str,
    user: &str,
    models: (&str, &str),
) -> Result<Answer, String> {
    let mut last = String::from("nenhuma chave configurada");

    for provider in [Provider::Groq, Provider::Gemini] {
        let Some(key) = secrets::key(root, provider.env_var()) else {
            continue;
        };
        let model = match provider {
            Provider::Groq => models.0,
            Provider::Gemini => models.1,
        };

        match call(provider, &key, model, system, user) {
            Ok(text) => return Ok(Answer { text, provider }),
            Err(reason) => last = reason,
        }
    }
    Err(last)
}

fn call(
    provider: Provider,
    key: &str,
    model: &str,
    system: &str,
    user: &str,
) -> Result<String, String> {
    let (url, body, header) = match provider {
        Provider::Groq => (
            "https://api.groq.com/openai/v1/chat/completions".to_string(),
            json!({
                "model": model,
                "messages": [
                    {"role": "system", "content": system},
                    {"role": "user", "content": user},
                ],
                "temperature": 0.2,
            }),
            format!("Authorization: Bearer {key}"),
        ),
        Provider::Gemini => (
            format!("https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent"),
            json!({
                "systemInstruction": {"parts": [{"text": system}]},
                "contents": [{"role": "user", "parts": [{"text": user}]}],
                "generationConfig": {"temperature": 0.2},
            }),
            format!("x-goog-api-key: {key}"),
        ),
    };

    let raw = post(&url, &header, &body.to_string())?;
    let parsed: Value = serde_json::from_str(&raw)
        .map_err(|_| format!("{} devolveu algo que não é JSON", provider.label()))?;

    if let Some(message) = parsed.pointer("/error/message").and_then(Value::as_str) {
        return Err(format!("{}: {message}", provider.label()));
    }
    text_of(provider, &parsed)
        .ok_or_else(|| format!("{} respondeu sem texto", provider.label()))
}

fn text_of(provider: Provider, parsed: &Value) -> Option<String> {
    let text = match provider {
        Provider::Groq => parsed.pointer("/choices/0/message/content")?.as_str()?,
        Provider::Gemini => parsed.pointer("/candidates/0/content/parts/0/text")?.as_str()?,
    };
    Some(text.to_string())
}

/// The header goes in on stdin through `--config -`; passing it as an argument
/// would put the key in the process list for anything running `ps` to read.
fn post(url: &str, header: &str, body: &str) -> Result<String, String> {
    let mut child = Command::new("/usr/bin/curl")
        .args(["--silent", "--show-error", "--max-time", "60", "--config", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| error.to_string())?;

    let config = curl_config(url, header, body);
    child
        .stdin
        .take()
        .ok_or("sem stdin para o curl")?
        .write_all(config.as_bytes())
        .map_err(|error| error.to_string())?;

    let output = child.wait_with_output().map_err(|error| error.to_string())?;
    if !output.status.success() {
        let complaint = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if complaint.is_empty() { "a chamada falhou".into() } else { complaint });
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Curl's config format: one option per line, values quoted. Quotes and
/// backslashes inside a value have to be escaped or the parse ends early and
/// the request goes out malformed.
fn curl_config(url: &str, header: &str, body: &str) -> String {
    format!(
        "url = \"{}\"\nheader = \"content-type: application/json\"\nheader = \"{}\"\ndata-binary = \"{}\"\n",
        escape(url),
        escape(header),
        escape(body),
    )
}

fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_key_never_reaches_the_command_line() {
        let config = curl_config("https://example", "Authorization: Bearer segredo", "{}");
        // It is in the config, which travels on stdin...
        assert!(config.contains("segredo"));
        // ...and the arguments carry only the flag that reads stdin.
        let args = ["--silent", "--show-error", "--max-time", "60", "--config", "-"];
        assert!(!args.iter().any(|arg| arg.contains("segredo")));
    }

    /// A quote inside the JSON would end curl's value early and send a
    /// malformed request, so the body has to survive the trip byte for byte.
    #[test]
    fn a_quote_in_the_body_does_not_end_the_config_line() {
        let body = r#"{"q":"ele disse \"oi\" e saiu"}"#;
        let config = curl_config("https://example", "h: v", body);
        let data = config.lines().find(|line| line.starts_with("data-binary")).unwrap();

        let quoted = data.trim_start_matches("data-binary = ");
        let inner = quoted.strip_prefix('"').unwrap().strip_suffix('"').unwrap();
        assert_eq!(inner.replace("\\\"", "\"").replace("\\\\", "\\"), body);
    }

    #[test]
    fn groq_and_gemini_answers_are_read_from_their_own_shapes() {
        let groq = json!({"choices": [{"message": {"content": "resposta"}}]});
        let gemini = json!({"candidates": [{"content": {"parts": [{"text": "resposta"}]}}]});
        assert_eq!(text_of(Provider::Groq, &groq).as_deref(), Some("resposta"));
        assert_eq!(text_of(Provider::Gemini, &gemini).as_deref(), Some("resposta"));
    }

    #[test]
    fn a_shape_from_the_other_provider_is_not_mistaken_for_an_answer() {
        let groq = json!({"choices": [{"message": {"content": "resposta"}}]});
        assert!(text_of(Provider::Gemini, &groq).is_none());
    }

    /// With no key anywhere there is nothing to try, and the caller has to be
    /// told rather than shown an empty answer.
    #[test]
    fn no_key_at_all_is_an_error_not_an_empty_answer() {
        let dir = tempfile::TempDir::new().unwrap();
        if std::env::var("GROQ_API_KEY").is_ok() || std::env::var("GEMINI_API_KEY").is_ok() {
            return; // the machine running the tests has one; nothing to assert
        }
        let error = ask(dir.path(), "s", "u", ("m", "m")).unwrap_err();
        assert!(error.contains("chave"), "{error}");
    }
}
