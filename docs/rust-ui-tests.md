# 以前の画面試験をRustと実ブラウザへ移す

移行前の単体試験は `ui/tests/` の168件、実サーバーの操作試験は447件でした。関数の内部をJavaScriptから直接呼んでいた試験は、Rustの純モデルか、同じ部品を使う実際の画面へ移します。条件を消したり、期待する振る舞いを移植後の都合へ変えたりはしません。

## 純ロジックの対応

| 以前の単体試験 | 現在のRust試験 |
|---|---|
| `input-parse` | `input::tests`。金額式、巨大な中間整数、誤り、日付、年/月末、うるう年 |
| `kana`、`combobox` | `choices::tests`。幅・かな、安定順位、完全一致、新規取引先、上限、上下配置 |
| `ime` | `interaction::tests`。変換中、229、50ms境界、Enter連打、監視解除・再登録 |
| `account-defaults`、`entry-options`、`entry-hints` | `entry_model::legacy_tests`。既定値追従、明示値保持、口座の親、締め、割合、日付の注意 |
| `entry-return` | `journal_return_path::tests`。条件・元帳への復帰、不正な外部URLの拒否 |
| `import-map` | `imports_model::tests`。列の検査、推測・保存値、見本、ファイルの行番号、合計 |
| `inbox-helpers` | `inbox_model::tests`。候補、税区分、選択・順序、部分失敗、2000件の送信境界 |
| `period`、`report-tree` | `report_period::tests`、`report_tree::tests`。暦・期間、親子の合計、単一子、月列 |
| `chart` | `home_chart_model::tests`。目盛り、欠けた月、未来、赤字の額と重なり |
| `checks`、`opening-form` | `settings_model::tests`、`setup_model::tests`、`master_model::tests`。検査結果、期首、インボイス番号 |
| `phrase` | `phrase::tests`。改行位置、送り仮名、英数字、画面の補足文、元の文への復元 |
| `toast`の表示期限 | `notification::tests`。4秒・10秒・明示時間、エラー、hoverとfocusの独立性 |

金額や日付の見た目は、各画面のRust試験に加え、大きな金額、帳票、登録、取込のブラウザ試験で確かめます。ブラウザに無いNaN等を整数のモデルへ持ち込む代わりに、JSON境界と誤りの表示を確認します。

## DOMと操作の対応

| 以前の単体試験 | 実際の画面で確かめる条件 |
|---|---|
| `fields` | `shell.spec.mjs` AC-10.2/10.6。必須の読み上げ名、キーの属性、誤りの領域、TextとAX IDの保持 |
| `keys` | `shell.spec.mjs`、`ime.spec.mjs`。1文字キーの切替と永続化、文字入力とIMEの抑止 |
| `toast`の配置・監視 | `shell.spec.mjs`、`rust-common.spec.mjs`、各画面。帯の寸法・非表示・差替え、監視解除、焦点、ダイアログの前面 |
| 表の横スクロール | `shell.spec.mjs` AC-11.6。実際のRust表を広げて縮め、Tabの止まり場所が追従 |
| `phrase`のDOM | `inbox-confirm.spec.mjs`と帳票。`.phrase .nw`と意味のある注意文 |

`shell.spec.mjs` の3つのJavaScript部品インポートは、登録フォーム・確認ダイアログ・勘定科目の実DOMを使う試験へ置き換えています。モック部品の見た目だけが通る状態を避け、Rustのイベントと再描画を通します。

## 実行

```sh
cargo test -p kamoku-ui --locked
cargo clippy -p kamoku-ui --target wasm32-unknown-unknown --all-targets --locked -- -D warnings
node --test ui/tests/
e2e/run-real.sh
```

実行の成功、画像を見た評価、実機での確認は別々に記録します。[試験の手順](testing-procedure.md)と[確認範囲](testing.md)も参照してください。
