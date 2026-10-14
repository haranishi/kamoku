# Phase 1 の実行記録（2026-10-14）

最初のアプリ（`kamoku-books` `kamoku-import` `kamoku-store` `kamoku-app` と、`ui/`）を、設計担当（1人）と実装担当（複数）で作ったときの記録です。AIの担当に、依頼書を渡して実装させ、別の担当に読み取りでレビューさせました。**コミットや公開は、設計担当が人の指示を受けて行います**。

## 担当の名前

| 名前 | 担当 | 依頼書 |
|---|---|---|
| R1 | 計算の部品（`kamoku-books`、`kamoku-import`） | `briefs/r1-core-libs.md` |
| R2a | 保存層（`kamoku-store`） | `briefs/r2a-store.md` |
| R2b | サーバーとコマンド（`kamoku-app`） | `briefs/r2b-app.md` |
| U1 | 画面の土台（`ui/` の部品、ホーム、取引を登録、仕訳帳） | `briefs/u1-ui-foundation.md` |
| U2 | 残りの画面（未確認、取込、帳票、登録簿、設定、初期設定） | `briefs/u2-ui-screens.md` |
| F1・F2 | 画面の直し 第1回（F1=共通の土台と帳票、F2=ほかの画面） | `briefs/ui-fix-*.md` |

## 記録の読み方

- `briefs/`: 担当に渡した依頼書（共通の約束は `00-common.md`、画面の採点役への依頼は `ui-scoring.md`）。
- `*-progress.md`: 担当が区切りごとに書いた「できたこと／次にやること／詰まり」。
- `spec-review.md`: 設計書への読み取りレビュー30件と、1件ずつの判断。
- `rust-review.md`、`app-review.md`: Rustの部品とサーバーへの読み取りレビューと、対応。コードや試験のコメントにある「S1」「B6」などは、この表の指摘の番号です。
- `spec-issues.md`: 実装担当が見つけた、仕様の曖昧さや矛盾と、その場の判断。
- `scoring-round1-*.md`: 画面の採点役（別の会話）の報告の全文。幅390・768・1440。

担当の報告は、モデルの出力です。完了の判断は、報告ではなく、試験の結果と実際のファイルで行っています。
