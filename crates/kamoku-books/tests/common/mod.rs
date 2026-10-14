// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 試験で共有する、合成データの科目と入力。名前はすべて架空（「サンプル」「デモ」）。
#![allow(dead_code)]

use kamoku_books::{
    Account, Chart, ComposeSettings, Composed, Kind, Rounding, SimpleInput, SimpleKind, Source,
    TaxMode, Wallet, WalletType, standard_accounts,
};
use kamoku_ledger::{AccountCode, Entry, EntryId};
use time::Date;

pub fn wallet(code: &str, name: &str, kind: WalletType, sort: u32) -> Account {
    Account {
        code: AccountCode::new(code).unwrap(),
        name: name.to_owned(),
        reading: String::new(),
        kind: kind.kind(),
        parent: Some(AccountCode::new(kind.default_parent()).unwrap()),
        wallet: Some(Wallet {
            kind,
            institution: String::new(),
        }),
        default_tax: None,
        default_pct: 100,
        form_line: None,
        enabled: true,
        system: false,
        sort,
    }
}
/// 標準の科目に、口座4つと、既定の割合30%の車両費、無効の科目を足した集合。
pub fn accounts() -> Vec<Account> {
    let mut list = standard_accounts();
    for a in &mut list {
        if a.code.as_str() == "exp_vehicle" {
            a.default_pct = 30;
        }
    }
    list.push(wallet("w01", "現金", WalletType::Cash, 1001));
    list.push(wallet("w02", "サンプルカード", WalletType::Card, 1002));
    list.push(wallet("w03", "デモ銀行", WalletType::Bank, 1003));
    let mut ewallet = wallet("w04", "デモ電子マネー", WalletType::Ewallet, 1004);
    ewallet.enabled = false;
    list.push(ewallet);
    list
}
pub fn chart() -> Chart {
    Chart::new(accounts()).unwrap()
}
pub fn settings(rounding: Rounding) -> ComposeSettings {
    ComposeSettings {
        rounding,
        tax_mode: TaxMode::Taxable,
        source: Source::Manual,
    }
}
pub fn floor() -> ComposeSettings {
    settings(Rounding::Floor)
}
pub fn input(
    kind: SimpleKind,
    date: Date,
    amount: i64,
    account: Option<&str>,
    wallet: &str,
) -> SimpleInput {
    SimpleInput {
        kind,
        date,
        amount,
        account: account.map(str::to_owned),
        wallet: wallet.to_owned(),
        to_wallet: None,
        tax_code: None,
        partner: String::new(),
        description: String::new(),
        business_pct: None,
        memo: String::new(),
    }
}
pub fn expense(date: Date, amount: i64, account: &str, wallet: &str) -> SimpleInput {
    let mut i = input(SimpleKind::Expense, date, amount, Some(account), wallet);
    i.business_pct = Some(100);
    i
}
pub fn income(date: Date, amount: i64, account: &str, wallet: &str) -> SimpleInput {
    input(SimpleKind::Income, date, amount, Some(account), wallet)
}
pub fn transfer(date: Date, amount: i64, from: &str, to: &str) -> SimpleInput {
    let mut i = input(SimpleKind::Transfer, date, amount, None, from);
    i.to_wallet = Some(to.to_owned());
    i
}
pub fn entry(composed: &Composed, id: &str) -> Entry {
    composed.to_entry(EntryId::new(id).unwrap()).unwrap()
}
/// 入力の並びから仕訳を作る（試験用。組み立ての失敗は試験の失敗）。
pub fn post_all(inputs: &[SimpleInput]) -> Vec<Entry> {
    let chart = chart();
    inputs
        .iter()
        .enumerate()
        .map(|(n, i)| {
            entry(
                &kamoku_books::compose_simple(i, &chart, &floor()).unwrap(),
                &format!("E{n}"),
            )
        })
        .collect()
}
pub fn refs(entries: &[Entry]) -> Vec<&Entry> {
    entries.iter().collect()
}
pub fn numbered(entries: &[Entry]) -> Vec<(u64, &Entry)> {
    (1u64..).zip(entries.iter()).collect()
}
pub fn kind_of(chart: &Chart, code: &str) -> Kind {
    chart.kind(code).unwrap()
}
