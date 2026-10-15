
## 2026-10-15 調査・設計
- 共通ハーネスとSSDルールを既読。依存導入・重いビルドなし、指定コピーだけを変更。Git操作、ブラウザ/e2e実行なし。既存Phase 0契約は今回のUI作業に適用せず、今回の依頼を範囲とする。
- ja()全呼び出しを個別確認。enterkeyhintのnext/doneだけは文字列へ戻す。それ以外はhの子、field hint、replaceChildren。
- phraseを単一spanに統一。14字上限、空白の後の境界、送り仮名・指示語・閉じ括弧後の助詞を保護。両部品の試験を統合。
- 通知表示中だけバーの位置を追跡。下端の座標と高さからliftを計算し、通常配置の帯は対象外。高さ変化・画面遷移・小窓・スクロールにも追従し、通知がなくなれば追跡解除。

## 実装・検証（第1周）
- 18画面ファイルでja()をphrase()へ50か所置換。entry-simple-fields.jsのenterkeyhint 2か所をnext/doneへ復元。他に文字列を必要とするja()なし。
- phrase()は単一span.phrase。その中にspan.nwとwbr。F1の4試験を保持し、F2の全CASESと空白・長文・空文字の試験を移行。F2の20字上限は統一方針に合わせて14字に強化。wholeの完全一致は維持。
- 初回phrase試験は5/6成功、開業日（または、の分割で1件失敗。短い名前に続くひらがな主体の括弧補足を結ぶ規則で修正し、6/6成功。
- 幅390のコード上の幅確認: main左右16pxで本文358px、card左右16px+枠で324px。14字×16px=224px、補足13pxなら182px。取込drop-zoneは左右16pxで326px、import-sourceは左右16px・18pxアイコン・8px間隔で300px。設定/初期設定の補足は狭幅1列。期首表のsubは12pxで、負債・カードの(9字=108px)と未払い額(4字)に分かれる。目視は未実施。
- 確定・訂正の確認文、20字を超えるdialogエラーとinbox保存失敗文にphraseを適用。
- 2つの帯にdata-bottom-bar。toastは位置・寸法・下端への接着を判定、ResizeObserver/MutationObserver/resize/scrollで追従。最終通知消去時に全監視・イベント・予約フレームを解除。小窓中と通常配置の保存帯はlift=0。
- 通知単体試験2件を追加し成功。e2eは指定2ファイルへ幅390/1440の計4試験を追加（未実行）。入力属性・確定文の塊も検査。
- 指定の単体107/107成功、方針111ファイル違反なし、コントラスト72組合格。
- 仕上げ確認でdialogのphrase import漏れ、閉じ括弧の境界条件への誤ったENDING条件追加を発見し修正。閉じ括弧の回帰assertを追加。残ったimport-sampleのja-chunkをphrase(v)に移行し、長い見本値も14字で折り返す。修正後の検査を次に記録する。

## 最終検証（第2周）
- node --test ui/tests/: 107件成功、失敗0、skip0。
- node ui/dev/check-policy.mjs: 111ファイル、違反0。
- node ui/dev/check-contrast.mjs: ライト/ダーク72組合格。
- 追加の構文確認: 変更JS/MJS 29ファイルでnode --check成功（e2eの実行ではない）。phrase import監査は最初bannerの型コメントをコードと誤認して失敗し、コメントを除外する監査へ修正して成功。実装変更なし。旧ja-wrap/ja-chunk参照なし。
- ja()置換50件、属性復元2件、import-sampleの手書きja-chunkはphrase(v)へ移行。F1の既存試験は変更せず、閉じ括弧境界assertを追加。F2は全CASES・whole完全一致を維持し、上限を20→14へ強化。
- ブラウザ目視・e2e未実行。発注者は追加4件を実行し、390幅の設定/初期設定/取込/確認文、390/1440幅の保存・確定直後の通知、通知中の帯の高さ変化・画面移動・小窓開閉も目視する。1440の保存帯は通常配置なのでlift=0。

### 変更ファイル（削除も含む）
- ui/css/components-overlay.css
- ui/css/screens-settings.css
- ui/css/screens.css
- ui/js/components/banner.js
- ui/js/components/dialog.js
- ui/js/components/phrase.js
- ui/js/components/toast.js
- ui/js/screens/accounts.js
- ui/js/screens/entry-correct.js
- ui/js/screens/entry-savebar.js
- ui/js/screens/entry-simple-fields.js
- ui/js/screens/entry-templates.js
- ui/js/screens/home-parts.js
- ui/js/screens/import-check.js
- ui/js/screens/import-columns.js
- ui/js/screens/import-file.js
- ui/js/screens/import-sample.js
- ui/js/screens/import.js
- ui/js/screens/inbox-bar.js
- ui/js/screens/inbox-row.js
- ui/js/screens/ja-wrap.js
- ui/js/screens/journal-detail.js
- ui/js/screens/master-forms.js
- ui/js/screens/opening-form.js
- ui/js/screens/partners.js
- ui/js/screens/settings-data.js
- ui/js/screens/settings.js
- ui/js/screens/setup.js
- ui/js/screens/wallets.js
- ui/tests/ja-wrap.test.mjs
- ui/tests/phrase.test.mjs
- ui/tests/toast.test.mjs
- e2e/entry-layout.spec.mjs
- e2e/inbox-confirm.spec.mjs
- .agent-harness/runs/2026-10-14-phase1/ui-fix-xfix-notes.md
