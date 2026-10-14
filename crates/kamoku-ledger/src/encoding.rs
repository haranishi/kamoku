// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! ハッシュの唯一の符号化仕様（文字列の正規化はしない）。
//! ```text
//! lp(s) = u32 big-endian(UTF-8バイト数) + UTF-8バイト列
//! opt_lp(s) = なし: 0x00 / あり: 0x01 + lp(s)
//! event_hash = SHA-256(次の順序で連結)
//!   lp("kamoku.ledger.event.v1") + prev_hash(32バイト)
//!   + u64 big-endian(seq) + u8(kind: Posted=1, Revised=2, Voided=3)
//!   + lp(entry_id) + u32 big-endian(revision) + i64 big-endian(recorded_at)
//!   + opt_lp(reason) + opt_entry(entry)
//! opt_entry = なし: 0x00 / あり: 0x01 + enc_entry
//! enc_entry = lp(id) + i32 big-endian(year) + u8(month 1..=12) + u8(day)
//!   + lp(description) + u32 big-endian(明細行数) + 行順の enc_line + enc_meta
//! enc_line = lp(account) + u8(side: Debit=1, Credit=2) + i64 big-endian(amount)
//!   + opt_lp(tax_code) + i64 big-endian(tax_amount) + lp(memo)
//! enc_meta = 付随情報が空: 何も足さない
//!   / 空でない: 0x01 + u32 big-endian(組の数) + 鍵の昇順（バイト順）に lp(key) + lp(value)
//! ```
//! `enc_meta` は符号化の末尾にあり、空のときは1バイトも足しません。付随情報の無い
//! イベントのハッシュは、付随情報を足す前の版と同じです（`tests/golden.rs` が守ります）。
//! 保存されたバイト列そのものを使い、Debug 表現は使用しません。

use crate::{Event, Hash, Side};
use sha2::{Digest, Sha256};

fn lp(hasher: &mut Sha256, s: &str) -> Option<()> {
    hasher.update(u32::try_from(s.len()).ok()?.to_be_bytes());
    hasher.update(s.as_bytes());
    Some(())
}
fn opt_lp(hasher: &mut Sha256, s: Option<&str>) -> Option<()> {
    hasher.update([u8::from(s.is_some())]);
    if let Some(s) = s {
        lp(hasher, s)?;
    }
    Some(())
}
pub(crate) fn event_hash(event: &Event) -> Option<Hash> {
    let mut h = Sha256::new();
    lp(&mut h, "kamoku.ledger.event.v1")?;
    h.update(event.prev_hash.as_bytes());
    h.update(event.seq.to_be_bytes());
    h.update([event.kind as u8]);
    lp(&mut h, event.entry_id.as_str())?;
    h.update(event.revision.to_be_bytes());
    h.update(event.recorded_at.to_be_bytes());
    opt_lp(&mut h, event.reason.as_deref())?;
    h.update([u8::from(event.entry.is_some())]);
    if let Some(entry) = &event.entry {
        lp(&mut h, entry.id().as_str())?;
        h.update(entry.date().year().to_be_bytes());
        h.update([u8::from(entry.date().month()), entry.date().day()]);
        lp(&mut h, entry.description())?;
        h.update(u32::try_from(entry.lines().len()).ok()?.to_be_bytes());
        for line in entry.lines() {
            lp(&mut h, line.account.as_str())?;
            h.update([match line.side {
                Side::Debit => 1,
                Side::Credit => 2,
            }]);
            h.update(line.amount.get().to_be_bytes());
            opt_lp(&mut h, line.tax_code.as_deref())?;
            h.update(line.tax_amount.get().to_be_bytes());
            lp(&mut h, &line.memo)?;
        }
        let meta = entry.meta();
        if !meta.is_empty() {
            h.update([1]);
            h.update(u32::try_from(meta.len()).ok()?.to_be_bytes());
            // BTreeMap<String, _> の順は、文字列のバイト順と一致する。
            for (key, value) in meta {
                lp(&mut h, key)?;
                lp(&mut h, value)?;
            }
        }
    }
    Some(Hash(h.finalize().into()))
}
