//! What a screenshot is about, decided from the text the OCR already produced.
//!
//! The rule scores rather than parses, because OCR mangles code in predictable
//! ways: `l` comes back as `1`, `{` as `(`, `;` as `:`. A classifier that
//! demanded valid syntax would fail on exactly the screenshots this app exists
//! for. What it scores lives in `marks.rs`, as data — this file is the decision
//! and the model call, not the vocabulary.

use std::path::Path;

use crate::marks::{Marks, Vocabulary};

/// The model is asked for one or two words of its own, not for a pick from a
/// list. A vault is not only code: a photo of a receipt used to come back bare
/// because "recibo" was not among the eleven things the app knew about.
const ASK_SYSTEM: &str = "\
Você classifica o texto extraído de um print de tela. Responda APENAS com uma \
linha no formato linguagem|assuntos. \
Linguagem: a linguagem de programação, se houver código, ou 'nenhuma'. \
Assuntos: de zero a dois, cada um com uma ou duas palavras em português \
minúsculas, dizendo do que o print trata — pode ser qualquer coisa, não só \
programação (recibo, receita, currículo, conversa). \
Se não der para dizer, responda 'nenhuma|'. Sem explicação. \
Exemplos: css|estilização — nenhuma|recibo, mercado";

/// The rule decides; the model only breaks a tie it could not break itself.
pub fn marks(root: &Path, text: &str, models: (&str, &str), may_ask: bool) -> Marks {
    let vocabulary = crate::marks::load(root);
    let (rule, score) = vocabulary.by_rule(text);

    if score >= vocabulary.confianca || !may_ask {
        return rule;
    }
    refine(root, text, models, &vocabulary).unwrap_or(rule)
}

/// Asks the model when the rule was neither confident nor empty. The language
/// it answers is checked against the vocabulary — a language the app cannot
/// score is a language it cannot have meant — but the subjects are taken as
/// given, because naming what a screenshot is about is the whole reason to ask.
fn refine(root: &Path, text: &str, models: (&str, &str), vocabulary: &Vocabulary) -> Option<Marks> {
    let sample: String = text.chars().take(2_000).collect();
    // No token: this pass has its own progress, and nothing on screen offers to
    // cancel the classification of a single screenshot.
    let chain = crate::provider::Chain { groq: models.0, gemini: &[models.1] };
    let answer = crate::provider::ask(root, ASK_SYSTEM, &sample, chain, None).ok()?;

    let marks = parse_reply(&answer.text, vocabulary);
    (!marks.is_empty()).then_some(marks)
}

/// Two or three words is a subject; a sentence is the model ignoring the
/// instruction, and gets dropped rather than filed.
const MAX_SUBJECT_WORDS: usize = 2;
const MAX_SUBJECT_CHARS: usize = 24;

fn parse_reply(reply: &str, vocabulary: &Vocabulary) -> Marks {
    let line = reply.lines().find(|line| line.contains('|')).unwrap_or(reply).trim();
    let (language, subjects) = line.split_once('|').unwrap_or((line, ""));

    let language = language.trim().to_lowercase();
    let language = vocabulary.find(&language).map(|mark| mark.slug.clone());

    let subjects = subjects
        .split(',')
        .map(|subject| subject.trim().to_lowercase())
        .filter(|subject| {
            !subject.is_empty()
                && subject != "nenhum"
                && subject != "nenhuma"
                && subject.split_whitespace().count() <= MAX_SUBJECT_WORDS
                && subject.chars().count() <= MAX_SUBJECT_CHARS
        })
        .take(2)
        .collect();

    Marks { language, subjects }
}

/// The file name a mark gets as a topic. Accents and case would make two files
/// for one mark on a case-insensitive disk.
pub fn slug_of(mark: &str) -> String {
    crate::vault::slugify(mark)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vocabulary() -> Vocabulary {
        crate::marks::default()
    }

    fn language_of(text: &str) -> Option<String> {
        vocabulary().by_rule(text).0.language
    }

    #[test]
    fn a_stylesheet_is_css_and_about_styling() {
        let text = ".card { display: flex; padding: 16px; color: #333; }\n@media (max-width: 600px) { .card { display: block; } }";
        let (marks, _) = vocabulary().by_rule(text);
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
        let (marks, _) = vocabulary().by_rule(text);
        assert!(marks.subjects.contains(&"erro".to_string()));
    }

    /// The case the whole floor exists for: a screenshot with no code must come
    /// back bare rather than be filed under a guess.
    #[test]
    fn ordinary_text_gets_no_mark_at_all() {
        let text = "Lista de compras: pão árabe, picanha, cheddar cremoso e molho barbecue para o fim de semana.";
        let (marks, _) = vocabulary().by_rule(text);
        assert!(marks.is_empty(), "{marks:?}");
    }

    #[test]
    fn an_empty_screenshot_gets_no_mark() {
        assert!(vocabulary().by_rule("").0.is_empty());
    }

    /// Prose *about* styling is not a stylesheet; the declaration shape is what
    /// separates them.
    #[test]
    fn writing_about_css_is_not_filed_as_css() {
        let text = "Anotação: preciso estudar flexbox e grid, além de revisar margin e padding na semana que vem.";
        assert_ne!(language_of(text).as_deref(), Some("css"));
    }

    /// The reason to ask a model at all: a vault is not only code, and the
    /// subject it names is taken even when the vocabulary never heard of it.
    #[test]
    fn a_subject_the_app_never_heard_of_is_kept() {
        let marks = parse_reply("nenhuma|recibo, mercado", &vocabulary());
        assert_eq!(marks.language, None);
        assert_eq!(marks.subjects, ["recibo", "mercado"]);
    }

    /// A language it cannot score is a language it cannot have meant.
    #[test]
    fn an_invented_language_is_dropped() {
        let marks = parse_reply("cobol|erro", &vocabulary());
        assert_eq!(marks.language, None);
        assert_eq!(marks.subjects, ["erro"]);
    }

    #[test]
    fn a_sentence_is_not_a_subject() {
        let marks = parse_reply(
            "nenhuma|isto parece ser uma captura de tela de um aplicativo qualquer",
            &vocabulary(),
        );
        assert!(marks.subjects.is_empty());
    }

    #[test]
    fn nothing_read_is_nothing_marked() {
        assert!(parse_reply("nenhuma|", &vocabulary()).is_empty());
        assert!(parse_reply("não consegui classificar", &vocabulary()).is_empty());
    }
}
