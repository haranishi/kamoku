# 依頼R2b: サーバーとコマンド（kamoku-app）

先に `00-common.md` を読んでください。あなたの担当は、手元で動くサーバー `kamoku-app`（実行ファイル名 `kamoku`）です。保存の層（`kamoku-store`）と計算の部品（`kamoku-books` `kamoku-import` `kamoku-ledger` `kamoku-jp-tax`）は、前の担当が作りました。画面（`ui/`）は別の担当が作っています。

- 触る場所: `crates/kamoku-app/`、`docs/legal/rights-check-phase1.md`（依存の行の追記だけ）、リポジトリ直下の `Cargo.toml` の `[workspace.dependencies]`、`deny.toml`（必要なときだけ）。`ui/` は**読むだけ**（画面の担当のもの。埋め込みの対象）。
- 進捗: `.agent-harness/runs/2026-10-14-phase1/r2b-progress.md`。
- 読むもの（必須）: `docs/spec/` の全ファイル（特に `api-*.md` が**契約**です。項目名・状態コード・失敗の`code`を一字一句そろえる）、`docs/architecture.md`、`docs/legal/license-policy.md`。前の担当の最終報告と `r1-progress.md` `r2a-progress.md` `spec-issues.md`。
- 依存: `tiny_http`、`serde` `serde_json`、`getrandom`、`time`（`formatting` `parsing` `macros` `std`）、`encoding_rs`（Shift_JISの書き出し）、`kamoku-*`。試験専用: `tempfile`。ほかは足さない。**`tiny_http` が `cargo deny` の脆弱性・未保守の指摘に当たる、または0.x系で穴があると分かった場合は、止めて報告する**（代わりの案は `axum`。選び直すのは設計担当）。

## 作るもの

`docs/spec/api-common.md` `api-masters.md` `api-entries.md` `api-inbox.md` `api-reports.md` の**すべての要求**と、`nfr.md` の「安全」を実装します。

### コマンド

```
kamoku serve  [--data DIR] [--port N] [--no-open] [--ui-dir DIR]
kamoku demo   [--port N] [--no-open] [--ui-dir DIR]
kamoku verify [--data DIR]
kamoku backup --out FILE [--data DIR]
kamoku --version / --help
```

- 引数の解析は自前（依存なし）。メッセージは日本語。`serve` は、データの置き場所を `--data` > 環境変数 `KAMOKU_DATA` > 既定（macOS `~/Library/Application Support/kamoku`、Linuxの `$XDG_DATA_HOME` または `~/.local/share/kamoku`、Windowsの `%APPDATA%\kamoku`）の順に決め、`kamoku.db` を使う。待ち受けは `127.0.0.1` だけ。ポートは既定8765で、使用中なら次の番号を最大20回試す。`--port 0` で空いている番号。起動すると `http://127.0.0.1:<port>/?t=<合言葉>` を表示し、`--no-open` でなければ標準のブラウザで開く（macOS `open`、Linux `xdg-open`、Windows `cmd /c start`。失敗しても続ける）。
- `demo` は、`std::env::temp_dir()/kamoku-demo/` を**毎回作り直す**（中身を消す前に、置き場所に私たちが作った目印のファイル `.kamoku-demo` があることを確かめる。目印が無ければ断る）。合成データを入れて起動する。本物の置き場所には一切触れない（AC-12.1）。`/status` の `mode` は `demo`。
- `verify`: 台帳を読み込んで検証し、イベント数・仕訳数・末尾のハッシュを表示。壊れていれば最初の連番と規則を表示して終了コード1。
- `backup`: `VACUUM INTO` で複製（既存のファイルがあれば断る）。

### サーバー

- 同期型の `tiny_http` を、数本（4本）のスレッドで回す。`Store` は `Mutex` の中。1要求の処理は、ロックを取って、計算して、放す。
- 経路は、1つの表で定義する（メソッド・パス・処理）。パスの `{id}` を取り出す。存在しない経路は404（`not_found`）。メソッド違いは405。
- 入り口と認証（`api-common.md`）: 起動ごとに `getrandom` で作る128ビットの合言葉。`GET /?t=` で定数時間に比べ、Cookie `kamoku_session`（HttpOnly; SameSite=Strict; Path=/）を付けて `/` へ303。合言葉なしの要求は401（画面用のページは日本語の短い案内。`/api/` はJSON）。`Host` の検査（421）と `Origin` の検査（403）、本文の上限（JSON 1MB・CSV 10MB。413）、`Content-Type` の検査。応答のヘッダー（CSPほか）はすべての応答に付ける。
- 読み取り専用（`store.read_only()`）のときは、書き込みの要求を `409 read_only` で断る。
- 静的ファイル: `build.rs` が `../../ui/` 以下（`tests/` `dev/` `node_modules/` `README.md` を除く）を走査して、`include_bytes!` の一覧（パス・種類・中身）を `OUT_DIR` に生成し、`rerun-if-changed` を付ける。`--ui-dir` を指定したときは、ディスクから読む（`..` を使ったパスの抜けを、正規化して拒む）。`GET /` は `index.html`。拡張子ごとのContent-Type。`Cache-Control: no-cache`。
- 失敗は、すべて `{ "error": { code, message, field? } }`。想定外の失敗（500）は、内容を応答に出さず、標準エラーに書く。
- ID: 仕訳IDは `e_` + 時刻（ミリ秒）の16進12桁 + 乱数8桁の16進（`EntryId` の規則に合う）。
- 仕訳の組み立ては `kamoku-books` を呼ぶだけで、規則を自前で持たない。帳票・ホームのまとめは `kamoku-books` の関数を呼んで組む。
- CSV（`api-reports.md`）: RFC 4180の引用符、改行はCRLF、`utf8bom`（既定）と`sjis`（`encoding_rs`で変換。表せない文字は`?`にして、応答のヘッダー `X-Kamoku-Warning` で知らせる）。式の実行を防ぐ処理（先頭が `= + - @` で、数値の列でない文字列には `'` を足す）。
- `/export/all.json` は、設定・科目・取引先・全イベント（明細・付随情報・ハッシュ）。`/export/backup.db` は、`VACUUM INTO` で一時ファイルに作って返し、消す。

### 合成データ（`demo`）

`demo.rs` に、**乱数の種を固定した**（自前の線形合同法などでよい。依存を足さない）合成データの作り手を置く。2026年の1月から9月までの帳簿。口座は「現金」「サンプル銀行 普通預金」「サンプルカード」の3つ。売上は月に1〜2件（架空の取引先）、費用は月に10〜25件（通信費、交通費、消耗品、会議費、外注費、ガソリンのような按分つきなど）、事業主貸・事業主借を数件、訂正を2件、取消を1件、期首残高。取引先は架空の名前5つ。未確認に、合成のCSVから取り込んだ明細を25件（科目は未設定。重複を含む）。名前に実在の会社・店は使わない。**同じ入力から同じ`head_hash`になる**こと（`recorded_at` も固定の時刻から作る。AC-12.2）。

## 試験

- `tests/`: 試験用に `127.0.0.1:0` で起動する小さな道具（一時フォルダ・合言葉の取得・Cookie付きのHTTPクライアントを`std::net::TcpStream`で）。**仕様の全要求**に、少なくとも1つの試験。正常系と、`api-*.md` に書いた失敗の`code`の主なものを確かめる。試験の名前かコメントに、満たす条件の番号を書く。
- 流れの試験（AC-02.2 AC-02.6 AC-03.1 AC-04.3 AC-04.4 AC-04.7 AC-05.4 AC-05.5 AC-06.1〜AC-06.5 AC-07.2〜AC-07.5 AC-08.1〜AC-08.7 AC-09.1〜AC-09.4 AC-13.1）:
  - preview と作成の明細が同じ。按分の合計が一致。貸借が合わない入力は422。
  - CSVを送る（UTF-8とShift_JISの両方。`crates/kamoku-import/tests/fixtures/synthetic/` を使う）→ preview → commit → 同じファイルをもう一度（重複）→ 未確認の一覧 → 科目を付けて確定 → 仕訳帳と試算表に現れる、確定前は現れない。
  - 訂正と取消、理由なしの拒否、履歴、締めた日、取消済みの二重取消（409）。
  - 帳票の貸借の一致（複数年の仕訳を入れた状態も）。ホームのまとめの項目が仕様と一致。
  - 書き出しのCSV（UTF-8のBOM、Shift_JIS、式の実行を防ぐ処理）、`all.json`、`backup.db` が開けて同じ`head_hash`。
- 安全の試験: 合言葉なし401、違う`Host` 421、違う`Origin` 403、`..` を含む静的パスは404、大きすぎる本文は413、壊れたJSONは400、メソッド違いは405、Cookieの属性、CSPなどのヘッダー、DBファイルの権限、`demo` が本物の置き場所を触らないこと（環境変数 `KAMOKU_DATA` に別の一時フォルダを入れて確かめる）。
- 改ざん: DBを壊してから `serve` を起動すると、読み取り専用になり `/status` に最初の不正が出て、書き込みは409。
- `examples/bench.rs`: 1万仕訳の台帳で、ホームのまとめの応答時間を測って表示（`nfr.md` の目標と比べる材料）。
- 外へ通信しない確認: ソースに、HTTPクライアント（`ureq` `reqwest` `hyper::client` など）や `TcpStream::connect` が本番コードに無いこと（試験用の道具を除く）を、ソースの文字列検索の試験で確かめる。

## 終えたと言える条件

- `00-common.md` の品質の門がすべて通る。前の担当の試験は無変更で通る。
- `cargo run -p kamoku-app -- demo --no-open` で起動し、`curl` で `/?t=` → Cookie → `/api/v1/status` と `/api/v1/reports/dashboard` が仕様どおりの形で返る（実際に実行して確かめ、結果を報告に貼る）。`ui/` に画面ができていれば、ブラウザで開いて目で確かめる。
- 最終報告に、**わざと誤りを入れても試験が赤くなるか**を5か所以上（例: Hostの検査を外す、Originの検査を外す、読み取り専用の検査を外す、CSVの式防止を外す、`demo` の置き場所の目印の検査を外す）確かめた結果を書く。
- 仕様の要求のうち、実装していないもの・ずれているものの一覧を書く（無ければ「無し」）。

## 設計レビューで追加された条件（2026-10-14。上の本文より優先）

別のモデルによる設計書のレビューで、次が加わりました。`docs/spec/api-common.md`（入り口と認証、失敗の形）、`api-entries.md`、`api-inbox.md`、`api-reports.md`（年またぎの決まり、消費税の集計、書き出し）を**読み直して**ください。

- **認証**: Cookieの値は、起動中に発行したセッション値と**照合**する（任意の値は401）。再起動で全部失効。`Host` の検査は `/?t=` を含む全経路。`POST` `PUT` `PATCH` `DELETE` は `Origin` が必須で、自分のオリジンと完全一致（欠落・`null`・不一致は403）。`Sec-Fetch-Site` があって `same-origin` でなければ403。`/?t=` の移動の応答にも `Cache-Control: no-store`。標準エラーや記録に、`t=` の値と、リクエストの問い合わせ文字列を出さない。
- **二重送信防止**: `POST /entries` の `client_id`（10分、メモリ上の表。同じ値で本文が同じなら最初の結果を `200` で返す。本文が違えば `409 client_id_reused`）。`PUT` と `void` の `expected_revision`（違えば `409 stale_revision`）。ストアの `Stale`（別のプロセスが書いた）は `409 stale`。
- **取込**: `POST /imports/upload?encoding=auto|utf-8|shift_jis` と応答の `warnings`。`mapping.id_col`。プレビューの `forceable`、コミットの `force_lines`。`PATCH /drafts/{id}` の `date`。`confirmed` の行への `PATCH` `ignore` は `409 already_confirmed`。確定は1件ずつ検査し、通った行を**1つのトランザクション**で（失敗時は全体の失敗で応答し、行ごとの `failed` と区別する）。
- **帳票**: 期間は1つの暦年に収まること（またぐと `422 period_spans_years`）。試算表の `opening` `closing` `balanced`、貸借対照表の `equity` の「当期純利益」の行、`own` と `children`、消費税の式（`kamoku-books` が実装済みのものを呼ぶだけ）。`/export/income-statement.csv?by=month`、`/export/tax-summary.csv`、`journal.csv` の `state=active|all`（既定は有効だけ）。CSVの文字列の欄の先頭が `=` `+` `-` `@`、タブ、CRのときに `'` を足す（試験にタブとCRを含める）。
- **状態**: `/status` の `ledger` に `events_loaded` と `events_in_db` を分けて入れる。読み取り専用のとき、読み取りは壊れた連番の手前までの台帳で答える。
- **期首残高**: `PUT /opening` の0円の行・全部0・債務超過の扱いは `api-masters.md` のとおり（`kamoku-books` の `compose_opening` を使う）。全部0なら、既存の仕訳を「期首残高なし」の理由で取り消す。
- **`demo`**: `--data` と環境変数 `KAMOKU_DATA` を**無視**する（デモは本物の置き場所を一切指さない）。一時フォルダがシンボリックリンクなら断る。
- **権限**: `-wal` `-shm`、バックアップ、`backup.db` の一時ファイルも0600。書き出しの一時ファイルは、返し終えたら消す。

## R1・U1の完了後に決まったこと（2026-10-14 20:10。上の本文より優先）

- `build.rs` の静的配信から、`ui/package.json` も外す（`tests/` `dev/` `node_modules/` `README.md` に加えて）。
- `GET /entries/{id}` の履歴の `kind` は、台帳のイベントの種類と同じ `posted` `revised` `voided`。`GET /entries` の `state` の既定は `active`。
- 期首残高（`meta.kind` が `opening`）の仕訳への `PUT /entries/{id}` と `void` は、`422 bad_request`（「期首残高は、設定の「期首残高」から見直してください」）。
- `GET /status` の `ledger` は、常に `events_loaded` と `events_in_db` を返す。
- `kamoku-books` の公開APIは、`.agent-harness/runs/2026-10-14-phase1/r1-progress.md` の「次の担当への引き継ぎ」と、ソースのdocコメントにある。仕様の疑問への決定は `spec-issues.md` と `docs/spec/` に反映済み（帳票の `months`、貸借対照表の当期純利益の行、消費税の `rows`、`disclaimer` の句点）。
- 模擬サーバー（`ui/dev/mock-server.mjs`）は、画面の担当（U1・U2）が仕様どおりに作った一時の道具です。**本物の応えの形の基準にしてはいけません**（仕様書が基準）。食い違いを見つけたら、`spec-issues.md` に書いてください。

## 受け口の決定（2026-10-14 20:45。上の本文の `tiny_http` に関する部分より優先）

R2bの実測で、`tiny_http` 0.12 は、巨大な `Content-Length` の要求1つで、メモリ確保に失敗してプロセスごと異常終了する（時間切れも接続数の上限もない）ことが分かった。**HTTPの受け口は、標準ライブラリだけで書いた小さなHTTP/1.1の部分集合にする**（依存は足さない）。範囲、上限、時間切れ、必須の試験は、設計担当からR2bへの連絡のとおり（`docs/architecture.md` の「依存を絞る」にも理由を記録した）。
