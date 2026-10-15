// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Synthetic journal inputs and a full-sort oracle independent of ledger queries.

use kamoku_ledger::{AccountCode, Entry, EntryId, Line, Side, Yen};
use kamoku_store::{OpenOptions, Settings, Store};
use serde_json::{Value, json};
use std::cmp::Reverse;
use std::error::Error;
use std::path::Path;
use time::{Date, Month};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const POSTED_AT: i64 = 1_800_000_000;
const PARTNER: &str = "Synthetic partner";
const VOID_REASON: &str = "Synthetic benchmark void";
const PAGE_SIZE: usize = 50;

struct Row {
    no: u32,
    changes: u32,
    date: Date,
    amount: i64,
    voided: bool,
}

impl Row {
    fn new(no: u32, changes: u32) -> Result<Self> {
        Ok(Self {
            no,
            changes,
            date: Date::from_calendar_date(
                2026,
                Month::try_from(u8::try_from((no + changes * 5) % 12 + 1)?)?,
                u8::try_from((no + changes * 3) % 28 + 1)?,
            )?,
            amount: 100 + i64::from(no % 9_900) + i64::from(changes) * 7,
            voided: false,
        })
    }

    fn id(&self) -> Result<EntryId> {
        Ok(EntryId::new(&format!("journal_bench_{:06}", self.no))?)
    }

    fn description(&self) -> String {
        format!(
            "Synthetic journal needle_{:06}_end revision {}",
            self.no, self.changes
        )
    }

    fn entry(&self) -> Result<Entry> {
        Ok(Entry::new(
            self.id()?,
            self.date,
            &self.description(),
            vec![
                Line::new(
                    AccountCode::new("exp_comm")?,
                    Side::Debit,
                    Yen::new(self.amount),
                ),
                Line::new(
                    AccountCode::new("cash")?,
                    Side::Credit,
                    Yen::new(self.amount),
                ),
            ],
        )?
        .with_meta("kind", "journal")?
        .with_meta("source", "demo")?
        .with_meta("partner", PARTNER)?)
    }

    fn json(&self, names: &[String; 2]) -> Value {
        let reason = if self.voided {
            Some(VOID_REASON.to_owned())
        } else if self.changes > 0 {
            Some(revision_reason(self.changes))
        } else {
            None
        };
        let lines: Vec<_> = [("exp_comm", "debit"), ("cash", "credit")]
            .into_iter()
            .zip(names)
            .map(|((account, side), name)| {
                json!({ "account": account, "account_name": name, "side": side,
                    "amount": self.amount, "tax_code": null, "tax_amount": 0, "memo": "" })
            })
            .collect();
        json!({
            "id": format!("journal_bench_{:06}", self.no), "no": self.no,
            "date": self.date.to_string(), "kind": "journal",
            "description": self.description(), "partner": PARTNER, "total": self.amount,
            "revision": self.changes + 1 + u32::from(self.voided),
            "state": if self.voided { "voided" } else { "active" },
            "recorded_at": POSTED_AT + i64::from(self.changes) + i64::from(self.voided),
            "last_reason": reason, "lines": lines,
            "meta": { "kind": "journal", "source": "demo", "partner": PARTNER },
        })
    }
}

fn revision_reason(changes: u32) -> String {
    format!("Synthetic benchmark revision {changes}")
}

pub(crate) struct Fixture {
    rows: Vec<Row>,
    names: [String; 2],
    pub(crate) events: u64,
    pub(crate) revised: u32,
    pub(crate) voided: u32,
}

pub(crate) struct Case {
    pub(crate) name: &'static str,
    pub(crate) query: String,
    pub(crate) expected: Value,
}

impl Fixture {
    pub(crate) fn build(path: &Path, entries: u32, revisions: u32) -> Result<Self> {
        let (mut store, _) = Store::open(
            path,
            OpenOptions {
                create_if_missing: true,
            },
        )?;
        store.save_settings(
            Settings {
                business_name: "Synthetic benchmark".into(),
                opened_on: "2026-01-01".into(),
                setup_done: true,
                ..Settings::default()
            },
            POSTED_AT,
        )?;
        let names = [
            store
                .chart()
                .name("exp_comm")
                .ok_or("missing account")?
                .into(),
            store.chart().name("cash").ok_or("missing account")?.into(),
        ];
        let mut rows = (1..=entries)
            .map(|no| Row::new(no, 0))
            .collect::<Result<Vec<_>>>()?;
        for chunk in rows.chunks(500) {
            let batch = chunk
                .iter()
                .map(|row| row.entry().map(|entry| (entry, None)))
                .collect::<Result<Vec<_>>>()?;
            store.post_many(batch, POSTED_AT)?;
        }
        for changes in 1..=revisions {
            for row in rows.iter_mut().filter(|row| row.no.is_multiple_of(10)) {
                let updated = Row::new(row.no, changes)?;
                store.revise(
                    updated.entry()?,
                    changes,
                    &revision_reason(changes),
                    POSTED_AT + i64::from(changes),
                    None,
                )?;
                *row = updated;
            }
        }
        let mut voided = 0;
        if revisions > 0 {
            for row in rows.iter_mut().filter(|row| row.no.is_multiple_of(20)) {
                store.void(
                    &row.id()?,
                    revisions + 1,
                    VOID_REASON,
                    POSTED_AT + i64::from(revisions) + 1,
                )?;
                row.voided = true;
                voided += 1;
            }
        }
        let revised = if revisions > 0 { entries / 10 } else { 0 };
        Ok(Self {
            rows,
            names,
            events: u64::from(entries)
                + u64::from(revised) * u64::from(revisions)
                + u64::from(voided),
            revised,
            voided,
        })
    }

    // Deliberately simple: filter and fully sort the generated input table.
    // No Ledger::current/history, derived index, or API presentation code is used.
    fn expected(&self, accepts: impl Fn(&Row) -> bool, offset: usize) -> Result<Value> {
        let mut selected: Vec<_> = self.rows.iter().filter(|row| accepts(row)).collect();
        selected.sort_by_key(|row| Reverse((row.date, row.no)));
        let sum = selected.iter().try_fold(0i64, |total, row| {
            total
                .checked_add(row.amount)
                .ok_or("reference sum overflow")
        })?;
        let items: Vec<_> = selected
            .iter()
            .skip(offset)
            .take(PAGE_SIZE)
            .map(|row| row.json(&self.names))
            .collect();
        Ok(json!({ "items": items, "total": selected.len(), "sum_amount": sum }))
    }

    pub(crate) fn cases(&self, entries: u32) -> Result<Vec<Case>> {
        let late = self.rows.iter().filter(|row| !row.voided).count() - PAGE_SIZE;
        let needle = entries - 10;
        Ok(vec![
            Case {
                name: "first50",
                query: "limit=50&offset=0".into(),
                expected: self.expected(|row| !row.voided, 0)?,
            },
            Case {
                name: "late50",
                query: format!("limit=50&offset={late}"),
                expected: self.expected(|row| !row.voided, late)?,
            },
            Case {
                name: "search_zero",
                query: "limit=50&q=no_such_synthetic_journal_needle".into(),
                expected: self.expected(|_| false, 0)?,
            },
            Case {
                name: "search_one",
                query: format!("limit=50&q=needle_{needle:06}_end"),
                expected: self.expected(|row| !row.voided && row.no == needle, 0)?,
            },
            Case {
                name: "date_filter",
                query: "limit=50&from=2026-07-01&to=2026-07-28".into(),
                expected: self.expected(|row| !row.voided && row.date.month() == Month::July, 0)?,
            },
            Case {
                name: "voided",
                query: "limit=50&state=voided".into(),
                expected: self.expected(|row| row.voided, 0)?,
            },
        ])
    }
}
