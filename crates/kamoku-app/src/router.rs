// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `/api/v1` の経路の表（メソッド・パス・本文の種類・書き込みか・処理）。経路はこの表だけで決める。

use crate::api::{self, Run};
use crate::http::Method;
use crate::query::percent_decode;

/// JSONの本文の上限（1MB）。
pub(crate) const JSON_LIMIT: u64 = 1024 * 1024;
/// CSVの送信の上限（10MB）。
pub(crate) const CSV_LIMIT: u64 = 10 * 1024 * 1024;

/// 本文の種類。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Body {
    /// 本文を使わない（届いても読み捨てる）。
    None,
    /// `application/json`。
    Json,
    /// `application/octet-stream`（CSVのファイル）。
    Csv,
}
impl Body {
    pub(crate) const fn limit(self) -> u64 {
        match self {
            Self::None | Self::Json => JSON_LIMIT,
            Self::Csv => CSV_LIMIT,
        }
    }
}

/// 経路。
pub(crate) struct Route {
    pub method: Method,
    pub path: &'static str,
    pub body: Body,
    /// 書き込みか（読み取り専用のときに断る）。
    pub write: bool,
    pub run: Run,
}

const fn route(method: Method, path: &'static str, body: Body, write: bool, run: Run) -> Route {
    Route {
        method,
        path,
        body,
        write,
        run,
    }
}

use Body::{Csv, Json, None as NoBody};
use Method::{Delete, Get, Patch, Post, Put};

static ROUTES: &[Route] = &[
    route(Get, "/status", NoBody, false, api::status),
    route(Post, "/verify", Json, false, api::verify),
    route(Get, "/settings", NoBody, false, api::settings_get),
    route(Put, "/settings", Json, true, api::settings_put),
    route(Get, "/accounts", NoBody, false, api::accounts_list),
    route(Post, "/accounts", Json, true, api::account_add),
    route(Patch, "/accounts/{code}", Json, true, api::account_patch),
    route(Get, "/wallets", NoBody, false, api::wallets),
    route(Get, "/partners", NoBody, false, api::partners_list),
    route(Post, "/partners", Json, true, api::partner_add),
    route(Patch, "/partners/{id}", Json, true, api::partner_patch),
    route(Get, "/templates", NoBody, false, api::templates_list),
    route(Post, "/templates", Json, true, api::template_add),
    route(
        Delete,
        "/templates/{id}",
        NoBody,
        true,
        api::template_delete,
    ),
    route(Get, "/opening", NoBody, false, api::opening_get),
    route(Put, "/opening", Json, true, api::opening_put),
    route(Post, "/entries/preview", Json, false, api::entry_preview),
    route(Get, "/entries", NoBody, false, api::entries_list),
    route(Post, "/entries", Json, true, api::entry_create),
    route(Get, "/entries/{id}", NoBody, false, api::entry_detail),
    route(Put, "/entries/{id}", Json, true, api::entry_revise),
    route(Post, "/entries/{id}/void", Json, true, api::entry_void),
    route(Post, "/imports/upload", Csv, true, api::import_upload),
    route(Post, "/imports/preview", Json, false, api::import_preview),
    route(Post, "/imports/commit", Json, true, api::import_commit),
    route(Get, "/imports/profile", NoBody, false, api::import_profile),
    route(Get, "/imports/batches", NoBody, false, api::import_batches),
    route(Get, "/drafts", NoBody, false, api::drafts_list),
    route(Post, "/drafts/confirm", Json, true, api::draft_confirm),
    route(Patch, "/drafts/{id}", Json, true, api::draft_patch),
    route(Post, "/drafts/{id}/ignore", NoBody, true, api::draft_ignore),
    route(
        Post,
        "/drafts/{id}/restore",
        NoBody,
        true,
        api::draft_restore,
    ),
    route(Get, "/reports/dashboard", NoBody, false, api::dashboard),
    route(
        Get,
        "/reports/trial-balance",
        NoBody,
        false,
        api::trial_balance,
    ),
    route(
        Get,
        "/reports/income-statement",
        NoBody,
        false,
        api::income_statement,
    ),
    route(
        Get,
        "/reports/balance-sheet",
        NoBody,
        false,
        api::balance_sheet,
    ),
    route(Get, "/reports/ledger", NoBody, false, api::ledger),
    route(Get, "/reports/tax-summary", NoBody, false, api::tax_summary),
    route(
        Get,
        "/export/journal.csv",
        NoBody,
        false,
        api::export_journal,
    ),
    route(
        Get,
        "/export/trial-balance.csv",
        NoBody,
        false,
        api::export_trial_balance,
    ),
    route(
        Get,
        "/export/income-statement.csv",
        NoBody,
        false,
        api::export_income_statement,
    ),
    route(
        Get,
        "/export/balance-sheet.csv",
        NoBody,
        false,
        api::export_balance_sheet,
    ),
    route(Get, "/export/ledger.csv", NoBody, false, api::export_ledger),
    route(
        Get,
        "/export/tax-summary.csv",
        NoBody,
        false,
        api::export_tax_summary,
    ),
    route(Get, "/export/all.json", NoBody, false, api::export_all),
    route(Get, "/export/backup.db", NoBody, false, api::export_backup),
];

/// 経路を探した結果。
pub(crate) enum Lookup {
    /// 経路と、パスの `{...}` の値。
    Route(&'static Route, Vec<String>),
    /// パスはあるが、メソッドが違う（使えるメソッドの一覧）。
    WrongMethod(Vec<&'static str>),
    /// パスが無い。
    Missing,
}

/// `/api/v1` より後ろのパスと、メソッドから経路を探します。HEAD は GET の経路を使う。
pub(crate) fn find(method: Method, path: &str) -> Lookup {
    let want = if method == Method::Head {
        Method::Get
    } else {
        method
    };
    let Some(segments) = path
        .split('/')
        .skip(1)
        .map(|s| percent_decode(s, false))
        .collect::<Option<Vec<String>>>()
    else {
        return Lookup::Missing;
    };
    if !path.starts_with('/') {
        return Lookup::Missing;
    }
    let mut allowed = Vec::new();
    for route in ROUTES {
        let Some(params) = match_path(route.path, &segments) else {
            continue;
        };
        if route.method == want {
            return Lookup::Route(route, params);
        }
        allowed.push(route.method.as_str());
    }
    if allowed.is_empty() {
        Lookup::Missing
    } else {
        if allowed.contains(&"GET") {
            allowed.push("HEAD");
        }
        Lookup::WrongMethod(allowed)
    }
}

fn match_path(pattern: &str, segments: &[String]) -> Option<Vec<String>> {
    let parts: Vec<&str> = pattern.split('/').skip(1).collect();
    if parts.len() != segments.len() {
        return None;
    }
    let mut params = Vec::new();
    for (part, segment) in parts.iter().zip(segments) {
        if part.starts_with('{') {
            if segment.is_empty() {
                return None;
            }
            params.push(segment.clone());
        } else if part != segment {
            return None;
        }
    }
    Some(params)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_routes_params_and_wrong_methods() {
        match find(Method::Post, "/entries/e_1/void") {
            Lookup::Route(r, p) => {
                assert_eq!(r.path, "/entries/{id}/void");
                assert_eq!(p, vec!["e_1".to_owned()]);
            }
            _ => panic!("経路が見つからない"),
        }
        assert!(
            matches!(find(Method::Post, "/drafts/confirm"), Lookup::Route(r, _) if r.path == "/drafts/confirm")
        );
        assert!(
            matches!(find(Method::Head, "/status"), Lookup::Route(r, _) if r.path == "/status")
        );
        assert!(
            matches!(find(Method::Delete, "/status"), Lookup::WrongMethod(m) if m.contains(&"GET"))
        );
        assert!(matches!(find(Method::Get, "/nothing"), Lookup::Missing));
        assert!(matches!(find(Method::Get, "/entries/%zz"), Lookup::Missing));
        assert!(matches!(find(Method::Get, "/entries/"), Lookup::Missing));
    }
}
