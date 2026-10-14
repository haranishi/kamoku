// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::StoreError;
use kamoku_books::{Kind, TaxCode, Wallet};
use kamoku_import::Mapping;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 開くときの指定。
#[derive(Debug, Clone, Copy, Default)]
pub struct OpenOptions {
    /// ファイルが無ければ作るか。
    pub create_if_missing: bool,
}
/// 設定。日付の文字列は保存前に `YYYY-MM-DD` を検査します。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    /// 事業者名。
    pub business_name: String,
    /// 事業主名。
    pub owner_name: String,
    /// `exempt` または `taxable`。
    pub tax_mode: String,
    /// Tと半角13桁。
    pub invoice_number: Option<String>,
    /// `floor`、`round`、`ceil`。
    pub rounding: String,
    /// 開業日。未設定なら空。
    pub opened_on: String,
    /// 締めた日。
    pub locked_until: Option<String>,
    /// 初期設定完了。
    pub setup_done: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            business_name: String::new(),
            owner_name: String::new(),
            tax_mode: "exempt".into(),
            invoice_number: None,
            rounding: "floor".into(),
            opened_on: String::new(),
            locked_until: None,
            setup_done: false,
        }
    }
}
/// 起動時の読み込み結果。
#[derive(Debug, Clone)]
pub struct LoadReport {
    /// DBのイベント数。
    pub events_in_db: u64,
    /// 検証済みの接頭部分のイベント数。
    pub events_loaded: u64,
    /// 登録された仕訳数（取消済みを含む）。
    pub entries: u64,
    /// 所要時間。
    pub elapsed: std::time::Duration,
    /// 検証結果。
    pub verification: Result<VerifyOk, crate::VerifyFailure>,
}
/// 検証できた台帳の状態。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerifyOk {
    /// イベント数。
    pub events: u64,
    /// 末尾ハッシュ。
    pub head_hash: String,
}
/// 台帳外に控えたハッシュの位置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NotedHash {
    /// 連番。
    pub seq: u64,
    /// 末尾か。
    pub is_head: bool,
}
/// 保存した仕訳の参照。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PostedRef {
    /// 仕訳ID。
    pub entry_id: String,
    /// 登録順の仕訳No。
    pub no: u64,
    /// 現在の版。
    pub revision: u32,
    /// イベントの連番。
    pub seq: u64,
}
/// 新しい科目。コードと分類に由来する側は保存層が決めます。
#[derive(Debug, Clone)]
pub struct NewAccount {
    /// 表示名。
    pub name: String,
    /// 読み。
    pub reading: String,
    /// 分類。
    pub kind: Kind,
    /// 親のコード。口座で省略したら標準の親。
    pub parent: Option<String>,
    /// 口座の情報。
    pub wallet: Option<Wallet>,
    /// 既定の税区分。
    pub default_tax: Option<TaxCode>,
    /// 既定の割合。
    pub default_pct: u8,
}
/// 科目の変更。外側のNoneは変更しない指定です。
#[derive(Debug, Clone, Default)]
pub struct AccountPatch {
    /// 名前。
    pub name: Option<String>,
    /// 読み。
    pub reading: Option<String>,
    /// 有効か。
    pub enabled: Option<bool>,
    /// 既定の税区分。Some(None)で消します。
    pub default_tax: Option<Option<TaxCode>>,
    /// 既定の割合。
    pub default_pct: Option<u8>,
}
/// 新しい取引先。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NewPartner {
    /// 名前（重複不可）。
    pub name: String,
    /// 読み。
    pub reading: String,
    /// 登録番号。
    pub invoice_number: Option<String>,
    /// メモ。
    pub note: String,
}
/// 保存された取引先。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Partner {
    /// ID。
    pub id: i64,
    /// 名前。
    pub name: String,
    /// 読み。
    pub reading: String,
    /// 登録番号。
    pub invoice_number: Option<String>,
    /// メモ。
    pub note: String,
    /// 有効か。
    pub enabled: bool,
}
/// 取引先の変更。
#[derive(Debug, Clone, Default)]
pub struct PartnerPatch {
    /// 名前。
    pub name: Option<String>,
    /// 読み。
    pub reading: Option<String>,
    /// 登録番号。
    pub invoice_number: Option<Option<String>>,
    /// メモ。
    pub note: Option<String>,
    /// 有効か。
    pub enabled: Option<bool>,
}
/// よく使う取引。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    /// ID。
    pub id: i64,
    /// 名前。
    pub name: String,
    /// 日付を除いた単純な入力。
    pub body: Value,
}
/// 変更の記録。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeLog {
    /// ID。
    pub id: i64,
    /// 操作者。
    pub actor: String,
    /// UNIX秒。
    pub recorded_at: i64,
    /// 操作名。
    pub kind: String,
    /// 操作対象と内容。
    pub body: Value,
}
/// 口座ごとに覚えた列の対応。
pub type ImportProfile = Mapping;
/// 下書きのID。
pub type DraftId = i64;
/// 下書き一覧。
#[derive(Debug, Clone, Serialize)]
pub struct Page<T> {
    /// このページ。
    pub items: Vec<T>,
    /// 全件数。
    pub total: u64,
}
/// 行ごとの確定結果。DB書き込みの失敗は関数全体のErrです。
#[derive(Debug)]
pub struct ConfirmResult {
    /// 下書きID。
    pub id: DraftId,
    /// 保存参照または入力の失敗。
    pub result: Result<PostedRef, StoreError>,
}
