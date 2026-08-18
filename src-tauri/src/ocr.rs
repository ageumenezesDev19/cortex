use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_shell::ShellExt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrBlock {
    pub text: String,
    /// `[x, y, width, height]`, normalised 0–1 with the origin at the TOP left.
    /// Vision reports bottom-left; the Swift helper flips it once so nothing
    /// downstream has to remember which convention it is looking at.
    pub bbox: [f64; 4],
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrResult {
    pub text: String,
    pub blocks: Vec<OcrBlock>,
    pub lang: String,
    pub ocr_at: String,
    /// What the screenshot is about. `None` means never classified; `Some`
    /// that carries nothing means classified and found to be nothing in
    /// particular — the difference is what stops a bare screenshot from being
    /// sent to the model on every reindex.
    #[serde(default)]
    pub marks: Option<crate::marks::Marks>,
}

/// `print-01.png` → `print-01.ocr.json`, sitting next to the image so the vault
/// stays self-describing without the database.
pub fn json_path(image: &Path) -> PathBuf {
    let stem = image.file_stem().map(|s| s.to_string_lossy().to_string());
    match stem {
        Some(stem) => image.with_file_name(format!("{stem}.ocr.json")),
        None => image.with_extension("ocr.json"),
    }
}

pub fn read_cached(image: &Path) -> Option<OcrResult> {
    let json = json_path(image);
    let image_mtime = crate::vault::mtime_of(image);
    if crate::vault::mtime_of(&json) < image_mtime {
        return None;
    }
    let raw = std::fs::read_to_string(&json).ok()?;
    serde_json::from_str(&raw).ok()
}

/// Runs the image through the Vision helper unless a fresh `.ocr.json` is
/// already on disk. Recognition is on-device and free; the cost is ~1s per
/// screenshot, which is why the caller keeps this on a serial queue.
pub fn ensure(app: &AppHandle, image: &Path) -> Result<OcrResult> {
    if let Some(cached) = read_cached(image) {
        return Ok(cached);
    }
    let result = run_helper(app, image)?;
    std::fs::write(json_path(image), serde_json::to_vec_pretty(&result)?)?;
    Ok(result)
}

/// Rewrites the sidecar after the marks are decided, so the next index reads
/// them instead of asking again.
pub fn save(image: &Path, result: &OcrResult) -> Result<()> {
    std::fs::write(json_path(image), serde_json::to_vec_pretty(result)?)?;
    Ok(())
}

fn run_helper(app: &AppHandle, image: &Path) -> Result<OcrResult> {
    let command = app
        .shell()
        .sidecar("cortex-ocr")
        .map_err(|e| anyhow!("OCR helper not bundled: {e}"))?
        .arg(image.to_string_lossy().to_string());

    let output = tauri::async_runtime::block_on(command.output())
        .map_err(|e| anyhow!("OCR helper failed to run: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("OCR helper exited with error: {}", stderr.trim()));
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}
