# kamokuのブラウザ用アセット

全画面の処理は `crates/kamoku-ui/src/` のRustです。ここにはHTML、CSS、ローダー、WASM生成先と開発用の道具を置きます。[構成とビルド](../docs/rust-ui.md)を先に読んでください。

## 起動と変更

```sh
bash tools/build-rust-ui.sh
cargo run -p kamoku-app -- demo --no-open --ui-dir ui
```

`--ui-dir` はディスク上のアセットを読む開発用です。HTML/CSSは再読込で反映されます。Rustは先にWASMを再生成します。起動時の合言葉つきアドレスはログやIssueへ貼らないでください。

配布用の本体はアセットを実行ファイルへ埋め込みます。生成物が無ければビルドは失敗します。配信するJavaScriptは `boot.js`、互換経路の `rust/boot.js`、生成された `rust/pkg/kamoku_ui.js` だけです。開発用Nodeスクリプトは配りません。

## 確かめる

```sh
cargo test -p kamoku-ui --locked
node --test ui/tests/
node ui/dev/check-contrast.mjs
node ui/dev/check-policy.mjs
e2e/run-real.sh
```

Rustで純ロジック、Nodeで検査器、実サーバーのPlaywrightでDOM・キー・IME模擬・履歴・操作・撮影を確かめます。準備と変更別の手順は[試験の実行手順](../docs/testing-procedure.md)、移した条件の対応は[画面の試験の移行](../docs/rust-ui-tests.md)にあります。

## 模擬サーバー

```sh
node ui/dev/mock-server.mjs --port 8766
```

WASMは先に生成してください。`--today 2026-10-14`、`--tax-mode exempt`、`--empty`、`--fresh`、`--broken`、`--demo`、`--delay 400` で合成データの状態を変えられます。認証は省いている開発用です。契約試験が本物との応答形式の差を見張ります。

`npx --no-install playwright test` は模擬サーバーを操作・撮影・初期設定・初期設定撮影の4つ立てます。PR前は `e2e/run-real.sh` で実サーバーと埋め込みアセットを検査してください。特定バイナリは `KAMOKU_BIN` で指定します。

## 守る決まり

- DOMは `app/core/dom.rs` の要素とTextで組み立てます。入力文字をHTMLとして解釈させません。
- キーは共通の `ime_ignore` を先に呼びます。変換中のキーを妨げず、直後50msのEnterと押しっぱなしのEnterを抑止します。
- 通信はRustのAPI境界へ集め、同じオリジンの `/api/v1` だけを使います。
- 画面や再描画の監視は `Scope` で解放します。提出済み書き込みの完了は、旧DOMの更新と分けます。
- CSSは `tokens.css` の色を使います。赤字は符号と文字で表し、警告の赤と区別します。
- SVGのclassは属性へ設定します。SVGの `className` は読み取り専用のオブジェクトです。

根拠は[画面の仕様](../docs/spec/nfr.md)と[UX設計](../docs/ux-five-layers.md)にあります。
