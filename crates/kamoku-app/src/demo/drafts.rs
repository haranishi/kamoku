// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 未確認の明細（25件）。合成のカードの明細を、本物と同じ取込の流れで2回取り込む。
//! 2回目は1回目と5行が重なり、その5行は重複として入らない。1回目には、同じ日・同じ店・同じ金額の
//! 本物の2件（出現番号1と2）を入れてある。科目は空のまま。

use super::{Demo, DemoError, day, fail};
use crate::api::commit_table;
use kamoku_import::{EncodingChoice, decode, guess_mapping, parse_csv};
use std::collections::HashSet;

const SHOPS: [&str; 6] = [
    "サンプルストア",
    "デモ書店",
    "サンプル交通",
    "デモカフェ",
    "サンプルスーパー",
    "デモ電器",
];

pub(super) fn import(demo: &mut Demo<'_>) -> Result<(), DemoError> {
    let mut rows = Vec::new();
    for i in 0..25u8 {
        let shop = SHOPS
            .get(usize::from(i) % SHOPS.len())
            .copied()
            .unwrap_or_default();
        let amount = demo.rng.amount(300, 9_800, 10);
        rows.push(format!("2026/09/{:02},{shop},{amount}", 16 + i / 2 % 15));
    }
    if let Some(copy) = rows.get(2).cloned()
        && let Some(slot) = rows.get_mut(3)
    {
        *slot = copy;
    }
    let header = "ご利用日,ご利用先,ご利用金額";
    let first = rows.iter().take(15).cloned().collect::<Vec<_>>();
    let second = rows.iter().skip(10).take(15).cloned().collect::<Vec<_>>();
    for (name, lines) in [
        ("sample-card-2026-09a.csv", first),
        ("sample-card-2026-09b.csv", second),
    ] {
        let text = format!("{header}\r\n{}\r\n", lines.join("\r\n"));
        let decoded = decode(text.as_bytes(), EncodingChoice::Auto).map_err(fail)?;
        let table = parse_csv(&decoded.text).map_err(fail)?;
        let mapping = guess_mapping(&table).ok_or_else(|| DemoError("列の対応".to_owned()))?;
        let at = demo.tick(day(2026, 9, 30)?);
        let card = demo.card.clone();
        commit_table(
            demo.store,
            &table,
            &card,
            &mapping,
            name,
            HashSet::new(),
            at,
        )
        .map_err(|e| fail(e.message))?;
    }
    Ok(())
}
