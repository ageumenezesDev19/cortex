//! What a screenshot is about, decided from the text the OCR already produced.
//!
//! Scoring rather than parsing, because OCR mangles code in predictable ways:
//! `l` comes back as `1`, `{` as `(`, `;` as `:`. A classifier that demanded
//! valid syntax would fail on exactly the screenshots this app exists for, so
//! signals are counted and the strongest wins — one missing brace cannot sink
//! the whole verdict.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// Below this, the text does not look like anything in particular and gets no
/// mark. A wrong label is worse than none: it would put a photo of a receipt
/// under `css` and quietly poison the filter.
const FLOOR: u32 = 4;
/// Above this the rule is sure enough to skip the model entirely.
const CONFIDENT: u32 = 9;
/// Long screenshots are common; scoring the whole of one buys nothing.
const MAX_SCAN: usize = 8_000;

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Marks {
    /// One language at most — the highest score, or nothing.
    pub language: Option<String>,
    /// What it is about. A screenshot can be several at once.
    pub subjects: Vec<String>,
}

impl Marks {
    pub fn is_empty(&self) -> bool {
        self.language.is_none() && self.subjects.is_empty()
    }

    /// Everything that should reach the index, language first.
    pub fn all(&self) -> Vec<String> {
        self.language.iter().cloned().chain(self.subjects.iter().cloned()).collect()
    }
}

/// Every mark the app will accept, from the rule or from the model. A label
/// outside this list is discarded rather than trusted.
pub const LANGUAGES: [&str; 8] =
    ["css", "sass", "javascript", "typescript", "html", "json", "sql", "terminal"];
pub const SUBJECTS: [&str; 3] = ["estilização", "erro", "configuração"];

/// The rule. Returns the marks and how sure it is, so the caller can decide
/// whether the model is worth a call.
pub fn by_rule(text: &str) -> (Marks, u32) {
    let text: String = text.chars().take(MAX_SCAN).collect();
    let lower = text.to_lowercase();

    let scores = [
        ("css", css_score(&lower)),
        ("sass", sass_score(&lower)),
        ("javascript", js_score(&lower)),
        ("typescript", ts_score(&lower)),
        ("html", html_score(&lower)),
        ("json", json_score(&text)),
        ("sql", sql_score(&lower)),
        ("terminal", terminal_score(&lower)),
    ];

    let (name, best) = scores.iter().max_by_key(|(_, score)| *score).copied().unwrap();
    let language = (best >= FLOOR).then(|| name.to_string());

    // Subjects stand on their own. A stack trace has no language to speak of
    // and would come back bare if it had to clear the language floor first.
    let mut subjects = Vec::new();
    if matches!(language.as_deref(), Some("css" | "sass")) {
        subjects.push("estilização".to_string());
    }
    let errors = error_score(&lower);
    if errors >= 3 {
        subjects.push("erro".to_string());
    }
    if config_score(&lower) >= 3 {
        subjects.push("configuração".to_string());
    }

    let confidence = best.max(errors);
    (Marks { language, subjects }, confidence)
}

fn count(haystack: &str, needles: &[&str]) -> u32 {
    needles.iter().map(|needle| haystack.matches(needle).count().min(3) as u32).sum()
}

fn style_words(text: &str) -> u32 {
    count(
        text,
        &["display:", "color:", "margin", "padding", "font-", "flex", "grid", "border", "width:"],
    )
}

fn css_score(text: &str) -> u32 {
    // The declaration shape is what separates a stylesheet from a note *about*
    // styling — "revisar margin e padding" carries every style word and is not
    // CSS. Without it the score is zero rather than merely lower.
    let declares = text.contains(':') && (text.contains(';') || text.contains('{'));
    if !declares {
        return 0;
    }
    style_words(text) + count(text, &["@media", ":hover", "!important", "px;", "rem;", "background"])
}

fn sass_score(text: &str) -> u32 {
    let markers = count(text, &["@mixin", "@include", "@extend", "&:", "@use ", "$"]);
    if markers == 0 {
        return 0;
    }
    // Sass is CSS plus its own markers, so it must outrank plain CSS when they
    // are there and never win on the markers alone.
    css_score(text) / 2 + markers * 2
}

fn js_score(text: &str) -> u32 {
    count(
        text,
        &["const ", "let ", "=>", "function", "import ", "export ", "return ", "async ", "await "],
    )
}

fn ts_score(text: &str) -> u32 {
    let markers = count(text, &["interface ", ": string", ": number", "<t>", "as const", "type "]);
    if markers == 0 {
        return 0;
    }
    js_score(text) / 2 + markers * 2
}

fn html_score(text: &str) -> u32 {
    count(text, &["<div", "</", "class=", "<span", "<script", "<html", "href="])
}

fn json_score(text: &str) -> u32 {
    let trimmed = text.trim_start();
    let mut score = count(text, &["\": \"", "\": {", "\": [", "\",", "null,"]);
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        score += 2;
    }
    score
}

fn sql_score(text: &str) -> u32 {
    count(text, &["select ", " from ", "where ", "join ", "insert into", "group by"])
}

fn terminal_score(text: &str) -> u32 {
    count(text, &["$ ", "npm ", "git ", "cd ", "sudo", "yarn ", "cargo ", "% ", "~/"])
}

fn error_score(text: &str) -> u32 {
    count(text, &["error", "exception", "traceback", "failed", "stack", "cannot find", "undefined is"])
}

fn config_score(text: &str) -> u32 {
    count(text, &["config", "\"scripts\"", "dependencies", "env", "settings", "package.json"])
}

const ASK_SYSTEM: &str = "\
Você classifica o texto extraído de um print de tela. Responda APENAS com uma \
linha no formato linguagem|assuntos, usando somente estes valores. \
Linguagem (escolha uma ou 'nenhuma'): css, sass, javascript, typescript, html, \
json, sql, terminal. Assuntos (zero ou mais, separados por vírgula): \
estilização, erro, configuração. \
Sem explicação. Exemplo: css|estilização";

/// Asks the model only when the rule was neither confident nor empty. Anything
/// it answers is checked against the known marks — a label the app does not
/// know is dropped, not trusted.
pub fn refine(root: &Path, text: &str, models: (&str, &str)) -> Option<Marks> {
    let sample: String = text.chars().take(2_000).collect();
    let answer = crate::provider::ask(root, ASK_SYSTEM, &sample, models).ok()?;
    let marks = parse_reply(&answer.text);
    (!marks.is_empty()).then_some(marks)
}

fn parse_reply(reply: &str) -> Marks {
    let line = reply.lines().find(|line| line.contains('|')).unwrap_or(reply).trim();
    let (language, subjects) = line.split_once('|').unwrap_or((line, ""));

    let language = language.trim().to_lowercase();
    let language = LANGUAGES.contains(&language.as_str()).then_some(language);

    let subjects = subjects
        .split(',')
        .map(|subject| subject.trim().to_lowercase())
        .filter(|subject| SUBJECTS.contains(&subject.as_str()))
        .collect();

    Marks { language, subjects }
}

/// How a mark reads in the panel, and the brand icon that goes with it. The
/// slugs are the ones the icon picker already knows, so an automatic topic
/// looks like one you made by hand.
pub fn presentation(mark: &str) -> (String, Option<&'static str>) {
    let (name, icon) = match mark {
        "css" => ("CSS", Some("css")),
        "sass" => ("Sass", Some("sass")),
        "javascript" => ("JavaScript", Some("javascript")),
        "typescript" => ("TypeScript", Some("typescript")),
        "html" => ("HTML", Some("html5")),
        "json" => ("JSON", Some("json")),
        "sql" => ("SQL", Some("postgresql")),
        "terminal" => ("Terminal", Some("gnubash")),
        "estilização" => ("Estilização", Some("css")),
        "erro" => ("Erro", None),
        "configuração" => ("Configuração", None),
        other => return (other.to_string(), None),
    };
    (name.to_string(), icon)
}

/// The file name a mark gets as a topic. Accents and case would make two files
/// for one mark on a case-insensitive disk.
pub fn slug_of(mark: &str) -> String {
    crate::vault::slugify(mark)
}

/// The rule decides; the model only breaks a tie it could not break itself.
pub fn marks(root: &Path, text: &str, models: (&str, &str), may_ask: bool) -> Marks {
    let (rule, score) = by_rule(text);
    if score >= CONFIDENT || !may_ask {
        return rule;
    }
    refine(root, text, models).unwrap_or(rule)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn language_of(text: &str) -> Option<String> {
        by_rule(text).0.language
    }

    #[test]
    fn a_stylesheet_is_css_and_about_styling() {
        let text = ".card { display: flex; padding: 16px; color: #333; }\n@media (max-width: 600px) { .card { display: block; } }";
        let (marks, _) = by_rule(text);
        assert_eq!(marks.language.as_deref(), Some("css"));
        assert!(marks.subjects.contains(&"estilização".to_string()));
    }

    /// Sass has to beat CSS when its own markers are there, or every Sass
    /// screenshot files itself under the wrong language.
    #[test]
    fn sass_outranks_css_when_its_markers_are_present() {
        let text = "$primary: #333;\n@mixin card {\n  display: flex;\n  padding: 16px;\n}\n.card { @include card; &:hover { color: $primary; } }";
        assert_eq!(language_of(text).as_deref(), Some("sass"));
    }

    #[test]
    fn typescript_outranks_javascript_when_types_are_present() {
        let js = "const soma = (a, b) => a + b\nexport function total() { return soma(1, 2) }";
        let ts = "interface Nota { path: string }\nconst abrir = (nota: Nota): string => nota.path\nexport type Lista = Nota[]";
        assert_eq!(language_of(js).as_deref(), Some("javascript"));
        assert_eq!(language_of(ts).as_deref(), Some("typescript"));
    }

    #[test]
    fn terminal_output_is_recognised() {
        let text = "$ npm run build\n~/Documents/cortex % git status\ncargo test";
        assert_eq!(language_of(text).as_deref(), Some("terminal"));
    }

    #[test]
    fn a_stack_trace_is_marked_as_an_error() {
        let text = "TypeError: Cannot find module 'react'\n    at Object.<anonymous>\nError: build failed\nstack trace follows";
        let (marks, _) = by_rule(text);
        assert!(marks.subjects.contains(&"erro".to_string()));
    }

    /// The case the whole floor exists for: a screenshot with no code must come
    /// back bare rather than be filed under a guess.
    #[test]
    fn ordinary_text_gets_no_mark_at_all() {
        let text = "Lista de compras: pão árabe, picanha, cheddar cremoso e molho barbecue para o fim de semana.";
        let (marks, _) = by_rule(text);
        assert!(marks.is_empty(), "{marks:?}");
    }

    #[test]
    fn an_empty_screenshot_gets_no_mark() {
        assert!(by_rule("").0.is_empty());
    }

    /// Prose *about* styling is not a stylesheet; the declaration shape is what
    /// separates them.
    #[test]
    fn writing_about_css_is_not_filed_as_css() {
        let text = "Anotação: preciso estudar flexbox e grid, além de revisar margin e padding na semana que vem.";
        assert_ne!(language_of(text).as_deref(), Some("css"));
    }

    #[test]
    fn the_model_reply_is_read_and_bounded_to_known_marks() {
        assert_eq!(
            parse_reply("css|estilização"),
            Marks { language: Some("css".into()), subjects: vec!["estilização".into()] }
        );
        // Unknown language and unknown subject are both dropped.
        assert_eq!(parse_reply("brainfuck|dança"), Marks::default());
        // A model that explains itself first still gets read.
        assert_eq!(parse_reply("Claro!\ntypescript|erro").language.as_deref(), Some("typescript"));
    }

    #[test]
    fn nenhuma_is_not_mistaken_for_a_language() {
        assert!(parse_reply("nenhuma|").is_empty());
    }

    #[test]
    fn the_marks_reach_the_index_language_first() {
        let marks = Marks {
            language: Some("css".into()),
            subjects: vec!["estilização".into()],
        };
        assert_eq!(marks.all(), ["css", "estilização"]);
    }
}
