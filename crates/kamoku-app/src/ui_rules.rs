// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 画面のファイルとして配ってよいものの決まりと、拡張子ごとの種類。
// build.rs からも include! で読むので、依存を持たない関数だけを置き、内側の属性や `//!` は書かない。
// 埋め込み（build.rs）とディスクから読む `--ui-dir` で、同じ決まりを使うため。

/// 配信するアセットのフォルダだけをたどる。
pub(crate) fn ui_dir_allowed(name: &str) -> bool {
    matches!(name, "css" | "rust" | "pkg")
}

/// 配ってよいファイルか（`ui/` からの相対パス。区切りは `/`）。
/// Rustの生成物、最小のローダー、HTML、CSS、アイコンだけを配る。
#[allow(
    clippy::case_sensitive_file_extension_comparisons,
    reason = "The asset allowlist deliberately requires the exact lowercase CSS extension"
)]
pub(crate) fn ui_path_allowed(rel: &str) -> bool {
    if matches!(
        rel,
        "index.html"
            | "favicon.svg"
            | "boot.js"
            | "rust/index.html"
            | "rust/boot.js"
            | "rust/preview.css"
            | "rust/pkg/kamoku_ui.js"
            | "rust/pkg/kamoku_ui_bg.wasm"
    ) {
        return true;
    }
    rel.split('/')
        .all(|part| !part.is_empty() && !part.starts_with('.'))
        && rel.split('/').next().is_some_and(ui_dir_allowed)
        && rel.strip_prefix("css/").is_some_and(|file| {
            file.ends_with(".css") && !file.starts_with('.') && !file.contains('/')
        })
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
        "wasm" => "application/wasm",
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
