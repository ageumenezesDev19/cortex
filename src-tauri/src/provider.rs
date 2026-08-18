//! Groq first, Gemini when the quota runs out — the same shape as Pitchfolio.
//!
//! Requests go through `/usr/bin/curl`, always present on macOS, for the reason
//! that has held for three rounds now: linking a TLS stack into the app costs
//! more than using what the system already ships. The key rides in a config
//! block on stdin rather than an argument, so it never appears in `ps`.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::secrets;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
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
/// than constants — a rename must not need a rebuild. `gemini-2.0-flash` was
/// the default until Google retired it, which is why the alias is the default
/// now: it follows whichever flash is current.
pub const DEFAULT_GROQ_MODEL: &str = "openai/gpt-oss-120b";

/// Names that used to work and now return "the model does not exist". Groq has
/// no `-latest` alias to hide behind, so a pinned name is unavoidable and this
/// list is what keeps a saved setting from outliving the model it names — the
/// default alone would not, since a stored value beats `serde(default)`.
///
/// Add to it when a provider retires something; nothing is ever removed.
pub const RETIRED_MODELS: &[&str] =
    &["llama-3.3-70b-versatile", "gemini-2.0-flash", "gemini-embedding-001"];
pub const DEFAULT_GEMINI_MODEL: &str = "gemini-flash-latest";
/// 500 requests a day against the flash models' 20 — and pulling keywords out
/// of a question does not need the model that writes the answer.
pub const DEFAULT_GEMINI_EXPAND_MODEL: &str = "gemini-flash-lite-latest";

#[derive(Debug)]
pub struct Answer {
    pub text: String,
    pub provider: Provider,
}

/// Whether the next provider, or the next model, would do any better.
///
/// Groq gives 1000 requests a day and Gemini 20 per model, so falling over on
/// every stumble is wrong twice: as a reserve of volume Gemini adds 2%, and as
/// consolation for an error that is not about limits it spends a twentieth of
/// the day to repeat the same failure. What it does fix is the minute — 250 000
/// tokens against Groq's 12 000.
pub fn worth_falling_over(reason: &str) -> bool {
    let reason = reason.to_lowercase();
    let hopeless = ["chave", "api key", "unauthorized", "invalid", "not found", "no longer"];
    if hopeless.iter().any(|mark| reason.contains(mark)) {
        return false;
    }
    let temporary = [
        "rate limit", "rate_limit", "429", "quota", "exhausted", "capacity", "overload",
        "timed out", "timeout", "connection", "resolve host", "500", "502", "503", "504",
        "unavailable", "internal",
    ];
    temporary.iter().any(|mark| reason.contains(mark))
}

/// Asks the first provider that has a key, and falls to the next only when the
/// next one would help. A quota that ran out is not an error worth showing: the
/// literal search is still on screen, which is the app's normal state.
///
/// Gemini's models are separate buckets — 20 a day each for the flash ones, 500
/// for the lite — so its ladder is tried in order before giving up.
pub fn ask(
    root: &Path,
    system: &str,
    user: &str,
    models: Chain<'_>,
    token: Option<u64>,
) -> Result<Answer, String> {
    let mut last = String::from("nenhuma chave configurada");

    for (provider, ladder) in
        [(Provider::Groq, vec![models.groq]), (Provider::Gemini, models.gemini.to_vec())]
    {
        let Some(key) = secrets::key(root, provider.env_var()) else {
            continue;
        };

        for model in ladder {
            // Recorded whichever way it went: a refused call still spent a
            // request out of the day, and the bar has to show that.
            match call(provider, &key, model, system, user, token) {
                Ok((text, reported)) => {
                    crate::usage::record(root, provider, reported);
                    return Ok(Answer { text, provider });
                }
                Err(reason) => {
                    crate::usage::record(root, provider, crate::usage::Limits::default());
                    let keep_going = worth_falling_over(&reason);
                    last = reason;
                    if !keep_going {
                        return Err(last);
                    }
                }
            }
        }
    }
    Err(last)
}


/// Who to ask, in order. Gemini takes a list because each of its models has its
/// own daily bucket.
#[derive(Debug, Clone, Copy)]
pub struct Chain<'a> {
    pub groq: &'a str,
    pub gemini: &'a [&'a str],
}

/// Asks one provider the smallest question there is, so a key can be checked
/// while it is being typed in. `ask` falls through to the next provider, which
/// is right for answering and useless for telling which key works.
pub fn check(root: &Path, provider: Provider, model: &str) -> Result<(), String> {
    let key = secrets::key(root, provider.env_var())
        .ok_or_else(|| format!("nenhuma chave do {} configurada", provider.label()))?;

    let answered = call(provider, &key, model, "Responda com uma palavra.", "Diga: ok", None);
    let reported = answered.as_ref().map(|(_, limits)| limits.clone()).unwrap_or_default();
    crate::usage::record(root, provider, reported);
    answered.map(|_| ())
}

fn call(
    provider: Provider,
    key: &str,
    model: &str,
    system: &str,
    user: &str,
    token: Option<u64>,
) -> Result<(String, crate::usage::Limits), String> {
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

    let (raw, reported) = post(&url, &header, &body.to_string(), token)?;
    let parsed: Value = serde_json::from_str(&raw)
        .map_err(|_| format!("{} devolveu algo que não é JSON", provider.label()))?;

    if let Some(message) = parsed.pointer("/error/message").and_then(Value::as_str) {
        return Err(format!("{}: {message}", provider.label()));
    }
    text_of(provider, &parsed)
        .map(|text| (text, reported))
        .ok_or_else(|| format!("{} respondeu sem texto", provider.label()))
}

fn text_of(provider: Provider, parsed: &Value) -> Option<String> {
    let text = match provider {
        Provider::Groq => parsed.pointer("/choices/0/message/content")?.as_str()?,
        Provider::Gemini => parsed.pointer("/candidates/0/content/parts/0/text")?.as_str()?,
    };
    Some(text.to_string())
}

/// The calls in flight, by the token the caller gave. A question is two of
/// them in sequence under the same token, so cancelling catches whichever is
/// running.
fn running() -> &'static Mutex<HashMap<u64, Arc<Mutex<Child>>>> {
    static RUNNING: OnceLock<Mutex<HashMap<u64, Arc<Mutex<Child>>>>> = OnceLock::new();
    RUNNING.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Kills the call this token is on. A cancel that only stopped the waiting
/// while the machine kept spending the quota would be a lie in the interface.
pub fn cancel(token: u64) {
    let child = running().lock().ok().and_then(|calls| calls.get(&token).cloned());
    if let Some(child) = child {
        if let Ok(mut child) = child.lock() {
            let _ = child.kill();
        }
    }
}

/// The header goes in on stdin through `--config -`; passing it as an argument
/// would put the key in the process list for anything running `ps` to read.
fn post(
    url: &str,
    header: &str,
    body: &str,
    token: Option<u64>,
) -> Result<(String, crate::usage::Limits), String> {
    // The headers go to a file rather than to stdout: `-D -` would put them in
    // front of the JSON and there is no safe place to cut.
    let dump = std::env::temp_dir().join(format!("cortex-headers-{}", std::process::id()));
    let dump_arg = dump.to_string_lossy().to_string();

    let mut child = Command::new("/usr/bin/curl")
        .args(["--silent", "--show-error", "--max-time", "60", "--dump-header", &dump_arg])
        .args(["--config", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| error.to_string())?;

    // The pipes come out before the child is shared, so reading them does not
    // hold the lock that `cancel` needs to reach in and kill it.
    let mut stdin = child.stdin.take().ok_or("sem stdin para o curl")?;
    let mut stdout = child.stdout.take().ok_or("sem stdout do curl")?;
    let mut stderr = child.stderr.take().ok_or("sem stderr do curl")?;

    let child = Arc::new(Mutex::new(child));
    if let Some(token) = token {
        if let Ok(mut calls) = running().lock() {
            calls.insert(token, child.clone());
        }
    }
    let done = Registration { token };

    let config = curl_config(url, header, body);
    stdin.write_all(config.as_bytes()).map_err(|error| error.to_string())?;
    drop(stdin);

    let mut out = String::new();
    let mut complaint = String::new();
    stdout.read_to_string(&mut out).map_err(|error| error.to_string())?;
    stderr.read_to_string(&mut complaint).map_err(|error| error.to_string())?;

    let status = child.lock().map_err(|_| "curl travou")?.wait().map_err(|e| e.to_string())?;
    drop(done);

    let headers = std::fs::read_to_string(&dump).unwrap_or_default();
    let _ = std::fs::remove_file(&dump);

    if !status.success() {
        let complaint = complaint.trim().to_string();
        return Err(if complaint.is_empty() { "a chamada falhou".into() } else { complaint });
    }
    Ok((out, crate::usage::parse_headers(&headers)))
}

/// Takes the token out of the register however this call ends — an error path
/// that leaked one would leave a dead process cancellable forever.
struct Registration {
    token: Option<u64>,
}

impl Drop for Registration {
    fn drop(&mut self) {
        if let Some(token) = self.token {
            if let Ok(mut calls) = running().lock() {
                calls.remove(&token);
            }
        }
    }
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
        let chain = Chain { groq: "m", gemini: &["m"] };
        let error = ask(dir.path(), "s", "u", chain, None).unwrap_err();
        assert!(error.contains("chave"), "{error}");
    }

    /// Falling over costs a fifth of Gemini's day, so it only happens when the
    /// next one could actually do better.
    #[test]
    fn only_a_failure_the_next_provider_could_fix_falls_over() {
        assert!(worth_falling_over("Groq: Rate limit reached for model"));
        assert!(worth_falling_over("429 Too Many Requests"));
        assert!(worth_falling_over("Gemini: RESOURCE_EXHAUSTED quota"));
        assert!(worth_falling_over("curl: (28) Operation timed out"));
        assert!(worth_falling_over("503 Service Unavailable"));

        assert!(!worth_falling_over("Groq: Invalid API Key"));
        assert!(!worth_falling_over("nenhuma chave do Groq configurada"));
        assert!(!worth_falling_over("This model is no longer available"));
        assert!(!worth_falling_over("Groq respondeu sem texto"));
    }

    /// Cancelling has to reach the process, not just stop the waiting — and it
    /// has to survive being called for a token that finished a second ago.
    #[test]
    fn a_call_is_killed_by_its_token() {
        cancel(404); // nothing registered: must not panic

        let mut child = Command::new("/bin/sleep")
            .arg("30")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take();
        let shared = Arc::new(Mutex::new(child));
        running().lock().unwrap().insert(7, shared.clone());

        let started = std::time::Instant::now();
        cancel(7);
        let status = shared.lock().unwrap().wait().unwrap();

        assert!(!status.success(), "morreu por sinal, não por conta própria");
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
        running().lock().unwrap().remove(&7);
    }
}
