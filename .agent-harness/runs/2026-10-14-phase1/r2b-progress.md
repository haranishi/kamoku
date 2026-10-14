# R2b の進み具合（サーバーとコマンド）

## いまの区切り（22:45 時点）: コードレビューの指摘 A1〜A3・B1〜B9 は、すべて直した

設計担当の依頼（22:00ごろ）。先に再現する試験を書いて赤を確かめてから直した（B1 と B2 の保存層の
関数は、新しく足す関数なので、赤は変異で確かめた。B2 の画面側は、ロックを古い形に戻して赤を見てから
戻し、cmp で一致を確かめた）。

| 指摘 | 直したところ | 試験（直す前の赤） |
|---|---|---|
| A1 記録が本文を丸ごと持つ | `src/api/client_ids.rs`（指紋と応答だけ、64バイト、8MB） | `src/api/entries_tests.rs`（本文を持つと上限超え） |
| A2 追い出し → 再送が二重登録 | 同上（いっぱいなら保存の前に503、再送は常に200） | 同上（上限＋1件の後の再送が201） |
| A3 確定の ids 15万件 | `src/api/drafts.rs`（2,000件まで、HashSet と HashMap） | `tests/bounds.rs`（応答なしのまま122秒） |
| B1 古いマスタで組み立て | 保存層 `refresh_masters`、`src/app.rs` | `tests/server.rs`（80%に変えても30%） |
| B2 panic の後のロック | 保存層 `recover`、`src/app.rs` | `src/app/tests.rs`、保存層 `recovery.rs` |
| B3 demo の一時フォルダ | `src/paths.rs`・`src/cli.rs` | `tests/cli.rs`（既存のフォルダを消した） |
| B4 `--ui-dir` の中のリンク | `src/statics.rs` | `statics.rs` の単体試験 |
| B5 出金の合計の桁 | `src/api/dashboard.rs` | `tests/bounds.rs`（2^53 を200で返した） |
| B6 列の対応の保存の失敗 | `src/api/import_rows.rs` | `tests/failures.rs`（失敗でも下書きが残った） |
| B7 取込履歴の offset と total | `src/api/imports.rs` | `tests/bounds.rs`（total が1） |
| B8 待ち時間が期限に入らない | `src/http/server.rs`・`conn.rs` | `tests/http_queue.rs`（1.6秒） |
| B9 HEAD への断りに本文 | 同上 | `tests/http_queue.rs` |

- 品質の門（22:39〜22:40）: fmt・clippy・test（ワークスペース263件、失敗0）・doc・`cargo +1.88 check`・`cargo deny check` がすべて終了コード0。
- 変異: 画面側は各修正を外した誤り18件を足して全部検出。保存層の2関数は `tests/mutations_store.json` の5件を全部検出。
- 依存: kamoku-app に `sha2 0.10`（本体）と `rusqlite 0.40.2`（試験だけ）。新しいクレートは増えない。
- 保存層は `src/recovery.rs` と `tests/refresh.rs` を足し、`lib.rs` に1行足しただけ。既存の関数は変えていない。

- 最後の変異の一覧（35件、22:41〜22:50）を通しで動かし、35件すべて検出、生き残り0。本物のファイルの指紋は前後で同じ。

## 次にやること

- なし（最終報告を出した）。

## 詰まり

- なし。
