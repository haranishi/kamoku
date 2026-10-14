# 通信の約束（1）状態・設定・科目・口座・取引先

共通のきまりは[こちら](api-common.md)です。

## 状態と検査

`GET /status`

```json
{ "app": "kamoku", "version": "0.0.1", "schema": 1, "mode": "normal", "setup_done": true,
  "ledger": { "events": 128, "events_loaded": 128, "events_in_db": 128, "entries": 97, "head_seq": 128,
              "head_hash": "ab12...(64桁)", "last_recorded_at": 1792000000, "verified": true, "error": null },
  "inbox": { "draft": 12, "ignored": 2 } }
```

`mode` は `normal` か `demo`。デモのときは、画面の上に「デモのデータです」と常に出す。`version` は実行ファイルの版。`ledger.error.kind` は `bad_hash` `broken_link` `wrong_sequence` `rule_violation` `quick_check` のどれか。

`POST /verify`  本文 `{ "noted_hash": "64桁" | null }`

```json
{ "ok": true, "events": 128, "head_hash": "...", "noted": { "found": true, "seq": 100, "is_head": false } }
```

連鎖が壊れているときは、`{ "ok": false, "error": { "seq": 55, "kind": "bad_hash", "rule": null } }`。どちらも状態は200。

## 設定

`GET /settings` と `PUT /settings`（全項目を置き換え）

```json
{ "business_name": "サンプル事務所", "owner_name": "", "tax_mode": "exempt",
  "invoice_number": null, "rounding": "floor", "opened_on": "2026-01-01",
  "locked_until": null, "setup_done": true }
```

初期設定の前（`setup_done` が偽）は、`opened_on` が空文字。`PUT /settings` は、`setup_done` が偽なら空を受け、真なら `422`。`tax_mode` は `exempt`（免税）か `taxable`（課税）。`rounding` は `floor` `round` `ceil`。`invoice_number` は課税のときだけ入れられ、`T`+13桁の形を検査する。`locked_until` 以前の日付への書き込みは、`422 locked_period`。

## 勘定科目と口座

`GET /accounts?kind=expense&wallet=1&include_disabled=1`

```json
{ "items": [
  { "code": "exp_comm", "name": "通信費", "reading": "つうしんひ", "kind": "expense",
    "normal_side": "debit", "parent": null, "wallet": null, "default_tax": "P10",
    "default_pct": 100, "form_line": "通信費", "enabled": true, "system": true, "sort": 550 },
  { "code": "w02", "name": "エポスカード", "reading": "えぽすかーど", "kind": "liability",
    "normal_side": "credit", "parent": "accrued", "wallet": { "type": "card", "institution": "" },
    "default_tax": null, "default_pct": 100, "form_line": null, "enabled": true, "system": false, "sort": 1002 } ],
  "total": 2 }
```

- `kind` は `asset` `liability` `equity` `revenue` `expense`。`wallet.type` は `cash` `bank` `card` `ewallet`。
- `POST /accounts` 本文 `{ "name", "reading", "kind", "parent", "wallet": {...}|null, "default_tax", "default_pct" }` → 201。コードは、口座なら `w01` から、科目なら `x01` から順に付ける。
- `PATCH /accounts/{code}` 本文は `name` `reading` `enabled` `default_tax` `default_pct` のうち変えるもの。`owner_draw` `owner_inv` `capital` は、どの欄も変えられず、分類・親・コード・口座の種類を変えようとしても、すべて `409 protected_account`。科目の名前は重複できる（取引先は断る）。
- 初期設定の前で口座が1つも無いときは、起動時に「現金」（`w01`）を足す。
- `GET /wallets?as_of=2026-10-31` → 口座だけを残高つきで返す。カードの `balance` は未払い額（正）。`{ "items": [{ "code", "name", "type", "balance" }], "cash_total": 1250000, "owed_total": 84000 }`

## 取引先

`GET /partners?q=` / `POST /partners` / `PATCH /partners/{id}`

```json
{ "id": 3, "name": "サンプル通信", "reading": "さんぷるつうしん", "invoice_number": "T1234567890123", "note": "", "enabled": true }
```

名前は重複できない（`409 duplicate_name`）。`invoice_number` は形式を検査する（`422 bad_invoice_number`）。

## テンプレート（よく使う取引）

`GET /templates` / `POST /templates` 本文 `{ "name", "body" }` / `DELETE /templates/{id}`（成功は `200 { "id", "deleted": true }`）。`body` は、日付を除いた取引の入力（[仕訳](api-entries.md)の単純な形）。

## 期首残高

`GET /opening` → `{ "date": "2026-01-01", "balances": [{ "account": "w01", "amount": 50000 }], "capital": 50000, "entry_id": "e_..." | null }`

`PUT /opening` 本文 `{ "date", "balances": [{ "account", "amount" }] }`。資産と負債の科目だけ受け付ける（事業主貸と事業主借は、毎年0から始まるので入れられない。同じ科目を2回入れることもできない）。金額は0以上で、0の行は仕訳に入れない。資産は借方、負債は貸方に入り、元入金は差額（資産−負債）で貸借を合わせる（債務超過なら元入金は借方）。`date` の既定は開業日で、最初の会計年度の中の日付にできる。すでに期首残高の仕訳があれば、理由「期首残高の見直し」で訂正する。入れる行が1つも残らないときは、仕訳を作らず `entry_id` は `null` のまま。すでにある仕訳は、理由「期首残高なし」で取り消す。

科目は親子を持てる（口座は、現金・預金・未払金の子）。親の科目にも直接記帳できる。親の行の金額は、帳票では自分の分と子の合計を足して見せるが、試算表は科目ごとに平らな行で出し、二重に数えない。
