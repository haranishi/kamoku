// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use kamoku_ledger::{AccountCode, Entry, EntryId, Line, Side, Yen};
use time::macros::date;

/// 試験用の識別子。
pub fn id(s: &str) -> EntryId {
    EntryId::new(s).unwrap()
}
/// 試験用の明細。
pub fn line(side: Side, amount: i64) -> Line {
    Line::new(AccountCode::new("101").unwrap(), side, Yen::new(amount))
}
/// 試験用の貸借が一致する仕訳。
pub fn entry(s: &str, amount: i64) -> Entry {
    Entry::new(
        id(s),
        date!(2026 - 01 - 05),
        "試験",
        vec![line(Side::Debit, amount), line(Side::Credit, amount)],
    )
    .unwrap()
}
