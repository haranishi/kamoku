#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
#
# 本物のサーバー（kamoku）を起動して、ブラウザの通し試験を流す。
#   e2e/run-real.sh [all]      スモーク、操作、契約、スクリーンショットの順にすべて
#   e2e/run-real.sh flows      操作の試験だけ
#   e2e/run-real.sh screens    スクリーンショットだけ（screens-out/ に出る）
#   e2e/run-real.sh contract   模擬サーバーと本物の応えの形を比べる契約試験だけ
#   e2e/run-real.sh docs-images READMEと「画面の紹介」の画像を docs/images/ に作る（all には含まれない）
# flows / screens / docs-images の追加引数は、その段階へ渡す。
# 既定ではRust UIのWASMを生成してからcargo buildで再ビルドし、Cargoが返す実行ファイルを使う。
# KAMOKU_BINを明示した場合だけ、既存の実行ファイルをそのまま使う。
#
# 127.0.0.1 の Cookie は、ポートが違っても共有される。そのため、段階ごとにサーバーを起動し直し、
# 前の段階のサーバーは止めてから次へ進む。試験が仕訳を足すので、段階ごとに新しい帳簿で始める。
set -euo pipefail

usage() {
  echo "使い方: e2e/run-real.sh [all | flows | contract | screens | docs-images | --help]"
  echo "  flows / screens / docs-images だけ、後ろにその段階の引数を追加できます。"
  echo "  既定でWASMと本体を再ビルドします。KAMOKU_BINを指定すると既存の実行ファイルと埋込UIを使います。"
}

WHICH="${1-all}"
if [ "$#" -gt 0 ]; then shift; fi
case "$WHICH" in
  all | contract)
    [ "$#" = 0 ] || { echo "$WHICH は追加引数を受け付けません。" >&2; usage >&2; exit 2; } ;;
  flows | screens | docs-images) ;;
  --help) [ "$#" = 0 ] || { usage >&2; exit 2; }; usage; exit 0 ;;
  *) echo "不明な段階: $WHICH" >&2; usage >&2; exit 2 ;;
esac
EXTRA=("$@")
cd "$(dirname "$0")"

missing=()
if command -v node >/dev/null 2>&1; then
  version="$(node --version 2>/dev/null || true)"
  if ! [[ "$version" =~ ^v([0-9]+)\. ]] || [ "${BASH_REMATCH[1]:-0}" -lt 24 ]; then
    missing+=("Node 24以上を導入（現在: ${version:-取得できません}）")
  fi
else
  missing+=("node (24以上) をPATHに追加")
fi
if [ "${KAMOKU_BIN+x}" = x ]; then
  BIN="$KAMOKU_BIN"
  if [ -f "$BIN" ] && [ -x "$BIN" ]; then
    case "$BIN" in /*) ;; *) BIN="$PWD/$BIN" ;; esac
  else
    missing+=("KAMOKU_BINに既存の実行可能ファイルを指定")
  fi
else
  command -v cargo >/dev/null 2>&1 || missing+=("cargo をPATHに追加")
  if command -v wasm-bindgen >/dev/null 2>&1; then
    if ! wasm_version="$(wasm-bindgen --version 2>/dev/null)" || [ "$wasm_version" != "wasm-bindgen 0.2.129" ]; then
      missing+=("wasm-bindgen 0.2.129: cargo install wasm-bindgen-cli --version 0.2.129 --locked")
    fi
  else
    missing+=("wasm-bindgen 0.2.129: cargo install wasm-bindgen-cli --version 0.2.129 --locked")
  fi
  if command -v rustup >/dev/null 2>&1; then
    if ! targets="$(rustup target list --installed 2>/dev/null)" || ! [[ "$targets" =~ (^|$'\n')wasm32-unknown-unknown($|$'\n') ]]; then
      missing+=("WASMターゲット: rustup target add wasm32-unknown-unknown")
    fi
  else
    missing+=("rustup をPATHに追加し、rustup target add wasm32-unknown-unknown")
  fi
fi
if [ "$WHICH" = all ] || [ "${KAMOKU_BIN+x}" != x ]; then
  command -v python3 >/dev/null 2>&1 || missing+=("python3 をPATHに追加（Cargoの出力の読取・スモーク試験）")
fi
PLAYWRIGHT="./node_modules/@playwright/test/cli.js"
if [ "$WHICH" != contract ] && [ ! -f "$PLAYWRIGHT" ]; then
  missing+=("Playwrightを事前に準備: cd e2e && npm ci")
fi
if [ "${#missing[@]}" -gt 0 ]; then
  echo "必要な前提が不足しています。試験を開始しません。" >&2
  printf '  - %s\n' "${missing[@]}" >&2
  exit 2
fi
if [ "$WHICH" != contract ]; then
  node -e '
    try {
      require("@playwright/test");
    } catch {
      console.error("ローカルのPlaywrightを事前に準備してください: cd e2e && npm ci");
      process.exit(2);
    }
  '
fi

TMP="$(mktemp -d)"
PIDS=()

stop_servers() {
  for pid in ${PIDS[@]+"${PIDS[@]}"}; do
    kill "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true   # 止めた印の「Terminated」を出さずに待つ
  done
  PIDS=()
}
cleanup() { stop_servers; rm -rf "$TMP"; }
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

if [ "${KAMOKU_BIN+x}" = x ]; then
  echo "== 実行ファイル: KAMOKU_BINの指定（ビルドを省略）: 埋込UIを使用"
else
  echo "== 実行ファイル: WASM生成とcargo buildで再ビルド（既定）"
  if ! (cd .. && bash tools/build-rust-ui.sh); then
    echo "Rust UIのWASM生成に失敗しました。試験を開始しません。" >&2
    exit 1
  fi
  if ! (cd .. && cargo build -p kamoku-app --bin kamoku --locked --message-format=json-render-diagnostics > "$TMP/build.jsonl"); then
    echo "Rustのビルドに失敗しました。試験を開始しません。" >&2
    exit 1
  fi
  BIN="$(python3 ../tools/cargo-executable.py "$TMP/build.jsonl")"
  if [ ! -f "$BIN" ] || [ ! -x "$BIN" ]; then
    echo "Cargoが返した実行ファイルが存在しないか、実行できません。" >&2
    exit 1
  fi
fi

if [ "$WHICH" = all ]; then
  echo "== スモーク試験（本物のバイナリ）"
  python3 ../tools/smoke-demo.py --bin "$BIN"
fi

# 起動して、アドレスと合言葉を <名前>_URL と <名前>_TOKEN に入れる。
start() {
  local name="$1"; shift
  "$@" > "$TMP/$name.log" 2>&1 < /dev/null &
  local pid="$!"
  PIDS+=("$pid")
  local tries=0 line=""
  while [ -z "$line" ]; do
    line="$(grep -o 'http://127.0.0.1:[0-9]*/?t=[0-9a-f]*' "$TMP/$name.log" 2>/dev/null | head -1 || true)"
    if ! kill -0 "$pid" 2>/dev/null; then
      echo "$name が起動前に終了しました。サーバーログは合言葉を含むため表示しません。" >&2
      exit 1
    fi
    tries=$((tries + 1))
    if [ "$tries" -gt 300 ]; then
      echo "$name の起動が時間切れになりました。サーバーログは合言葉を含むため表示しません。" >&2
      exit 1
    fi
    [ -n "$line" ] || sleep 0.2
  done
  printf -v "${name}_URL" '%s' "${line%%/?t=*}"
  printf -v "${name}_TOKEN" '%s' "${line##*t=}"
}

if [ "$WHICH" = "all" ] || [ "$WHICH" = "flows" ]; then
  echo "== 操作の試験（本物のサーバー）"
  start DEMO "$BIN" demo --no-open --port 0
  start SETUP "$BIN" serve --data "$TMP/setup" --no-open --port 0
  KAMOKU_URL="$DEMO_URL" KAMOKU_TOKEN="$DEMO_TOKEN" \
    KAMOKU_SETUP_URL="$SETUP_URL" KAMOKU_SETUP_TOKEN="$SETUP_TOKEN" \
    node "$PLAYWRIGHT" test --project=flows ${EXTRA[@]+"${EXTRA[@]}"}
  stop_servers
fi

if [ "$WHICH" = "all" ] || [ "$WHICH" = "contract" ]; then
  echo "== 契約試験（模擬サーバーと本物の応えの形）"
  start DEMO "$BIN" demo --no-open --port 0
  KAMOKU_URL="$DEMO_URL" KAMOKU_TOKEN="$DEMO_TOKEN" node --test contract.test.mjs
  stop_servers
fi

if [ "$WHICH" = "docs-images" ]; then
  echo "== 文書用の画像（本物のデモを撮る）"
  mkdir -p ../docs/images
  start DEMO "$BIN" demo --no-open --port 0
  KAMOKU_URL="$DEMO_URL" KAMOKU_TOKEN="$DEMO_TOKEN" node docs-images.mjs ${EXTRA[@]+"${EXTRA[@]}"}
  stop_servers
fi

if [ "$WHICH" = "all" ] || [ "$WHICH" = "screens" ]; then
  echo "== スクリーンショット（本物のサーバー）"
  start DEMO "$BIN" demo --no-open --port 0
  start SHOTS "$BIN" serve --data "$TMP/shots" --no-open --port 0
  KAMOKU_URL="$DEMO_URL" KAMOKU_TOKEN="$DEMO_TOKEN" \
    KAMOKU_SETUP_SHOTS_URL="$SHOTS_URL" KAMOKU_SETUP_SHOTS_TOKEN="$SHOTS_TOKEN" \
    node "$PLAYWRIGHT" test --project=screens ${EXTRA[@]+"${EXTRA[@]}"}
  stop_servers
fi
