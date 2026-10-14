// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 外へ通信しない確認（`docs/spec/nfr.md` の安全、`architecture.md` の「通信しない」）。
//! 本番のソース（`src/` と `build.rs`）と依存の宣言を、文字列で検索する。試験の道具（`tests/`）は除く。

use std::path::{Path, PathBuf};

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn production_code_has_no_network_client() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = vec![root.join("build.rs")];
    rust_files(&root.join("src"), &mut files);
    assert!(files.len() > 10);
    let forbidden = [
        "TcpStream::connect",
        "UdpSocket",
        "ureq",
        "reqwest",
        "hyper::client",
        "isahc",
        "attohttpc",
        "curl::",
        "std::net::TcpStream::connect",
    ];
    for file in &files {
        let text = std::fs::read_to_string(file).unwrap();
        for word in forbidden {
            assert!(
                !text.contains(word),
                "{} に「{word}」がある",
                file.display()
            );
        }
    }
    let manifest = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();
    for client in [
        "ureq",
        "reqwest",
        "hyper",
        "curl",
        "isahc",
        "attohttpc",
        "tiny_http",
        "tokio",
    ] {
        assert!(
            !manifest.contains(&format!("\n{client} "))
                && !manifest.contains(&format!("\n{client}=")),
            "依存に {client}"
        );
    }
}
