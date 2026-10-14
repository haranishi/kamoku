// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 起動と、別のプロセスとの同時の書き込み（`docs/spec/api-common.md`）。

mod common;
use common::*;
use kamoku_books::{ComposeSettings, Rounding, SimpleInput, SimpleKind, Source, TaxMode};
use kamoku_ledger::EntryId;

// api-common「別のプロセスが書いたとき」: 末尾が食い違えば409 stale。読み直すので、次の書き込みは通る。
#[test]
fn another_writer_causes_stale_then_reload() {
    let s = TestServer::start();
    let c = s.login();
    books(&c);
    create(&c, &expense("2026-10-01", 1100, "exp_comm", "w01"));
    {
        // 別のプロセスの代わりに、同じDBを別の接続で開いて書く。
        let (mut other, _) =
            kamoku_store::Store::open(&s.db, kamoku_store::OpenOptions::default()).unwrap();
        let input = SimpleInput {
            kind: SimpleKind::Expense,
            date: time::macros::date!(2026 - 10 - 03),
            amount: 550,
            account: Some("exp_supplies".to_owned()),
            wallet: "w01".to_owned(),
            to_wallet: None,
            tax_code: None,
            partner: String::new(),
            description: "別の接続".to_owned(),
            business_pct: None,
            memo: String::new(),
        };
        let settings = ComposeSettings {
            rounding: Rounding::Floor,
            tax_mode: TaxMode::Taxable,
            source: Source::Manual,
        };
        let composed = kamoku_books::compose_simple(&input, other.chart(), &settings).unwrap();
        other
            .post(
                composed.to_entry(EntryId::new("e_other").unwrap()).unwrap(),
                1,
                None,
            )
            .unwrap();
    }
    let stale = c.post("/entries", &expense("2026-10-02", 2200, "exp_comm", "w01"));
    assert_eq!(
        (stale.status, stale.code().as_str()),
        (409, "stale"),
        "{}",
        stale.text()
    );
    let again = c.post("/entries", &expense("2026-10-02", 2200, "exp_comm", "w01"));
    assert_eq!(again.status, 201, "{}", again.text());
    assert_eq!(c.api("/entries").json()["total"], 3);
    assert_eq!(c.api("/status").json()["ledger"]["events"], 3);
}

// 起動: 番号が使用中なら、次の番号を試す（最大20回）。待ち受けは 127.0.0.1 だけ。
#[test]
fn busy_port_moves_to_the_next_number() {
    let taken = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let busy = taken.local_addr().unwrap().port();
    let s = TestServer::start_with(|o| o.port = busy);
    assert!(
        s.port > busy && s.port < busy.saturating_add(20),
        "{busy} → {}",
        s.port
    );
    assert_eq!(s.login().api("/status").status, 200);
    let refused = std::net::TcpStream::connect(("::1", s.port));
    assert!(refused.is_err(), "IPv6 の localhost では待ち受けていない");
}

// B1: 別のプロセスが科目の既定の割合を変えたあと、古い割合で組み立てて黙って保存しない。
// 変更系の要求（プレビューも）は、組み立てる前に、設定と科目をDBから読み直す。
#[test]
fn masters_changed_elsewhere_are_used_before_composing() {
    let s = TestServer::start();
    let c = s.login();
    books(&c);
    let r = c.patch(
        "/accounts/exp_vehicle",
        &serde_json::json!({ "default_pct": 30 }),
    );
    assert_eq!(r.status, 200, "{}", r.text());
    {
        let (mut other, _) =
            kamoku_store::Store::open(&s.db, kamoku_store::OpenOptions::default()).unwrap();
        let patch = kamoku_store::AccountPatch {
            default_pct: Some(80),
            ..kamoku_store::AccountPatch::default()
        };
        other.update_account("exp_vehicle", patch, 1).unwrap();
    }
    let input = expense("2026-10-02", 11000, "exp_vehicle", "w01");
    let owner_draw = |entry: &serde_json::Value| {
        entry["lines"]
            .as_array()
            .unwrap()
            .iter()
            .find(|l| l["account"] == "owner_draw")
            .map(|l| l["amount"].clone())
    };
    let preview = c.post("/entries/preview", &input).json();
    assert_eq!(owner_draw(&preview), Some(2200.into()), "{preview}");
    let saved = create(&c, &input);
    assert_eq!(saved["meta"]["business_pct"], "80", "{saved}");
    assert_eq!(owner_draw(&saved), Some(2200.into()));
}
