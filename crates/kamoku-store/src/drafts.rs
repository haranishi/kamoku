// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{
    ConfirmResult, Draft, DraftAction, DraftCounts, DraftFilter, DraftId, DraftPatch, DraftStatus,
    NewPartner, Page, Store, StoreError, masters,
};
use kamoku_ledger::{Entry, EntryId, EventKind};
use rusqlite::params;
use serde_json::json;

const UPDATE_DRAFT: &str = "
    UPDATE drafts SET date=?, account=?, tax_code=?, partner=?, memo=?, business_pct=?
    WHERE id=?";

pub(crate) const DRAFT_SELECT: &str = "
    SELECT id, batch_id, wallet, date, description, direction, amount,
        fingerprint, occurrence, external_id, account, tax_code, partner,
        memo, business_pct, status, entry_id
    FROM drafts";
const DRAFT_FILTER: &str = "
    WHERE (?1 IS NULL OR status=?1)
        AND (?2 IS NULL OR wallet=?2)
        AND (?3 IS NULL OR instr(description,?3)>0
            OR instr(partner,?3)>0 OR instr(memo,?3)>0)
        AND (?4 IS NULL OR date>=?4)
        AND (?5 IS NULL OR date<=?5)";
pub(crate) fn draft_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Draft> {
    let status: String = r.get(15)?;
    let status = match status.as_str() {
        "draft" => DraftStatus::Draft,
        "ignored" => DraftStatus::Ignored,
        "confirmed" => DraftStatus::Confirmed,
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    Ok(Draft {
        id: r.get(0)?,
        batch_id: r.get(1)?,
        wallet: r.get(2)?,
        date: r.get(3)?,
        description: r.get(4)?,
        direction: r.get(5)?,
        amount: r.get(6)?,
        fingerprint: r.get(7)?,
        occurrence: r.get(8)?,
        external_id: r.get(9)?,
        account: r.get(10)?,
        tax_code: r.get(11)?,
        partner: r.get(12)?,
        memo: r.get(13)?,
        business_pct: r.get(14)?,
        status,
        entry_id: r.get(16)?,
    })
}
impl Store {
    /// 状態、口座、部分文字列、日付で絞った下書きのページ。
    ///
    /// # Errors
    /// 不正な件数・期間または`SQLite`の読み取りの失敗。
    // 操作の入力は、仕訳や変更パッチと同じく所有権を受け取る公開契約にそろえる。
    #[allow(clippy::needless_pass_by_value)]
    pub fn drafts(&self, filter: DraftFilter) -> Result<Page<Draft>, StoreError> {
        if !(1..=500).contains(&filter.limit) {
            return Err(StoreError::Invalid("件数は1〜500にしてください".into()));
        }
        if let Some(d) = &filter.from {
            masters::date(d)?;
        }
        if let Some(d) = &filter.to {
            masters::date(d)?;
        }
        if filter
            .from
            .as_ref()
            .zip(filter.to.as_ref())
            .is_some_and(|(f, t)| f > t)
        {
            return Err(StoreError::Invalid("期間の開始が終了より後です".into()));
        }

        let status = filter.status.map(DraftStatus::as_str);
        let total = self.conn.query_row(
            &format!(
                "
                SELECT count(*) FROM drafts {DRAFT_FILTER}"
            ),
            params![status, filter.wallet, filter.q, filter.from, filter.to],
            |r| crate::sql::unsigned(r, 0),
        )?;
        let mut stmt = self.conn.prepare(&format!(
            "{DRAFT_SELECT} {DRAFT_FILTER} ORDER BY date,id LIMIT ?6 OFFSET ?7"
        ))?;
        let items = stmt
            .query_map(
                params![
                    status,
                    filter.wallet,
                    filter.q,
                    filter.from,
                    filter.to,
                    filter.limit,
                    crate::sql::integer(filter.offset)?
                ],
                draft_row,
            )?
            .collect::<Result<_, _>>()?;
        Ok(Page { items, total })
    }
    /// 状態ごとの全件数。
    ///
    /// # Errors
    /// `SQLite`の読み取りの失敗。
    pub fn draft_counts(&self) -> Result<DraftCounts, StoreError> {
        let mut result = DraftCounts::default();
        let mut stmt = self.conn.prepare(
            "
                SELECT status,count(*) FROM drafts GROUP BY status",
        )?;
        for r in stmt.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, crate::sql::unsigned(r, 1)?))
        })? {
            let (status, count) = r?;
            match status.as_str() {
                "draft" => result.draft = count,
                "ignored" => result.ignored = count,
                "confirmed" => result.confirmed = count,
                _ => return Err(StoreError::Invalid("下書きの状態".into())),
            }
        }
        Ok(result)
    }
    /// 未確認行の編集。指紋・出現番号・取引番号は変えません。
    ///
    /// # Errors
    /// 対象なし、確定済み、無視済み、日付・割合・科目・税区分または保存の失敗。
    pub fn update_draft(
        &mut self,
        id: DraftId,
        patch: DraftPatch,
        now: i64,
    ) -> Result<Draft, StoreError> {
        self.atomic(|s| {
            let mut draft = s.get_draft(id)?;
            if draft.status == DraftStatus::Confirmed {
                return Err(StoreError::AlreadyConfirmed);
            }
            if draft.status != DraftStatus::Draft {
                return Err(StoreError::Invalid(
                    "未確認に戻してから編集してください".into(),
                ));
            }
            let before = json!(draft);
            if let Some(value) = patch.date {
                masters::date(&value)?;
                draft.date = value;
            }
            if let Some(value) = patch.account {
                if let Some(account) = &value {
                    s.account_check(account)?;
                }
                draft.account = value;
            }
            if let Some(value) = patch.tax_code {
                if value
                    .as_deref()
                    .is_some_and(|t| kamoku_books::TaxCode::parse(t).is_none())
                {
                    return Err(StoreError::BadTaxCode);
                }
                draft.tax_code = value;
            }
            if let Some(value) = patch.partner {
                draft.partner = value;
            }
            if let Some(value) = patch.memo {
                if value.len() > 1000 {
                    return Err(StoreError::Invalid(
                        "メモは1000バイト以下にしてください".into(),
                    ));
                }
                draft.memo = value;
            }
            if let Some(value) = patch.business_pct {
                if value > 100 {
                    return Err(StoreError::Invalid("割合は0〜100にしてください".into()));
                }
                draft.business_pct = value;
            }
            s.conn.execute(
                UPDATE_DRAFT,
                params![
                    draft.date,
                    draft.account,
                    draft.tax_code,
                    draft.partner,
                    draft.memo,
                    draft.business_pct,
                    id
                ],
            )?;
            s.log(
                "draft.update",
                &json!({"before": before, "after": draft}),
                now,
            )?;
            Ok(draft)
        })
    }

    /// 無視または復元。確定済みは、対応する仕訳が取消済みのときだけ復元できます。
    ///
    /// # Errors
    /// 対象なし、確定済み、状態の不一致または保存の失敗。
    pub fn set_draft_status(
        &mut self,
        id: DraftId,
        action: DraftAction,
        now: i64,
    ) -> Result<(), StoreError> {
        self.atomic(|s| {
            let d = s.get_draft(id)?;
            let status = match (d.status, action) {
                (DraftStatus::Draft, DraftAction::Ignore) => "ignored",
                (DraftStatus::Ignored, DraftAction::Restore) => "draft",
                (DraftStatus::Confirmed, DraftAction::Restore) => {
                    let cancelled = d
                        .entry_id
                        .as_deref()
                        .and_then(|v| EntryId::new(v).ok())
                        .is_some_and(|id| {
                            s.ledger
                                .history(&id)
                                .last()
                                .is_some_and(|e| e.kind == EventKind::Voided)
                        });
                    if !cancelled {
                        return Err(StoreError::AlreadyConfirmed);
                    }
                    "draft"
                }
                (DraftStatus::Confirmed, _) => return Err(StoreError::AlreadyConfirmed),
                _ => return Err(StoreError::Invalid("下書きの状態を確認してください".into())),
            };
            s.conn.execute(
                "
                    UPDATE drafts SET status=?,entry_id=NULL WHERE id=?",
                params![status, id],
            )?;
            s.log(
                "draft.status",
                &json!({"id":id,"status":status,"previous_entry_id":d.entry_id}),
                now,
            )
        })
    }
    /// 行ごとに検査し、通った仕訳・下書き・取引先・履歴を同じトランザクションで保存します。
    /// `source=import` と `draft=<id>` を必ず付けます。
    ///
    /// # Errors
    /// DBへの保存失敗は全体のErr。入力の失敗は各 `ConfirmResult` に返します。
    pub fn confirm_drafts(
        &mut self,
        inputs: Vec<(DraftId, Entry)>,
        now: i64,
    ) -> Result<Vec<ConfirmResult>, StoreError> {
        self.atomic(|s| {
            let mut results = Vec::new();
            for (id, entry) in inputs {
                let validation = (|| {
                    let d = s.get_draft(id)?;
                    if d.status == DraftStatus::Confirmed {
                        return Err(StoreError::AlreadyConfirmed);
                    }
                    if d.status != DraftStatus::Draft {
                        return Err(StoreError::Invalid(
                            "未確認に戻してから確定してください".into(),
                        ));
                    }
                    let account = d.account.as_deref().ok_or(StoreError::MissingAccount)?;
                    s.account_check(account)?;
                    s.wallet_check(&d.wallet)?;
                    s.match_draft(&d, &entry)?;
                    if !s.ledger.history(entry.id()).is_empty() {
                        return Err(
                            kamoku_ledger::LedgerError::DuplicateId(entry.id().clone()).into()
                        );
                    }
                    let entry = entry
                        .with_meta("draft", &id.to_string())?
                        .with_meta("source", "import")?;
                    s.entry_check(&entry)?;
                    let partner = if d.partner.is_empty() {
                        None
                    } else {
                        masters::name(&d.partner)?;
                        Some(NewPartner {
                            name: d.partner.clone(),
                            invoice_number: entry.meta().get("invoice_no").cloned(),
                            ..NewPartner::default()
                        })
                    };
                    Ok((entry, partner))
                })();
                match validation {
                    Err(error) => results.push(ConfirmResult {
                        id,
                        result: Err(error),
                    }),
                    Ok((entry, partner)) => {
                        let reference = s.post_inside(entry, now, partner.as_ref())?;
                        s.conn.execute(
                            "
                                UPDATE drafts SET status='confirmed',entry_id=? WHERE id=?",
                            params![reference.entry_id, id],
                        )?;
                        s.log(
                            "draft.confirm",
                            &json!({"id":id,"entry_id":reference.entry_id}),
                            now,
                        )?;
                        results.push(ConfirmResult {
                            id,
                            result: Ok(reference),
                        });
                    }
                }
            }
            Ok(results)
        })
    }
}
