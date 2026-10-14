// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{
    AccountCode, ChainError, Entry, EntryId, Hash, LedgerError, Side, Yen, encoding::event_hash,
};
use std::collections::{BTreeMap, HashMap};

/// 追記イベントの種類。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
#[non_exhaustive]
pub enum EventKind {
    /// 初回登録。
    Posted = 1,
    /// 訂正。
    Revised = 2,
    /// 取消。
    Voided = 3,
}
/// 保存層から復元できるイベント。復元時は台帳の検証が必要です。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    /// 1始まりの連番。
    pub seq: u64,
    /// イベントの種類。
    pub kind: EventKind,
    /// 対象の仕訳識別子。
    pub entry_id: EntryId,
    /// 同じ識別子の1始まりの版。
    pub revision: u32,
    /// 呼び出し側が指定したUNIX秒。
    pub recorded_at: i64,
    /// 訂正または取消の理由。
    pub reason: Option<String>,
    /// 登録または訂正の仕訳。
    pub entry: Option<Entry>,
    /// 直前イベントのハッシュ。
    pub prev_hash: Hash,
    /// このイベントのハッシュ。
    pub hash: Hash,
}
/// 勘定科目の借方・貸方の累計。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Balance {
    /// 借方累計。
    pub debit: Yen,
    /// 貸方累計。
    pub credit: Yen,
}
/// 検証済みイベントを追記だけで保持する台帳。
#[derive(Debug, Clone, Default)]
pub struct Ledger {
    events: Vec<Event>,
    // Noneは空の台帳のみ。const fn new()の公開契約を保つ。
    index: Option<HashMap<EntryId, Vec<usize>>>,
}
impl Ledger {
    /// 空の台帳を作ります。
    #[must_use]
    pub const fn new() -> Self {
        Self {
            events: Vec::new(),
            index: None,
        }
    }
    /// 連鎖とイベント規則を検証して台帳を復元します。
    ///
    /// # Errors
    /// 連番・連鎖・ハッシュ・イベント規則の違反を返します。
    pub fn from_events(events: Vec<Event>) -> Result<Self, ChainError> {
        let mut ledger = Self {
            events,
            index: None,
        };
        ledger.verify()?;
        ledger.rebuild_index();
        Ok(ledger)
    }
    /// 追記順のイベント列。
    #[must_use]
    pub fn events(&self) -> &[Event] {
        &self.events
    }
    /// 末尾のハッシュ。空なら `Hash::GENESIS`。
    #[must_use]
    pub fn head_hash(&self) -> Hash {
        self.events.last().map_or(Hash::GENESIS, |e| e.hash)
    }
    /// 新規仕訳を登録します。
    ///
    /// # Errors
    /// 識別子の重複または連番・符号化長の桁あふれを返します。
    pub fn post(&mut self, entry: Entry, recorded_at: i64) -> Result<&Event, LedgerError> {
        if self.latest(entry.id()).is_some() {
            return Err(LedgerError::DuplicateId(entry.id().clone()));
        }
        self.append(
            EventKind::Posted,
            entry.id().clone(),
            1,
            recorded_at,
            None,
            Some(entry),
        )
    }
    /// 既存仕訳を新しい版で訂正し、trimした理由を保存します。
    ///
    /// # Errors
    /// 未登録・取消済み・空理由・連番や版の桁あふれを返します。
    pub fn revise(
        &mut self,
        entry: Entry,
        reason: &str,
        recorded_at: i64,
    ) -> Result<&Event, LedgerError> {
        let revision = self.next_revision(entry.id())?;
        let reason = checked_reason(reason)?;
        self.append(
            EventKind::Revised,
            entry.id().clone(),
            revision,
            recorded_at,
            Some(reason),
            Some(entry),
        )
    }
    /// 既存仕訳を取消し、trimした理由を保存します。
    ///
    /// # Errors
    /// 未登録・取消済み・空理由・連番や版の桁あふれを返します。
    pub fn void(
        &mut self,
        id: &EntryId,
        reason: &str,
        recorded_at: i64,
    ) -> Result<&Event, LedgerError> {
        let revision = self.next_revision(id)?;
        let reason = checked_reason(reason)?;
        self.append(
            EventKind::Voided,
            id.clone(),
            revision,
            recorded_at,
            Some(reason),
            None,
        )
    }
    fn latest(&self, id: &EntryId) -> Option<&Event> {
        let position = self.index.as_ref()?.get(id)?.last()?;
        self.events.get(*position)
    }
    fn rebuild_index(&mut self) {
        let mut index = HashMap::<EntryId, Vec<usize>>::new();
        for (position, event) in self.events.iter().enumerate() {
            index
                .entry(event.entry_id.clone())
                .or_default()
                .push(position);
        }
        self.index = Some(index);
    }
    #[cfg(test)]
    fn from_unverified_events(events: Vec<Event>) -> Self {
        let mut ledger = Self {
            events,
            index: None,
        };
        ledger.rebuild_index();
        ledger
    }
    fn next_revision(&self, id: &EntryId) -> Result<u32, LedgerError> {
        let e = self
            .latest(id)
            .ok_or_else(|| LedgerError::UnknownId(id.clone()))?;
        if e.kind == EventKind::Voided {
            return Err(LedgerError::AlreadyVoided(id.clone()));
        }
        e.revision.checked_add(1).ok_or(LedgerError::Overflow)
    }
    fn append(
        &mut self,
        kind: EventKind,
        entry_id: EntryId,
        revision: u32,
        recorded_at: i64,
        reason: Option<String>,
        entry: Option<Entry>,
    ) -> Result<&Event, LedgerError> {
        let seq = u64::try_from(self.events.len())
            .ok()
            .and_then(|n| n.checked_add(1))
            .ok_or(LedgerError::Overflow)?;
        let mut event = Event {
            seq,
            kind,
            entry_id,
            revision,
            recorded_at,
            reason,
            entry,
            prev_hash: self.head_hash(),
            hash: Hash::GENESIS,
        };
        event.hash = event_hash(&event).ok_or(LedgerError::Overflow)?;
        self.index
            .get_or_insert_with(HashMap::new)
            .entry(event.entry_id.clone())
            .or_default()
            .push(self.events.len());
        self.events.push(event);
        self.events.last().ok_or(LedgerError::Overflow)
    }
    /// 取消済みを除く最新仕訳を最初の登録順で返します。
    #[must_use]
    pub fn current(&self) -> Vec<&Entry> {
        self.events
            .iter()
            .filter(|e| e.kind == EventKind::Posted)
            .filter_map(|e| self.latest(&e.entry_id).and_then(|e| e.entry.as_ref()))
            .collect()
    }
    /// 指定仕訳の全イベントを追記順で返します。
    #[must_use]
    pub fn history(&self, id: &EntryId) -> Vec<&Event> {
        self.index
            .as_ref()
            .and_then(|index| index.get(id))
            .into_iter()
            .flatten()
            .filter_map(|position| self.events.get(*position))
            .collect()
    }
    /// 最新の有効な仕訳を勘定科目ごとに集計します。
    ///
    /// # Errors
    /// 勘定科目ごとの累計が桁あふれした場合に失敗します。
    pub fn trial_balance(&self) -> Result<BTreeMap<AccountCode, Balance>, LedgerError> {
        let mut result = BTreeMap::<AccountCode, Balance>::new();
        for entry in self.current() {
            for line in entry.lines() {
                let balance = result.entry(line.account.clone()).or_default();
                let total = match line.side {
                    Side::Debit => &mut balance.debit,
                    Side::Credit => &mut balance.credit,
                };
                *total = total
                    .checked_add(line.amount)
                    .ok_or(LedgerError::Overflow)?;
            }
        }
        Ok(result)
    }
    /// 先頭から連番、リンク、再計算ハッシュ、イベント規則の順で検証します。
    ///
    /// # Errors
    /// 最初に見つかった連鎖の不正を返します。
    pub fn verify(&self) -> Result<(), ChainError> {
        let mut previous = Hash::GENESIS;
        let mut states = BTreeMap::<&EntryId, &Event>::new();
        for (index, event) in self.events.iter().enumerate() {
            let violation = |rule| ChainError::RuleViolation {
                seq: event.seq,
                rule,
            };
            let expected = u64::try_from(index)
                .ok()
                .and_then(|n| n.checked_add(1))
                .ok_or_else(|| violation("連番の桁あふれ"))?;
            if event.seq != expected {
                return Err(ChainError::WrongSequence {
                    expected,
                    found: event.seq,
                });
            }
            if event.prev_hash != previous {
                return Err(ChainError::BrokenLink { seq: event.seq });
            }
            let computed = event_hash(event).ok_or_else(|| violation("符号化長の桁あふれ"))?;
            if event.hash != computed {
                return Err(ChainError::BadHash { seq: event.seq });
            }
            match states.get(&event.entry_id) {
                None => {
                    if event.kind != EventKind::Posted || event.revision != 1 {
                        return Err(violation("初回は登録・版1"));
                    }
                }
                Some(prior) => {
                    if prior.kind == EventKind::Voided {
                        return Err(violation("取消後のイベント"));
                    }
                    if event.kind == EventKind::Posted {
                        return Err(violation("登録の重複"));
                    }
                    if prior.revision.checked_add(1) != Some(event.revision) {
                        return Err(violation("版の不連続"));
                    }
                }
            }
            match event.kind {
                EventKind::Posted | EventKind::Revised => {
                    if event
                        .entry
                        .as_ref()
                        .is_none_or(|e| e.id() != &event.entry_id)
                    {
                        return Err(violation("仕訳と識別子の不一致"));
                    }
                }
                EventKind::Voided => {
                    if event.entry.is_some() {
                        return Err(violation("取消に仕訳が存在"));
                    }
                }
            }
            match event.kind {
                EventKind::Posted => {
                    if event.reason.is_some() {
                        return Err(violation("登録に理由が存在"));
                    }
                }
                EventKind::Revised | EventKind::Voided => {
                    if event.reason.as_deref().is_none_or(|r| r.trim().is_empty()) {
                        return Err(violation("訂正・取消の理由なし"));
                    }
                }
            }
            states.insert(&event.entry_id, event);
            previous = event.hash;
        }
        Ok(())
    }
}
fn checked_reason(reason: &str) -> Result<String, LedgerError> {
    let reason = reason.trim();
    if reason.is_empty() {
        return Err(LedgerError::EmptyReason);
    }
    Ok(reason.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn revision_overflow_is_atomic() {
        let id = EntryId::new("E").unwrap();
        let event = Event {
            seq: 1,
            kind: EventKind::Revised,
            entry_id: id.clone(),
            revision: u32::MAX,
            recorded_at: 0,
            reason: Some("訂正".to_owned()),
            entry: None,
            prev_hash: Hash::GENESIS,
            hash: Hash::GENESIS,
        };
        // 2^32個のイベントを確保せず、内部状態だけで境界を検査する。
        let mut ledger = Ledger::from_unverified_events(vec![event]);
        let before_events = ledger.events().to_vec();
        let before_index = ledger.index.clone();
        let before_head = ledger.head_hash();
        assert_eq!(ledger.void(&id, "取消", 1), Err(LedgerError::Overflow));
        assert_eq!(ledger.events(), before_events);
        assert_eq!(ledger.index, before_index);
        assert_eq!(ledger.head_hash(), before_head);
    }
}
