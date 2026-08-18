//! Runs the classifier over the OCR text of every screenshot already in the
//! vault and prints what it decided.
//!
//! The unit tests check text I wrote. This checks text Vision produced from a
//! real screen, which is where OCR noise lives — `l` as `1`, `{` as `(`.
//!
//!   cargo run --example classificar_amostra

use cortex_lib::{marks, vault};

fn main() {
    let root = vault::ensure().expect("não consegui abrir o vault");
    // The vocabulary is read from the vault now, so this checks what the app
    // would actually decide — including anything added to the file by hand.
    let vocabulary = marks::load(&root);
    let mut seen = 0;
    let mut marked = 0;

    for entry in walkdir::WalkDir::new(&root).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if !path.to_string_lossy().ends_with(".ocr.json") {
            continue;
        }

        let Ok(raw) = std::fs::read_to_string(path) else { continue };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else { continue };
        let Some(text) = value.get("text").and_then(|t| t.as_str()) else { continue };

        seen += 1;
        let (marks, score) = vocabulary.by_rule(text);
        if !marks.is_empty() {
            marked += 1;
        }

        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let name = name.trim_end_matches(".ocr.json");
        let verdict = if marks.is_empty() { "—".to_string() } else { marks.all().join(" · ") };
        // A short sample of the text, so a wrong verdict is diagnosable without
        // opening the file.
        let sample: String = text.chars().filter(|c| *c != '\n').take(60).collect();

        println!("{name:44.44}  {score:>3}  {verdict:24}  {sample}");
    }

    println!();
    println!("prints com OCR : {seen}");
    println!("marcados       : {marked}");
    println!("sem marca      : {}", seen - marked);
}
