// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! コマンド（`kamoku serve|demo|verify|backup`）。引数は依存を足さずに自前で読む。

use crate::server::{self, Mode, ServeOptions};
use crate::{browser, demo, paths};
use kamoku_store::{OpenOptions, Store};
use std::io::Write;
use std::path::PathBuf;

const USAGE: &str = "kamoku — 手元で動く、個人事業主の帳簿

使い方:
  kamoku serve  [--data DIR] [--port N] [--no-open] [--ui-dir DIR]
  kamoku demo   [--port N] [--no-open] [--ui-dir DIR]（--data は見ない）
  kamoku verify [--data DIR]
  kamoku backup --out FILE [--data DIR]
  kamoku --version
  kamoku --help

  serve   帳簿のサーバーを起動し、ブラウザで開きます。
  demo    合成データの帳簿で起動します。起動のたびに一時フォルダの中に新しいフォルダを作り、
          本物の帳簿には触れません（作ったフォルダは消しません）。
  verify  台帳の連鎖を検証します。壊れていれば、最初の連番と規則を出して、終了コード1で終わります。
  backup  帳簿の複製を書き出します。すでにあるファイルは上書きしません。

  --data DIR    帳簿の置き場所。無ければ環境変数 KAMOKU_DATA、それも無ければOSごとの既定の場所。
  --port N      待ち受けの番号（既定は8765。使用中なら次の番号を試す。0なら空いている番号）。
  --no-open     ブラウザを開きません。
  --ui-dir DIR  画面のファイルを、このフォルダから読みます（画面を直しながら確かめるとき）。
  --out FILE    バックアップの書き出し先。
";

/// 読んだ引数。
#[derive(Debug, Default)]
struct Args {
    data: Option<PathBuf>,
    port: Option<u16>,
    no_open: bool,
    ui_dir: Option<PathBuf>,
    out: Option<PathBuf>,
}

/// コマンドを実行して、終了コードを返します。0は成功、1は失敗や台帳の不正、2は使い方の誤り。
#[must_use]
pub fn run(args: &[String]) -> i32 {
    let Some((command, rest)) = args.split_first() else {
        eprint!("{USAGE}");
        return 2;
    };
    match command.as_str() {
        "--help" | "-h" | "help" => {
            print!("{USAGE}");
            0
        }
        "--version" | "-V" | "version" => {
            println!("kamoku {}", env!("CARGO_PKG_VERSION"));
            0
        }
        "serve" => with_args(rest, &["--data", "--port", "--no-open", "--ui-dir"], serve),
        "demo" => with_args(
            rest,
            &["--data", "--port", "--no-open", "--ui-dir"],
            demo_command,
        ),
        "verify" => with_args(rest, &["--data"], verify),
        "backup" => with_args(rest, &["--data", "--out"], backup),
        other => {
            eprintln!("知らないコマンドです: {other}\n");
            eprint!("{USAGE}");
            2
        }
    }
}

fn with_args(rest: &[String], allowed: &[&str], run: fn(&Args) -> i32) -> i32 {
    match parse(rest, allowed) {
        Ok(args) => run(&args),
        Err(message) => {
            eprintln!("{message}\n");
            eprint!("{USAGE}");
            2
        }
    }
}

fn parse(rest: &[String], allowed: &[&str]) -> Result<Args, String> {
    let mut args = Args::default();
    let mut iter = rest.iter();
    while let Some(arg) = iter.next() {
        let (name, inline) = match arg.split_once('=') {
            Some((name, value)) if name.starts_with("--") => (name, Some(value.to_owned())),
            _ => (arg.as_str(), None),
        };
        if !allowed.contains(&name) {
            return Err(format!("このコマンドでは使えない指定です: {name}"));
        }
        if name == "--no-open" {
            if inline.is_some() {
                return Err("--no-open には値を付けません。".to_owned());
            }
            args.no_open = true;
            continue;
        }
        let value = match inline {
            Some(value) => value,
            None => iter
                .next()
                .cloned()
                .ok_or_else(|| format!("{name} の後に値を書いてください。"))?,
        };
        match name {
            "--data" => args.data = Some(PathBuf::from(value)),
            "--ui-dir" => args.ui_dir = Some(PathBuf::from(value)),
            "--out" => args.out = Some(PathBuf::from(value)),
            _ => {
                args.port = Some(value.parse().map_err(|_| {
                    format!("--port は0から65535までの数で書いてください（いまは「{value}」）。")
                })?);
            }
        }
    }
    Ok(args)
}

fn options_for(db_path: PathBuf, mode: Mode, args: &Args) -> ServeOptions {
    let mut options = ServeOptions::new(db_path);
    options.mode = mode;
    if let Some(port) = args.port {
        options.port = port;
    }
    options.ui_dir.clone_from(&args.ui_dir);
    options
}

fn serve(args: &Args) -> i32 {
    let dir = match paths::data_dir(args.data.as_deref()) {
        Ok(dir) => dir,
        Err(message) => {
            eprintln!("{message}");
            return 1;
        }
    };
    let options = options_for(dir.join(paths::DB_FILE), Mode::Normal, args);
    launch(&options, args.no_open, &format!("帳簿: {}", dir.display()))
}

fn demo_command(args: &Args) -> i32 {
    // デモは --data も環境変数 KAMOKU_DATA も見ない。本物の置き場所を一切指さないため。
    if args.data.is_some() {
        eprintln!(
            "デモは --data を使いません。本物の帳簿の置き場所には触れず、一時フォルダで動きます。"
        );
    }
    let dir = match paths::prepare_demo_dir() {
        Ok(dir) => dir,
        Err(message) => {
            eprintln!("{message}");
            return 1;
        }
    };
    let db = dir.join(paths::DB_FILE);
    let populated = Store::open(
        &db,
        OpenOptions {
            create_if_missing: true,
        },
    )
    .map_err(|e| e.to_string())
    .and_then(|(mut store, _)| demo::populate(&mut store).map_err(|e| e.to_string()));
    if let Err(message) = populated {
        eprintln!("合成データを作れませんでした: {message}");
        return 1;
    }
    let options = options_for(db, Mode::Demo, args);
    launch(
        &options,
        args.no_open,
        &format!(
            "デモのデータ。起動のたびに新しいフォルダに作ります: {}",
            dir.display()
        ),
    )
}

fn launch(options: &ServeOptions, no_open: bool, place: &str) -> i32 {
    let server = match server::start(options) {
        Ok(server) => server,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
    if let Some(reason) = server.read_only() {
        eprintln!(
            "台帳の検証で不正が見つかりました（{reason}）。読み取り専用で開きます。書き込みはできません。"
        );
    }
    // 合言葉つきのアドレスは標準出力にだけ出す（標準エラーや記録には出さない）。
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "kamoku を起動しました（{place}）。");
    let _ = writeln!(
        out,
        "ブラウザで次のアドレスを開いてください。起動のたびに変わります。人に教えないでください:"
    );
    let _ = writeln!(out, "{}", server.url());
    let _ = writeln!(out, "止めるときは Ctrl+C を押します。");
    let _ = out.flush();
    drop(out);
    if !no_open && !browser::open(&server.url()) {
        eprintln!("ブラウザを開けませんでした。上のアドレスを開いてください。");
    }
    server.join();
    0
}

fn open_existing(args: &Args) -> Result<(PathBuf, Store, kamoku_store::LoadReport), String> {
    let dir = paths::data_dir(args.data.as_deref())?;
    let db = dir.join(paths::DB_FILE);
    if !db.is_file() {
        return Err(format!("帳簿が見つかりません: {}", db.display()));
    }
    let (store, report) = Store::open(&db, OpenOptions::default())
        .map_err(|e| format!("帳簿を開けませんでした: {e}"))?;
    Ok((db, store, report))
}

fn verify(args: &Args) -> i32 {
    let (db, store, report) = match open_existing(args) {
        Ok(opened) => opened,
        Err(message) => {
            eprintln!("{message}");
            return 1;
        }
    };
    println!("帳簿: {}", db.display());
    println!(
        "イベント数: {}（検証できたもの: {}）",
        report.events_in_db, report.events_loaded
    );
    println!("仕訳数: {}", report.entries);
    match report.verification.and_then(|_| store.verify()) {
        Ok(ok) => {
            println!("末尾のハッシュ: {}", ok.head_hash);
            println!("台帳は正常です。");
            0
        }
        Err(failure) => {
            println!(
                "末尾のハッシュ（検証できた部分）: {}",
                store.ledger().head_hash().to_hex()
            );
            println!(
                "台帳に不正があります。最初の連番: {}、規則: {}{}",
                failure.seq,
                failure.kind,
                failure
                    .rule
                    .as_deref()
                    .map(|r| format!("（{r}）"))
                    .unwrap_or_default()
            );
            1
        }
    }
}

fn backup(args: &Args) -> i32 {
    let Some(out) = &args.out else {
        eprintln!("--out で、書き出し先のファイルを指定してください。\n");
        eprint!("{USAGE}");
        return 2;
    };
    let (_, store, _) = match open_existing(args) {
        Ok(opened) => opened,
        Err(message) => {
            eprintln!("{message}");
            return 1;
        }
    };
    match paths::backup_to_new_file(&store, out) {
        Ok(()) => {
            println!("バックアップを書き出しました: {}", out.display());
            if store.read_only().is_some() {
                println!(
                    "注意: 台帳に不正があるまま複製しました。調べるための証拠として残してください。"
                );
            }
            0
        }
        Err(message) => {
            eprintln!("{message}");
            1
        }
    }
}
