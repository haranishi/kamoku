// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Phase 0 の受け入れ試験。

use kamoku_jp_tax::{
    CreditRounding, InvoiceNumber, InvoiceNumberError, Rate, Rounding, TaxError, TaxSplit,
    add_tax_excluded, split_tax_included, transitional_credit, transitional_percent,
    transitional_rules,
};
use kamoku_ledger::Yen;
use time::macros::date;

#[test]
fn hand_checked_splits() {
    assert_eq!(Rate::Standard.percent(), 10);
    assert_eq!(Rate::Reduced.percent(), 8);
    assert_eq!(
        split_tax_included(Yen::new(11_000), Rate::Standard, Rounding::Floor).unwrap(),
        TaxSplit {
            net: Yen::new(10_000),
            tax: Yen::new(1000)
        }
    );
    for (rounding, expected) in [
        (Rounding::Floor, 90),
        (Rounding::Round, 91),
        (Rounding::Ceil, 91),
    ] {
        let result = split_tax_included(Yen::new(1000), Rate::Standard, rounding).unwrap();
        assert_eq!(result.tax, Yen::new(expected));
        assert_eq!(result.net, Yen::new(1000 - expected));
    }
    for rounding in [Rounding::Floor, Rounding::Round, Rounding::Ceil] {
        assert_eq!(
            split_tax_included(Yen::new(1080), Rate::Reduced, rounding).unwrap(),
            TaxSplit {
                net: Yen::new(1000),
                tax: Yen::new(80)
            }
        );
        for rate in [Rate::Standard, Rate::Reduced] {
            assert_eq!(
                split_tax_included(Yen::ZERO, rate, rounding).unwrap(),
                TaxSplit {
                    net: Yen::ZERO,
                    tax: Yen::ZERO
                }
            );
            assert_eq!(
                add_tax_excluded(Yen::ZERO, rate, rounding).unwrap(),
                TaxSplit {
                    net: Yen::ZERO,
                    tax: Yen::ZERO
                }
            );
        }
    }
    assert_eq!(
        add_tax_excluded(Yen::new(1000), Rate::Reduced, Rounding::Floor).unwrap(),
        TaxSplit {
            net: Yen::new(1000),
            tax: Yen::new(80)
        }
    );
    for (rounding, tax) in [
        (Rounding::Floor, 99),
        (Rounding::Round, 100),
        (Rounding::Ceil, 100),
    ] {
        assert_eq!(
            add_tax_excluded(Yen::new(999), Rate::Standard, rounding)
                .unwrap()
                .tax,
            Yen::new(tax)
        );
    }
    // 0.5ちょうどは切り上げる。
    assert_eq!(
        add_tax_excluded(Yen::new(5), Rate::Standard, Rounding::Round)
            .unwrap()
            .tax,
        Yen::new(1)
    );
}
#[test]
fn hand_checked_credits() {
    assert_eq!(
        transitional_credit(Yen::new(11_000), Rate::Standard, 80, CreditRounding::Floor).unwrap(),
        Yen::new(624)
    );
    for (amount, rate, percent, floor, round) in [
        (300, Rate::Standard, 80, 17, 17),
        (1, Rate::Standard, 80, 0, 0),
        (1000, Rate::Standard, 80, 56, 57),
        (1080, Rate::Reduced, 80, 49, 50),
        (1080, Rate::Reduced, 70, 43, 44),
    ] {
        assert_eq!(
            transitional_credit(Yen::new(amount), rate, percent, CreditRounding::Floor).unwrap(),
            Yen::new(floor)
        );
        assert_eq!(
            transitional_credit(Yen::new(amount), rate, percent, CreditRounding::Round).unwrap(),
            Yen::new(round)
        );
    }
    for rate in [Rate::Standard, Rate::Reduced] {
        for rounding in [CreditRounding::Floor, CreditRounding::Round] {
            assert_eq!(
                transitional_credit(Yen::ZERO, rate, 100, rounding).unwrap(),
                Yen::ZERO
            );
            assert_eq!(
                transitional_credit(Yen::new(i64::MAX), rate, 0, rounding).unwrap(),
                Yen::ZERO
            );
            assert!(transitional_credit(Yen::new(i64::MAX), rate, 100, rounding).is_ok());
        }
    }
}
#[test]
fn errors_and_extremes() {
    for amount in [-1, i64::MIN] {
        assert_eq!(
            split_tax_included(Yen::new(amount), Rate::Standard, Rounding::Floor),
            Err(TaxError::NegativeAmount)
        );
        assert_eq!(
            add_tax_excluded(Yen::new(amount), Rate::Standard, Rounding::Floor),
            Err(TaxError::NegativeAmount)
        );
        assert_eq!(
            transitional_credit(Yen::new(amount), Rate::Standard, 80, CreditRounding::Floor),
            Err(TaxError::NegativeAmount)
        );
    }
    for percent in [101, 255] {
        assert_eq!(
            transitional_credit(Yen::ZERO, Rate::Standard, percent, CreditRounding::Floor),
            Err(TaxError::InvalidPercent)
        );
    }
    for rate in [Rate::Standard, Rate::Reduced] {
        for rounding in [Rounding::Floor, Rounding::Round, Rounding::Ceil] {
            let split = split_tax_included(Yen::new(i64::MAX), rate, rounding).unwrap();
            assert_eq!(split.net.checked_add(split.tax), Some(Yen::new(i64::MAX)));
            assert_eq!(
                add_tax_excluded(Yen::new(i64::MAX), rate, rounding),
                Err(TaxError::Overflow)
            );
        }
    }
    assert_eq!(TaxError::Overflow.to_string(), "税計算が桁あふれしました");
}
#[test]
fn date_boundaries_and_sources() {
    for (day, percent) in [
        (date!(2023 - 09 - 30), 0),
        (date!(2023 - 10 - 01), 80),
        (date!(2026 - 09 - 30), 80),
        (date!(2026 - 10 - 01), 70),
        (date!(2028 - 09 - 30), 70),
        (date!(2028 - 10 - 01), 50),
        (date!(2030 - 09 - 30), 50),
        (date!(2030 - 10 - 01), 30),
        (date!(2031 - 09 - 30), 30),
        (date!(2031 - 10 - 01), 0),
    ] {
        assert_eq!(transitional_percent(day), percent);
    }
    let rules = transitional_rules();
    assert_eq!(rules.len(), 4);
    for rule in rules {
        assert_eq!(
            rule.source,
            "https://www.nta.go.jp/taxes/shiraberu/zeimokubetsu/shohi/keigenzeiritsu/pdf/qa/113-3.pdf"
        );
        assert_eq!(transitional_percent(rule.from), rule.percent);
        assert_eq!(transitional_percent(rule.to), rule.percent);
    }
}
#[test]
fn invoice_format_only() {
    let invoice = InvoiceNumber::parse("T1234567890123").unwrap();
    assert_eq!(invoice.as_str(), "T1234567890123");
    assert_eq!(invoice.to_string(), "T1234567890123");
    assert!(InvoiceNumber::parse("T0000000000000").is_ok());
    for bad in [
        "t1234567890123",
        "T123456789012",
        "T12345678901234",
        "T123-4567890123",
        "T１２３４５６７８９０１２３",
        " T1234567890123",
        "T1234567890123 ",
        "",
        "T123456789012a",
    ] {
        assert_eq!(
            InvoiceNumber::parse(bad),
            Err(InvoiceNumberError::InvalidFormat)
        );
    }
}

#[test]
fn fractions_below_half_round_down() {
    for (rounding, low_tax, higher_tax) in [
        (Rounding::Floor, 0, 1),
        (Rounding::Round, 0, 1),
        (Rounding::Ceil, 1, 2),
    ] {
        for amount in [1, 4] {
            assert_eq!(
                add_tax_excluded(Yen::new(amount), Rate::Standard, rounding).unwrap(),
                TaxSplit {
                    net: Yen::new(amount),
                    tax: Yen::new(low_tax)
                }
            );
        }
        for (amount, tax) in [(1, low_tax), (12, higher_tax)] {
            assert_eq!(
                split_tax_included(Yen::new(amount), Rate::Standard, rounding).unwrap(),
                TaxSplit {
                    net: Yen::new(amount - tax),
                    tax: Yen::new(tax)
                }
            );
        }
    }
}
