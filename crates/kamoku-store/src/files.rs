// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{Store, StoreError};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

#[cfg(unix)]
fn mode(path: &Path, bits: u32) -> Result<(), StoreError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(bits))?;
    Ok(())
}
#[cfg(not(unix))]
fn mode(_path: &Path, _bits: u32) -> Result<(), StoreError> {
    Ok(())
}
fn parent(path: &Path) -> &Path {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}
pub(crate) fn prepare(path: &Path, create: bool) -> Result<(), StoreError> {
    if !path.exists() && !create {
        return Err(StoreError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "DBがありません",
        )));
    }
    let directory = parent(path);
    if create {
        #[cfg(unix)]
        {
            use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
            fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(directory)?;
            if !path.exists() {
                match fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(path)
                {
                    Ok(_) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(e) => return Err(e.into()),
                }
            }
        }
        #[cfg(not(unix))]
        {
            fs::create_dir_all(directory)?;
            if !path.exists() {
                fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(path)?;
            }
        }
    }
    secure_db(path)
}
pub(crate) fn secure_db(path: &Path) -> Result<(), StoreError> {
    mode(path, 0o600)?;
    for suffix in ["-wal", "-shm"] {
        let mut name = path.as_os_str().to_os_string();
        name.push(suffix);
        let sidecar = PathBuf::from(name);
        if sidecar.exists() {
            mode(&sidecar, 0o600)?;
        }
    }
    Ok(())
}
static BACKUP_ID: AtomicU64 = AtomicU64::new(0);
impl Store {
    /// 一貫した複製を同じフォルダの一時ファイルへ作り、完成後だけ公開します。
    /// 既存ファイルを上書きしません。台帳が壊れていても証拠の複製は作れます。
    ///
    /// # Errors
    /// 既存の宛先、ファイル権限、VACUUMまたはファイル操作の失敗。
    pub fn backup_to(&self, path: impl AsRef<Path>) -> Result<(), StoreError> {
        let path = path.as_ref();
        if path.exists() {
            return Err(StoreError::BackupExists);
        }
        let directory = parent(path);
        // 専用ディレクトリを排他的に確保するため、予測可能な名前でも既存の内容に書かない。
        let temp = loop {
            let n = BACKUP_ID.fetch_add(1, Ordering::Relaxed);
            let temp = directory.join(format!(".kamoku-backup-{}-{n}", std::process::id()));
            let mut builder = fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            match builder.create(&temp) {
                Ok(()) => break temp,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e.into()),
            }
        };
        let file = temp.join("backup.sqlite");
        let result = (|| {
            // VACUUM INTOは空の既存ファイルも受け付ける。書いている間も0600を守る。
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(&file)?;
            }
            #[cfg(not(unix))]
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&file)?;
            let name = file
                .to_str()
                .ok_or_else(|| StoreError::Invalid("複製先はUTF-8のパスにしてください".into()))?;
            self.conn.execute("VACUUM INTO ?", [name])?;
            mode(&file, 0o600)?;
            fs::File::open(&file)?.sync_all()?;
            // renameは既存の宛先を置き換えるため、同じFSの排他的なhard_linkで完成品を公開する。
            match fs::hard_link(&file, path) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    Err(StoreError::BackupExists)
                }
                Err(e) => Err(e.into()),
            }
        })();
        if file.exists() {
            fs::remove_file(&file)?;
        }
        fs::remove_dir(&temp)?;
        result
    }
}
