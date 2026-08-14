//! Runs the Notes.app converter over a file of real note bodies and reports how
//! the output looks, without printing anyone's notes.
//!
//! The unit tests check HTML I wrote from memory. This checks HTML Notes wrote.
//!
//!   cargo run --example converter_amostra -- <arquivo.html>

use cortex_lib::html;

fn main() {
    let path = std::env::args().nth(1).expect("passe o arquivo de amostra");
    let raw = std::fs::read_to_string(path).expect("não consegui ler a amostra");

    let mut notes = 0;
    let mut empty = 0;
    let mut attachments = 0;
    let mut suspicious: Vec<(usize, &'static str)> = Vec::new();

    for body in raw.split("<<<NOTA>>>").filter(|part| !part.trim().is_empty()) {
        notes += 1;
        let converted = html::to_markdown(body);
        let markdown = &converted.markdown;
        attachments += converted.attachments;

        if markdown.trim().is_empty() {
            empty += 1;
        }
        // Leftovers that would mean the walker missed a branch.
        for (needle, label) in [
            ("<div", "sobrou <div"),
            ("<span", "sobrou <span"),
            ("&nbsp;", "sobrou &nbsp;"),
            ("****", "negrito vazio"),
            ("``", "código vazio"),
        ] {
            if markdown.contains(needle) {
                suspicious.push((notes, label));
            }
        }
        if markdown.lines().any(|line| line.trim() == "#") {
            suspicious.push((notes, "título vazio"));
        }
        if markdown.contains("\n\n\n") {
            suspicious.push((notes, "três linhas em branco"));
        }
    }

    println!("notas convertidas : {notes}");
    println!("saíram vazias     : {empty}");
    println!("anexos marcados   : {attachments}");
    println!("suspeitas         : {}", suspicious.len());
    for (note, label) in &suspicious {
        println!("  nota {note}: {label}");
    }
}