// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! いまの時刻と「今日」。

use std::time::{SystemTime, UNIX_EPOCH};
use time::{Date, OffsetDateTime};

// 帳簿の日付は日本の日付で付けるので、「今日」は日本時間で決める。`time` の地方時の機能
// （local-offset）は依存を増やすので使わない。
const JST_OFFSET_SECS: i64 = 9 * 3600;

/// いまのUNIX秒。時計が1970年より前を指していれば0。
pub(crate) fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// いまのミリ秒（仕訳IDに使う）。
pub(crate) fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// UNIX秒の、日本時間の日付。
pub(crate) fn jst_date(secs: i64) -> Date {
    OffsetDateTime::from_unix_timestamp(secs.saturating_add(JST_OFFSET_SECS))
        .map_or(Date::MIN, OffsetDateTime::date)
}

/// 日本時間の今日。
pub(crate) fn today() -> Date {
    jst_date(now_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::date;

    #[test]
    fn jst_changes_day_at_15_utc() {
        // 2026-10-14T14:59:59Z は日本時間の23:59:59、15:00:00Z は翌日の0時。
        assert_eq!(jst_date(1_791_989_999), date!(2026 - 10 - 14));
        assert_eq!(jst_date(1_791_990_000), date!(2026 - 10 - 15));
    }
}
