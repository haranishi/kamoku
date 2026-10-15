# 道具: 固定値・試験・文書・受け入れ条件を、手元で点検する

リポジトリの点検に使う小さなスクリプトです。Pythonのものは標準ライブラリだけで動き、どれもネットワークにはつなぎません。リポジトリの直下で実行します。PRの前は、まず `tools/check-all.sh` を流してください。個別の道具は、自分の変更に関係するものだけで足ります。

## まとめて流す: check-all.sh

書式、clippy、Rustの試験、文書の警告、最小のRust（1.88）、依存の検査、画面と実行手順の回帰試験、文書のリンク、台帳の固定値、受け入れ条件の対応、ライセンス表示を順に流し、失敗した検査の名前を最後に一覧します。必須ツールの不足は開始前にエラーにし、quickで省く項目は結果にも明記します。REUSEは準備済みの環境をオフラインで使います。

```
tools/check-all.sh          # 全部
tools/check-all.sh --quick  # 依存の検査と、最小のRustの確認を省く
```

ブラウザを動かす通し試験は含みません（`e2e/run-real.sh`）。準備と変更別のコマンドは[試験の実行手順](../docs/testing-procedure.md)、各層の役割は[試験の地図](../docs/testing.md)にあります。

## 台帳のハッシュは、Rustとは別の実装で計算し直して比べる

`verify-ledger-golden.py` は、台帳（`kamoku-ledger`）のハッシュの符号化を、Pythonで書き直したものです。`crates/kamoku-ledger/tests/golden.rs` の中で `// GOLDEN-BEGIN 名前` から `// GOLDEN-END` までに書いた固定値を読み、自分で計算した値と比べます。

```
python3 tools/verify-ledger-golden.py          # 食い違えば終了コード1
python3 tools/verify-ledger-golden.py --print  # 計算した値を表示するだけ
```

符号化を変えるときの順番は決まっています。まず `crates/kamoku-ledger/src/encoding.rs` の先頭の仕様を書き換え、次にこのスクリプトを仕様に合わせて直し、`--print` の値を試験に貼ります。Rustの出力を貼ると、実装の誤りまで「正しい値」として固定してしまうためです。

## 取込の試験のCSVは、手で作らずにスクリプトで作る

Shift_JIS（Windows-31J）やBOMつきのファイルをエディタで作ると、文字コードや改行を取り違えやすくなります。そこで `make-fixtures.py` が、`crates/kamoku-import/tests/fixtures/synthetic/` に合成データのCSVを書きます。

```
python3 tools/make-fixtures.py          # 書く
python3 tools/make-fixtures.py --check  # 今のファイルと同じかを確かめる（違えば終了コード1）
```

中身は、名前に「サンプル」「デモ」を含む架空の店と金額だけです。ファイルを足すときも、実在の口座・店・金額は使わないでください。

## 受け入れ条件ごとに、試験からの参照を数える

どの受け入れ条件に、まだ試験が無いか。`check-spec-trace.py` は、`docs/spec/stories-*.md` の条件（`AC-02.1` など）ごとに、試験のどこから参照されているかを数えて答えます。探すのは `crates/**/*.rs`、`ui/**/*.mjs`、`e2e/**/*.mjs` で、種類が `manual` の条件は数えません。

```
python3 tools/check-spec-trace.py           # 一覧を出す（終了コードは常に0）
python3 tools/check-spec-trace.py --strict  # rust・node・e2e の条件に参照が無ければ終了コード1
```

試験の名前かコメントに条件の番号を書けば、参照として数えられます。数えるのは文字列の一致だけです。たとえば e2e の条件が crates からしか参照されていなければ、ブラウザの試験はまだ無いということです。試験の中身が条件を本当に満たしているかは、この道具では分からないので、レビューで確かめます。

## 試験が「門番」として働くかを、わざと誤りを入れて確かめる

`mutate.py` は、ソースの1か所にわざと誤りを入れ、試験が赤くなるかを見ます。赤くならない誤り（生き残り）は、試験の穴です。誤りの一覧は `tools/mutations/*.json` にあります。

```
python3 tools/mutate.py tools/mutations/books.json              # 一覧の全部
python3 tools/mutate.py tools/mutations/books.json --only 按分  # ラベルに「按分」を含むものだけ
```

リポジトリを一時的な場所へ写して試すので、手元のファイルは変わりません。コンパイルが通らない誤りは「検出」に数えず、直すべきものとして一覧に出します。新しい守りを足したら、その守りを外す誤りを一覧に足してください。

## 実際のバイナリで、入口と頑丈さを外から確かめる

`smoke-demo.py` は、できあがった `kamoku` を `demo` で起動し、HTTPで一通りたたきます。合言葉・Cookie・Host・Originの扱い、帳票の貸借の一致、CSVの書き出し、巨大なContent-Lengthやでたらめなバイト列を送ったあとも生きていることを確かめます。

```
cargo build -p kamoku-app
python3 tools/smoke-demo.py --bin target/debug/kamoku
```

## 文書のリンクと長さ

`check-links.py` は、文書の相対リンク切れ、1ファイル100行の上限、Obsidianなどでリンクを壊すファイル名（`#` `|` `[` `]` `^`）を検査します。

```
python3 tools/check-links.py
```
