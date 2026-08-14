//! Runs the Notes.app import against the real vault, the same code path the
//! button in the app calls. Exists so the import can be exercised and timed
//! without a human clicking through it.
//!
//!   cargo run --release --example importar

use std::time::Instant;

use cortex_lib::{import, vault};

fn main() {
    let root = vault::ensure().expect("não consegui abrir o vault");
    println!("vault: {}", root.display());

    let started = Instant::now();
    let mut last = 0;

    let report = import::run(&root, |done, total, name| {
        // One line every twenty notes; 232 lines of progress helps nobody.
        if done - last >= 20 || done == total {
            last = done;
            let name: String = name.chars().take(40).collect();
            println!("  {done}/{total}  {name}");
        }
    })
    .expect("o import falhou");

    println!();
    println!("trazidas   : {}", report.imported);
    println!("puladas    : {}", report.skipped);
    println!("falharam   : {}", report.failed);
    println!("imagens    : {} marcadas, ainda no Notes", report.attachments);
    println!("levou      : {:.1}s", started.elapsed().as_secs_f64());
}