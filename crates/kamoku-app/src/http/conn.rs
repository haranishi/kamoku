// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 1つの接続から、時間切れと上限つきで要求を読み、応答を書いて閉じる。

use super::request::{self, Head, Method, Request};
use super::response::Response;
use super::server::Limits;
use std::io::{ErrorKind, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::time::{Duration, Instant};

/// 読み取りの結果。
pub(crate) enum Outcome {
    /// 本文まで読めた。
    Request(Request),
    /// 検査で断った（応答と、本文を送るか）。
    Respond(Response, bool),
    /// 受け口が自分で断る（状態、原因、本文を送るか）。HEAD への断りには本文を付けない。
    Reject {
        /// 状態。
        status: u16,
        /// 原因（利用者向けの文）。
        message: &'static str,
        /// 本文を送るか。
        body: bool,
    },
    /// 何も返さずに閉じる（相手がいない、何も届かないまま時間切れ）。
    Close,
}

enum Step {
    Data(usize),
    Eof,
    Timeout,
}

struct Reader<'a> {
    stream: &'a TcpStream,
    deadline: Instant,
    per_read: Duration,
}
impl Reader<'_> {
    // 1回の読み取りは per_read まで、全体は deadline まで。どちらか早い方で切る。
    fn read_some(&self, buf: &mut [u8]) -> Step {
        loop {
            let Some(left) = self
                .deadline
                .checked_duration_since(Instant::now())
                .filter(|d| !d.is_zero())
            else {
                return Step::Timeout;
            };
            if self
                .stream
                .set_read_timeout(Some(left.min(self.per_read)))
                .is_err()
            {
                return Step::Eof;
            }
            match (&*self.stream).read(buf) {
                Ok(0) => return Step::Eof,
                Ok(n) => return Step::Data(n),
                Err(e) if e.kind() == ErrorKind::Interrupted => {}
                Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                    return Step::Timeout;
                }
                Err(_) => return Step::Eof,
            }
        }
    }
}

const fn reject(status: u16, message: &'static str, body: bool) -> Outcome {
    Outcome::Reject {
        status,
        message,
        body,
    }
}

// 見出しを読み切る前の断りでも、HEAD を名乗る要求には本文を付けない。
fn body_for(buf: &[u8]) -> bool {
    !buf.starts_with(b"HEAD ")
}

/// 見出しを読み、`inspect` で検査し、通れば本文を読みます。
/// `inspect` は、通すなら本文の上限を、断るなら応答を返します。
/// 期限は、処理のスレッドが取り出した時ではなく、受け付けた時（`accepted`）から数える。
pub(crate) fn read_request(
    stream: &TcpStream,
    limits: &Limits,
    accepted: Instant,
    inspect: impl FnOnce(&Head) -> Result<u64, Response>,
) -> Outcome {
    let deadline = accepted + limits.request_timeout;
    // 処理のスレッドを待つ間に期限を過ぎた接続は、読まずに断る。まだ何も読んでいないので、
    // HEAD かどうかは分からず、本文を付ける。
    if Instant::now() >= deadline {
        return reject(
            408,
            "混み合っていて、時間内に受け取れませんでした。もう一度試してください。",
            true,
        );
    }
    let reader = Reader {
        stream,
        deadline,
        per_read: limits.read_timeout,
    };
    let mut buf: Vec<u8> = Vec::with_capacity(2048);
    let mut chunk = [0u8; 4096];
    let end = loop {
        if let Some(end) = request::head_end(&buf) {
            break end;
        }
        if let Some(r) = request::head_overflow(&buf, limits) {
            return reject(r.status, r.message, body_for(&buf));
        }
        match reader.read_some(&mut chunk) {
            Step::Data(n) => buf.extend_from_slice(chunk.get(..n).unwrap_or_default()),
            Step::Eof => return Outcome::Close,
            // 何も届かないまま切れた接続（ブラウザの先回りの接続など）には、何も返さない。
            Step::Timeout if buf.is_empty() => return Outcome::Close,
            Step::Timeout => {
                return reject(
                    408,
                    "見出しの受け取りが時間切れになりました。",
                    body_for(&buf),
                );
            }
        }
    };
    let head = match request::parse_head(buf.get(..end).unwrap_or_default(), limits) {
        Ok(head) => head,
        Err(r) => return reject(r.status, r.message, body_for(&buf)),
    };
    let send_body = head.method != Method::Head;
    let limit = match inspect(&head) {
        Ok(limit) => limit,
        Err(response) => return Outcome::Respond(response, send_body),
    };
    // 本文は読む前に長さを比べる。巨大な長さを書いた要求でも、何も確保しない。
    if head.content_length > limit {
        return reject(413, "本文が大きすぎます。", send_body);
    }
    let Ok(want) = usize::try_from(head.content_length) else {
        return reject(413, "本文が大きすぎます。", send_body);
    };
    // 暫定の応答（100）には、共通の見出し（CSPなど）を付けない。中身の無い合図で、ブラウザは
    // 表示に使わず、共通の見出しは続く最終の応答に付くため。
    if head.expect_continue
        && (&*stream)
            .write_all(b"HTTP/1.1 100 Continue\r\n\r\n")
            .is_err()
    {
        return Outcome::Close;
    }
    let mut body = buf.split_off(end);
    // 後に続く要求（パイプライン）は読まない。最初の要求の本文の分だけ使う。
    body.truncate(want);
    while body.len() < want {
        let room = (want - body.len()).min(chunk.len());
        let Some(slot) = chunk.get_mut(..room) else {
            return Outcome::Close;
        };
        match reader.read_some(slot) {
            Step::Data(n) => body.extend_from_slice(chunk.get(..n).unwrap_or_default()),
            Step::Eof => {
                return reject(
                    400,
                    "本文が Content-Length より短いまま、送信が終わりました。",
                    send_body,
                );
            }
            Step::Timeout => {
                return reject(408, "本文の受け取りが時間切れになりました。", send_body);
            }
        }
    }
    Outcome::Request(Request { head, body })
}

/// 応答を書き、こちらの送信を閉じてから、相手の残りの送信を少しだけ読み捨てて閉じる。
/// 読み残しがあるまま閉じると、相手に応答が届く前に接続が切られる（RST）ことがあるため。
pub(crate) fn finish(stream: &TcpStream, limits: &Limits, bytes: Option<&[u8]>) {
    if let Some(bytes) = bytes
        && stream.set_write_timeout(Some(limits.write_timeout)).is_ok()
    {
        let _ = (&*stream).write_all(bytes);
        let _ = (&*stream).flush();
    }
    let _ = stream.shutdown(Shutdown::Write);
    let until = Instant::now() + limits.linger;
    let mut total = 0usize;
    let mut sink = [0u8; 4096];
    while total < limits.linger_bytes {
        let Some(left) = until
            .checked_duration_since(Instant::now())
            .filter(|d| !d.is_zero())
        else {
            break;
        };
        if stream.set_read_timeout(Some(left)).is_err() {
            break;
        }
        match (&*stream).read(&mut sink) {
            Ok(0) | Err(_) => break,
            Ok(n) => total = total.saturating_add(n),
        }
    }
}
