// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 標準の勘定科目。名前と決算書の行名は、国税庁の「所得税青色申告決算書（一般用）」
//! 【令和5年分以降用】の項目名と突き合わせた（docs/legal/rights-check-phase1.md）。

use crate::{Account, Kind, TaxCode};
use kamoku_ledger::AccountCode;
use std::fmt::Write as _;

/// 固定の行が無い経費の、決算書の行名。
pub const BLANK_FORM_LINE: &str = "（空欄）";
const SALES_LINE: &str = "売上（収入）金額（雑収入を含む）";

// (コード, 名前, 読み, 有効か, 既定の税区分, 決算書の行名)
type Row = (
    &'static str,
    &'static str,
    &'static str,
    bool,
    Option<TaxCode>,
    &'static str,
);
const ON: bool = true;
const OFF: bool = false;
const P10: Option<TaxCode> = Some(TaxCode::P10);
const NT: Option<TaxCode> = Some(TaxCode::NT);

#[rustfmt::skip]
const ASSETS: [Row; 17] = [
    ("cash", "現金", "げんきん", ON, None, "現金"),
    ("dep_current", "当座預金", "とうざよきん", OFF, None, "当座預金"),
    ("dep_fixed", "定期預金", "ていきよきん", OFF, None, "定期預金"),
    ("dep_other", "普通預金・その他の預金", "ふつうよきんそのたのよきん", ON, None, "その他の預金"),
    ("notes_recv", "受取手形", "うけとりてがた", OFF, None, "受取手形"),
    ("ar", "売掛金", "うりかけきん", ON, None, "売掛金"),
    ("securities", "有価証券", "ゆうかしょうけん", OFF, None, "有価証券"),
    ("inventory", "棚卸資産", "たなおろししさん", ON, None, "棚卸資産"),
    ("prepaid", "前払金", "まえばらいきん", ON, None, "前払金"),
    ("loan_recv", "貸付金", "かしつけきん", OFF, None, "貸付金"),
    ("bldg", "建物", "たてもの", OFF, None, "建物"),
    ("bldg_fix", "建物附属設備", "たてものふぞくせつび", OFF, None, "建物附属設備"),
    ("mach", "機械装置", "きかいそうち", OFF, None, "機械装置"),
    ("vehicle", "車両運搬具", "しゃりょううんぱんぐ", OFF, None, "車両運搬具"),
    ("tools", "工具器具備品", "こうぐきぐびひん", ON, None, "工具器具備品"),
    ("land", "土地", "とち", OFF, None, "土地"),
    ("owner_draw", "事業主貸", "じぎょうぬしかし", ON, None, "事業主貸"),
];
#[rustfmt::skip]
const LIABILITIES: [Row; 8] = [
    ("notes_pay", "支払手形", "しはらいてがた", OFF, None, "支払手形"),
    ("ap", "買掛金", "かいかけきん", ON, None, "買掛金"),
    ("loan_pay", "借入金", "かりいれきん", ON, None, "借入金"),
    ("accrued", "未払金", "みばらいきん", ON, None, "未払金"),
    ("adv_recv", "前受金", "まえうけきん", ON, None, "前受金"),
    ("deposit_recv", "預り金", "あずかりきん", ON, None, "預り金"),
    ("bad_debt_res", "貸倒引当金", "かしだおれひきあてきん", OFF, None, "貸倒引当金"),
    ("owner_inv", "事業主借", "じぎょうぬしかり", ON, None, "事業主借"),
];
const EQUITY: [Row; 1] = [("capital", "元入金", "もといれきん", ON, None, "元入金")];
#[rustfmt::skip]
const REVENUE: [Row; 3] = [
    ("sales", "売上高", "うりあげだか", ON, Some(TaxCode::S10), SALES_LINE),
    ("misc_income", "雑収入", "ざっしゅうにゅう", ON, None, SALES_LINE),
    ("home_use", "家事消費", "かじしょうひ", OFF, None, SALES_LINE),
];
#[rustfmt::skip]
const EXPENSES: [Row; 24] = [
    ("exp_purchase", "仕入高", "しいれだか", ON, P10, "仕入金額（製品製造原価）"),
    ("exp_tax", "租税公課", "そぜいこうか", ON, None, "租税公課"),
    ("exp_freight", "荷造運賃", "にづくりうんちん", ON, P10, "荷造運賃"),
    ("exp_utility", "水道光熱費", "すいどうこうねつひ", ON, P10, "水道光熱費"),
    ("exp_travel", "旅費交通費", "りょひこうつうひ", ON, None, "旅費交通費"),
    ("exp_comm", "通信費", "つうしんひ", ON, P10, "通信費"),
    ("exp_ads", "広告宣伝費", "こうこくせんでんひ", ON, P10, "広告宣伝費"),
    ("exp_entertain", "接待交際費", "せったいこうさいひ", ON, P10, "接待交際費"),
    ("exp_insurance", "損害保険料", "そんがいほけんりょう", ON, NT, "損害保険料"),
    ("exp_repair", "修繕費", "しゅうぜんひ", ON, P10, "修繕費"),
    ("exp_supplies", "消耗品費", "しょうもうひんひ", ON, P10, "消耗品費"),
    ("exp_depr", "減価償却費", "げんかしょうきゃくひ", ON, None, "減価償却費"),
    ("exp_welfare", "福利厚生費", "ふくりこうせいひ", ON, None, "福利厚生費"),
    ("exp_salary", "給料賃金", "きゅうりょうちんぎん", ON, None, "給料賃金"),
    ("exp_outsource", "外注費", "がいちゅうひ", ON, P10, "外注工賃"),
    ("exp_interest", "利子割引料", "りしわりびきりょう", ON, NT, "利子割引料"),
    ("exp_rent", "地代家賃", "ちだいやちん", ON, None, "地代家賃"),
    ("exp_baddebt", "貸倒金", "かしだおれきん", ON, None, "貸倒金"),
    ("exp_misc", "雑費", "ざっぴ", ON, None, "雑費"),
    ("exp_meeting", "会議費", "かいぎひ", ON, P10, BLANK_FORM_LINE),
    ("exp_books", "新聞図書費", "しんぶんとしょひ", ON, None, BLANK_FORM_LINE),
    ("exp_dues", "諸会費", "しょかいひ", ON, None, BLANK_FORM_LINE),
    ("exp_vehicle", "車両費", "しゃりょうひ", ON, P10, BLANK_FORM_LINE),
    ("exp_fee", "支払手数料", "しはらいてすうりょう", ON, P10, BLANK_FORM_LINE),
];

fn groups() -> [(Kind, u32, &'static [Row]); 5] {
    [
        (Kind::Asset, 100, &ASSETS),
        (Kind::Liability, 200, &LIABILITIES),
        (Kind::Equity, 300, &EQUITY),
        (Kind::Revenue, 400, &REVENUE),
        (Kind::Expense, 500, &EXPENSES),
    ]
}

/// 標準の科目（53個）。並び順は分類ごとに100・200・300・400・500から10刻み。
/// 既定の割合はすべて100。親と口座は持たない（口座は利用者が足す）。
#[must_use]
pub fn standard_accounts() -> Vec<Account> {
    let mut out = Vec::new();
    for (kind, base, rows) in groups() {
        for (step, &(code, name, reading, enabled, default_tax, form_line)) in
            (0u32..).zip(rows.iter())
        {
            // コードは定数で、どれも AccountCode の規則に合う（試験で数を確かめている）。
            let Ok(code) = AccountCode::new(code) else {
                continue;
            };
            out.push(Account {
                code,
                name: name.to_owned(),
                reading: reading.to_owned(),
                kind,
                parent: None,
                wallet: None,
                default_tax,
                default_pct: 100,
                form_line: Some(form_line.to_owned()),
                enabled,
                system: true,
                sort: base + step * 10,
            });
        }
    }
    out
}

/// 標準の科目の一覧を、Markdownの文書にして返します（`docs/accounts.md` の中身）。
#[must_use]
pub fn standard_accounts_markdown() -> String {
    let mut out = String::from(
        "# 標準の勘定科目\n\n\
         kamokuが最初に用意する勘定科目の一覧です。名前は、国税庁の青色申告決算書（一般用）の項目名に合わせました。\
         決算書の行名と違う科目（外注費など）は、「決算書の行」の欄に行名を書いています。\
         突き合わせの記録は[権利の確認記録（Phase 1）](legal/rights-check-phase1.md)にあります。\n\n\
         このファイルは手で直しません。`crates/kamoku-books/src/standard.rs` を直してから、\
         `cargo run -p kamoku-books --example accounts_doc > docs/accounts.md` で作り直します。\
         中身とコードがずれると、`kamoku-books` の試験が落ちます。\n\n\
         「既定の税区分」と「既定の割合」は、科目を選んだときに入る初期値です。勘定科目の画面で変えられます。\
         状態が「無効」の科目は、使うときに有効にします。\n\n\
         | コード | 名前 | 読み | 分類 | 既定の税区分 | 既定の割合 | 決算書の行 | 状態 |\n\
         |---|---|---|---|---|---|---|---|\n",
    );
    for a in standard_accounts() {
        let tax = a
            .default_tax
            .map_or_else(|| "なし".to_owned(), |t| format!("{t}（{}）", t.label()));
        let _ = writeln!(
            out,
            "| `{}` | {} | {} | {} | {} | {}% | {} | {} |",
            a.code,
            a.name,
            a.reading,
            a.kind.label(),
            tax,
            a.default_pct,
            a.form_line.as_deref().unwrap_or(""),
            if a.enabled { "有効" } else { "無効" },
        );
    }
    out.push_str(
        "\n口座（現金・預金・クレジットカード・電子マネー）は、利用者が初期設定で足します。\
         口座の親の科目は、現金と電子マネーが `cash`、預金が `dep_other`、カードが `accrued` です。\n\n\
         「（空欄）」は、決算書に決まった行が無い経費です。決算書では、空いている行に科目名を書いて使います。\n",
    );
    out
}
