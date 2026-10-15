# 全画面をRustで書き、WebAssemblyで動かす

画面、入力の検査、候補、キー操作、通信、通知は `crates/kamoku-ui/src/` にあります。ブラウザの標準入力と意味を持つHTMLを使い、既存のアドレスとCSSを引き継いでいます。

JavaScriptとして配信するのは、WASMの読み込みを始めるローダーと、wasm-bindgenが生成する接続コードです。会計や画面の処理をJavaScriptへ戻したり、以前のJavaScriptをRustから実行したりはしません。開発用のNodeスクリプトとPlaywrightは配信対象に含めません。

## 生成してから本体をビルドする

Rust 1.88以上、Python 3、Cコンパイラを用意し、最初に次を実行します。

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129 --locked
bash tools/build-rust-ui.sh
cargo run -p kamoku-app -- demo --no-open
```

生成物は `ui/rust/pkg/` に置き、Gitには含めません。`kamoku-app` は必要な生成物が無ければビルドを失敗させます。Rustを変更したら、WASMを再生成してから本体をビルドしてください。`--ui-dir ui` で開発する場合もWASMの再生成は必要です。

`CARGO_TARGET_DIR` でビルド先を指定できます。ビルド用スクリプトはツールやターゲットを自動導入しません。

## 変更する場所

| 場所 | 役割 |
|---|---|
| `app/screens/` | ホーム、登録、仕訳帳、未確認、取込、帳票、登録簿、設定、初期設定 |
| `app/core/` | API、キャッシュ、URLと履歴、キー、テーマ、DOM、画面ごとの監視と解放 |
| `app/components/` | 候補、欄の説明と誤り、確認ダイアログ、通知 |
| `input.rs`、`interaction.rs`、`phrase.rs` | 整数の金額式、日付、検索文字、IME、改行の純ロジック |
| `ui/css/` | 色・密度・レスポンシブ表示。Rustはクラスを使って接続 |

画面は `mount(Context) -> Result<Screen, JsValue>` を持ちます。`Screen.update` は同じ画面の条件変更に使います。監視は `Scope` に登録し、画面や再描画の終了時に解放します。提出済みの書き込みと、離れた画面の表示更新は区別します。

キーの処理は共通の `ime_ignore` を先に呼びます。文字列はTextノードとして入れ、HTMLとして解釈させません。通信は同じオリジンのAPI境界へ集めます。金額JSONはRustの整数として読み、JavaScriptの浮動小数点数へ渡しません。

## 検証

```sh
cargo test -p kamoku-ui --locked
cargo clippy -p kamoku-ui --target wasm32-unknown-unknown --locked -- -D warnings
tools/check-all.sh
e2e/run-real.sh
python3 tools/test-rust-ui.py
```

CIはWASMを生成・検査してから、その生成物をLinux/macOSの本体試験、Rust 1.88、実サーバーのブラウザ試験へ渡します。生成物が無い状態で以前の画面へ戻す処理は持ちません。

速さは同じ合成データと環境で測ります。Rustの採用だけでは高速化を証明できません。実機のIME、読み上げ、Safari、Firefox、Windowsの確認範囲は[試験の地図](testing.md)へ分けて記録します。
