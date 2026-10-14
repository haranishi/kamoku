// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 日付・金額・摘要の読み取りの試験。

use kamoku_import::{AmountError, normalize_text, parse_amount, parse_date};
use time::macros::date;

// AC-04.3（読み取りの部分）: 依頼書と api-inbox.md に挙げた日付の形を、全角でも読む。
#[test]
fn dates() {
    for (text, expected) in [
        ("2026/10/1", date!(2026 - 10 - 01)),
        ("2026-10-01", date!(2026 - 10 - 01)),
        ("2026.10.01", date!(2026 - 10 - 01)),
        ("20261001", date!(2026 - 10 - 01)),
        ("2026年10月1日", date!(2026 - 10 - 01)),
        ("R8.10.1", date!(2026 - 10 - 01)),
        ("r08.10.01", date!(2026 - 10 - 01)),
        ("令和8年10月1日", date!(2026 - 10 - 01)),
        ("令和元年5月1日", date!(2019 - 05 - 01)),
        ("H31.4.30", date!(2019 - 04 - 30)),
        ("平成31年4月30日", date!(2019 - 04 - 30)),
        ("２０２６／１０／０１", date!(2026 - 10 - 01)),
        ("Ｒ８．１０．１", date!(2026 - 10 - 01)),
        (" 2026/10/1 ", date!(2026 - 10 - 01)),
        ("2028/2/29", date!(2028 - 02 - 29)),
    ] {
        assert_eq!(parse_date(text), Some(expected), "{text}");
    }
    // 実在しない日、元号の期間の外、形の崩れたものは読まない。
    for text in [
        "2026/2/30",
        "2027/2/29",
        "2026/13/01",
        "2026/00/10",
        "R1.4.30",
        "H31.5.1",
        "令和0年5月1日",
        "",
        "abc",
        "2026/10/01 12:00",
        "2026/10-01",
        "26/10/01",
        "2026年10月1",
        "S64.1.7",
        "R8年10月",
    ] {
        assert_eq!(parse_date(text), None, "{text}");
    }
}

#[test]
fn amounts() {
    for (text, expected) in [
        ("1,650", 1650),
        ("１６５０", 1650),
        ("１，６５０", 1650),
        ("¥1,650", 1650),
        ("￥1,650円", 1650),
        ("\\1,650", 1650),
        ("△1,650", -1650),
        ("▲220", -220),
        ("−500", -500),
        ("-500", -500),
        ("－500", -500),
        ("(1,650)", -1650),
        ("1,650-", -1650),
        ("-¥3,300", -3300),
        ("+1", 1),
        (" 0 ", 0),
    ] {
        assert_eq!(parse_amount(text), Ok(Some(expected)), "{text}");
    }
    assert_eq!(parse_amount(""), Ok(None));
    assert_eq!(parse_amount("  "), Ok(None));
    assert_eq!(parse_amount("1650.00"), Err(AmountError::Decimal));
    assert_eq!(parse_amount("１．５"), Err(AmountError::Decimal));
    for text in ["abc", "△-1", "-", "1-2", "(1", "12a"] {
        assert_eq!(parse_amount(text), Err(AmountError::NotNumber), "{text}");
    }
    assert_eq!(
        parse_amount("99999999999999999999"),
        Err(AmountError::TooLarge)
    );
}

#[test]
fn description_normalization() {
    assert_eq!(
        normalize_text("  ＳＡＭＰＬＥ　ｼｮｯﾌﾟ\t 001  "),
        "SAMPLE ｼｮｯﾌﾟ 001"
    );
    assert_eq!(normalize_text("デモ（店）！"), "デモ(店)!");
    assert_eq!(normalize_text("\u{3000}"), "");
}
