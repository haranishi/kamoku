<!-- SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors -->
<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->

# UI レビュー対応 Y（2026-10-15）

## 調査・設計
- 共通 workflow.md・ssd-storage.md とコピー内の契約を読み取り。旧契約は Phase 0 向けであり、今回の直接指示の範囲（ui/・指定 e2e・記録）を優先する。
- このコピーだけを編集。git 操作、依存導入、大きなビルド、ブラウザ・e2e 実行はしない。
- export_all を確認: kamoku/schema:1 の独自 JSON に確定記録・設定・科目・取引先等が入り、未確認明細・テンプレート・取込設定は入らない。事業者名は shell のメニュー上に表示。
- 確定では focusSpot を保存し、最初の await 前に表・絞り込み・ページ送りを inert にする。解除とフォーカス復帰は finally に集約。
- phrase は inline-block と max-width で塊間の区切りを維持し、入れ物より長い塊だけ内部の折り返しを許す。プレビューは名前と金額を別の塊にする。
- 月次の名前は表示月に今月より後がある場合だけ変更。月次 e2e は API の月・年間値、科目別値との比較を追加する。
- 指示された対象外4件と元帳の10桁の限界は実装変更しない。限界は spec-issues.md に追記する。

## 実装・試験コード
- settings-data.js: 独自形式の JSON と明記し、控えは丸ごと開き直せること・JSON の対象と対象外・他ソフト利用には変換が必要なことを説明。事業者名の補足と理由コメントを実表示に合わせた。
- inbox-confirm.js: focusSpot の保存後、try 内の最初に freeze(true)。finally は常に sending の解除・freeze(false)、画面が残っていればフォーカス復帰。
- e2e/inbox-confirm.spec.mjs: PATCH の到着を gate で検知して応答を保持。保存待ち中の inert/aria-busy、実際のクリックによる候補変更の阻止、キャンセル後の3領域の解除・元のメモ欄への復帰・再操作を確認する試験を追加。以前のコードでは保存待ち中の aria-busy/inert の検査で失敗する構成。
- components.css・phrase.js・screens-settings.css: .nw の説明と実装を揃え、枠内の塊は維持し、枠を超える塊だけ折る。使用箇所（帳票の札・結論のラベル・banner の本文・補足）はインラインの文要素であり、行や表の構造は変えない。実描画は未確認。
- entry-preview.js・screens-entry.css: 名前を専用の span にし、名前と金額の塊・側全体を max-width:100% に制限して折り返す。名前の keep/overflow-wrap:anywhere を保つ。
- report-views.js: 表示月に今月より後が含まれる場合だけ表の名前を変更。
- e2e/reports.spec.mjs: 月別 API 応答から表示すべき月、売上・経費・純利益、年間合計を比較。科目別の月・年の値と区分合計も API から照合。全ゼロ化の不具合は、来月の費用と API 年間合計の比較で失敗する。従来の合計比較・振替の検査も保持。
- e2e/screens.spec.mjs: 幅640px以上の表の可視検査を復元。640px未満は要約の可視、開いた表の可視、再び畳んだ要約の可視を確認して撮影時の状態を戻す。
- 大きな金額の試験の名前とコメントに元帳の除外範囲を明記。spec-issues.md に10桁以上で列が隠れる限界を追記。

## 検証結果・引き継ぎ
- `node --test ui/tests/`: exit 0、107件合格、失敗・スキップ0。単体試験の既存ファイルは変更していない。
- `node ui/dev/check-policy.mjs`: exit 0、111ファイル、違反なし。
- `node ui/dev/check-contrast.mjs`: exit 0、ライト・ダークの72組すべて基準を満たす。
- 変更した3つの e2e と inbox-confirm.js・entry-preview.js・report-views.js の `node --check`: exit 0。最後の試験コード調整後も reports.spec.mjs の構文検査は合格。
- DOM の代用品を使い createConfirmFlow を直接実行: flush の呼出し時に3領域が inert、aria-busy=true。flush が例外を返した後に3領域の inert と aria-busy が解除され、元の memo 列へフォーカス復帰することを assert で確認。ブラウザの実操作検証ではない。
- 科目別 API 比較は、子を含む親の合計に加え、親への直接の記帳の行は API のその科目自身の値と比較する。実装の treeRows/monthColumns を試験側で使わず、API から期待値を作る。
- 意図した表示変更: 月次推移の名前は将来月の有無に応じて変える。そのため既存の名前のセレクターは両方を許し、来月の試験では API から判定した名前を完全一致で検査する。可視や金額の検査は削除せず強化した。
- 未実行: ブラウザでの目視・e2e 全件。発注者は inbox-confirm.spec.mjs、reports.spec.mjs、screens.spec.mjs を実行し、保存待ちからキャンセルまでの inert とフォーカス、幅390pxの長い科目/口座名・大きな金額、文字200%の帳票内・banner・補足の折り返しを確認する。撮影試験は表を一度開いて可視を確かめてから畳む。
- 元帳は幅640〜1023pxで10桁以上が既知の限界。実装は変更しない。
- 変更しなかった4件: 幅変更直後の隠れた欄へのフォーカス（操作中の幅変更はまれ）、別画面で無視した候補の残存（複数画面同時利用に限る）、13桁で数字が約11px（極端な金額）、未対応ブラウザのコンテナクエリによる並べ替え差（公開時点の最新版が対象）。
- 禁止ディレクトリ・禁止 e2e ファイル・対象外4件の実装には書き込んでいない。git操作なし。
