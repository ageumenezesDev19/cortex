#!/usr/bin/env bash
# Regenerates demo/data-{pt,en}.json — the chunks and vectors the web demo searches.
#
# Kept apart from the web build because it needs Rust and the embedding model on
# disk. The host that deploys the demo has neither, so its output is committed.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

echo "→ gerando os vetores com o mesmo modelo do desktop"
(cd src-tauri && cargo run --release --quiet --example gerar_demo)
