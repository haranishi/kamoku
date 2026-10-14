// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{
    ImportBatch, ImportSummary, KnownFingerprints, NewBatch, NewDraft, Store, StoreError, masters,
};
use rusqlite::{OptionalExtension, params};
use serde_json::json;
use std::collections::HashSet;

const INSERT_BATCH: &str = "
    INSERT INTO import_batches (
        filename, wallet, recorded_at, created, duplicates, invalid
    ) VALUES (?, ?, ?, 0, 0, ?)";

impl Store {
    /// 存在する指紋と出現番号、各指紋の最大出現番号を返します。
    ///
    /// # Errors
    /// `SQLite`の読み取りの失敗。
    pub fn known_fingerprints(
        &self,
        keys: &[(String, u32)],
    ) -> Result<KnownFingerprints, StoreError> {
        let mut result = KnownFingerprints::default();
        let mut max = self.conn.prepare_cached(
            "
                SELECT coalesce(max(occurrence),0) FROM drafts WHERE fingerprint=?",
        )?;
        let mut exists = self.conn.prepare_cached(
            "
                SELECT EXISTS(SELECT 1 FROM drafts WHERE fingerprint=? AND occurrence=?)",
        )?;
        for (fingerprint, occurrence) in keys {
            result.maxima.insert(
                fingerprint.clone(),
                max.query_row([fingerprint], |r| r.get(0))?,
            );
            if exists.query_row(params![fingerprint, occurrence], |r| r.get::<_, bool>(0))? {
                result.existing.insert((fingerprint.clone(), *occurrence));
            }
        }
        Ok(result)
    }
    /// 取込記録と行を一括保存します。強制指定はファイル内の1始まりの行番号です。
    ///
    /// # Errors
    /// 無効な口座・行の形、読み取り専用、Staleまたは保存の失敗。
    // 操作の入力は、仕訳や変更パッチと同じく所有権を受け取る公開契約にそろえる。
    #[allow(clippy::needless_pass_by_value)]
    pub fn import_commit(
        &mut self,
        batch: NewBatch,
        drafts: Vec<NewDraft>,
        force: HashSet<usize>,
    ) -> Result<ImportSummary, StoreError> {
        self.atomic(|s| {
            s.wallet_check(&batch.wallet)?;
            let mut fresh = Vec::new();
            let mut forced = Vec::new();
            let mut duplicates = 0;
            // 挿入前の状態で分類し、新規の行に必要な番号を強制行が消費しないようにする。
            for draft in drafts {
                check_draft(&draft)?;
                if draft_exists(&s.conn, &draft)? {
                    if force.contains(&draft.line) {
                        forced.push(draft);
                    } else {
                        duplicates += 1;
                    }
                } else {
                    fresh.push(draft);
                }
            }
            s.conn.execute(
                INSERT_BATCH,
                params![
                    batch.filename,
                    batch.wallet,
                    batch.recorded_at,
                    crate::sql::integer(batch.invalid)?
                ],
            )?;
            let mut summary = ImportSummary {
                batch_id: s.conn.last_insert_rowid(),
                created: 0,
                duplicates,
                invalid: batch.invalid,
            };
            for draft in fresh {
                insert_draft(&s.conn, &batch.wallet, &draft, &mut summary)?;
            }
            for mut draft in forced {
                draft.occurrence = next_occurrence(&s.conn, &draft.fingerprint)?;
                insert_draft(&s.conn, &batch.wallet, &draft, &mut summary)?;
            }
            s.conn.execute(
                "
                    UPDATE import_batches SET created=?, duplicates=? WHERE id=?",
                params![
                    crate::sql::integer(summary.created)?,
                    crate::sql::integer(summary.duplicates)?,
                    summary.batch_id
                ],
            )?;
            s.log(
                "import.commit",
                &json!({"summary": summary, "filename": batch.filename,
                    "wallet": batch.wallet, "force_lines": force}),
                batch.recorded_at,
            )?;
            Ok(summary)
        })
    }

    /// 取込記録を新しい順で返します。
    ///
    /// # Errors
    /// 範囲外の件数または`SQLite`の失敗。
    pub fn import_batches(&self, limit: u32) -> Result<Vec<ImportBatch>, StoreError> {
        if !(1..=500).contains(&limit) {
            return Err(StoreError::Invalid("件数は1〜500にしてください".into()));
        }
        Ok(self
            .conn
            .prepare(
                "SELECT id, created, duplicates, invalid, filename, wallet, recorded_at
                 FROM import_batches ORDER BY id DESC LIMIT ?",
            )?
            .query_map([limit], |r| {
                Ok(ImportBatch {
                    summary: ImportSummary {
                        batch_id: r.get(0)?,
                        created: crate::sql::unsigned(r, 1)?,
                        duplicates: crate::sql::unsigned(r, 2)?,
                        invalid: crate::sql::unsigned(r, 3)?,
                    },
                    filename: r.get(4)?,
                    wallet: r.get(5)?,
                    recorded_at: r.get(6)?,
                })
            })?
            .collect::<Result<_, _>>()?)
    }
    pub(crate) fn get_draft(&self, id: crate::DraftId) -> Result<crate::Draft, StoreError> {
        self.conn
            .query_row(
                &format!("{} WHERE id=?", crate::drafts::DRAFT_SELECT),
                [id],
                crate::drafts::draft_row,
            )
            .optional()?
            .ok_or(StoreError::NotFound)
    }
}

fn check_draft(draft: &NewDraft) -> Result<(), StoreError> {
    masters::date(&draft.date)?;
    if draft.amount <= 0 || draft.amount > kamoku_books::MAX_AMOUNT {
        return Err(StoreError::AmountRange);
    }
    if draft.occurrence == 0
        || draft.line == 0
        || !matches!(draft.direction.as_str(), "in" | "out")
        || kamoku_ledger::Hash::from_hex(&draft.fingerprint).is_err()
    {
        return Err(StoreError::Invalid("取込の行の形が違います".into()));
    }
    Ok(())
}
fn draft_exists(conn: &rusqlite::Connection, draft: &NewDraft) -> Result<bool, StoreError> {
    Ok(conn.query_row(
        "
            SELECT EXISTS(SELECT 1 FROM drafts WHERE fingerprint=? AND occurrence=?)",
        params![draft.fingerprint, draft.occurrence],
        |row| row.get(0),
    )?)
}
fn next_occurrence(conn: &rusqlite::Connection, fingerprint: &str) -> Result<u32, StoreError> {
    let max: u32 = conn.query_row(
        "
            SELECT coalesce(max(occurrence),0) FROM drafts WHERE fingerprint=?",
        [fingerprint],
        |row| row.get(0),
    )?;
    max.checked_add(1).ok_or(StoreError::AmountRange)
}
fn insert_draft(
    conn: &rusqlite::Connection,
    wallet: &str,
    draft: &NewDraft,
    summary: &mut ImportSummary,
) -> Result<(), StoreError> {
    const INSERT_DRAFT: &str = "
        INSERT INTO drafts (
            batch_id, wallet, line, date, description, direction, amount,
            fingerprint, occurrence, external_id
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(fingerprint, occurrence) DO NOTHING";
    // 指紋の組以外のCHECK違反は重複として隠さない。
    let changed = conn.execute(
        INSERT_DRAFT,
        params![
            summary.batch_id,
            wallet,
            i64::try_from(draft.line).map_err(|_| StoreError::AmountRange)?,
            draft.date,
            draft.description,
            draft.direction,
            draft.amount,
            draft.fingerprint,
            draft.occurrence,
            draft.external_id
        ],
    )?;
    summary.created += changed as u64;
    summary.duplicates += u64::from(changed == 0);
    Ok(())
}
