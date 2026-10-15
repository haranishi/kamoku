#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
#
# 手元で、PRの前に通す検査を、まとめて実行する。
#   tools/check-all.sh          全部
#   tools/check-all.sh --quick  cargo-denyと最小のRustの確認を明示して省く
# 失敗した検査の名前を最後に一覧する。ブラウザを使う試験(e2e/)は含まない
# (docs/development.md の手順で、cd e2e && ./run-real.sh を別に実行する)。
set -uo pipefail

usage() {
  echo "使い方: tools/check-all.sh [--quick | --help]"
  echo "  --quick  Rust 1.88とcargo-denyの検査を省略する（完全な検査ではありません）"
}

quick=0
[ "$#" -le 1 ] || { usage >&2; exit 2; }
if [ "$#" = 1 ]; then
  case "$1" in
    --quick) quick=1 ;;
    --help) usage; exit 0 ;;
    *) echo "不明な引数: $1" >&2; usage >&2; exit 2 ;;
  esac
fi

cd "$(dirname "$0")/.." || exit 2
missing=()
for tool in cargo rustup wasm-bindgen node python3 uvx git; do
  command -v "$tool" >/dev/null 2>&1 || missing+=("$tool をPATHに追加")
done
if command -v node >/dev/null 2>&1; then
  version="$(node --version 2>/dev/null)"
  if ! [[ "$version" =~ ^v([0-9]+)\. ]] || [ "${BASH_REMATCH[1]:-0}" -lt 24 ]; then
    missing+=("Node 24以上を導入（現在: ${version:-取得できません}）")
  fi
fi
if command -v wasm-bindgen >/dev/null 2>&1; then
  if ! wasm_version="$(wasm-bindgen --version 2>/dev/null)" || [ "$wasm_version" != "wasm-bindgen 0.2.129" ]; then
    missing+=("wasm-bindgen 0.2.129: cargo install wasm-bindgen-cli --version 0.2.129 --locked")
  fi
fi
if command -v rustup >/dev/null 2>&1; then
  if ! targets="$(rustup target list --installed 2>/dev/null)" || ! [[ "$targets" =~ (^|$'\n')wasm32-unknown-unknown($|$'\n') ]]; then
    missing+=("WASMターゲット: rustup target add wasm32-unknown-unknown")
  fi
fi
msrv=""
if [ "$quick" = 0 ]; then
  if command -v rustup >/dev/null 2>&1; then
    if toolchains="$(rustup toolchain list 2>/dev/null)"; then
      while IFS= read -r line; do
        name="${line%% *}"
        case "$name" in
          1.88 | 1.88.0 | 1.88-* | 1.88.0-*) msrv="$name"; break ;;
        esac
      done <<< "$toolchains"
    fi
    [ -n "$msrv" ] || missing+=("Rust 1.88: rustup toolchain install 1.88 --profile minimal")
    if [ -n "$msrv" ]; then
      if ! targets="$(rustup target list --installed --toolchain "$msrv" 2>/dev/null)" || ! [[ "$targets" =~ (^|$'\n')wasm32-unknown-unknown($|$'\n') ]]; then
        missing+=("Rust 1.88のWASMターゲット: rustup target add wasm32-unknown-unknown --toolchain $msrv")
      fi
    fi
  else
    missing+=("rustup をPATHに追加し、rustup toolchain install 1.88 --profile minimal")
  fi
  command -v cargo-deny >/dev/null 2>&1 || missing+=("cargo-deny: cargo install cargo-deny --locked")
fi
if [ "${#missing[@]}" -gt 0 ]; then
  echo "必要な前提が不足しています。検査を開始しません。" >&2
  echo "必要なコマンド: cargo rustup wasm-bindgen (0.2.129) node (24以上) python3 uvx git" >&2
  echo "使用中のRustにwasm32-unknown-unknownターゲットが必要です。" >&2
  [ "$quick" = 1 ] || echo "完全モードでは Rust 1.88 / cargo-deny も必要です。" >&2
  printf '  - %s\n' "${missing[@]}" >&2
  exit 2
fi
if [ "$quick" = 1 ]; then
  echo "quick: 明示的に省略: Rust 1.88、依存のcargo-deny検査"
fi
failed=()

step() {
  local name="$1"; shift
  printf '\n== %s\n' "$name"
  if "$@"; then echo "   -> 通過"; else echo "   -> 失敗"; failed+=("$name"); fi
}

step "Rust: 書式" cargo fmt --all -- --check
step "Rust UI: WASM生成" bash tools/build-rust-ui.sh
step "Rust UI: WASM clippy" cargo clippy -p kamoku-ui --target wasm32-unknown-unknown --locked -- -D warnings
step "Rust: clippy" cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
step "Rust: 試験" cargo test --workspace --locked
step "Rust: 文書" env RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked
if [ "$quick" = 0 ]; then
  step "Rust: 最小の1.88" cargo "+$msrv" check --workspace --locked
  step "Rust UI: 最小の1.88" cargo "+$msrv" check -p kamoku-ui --target wasm32-unknown-unknown --locked
  step "依存: cargo deny" cargo deny --locked check
fi
step "画面: 単体試験" node --test ui/tests/
step "画面: 色のコントラスト" node ui/dev/check-contrast.mjs
step "画面: 禁止事項の検索" node ui/dev/check-policy.mjs
step "道具: 回帰試験" python3 -m unittest discover -s tools/tests -v
step "文書: リンクと行数" python3 tools/check-links.py
step "台帳: 固定値の別計算" python3 tools/verify-ledger-golden.py
step "仕様: 条件と試験の対応" python3 tools/check-spec-trace.py --strict
step "ライセンス表示(REUSE)" uvx --offline --from "reuse[charset-normalizer]" reuse lint

printf '\n== まとめ\n'
if [ "${#failed[@]}" = 0 ]; then
  if [ "$quick" = 1 ]; then
    echo "quickの範囲は通過しました（Rust 1.88とcargo-denyの省略あり）。"
  else
    echo "完全モードの検査がすべて通過しました。"
  fi
  exit 0
fi
printf '失敗: %s\n' "${failed[@]}"; exit 1
