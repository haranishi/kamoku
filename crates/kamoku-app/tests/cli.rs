// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! コマンド（`kamoku serve|demo`、版と使い方）。実行ファイルを動かして確かめる。

mod cli_support;
mod common;
use cli_support::{Sandbox, Spawned};
use common::*;
use serde_json::json;

#[test]
fn version_help_and_usage_errors() {
    let sb = Sandbox::new();
    let (code, out, _) = sb.run(&["--version"]);
    assert_eq!(
        (code, out.trim()),
        (0, format!("kamoku {}", env!("CARGO_PKG_VERSION")).as_str())
    );
    let (code, out, _) = sb.run(&["--help"]);
    assert_eq!(code, 0);
    assert!(out.contains("kamoku serve") && out.contains("kamoku backup"));
    assert_eq!(sb.run(&[]).0, 2);
    assert_eq!(sb.run(&["nothing"]).0, 2);
    assert_eq!(sb.run(&["serve", "--bogus"]).0, 2);
    assert_eq!(sb.run(&["serve", "--port", "x"]).0, 2);
    assert_eq!(sb.run(&["backup"]).0, 2);
    assert_eq!(sb.run(&["verify", "--data"]).0, 2);
}

// AC-12.1: demo は合成データで起動し、本物の置き場所（--data・KAMOKU_DATA・ホーム）には一切触れない。
#[test]
fn demo_runs_in_temp_and_never_touches_real_places() {
    let sb = Sandbox::new();
    let server = Spawned::start(
        &sb,
        &[
            "demo",
            "--no-open",
            "--port",
            "0",
            "--data",
            sb.path("data").to_str().unwrap(),
        ],
    );
    let c = server.login();
    let st = c.api("/status").json();
    assert_eq!(st["mode"], "demo");
    assert_eq!(st["ledger"]["verified"], true);
    assert_eq!(st["inbox"]["draft"], 25);
    assert_eq!(st["setup_done"], true);
    let head = st["ledger"]["head_hash"].clone();
    let dashboard = c.api("/reports/dashboard?month=2026-09").json();
    assert!(dashboard["month_summary"]["revenue"].as_i64().unwrap() > 0);
    let err = server.stop();
    assert!(err.contains("--data を使いません"));
    for place in ["data", "home", "xdg", "appdata"] {
        assert!(sb.is_empty(place), "{place} に何か作られた");
    }
    let made = sb.demo_dirs();
    assert_eq!(made.len(), 1, "{made:?}");
    assert!(made[0].join(".kamoku-demo").is_file());
    assert!(made[0].join("kamoku.db").is_file());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let bits = std::fs::metadata(&made[0]).unwrap().permissions().mode();
        assert_eq!(bits & 0o777, 0o700);
    }
    // AC-12.2: 起動し直すと別の新しいフォルダに作るが、台帳は同じ（同じ head_hash）。
    let again = Spawned::start(&sb, &["demo", "--no-open", "--port", "0"]);
    assert_eq!(
        again.login().api("/status").json()["ledger"]["head_hash"],
        head
    );
    again.stop();
    assert_eq!(sb.demo_dirs().len(), 2);
}

// AC-12.2: 合成データは毎回同じ内容になる（同じ入力から同じ head_hash）。
#[test]
fn demo_data_is_deterministic() {
    let make = || {
        let dir = tempfile::tempdir().unwrap();
        let (mut store, _) = kamoku_store::Store::open(
            dir.path().join("kamoku.db"),
            kamoku_store::OpenOptions {
                create_if_missing: true,
            },
        )
        .unwrap();
        kamoku_app::demo::populate(&mut store).unwrap()
    };
    let a = make();
    let b = make();
    assert_eq!(a, b);
    assert_eq!(a.drafts, 25);
    assert!(a.entries > 9 * 12, "{a:?}");
}

// B3: demo は起動のたびに新しいフォルダを作り、すでにあるフォルダは消さない（目印があっても、
// リンクであっても）。中を見ずに作るので、一時フォルダの祖先がリンクでも、リンクの先を消さない。
#[test]
fn demo_never_deletes_existing_folders() {
    let sb = Sandbox::new();
    let old = sb.path("tmp").join("kamoku-demo");
    std::fs::create_dir(&old).unwrap();
    std::fs::write(old.join(".kamoku-demo"), "").unwrap();
    std::fs::write(old.join("大事なもの.txt"), "消さないで").unwrap();
    let target = sb.path("home").join("target");
    std::fs::create_dir(&target).unwrap();
    std::fs::write(target.join("本物.txt"), "x").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&target, sb.path("tmp").join("kamoku-demo-link")).unwrap();
    let server = Spawned::start(&sb, &["demo", "--no-open", "--port", "0"]);
    assert_eq!(server.login().api("/status").json()["mode"], "demo");
    server.stop();
    assert!(old.join("大事なもの.txt").is_file());
    assert!(target.join("本物.txt").is_file());
    assert_eq!(sb.demo_dirs().len(), 1, "{:?}", sb.demo_dirs());
}

// B3: 2つの demo を同時に動かしても、互いの帳簿を消さない。それぞれ登録・検証・バックアップができ、
// 止めたあとも、それぞれのフォルダに自分の登録だけが残る。
#[test]
fn two_demos_keep_separate_books() {
    let sb = Sandbox::new();
    let a = Spawned::start(&sb, &["demo", "--no-open", "--port", "0"]);
    let a_dir = sb.demo_dirs().pop().unwrap();
    let a_client = a.login();
    let mut input = expense("2026-10-02", 1100, "exp_comm", "w01");
    input["description"] = json!("Aの登録");
    create(&a_client, &input);
    let b = Spawned::start(&sb, &["demo", "--no-open", "--port", "0"]);
    let dirs = sb.demo_dirs();
    assert_eq!(dirs.len(), 2, "{dirs:?}");
    let b_dir = dirs.into_iter().find(|d| *d != a_dir).unwrap();
    let b_client = b.login();
    input["description"] = json!("Bの登録");
    create(&b_client, &input);
    for c in [&a_client, &b_client] {
        let verified = c.post("/verify", &json!({}));
        assert_eq!(verified.json()["ok"], true, "{}", verified.text());
        let backup = c.api("/export/backup.db");
        assert_eq!(backup.status, 200);
        assert!(backup.body.starts_with(b"SQLite format 3\0"));
    }
    a.stop();
    b.stop();
    for (dir, mine, other) in [
        (&a_dir, "Aの登録", "Bの登録"),
        (&b_dir, "Bの登録", "Aの登録"),
    ] {
        let (store, _) =
            kamoku_store::Store::open(dir.join("kamoku.db"), kamoku_store::OpenOptions::default())
                .unwrap();
        let texts: Vec<String> = store
            .ledger()
            .events()
            .iter()
            .filter_map(|e| e.entry.as_ref().map(|x| x.description().to_owned()))
            .collect();
        assert!(texts.iter().any(|t| t == mine), "{dir:?}");
        assert!(!texts.iter().any(|t| t == other), "{dir:?}");
    }
}

// serve の置き場所は --data > KAMOKU_DATA。合言葉を標準エラーに出さない。
#[test]
fn serve_uses_flag_then_environment() {
    let sb = Sandbox::new();
    let server = Spawned::start(&sb, &["serve", "--no-open", "--port", "0"]);
    let token = server.token.clone();
    assert_eq!(server.login().api("/status").json()["mode"], "normal");
    let bad = raw(
        server.port,
        format!(
            "GET /?t=wrong{token} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n\r\n",
            server.port
        )
        .as_bytes(),
    );
    assert_eq!(bad.status, 401);
    let err = server.stop();
    assert!(
        !err.contains(&token) && !err.contains("t="),
        "標準エラーに合言葉: {err}"
    );
    assert!(sb.path("data").join("kamoku.db").is_file());
    let flag = sb.path("home").join("flag");
    let server = Spawned::start(
        &sb,
        &[
            "serve",
            "--no-open",
            "--port",
            "0",
            "--data",
            flag.to_str().unwrap(),
        ],
    );
    server.stop();
    assert!(flag.join("kamoku.db").is_file());
    assert!(sb.is_empty("xdg"));
}
