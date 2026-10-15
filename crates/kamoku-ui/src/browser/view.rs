// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::model::{Entry, EntryPage, Filters, PAGE_SIZE, yen};
use wasm_bindgen::JsValue;
use web_sys::{Element, HtmlInputElement, HtmlSelectElement};

use super::{dom, navigation};
use dom::{append, element};

pub(super) struct View {
    pub root: Element,
    pub search: HtmlInputElement,
    pub from: HtmlInputElement,
    pub to: HtmlInputElement,
    pub state: HtmlSelectElement,
    pub shortcuts: HtmlInputElement,
    pub summary: Element,
    pub tbody: Element,
    pub results: Element,
    pub empty: Element,
    pub error: Element,
    pub retry: Element,
    pub previous: Element,
    pub next: Element,
    pub page_count: Element,
}

impl View {
    pub fn sync(&self, filters: &Filters) {
        self.search.set_value(&filters.q);
        self.from.set_value(&filters.from);
        self.to.set_value(&filters.to);
        self.state.set_value(&filters.state);
    }

    pub fn loading(&self) -> Result<(), JsValue> {
        self.results.set_attribute("aria-busy", "true")?;
        self.error.set_attribute("hidden", "")?;
        self.retry.set_attribute("hidden", "")?;
        self.summary
            .set_text_content(Some("仕訳を読み込んでいます…"));
        dom::disabled(&self.previous, true)?;
        dom::disabled(&self.next, true)
    }

    pub fn paint(&self, page: &EntryPage, filters: &Filters) -> Result<(), JsValue> {
        self.tbody.set_text_content(None);
        for entry in &page.items {
            append(&self.tbody, &entry_row(entry, filters)?)?;
        }
        self.results.set_attribute("aria-busy", "false")?;
        if page.items.is_empty() {
            self.empty.remove_attribute("hidden")?;
        } else {
            self.empty.set_attribute("hidden", "")?;
        }
        let start = if page.items.is_empty() {
            0
        } else {
            filters.offset() + 1
        };
        let end = filters.offset() + u64::try_from(page.items.len()).unwrap_or_default();
        let scope = if filters.state == "all" {
            "（取消済みも含む）"
        } else if filters.state == "voided" {
            "（取消済みだけ）"
        } else {
            ""
        };
        self.summary.set_text_content(Some(&format!(
            "{}件のうち {start}〜{}件目。条件に合う全件の合計{scope} {}円",
            page.total,
            if page.items.is_empty() { 0 } else { end },
            yen(page.sum_amount)
        )));
        let pages = page.total.div_ceil(u64::from(PAGE_SIZE)).max(1);
        self.page_count
            .set_text_content(Some(&format!("{} / {pages} ページ", filters.page)));
        dom::disabled(&self.previous, filters.page <= 1)?;
        dom::disabled(&self.next, u64::from(filters.page) >= pages)
    }

    pub fn fail(&self, message: &str) -> Result<(), JsValue> {
        self.results.set_attribute("aria-busy", "false")?;
        self.tbody.set_text_content(None);
        self.empty.set_attribute("hidden", "")?;
        self.summary
            .set_text_content(Some("仕訳を読み込めませんでした。"));
        self.error.set_text_content(Some(message));
        self.error.remove_attribute("hidden")?;
        self.retry.remove_attribute("hidden")
    }
}

fn entry_row(entry: &Entry, filters: &Filters) -> Result<Element, JsValue> {
    let row = element(
        "tr",
        if entry.state == "voided" {
            "voided"
        } else {
            ""
        },
        "",
    )?;
    row.set_attribute("role", "row")?;
    cell(&row, "No", "c-no number", &entry.no.to_string())?;
    cell(&row, "日付", "c-date", &entry.date)?;
    let description = cell(
        &row,
        "摘要・取引先",
        "c-desc",
        if entry.description.is_empty() {
            "（摘要なし）"
        } else {
            &entry.description
        },
    )?;
    if !entry.partner.is_empty() {
        append(
            &description,
            &element("span", "partner", &format!("取引先 {}", entry.partner))?,
        )?;
    }
    for (side, label) in [("debit", "借方"), ("credit", "貸方")] {
        let mut lines = entry.lines.iter().filter(|line| line.side == side);
        let first = lines.next().map_or("", |line| line.account_name.as_str());
        let extra = lines.count();
        cell(
            &row,
            label,
            "c-side",
            &if extra == 0 {
                first.to_owned()
            } else {
                format!("{first} ほか{extra}行")
            },
        )?;
    }
    let amount = cell(&row, "金額", "c-amount number", "")?;
    append(
        &amount,
        &element("span", "amount", &format!("{}円", yen(entry.total)))?,
    )?;
    let detail = cell(&row, "詳細", "c-detail", "")?;
    let link = dom::link(
        "詳細を開く",
        &navigation::old_detail(&entry.id, filters)?,
        "detail-link",
    )?;
    link.set_attribute("data-entry-id", &entry.id)?;
    link.set_attribute(
        "aria-label",
        &format!("No.{} の詳細を通常の画面で開く", entry.no),
    )?;
    append(&detail, &link)?;
    if entry.state == "voided" {
        append(&detail, &element("span", "status-note", "取消済み")?)?;
    }
    Ok(row)
}

fn cell(row: &Element, label: &str, class: &str, text: &str) -> Result<Element, JsValue> {
    let cell = element("td", class, text)?;
    cell.set_attribute("role", "cell")?;
    cell.set_attribute("data-label", label)?;
    append(row, &cell)?;
    Ok(cell)
}
