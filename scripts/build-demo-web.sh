#!/usr/bin/env bash
# Builds the static web demo into dist-demo/. Node only — this is what the
# deploy host runs, so it must never reach for cargo.
#
# The vault data is copied into the output rather than into public/, because
# public/ ships inside the desktop app — a .app carrying a vault of invented
# notes would be a strange thing to hand someone.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

echo "→ build"
VITE_DEMO=1 npx vite build --outDir dist-demo --emptyOutDir

cp demo/data-pt.json dist-demo/demo-data-pt.json
cp demo/data-en.json dist-demo/demo-data-en.json

# Vite emits the ONNX runtime as an asset, but Transformers.js never asks for
# that copy — it fetches its own from a CDN at load time. Measured on the wire:
# zero requests to the local file, two to jsdelivr. Twenty-two megabytes that
# only ever cost upload time, so they go.
rm -f dist-demo/assets/ort-wasm-*.wasm
echo "→ pronto: dist-demo/ ($(du -sh dist-demo | cut -f1))"
