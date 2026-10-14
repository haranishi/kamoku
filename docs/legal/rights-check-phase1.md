# 権利の確認記録（Phase 1・計算の部品）: 足した依存は許可リストに収まり、配るときの作業が1つ増える

Phase 1 の計算の部品（`kamoku-books` と `kamoku-import`）で足した依存と、参照した公開資料の記録です（確認日はすべて2026-10-14）。結論は3つあります。足した依存のライセンスは、どれも `deny.toml` の許可リストに収まり、`cargo deny check` が通りました。バイナリで配るときは、`encoding_rs` に含まれるWHATWG由来のデータの著作権表示を同梱する作業が増えます。標準の勘定科目の名前は、国税庁の様式と突き合わせ、違う箇所を下に書きました。弁護士・弁理士・税理士の確認は受けていません。全体の方針は[ライセンス方針](license-policy.md)、Phase 0 の記録は[権利の確認記録](rights-check.md)にあります。

## 足した依存と、そのライセンス

| crate | 版 | ライセンス | 使う場所 |
|---|---|---|---|
| `encoding_rs` | 0.8.42 | (Apache-2.0 OR MIT) AND BSD-3-Clause | `kamoku-import`（Shift_JIS・UTF-16を文字にする） |
| `scopeguard` / `simdutf8` / `multiversion_no_op` | 1.2.0 / 0.1.5 / 1.0.0 | MIT OR Apache-2.0 | `encoding_rs` の依存 |
| `rustversion` | 1.0.23 | MIT OR Apache-2.0 | `encoding_rs` のビルド時だけ |
| `core_detect` | 1.0.0 | MIT/Apache-2.0 | `encoding_rs` の依存（x86だけ。以前から Cargo.lock にあった） |
| `serde` / `serde_core` / `serde_derive` | 1.0.229 | MIT OR Apache-2.0 | 帳票・入力・列の対応をJSONにする |
| `proc-macro2` / `quote` / `syn` | 1.0.107 / 1.0.47 / 3.0.9 | MIT OR Apache-2.0 | `serde_derive` の依存（コンパイル時だけ） |
| `unicode-ident` | 1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 | 同上 |
| `serde_json` | 1.0.154 | MIT OR Apache-2.0 | 試験だけ（JSONの形の比較） |
| `itoa` / `zmij` / `memchr` | 1.0.18 / 1.0.23 / 2.8.3 | MIT OR Apache-2.0 / MIT / Unlicense OR MIT | `serde_json` の依存（試験だけ） |
| `rusqlite` / `libsqlite3-sys` | 0.40.2 / 0.38.2 | MIT | `kamoku-store`（保存層。bundled）。同梱SQLiteはpublic domain（取得済みREADMEの表記を確認） |
| `hashlink` / `hashbrown` / `smallvec` | 0.12.2 / 0.17.1 / 1.16.3 | MIT OR Apache-2.0 | rusqliteの依存 |
| `foldhash` | 0.2.0 | Zlib | hashbrownの依存。既存許可リストにZlibあり |
| `fallible-iterator` / `fallible-streaming-iterator` | 0.3.0 / 0.1.9 | MIT OR Apache-2.0 | rusqliteの依存 |
| `cc` / `find-msvc-tools` / `shlex` / `pkg-config` / `vcpkg` | 1.8.0 / 0.1.14 / 2.0.1 / 0.3.34 / 0.2.15 | MIT OR Apache-2.0 | libsqlite3-sysのビルド時。`kamoku-store` は依存を追加せず、取得済みメタデータで確認した |
| `serde_json` / `itoa` / `zmij` / `memchr` | 1.0.154 / 1.0.18 / 1.0.23 / 2.8.3 | MIT OR Apache-2.0 / MIT / Unlicense OR MIT | `kamoku-store` は設定・テンプレート・列対応・履歴にも使う（本番）。新たな依存は足していない |
| `getrandom` / `cfg-if` | 0.4.3 / 1.0.5 | MIT OR Apache-2.0 | `kamoku-app`（合言葉とセッション値の乱数）。Linuxでは `libc` 0.2.190（MIT OR Apache-2.0）、UEFIだけ `r-efi` 6.0.0（MIT OR Apache-2.0 OR LGPL-2.1-or-later。MIT か Apache-2.0 を選ぶ） |
| `time`（formatting・parsing・macros・std） / `time-macros` | 0.3.55 / 0.2.32 | MIT OR Apache-2.0 | `kamoku-app`（日付と「今日」）。版は Phase 0 と同じで、機能を足しただけ |
| `serde` / `serde_json` / `encoding_rs` | 1.0.229 / 1.0.154 / 0.8.42 | 上の行と同じ | `kamoku-app`（JSONの応答、CSVの Shift_JIS の書き出し）。新しい crate は増えない |
| `tempfile` / `fastrand` / `rustix` | 3.27.0 / 2.5.0 / 1.1.5 | MIT OR Apache-2.0 / Apache-2.0 OR MIT / Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | `kamoku-app` の試験だけ（`kamoku-store` と同じ版） |
| `tiny_http`（使わない） | 0.12.0 | MIT OR Apache-2.0 | 最初の設計の受け口。巨大な `Content-Length` の要求1つで異常終了することを実測したため（2026-10-14）、標準ライブラリだけの受け口を自作した |
| `sha2`（`kamoku-app` でも使う） | 0.10.9 | MIT OR Apache-2.0 | `kamoku-app`（再送防止の表に、本文の代わりに SHA-256 を置く。2026-10-14 のレビューの修正）。台帳と取込が使う版と同じで、新しいクレートは増えない |
| `rusqlite`（`kamoku-app` は試験だけ） | 0.40.2 | MIT | `kamoku-app` の試験だけ（同じ帳簿を別の接続で開き、保存の失敗を起こす）。保存層と同じ版で、新しいクレートは増えない |

`sha2`（0.10.9）と `time`（0.3.55）は Phase 0 から台帳が使っていて、`kamoku-import` でも使います。cargo-deny 0.20.2 の結果は `advisories ok, bans ok, licenses ok, sources ok` でした。

`encoding_rs` は、最小のRust（1.88）で動く最新の0.8.42にしました。0.8.35 なら依存は `cfg-if` だけですが、修正を取り込める新しい版を優先しました。そのぶん増えた4つの crate も、上の表のとおり許可リストのライセンスです。

## 配るときは、WHATWG の著作権表示を同梱する

`encoding_rs` の `COPYRIGHT` によると、本体は Apache-2.0 か MIT で、WHATWG Encoding Standard のデータに由来する部分は BSD-3-Clause（同梱の `LICENSE-WHATWG`）です。BSD-3-Clause の第2項は、バイナリで再配布するとき、著作権表示・条件・免責の文を、文書かほかの資料に含めることを求めます。`deny.toml` の許可リストに BSD-3-Clause が入っていることは確かめました。

配布の手順に足す作業は、第三者のライセンスの一覧を作って同梱することです。MIT と Apache-2.0 の依存も許諾文の同梱が条件なので、同じ一覧にまとめます。いまはバイナリを配っていないため、この作業はまだしていません。

## 勘定科目の名前は、決算書の行名とほぼ一致した

突き合わせに使ったのは、次の2つです。

- 所得税青色申告決算書（一般用）【令和５年分以降用】（PDF）: <https://www.nta.go.jp/taxes/shiraberu/shinkoku/yoshiki/01/shinkokusho/pdf/r05/10.pdf>。様式の一覧は <https://www.nta.go.jp/taxes/shiraberu/shinkoku/yoshiki/01/shinkokusho/r06.htm>
- 令和7年分 青色申告決算書（一般用）の書き方: <https://www.nta.go.jp/taxes/shiraberu/shinkoku/tebiki/2025/pdf/037.pdf>。行の番号（⑧租税公課、⑱減価償却費など）が、上の様式と同じことを確かめた

資産・負債・資本の科目は、貸借対照表の科目名と一致しました。経費は、外注費（㉑）を除いて、損益計算書の⑧〜㉔と㉛の行名と一致しました。違いは次のとおりで、どれも `form_line`（決算書の行名）で行を示しています。

- 外注費は、行名が「外注工賃」（㉑）
- 仕入高は「仕入金額（製品製造原価）」（③）、売上高・雑収入・家事消費は「売上（収入）金額（雑収入を含む）」（①）。家事消費は、2ページの「家事消費等」の欄も通る
- 普通預金・その他の預金は「その他の預金」。工具器具備品は、様式では「工具 器具 備品」と空けて書かれている
- 会議費・新聞図書費・諸会費・車両費・支払手数料には決まった行が無く、㉕〜㉚の空欄に科目名を書いて使う（`form_line` は「（空欄）」）

使ったのは項目名という事実だけで、様式の図や説明の文章は写していません。出典のURLはこのページに書き、`docs/accounts.md` からこのページへリンクしています。

## 読み取りの規則は、公開の仕様から書いた

CSVの読み取りは、RFC 4180（<https://www.rfc-editor.org/rfc/rfc4180>）の引用符・二重の引用符・改行の規則に従いました。違うのは1点で、引用符の外の行末の空白を落とします（仕様の決まり）。RFC は「空白は欄の一部」としています。

文字コードは WHATWG Encoding Standard（<https://encoding.spec.whatwg.org/>、2026-05-21更新の版を閲覧）を確かめました。Shift_JIS の表は IBM と NEC の拡張を含むので、Windows-31J の「①」も読めます。0x5C はバックスラッシュ（U+005C）として読まれるため、金額の欄では円記号として扱います。

元号の日付は、e-Gov 法令APIで2つの政令を確かめて、期間の外の日を読まないようにしました。令和は平成31年政令第143号で2019-05-01から、平成は昭和64年政令第1号（1月7日公布、翌日施行）で1989-01-08からです。

ほかの会計OSSやCSVのライブラリのコードは見ていません。仕様だけから書きました。試験のCSVは `tools/make-fixtures.py` が作る合成データで、店名には「サンプル」「デモ」を含めています。

## 確認できていないこと

1. 弁護士・弁理士・税理士による確認
2. 令和7年分以降の決算書の様式のPDFそのもの。現行の様式の一覧のページ（`02.htm`）は2026-10-14に「表示できません」となり、行番号は書き方の資料でしか確かめていない
3. 標準の科目の既定の税区分が、ふつうの使い方に合うか（迷った科目は、作業記録の `spec-issues.md` に書いた。Issueでも受け付ける）
4. 配布するときの、第三者のライセンスの一覧
