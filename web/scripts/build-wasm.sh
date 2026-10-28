#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
OUT="$ROOT/web/public/wasm"
mkdir -p "$OUT"
cargo build --manifest-path "$ROOT/Cargo.toml" --lib --release --target wasm32-unknown-unknown
wasm-bindgen "$ROOT/target/wasm32-unknown-unknown/release/moodle_doc_converter.wasm" \
  --target web --out-dir "$OUT" --out-name converter
