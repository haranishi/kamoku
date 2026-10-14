// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 標準のブラウザでアドレスを開く。失敗しても起動は続ける（アドレスは画面に出してある）。

use std::process::{Command, Stdio};

/// 開けたら真。
pub(crate) fn open(url: &str) -> bool {
    let mut command = opener(url);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    match command.spawn() {
        Ok(mut child) => {
            // 終わった子を片付ける（残すと、止めるまでゾンビになる）。
            let _ = std::thread::Builder::new()
                .name("kamoku-browser".to_owned())
                .spawn(move || {
                    let _ = child.wait();
                });
            true
        }
        Err(_) => false,
    }
}

#[cfg(target_os = "macos")]
fn opener(url: &str) -> Command {
    let mut command = Command::new("open");
    command.arg(url);
    command
}

#[cfg(windows)]
fn opener(url: &str) -> Command {
    let mut command = Command::new("cmd");
    // start の最初の引用符つきの引数は窓の題名として読まれるので、空の題名を先に渡す。
    command.args(["/c", "start", "", url]);
    command
}

#[cfg(all(not(target_os = "macos"), not(windows)))]
fn opener(url: &str) -> Command {
    let mut command = Command::new("xdg-open");
    command.arg(url);
    command
}
