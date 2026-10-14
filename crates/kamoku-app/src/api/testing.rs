// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 単体の試験の道具。処理の関数を、受け口を通さずに、メモリの帳簿で呼ぶ。

use super::{ClientIds, Reply, Run, Uploads};
use crate::query::Query;
use crate::server::{Mode, ensure_cash_wallet};
use crate::state::State;
use kamoku_store::Store;
use serde_json::{Value, json};

/// 試験の帳簿（課税事業者、開業日2026-01-01、口座は現金・預金・カード）の状態。
pub(crate) fn state(clients: ClientIds) -> State {
    let (mut store, report) = Store::open_in_memory().unwrap();
    ensure_cash_wallet(&mut store).unwrap();
    let mut state = State {
        store,
        mode: Mode::Normal,
        events_in_db: report.events_in_db,
        clients,
        uploads: Uploads::default(),
    };
    let settings = json!({
        "business_name": "サンプル事務所", "owner_name": "", "tax_mode": "taxable",
        "invoice_number": null, "rounding": "floor", "opened_on": "2026-01-01",
        "locked_until": null, "setup_done": true
    });
    assert_eq!(run(&mut state, super::settings_put, &settings).0, 200);
    for (name, kind) in [("サンプル銀行", "bank"), ("サンプルカード", "card")] {
        let account = json!({ "name": name, "reading": "", "wallet": { "type": kind } });
        assert_eq!(run(&mut state, super::account_add, &account).0, 201);
    }
    state
}

/// 処理を呼び、状態と本文を返す。失敗も、受け口と同じ状態と本文にする。
pub(crate) fn run(state: &mut State, f: Run, body: &Value) -> (u16, Value) {
    let query = Query::parse("").unwrap();
    let bytes = serde_json::to_vec(body).unwrap();
    let call = super::Call {
        params: &[],
        query: &query,
        body: &bytes,
        now: 1_791_000_000,
        today: time::macros::date!(2026 - 10 - 14),
    };
    match f(state, &call) {
        Ok(Reply::Json(status, value)) => (status, value),
        Ok(Reply::File(_)) => panic!("ファイルの応答は扱わない"),
        Err(e) => (e.status, e.body()),
    }
}

/// 支出の入力（試験の決まった形）。
pub(crate) fn expense(amount: i64, client_id: &str) -> Value {
    json!({ "kind": "expense", "date": "2026-10-05", "amount": amount, "account": "exp_comm",
            "wallet": "w01", "tax_code": "P10", "description": "試験の支出", "client_id": client_id })
}
