// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 帳簿の置き場所、デモの一時フォルダ、バックアップの書き出し先。

use crate::ids;
use kamoku_store::{Store, StoreError};
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// デモのフォルダに置く目印のファイル（kamoku が作ったデモのフォルダだという印。消す処理は無い）。
pub(crate) const DEMO_MARKER: &str = ".kamoku-demo";
/// 帳簿のファイルの名前。
pub(crate) const DB_FILE: &str = "kamoku.db";

/// 帳簿の置き場所。`--data` > 環境変数 `KAMOKU_DATA` > OSごとの既定の場所。
pub(crate) fn data_dir(flag: Option<&Path>) -> Result<PathBuf, String> {
    if let Some(dir) = flag {
        return Ok(dir.to_path_buf());
    }
    if let Some(dir) = env::var_os("KAMOKU_DATA").filter(|v| !v.is_empty()) {
        return Ok(PathBuf::from(dir));
    }
    default_data_dir().ok_or_else(|| {
        "帳簿の既定の置き場所を決められません（ホームのフォルダが分かりません）。--data で指定してください。"
            .to_owned()
    })
}

fn non_empty_var(name: &str) -> Option<PathBuf> {
    env::var_os(name)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

#[cfg(target_os = "macos")]
fn default_data_dir() -> Option<PathBuf> {
    non_empty_var("HOME").map(|home| {
        home.join("Library")
            .join("Application Support")
            .join("kamoku")
    })
}

#[cfg(windows)]
fn default_data_dir() -> Option<PathBuf> {
    non_empty_var("APPDATA").map(|dir| dir.join("kamoku"))
}

#[cfg(all(not(target_os = "macos"), not(windows)))]
fn default_data_dir() -> Option<PathBuf> {
    non_empty_var("XDG_DATA_HOME")
        .or_else(|| non_empty_var("HOME").map(|home| home.join(".local").join("share")))
        .map(|dir| dir.join("kamoku"))
}

/// 自分だけが使える（0700）新しいフォルダを、`parent` の中に作ります。名前は乱数で決める。
pub(crate) fn private_dir(parent: &Path, prefix: &str) -> io::Result<PathBuf> {
    for _ in 0..8 {
        let name = ids::random_hex(8).map_err(io::Error::other)?;
        let dir = parent.join(format!("{prefix}-{name}"));
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        match builder.create(&dir) {
            Ok(()) => return Ok(dir),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::from(io::ErrorKind::AlreadyExists))
}

/// デモの帳簿を置く新しいフォルダ（`一時フォルダ/kamoku-demo-<乱数>`、0700）を作ります。
/// すでにあるフォルダは、消さず、中も見ない。同時に動く別の demo の帳簿や、リンクの先を壊さないため
/// （名前が重なれば別の乱数で作り直す）。作ったフォルダは、終わっても消さない（OSの一時フォルダの
/// 掃除に任せる）。目印のファイルは、kamoku が作ったデモのフォルダだと人が見て分かるように置く。
pub(crate) fn prepare_demo_dir() -> Result<PathBuf, String> {
    let temp = env::temp_dir();
    let dir = private_dir(&temp, "kamoku-demo").map_err(|e| {
        format!(
            "デモのフォルダを {} の中に作れませんでした: {e}",
            temp.display()
        )
    })?;
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dir.join(DEMO_MARKER))
        .map_err(|e| format!("目印のファイルを作れませんでした: {e}"))?;
    Ok(dir)
}

/// バックアップを、まだ無いファイル `out` に書き出します。既にあれば断る（上書きしない）。
/// 一貫した複製（`VACUUM INTO`）、0600 の権限、完成品だけを公開することは、保存層の `backup_to` が受け持つ。
pub(crate) fn backup_to_new_file(store: &Store, out: &Path) -> Result<(), String> {
    match store.backup_to(out) {
        Ok(()) => Ok(()),
        Err(StoreError::BackupExists) => Err(format!(
            "{} はすでにあります。上書きしないので、別の名前を指定してください。",
            out.display()
        )),
        Err(e) => Err(format!("{} に書き出せませんでした: {e}", out.display())),
    }
}

/// バックアップを作って、中身を返します（`/export/backup.db`）。一時ファイルは返す前に消す。
pub(crate) fn backup_bytes(store: &Store) -> Result<Vec<u8>, String> {
    let temp = private_dir(&env::temp_dir(), "kamoku-export")
        .map_err(|e| format!("一時フォルダを作れませんでした: {e}"))?;
    let staged = temp.join(DB_FILE);
    let result = store
        .backup_to(&staged)
        .map_err(|e| e.to_string())
        .and_then(|()| fs::read(&staged).map_err(|e| e.to_string()));
    let _ = fs::remove_file(&staged);
    let _ = fs::remove_dir(&temp);
    result
}
