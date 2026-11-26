#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
# ブラウザだけで動くお試し版を組み立てる。道具は入れない。足りなければ入れ方を示して止まる。
# CARGO_TARGET_DIR は呼び側の設定のまま使う。出力先は別のディレクトリ（既定 target/web-demo）。
set -euo pipefail

# 出力先の相対パスは、呼び出した場所を基準にする（既定だけはリポジトリの target/web-demo）。
repo="$(cd "$(dirname "$0")/.." && pwd)"
if [[ $# -ge 1 ]]; then
  out="$1"
  case "$out" in
    /*) ;;
    *) out="$PWD/$out" ;;
  esac
else
  out="$repo/target/web-demo"
fi
cd "$repo"

command -v python3 >/dev/null 2>&1 || { echo "python3 が要ります。" >&2; exit 2; }
if ! rustup target list --installed 2>/dev/null | grep -qx wasm32-unknown-unknown; then
  echo "wasm32 のターゲットが要ります: rustup target add wasm32-unknown-unknown" >&2
  exit 2
fi
# SQLite（C）を WASM の静的ライブラリにまとめる ar。macOS の ar は WASM のオブジェクトを捨てるので、
# macOS でも Linux でも、Rust が配る LLVM の ar を使う。
host="$(rustc -vV | sed -n 's/^host: //p')"
llvm_ar="$(rustc --print sysroot)/lib/rustlib/$host/bin/llvm-ar"
if [[ ! -x "$llvm_ar" ]]; then
  echo "LLVM の ar が見つかりません（$llvm_ar）。" >&2
  echo "次で入れてから、もう一度実行してください: rustup component add llvm-tools" >&2
  exit 2
fi
export AR_wasm32_unknown_unknown="$llvm_ar"

# 公開する WASM に手元の絶対パス（利用者名を含む）を残さない。panic の場所（Rust）と
# __FILE__（SQLite の C）の両方を置き換える。--target を付けるので、build.rs には効かない。
target_dir="$(cargo metadata --no-deps --format-version=1 --locked | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
sysroot="$(rustc --print sysroot)"
remaps=(
  "${RUSTUP_HOME:-$HOME/.rustup}=/rustup"
  "$sysroot=/rustc-sysroot"
  "${CARGO_HOME:-$HOME/.cargo}=/cargo"
  "$PWD=/kamoku"
  "$target_dir=/target"
)
rustflags=()
cflags=""
for pair in "${remaps[@]}"; do
  if [[ "$pair" =~ [[:space:]] ]]; then
    echo "空白を含むパスは置き換えられません（$pair）。空白の無い場所で実行してください。" >&2
    exit 2
  fi
  # 同じパスに当たる置き換えが複数あると後のものが勝つので、内側（長い方）を後に書いてある。
  rustflags+=("--remap-path-prefix=$pair")
  cflags+=" -ffile-prefix-map=$pair"
done
# 画面のソースの「screens/home/」は、パスの漏洩検査が「/home/」として拾う。panic の場所の名前だけを変える。
rustflags+=("--remap-path-prefix=crates/kamoku-ui/src/app/screens/home=crates/kamoku-ui/src/app/screens/home-screen")
CARGO_ENCODED_RUSTFLAGS="$(IFS=$'\x1f'; echo "${rustflags[*]}")"
export CARGO_ENCODED_RUSTFLAGS
export CFLAGS_wasm32_unknown_unknown="${CFLAGS_wasm32_unknown_unknown:-}$cflags"

# 画面の WASM（ui/rust/pkg）。wasm-bindgen の版はこちらが確かめる。
bash tools/build-rust-ui.sh

CARGO_PROFILE_RELEASE_LTO=thin CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 \
  CARGO_PROFILE_RELEASE_PANIC=abort CARGO_PROFILE_RELEASE_STRIP=debuginfo \
  cargo build -p kamoku-web --target wasm32-unknown-unknown --release --locked

wasm="$target_dir/wasm32-unknown-unknown/release/kamoku_web.wasm"
if [[ ! -f "$wasm" ]]; then
  echo "帳簿の WASM がありません: $wasm" >&2
  exit 1
fi

pkg="$(mktemp -d)"
trap 'rm -rf "$pkg"' EXIT
wasm-bindgen "$wasm" --target web --no-typescript --out-dir "$pkg" --out-name kamoku_web
python3 tools/assemble-web-demo.py "$out" "$pkg"
# 手元の絶対パス・秘密に見える行・25MiB 超えがあれば失敗する。通れば各ファイルの大きさを出す。
python3 tools/check-web-demo-output.py "$out"
echo "お試し版を組み立てました: $out"
