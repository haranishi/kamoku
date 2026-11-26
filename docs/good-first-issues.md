# 最初の貢献の候補

初めての人向けに、小さく始められる作業を並べました。どれも、期待する結果と、対象のファイル、完了の確かめ方が決まっています。やりたいものがあれば、先にIssueで「これをやります」と書いてください（重複を避けるためです）。進め方は[参加の手引き](../CONTRIBUTING.md)、手元での動かし方は[開発の手引き](development.md)にあります。

返信の目安は1週間です。この一覧の作業は、受け入れる前提で用意しています。

## コードを書かない作業

### 1. 勘定科目の「読み」を点検する（難しさ: 易）

画面の科目の検索は、ひらがなの読みでも絞れます。標準の科目の読みに、誤りや不自然な読みがないかを見てください。
- 対象: `crates/kamoku-books/src/standard.rs`。一覧は[docs/accounts.md](accounts.md)。
- 完了: 直す読みと理由をIssueに書く。直す場合は、`cargo run -p kamoku-books --example accounts_doc > docs/accounts.md` で一覧も作り直し、`cargo test -p kamoku-books` を通す。

### 2. 税区分の説明を、国税庁の資料と見比べる（易）

税区分の名前（「課税仕入10%」など）の言い方が、国税庁の資料の言い方と合っているかを見てください。
- 対象: `crates/kamoku-books/src/tax.rs` の `label`。
- 完了: 違う箇所を、国税庁の資料のURLつきでIssueに書く。URLの無い指摘は、確かめられるまで保留します。

### 3. 初めて使う人として、手順書を試す（易）

[使い方](getting-started.md)の手順だけで、15分以内に画面を開けるかを試してください。
- 完了: つまずいた所（環境、コマンド、言葉）をIssueに書く。OSとRustの版も添える。Windows・Linuxでの結果は特に助かります。

## 小さなコードの作業

### 4. 明細CSVの見出しの語を足す（中）

取込は、見出しの語から列の対応を推測します。使っている金融機関やカードの見出しの語が足りなければ、足してください。
- 対象: `crates/kamoku-import/src/mapping.rs` の `DATE_WORDS` `DESC_WORDS` `OUT_WORDS` `IN_WORDS`。
- 完了: 語を足し、その見出しを持つ**合成の**CSV（実在の店・口座・金額は使わない）を `tools/make-fixtures.py` に足して、`crates/kamoku-import/tests/` に試験を足す。`cargo test -p kamoku-import` と `python3 tools/make-fixtures.py --check` を通す。

### 5. 人が確かめる項目を、手元で確かめる（易）

自動の試験では確かめきれない項目があります。SafariとFirefoxでの日本語入力（[キー操作](spec/keyboard.md)）、初めての人が15分以内に起動できるか（AC-12.3）、ExcelでCSVが文字化けしないか（AC-13.2）などです。条件の一覧は、`docs/spec/stories-*.md` の `[manual]` の行にあります。
- 完了: OS・ブラウザ・アプリの版と、確かめた結果（できた／できない）をIssueに書く。帳簿は合成のデータを使う。

### 6. ブラウザの試験に、FirefoxかWebKitを足す（中）

画面の通し試験はChromiumだけで動かしています。日本語入力の変換中のEnterは、ブラウザごとに挙動が違います。
- 対象: `e2e/playwright.config.mjs`。
- 完了: FirefoxかWebKitの設定を足し、動く試験と動かない試験を分けて書く。動かない理由（変換の模擬ができない、など）も書く。

### 7. 画面の文言を、やさしい日本語に直す（中）

専門用語が多い、長い、原因と直し方が書かれていないエラー文を見つけたら、直してください。
- 対象: `crates/kamoku-ui/src/` の下の文言。エラーは「原因と直し方」の形にします。
- 完了: `cargo test -p kamoku-ui` と `e2e/run-real.sh` が通る（準備は[試験の実行手順](testing-procedure.md)）。直した前後の文言をPRに書く。

### 8. 色の組み合わせの検査を足す（易）

文字と背景の組み合わせは、ライトとダークの両方で、4.5:1以上を検査しています。画面に新しい色の組み合わせがあれば、検査に足してください。
- 対象: `ui/dev/check-contrast.mjs`。
- 完了: `node ui/dev/check-contrast.mjs` が通る。足した組み合わせがわざと基準を下回ると、落ちることも確かめる。

## 作業を提案したいとき

新しい機能、画面、依存、保存形式の変更は、実装の前にIssueで相談してください（[参加の手引き](../CONTRIBUTING.md)）。採用しない場合は、理由と、再検討に必要な情報を返します。
