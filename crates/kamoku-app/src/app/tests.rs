// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 処理の途中の停止（panic）からの立て直し（コードレビューの指摘 B2）。

use super::App;
use crate::api::{ClientIds, Uploads};
use crate::server::{Mode, ensure_cash_wallet};
use crate::state::State;
use crate::statics::Statics;
use kamoku_ledger::{AccountCode, Entry, EntryId, Line, Side, Yen};
use kamoku_store::{OpenOptions, Store};
use std::panic::{AssertUnwindSafe, catch_unwind};

fn entry(id: &str) -> Entry {
    let line = |code: &str, side| Line::new(AccountCode::new(code).unwrap(), side, Yen::new(110));
    Entry::new(
        EntryId::new(id).unwrap(),
        time::macros::date!(2026 - 10 - 01),
        "試験の仕訳",
        vec![line("exp_comm", Side::Debit), line("cash", Side::Credit)],
    )
    .unwrap()
}

// B2: 錠を握ったまま処理が止まった（panicした）あと、次の要求は、保存の層をDBからやり直してから使う。
// 別の接続が足した仕訳が見え、錠の汚れは消え、続けて書き込める。未保存の仕訳を落とすこと自体は、
// kamoku-store の recover の試験（トランザクションの途中で止める）で確かめている。
#[test]
fn a_panic_while_locked_is_recovered_from_the_database() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("kamoku.db");
    let create = OpenOptions {
        create_if_missing: true,
    };
    let (mut store, report) = Store::open(&db, create).unwrap();
    ensure_cash_wallet(&mut store).unwrap();
    let state = State {
        store,
        mode: Mode::Normal,
        events_in_db: report.events_in_db,
        clients: ClientIds::default(),
        uploads: Uploads::default(),
    };
    let app = App::new(0, "0".repeat(32), Statics::Embedded, state);
    {
        let (mut other, _) = Store::open(&db, OpenOptions::default()).unwrap();
        other.post(entry("e_other"), 1, None).unwrap();
    }
    let injected = catch_unwind(AssertUnwindSafe(|| {
        let _held = app.lock();
        panic!("試験で注入した停止");
    }));
    assert!(injected.is_err());
    assert!(app.state.is_poisoned());
    let mut state = app.lock();
    assert_eq!(
        state.store.ledger().events().len(),
        1,
        "DBからやり直していない"
    );
    state.store.post(entry("e_after"), 2, None).unwrap();
    drop(state);
    assert!(!app.state.is_poisoned());
    assert_eq!(app.lock().store.ledger().events().len(), 2);
}
