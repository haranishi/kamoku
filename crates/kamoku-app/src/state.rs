// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! サーバーが持つ状態。全部を1つの `Mutex` に入れ、1つの要求の処理は「ロックを取って、計算して、放す」。

use crate::api::{ClientIds, Uploads};
use crate::server::Mode;
use kamoku_store::Store;

/// サーバーが持つ状態。
pub(crate) struct State {
    /// 保存先。
    pub store: Store,
    /// 通常かデモか。
    pub mode: Mode,
    /// 起動時のDBのイベント数。読み取り専用のときに、読み込めた数と分けて返す。
    pub events_in_db: u64,
    /// 再送防止の印（`client_id`）の表。
    pub clients: ClientIds,
    /// 取込の一時置き場。
    pub uploads: Uploads,
}
