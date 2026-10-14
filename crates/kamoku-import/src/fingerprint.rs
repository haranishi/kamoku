// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 重複の判定に使う指紋（`docs/spec/data-model.md` の決まり4）。
//!
//! ```text
//! lp(s) = u32 big-endian(UTF-8バイト数) + UTF-8バイト列
//! 取引番号あり: SHA-256(lp("kamoku.import.id.v1") + lp(口座) + lp(正規化した取引番号))
//! 取引番号なし: SHA-256(lp("kamoku.import.row.v1") + lp(口座) + lp(日付 YYYY-MM-DD)
//!                       + lp(符号つき金額の10進) + lp(正規化した摘要))
//! ```
//! 長さを前に付けるのは、欄の境目をずらした別の行と、同じバイト列にならないようにするため。

use crate::ParsedRow;
use crate::values::normalize_text;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fmt::Write as _;

fn lp(hasher: &mut Sha256, s: &str) {
    // 欄は64KBまでなので、長さは u32 に必ず収まる。
    hasher.update(u32::try_from(s.len()).unwrap_or(u32::MAX).to_be_bytes());
    hasher.update(s.as_bytes());
}

/// 行の指紋（SHA-256の小文字16進64桁）。取引番号があれば「口座 + 取引番号」、無ければ
/// 「口座・日付・符号つき金額・正規化した摘要」から作ります。
#[must_use]
pub fn fingerprint(wallet: &str, row: &ParsedRow) -> String {
    let mut h = Sha256::new();
    let id = row
        .id
        .as_deref()
        .map(normalize_text)
        .filter(|id| !id.is_empty());
    if let Some(id) = id {
        lp(&mut h, "kamoku.import.id.v1");
        lp(&mut h, wallet);
        lp(&mut h, &id);
    } else {
        let date = row.date;
        let day = format!(
            "{:04}-{:02}-{:02}",
            date.year(),
            u8::from(date.month()),
            date.day()
        );
        lp(&mut h, "kamoku.import.row.v1");
        lp(&mut h, wallet);
        lp(&mut h, &day);
        lp(&mut h, &row.signed_amount().to_string());
        lp(&mut h, &normalize_text(&row.description));
    }
    let mut out = String::with_capacity(64);
    for b in h.finalize() {
        let _ = write!(out, "{b:02x}");
    }
    out
}

/// 各行の（指紋, 出現番号）。同じ指紋の行に、ファイルの中の順で 1, 2, … を振ります。
/// 同じ日・同じ金額・同じ摘要の取引が本当に2件あっても、2件とも別の組になります（AC-04.5）。
#[must_use]
pub fn number_occurrences(wallet: &str, rows: &[ParsedRow]) -> Vec<(String, u32)> {
    let mut seen = HashMap::<String, u32>::new();
    rows.iter()
        .map(|row| {
            let print = fingerprint(wallet, row);
            let count = seen.entry(print.clone()).or_insert(0);
            *count = count.saturating_add(1);
            (print, *count)
        })
        .collect()
}
