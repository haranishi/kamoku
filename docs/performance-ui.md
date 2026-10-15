<!-- SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors -->
<!-- SPDX-License-Identifier: CC-BY-4.0 -->

# UIの移行前後を同じ条件で測る

UIをRustへ移した効果は、画面が操作できるまでの時間と配信量で比較します。
`tools/bench-ui.py` は、移行前後の実行ファイルに埋め込んだ画面を測ります。
実行前の準備と、結果を読むときの限界を以下に示します。

## 同じビルド条件の実行ファイルを用意する

移行前後を別のcheckout・targetでreleaseビルドします。Rustの版と最適化の指定を
そろえ、headは `tools/build-rust-ui.sh` でWASMを作ってから本体をビルドします。
古い実行ファイルと、編集途中の `--ui-dir` を組み合わせた結果は使いません。
比較元のソース、toolchain、ビルド指定をJSONと一緒に記録してください。

Node、`e2e/node_modules`、ChromiumまたはChromeは準備済みのものを使います。
この道具は依存やブラウザを取得しません。他のE2Eやビルドが終わった、負荷の低い
同じ機械で実行します。次のパスは実際の置き場所に置き換えてください。

```sh
python3 tools/bench-ui.py \
  --baseline /path/to/baseline/kamoku --head /path/to/head/kamoku \
  --baseline-ref 9ad343060b821b9d9789c10766664c8689893120 \
  --head-ref uncommitted --work-dir /path/to/scratch \
  --samples 10 --input-samples 20 --output /path/to/ui-performance.json
```

## 同じ合成帳簿で初回表示・移動・入力を比較する

両方の `demo` に、同じ合成CSVを200行ずつ追加します。デモの台帳ハッシュ、件数、
仕訳帳の先頭ページが一致してから測ります。実データの保存先は使いません。

| 項目 | 計測する範囲 |
|---|---|
| `cold_home` / `cold_journal` | 新しいbrowser contextで、本文の数字・仕訳行がそろうまで |
| `home_to_journal` / `journal_to_home` | 読み込み済みの画面で、移動リンクを押して本文がそろうまで |
| `inbox_input` | 未確認200行の科目欄へ「つうし」を入力し、「通信費」の候補が選ばれるまで |

移行前後の実行順は交互に反転します。初回表示・移動は1回、入力は3回を慣らしに
使い、その後の生データ、中央値、p95をJSONへ出します。p95は値を小さい順に並べた
95百分位の順位で求めます。処理時間の上限を超えた試行や、画面を準備できない試行は
成功扱いにせず、終了コード1と `complete: false` を返します。

## ページ内の時計と操作を観測する時計を分けて読む

`p50_ms` / `p95_ms` はページ内の時計です。本文がそろうか候補が選ばれた後、
描画枠のコールバックと次のタスクまで測ります。`observed_*` はNode側の時計で、
Playwrightとの往復、クリック可能になるまでの待ち、条件の確認も含みます。
描画の機会までを測る値であり、画面へのピクセル表示やCore Web VitalsのINPを
測った値ではありません。`insertText` は日本語の変換操作を再現しません。

`bytes` はHTTPヘッダー等を含む転送量、圧縮後の本文、展開後の本文を分けます。
各試行の `resource_samples` にはJavaScript・WASM・CSS・API別の内訳も残します。
準備完了時までに記録された要求が対象で、その後の要求は含まれない場合があります。
URL、認証値、応答本文は残しません。

ブラウザは1プロセスを使い、OS・ファイル・サーバーは温まった状態です。新しい
contextのHTTPキャッシュは空でも、ブラウザ全体のWASM・コードのキャッシュが残る
可能性があります。CPU・回線の速度は制限せず、Chromium系の1環境だけを測ります。
別の機種や、本当の初回起動の速さを保証する結果には使えません。

共有するのは結果JSONとビルド条件です。認証URLを含む標準出力、Cookie、trace、
動画、DBは保存・共有しません。計測用server、browser、一時帳簿は終了時に片付けます。

PRの `performance` workflowも同じ入口を使い、`ui.json` とビルド条件を残します。
実行環境の揺れを含むため、速度の判断では生データと比較条件を確認してください。
