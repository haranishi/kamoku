# 共通の約束（全担当）

あなたは実装担当です。設計は確定済みです。会話の文脈は見えない前提で書いてあります。仕様が曖昧・矛盾するときは、勝手に決めずに `.agent-harness/runs/2026-10-14-phase1/spec-issues.md` へ追記し、安全側（厳しい側）で実装して、最終報告に書いてください。

## 場所

- リポジトリは `<リポジトリの場所>`（ブランチ `feat/readme-and-first-app`）。触ってよい場所は、各依頼書の「触る場所」だけです。
- `docs/spec/` `docs/architecture.md` `docs/roadmap.md` `docs/ux-five-layers.md` `README.md` は設計担当のものなので、触らないでください。直したい点は `spec-issues.md` に書きます。
- まず読むもの: `docs/spec/` の全ファイル、`docs/architecture.md`、`docs/legal/license-policy.md`、`CONTRIBUTING.md`、既存の `crates/kamoku-ledger` と `crates/kamoku-jp-tax` のソース。

## してはいけないこと

- `git commit` `git push` など履歴を変える操作。`git status` `git diff` `git log` は使えます。
- 他の会計OSS（GnuCash、hledger、beancount、rustledger、Firefly III、Bigcapital、Frappe Books、Akaunting など。GPL/AGPLを含む）のコード・試験・画面を見て写すこと。freee、マネーフォワード、弥生の画面・文言・アイコン・配色を真似ること。この依頼書と `docs/spec/` だけから書きます（クリーンルーム）。
- 実在の帳簿・領収書・口座・人名・会社名を使うこと。サンプルは、架空の名前（「サンプル」「デモ」を含む）の合成データだけです。
- 本番コードに、ネットへ送信する処理（HTTPクライアント、計測、外部フォントやCDN）を入れること。
- 本番コードで `unwrap` `expect` `panic!` `unreachable!` `todo!`、panicしうる添字、浮動小数点での金額計算、`unsafe` を使うこと。試験では `unwrap` を使ってかまいません。
- 依頼書にない依存を足すこと。必要なら手を止めて、理由を最終報告に書きます。
- 文書や画面の文言に、AIらしい決まり文句（「〜と言えるでしょう」「重要です」の連発、「シームレス」「強力な」など）を使うこと。

## 環境

```
export PATH="$HOME/.cargo/bin:$PATH"
cd <リポジトリの場所>
```

Rust 1.99 stable、edition 2024、最小の対応は1.88。cargoの依存の取得にはネットワークを使えます。`resolver = "3"` なので、MSRVを見て依存の版が選ばれます。依存を足したら `cargo deny check` を通します（許可リストは `deny.toml`。足りないライセンスが必要なら、止めて報告）。

## 品質の門（区切りごとに通す。失敗はそのまま報告に書く）

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo +1.88 check --workspace      # toolchainが無ければ rustup toolchain install 1.88 --profile minimal
cargo deny check
```

## コードの書き方

- 周囲のコードに合わせます。各 `.rs` の先頭にSPDXの2行（既存ファイルと同じ）。`pub` な項目には、日本語のdocコメントを必ず付けます（`missing_docs` が警告で、`-D warnings` で落ちます）。
- コメントは「なぜ」を書きます。「何をしているか」は書きません。
- 試験の名前かコメントに、満たす条件の番号（`AC-02.1` など。`docs/spec/stories-*.md`）を書きます。
- 1つのファイルは300行を目安に分けます。
- 文書（Markdown）は1ファイル100行以内で、やさしい日本語。結論を先に書き、同じ型の段落を3回続けません。

## 進め方

- 小さく作り、毎回、品質の門を通します。試験を先に書ける所は先に書きます（金額・税・集計・符号化・CSV）。
- 区切りごとに、自分の `progress.md`（各依頼書に場所を書いてあります）を、「できたこと／次にやること／詰まり」の3項目で上書きします。途中で止められても、続きから再開できるようにするためです。
- 試験は「門番」として意味があるか確かめます。わざと誤りを入れても赤くならない試験は、役に立ちません。重要な計算は、手計算の固定値の試験と、性質の試験（proptest）を併用します。

## 最終報告の形

1. 作ったもの（ファイルと役割）
2. 実行したコマンドと結果（失敗もそのまま）
3. 仕様からずれた点と、理由
4. 仕様の疑問（`spec-issues.md` にも書く）
5. 次の担当への引き継ぎ（公開APIの要点、罠）
