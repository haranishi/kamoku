// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Phase 0 の受け入れ試験。

use kamoku_jp_tax::{
    CreditRounding, Rate, Rounding, add_tax_excluded, split_tax_included, transitional_credit,
};
use kamoku_ledger::Yen;
use proptest::prelude::*;

fn rounding_direction(tax: Yen, numerator: i128, denominator: i128, rounding: Rounding) -> bool {
    let scaled_tax = i128::from(tax.get()) * denominator;
    match rounding {
        Rounding::Floor => scaled_tax <= numerator,
        Rounding::Ceil => scaled_tax >= numerator,
        Rounding::Round => {
            let twice_error = (scaled_tax - numerator).abs() * 2;
            twice_error <= denominator && (twice_error != denominator || scaled_tax >= numerator)
        }
    }
}

proptest! {
    #[test]
    fn included_invariants(amount in 0i64..=1_000_000_000_000, other in 0i64..=1_000_000_000_000) {
        for rate in [Rate::Standard, Rate::Reduced] {
            for rounding in [Rounding::Floor, Rounding::Round, Rounding::Ceil] {
                let split = split_tax_included(Yen::new(amount), rate, rounding).unwrap();
                prop_assert_eq!(split.net.get() + split.tax.get(), amount);
                prop_assert!(0 <= split.tax.get() && split.tax.get() <= amount);
                let denominator = i128::from(100 + rate.percent());
                let exact_numerator = i128::from(amount) * i128::from(rate.percent());
                let difference = (i128::from(split.tax.get()) * denominator - exact_numerator).abs();
                prop_assert!(difference < denominator);
                prop_assert!(rounding_direction(split.tax, exact_numerator, denominator, rounding));
                let larger = amount.max(other); let smaller = amount.min(other);
                let high = split_tax_included(Yen::new(larger), rate, rounding).unwrap();
                let low = split_tax_included(Yen::new(smaller), rate, rounding).unwrap();
                prop_assert!(low.tax <= high.tax);
            }
        }
    }
    #[test]
    fn excluded_and_credit_accuracy(amount in 0i64..=1_000_000_000_000, percent in 0u8..=100) {
        for rate in [Rate::Standard, Rate::Reduced] {
            for rounding in [Rounding::Floor, Rounding::Round, Rounding::Ceil] {
                let split = add_tax_excluded(Yen::new(amount), rate, rounding).unwrap();
                prop_assert_eq!(split.net, Yen::new(amount));
                let difference = (i128::from(split.tax.get()) * 100 - i128::from(amount) * i128::from(rate.percent())).abs();
                prop_assert!(difference < 100);
                prop_assert!(rounding_direction(split.tax, i128::from(amount) * i128::from(rate.percent()), 100, rounding));
            }
            for rounding in [CreditRounding::Floor, CreditRounding::Round] {
                let tax = transitional_credit(Yen::new(amount), rate, percent, rounding).unwrap();
                let (factor, denominator) = match rate { Rate::Standard => (78i128, 110_000i128), Rate::Reduced => (624, 1_080_000), _ => unreachable!() };
                let difference = (i128::from(tax.get()) * denominator - i128::from(amount) * factor * i128::from(percent)).abs();
                prop_assert!(difference < denominator);
                let direction = match rounding { CreditRounding::Floor => Rounding::Floor, CreditRounding::Round => Rounding::Round };
                prop_assert!(rounding_direction(tax, i128::from(amount) * factor * i128::from(percent), denominator, direction));
            }
        }
    }
}

#[test]
fn rounding_directions_cover_residues_and_ties() {
    for rate in [Rate::Standard, Rate::Reduced] {
        let (factor, denominator) = match rate {
            Rate::Standard => (78i128, 110_000i128),
            Rate::Reduced => (624, 1_080_000),
            _ => unreachable!(),
        };
        for amount in 0..=55_000 {
            for rounding in [Rounding::Floor, Rounding::Round, Rounding::Ceil] {
                let excluded = add_tax_excluded(Yen::new(amount), rate, rounding).unwrap();
                assert!(rounding_direction(
                    excluded.tax,
                    i128::from(amount) * i128::from(rate.percent()),
                    100,
                    rounding
                ));
                let included = split_tax_included(Yen::new(amount), rate, rounding).unwrap();
                assert!(rounding_direction(
                    included.tax,
                    i128::from(amount) * i128::from(rate.percent()),
                    i128::from(100 + rate.percent()),
                    rounding
                ));
            }
            // percent=1なら両税率で半円ちょうどと全剰余を含む。
            for rounding in [CreditRounding::Floor, CreditRounding::Round] {
                let tax = transitional_credit(Yen::new(amount), rate, 1, rounding).unwrap();
                let direction = match rounding {
                    CreditRounding::Floor => Rounding::Floor,
                    CreditRounding::Round => Rounding::Round,
                };
                assert!(rounding_direction(
                    tax,
                    i128::from(amount) * factor,
                    denominator,
                    direction
                ));
            }
        }
    }
}
