#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
# Build WASM before kamoku-app embeds ui/. This script never installs tools.
set -euo pipefail

cd "$(dirname "$0")/.."
expected="wasm-bindgen 0.2.129"
if ! command -v wasm-bindgen >/dev/null 2>&1 || [ "$(wasm-bindgen --version)" != "$expected" ]; then
  echo "Required: cargo install wasm-bindgen-cli --version 0.2.129 --locked" >&2
  exit 2
fi
command -v python3 >/dev/null 2>&1 || { echo "Required: python3" >&2; exit 2; }
# Optimize the browser artifact across Rust crates; native server profiles stay unchanged.
CARGO_PROFILE_RELEASE_LTO=thin CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 \
  CARGO_PROFILE_RELEASE_PANIC=abort CARGO_PROFILE_RELEASE_STRIP=debuginfo \
  cargo build -p kamoku-ui --target wasm32-unknown-unknown --release --locked
target_dir="$(cargo metadata --no-deps --format-version=1 --locked | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
mkdir -p ui/rust/pkg
wasm-bindgen "$target_dir/wasm32-unknown-unknown/release/kamoku_ui.wasm" \
  --target web --no-typescript --out-dir ui/rust/pkg --out-name kamoku_ui
echo "Rust UI built. Build kamoku-app next to embed these assets."
