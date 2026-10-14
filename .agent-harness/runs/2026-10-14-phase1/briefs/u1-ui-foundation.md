# 依頼U1: 画面の土台と、ホーム・取引の登録・仕訳帳

先に `00-common.md` を読んでください（Rust向けの記述は、画面では読み替えます。`unwrap` の禁止などは対象外で、代わりにこの文書の「守る決まり」が効きます）。あなたの担当は、ブラウザで動く画面の土台と、最初の3画面です。サーバーは別の担当が並行して作ります。そのため、あなたは**仕様どおりに応える模擬サーバー**を使って作ります。

- 触る場所: `ui/`、`e2e/`、`.agent-harness/runs/2026-10-14-phase1/u1-progress.md`。これ以外は触らない（`.gitignore` は済み）。
- 読むもの（必須）: `docs/spec/` の全ファイル（特に `screens-main.md` `screens-other.md` `keyboard.md` `nfr.md` `api-*.md`）と、`docs/ux-five-layers.md`。
- 次の担当（U2）が、同じ土台の上に、未確認・取込・帳票・登録簿・設定・初期設定の画面を足します。**部品を再利用しやすく作ってください。**

## 守る決まり（画面の「品質の門」）

1. ビルド工程なし。素のHTML・CSS・JavaScript（ES modules）。外部ライブラリ、CDN、Webフォントを使わない。npmの依存は `e2e/` の開発専用のものだけ（`@playwright/test` を `1.62.1` に固定）。
2. CSP（`default-src 'self'` ほか。`docs/spec/api-common.md`）で動くこと。**インラインの`<script>` `<style>`、`style="..."` の属性、`setAttribute('style', ...)` を使わない。** 動的な寸法は CSSOM（`el.style.width = ...`、`el.style.setProperty('--x', ...)`）か、SVGの属性で。
3. `innerHTML` `outerHTML` `insertAdjacentHTML` `document.write` `eval` `new Function` を使わない。DOMは `dom.js` の `h()` で作る。
4. 外のURL（`http://` `https://`）を、コードに書かない。`fetch` は `api.js` からだけ。
5. 色は `css/tokens.css` の変数だけを使う（部品のCSSに16進の色を書かない）。
6. 色だけで意味を伝えない。アイコンは自作のインラインSVG（`js/icons.js`。線の太さ1.75、20px、線の端は丸）。絵文字をアイコンにしない。
7. 文言は、やさしい日本語。AIらしい決まり文句を使わない。エラーは「原因と直し方」。
8. 1つのファイルは300行を目安に。部品は `js/components/` に1ファイル1部品。
9. 試験の名前かコメントに `AC-xx.y` を書く。

## ファイル構成

```
ui/
  index.html  favicon.svg  README.md（100行以内。起動、構成、画面と部品の足し方）
  css/ tokens.css base.css layout.css components.css screens.css
  js/  main.js router.js api.js state.js dom.js format.js input-parse.js kana.js
       ime.js keys.js theme.js icons.js
       components/  combobox.js table.js dialog.js drawer.js toast.js tabs.js
                    segmented.js fields.js chart.js command-palette.js shortcuts-help.js
                    empty.js skeleton.js banner.js
       screens/     home.js entry.js entry-journal.js journal.js journal-detail.js
                    placeholder.js（U2の画面が来るまでの「準備中」）
  tests/*.test.mjs      # node --test（部品の単体試験）
  dev/mock-server.mjs   # 一時の模擬サーバー（本物ができたら捨てる）
       check-contrast.mjs  check-policy.mjs
e2e/ package.json playwright.config.mjs  *.spec.mjs
```

画面は `export function mount(root, ctx)` を持ち、片付けの関数を返します。ルーターは `#/` 以降を切り替え、画面を替えたら本文の見出し（`<h1 tabindex="-1">`）へフォーカスを移し、`document.title` を更新します。

## 色・文字・間隔（`tokens.css`）

意味ごとの変数名で持ちます（`--c-bg` `--c-surface` `--c-surface-2` `--c-text` `--c-muted` `--c-border` `--c-border-strong` `--c-primary` `--c-on-primary` `--c-primary-soft` `--c-income` `--c-warn` `--c-danger` と、状態の背景 `--c-pending-bg` `--c-ok-bg` `--c-danger-bg`）。次の値は、コントラストを計算して確かめた値です。この値から始めてください。

| 変数 | ライト | ダーク |
|---|---|---|
| bg / surface / surface-2 | #F6F5F1 / #FFFFFF / #FAF9F6 | #10151B / #18202A / #1D2733 |
| text / muted | #1B2430 / #566170 | #E8ECF1 / #A3AEBC |
| border / border-strong（入力欄の枠） | #D9D5CB / #8A8576 | #2E3A48 / #667587 |
| primary（藍）/ on-primary / primary-soft | #1F4E79 / #FFFFFF / #E4EDF6 | #8DB9E3 / #0B1722 / #1F3347 |
| income / warn / danger（朱） | #17653F / #8A4B08 / #A8301A | #6FCB9A / #E5B15C / #F29A86 |
| pending-bg / ok-bg / danger-bg | #FBEFD5 / #DDF0E4 / #F8DDD6 | #3A2E14 / #173626 / #40201A |

- ダークは `@media (prefers-color-scheme: dark)` の `:root:not([data-theme="light"])` と、`:root[data-theme="dark"]` の両方に同じ値を定義します（手動の切り替え）。
- 赤（朱）は「訂正・取消・失敗」だけに使い、費用の金額には使いません。赤字は「赤字」の文字と符号で示します。
- 書体: `"Hiragino Sans", "Hiragino Kaku Gothic ProN", "Yu Gothic UI", "Meiryo", "Noto Sans CJK JP", system-ui, sans-serif`。本文14px（スマホ幅は16px。入力欄はiOSの拡大を避けて16px）、表は14px、見出し22・18px。数字には `font-variant-numeric: tabular-nums`。
- 間隔は4pxの倍数（4 8 12 16 24 32 48）、角丸 6/10px、影は1段階だけ。動きは150〜200msで `transform` と `opacity` のみ。`prefers-reduced-motion` では止める。`z-index` は 0/10/20/40/100/1000 の表で。
- 密度: `data-density="compact"` で行の高さを40px→32pxに。
- `dev/check-contrast.mjs`: `tokens.css` を読み、次の組を計算する。ライトとダークの両方で、文字（text・muted・primary・income・warn・danger を surface と bg の上に、状態の文字を状態の背景の上に）は4.5:1以上、入力欄の枠（border-strong）とフォーカスの輪（primary）は surface の上で3:1以上。足りなければ非0で終わる。AC-11.2。

## 部品の決まり

- **IME**（`ime.js`。`keyboard.md` の決めごとを1か所に）: `installImeTracking(document, clock = performance.now)` が `compositionstart`/`compositionend` を捕まえて全体の状態を持つ。`isImeConfirm(event)` は、`event.isComposing`、`keyCode === 229`、変換中の状態、`compositionend` から50ms以内のEnter、のどれかなら真。**全画面のキー処理は、この関数が真のときは何もしない。** 偽のイベントを渡す単体試験を書く（時計を差し替えられること）。
- **combobox**: ARIA 1.2のコンボボックス（`role="combobox"` `aria-expanded` `aria-controls` `aria-activedescendant` `aria-autocomplete="list"`、候補は`role="listbox"`/`option`）。`input` と `compositionupdate` の両方で絞る（変換中の文字でも絞る）。矢印で動かし、**Enterで選ぶのは、変換中でなく、候補が開いていて、印のある候補があるときだけ**。Tabは、候補が開いて印があれば選んで次へ。Escで閉じる。blurで、文字がちょうど1つの候補の名前（または読み）に一致すれば選ぶ。候補は `{ value, label, reading, hint }`。絞りは `kana.js`（全角半角、ひらがな/カタカナ、大文字小文字を畳む。名前・読み・コードの部分一致。前方一致を上に）。自由入力を許す指定（取引先用）を持つ。AC-05.1 AC-05.2 の土台。
- **table**: `<table>` `<caption>`（読み上げ用）`<th scope>`、数値の列は右寄せ（`class="num"`）、`aria-sort`、行にフォーカスして`Enter`で開く指定。ページ全体ではなく、表の入れ物だけが横にスクロールする（入れ物に `role="region"` `aria-label` `tabindex="0"`）。幅640px未満では、`data-label` を使った積み上げ表示にできる。
- **dialog / drawer**: 標準の `<dialog>` と `showModal()`（フォーカスの閉じ込めとEscを任せる）。閉じたら開いた要素へフォーカスを戻す。引き出しは右からの`<dialog>`。
- **toast**: `aria-live="polite"` の入れ物。成功は4秒で消え、エラーは `role="alert"` で消さない。フォーカスを奪わない。
- **chart**: 棒（売上と経費）と線（利益）のSVG。経費の棒は斜線のパターンで塗り、色だけに頼らない。`role="img"` と要約の `aria-label`、同じ数字の表を「表で見る」で切り替える。値は`format.js`で整形。
- **command-palette / shortcuts-help**: `keyboard.md` のとおり。移動と主な操作のコマンド、仕訳の文字列検索（`#/journal?q=`へ）。未実装の画面への移動は、準備中の画面に着く。

## 補助の関数（`input-parse.js` `format.js`、単体試験つき）

- `parseAmountExpr(text)`: 全角→半角、カンマ・空白・`円`・`¥` を除き、`+ - * /` と丸括弧の整数の式を**自前の構文解析**で評価（`eval` 禁止）。割り算は切り捨て（`rounded: true` を返し、画面は「切り捨てました」と出す）。結果が1未満、または9,999,999,999,999超は断る。AC-02.3。
- `parseDateInput(text, { today, year })`: `1005` `10/5` `10-5` `2026/10/5` `2026-10-05` `20261005` `t`（今日）`y`（昨日）。全角も可。実在しない日は断る。`addDays(iso, n)`。AC-02.4。
- `formatYen(n)`（「11,000」）、`formatSigned(n)`（負は半角ハイフン）、`formatDate(iso)`（「2026-10-05 (月)」）、`formatPct`。

## 画面

仕様は `screens-main.md` / `keyboard.md` / `api-*.md`。ここでは決めごとだけ足します。

**ホーム `#/`**: 一番上は「次にやること」。0件なら、次の候補（月末の最終日の前後なら帳票の確認）。月の選択は、前後のボタンと選択欄。数字の差には、記号と文字（「前月より 10,800円 多い」）。台帳の状態カードに、整合性の印、ハッシュの先頭8桁、「写し取る」（クリップボード。失敗したら選択状態にして手動のコピーを案内）。AC-07.1 AC-07.4。

**取引の登録 `#/entry`**: `keyboard.md` の並びと動き。種別は3つのボタン（`segmented`）で、`1` `2` `3`。科目の候補は、種別に合う分類を上に（支出は費用、収入は収益と事業主借）。科目を選ぶと、`default_tax` と `default_pct` を入れる（利用者がすでに手で変えた欄は上書きしない）。入力のたびに（150msの待ちの後）`POST /entries/preview` で仕訳を出し、通信が失敗したら保存を止めて理由を出す。金額・日付は、離れたときに整形して表示する。免税事業者（`settings.tax_mode === 'exempt'`）では税区分の欄を隠す（「詳細」で出せる）。保存は `Ctrl/Cmd+Enter` と最後の欄のEnter（変換中を除く）。保存後は、トーストと「仕訳を見る」、種別・日付・口座を残して金額へ戻す。AC-02.1 AC-02.5 AC-03.4。
- 「仕訳で入力」への切り替えで、行を足せる表（貸借・科目・金額・税区分・メモ）。貸借の差を常に出し、0のときだけ保存できる。
- テンプレート: 選ぶと入力欄に反映、「テンプレートに登録」で名前を聞いて保存。
- 訂正 `#/entry/<id>`: 詳細の `input` があれば単純な形、なければ仕訳の表で開く。理由の欄（必須）と、元の値との差分（変わった欄を「変更前→変更後」で）を保存の前に出す。AC-06.4。

**仕訳帳 `#/journal`**: 絞り込みはアドレスの`?`に持つ（戻る操作で復元）。表と、ページ送り（50件）。行は`Enter`で詳細。複合仕訳は「ほか2行」。取消済みの行は文字と記号で示し、絞り込みで隠せる。詳細は右の引き出し（`#/journal/<id>`）。明細の表、取引先、メモ、履歴（時系列。登録・訂正・取消と理由）、「訂正」と「取消」のボタン。取消は理由の入力（必須）と赤系の確認ボタンのダイアログ。`j` `k` `e` `v` `[` `]` を効かせる。

**読み取り専用**: `status.ledger.verified === false` のとき、画面の上に目立つ帯を出し、書き込みの操作（保存・取消・確定）を無効にして理由を出す。

## 模擬サーバー（`ui/dev/mock-server.mjs`）

`node ui/dev/mock-server.mjs --port 8766` で、`ui/` の静的配信と `/api/v1` を返す、Node標準だけの小さなサーバー。認証は省く。起動時に、**合成データ**（架空の名前。固定の疑似乱数）を、12か月分・約60仕訳・口座3つ・取引先5つ作る。必要な応えは `status` `verify` `settings` `accounts` `wallets` `partners` `templates` `entries`（一覧・詳細・preview・作成・訂正・取消）`reports/dashboard`。`preview` と作成の組み立ては、仕様の規則（按分は事業分を四捨五入、残りは事業主貸、税は切捨て）をJSで簡単に再現する。**一時の道具**で、本物のサーバーができたら捨てます。応えの形は `docs/spec/api-*.md` と必ず一致させます（本物に付け替えるときに画面を直さずに済むように）。

## 試験

- `ui/tests/*.test.mjs`（`node --test ui/tests/`）: `parseAmountExpr` `parseDateInput` `kana` `ime`（偽のイベントと偽の時計）`format` `combobox`の絞り込み（DOM不要の部分）。`node ui/dev/check-contrast.mjs` と `node ui/dev/check-policy.mjs`（上の「守る決まり」2〜4を、ソースの文字列検索で確かめる。違反があれば非0）。
- `e2e/`（Playwright。`npm ci` で入れる。ブラウザは新しく取得しない。`~/Library/Caches/ms-playwright` にある物か、入っている Google Chrome（`channel: 'chrome'`）を使う）: 設定の`baseURL`は環境変数 `KAMOKU_URL`（既定は模擬サーバー）。試験:
  - `entry.spec.mjs`: キーボードだけで1件登録（AC-02.1、02.5）。按分でプレビューが変わる（AC-03.4）。金額に「1000+650」（AC-02.3）。日付に「1005」（AC-02.4）。
  - `ime.spec.mjs`: Chromiumで `Input.imeSetComposition` を使い、科目の欄で変換中に Enter を押しても候補が選ばれず、変換を確定してからの Enter で選ばれる（AC-05.2 の部品の確認）。変換中のテキストでも候補が絞られる。文字入力中に `n` `g` が移動を起こさない（AC-10.4）。
  - `shell.spec.mjs`: Ctrl/Cmd+K（AC-10.1）、`?`（AC-10.2）、ライト/ダークの切り替えとフォーカスの輪が見えること（AC-10.3 AC-11.3）、動きを減らす設定（AC-11.4）。
  - `layout.spec.mjs`: 幅390・768・1440で、ホーム・登録・仕訳帳に横スクロールが無い（`document.documentElement.scrollWidth <= clientWidth`。AC-11.1）。
  - `screens.spec.mjs`: 3つの幅 × ライト/ダーク × 3画面のスクリーンショットを、リポジトリ直下の `screens-out/`（gitignore済み）に、`<画面>-<幅>-<テーマ>.png` で保存する（採点に使う）。

## 終えたと言える条件

- `node --test ui/tests/`、`check-contrast`、`check-policy`、`npx playwright test`（`e2e/`）がすべて通る。
- 3つの幅 × ライト/ダークのスクリーンショットを自分で見て（画像を開いて）、崩れ・重なり・読みにくさ・余白の不ぞろい・文字の切れを直した。直したことを報告に書く。
- ホーム・登録・仕訳帳が、模擬サーバーの上で、キーボードだけで一通り動く。
- 最終報告に、U2が使う部品の一覧（名前・引数・使い方の例）と、既知の弱点を書く。
