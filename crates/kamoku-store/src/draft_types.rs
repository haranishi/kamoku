// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::DraftId;
use serde::{Deserialize, Serialize};

/// 下書きの状態。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DraftStatus {
    /// 未確認。
    Draft,
    /// 確定済み。
    Confirmed,
    /// 取り込まない。
    Ignored,
}
impl DraftStatus {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Confirmed => "confirmed",
            Self::Ignored => "ignored",
        }
    }
}
/// 取込の記録の入力。
#[derive(Debug, Clone)]
pub struct NewBatch {
    /// ファイル名。
    pub filename: String,
    /// 口座。
    pub wallet: String,
    /// UNIX秒。
    pub recorded_at: i64,
    /// 読めなかった行数。
    pub invalid: u64,
}
/// 取込の記録。
#[derive(Debug, Clone, Serialize)]
pub struct ImportSummary {
    /// バッチID。
    pub batch_id: i64,
    /// 追加数。
    pub created: u64,
    /// 重複数（強制分は除く）。
    pub duplicates: u64,
    /// 無効な行数。
    pub invalid: u64,
}
/// 保存された取込記録。
#[derive(Debug, Clone, Serialize)]
pub struct ImportBatch {
    /// 取込結果。
    #[serde(flatten)]
    pub summary: ImportSummary,
    /// ファイル名。
    pub filename: String,
    /// 口座。
    pub wallet: String,
    /// UNIX秒。
    pub recorded_at: i64,
}
/// 保存する取込の行。指紋は取り込みの部品で作って渡します。
#[derive(Debug, Clone)]
pub struct NewDraft {
    /// ファイル内の行番号（1始まり）。
    pub line: usize,
    /// 日付。
    pub date: String,
    /// 摘要。
    pub description: String,
    /// `in` または `out`。
    pub direction: String,
    /// 正の金額。
    pub amount: i64,
    /// SHA-256の小文字16進。
    pub fingerprint: String,
    /// 同じ指紋の出現番号（1始まり）。
    pub occurrence: u32,
    /// 金融機関の取引番号。
    pub external_id: Option<String>,
}
/// 保存された下書き。
#[derive(Debug, Clone, Serialize)]
pub struct Draft {
    /// ID。
    pub id: DraftId,
    /// バッチID。
    pub batch_id: i64,
    /// 口座コード。
    pub wallet: String,
    /// 日付。
    pub date: String,
    /// 摘要。
    pub description: String,
    /// お金の向き。
    pub direction: String,
    /// 金額。
    pub amount: i64,
    /// 指紋（編集しても変えない）。
    pub fingerprint: String,
    /// 出現番号。
    pub occurrence: u32,
    /// 取引番号。
    pub external_id: Option<String>,
    /// 相手科目。
    pub account: Option<String>,
    /// 税区分。
    pub tax_code: Option<String>,
    /// 取引先名。
    pub partner: String,
    /// メモ。
    pub memo: String,
    /// 事業割合。
    pub business_pct: u8,
    /// 状態。
    pub status: DraftStatus,
    /// 確定先。
    pub entry_id: Option<String>,
}
/// 下書きの変更。
#[derive(Debug, Clone, Default)]
pub struct DraftPatch {
    /// 日付。
    pub date: Option<String>,
    /// 科目。Some(None)で消します。
    pub account: Option<Option<String>>,
    /// 税区分。
    pub tax_code: Option<Option<String>>,
    /// 取引先名。
    pub partner: Option<String>,
    /// メモ。
    pub memo: Option<String>,
    /// 事業割合。
    pub business_pct: Option<u8>,
}
/// 状態変更の操作。
#[derive(Debug, Clone, Copy)]
pub enum DraftAction {
    /// 未確認を取り込まない状態にします。
    Ignore,
    /// 無視済み、または取消された確定済みを未確認に戻します。
    Restore,
}
/// 下書き一覧の条件。
#[derive(Debug, Clone)]
pub struct DraftFilter {
    /// 状態。Noneなら全部。
    pub status: Option<DraftStatus>,
    /// 口座。
    pub wallet: Option<String>,
    /// 摘要・取引先・メモの部分一致。
    pub q: Option<String>,
    /// 開始日（含む）。
    pub from: Option<String>,
    /// 終了日（含む）。
    pub to: Option<String>,
    /// 1〜500件。
    pub limit: u32,
    /// 飛ばす件数。
    pub offset: u64,
}
impl Default for DraftFilter {
    fn default() -> Self {
        Self {
            status: Some(DraftStatus::Draft),
            wallet: None,
            q: None,
            from: None,
            to: None,
            limit: 50,
            offset: 0,
        }
    }
}
/// 状態ごとの件数。
#[derive(Debug, Clone, Default, Serialize)]
pub struct DraftCounts {
    /// 未確認。
    pub draft: u64,
    /// 取り込まない。
    pub ignored: u64,
    /// 確定済み。
    pub confirmed: u64,
}
/// プレビューで使う存在判定と最大の出現番号。
#[derive(Debug, Clone, Default)]
pub struct KnownFingerprints {
    /// 存在する組。
    pub existing: std::collections::HashSet<(String, u32)>,
    /// 問い合わせた各指紋の最大出現番号。未登録は0。
    pub maxima: std::collections::HashMap<String, u32>,
}
