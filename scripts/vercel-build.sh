#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
WASM_TARGET="wasm32-unknown-unknown"
WASM_BINDGEN_VERSION="0.2.129"

if ! command -v rustup >/dev/null 2>&1; then
  if ! command -v curl >/dev/null 2>&1; then
    echo "Error: curl is required to install Rust on the Vercel build image." >&2
    exit 1
  fi

  rustup_installer="$(mktemp)"
  trap 'rm -f "$rustup_installer"' EXIT
  curl --proto '=https' --tlsv1.2 --fail --silent --show-error \
    https://sh.rustup.rs --output "$rustup_installer"
  sh "$rustup_installer" -y --profile minimal --default-toolchain stable
fi

# rustup's installer adds Cargo to this file; Vercel runs each build command in
# a fresh shell, so load it here before checking the toolchain.
if [[ -f "$HOME/.cargo/env" ]]; then
  # shellcheck disable=SC1091
  source "$HOME/.cargo/env"
fi

if ! command -v cargo >/dev/null 2>&1; then
  echo "Error: Cargo is unavailable after installing Rust." >&2
  exit 1
fi

rustup target add "$WASM_TARGET"

if ! command -v wasm-bindgen >/dev/null 2>&1 || \
  [[ "$(wasm-bindgen --version | awk '{print $2}')" != "$WASM_BINDGEN_VERSION" ]]; then
  cargo install wasm-bindgen-cli \
    --version "$WASM_BINDGEN_VERSION" \
    --locked \
    --force
fi

cd "$ROOT_DIR/web"
bun run build
