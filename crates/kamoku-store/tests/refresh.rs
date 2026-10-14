// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 別の接続が変えたマスタと台帳に合わせ直す `refresh_masters`（コードレビューの指摘 B1）。

mod common;
use common::*;
use kamoku_store::*;

// B1: 別の接続が科目の既定の割合と設定を変えても、読み直すまでは古い値のまま。読み直せば新しい値になる。
#[test]
fn refresh_masters_reads_masters_changed_elsewhere() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kamoku.db");
    let mut mine = opened(&path);
    let mut other = opened(&path);
    let vehicle = AccountPatch {
        default_pct: Some(80),
        ..AccountPatch::default()
    };
    other.update_account("exp_vehicle", vehicle, 1).unwrap();
    let mut settings = other.settings().clone();
    settings.business_name = "別の接続で変えた名前".into();
    other.save_settings(settings, 2).unwrap();
    assert_ne!(mine.chart().get("exp_vehicle").unwrap().default_pct, 80);
    mine.refresh_masters().unwrap();
    assert_eq!(mine.chart().get("exp_vehicle").unwrap().default_pct, 80);
    assert_eq!(mine.settings().business_name, "別の接続で変えた名前");
}

// B1: 台帳の末尾が変わっていれば、台帳も読み直して Stale を返す。読み直したあとは Ok で、書き込める。
#[test]
fn refresh_masters_reloads_a_moved_ledger_and_reports_stale() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kamoku.db");
    let mut mine = opened(&path);
    let mut other = opened(&path);
    other.post(entry("e_other", "別の接続"), 1, None).unwrap();
    assert_eq!(mine.ledger().events().len(), 0);
    assert!(matches!(mine.refresh_masters(), Err(StoreError::Stale)));
    assert_eq!(mine.ledger().events().len(), 1);
    mine.refresh_masters().unwrap();
    mine.post(entry("e_mine", "自分"), 2, None).unwrap();
    assert_eq!(mine.ledger().events().len(), 2);
}
