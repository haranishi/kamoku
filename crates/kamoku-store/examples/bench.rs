// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! AC-09: 10万イベントの開く時間と1件追記を測る。合成データ専用。
use kamoku_ledger::{AccountCode, Entry, EntryId, Line, Side, Yen};
use kamoku_store::{OpenOptions, Store};
use std::time::Instant;
use time::macros::date;

fn entry(index: u64) -> Result<Entry, Box<dyn std::error::Error>> {
    Ok(Entry::new(
        EntryId::new(&format!("demo_{index}"))?,
        date!(2026 - 10 - 01),
        "デモの通信費",
        vec![
            Line::new(AccountCode::new("exp_comm")?, Side::Debit, Yen::new(110)),
            Line::new(AccountCode::new("cash")?, Side::Credit, Yen::new(110)),
        ],
    )?)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("benchmark.sqlite");
    let (mut store, _) = Store::open(
        &path,
        OpenOptions {
            create_if_missing: true,
        },
    )?;
    let start = Instant::now();
    for chunk in 0..100 {
        let entries = (chunk * 1000..(chunk + 1) * 1000)
            .map(|i| entry(i).map(|e| (e, None)))
            .collect::<Result<Vec<_>, _>>()?;
        store.post_many(entries, 1)?;
    }
    println!("100000イベント作成: {:?}", start.elapsed());
    drop(store);
    let start = Instant::now();
    let (mut store, report) = Store::open(&path, OpenOptions::default())?;
    println!(
        "open全体: {:?}, LoadReport: {:?}, events={}",
        start.elapsed(),
        report.elapsed,
        report.events_loaded
    );
    let start = Instant::now();
    store.post(entry(100_000)?, 2, None)?;
    println!("1件追記: {:?}", start.elapsed());
    assert_eq!(store.ledger().events().len(), 100_001);
    Ok(())
}
