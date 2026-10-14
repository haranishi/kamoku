// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(dead_code)]
use kamoku_books::{Kind, Wallet, WalletType};
use kamoku_ledger::{AccountCode, Entry, EntryId, Line, Side, Yen};
use kamoku_store::*;
use time::macros::date;
pub fn entry(id: &str, text: &str) -> Entry {
    Entry::new(
        EntryId::new(id).unwrap(),
        date!(2026 - 10 - 01),
        text,
        vec![
            Line::new(
                AccountCode::new("exp_comm").unwrap(),
                Side::Debit,
                Yen::new(110),
            ),
            Line::new(
                AccountCode::new("cash").unwrap(),
                Side::Credit,
                Yen::new(110),
            ),
        ],
    )
    .unwrap()
}
pub fn opened(path: &std::path::Path) -> Store {
    Store::open(
        path,
        OpenOptions {
            create_if_missing: true,
        },
    )
    .unwrap()
    .0
}
pub fn wallet(s: &mut Store) -> String {
    s.add_account(
        NewAccount {
            name: "デモ口座".into(),
            reading: "でも".into(),
            kind: Kind::Asset,
            parent: None,
            wallet: Some(Wallet {
                kind: WalletType::Bank,
                institution: "サンプル".into(),
            }),
            default_tax: None,
            default_pct: 100,
        },
        1,
    )
    .unwrap()
    .code
    .to_string()
}
pub fn batch(wallet: &str) -> NewBatch {
    NewBatch {
        filename: "デモ.csv".into(),
        wallet: wallet.into(),
        recorded_at: 1,
        invalid: 1,
    }
}
pub fn draft(line: usize, occurrence: u32) -> NewDraft {
    NewDraft {
        line,
        date: "2026-10-01".into(),
        description: "サンプル通信".into(),
        direction: "out".into(),
        amount: 110,
        fingerprint: "a".repeat(64),
        occurrence,
        external_id: Some("デモ取引番号".into()),
    }
}

pub fn import_entry(store: &Store, id: DraftId, entry_id: &str) -> Entry {
    let draft = store
        .drafts(DraftFilter::default())
        .unwrap()
        .items
        .into_iter()
        .find(|d| d.id == id)
        .unwrap();
    let input = kamoku_books::SimpleInput {
        kind: if draft.direction == "out" {
            kamoku_books::SimpleKind::Expense
        } else {
            kamoku_books::SimpleKind::Income
        },
        date: kamoku_books::parse_date(&draft.date).unwrap(),
        amount: draft.amount,
        account: draft.account,
        wallet: draft.wallet,
        to_wallet: None,
        tax_code: draft.tax_code,
        partner: draft.partner,
        description: draft.description,
        business_pct: Some(draft.business_pct),
        memo: draft.memo,
    };
    let settings = kamoku_books::ComposeSettings {
        rounding: kamoku_books::Rounding::Floor,
        tax_mode: kamoku_books::TaxMode::Exempt,
        source: kamoku_books::Source::Import,
    };
    kamoku_books::compose_simple(&input, store.chart(), &settings)
        .unwrap()
        .to_entry(EntryId::new(entry_id).unwrap())
        .unwrap()
}
