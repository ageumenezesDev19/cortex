//! How much of the day's quota is gone.
//!
//! Two sources, and the difference matters. Groq states its own numbers in the
//! headers of every reply — total, remaining, and when it renews — so what the
//! app shows for it is the provider's arithmetic. Gemini sends nothing at all,
//! so its bar is this app counting its own calls against a written-down limit,
//! and it cannot know about a request made from anywhere else.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::provider::Provider;

const DIR: &str = ".server";
const FILE: &str = "usage.json";

/// Gemini's daily quota resets at midnight Pacific, which the docs state and
/// nothing in the response confirms. -8 is standard time; an hour of drift
/// twice a year is not worth a timezone crate for a counter.
const PACIFIC_OFFSET_HOURS: i64 = -8;

/// What a provider said about its own limits, as of the last call.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Limits {
    pub limit_requests: Option<u32>,
    pub remaining_requests: Option<u32>,
    pub reset_requests: Option<String>,
    pub limit_tokens: Option<u32>,
    pub remaining_tokens: Option<u32>,
    pub reset_tokens: Option<String>,
}

impl Limits {
    pub fn is_empty(&self) -> bool {
        *self == Limits::default()
    }
}

/// One provider's day: what it told us, and what we spent.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Spent {
    /// Days since the epoch, Pacific, so a stale count is recognised as stale
    /// rather than added to.
    #[serde(default)]
    day: i64,
    #[serde(default)]
    pub calls: u32,
    #[serde(default)]
    pub reported: Limits,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    #[serde(default)]
    pub groq: Spent,
    #[serde(default)]
    pub gemini: Spent,
}

pub fn path(root: &Path) -> PathBuf {
    root.join(DIR).join(FILE)
}

pub fn load(root: &Path) -> Usage {
    let mut usage: Usage = std::fs::read_to_string(path(root))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default();

    let today = pacific_day(now());
    usage.groq.roll_over(today);
    usage.gemini.roll_over(today);
    usage
}

impl Spent {
    /// A count from yesterday is not a count: the quota renewed while nobody
    /// was looking.
    fn roll_over(&mut self, today: i64) {
        if self.day != today {
            self.day = today;
            self.calls = 0;
            // What the provider reported belongs to the day it was reported in.
            self.reported = Limits::default();
        }
    }
}

/// Records one call and whatever the provider said while answering it.
pub fn record(root: &Path, provider: Provider, reported: Limits) {
    let mut usage = load(root);
    let spent = match provider {
        Provider::Groq => &mut usage.groq,
        Provider::Gemini => &mut usage.gemini,
    };
    spent.calls += 1;
    if !reported.is_empty() {
        spent.reported = reported;
    }
    save(root, &usage);
}

fn save(root: &Path, usage: &Usage) {
    let file = path(root);
    if let Some(parent) = file.parent() {
        if std::fs::create_dir_all(parent).is_err() {
            return;
        }
    }
    if let Ok(body) = serde_json::to_vec_pretty(usage) {
        let _ = std::fs::write(file, body);
    }
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs() as i64)
        .unwrap_or_default()
}

fn pacific_day(seconds: i64) -> i64 {
    (seconds + PACIFIC_OFFSET_HOURS * 3600).div_euclid(86_400)
}

/// Seconds until midnight Pacific, for the "renews in" beside Gemini's bar.
pub fn seconds_to_reset() -> i64 {
    let seconds = now() + PACIFIC_OFFSET_HOURS * 3600;
    86_400 - seconds.rem_euclid(86_400)
}

/// Reads the rate-limit headers curl dumped. Groq sends all six; Gemini sends
/// none, and an absent header is missing rather than zero — a bar drawn from a
/// zero it invented would be worse than no bar.
pub fn parse_headers(dump: &str) -> Limits {
    let value = |name: &str| -> Option<String> {
        dump.lines()
            .filter_map(|line| line.split_once(':'))
            .find(|(key, _)| key.trim().eq_ignore_ascii_case(name))
            .map(|(_, value)| value.trim().to_string())
    };
    let number = |name: &str| value(name).and_then(|raw| raw.parse::<u32>().ok());

    Limits {
        limit_requests: number("x-ratelimit-limit-requests"),
        remaining_requests: number("x-ratelimit-remaining-requests"),
        reset_requests: value("x-ratelimit-reset-requests"),
        limit_tokens: number("x-ratelimit-limit-tokens"),
        remaining_tokens: number("x-ratelimit-remaining-tokens"),
        reset_tokens: value("x-ratelimit-reset-tokens"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    const GROQ: &str = "HTTP/2 200 \r
x-ratelimit-limit-requests: 1000\r
x-ratelimit-limit-tokens: 12000\r
x-ratelimit-remaining-requests: 999\r
x-ratelimit-remaining-tokens: 11963\r
x-ratelimit-reset-requests: 1m26.4s\r
x-ratelimit-reset-tokens: 185ms\r
";

    #[test]
    fn the_six_numbers_come_out_of_the_headers() {
        let limits = parse_headers(GROQ);
        assert_eq!(limits.limit_requests, Some(1000));
        assert_eq!(limits.remaining_requests, Some(999));
        assert_eq!(limits.reset_requests.as_deref(), Some("1m26.4s"));
        assert_eq!(limits.limit_tokens, Some(12000));
        assert_eq!(limits.remaining_tokens, Some(11963));
        assert_eq!(limits.reset_tokens.as_deref(), Some("185ms"));
    }

    /// Gemini answers with none of them, and that has to read as "unknown".
    #[test]
    fn a_reply_without_the_headers_reports_nothing() {
        let limits = parse_headers("HTTP/2 200 \r\nx-gemini-service-tier: standard\r\n");
        assert!(limits.is_empty());
        assert_eq!(limits.limit_requests, None);
    }

    #[test]
    fn a_call_is_counted_and_survives_a_reload() {
        let dir = TempDir::new().unwrap();
        record(dir.path(), Provider::Gemini, Limits::default());
        record(dir.path(), Provider::Gemini, Limits::default());

        let usage = load(dir.path());
        assert_eq!(usage.gemini.calls, 2);
        assert_eq!(usage.groq.calls, 0);
    }

    #[test]
    fn what_the_provider_reported_is_kept_beside_the_count() {
        let dir = TempDir::new().unwrap();
        record(dir.path(), Provider::Groq, parse_headers(GROQ));

        let usage = load(dir.path());
        assert_eq!(usage.groq.reported.remaining_requests, Some(999));
        assert_eq!(usage.groq.calls, 1);
    }

    /// Yesterday's count is not today's: the quota renewed in between.
    #[test]
    fn a_count_from_another_day_starts_over() {
        let dir = TempDir::new().unwrap();
        record(dir.path(), Provider::Groq, parse_headers(GROQ));

        let mut stale = load(dir.path());
        stale.groq.day -= 1;
        save(dir.path(), &stale);

        let fresh = load(dir.path());
        assert_eq!(fresh.groq.calls, 0);
        assert!(fresh.groq.reported.is_empty(), "o que ele reportou era de ontem");
    }

    #[test]
    fn the_reset_is_somewhere_in_the_next_day() {
        let left = seconds_to_reset();
        assert!(left > 0 && left <= 86_400, "{left}");
    }

    /// It sits where the token and the keys sit: invisible to the vault and
    /// unreachable through a path.
    #[test]
    fn the_file_is_invisible_to_the_vault() {
        let dir = TempDir::new().unwrap();
        assert!(crate::vault::is_ignored(dir.path(), &path(dir.path())));
        assert!(crate::vault::resolve(dir.path(), ".server/usage.json").is_err());
    }
}
