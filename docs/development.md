# 開発の手引き: 手元で動かして、直して、確かめる

コードを直したい人向けの案内です。動かすだけなら[使い方](getting-started.md)を見てください。

## 必要なもの

- Rust 1.88以上と、Cのコンパイラ（[使い方](getting-started.md)）
- Node 24（画面の単体試験と、ブラウザの通し試験）
- Python 3（`tools/` の点検の道具。標準ライブラリだけで動きます）
- PR前の完全検証には `cargo-deny`、Rust 1.88、`uvx` と準備済みの `reuse` も必要です（[試験の実行手順](testing-procedure.md)）。

## どこに何があるか

| 場所 | 役割 |
|---|---|
| `crates/kamoku-ledger` | 追記だけの複式簿記の台帳。ハッシュの連鎖で、書き換えを見つける |
| `crates/kamoku-jp-tax` | 消費税の税率、端数処理、インボイスの経過措置（根拠つきの表） |
| `crates/kamoku-books` | 勘定科目、仕訳の組み立て、帳票、消費税の集計。入出力を持たない |
| `crates/kamoku-import` | 明細CSVの読み取り（文字コード、列の対応、指紋）。入出力を持たない |
| `crates/kamoku-store` | SQLiteへの保存、起動時の検証、下書きと確定 |
| `crates/kamoku-app` | 手元のサーバーとコマンド（`kamoku`）。HTTPの受け口は標準ライブラリだけ |
| `ui/` | 通常の画面は素のJavaScriptでビルド不要。任意の[Rustプレビュー](rust-ui-preview.md)も置く（[ui/README.md](../ui/README.md)） |
| `e2e/` | ブラウザを動かす通し試験（Playwright。開発専用） |
| `tools/` | 点検の道具（[tools/README.md](../tools/README.md)） |
| `docs/` | 設計と仕様。`docs/spec/` が、画面とサーバーの約束です |
| `.agent-harness/` | 作業の記録（実行の証拠、レビューの結果と対応） |

計算の部品（ledger・jp-tax・books・import）は、ファイルも通信も持ちません。保存とサーバーは外側の2つに閉じ込めてあります（[設計](architecture.md)）。

## 動かす

```
cargo run -p kamoku-app -- demo --ui-dir ui
```

`--ui-dir ui` を付けると、画面のファイルをディスクから読みます。`ui/` の下を直して、ブラウザを再読込するだけで確かめられます（付けないと、ビルド時に埋め込んだ版を使います）。サーバーなしで画面だけ見たいときは、模擬サーバー（`node ui/dev/mock-server.mjs`）があります。

Rustで書く画面の最初の範囲は、別経路の仕訳帳です。[ビルドと専用試験](rust-ui-preview.md)は通常の画面と分けています。

## 試験を通す

PRの前に、次を流してください。

```
tools/check-all.sh           # 書式、clippy、試験、文書、依存、ライセンス表示、画面の単体試験
e2e/run-real.sh              # ビルド、スモーク、操作、契約試験、スクリーンショット
```

`run-real.sh` はRustサーバーをビルドし、段階ごとに新しく起動します。前の段階のものを止めてから次へ進みます（127.0.0.1 のCookieはポートが違っても共有されるため）。`flows` `contract` `screens` を渡すと、その段階だけ流します。依存とブラウザの準備、変更別のコマンド、失敗時の記録は[試験の実行手順](testing-procedure.md)にあります。

`tools/check-all.sh --quick` は、依存の検査と最小のRustの確認を省き、結果にも省略を明記します。個別に流すなら、`cargo test -p kamoku-books --locked`、`node --test ui/tests/` のように、対象だけを指定します。

## 試験が「門番」として働くかを確かめる

試験が通っていても、守りたい規則が試験に守られているとは限りません。`tools/mutate.py` は、ソースにわざと誤りを入れて、試験が赤くなるかを確かめます。誤りの一覧は `tools/mutations/*.json` にあり、新しい守りを足したら、その守りを外す誤りも足してください。使い方は[tools/README.md](../tools/README.md)にあります。生き残った誤りは、試験の穴です。

## 仕様と試験の対応

画面とサーバーの約束は `docs/spec/` の「受け入れ条件」（`AC-02.1` のような番号）に書いてあります。試験の名前かコメントに、その番号を書いてください。`python3 tools/check-spec-trace.py --strict` は、試験からの参照が無い自動検証の条件があれば失敗します。**最初の貢献に向く作業**です（[最初の貢献の候補](good-first-issues.md)）。

## 変更を出すとき

[参加の手引き](../CONTRIBUTING.md)に従ってください。特に、次の3つを守ってください。

- 新しい機能、依存、保存形式の変更は、実装の前にIssueで相談する。
- 文書は1ファイル100行以内。`python3 tools/check-links.py` が、リンク切れと行数を検査します。
- 他の会計ソフトのコードや、特定の製品の画面・文言を写さない。
