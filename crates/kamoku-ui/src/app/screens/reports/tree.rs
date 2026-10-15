// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Presentation hierarchy. Parent summaries never replace server report totals.

use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const DIRECT: &str = "この科目への直接の記帳";
fn text<'a>(v: &'a Value, k: &str) -> &'a str {
    v[k].as_str().unwrap_or("")
}
fn int(v: &Value, k: &str) -> i64 {
    v[k].as_i64().unwrap_or(0)
}
pub(crate) fn chart_key(v: &Value) -> (u8, i64, String) {
    let kind = match text(v, "kind") {
        "asset" => 0,
        "liability" => 1,
        "equity" => 2,
        "revenue" => 3,
        "expense" => 4,
        _ => 9,
    };
    (kind, int(v, "sort"), text(v, "code").into())
}
fn add(target: &mut Value, source: &Value, keys: &[&str]) -> Result<(), String> {
    for key in keys {
        if let Some(values) = source[*key].as_array() {
            let previous = target[*key]
                .as_array()
                .cloned()
                .unwrap_or_else(|| vec![json!(0); values.len()]);
            let mut out = Vec::new();
            for (i, value) in values.iter().enumerate() {
                out.push(json!(
                    previous
                        .get(i)
                        .and_then(Value::as_i64)
                        .unwrap_or(0)
                        .checked_add(value.as_i64().unwrap_or(0))
                        .ok_or("表示する合計が大きすぎます。")?
                ));
            }
            target[*key] = json!(out);
        } else {
            target[*key] = json!(
                int(target, key)
                    .checked_add(int(source, key))
                    .ok_or("表示する合計が大きすぎます。")?
            );
        }
    }
    Ok(())
}
fn visit(
    code: &str,
    depth: usize,
    own: &BTreeMap<String, &Value>,
    by: &BTreeMap<String, &Value>,
    kids: &BTreeMap<String, BTreeSet<String>>,
    keys: &[&str],
    out: &mut Vec<Value>,
) -> Result<Value, String> {
    let mut row = own.get(code).map_or_else(
        || {
            by.get(code)
                .map_or_else(|| json!({"code":code,"name":code}), |a| (*a).clone())
        },
        |a| (*a).clone(),
    );
    row["depth"] = json!(depth);
    row["direct"] = json!(false);
    let children = kids.get(code);
    row["parent"] = json!(children.is_some_and(|c| !c.is_empty()));
    let position = out.len();
    if let Some(children) = children {
        for key in keys {
            row[*key] = json!(0);
        }
        out.push(row.clone());
        if let Some(source) = own.get(code) {
            let nonzero = keys.iter().any(|key| {
                source[*key].as_array().map_or_else(
                    || int(source, key) != 0,
                    |v| v.iter().any(|n| n.as_i64().unwrap_or(0) != 0),
                )
            });
            if nonzero {
                let mut direct = (*source).clone();
                direct["name"] = json!(DIRECT);
                direct["depth"] = json!(depth + 1);
                direct["parent"] = json!(false);
                direct["direct"] = json!(true);
                out.push(direct);
                add(&mut row, source, keys)?;
            }
        }
        let mut ordered: Vec<_> = children.iter().collect();
        ordered.sort_by_key(|c| by.get(*c).map_or((9, 0, (*c).clone()), |a| chart_key(a)));
        for child in ordered {
            let total = visit(child, depth + 1, own, by, kids, keys, out)?;
            add(&mut row, &total, keys)?;
        }
        out[position] = row.clone();
    } else {
        out.push(row.clone());
    }
    Ok(row)
}

pub(crate) fn tree_rows(
    rows: &[Value],
    accounts: &[Value],
    keys: &[&str],
) -> Result<Vec<Value>, String> {
    let by: BTreeMap<_, _> = accounts
        .iter()
        .map(|a| (text(a, "code").to_owned(), a))
        .collect();
    let own: BTreeMap<_, _> = rows
        .iter()
        .map(|a| (text(a, "code").to_owned(), a))
        .collect();
    let mut kids = BTreeMap::<String, BTreeSet<String>>::new();
    let mut roots = BTreeSet::new();
    for row in rows {
        let mut code = text(row, "code").to_owned();
        let mut seen = BTreeSet::new();
        loop {
            if !seen.insert(code.clone()) {
                return Err("科目の親子関係を確認できませんでした。".into());
            }
            let parent = by.get(&code).map_or("", |a| text(a, "parent"));
            if parent.is_empty() || !by.contains_key(parent) {
                roots.insert(code);
                break;
            }
            kids.entry(parent.into()).or_default().insert(code);
            code = parent.into();
        }
    }
    let mut ordered: Vec<_> = roots.into_iter().collect();
    ordered.sort_by_key(|c| by.get(c).map_or((9, 0, c.clone()), |a| chart_key(a)));
    let mut out = Vec::new();
    for code in ordered {
        visit(&code, 0, &own, &by, &kids, keys, &mut out)?;
    }
    Ok(out)
}
pub(crate) fn collapse_rows(rows: &[Value]) -> Vec<Value> {
    let mut current = rows.to_vec();
    loop {
        let mut out = Vec::new();
        let mut at = 0;
        while at < current.len() {
            let mut row = current[at].clone();
            let depth = int(&row, "depth");
            let mut end = at + 1;
            while end < current.len() && int(&current[end], "depth") > depth {
                end += 1;
            }
            if row["parent"] == true
                && end == at + 2
                && current[at + 1]["parent"] != true
                && current[at + 1]["direct"] != true
            {
                let child = &current[at + 1];
                let names: Vec<_> = [text(child, "name"), text(child, "sub")]
                    .into_iter()
                    .filter(|s| !s.is_empty() && *s != text(&row, "name"))
                    .collect();
                if !names.is_empty() {
                    row["sub"] = json!(names.join("・"));
                }
                row["parent"] = json!(false);
                at += 1;
            }
            out.push(row);
            at += 1;
        }
        if out.len() == current.len() {
            return out;
        }
        current = out;
    }
}
pub(crate) fn bs_rows(rows: &[Value]) -> Vec<Value> {
    fn collapse(row: &Value) -> Value {
        let mut out = row.clone();
        let children: Vec<_> = row["children"]
            .as_array()
            .into_iter()
            .flatten()
            .map(collapse)
            .collect();
        if children.len() == 1
            && int(row, "own") == 0
            && children[0]["children"].as_array().is_none_or(Vec::is_empty)
        {
            let names: Vec<_> = [text(&children[0], "name"), text(&children[0], "sub")]
                .into_iter()
                .filter(|s| !s.is_empty() && *s != text(row, "name"))
                .collect();
            if !names.is_empty() {
                out["sub"] = json!(names.join("・"));
            }
            out["children"] = json!([]);
        } else {
            out["children"] = json!(children);
        }
        out
    }
    fn flatten(rows: &[Value], depth: usize, out: &mut Vec<Value>) {
        for source in rows {
            let children = source["children"].as_array().cloned().unwrap_or_default();
            let mut row = source.clone();
            row["depth"] = json!(depth);
            row["parent"] = json!(!children.is_empty());
            row["direct"] = json!(false);
            out.push(row);
            if !children.is_empty() && int(source, "own") != 0 {
                let mut direct = source.clone();
                direct["name"] = json!(DIRECT);
                direct["amount"] = source["own"].clone();
                direct["depth"] = json!(depth + 1);
                direct["parent"] = json!(false);
                direct["direct"] = json!(true);
                out.push(direct);
            }
            flatten(&children, depth + 1, out);
        }
    }
    let collapsed: Vec<_> = rows.iter().map(collapse).collect();
    let mut out = Vec::new();
    flatten(&collapsed, 0, &mut out);
    out
}
pub(crate) fn month_columns(series: &[Value], rows: &[Value], until: &str) -> Vec<usize> {
    series
        .iter()
        .enumerate()
        .filter(|(i, m)| {
            text(m, "month") <= until
                || rows
                    .iter()
                    .any(|r| r["months"].get(*i).and_then(Value::as_i64).unwrap_or(0) != 0)
        })
        .map(|(i, _)| i)
        .collect()
}

pub(crate) fn month_list(months: &[u32]) -> String {
    if months.len() == 2 {
        return format!("{}月と{}月", months[0], months[1]);
    }
    let mut runs: Vec<Vec<u32>> = vec![];
    for month in months {
        if let Some(last) = runs.last_mut()
            && last.last().is_some_and(|m| *m + 1 == *month)
        {
            last.push(*month);
            continue;
        }
        runs.push(vec![*month]);
    }
    runs.into_iter()
        .flat_map(|r| {
            if r.len() >= 3 {
                vec![format!("{}月〜{}月", r[0], r[r.len() - 1])]
            } else {
                r.iter().map(|m| format!("{m}月")).collect()
            }
        })
        .collect::<Vec<_>>()
        .join("・")
}
pub(crate) fn hidden_months_note(series: &[Value], cols: &[usize], this_month: &str) -> String {
    let hidden: Vec<_> = series
        .iter()
        .enumerate()
        .filter(|(i, _)| !cols.contains(i))
        .filter_map(|(_, v)| text(v, "month").get(5..7)?.parse::<u32>().ok())
        .collect();
    if hidden.is_empty() {
        return String::new();
    }
    let current_year = series
        .first()
        .is_some_and(|v| text(v, "month").get(..4) == this_month.get(..4));
    let future = cols.iter().any(|i| {
        series
            .get(*i)
            .is_some_and(|v| text(v, "month") > this_month)
    });
    format!(
        "{}{}は、その月になるか、記録ができたら出ます。",
        if current_year && !future {
            "表は今月までです。"
        } else {
            ""
        },
        month_list(&hidden)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hierarchy_keeps_own_postings_separate_and_never_doubles_server_totals() {
        let accounts = vec![
            json!({"code":"p","name":"親","kind":"asset"}),
            json!({"code":"c","name":"子","parent":"p","kind":"asset"}),
        ];
        let out = tree_rows(
            &[
                json!({"code":"p","name":"親","amount":10}),
                json!({"code":"c","name":"子","amount":20}),
            ],
            &accounts,
            &["amount"],
        )
        .unwrap();
        assert_eq!(out.len(), 3);
        assert_eq!(out[0]["amount"], 30);
        assert_eq!(out[1]["name"], DIRECT);
        assert_eq!(out[2]["depth"], 1);
        assert_eq!(collapse_rows(&out).len(), 3);
    }
    #[test]
    fn single_child_collapse_preserves_names_and_exact_integers() {
        let rows = vec![
            json!({"code":"p","name":"親","amount":9_007_199_254_740_993_i64,"own":0,"children":[{"name":"子","amount":9_007_199_254_740_993_i64,"children":[]}]}),
        ];
        let out = bs_rows(&rows);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0]["sub"], "子");
        assert_eq!(out[0]["amount"], 9_007_199_254_740_993_i64);
    }
    #[test]
    fn future_offsetting_postings_still_make_a_month_visible() {
        let series = vec![
            json!({"month":"2026-10"}),
            json!({"month":"2026-11"}),
            json!({"month":"2026-12"}),
        ];
        let rows = vec![json!({"months":[0,100,0]}), json!({"months":[0,-100,0]})];
        assert_eq!(month_columns(&series, &rows, "2026-10"), vec![0, 1]);
    }
    #[test]
    fn hidden_month_wording_preserves_future_explanation() {
        assert_eq!(month_list(&[11, 12]), "11月と12月");
        assert_eq!(month_list(&[4, 5, 6, 8]), "4月〜6月・8月");
        let series = vec![
            json!({"month":"2026-10"}),
            json!({"month":"2026-11"}),
            json!({"month":"2026-12"}),
        ];
        assert_eq!(
            hidden_months_note(&series, &[0], "2026-10"),
            "表は今月までです。11月と12月は、その月になるか、記録ができたら出ます。"
        );
        assert_eq!(
            hidden_months_note(&series, &[0, 2], "2026-10"),
            "11月は、その月になるか、記録ができたら出ます。"
        );
    }
    #[test]
    fn malformed_parent_cycles_and_presentation_overflow_are_rejected() {
        assert!(
            tree_rows(
                &[json!({"code":"a"})],
                &[
                    json!({"code":"a","parent":"b"}),
                    json!({"code":"b","parent":"a"})
                ],
                &[]
            )
            .is_err()
        );
        let a = vec![
            json!({"code":"a","parent":"p"}),
            json!({"code":"b","parent":"p"}),
            json!({"code":"p"}),
        ];
        assert!(
            tree_rows(
                &[
                    json!({"code":"a","amount":i64::MAX}),
                    json!({"code":"b","amount":1})
                ],
                &a,
                &["amount"]
            )
            .is_err()
        );
    }
    fn accounts() -> Vec<Value> {
        vec![
            json!({"code":"cash","name":"現金","kind":"asset","sort":100,"parent":null}),
            json!({"code":"dep_other","name":"普通預金・その他の預金","kind":"asset","sort":130,"parent":null}),
            json!({"code":"owner_draw","name":"事業主貸","kind":"asset","sort":260,"parent":null}),
            json!({"code":"w01","name":"現金","kind":"asset","sort":1001,"parent":"cash"}),
            json!({"code":"w02","name":"デモ銀行","kind":"asset","sort":1002,"parent":"dep_other"}),
            json!({"code":"w04","name":"デモ電子マネー","kind":"asset","sort":1004,"parent":"cash"}),
            json!({"code":"accrued","name":"未払金","kind":"liability","sort":230,"parent":null}),
        ]
    }
    fn months_at(index: usize, value: i64) -> Vec<i64> {
        (0..12)
            .map(|at| if at == index { value } else { 0 })
            .collect()
    }
    fn series(year: i32) -> Vec<Value> {
        (1..=12)
            .map(|month| json!({"month":format!("{year}-{month:02}")}))
            .collect()
    }
    #[test]
    fn hierarchy_orders_synthetic_parents_sums_month_arrays_and_keeps_kind_before_sort() {
        let source = vec![
            json!({"code":"owner_draw","name":"事業主貸","closing":400}),
            json!({"code":"w01","name":"現金","closing":100}),
            json!({"code":"w02","name":"デモ銀行","closing":2000}),
            json!({"code":"w04","name":"デモ電子マネー","closing":30}),
        ];
        let out = tree_rows(&source, &accounts(), &["closing"]).unwrap();
        assert_eq!(
            out.iter()
                .map(|r| (
                    text(r, "code"),
                    int(r, "depth"),
                    r["parent"].as_bool().unwrap(),
                    int(r, "closing")
                ))
                .collect::<Vec<_>>(),
            vec![
                ("cash", 0, true, 130),
                ("w01", 1, false, 100),
                ("w04", 1, false, 30),
                ("dep_other", 0, true, 2000),
                ("w02", 1, false, 2000),
                ("owner_draw", 0, false, 400)
            ]
        );
        assert_eq!(out[0]["name"], "現金");
        let monthly = tree_rows(
            &[
                json!({"code":"w01","amount":3,"months":months_at(0,3)}),
                json!({"code":"w04","amount":4,"months":months_at(0,4)}),
            ],
            &accounts(),
            &["amount", "months"],
        )
        .unwrap();
        assert_eq!(monthly[0]["amount"], 7);
        assert_eq!(monthly[0]["months"][0], 7);
        assert_eq!(monthly[0]["months"].as_array().unwrap().len(), 12);
        let mut ordered = accounts();
        ordered.sort_by_key(chart_key);
        assert_eq!(
            ordered[..3]
                .iter()
                .map(|a| text(a, "code"))
                .collect::<Vec<_>>(),
            vec!["cash", "dep_other", "owner_draw"]
        );
        assert_eq!(text(ordered.last().unwrap(), "code"), "accrued");
    }
    #[test]
    fn balance_tree_keeps_direct_postings_and_only_collapses_terminal_single_children() {
        let source = vec![
            json!({"code":"cash","name":"現金","amount":145,"own":15,"children":[{"code":"w01","name":"現金","amount":130,"own":130}]}),
            json!({"code":"owner_draw","name":"事業主貸","amount":400,"own":400}),
        ];
        let flat = bs_rows(&source);
        assert_eq!(
            flat.iter()
                .map(|r| (
                    text(r, "name"),
                    int(r, "depth"),
                    int(r, "amount"),
                    r["parent"].as_bool().unwrap()
                ))
                .collect::<Vec<_>>(),
            vec![
                ("現金", 0, 145, true),
                (DIRECT, 1, 15, false),
                ("現金", 1, 130, false),
                ("事業主貸", 0, 400, false)
            ]
        );
        let collapsed = bs_rows(&[
            json!({"code":"cash","name":"現金","amount":49770,"own":0,"children":[{"code":"w01","name":"現金","amount":49770,"own":49770}]}),
            json!({"code":"dep_other","name":"普通預金・その他の預金","amount":4_622_031,"own":0,"children":[{"code":"w02","name":"サンプル銀行 普通預金","amount":4_622_031,"own":4_622_031}]}),
        ]);
        assert_eq!(collapsed.len(), 2);
        assert_eq!(collapsed[0]["parent"], false);
        assert_eq!(text(&collapsed[0], "sub"), "");
        assert_eq!(collapsed[1]["sub"], "サンプル銀行 普通預金");
        assert_eq!(source[0]["children"].as_array().unwrap().len(), 1);
        let deep = json!({"name":"親","amount":30,"own":0,"children":[{"name":"子","amount":30,"own":0,"children":[{"name":"孫1","amount":10,"own":10},{"name":"孫2","amount":20,"own":20}]}]});
        let deep = bs_rows(&[deep]);
        assert_eq!(deep.len(), 4);
        assert_eq!(deep[0]["parent"], true);
        assert_eq!(deep[1]["parent"], true);
        let chain = bs_rows(&[
            json!({"name":"未払金","amount":9,"own":0,"children":[{"name":"未払金","amount":9,"own":0,"children":[{"name":"サンプルカード","amount":9,"own":9}]}]}),
        ]);
        assert_eq!(chain.len(), 1);
        assert_eq!(chain[0]["sub"], "サンプルカード");
        assert_eq!(chain[0]["amount"], 9);
    }
    #[test]
    fn flat_single_chains_preserve_intermediate_names_without_mutating_inputs() {
        let flat=tree_rows(&[json!({"code":"w01","name":"現金","opening":30,"debit":90,"credit":70,"closing":50}),json!({"code":"w02","name":"デモ銀行","opening":1200,"debit":5500,"credit":2080,"closing":4620}),json!({"code":"owner_draw","name":"事業主貸","opening":0,"debit":450,"credit":0,"closing":450})],&accounts(),&["opening","debit","credit","closing"]).unwrap();
        assert_eq!(flat.len(), 5);
        let out = collapse_rows(&flat);
        assert_eq!(
            out.iter()
                .map(|r| (
                    text(r, "code"),
                    text(r, "sub"),
                    int(r, "closing"),
                    int(r, "debit")
                ))
                .collect::<Vec<_>>(),
            vec![
                ("cash", "", 50, 90),
                ("dep_other", "デモ銀行", 4620, 5500),
                ("owner_draw", "", 450, 450)
            ]
        );
        assert_eq!(flat[0]["parent"], true);
        let two = tree_rows(
            &[
                json!({"code":"w01","closing":100}),
                json!({"code":"w04","closing":30}),
            ],
            &accounts(),
            &["closing"],
        )
        .unwrap();
        assert_eq!(collapse_rows(&two).len(), 3);
        let direct = tree_rows(
            &[
                json!({"code":"cash","name":"現金","closing":15}),
                json!({"code":"w01","name":"現金","closing":130}),
            ],
            &accounts(),
            &["closing"],
        )
        .unwrap();
        assert_eq!(
            collapse_rows(&direct)
                .iter()
                .map(|r| text(r, "name"))
                .collect::<Vec<_>>(),
            vec!["現金", DIRECT, "現金"]
        );
        let chain = vec![
            json!({"code":"a","name":"未払金","depth":0,"parent":true,"direct":false,"amount":9}),
            json!({"code":"b","name":"カード","depth":1,"parent":true,"direct":false,"amount":9}),
            json!({"code":"c","name":"サンプルカード","depth":2,"parent":false,"direct":false,"amount":9}),
            json!({"code":"d","name":"事業主借","depth":0,"parent":false,"direct":false,"amount":1}),
        ];
        let out = collapse_rows(&chain);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0]["sub"], "カード・サンプルカード");
        assert_eq!(out[0]["depth"], 0);
        assert_eq!(out[0]["parent"], false);
        assert_eq!(out[0]["amount"], 9);
    }
    #[test]
    fn month_columns_keep_future_and_offsetting_rows_and_explain_every_hidden_month() {
        let rows = [
            json!({"code":"sales","amount":1000,"months":months_at(2,1000)}),
            json!({"code":"exp_comm","amount":1734,"months":(0..12).map(|i|if i==10{1234}else if i==4{500}else{0}).collect::<Vec<_>>()}),
        ];
        let calendar = series(2026);
        let cols = month_columns(&calendar, &rows, "2026-10");
        assert_eq!(cols.len(), 11);
        for row in &rows {
            assert_eq!(
                cols.iter()
                    .map(|i| row["months"][*i].as_i64().unwrap())
                    .sum::<i64>(),
                int(row, "amount")
            );
        }
        assert_eq!(
            month_columns(&series(2027), &rows, "2027-00"),
            vec![2, 4, 10]
        );
        let transfer = [
            json!({"months":months_at(6,10000)}),
            json!({"months":months_at(6,-10000)}),
        ];
        assert!(month_columns(&calendar, &transfer, "2026-01").contains(&6));
        for (input, expected) in [
            (&[12][..], "12月"),
            (&[11, 12][..], "11月と12月"),
            (&[4, 5, 6, 7][..], "4月〜7月"),
            (&[4, 6][..], "4月と6月"),
            (&[2, 3, 5][..], "2月・3月・5月"),
            (&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 12][..], "1月〜10月・12月"),
        ] {
            assert_eq!(month_list(input), expected);
        }
        let empty = vec![];
        assert_eq!(
            hidden_months_note(
                &calendar,
                &month_columns(&calendar, &empty, "2026-10"),
                "2026-10"
            ),
            "表は今月までです。11月と12月は、その月になるか、記録ができたら出ます。"
        );
        assert_eq!(
            hidden_months_note(
                &calendar,
                &month_columns(&calendar, &empty, "2026-12"),
                "2026-12"
            ),
            ""
        );
        assert_eq!(
            hidden_months_note(
                &series(2027),
                &month_columns(&series(2027), &rows, "2027-00"),
                "2026-10"
            ),
            "1月と2月・4月・6月〜10月・12月は、その月になるか、記録ができたら出ます。"
                .replace("1月と2月", "1月・2月")
        );
        assert_eq!(
            hidden_months_note(&series(2027), &[], "2026-10"),
            "1月〜12月は、その月になるか、記録ができたら出ます。"
        );
        assert_eq!(
            hidden_months_note(
                &series(2025),
                &month_columns(&series(2025), &rows, "2025-12"),
                "2026-10"
            ),
            ""
        );
    }
}
