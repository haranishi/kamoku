// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 画面のファイル（リポジトリの `ui/`）を実行ファイルに埋め込むための一覧を、`OUT_DIR` に作る。
//! 配らないもの（試験・開発の道具・依存の置き場所・README.md・package.json・隠しファイル）は
//! `src/ui_rules.rs` の決まりで除く。

use std::error::Error;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

include!("src/ui_rules.rs");

fn main() -> Result<(), Box<dyn Error>> {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let ui = manifest.join("..").join("..").join("ui");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/ui_rules.rs");
    // フォルダを指すと、cargo は中のファイルの変更まで見て作り直す。
    println!("cargo:rerun-if-changed={}", ui.display());
    let mut files = Vec::new();
    if ui.is_dir() {
        collect(&ui, &ui, &mut files)?;
    } else {
        println!("cargo:warning=ui/ が見つからないため、画面のファイルを埋め込みません");
    }
    files.sort();
    let mut out = String::from(
        "// build.rs が作る。手で直さない。\n\
         /// 埋め込んだ画面のファイル（`ui/` からの相対パス、種類、中身）。\n\
         pub(crate) static UI_FILES: &[(&str, &str, &[u8])] = &[\n",
    );
    for (rel, abs) in &files {
        writeln!(
            out,
            "    ({rel:?}, {:?}, include_bytes!({:?})),",
            content_type(rel),
            abs.display().to_string()
        )?;
    }
    out.push_str("];\n");
    let dest = PathBuf::from(std::env::var("OUT_DIR")?).join("ui_files.rs");
    std::fs::write(dest, out)?;
    Ok(())
}

fn collect(
    root: &Path,
    dir: &Path,
    out: &mut Vec<(String, PathBuf)>,
) -> Result<(), Box<dyn Error>> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        // シンボリックリンクはたどらない。ui/ の外のファイルを埋め込まないため。
        let kind = entry.file_type()?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if kind.is_dir() {
            if ui_dir_allowed(&name) {
                collect(root, &path, out)?;
            }
            continue;
        }
        if !kind.is_file() {
            continue;
        }
        let rel: Vec<String> = path
            .strip_prefix(root)?
            .components()
            .filter_map(|c| c.as_os_str().to_str().map(str::to_owned))
            .collect();
        let rel = rel.join("/");
        if ui_path_allowed(&rel) {
            out.push((rel, path));
        }
    }
    Ok(())
}
