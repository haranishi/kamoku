// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 再送防止の印（`client_id`）の表（api-entries.md、AC-02.7）。メモリに10分だけ置き、再起動で消える。
//!
//! - 記録には、本文そのものではなく、本文の SHA-256 と、最初の応答（仕訳の表現）だけを入れる。
//!   本文を丸ごと持つと、知らない項目で膨らませた要求を繰り返すだけで、メモリを使い切れるため。
//! - 件数とバイト数に上限がある。いっぱいのときは、新しい印の要求を、保存の前に `503 busy` で断る。
//! - 期限内の記録は追い出さない。追い出した直後に同じ要求が再送されると、二重に登録されるため。
//!   同じ印・同じ本文の再送は、いっぱいでも必ず最初の結果を返す。

use super::ApiError;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// `client_id` の長さの上限（バイト）。
pub(crate) const CLIENT_ID_MAX_LEN: usize = 64;

// 1件の記録の、印・指紋・応答の他にかかる分の見積もり（表の管理の分）。
const RECORD_OVERHEAD: usize = 128;

/// 表の上限。
#[derive(Debug, Clone, Copy)]
pub(crate) struct ClientLimits {
    /// 記録を置く時間。
    pub ttl: Duration,
    /// 記録の件数の上限。
    pub count: usize,
    /// 記録のバイト数の合計の上限。新しい印は、合計がこれより小さいときだけ受け付ける
    /// （超えるのは、最後に入れた1件の分まで）。
    pub bytes: usize,
}
impl Default for ClientLimits {
    fn default() -> Self {
        Self {
            ttl: Duration::from_secs(10 * 60),
            count: 10_000,
            bytes: 8 * 1024 * 1024,
        }
    }
}

/// 本文の指紋（SHA-256）。
pub(crate) type BodyDigest = [u8; 32];

/// 本文の指紋を作ります。`serde_json` の `Map` は鍵の順に並ぶので、同じ内容なら同じ指紋になる。
pub(crate) fn digest(body: &Map<String, Value>) -> Result<BodyDigest, ApiError> {
    let bytes = serde_json::to_vec(body).map_err(ApiError::internal)?;
    Ok(Sha256::digest(&bytes).into())
}

/// 表を引いた結果。
pub(crate) enum Seen {
    /// 初めての印（保存してよい）。
    New,
    /// 同じ本文の再送（最初の応答）。
    Again(Value),
}

struct Record {
    at: Instant,
    digest: BodyDigest,
    response: Value,
    size: usize,
}

/// 再送防止の印の表。
#[derive(Default)]
pub(crate) struct ClientIds {
    records: HashMap<String, Record>,
    bytes: usize,
    limits: ClientLimits,
}

impl ClientIds {
    /// 上限を決めて作ります（試験用。本番は既定の10分・1万件・8MB）。
    #[cfg(test)]
    pub(crate) fn with_limits(limits: ClientLimits) -> Self {
        Self {
            limits,
            ..Self::default()
        }
    }

    fn purge(&mut self, now: Instant) {
        let ttl = self.limits.ttl;
        let mut freed = 0usize;
        self.records.retain(|_, r| {
            let keep = now.saturating_duration_since(r.at) < ttl;
            if !keep {
                freed = freed.saturating_add(r.size);
            }
            keep
        });
        self.bytes = self.bytes.saturating_sub(freed);
    }

    /// 保存の前に引きます。期限切れを掃除してから、同じ印があれば本文の指紋で比べる。
    ///
    /// # Errors
    /// 同じ印で本文が違えば `409 client_id_reused`。新しい印で表がいっぱいなら `503 busy`。
    pub(crate) fn check(
        &mut self,
        id: &str,
        digest: &BodyDigest,
        now: Instant,
    ) -> Result<Seen, ApiError> {
        self.purge(now);
        if let Some(seen) = self.records.get(id) {
            if seen.digest == *digest {
                return Ok(Seen::Again(seen.response.clone()));
            }
            return Err(ApiError::new(
                409,
                "client_id_reused",
                "同じ送信の印（client_id）で、違う内容が届きました。画面を読み込み直してから、もう一度保存してください。",
            )
            .with_field("client_id"));
        }
        if self.records.len() >= self.limits.count || self.bytes >= self.limits.bytes {
            return Err(ApiError::new(
                503,
                "busy",
                "混み合っています。数分たってから、もう一度保存してください。",
            ));
        }
        Ok(Seen::New)
    }

    /// 保存できた要求を記録します。空きは [`ClientIds::check`] で確かめてあるので、追い出さない。
    pub(crate) fn insert(&mut self, id: String, digest: BodyDigest, response: Value, now: Instant) {
        let size = serde_json::to_vec(&response)
            .map_or(0, |b| b.len())
            .saturating_add(id.len())
            .saturating_add(digest.len())
            .saturating_add(RECORD_OVERHEAD);
        let record = Record {
            at: now,
            digest,
            response,
            size,
        };
        if let Some(old) = self.records.insert(id, record) {
            self.bytes = self.bytes.saturating_sub(old.size);
        }
        self.bytes = self.bytes.saturating_add(size);
    }

    /// 記録のバイト数の合計（見積もり）。
    #[cfg(test)]
    pub(crate) const fn bytes(&self) -> usize {
        self.bytes
    }

    /// 記録の件数。
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.records.len()
    }
}
