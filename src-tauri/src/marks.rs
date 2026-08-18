//! What a screenshot can be said to be about — as data, not as code.
//!
//! This started as eight scoring functions and two `const` arrays, which meant
//! the app could only ever recognise the eleven things I had typed, and adding
//! a twelfth needed a rebuild. It also meant the app could only describe code:
//! a photo of a receipt came back bare, because "receipt" was not on my list.
//!
//! Now the vocabulary lives in `.server/marks.json`, next to the settings and
//! invisible to the vault. The file ships with the eleven, so nothing about the
//! rule changes on the day of the update — what changes is what it costs to
//! change it afterwards.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const DIR: &str = ".server";
const FILE: &str = "marks.json";
/// Long screenshots are common; scoring the whole of one buys nothing.
const MAX_SCAN: usize = 8_000;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// At most one wins, and only if it clears the floor.
    Linguagem,
    /// Any number can be true at once, each with its own floor.
    Assunto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mark {
    pub slug: String,
    pub nome: String,
    #[serde(default)]
    pub icone: Option<String>,
    pub tipo: Kind,
    /// Counted in the text, three times each at most, as the rule always did.
    pub sinais: Vec<String>,
    /// Shapes that must be present for the mark to score at all: a list of
    /// alternatives, all of which must be satisfied. This is what separates a
    /// stylesheet from a note *about* styling — `:` and one of `;` or `{`.
    #[serde(default)]
    pub exige: Vec<Vec<String>>,
    /// Half of another mark's score, for a dialect: Sass is CSS plus its own
    /// markers, and must outrank it when they are there.
    #[serde(default)]
    pub herda: Option<String>,
    /// Comes along when this one wins.
    #[serde(default)]
    pub implica: Vec<String>,
    /// Its own floor, for the subjects that do not use the language one.
    #[serde(default)]
    pub piso: Option<u32>,
    /// Two points for a text that opens like this — JSON is recognised as much
    /// by its first character as by its contents.
    #[serde(default)]
    pub comeca_com: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Vocabulary {
    /// How many screenshots have to share a mark before it is worth offering as
    /// a topic. One is not a grouping.
    #[serde(default = "three")]
    pub minimo_para_topico: u32,
    /// Below this, the text does not look like anything in particular and gets
    /// no mark. A wrong label is worse than none.
    #[serde(default = "four")]
    pub piso: u32,
    /// Above this the rule is sure enough to skip the model entirely.
    #[serde(default = "nine")]
    pub confianca: u32,
    pub marcas: Vec<Mark>,
}

fn three() -> u32 {
    3
}
fn four() -> u32 {
    4
}
fn nine() -> u32 {
    9
}

pub fn path(root: &Path) -> PathBuf {
    root.join(DIR).join(FILE)
}

/// Reads the vocabulary, writing the default one the first time. A corrupt file
/// falls back rather than leaving the app unable to classify anything.
pub fn load(root: &Path) -> Vocabulary {
    if let Some(read) = std::fs::read_to_string(path(root))
        .ok()
        .and_then(|raw| serde_json::from_str::<Vocabulary>(&raw).ok())
    {
        return read;
    }
    let fallback = default();
    let _ = save(root, &fallback);
    fallback
}

pub fn save(root: &Path, vocabulary: &Vocabulary) -> Result<(), String> {
    let file = path(root);
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let body = serde_json::to_vec_pretty(vocabulary).map_err(|error| error.to_string())?;
    std::fs::write(file, body).map_err(|error| error.to_string())
}

/// The eleven the app shipped with, written out so they can be edited.
pub fn default() -> Vocabulary {
    let language = |slug: &str, nome: &str, icone: Option<&str>, sinais: &[&str]| Mark {
        slug: slug.into(),
        nome: nome.into(),
        icone: icone.map(str::to_string),
        tipo: Kind::Linguagem,
        sinais: sinais.iter().map(|s| s.to_string()).collect(),
        exige: Vec::new(),
        herda: None,
        implica: Vec::new(),
        piso: None,
        comeca_com: Vec::new(),
    };

    let mut css = language(
        "css",
        "CSS",
        Some("css"),
        &[
            "display:", "color:", "margin", "padding", "font-", "flex", "grid", "border", "width:",
            "@media", ":hover", "!important", "px;", "rem;", "background",
        ],
    );
    // A stylesheet declares something; prose that merely says "revisar margin e
    // padding" carries every style word and is not CSS.
    css.exige = vec![vec![":".into()], vec![";".into(), "{".into()]];
    css.implica = vec!["estilização".into()];

    // `$` used to be a Sass signal, and it filed a screenshot of a shop — where
    // `$` is a price — under Sass. A dialect of CSS still has to look like CSS:
    // the same declaration shape is required, and `$` only counts next to one.
    let mut sass = language("sass", "Sass", Some("sass"), &["@mixin", "@include", "@extend", "&:", "@use ", "$"]);
    sass.exige = vec![vec![":".into()], vec![";".into(), "{".into()]];
    sass.herda = Some("css".into());
    sass.implica = vec!["estilização".into()];

    let js = language(
        "javascript",
        "JavaScript",
        Some("javascript"),
        &["const ", "let ", "=>", "function", "import ", "export ", "return ", "async ", "await "],
    );

    let mut ts = language(
        "typescript",
        "TypeScript",
        Some("typescript"),
        &["interface ", ": string", ": number", "<t>", "as const", "type "],
    );
    ts.herda = Some("javascript".into());

    let mut json = language("json", "JSON", Some("json"), &["\": \"", "\": {", "\": [", "\",", "null,"]);
    json.comeca_com = vec!["{".into(), "[".into()];

    let subject = |slug: &str, nome: &str, icone: Option<&str>, piso: u32, sinais: &[&str]| Mark {
        slug: slug.into(),
        nome: nome.into(),
        icone: icone.map(str::to_string),
        tipo: Kind::Assunto,
        sinais: sinais.iter().map(|s| s.to_string()).collect(),
        exige: Vec::new(),
        herda: None,
        implica: Vec::new(),
        piso: Some(piso),
        comeca_com: Vec::new(),
    };

    Vocabulary {
        minimo_para_topico: three(),
        piso: four(),
        confianca: nine(),
        marcas: vec![
            css,
            sass,
            js,
            ts,
            language("html", "HTML", Some("html5"), &["<div", "</", "class=", "<span", "<script", "<html", "href="]),
            json,
            language("sql", "SQL", Some("postgresql"), &["select ", " from ", "where ", "join ", "insert into", "group by"]),
            language("terminal", "Terminal", Some("gnubash"), &["$ ", "npm ", "git ", "cd ", "sudo", "yarn ", "cargo ", "% ", "~/"]),
            subject("estilização", "Estilização", Some("css"), 4, &[]),
            subject(
                "erro",
                "Erro",
                None,
                3,
                &["error", "exception", "traceback", "failed", "stack", "cannot find", "undefined is"],
            ),
            subject(
                "configuração",
                "Configuração",
                None,
                3,
                &["config", "\"scripts\"", "dependencies", "env", "settings", "package.json"],
            ),
        ],
    }
}

impl Vocabulary {
    pub fn find(&self, slug: &str) -> Option<&Mark> {
        self.marcas.iter().find(|mark| mark.slug == slug)
    }

    /// How a mark reads in the panel and which brand icon goes with it. An
    /// unknown slug — one the model invented — reads as itself.
    pub fn presentation(&self, slug: &str) -> (String, Option<String>) {
        match self.find(slug) {
            Some(mark) => (mark.nome.clone(), mark.icone.clone()),
            None => (slug.to_string(), None),
        }
    }

    /// The rule. Returns the marks and how sure it is, so the caller can decide
    /// whether the model is worth a call.
    pub fn by_rule(&self, text: &str) -> (Marks, u32) {
        let text: String = text.chars().take(MAX_SCAN).collect();
        let lower = text.to_lowercase();

        let languages: Vec<(&Mark, u32)> = self
            .marcas
            .iter()
            .filter(|mark| mark.tipo == Kind::Linguagem)
            .map(|mark| (mark, self.score(mark, &lower, &text)))
            .collect();

        let best = languages.iter().max_by_key(|(_, score)| *score);
        let (language, best_score) = match best {
            Some((mark, score)) if *score >= self.piso => (Some(mark.slug.clone()), *score),
            Some((_, score)) => (None, *score),
            None => (None, 0),
        };

        // Subjects stand on their own. A stack trace has no language to speak
        // of and would come back bare if it had to clear the language floor.
        let mut subjects: Vec<String> = Vec::new();
        let mut confidence = best_score;

        if let Some(slug) = &language {
            if let Some(mark) = self.find(slug) {
                subjects.extend(mark.implica.iter().cloned());
            }
        }
        for mark in self.marcas.iter().filter(|mark| mark.tipo == Kind::Assunto) {
            if mark.sinais.is_empty() {
                continue;
            }
            let score = self.score(mark, &lower, &text);
            if score >= mark.piso.unwrap_or(self.piso) && !subjects.contains(&mark.slug) {
                subjects.push(mark.slug.clone());
            }
            confidence = confidence.max(score);
        }

        (Marks { language, subjects }, confidence)
    }

    fn score(&self, mark: &Mark, lower: &str, raw: &str) -> u32 {
        if !self.shapes_met(mark, lower) {
            return 0;
        }
        let own = count(lower, &mark.sinais);
        // A dialect must not win on its parent's score alone: no markers of its
        // own, no claim.
        if mark.herda.is_some() && own == 0 {
            return 0;
        }

        let inherited = mark
            .herda
            .as_ref()
            .and_then(|slug| self.find(slug))
            .map(|parent| self.score(parent, lower, raw) / 2)
            .unwrap_or(0);

        let weight = if mark.herda.is_some() { 2 } else { 1 };
        let opening = mark
            .comeca_com
            .iter()
            .any(|start| raw.trim_start().starts_with(start.as_str()))
            .then_some(2)
            .unwrap_or(0);

        inherited + own * weight + opening
    }

    fn shapes_met(&self, mark: &Mark, lower: &str) -> bool {
        mark.exige
            .iter()
            .all(|alternatives| alternatives.iter().any(|shape| lower.contains(shape.as_str())))
    }
}

fn count(haystack: &str, needles: &[String]) -> u32 {
    needles.iter().map(|needle| haystack.matches(needle.as_str()).count().min(3) as u32).sum()
}

/// What a screenshot turned out to be.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Marks {
    /// One language at most — the highest score, or nothing.
    pub language: Option<String>,
    /// What it is about. A screenshot can be several at once, and since the
    /// model may name something the vocabulary never heard of, these are not
    /// limited to the file.
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn the_file_writes_itself_the_first_time() {
        let dir = TempDir::new().unwrap();
        assert!(!path(dir.path()).exists());

        let vocabulary = load(dir.path());
        assert!(path(dir.path()).exists(), "o padrão fica no disco para ser editado");
        assert_eq!(vocabulary.marcas.len(), 11);
        assert_eq!(vocabulary.minimo_para_topico, 3);
    }

    #[test]
    fn a_corrupt_file_falls_back_instead_of_classifying_nothing() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(DIR)).unwrap();
        std::fs::write(path(dir.path()), "isto não é json").unwrap();

        assert_eq!(load(dir.path()).marcas.len(), 11);
    }

    /// The point of the file: a twelfth mark without a rebuild.
    #[test]
    fn a_mark_added_to_the_file_is_recognised() {
        let dir = TempDir::new().unwrap();
        let mut vocabulary = default();
        vocabulary.marcas.push(Mark {
            slug: "rust".into(),
            nome: "Rust".into(),
            icone: Some("rust".into()),
            tipo: Kind::Linguagem,
            sinais: vec!["fn ".into(), "impl ".into(), "let mut".into(), "->".into()],
            exige: Vec::new(),
            herda: None,
            implica: Vec::new(),
            piso: None,
            comeca_com: Vec::new(),
        });
        save(dir.path(), &vocabulary).unwrap();

        let read = load(dir.path());
        let code = "fn main() { let mut total = 0; }\nimpl Nota { fn abrir(&self) -> String {} }";
        assert_eq!(read.by_rule(code).0.language.as_deref(), Some("rust"));
    }

    #[test]
    fn the_file_is_invisible_to_the_vault() {
        let dir = TempDir::new().unwrap();
        assert!(crate::vault::is_ignored(dir.path(), &path(dir.path())));
        assert!(crate::vault::resolve(dir.path(), ".server/marks.json").is_err());
    }
}
