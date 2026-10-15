# Rustで書いた仕訳帳を別の経路で確かめる

検索・期間と状態の絞り込み・50件ずつの一覧をRustで動かす、最初の移行範囲です。通常の画面が引き続き既定です。取引の登録・訂正・詳細は通常の画面へ進み、ブラウザで戻ると検索条件と選んだ仕訳へ戻ります。全画面の移行や、通常画面との機能の同等性を達成したものではありません。

## ブラウザを残す理由

日本語入力、意味を持つHTMLの表、既存のブラウザ試験を引き継ぐため、RustをWASMにコンパイルしてDOMを操作します。初回の範囲では `web-sys` を使い、大きなUIの枠組みを加えません。ネイティブの独自描画へ移る案は、入力・読み上げ・画面試験を広く作り直すため採っていません。

検索条件・通信・画面・キーの処理はRustです。HTML/CSSと、生成されたJavaScriptの接続コード、初期化と起動失敗表示だけを行う短いローダーも必要です。会計の金額はJSONをRustの整数として読み、JavaScriptの浮動小数点数を通しません。

## 用意してからビルドする

Rust 1.88以上、Python 3に加え、WASMターゲットと本体のcrateに合う版のCLIが必要です。依存の取得とコンパイルには容量が必要なので、開発用ストレージを確保してから実行してください。

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129 --locked
bash tools/build-rust-ui.sh
cargo run -p kamoku-app -- demo --no-open
```

起動時のアドレスから認証した後、同じオリジンの `/rust/index.html` を開きます。WASMを作ってから本体をビルドすることで、生成物も実行ファイルに埋め込みます。`--ui-dir ui` でディスクから読む場合も、Rustの変更後はWASMを作り直します。ビルド用スクリプトは道具を自動導入しません。

生成物はGitに含めません。未ビルドや起動失敗の場合は、プレビューの起動失敗表示から通常の画面へ戻れます。通常の画面を編集するだけなら、WASM用の道具は不要です。

## 変更したら確かめる

通常の[PR前の試験](testing-procedure.md)に加え、次を流します。ブラウザは既存の手順で準備してください。

```sh
cargo test -p kamoku-ui --locked
cargo clippy -p kamoku-ui --target wasm32-unknown-unknown --locked -- -D warnings
cargo +1.88 check -p kamoku-ui --target wasm32-unknown-unknown --locked
bash tools/build-rust-ui.sh
python3 tools/test-rust-ui.py
```

最後の試験は、本体をビルドして合成データのサーバーを起動し、埋め込まれたWASMを本物のChromiumで操作します。検索、合計、ページ、履歴、詳細からの復帰、IMEの模擬、応答の競合、通信失敗からの再試行、安全な文字表示を確かめます。Rust 1.88の検査には、そのツールチェーンにもWASMターゲットが必要です。

`e2e/rust-ui-results/` に390／768／1440pxの画像と計測JSONを出します。専用の `rust-ui` CIも同じ手順を使います。アップロードするのは合成データの画像と計測値だけで、認証URL・サーバーログ・トレース・録画を含めません。

## 速さと未確認の範囲

計測は、通常画面とプレビューの表示準備時間、検索して次の描画までの時間、生成物のバイト数です。表示準備はHTTPキャッシュのない別々のContextで5回ずつ、実行順を交互にして測ります。ブラウザ本体・OS・サーバーは起動済みです。CI上の1環境、debugのサーバー、releaseのWASMという条件で、表示準備には試験側の待ち合わせも含まれます。Rustの採用自体を高速化の証拠にはしません。

Safari・Firefox・Windowsの実際の日本語入力と読み上げは別途、人が確認します。表の横はみ出し、キーボード、文字の表示は自動試験で確かめ、画像を見た評価と試験の成功を分けて記録します。
