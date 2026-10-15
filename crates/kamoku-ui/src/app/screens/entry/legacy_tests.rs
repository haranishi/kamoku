// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// Behavior carried from ui/tests/account-defaults, entry-options and entry-hints.
use super::{
    account_options, date_concern, is_transitional, is_wallet, jp_date, line_memo, locked_detail,
    locked_note, next_defaults, parse_pct, pct_source_note, tax_options, tax_source_note,
    wallet_options, wallet_parent_codes,
};
use serde_json::{Value, json};
use std::collections::BTreeSet;

fn defaults() -> (Value, Value, Value, Value) {
    (
        json!({"code":"exp_comm","default_tax":"P10","default_pct":70,"wallet":null}),
        json!({"code":"exp_vehicle","default_tax":"P10","default_pct":30,"wallet":null}),
        json!({"code":"exp_rent","default_tax":null,"default_pct":100,"wallet":null}),
        json!({"code":"w03","default_tax":null,"default_pct":100,"wallet":{"type":"card"}}),
    )
}

#[test]
fn account_defaults_start_with_the_selected_accounts_defaults() {
    let (comm, _, _, _) = defaults();
    assert_eq!(
        next_defaults(&Value::Null, &comm, "", 100),
        ("P10".into(), 70)
    );
}

#[test]
fn account_defaults_follow_matching_old_defaults_including_null_tax() {
    let (comm, vehicle, rent, _) = defaults();
    assert_eq!(
        next_defaults(&comm, &vehicle, "P10", 70),
        ("P10".into(), 30)
    );
    assert_eq!(next_defaults(&comm, &rent, "P10", 70), (String::new(), 100));
}

#[test]
fn account_defaults_preserve_tax_and_percentage_changed_by_the_user() {
    let (comm, vehicle, rent, _) = defaults();
    assert_eq!(
        next_defaults(&comm, &vehicle, "P10", 50),
        ("P10".into(), 50)
    );
    assert_eq!(next_defaults(&comm, &rent, "P8", 70), ("P8".into(), 100));
}

#[test]
fn wallet_counterparts_lock_values_and_release_them_to_the_next_account() {
    let (comm, _, _, card) = defaults();
    assert!(is_wallet(&card));
    assert_eq!(next_defaults(&comm, &card, "P10", 50), (String::new(), 100));
    assert_eq!(next_defaults(&card, &comm, "", 100), ("P10".into(), 70));
}

#[test]
fn tax_choices_follow_the_transaction_kind_and_identify_transitional_rates() {
    assert_eq!(
        tax_options("expense")
            .iter()
            .map(|(code, _)| *code)
            .collect::<Vec<_>>(),
        ["", "P10", "P8", "P10X", "P8X", "NT"]
    );
    assert_eq!(
        tax_options("income")
            .iter()
            .map(|(code, _)| *code)
            .collect::<Vec<_>>(),
        ["", "S10", "S8", "NT"]
    );
    assert!(is_transitional("P10X"));
    assert!(is_transitional("P8X"));
    assert!(!is_transitional("P10"));
}

fn accounts() -> Vec<Value> {
    vec![
        json!({"code":"cash","name":"現金","kind":"asset","enabled":true,"sort":0,"wallet":null}),
        json!({"code":"dep_other","name":"普通預金・その他の預金","kind":"asset","enabled":true,"sort":0,"wallet":null}),
        json!({"code":"accrued","name":"未払金","kind":"liability","enabled":true,"sort":0,"wallet":null}),
        json!({"code":"exp_comm","name":"通信費","kind":"expense","enabled":true,"sort":0,"wallet":null}),
        json!({"code":"w01","name":"現金","kind":"asset","enabled":true,"sort":0,"parent":"cash","wallet":{"type":"cash"}}),
        json!({"code":"w02","name":"サンプル銀行 普通預金","kind":"asset","enabled":true,"sort":0,"parent":"dep_other","wallet":{"type":"bank"}}),
        json!({"code":"w03","name":"サンプルカード","kind":"liability","enabled":true,"sort":0,"parent":"accrued","wallet":{"type":"card"}}),
    ]
}

fn contains_code(accounts: &[Value], code: &str) -> bool {
    accounts.iter().any(|account| account["code"] == code)
}

#[test]
fn wallet_parents_include_cash_and_deposits_but_not_unpaid_card_balances() {
    assert_eq!(
        wallet_parent_codes(&accounts()),
        BTreeSet::from(["cash".into(), "dep_other".into()])
    );
}

#[test]
fn entry_candidates_hide_wallet_parents_keep_real_accounts_and_avoid_duplicate_cash() {
    let entries = account_options(&accounts(), "expense", true, &[]);
    assert!(!contains_code(&entries, "cash"));
    assert!(!contains_code(&entries, "dep_other"));
    for code in ["w01", "w02", "w03", "accrued", "exp_comm"] {
        assert!(contains_code(&entries, code));
    }
    assert_eq!(
        entries
            .iter()
            .filter(|account| account["name"] == "現金")
            .count(),
        1
    );
}

#[test]
fn filter_candidates_can_still_select_parent_accounts() {
    let entries = account_options(&accounts(), "expense", false, &[]);
    assert!(contains_code(&entries, "cash"));
    assert!(contains_code(&entries, "dep_other"));
}

#[test]
fn correction_candidates_keep_only_the_requested_parent_account() {
    let entries = account_options(&accounts(), "expense", true, &["cash".into()]);
    assert!(contains_code(&entries, "cash"));
    assert!(!contains_code(&entries, "dep_other"));
}

#[test]
fn books_without_wallets_keep_all_parent_accounts() {
    let accounts: Vec<Value> = accounts()
        .into_iter()
        .filter(|account| !is_wallet(account))
        .collect();
    assert!(wallet_parent_codes(&accounts).is_empty());
    let entries = account_options(&accounts, "expense", true, &[]);
    assert!(contains_code(&entries, "cash"));
    assert!(contains_code(&entries, "dep_other"));
}

#[test]
fn disabling_a_wallet_releases_its_parent_as_an_entry_candidate() {
    let mut accounts = accounts();
    for account in &mut accounts {
        if account["code"] == "w01" {
            account["enabled"] = json!(false);
        }
    }
    let entries = account_options(&accounts, "expense", true, &[]);
    assert!(contains_code(&entries, "cash"));
    assert!(!contains_code(&entries, "dep_other"));
    assert!(!contains_code(&wallet_options(&accounts), "w01"));
}

#[test]
fn percentages_accept_only_integer_zero_to_one_hundred_after_width_normalization() {
    for (raw, expected) in [("70", 70), ("０", 0), (" 100 ", 100)] {
        assert_eq!(parse_pct(raw), Some(expected));
    }
    for raw in ["150", "-10", "abc", "", "12.5", "5e1", "1000"] {
        assert_eq!(parse_pct(raw), None);
    }
    assert_eq!(parse_pct("⑦⓪"), Some(70));
}

#[test]
fn percentage_note_explains_only_non_wallet_defaults_below_one_hundred() {
    assert_eq!(
        pct_source_note(&json!({"kind":"expense","default_pct":30})),
        "この科目の既定の割合は、事業30%です。「勘定科目」の画面で変えられます。"
    );
    for account in [
        json!({"kind":"expense","default_pct":100}),
        json!({"kind":"expense"}),
        json!({"kind":"asset","default_pct":30,"wallet":{"type":"card"}}),
        Value::Null,
    ] {
        assert_eq!(pct_source_note(&account), "");
    }
}

#[test]
fn tax_note_only_warns_taxable_expenses_with_both_defaults_and_selection_outside_scope() {
    let travel = json!({"kind":"expense","default_tax":null});
    assert_eq!(
        tax_source_note(true, "expense", &travel, ""),
        "この科目の既定は「対象外」です。課税の取引なら、税区分を選んでください。"
    );
    assert_eq!(tax_source_note(true, "expense", &travel, "P10"), "");
    assert_eq!(tax_source_note(false, "expense", &travel, ""), "");
    assert_eq!(
        tax_source_note(
            true,
            "income",
            &json!({"kind":"revenue","default_tax":null}),
            ""
        ),
        ""
    );
    assert_eq!(
        tax_source_note(
            true,
            "expense",
            &json!({"kind":"asset","default_tax":null}),
            ""
        ),
        ""
    );
    assert_eq!(
        tax_source_note(
            true,
            "expense",
            &json!({"kind":"expense","default_tax":"P10"}),
            ""
        ),
        ""
    );
}

#[test]
fn date_notice_checks_exact_future_and_opening_date_boundaries() {
    let today = "2026-10-15";
    assert_eq!(jp_date("2030-01-01"), "2030年1月1日");
    assert_eq!(date_concern("2026-11-15", today, "2026-01-01", ""), "");
    assert_eq!(
        date_concern("2026-11-16", today, "2026-01-01", ""),
        "2026年11月16日の取引として保存します。今日より31日を超えて先の日付です。年は合っていますか。"
    );
    assert!(
        date_concern("2025-12-31", today, "2026-01-01", "")
            .contains("開業日（2026年1月1日）より前の日付です。")
    );
    assert_eq!(date_concern("2026-01-01", today, "2026-01-01", ""), "");
    assert_eq!(date_concern("2025-12-31", today, "", ""), "");
    assert_eq!(date_concern("", today, "2026-01-01", ""), "");
}

#[test]
fn owner_draw_memo_is_reworded_only_for_the_private_share() {
    for (account, memo, expected) in [
        ("owner_draw", "家事按分 30%", "私用の分 30%"),
        ("owner_draw", "家事按分 100%", "私用の分 100%"),
        ("exp_comm", "家事按分 30%", "家事按分 30%"),
        ("owner_draw", "立替の精算", "立替の精算"),
        ("w01", "", ""),
    ] {
        assert_eq!(line_memo(&json!({"account":account,"memo":memo})), expected);
    }
}

#[test]
fn locked_notice_includes_the_closing_date_and_the_inclusive_boundary() {
    let locked = "2026-09-30";
    for date in ["2026-09-30", "2026-09-15"] {
        assert_eq!(
            locked_note(date, locked, false, "", false),
            "2026年9月30日まで締めてあるため、この日付には登録できません。"
        );
    }
    assert_eq!(locked_note("2026-10-01", locked, false, "", false), "");
    for locked in ["", "2026/09/30"] {
        assert_eq!(locked_note("2026-09-15", locked, false, "", false), "");
    }
    assert_eq!(locked_note("", locked, false, "", false), "");
}

#[test]
fn locked_save_errors_explain_how_to_fix_and_preserve_the_original_correction_date() {
    let locked = "2026-09-30";
    assert_eq!(
        locked_note("2026-09-15", locked, false, "", true),
        "2026年9月30日まで締めてあるため、この日付には登録できません。日付を直すか、設定の「締め」で締めた日を見直してください。"
    );
    assert_eq!(
        locked_note("2026-09-15", locked, true, "2026-10-02", false),
        "2026年9月30日まで締めてあるため、この日付には訂正できません。"
    );
    assert_eq!(
        locked_note("2026-10-05", locked, true, "2026-09-10", false),
        "2026年9月30日まで締めてあるため、2026年9月10日の仕訳は訂正できません。"
    );
    assert_eq!(
        locked_note("2026-10-05", locked, true, "2026-09-10", true),
        "2026年9月30日まで締めてあるため、2026年9月10日の仕訳は訂正できません。締めた日は、設定の「締め」で見直せます。"
    );
}

#[test]
fn detail_note_blocks_both_correction_and_void_before_and_on_the_closing_date() {
    assert_eq!(
        locked_detail("2026-10-02", "2026-10-10"),
        "2026年10月10日まで締めてあるため、この仕訳は訂正も取消もできません。締めた日は、設定の「締め」で見直せます。"
    );
    assert_ne!(locked_detail("2026-10-10", "2026-10-10"), "");
    assert_eq!(locked_detail("2026-10-11", "2026-10-10"), "");
    assert_eq!(locked_detail("2026-10-02", ""), "");
    assert_eq!(locked_detail("", "2026-10-10"), "");
}
