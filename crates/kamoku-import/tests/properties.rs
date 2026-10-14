// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 取込の性質の試験（proptest）。

use kamoku_import::{
    Direction, EncodingChoice, ParsedRow, decode, fingerprint, number_occurrences, parse_csv,
};
use proptest::prelude::*;
use std::collections::{BTreeMap, HashSet};
use time::{Date, Month};

/// 試験用の簡単なCSVの書き手。すべての欄を引用符で囲み、`"` は2つにする。
fn write_csv(rows: &[Vec<String>], delimiter: &str) -> String {
    rows.iter()
        .map(|row| {
            row.iter()
                .map(|c| format!("\"{}\"", c.replace('"', "\"\"")))
                .collect::<Vec<_>>()
                .join(delimiter)
        })
        .collect::<Vec<_>>()
        .join("\r\n")
}
fn cell() -> impl Strategy<Value = String> {
    prop_oneof![
        "[a-z0-9 ]{0,8}",
        "[,\"\r\n\t;]{0,4}",
        "(サンプル|デモ|①|ｶﾅ|　){0,3}",
        ".{0,6}",
    ]
}
fn table() -> impl Strategy<Value = Vec<Vec<String>>> {
    (1usize..5).prop_flat_map(|width| {
        prop::collection::vec(prop::collection::vec(cell(), width), 1..20).prop_map(|mut rows| {
            // 空白だけの行は空の行として読み飛ばされるので、1つは文字を入れる。
            for row in &mut rows {
                if row.iter().all(|c| c.trim().is_empty()) {
                    row[0].push('x');
                }
            }
            rows
        })
    })
}
fn row(day: u8, description: &str, amount: i64) -> ParsedRow {
    ParsedRow {
        line: 1,
        date: Date::from_calendar_date(2026, Month::October, day).unwrap(),
        description: description.to_owned(),
        direction: if amount < 0 {
            Direction::Out
        } else {
            Direction::In
        },
        amount: amount.abs(),
        id: None,
    }
}
fn pool_row() -> impl Strategy<Value = ParsedRow> {
    (
        1u8..4,
        prop::sample::select(vec!["サンプル", "デモ", "デモ 店"]),
        prop::sample::select(vec![-100i64, 100, 250]),
    )
        .prop_map(|(day, text, amount)| row(day, text, amount))
}

proptest! {
    // 任意のバイト列で、文字にする処理とCSVの読み取りは止まらない（panicしない）。
    #[test]
    fn never_panics(bytes in prop::collection::vec(any::<u8>(), 0..600)) {
        for choice in [EncodingChoice::Auto, EncodingChoice::Utf8, EncodingChoice::ShiftJis] {
            if let Ok(decoded) = decode(&bytes, choice) {
                let _ = parse_csv(&decoded.text);
            }
        }
        let _ = parse_csv(&String::from_utf8_lossy(&bytes));
    }
    // AC-04.2: 引用符・区切り・改行を含む欄でも、書き出して読み直すと同じ値に戻る（区切りは3種類）。
    #[test]
    fn write_then_read_round_trips(rows in table(), delimiter in prop::sample::select(vec![",", "\t", ";"])) {
        let parsed = parse_csv(&write_csv(&rows, delimiter)).unwrap();
        let cells: Vec<Vec<String>> = parsed.rows.into_iter().map(|r| r.cells).collect();
        prop_assert_eq!(cells, rows);
    }
    // 同じ入力からは同じ指紋。口座が違えば違う指紋。
    #[test]
    fn fingerprint_is_deterministic(r in pool_row(), wallet in "w[0-9]{2}") {
        prop_assert_eq!(fingerprint(&wallet, &r), fingerprint(&wallet, &r.clone()));
        prop_assert_ne!(fingerprint(&wallet, &r), fingerprint(&format!("{wallet}x"), &r));
        prop_assert_eq!(fingerprint(&wallet, &r).len(), 64);
    }
    // AC-04.5: 番号付けは、並べ替えても（指紋, 番号）の組の集まりが変わらず、同じ指紋には 1..k が振られる。
    #[test]
    fn numbering_survives_shuffles((rows, shuffled) in prop::collection::vec(pool_row(), 0..30)
        .prop_flat_map(|rows| (Just(rows.clone()), Just(rows).prop_shuffle()))) {
        let mut a = number_occurrences("w01", &rows);
        let mut b = number_occurrences("w01", &shuffled);
        a.sort();
        b.sort();
        prop_assert_eq!(&a, &b);
        let mut per_print = BTreeMap::<String, Vec<u32>>::new();
        for (print, n) in a {
            per_print.entry(print).or_default().push(n);
        }
        for numbers in per_print.values() {
            prop_assert_eq!(numbers.clone(), (1..=u32::try_from(numbers.len()).unwrap()).collect::<Vec<_>>());
        }
    }
    // AC-04.4: 期間が重なる別のファイルでは、重なった分の組はすでにあり、新しい行の組だけが残る。
    #[test]
    fn overlapping_file_adds_only_new_rows(first in prop::collection::vec(pool_row(), 0..20), keep in prop::collection::vec(any::<bool>(), 20), fresh in 0usize..5) {
        let old: HashSet<(String, u32)> = number_occurrences("w01", &first).into_iter().collect();
        let mut second: Vec<ParsedRow> = first.iter().zip(&keep).filter(|(_, k)| **k).map(|(r, _)| r.clone()).collect();
        let new_rows: Vec<ParsedRow> = (0..fresh).map(|i| row(9, &format!("新しい行 {i}"), 1)).collect();
        second.extend(new_rows.iter().cloned());
        let added: Vec<(String, u32)> = number_occurrences("w01", &second).into_iter().filter(|p| !old.contains(p)).collect();
        prop_assert_eq!(added.len(), fresh);
    }
}
