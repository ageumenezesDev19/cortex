use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use anyhow::Result;
use notify::RecursiveMode;
use notify_debouncer_full::new_debouncer;
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::index::{Counts, Indexer};
use crate::{db, ocr, vault};

/// Long enough that saving in an editor is one event, short enough that the
/// result is on screen before you switch windows.
const DEBOUNCE: Duration = Duration::from_millis(300);

pub const STATUS_EVENT: &str = "index:status";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Status {
    phase: &'static str,
    counts: Counts,
    current: Option<String>,
    pending: usize,
}

pub fn spawn(app: AppHandle, root: PathBuf) {
    std::thread::spawn(move || {
        if let Err(error) = run(&app, root) {
            eprintln!("[cortex] indexer stopped: {error:#}");
        }
    });
}

fn run(app: &AppHandle, root: PathBuf) -> Result<()> {
    let conn = db::open(&vault::db_path(&root))?;
    let mut indexer = Indexer::new(root.clone(), conn);

    let (tx, rx) = mpsc::channel();
    let mut debouncer = new_debouncer(DEBOUNCE, None, tx)?;
    debouncer.watch(&root, RecursiveMode::Recursive)?;

    emit(app, &indexer, "scanning", None, 0);
    let _ = indexer.sync_topics();
    let pending = indexer.scan()?;
    emit(app, &indexer, "ocr", None, pending.len());

    // Vision is expensive and the machine has other things to do: one image at
    // a time, on this thread, is deliberate.
    for (done, image) in pending.iter().enumerate() {
        let label = display_name(&root, image);
        emit(app, &indexer, "ocr", Some(label), pending.len() - done);
        run_ocr(app, &mut indexer, image);
    }
    embed_waiting(app, &indexer);
    emit(app, &indexer, "idle", None, 0);

    for result in rx {
        let Ok(events) = result else { continue };
        let mut touched = false;
        for event in events {
            for path in &event.paths {
                touched |= apply(app, &mut indexer, path);
            }
        }
        if touched {
            embed_waiting(app, &indexer);
            emit(app, &indexer, "idle", None, 0);
        }
    }
    Ok(())
}

/// Vectors for whatever has none, on this thread and for the same reason the
/// OCR is on it: slow, external, per item, and part of the index rather than a
/// button someone has to remember.
///
/// Silent with no Gemini key — that is the optional half of the app being
/// absent, not a failure — and quiet again after a refusal, which the next
/// change to the vault retries.
fn embed_waiting(app: &AppHandle, indexer: &Indexer) {
    let root = indexer.root().to_path_buf();
    if !crate::embed::wanted(&root) {
        return;
    }
    let model = crate::settings::load(&root).embedding_model;
    // A cancel stops the pass it was pressed during, not the feature — the
    // switch in the panel is what turns this off. Without the reset, one cancel
    // would leave the automatic pass dead until someone ran it by hand.
    crate::commands::resume_embedding();

    let outcome = crate::embed::drain(
        &root,
        indexer.connection(),
        &model,
        |done, total| emit_embedding(app, indexer, done, total),
        crate::commands::embedding_cancelled,
    );
    if let Err(reason) = outcome {
        eprintln!("[cortex] embedding parou: {reason}");
    }
}

fn emit_embedding(app: &AppHandle, indexer: &Indexer, done: u32, total: u32) {
    let _ = app.emit(
        STATUS_EVENT,
        Status {
            phase: "embedding",
            counts: indexer.counts().unwrap_or_default(),
            current: Some(format!("{done} de {total}")),
            pending: total.saturating_sub(done) as usize,
        },
    );
}

/// Returns whether the change altered the index.
fn apply(app: &AppHandle, indexer: &mut Indexer, path: &Path) -> bool {
    let root = indexer.root().to_path_buf();
    if vault::is_ignored(&root, path) {
        return false;
    }
    if vault::is_topic_manifest(&root, path) {
        return indexer.sync_topics().is_ok();
    }
    let is_markdown = vault::is_markdown(path);
    let is_image = vault::is_image(path);
    if !is_markdown && !is_image {
        return false;
    }

    if !path.exists() {
        let removed = if is_markdown {
            indexer.remove_note(path)
        } else {
            indexer.remove_asset(path)
        };
        return removed.is_ok();
    }

    if is_markdown {
        return indexer.index_note(path).unwrap_or(false);
    }
    if indexer.asset_is_current(path).unwrap_or(false) {
        return false;
    }
    emit(app, indexer, "ocr", Some(display_name(&root, path)), 1);
    run_ocr(app, indexer, path)
}

fn run_ocr(app: &AppHandle, indexer: &mut Indexer, image: &Path) -> bool {
    match ocr::ensure(app, image) {
        Ok(mut result) => {
            classify(indexer.root(), image, &mut result);
            indexer.index_asset(image, &result).is_ok()
        }
        Err(error) => {
            eprintln!("[cortex] OCR failed for {}: {error:#}", image.display());
            false
        }
    }
}

/// Decides what the screenshot is about, once, and writes it back into the
/// sidecar. A screenshot that already carries marks is left alone even when
/// they are empty — that is a verdict, not a gap.
fn classify(root: &Path, image: &Path, result: &mut ocr::OcrResult) {
    if result.marks.is_some() || result.text.trim().is_empty() {
        return;
    }
    let settings = crate::settings::load(root);
    let relative = vault::relative(root, image).unwrap_or_default();
    // A screenshot under an excluded path is never sent anywhere, so the rule
    // decides alone.
    let may_ask = !crate::ask::is_excluded(&relative, &settings.excluded);

    let marks = crate::classify::marks(
        root,
        &result.text,
        (&settings.groq_model, &settings.gemini_model),
        may_ask,
    );

    // A mark is also a topic, so the chip that already scopes the search can
    // scope it to "only styling" without any new interface. The name and the
    // icon come from the vocabulary file now, not from a match in the code.
    let vocabulary = crate::marks::load(root);
    for mark in marks.all() {
        let (name, icon) = vocabulary.presentation(&mark);
        let slug = crate::classify::slug_of(&mark);
        if let Err(error) =
            crate::topics::add_auto_member(root, &slug, &name, icon.as_deref(), &relative)
        {
            eprintln!("[cortex] could not file {relative} under {slug}: {error:#}");
        }
    }

    result.marks = Some(marks);
    if let Err(error) = ocr::save(image, result) {
        eprintln!("[cortex] could not store the marks for {}: {error:#}", image.display());
    }
}

fn display_name(root: &Path, path: &Path) -> String {
    vault::relative(root, path).unwrap_or_else(|_| path.display().to_string())
}

fn emit(
    app: &AppHandle,
    indexer: &Indexer,
    phase: &'static str,
    current: Option<String>,
    pending: usize,
) {
    let status = Status {
        phase,
        counts: indexer.counts().unwrap_or_default(),
        current,
        pending,
    };
    let _ = app.emit(STATUS_EVENT, status);
}
