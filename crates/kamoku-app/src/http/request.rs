// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 要求の見出し（要求行と見出し）の読み取り。バイト列だけを相手にする純粋な関数。

use super::server::Limits;

/// 受け付けるメソッド。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// 読む。
    Get,
    /// 見出しだけ読む。
    Head,
    /// 作る。
    Post,
    /// 置き換える。
    Put,
    /// 一部を変える。
    Patch,
    /// 消す。
    Delete,
}
impl Method {
    /// 名前から返します。知らない名前は `None`（大文字・小文字は区別する）。
    #[must_use]
    pub fn parse(token: &str) -> Option<Self> {
        match token {
            "GET" => Some(Self::Get),
            "HEAD" => Some(Self::Head),
            "POST" => Some(Self::Post),
            "PUT" => Some(Self::Put),
            "PATCH" => Some(Self::Patch),
            "DELETE" => Some(Self::Delete),
            _ => None,
        }
    }
    /// 名前。
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Head => "HEAD",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
        }
    }
    /// 状態を変える要求か（`Origin` の検査の対象）。
    #[must_use]
    pub const fn changes_state(self) -> bool {
        matches!(self, Self::Post | Self::Put | Self::Patch | Self::Delete)
    }
}

/// 要求の見出し。本文を読む前の検査に使う。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Head {
    /// メソッド。
    pub method: Method,
    /// パス（`?` より前。パーセント符号化のまま）。
    pub path: String,
    /// 問い合わせの文字列（`?` より後。無ければ空）。合言葉を含みうるので、記録に残さない。
    pub query: String,
    /// HTTP の版の小数点の後の数（1.0 なら0、1.1 なら1）。
    pub minor_version: u8,
    /// 見出し。名前は小文字にしてある。
    pub headers: Vec<(String, String)>,
    /// 本文の長さ（`Content-Length`。無ければ0）。
    pub content_length: u64,
    /// `Expect: 100-continue` があったか（HTTP/1.1 のときだけ真）。
    pub expect_continue: bool,
}
impl Head {
    /// 名前（小文字）の最初の見出しの値。
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }
    /// 名前（小文字）の見出しの値を、すべて。
    pub fn headers_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a str> + 'a {
        self.headers
            .iter()
            .filter(move |(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }
}

/// 要求（見出しと本文）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// 見出し。
    pub head: Head,
    /// 本文。
    pub body: Vec<u8>,
}

/// 受け口が自分で断る理由（状態と、原因の日本語）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Reject {
    pub status: u16,
    pub message: &'static str,
}
const fn reject(status: u16, message: &'static str) -> Reject {
    Reject { status, message }
}
const MALFORMED: Reject = reject(400, "要求の形が正しくありません。");

/// 見出しの終わり（最初の空の行の直後）の位置。まだ届いていなければ `None`。
/// 行の終わりは CRLF を基本に、LF だけも受け付ける（RFC 9112 の2.2）。
pub(crate) fn head_end(buf: &[u8]) -> Option<usize> {
    let mut line_start = 0;
    for (i, byte) in buf.iter().enumerate() {
        if *byte == b'\n' {
            let line = buf.get(line_start..i)?;
            if line.is_empty() || line == b"\r" {
                return Some(i + 1);
            }
            line_start = i + 1;
        }
    }
    None
}

/// 見出しの終わりがまだ無いのに、上限を超えたか。超えたら断る理由を返す。
pub(crate) fn head_overflow(buf: &[u8], limits: &Limits) -> Option<Reject> {
    match buf.iter().position(|b| *b == b'\n') {
        None if buf.len() > limits.request_line => Some(reject(414, "要求行が長すぎます。")),
        Some(first) if first > limits.request_line => Some(reject(414, "要求行が長すぎます。")),
        Some(first) if buf.len().saturating_sub(first + 1) > limits.header_bytes => {
            Some(reject(431, "見出しが大きすぎます。"))
        }
        _ => None,
    }
}

fn is_tchar(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b)
}
// パスと問い合わせに使える文字（RFC 3986 の pchar と `/` `?` `%`）。ブラウザはほかの文字を符号化して送る。
fn is_target_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b"-._~!$&'()*+,;=:@/?%".contains(&b)
}
fn strip_cr(line: &[u8]) -> &[u8] {
    line.strip_suffix(b"\r").unwrap_or(line)
}

/// 見出し（空の行まで）を読みます。
pub(crate) fn parse_head(bytes: &[u8], limits: &Limits) -> Result<Head, Reject> {
    let mut lines = bytes.split(|b| *b == b'\n').map(strip_cr);
    let first = lines.next().ok_or(MALFORMED)?;
    if first.len() > limits.request_line {
        return Err(reject(414, "要求行が長すぎます。"));
    }
    let (method, path, query, minor_version) = request_line(first)?;
    let header_bytes = bytes.len().saturating_sub(first.len() + 1);
    if header_bytes > limits.header_bytes.saturating_add(2) {
        return Err(reject(431, "見出しが大きすぎます。"));
    }
    let mut headers = Vec::new();
    for line in lines.take_while(|l| !l.is_empty()) {
        if headers.len() >= limits.header_count {
            return Err(reject(431, "見出しが多すぎます。"));
        }
        headers.push(header_line(line)?);
    }
    let mut head = Head {
        method,
        path,
        query,
        minor_version,
        headers,
        content_length: 0,
        expect_continue: false,
    };
    if head.headers_named("host").count() > 1 {
        return Err(reject(400, "Host の見出しが2つ以上あります。"));
    }
    if head.header("transfer-encoding").is_some() {
        return Err(reject(
            501,
            "Transfer-Encoding の本文は扱いません。Content-Length を付けて送ってください。",
        ));
    }
    head.content_length = content_length(&head)?;
    if let Some(expect) = head.header("expect") {
        if !expect.eq_ignore_ascii_case("100-continue") {
            return Err(reject(417, "Expect の値は 100-continue だけを扱います。"));
        }
        head.expect_continue = head.minor_version == 1;
    }
    Ok(head)
}

fn request_line(line: &[u8]) -> Result<(Method, String, String, u8), Reject> {
    let mut parts = line.split(|b| *b == b' ');
    let (Some(method), Some(target), Some(version), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(MALFORMED);
    };
    if method.is_empty() || !method.iter().all(|b| is_tchar(*b)) {
        return Err(MALFORMED);
    }
    let method = std::str::from_utf8(method).map_err(|_| MALFORMED)?;
    let method = Method::parse(method).ok_or(reject(405, "このメソッドは使えません。"))?;
    if !target.starts_with(b"/") || !target.iter().all(|b| is_target_char(*b)) {
        return Err(MALFORMED);
    }
    let target = std::str::from_utf8(target).map_err(|_| MALFORMED)?;
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let minor = match version {
        b"HTTP/1.1" => 1,
        b"HTTP/1.0" => 0,
        [b'H', b'T', b'T', b'P', b'/', major, b'.', minor]
            if major.is_ascii_digit() && minor.is_ascii_digit() =>
        {
            return Err(reject(505, "HTTP/1.1 と 1.0 だけを扱います。"));
        }
        _ => return Err(MALFORMED),
    };
    Ok((method, path.to_owned(), query.to_owned(), minor))
}

fn header_line(line: &[u8]) -> Result<(String, String), Reject> {
    // 制御文字（タブを除く）は、見出しの分割や記録の書き換えに使われうるので断る。
    if line
        .iter()
        .any(|b| (*b < 0x20 && *b != b'\t') || *b == 0x7f)
    {
        return Err(reject(400, "見出しに使えない文字があります。"));
    }
    let colon = line.iter().position(|b| *b == b':').ok_or(MALFORMED)?;
    let (name, rest) = line.split_at(colon);
    if name.is_empty() || !name.iter().all(|b| is_tchar(*b)) {
        return Err(MALFORMED);
    }
    let value = rest.get(1..).unwrap_or_default();
    let value = std::str::from_utf8(value)
        .map_err(|_| reject(400, "見出しの値が UTF-8 ではありません。"))?
        .trim_matches([' ', '\t']);
    let name = std::str::from_utf8(name).map_err(|_| MALFORMED)?;
    Ok((name.to_ascii_lowercase(), value.to_owned()))
}

fn content_length(head: &Head) -> Result<u64, Reject> {
    let bad = reject(400, "Content-Length の値が正しくありません。");
    let mut found: Option<u64> = None;
    for value in head.headers_named("content-length") {
        if value.is_empty() || value.len() > 20 || !value.bytes().all(|b| b.is_ascii_digit()) {
            return Err(bad);
        }
        let parsed: u64 = value.parse().map_err(|_| bad)?;
        if found.is_some_and(|f| f != parsed) {
            return Err(bad);
        }
        found = Some(parsed);
    }
    Ok(found.unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Result<Head, Reject> {
        parse_head(text.as_bytes(), &Limits::default())
    }

    #[test]
    fn reads_a_plain_request() {
        let head =
            parse("POST /api/v1/x?y=1 HTTP/1.1\r\nHost: a\r\nContent-Length: 12\r\n\r\n").unwrap();
        assert_eq!(head.method, Method::Post);
        assert_eq!(head.path, "/api/v1/x");
        assert_eq!(head.query, "y=1");
        assert_eq!(head.content_length, 12);
        assert_eq!(head.header("host"), Some("a"));
    }

    #[test]
    fn head_end_accepts_crlf_and_lf() {
        assert_eq!(head_end(b"GET / HTTP/1.1\r\n\r\nrest"), Some(18));
        assert_eq!(head_end(b"GET / HTTP/1.1\n\nrest"), Some(16));
        assert_eq!(head_end(b"GET / HTTP/1.1\r\nHost: a\r\n"), None);
    }

    #[test]
    fn rejects_broken_lengths_and_encodings() {
        let status = |t: &str| parse(t).map_or_else(|r| r.status, |_| 0);
        assert_eq!(status("GET / HTTP/1.1\r\nContent-Length: -1\r\n\r\n"), 400);
        assert_eq!(status("GET / HTTP/1.1\r\nContent-Length: 1x\r\n\r\n"), 400);
        assert_eq!(
            status("GET / HTTP/1.1\r\nContent-Length: 99999999999999999999\r\n\r\n"),
            400
        );
        assert_eq!(
            status("GET / HTTP/1.1\r\nContent-Length: 1\r\nContent-Length: 2\r\n\r\n"),
            400
        );
        assert_eq!(
            status("GET / HTTP/1.1\r\nContent-Length: 2\r\nContent-Length: 2\r\n\r\n"),
            0
        );
        assert_eq!(
            status("POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n"),
            501
        );
        assert_eq!(status("GET / HTTP/1.1\r\nExpect: something\r\n\r\n"), 417);
        assert_eq!(status("GET / HTTP/2.0\r\n\r\n"), 505);
        assert_eq!(status("OPTIONS / HTTP/1.1\r\n\r\n"), 405);
        assert_eq!(status("GET  / HTTP/1.1\r\n\r\n"), 400);
        assert_eq!(status("GET http://x/ HTTP/1.1\r\n\r\n"), 400);
        assert_eq!(status("GET / HTTP/1.1\r\nX-A: a\u{1}b\r\n\r\n"), 400);
        assert_eq!(status("GET / HTTP/1.1\r\n folded\r\n\r\n"), 400);
        assert_eq!(status("GET / HTTP/1.1\r\nHost: a\r\nHost: b\r\n\r\n"), 400);
        assert_eq!(status("GET / HTTP/1.1\r\nBad Name: a\r\n\r\n"), 400);
    }

    #[test]
    fn enforces_counts_and_sizes() {
        let limits = Limits {
            request_line: 32,
            header_bytes: 64,
            header_count: 2,
            ..Limits::default()
        };
        let many = "GET / HTTP/1.1\r\nA: 1\r\nB: 2\r\nC: 3\r\n\r\n";
        assert_eq!(
            parse_head(many.as_bytes(), &limits).unwrap_err().status,
            431
        );
        let long = format!("GET /{} HTTP/1.1\r\n\r\n", "a".repeat(40));
        assert_eq!(
            parse_head(long.as_bytes(), &limits).unwrap_err().status,
            414
        );
        assert_eq!(head_overflow(&[b'a'; 40], &limits).unwrap().status, 414);
        let big = format!("GET / HTTP/1.1\r\nA: {}", "b".repeat(80));
        assert_eq!(head_overflow(big.as_bytes(), &limits).unwrap().status, 431);
    }
}
