# 変更に合う試験を選び、PRの前に一通り通す

コードや文書を直す人向けの実行手順です。初回は「準備」と「PRの前」を読み、作業中は変更別の表を使ってください。各試験の役割と人が確かめる項目は[試験の地図](testing.md)にあります。

## 初回は道具とブラウザを用意する

RustとCのコンパイラに加え、次の道具が必要です。

| 道具 | 用途 |
|---|---|
| Rust 1.88以上、rustfmt、clippy | 本体のビルド・試験・静的検査 |
| Rust 1.88のツールチェーン、cargo-deny | 最小バージョンと依存の検査 |
| WASMターゲット、wasm-bindgen-cli 0.2.129 | Rust画面の生成 |
| Node 24以上、npm | 静的検査器の試験、Playwright |
| Python 3 | 文書・仕様・台帳・実行手順の検査 |
| uvx | REUSEによるライセンス表示の検査 |

導入後はリポジトリの直下で次を実行します。`npm ci` はlockファイルの版を使い、最後のコマンドは対応するChromiumを取得します。依存やブラウザの更新時にも実行してください。

```sh
rustup target add wasm32-unknown-unknown
rustup toolchain install 1.88 --profile minimal --target wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129 --locked
uvx --from 'reuse[charset-normalizer]' reuse --version
npm --prefix e2e ci
(cd e2e && npx --no-install playwright install chromium)
```

Rustの出力先は `CARGO_TARGET_DIR`、ブラウザの保存先は `PLAYWRIGHT_BROWSERS_PATH` で指定できます。試験の実行時も同じ設定を使ってください。

## 作業中は、変更した範囲から確かめる

| 変更 | まず流すもの |
|---|---|
| 文書だけ | `python3 tools/check-links.py` |
| Rustの計算 | `cargo test -p <変更したcrate> --locked`。台帳の符号化は `python3 tools/verify-ledger-golden.py` も |
| API・保存・取込 | 関連crateのRust試験と `e2e/run-real.sh flows <対象>.spec.mjs`、`e2e/run-real.sh contract` |
| 画面・キー操作 | `cargo test -p kamoku-ui --locked`、`node ui/dev/check-contrast.mjs`、`node ui/dev/check-policy.mjs`、対象のflows |
| Rust/WASM | [画面のビルド](rust-ui.md)、WASM clippy、対象flows。互換経路は[専用試験](rust-ui-preview.md)も |
| 実行スクリプト | `python3 -m unittest discover -s tools/tests -v` と `bash -n tools/check-all.sh e2e/run-real.sh` |
| RustやPlaywrightの依存 | 下のPR前の手順。古いブラウザを流用せず、上の準備も実行 |

不具合修正では、元の操作で失敗することを先に確かめ、同じ不具合を検出する試験を足します。帳簿を使う試験には合成データを使います。

## PRの前は、同じコミットで2つの入口を通す

```sh
tools/check-all.sh
e2e/run-real.sh
```

前者はWASM生成・Rustと画面の単体試験・静的検査・仕様・文書・依存・ライセンスを確認します。後者はWASMを生成してRustサーバーへ組み込み、スモーク、ブラウザ操作、契約試験、画面撮影を順に流します。各段階で新しい帳簿を使い、サーバーを止めてから次へ進みます。CIも同じ通し試験の入口を使います。

Nodeなど必須コマンドが無ければ開始前に失敗します。スクリプトは不足したRustツールやPlaywrightを自動導入しません。REUSEは準備済みの環境をオフラインで使い、未準備なら検査失敗にします。`--quick` はRust 1.88とcargo-denyを明示的に省く作業中の確認用で、完全な検証の代わりにはなりません。

## 一部だけ再実行する

```sh
tools/check-all.sh --quick
e2e/run-real.sh flows entry.spec.mjs
e2e/run-real.sh contract
e2e/run-real.sh screens
```

`flows` と `screens` の追加引数はPlaywrightへ渡します。`all` と `contract` は追加引数を受け付けません。文書用画像の更新だけなら `e2e/run-real.sh docs-images` を使います。

通常の通し試験は、Cargoが今回のビルドで返した実行ファイルを使います。配布物など特定のバイナリを検査するときだけ、`KAMOKU_BIN=/絶対パス/kamoku e2e/run-real.sh` で指定してください。この指定ではビルドを省くため、PRには対象バイナリの由来を残します。この指定ではディスクのUIへ差し替えず、埋め込まれた画面を検査します。

## 失敗・省略・未確認を分けて記録する

- 終了コード `0` は指定した範囲の成功、`1` は検査失敗、`2` は引数や準備の不足です。quickの成功には省略があります。
- 失敗したコマンド、対象のコミット、再現操作、期待と実際の結果をPRに残してください。修正したら影響する試験を再実行します。
- 仕様と試験の参照は `python3 tools/check-spec-trace.py --strict` で確認します。参照があるだけでは、試験内容が正しい証明にはなりません。
- 画面撮影の成功と、画像を見て操作を評価した結果は分けます。実機のIME・読み上げ・Safari・Firefox・Windowsなどは、未確認ならそのまま書きます。
- ログに合言葉、認証URL、実際の帳簿を貼らないでください。解決しない場合は、秘密情報を除いた最小の手順を[Issue](https://github.com/haranishi/kamoku/issues)へ記録してください。

スモークは「本物の実行ファイルを外から短く確かめる試験」、契約試験は「模擬と本物の応答形式を比べる試験」です。更新日: 2026-10-15。
