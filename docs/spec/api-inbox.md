# 通信の約束（3）取込と未確認

共通のきまりは[こちら](api-common.md)です。取り込んだ明細は「下書き」に入り、確定するまで台帳には入りません（[データの形](data-model.md)）。

## ファイルを送る

`POST /imports/upload?name=meisai.csv&encoding=auto`  本文はCSVのバイト列（10MBまで）。`encoding` は `auto`（既定）`utf-8` `shift_jis` で、画面から選び直せる。サーバーは、文字コードと区切りを判定して読み、メモリに最大30分置く。正しく読めないバイトがあれば、`warnings` に件数を入れる。

```json
{ "upload_id": "u_9f3a...", "filename": "meisai.csv", "encoding": "shift_jis", "delimiter": ",",
  "rows": 312, "header": ["日付", "摘要", "出金", "入金", "残高"],
  "sample": [["2026/10/01", "○○ストア", "1,650", "", "98,350"]],
  "guess": { "skip_rows": 1, "date_col": 0, "desc_cols": [1],
             "amount": { "mode": "split", "out_col": 2, "in_col": 3 } },
  "warnings": [], "saved": null }
```

`saved` は、その口座に覚えさせた対応（`wallet` を付けて問い合わせたとき）。壊れたCSVは `400 invalid_csv` で、行番号（`line`）を入れる。`rows` はファイル全体の行数（見出しを含む）、`header` は推測した読み飛ばしの行の直前の行、`sample` はその後ろの20行。見本の表の番号は見本の中の順で、ファイルの行番号ではない（確かめの `line` がファイルの行番号）。

## 列の対応（`mapping`）

```json
{ "skip_rows": 1, "date_col": 0, "desc_cols": [1], "id_col": null,
  "amount": { "mode": "split", "out_col": 2, "in_col": 3 } }
```

金額は2通り。`split` は入金と出金が別の列。`signed` は符号つきの1列で、`{ "mode": "signed", "col": 2, "inflow": "positive" }`（`inflow` は入金が `positive` か `negative` か）。日付は、`2026/10/1` `2026-10-01` `20261001` `2026年10月1日` `R8.10.1` `令和8年10月1日` を読む。金額は、カンマ・円記号・全角数字・`△` `▲` `−` の負を読む。

`id_col` は、金融機関が付けた取引番号の列（任意）。あれば、その番号が重複の判定のいちばん強い手がかりになる。

行ごとの判定（`invalid` の理由になる）:

- 摘要は、複数列なら1つの空白で連ねる。前後の空白を除き、連続する空白（全角を含む）は1つにする。全角の英数字と記号は半角にする。
- 入金と出金の両方に金額がある、どちらも空または0円、小数点がある、日付が読めない（2月30日など）、摘要が1000バイトを超える、摘要の列の指定が8個を超えるは `invalid`。取り込みでは飛ばし、一覧で理由を見せる。
- CSVの壊れた引用符（閉じた引用符のあとに、区切りと改行以外の文字が続く）は、金額が黙って変わらないよう、`400 invalid_csv`（行番号つき）で断る。1行の列数は1,000まで、ファイル全体のセルは200万までで、超えても `invalid_csv`。

## 取り込む前に確かめる

`POST /imports/preview`  本文 `{ "upload_id", "wallet": "w02", "mapping": {...} }`

```json
{ "rows": [ { "line": 2, "date": "2026-10-01", "description": "○○ストア", "direction": "out",
              "amount": 1650, "status": "new", "reason": null, "forceable": false } ],
  "counts": { "new": 300, "duplicate": 10, "invalid": 2 }, "truncated": true }
```

`status` は `new` `duplicate` `invalid`。`rows` は先頭の500行まで（`truncated`）。件数は全体の数。`duplicate` の行は `forceable: true` で、同じ日・金額・摘要の別の取引かもしれないので、利用者が選べば取り込める（下の `force_lines`）。

## 取り込む

`POST /imports/commit`  本文はプレビューと同じに `"save_profile": true` と、重複でも取り込む行の番号 `"force_lines": [14, 15]`（任意）を足す → `{ "batch_id": 7, "created": 300, "duplicates": 10, "invalid": 2 }`。重複の判定は[データの形](data-model.md)の決まり4。この方式は、部分的に重なるファイルで、同じ内容の本物の取引を落とすことがある。そのため、落とした行は黙って捨てず、プレビューに理由つきで見せる。

`GET /imports/profile?wallet=w02` → 覚えさせた対応か `null`。`GET /imports/batches?limit=&offset=` → 取込の記録（`items` `total`）。保存の層が読める新しい順の500件までで、`total` もその数。

## 未確認の一覧と直し方

`GET /drafts?status=draft|ignored|confirmed&wallet=&q=&from=&to=&limit=&offset=`

```json
{ "items": [ { "id": 31, "batch_id": 7, "wallet": "w02", "wallet_name": "エポスカード",
               "date": "2026-10-01", "description": "○○ストア", "direction": "out", "amount": 1650,
               "account": null, "tax_code": null, "partner": "", "memo": "", "business_pct": 100,
               "status": "draft", "entry_id": null } ],
  "total": 12, "counts": { "draft": 12, "ignored": 2, "confirmed": 80 } }
```

`counts` は、口座・文字・期間の絞り込みに関係なく、帳簿全体の数。

- `PATCH /drafts/{id}`  本文は `date` `account` `tax_code` `partner` `memo` `business_pct` のうち変えるもの。日付は、経過措置の判定に使う課税仕入れの日に合わせて直せる。`confirmed` の行は `409 already_confirmed`。
- 科目を変えたとき、`tax_code` と `business_pct` を新しい科目の既定値に変えるかは、画面の仕事（サーバーは入れない）。画面の決まり: いまの値が、**変える前の科目の既定値と同じ**なら新しい科目の既定値にする。違えば、利用者が決めた値とみなして残す。
- `POST /drafts/{id}/ignore`、`POST /drafts/{id}/restore`。どちらの応えも `{ "id", "status", "entry_id": null }`。無視した行の `PATCH` は `422 bad_request`。

状態の移り方:

| いまの状態 | できること |
|---|---|
| `draft` | 直す、`ignore`、`confirm` |
| `ignored` | `restore`（`draft` に戻る） |
| `confirmed` | 直せない。結びついた仕訳が取消済みのときだけ `restore`（`draft` に戻り、`entry_id` は空になる。以前の仕訳IDは変更履歴に残る） |

- `POST /drafts/confirm`  本文 `{ "ids": [31, 32] }`（一度に2,000件まで。送った要素の数で数え、超えたら何も始める前に `422 bad_request`。知らない番号は行ごとの `not_found`、無視した行は行ごとの `422 bad_request`で、`failed` は送った順）

```json
{ "confirmed": [ { "id": 31, "entry_id": "e_...", "no": 98 } ],
  "failed": [ { "id": 32, "error": { "code": "missing_account", "message": "科目が選ばれていません。" } } ] }
```

規則: 出金は「借方 科目（割合が100未満なら事業主貸も）／貸方 口座」、入金は「借方 口座／貸方 科目」。付随情報は `source=import` `draft=<id>`。検査は1件ずつ行い、通った行の仕訳、下書きの状態、取引先の追加、変更履歴を、**1つのトランザクション**で書く。書き込みに失敗したら全部戻し、応答は全体の失敗にする（行ごとの検査の失敗とは区別する）。すでに `confirmed` の行は `already_confirmed` で飛ばし、二重には書かない。

