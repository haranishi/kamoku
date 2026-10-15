// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Synthetic journal HTTP benchmark; copy this file and `bench_journal/` unchanged
//! into base/head checkouts, then build both with identical release flags.
//! `cargo run --release -p kamoku-app --example bench_journal -- --entries 10000
//! --revisions 5 --samples 10 --output journal.json`
//! Fixture creation, startup, reference construction, and validation are untimed.

#[path = "bench_journal/fixture.rs"]
mod fixture;

use fixture::{Case, Fixture};
use kamoku_app::server::{self, ServeOptions, Server};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::error::Error;
use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::time::{Duration, Instant};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const WARMUPS: usize = 1;
const TIMEOUT: Duration = Duration::from_secs(30);

struct Args {
    entries: u32,
    revisions: u32,
    samples: usize,
    output: PathBuf,
}

impl Args {
    fn read() -> Result<Self> {
        let (mut entries, mut revisions, mut samples, mut output) = (10_000, 0, 10, None);
        let mut args = std::env::args().skip(1);
        while let Some(key) = args.next() {
            let value = args.next().ok_or("each option needs a value")?;
            match key.as_str() {
                "--entries" => entries = value.parse()?,
                "--revisions" => revisions = value.parse()?,
                "--samples" => samples = value.parse()?,
                "--output" => output = Some(PathBuf::from(value)),
                _ => return Err("unknown benchmark option".into()),
            }
        }
        if !matches!(entries, 10_000 | 100_000) || revisions > 20 || !(1..=1_000).contains(&samples)
        {
            return Err("entries must be 10000/100000, revisions 0..20, samples 1..1000".into());
        }
        Ok(Self {
            entries,
            revisions,
            samples,
            output: output.ok_or("--output is required")?,
        })
    }
}

struct ServerGuard(Option<Server>);

impl Drop for ServerGuard {
    fn drop(&mut self) {
        if let Some(server) = self.0.take() {
            server.request_stop();
            let addr = SocketAddr::from(([127, 0, 0, 1], server.port()));
            let _ = TcpStream::connect_timeout(&addr, Duration::from_secs(1));
            server.join();
        }
    }
}

fn request(port: u16, path: &str, cookie: Option<&str>) -> Result<Vec<u8>> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let mut stream = TcpStream::connect_timeout(&addr, TIMEOUT)?;
    stream.set_read_timeout(Some(TIMEOUT))?;
    stream.set_write_timeout(Some(TIMEOUT))?;
    let cookie = cookie.map_or_else(String::new, |value| format!("Cookie: {value}\r\n"));
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n{cookie}Connection: close\r\n\r\n"
    )?;
    let mut response = Vec::new();
    stream.read_to_end(&mut response)?;
    Ok(response)
}

fn response_parts(bytes: &[u8], status: u16) -> Result<(&str, &str)> {
    let text = std::str::from_utf8(bytes)?;
    let (headers, body) = text
        .split_once("\r\n\r\n")
        .ok_or("incomplete HTTP response")?;
    let received = headers
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|number| number.parse::<u16>().ok());
    if received != Some(status) {
        // Never include a raw response, URL, cookie, or header in an error.
        return Err("unexpected HTTP status".into());
    }
    Ok((headers, body))
}

fn body_json(bytes: &[u8]) -> Result<Value> {
    Ok(serde_json::from_str(response_parts(bytes, 200)?.1)?)
}

fn authenticate(port: u16, token: &str) -> Result<String> {
    let login = request(port, &format!("/?t={token}"), None)?;
    let (headers, _) = response_parts(&login, 303)?;
    headers
        .lines()
        .find_map(|line| line.strip_prefix("Set-Cookie: "))
        .and_then(|line| line.split(';').next())
        .filter(|value| value.contains('='))
        .map(str::to_owned)
        .ok_or_else(|| "authentication cookie missing".into())
}

fn measure(port: u16, cookie: &str, case: &Case, samples: usize) -> Result<Value> {
    let path = format!("/api/v1/entries?{}", case.query);
    let mut raw = Vec::with_capacity(samples);
    for index in 0..WARMUPS + samples {
        let started = Instant::now();
        let response = request(port, &path, Some(cookie))?;
        let ms = started.elapsed().as_secs_f64() * 1_000.0;
        // Decode and compare the complete result outside the latency window.
        if body_json(&response)? != case.expected {
            return Err("journal result disagreed with the synthetic full-sort reference".into());
        }
        if index >= WARMUPS {
            raw.push(ms);
        }
    }
    let mut sorted = raw.clone();
    sorted.sort_by(f64::total_cmp);
    let middle = sorted.len() / 2;
    let p50 = if sorted.len().is_multiple_of(2) {
        f64::midpoint(sorted[middle - 1], sorted[middle])
    } else {
        sorted[middle]
    };
    Ok(json!({
        "query": case.query, "sample_count": raw.len(), "raw_ms": raw,
        "p50_ms": p50, "p95_ms": sorted[(sorted.len() * 95).div_ceil(100) - 1],
        "max_ms": sorted.last(), "expected_total": case.expected["total"],
        "expected_sum_amount": case.expected["sum_amount"],
        "expected_page_items": case.expected["items"].as_array().map(Vec::len),
    }))
}

fn binary_sha256() -> Result<String> {
    let mut file = File::open(std::env::current_exe()?)?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 8_192];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            let digest = hash.finalize();
            let mut hex = String::with_capacity(64);
            for byte in &digest {
                write!(hex, "{byte:02x}")?;
            }
            return Ok(hex);
        }
        hash.update(&buffer[..count]);
    }
}

fn run(args: &Args) -> Result<Value> {
    let dir = tempfile::tempdir()?;
    let db = dir.path().join("synthetic.sqlite");
    let fixture = Fixture::build(&db, args.entries, args.revisions)?;
    let cases = fixture.cases(args.entries)?;
    let mut options = ServeOptions::new(db);
    options.port = 0;
    let server = server::start(&options)?;
    let (port, token, read_only) = (
        server.port(),
        server.token().to_owned(),
        server.read_only().is_some(),
    );
    let _server = ServerGuard(Some(server));
    if read_only {
        return Err("synthetic fixture failed ledger verification".into());
    }
    let cookie = authenticate(port, &token)?;
    let status = body_json(&request(port, "/api/v1/status", Some(&cookie))?)?;
    if status["ledger"]["verified"] != true
        || status["ledger"]["events"] != fixture.events
        || status["ledger"]["entries"] != args.entries
    {
        return Err("unexpected synthetic fixture counts".into());
    }
    let mut measured = serde_json::Map::new();
    for case in &cases {
        measured.insert(
            case.name.to_owned(),
            measure(port, &cookie, case, args.samples)?,
        );
    }
    let revision = std::env::var("KAMOKU_BENCH_REVISION").ok().filter(|value| {
        matches!(value.len(), 40 | 64) && value.bytes().all(|b| b.is_ascii_hexdigit())
    });
    Ok(json!({
        "schema": 1, "complete": true,
        "provenance": { "source_revision": revision, "binary_sha256": binary_sha256()?,
            "os": std::env::consts::OS, "arch": std::env::consts::ARCH,
            "profile": if cfg!(debug_assertions) { "debug" } else { "release" } },
        "fixture": { "version": 1, "entries": args.entries,
            "revisions_per_tenth_entry": args.revisions, "revised_entries": fixture.revised,
            "voided_entries": fixture.voided, "active_entries": args.entries - fixture.voided,
            "events": fixture.events, "batch_size": 500 },
        "measurement": { "samples": args.samples, "warmups": WARMUPS, "units": "ms",
            "p50": "median", "p95": "nearest rank", "request_timeout_seconds": TIMEOUT.as_secs(),
            "scope": "new loopback TCP connection through complete response body; JSON parsing and full-result validation excluded",
            "cache": "one warmup per case; server and OS warm; fixture/startup/reference construction excluded" },
        "cases": measured,
    }))
}

fn main() {
    let Ok(args) = Args::read() else {
        eprintln!(
            "usage: bench_journal --entries 10000|100000 --revisions 0..20 --samples 1..1000 --output FILE"
        );
        std::process::exit(2);
    };
    let result = run(&args);
    let report = result.as_ref().map_or_else(
        |_| json!({ "schema": 1, "complete": false, "error": "benchmark failed; private error details withheld" }),
        Clone::clone,
    );
    let saved = serde_json::to_vec_pretty(&report)
        .map_err(|_| ())
        .and_then(|bytes| fs::write(&args.output, bytes).map_err(|_| ()));
    if result.is_err() || saved.is_err() {
        eprintln!("journal benchmark failed; authentication URLs and HTTP responses were withheld");
        std::process::exit(1);
    }
    println!("{report}");
}
