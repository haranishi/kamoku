// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 組み立てと帳票の性質の試験（proptest）。どんな入力でも成り立つべき等式を確かめる。

mod common;
use common::{chart, settings};
use kamoku_books::{
    JournalInput, JournalLineInput, Kind, Rounding, SimpleInput, SimpleKind, TaxCode,
    account_ledger, balance_sheet, compose_journal, compose_simple, income_statement,
    monthly_series, reconstruct_simple, tax_included, trial_balance,
};
use kamoku_ledger::{Entry, EntryId, Side, Yen};
use proptest::prelude::*;
use time::{Date, Duration, Month};

const COUNTER: [&str; 14] = [
    "exp_comm",
    "exp_supplies",
    "exp_vehicle",
    "exp_outsource",
    "exp_insurance",
    "sales",
    "misc_income",
    "owner_draw",
    "owner_inv",
    "tools",
    "loan_pay",
    "w01",
    "w02",
    "w03",
];
const WALLETS: [&str; 3] = ["w01", "w02", "w03"];
const TAXES: [Option<&str>; 7] = [
    None,
    Some("P10"),
    Some("P8"),
    Some("P10X"),
    Some("P8X"),
    Some("NT"),
    Some("S10"),
];
const JOURNAL: [&str; 9] = [
    "capital",
    "owner_draw",
    "owner_inv",
    "sales",
    "exp_rent",
    "w01",
    "w02",
    "w03",
    "loan_pay",
];
const TEXTS: [&str; 4] = ["", "デモ", "サンプル 商店", "メモ"];

fn day() -> impl Strategy<Value = Date> {
    (2025i32..=2027, 1u16..=365).prop_map(|(y, d)| Date::from_ordinal_date(y, d).unwrap())
}
fn amount() -> impl Strategy<Value = i64> {
    prop_oneof![1i64..=3000, 1i64..=9_999_999_999_999]
}
fn text() -> impl Strategy<Value = String> {
    prop::sample::select(TEXTS.to_vec()).prop_map(str::to_owned)
}
/// 検査に通る形の入力（組み立て直したときと同じ、決まった形）。
fn simple() -> impl Strategy<Value = SimpleInput> {
    (
        (
            0u8..3,
            day(),
            amount(),
            0..COUNTER.len(),
            0..WALLETS.len(),
            1..WALLETS.len(),
        ),
        (0..TAXES.len(), 0u8..=100, text(), text(), text()),
    )
        .prop_map(
            |((kind, date, amount, acc, w, shift), (tax, pct, partner, description, memo))| {
                let wallet = WALLETS[w];
                let mut account = COUNTER[acc];
                if account == wallet {
                    account = COUNTER[(acc + 1) % COUNTER.len()];
                }
                let counter_is_wallet = WALLETS.contains(&account);
                let tax_code = if counter_is_wallet {
                    None
                } else {
                    TAXES[tax].map(str::to_owned)
                };
                let kind = [
                    SimpleKind::Expense,
                    SimpleKind::Income,
                    SimpleKind::Transfer,
                ][usize::from(kind)];
                let transfer = kind == SimpleKind::Transfer;
                SimpleInput {
                    kind,
                    date,
                    amount,
                    account: (!transfer).then(|| account.to_owned()),
                    wallet: wallet.to_owned(),
                    to_wallet: transfer.then(|| WALLETS[(w + shift) % WALLETS.len()].to_owned()),
                    tax_code: if transfer { None } else { tax_code },
                    partner,
                    description,
                    business_pct: (kind == SimpleKind::Expense).then_some(if counter_is_wallet {
                        100
                    } else {
                        pct
                    }),
                    memo,
                }
            },
        )
}
fn journal() -> impl Strategy<Value = JournalInput> {
    (
        day(),
        0..JOURNAL.len(),
        0..JOURNAL.len(),
        0..JOURNAL.len(),
        1i64..=50_000,
        1i64..=50_000,
    )
        .prop_map(|(date, a, b, c, x, y)| {
            let line = |i: usize, side, amount| JournalLineInput {
                account: JOURNAL[i].to_owned(),
                side,
                amount,
                tax_code: None,
                tax_amount: None,
                memo: String::new(),
            };
            JournalInput {
                date,
                description: "仕訳".to_owned(),
                partner: String::new(),
                lines: vec![
                    line(a, Side::Debit, x),
                    line(b, Side::Debit, y),
                    line(c, Side::Credit, x + y),
                ],
            }
        })
}
fn book() -> impl Strategy<Value = Vec<Entry>> {
    (
        prop::collection::vec(simple(), 0..30),
        prop::collection::vec(journal(), 0..8),
    )
        .prop_map(|(simples, journals)| {
            let chart = chart();
            let s = settings(Rounding::Floor);
            let mut out: Vec<Entry> = simples
                .iter()
                .enumerate()
                .map(|(n, i)| {
                    compose_simple(i, &chart, &s)
                        .unwrap()
                        .to_entry(EntryId::new(&format!("S{n}")).unwrap())
                        .unwrap()
                })
                .collect();
            out.extend(journals.iter().enumerate().map(|(n, j)| {
                compose_journal(j, &chart, &s)
                    .unwrap()
                    .to_entry(EntryId::new(&format!("J{n}")).unwrap())
                    .unwrap()
            }));
            out
        })
}
/// 1つの暦年の中の期間。
fn period() -> impl Strategy<Value = (Date, Date)> {
    (2025i32..=2027, 1u16..=365, 0i64..365).prop_map(|(y, d, len)| {
        let from = Date::from_ordinal_date(y, d).unwrap();
        let end = Date::from_ordinal_date(y, 365).unwrap();
        (from, (from + Duration::days(len)).min(end))
    })
}
fn half_up(amount: i64, pct: u8) -> i64 {
    let scaled = i128::from(amount) * i128::from(pct);
    i64::try_from((scaled + 50) / 100).unwrap()
}
fn sum(lines: &[kamoku_ledger::Line], side: Side) -> i128 {
    lines
        .iter()
        .filter(|l| l.side == side)
        .map(|l| i128::from(l.amount.get()))
        .sum()
}

proptest! {
    // AC-03.1 AC-03.2 AC-03.5: 明細は常に貸借が合い、借方合計は支払額。事業分は四捨五入、0円の明細は無い。
    #[test]
    fn composed_lines_balance(i in simple(), r in 0usize..3) {
        let rounding = [Rounding::Floor, Rounding::Round, Rounding::Ceil][r];
        let c = compose_simple(&i, &chart(), &settings(rounding)).unwrap();
        prop_assert_eq!(sum(&c.lines, Side::Debit), i128::from(i.amount));
        prop_assert_eq!(sum(&c.lines, Side::Credit), i128::from(i.amount));
        prop_assert!(c.lines.iter().all(|l| l.amount > Yen::ZERO && l.tax_amount <= l.amount));
        if i.kind == SimpleKind::Expense {
            let pct = i.business_pct.unwrap();
            let business = half_up(i.amount, pct);
            let first = &c.lines[0];
            if business > 0 {
                prop_assert_eq!(first.account.as_str(), i.account.as_deref().unwrap());
                prop_assert_eq!(first.amount.get(), business);
                let expected_tax = i.tax_code.as_deref().and_then(TaxCode::parse)
                    .map(|code| tax_included(code, Yen::new(business), rounding).unwrap());
                prop_assert_eq!(first.tax_code.is_some(), expected_tax.is_some());
                prop_assert_eq!(first.tax_amount, expected_tax.unwrap_or(Yen::ZERO));
            } else {
                prop_assert_eq!(first.account.as_str(), "owner_draw");
                prop_assert!(c.lines.iter().all(|l| l.tax_code.is_none()));
            }
            let private: i64 = c.lines.iter().filter(|l| l.memo.starts_with("家事按分")).map(|l| l.amount.get()).sum();
            prop_assert_eq!(private, i.amount - business);
        }
    }
    // 組み立て直し: 事業分が0円の支出だけ None、それ以外は元の入力に戻る（AC-06.4 の土台）。
    #[test]
    fn reconstruct_inverts_compose(i in simple(), r in 0usize..3) {
        let rounding = [Rounding::Floor, Rounding::Round, Rounding::Ceil][r];
        let chart = chart();
        let c = compose_simple(&i, &chart, &settings(rounding)).unwrap();
        let entry = c.to_entry(EntryId::new("E").unwrap()).unwrap();
        let zero_share = i.kind == SimpleKind::Expense && half_up(i.amount, i.business_pct.unwrap()) == 0;
        let back = reconstruct_simple(&entry, &chart);
        if zero_share { prop_assert_eq!(back, None); } else { prop_assert_eq!(back, Some(i)); }
    }
    // AC-08.1: 試算表の動きの合計と、期末の残高の借方側・貸方側が一致する。期首は1日前の期末と同じ。
    #[test]
    fn trial_balance_balances(entries in book(), (from, to) in period()) {
        let chart = chart();
        let all: Vec<&Entry> = entries.iter().collect();
        let tb = trial_balance(&all, &chart, from, to).unwrap();
        prop_assert!(tb.balanced);
        prop_assert_eq!(tb.total_debit, tb.total_credit);
        let (mut debit_side, mut credit_side) = (0i128, 0i128);
        for row in &tb.rows {
            let side = chart.get(&row.code).unwrap().normal_side();
            let change = match side { Side::Debit => row.debit.get() - row.credit.get(), Side::Credit => row.credit.get() - row.debit.get() };
            prop_assert_eq!(row.closing.get(), row.opening.get() + change);
            match side { Side::Debit => debit_side += i128::from(row.closing.get()), Side::Credit => credit_side += i128::from(row.closing.get()) }
        }
        prop_assert_eq!(debit_side, credit_side);
        // 資産・負債・資本の期末は、同じ日の貸借対照表の自分の分と同じ（同じ仮想の繰越）。
        let bs = balance_sheet(&all, &chart, to).unwrap();
        for row in tb.rows.iter().filter(|r| matches!(r.kind, Kind::Asset | Kind::Liability | Kind::Equity)) {
            prop_assert_eq!(row.closing, own_in(&bs, &row.code), "{}", row.code);
        }
    }
    // AC-08.2: 貸借対照表は、年をまたいでも、どの日でも成り立つ。当期純利益は損益計算書と同じ。
    #[test]
    fn balance_sheet_balances(entries in book(), as_of in day()) {
        let chart = chart();
        let all: Vec<&Entry> = entries.iter().collect();
        let bs = balance_sheet(&all, &chart, as_of).unwrap();
        prop_assert!(bs.balanced);
        prop_assert_eq!(bs.assets.total.get(), bs.liabilities.total.get() + bs.equity.total.get());
        let jan1 = Date::from_ordinal_date(as_of.year(), 1).unwrap();
        let is = income_statement(&all, &chart, jan1, as_of, false).unwrap();
        prop_assert_eq!(is.net_income, bs.net_income_current);
        for section in [&bs.assets, &bs.liabilities, &bs.equity] {
            for row in &section.rows { check_tree(row)?; }
        }
    }
    // AC-08.3: 月別の合計は総計と一致する。12か月の推移も同じ数字。
    #[test]
    fn monthly_sums_match_totals(entries in book(), (from, to) in period()) {
        let chart = chart();
        let all: Vec<&Entry> = entries.iter().collect();
        let is = income_statement(&all, &chart, from, to, true).unwrap();
        for section in [&is.revenue, &is.expense] {
            let mut total = 0i128;
            for row in &section.rows {
                let months: i128 = row.months.unwrap().iter().map(|v| i128::from(v.get())).sum();
                prop_assert_eq!(months, i128::from(row.amount.get()));
                total += i128::from(row.amount.get());
            }
            prop_assert_eq!(total, i128::from(section.total.get()));
        }
        let months = is.months.unwrap();
        prop_assert_eq!(months.len(), 12);
        prop_assert_eq!(months.iter().map(|m| i128::from(m.net_income.get())).sum::<i128>(), i128::from(is.net_income.get()));
        let first = Date::from_calendar_date(from.year(), Month::January, 1).unwrap();
        let last = Date::from_calendar_date(from.year(), Month::December, 31).unwrap();
        let year = income_statement(&all, &chart, first, last, false).unwrap();
        let series = monthly_series(&all, &chart, from.year()).unwrap();
        prop_assert_eq!(series.iter().map(|m| i128::from(m.profit.get())).sum::<i128>(), i128::from(year.net_income.get()));
    }
    // AC-08.4 AC-08.1: 元帳の期首と期末は、試算表と同じ値。残高の推移の末尾が期末。
    #[test]
    fn ledger_matches_trial_balance(entries in book(), (from, to) in period()) {
        let chart = chart();
        let all: Vec<&Entry> = entries.iter().collect();
        let numbered: Vec<(u64, &Entry)> = (1u64..).zip(entries.iter()).collect();
        let tb = trial_balance(&all, &chart, from, to).unwrap();
        for row in &tb.rows {
            let ledger = account_ledger(&numbered, &chart, &row.code, from, to).unwrap();
            prop_assert_eq!(ledger.opening, row.opening, "{}", row.code);
            prop_assert_eq!(ledger.closing, row.closing, "{}", row.code);
            prop_assert_eq!(ledger.rows.last().map_or(ledger.opening, |r| r.balance), ledger.closing);
            let moved: i128 = ledger.rows.iter().map(|r| i128::from(r.debit.get() - r.credit.get())).sum();
            prop_assert_eq!(moved, i128::from(row.debit.get() - row.credit.get()));
        }
    }
}

fn own_in(bs: &kamoku_books::BalanceSheet, code: &str) -> Yen {
    fn find(rows: &[kamoku_books::BsRow], code: &str) -> Option<Yen> {
        rows.iter().find_map(|r| {
            if r.code == code {
                Some(r.own)
            } else {
                find(&r.children, code)
            }
        })
    }
    // 貸借対照表は残高0の行を省くので、見つからなければ0。
    find(&bs.assets.rows, code)
        .or_else(|| find(&bs.liabilities.rows, code))
        .or_else(|| find(&bs.equity.rows, code))
        .unwrap_or(Yen::ZERO)
}
fn check_tree(row: &kamoku_books::BsRow) -> Result<(), TestCaseError> {
    let children: i128 = row
        .children
        .iter()
        .map(|c| i128::from(c.amount.get()))
        .sum();
    prop_assert_eq!(
        i128::from(row.amount.get()),
        i128::from(row.own.get()) + children
    );
    for child in &row.children {
        check_tree(child)?;
    }
    Ok(())
}
