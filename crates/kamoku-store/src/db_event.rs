// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::db_ledger::EventGroups;
use crate::{StoreError, db_ledger::convert};
use kamoku_ledger::{Entry, EntryId, Event, EventKind, Hash, Line, Side};
use rusqlite::{Connection, params};

pub(crate) fn reconstruct(
    row: &rusqlite::Row<'_>,
    lines: &mut EventGroups<Line>,
    metas: &mut EventGroups<(String, String)>,
) -> Result<Event, StoreError> {
    let seq = crate::sql::unsigned(row, 0)?;
    let kind = match row.get::<_, u8>(1)? {
        1 => EventKind::Posted,
        2 => EventKind::Revised,
        3 => EventKind::Voided,
        _ => return Err(StoreError::Invalid("イベント種別".into())),
    };
    let id: String = row.get(2)?;
    let entry_id = convert(EntryId::new(&id))?;
    let ls = lines
        .remove(&seq)
        .transpose()
        .map_err(StoreError::Invalid)?
        .unwrap_or_default();
    let ms = metas
        .remove(&seq)
        .transpose()
        .map_err(StoreError::Invalid)?
        .unwrap_or_default();
    let date: Option<String> = row.get(6)?;
    let description: Option<String> = row.get(7)?;
    let entry = if kind == EventKind::Voided {
        if !ls.is_empty() || !ms.is_empty() || date.is_some() || description.is_some() {
            return Err(StoreError::Invalid("取消に仕訳が存在".into()));
        }
        None
    } else {
        let date = date
            .as_deref()
            .and_then(kamoku_books::parse_date)
            .ok_or_else(|| StoreError::Invalid("日付を復元できません".into()))?;
        let description =
            description.ok_or_else(|| StoreError::Invalid("摘要がありません".into()))?;
        let mut entry = Entry::new(entry_id.clone(), date, &description, ls)?;
        for (k, v) in ms {
            entry = entry.with_meta(&k, &v)?;
        }
        Some(entry)
    };
    Ok(Event {
        seq,
        kind,
        entry_id,
        revision: row.get(3)?,
        recorded_at: row.get(4)?,
        reason: row.get(5)?,
        entry,
        prev_hash: convert(Hash::from_hex(&row.get::<_, String>(8)?))?,
        hash: convert(Hash::from_hex(&row.get::<_, String>(9)?))?,
    })
}
pub(crate) fn insert(conn: &Connection, e: &Event) -> Result<(), StoreError> {
    let kind = match e.kind {
        EventKind::Posted => 1,
        EventKind::Revised => 2,
        EventKind::Voided => 3,
        _ => return Err(StoreError::Invalid("イベント種別".into())),
    };
    let date = e
        .entry
        .as_ref()
        .map(|e| kamoku_books::format_date(e.date()));
    conn.execute(
        "INSERT INTO ledger_events (
            seq, kind, entry_id, revision, recorded_at, reason, date, description, prev_hash, hash
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![
            crate::sql::integer(e.seq)?,
            kind,
            e.entry_id.as_str(),
            e.revision,
            e.recorded_at,
            e.reason,
            date,
            e.entry.as_ref().map(Entry::description),
            e.prev_hash.to_hex(),
            e.hash.to_hex()
        ],
    )?;
    if let Some(entry) = &e.entry {
        let mut stmt = conn.prepare_cached(
            "INSERT INTO ledger_lines (
                seq, position, account, side, amount, tax_code, tax_amount, memo
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )?;
        for (pos, line) in entry.lines().iter().enumerate() {
            stmt.execute(params![
                crate::sql::integer(e.seq)?,
                u32::try_from(pos).map_err(|_| StoreError::AmountRange)?,
                line.account.as_str(),
                match line.side {
                    Side::Debit => "debit",
                    Side::Credit => "credit",
                },
                line.amount.get(),
                line.tax_code,
                line.tax_amount.get(),
                line.memo
            ])?;
        }
        let mut stmt = conn.prepare_cached(
            "
                INSERT INTO ledger_meta (seq, key, value) VALUES (?, ?, ?)",
        )?;
        for (k, v) in entry.meta() {
            stmt.execute(params![crate::sql::integer(e.seq)?, k, v])?;
        }
    }
    Ok(())
}
