// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! コマンドの試験の道具。実行ファイルを、本物の置き場所に触れない環境で動かす。

// 試験のファイルごとに使う道具が違うため、使わないものがあっても警告にしない。
#![allow(dead_code)]

use crate::common::{Client, raw};
use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

pub const KAMOKU: &str = env!("CARGO_BIN_EXE_kamoku");

/// 本物の置き場所を指しうる環境変数を、すべて一時フォルダに向けた環境。
pub struct Sandbox {
    root: tempfile::TempDir,
}
impl Sandbox {
    pub fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        for name in ["home", "data", "xdg", "appdata", "tmp"] {
            std::fs::create_dir(root.path().join(name)).unwrap();
        }
        Self { root }
    }
    pub fn path(&self, name: &str) -> PathBuf {
        self.root.path().join(name)
    }
    pub fn command(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(KAMOKU);
        cmd.args(args)
            .env("HOME", self.path("home"))
            .env("KAMOKU_DATA", self.path("data"))
            .env("XDG_DATA_HOME", self.path("xdg"))
            .env("APPDATA", self.path("appdata"))
            .env("TMPDIR", self.path("tmp"))
            .stdin(Stdio::null());
        cmd
    }
    /// 終わるはずのコマンドを動かす。60秒で終わらなければ止めて、終了コードを -1 にする
    /// （断るはずの demo が誤って起動したとき、試験が止まらずに赤くなるように）。
    pub fn run(&self, args: &[&str]) -> (i32, String, String) {
        let mut child = self
            .command(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let read = |pipe: Option<Box<dyn Read + Send>>| {
            std::thread::spawn(move || {
                let mut text = String::new();
                if let Some(mut p) = pipe {
                    let _ = p.read_to_string(&mut text);
                }
                text
            })
        };
        let out = read(
            child
                .stdout
                .take()
                .map(|p| Box::new(p) as Box<dyn Read + Send>),
        );
        let err = read(
            child
                .stderr
                .take()
                .map(|p| Box::new(p) as Box<dyn Read + Send>),
        );
        let deadline = std::time::Instant::now() + Duration::from_secs(60);
        let code = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status.code().unwrap_or(-1);
            }
            if std::time::Instant::now() > deadline {
                let _ = child.kill();
                let _ = child.wait();
                break -1;
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        (code, out.join().unwrap(), err.join().unwrap())
    }
    /// 一時フォルダの中の、demo が作ったフォルダ（`kamoku-demo-` で始まるもの）。
    pub fn demo_dirs(&self) -> Vec<PathBuf> {
        let mut dirs: Vec<PathBuf> = std::fs::read_dir(self.path("tmp"))
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().starts_with("kamoku-demo-"))
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .map(|e| e.path())
            .collect();
        dirs.sort();
        dirs
    }
    pub fn is_empty(&self, name: &str) -> bool {
        std::fs::read_dir(self.path(name)).unwrap().next().is_none()
    }
}

/// 起動したサーバーのプロセス。手放すと止める。
pub struct Spawned {
    child: Child,
    pub port: u16,
    pub token: String,
}
impl Spawned {
    pub fn start(sandbox: &Sandbox, args: &[&str]) -> Self {
        let child = sandbox
            .command(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        // 先に包んでおく。アドレスを待つ間に失敗しても、手放した時点で止まる。
        let mut me = Self {
            child,
            port: 0,
            token: String::new(),
        };
        let stdout = me.child.stdout.take().unwrap();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                let _ = tx.send(line);
            }
        });
        let url = loop {
            let line = rx
                .recv_timeout(Duration::from_secs(120))
                .expect("アドレスが出ない");
            if let Some(rest) = line.strip_prefix("http://127.0.0.1:") {
                break rest.to_owned();
            }
        };
        let (port, token) = url.split_once("/?t=").unwrap();
        me.port = port.parse().unwrap();
        token.clone_into(&mut me.token);
        me
    }
    pub fn login(&self) -> Client {
        let r = raw(
            self.port,
            format!(
                "GET /?t={} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n\r\n",
                self.token, self.port
            )
            .as_bytes(),
        );
        assert_eq!(r.status, 303);
        Client {
            port: self.port,
            cookie: Some(
                r.header("set-cookie")
                    .unwrap()
                    .split(';')
                    .next()
                    .unwrap()
                    .to_owned(),
            ),
        }
    }
    pub fn stop(mut self) -> String {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let mut err = String::new();
        if let Some(mut e) = self.child.stderr.take() {
            let _ = e.read_to_string(&mut err);
        }
        err
    }
}
impl Drop for Spawned {
    // 試験が途中で失敗しても、起動したサーバーを残さない。変異試験の後に、
    // 消えた作業場所のサーバーが待ち受けたまま残っていたことがある。
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
