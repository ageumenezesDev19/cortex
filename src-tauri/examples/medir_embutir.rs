//! Times a full embedding pass over passages shaped like the real vault.
//!
//! The lengths come from the vault's own chunks; the words are filler. That is
//! the whole point: ONNX pads every text in a batch up to the longest one in
//! it, so what costs time is the *spread* of lengths, not the words. An earlier
//! benchmark used 484 identical sentences, measured 5.5s, and predicted nothing
//! about a vault whose longest passage is forty times its median.
//!
//!   cargo run --release --example medir_embutir -- [lote] [ordenar]
//!
//! `lote` is passages per call (default 100, the shipping value); `ordenar`
//! groups similar lengths together before batching.

use std::time::Instant;

use cortex_lib::{embed, vault};
use rusqlite::Connection;

fn main() {
    let lote: usize = std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(embed::BATCH);
    let ordenar = std::env::args().nth(2).is_some_and(|a| a == "ordenar");

    let root = vault::ensure().expect("não consegui abrir o vault");
    // Aberto direto, e só para ler tamanhos: o benchmark não precisa das
    // migrações nem de escrever nada no índice de verdade.
    let conn = Connection::open(vault::db_path(&root)).expect("não consegui abrir o índice");

    let mut lengths: Vec<usize> = conn
        .prepare("SELECT length(text) FROM chunks")
        .and_then(|mut s| s.query_map([], |row| row.get::<_, i64>(0)).map(|rows| {
            rows.filter_map(Result::ok).map(|n| n as usize).collect()
        }))
        .expect("não consegui ler os tamanhos");

    if lengths.is_empty() {
        eprintln!("o vault não tem trechos — nada a medir");
        return;
    }
    if ordenar {
        lengths.sort_unstable();
    }

    let textos: Vec<String> = lengths.iter().map(|&n| filler(n)).collect();
    let media = lengths.iter().sum::<usize>() / lengths.len();
    let maior = lengths.iter().max().copied().unwrap_or(0);

    println!("{} trechos · média {media} · maior {maior}", textos.len());
    println!("lote {lote} · {}", if ordenar { "ordenado por tamanho" } else { "na ordem do vault" });

    // Carrega antes de cronometrar: a primeira leitura do disco não é o que se
    // está medindo, e ela mede 465 MB.
    embed::load_model("multilingual-e5-small").expect("o modelo precisa estar baixado");
    let _ = embed::embed_batch(&root, "multilingual-e5-small", &[String::from("aquecendo")]);

    let inicio = Instant::now();
    let mut desperdicio = 0usize;
    for batch in textos.chunks(lote) {
        let maior_do_lote = batch.iter().map(|t| t.chars().count()).max().unwrap_or(0);
        desperdicio += batch.iter().map(|t| maior_do_lote - t.chars().count()).sum::<usize>();
        embed::embed_batch(&root, "multilingual-e5-small", batch).expect("embutiu");
    }
    let levou = inicio.elapsed();

    let uteis: usize = textos.iter().map(|t| t.chars().count()).sum();
    println!("\ntempo   {:.1}s ({:.0} trechos/s)", levou.as_secs_f32(), textos.len() as f32 / levou.as_secs_f32());
    println!("enchimento  {:.0}% do que foi processado era preenchimento", desperdicio as f32 / (uteis + desperdicio) as f32 * 100.0);
}

/// Words rather than one repeated letter: the tokenizer splits on word
/// boundaries, so a wall of `a` would produce a token count nothing like a real
/// passage of the same length.
fn filler(chars: usize) -> String {
    const PALAVRAS: [&str; 8] =
        ["estoque", "produção", "relatório", "fornecedor", "prazo", "nota", "cliente", "entrega"];
    let mut out = String::with_capacity(chars * 2);
    let mut at = 0;
    let mut posto = 0;
    while posto < chars {
        let palavra = PALAVRAS[at % PALAVRAS.len()];
        out.push_str(palavra);
        out.push(' ');
        posto += palavra.chars().count() + 1;
        at += 1;
    }
    // Por caractere, nunca por byte: "produção" ocupa mais bytes que letras, e
    // cortar por byte cai no meio de um deles.
    out.chars().take(chars).collect()
}
