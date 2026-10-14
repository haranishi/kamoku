// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! サーバーの起動。保存先を開き、`127.0.0.1` で待ち受け、合言葉を作って受け口を動かす。

use crate::api::{ClientIds, Uploads};
use crate::app::App;
use crate::http::{self, Limits, Running};
use crate::state::State;
use crate::statics::Statics;
use crate::{clock, ids};
use kamoku_books::{Kind, Wallet, WalletType};
use kamoku_store::{NewAccount, OpenOptions, Store, StoreError};
use std::fmt;
use std::io;
use std::net::{Ipv4Addr, TcpListener};
use std::path::PathBuf;
use std::sync::Arc;

/// 動かし方。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// 本物の帳簿。
    Normal,
    /// 合成データの帳簿（画面の上に「デモのデータです」と出す）。
    Demo,
}
impl Mode {
    /// `/status` の `mode` に入れる名前。
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Demo => "demo",
        }
    }
}

/// 起動の指定。
#[derive(Debug, Clone)]
pub struct ServeOptions {
    /// DBファイルの場所（無ければ作る）。
    pub db_path: PathBuf,
    /// 動かし方。
    pub mode: Mode,
    /// 待ち受けの番号。0なら、OSが空いている番号を選ぶ。
    pub port: u16,
    /// 番号が使用中のとき、次の番号を試す回数（最初の1回を含む）。
    pub port_attempts: u16,
    /// 画面のファイルを読むフォルダ（`--ui-dir`）。`None` なら実行ファイルに埋め込んだもの。
    pub ui_dir: Option<PathBuf>,
    /// 受け口の上限と時間切れ。
    pub limits: Limits,
}
impl ServeOptions {
    /// 既定の指定（通常の帳簿、番号8765から20回、埋め込みの画面）。
    #[must_use]
    pub fn new(db_path: PathBuf) -> Self {
        Self {
            db_path,
            mode: Mode::Normal,
            port: 8765,
            port_attempts: 20,
            ui_dir: None,
            limits: Limits::default(),
        }
    }
}

/// 起動できなかった理由。
#[derive(Debug)]
pub enum StartError {
    /// 帳簿を開けない。
    Open(String),
    /// 待ち受けを作れない。
    Bind(io::Error),
    /// `--ui-dir` のフォルダが無い。
    UiDir(io::Error),
    /// 合言葉の乱数を作れない。
    Random(String),
    /// スレッドを作れない。
    Spawn(io::Error),
}
impl fmt::Display for StartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Open(e) => write!(f, "帳簿を開けませんでした: {e}"),
            Self::Bind(e) => write!(
                f,
                "待ち受けを作れませんでした（番号が使用中かもしれません）: {e}"
            ),
            Self::UiDir(e) => write!(f, "--ui-dir のフォルダを開けませんでした: {e}"),
            Self::Random(e) => write!(f, "合言葉の乱数を作れませんでした: {e}"),
            Self::Spawn(e) => write!(f, "処理のスレッドを作れませんでした: {e}"),
        }
    }
}
impl std::error::Error for StartError {}

/// 動いているサーバー。
#[derive(Debug)]
pub struct Server {
    running: Running,
    token: String,
    read_only: Option<String>,
}
impl Server {
    /// 待ち受けの番号。
    #[must_use]
    pub const fn port(&self) -> u16 {
        self.running.addr().port()
    }
    /// 起動ごとの合言葉（16進32桁）。
    #[must_use]
    pub fn token(&self) -> &str {
        &self.token
    }
    /// ブラウザで開くアドレス（合言葉つき）。
    #[must_use]
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/?t={}", self.port(), self.token)
    }
    /// 読み取り専用で起動したときの理由（最初の不正）。
    #[must_use]
    pub fn read_only(&self) -> Option<&str> {
        self.read_only.as_deref()
    }
    /// 止める合図を出します（[`Running::request_stop`]）。
    pub fn request_stop(&self) {
        self.running.request_stop();
    }
    /// 止まるまで待ちます。通常の起動では、止める合図が無い限り戻りません。
    pub fn join(self) {
        self.running.join();
    }
}

/// 保存先を開いて、サーバーを起動します。
///
/// # Errors
/// 帳簿を開けない、待ち受けを作れない、乱数やスレッドを作れないとき。
pub fn start(options: &ServeOptions) -> Result<Server, StartError> {
    let (mut store, report) = Store::open(
        &options.db_path,
        OpenOptions {
            create_if_missing: true,
        },
    )
    .map_err(|e| StartError::Open(e.to_string()))?;
    ensure_cash_wallet(&mut store).map_err(|e| StartError::Open(e.to_string()))?;
    let read_only = store.read_only().map(|f| {
        format!(
            "連番{}で不正（{}{}）",
            f.seq,
            f.kind,
            f.rule
                .as_deref()
                .map(|r| format!("、{r}"))
                .unwrap_or_default()
        )
    });
    let statics = match &options.ui_dir {
        Some(dir) => Statics::Dir(dir.canonicalize().map_err(StartError::UiDir)?),
        None => Statics::Embedded,
    };
    let listener = bind(options.port, options.port_attempts).map_err(StartError::Bind)?;
    let port = listener.local_addr().map_err(StartError::Bind)?.port();
    let token = ids::random_hex(16).map_err(|e| StartError::Random(e.to_string()))?;
    let state = State {
        store,
        mode: options.mode,
        events_in_db: report.events_in_db,
        clients: ClientIds::default(),
        uploads: Uploads::default(),
    };
    let app = Arc::new(App::new(port, token.clone(), statics, state));
    let running = http::start(listener, &options.limits, app).map_err(StartError::Spawn)?;
    Ok(Server {
        running,
        token,
        read_only,
    })
}

// 初期設定の画面（SC-00）は「現金は最初から1つ」。保存層は口座を勝手に足さないので、ここで足す。
pub(crate) fn ensure_cash_wallet(store: &mut Store) -> Result<(), StoreError> {
    if store.read_only().is_some()
        || store.settings().setup_done
        || store.chart().wallets().next().is_some()
    {
        return Ok(());
    }
    store
        .add_account(
            NewAccount {
                name: "現金".to_owned(),
                reading: "げんきん".to_owned(),
                kind: Kind::Asset,
                parent: None,
                wallet: Some(Wallet {
                    kind: WalletType::Cash,
                    institution: String::new(),
                }),
                default_tax: None,
                default_pct: 100,
            },
            clock::now_secs(),
        )
        .map(|_| ())
}

fn bind(port: u16, attempts: u16) -> io::Result<TcpListener> {
    if port == 0 {
        return TcpListener::bind((Ipv4Addr::LOCALHOST, 0));
    }
    let mut last = None;
    for offset in 0..attempts.max(1) {
        let Some(candidate) = port.checked_add(offset) else {
            break;
        };
        match TcpListener::bind((Ipv4Addr::LOCALHOST, candidate)) {
            Ok(listener) => return Ok(listener),
            Err(e) if e.kind() == io::ErrorKind::AddrInUse => last = Some(e),
            Err(e) => return Err(e),
        }
    }
    Err(last.unwrap_or_else(|| io::Error::from(io::ErrorKind::AddrInUse)))
}
