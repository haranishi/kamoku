// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde_json::{Value, json};
use std::collections::BTreeSet;

pub(super) const CSV_LIMIT: u32 = 10 * 1024 * 1024;
pub(super) fn text<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or("")
}
pub(super) fn array(v: &Value) -> &[Value] {
    v.as_array().map_or(&[], Vec::as_slice)
}
pub(super) fn group(n: i128) -> String {
    let raw = n.to_string();
    let digits = raw.trim_start_matches('-');
    let mut out = String::new();
    if n < 0 {
        out.push('-');
    }
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}
pub(super) fn encoding_label(code: &str) -> &str {
    match code {
        "utf-8" | "utf8" => "UTF-8",
        "shift_jis" => "Shift_JIS",
        "auto" => "自動で判定",
        "" => "不明",
        _ => code,
    }
}
pub(super) fn delimiter_label(d: &str) -> String {
    match d {
        "," => "カンマ".into(),
        "\t" => "タブ".into(),
        ";" => "セミコロン".into(),
        _ => format!("「{d}」"),
    }
}
pub(super) fn column_count(up: &Value) -> usize {
    array(&up["sample"])
        .iter()
        .map(|r| array(r).len())
        .chain([array(&up["header"]).len(), 1])
        .max()
        .unwrap_or(1)
}
#[derive(Debug, PartialEq)]
pub(super) struct Column {
    pub value: usize,
    pub name: String,
    pub label: String,
    pub example: String,
}
pub(super) fn column_options(up: &Value) -> Vec<Column> {
    let header = array(&up["header"]);
    let first = array(&up["sample"]).iter().find(|r| {
        array(r)
            .iter()
            .any(|c| !c.as_str().unwrap_or("").trim().is_empty())
    });
    (0..column_count(up))
        .map(|i| {
            let name = header
                .get(i)
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_owned();
            let value = first
                .and_then(|r| r.get(i))
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            Column {
                value: i,
                label: if name.is_empty() {
                    format!("{}列目", i + 1)
                } else {
                    format!("{}列目 {name}", i + 1)
                },
                example: if value == name {
                    String::new()
                } else {
                    value.chars().take(24).collect()
                },
                name,
            }
        })
        .collect()
}
pub(super) fn column_examples(up: &Value, col: usize, limit: usize) -> Vec<String> {
    let name = up["header"][col].as_str().unwrap_or("").trim();
    array(&up["sample"])
        .iter()
        .filter_map(|r| r[col].as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty() && *s != name)
        .take(limit)
        .map(str::to_owned)
        .collect()
}
pub(super) fn is_number_column(up: &Value, col: usize) -> bool {
    let values = column_examples(up, col, 20);
    !values.is_empty()
        && values.iter().all(|s| {
            let first = s.chars().next();
            let s = first
                .filter(|c| ['-', '+', '−', '△', '▲'].contains(c))
                .map_or(s.as_str(), |c| &s[c.len_utf8()..]);
            let (integer, fraction) = s.split_once('.').map_or((s, None), |(a, b)| (a, Some(b)));
            !integer.is_empty()
                && integer
                    .chars()
                    .all(|c| c.is_ascii_digit() || [',', '，'].contains(&c))
                && fraction.is_none_or(|s| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit()))
        })
}
pub(super) fn fallback_mapping(up: &Value) -> Value {
    let n = column_count(up);
    json!({"skip_rows":u8::from(up["header"].is_array()),"date_col":0,"desc_cols":if n>1{vec![1]}else{vec![]},"id_col":null,"amount":{"mode":"signed","col":2.min(n-1),"inflow":"positive"}})
}
pub(super) fn initial_mapping(up: &Value, kept: Option<&Value>) -> (Value, &'static str) {
    if let Some(m) = kept {
        return (m.clone(), "kept");
    }
    let n = column_count(up) as u64;
    for (source, m) in [("saved", &up["saved"]), ("guess", &up["guess"])] {
        let fits = m.is_object()
            && [
                &m["date_col"],
                &m["amount"]["col"],
                &m["amount"]["in_col"],
                &m["amount"]["out_col"],
            ]
            .into_iter()
            .chain(array(&m["desc_cols"]))
            .all(|v| v.is_null() || v.as_u64().is_some_and(|v| v < n));
        if fits {
            return (m.clone(), source);
        }
    }
    (fallback_mapping(up), "none")
}
pub(super) fn describe_source(source: &str) -> &'static str {
    match source {
        "kept" => {
            "直前に選んだ列の対応のままです。直したら、もう一度「確かめる」を押してください。"
        }
        "saved" => "この口座で前に使った列の対応を入れました。違っていたら直してください。",
        "guess" => "見出しの言葉から、列の対応を推測しました。見本と見比べて確かめてください。",
        _ => "見出しから列を推測できませんでした。日付・摘要・金額の列を選んでください。",
    }
}
pub(super) fn mapping_problems(m: &Value, n: usize) -> Vec<(&'static str, &'static str)> {
    let mut out = vec![];
    let in_range = |v: &Value| v.as_u64().is_some_and(|i| i < (n as u64));
    if !in_range(&m["date_col"]) {
        out.push(("date_col", "日付の列を選んでください。"));
    }
    if m["skip_rows"].as_u64().is_none() {
        out.push((
            "skip_rows",
            "読み飛ばす行の数は、0以上の整数にしてください。",
        ));
    }
    let a = &m["amount"];
    let cols = if text(a, "mode") == "split" {
        if !in_range(&a["out_col"]) || !in_range(&a["in_col"]) {
            out.push(("amount", "出金と入金の列を選んでください。"));
        } else if a["out_col"] == a["in_col"] {
            out.push(("amount","出金と入金に、同じ列が選ばれています。別々の列を選ぶか、「符号つきの1列」にしてください。"));
        }
        vec![&a["out_col"], &a["in_col"]]
    } else {
        if !in_range(&a["col"]) {
            out.push(("amount", "金額の列を選んでください。"));
        }
        vec![&a["col"]]
    };
    if cols.contains(&&m["date_col"]) {
        out.push(("date_col", "日付と金額に、同じ列が選ばれています。"));
    }
    if !m["id_col"].is_null() && !in_range(&m["id_col"]) {
        out.push(("id_col", "取引番号の列を選び直してください。"));
    }
    out
}
fn breaks(cells: &Value) -> u64 {
    array(cells)
        .iter()
        .filter_map(Value::as_str)
        .map(|s| {
            s.replace("\r\n", "\n")
                .replace('\r', "\n")
                .bytes()
                .filter(|b| *b == b'\n')
                .count() as u64
        })
        .sum()
}
pub(super) fn sample_lines(up: &Value) -> Vec<u64> {
    let mut line = up["guess"]["skip_rows"].as_u64().unwrap_or(0) + 1 + breaks(&up["header"]);
    array(&up["sample"])
        .iter()
        .map(|r| {
            let at = line;
            line += 1 + breaks(r);
            at
        })
        .collect()
}
pub(super) fn sample_skipped(up: &Value, skip: Option<u64>, i: usize) -> bool {
    skip.is_some_and(|s| up["guess"]["skip_rows"].as_u64().unwrap_or(0) + (i as u64) < s)
}
pub(super) fn roles_of(m: &Value, i: usize) -> Vec<&'static str> {
    let i = json!(i);
    let mut out = vec![];
    if m["date_col"] == i {
        out.push("日付");
    }
    if array(&m["desc_cols"]).contains(&i) {
        out.push("摘要");
    }
    let a = &m["amount"];
    if text(a, "mode") == "split" {
        if a["out_col"] == i {
            out.push("出金");
        }
        if a["in_col"] == i {
            out.push("入金");
        }
    } else if a["col"] == i {
        out.push("金額");
    }
    if m["id_col"] == i {
        out.push("取引番号");
    }
    out
}
pub(super) fn date_warning(rows: &[Value]) -> String {
    let bad = rows
        .iter()
        .filter(|r| text(r, "status") == "invalid" && text(r, "reason").starts_with("日付"))
        .count();
    if rows.is_empty() || (rows.len() - bad) * 2 >= rows.len() {
        String::new()
    } else {
        format!(
            "この列は日付に読めない行が多いようです（{}行のうち{bad}行）。日付の列を選び直してください。",
            rows.len()
        )
    }
}
pub(super) fn counts_text(counts: &Value) -> String {
    format!(
        "新規 {}件・重複 {}件・読めない {}件",
        counts["new"].as_u64().unwrap_or(0),
        counts["duplicate"].as_u64().unwrap_or(0),
        counts["invalid"].as_u64().unwrap_or(0)
    )
}
pub(super) fn summary_text(rows: &[Value], force: &BTreeSet<u64>, truncated: bool) -> String {
    let take: Vec<_> = rows
        .iter()
        .filter(|r| {
            text(r, "status") == "new"
                || text(r, "status") == "duplicate"
                    && force.contains(&r["line"].as_u64().unwrap_or(0))
        })
        .collect();
    if take.is_empty() {
        return String::new();
    }
    let mut dates: Vec<_> = take
        .iter()
        .map(|r| text(r, "date"))
        .filter(|s| !s.is_empty())
        .collect();
    dates.sort_unstable();
    let from = dates.first().copied().unwrap_or("");
    let to = dates.last().copied().unwrap_or("");
    let mut parts = vec![if from == to {
        from.into()
    } else {
        format!("{from}〜{to}")
    }];
    for (dir, label) in [("out", "出金"), ("in", "入金")] {
        let selected: Vec<_> = take
            .iter()
            .filter(|r| text(r, "direction") == dir)
            .collect();
        if !selected.is_empty() {
            let sum: i128 = selected
                .iter()
                .map(|r| i128::from(r["amount"].as_i64().unwrap_or(0)))
                .sum();
            parts.push(format!(
                "{label} {}件 {}円",
                group(selected.len() as i128),
                group(sum)
            ));
        }
    }
    format!(
        "{}取り込む{}件: {}",
        if truncated {
            "表に出している行のうち、"
        } else {
            ""
        },
        group(take.len() as i128),
        parts.join("・")
    )
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
