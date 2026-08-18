//! Builds the static data the web demo searches.
//!
//! The chunking is the app's own `parse::chunk_markdown` and the vectors come
//! from the same fastembed model, so a passage is split and placed exactly as
//! it would be on the desktop. Only the ranking is reimplemented in the
//! browser — see the demo's search module for what that costs.
//!
//!   cargo run --release --example gerar_demo
//!
//! Reads `demo/vault-pt` and `demo/vault-en`, writes one data file each.
//!
//! Two vaults rather than one bilingual vault because the model prefers the
//! same language over the same meaning: a Portuguese question ranked a note
//! about feeding sourdough above an English one about task switching. Keeping
//! question and passage in one language is what makes the search work.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use cortex_lib::{embed, parse};
use serde_json::{json, Value};

const MODEL: &str = "multilingual-e5-small";

fn main() {
    let raiz = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../demo");
    embed::load_model(MODEL).expect("o modelo precisa estar baixado");

    for idioma in ["pt", "en"] {
        gerar(&raiz, idioma);
    }
}

fn gerar(raiz: &Path, idioma: &str) {
    let vault = raiz.join(format!("vault-{idioma}"));

    let mut arquivos: Vec<PathBuf> = walkdir::WalkDir::new(&vault)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "md"))
        .map(|e| e.path().to_path_buf())
        .collect();
    arquivos.sort();
    assert!(!arquivos.is_empty(), "não achei nota nenhuma em {}", vault.display());

    let mut notas = BTreeMap::new();
    let mut trechos: Vec<Value> = Vec::new();
    let mut textos: Vec<String> = Vec::new();

    for arquivo in &arquivos {
        let relativo = arquivo.strip_prefix(&vault).unwrap().to_string_lossy().to_string();
        let fonte = std::fs::read_to_string(arquivo).expect("li a nota");
        let titulo = parse::title_of(&fonte, &relativo);
        let pedacos = parse::chunk_markdown(&fonte);

        notas.insert(relativo.clone(), json!({
            "path": relativo,
            "title": titulo,
            "source": fonte,
            "chunks": pedacos.iter().map(|c| json!({
                "headingPath": c.heading_path, "start": c.start, "end": c.end,
            })).collect::<Vec<_>>(),
        }));

        for pedaco in pedacos {
            trechos.push(json!({
                "chunkId": trechos.len(),
                "path": relativo,
                "headingPath": pedaco.heading_path,
                "text": pedaco.text,
                "start": pedaco.start,
                "end": pedaco.end,
            }));
            textos.push(pedaco.text);
        }
    }

    let mut vetores = Vec::new();
    for lote in textos.chunks(embed::BATCH) {
        vetores.extend(embed::embed_batch(Path::new("."), MODEL, lote).expect("embutiu"));
    }
    assert_eq!(vetores.len(), trechos.len());

    for (trecho, vetor) in trechos.iter_mut().zip(&vetores) {
        // Quatro casas: o cosseno não distingue mais que isso, e o arquivo que
        // o navegador baixa cai pela metade.
        let curto: Vec<f32> = vetor.iter().map(|v| (v * 10_000.0).round() / 10_000.0).collect();
        trecho["vector"] = json!(curto);
    }

    let saida = json!({
        "topics": topicos(idioma),
        "vault": {
            "root": "Cortex (demo)",
            "notes": arquivos.len(),
            "images": 0,
            "chunks": trechos.len(),
        },
        "tree": arvore(&vault, ""),
        "notes": notas,
        "chunks": trechos,
        "dimensions": embed::DIMENSIONS,
    });

    let destino = raiz.join(format!("data-{idioma}.json"));
    std::fs::write(&destino, serde_json::to_vec(&saida).unwrap()).expect("escrevi");
    let tamanho = std::fs::metadata(&destino).unwrap().len();
    println!(
        "{idioma}: {} notas · {} trechos · {} KB",
        arquivos.len(),
        trechos.len(),
        tamanho / 1024
    );
}

/// Three topics, one of them deliberately spanning folders — that is what the
/// empty topics panel promises and what a seeded vault should show.
fn topicos(idioma: &str) -> Value {
    // "Tools" in both: a tool keeps its name across languages, the way Blender
    // and Front-end already do here.
    let (craft, front) = ("Tools", "Front-end");
    let n = |base: &str, ext: &str| if idioma == "pt" { base.to_string() } else { ext.to_string() };

    json!([
        {
            "slug": "ferramentas", "name": craft, "icon": "git", "color": null, "auto": false,
            "members": [
                n("dev/rust-notas.md", "dev/rust-notes.md"),
                n("dev/atalhos-do-editor.md", "dev/editor-shortcuts.md"),
                n("git/desfazer.md", "git/undoing.md"),
            ],
        },
        {
            "slug": "front-end", "name": front, "icon": "css", "color": null, "auto": false,
            "members": [
                n("dev/css-grid.md", "dev/css-grid.md"),
                n("reading/tipografia.md", "reading/typography.md"),
            ],
        },
        {
            "slug": "blender", "name": "Blender", "icon": "blender", "color": null, "auto": false,
            "members": [
                n("blender/iluminacao.md", "blender/lighting.md"),
                n("blender/render-final.md", "blender/final-render.md"),
            ],
        },
    ])
}

/// A árvore que a barra lateral desenha, em ordem: pastas antes de notas.
///
/// `prefixo` carrega o caminho relativo à raiz, porque uma entrada precisa
/// bater com a chave do mapa de notas — só o nome do arquivo não identifica
/// nada quando duas pastas têm um `notas.md`.
fn arvore(dir: &Path, prefixo: &str) -> Vec<Value> {
    let mut entradas: Vec<_> = std::fs::read_dir(dir).unwrap().filter_map(Result::ok).collect();
    entradas.sort_by_key(|e| (e.path().is_file(), e.file_name()));
    entradas
        .iter()
        .filter(|e| e.path().is_dir() || e.path().extension().is_some_and(|x| x == "md"))
        .map(|e| {
            let caminho = e.path();
            let nome = e.file_name().to_string_lossy().to_string();
            let relativo =
                if prefixo.is_empty() { nome.clone() } else { format!("{prefixo}/{nome}") };
            if caminho.is_dir() {
                let filhos = arvore(&caminho, &relativo);
                json!({ "path": relativo, "name": nome, "kind": "folder", "children": filhos })
            } else {
                json!({ "path": relativo, "name": nome, "kind": "note", "children": [] })
            }
        })
        .collect()
}
