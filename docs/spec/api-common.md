# 通信の約束（0）共通のきまり

画面（ブラウザ）と、手元のサーバーの間の約束です。0.x の間は、予告なく変わります。個別の約束は次の4つにあります。

- [設定・科目・口座・取引先](api-masters.md)
- [仕訳](api-entries.md)
- [取込と未確認](api-inbox.md)
- [帳票・ホーム・書き出し](api-reports.md)

## 基本

- 基点は `/api/v1`。本文はJSON（UTF-8）で、`Content-Type: application/json`。ファイルの送信だけ `application/octet-stream`。
- 金額は整数の円。日付は `YYYY-MM-DD`、月は `YYYY-MM`、年は `YYYY`。時刻はUNIX秒。
- 一覧は `limit`（1〜500、既定50）と `offset` で区切り、`{ "items": [...], "total": 123 }` を返す。
- 項目名は小文字の英数字と `_`。表示用の日本語の名前は、`name` `label` `message` に入る。

## 入り口と認証

サーバーは `127.0.0.1` だけで待ち受けます。起動すると、毎回違う合言葉（トークン）が付いたアドレス `http://127.0.0.1:<port>/?t=<token>` を表示してブラウザで開きます。

1. `GET /?t=<token>` が合言葉を確かめ、新しいセッション値（128ビットの乱数）を作って、Cookie `kamoku_session`（HttpOnly、SameSite=Strict、Path=/）に入れ、`/` へ移す。合言葉は、同じ起動の間は何度でも使える。再起動すると、合言葉もセッション値も全部失効する。
2. 以後の要求は、Cookieの値が、起動中に発行したセッション値のどれかと一致しなければ `401`。値の中身を調べずに通してはいけない。
3. `Host` が `127.0.0.1:<port>` か `localhost:<port>` でなければ `421`。この検査は、`/?t=` を含む全部の経路に掛ける。DNSの付け替えによる攻撃への備え。
4. `POST` `PUT` `PATCH` `DELETE` は、`Origin` が**必須**で、`http://127.0.0.1:<port>` か `http://localhost:<port>` のどちらかと完全に一致すること。欠けている、`null`、違うオリジンはすべて `403`。`Sec-Fetch-Site` が付いていて `same-origin` でなければ `403`。ブラウザは変更の要求に必ず `Origin` を付けるので、通常の利用には影響しない。`curl` など別の道具は、`Origin` を自分で付ける。

応答には、`Content-Security-Policy: default-src 'self'; img-src 'self' data:; style-src 'self'; script-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'`、`X-Content-Type-Options: nosniff`、`Referrer-Policy: no-referrer` を、すべてに付けます。`Cache-Control: no-store` は、API と、`/?t=` の移動の応答に付けます。アクセスの記録には、合言葉（`t=`）を残しません。外部への通信は、サーバーも画面も一切しません。

## 失敗の形

```json
{ "error": { "code": "unbalanced", "message": "貸借が一致しません。借方 11,000円、貸方 10,000円です。差の1,000円を、どちらかの行に足してください。", "field": "lines" } }
```

`message` は、原因と直し方を日本語で書く。`field` は、入力欄に結びつくときだけ付ける。

| 状態 | 意味 | `code` の例 |
|---|---|---|
| 400 | 形が壊れている。`Content-Type` が違うときも（415は使わない） | `bad_request` `invalid_csv` |
| 401 | Cookieがない、または値が無効 | `unauthorized` |
| 403 | 出所が違う | `forbidden_origin` |
| 404 | 見つからない | `not_found` |
| 405 | その経路で使えない方法（`Allow` を付ける） | `method_not_allowed` |
| 408 | 受け付けてから30秒を過ぎた、または読み取りの時間切れ | `timeout` |
| 409 | いまの状態と合わない | `already_voided` `duplicate_name` `read_only` `stale` `stale_revision` `client_id_reused` `already_confirmed` `protected_account` |
| 413・414・431 | 大きすぎる（JSONは1MB、CSVは10MB、要求行は8KB、見出しは16KB） | `too_large` |
| 421 | `Host` が違う | `bad_host` |
| 422 | 入力が規則に反する | `unbalanced` `locked_period` `unknown_account` `disabled_account` `amount_range` `reason_required` `missing_account` `bad_tax_code` `bad_invoice_number` `period_spans_years` |
| 500 | 想定外。内容は画面に出さず、サーバーの標準エラーに書く | `internal` |
| 501 | 扱わない通信（`Transfer-Encoding`） | `not_implemented` |
| 503 | 同時接続の上限、または再送防止の表がいっぱい | `busy` |

HEADへの応答には、断るときも本文を付けません（期限切れの408と混雑の503は、要求を読む前に決めるので、本文を付けます）。`DELETE` の成功は、`Content-Length` を毎回付けるため、204でなく `200 { "id": ..., "deleted": true }` です。

## 受け口の限界

HTTP/1.1（と1.0）だけを、標準ライブラリで受けます。毎回 `Connection: close` で、持続接続と分割転送（`Transfer-Encoding`）は扱いません。`Expect: 100-continue` は、見出しの検査を通ったときだけ受けます。要求を読む時間は、見出し10秒・本文30秒・応答の書き込み30秒で、超えたら閉じます。処理のスレッドは8本、同時の接続は32本までで、超えた分は `503 busy` で断ります。巨大な `Content-Length` は、本文を読む前に経路ごとの上限と比べて `413` にします。

## 読み取り専用の状態

起動時の連鎖の検証に失敗したときは、書き込みの要求をすべて `409 read_only` で断り、`GET /status` の `ledger.verified` を `false`、`ledger.error` に最初の不正（連番・規則）を入れます。読み取りは、壊れた連番の手前までの台帳だけを元に答えます（`ledger.events_loaded` と `ledger.events_in_db` を分けて返す）。

## 別のプロセスが書いたとき

同じ保存先に別のプロセスが書き込むと、メモリの台帳が古くなります。変更の要求（`POST` `PUT` `PATCH` `DELETE`。プレビューも）は、処理の前に、設定と科目をDBから読み直します。台帳の末尾も変わっていれば、読み直したうえで、書き込みは `409 stale` を返し、プレビューなど書き込まない要求は続けます。画面は「他の場所で更新されました。読み込み直します」と出します。`GET` は読み直さないので、別のプロセスが変えた科目の名前などは、次の変更の要求までメモリの値で見えます。
