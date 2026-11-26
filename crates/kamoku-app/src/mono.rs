// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 単調に進む時計。再送防止と取込の一時置き場の期限に使う。
//! ネイティブは `Instant`（起動からの時間）。wasm32 にはそれが無く、実行時に止まるので、
//! `js_sys::Date` のミリ秒を使う。期限の比較だけに使い、仕訳の記録時刻には使わない。

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use std::time::Instant as Mono;

#[cfg(target_arch = "wasm32")]
use std::time::Duration;

/// `Date.now()` のミリ秒。有限でない、または負なら 0。
#[cfg(target_arch = "wasm32")]
pub(crate) fn date_millis() -> u64 {
    nonnegative_millis(js_sys::Date::now())
}

#[cfg(target_arch = "wasm32")]
fn nonnegative_millis(ms: f64) -> u64 {
    // f64 が整数を正確に表せる上限（2^53-1）。超えたら打ち切る。
    const MAX_EXACT: f64 = 9_007_199_254_740_991.0;
    if !ms.is_finite() || ms <= 0.0 {
        return 0;
    }
    if ms >= MAX_EXACT {
        return u64::MAX;
    }
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss,
        reason = "有限・非負・2^53 未満なので、小数を切り捨てた整数は u64 に正確に入る"
    )]
    let whole = ms.floor() as u64;
    whole
}

/// wasm32 の単調時計（`Date.now()` のミリ秒）。壁時計なので戻ることがある。戻りは 0 として扱う。
#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Mono(u64);

#[cfg(target_arch = "wasm32")]
impl Mono {
    pub(crate) fn now() -> Self {
        Self(date_millis())
    }

    pub(crate) fn saturating_duration_since(self, earlier: Self) -> Duration {
        Duration::from_millis(self.0.saturating_sub(earlier.0))
    }
}
