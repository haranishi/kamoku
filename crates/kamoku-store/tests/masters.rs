// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 保存層の受け入れ条件の回帰試験。

mod common;
use common::*;
use kamoku_books::Kind;
use kamoku_store::*;
use serde_json::json;

#[test]
fn account_rules_and_log() {
    let (mut s, _) = Store::open_in_memory().unwrap();
    s.set_actor("デモ利用者").unwrap();
    let w = wallet(&mut s);
    assert_eq!(w, "w01");
    let x = s
        .add_account(
            NewAccount {
                name: "サンプル費用".into(),
                reading: "さんぷる".into(),
                kind: Kind::Expense,
                parent: Some("exp_misc".into()),
                wallet: None,
                default_tax: None,
                default_pct: 50,
            },
            2,
        )
        .unwrap();
    assert_eq!(x.code.as_str(), "x01");
    assert!(
        s.update_account(
            "cash",
            AccountPatch {
                enabled: Some(false),
                ..AccountPatch::default()
            },
            3
        )
        .is_err()
    );
    assert!(
        s.update_account(
            "capital",
            AccountPatch {
                name: Some("デモ".into()),
                ..AccountPatch::default()
            },
            3
        )
        .is_err()
    );
    s.update_account(
        "x01",
        AccountPatch {
            name: Some("デモ費用".into()),
            enabled: Some(false),
            ..AccountPatch::default()
        },
        3,
    )
    .unwrap();
    let mut lines = entry("a", "").lines().to_vec();
    lines[0].account = x.code;
    let e = kamoku_ledger::Entry::new(
        entry("a", "").id().clone(),
        entry("a", "").date(),
        "デモ",
        lines,
    )
    .unwrap();
    assert!(matches!(
        s.post(e, 4, None),
        Err(StoreError::DisabledAccount)
    ));
    let log = s.change_log(50).unwrap();
    assert_eq!(log.len(), 3);
    assert!(log.iter().all(|l| l.actor == "デモ利用者"));
}
#[test]
fn settings_and_partner_validation() {
    let (mut s, _) = Store::open_in_memory().unwrap();
    for (number, tax, rounding) in [
        (Some("T123"), "taxable", "floor"),
        (Some("T1234567890123"), "exempt", "floor"),
        (None, "taxable", "bad"),
    ] {
        let mut settings = s.settings().clone();
        settings.invoice_number = number.map(str::to_owned);
        settings.tax_mode = tax.into();
        settings.rounding = rounding.into();
        assert!(s.save_settings(settings, 1).is_err());
    }
    let p = NewPartner {
        name: "サンプル取引先".into(),
        invoice_number: Some("T1234567890123".into()),
        ..NewPartner::default()
    };
    let partner = s.add_partner(p.clone(), 2).unwrap();
    assert!(matches!(
        s.add_partner(p, 3),
        Err(StoreError::DuplicateName)
    ));
    s.update_partner(
        partner.id,
        PartnerPatch {
            note: Some("デモメモ".into()),
            ..PartnerPatch::default()
        },
        4,
    )
    .unwrap();
    assert_eq!(s.partners().unwrap()[0].note, "デモメモ");
    let w = wallet(&mut s);
    let profile = kamoku_import::Mapping {
        skip_rows: 1,
        date_col: 0,
        desc_cols: vec![1],
        id_col: Some(3),
        amount: kamoku_import::AmountMapping::Signed {
            col: 2,
            inflow: kamoku_import::Inflow::Positive,
        },
    };
    s.save_import_profile(&w, profile.clone(), 5).unwrap();
    assert_eq!(s.import_profile(&w).unwrap(), Some(profile));
    let t = s
        .add_template(
            "サンプル支出",
            json!({"kind":"expense","amount":110,"account":"exp_comm","wallet":w}),
            6,
        )
        .unwrap();
    assert_eq!(s.templates().unwrap().len(), 1);
    s.delete_template(t.id, 7).unwrap();
    assert!(s.templates().unwrap().is_empty());
    assert!(
        s.change_log(50)
            .unwrap()
            .iter()
            .any(|l| l.kind == "template.delete")
    );
}
#[test]
fn single_line_limit_not_total_limit() {
    let (mut s, _) = Store::open_in_memory().unwrap();
    let mut lines = entry("a", "").lines().to_vec();
    for l in &mut lines {
        l.amount = kamoku_ledger::Yen::new(10_000_000_000_000);
    }
    let e = kamoku_ledger::Entry::new(
        entry("a", "").id().clone(),
        entry("a", "").date(),
        "デモ",
        lines,
    )
    .unwrap();
    assert!(matches!(s.post(e, 1, None), Err(StoreError::AmountRange)));
}
