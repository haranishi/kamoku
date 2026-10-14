# 通信の約束（2）仕訳

共通のきまりは[こちら](api-common.md)です。

## 入力の形

単純な形（`kind` が `expense` `income` `transfer`）と、仕訳の形（`journal`）の2つです。

```json
{ "kind": "expense", "date": "2026-10-05", "amount": 11000, "account": "exp_comm",
  "wallet": "w02", "tax_code": "P10", "partner": "サンプル通信",
  "description": "10月分 通信費", "business_pct": 70, "memo": "", "client_id": "c_7f3a..." }
```

- `expense`: `account` は費用などの相手科目、`wallet` は支払った口座。`business_pct`（0〜100、既定は科目の `default_pct`）が100未満なら、残りを事業主貸に入れる。
- `income`: `account` は収益などの相手科目（`sales` `misc_income` `owner_inv` など）、`wallet` は入金先。`business_pct` は100だけ。
- `transfer`: `wallet`（移す元）と `to_wallet`（移す先）。科目・税区分・割合は無い。同じ口座どうしは `422`。
- 相手科目が**口座**のとき（カードの引き落とし、預金から現金の引き出しなど）は、税区分と事業の割合を使えない。`tax_code` が空でない、または `business_pct` が100でなければ `422 bad_request`。カードで買ったときは「費用／カード」、引き落としは「カード／預金」と、2回に分けて入れる。
- `tax_code` は `S10` `S8` `P10` `P8` `P10X` `P8X` `NT` のどれか、または `null`（対象外）。税込の金額から税額を出す。`NT` の税額は0。`P10X` `P8X` の判定は仕訳の日付なので、日付は課税仕入れの日（商品の引渡し、役務の提供が完了した日）にする。
- `journal`: `{ "kind": "journal", "date", "description", "partner", "lines": [{ "account", "side": "debit"|"credit", "amount", "tax_code", "tax_amount", "memo" }] }`。貸借が合わなければ `422 unbalanced`。`tax_amount` を省くと、その明細の金額から計算する（記帳上の目安）。
- `client_id` は、画面が作る再送防止の印（任意、64バイトまで。超えたら `400 bad_request`）。同じ値の `POST` が10分以内にもう一度来たら、新しく作らず、最初の結果をそのまま返す（`200`）。同じ値で本文が違えば `409 client_id_reused`。サーバーは本文のSHA-256と最初の応えだけを、10分・1万件・8MBまで覚える。いっぱいのときは、期限内の記録を追い出さず、新しい印の要求を保存の前に `503 busy` で断る。同じ印・同じ本文の再送は、いっぱいでも `200` で最初の結果を返す。

税区分のコード: `S10` 課税売上10%、`S8` 課税売上8%（軽減）、`P10` 課税仕入10%、`P8` 課税仕入8%（軽減）、`P10X` `P8X` 免税事業者等からの課税仕入れ（経過措置）、`NT` 非課税。

### 明細の組み立て

- 支出: 借方に「事業分の科目」と、残りがあれば「事業主貸」、貸方に口座。事業分 = 支払額×割合を円未満で四捨五入。**0円の明細は作らない。** 割合が100なら事業主貸の行は無く、事業分が0円になる（割合0、または端数）なら科目の行は無い（借方は事業主貸だけ）。事業分が0円のときは `warnings` に出す。
- 収入: 借方に口座、貸方に科目。振替: 借方に移動先、貸方に移動元。

## 仕訳の表現（応答）

```json
{ "id": "e_0192ab3c4d5e6f7a8b9c", "no": 97, "date": "2026-10-05", "kind": "expense",
  "description": "10月分 通信費", "partner": "サンプル通信", "total": 11000,
  "revision": 1, "state": "active", "recorded_at": 1792000000, "last_reason": null,
  "lines": [
    { "account": "exp_comm", "account_name": "通信費", "side": "debit", "amount": 7700, "tax_code": "P10", "tax_amount": 700, "memo": "" },
    { "account": "owner_draw", "account_name": "事業主貸", "side": "debit", "amount": 3300, "tax_code": null, "tax_amount": 0, "memo": "家事按分 30%" },
    { "account": "w02", "account_name": "エポスカード", "side": "credit", "amount": 11000, "tax_code": null, "tax_amount": 0, "memo": "" } ],
  "meta": { "kind": "expense", "source": "manual", "partner": "サンプル通信", "business_pct": "70" } }
```

`state` は `active` か `voided`。取消済みのとき `lines` は最後に有効だった版の明細を返す（見せるため）。

詳細（`GET /entries/{id}`）と、作る・直す応答には `"input"` も付く。単純な形で作った仕訳を、明細から組み立て直した入力（`kind` `date` `amount` `account` `wallet` `to_wallet` `tax_code` `partner` `description` `business_pct` `memo`）で、訂正の画面の初期値に使う。仕訳の形（`journal`）や期首残高、組み立て直せない明細のときは `null`で、画面は仕訳の表で開く。

## 一覧と詳細

`GET /entries?from=&to=&account=&wallet=&q=&kind=&state=active|voided|all&min=&max=&source=&limit=&offset=`（`state` の既定は `active`）
→ `{ "items": [仕訳...], "total": 97, "sum_amount": 4630000 }`。日付の新しい順、同じ日は番号の大きい順。`q` は摘要・取引先・メモ・科目名・金額の文字列に効く。`account` は、その科目を含む仕訳。

`GET /entries/{id}` → 仕訳に `"history": [{ "seq", "kind", "revision", "recorded_at", "reason", "hash", "lines": [...]|null }]` を足したもの。履歴の `kind` は、台帳のイベントの種類と同じ `posted`（登録）`revised`（訂正）`voided`（取消）。

## 作る・直す・取り消す

- `POST /entries/preview` → 本文は上の入力と同じ。保存せずに `{ "lines": [...], "total": 11000, "warnings": ["..."] }` を返す。保存で作られる明細と、同じ入力・同じ設定なら必ず同じ。
- `POST /entries` → 201、仕訳の表現。
- `PUT /entries/{id}` → 本文は入力と同じに `"reason": "金額の入力ミス"`（必須）と `"expected_revision": 1`（画面が見ていた版。必須）を足す。版が違えば `409 stale_revision`。新しい版ができる。
- `POST /entries/{id}/void` → 本文 `{ "reason": "二重に登録したため", "expected_revision": 1 }`（どちらも必須）。取消済みの仕訳には `409 already_voided`。

## 検査（サーバーが必ず行う）

- 金額は1以上、9,999,999,999,999以下（`amount_range`）。割合は0〜100の整数。日付は実在する日。
- 科目と口座は存在し、有効であること（`unknown_account` `disabled_account`）。
- `locked_until` 以前の日付は断る（`locked_period`）。訂正で日付を動かすときは、動かす前後の両方を見る。
- 訂正と取消の理由は、空白を除いて1文字以上（`reason_required`）。
- 期首残高（`meta.kind` が `opening`）の仕訳は、`PUT` と `void` を `422 bad_request`（「期首残高は、設定の「期首残高」から見直してください」）で断る。見直しは `PUT /opening` から行う。
- 取引先の名前があって、マスタに無ければ、同じトランザクションの中で追加する。登録番号があれば、`invoice_no` に入れる。
- `tax_code` が `P10X` `P8X` のとき、経過措置の割合は仕訳に入れず、集計のときに日付で表から引く。
