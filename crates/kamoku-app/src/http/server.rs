// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 待ち受けと、決まった数の処理のスレッド。

use super::conn::{self, Outcome};
use super::request::{Head, Request};
use super::response::Response;
use std::io::{self, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// 上限と時間切れ。既定値は `docs/spec/api-common.md` の「受け口の限界」と同じ。試験では小さな値に差し替えられる。
#[derive(Debug, Clone)]
pub struct Limits {
    /// 要求行の上限（バイト）。
    pub request_line: usize,
    /// 見出し全体の上限（バイト）。
    pub header_bytes: usize,
    /// 見出しの数の上限。
    pub header_count: usize,
    /// 1回の読み取りの時間切れ。
    pub read_timeout: Duration,
    /// 見出しと本文を読み終えるまでの時間切れ。
    pub request_timeout: Duration,
    /// 書き込みの時間切れ。
    pub write_timeout: Duration,
    /// 応答の後、相手の残りの送信を読み捨てて待つ時間の上限。
    pub linger: Duration,
    /// 応答の後に読み捨てるバイト数の上限。
    pub linger_bytes: usize,
    /// 同時に受け持つ接続の上限。超えた接続には503を返して閉じる。
    pub max_connections: usize,
    /// 要求を処理するスレッドの数（接続ごとにスレッドを作らない）。
    pub workers: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            request_line: 8 * 1024,
            header_bytes: 16 * 1024,
            header_count: 100,
            read_timeout: Duration::from_secs(10),
            request_timeout: Duration::from_secs(30),
            write_timeout: Duration::from_secs(30),
            linger: Duration::from_secs(1),
            linger_bytes: 1024 * 1024,
            max_connections: 32,
            workers: 8,
        }
    }
}

/// 受け口の向こう側（認証、仕訳・帳票などの処理）。受け口は、この4つだけを呼ぶ。
pub trait Handler: Send + Sync + 'static {
    /// 本文を読む前の検査。通すなら本文の上限（バイト）を返す。
    ///
    /// # Errors
    /// 断るときは、その応答を返す（本文は読まずに閉じる）。
    fn inspect(&self, head: &Head) -> Result<u64, Response>;
    /// 要求に応える。
    fn handle(&self, request: Request) -> Response;
    /// 受け口が自分で断るときの応答（400・408・413・414・417・431・501・503・505）。
    fn reject(&self, status: u16, message: &str) -> Response;
    /// すべての応答に共通の見出し（CSPなど）を足す。
    fn decorate(&self, response: &mut Response);
}

/// 動いている受け口。
#[derive(Debug)]
pub struct Running {
    addr: SocketAddr,
    stop: Arc<AtomicBool>,
    accept: Option<JoinHandle<()>>,
}
impl Running {
    /// 待ち受けている番地。
    #[must_use]
    pub const fn addr(&self) -> SocketAddr {
        self.addr
    }
    /// 止める合図を出します。待ち受けは、次の接続を受けたときに終わります
    /// （標準ライブラリだけでは、受け付けの待ちを外から解けないため）。
    pub fn request_stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }
    /// 待ち受けと処理のスレッドが終わるまで待ちます。先に [`Running::request_stop`] を呼び、
    /// 1度接続して待ちを解いてください。
    pub fn join(mut self) {
        if let Some(accept) = self.accept.take() {
            let _ = accept.join();
        }
    }
}

// 受け持っている接続の数。手放すと1つ減る。
struct Slot(Arc<AtomicUsize>);
impl Drop for Slot {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}
// 接続、受け持ちの数の印、受け付けた時刻（要求の期限の起点）。
type Job = (TcpStream, Slot, Instant);

/// 受け付けを始めます。`listener` は呼び出し側が `127.0.0.1` で作ったもの。
///
/// # Errors
/// スレッドを作れないとき。
pub fn start<H: Handler>(
    listener: TcpListener,
    limits: &Limits,
    handler: Arc<H>,
) -> io::Result<Running> {
    let limits = limits.clone();
    let addr = listener.local_addr()?;
    let stop = Arc::new(AtomicBool::new(false));
    let (sender, receiver) = mpsc::channel::<Job>();
    let receiver = Arc::new(Mutex::new(receiver));
    let mut workers = Vec::new();
    for i in 0..limits.workers.max(1) {
        let receiver = Arc::clone(&receiver);
        let handler = Arc::clone(&handler);
        let limits = limits.clone();
        workers.push(
            thread::Builder::new()
                .name(format!("kamoku-worker-{i}"))
                .spawn(move || work(&receiver, handler.as_ref(), &limits))?,
        );
    }
    let flag = Arc::clone(&stop);
    let accept = thread::Builder::new()
        .name("kamoku-accept".to_owned())
        .spawn(move || {
            let active = Arc::new(AtomicUsize::new(0));
            for incoming in listener.incoming() {
                if flag.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(stream) = incoming else {
                    // 開けるファイルの数が尽きたときなど。待ち受けは続ける。
                    thread::sleep(Duration::from_millis(50));
                    continue;
                };
                if active.load(Ordering::SeqCst) >= limits.max_connections {
                    refuse_busy(&stream, handler.as_ref());
                    continue;
                }
                active.fetch_add(1, Ordering::SeqCst);
                let job = (stream, Slot(Arc::clone(&active)), Instant::now());
                if sender.send(job).is_err() {
                    break;
                }
            }
            drop(sender);
            for worker in workers {
                let _ = worker.join();
            }
        })?;
    Ok(Running {
        addr,
        stop,
        accept: Some(accept),
    })
}

fn work<H: Handler>(receiver: &Mutex<Receiver<Job>>, handler: &H, limits: &Limits) {
    loop {
        let job = receiver
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .recv();
        let Ok((stream, slot, accepted)) = job else {
            return;
        };
        // 処理の中の想定外の失敗でも、このスレッドは止めない（止まると受け口が細っていく）。
        let _ = catch_unwind(AssertUnwindSafe(|| {
            serve(&stream, handler, limits, accepted);
        }));
        drop(stream);
        drop(slot);
    }
}

fn serve<H: Handler>(stream: &TcpStream, handler: &H, limits: &Limits, accepted: Instant) {
    let outcome = conn::read_request(stream, limits, accepted, |h| handler.inspect(h));
    let (mut response, send_body) = match outcome {
        Outcome::Request(request) => {
            let send_body = request.head.method != super::Method::Head;
            (handler.handle(request), send_body)
        }
        Outcome::Respond(response, send_body) => (response, send_body),
        Outcome::Reject {
            status,
            message,
            body,
        } => (handler.reject(status, message), body),
        Outcome::Close => {
            conn::finish(stream, limits, None);
            return;
        }
    };
    handler.decorate(&mut response);
    conn::finish(stream, limits, Some(&response.to_bytes(send_body)));
}

fn refuse_busy<H: Handler>(stream: &TcpStream, handler: &H) {
    let mut response = handler.reject(
        503,
        "混み合っています。少し待ってから、もう一度試してください。",
    );
    handler.decorate(&mut response);
    if stream
        .set_write_timeout(Some(Duration::from_secs(1)))
        .is_ok()
    {
        let _ = (&*stream).write_all(&response.to_bytes(true));
    }
    let _ = stream.shutdown(Shutdown::Both);
}
