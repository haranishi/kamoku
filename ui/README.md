# kamoku の画面（ui/）

ブラウザで開く画面です。ビルドの工程も依存も無く、ファイルを直して再読込すれば確かめられます。サーバーとは `/api/v1` のJSONだけでやりとりします（約束は `docs/spec/api-*.md`）。

初めての人は「起動」から「画面を足す」までを通読してください。部品の表と決まりの節は、必要なときに引けば足ります。

## 起動は、本物のデモか模擬サーバー

画面を直しながら本物のサーバーで確かめるときは、`--ui-dir` で手元のファイルを読ませます。起動のたびに合成データ（205仕訳・未確認25件）に戻り、合言葉つきのアドレスが標準出力に出ます。

```
cargo run -p kamoku-app -- demo --no-open --ui-dir ui
```

模擬サーバーは Node だけで動き、試験を速く回すためと、本物では作りにくい状態を作るために残しています。Node 24 で確かめています（Node 22以降なら動く想定）。

```
node ui/dev/mock-server.mjs --port 8766
# ブラウザで http://127.0.0.1:8766/ を開く
```

`--today 2026-10-14`（今日の固定）、`--tax-mode exempt`（免税の設定）、`--empty`（仕訳なし）、`--fresh`（初期設定の前の帳簿）、`--broken`（読み取り専用）、`--demo`（デモの帯）、`--delay 400`（遅い応え。骨組みの表示を見るため）を付けられます。合言葉とCookieの認証は省いています。応えの形が本物と食い違っていないかは、契約試験（`e2e/contract.test.mjs`）が見張っています。

## 試験は4つのコマンドで通す

```
node --test ui/tests/             # 部品の単体試験（金額の式、日付、IMEの判定、絞り込み、帳票の期間と親子の行、取込の列、検査の門番）
node ui/dev/check-contrast.mjs    # tokens.css の色の組を計算する（AC-11.2）
node ui/dev/check-policy.mjs      # 下の「守る決まり」をソースの文字列で確かめる
cd e2e && npm ci && ./run-real.sh   # 本物のサーバーで、通し試験・契約試験・スクリーンショット
```

`./run-real.sh` は、先に `cargo build -p kamoku-app` で作った実行ファイルを使います（`KAMOKU_BIN` で場所を変えられます）。`npx playwright test` だけで流すと、e2e は模擬サーバーを4つ立てます（操作用の8766、スクリーンショット用の8767、初期設定の試験用の8768と、その撮影用の8769。後の2つは `--fresh`）。スクリーンショットはリポジトリ直下の `screens-out/` に出ます。ブラウザは新しく取得せず、手元の Playwright の Chromium か、`KAMOKU_E2E_CHANNEL=chrome` で入っている Chrome を使います。

手でサーバーを立てて試すときは、`KAMOKU_URL` と `KAMOKU_TOKEN`（起動時の合言葉）を入れます。初期設定の試験には、空の帳簿のサーバーが別に要ります（`kamoku serve --data <空のフォルダ> --no-open`）。試験には `KAMOKU_SETUP_URL` と `KAMOKU_SETUP_TOKEN`、撮影には `KAMOKU_SETUP_SHOTS_URL` と `KAMOKU_SETUP_SHOTS_TOKEN` で渡します。試験は仕訳や口座を足していくので、スクリーンショットの前にデモを起動し直してください。

## 画面を足す: screens/ に1ファイル置き、routes.js に1行足す

画面は `export function mount(root, ctx)` を持ち、片付けの関数を返します。`ctx` には `params` `query` `api` `state` `toast` `keys` `navigate` が入ります。

- 本文の最初に `<h1 tabindex="-1">` を置く。画面を替えたとき、ルーターがここへフォーカスを移す（読み上げが画面の名前から始まる）。
- 絞り込みや引き出しの開閉のように、同じ画面のまま条件だけ変わるときは、片付けの関数に `update(ctx)` を生やす。ルーターは作り直さずにそれを呼ぶので、入力中の値とフォーカスが残る。
- 画面だけの1文字のキーは `ctx.keys.register({ j: fn })` で登録し、片付けで外す。文字入力の欄や変換中は、登録しても働かない。
- 状態はアドレスの `?` に持つ。戻る操作で前の状態に戻れるようにするため。

`screens/placeholder.js` は、どの画面にも当たらないアドレスの画面として残っています。

## 部品は components/ から選ぶ

| したいこと | 部品 |
|---|---|
| ラベル・欄の下のエラー・説明を付けた入力欄 | `fields.js` の `field()` `textInput()` `selectInput()` `button()` |
| 科目・口座・取引先のように、打った文字で候補を絞る | `combobox.js`（`onSelect` が `'stop'` を返すと、そのEnterは外へ伝わらない） |
| 日付と金額 | `date-input.js` `amount-input.js` |
| 種別のような二択・三択 | `segmented.js`（標準のラジオボタン） |
| 一覧の表、行を開く | `table.js`（640px未満の積み上げ表示つき） |
| 確認・入力のダイアログ、右の引き出し | `dialog.js` の `openModal()` `confirmDialog()`、`drawer.js` |
| 保存の知らせ、失敗の知らせ | `toast.js`（失敗は消えない） |
| 読み取り専用やデモの帯、空の状態、骨組み | `banner.js` `empty.js` `skeleton.js` |
| 月ごとの推移のグラフ | `chart.js`（同じ数字の表に切り替えられる） |
| 書き出しのリンク（`<a href download>`） | `download.js` の `enhanceDownload()`（Shift_JISで「?」にした文字の数と、失敗の理由を知らせる） |

アイコンは `icons.js` の `icon(name)` です。足すときは、20×20の格子に線の太さ1.75で描きます。

## キー処理は、最初に shouldIgnoreKey を呼ぶ

日本語入力の変換を決めるEnterで、保存や候補の選択が動いてはいけません。判定は `js/ime.js` の `shouldIgnoreKey(event)` だけにあり、真ならそのキーは何もしません（変換中、変換の直後50ms以内のEnter、押しっぱなしのEnter）。自分で `isComposing` や `keyCode` を見ると、check-policy が落とします。`<form>` の送信は常に止め、保存はキー処理かボタンからだけ呼びます。

## 守る決まりと、破ると起きること

- `innerHTML` などでHTMLを組み立てる → 摘要や取引先の文字が、HTMLとして動きうる。要素は `dom.js` の `h()` で作る。
- `style="..."` や `setAttribute('style', …)` を使う → CSP（`style-src 'self'`）で止まり、見た目が崩れる。寸法は `el.style.setProperty()` で入れる。
- `fetch` を `api.js` の外で呼ぶ、外のURLを書く → 「外へ通信しない」約束が崩れる。
- 部品のCSSに16進の色を書く → コントラストの検査を通らない色が混ざる。色は `css/tokens.css` の変数だけ。
- 赤（`--c-danger`）を費用や赤字に使う → 訂正・取消・失敗の警告と見分けがつかなくなる。赤字は「赤字」の文字と符号で示す。

決まりの根拠は `docs/spec/nfr.md` と `docs/ux-five-layers.md` にあります。仕様の疑問は、Issueに書いてください。
