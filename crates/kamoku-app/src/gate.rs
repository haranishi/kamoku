// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 入り口と認証（`docs/spec/api-common.md`）。合言葉・セッション・`Host`・`Origin` の検査と、
//! すべての応答に付ける見出し。受け口に依存しない、見出しを見るだけの関数。

use crate::http::{Head, Method, Response};
use crate::ids;
use crate::query::percent_decode;
use std::collections::VecDeque;
use std::sync::{Mutex, PoisonError};

/// すべての応答に付ける `Content-Security-Policy`。外部の読み込みを禁じる。
pub(crate) const CSP: &str = "default-src 'self'; img-src 'self' data:; style-src 'self'; script-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'";
/// セッションのCookieの名前。
pub(crate) const COOKIE: &str = "kamoku_session";
// 同じ起動の間に何度も合言葉から入り直しても、表が際限なく育たないようにする。
const MAX_SESSIONS: usize = 32;

/// 起動中に発行したセッション値。再起動で全部消える。
#[derive(Default)]
pub(crate) struct Sessions(Mutex<VecDeque<String>>);
impl Sessions {
    /// 新しいセッション値（128ビットの乱数）を作って覚えます。古いものから忘れます。
    pub(crate) fn issue(&self) -> Result<String, getrandom::Error> {
        let value = ids::random_hex(16)?;
        let mut list = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if list.len() >= MAX_SESSIONS {
            list.pop_front();
        }
        list.push_back(value.clone());
        Ok(value)
    }
    /// 発行したどれかと一致するか（定数時間で比べる）。
    pub(crate) fn valid(&self, value: &str) -> bool {
        let list = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        list.iter()
            .fold(false, |hit, issued| ids::same_secret(issued, value) | hit)
    }
}

/// `Host` が `127.0.0.1:<port>` か `localhost:<port>` か。DNSの付け替えによる攻撃への備え。
pub(crate) fn host_ok(head: &Head, port: u16) -> bool {
    head.header("host").is_some_and(|host| {
        let host = host.to_ascii_lowercase();
        host == format!("127.0.0.1:{port}") || host == format!("localhost:{port}")
    })
}

/// 状態を変える要求の出所の検査。`Origin` は必須で、自分のオリジンと完全に一致すること。
/// `Sec-Fetch-Site` があれば `same-origin` であること。
pub(crate) fn origin_ok(head: &Head, port: u16) -> bool {
    let mut origins = head.headers_named("origin");
    let (Some(origin), None) = (origins.next(), origins.next()) else {
        return false;
    };
    let ours = origin == format!("http://127.0.0.1:{port}")
        || origin == format!("http://localhost:{port}");
    let site = head
        .headers_named("sec-fetch-site")
        .all(|v| v == "same-origin");
    ours && site
}

/// 合言葉の入り口（`GET /?t=...`）か。
pub(crate) fn is_entrance(head: &Head) -> bool {
    matches!(head.method, Method::Get | Method::Head)
        && head.path == "/"
        && token_in(&head.query).is_some()
}

/// 問い合わせの `t` の値。
pub(crate) fn token_in(query: &str) -> Option<String> {
    query
        .split('&')
        .find_map(|pair| pair.strip_prefix("t="))
        .and_then(|v| percent_decode(v, true))
}

/// Cookie の `kamoku_session` の値（すべて）。
pub(crate) fn session_values(head: &Head) -> Vec<String> {
    head.headers_named("cookie")
        .flat_map(|line| line.split(';'))
        .filter_map(|pair| {
            let (name, value) = pair.trim().split_once('=')?;
            (name == COOKIE).then(|| value.trim().to_owned())
        })
        .collect()
}

/// セッションのCookieを渡す `Set-Cookie` の値。
pub(crate) fn set_cookie(session: &str) -> String {
    format!("{COOKIE}={session}; HttpOnly; SameSite=Strict; Path=/")
}

/// すべての応答に共通の見出し。`Cache-Control` が無ければ `no-store` にする（静的ファイルは自分で付ける）。
pub(crate) fn decorate(response: &mut Response) {
    response.set_header("Content-Security-Policy", CSP);
    response.set_header("X-Content-Type-Options", "nosniff");
    response.set_header("Referrer-Policy", "no-referrer");
    if response.header("Cache-Control").is_none() {
        response.set_header("Cache-Control", "no-store");
    }
}

/// 画面用の短い案内のページ（日本語）。値を埋め込まない固定の文だけで作る。
pub(crate) fn page(status: u16, title: &str, message: &str) -> Response {
    let html = format!(
        "<!doctype html>\n<html lang=\"ja\">\n<head><meta charset=\"utf-8\"><title>kamoku: {title}</title></head>\n<body>\n<h1>{title}</h1>\n<p>{message}</p>\n</body>\n</html>\n"
    );
    Response::new(status).with_body("text/html; charset=utf-8", html.into_bytes())
}

/// 401 のときの案内。
pub(crate) const AUTH_MESSAGE: &str = "合言葉の付いたアドレスから開いてください。サーバーを起動したときに、ターミナルに表示されたアドレス（http://127.0.0.1:番号/?t=…）です。サーバーを起動し直したときは、新しいアドレスを開き直してください。";
/// 421 のときの案内。
pub(crate) const HOST_MESSAGE: &str = "開いたアドレスが違います。サーバーを起動したときに表示されたアドレス（http://127.0.0.1:番号/?t=…）から開いてください。";
/// 403 のときの案内。
pub(crate) const ORIGIN_MESSAGE: &str = "ほかのページからの操作として断りました。kamokuの画面から操作してください。curl などの道具から送るときは、Origin に http://127.0.0.1:番号 を付けてください。";
