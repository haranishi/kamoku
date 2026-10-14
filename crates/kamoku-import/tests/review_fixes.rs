// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! AC-04.2 / AC-04.3: 入力の上限と引用符の回帰試験。
use kamoku_import::*;
fn mapping() -> Mapping {
    Mapping {
        skip_rows: 0,
        date_col: 0,
        desc_cols: vec![1],
        id_col: None,
        amount: AmountMapping::Signed {
            col: 2,
            inflow: Inflow::Negative,
        },
    }
}
fn outcomes(description: &str, m: &Mapping) -> Vec<RowOutcome> {
    let table = Table {
        delimiter: Delimiter::Comma,
        rows: vec![Row {
            line: 1,
            cells: vec!["2026-10-01".into(), description.into(), "100".into()],
        }],
    };
    apply_mapping(&table, m)
}
fn invalid(description: &str, m: &Mapping, message: &str) {
    assert!(
        matches!(&outcomes(description, m)[0], RowOutcome::Invalid { reason, .. } if reason == message)
    );
}
#[test]
fn i1_too_many_description_columns_rejected_before_join() {
    let mut m = mapping();
    m.desc_cols = vec![1; 100_000];
    // 未修正での赤確認にも巨大な割当を避けるため、まず空のセルで検査する。
    invalid("", &m, "摘要の列の指定が多すぎます（8個まで）");
    let start = std::time::Instant::now();
    invalid(
        &"x".repeat(64 * 1024),
        &m,
        "摘要の列の指定が多すぎます（8個まで）",
    );
    let elapsed = start.elapsed();
    eprintln!("I1 100000列・64KiBセル: {elapsed:?}");
    assert!(elapsed < std::time::Duration::from_millis(100));
    m.desc_cols = vec![1; 8];
    assert!(matches!(&outcomes("x", &m)[0], RowOutcome::Parsed(_)));
    m.desc_cols.push(1);
    invalid("x", &m, "摘要の列の指定が多すぎます（8個まで）");
}
#[test]
fn i1_i3_description_byte_boundaries() {
    let m = mapping();
    for accepted in ["x".repeat(1000), "あ".repeat(333) + "x"] {
        assert!(matches!(&outcomes(&accepted, &m)[0], RowOutcome::Parsed(_)));
    }
    for rejected in [
        "x".repeat(1001),
        "あ".repeat(333) + "xx",
        "あ".repeat(334),
        " ".repeat(1001),
    ] {
        invalid(&rejected, &m, "摘要が長すぎます（1000バイトまで）");
    }
    let mut m = mapping();
    m.desc_cols = vec![1, 1];
    assert!(matches!(
        &outcomes(&"x".repeat(499), &m)[0],
        RowOutcome::Parsed(_)
    ));
    invalid(&"x".repeat(500), &m, "摘要が長すぎます（1000バイトまで）");
}
#[test]
fn i2_column_limit_and_delimiter_detection_are_bounded() {
    let row = "x,".repeat(999) + "x";
    assert_eq!(parse_csv(&row).unwrap().rows[0].cells.len(), 1000);
    for d in [',', '\t', ';'] {
        let input = d.to_string().repeat(1000);
        let error = parse_csv(&input).unwrap_err();
        assert_eq!((error.code(), error.line()), ("invalid_csv", Some(1)));
    }
    let input = ",".repeat(10_000_000);
    let start = std::time::Instant::now();
    let error = parse_csv(&input).unwrap_err();
    eprintln!("I2 1000万カンマ: {:?}", start.elapsed());
    assert_eq!((error.code(), error.line()), ("invalid_csv", Some(1)));
}
#[test]
fn i2_total_cell_limit_includes_blank_records() {
    let row = "x,".repeat(999) + "x\n";
    let accepted = row.repeat(2000);
    assert_eq!(parse_csv(&accepted).unwrap().rows.len(), 2000);
    let Err(error) = parse_csv(&(accepted + "x\n")) else {
        panic!("セル上限で拒否されるはず");
    };
    assert_eq!((error.code(), error.line()), ("invalid_csv", Some(2001)));
    let blank = (",".repeat(999) + "\n").repeat(2000) + "x\n";
    let error = parse_csv(&blank).unwrap_err();
    assert_eq!(error.line(), Some(2001));
}
#[test]
fn i4_characters_after_closing_quote_are_invalid_csv() {
    for suffix in ["00", " ", "\t", "z"] {
        let input = format!("date,desc,amount\n2026-10-01,デモ,\"1\"{suffix}\n");
        let error = parse_csv(&input).unwrap_err();
        assert_eq!((error.code(), error.line()), ("invalid_csv", Some(2)));
    }
    assert!(parse_csv("a,b\n\"1\",\"2\"\n").is_ok());
    assert!(parse_csv("a,b\n1,2   \n").is_ok());
    assert!(parse_csv("a,b\n1,\"2\"").is_ok());
}
