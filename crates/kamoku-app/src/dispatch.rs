// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `/api/v1` の振り分け。経路・本文の種類・上限・読み取り専用の検査と、処理の実行。
//! 受け口（HTTPの接続）には依存しない。サーバーもブラウザ版も、ここを通す。

use crate::api::{ApiError, Call, Reply};
use crate::method::Method;
use crate::query::Query;
use crate::router::{self, Body, Lookup, Route};
use crate::state::State;
use kamoku_store::StoreError;
use time::Date;

/// 1つの応答（状態、見出し、本文）。セキュリティの共通見出し（CSPなど）は受け口が付ける。
pub(crate) struct Outcome {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// 振り分けに渡す要求。パスは `/api/v1` から始める。
pub(crate) struct Incoming<'a> {
    pub method: Method,
    pub path: &'a str,
    pub query: &'a str,
    pub body: &'a [u8],
    pub now: i64,
    pub today: Date,
}

/// 経路と問い合わせを読み終えた要求。状態のロックを取る前に作る。
pub(crate) struct Prepared {
    route: &'static Route,
    params: Vec<String>,
    query: Query,
}

const API_PREFIX: &str = "/api/v1";

/// 本文を読む前の検査。成功したら、本文の上限（バイト）を返す。
/// `read_only` は書き込みの経路のときだけ呼ぶ（サーバーは、そのときだけ状態のロックを取る）。
pub(crate) fn inspect(
    method: Method,
    path: &str,
    content_type: Option<&str>,
    read_only: impl FnOnce() -> bool,
) -> Result<u64, Outcome> {
    let Some(path) = path.strip_prefix(API_PREFIX) else {
        return Err(error_outcome(
            &ApiError::not_found("その経路はありません。"),
            &[],
        ));
    };
    let route = match router::find(method, path) {
        Lookup::Route(route, _) => route,
        Lookup::Missing => {
            return Err(error_outcome(
                &ApiError::not_found("その経路はありません。"),
                &[],
            ));
        }
        Lookup::WrongMethod(allowed) => {
            let error = ApiError::new(
                405,
                "method_not_allowed",
                "この経路では、そのメソッドは使えません。",
            );
            let allow = allowed.join(", ");
            return Err(error_outcome(&error, &[("Allow", &allow)]));
        }
    };
    let content_type = media_type(content_type);
    let wanted = match route.body {
        Body::Json => Some("application/json"),
        Body::Csv => Some("application/octet-stream"),
        Body::None => None,
    };
    if let Some(wanted) = wanted
        && content_type.as_deref() != Some(wanted)
    {
        let error = ApiError::bad_request(format!("Content-Type は {wanted} にしてください。"))
            .with_field("Content-Type");
        return Err(error_outcome(&error, &[]));
    }
    if route.write && read_only() {
        return Err(error_outcome(&read_only_error(), &[]));
    }
    Ok(route.body.limit())
}

/// 経路と問い合わせを読む（ロックの外）。
pub(crate) fn prepare(incoming: &Incoming<'_>) -> Result<Prepared, ApiError> {
    let path = incoming.path.strip_prefix(API_PREFIX).unwrap_or_default();
    let Lookup::Route(route, params) = router::find(incoming.method, path) else {
        return Err(ApiError::not_found("その経路はありません。"));
    };
    let query = Query::parse(incoming.query)?;
    Ok(Prepared {
        route,
        params,
        query,
    })
}

/// 処理する（状態のロックの中）。応答の組み立ては [`respond`] がロックの外で行う。
pub(crate) fn run(
    state: &mut State,
    prepared: &Prepared,
    incoming: &Incoming<'_>,
) -> Result<Reply, ApiError> {
    let call = Call {
        params: &prepared.params,
        query: &prepared.query,
        body: incoming.body,
        now: incoming.now,
        today: incoming.today,
    };
    if prepared.route.write && state.store.read_only().is_some() {
        return Err(read_only_error());
    }
    refresh_masters(state, incoming.method, prepared.route.write)
        .and_then(|()| (prepared.route.run)(state, &call))
}

/// 処理の結果を応答（状態・見出し・本文）にする（ロックの外）。
pub(crate) fn respond(incoming: &Incoming<'_>, result: Result<Reply, ApiError>) -> Outcome {
    match result {
        Ok(Reply::Json(status, value)) => match serde_json::to_vec(&value) {
            Ok(bytes) => Outcome {
                status,
                headers: vec![(
                    "Content-Type".to_owned(),
                    "application/json; charset=utf-8".to_owned(),
                )],
                body: bytes,
            },
            Err(e) => error_outcome(&log_detail(incoming, &ApiError::internal(e)), &[]),
        },
        Ok(Reply::File(file)) => {
            let mut headers = vec![
                ("Content-Type".to_owned(), file.content_type.to_owned()),
                (
                    "Content-Disposition".to_owned(),
                    format!("attachment; filename=\"{}\"", file.filename),
                ),
            ];
            if let Some(warning) = file.warning {
                headers.push(("X-Kamoku-Warning".to_owned(), warning));
            }
            Outcome {
                status: 200,
                headers,
                body: file.body,
            }
        }
        Err(e) => error_outcome(&log_detail(incoming, &e), &[]),
    }
}

/// 検査を通った要求を、ロックの区切りなしに処理する（ブラウザ版と試験用）。
pub(crate) fn execute(state: &mut State, incoming: &Incoming<'_>) -> Outcome {
    let result = prepare(incoming).and_then(|prepared| run(state, &prepared, incoming));
    respond(incoming, result)
}

/// 知らないメソッド。受け口が要求行で返すのと同じ文面。
pub(crate) fn unknown_method() -> Outcome {
    error_outcome(
        &ApiError::new(405, "method_not_allowed", "このメソッドは使えません。"),
        &[],
    )
}

/// 本文が上限を超えた。受け口が `Content-Length` で返すのと同じ文面。
pub(crate) fn too_large() -> Outcome {
    error_outcome(
        &ApiError::new(413, "too_large", "本文が大きすぎます。"),
        &[],
    )
}

// 変更系の要求は、組み立てる前に、設定と科目をDBの今の値にそろえる。別のプロセスが既定の割合などを
// 変えていても、古い値で組み立てて黙って保存しないため。台帳の末尾も変わっていれば、保存の層が読み直した
// うえで、書き込みは 409 stale（api-common.md）。書き込みでない要求（プレビューなど）は、そのまま続ける。
fn refresh_masters(state: &mut State, method: Method, write: bool) -> Result<(), ApiError> {
    if !method.changes_state() {
        return Ok(());
    }
    match state.store.refresh_masters() {
        Ok(()) => Ok(()),
        Err(StoreError::Stale) if !write => Ok(()),
        Err(e) => Err(e.into()),
    }
}

fn read_only_error() -> ApiError {
    ApiError::from(StoreError::ReadOnly)
}

fn media_type(header: Option<&str>) -> Option<String> {
    header.map(|value| {
        value
            .split(';')
            .next()
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
    })
}

fn error_outcome(error: &ApiError, extra: &[(&str, &str)]) -> Outcome {
    let mut headers = vec![(
        "Content-Type".to_owned(),
        "application/json; charset=utf-8".to_owned(),
    )];
    for (name, value) in extra {
        headers.push(((*name).to_owned(), (*value).to_owned()));
    }
    Outcome {
        status: error.status,
        headers,
        body: serde_json::to_vec(&error.body()).unwrap_or_default(),
    }
}

// 想定外の失敗の中身は応答に出さず、標準エラーに書く。問い合わせの文字列は書かない（合言葉を含みうるため）。
fn log_detail(incoming: &Incoming<'_>, error: &ApiError) -> ApiError {
    if let Some(detail) = &error.detail {
        eprintln!(
            "kamoku: 想定外の失敗（{} {}）: {detail}",
            incoming.method.as_str(),
            incoming.path
        );
    }
    error.clone()
}
