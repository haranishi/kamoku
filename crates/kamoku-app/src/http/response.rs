// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 応答の形と、バイト列への書き出し。

/// 応答。`Content-Length` と `Connection: close` は書き出すときに受け口が付ける。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    /// 状態。
    pub status: u16,
    /// 見出し（名前、値）。
    pub headers: Vec<(String, String)>,
    /// 本文。
    pub body: Vec<u8>,
}
impl Response {
    /// 本文の無い応答を作ります。
    #[must_use]
    pub const fn new(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: Vec::new(),
        }
    }
    /// 見出しを足した応答を返します。同じ名前があれば置き換えます。
    #[must_use]
    pub fn with_header(mut self, name: &str, value: &str) -> Self {
        self.set_header(name, value);
        self
    }
    /// 本文と、その種類を入れた応答を返します。
    #[must_use]
    pub fn with_body(mut self, content_type: &str, body: Vec<u8>) -> Self {
        self.set_header("Content-Type", content_type);
        self.body = body;
        self
    }
    /// 見出しを入れます。同じ名前（大文字・小文字を区別しない）があれば置き換えます。
    pub fn set_header(&mut self, name: &str, value: &str) {
        self.headers.retain(|(n, _)| !n.eq_ignore_ascii_case(name));
        self.headers.push((name.to_owned(), value.to_owned()));
    }
    /// 見出しの値（大文字・小文字を区別しない）。
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// 書き出すバイト列。`send_body` が偽（HEAD）なら本文を省く（長さは本文のものを書く）。
    pub(crate) fn to_bytes(&self, send_body: bool) -> Vec<u8> {
        let no_body = matches!(self.status, 100..=199 | 204 | 304);
        let mut out = Vec::with_capacity(256 + self.body.len());
        out.extend_from_slice(
            format!(
                "HTTP/1.1 {} {}\r\n",
                self.status,
                reason_phrase(self.status)
            )
            .as_bytes(),
        );
        for (name, value) in &self.headers {
            // 長さと接続は受け口が決める。改行を含む値は、見出しの分割に使われうるので書かない。
            let reserved = ["content-length", "connection", "transfer-encoding"]
                .iter()
                .any(|r| name.eq_ignore_ascii_case(r));
            let unsafe_text = |s: &str| s.bytes().any(|b| b == b'\r' || b == b'\n' || b == 0);
            if reserved || name.is_empty() || unsafe_text(name) || unsafe_text(value) {
                continue;
            }
            out.extend_from_slice(format!("{name}: {value}\r\n").as_bytes());
        }
        if !no_body {
            out.extend_from_slice(format!("Content-Length: {}\r\n", self.body.len()).as_bytes());
        }
        out.extend_from_slice(b"Connection: close\r\n\r\n");
        if send_body && !no_body {
            out.extend_from_slice(&self.body);
        }
        out
    }
}

/// 状態の短い説明（英語。HTTP の決まりの文句）。
#[must_use]
pub const fn reason_phrase(status: u16) -> &'static str {
    match status {
        100 => "Continue",
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        303 => "See Other",
        304 => "Not Modified",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        409 => "Conflict",
        413 => "Content Too Large",
        414 => "URI Too Long",
        417 => "Expectation Failed",
        421 => "Misdirected Request",
        422 => "Unprocessable Content",
        431 => "Request Header Fields Too Large",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        503 => "Service Unavailable",
        505 => "HTTP Version Not Supported",
        _ => "Unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_length_and_close_and_drops_injected_lines() {
        let r = Response::new(200)
            .with_body("text/plain", b"abc".to_vec())
            .with_header("X-Evil", "a\r\nSet-Cookie: x=1")
            .with_header("Content-Length", "999");
        let text = String::from_utf8(r.to_bytes(true)).unwrap();
        assert!(text.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(text.contains("Content-Length: 3\r\n"));
        assert!(!text.contains("999"));
        assert!(!text.contains("Set-Cookie"));
        assert!(text.ends_with("Connection: close\r\n\r\nabc"));
        let head_only = String::from_utf8(r.to_bytes(false)).unwrap();
        assert!(head_only.ends_with("\r\n\r\n") && head_only.contains("Content-Length: 3"));
    }
}
