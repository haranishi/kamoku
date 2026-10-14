# 依頼R1: 計算の部品（台帳の付随情報、books、import、道具）

先に `00-common.md` を読んでください。あなたの担当は、入出力を持たない計算の部品です。保存、サーバー、画面は別の担当が作ります。

- 触る場所: `crates/kamoku-ledger/`、`crates/kamoku-jp-tax/`（T0のみ）、`crates/kamoku-books/`、`crates/kamoku-import/`、`tools/`、`docs/accounts.md`、`docs/legal/rights-check-phase1.md`（新規）、リポジトリ直下の `Cargo.toml`（members は変えない。`[workspace.dependencies]` は足してよい）、`deny.toml`（許可リストの追加が必要な場合だけ。理由を報告）。
- 進捗: `.agent-harness/runs/2026-10-14-phase1/r1-progress.md`。
- 読むもの（仕様）: `docs/spec/api-entries.md` `api-reports.md` `data-model.md` `stories-record.md` `stories-review.md`。

骨組みの `kamoku-books` と `kamoku-import` は作成済みです（依存なし）。

## T0 税の部品に確認日を足す

`kamoku-jp-tax` に、経過措置の表の確認日を返す `pub const TRANSITIONAL_TABLE_CHECKED_ON: &str = "2026-10-14";` を足します。画面の「使った表の最終確認日」に使います。ほかは変えません。

## T1 台帳に付随情報（meta）を足す

`kamoku-ledger` の `Entry` に付随情報を足します。設計の理由: 取引先などは、帳簿の意味に関わるので、ハッシュの対象にします。

- `Entry::with_meta(self, key: &str, value: &str) -> Result<Entry, EntryError>` と `Entry::meta(&self) -> &BTreeMap<String, String>`。同じ鍵を入れ直すと上書き。`Entry::new` の形は変えません。
- 検査: 鍵は半角の小文字・数字・`_` `-` `.` の1〜32バイト、値は1000バイト以下、組は16まで。違反は `EntryError` に新しい項目（`BadMetaKey` `MetaValueTooLong` `TooManyMeta`）を足して返す（`#[non_exhaustive]` なので互換性は保たれる）。
- 符号化（`encoding.rs`）: 仕訳の符号化の**末尾**に、付随情報が空でなければ `0x01` + `u32`ビッグエンディアン(組の数) + 鍵の昇順（バイト順）に `lp(鍵) + lp(値)` を足します。空なら何も足しません。これで、**付随情報の無い既存のイベントのハッシュは1ビットも変わりません**。既存の `tests/golden.rs` の固定値を一切変えずに通ることが、必須の確認です。ファイル先頭の符号化の仕様コメントも更新します。
- 新しい固定値: 付随情報つきのイベントを3種類（1組、3組で挿入順が逆、日本語の値）作り、ハッシュを **Rustの出力からではなく**、符号化の仕様だけから書いたPythonで別に計算して、Rustの試験に貼ります。Pythonは `tools/verify-ledger-golden.py` として残します。このスクリプトは、`crates/kamoku-ledger/tests/golden.rs` の `// GOLDEN-BEGIN` と `// GOLDEN-END` の間の固定値（既存の分も含む）を読み取り、自分で計算した値と比べて、食い違えば非0で終わります。
- 試験: 鍵の検査、挿入順に依存しないこと、付随情報を変えるとハッシュが変わること、訂正のイベントにも付くこと、`Ledger::from_events` が付随情報つきの連鎖を検証できること。proptest も足します。
- 満たす条件: AC-02.2（保存と表示の一致の土台）、`docs/spec/data-model.md` の「仕訳の付随情報」。

## T2 kamoku-books

依存: `kamoku-ledger`、`kamoku-jp-tax`、`serde`（derive。帳票の型をそのままJSONにするため）、`time`。試験専用: `proptest`、`serde_json`。

### 公開する型と関数（名前はこのとおりに。引数の細部は改善してかまいません）

- `Kind`（資産・負債・資本・収益・費用。JSONは `asset` `liability` `equity` `revenue` `expense`）と `Kind::normal_side()`。
- `WalletType`（`cash` `bank` `card` `ewallet`）、`Wallet { kind, institution }`、`Account`（`docs/spec/api-masters.md` の項目と同じ）。
- `TaxCode`（`S10 S8 P10 P8 P10X P8X NT`）。`parse` `as_str` `label`（`docs/spec/api-entries.md` の説明の日本語）、`class()`（売上・仕入・経過措置の仕入・非課税）、`rate()`。`tax_included(code, gross, rounding)` は、税込の金額に含まれる税額を `kamoku-jp-tax` の `split_tax_included` で出す（非課税は0）。
- `Chart`: 科目の集合。`Chart::new(Vec<Account>)` が検査する: コードの重複なし、親が存在して同じ`Kind`、循環なし、必須の科目（`cash` `dep_other` `accrued` `owner_draw` `owner_inv` `capital` `sales`）の存在。`get` `iter` `children` `name` `kind`。
- `standard_accounts() -> Vec<Account>`: 標準の科目（下の一覧）。
- 仕訳の組み立て: `SimpleInput`（`docs/spec/api-entries.md` の単純な形と同じ項目）、`compose_simple(&SimpleInput, &Chart, Rounding) -> Result<Composed, ComposeError>`、`compose_journal(...)`、`compose_opening(&[(AccountCode, Yen)], &Chart)`、`reconstruct_simple(&Entry, &Chart) -> Option<SimpleInput>`。`Composed { description, lines, meta, warnings }`。`ComposeError::code()` は `docs/spec/api-common.md` の失敗の`code`（`unbalanced` `unknown_account` `disabled_account` `amount_range` `bad_tax_code` `bad_request`）を返し、`message()` は原因と直し方の日本語。
- 帳票: `trial_balance` `income_statement`（総計と月別）`balance_sheet` `account_ledger` `tax_summary` `monthly_series` `wallet_balances`。入力は `Ledger::current()` が返す `&[&Entry]`（元帳だけは仕訳Noも要るので `(u64, &Entry)`）。出力の構造体は `serde::Serialize` で、`docs/spec/api-reports.md` のJSONの項目名と一致させ、一致を試験で確かめる（例のJSONを試験に貼って、`serde_json` で比べる）。
- 金額は `Yen` のまま、`i64` です。桁あふれは `Result` で返します。

### 組み立ての決まり（`docs/spec/api-entries.md` と `data-model.md` の「端数処理」）

- 支出: 借方に「事業分の科目」と（割合が100未満なら）「事業主貸」、貸方に口座。事業分 = 支払額×割合を円未満で四捨五入（整数）、残りが事業主貸。事業分の明細に税区分と税額（事業分から出す）を付ける。事業主貸の明細のメモは「家事按分 30%」の形。付随情報は `kind` `source`（呼び出し側が渡す）`partner` `business_pct`。
- 収入: 借方に口座、貸方に科目（税区分があれば税額）。割合は無視せず、100以外なら `bad_request`。
- 振替: 借方に移動先、貸方に移動元。同じ口座なら断る。
- 口座・科目は、存在し有効であること。支出や収入の科目が口座と同じなら断る。金額は1〜9,999,999,999,999。割合は0〜100。割合0なら、全額が事業主貸になる。
- `warnings`: 収入に費用の科目、支出に収益の科目、免税事業者の設定ではないのに税区分が空、など。一覧は少なくてよい（3つ以上）。
- 期首残高: 資産は借方、負債は貸方、差額を元入金で合わせる（債務超過なら元入金が借方）。付随情報 `kind=opening` `source=opening`。

### 帳票の決まり（`docs/spec/api-reports.md`）

- 試算表: 期間内の借方・貸方の合計と、通常の側から見た残高。
- 損益計算書: 収益の科目の合計と費用の科目の合計。月別の列は、期間が1つの暦年に収まるときだけ（12個。期間外は0）。
- 貸借対照表: 事業主貸と事業主借は、`as_of` のある暦年の1月1日からの動きだけ。前年までの当期利益と、前年までの事業主借−事業主貸は、元入金の行にまとめる（名前は「元入金（前年までの損益と事業主勘定を含む）」）。口座は親の科目の子として出す。残高が0の行は出さない。`balanced` を検算で返す。
- 総勘定元帳: 期首の残高、日付順の行（同じ日は仕訳No順）、相手科目（複数なら「諸口」）、残高の推移、期末の残高。
- 消費税: 税区分ごとに、課税標準（税込）と税額。経過措置の仕入は、日付ごとに `transitional_percent` で割合を引き、明細ごとに `transitional_credit`（切捨て）で控除額を出す。見込み額 = 売上税額 − 仕入税額（控除額を含む）。`disclaimer` に表の確認日（T0の定数）と「参考値」を入れる。
- 月次の系列: 売上・経費・利益の12か月分。事業主貸と事業主借は入れない。

### 標準の科目（コード 名前 分類。無効は「(無)」）

一覧は、国税庁の青色申告決算書（一般用）の項目名に合わせます。**項目名を、国税庁の様式（PDF）を取得して突き合わせ**、URLと確認日を `docs/legal/rights-check-phase1.md` に書いてください。取得できなければ「未確認」と明記します。

- 資産: `cash` 現金／`dep_current` 当座預金(無)／`dep_fixed` 定期預金(無)／`dep_other` 普通預金・その他の預金／`notes_recv` 受取手形(無)／`ar` 売掛金／`securities` 有価証券(無)／`inventory` 棚卸資産／`prepaid` 前払金／`loan_recv` 貸付金(無)／`bldg` 建物(無)／`bldg_fix` 建物附属設備(無)／`mach` 機械装置(無)／`vehicle` 車両運搬具(無)／`tools` 工具器具備品／`land` 土地(無)／`owner_draw` 事業主貸（システム）
- 負債: `notes_pay` 支払手形(無)／`ap` 買掛金／`loan_pay` 借入金／`accrued` 未払金／`adv_recv` 前受金／`deposit_recv` 預り金／`bad_debt_res` 貸倒引当金(無)／`owner_inv` 事業主借（システム）
- 資本: `capital` 元入金（システム）
- 収益: `sales` 売上高／`misc_income` 雑収入／`home_use` 家事消費(無)
- 費用: `exp_purchase` 仕入高／`exp_tax` 租税公課／`exp_freight` 荷造運賃／`exp_utility` 水道光熱費／`exp_travel` 旅費交通費／`exp_comm` 通信費／`exp_ads` 広告宣伝費／`exp_entertain` 接待交際費／`exp_insurance` 損害保険料／`exp_repair` 修繕費／`exp_supplies` 消耗品費／`exp_depr` 減価償却費／`exp_welfare` 福利厚生費／`exp_salary` 給料賃金／`exp_outsource` 外注費／`exp_interest` 利子割引料／`exp_rent` 地代家賃／`exp_baddebt` 貸倒金／`exp_misc` 雑費／`exp_meeting` 会議費／`exp_books` 新聞図書費／`exp_dues` 諸会費／`exp_vehicle` 車両費／`exp_fee` 支払手数料

各科目に、ひらがなの読み、`sort`（分類ごとに100・200・300・400・500から10刻み。口座は1000から）、`default_tax`（課税仕入が普通の費用は`P10`。損害保険料と利子割引料は`NT`。租税公課・減価償却費・給料賃金・福利厚生費・諸会費・地代家賃・新聞図書費・旅費交通費は空。売上高は`S10`。迷う科目は空にして、報告に書く）、`default_pct`（すべて100）、`form_line`（青色申告決算書の行名。固定の行が無い会議費・新聞図書費・諸会費・車両費・支払手数料は「（空欄）」）を付けます。口座の親の既定は、現金`cash`・預金`dep_other`・カード`accrued`・電子マネー`cash`。

`docs/accounts.md`（100行以内の表）を、`cargo run -p kamoku-books --example accounts_doc > docs/accounts.md` で生成できるようにし、ファイルの中身とコードがずれたら落ちる試験を置きます。

### 試験（門番として意味のあるもの）

- 手計算の固定値: 11,000円・割合70%・`P10`・切捨て、1円・割合50%の端数、割合0と100、振替、期首残高（債務超過を含む）。
- 性質（proptest）: 組み立てた明細は常に貸借が合い、借方合計が支払額に等しい。`reconstruct_simple(compose_simple(x)) == x`。試算表の借方合計=貸方合計。貸借対照表の等式が、複数年のランダムな仕訳でも成り立つ。月別の合計=総計。元帳の期末の残高=試算表の残高。
- 満たす条件: AC-02.2 AC-02.6 AC-03.1 AC-03.2 AC-03.3（`default_pct`の保持）AC-01.3 AC-07.3 AC-08.1〜AC-08.5。

## T3 kamoku-import

依存: `encoding_rs`、`serde`（derive）、`sha2`、`time`。試験専用: `proptest`、`serde_json`。ネットワークとファイルを使いません（バイト列を受けて返します）。

- `decode(&[u8]) -> Result<Decoded, ImportError>`: BOM（UTF-8、UTF-16LE/BE）。BOMが無ければ、UTF-8として正しければUTF-8、そうでなければShift_JIS（`encoding_rs` の `SHIFT_JIS` はWindows-31Jを含む）。判定結果の名前を返す。
- `parse_csv(&str) -> Result<Table, ImportError>`: 区切りは `,` タブ `;` から先頭の数行で判定。RFC 4180の引用符（引用符の中の改行・区切り・二重の引用符）、CRLF/LF/CR、空行を飛ばす、行末の空白。上限: 行20万、1欄64KB。壊れたCSV（閉じていない引用符）は、行番号つきで断る。自前の実装でよい（依存を足さない）。
- `Mapping`（`docs/spec/api-inbox.md` の形。`serde`）、`guess_mapping(&Table) -> Option<Mapping>`（見出しの語: 日付・年月日・ご利用日・取引日／摘要・内容・ご利用先・お取引内容／出金・お支払金額・引出／入金・お預り金額・預入／金額）、`apply_mapping(&Table, &Mapping) -> Vec<RowOutcome>`。
- 日付: `2026/10/1` `2026-10-01` `20261001` `2026年10月1日` `R8.10.1` `令和8年10月1日` `令和元年5月1日` `H31.4.30`。全角数字も可。実在しない日（2月30日）は読めない行にする。
- 金額: カンマ、`¥` `￥` `円`、全角数字、`△` `▲` `−`(U+2212) `-` の負、`(1,650)` と末尾の `-`。小数点が付く金額は読めない行にする。符号つきの1列の `inflow` 向きの対応。
- `fingerprint(wallet: &str, row: &ParsedRow) -> String`: 口座・日付・符号つき金額・正規化した摘要（前後の空白を除き、連続する空白を1つに、全角の英数記号を半角に）のSHA-256の16進。`number_occurrences(&[ParsedRow]) -> Vec<(String, u32)>`: 同じ指紋の行に、ファイル内の順で1, 2, ...を振る。
- `docs/spec/api-inbox.md` の `status` の判定そのもの（DBを見る部分）は、保存を担当する人の仕事。ここは純粋な部分だけ。
- 試験用のCSVは `crates/kamoku-import/tests/fixtures/synthetic/` に置きます（合成データ。`.gitignore` がここだけCSVを許しています）。UTF-8、BOMつきUTF-8、Shift_JIS、入出金が別の列、符号つき1列、カード明細ふう、引用符と改行を含む、壊れた引用符、の各1つ。Shift_JISのファイルは、`tools/make-fixtures.py`（Pythonの標準ライブラリだけ）で作り、スクリプトもコミットします。
- 性質の試験: 任意のバイト列で `decode` と `parse_csv` がpanicしない。書き出し（試験用の簡単なCSV書き手）→読み込みで同じ値に戻る。同じ入力から同じ指紋。順序を入れ替えても、番号付けの対応関係が保たれる。
- 満たす条件: AC-04.1 AC-04.2 AC-04.3（読み取りの部分）AC-04.4 AC-04.5（指紋と出現番号の部分）。

## T4 道具

- `tools/check-spec-trace.py`（標準ライブラリだけ）: `docs/spec/stories-*.md` の `- AC-xx.y [種類]` の行を集め、`crates/**/*.rs`、`ui/**/*.mjs`、`e2e/**/*.mjs` に `AC-xx.y` の文字列があるかを調べて、表（条件・種類・参照した場所の数）を出す。種類が `manual` のものは対象外。`--strict` を付けると、種類が `rust` `node` `e2e` で参照が無い条件があれば非0で終わる。引数なしは常に0で終わる。
- `tools/verify-ledger-golden.py`（T1）。
- `tools/make-fixtures.py`（T3）。
- 100行以内の `tools/README.md` に、3つの道具の使い方を書く。

## T5 権利の確認記録

`docs/legal/rights-check-phase1.md`（100行以内）を新規に作り、R1で足した依存（`encoding_rs` `serde` `serde_json` ほか）のライセンスと版、国税庁の様式の確認、読んだ公開資料のURLと確認日を書く。`encoding_rs` は `(Apache-2.0 OR MIT) AND BSD-3-Clause` なので、`deny.toml` に `BSD-3-Clause` がすでにあることを確認し、**バイナリで配るときは著作権表示の同梱が要る**ことを書く（配布時の作業として）。

## 終えたと言える条件

- `00-common.md` の品質の門がすべて通る。既存の試験（41件）は無変更で通る。
- `python3 tools/verify-ledger-golden.py` が0で終わる。`python3 tools/check-spec-trace.py` が動く。
- `kamoku-books` と `kamoku-import` に、手計算の固定値の試験と性質の試験がある。最終報告に、**わざと誤りを入れても試験が赤くなるか**を、重要な計算（按分・税・元帳の残高・貸借対照表の等式・指紋）で5か所以上確かめた結果を書く。
