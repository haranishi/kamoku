// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! ブラウザ版の入口。サーバーと同じ経路表・本文の上限・Content-Type の検査（`dispatch`）で、
//! メモリ上の帳簿に応える。ネイティブでもコンパイルでき、単体試験はここで行う。

use crate::api::{ClientIds, Uploads};
use crate::clock;
use crate::dispatch::{self, Incoming, Outcome};
use crate::method::Method;
use crate::mode::Mode;
use crate::state::State;
use kamoku_store::Store;
use time::Date;

/// メモリ上の帳簿。
pub struct Book {
    state: State,
    today: Date,
}

/// [`Book::call`] の応答。状態、見出し、本文。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplyBytes {
    /// HTTPの状態。
    pub status: u16,
    /// 見出し（名前、値）。CSPなどの共通見出しは含まない。
    pub headers: Vec<(String, String)>,
    /// 本文。
    pub body: Vec<u8>,
}

impl Book {
    /// 架空の帳簿で開く。`today` は日付の判定に使う（仕訳の記録時刻は、呼んだときの時計）。
    ///
    /// # Errors
    /// メモリ上のSQLiteを開けない、または合成データを入れられない。
    pub fn demo(today: Date) -> Result<Self, String> {
        let (mut store, report) = Store::open_in_memory().map_err(|e| e.to_string())?;
        crate::demo::populate(&mut store).map_err(|e| e.to_string())?;
        Ok(Self {
            state: State {
                store,
                mode: Mode::Demo,
                events_in_db: report.events_in_db,
                clients: ClientIds::default(),
                uploads: Uploads::default(),
            },
            today,
        })
    }

    /// 1つのAPI要求に応える。`path` は `/api/v1/...`。`query` は `?` を含めない。
    /// 知らないメソッド、経路、Content-Type、本文の上限は、サーバーの受け口と同じ順で断る。
    /// HEAD も、この層では本文を返す（接続へ書くときに省くのは受け口の仕事）。
    #[must_use]
    pub fn call(
        &mut self,
        method: &str,
        path: &str,
        query: &str,
        content_type: Option<&str>,
        body: &[u8],
    ) -> ReplyBytes {
        let Some(method) = Method::parse(method) else {
            return ReplyBytes::from(dispatch::unknown_method());
        };
        let state = &self.state;
        let limit = match dispatch::inspect(method, path, content_type, || {
            state.store.read_only().is_some()
        }) {
            Ok(limit) => limit,
            Err(outcome) => return ReplyBytes::from(outcome),
        };
        if u64::try_from(body.len()).unwrap_or(u64::MAX) > limit {
            return ReplyBytes::from(dispatch::too_large());
        }
        let incoming = Incoming {
            method,
            path,
            query,
            body,
            now: clock::now_secs(),
            today: self.today,
        };
        ReplyBytes::from(dispatch::execute(&mut self.state, &incoming))
    }
}

impl From<Outcome> for ReplyBytes {
    fn from(outcome: Outcome) -> Self {
        Self {
            status: outcome.status,
            headers: outcome.headers,
            body: outcome.body,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo;
    use time::macros::date;

    fn populated() -> (State, String) {
        let (mut store, report) = Store::open_in_memory().expect("memory");
        let summary = demo::populate(&mut store).expect("demo");
        let state = State {
            events_in_db: report.events_in_db,
            store,
            mode: Mode::Demo,
            clients: ClientIds::default(),
            uploads: Uploads::default(),
        };
        (state, summary.head_hash)
    }

    #[test]
    fn browser_entry_matches_server_dispatch_and_demo_hash() {
        // AC-14.7 ブラウザ版の入口は、サーバーと同じ振り分けで同じ内容を返し、デモの末尾ハッシュも同じ。
        let today = date!(2026 - 09 - 30);
        let mut book = Book::demo(today).expect("book");
        let (mut state, head_hash) = populated();
        let now = clock::now_secs();
        for (path, query) in [
            ("/api/v1/status", ""),
            ("/api/v1/accounts", ""),
            ("/api/v1/reports/dashboard", "month=2026-09"),
        ] {
            let via_book = book.call("GET", path, query, None, &[]);
            let via_dispatch = dispatch::execute(
                &mut state,
                &Incoming {
                    method: Method::Get,
                    path,
                    query,
                    body: &[],
                    now,
                    today,
                },
            );
            assert_eq!(via_book.status, via_dispatch.status, "{path}");
            assert_eq!(via_book.body, via_dispatch.body, "{path}");
            assert_eq!(via_book.headers, via_dispatch.headers, "{path}");
        }
        let status = book.call("GET", "/api/v1/status", "", None, &[]);
        let json: serde_json::Value = serde_json::from_slice(&status.body).expect("json");
        assert_eq!(json["mode"], "demo");
        assert_eq!(json["ledger"]["head_hash"], head_hash);

        let mut again = Book::demo(today).expect("second");
        let second = again.call("GET", "/api/v1/status", "", None, &[]);
        assert_eq!(status.body, second.body);

        let bad_type = book.call("POST", "/api/v1/verify", "", Some("text/plain"), b"{}");
        assert_eq!(bad_type.status, 400);
        assert!(
            String::from_utf8_lossy(&bad_type.body).contains("Content-Type"),
            "{}",
            String::from_utf8_lossy(&bad_type.body)
        );

        let limit = usize::try_from(crate::router::JSON_LIMIT).expect("limit fits");
        let at_limit = vec![b' '; limit];
        let parsed = book.call(
            "POST",
            "/api/v1/verify",
            "",
            Some("application/json"),
            &at_limit,
        );
        assert_ne!(parsed.status, 413);
        let mut over = at_limit;
        over.push(b' ');
        let too_big = book.call(
            "POST",
            "/api/v1/verify",
            "",
            Some("application/json"),
            &over,
        );
        assert_eq!(too_big.status, 413);
        assert!(String::from_utf8_lossy(&too_big.body).contains("本文が大きすぎます。"));

        let missing = book.call("GET", "/api/v1/no-such", "", None, &[]);
        assert_eq!(missing.status, 404);
        let wrong = book.call(
            "POST",
            "/api/v1/status",
            "",
            Some("application/json"),
            b"{}",
        );
        assert_eq!(wrong.status, 405);
        let allow = wrong
            .headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case("Allow"))
            .map_or("", |(_, value)| value.as_str());
        assert!(allow.contains("GET"), "{allow}");
        assert!(allow.contains("HEAD"), "{allow}");
        let unknown = book.call("FOO", "/api/v1/status", "", None, &[]);
        assert_eq!(unknown.status, 405);
        assert!(String::from_utf8_lossy(&unknown.body).contains("このメソッドは使えません。"));
        assert!(
            !unknown
                .headers
                .iter()
                .any(|(name, _)| name.eq_ignore_ascii_case("Allow"))
        );
    }
}
