# F1 のメモ（他の担当への依頼と、共通の部品の変更）

## 第2回（2026-10-15）で足した依頼と変更

### 他の担当への依頼（F2）

1. **ホーム（幅390）: 10億円を超える金額で、ページが横に広がる。** 作業コピーのサーバーに API で大きな金額（売上 1,234,567,890円、
   経費 1,500,000,000円・987,654,321円）を足すと、数字のカードの「2026年の利益 -1,249,321,553円」の `span.money` の右端が403pxになり、
   ページが 403>390 になる（AC-11.1・AC-11.6）。見立て: 2つ並びのカードの中で、`.money`（nowrap）が24pxの文字のまま入りきらない。
   帳票では、1億円以上（11字以上）の数字があるときだけ1列に並べる印（`rt-figures--long`）で直した。同じやり方か、数字の大きさを枠の幅で下げる。
2. **確定のダイアログ（幅390）: 説明の文の最後が「…変えられま／せん。」と2文字だけ次の行に落ちる**（inbox-bar.js の文）。
   この Chromium では `text-wrap: pretty` は日本語の2文字の取り残しを直さず、`balance` は「取／消」と語の途中で割る（実測）。
   帳票で使った `js/components/phrase.js` の `phrase(text)`（文を塊に分け、nowrap の span と `<wbr>` で並べる）を、
   ダイアログや補足の長い文に使える。例: `h('p', null, phrase('確定すると、…変えられません。'))`。
3. **`--bar-bottom` が、幅767px以下で iPhone の下の余白を含むようになった**（`calc(var(--bottomnav-h) + env(safe-area-inset-bottom, 0px))`）。
   未確認の固定バー（`bottom: var(--bar-bottom)`）は、そのままで下のバーの真上に載る（CDP で余白34pxを擬似して確認。試験は shell.spec の AC-11.8）。
   使う側で `env(safe-area-inset-bottom)` を足し直さないこと。`screens-inbox-bar.css` の `scroll-padding-bottom: calc(var(--bottomnav-h) + 200px)` は
   余白を含まないので、`calc(var(--bar-bottom) + 200px)` にするとそろう。取引を登録の保存の帯も `bottom: var(--bar-bottom)` だけでよい。
4. **ダークの `--c-border-strong` を #667587 → #6D7C8E に明るくした。** 未確認の確定候補の行（藍の淡い地）の中の入力欄の枠が、
   2.75:1 で 3:1 に足りなかったため（check-contrast に組を足した）。ダークの入力欄・ボタン・チェックボックスの枠が少し明るくなる。

### 共通の部品の変更（両方の担当に関係する）

- 新規 `js/components/phrase.js`: `phrase(text)` と `chunks(text)`。`.nw`（nowrap）を components.css に足した。
- `banner()` の `title` と `body` に、Node の並び（`phrase()` の結果）を渡せる。本文のすぐ後に置く `extra` を足した。
- 帳票の表（report-table.js）のカードへの切り替えは、枠の幅の線（560px・1100px）をやめ、表の形で測って入りきらないときだけにした
  （`fitToWidth`。枠に `.rt-cards` と `data-fit`）。帳票の外の表（`createTable` の `table--stack`）は変えていない。

## 第1回（2026-10-14）の依頼と変更

## 他の担当への依頼（F2）

1. **表の中の選択欄の右の余白。** 標準の選択欄（`.select`）を `appearance: none` にし、矢印を背景（右から10px、16px角）に描くようにした。中の文字が矢印に重ならないよう、`.select` は右に36pxを空けている。画面のCSSで `padding` を詰めているところは、右を28px以上残してほしい。
   - `screens-inbox.css` の `.inbox-table .select { padding: 0 var(--sp-2) }` → 例: `padding: 0 28px 0 var(--sp-2)`
   - `screens-import.css` の `.enc-dd .select`（`height: 32px` も、幅1023px以下では44px が要る。AC-11.7）
2. **「取引を登録」を枠だけのボタンに。** ホーム（`home.js` の page-head）と仕訳帳（`journal.js` の page-head）の `btn btn--primary` を `btn` にしてほしい（塗りの主ボタンは、各画面の主な操作に1つだけ。ホームでは「未確認を開く」）。F1の依頼書の P1-7 の項目だが、ファイルがF2の担当なので、F1は触っていない。
3. **押せる大きさの土台が変わった。** `--control-h` は幅1023px以下で44px（前は767px以下だけ）。画面のCSSで高さを固定しているところ（表の中の 36px、取込の文字コードの 32px など）は、幅1023px以下で44pxにそろえる必要がある。`.btn--sm`、`.icon-btn`、`.seg-option`、`.tab`、`.check`（包むラベル）は、共通のCSSで幅1023px以下を44pxにした。
4. **チェックボックスの見た目を共通で変えた。** `input[type="checkbox"]` を自作の見た目（18px、未選択は `--c-border-strong` の枠だけ、選ぶと藍の塗りに白抜きの印）にした。未確認の `.cand-check` は、包むラベルの幅が32pxなので、44px にしてほしい（AC-11.7 の赤の元）。
5. **補足の文（`.field-hint`）を 12px → 13px にし、`text-wrap: pretty` を足した。** 入力欄の幅に合わせて狭くなっている補足（設定・初期設定の「してくだ／さい。」）は、`.field` の外の幅で決まるので、画面のCSSで補足の幅を広げてほしい。
6. **確定ダイアログの「件数・出金の合計・入金の合計」の行は、`components-overlay.css` の `.dialog-body dl > div { align-items: baseline }` でベースラインがそろうようになった。** `screens-inbox-bar.css` の側の変更は要らない。

## 統合する人への依頼（担当の決まっていないファイル）

- **デモの帯の文（`js/main.js`）。** 帯は、共通のCSSで無彩色の1行にした。幅639px以下は見出し（「デモのデータです」）だけを見せ、本文は読み上げにだけ残している（幅390で高さ40px以下にするため）。本文を「本物の帳簿には書き込みません。」のように短くすれば、狭い幅でも本文まで1行に入る。

## 共通の部品の変更（両方の担当に関係する）

- `.visually-hidden` に `text-overflow: clip` を足した。ファイルの欄（`input[type="file"]`）は、ブラウザの既定で `text-overflow: ellipsis` になり、見えない1pxの欄が AC-11.6 の「…で欠けた文字」に数えられていた（取込の画面の赤の元）。見えない要素に「…」は出ないので、試験は弱めていない。
- ダイアログの幕を濃くした（`--c-backdrop`: ライト 0.45→0.6、ダーク 0.6→0.72）。開いたときに最初に置くフォーカス（`autofocus`）は、マウスで開いても輪を出す。
- 通知（`.toasts`）の下の位置は `--bar-bottom` を使う。
- 下のバーの文字は 11px → 12px。下のバーに無い画面にいる間は、「メニュー」に `aria-current="true"` が付く（`[aria-current]` で選んだ表示）。
