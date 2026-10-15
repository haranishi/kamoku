// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;
fn upload() -> Value {
    json!({"header":["日付","摘要","出金","入金","残高"],"sample":[["2026/10/01","サンプルストア","1,650","","98,350"]],"guess":{"skip_rows":1,"date_col":0,"desc_cols":[1],"id_col":null,"amount":{"mode":"split","out_col":2,"in_col":3}}})
}

// AC-04.3: saved -> guessed -> fallback; back preserves a copy of user choices.
#[test]
fn mapping_sources_preserve_values_and_reject_out_of_range_saved_columns() {
    let mut up = upload();
    assert_eq!(initial_mapping(&up, None).1, "guess");
    let mut guessed = initial_mapping(&up, None).0;
    guessed["desc_cols"] = json!([1, 4]);
    guessed["amount"]["out_col"] = json!(4);
    assert_eq!(up["guess"]["desc_cols"], json!([1]));
    assert_eq!(up["guess"]["amount"]["out_col"], json!(2));
    up["saved"] = up["guess"].clone();
    up["saved"]["desc_cols"] = json!([1, 4]);
    let (mut saved, source) = initial_mapping(&up, None);
    assert_eq!(source, "saved");
    saved["desc_cols"] = json!([0]);
    assert_eq!(up["saved"]["desc_cols"], json!([1, 4]));
    let (mut kept, source) = initial_mapping(&up, Some(&saved));
    assert_eq!(source, "kept");
    assert_eq!(kept, saved);
    kept["amount"]["out_col"] = json!(4);
    assert_eq!(saved["amount"]["out_col"], json!(2));
    assert_eq!(
        describe_source(source),
        "直前に選んだ列の対応のままです。直したら、もう一度「確かめる」を押してください。"
    );
    up["saved"]["date_col"] = json!(9);
    assert_eq!(initial_mapping(&up, None).1, "guess");
    up["guess"] = Value::Null;
    up["header"] = Value::Null;
    assert_eq!(initial_mapping(&up, None).1, "none");
    assert!(describe_source("none").contains("推測できませんでした"));
    assert_eq!(initial_mapping(&up, None).0["amount"]["mode"], "signed");
    assert_eq!(
        fallback_mapping(&json!({"sample":[["a"]]}))["amount"]["col"],
        0
    );
}
#[test]
fn column_labels_examples_and_alignment_do_not_include_long_examples_in_selects() {
    let up = upload();
    let cols = column_options(&up);
    assert_eq!(cols.len(), 5);
    assert_eq!(cols[2].label, "3列目 出金");
    assert_eq!(cols[2].example, "1,650");
    assert_eq!(cols[3].example, "");
    assert_eq!(cols[3].label, "4列目 入金");
    assert_eq!(
        column_options(&json!({"sample":[["20261001"]]}))[0].label,
        "1列目"
    );
    assert_eq!(column_examples(&up, 2, 3), ["1,650"]);
    assert!(is_number_column(&up, 2));
    assert!(!is_number_column(&up, 0));
    assert!(!is_number_column(&json!({"sample":[[""]]}), 0));
    assert_eq!(encoding_label("shift_jis"), "Shift_JIS");
    assert_eq!(delimiter_label("\t"), "タブ");
    assert_eq!(encoding_label("utf-8"), "UTF-8");
    assert_eq!(
        counts_text(&json!({"new":300,"duplicate":10,"invalid":2})),
        "新規 300件・重複 10件・読めない 2件"
    );
    assert_eq!(fallback_mapping(&json!({"sample":[["a"]]}))["date_col"], 0);
}

#[test]
fn sample_examples_preserve_each_value_and_numeric_alignment_uses_the_legacy_pattern() {
    let up = json!({"header":["日付","摘要","出金","残高"],"sample":[["2026/10/01","ストア","1,650","98350"],["2026/10/02","カフェ","","-5,400"],["2026/10/03","鉄道","300","92950"]]});
    assert_eq!(column_examples(&up, 2, 3), ["1,650", "300"]);
    assert_eq!(column_examples(&up, 1, 2), ["ストア", "カフェ"]);
    assert!(is_number_column(&up, 2));
    assert!(is_number_column(&up, 3));
    assert!(!is_number_column(&up, 0));
    assert!(!is_number_column(&up, 1));
    for valid in ["1,650", "-5,400", "+1，200.50", "−2", "△3", "▲4"] {
        assert!(is_number_column(&json!({"sample":[[valid]]}), 0), "{valid}");
    }
    for invalid in ["", "2026/10/01", "--1", "−▲1", ".5", "1.", "1.2,3"] {
        assert!(
            !is_number_column(&json!({"sample":[[invalid]]}), 0),
            "{invalid}"
        );
    }
}
#[test]
fn mapping_validation_rejects_clashing_missing_negative_and_fractional_columns() {
    let mut m = upload()["guess"].clone();
    assert_eq!(mapping_problems(&m, 5), [] as [(&str, &str); 0]);
    m["amount"]["in_col"] = json!(2);
    assert!(mapping_problems(&m, 5).iter().any(|p| p.0 == "amount"));
    m["date_col"] = json!(2);
    assert!(mapping_problems(&m, 5).iter().any(|p| p.0 == "date_col"));
    let clash = json!({"skip_rows":1,"date_col":0,"desc_cols":[1],"id_col":null,"amount":{"mode":"signed","col":0,"inflow":"positive"}});
    assert!(
        mapping_problems(&clash, 5)
            .iter()
            .any(|p| p.0 == "date_col")
    );
    m["skip_rows"] = json!(-1);
    m["id_col"] = json!(6);
    assert!(mapping_problems(&m, 5).iter().any(|p| p.0 == "skip_rows"));
    assert!(mapping_problems(&m, 5).iter().any(|p| p.0 == "id_col"));
    m["date_col"] = json!(0.5);
    assert!(mapping_problems(&m, 5).iter().any(|p| p.0 == "date_col"));
}
// SC-04: physical CSV lines count quoted CR/LF; skipped sample rows are marked.
#[test]
fn samples_keep_physical_line_numbers_and_roles() {
    let mut up = upload();
    up["sample"] = json!([["a"], ["two\r\nlines"], ["b\rc"], ["d"]]);
    assert_eq!(sample_lines(&up), [2, 3, 5, 7]);
    assert_eq!(sample_lines(&json!({"sample":[["a"],["b"]]})), [1, 2]);
    let mut skipped = upload();
    skipped["guess"]["skip_rows"] = json!(2);
    skipped["sample"] = json!([["a"], ["b"]]);
    assert_eq!(sample_lines(&skipped), [3, 4]);
    assert!(!sample_skipped(&up, Some(1), 0));
    assert!(sample_skipped(&up, Some(2), 0));
    assert!(!sample_skipped(&up, None, 0));
    assert!(!sample_skipped(&up, Some(2), 1));
    assert!(sample_skipped(&json!({"sample":[["a"]]}), Some(1), 0));
    assert_eq!(roles_of(&up["guess"], 2), ["出金"]);
    assert_eq!(roles_of(&up["guess"], 1), ["摘要"]);
}
#[test]
fn date_warning_requires_more_than_half_server_date_failures() {
    let bad = json!({"status":"invalid","reason":"日付が空です。"});
    let other = json!({"status":"invalid","reason":"金額が空です。"});
    assert_eq!(date_warning(&[bad.clone(), other.clone()]), "");
    assert_eq!(
        date_warning(&[bad.clone(), bad, other]),
        "この列は日付に読めない行が多いようです（3行のうち2行）。日付の列を選び直してください。"
    );
    assert_eq!(date_warning(&[]), "");
    assert_eq!(
        date_warning(&[json!({"status":"duplicate","reason":"取り込み済みの明細と同じです。"})]),
        ""
    );
    let all_bad: [Value; 3] =
        std::array::from_fn(|_| json!({"status":"invalid","reason":"日付が空です。"}));
    assert_eq!(
        date_warning(&all_bad),
        "この列は日付に読めない行が多いようです（3行のうち3行）。日付の列を選び直してください。"
    );
}
#[test]
fn import_summary_excludes_invalid_and_unselected_duplicates_and_is_exact() {
    let rows = json!([{"line":2,"status":"new","date":"2026-09-03","direction":"out","amount":880},{"line":3,"status":"new","date":"2026-09-01","direction":"out","amount":1100},{"line":4,"status":"new","date":"2026-09-30","direction":"in","amount":118_640},{"line":5,"status":"duplicate","date":"2026-08-31","direction":"out","amount":500},{"status":"invalid","amount":999}]);
    assert_eq!(
        summary_text(array(&rows), &BTreeSet::new(), false),
        "取り込む3件: 2026-09-01〜2026-09-30・出金 2件 1,980円・入金 1件 118,640円"
    );
    assert_eq!(
        summary_text(array(&rows), &BTreeSet::from([5]), false),
        "取り込む4件: 2026-08-31〜2026-09-30・出金 3件 2,480円・入金 1件 118,640円"
    );
    let big = json!([{"status":"new","date":"2026-09-03","direction":"out","amount":i64::MAX},{"status":"new","date":"2026-09-03","direction":"out","amount":i64::MAX}]);
    assert!(
        summary_text(array(&big), &BTreeSet::new(), true).contains("18,446,744,073,709,551,614円")
    );
    assert_eq!(summary_text(&[], &BTreeSet::new(), false), "");
    assert_eq!(
        summary_text(&array(&rows)[3..], &BTreeSet::new(), false),
        ""
    );
    assert_eq!(
        summary_text(&array(&rows)[..1], &BTreeSet::new(), false),
        "取り込む1件: 2026-09-03・出金 1件 880円"
    );
    assert_eq!(
        summary_text(&array(&rows)[..1], &BTreeSet::new(), true),
        "表に出している行のうち、取り込む1件: 2026-09-03・出金 1件 880円"
    );
}
