// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 合成データのCSV（tests/fixtures/synthetic/、`tools/make-fixtures.py` が作る）を読む試験。

use kamoku_import::{
    AmountMapping, Delimiter, Direction, EncodingChoice, ImportError, Inflow, Mapping, ParsedRow,
    RowOutcome, Table, TextEncoding, apply_mapping, decode, fingerprint, guess_mapping,
    number_occurrences, parse_csv,
};
use time::macros::date;

fn table(bytes: &[u8]) -> Table {
    parse_csv(&decode(bytes, EncodingChoice::Auto).unwrap().text).unwrap()
}
fn parsed(outcomes: &[RowOutcome]) -> Vec<&ParsedRow> {
    outcomes
        .iter()
        .map(|o| match o {
            RowOutcome::Parsed(p) => p,
            RowOutcome::Invalid { line, reason } => panic!("{line}行目: {reason}"),
        })
        .collect()
}
fn brief(rows: &[&ParsedRow]) -> Vec<(time::Date, String, Direction, i64)> {
    rows.iter()
        .map(|r| (r.date, r.description.clone(), r.direction, r.amount))
        .collect()
}
fn row(
    d: time::Date,
    text: &str,
    dir: Direction,
    amount: i64,
) -> (time::Date, String, Direction, i64) {
    (d, text.to_owned(), dir, amount)
}
const UTF8: &[u8] = include_bytes!("fixtures/synthetic/utf8.csv");
const UTF8_BOM: &[u8] = include_bytes!("fixtures/synthetic/utf8_bom.csv");
const SJIS: &[u8] = include_bytes!("fixtures/synthetic/sjis.csv");
const SPLIT: &[u8] = include_bytes!("fixtures/synthetic/split_columns.csv");
const SIGNED: &[u8] = include_bytes!("fixtures/synthetic/signed.csv");
const CARD: &[u8] = include_bytes!("fixtures/synthetic/card.csv");
const QUOTED: &[u8] = include_bytes!("fixtures/synthetic/quoted.csv");
const BROKEN: &[u8] = include_bytes!("fixtures/synthetic/broken_quote.csv");

// AC-04.1 AC-04.3 AC-04.5: UTF-8の銀行の明細（入金と出金が別の列）。同じ内容の2行は、番号1と2で別の組になる。
#[test]
fn utf8_bank_statement() {
    let d = decode(UTF8, EncodingChoice::Auto).unwrap();
    assert_eq!(
        (d.encoding, d.bom, d.malformed),
        (TextEncoding::Utf8, false, 0)
    );
    let t = parse_csv(&d.text).unwrap();
    assert_eq!((t.delimiter, t.rows.len()), (Delimiter::Comma, 5));
    let m = guess_mapping(&t).unwrap();
    assert_eq!(
        m,
        Mapping {
            skip_rows: 1,
            date_col: 0,
            desc_cols: vec![1],
            id_col: None,
            amount: AmountMapping::Split {
                out_col: 2,
                in_col: 3
            }
        }
    );
    let outcomes = apply_mapping(&t, &m);
    let rows = parsed(&outcomes);
    assert_eq!(
        brief(&rows),
        vec![
            row(
                date!(2026 - 10 - 01),
                "サンプルストア",
                Direction::Out,
                1650
            ),
            row(date!(2026 - 10 - 02), "デモ鉄道", Direction::Out, 420),
            row(
                date!(2026 - 10 - 05),
                "振込 サンプル商事",
                Direction::In,
                55_000
            ),
            row(
                date!(2026 - 10 - 05),
                "振込 サンプル商事",
                Direction::In,
                55_000
            ),
        ]
    );
    assert_eq!(
        rows.iter().map(|r| r.line).collect::<Vec<_>>(),
        vec![2, 3, 4, 5]
    );
    // 指紋は Python の hashlib で、src/fingerprint.rs の仕様から別に計算した値。
    assert_eq!(
        fingerprint("w02", rows[0]),
        "009431b69a560bfb923ad417242f3db05aef59c3de0044b355dc6cd0d4bbfd09"
    );
    assert_eq!(
        fingerprint("w03", rows[2]),
        "d2ab81199d9e8ce996b589b4fe603fea386f5d178f070725a0e91ada76cbdbd4"
    );
    let owned: Vec<ParsedRow> = rows.into_iter().cloned().collect();
    let numbered = number_occurrences("w03", &owned);
    assert_eq!(numbered[2].0, numbered[3].0);
    assert_eq!((numbered[2].1, numbered[3].1), (1, 2));
    assert!(numbered[..2].iter().all(|(_, n)| *n == 1));
}

// AC-04.1: BOMつきUTF-8（CRLF）。全角の英数字と空白は、摘要の正規化で半角になる。
#[test]
fn utf8_with_bom() {
    let d = decode(UTF8_BOM, EncodingChoice::Auto).unwrap();
    assert_eq!((d.encoding, d.bom), (TextEncoding::Utf8, true));
    assert!(!d.text.starts_with('\u{FEFF}'));
    let t = parse_csv(&d.text).unwrap();
    let m = guess_mapping(&t).unwrap();
    assert_eq!(
        (m.date_col, m.desc_cols.clone(), m.amount),
        (
            0,
            vec![1],
            AmountMapping::Split {
                out_col: 2,
                in_col: 3
            }
        )
    );
    let outcomes = apply_mapping(&t, &m);
    assert_eq!(
        brief(&parsed(&outcomes)),
        vec![
            row(
                date!(2026 - 10 - 01),
                "ATM サンプル",
                Direction::Out,
                10_000
            ),
            row(date!(2026 - 10 - 03), "利息 デモ", Direction::In, 1),
        ]
    );
}

// AC-04.1: Shift_JIS（Windows-31J の①を含む）を判定して読む。元号の日付も読む。指定を誤ると、読めないバイトを数える。
#[test]
fn shift_jis_statement() {
    let d = decode(SJIS, EncodingChoice::Auto).unwrap();
    assert_eq!(
        (d.encoding, d.bom, d.malformed),
        (TextEncoding::ShiftJis, false, 0)
    );
    assert!(d.text.contains("デモ商店①"));
    let t = parse_csv(&d.text).unwrap();
    let m = guess_mapping(&t).unwrap();
    assert_eq!(
        m.amount,
        AmountMapping::Split {
            out_col: 2,
            in_col: 3
        }
    );
    assert_eq!(
        brief(&parsed(&apply_mapping(&t, &m))),
        vec![
            row(date!(2026 - 10 - 01), "デモ商店①", Direction::Out, 1650),
            row(date!(2026 - 10 - 02), "サンプル電力", Direction::Out, 5400),
            row(date!(2026 - 10 - 03), "デモ給与", Direction::In, 300_000),
        ]
    );
    let wrong = decode(SJIS, EncodingChoice::Utf8).unwrap();
    assert!(wrong.malformed > 0 && wrong.text.contains('\u{FFFD}'));
    assert_eq!(decode(SJIS, EncodingChoice::ShiftJis).unwrap().text, d.text);
}

// AC-04.3: タブ区切り、入金と出金が別の列、YYYYMMDD の日付。
#[test]
fn tab_separated_split_columns() {
    let t = table(SPLIT);
    assert_eq!(t.delimiter, Delimiter::Tab);
    let m = guess_mapping(&t).unwrap();
    assert_eq!(
        brief(&parsed(&apply_mapping(&t, &m))),
        vec![
            row(date!(2026 - 10 - 01), "サンプルカフェ", Direction::Out, 880),
            row(date!(2026 - 10 - 02), "デモ書店", Direction::Out, 1980),
            row(date!(2026 - 10 - 03), "返金 デモ書店", Direction::In, 1980),
        ]
    );
}

// AC-04.3: 符号つきの1列。推測は「正が出金」。銀行の形なので、正を入金に直して読む。負の書き方は5通り。
#[test]
fn signed_single_column() {
    let t = table(SIGNED);
    let mut m = guess_mapping(&t).unwrap();
    assert_eq!(
        m.amount,
        AmountMapping::Signed {
            col: 2,
            inflow: Inflow::Negative
        }
    );
    m.amount = AmountMapping::Signed {
        col: 2,
        inflow: Inflow::Positive,
    };
    let got: Vec<(Direction, i64)> = parsed(&apply_mapping(&t, &m))
        .iter()
        .map(|r| (r.direction, r.amount))
        .collect();
    assert_eq!(
        got,
        vec![
            (Direction::In, 300_000),
            (Direction::Out, 45_600),
            (Direction::Out, 220),
            (Direction::Out, 1650),
            (Direction::Out, 1650),
            (Direction::Out, 500),
        ]
    );
}

// AC-04.3: カード明細ふう。見出しの前の題名の行を飛ばし、「ご利用金額」は正が出金、負（返品）が入金。
#[test]
fn card_statement() {
    let t = table(CARD);
    let m = guess_mapping(&t).unwrap();
    assert_eq!(
        (m.skip_rows, m.date_col, m.desc_cols.clone()),
        (2, 0, vec![1])
    );
    assert_eq!(
        m.amount,
        AmountMapping::Signed {
            col: 2,
            inflow: Inflow::Negative
        }
    );
    assert_eq!(
        brief(&parsed(&apply_mapping(&t, &m))),
        vec![
            row(
                date!(2026 - 10 - 01),
                "サンプルマート",
                Direction::Out,
                1650
            ),
            row(
                date!(2026 - 10 - 02),
                "デモオンライン",
                Direction::Out,
                3300
            ),
            row(
                date!(2026 - 10 - 05),
                "デモオンライン 返品",
                Direction::In,
                3300
            ),
        ]
    );
}

// AC-04.2: 引用符の中のカンマ・二重の引用符・改行を読む。行番号は、レコードが始まった行。
#[test]
fn quotes_and_newlines() {
    let t = table(QUOTED);
    assert_eq!(
        t.rows.iter().map(|r| r.line).collect::<Vec<_>>(),
        vec![1, 2, 3, 4, 6]
    );
    assert_eq!(t.rows[1].cells[1], "サンプル商店, 本店");
    assert_eq!(t.rows[2].cells[1], "デモ\"引用\"店");
    assert_eq!(t.rows[3].cells[1], "二行の\r\n摘要");
    let m = guess_mapping(&t).unwrap();
    let descriptions: Vec<String> = parsed(&apply_mapping(&t, &m))
        .iter()
        .map(|r| r.description.clone())
        .collect();
    assert_eq!(
        descriptions,
        vec![
            "サンプル商店, 本店",
            "デモ\"引用\"店",
            "二行の 摘要",
            "ふつうの行"
        ]
    );
}

// AC-04.2: 閉じていない引用符は、行番号つきの日本語で断る。
#[test]
fn broken_quote_is_rejected_with_line() {
    let e = parse_csv(&decode(BROKEN, EncodingChoice::Auto).unwrap().text).unwrap_err();
    assert_eq!(e, ImportError::UnclosedQuote { line: 3 });
    assert_eq!((e.code(), e.line()), ("invalid_csv", Some(3)));
    assert!(e.message().starts_with("3行目"), "{}", e.message());
}

// AC-04.4: 同じファイルを2回読んでも、（指紋, 出現番号）の組は同じ。重なった分は、同じ組になる。
#[test]
fn same_file_twice_gives_same_pairs() {
    let read = |bytes: &[u8]| -> Vec<(String, u32)> {
        let t = table(bytes);
        let m = guess_mapping(&t).unwrap();
        let rows: Vec<ParsedRow> = parsed(&apply_mapping(&t, &m))
            .into_iter()
            .cloned()
            .collect();
        number_occurrences("w03", &rows)
    };
    assert_eq!(read(UTF8), read(UTF8));
    // 後半の2行だけを含む、期間が重なる別のファイル。
    let overlap = "日付,摘要,出金,入金\n2026/10/05,振込 サンプル商事,,\"55,000\"\n2026/10/06,新しい行 デモ,100,\n";
    let first: std::collections::HashSet<(String, u32)> = read(UTF8).into_iter().collect();
    let second = read(overlap.as_bytes());
    assert!(first.contains(&second[0]));
    assert!(!first.contains(&second[1]));
}
