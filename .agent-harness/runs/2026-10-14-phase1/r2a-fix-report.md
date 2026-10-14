# レビュー指摘の修正結果

S1〜S7、I1〜I4、B1・B2を修正しました。品質の門4つは最終実行で全て成功しました。
指定コピーの3クレートと進捗・報告だけを変更し、本物のリポジトリ・仕様・依存定義・履歴は変更していません。

## 作ったものと試験

各クレートに `tests/review_fixes.rs` を追加しました（store 10件、import 5件、books 2件）。以下の試験名はこのファイル内です。

| 指摘 | 修正したファイル・方法 | 追加した試験 |
|---|---|---|
| S1 | store/src/inbox.rs：挿入前に重複を全件判定。新規を先に挿入し、強制重複に最大番号+1を割り当てる。検査・重複判定・採番・挿入を小関数に分割 | s1_force_reserves_new_occurrences_first |
| S2 | store/migrations/0001_init.sql：4表にBEFORE INSERTトリガー。イベントのseqと(entry_id,revision)の両方を保護 | s2_replace_cannot_overwrite_append_only_rows |
| S3 | store/src/db_ledger.rs、db_event.rs：明細・metaをイベントごとのResultで復元。負の連番・イベントのない行はVerifyFailureとして読み取り専用にする | s3_invalid_meta_opens_verified_prefix、s3_orphan_rows_are_corruption_not_open_errors |
| S4 | store/src/lib.rs：再読込がErrなら台帳と採番を空にし、再読込失敗のVerifyFailureを設定。元の保存エラーを返す | s4_failed_reload_clears_unpersisted_memory_and_preserves_save_error |
| S5 | store/src/confirmation.rs：期待したmetaにない鍵を拒否。invoice_no・draft・sourceは許可。既存の値の照合は保持 | s5_unexpected_meta_is_rejected、s5_invoice_and_store_owned_meta_are_allowed |
| S6 | store/src/posting.rs：TaxCode::parse、NT税額0、保存可能年0〜9999をpostとreviseの共通検査に追加 | s6_post_and_revise_reject_unreadable_date_and_tax、s6_accepted_date_boundaries_roundtrip |
| S7 | store/src/files.rs：既存フォルダのchmodを撤去。新規フォルダとバックアップ作業フォルダはDirBuilderの作成時に0700。ファイル0600は保持 | s7_existing_directories_keep_permissions |
| I1・I3 | import/src/mapping.rs：8列超を先に拒否。連結する各セルと空白を加える前に1000バイト上限を確認。正規化後も検査 | i1_too_many_description_columns_rejected_before_join、i1_i3_description_byte_boundaries |
| I2 | import/src/csv.rs、error.rs：セルを追加する前に1000列・総200万セルの上限を検査。区切り判定にも適用。空白行のセルも数える | i2_column_limit_and_delimiter_detection_are_bounded、i2_total_cell_limit_includes_blank_records |
| I4 | import/src/csv.rs、error.rs：閉じ引用符の後の区切り・改行・終端以外を行番号つきinvalid_csvで拒否 | i4_characters_after_closing_quote_are_invalid_csv |
| B1 | books/src/chart.rs、reports/mod.rs：親をたどる内部補助で事業主勘定の子・孫も年初に0へ戻す | b1_owner_descendants_reset_and_carry_to_capital |
| B2 | books/src/reports/tax.rs：i128の整数除算で0へ向かって切り捨てる。負側の説明コメントとdocも変更 | b2_purchase_and_full_refund_cancel_transitional_deduction |

store/src/inbox.rs、partners.rs、drafts.rs、db_event.rsのSQLを名前つき定数または複数行文字列にしました。
明細・meta・イベントのINSERTは列名を明記し、パラメータの対応を読めるようにしました。
4ファイルの最大行長はそれぞれ98文字で、100文字を超える行はありません。

## 実行したコマンドと結果

- 最初に再現試験を追加し、未修正で `cargo test -p kamoku-import -p kamoku-books --test review_fixes --no-fail-fast`：7件失敗（全て指摘の再現）。
- 未修正で `cargo test -p kamoku-store --test review_fixes`：8件失敗、2件成功（許可metaと日付の往復は正常例）。
- 修正後の同コマンド：import/books 7件成功、store 10件成功。
- 読みやすさの整理前と整理後の `cargo test --workspace`：どちらも151件成功、37試験群（0件のdoc-testsを含む）。既存の `duplicates_and_force` も無変更で成功。
- `cargo test -p kamoku-import --test review_fixes -- --nocapture`：5件成功。10万列指定・64KiBセルの拒否20.542µs、1000万個のカンマの拒否2.630166ms（この環境のdebug実行）。

最終実行（全てexit=0）：

```text
cargo fmt --all -- --check                                             0.23秒
cargo clippy --workspace --all-targets --all-features -- -D warnings     1.12秒
cargo test --workspace                                                10.33秒
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps               0.48秒
```

途中の失敗も修正しました。ClippyはSQL constの位置4箇所（items_after_statements）、試験のmatch（manual_let_else）、空配列確認3箇所（assert_is_empty）を順に指摘しました。
最初の個別Clippyでも試験のerr().expect()を指摘され、巨大な失敗出力を避けるlet-elseへ直しました。
`cargo deny`、`cargo +1.88` は指示どおり実行していません。
ログは `/tmp/kamoku-r2a-gate-{fmt,clippy,test,doc}.log`、終了コードは `/tmp/kamoku-r2a-gates.json` です。

## 修正を外した赤と復元の確認

修正後の各ファイルを一時退避し、修正前のファイルに戻して、該当する試験だけを実行しました。
全12指摘で `cargo test -p <crate> --test review_fixes <prefix>` がexit=101、コンパイルエラーではなく期待値の失敗となりました。
毎回finallyで修正済みファイルを戻し、`cmp <実ファイル> <退避ファイル>` がexit=0であることを確認しました。

| 対象 | 試験prefix | 復元確認 |
|---|---|---|
| S1・S2・S4・S5・S6・S7 | s1_ / s2_ / s4_ / s5_ / s6_ / s7_ | 各1ファイル、全てcmp=0 |
| S3 | s3_ | db_ledger.rsとdb_event.rsを組で戻し、両方cmp=0 |
| I1 | i1_ | mapping.rs、cmp=0 |
| I2・I4 | i2_ / i4_ | 同じcsv.rsをそれぞれ修正前に戻し、個別に試験。各回cmp=0 |
| B1・B2 | b1_ / b2_ | reports/mod.rs、reports/tax.rs、各cmp=0 |

ログは `/tmp/kamoku-r2a-mutation-{S1,S2,S3,S4,S5,S6,S7,I1,I2,I4,B1,B2}.log` です。
I1は未修正で数GBを確保しないよう、空のセルで列数拒否の試験を先に実行して失敗させています。

## 仕様・既存試験の変更点、疑問と制約

- 既存試験の変更はimport/tests/unit.rsのcsv_edgesだけです。`"x"  ,"y"z` を受理する期待を、2行目のInvalidQuoteに変更しました。仕様・tools/make-fixtures.py・合成CSVは変更していません。fixtures試験9件は通過し、寛容さに依存する合成CSVは見つかりませんでした。
- ImportErrorは元からnon_exhaustiveのため、InvalidQuote・TooManyColumns・TooManyCellsを追加しました。全てcodeはinvalid_csv、lineはSomeです。既存の公開関数の名前・引数・戻り値は変更していません（元ファイルとの宣言比較でも確認）。
- S4の不正metaだけのケースは、S3修正後には接頭復元が成功します。再読込自体のErrも確認するため、別ケースでledger_meta表欠落を重ねました。どちらも未保存の仕訳を公開せず、保存失敗のエラーを保持します。
- B2の負側のfloorと、S7の既存親フォルダの権限は仕様本文の文字どおりの解釈と異なります。今回の明示指示を優先しました。仕様の修正は担当範囲外のため行っていません。判断待ちの疑問はありません。
- 最初の3クレート一括試験は依存cc 1.8.0のsrc/target/4ファイル欠落でコンパイルに失敗しました。指定キャッシュを変更せず、取得済みアーカイブから一時コピーに復元し、バイト一致を確認しました。ネットワーク・依存追加はありません。
- 品質チェックのCARGO_HOMEは `/tmp/kamoku-r2a-cargo-4uyn1rfv` です。CARGO_NET_OFFLINE、CARGO_TARGET_DIR、TMPDIRは指定どおり。元の `$W/.cargo-home` の欠落は残っているため、同じ検証を再実行する場合はこの一時コピーを使うか、ccの欠落を復旧してください。
- 時間計測はこの環境の観測値です。メモリ使用量の計測は行っていません。上限の追加前にセル・摘要を増やさない処理と境界試験で確認しました。独立エージェントのレビューは実施していません。
