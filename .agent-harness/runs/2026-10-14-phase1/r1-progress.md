# R1 の進み具合（計算の部品）

## できたこと

- T0〜T5 を終えた。コーディネーターの仕様変更1〜5も反映した（帳票の仮想の繰越と1暦年の期間、消費税の新しい式、0円の明細を作らない組み立て、期首残高の0円の扱い、取込の `id_col`・正規化・invalid・文字コードの指定）。
- 品質の門は全部通過（fmt、clippy、test 107件、doc、1.88、cargo deny）。`tools/check-all.sh`、REUSE、`tools/check-links.py` も通過。
- わざと誤りを入れる確認: 15か所すべてで試験が赤くなった（自前の台本と、共有の `tools/mutate.py` の両方で確認。生き残りなし）。定義は `tools/mutations/r1-*.json`（`python3 tools/mutate.py <ファイル>` で再実行できる）。
- proptest が見つけた不具合（区切りの判定の取り違え）を直し、固定の試験を足した。失敗の種は `tests/*.proptest-regressions` に残してある（変異の確認で見つかった種も含む。消してもよい）。

## 次にやること（次の担当へ）

- 保存（R2a）: 仕訳は `Composed::to_entry(id)` か `Entry::new` + `with_meta` で作る。`meta` はハッシュの対象なので、読み戻しで1バイトも変えない。下書きの確定は、下書きから `SimpleInput` を作って `compose_simple`（`Source::Import`）を呼び、`meta` に `draft` を足す。
- サーバー（R2b）: プレビューと保存は同じ `Composed` から作る（`line_views`・`warnings`・`total`）。`input` は `reconstruct_simple`。失敗は `ComposeError` / `ReportError` / `ImportError` の `code()`・`message()`・`field()`/`line()`。帳票は `kamoku-books` の関数を呼ぶだけ（期間は1暦年）。取込の重複は、読めた行だけを `number_occurrences(wallet, rows)` に渡す。
- 疑問と判断は `spec-issues.md` の「R1（計算の部品）」の節（20項目）。

## 詰まり

- なし。依頼書の外の依存は足していない（`encoding_rs` が引き込む小さな crate は `docs/legal/rights-check-phase1.md` に記録）。
