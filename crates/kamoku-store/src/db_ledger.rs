// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{StoreError, VerifyFailure};
use kamoku_ledger::{AccountCode, Ledger, Line, Side, Yen};
use rusqlite::Connection;
use std::collections::BTreeMap;

const SELECT_EVENTS: &str = "
    SELECT seq, kind, entry_id, revision, recorded_at, reason,
        date, description, prev_hash, hash
    FROM ledger_events ORDER BY seq";

pub(crate) struct Loaded {
    pub ledger: Ledger,
    pub count: u64,
    pub failure: Option<VerifyFailure>,
}
pub(crate) fn convert<T, E: std::fmt::Display>(result: Result<T, E>) -> Result<T, StoreError> {
    result.map_err(|e| StoreError::Invalid(e.to_string()))
}
pub(crate) type EventGroups<T> = BTreeMap<u64, Result<Vec<T>, String>>;

struct Grouped<T> {
    groups: EventGroups<T>,
    orphan: Option<String>,
}
// 復元失敗はイベントに結び付ける。連番すら読めない行は孤立行として別に記録する。
fn read_groups<T>(
    conn: &Connection,
    query: &str,
    restore: impl Fn(&rusqlite::Row<'_>, usize) -> Result<T, StoreError>,
) -> Result<Grouped<T>, StoreError> {
    let mut stmt = conn.prepare(query)?;
    let mut rows = stmt.query([])?;
    let mut out = Grouped {
        groups: BTreeMap::new(),
        orphan: None,
    };
    while let Some(row) = rows.next()? {
        let seq = match crate::sql::unsigned(row, 0) {
            Ok(seq) => seq,
            Err(error) => {
                out.orphan
                    .get_or_insert_with(|| format!("イベントに結び付かない行: {error}"));
                continue;
            }
        };
        let group = out.groups.entry(seq).or_insert_with(|| Ok(Vec::new()));
        if let Ok(values) = group {
            match restore(row, values.len()) {
                Ok(value) => values.push(value),
                Err(error) => *group = Err(error.to_string()),
            }
        }
    }
    Ok(out)
}
fn line_groups(conn: &Connection) -> Result<Grouped<Line>, StoreError> {
    const SELECT_LINES: &str = "
        SELECT seq, position, account, side, amount, tax_code, tax_amount, memo
        FROM ledger_lines ORDER BY seq, position";
    read_groups(conn, SELECT_LINES, |row, position| {
        if row.get::<_, u32>(1)? as usize != position {
            return Err(StoreError::Invalid("明細順の不連続".into()));
        }
        let code: String = row.get(2)?;
        let side: String = row.get(3)?;
        let amount: i64 = row.get(4)?;
        if amount > kamoku_books::MAX_AMOUNT {
            return Err(StoreError::AmountRange);
        }
        Ok(Line {
            account: convert(AccountCode::new(&code))?,
            side: match side.as_str() {
                "debit" => Side::Debit,
                "credit" => Side::Credit,
                _ => return Err(StoreError::Invalid("借貸の形".into())),
            },
            amount: Yen::new(amount),
            tax_code: row.get(5)?,
            tax_amount: Yen::new(row.get(6)?),
            memo: row.get(7)?,
        })
    })
}
fn meta_groups(conn: &Connection) -> Result<Grouped<(String, String)>, StoreError> {
    read_groups(
        conn,
        "SELECT seq,key,value FROM ledger_meta ORDER BY seq,key",
        |row, _| Ok((row.get(1)?, row.get(2)?)),
    )
}
pub(crate) fn load(conn: &Connection) -> Result<Loaded, StoreError> {
    let count = conn.query_row("SELECT count(*) FROM ledger_events", [], |r| {
        crate::sql::unsigned(r, 0)
    })?;
    let lines = line_groups(conn)?;
    let metas = meta_groups(conn)?;
    let orphan = lines.orphan.or(metas.orphan);
    let mut lines = lines.groups;
    let mut metas = metas.groups;
    let mut stmt = conn.prepare(SELECT_EVENTS)?;
    let mut rows = stmt.query([])?;
    let mut events = Vec::new();
    let mut failure = None;
    while let Some(row) = rows.next()? {
        let expected = u64::try_from(events.len()).map_err(|_| StoreError::AmountRange)? + 1;
        let seq = crate::sql::unsigned(row, 0).unwrap_or(expected);
        if seq != expected {
            failure = Some(VerifyFailure {
                seq: expected,
                kind: "wrong_sequence".into(),
                rule: Some(format!("期待した連番{expected}、実際の連番{seq}")),
            });
            break;
        }
        let result = crate::db_event::reconstruct(row, &mut lines, &mut metas);
        match result {
            Ok(e) => events.push(e),
            Err(e) => {
                failure = Some(VerifyFailure::row(seq, e.to_string()));
                break;
            }
        }
    }
    // 一度だけ全件検証し、失敗した時だけ接頭部分を再検証する。正常時は線形で読み込む。
    let ledger = match Ledger::from_events(events.clone()) {
        Ok(ledger) => ledger,
        Err(e) => {
            let f = VerifyFailure::from(e);
            events.retain(|e| e.seq < f.seq);
            failure = Some(f);
            Ledger::from_events(events).map_err(|e| StoreError::Invalid(e.to_string()))?
        }
    };
    if failure.is_none() && (orphan.is_some() || !lines.is_empty() || !metas.is_empty()) {
        failure = Some(VerifyFailure::row(
            count + 1,
            orphan.unwrap_or_else(|| "イベントのない明細または付随情報".into()),
        ));
    }
    Ok(Loaded {
        ledger,
        count,
        failure,
    })
}
