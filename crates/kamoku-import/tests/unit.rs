// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 行ごとの判定、指紋、文字コードとCSVの境界の試験。

use kamoku_import::{
    AmountMapping, Delimiter, Direction, EncodingChoice, ImportError, Inflow, MAX_CELL_BYTES,
    MAX_ROWS, Mapping, ParsedRow, RowOutcome, TextEncoding, apply_mapping, decode, fingerprint,
    parse_csv,
};
use time::macros::date;

fn table(text: &str) -> kamoku_import::Table {
    parse_csv(text).unwrap()
}
fn split() -> Mapping {
    Mapping {
        skip_rows: 1,
        date_col: 0,
        desc_cols: vec![1, 2],
        id_col: None,
        amount: AmountMapping::Split {
            out_col: 3,
            in_col: 4,
        },
    }
}
fn reasons(outcomes: &[RowOutcome]) -> Vec<String> {
    outcomes
        .iter()
        .map(|o| match o {
            RowOutcome::Parsed(p) => format!("ok {}", p.description),
            RowOutcome::Invalid { line, reason } => format!("{line}: {reason}"),
        })
        .collect()
}

// 行ごとの判定（api-inbox.md）: 両方に金額、どちらも空か0円、小数点、読めない日付は invalid。
#[test]
fn invalid_rows_have_reasons() {
    let t = table(
        "日付,摘要,補足,出金,入金\n\
         2026/10/01,サンプル,店,100,200\n\
         2026/10/02,サンプル,,,\n\
         2026/10/03,サンプル,,0,0\n\
         2026/10/04,サンプル,,1.5,\n\
         2026/02/30,サンプル,,100,\n\
         2026/10/06,サンプル,,-100,\n\
         2026/10/07\n\
         2026/10/08,ＤＥＭＯ　店,支店 ,100,0\n",
    );
    let got = reasons(&apply_mapping(&t, &split()));
    assert_eq!(
        got,
        vec![
            "2: 入金と出金の両方に金額があります。",
            "3: 入金と出金が、どちらも空か0円です。",
            "4: 入金と出金が、どちらも空か0円です。",
            "5: 金額に小数点があります。円未満の金額は取り込めません。",
            "6: 日付「2026/02/30」を読めません（実在しない日を含む）。",
            "7: 入金・出金の列に負の金額があります。符号つきの1列の形を選んでください。",
            "8: 入金と出金が、どちらも空か0円です。",
            "ok DEMO 店 支店",
        ]
    );
    let big = table("日付,摘要,金額\n2026/10/01,デモ,10000000000000\n");
    let m = Mapping {
        skip_rows: 1,
        date_col: 0,
        desc_cols: vec![1],
        id_col: None,
        amount: AmountMapping::Signed {
            col: 2,
            inflow: Inflow::Positive,
        },
    };
    assert_eq!(
        reasons(&apply_mapping(&big, &m)),
        vec!["2: 金額が大きすぎます（9,999,999,999,999円まで）。"]
    );
}

fn parsed_row(id: Option<&str>, description: &str, amount: i64) -> ParsedRow {
    ParsedRow {
        line: 2,
        date: date!(2026 - 10 - 01),
        description: description.to_owned(),
        direction: Direction::Out,
        amount,
        id: id.map(str::to_owned),
    }
}

// 取引番号の列があれば、指紋は「口座 + 取引番号」。摘要や金額が違っても同じ取引とみなす。
#[test]
fn id_column_fingerprint() {
    let first = parsed_row(Some("TX-0001"), "サンプル", 100);
    let same_id = parsed_row(Some(" ＴＸ－０００１ "), "別の摘要", 999);
    assert_eq!(fingerprint("w03", &first), fingerprint("w03", &same_id));
    // Python の hashlib で別に計算した値。
    assert_eq!(
        fingerprint("w03", &first),
        "cae31de3632873768551059b4ff0e41314973cb0f5c8c0375702de8bbc674eb8"
    );
    assert_ne!(fingerprint("w03", &first), fingerprint("w02", &first));
    assert_ne!(
        fingerprint("w03", &first),
        fingerprint("w03", &parsed_row(Some("TX-0002"), "サンプル", 100))
    );
    assert_eq!(
        fingerprint("w03", &parsed_row(Some(" "), "サンプル", 100)),
        fingerprint("w03", &parsed_row(None, "サンプル", 100))
    );
    assert_ne!(
        fingerprint("w03", &first),
        fingerprint("w03", &parsed_row(None, "サンプル", 100))
    );
    let csv = table("日付,摘要,番号,金額\n2026/10/01,デモ,TX-9,100\n2026/10/02,デモ,,100\n");
    let mapping = Mapping {
        skip_rows: 1,
        date_col: 0,
        desc_cols: vec![1],
        id_col: Some(2),
        amount: AmountMapping::Signed {
            col: 3,
            inflow: Inflow::Negative,
        },
    };
    let ids: Vec<Option<String>> = apply_mapping(&csv, &mapping)
        .into_iter()
        .map(|o| match o {
            RowOutcome::Parsed(p) => p.id,
            RowOutcome::Invalid { .. } => None,
        })
        .collect();
    assert_eq!(ids, vec![Some("TX-9".to_owned()), None]);
}

// 指紋は、口座・日付・符号つき金額・正規化した摘要のどれが違っても変わる。正規化で同じになる摘要は同じ。
#[test]
fn row_fingerprint_inputs() {
    let base = parsed_row(None, "サンプル ストア", 1650);
    let print = fingerprint("w01", &base);
    let mut other = base.clone();
    other.description = "　サンプル　　ストア ".to_owned();
    assert_eq!(fingerprint("w01", &other), print);
    let mut other = base.clone();
    other.direction = Direction::In;
    assert_ne!(fingerprint("w01", &other), print);
    let mut other = base.clone();
    other.date = date!(2026 - 10 - 02);
    assert_ne!(fingerprint("w01", &other), print);
    let mut other = base.clone();
    other.amount = 1651;
    assert_ne!(fingerprint("w01", &other), print);
    assert_ne!(fingerprint("w02", &base), print);
    // 長さを前に付けるので、欄の境目をずらしても同じにならない。
    assert_ne!(
        fingerprint("w0", &parsed_row(None, "1サンプル", 1)),
        fingerprint("w01", &parsed_row(None, "サンプル", 1))
    );
}

// AC-04.1: UTF-16 は BOM があるときだけ読む。NUL を含むファイルは文字のCSVではないとして断る。
#[test]
fn decode_edges() {
    let little_endian: Vec<u8> = [0xFF, 0xFE]
        .into_iter()
        .chain("日付,金額\n".encode_utf16().flat_map(u16::to_le_bytes))
        .collect();
    let d = decode(&little_endian, EncodingChoice::Auto).unwrap();
    assert_eq!(
        (d.encoding, d.bom, d.text.as_str()),
        (TextEncoding::Utf16Le, true, "日付,金額\n")
    );
    let big_endian: Vec<u8> = [0xFE, 0xFF]
        .into_iter()
        .chain("a,b".encode_utf16().flat_map(u16::to_be_bytes))
        .collect();
    assert_eq!(
        decode(&big_endian, EncodingChoice::Auto).unwrap().text,
        "a,b"
    );
    assert_eq!(
        decode(b"a,b\0c", EncodingChoice::Auto).unwrap_err(),
        ImportError::NotText
    );
    assert_eq!(decode(b"", EncodingChoice::Auto).unwrap().text, "");
    assert_eq!(parse_csv("").unwrap_err(), ImportError::Empty);
    assert_eq!(parse_csv("\n\r\n , \n").unwrap_err(), ImportError::Empty);
    let utf8 = decode("\u{FEFF}あ".as_bytes(), EncodingChoice::Utf8).unwrap();
    assert_eq!((utf8.text.as_str(), utf8.bom), ("あ", true));
    assert_eq!(
        decode(&[0x82, 0xA0], EncodingChoice::Auto).unwrap().text,
        "あ"
    );
    assert_eq!(
        decode(&[0x82], EncodingChoice::ShiftJis).unwrap().malformed,
        1
    );
    assert_eq!(serde_json::to_value(EncodingChoice::Utf8).unwrap(), "utf-8");
    assert_eq!(
        serde_json::from_value::<EncodingChoice>(serde_json::json!("shift_jis")).unwrap(),
        EncodingChoice::ShiftJis
    );
    assert_eq!(TextEncoding::ShiftJis.as_str(), "shift_jis");
}

// AC-04.2: 区切りの判定、CRだけの改行、空の行、行末の空白、上限。
#[test]
fn csv_edges() {
    assert_eq!(table("a;b;c\n1;2;3\n").delimiter, Delimiter::Semicolon);
    assert_eq!(table("a\tb\n1\t2\n").delimiter, Delimiter::Tab);
    assert_eq!(
        table("\"a,b\";c\n\"1,2\";3\n").delimiter,
        Delimiter::Semicolon
    );
    assert_eq!(table("only\n").delimiter, Delimiter::Comma);
    // 区切りを取り違えると引用符の読み方が崩れるので、崩れない方を選ぶ（proptest が見つけた例）。
    let quoted_tabs = table("\"\",\"\t\t,\"\n");
    assert_eq!(quoted_tabs.delimiter, Delimiter::Comma);
    assert_eq!(quoted_tabs.rows[0].cells, vec!["", "\t\t,"]);
    let t = table("a,b\r1,2\r\r3,4   \r\n");
    assert_eq!(
        t.rows
            .iter()
            .map(|r| (r.line, r.cells.clone()))
            .collect::<Vec<_>>(),
        vec![
            (1, vec!["a".to_owned(), "b".to_owned()]),
            (2, vec!["1".to_owned(), "2".to_owned()]),
            (4, vec!["3".to_owned(), "4".to_owned()]),
        ]
    );
    let t = table("\"a \",b\n");
    assert_eq!(t.rows[0].cells, vec!["a ", "b"]);
    // api-inbox.md: 閉じ引用符の後の空白・文字は、数値を黙って変えないよう断る。
    assert_eq!(
        parse_csv("\"a \",b\n\"x\"  ,\"y\"z\n").unwrap_err(),
        ImportError::InvalidQuote { line: 2 }
    );
    let many = "1\n".repeat(MAX_ROWS);
    assert_eq!(parse_csv(&many).unwrap().rows.len(), MAX_ROWS);
    assert_eq!(
        parse_csv(&format!("{many}1\n")).unwrap_err(),
        ImportError::TooManyRows
    );
    let wide = format!("ok\n{}\n", "x".repeat(MAX_CELL_BYTES + 1));
    assert_eq!(
        parse_csv(&wide).unwrap_err(),
        ImportError::CellTooLarge { line: 2 }
    );
    assert!(parse_csv(&"x".repeat(MAX_CELL_BYTES)).is_ok());
    assert_eq!(
        parse_csv("a\n\"b\nc").unwrap_err(),
        ImportError::UnclosedQuote { line: 2 }
    );
}

#[test]
fn mapping_json_shape() {
    let m: Mapping = serde_json::from_value(serde_json::json!({ "skip_rows": 1, "date_col": 0, "desc_cols": [1], "id_col": null, "amount": { "mode": "split", "out_col": 2, "in_col": 3 } })).unwrap();
    assert_eq!(
        m.amount,
        AmountMapping::Split {
            out_col: 2,
            in_col: 3
        }
    );
    let s: Mapping = serde_json::from_value(serde_json::json!({ "skip_rows": 0, "date_col": 0, "desc_cols": [], "amount": { "mode": "signed", "col": 2, "inflow": "positive" } })).unwrap();
    assert_eq!(
        (s.id_col, s.amount),
        (
            None,
            AmountMapping::Signed {
                col: 2,
                inflow: Inflow::Positive
            }
        )
    );
    assert_eq!(
        serde_json::to_value(&m).unwrap(),
        serde_json::json!({ "skip_rows": 1, "date_col": 0, "desc_cols": [1], "id_col": null, "amount": { "mode": "split", "out_col": 2, "in_col": 3 } })
    );
    let row = parsed_row(Some("A1"), "デモ", 1650);
    assert_eq!(
        serde_json::to_value(&row).unwrap(),
        serde_json::json!({ "line": 2, "date": "2026-10-01", "description": "デモ", "direction": "out", "amount": 1650, "id": "A1" })
    );
}
