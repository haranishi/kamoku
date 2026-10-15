# F2 のメモ（他の担当への依頼と、統合する人への注意）— 第2回 2026-10-15

## F1への依頼（共通の部品に原因があるもの）

1. **通知（`.toasts`）が、下に固定した帯に重なる。** 画面: 取引を登録（幅1279px以下）と未確認（全幅）。幅390で「保存して続ける」の直後、
   通知（高さ約56px）が `bottom: calc(var(--bar-bottom) + 12px)` に出て、保存の帯（下のバーの上、高さ約61px）のボタンを数秒覆う。
   未確認の下のバー（高さ約110px）でも、確定の直後に同じ。見立て: `components-overlay.css` の通知の位置が、画面の固定の帯を知らない。
   案: 帯のある画面では帯の上に出す（例: `:root:has(.entry-actions-card)` と `:root:has(.inbox-bar:not([hidden]))` のときに、通知の bottom に帯の高さを足す。
   帯の高さは `--sticky-bar-h` のような変数を、F2 の画面の CSS で入れる形でもよい）。F2 は共通の部品を触っていない。
2. **補足文の語の途中の折り返し（帳票の画面も）。** `text-wrap: pretty`（`.field-hint`）では、日本語の語の途中の折り返し（「反映していま／せん」「してくだ／さい」）は止まらない。
   F2 は `js/screens/ja-wrap.js` の `ja(text)`（文を、ひらがなの語の終わり・句読点・閉じかっこで「かたまり」に分け、`.ja-chunk { display: inline-block }`
   で、かたまりの間だけで折る）を作って、帳票以外の補足文に使った。帳票の注意の帯（消費税の「参考値…反映していません」）や表の下の説明にも使えるので、
   F1 で使う場合は `js/components/` に移すとよい（`field()` の hint は Node を受けるので、`hint: ja('…')` で使える）。
3. **`check-contrast.mjs` の組**（前回からの依頼。今回 F1 が直す予定と聞いた）: `--c-muted / --c-ok-bg`、`--c-muted / --c-primary-soft`。

## 統合する人への注意

- **F1 の `phrase()` と、F2 の `ja()`（`js/screens/ja-wrap.js`）は同じ目的**（日本語の文を、語の途中で折らないかたまりに分ける）。F2 のコピーには
  `phrase.js` が無いので、F2 は自分の `ja()` を補足文・説明の文に使った。統合のときは、どちらか一方にそろえてほしい（`ja()` の区切り方と試験は
  `ui/tests/ja-wrap.test.mjs`。かたまりは `.ja-chunk { display: inline-block }`＝行より長いかたまりだけは中で折れるので、はみ出さない）。
  確定ダイアログの「…変えられま／せん。」は、連絡どおり F2 は触っていない（`inbox-bar.js` の `confirmCandidates`）。
- **`--bar-bottom` が下の余白（セーフエリア）を含む前提に合わせた**（F1 の第2回）。F2 の CSS は `env(safe-area-inset-bottom)` を足し直さない:
  未確認のスクロールの余白は `calc(var(--bar-bottom) + 200px)`、本文の下の余白の上書きは `var(--bar-bottom)` だけ。取引を登録の帯も `bottom: var(--bar-bottom)` だけ。
  F2 のコピーの `tokens.css` は F1 の第2回より前なので、ここでは余白0の Chromium で確かめた（iPhone の実機は確かめていない）。

- **`index.html` は変えていない**（新しい CSS のファイルは無い。新しい JS は `js/screens/ja-wrap.js` と `js/screens/inbox-confirm.js`（未確認の確定の流れを inbox.js から分けた）で、どちらも画面の JS が import する）。
- **本文の下の余白を、画面の CSS で上書きしている所がある。** `.main:has(> .entry-layout .entry-actions-card)`（幅1279px以下）と
  `.main:has(.inbox-bar:not([hidden]))` で、`padding-bottom` を `var(--bar-bottom)`（下のバーの高さ。768px以上は0）にした（下まで送ったとき、
  固定の帯と下のバーの間に本文の余白が見えたため）。`.main` の余白（`layout*.css`）を F1 が変えるときは、この2か所も合わせて見てほしい。
- **試験の数の変化。** 新しい試験のファイル: `e2e/inbox-confirm.spec.mjs`（9件）、`e2e/entry-layout.spec.mjs`（5件）、`e2e/big-amounts.spec.mjs`（3件。設計担当の連絡と同じ金額 -1,249,321,553円 の1件を含む）、
  `ui/tests/ja-wrap.test.mjs`（2件）。`ui/tests/inbox-helpers.test.mjs` に6件足した。意図して直した試験: `e2e/inbox.spec.mjs`（日付の期待に曜日）、
  `e2e/screens.spec.mjs`（取引を登録の入力の待ち方を「表が見える」→「表ができる」。幅390では表を畳むため）。
- **未確認の表の列の幅**（1080px以上の入れ物）: 税区分 148→160px、日付 96→104px、メモ 116→108px。表の幅は 1,110px のまま（幅1440で枠に収まる）。
