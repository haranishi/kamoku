# 通信の約束（4）ホーム・帳票・書き出し

共通のきまりは[こちら](api-common.md)です。集計に使うのは、取消を除いた最新の版の仕訳だけです。

## ホーム用のまとめ

`GET /reports/dashboard?month=2026-10`。画面の最初の描画に要る数字を、1回で返します（通信を重ねないため）。

```json
{ "month": "2026-10", "fiscal_year": 2026,
  "next_actions": { "inbox_draft": 12, "inbox_out_amount": 45600 },
  "month_summary": { "revenue": 250800, "expense": 96400, "profit": 154400,
                     "prev": { "revenue": 240000, "expense": 101000, "profit": 139000 } },
  "ytd": { "revenue": 2100000, "expense": 880000, "profit": 1220000 },
  "wallets": { "items": [{ "code": "w01", "name": "現金", "type": "cash", "balance": 52000 }],
               "cash_total": 1250000, "owed_total": 84000 },
  "series": [{ "month": "2026-01", "revenue": 100000, "expense": 40000, "profit": 60000 }],
  "top_expenses": [{ "account": "exp_comm", "name": "通信費", "amount": 31000 }],
  "tax": { "applicable": false },
  "ledger": { "events": 128, "head_hash": "...", "verified": true } }
```

`series` は、その年の1月から12月まで（月がなければ0）。`tax` は課税事業者のとき `{ "applicable": true, "sales_tax", "purchase_tax", "estimate", "as_of" }`。

## 年またぎの決まり（仮想の繰越）

会計年度は暦年（1月1日〜12月31日）です。台帳に締めの仕訳はまだ作らず、すべての帳票が、次の「仮想の繰越」を計算の中でだけ行います。

- 各年の初めに、収益と費用の科目は0から始める。
- 前の年までの当期利益（収益−費用の累計）と、前の年までの「事業主借−事業主貸」の累計は、元入金に足す。事業主貸と事業主借も、その年の動きだけから始める（これらの子の科目も同じ）。
- 期間を取る帳票（試算表・損益計算書・総勘定元帳・消費税）は、1つの暦年に収まる期間だけ受け付ける。またぐ場合は `422 period_spans_years`。
- 期首残高の仕訳（`meta.kind` が `opening`）は、日付が期間の初日と同じなら、動きではなく期首として数える（試算表と総勘定元帳。開業の年の「期首」の列に期首残高が出るように）。期間の途中の日付なら、その日の動きとして扱う。

## 帳票

期間は `from` と `to`（両端を含む）で渡す。`year=2026` は、1月1日〜12月31日の略記。

- `GET /reports/trial-balance?from&to` → `{ rows: [{ code, name, kind, opening, debit, credit, closing }], total_debit, total_credit, balanced }`。`debit` と `credit` は期間内の動き。`opening` と `closing` は、通常の側から見た残高（資産・費用は借方−貸方、負債・資本・収益は貸方−借方）で、`closing = opening + 通常の側の動き`。行は科目ごとに平らで、親に直接記帳した分は親の行に出る（親子を足して二重に数えない）。`opening` は仮想の繰越を含む。`balanced` は、動きの合計の一致と、`closing` の借方側と貸方側の合計の一致の両方。
- `GET /reports/income-statement?from&to&by=total|month` → `{ revenue: { rows, total }, expense: { rows, total }, net_income, months? }`。行は `{ code, name, amount, months?: [12個の数] }` で、科目ごとに平ら（親子を足さない）。`by=month` は1つの暦年の1月〜12月で、期間の外の月は0。最上位の `months` は `[{ month: "2026-01", revenue, expense, net_income }]` の12個。
- `GET /reports/balance-sheet?as_of=` → `{ as_of, assets: { rows, total }, liabilities: { rows, total }, equity: { rows, total }, net_income_current, balanced }`。行は `{ code, name, amount, own, children? }` で、親の `amount` は自分の `own` と子の合計を足した値。`equity.rows` の最後は「当期純利益」の行（`code` は `net_income_current`。0円でも出す）で、`equity.total` はそれを含む。残高が0で、残高のある子も無い行は出さない。`balanced` は `assets.total == liabilities.total + equity.total`。
- `GET /reports/ledger?account&from&to` → `{ account, opening, rows: [{ date, no, entry_id, description, counter, debit, credit, balance }], closing }`。`opening` は試算表と同じ値。
- `GET /reports/tax-summary?from&to` → `{ rows, sales_tax, purchase_tax, estimate, transitional, disclaimer }`。`rows` は `[{ tax_code, label, gross, tax }]`（使われた区分だけ）。

貸借対照表の例: 2026年に元入金500,000円、当期利益300,000円、事業主貸100,000円、事業主借20,000円。2027-03-31の時点で、2027年の利益が50,000円、事業主貸が10,000円のとき、元入金の行は 500,000+300,000+20,000−100,000 = 720,000円、当期純利益は50,000円、資産に事業主貸10,000円を含む。

## 消費税の集計（国税と地方消費税の合計での参考値）

- 税区分ごとの `gross`（税込の課税標準）と `tax`（含まれる税額）。売上の区分は貸方−借方、仕入の区分は借方−貸方で、返品や値引きは差し引かれる。
- 売上税額 = `S10` `S8` の `tax` の合計。仕入税額 = `P10` `P8` の `tax` の合計に、`P10X` `P8X` の明細ごとの控除額を足す。控除額は、税額の絶対値に経過措置の割合をかけて円未満を切り捨て、税額の符号を付ける（0へ向かって切り捨てる。仕入と、その全額の返品が、0円で相殺される）。割合は取引の日付で表から引く。`kamoku-jp-tax` の `transitional_credit` は国税分だけを返すので、この集計では使わない。
- 見込み額 = 売上税額 − 仕入税額（本則課税の参考値）。簡易課税と特例は計算しない。
- `transitional` は、割合ごとの `{ percent, gross, tax, deductible }`。
- `disclaimer`: 「参考値です。簡易課税と特例、経過措置の適用条件（請求書等と帳簿の保存、一の仕入先ごとの上限）は反映していません。課税の区分は現在の設定を使っています。経過措置の表の最終確認日は2026-10-14です。」。日付は `TRANSITIONAL_TABLE_CHECKED_ON` から出す。

## 書き出し

すべて `GET`。ファイル名は `Content-Disposition` で付ける。CSVは `?encoding=utf8bom|sjis`（既定は `utf8bom`）、改行はCRLF、引用符はRFC 4180。

| 要求 | 内容 |
|---|---|
| `/export/journal.csv?from&to&state=active\|all` | 1行が1明細。列: 仕訳No, 版, 状態, 日付, 摘要, 取引先, 行, 貸借, 科目コード, 科目名, 金額, 税区分, 税額, 行メモ。既定は有効な仕訳だけ |
| `/export/trial-balance.csv` `/export/income-statement.csv?by=total\|month` `/export/balance-sheet.csv` `/export/ledger.csv?account` `/export/tax-summary.csv` | 画面の帳票と同じ数字 |
| `/export/all.json` | 設定、科目、取引先、全イベント（明細・付随情報・ハッシュ）。`schema` の版を持つ |
| `/export/backup.db` | `VACUUM INTO` で作ったSQLiteの複製 |

CSVの文字列の欄（摘要・取引先・メモ・科目名など）は、先頭が `=` `+` `-` `@`、タブ、改行（CR）のとき、先頭に `'` を足す。数値の欄は対象外。

`sjis` で書き出すとき、Shift_JISに無い文字は `?` に置き換え、その数を応答の見出し `X-Kamoku-Warning: unmappable=<数>` で知らせる（摘要は明細の行ごとに出るので、行の数だけ数える）。画面は、これを日本語の案内に読み替える。
