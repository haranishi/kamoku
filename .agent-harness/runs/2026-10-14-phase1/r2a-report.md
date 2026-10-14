# R2a 最終報告

保存層を実装した。品質4門、既存R1の107試験と保存層の20試験、変異6か所が通過。依存追加と履歴操作はしていない。denyとRust 1.88は指定通り発注者担当。

## 1. 作ったもの

- `crates/kamoku-store/migrations/0001_init.sql`: 12表のSTRICTスキーマ、CHECK・FK・指紋の組の一意制約、追記専用トリガー8個。
- `src/lib.rs` / `types.rs` / `draft_types.rs` / `error.rs`: 接続、移行、起動報告、公開型、エラー、共通の原子的書き込み。
- `src/db_ledger.rs` / `db_event.rs` / `db_masters.rs` / `sql.rs`: 正規化された行とEntryの完全な往復、接頭復元、SQLite整数の境界検査。
- `src/posting.rs` / `masters.rs` / `partners.rs`: 台帳の登録・訂正・取消、設定・マスタと変更履歴。
- `src/inbox.rs` / `drafts.rs` / `confirmation.rs`: 重複と強制取込、編集、復元、行ごとの検査と全体での確定保存。
- `src/files.rs`: Unix権限と、VACUUM後に完成品を排他的に公開するバックアップ。
- `tests/{ledger,inbox,masters,files}.rs` / `common/mod.rs`: AC-04.4/04.5/04.7/05.4/05.5/06.3/09.1/09.3/09.4。Unicode・金額・税額・訂正取消の往復はproptest（32ケース）。
- `migrations/fixtures/0000.sql`、`examples/bench.rs`、`tests/mutations.py`: 旧版からの移行、10万イベント測定、再実行できる変異試験。
- 進捗、仕様疑問R2a節、権利記録の依存行を更新。

## 2. 実行したコマンドと結果

すべて指定のPATH・CARGO_HOME・CARGO_NET_OFFLINE・CARGO_TARGET_DIR・TMPDIRで実行。ネットワークは使っていない。

| コマンド | 結果 |
|---|---|
| `cargo check -p kamoku-store` | 通過 |
| `cargo fmt -p kamoku-store` | 保存層だけ整形 |
| `cargo fmt --all -- --check` | 通過 |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 通過 |
| `cargo test -p kamoku-store` | 通過（単独実行時18試験。最終20試験はworkspaceで確認） |
| `cargo test -p kamoku-store --test files` | 通過（0600の空ファイルを先に作る変更後、3試験） |
| `cargo test --workspace` | 通過（127試験。R1のソース・試験を変更せず107試験も通過） |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | 通過 |
| `python3 crates/kamoku-store/tests/mutations.py` | 6/6検出。下表参照。全ファイル復元済み |
| `cargo run --release -p kamoku-store --example bench` | 10万イベント作成1.212秒、open全体317.535ms、LoadReport 317.053ms、1件追記787.209µs |
| `cargo metadata --format-version 1 --offline` | 取得済み依存の版・ライセンスを確認 |
| `cargo deny` / `cargo +1.88` | 未実施（ユーザー指定）。独立した別エージェントのレビューも未実施 |

途中の失敗も記録する。初回checkはrusqliteのu64/usize変換とtimeの無効機能により失敗し、検査付きi64変換とSystemTimeに修正。台帳試験は改ざん用接続のFKと連番エラーの優先順で失敗し、FKを外す試験条件と復元前の連番検査に修正。clippyの文書・所有権・関数長・試験の記法、試験変数の改名時のコンパイル失敗も修正済み。最後の4門はすべて成功。

| 意図した誤り | 赤になった試験（すべてexit 101、コンパイル失敗でなくassertion失敗） |
|---|---|
| 台帳更新トリガーの対象をずらす | `append_only` |
| 指紋の重複判定から出現番号を除く | `duplicates_and_force` |
| 締め当日を許可する | `locked_period` |
| 失敗後のDB再読込を除く | `rollback_on_database_failure` |
| DB末尾との不一致を無視する | `stale_and_revision` |
| expected_revisionの照合を除く | `stale_and_revision` |

## 3. 仕様との差と理由

- 追加条件に合わせてpost系に取引先を渡す。`post_many`は`Vec<(Entry, Option<NewPartner>)>`で各件の取引先を同じトランザクションに含める。
- バックアップの最後はrenameでなくhard_link＋一時名削除。既存の宛先との競合でも上書きしないため。対応しないFSではErr。
- 下書き確定は保存済み行からR1で再計算し、渡されたEntryの日付・摘要・全明細・必須metaと照合する。古い入力は行ごとのbad_requestで断る。
- 設定の未設定開業日は空文字。setup_done=trueのときは実在する日を要求する。

## 4. 仕様の疑問

`spec-issues.md`末尾のR2a節に5件を追記した。初回現金口座、保護科目の409コード、古い下書きからの確定コード、既定の開業日、バックアップの公開手段。保護科目は全変更を断る厳しい側で実装した。

## 5. R2bへの公開API引継ぎ

以下はStoreのメソッド。表の`R<T>`は`Result<T, StoreError>`。書き込みのnowはi64のUNIX秒、IDは下書き・取引先・テンプレートがi64、仕訳がEntryId。Settingsの日付は検査付き文字列、Entryの日付はtime::Date。公開型の全フィールドには日本語docがある。

| 名前と引数 | 戻り値・使い方 |
|---|---|
| `open(path: impl AsRef<Path>, OpenOptions)` / `open_in_memory()` | `R<(Store, LoadReport)>`。create_if_missingの既定はfalse。破損でも成功し接頭部分を公開 |
| `ledger()` / `chart()` / `settings()` / `read_only()` | `&Ledger` / `&Chart` / `&Settings` / `Option<&VerifyFailure>` |
| `entry_no(&EntryId)` / `set_actor(&str)` | `Option<u64>` / `R<()>`。操作者の既定はlocal |
| `partners()` / `templates()` / `change_log(limit: u32)` | `R<Vec<Partner>>` / `R<Vec<Template>>` / `R<Vec<ChangeLog>>` |
| `save_settings(Settings, now)` | `R<()>`。全置換 |
| `add_account(NewAccount, now)` / `update_account(code: &str, AccountPatch, now)` | `R<Account>`。口座w01〜、科目x01〜、分類・親・型を変更する欄なし |
| `add_partner(NewPartner, now)` / `update_partner(id, PartnerPatch, now)` | `R<Partner>`。名前の重複はDuplicateName |
| `add_template(name: &str, body: Value, now)` / `delete_template(id, now)` | `R<Template>` / `R<()>`。bodyは日付なしSimpleInput、kindはexpense/income/transfer |
| `import_profile(wallet: &str)` / `save_import_profile(wallet: &str, Mapping, now)` | `R<Option<Mapping>>` / `R<()>` |
| `post(Entry, now, Option<NewPartner>)` | `R<PostedRef>`。自動追加する取引先を最後に渡す |
| `revise(Entry, expected_revision: u32, reason: &str, now, Option<NewPartner>)` / `void(&EntryId, expected_revision: u32, reason: &str, now)` | `R<PostedRef>`。締めと最新の版を確認 |
| `post_many(Vec<(Entry, Option<NewPartner>)>, now)` | `R<Vec<PostedRef>>`。全件成功か全部巻き戻し |
| `import_commit(NewBatch, Vec<NewDraft>, force: HashSet<usize>)` | `R<ImportSummary>`。forceは元ファイルの1始まりの行番号 |
| `known_fingerprints(&[(String,u32)])` / `import_batches(limit: u32)` | `R<KnownFingerprints>`（existing/maxima） / `R<Vec<ImportBatch>>`（summary/filename/wallet/recorded_at） |
| `drafts(DraftFilter)` / `draft_counts()` | `R<Page<Draft>>` / `R<DraftCounts>`。一覧は既定draft/50件、上限500件 |
| `update_draft(id, DraftPatch, now)` / `set_draft_status(id, DraftAction, now)` | `R<Draft>` / `R<()>`。Ignore/Restore。確定済み復元は取消後だけ |
| `confirm_drafts(Vec<(DraftId,Entry)>, now)` | `R<Vec<ConfirmResult>>`。各行のresultと関数全体のErrを区別 |
| `verify()` / `noted_hash_lookup(hash: &str)` / `backup_to(path)` | `Result<VerifyOk,VerifyFailure>` / `Option<NotedHash>` / `R<()>` |

例: `let (mut store, report) = Store::open(path, OpenOptions { create_if_missing: true })?; store.set_actor("デモ利用者")?; let saved = store.post(composed.to_entry(id)?, now, Some(new_partner))?;`。一括登録は`store.post_many(vec![(entry, None)], now)?`。確定用Entryは下書きの全フィールドからSimpleInputを作り、現在の設定とSource::Importでcompose_simple→to_entryする。draft/sourceは保存層が付ける。invoice_noは任意で追加できる。

エラー: ReadOnly/Stale/StaleRevision/AlreadyConfirmed/DuplicateName/ProtectedAccountは409、NotFoundは404、LockedPeriod/MissingAccount/UnknownAccount/DisabledAccount/AmountRange/BadInvoiceNumber/BadTaxCodeは422として扱える。Invalidはbad_request（形と業務規則で400/422をR2bが選ぶ）。ComposeはR1のcode/status/fieldを使う。LedgerのAlreadyVoided/EmptyReasonはalready_voided/reason_required。Sql/Io/Json/FutureSchemaなど内部失敗の詳細は画面に出さない。BackupExistsは既存宛先との衝突。

罠: Patchの`Option<Option<T>>`は変更なし/消す/設定するを分ける。JSONから直接deriveしないのでR2bで欄の存在を調べて組み立てる。Staleなら入力を再確認して再試行し、無条件に再送しない。読み取り専用でバックアップは作れる。DB用の専用フォルダを渡す（Unixでその親を0700にする）。末尾削除は連鎖だけでは検知できず、控えたハッシュで照合する。初回現金口座の追加、wallet_name付加、全件JSON/CSV、集計の桁の制限はR2b/帳票側に引き継ぐ。
