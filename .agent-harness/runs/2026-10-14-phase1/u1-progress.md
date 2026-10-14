# U1 最終報告: 画面の土台、ホーム・登録・仕訳帳

2026-10-14。依頼書の「終えたと言える条件」はすべて満たした。試験と検査は、何も立っていない状態から通した（下の数字）。git の commit と push はしていない。触ったのは `ui/` `e2e/` とこのファイル、`spec-issues.md` の追記だけ。

## 実行したコマンドと結果

| コマンド | 結果 |
|---|---|
| `node --test ui/tests/` | 55件すべて通過 |
| `node ui/dev/check-contrast.mjs` | 64組すべて基準を満たす（ライトとダーク） |
| `node ui/dev/check-policy.mjs` | 64ファイル、違反0件 |
| `cd e2e && npm ci && npx playwright test` | 70件すべて通過（操作46件、スクリーンショット24枚） |

試験が門番として働くかは、誤りを1つずつ入れて確かめた（16か所。単体6、e2e 10）。最初の1回は、元に戻す控えを作れず誤りが5つ残った（作業用の場所に、別の担当の同名のディレクトリがあったため）。手で戻し、試験で確かめてから、`cmp` で戻せたことを毎回確かめる手順でやり直した。見た目の誤り1つ（狭い幅で効かないCSS）は試験が通ってしまったが、これは誤りの入れ方の問題で、効く形に変えると赤くなった。

## 19:00 の設計変更の反映（項目ごと）

1. IMEの判定の順序: 反映。`js/ime.js` の `classifyKey`（composing / swallow / repeat / normal）と `shouldIgnoreKey`。(a)(b)(c) は `ui/tests/ime.test.mjs` に別々の試験。`<form>` の送信は常に止め、保存はキー処理とボタンからだけ。
2. 通信の変更: (a) `client_id` を付け、成功したら作り直す。(b)(c) 訂正と取消に `expected_revision`。409 `stale_revision` と `stale` は「他の場所で更新されました。読み込み直します」と出し、最新を読み直して、入力は残したまま差分を見せる。(d) 相手科目が口座なら税区分と割合の欄を隠し、対象外・100で送る。(e) P10X / P8X のとき、日付の下に課税仕入れの日の案内。(f) 模擬サーバーも、`client_id` の再送に同じ結果、版の不一致に409を返す。すべて反映し、e2e で確かめた。
3. combobox の選んだ後の動き: 反映。`onSelect(item, { via, event })` が `'stop'` を返すと、そのEnterは外へ伝わらない。返さなければ、外の処理（「次の欄へ」「確定候補にして次の行へ」）に伝わる。`arrowOpens: false` で、閉じているときの上下の矢印を外へ回せる（未確認の表で、隣の行へ移るため）。

## 作ったもの

- 土台: `index.html`、`css/`（14ファイル。色は `tokens.css` だけ）、`js/` の dom・format・input-parse・kana・ime・keys・theme・theme-boot・icons・api・state・router・routes・shell・main・account-defaults。
- 部品 `js/components/`: 依頼書の14部品に、date-input と amount-input を足した。使い方は `ui/README.md` の表と、下の「U2へ」。
- 画面: ホーム（home, home-parts）、登録と訂正（entry と entry-* の8ファイル）、仕訳帳と詳細（journal, journal-filters, journal-detail）、準備中（placeholder）。
- 模擬サーバー `dev/mock-server.mjs` と `dev/mock/`（12か月・76仕訳・口座3・取引先5）。検査 `dev/check-contrast.mjs` `dev/check-policy.mjs`。
- 試験: `ui/tests/` 7ファイル、`e2e/` 8ファイル（entry, entry-rules, ime, journal, home, shell, layout, screens）。

## 目で見つけて直した見た目

サイドバーの背景が最初の画面の高さで切れる／広い画面に効かない「広げる」ボタンが出る／細いサイドバーの下の名前が2行／プレビューに「null」の文字（`replaceChildren` に null を渡していた。同じ書き方を全部直した）／プレビューの科目名が細い列で割れる（左右並び→1行ずつ）／スマホでプレビューが保存ボタンの下（入力→プレビュー→保存の順に）／768pxで仕訳帳の金額が表の外（入れ物の幅で3行のカードに）／スマホの見出しが3段／前月比の矢印が2行の間に浮く／消費税の日付や説明が「す。」「ん。」だけ次の行／口座の説明の折れ／引き出しの×に輪が出る／スマホで引き出しが画面いっぱいにならない（ブラウザの既定の最大幅）／詳細の明細の表がはみ出す／検索欄の案内文が切れる／スマホのタブとボタンが44px未満。

## 仕様からずれた点

- 依頼書に無いファイルを足した: `ui/package.json`（`"type": "module"` だけ）、`ui/tests/index.js`（Node 22以降は `node --test <ディレクトリ>` を展開しないため）、`js/theme-boot.js`（最初の描画の前に明暗を当てる）、CSSの分割、部品2つ。
- 模擬サーバーの仕訳は76件（約60の指定より多い。経費の棒を見える大きさにするため）。
- 疑問と解釈は `spec-issues.md` の U1 の1〜16。主なもの: 履歴の `kind` の値、免税で隠れた税区分に既定値を入れるか、画面下のバーの5項目、「保存」の後の行き先。

## U2へ（部品の使い方と罠）

- 画面: `export function mount(root, ctx)`。`ctx` は `params` `query` `api` `state` `toast` `keys` `navigate` `router.buildHash`。片付けの関数に `update(ctx)` を生やすと、同じ画面のまま条件が変わったときに呼ばれる。`routes.js` の placeholder の行を差し替える。
- 候補欄: `createCombobox({ items: [{ value, label, reading, hint }], value, allowFree, arrowOpens, onSelect, onInput })` → `{ el, input, value, item, setItems, setValue, open, close, focus, destroy }`。例: `onSelect: (item, info) => { if (info.via === 'enter' && item) { markRow(); focusNextRow(); return 'stop'; } }`。
- 表: `createTable({ caption, columns: [{ key, label, num, render, className, stackLabel }], rows, rowKey, onOpen, stack, rowClass })` → `{ el, setRows, focusRow, focusedIndex, rowAt, length }`。
- ダイアログ: `openModal({ title, body, actions: [{ label, variant, autofocus, onClick: (c) => c.close() }], onClose })` → `{ close, setBusy, setError, setBody, setTitle }`。`confirmDialog({ title, message, confirmLabel, danger })` は真偽を返す。引き出しは `openDrawer({ title, body, side, onClose })`。
- そのほか: `field({ label, control, labelFor, hint, required })` → `{ el, setError, setHint, setLabel }`、`createDateInput` / `createAmountInput`（`{ el, value, setValue, commit }`）、`createSegmented`、`createTabs`、`toast.success(msg, { action: { label, href } })`、`banner` `emptyState` `delayedSkeleton` `createChart`、`icon(name)`。
- 既定値: 科目を変えたときは `account-defaults.js` の `nextDefaults({ prev, next, tax, pct })`（api-inbox.md の決まり）。キー処理の入口は必ず `shouldIgnoreKey(e)`。
- 通信: `api.get/post/put/patch/del`。失敗は `ApiError`（`isNetwork` `isStale` `isReadOnly` `field`）。共有の読み込みは `state.load('settings', { force: true })` など。設定に左右される画面は、開くたびに force で読み直す（使い回すと、ほかの画面で変えた設定が反映されない。実際に試験で落ちた）。
- 罠: `el.replaceChildren(a, null)` は「null」という文字を出す（`dom.js` の `replace()` を使う）。`.icon-btn` の `display` は後から読むCSSが勝つ。`<dialog>` にはブラウザの既定の最大幅がある。試験で `page.goto` がハッシュだけ変えると読み込み直さない（画面の中の読み込み済みの値が残る）。
- 模擬サーバーを広げるときは `dev/mock/routes.mjs` の `ROUTES` に足す。未確認の行は `store.db.drafts`（`seed.mjs` の `makeDrafts`）にある。

## 既知の弱点

- 自動の試験は Chromium だけ。Safari・Firefox・Windows の変換は、人が確かめていない。50msの窓は経験値。
- 本物のサーバーに付け替えていない。`KAMOKU_URL` と `KAMOKU_TOKEN` の口は作ったが、未検証。
- 免税のとき、隠れた税区分にも科目の既定の税区分が入る（spec-issues の4）。
- 速さの目標（画面の操作100ms）は測っていない。`text-wrap: pretty`・コンテナクエリ・`:has()` を使うので、古いブラウザでは見た目が少し変わる。
- 押しっぱなしのEnterは、文字の欄の改行の繰り返しも止める（spec-issues の13）。
