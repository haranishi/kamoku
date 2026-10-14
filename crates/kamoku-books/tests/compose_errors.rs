// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 仕訳の組み立ての、入力の誤り・口座どうし・警告・入力の形の試験。

mod common;
use common::{chart, expense, floor, income, transfer};
use kamoku_books::{ComposeError, SimpleKind, TaxMode, compose_simple};
use time::macros::date;

type Change = Box<dyn Fn(&mut kamoku_books::SimpleInput)>;
fn err(i: &kamoku_books::SimpleInput) -> ComposeError {
    compose_simple(i, &chart(), &floor()).unwrap_err()
}

// AC-02.6: 金額、科目、税区分の誤りは保存せず、原因と直し方を日本語で返す。
#[test]
fn input_errors_have_codes_and_messages() {
    let base = expense(date!(2026 - 10 - 05), 1000, "exp_comm", "w01");
    for amount in [0, -1, 10_000_000_000_000] {
        let mut i = base.clone();
        i.amount = amount;
        let e = err(&i);
        assert_eq!((e.code(), e.field()), ("amount_range", Some("amount")));
        assert!(e.message().contains("1円から"), "{}", e.message());
    }
    let mut i = base.clone();
    i.amount = 9_999_999_999_999;
    assert!(compose_simple(&i, &chart(), &floor()).is_ok());
    let cases: Vec<(Change, &str, &str)> = vec![
        (
            Box::new(|i| i.account = Some("nope".into())),
            "unknown_account",
            "account",
        ),
        (Box::new(|i| i.account = None), "missing_account", "account"),
        (
            Box::new(|i| i.wallet = "w99".into()),
            "unknown_account",
            "wallet",
        ),
        (
            Box::new(|i| i.wallet = "exp_fee".into()),
            "unknown_account",
            "wallet",
        ),
        (
            Box::new(|i| i.wallet = "w04".into()),
            "disabled_account",
            "wallet",
        ),
        (
            Box::new(|i| i.account = Some("land".into())),
            "disabled_account",
            "account",
        ),
        (
            Box::new(|i| i.tax_code = Some("P5".into())),
            "bad_tax_code",
            "tax_code",
        ),
        (
            Box::new(|i| i.tax_code = Some("p10".into())),
            "bad_tax_code",
            "tax_code",
        ),
        (
            Box::new(|i| i.business_pct = Some(101)),
            "bad_request",
            "business_pct",
        ),
        (
            Box::new(|i| i.account = Some("w01".into())),
            "bad_request",
            "account",
        ),
        (
            Box::new(|i| i.to_wallet = Some("w02".into())),
            "bad_request",
            "to_wallet",
        ),
        (
            Box::new(|i| i.description = "あ".repeat(334)),
            "bad_request",
            "description",
        ),
        (
            Box::new(|i| i.memo = "x".repeat(1001)),
            "bad_request",
            "memo",
        ),
        (
            Box::new(|i| i.partner = "x".repeat(1001)),
            "bad_request",
            "partner",
        ),
    ];
    for (change, code, field) in cases {
        let mut i = base.clone();
        change(&mut i);
        let e = err(&i);
        assert_eq!((e.code(), e.field()), (code, Some(field)), "{e:?}");
        assert!(
            e.message().ends_with('。') && e.message().chars().count() > 10,
            "{}",
            e.message()
        );
        assert_eq!(e.http_status(), 422);
    }
}

// AC-02.8: 相手科目が口座なら、税区分と割合を断る。同じ口座どうしの振替も断る。収入の割合は100だけ。
#[test]
fn wallet_counterparts_and_transfers() {
    let mut pay_card = expense(date!(2026 - 10 - 27), 50_000, "w02", "w03");
    assert!(compose_simple(&pay_card, &chart(), &floor()).is_ok());
    pay_card.tax_code = Some("P10".to_owned());
    let e = err(&pay_card);
    assert_eq!((e.code(), e.field()), ("bad_request", Some("tax_code")));
    assert!(e.message().contains("2回に分けて"));
    pay_card.tax_code = None;
    pay_card.business_pct = Some(50);
    assert_eq!(err(&pay_card).field(), Some("business_pct"));
    let same = transfer(date!(2026 - 10 - 27), 1000, "w01", "w01");
    assert_eq!(
        (err(&same).code(), err(&same).field()),
        ("bad_request", Some("to_wallet"))
    );
    let mut t = transfer(date!(2026 - 10 - 27), 1000, "w01", "w03");
    t.tax_code = Some("P10".to_owned());
    assert_eq!(err(&t).field(), Some("tax_code"));
    let mut t = transfer(date!(2026 - 10 - 27), 1000, "w01", "w03");
    t.account = Some("exp_comm".to_owned());
    assert_eq!(err(&t).field(), Some("account"));
    let mut t = transfer(date!(2026 - 10 - 27), 1000, "w01", "w03");
    t.to_wallet = None;
    assert_eq!(
        (err(&t).code(), err(&t).field()),
        ("missing_account", Some("to_wallet"))
    );
    let mut i = income(date!(2026 - 10 - 27), 1000, "sales", "w03");
    i.business_pct = Some(70);
    assert_eq!(
        (err(&i).code(), err(&i).field()),
        ("bad_request", Some("business_pct"))
    );
    i.business_pct = Some(100);
    assert!(compose_simple(&i, &chart(), &floor()).is_ok());
}

// 警告: 収入に費用の科目、支出に収益の科目、課税なのに税区分が空、経過措置の日付。
#[test]
fn warnings_for_suspicious_inputs() {
    let warnings = |i: &kamoku_books::SimpleInput, mode: TaxMode| {
        let mut s = floor();
        s.tax_mode = mode;
        compose_simple(i, &chart(), &s).unwrap().warnings
    };
    let w = warnings(
        &income(date!(2026 - 10 - 01), 1000, "exp_comm", "w01"),
        TaxMode::Exempt,
    );
    assert!(w.iter().any(|w| w.contains("収入の科目に、費用")), "{w:?}");
    let w = warnings(
        &expense(date!(2026 - 10 - 01), 1000, "sales", "w01"),
        TaxMode::Exempt,
    );
    assert!(w.iter().any(|w| w.contains("支出の科目に、収益")), "{w:?}");
    let w = warnings(
        &expense(date!(2026 - 10 - 01), 1000, "exp_comm", "w01"),
        TaxMode::Taxable,
    );
    assert!(w.iter().any(|w| w.contains("税区分が空")), "{w:?}");
    assert_eq!(
        warnings(
            &expense(date!(2026 - 10 - 01), 1000, "exp_comm", "w01"),
            TaxMode::Exempt
        ),
        Vec::<String>::new()
    );
    let mut x = expense(date!(2031 - 10 - 01), 1000, "exp_outsource", "w01");
    x.tax_code = Some("P10X".to_owned());
    let w = warnings(&x, TaxMode::Taxable);
    assert!(w.iter().any(|w| w.contains("経過措置の期間の外")), "{w:?}");
    x.date = date!(2026 - 10 - 01);
    let w = warnings(&x, TaxMode::Taxable);
    assert!(w.iter().any(|w| w.contains("70%")), "{w:?}");
    let mut s = expense(date!(2026 - 10 - 01), 1000, "exp_comm", "w01");
    s.tax_code = Some("S10".to_owned());
    assert!(
        warnings(&s, TaxMode::Taxable)
            .iter()
            .any(|w| w.contains("売上の税区分"))
    );
}

// 通信の約束の入力の例（docs/spec/api-entries.md）をそのまま読める。client_id は組み立てでは使わない。
#[test]
fn simple_input_reads_spec_example() {
    let json = r#"{ "kind": "expense", "date": "2026-10-05", "amount": 11000, "account": "exp_comm",
      "wallet": "w02", "tax_code": "P10", "partner": "サンプル通信",
      "description": "10月分 通信費", "business_pct": 70, "memo": "", "client_id": "c_7f3a" }"#;
    let i: kamoku_books::SimpleInput = serde_json::from_str(json).unwrap();
    assert_eq!(i.kind, SimpleKind::Expense);
    assert_eq!(i.date, date!(2026 - 10 - 05));
    assert_eq!(i.business_pct, Some(70));
    let back = serde_json::to_value(&i).unwrap();
    assert_eq!(back["date"], "2026-10-05");
    assert_eq!(back["to_wallet"], serde_json::Value::Null);
    assert!(
        serde_json::from_str::<kamoku_books::SimpleInput>(
            &json.replace("2026-10-05", "2026-02-30")
        )
        .is_err()
    );
}
