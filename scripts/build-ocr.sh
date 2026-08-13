#!/usr/bin/env bash
# Compiles the Vision helper into the sidecar name Tauri expects for this host.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
triple="$(rustc -vV | awk '/^host:/ { print $2 }')"
output="$root/src-tauri/binaries/cortex-ocr-$triple"

mkdir -p "$(dirname "$output")"
swiftc -O -framework Vision -o "$output" "$root/ocr/main.swift"

echo "built $output"
