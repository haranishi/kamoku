// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;
use serde_json::json;

// AC-05.3 SC-03: selection is independent of visible pages and remains ordered.
#[test]
fn selection_is_exact_ordered_and_deduplicated() {
    let mut s = Selection::default();
    let a = json!({"id":9,"direction":"out","amount":i64::MAX});
    let b = json!({"id":3,"direction":"in","amount":55000});
    let c = json!({"id":4,"direction":"out","amount":i64::MAX});
    for row in [&a, &b, &c, &a] {
        s.set(row, true);
    }
    assert_eq!(s.ids(), [9, 3, 4]);
    assert_eq!(
        s.summary(),
        Summary {
            count: 3,
            incoming: 55000,
            outgoing: i128::from(i64::MAX) * 2
        }
    );
    s.remove(3);
    s.keep(&[9, 12]);
    assert_eq!(s.ids(), [9]);
    s.keep(&[9]);
    assert_eq!(s.ids(), [9]);
    s.set(&a, false);
    assert_eq!(s.summary(), Summary::default());
    assert!(!s.contains(9));
    assert_eq!(
        group(i128::MIN),
        "-170,141,183,460,469,231,731,687,303,715,884,105,728"
    );
}

// SC-03 g2-1: a direction-incompatible default must not become a manual value.
#[test]
fn account_tax_follows_defaults_until_manually_selected() {
    let comm = json!({"kind":"expense","default_tax":"P10","default_pct":70});
    let sale = json!({"kind":"revenue","default_tax":"S10"});
    let empty = json!({"kind":"expense","default_tax":null});
    let wallet = json!({"wallet":{"type":"card"}});
    assert_eq!(tax_after(&comm, true, false, "", false), "");
    assert_eq!(tax_after(&sale, true, false, "", false), "S10");
    assert_eq!(tax_after(&empty, false, false, "P8", true), "P8");
    assert_eq!(tax_after(&wallet, false, false, "P8", true), "");
    assert_eq!(tax_after(&comm, false, true, "P8", true), "");
    assert!(!tax_touched(&comm, false, false, "P10"));
    assert!(tax_touched(&comm, false, false, "P8"));
    assert!(tax_touched(&sale, true, false, ""));
    assert!(tax_touched(&Value::Null, false, false, "P8"));
    assert!(!tax_touched(&Value::Null, false, false, ""));
    assert!(!tax_touched(&comm, false, true, "P8"));
    assert!(!tax_differs(&comm, false, true, "P8"));
    assert!(!tax_differs(&comm, false, false, "P10"));
    assert!(tax_differs(&comm, false, false, "P8"));
    assert_eq!(default_tax(&comm, false, false), "P10");
    assert_eq!(default_tax(&comm, true, false), "");
    assert_eq!(default_tax(&sale, true, false), "S10");
    assert_eq!(default_tax(&wallet, false, false), "");
    assert_eq!(default_tax(&Value::Null, false, false), "");
}

fn pick_sequence(
    income: bool,
    accounts: &[Value],
    tax: &str,
    touched: bool,
    exempt: bool,
) -> Vec<String> {
    let mut tax = tax.to_owned();
    let mut touched = touched;
    accounts
        .iter()
        .map(|account| {
            if is_wallet(account) {
                touched = false;
            }
            tax = tax_after(account, income, exempt, &tax, touched);
            tax.clone()
        })
        .collect()
}
#[test]
fn direction_incompatible_defaults_never_turn_into_manual_values_across_account_changes() {
    let comm = json!({"kind":"expense","default_tax":"P10"});
    let sales = json!({"kind":"revenue","default_tax":"S10"});
    let travel = json!({"kind":"expense","default_tax":null});
    let misc = json!({"kind":"revenue","default_tax":null});
    let wallet = json!({"wallet":{"type":"card"}});
    assert_eq!(
        pick_sequence(true, &[comm.clone(), sales.clone()], "", false, false),
        ["", "S10"]
    );
    assert_eq!(
        pick_sequence(
            true,
            &[sales.clone(), misc, sales.clone()],
            "",
            false,
            false
        ),
        ["S10", "", "S10"]
    );
    assert_eq!(
        pick_sequence(
            false,
            &[sales.clone(), comm.clone(), travel.clone()],
            "",
            false,
            false
        ),
        ["", "P10", ""]
    );
    assert_eq!(
        pick_sequence(false, &[comm.clone(), travel], "P8", true, false),
        ["P8", "P8"]
    );
    assert_eq!(
        pick_sequence(false, &[wallet, comm.clone()], "P8", true, false),
        ["", "P10"]
    );
    assert_eq!(
        pick_sequence(false, &[comm, sales], "", false, true),
        ["", ""]
    );
}

#[test]
fn tax_names_placeholders_and_direction_warnings_match_recording_screen() {
    let travel = json!({"kind":"expense","default_tax":null});
    let comm = json!({"kind":"expense","default_tax":"P10"});
    assert_eq!(
        unset_tax(&Value::Null, false, false),
        (
            "科目で決まる".into(),
            "科目を選ぶと、科目の既定の税区分が入ります".into()
        )
    );
    assert_eq!(
        unset_tax(&travel, true, false),
        (
            "対象外（既定）".into(),
            "この科目の既定の税区分は「対象外」です。このまま確定すると「対象外」になります。"
                .into()
        )
    );
    assert_eq!(unset_tax(&comm, true, true), ("対象外".into(), "この科目の既定の税区分（課税仕入10%）は、入金の行では使えません。このまま確定すると「対象外」になります。".into()));
    assert_eq!(
        unset_tax(&Value::Null, true, false),
        (
            "対象外".into(),
            "このまま確定すると「対象外」になります。".into()
        )
    );
    assert!(direction_warning(&travel, true).starts_with("入金の行に、費用"));
    assert_eq!(direction_warning(&travel, false), "");
    let sales = json!({"kind":"revenue"});
    assert!(direction_warning(&sales, false).starts_with("出金の行に、収益"));
    assert_eq!(direction_warning(&sales, true), "");
    assert_eq!(
        direction_warning(&json!({"wallet":{"type":"card"}}), false),
        ""
    );
    assert_eq!(direction_warning(&Value::Null, true), "");
    assert_eq!(
        tax_diff_text(&comm, false, false),
        "税区分が、科目の既定（課税仕入10%）と違います。"
    );
    assert_eq!(
        tax_diff_text(&travel, false, false),
        "税区分が、科目の既定（対象外）と違います。"
    );
    for (code, short) in [
        ("S10", "課税売上10%"),
        ("S8", "課税売上8%（軽減）"),
        ("P10", "課税仕入10%"),
        ("P8", "課税仕入8%（軽減）"),
        ("P10X", "経過措置の仕入10%"),
        ("P8X", "経過措置の仕入8%"),
        ("NT", "非課税"),
        ("", "対象外"),
    ] {
        assert_eq!(tax_choice_label(code), short);
        assert_eq!(
            tax_label(code).strip_prefix(short).unwrap(),
            if ["P10X", "P8X"].contains(&code) {
                "（免税事業者等から）"
            } else {
                ""
            }
        );
    }
}

// SC-03 g2-2: native select arrows skip hidden/disabled placeholders and stop at edges.
#[test]
fn tax_arrows_skip_unselectable_options_and_do_not_wrap() {
    let options = [false, true, true, true, true];
    assert_eq!(step_option(&options, 0, 1), Some(1));
    assert_eq!(step_option(&options, 1, 1), Some(2));
    assert_eq!(step_option(&options, 4, 1), None);
    assert_eq!(step_option(&options, 1, -1), None);
    assert_eq!(step_option(&options, 3, -1), Some(2));
    assert_eq!(step_option(&[true, false, true], 0, 1), Some(2));
    assert_eq!(step_option(&[true, false, true], 2, -1), Some(0));
    assert_eq!(step_option(&[], -1, 1), None);
    assert_eq!(step_option(&options, -1, 1), Some(1));
    assert_eq!(step_option(&options, 1, 0), None);
}

#[test]
fn confirmation_classifies_terminal_failures_and_keeps_exact_batch_boundaries() {
    assert!(gone("already_confirmed"));
    assert!(gone("not_found"));
    assert!(!gone("invalid"));
    for (n, expected) in [
        (0, vec![]),
        (2000, vec![2000]),
        (2001, vec![2000, 1]),
        (4001, vec![2000, 2000, 1]),
    ] {
        let ids: Vec<_> = (0..n).collect();
        let mut batches = Confirmation::new(ids.clone());
        let mut sizes = vec![];
        let mut sent = vec![];
        while let Some(chunk) = batches.take_next() {
            sizes.push(chunk.len());
            sent.extend_from_slice(&chunk);
            assert!(batches.take_next().is_none());
            let confirmed: Vec<_> = chunk
                .iter()
                .map(|key| json!({"id":key,"no":key+100}))
                .collect();
            let result = json!({"confirmed":confirmed,"failed":[]});
            batches.complete(&result);
            batches.complete(&result); // A duplicate completion cannot duplicate results.
        }
        assert_eq!(sizes, expected);
        assert_eq!(sent, ids);
        assert_eq!(batches.confirmed.iter().map(id).collect::<Vec<_>>(), ids);
        assert_eq!(batches.failed, [] as [serde_json::Value; 0]);
    }
    assert_eq!(confirmed_nos(&[212]), "No.212");
    assert_eq!(confirmed_nos(&[225, 206, 210]), "No.206〜No.225");
    assert_eq!(confirmed_nos(&[]), "");
}

#[test]
fn mixed_confirm_outcome_unmarks_only_successes_and_terminal_failures() {
    let confirmed = vec![json!({"id":1,"no":101})];
    let failed = vec![
        json!({"id":2,"error":{"code":"already_confirmed","message":"確定済み"}}),
        json!({"id":3,"error":{"code":"not_found","message":"見つかりません"}}),
        json!({"id":4,"error":{"code":"missing_account","message":"科目がありません","field":"account"}}),
        json!({"id":5,"error":{"code":"invalid","message":"税区分","field":"tax_code"}}),
    ];
    let outcome = confirm_outcome(&confirmed, &failed);
    assert_eq!(outcome.unmark, [1, 2, 3]);
    assert_eq!(outcome.gone, failed[..2]);
    assert_eq!(outcome.fixable, failed[2..]);
    assert_eq!(
        confirm_outcome(&[], &[]),
        Outcome {
            unmark: vec![],
            gone: vec![],
            fixable: vec![]
        }
    );
}

#[test]
fn confirmation_waits_for_each_response_and_keeps_prior_partial_results_on_transport_failure() {
    let mut pending = Confirmation::new((1..=4001).collect());
    assert_eq!(pending.take_next().unwrap(), (1..=2000).collect::<Vec<_>>());
    assert!(
        pending.take_next().is_none(),
        "a second request must wait for the first response"
    );
    pending.complete(&json!({"confirmed":[{"id":1,"no":101}],"failed":[{"id":2,"error":{"code":"invalid","field":"tax_code","message":"税区分"}}]}));
    assert_eq!(
        pending.take_next().unwrap(),
        (2001..=4000).collect::<Vec<_>>()
    );
    pending.stop();
    assert!(
        pending.take_next().is_none(),
        "transport failure must stop later chunks"
    );
    assert_eq!(pending.confirmed, vec![json!({"id":1,"no":101})]);
    assert_eq!(pending.failed[0]["error"]["field"], "tax_code");
    let mut empty = Confirmation::new(vec![]);
    assert!(empty.take_next().is_none());
}

#[test]
fn row_labels_and_announcements_include_identity_and_separate_directions() {
    assert_eq!(
        row_label(
            &json!({"date":"2026-09-18","description":"デモ電器","amount":7470}),
            "2026"
        ),
        "09/18 デモ電器 7,470円"
    );
    assert_eq!(
        row_label(&json!({"date":"2025-12-30","amount":120_000}), "2026"),
        "2025/12/30 摘要なし 120,000円"
    );
    assert_eq!(
        Summary {
            count: 3,
            incoming: 0,
            outgoing: 3300
        }
        .speech(),
        "確定する行 3件、入金 0円、出金 3,300円"
    );
    assert_eq!(
        signed(&json!({"direction":"out","amount":i64::MAX})),
        "-9,223,372,036,854,775,807"
    );
}

#[test]
fn keyboard_and_layout_preserve_cell_order_and_text_undo() {
    assert_eq!(cell_order(599.).0, "stack");
    assert_eq!(cell_order(600.).0, "cards");
    assert_eq!(cell_order(1080.).0, "table");
    assert_eq!(
        &cell_order(390.).1[..6],
        &["cand", "desc", "amount", "ops", "account", "more"]
    );
    assert!(is_text_entry("INPUT", "search", false));
    assert!(is_text_entry("INPUT", "text", false));
    assert!(is_text_entry("TEXTAREA", "", false));
    assert!(!is_text_entry("INPUT", "checkbox", false));
    assert!(!is_text_entry("SELECT", "", false));
    assert!(!is_text_entry("BUTTON", "", false));
    assert!(!is_text_entry("", "", false));
    assert!(is_text_entry("DIV", "", true));
    let modifiers = |meta, ctrl, extra| UndoModifiers { meta, ctrl, extra };
    assert!(undo_key("z", true, modifiers(true, false, false)));
    assert!(!undo_key("z", true, modifiers(false, true, false)));
    assert!(undo_key("Z", false, modifiers(false, true, false)));
    assert!(!undo_key("Z", false, modifiers(false, true, true)));
    assert!(!undo_key("Z", true, modifiers(true, false, true)));
    assert!(!undo_key("y", true, modifiers(true, false, false)));
    assert_eq!(pct_value("１００"), Some(100));
    for invalid in ["", "-1", "101", "5e1", "12.5"] {
        assert_eq!(pct_value(invalid), None);
    }
}
