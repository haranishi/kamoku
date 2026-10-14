#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
#
# 本物のサーバー（kamoku）を起動して、ブラウザの通し試験を流す。
#   e2e/run-real.sh            操作の試験（flows）と、スクリーンショット（screens）の両方
#   e2e/run-real.sh flows      操作の試験だけ
#   e2e/run-real.sh screens    スクリーンショットだけ（screens-out/ に出る）
#   e2e/run-real.sh contract   模擬サーバーと本物の応えの形を比べる契約試験だけ
#   e2e/run-real.sh docs-images READMEと「画面の紹介」の画像を docs/images/ に作る（all には含まれない）
# 段階の名前のあとに足した引数は、Playwright にそのまま渡す（例: e2e/run-real.sh flows layout.spec.mjs -g "AC-11.6"）。
# 実行ファイルは KAMOKU_BIN（既定は ../target/debug/kamoku。先に cargo build -p kamoku-app）。
#
# 127.0.0.1 の Cookie は、ポートが違っても共有される。そのため、段階ごとにサーバーを起動し直し、
# 前の段階のサーバーは止めてから次へ進む。試験が仕訳を足すので、段階ごとに新しい帳簿で始める。
set -euo pipefail
cd "$(dirname "$0")"
BIN="${KAMOKU_BIN:-../target/debug/kamoku}"
WHICH="${1:-all}"
[ "$#" -gt 0 ] && shift
EXTRA=("$@")
[ -x "$BIN" ] || { echo "実行ファイルがありません: $BIN（cargo build -p kamoku-app を先に）"; exit 2; }
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

# 起動して、アドレスと合言葉を <名前>_URL と <名前>_TOKEN に入れる。
start() {
  local name="$1"; shift
  "$@" > "$TMP/$name.log" 2>&1 < /dev/null &
  PIDS+=("$!")
  local tries=0 line=""
  while [ -z "$line" ]; do
    line="$(grep -o 'http://127.0.0.1:[0-9]*/?t=[0-9a-f]*' "$TMP/$name.log" 2>/dev/null | head -1 || true)"
    tries=$((tries + 1))
    if [ "$tries" -gt 300 ]; then
      echo "$name が起動しませんでした:"; cat "$TMP/$name.log"; exit 1
    fi
    [ -n "$line" ] || sleep 0.2
  done
  printf -v "${name}_URL" '%s' "${line%%/?t=*}"
  printf -v "${name}_TOKEN" '%s' "${line##*t=}"
}

if [ "$WHICH" = "all" ] || [ "$WHICH" = "flows" ]; then
  echo "== 操作の試験（本物のサーバー）"
  start DEMO "$BIN" demo --no-open --port 0 --ui-dir ../ui
  start SETUP "$BIN" serve --data "$TMP/setup" --no-open --port 0 --ui-dir ../ui
  KAMOKU_URL="$DEMO_URL" KAMOKU_TOKEN="$DEMO_TOKEN" \
    KAMOKU_SETUP_URL="$SETUP_URL" KAMOKU_SETUP_TOKEN="$SETUP_TOKEN" \
    npx playwright test --project=flows ${EXTRA[@]+"${EXTRA[@]}"}
  stop_servers
fi

if [ "$WHICH" = "all" ] || [ "$WHICH" = "contract" ]; then
  echo "== 契約試験（模擬サーバーと本物の応えの形）"
  start DEMO "$BIN" demo --no-open --port 0 --ui-dir ../ui
  KAMOKU_URL="$DEMO_URL" KAMOKU_TOKEN="$DEMO_TOKEN" node --test contract.test.mjs
  stop_servers
fi

if [ "$WHICH" = "docs-images" ]; then
  echo "== 文書用の画像（本物のデモを撮る）"
  mkdir -p ../docs/images
  start DEMO "$BIN" demo --no-open --port 0 --ui-dir ../ui
  KAMOKU_URL="$DEMO_URL" KAMOKU_TOKEN="$DEMO_TOKEN" node docs-images.mjs ${EXTRA[@]+"${EXTRA[@]}"}
  stop_servers
fi

if [ "$WHICH" = "all" ] || [ "$WHICH" = "screens" ]; then
  echo "== スクリーンショット（本物のサーバー）"
  start DEMO "$BIN" demo --no-open --port 0 --ui-dir ../ui
  start SHOTS "$BIN" serve --data "$TMP/shots" --no-open --port 0 --ui-dir ../ui
  KAMOKU_URL="$DEMO_URL" KAMOKU_TOKEN="$DEMO_TOKEN" \
    KAMOKU_SETUP_SHOTS_URL="$SHOTS_URL" KAMOKU_SETUP_SHOTS_TOKEN="$SHOTS_TOKEN" \
    npx playwright test --project=screens ${EXTRA[@]+"${EXTRA[@]}"}
  stop_servers
fi
