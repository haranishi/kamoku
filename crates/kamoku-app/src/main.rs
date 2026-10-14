// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `kamoku` コマンド。中身は [`kamoku_app::cli::run`]。

fn main() {
    // std::env::args は UTF-8 でない引数で止まる（panic）ので、args_os から自分で変換する。
    let mut args = Vec::new();
    for arg in std::env::args_os().skip(1) {
        match arg.into_string() {
            Ok(text) => args.push(text),
            Err(raw) => {
                eprintln!(
                    "引数を読めません（UTF-8 ではない文字があります）: {}",
                    raw.to_string_lossy()
                );
                std::process::exit(2);
            }
        }
    }
    std::process::exit(kamoku_app::cli::run(&args));
}
