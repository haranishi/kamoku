// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 受け口（[`crate::http`]）の向こう側。検査の順は、`Host`（421）→ セッション（401）→
//! 変更の要求の出所（403）→ 経路（404・405）→ 本文の種類（400）→ 読み取り専用（409）→ 処理。
//! 本文は、ここまでの検査を通った要求のものだけを読む。

use crate::api::ApiError;
use crate::dispatch::{self, Incoming, Outcome};
use crate::gate::{self, Sessions};
use crate::http::{Handler, Head, Method, Request, Response};
use crate::router::JSON_LIMIT;
use crate::state::State;
use crate::statics::{self, Statics};
use crate::{clock, ids};
use std::sync::{Mutex, MutexGuard};

/// サーバーの中身。受け口の処理のスレッドから共有される。
pub(crate) struct App {
    port: u16,
    token: String,
    sessions: Sessions,
    statics: Statics,
    state: Mutex<State>,
}

#[cfg(test)]
mod tests;

impl App {
    pub(crate) fn new(port: u16, token: String, statics: Statics, state: State) -> Self {
        Self {
            port,
            token,
            sessions: Sessions::default(),
            statics,
            state: Mutex::new(state),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                // 前の要求の処理が途中で止まった（panicした）。保存の層の開いたままのトランザクションと、
                // 保存されていないかもしれないメモリの状態を引き継がないよう、DBからやり直してから使う。
                let mut guard = poisoned.into_inner();
                guard.store.recover();
                self.state.clear_poison();
                guard
            }
        }
    }

    // 画面のページには日本語の短い案内を、/api/ には JSON を返す。
    fn deny(api: bool, error: &ApiError, title: &str) -> Response {
        if api {
            json_error(error)
        } else {
            gate::page(error.status, title, &error.message)
        }
    }

    fn session_ok(&self, head: &Head) -> bool {
        gate::session_values(head)
            .iter()
            .any(|value| self.sessions.valid(value))
    }

    fn enter(&self, head: &Head) -> Response {
        let given = gate::token_in(&head.query);
        if !given.is_some_and(|t| ids::same_secret(&t, &self.token)) {
            return gate::page(401, "合言葉が違います", gate::AUTH_MESSAGE);
        }
        match self.sessions.issue() {
            Ok(session) => Response::new(303)
                .with_header("Location", "/")
                .with_header("Set-Cookie", &gate::set_cookie(&session))
                .with_header("Cache-Control", "no-store"),
            Err(e) => {
                eprintln!("kamoku: セッション値の乱数を作れませんでした: {e}");
                gate::page(
                    500,
                    "起動の問題",
                    "乱数を作れませんでした。サーバーを起動し直してください。",
                )
            }
        }
    }

    fn api(&self, head: &Head, body: &[u8]) -> Response {
        let incoming = Incoming {
            method: head.method,
            path: &head.path,
            query: &head.query,
            body,
            now: clock::now_secs(),
            today: clock::today(),
        };
        // ロックの中では処理だけを行い、JSON への変換はロックの外で行う。
        let result = dispatch::prepare(&incoming).and_then(|prepared| {
            let mut state = self.lock();
            dispatch::run(&mut state, &prepared, &incoming)
        });
        outcome_response(dispatch::respond(&incoming, result))
    }

    fn inspect_api(&self, head: &Head) -> Result<u64, Response> {
        // 読み取り専用かは、書き込みの経路のときだけロックを取って見る。
        dispatch::inspect(head.method, &head.path, head.header("content-type"), || {
            self.lock().store.read_only().is_some()
        })
        .map_err(outcome_response)
    }
}

fn outcome_response(outcome: Outcome) -> Response {
    let mut response = Response::new(outcome.status);
    response.body = outcome.body;
    for (name, value) in outcome.headers {
        response.set_header(&name, &value);
    }
    response
}

fn json_error(error: &ApiError) -> Response {
    let bytes = serde_json::to_vec(&error.body()).unwrap_or_default();
    Response::new(error.status).with_body("application/json; charset=utf-8", bytes)
}

impl Handler for App {
    fn inspect(&self, head: &Head) -> Result<u64, Response> {
        let api = head.path.starts_with("/api/");
        if !gate::host_ok(head, self.port) {
            let error = ApiError::new(421, "bad_host", gate::HOST_MESSAGE);
            return Err(Self::deny(api, &error, "アドレスが違います"));
        }
        if gate::is_entrance(head) {
            return Ok(JSON_LIMIT);
        }
        if !self.session_ok(head) {
            let error = ApiError::new(401, "unauthorized", gate::AUTH_MESSAGE);
            return Err(Self::deny(api, &error, "合言葉が必要です"));
        }
        if head.method.changes_state() && !gate::origin_ok(head, self.port) {
            let error = ApiError::new(403, "forbidden_origin", gate::ORIGIN_MESSAGE);
            return Err(Self::deny(api, &error, "ほかのページからの操作です"));
        }
        if api {
            return self.inspect_api(head);
        }
        if matches!(head.method, Method::Get | Method::Head) {
            Ok(JSON_LIMIT)
        } else {
            let error = ApiError::new(405, "method_not_allowed", "画面のファイルは読むだけです。");
            Err(Self::deny(false, &error, "使えない操作です").with_header("Allow", "GET, HEAD"))
        }
    }

    fn handle(&self, request: Request) -> Response {
        let head = &request.head;
        if gate::is_entrance(head) {
            return self.enter(head);
        }
        if head.path.starts_with("/api/") {
            return self.api(head, &request.body);
        }
        match statics::lookup(&self.statics, &head.path) {
            Some((kind, bytes)) => Response::new(200)
                .with_body(kind, bytes)
                .with_header("Cache-Control", "no-cache"),
            None => gate::page(
                404,
                "見つかりません",
                "そのページはありません。kamokuの画面は / から開きます。",
            ),
        }
    }

    fn reject(&self, status: u16, message: &str) -> Response {
        let code = match status {
            405 => "method_not_allowed",
            408 => "timeout",
            413 | 414 | 431 => "too_large",
            501 => "not_implemented",
            503 => "busy",
            _ => "bad_request",
        };
        json_error(&ApiError::new(status, code, message))
    }

    fn decorate(&self, response: &mut Response) {
        gate::decorate(response);
    }
}
