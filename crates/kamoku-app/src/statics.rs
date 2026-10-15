// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 画面のファイルの配信。既定は実行ファイルに埋め込んだもの、`--ui-dir` ならディスクから読む。

use crate::query::percent_decode;
use crate::ui_rules::{content_type, ui_path_allowed};
use std::path::PathBuf;

include!(concat!(env!("OUT_DIR"), "/ui_files.rs"));

/// 画面のファイルの出どころ。
#[derive(Debug, Clone)]
pub(crate) enum Statics {
    /// 実行ファイルに埋め込んだもの。
    Embedded,
    /// ディスクのフォルダ（正規化した絶対パス）。
    Dir(PathBuf),
}

/// パスのファイル（種類と中身）。配らないもの、`..` などで外へ抜けるものは `None`。
pub(crate) fn lookup(statics: &Statics, path: &str) -> Option<(&'static str, Vec<u8>)> {
    let decoded = percent_decode(path, false)?;
    let rel = decoded.strip_prefix('/')?;
    let rel = if rel.is_empty() { "index.html" } else { rel };
    if rel.contains(['\\', '\0']) || !ui_path_allowed(rel) {
        return None;
    }
    match statics {
        Statics::Embedded => UI_FILES
            .iter()
            .find(|(name, _, _)| *name == rel)
            .map(|(_, kind, bytes)| (*kind, bytes.to_vec())),
        Statics::Dir(root) => {
            // シンボリックリンクで外を指していても、正規化した場所がフォルダの中でなければ配らない。
            // 中を指すリンクでも、行き先に同じ決まりを掛ける（別名で dev/ や隠しファイルを配らないため）。
            let file = root.join(rel).canonicalize().ok()?;
            let canonical = file
                .strip_prefix(root)
                .ok()?
                .components()
                .map(|c| c.as_os_str().to_str())
                .collect::<Option<Vec<_>>>()?
                .join("/");
            if !ui_path_allowed(&canonical) || !file.is_file() {
                return None;
            }
            std::fs::read(&file)
                .ok()
                .map(|bytes| (content_type(&canonical), bytes))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_files_follow_the_rules() {
        assert!(UI_FILES.iter().all(|(name, _, _)| ui_path_allowed(name)));
        assert!(
            !UI_FILES
                .iter()
                .any(|(name, _, _)| name.starts_with("tests/")
                    || name.starts_with("dev/")
                    || name.ends_with("README.md")
                    || name.ends_with("package.json"))
        );
    }

    #[test]
    fn traversal_and_hidden_paths_are_refused() {
        for path in [
            "/../Cargo.toml",
            "/%2e%2e/Cargo.toml",
            "/css/../../Cargo.toml",
            "/dev/mock-server.mjs",
            "/tests/index.js",
            "/package.json",
            "/README.md",
            "/.git/config",
            "/js\\main.js",
            "/%00",
        ] {
            assert!(lookup(&Statics::Embedded, path).is_none(), "{path}");
        }
    }

    #[test]
    fn ui_dir_cannot_escape() {
        let dir = tempfile::tempdir().unwrap();
        let ui = dir.path().join("ui");
        std::fs::create_dir(&ui).unwrap();
        std::fs::write(ui.join("index.html"), b"<p>ok</p>").unwrap();
        std::fs::write(dir.path().join("secret.txt"), b"secret").unwrap();
        let statics = Statics::Dir(ui.canonicalize().unwrap());
        assert_eq!(lookup(&statics, "/").unwrap().1, b"<p>ok</p>");
        assert!(lookup(&statics, "/../secret.txt").is_none());
        assert!(lookup(&statics, "/%2e%2e/secret.txt").is_none());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(dir.path().join("secret.txt"), ui.join("link.txt")).unwrap();
            assert!(lookup(&statics, "/link.txt").is_none());
        }
    }

    #[test]
    fn wasm_assets_have_streaming_compilation_mime_type() {
        let dir = tempfile::tempdir().unwrap();
        let pkg = dir.path().join("rust/pkg");
        std::fs::create_dir_all(&pkg).unwrap();
        let header = b"\0asm\x01\0\0\0";
        std::fs::write(pkg.join("kamoku_ui_bg.wasm"), header).unwrap();
        let statics = Statics::Dir(dir.path().canonicalize().unwrap());
        let (mime, bytes) = lookup(&statics, "/rust/pkg/kamoku_ui_bg.wasm").unwrap();
        assert_eq!(mime, "application/wasm");
        assert_eq!(bytes, header);
    }

    // フォルダの中のリンクで、配らないファイル（dev/・隠しファイル・README.md）を別名で配らない。
    // 正規化したあとの場所にも、同じ決まりを掛ける。中を指す普通のリンクは配る。
    #[cfg(unix)]
    #[test]
    fn ui_dir_links_cannot_rename_forbidden_files() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let ui = dir.path().join("ui");
        std::fs::create_dir_all(ui.join("dev")).unwrap();
        std::fs::write(ui.join("index.html"), b"<p>ok</p>").unwrap();
        std::fs::write(ui.join("dev/mock-server.mjs"), b"mock").unwrap();
        std::fs::write(ui.join(".env"), b"secret").unwrap();
        std::fs::write(ui.join("README.md"), b"readme").unwrap();
        symlink(ui.join("dev/mock-server.mjs"), ui.join("app.js")).unwrap();
        symlink(ui.join(".env"), ui.join("settings.json")).unwrap();
        symlink(ui.join("README.md"), ui.join("notes.txt")).unwrap();
        symlink(ui.join("dev"), ui.join("lib")).unwrap();
        symlink(ui.join("index.html"), ui.join("home.html")).unwrap();
        let statics = Statics::Dir(ui.canonicalize().unwrap());
        for path in [
            "/app.js",
            "/settings.json",
            "/notes.txt",
            "/lib/mock-server.mjs",
        ] {
            assert!(lookup(&statics, path).is_none(), "{path}");
        }
        assert_eq!(lookup(&statics, "/home.html").unwrap().1, b"<p>ok</p>");
    }
}
