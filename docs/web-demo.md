# ブラウザのお試し版

インストールもサーバーも無しで、静的ファイルを開くだけで kamoku を操作する版です。会計の処理と SQLite はブラウザの中のメモリで動き、入力は端末の外へ出ません。再読み込みかタブを閉じると、架空の帳簿に戻ります。

## 仕組み

画面（`kamoku-ui`）は、これまで通り `/api/v1/...` を `window.fetch` で呼びます。`web/shim.js` が先に WASM の帳簿（`kamoku-web`）を開き、`/api/v1/` だけをメモリ上の処理へ渡します。それ以外の取得（WASM や CSS）は、元の `fetch` のままです。帳簿の WASM が途中で例外（panic）を投げたら、以後の `/api/v1/` にはサーバーの想定外の失敗と同じ形の 500 を返し、再読み込みを案内します。ダウンロードのリンク（`/api/v1/` を指す）を修飾キーつきや中ボタンで押したときは、通常のクリックに置き換えます。配信先に `/api/` が無いので、新しいタブやブラウザの直接のダウンロードにさせないためです。

入口は `kamoku-app` の `web::Book` です。経路表、本文の上限、Content-Type の検査はサーバーと同じ `dispatch` を使います。サーバー固有の待ち受け、合言葉、ファイルへの控えは wasm32 ではコンパイルしません。

「今日」は `index.html` の `<meta name="kamoku-demo-today" content="2026-09-30">` です。画面の `today()` はこの meta があるときだけその日付を返します。サーバー版の画面は meta が無いので、今まで通りです。仕訳の記録時刻と、書き出すファイル名の日付は、固定の今日ではなく端末の日付です。

案内の帯は `<meta name="kamoku-edition" content="web-demo">` のときだけ、お試し版の文面になります。帯の「ソース（GitHub）」の行き先は `<meta name="kamoku-source">` から読みます。画面のソースには外のURLを書かない決まり（`ui/dev/check-policy.mjs`）があるためです。受け入れ条件は [利用者の物語](spec/stories-web-demo.md) にあります。

## できないこと

- 帳簿の控え（バックアップ）は作れません。控えはファイルの複製で、この版にはファイルがありません。ソースから `kamoku demo` を動かします。
- 再読み込みで、足した仕訳は消えます。
- 配信元以外へは通信しません。GitHub へのリンクは、押すまで取得しません。
- 税の数字は参考値です。

## 組み立て

Rust 1.89 以上、`wasm32-unknown-unknown`、`wasm-bindgen` 0.2.129、Python 3、C のコンパイラ（clang）と、Rust が配る LLVM の ar（`rustup component add llvm-tools`）が要ります。macOS の `ar` は WASM のオブジェクトを捨てて SQLite が空になるので、macOS でも Linux でも LLVM の ar を使います。スクリプトは道具を入れず、足りなければ入れ方を示して止まります。最初のビルドは数分かかります。

```
bash tools/build-web-demo.sh
```

出力先の既定は `target/web-demo` です。引数で変えられます（相対パスは実行した場所が基準）。出力先は、空か、前回の出力のときだけ消して作り直します。前回の出力とは、`build-info.json`・`web/shim.js`・`THIRD_PARTY_LICENSES.txt` がそろい、直下に出力以外のものが無いフォルダです。それ以外は、何も消さずに止まります。`CARGO_TARGET_DIR` は、設定されていればそのまま使います。

公開する WASM に手元の絶対パス（利用者名を含む）が残らないよう、Rust の `--remap-path-prefix` と C の `-ffile-prefix-map` で置き換えます。最後に `tools/check-web-demo-output.py` が出力の全ファイルを調べ、`/Users/`・`/home/`・`/Volumes/`・HOME の値、秘密の値に見える行、25MiB を超えるファイルがあれば失敗します。

ES モジュールと WASM は `file://` では読めません。HTTP で配ってください（手元なら `python3 -m http.server -d target/web-demo`）。

中身は `index.html`、`favicon.svg`、`css/`、画面の WASM（`rust/pkg/`）、`web/shim.js`、`web/demo.css`、帳簿の WASM（`web/pkg/`）、`build-info.json`（元のコミット。コミットしていない変更があれば `"modified": true`。日時は無し）、`THIRD_PARTY_LICENSES.txt` です。パスはすべて相対です。ディレクトリの末尾が `/` のサブパスへ置けます。

配信側の CSP は `script-src 'self' 'wasm-unsafe-eval'` と `connect-src 'self'` を想定しています。インラインの script と style、外部の書体、画像、CDN は使いません。`.wasm` の種類は `application/wasm` にしてください。

## 試験

`cargo test -p kamoku-app --lib web::` が、サーバーと同じ振り分けと同じデモの末尾ハッシュを見ます。ブラウザの通し試験は、組み立てたあと `e2e` で `npx playwright test -c web-demo.config.mjs` です。静的サーバーは Python の標準ライブラリで、`/kamoku-demo/` に置いたものとして配ります。

## サーバー版との差

`Method` と `Mode` は、HTTP の受け口から切り出して共有しています。受け口は今まで通り再公開します。ゲートのセキュリティ見出し（CSP など）は、ブラウザ版の API 応答には付けません。接続の前で付けるものだからです。ネイティブの `Book::call` の控えは、サーバーと同じくファイルを作ります。ブラウザの画面は、控えのボタンの代わりに理由を出します。`kamoku-web` はネイティブでは空のライブラリで、WASM のときだけ `DemoBook` を出します。
