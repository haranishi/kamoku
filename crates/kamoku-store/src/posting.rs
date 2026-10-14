// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{NewPartner, PostedRef, Store, StoreError, db_event};
use kamoku_ledger::{Entry, EntryId, Event, EventKind};
use serde_json::json;

// parse_date は4桁の非負の年だけを受け付ける。保存した日付を必ず読み戻すため。
const STORED_YEAR_RANGE: std::ops::RangeInclusive<i32> = 0..=9999;

impl Store {
    pub(crate) fn account_check(&self, code: &str) -> Result<(), StoreError> {
        let a = self.chart.get(code).ok_or(StoreError::UnknownAccount)?;
        if !a.enabled {
            return Err(StoreError::DisabledAccount);
        }
        Ok(())
    }
    pub(crate) fn wallet_check(&self, code: &str) -> Result<(), StoreError> {
        self.account_check(code)?;
        if self.chart.get(code).is_none_or(|a| !a.is_wallet()) {
            return Err(StoreError::Invalid("口座を選んでください".into()));
        }
        Ok(())
    }
    fn locked(&self, date: time::Date) -> Result<(), StoreError> {
        if self
            .settings
            .locked_until
            .as_deref()
            .and_then(kamoku_books::parse_date)
            .is_some_and(|d| date <= d)
        {
            return Err(StoreError::LockedPeriod);
        }
        Ok(())
    }
    pub(crate) fn entry_check(&self, entry: &Entry) -> Result<(), StoreError> {
        if !STORED_YEAR_RANGE.contains(&entry.date().year()) {
            return Err(StoreError::Invalid(
                "日付の年は0〜9999にしてください".into(),
            ));
        }
        self.locked(entry.date())?;
        for l in entry.lines() {
            self.account_check(l.account.as_str())?;
            if let Some(text) = &l.tax_code {
                let code = kamoku_books::TaxCode::parse(text).ok_or(StoreError::BadTaxCode)?;
                if code == kamoku_books::TaxCode::NT && l.tax_amount != kamoku_ledger::Yen::ZERO {
                    return Err(StoreError::BadTaxCode);
                }
            }
            if l.amount.get() > kamoku_books::MAX_AMOUNT {
                return Err(StoreError::AmountRange);
            }
        }
        Ok(())
    }
    fn existing_check(&self, id: &EntryId, revision: u32) -> Result<(), StoreError> {
        let history = self.ledger.history(id);
        let latest = history.last().ok_or(StoreError::NotFound)?;
        if latest.revision != revision {
            return Err(StoreError::StaleRevision);
        }
        if latest.kind == EventKind::Voided {
            return Err(kamoku_ledger::LedgerError::AlreadyVoided(id.clone()).into());
        }
        if let Some(entry) = &latest.entry {
            self.locked(entry.date())?;
        }
        Ok(())
    }
    fn persist_event(
        &mut self,
        event: &Event,
        ensure_partner: Option<&NewPartner>,
    ) -> Result<PostedRef, StoreError> {
        if let Some(p) = ensure_partner {
            self.partner_insert(p, event.recorded_at, true)?;
        }
        db_event::insert(&self.conn, event)?;
        self.log("ledger",&json!({"seq":event.seq,"entry_id":event.entry_id.as_str(),"revision":event.revision,"hash":event.hash.to_hex()}),event.recorded_at)?;
        let no = if event.kind == EventKind::Posted {
            let no = self.numbers.len() as u64 + 1;
            self.numbers.insert(event.entry_id.clone(), no);
            no
        } else {
            self.entry_no(&event.entry_id).ok_or(StoreError::NotFound)?
        };
        Ok(PostedRef {
            entry_id: event.entry_id.as_str().into(),
            no,
            revision: event.revision,
            seq: event.seq,
        })
    }
    pub(crate) fn post_inside(
        &mut self,
        entry: Entry,
        now: i64,
        partner: Option<&NewPartner>,
    ) -> Result<PostedRef, StoreError> {
        self.entry_check(&entry)?;
        let event = self.ledger.post(entry, now)?.clone();
        self.persist_event(&event, partner)
    }
    /// 登録と任意の取引先追加、履歴を同じトランザクションで保存します。
    ///
    /// # Errors
    /// 締め、科目、金額、重複ID、読み取り専用、Staleまたは保存の失敗。
    // 操作の入力は、仕訳や変更パッチと同じく所有権を受け取る公開契約にそろえる。
    #[allow(clippy::needless_pass_by_value)]
    pub fn post(
        &mut self,
        entry: Entry,
        now: i64,
        ensure_partner: Option<NewPartner>,
    ) -> Result<PostedRef, StoreError> {
        self.atomic(|s| s.post_inside(entry, now, ensure_partner.as_ref()))
    }
    /// 最新版を訂正します。元と新しい日付の両方を締めと照合します。
    ///
    /// # Errors
    /// 古い版、取消済み、空理由、締め、科目、金額または保存の失敗。
    // 操作の入力は、仕訳や変更パッチと同じく所有権を受け取る公開契約にそろえる。
    #[allow(clippy::needless_pass_by_value)]
    pub fn revise(
        &mut self,
        entry: Entry,
        expected_revision: u32,
        reason: &str,
        now: i64,
        ensure_partner: Option<NewPartner>,
    ) -> Result<PostedRef, StoreError> {
        self.atomic(|s| {
            s.existing_check(entry.id(), expected_revision)?;
            s.entry_check(&entry)?;
            let event = s.ledger.revise(entry, reason, now)?.clone();
            s.persist_event(&event, ensure_partner.as_ref())
        })
    }
    /// 最新版を理由つきで取り消します。
    ///
    /// # Errors
    /// 古い版、取消済み、空理由、締めまたは保存の失敗。
    pub fn void(
        &mut self,
        id: &EntryId,
        expected_revision: u32,
        reason: &str,
        now: i64,
    ) -> Result<PostedRef, StoreError> {
        self.atomic(|s| {
            s.existing_check(id, expected_revision)?;
            let event = s.ledger.void(id, reason, now)?.clone();
            s.persist_event(&event, None)
        })
    }
    /// 全件と任意の取引先追加を一括登録します。1件でも失敗すれば全部戻します。
    ///
    /// # Errors
    /// 登録と同じ失敗。各入力は `(entry, ensure_partner)` です。
    pub fn post_many(
        &mut self,
        entries: Vec<(Entry, Option<NewPartner>)>,
        now: i64,
    ) -> Result<Vec<PostedRef>, StoreError> {
        self.atomic(|s| {
            entries
                .into_iter()
                .map(|(e, p)| s.post_inside(e, now, p.as_ref()))
                .collect()
        })
    }
}
