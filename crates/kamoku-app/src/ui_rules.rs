// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 画面のファイルとして配ってよいものの決まりと、拡張子ごとの種類。
// build.rs からも include! で読むので、依存を持たない関数だけを置き、内側の属性や `//!` は書かない。
// 埋め込み（build.rs）とディスクから読む `--ui-dir` で、同じ決まりを使うため。

/// 中へ降りてよいフォルダか。試験・開発の道具・依存の置き場所・隠しフォルダは配らない。
pub(crate) fn ui_dir_allowed(name: &str) -> bool {
    !(name.is_empty() || name.starts_with('.') || matches!(name, "tests" | "dev" | "node_modules"))
}

/// 配ってよいファイルか（`ui/` からの相対パス。区切りは `/`）。
/// `..` や空の区切り、隠しファイル、README.md と package.json は断る。
pub(crate) fn ui_path_allowed(rel: &str) -> bool {
    let mut parts: Vec<&str> = rel.split('/').collect();
    let Some(file) = parts.pop() else {
        return false;
    };
    if file.is_empty() || file.starts_with('.') || matches!(file, "README.md" | "package.json") {
        return false;
    }
    parts.iter().all(|dir| *dir != ".." && ui_dir_allowed(dir))
}

/// 拡張子ごとの `Content-Type`。知らない拡張子は、ブラウザに解釈させない種類にする。
pub(crate) fn content_type(rel: &str) -> &'static str {
    let ext = rel
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "json" => "application/json; charset=utf-8",
        "webmanifest" => "application/manifest+json",
        "txt" => "text/plain; charset=utf-8",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    }
}
