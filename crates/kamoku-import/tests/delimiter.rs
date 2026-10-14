// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 区切りの判定。引用符の崩れが少ない方を選ぶ。
//! 閉じた引用符のあとを厳しく読むようにした後も、崩れの数で同点を決める処理が
//! 試験から見えなくなっていたため（変異試験で生き残った）、1つずつ固定する。

use kamoku_import::{Delimiter, parse_csv};

fn cells(table: &kamoku_import::Table, row: usize) -> Vec<&str> {
    table.rows[row].cells.iter().map(String::as_str).collect()
}

// 引用符で囲んだ欄の中のカンマを、区切りと取り違えない。
// カンマで読むと、欄の途中に引用符が出る崩れが4つ。セミコロンなら0。
#[test]
fn quoted_commas_do_not_make_comma_the_delimiter() {
    let table = parse_csv("x;\"a,b\"\ny;\"c,d\"\n").unwrap();
    assert_eq!(table.delimiter, Delimiter::Semicolon);
    assert_eq!(cells(&table, 0), ["x", "a,b"]);
    assert_eq!(cells(&table, 1), ["y", "c,d"]);
}

// タブ区切りで、引用符で囲んだ欄の中にセミコロンがあっても、タブを選ぶ。
#[test]
fn quoted_semicolons_do_not_make_semicolon_the_delimiter() {
    let table = parse_csv("x\t\"a;b\"\ny\t\"c;d\"\n").unwrap();
    assert_eq!(table.delimiter, Delimiter::Tab);
    assert_eq!(cells(&table, 0), ["x", "a;b"]);
}

// 崩れが無く、欄の数がそろうなら、同点の順（カンマ、タブ、セミコロン）で決まる。
#[test]
fn clean_comma_file_stays_comma() {
    let table = parse_csv("a,b,c\n1,2,3\n").unwrap();
    assert_eq!(table.delimiter, Delimiter::Comma);
    assert_eq!(cells(&table, 1), ["1", "2", "3"]);
}
