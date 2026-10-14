# 依頼R2a: 保存の層（kamoku-store）

先に `00-common.md` を読んでください。あなたの担当は、台帳・下書き・マスタをSQLiteに保存する `kamoku-store` です。サーバーは次の担当（R2b）が、あなたの公開APIの上に作ります。

- 触る場所: `crates/kamoku-store/`、`docs/legal/rights-check-phase1.md`（依存の行の追記だけ）、`tools/`（新しい道具が要るときだけ）、リポジトリ直下の `Cargo.toml` の `[workspace.dependencies]`、`deny.toml`（許可リストの追加が要るときだけ。理由を報告）。
- 進捗: `.agent-harness/runs/2026-10-14-phase1/r2a-progress.md`。
- 読むもの（必須）: `docs/spec/data-model.md`（設計の核）、`api-common.md` `api-masters.md` `api-entries.md` `api-inbox.md`、`stories-*.md`。前の担当（R1）が作った `crates/kamoku-ledger` `crates/kamoku-books` `crates/kamoku-import` のソースと、`.agent-harness/runs/2026-10-14-phase1/r1-progress.md`、`spec-issues.md`。
- 依存: `rusqlite`（`bundled` 機能。SQLite同梱）、`serde` `serde_json`、`time`、`kamoku-ledger` `kamoku-books` `kamoku-import`。試験専用: `tempfile` `proptest`。ほかは足さない。

## 作るもの

`docs/spec/data-model.md` の「3つの層」と「守るべき決まり」を、すべて実装します。列の定義は `crates/kamoku-store/migrations/0001_init.sql`（`include_str!`で取り込む）に置きます。表は `data-model.md` の一覧のとおりで、`STRICT` 表と `CHECK` 制約を使います。

### 公開する型と関数（名前はこのとおりに。細部は改善してかまいません）

- `Store::open(path, OpenOptions) -> Result<(Store, LoadReport), StoreError>` と `Store::open_in_memory()`（試験用）。`OpenOptions` は `create_if_missing`。開くときに: フォルダ0700・ファイル0600（Unix系）、PRAGMA（`data-model.md` の10）、移行（`PRAGMA user_version`）、初回は標準の科目と既定の設定を入れる、台帳を全件読み込んで `Ledger::from_events` で検証する。検証に失敗したら**開くのは成功させ**、`Store` を読み取り専用（`read_only() -> Option<&VerifyFailure>`）にして、書き込みは `StoreError::ReadOnly` で断る。`LoadReport` は、イベント数・仕訳数・所要時間・検証結果。
- 読み取り: `ledger() -> &Ledger`、`chart() -> &Chart`、`settings() -> &Settings`、`entry_no(&EntryId) -> Option<u64>`（登録イベントの出現順。読み込み時に数え、追記のたびに更新）、`partners()`、`templates()`、`import_profile(wallet)`、`change_log(limit)`。
- 設定とマスタ: `save_settings`（検査: 課税区分と登録番号の整合、`T`+13桁、端数処理の値）、`add_account` `update_account`（`docs/spec/api-masters.md` の規則。コードの採番は口座`w01`〜、科目`x01`〜。分類と親は変えない。必須の科目は無効にできない）、`add_partner` `update_partner`、`add_template` `delete_template`、`save_import_profile`。**変更はすべて `change_log` に追記**する（誰が・いつ・何を）。
- 台帳の書き込み: `post(entry, now) -> Result<PostedRef, StoreError>`、`revise(entry, reason, now)`、`void(id, reason, now)`、`post_many(Vec<Entry>, now)`（1つのトランザクション。途中で失敗したら全部戻す）。いずれも、(1) 締めた日（`settings.locked_until`）以前への書き込みを `StoreError::LockedPeriod` で断る（訂正は、元の日付と新しい日付の両方を見る）、(2) 科目が存在し有効であることを `Chart` で確かめる、(3) メモリ上の `Ledger` に追記、(4) 同じイベントを1つの `BEGIN IMMEDIATE` トランザクションで書く、の順。書く前に、DBの最大の連番と末尾のハッシュが、メモリの台帳と一致するかを確かめ、食い違えば（別のプロセスが書いた）、台帳を読み直したうえで `StoreError::Stale` を返す。書き込みが失敗したら、DBから読み直してメモリを戻す。
- 取込: `import_commit(NewBatch, Vec<NewDraft>) -> ImportSummary`（指紋と出現番号の一意制約に当たる行は重複として数え、入れない。行の追加と記録は1つのトランザクション）、`known_fingerprints(&[(String, u32)]) -> HashSet<(String, u32)>`（プレビューの「重複」判定に使う）、`drafts(filter) -> Page<Draft>`（状態・口座・文字列・期間・件数）、`draft_counts()`、`update_draft`、`set_draft_status`（`ignore` `restore`。確定済みは、結びついた仕訳が取消済みのときだけ未確認に戻せる）、`confirm_drafts(Vec<(DraftId, Entry)>, now) -> Vec<ConfirmResult>`（`post_many` と下書きの状態更新を**同じトランザクション**で。仕訳の付随情報には `draft=<id>` を入れる）。
- 保守: `verify() -> Result<VerifyOk, VerifyFailure>`（メモリの台帳の再検証に加え、DBの行数と連番の連続、`PRAGMA quick_check`）、`noted_hash_lookup(hash) -> Option<NotedHash{seq, is_head}>`、`backup_to(path)`（`VACUUM INTO`。既存のファイルがあれば断る。出来たファイルを0600に）。

### 台帳の追記だけを守る仕掛け

- `ledger_events` `ledger_lines` `ledger_meta` に、`BEFORE UPDATE` と `BEFORE DELETE` のトリガー（`RAISE(ABORT, ...)`）。`change_log` も同じ。
- 台帳から読み戻した `Entry`（`Entry::new` + `with_meta`）は、保存前と**完全に同じ**であること（文字列は1バイトも変えない。日本語、絵文字、改行、前後の空白、`'` `"` `\`、NULを除く）。proptestで確かめる。

## 試験（門番として意味のあるもの）

- 往復: 任意の仕訳列（訂正と取消を含む）を書き込んで開き直すと、`Ledger` が同じで `head_hash` が一致する。
- 追記専用: 別の接続で `UPDATE` / `DELETE` を試みるとトリガーで失敗する（AC-09.1の土台）。
- 改ざんの検知: 「トリガーを外した別の接続で、金額を1円変える」「連番を飛ばす」「prev_hashを変える」「訂正の理由を消す」のそれぞれで、`open` が読み取り専用になり、最初に壊れた連番と規則を返す。末尾のイベントの削除は検知できないこと（限界）を、`noted_hash_lookup` が控えと違うことで見つける試験として残す。
- 失敗時の巻き戻し: 書き込みに失敗する状況（読み取り専用のDB、ディスクが書けない代わりに、トリガーで失敗させる）で、メモリとDBが一致したままであること。
- 取込: 同じ入力を2回、期間の重なる別の入力、同じ日・同じ金額・同じ摘要が本当に2件ある入力（AC-04.4 AC-04.5 AC-04.7）。確定で下書きが `confirmed` になり `entry_id` を持つこと（AC-05.5）。科目が空の行は確定せず、ほかの行は確定すること（AC-05.4）。
- 締め: 締めた日以前の登録・訂正・取消の拒否（AC-06.3）。
- 移行: 版0（空のファイル）から最新へ。将来の版のために、`migrations/` にテスト用の古い版のファイルを置く仕組み。
- 権限: Unix系でフォルダ0700・ファイル0600（AC-09.4）。バックアップ後の複製が開けて同じ`head_hash`。
- 規模（AC-09の速さの目標の下調べ）: 10万イベントを書いて、開く時間と1件追記の時間を測り、結果を標準出力に出す（`#[ignore]` の試験か `examples/bench.rs`。通常の試験は10秒以内に終わる大きさにする）。

## 終えたと言える条件

- `00-common.md` の品質の門がすべて通る。R1の既存の試験は無変更で通る。
- 満たす条件の番号（AC-04.4 AC-04.5 AC-04.7 AC-05.4 AC-05.5 AC-06.3 AC-09.1 AC-09.3 AC-09.4）が、試験の名前かコメントにある。
- 最終報告に、**わざと誤りを入れても試験が赤くなるか**を5か所以上確かめた結果（例: トリガーを外す、指紋の出現番号を外す、締めの判定を外す、巻き戻しを外す、Stale検知を外す）を書く。
- 次の担当（R2b）が使う公開APIの一覧（名前・引数・使い方の例・エラーの種類）を書く。

## 設計レビューで追加された条件（2026-10-14。上の本文より優先）

別のモデルによる設計書のレビューで、次が加わりました。`docs/spec/data-model.md`（決まり3・4・13〜16）と `api-inbox.md` を読み直してください。

- **トランザクションの範囲**: `post` `revise` `void` `post_many` `confirm_drafts` は、仕訳のイベント・明細・付随情報に加えて、**取引先の自動追加、下書きの状態の更新、変更履歴を、同じトランザクション**に入れる（`post` 系に `ensure_partner: Option<NewPartner>` を渡せる形にする）。途中で失敗したら全部戻し、メモリの台帳も戻す。
- **古い版の訂正・取消**: `revise` と `void` は `expected_revision` を受け取り、最新の版と違えば `StoreError::StaleRevision`。
- **壊れていたとき（決まり14）**: 検証に失敗したら、壊れた連番の手前までの接頭部分で台帳を作る（空でもよい）。DBの行から `Entry` を作り直せない行も、その連番で壊れているとみなす。`LoadReport` に `events_in_db` と `events_loaded` を分けて入れる。読み取り専用の間は、壊れた先を使って書かない。
- **重複の強制取り込み**: `import_commit` は `force: HashSet<行番号>` を受け取る。重複と判定した行でも、強制の指定があれば、その指紋の最大の出現番号+1を付けて入れる。`known_fingerprints` は、(指紋, 出現番号) の存在だけでなく、その指紋の最大の番号も返せるようにする。
- **取引番号の列（`id_col`）**: 下書きに `external_id: Option<String>` の列を足す。指紋は取り込みの部品（`kamoku-import`）が作るので、ここでは保存するだけ。
- **下書きの日付**: `update_draft` で `date` も直せる。指紋は変えない（取り込んだときの値のまま）。
- **`restore`**: `ignored` → `draft` はそのまま。`confirmed` → `draft` は、結びついた仕訳が取消済みのときだけ。`entry_id` を空にし、以前の仕訳IDを `change_log` に残す。それ以外の `confirmed` の編集・`ignore` は `StoreError::AlreadyConfirmed`。
- **権限とバックアップ（決まり15）**: `-wal` `-shm` の権限もDB本体と同じ0600になっていることを試験で確かめる。`backup_to` は、同じフォルダの一時名に最後まで書いてから、本来の名前へ移す（途中で止まった不完全な複製を残さない）。
- **集計の桁（決まり16）**: 集計値が9,007,199,254,740,991を超えるときの扱い（`amount_range`）は、帳票側の仕事。保存の層は、1明細の上限だけ検査する。
