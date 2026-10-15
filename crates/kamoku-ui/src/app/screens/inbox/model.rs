// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde_json::Value;

pub(super) const PAGE_SIZE: u64 = 200;
pub(super) const CONFIRM_LIMIT: usize = 2000;
pub(super) const PCT_NOTE: &str = "仕事に使った分の割合。残りは事業主貸に入ります。";
pub(super) const COLUMNS: [&str; 10] = [
    "cand", "date", "desc", "amount", "account", "tax", "partner", "memo", "pct", "ops",
];

pub(super) fn text<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or("")
}
pub(super) fn items(v: &Value) -> Vec<Value> {
    v.as_array()
        .or_else(|| v["items"].as_array())
        .cloned()
        .unwrap_or_default()
}
pub(super) fn id(v: &Value) -> u64 {
    v["id"].as_u64().unwrap_or(0)
}
pub(super) fn is_wallet(a: &Value) -> bool {
    a["wallet"].is_object()
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
pub(super) fn signed(d: &Value) -> String {
    let n = i128::from(d["amount"].as_i64().unwrap_or(0));
    if text(d, "direction") == "in" {
        format!("+{}", group(n))
    } else {
        group(-n)
    }
}
pub(super) fn date_label(iso: &str, year: &str) -> String {
    if iso.len() == 10 && iso.starts_with(year) {
        iso[5..].replace('-', "/")
    } else {
        iso.replace('-', "/")
    }
}
pub(super) fn row_label(d: &Value, year: &str) -> String {
    format!(
        "{} {} {}円",
        date_label(text(d, "date"), year),
        if text(d, "description").is_empty() {
            "摘要なし"
        } else {
            text(d, "description")
        },
        group(i128::from(d["amount"].as_i64().unwrap_or(0)))
    )
}
pub(super) fn tax_label(code: &str) -> &str {
    match code {
        "S10" => "課税売上10%",
        "S8" => "課税売上8%（軽減）",
        "P10" => "課税仕入10%",
        "P8" => "課税仕入8%（軽減）",
        "P10X" => "経過措置の仕入10%（免税事業者等から）",
        "P8X" => "経過措置の仕入8%（免税事業者等から）",
        "NT" => "非課税",
        "" => "対象外",
        _ => code,
    }
}
pub(super) fn tax_choice_label(code: &str) -> &str {
    let label = tax_label(code);
    label.strip_suffix("（免税事業者等から）").unwrap_or(label)
}
pub(super) fn tax_codes(income: bool) -> &'static [&'static str] {
    if income {
        &["", "S10", "S8", "NT"]
    } else {
        &["", "P10", "P8", "P10X", "P8X", "NT"]
    }
}
pub(super) fn step_option(selectable: &[bool], current: i32, step: i32) -> Option<usize> {
    if ![-1, 1].contains(&step) {
        return None;
    }
    let mut next = current.checked_add(step)?;
    while let Ok(index) = usize::try_from(next) {
        if *selectable.get(index)? {
            return Some(index);
        }
        next = next.checked_add(step)?;
    }
    None
}
pub(super) fn default_tax(a: &Value, income: bool, exempt: bool) -> String {
    let code = text(a, "default_tax");
    if exempt || is_wallet(a) || !tax_codes(income).contains(&code) {
        String::new()
    } else {
        code.into()
    }
}
pub(super) fn tax_after(a: &Value, income: bool, exempt: bool, tax: &str, touched: bool) -> String {
    if exempt || is_wallet(a) {
        String::new()
    } else if touched {
        tax.into()
    } else {
        default_tax(a, income, exempt)
    }
}
pub(super) fn tax_differs(a: &Value, income: bool, exempt: bool, tax: &str) -> bool {
    !a.is_null() && !exempt && !is_wallet(a) && tax != default_tax(a, income, exempt)
}
pub(super) fn tax_diff_text(a: &Value, income: bool, exempt: bool) -> String {
    format!(
        "税区分が、科目の既定（{}）と違います。",
        tax_label(&default_tax(a, income, exempt))
    )
}
pub(super) fn tax_touched(a: &Value, income: bool, exempt: bool, tax: &str) -> bool {
    !exempt
        && if a.is_null() {
            !tax.is_empty()
        } else {
            tax_differs(a, income, exempt, tax)
        }
}
pub(super) fn unset_tax(a: &Value, has: bool, income: bool) -> (String, String) {
    if !has {
        return (
            "科目で決まる".into(),
            "科目を選ぶと、科目の既定の税区分が入ります".into(),
        );
    }
    let code = text(a, "default_tax");
    if !a.is_null() && code.is_empty() {
        return (
            "対象外（既定）".into(),
            "この科目の既定の税区分は「対象外」です。このまま確定すると「対象外」になります。"
                .into(),
        );
    }
    let title = if code.is_empty() {
        "このまま確定すると「対象外」になります。".into()
    } else {
        format!(
            "この科目の既定の税区分（{}）は、{}の行では使えません。このまま確定すると「対象外」になります。",
            tax_label(code),
            if income { "入金" } else { "出金" }
        )
    };
    ("対象外".into(), title)
}
pub(super) fn direction_warning(a: &Value, income: bool) -> &'static str {
    match (income, text(a, "kind")) {
        (false, "revenue") => {
            "出金の行に、収益の科目が選ばれています。返金などでなければ、科目を見直してください。"
        }
        (true, "expense") => {
            "入金の行に、費用の科目が選ばれています。返金などでなければ、科目を見直してください。"
        }
        _ => "",
    }
}
pub(super) fn pct_value(raw: &str) -> Option<u64> {
    let normalized: String = raw
        .chars()
        .map(|c| {
            if ('０'..='９').contains(&c) {
                char::from_u32(c as u32 - '０' as u32 + '0' as u32).unwrap_or(c)
            } else {
                c
            }
        })
        .collect();
    let s = normalized.trim();
    (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
        .then(|| s.parse::<u64>().ok())
        .flatten()
        .filter(|n| *n <= 100)
}
pub(super) fn gone(code: &str) -> bool {
    matches!(code, "already_confirmed" | "not_found")
}
#[derive(Debug, PartialEq)]
pub(super) struct Outcome {
    pub unmark: Vec<u64>,
    pub gone: Vec<Value>,
    pub fixable: Vec<Value>,
}
pub(super) fn confirm_outcome(confirmed: &[Value], failed: &[Value]) -> Outcome {
    let (gone, fixable): (Vec<_>, Vec<_>) = failed
        .iter()
        .cloned()
        .partition(|row| gone(text(&row["error"], "code")));
    Outcome {
        unmark: confirmed.iter().chain(&gone).map(id).collect(),
        gone,
        fixable,
    }
}
pub(super) fn confirmed_nos(nos: &[u64]) -> String {
    match (nos.iter().min(), nos.iter().max()) {
        (Some(a), Some(b)) if a == b => format!("No.{a}"),
        (Some(a), Some(b)) => format!("No.{a}〜No.{b}"),
        _ => String::new(),
    }
}

/// A batch is not offered again until its response arrives. Earlier results
/// remain available when a later request has an uncertain transport outcome.
pub(super) struct Confirmation {
    ids: Vec<u64>,
    offset: usize,
    pending: bool,
    stopped: bool,
    pub confirmed: Vec<Value>,
    pub failed: Vec<Value>,
}
impl Confirmation {
    pub fn new(ids: Vec<u64>) -> Self {
        Self {
            ids,
            offset: 0,
            pending: false,
            stopped: false,
            confirmed: vec![],
            failed: vec![],
        }
    }
    pub fn take_next(&mut self) -> Option<Vec<u64>> {
        if self.pending || self.stopped || self.offset == self.ids.len() {
            return None;
        }
        self.pending = true;
        Some(self.ids[self.offset..self.ids.len().min(self.offset + CONFIRM_LIMIT)].to_vec())
    }
    pub fn complete(&mut self, response: &Value) {
        if !self.pending || self.stopped {
            return;
        }
        self.confirmed.extend(items(&response["confirmed"]));
        self.failed.extend(items(&response["failed"]));
        self.offset = self.ids.len().min(self.offset + CONFIRM_LIMIT);
        self.pending = false;
    }
    pub fn stop(&mut self) {
        self.stopped = true;
        self.pending = false;
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Summary {
    pub count: usize,
    pub incoming: i128,
    pub outgoing: i128,
}
impl Summary {
    pub fn speech(&self) -> String {
        format!(
            "確定する行 {}件、入金 {}円、出金 {}円",
            group(self.count as i128),
            group(self.incoming),
            group(self.outgoing)
        )
    }
}
#[derive(Default)]
pub(super) struct Selection(Vec<(u64, bool, i64)>);
impl Selection {
    pub fn contains(&self, id: u64) -> bool {
        self.0.iter().any(|x| x.0 == id)
    }
    pub fn set(&mut self, d: &Value, on: bool) {
        let key = id(d);
        if !on {
            self.remove(key);
            return;
        }
        let value = (
            key,
            text(d, "direction") == "in",
            d["amount"].as_i64().unwrap_or(0),
        );
        if let Some(old) = self.0.iter_mut().find(|x| x.0 == key) {
            *old = value;
        } else {
            self.0.push(value);
        }
    }
    pub fn remove(&mut self, id: u64) {
        self.0.retain(|x| x.0 != id);
    }
    pub fn keep(&mut self, ids: &[u64]) {
        self.0.retain(|x| ids.contains(&x.0));
    }
    pub fn ids(&self) -> Vec<u64> {
        self.0.iter().map(|x| x.0).collect()
    }
    pub fn summary(&self) -> Summary {
        let mut s = Summary {
            count: self.0.len(),
            ..Summary::default()
        };
        for (_, incoming, amount) in &self.0 {
            if *incoming {
                s.incoming += i128::from(*amount);
            } else {
                s.outgoing += i128::from(*amount);
            }
        }
        s
    }
}
pub(super) fn cell_order(width: f64) -> (&'static str, &'static [&'static str]) {
    if width < 600. {
        (
            "stack",
            &[
                "cand", "desc", "amount", "ops", "account", "more", "date", "tax", "partner",
                "memo", "pct",
            ],
        )
    } else if width < 1080. {
        (
            "cards",
            &[
                "cand", "desc", "amount", "account", "more", "ops", "date", "tax", "partner",
                "memo", "pct",
            ],
        )
    } else {
        (
            "table",
            &[
                "cand", "date", "desc", "amount", "account", "more", "tax", "partner", "memo",
                "pct", "ops",
            ],
        )
    }
}
pub(super) fn is_text_entry(tag: &str, kind: &str, editable: bool) -> bool {
    editable
        || tag == "TEXTAREA"
        || tag == "INPUT"
            && ![
                "checkbox", "radio", "button", "submit", "reset", "range", "color", "file", "image",
            ]
            .contains(&kind)
}
#[derive(Clone, Copy)]
pub(super) struct UndoModifiers {
    pub meta: bool,
    pub ctrl: bool,
    pub extra: bool,
}
pub(super) fn undo_key(key: &str, mac: bool, modifiers: UndoModifiers) -> bool {
    matches!(key, "z" | "Z")
        && !modifiers.extra
        && if mac {
            modifiers.meta && !modifiers.ctrl
        } else {
            modifiers.ctrl && !modifiers.meta
        }
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
