# Complete Rust UI migration: verification record

Runtime source: `c857db305b22679027403a4a4c8a111a7ff09df3`.
Baseline: `9ad343060b821b9d9789c10766664c8689893120`.
PR: <https://github.com/haranishi/kamoku/pull/9>.
Follow-up changes affect the measurement harness and records only.

## Scope

Every default screen, shared input/state/routing/API/DOM operation is authored
in Rust/WASM. Existing HTML/CSS, hash routes and native browser controls remain.
Legacy business/UI JS is removed from both embedded and disk-served allowlists.
Only minimal loaders and generated wasm-bindgen adapters execute as JS.
Accounting, tax, persistence, authentication and hash-chain rules are unchanged.

## Mechanical evidence

- Local `tools/check-all.sh` full mode: exit 0, including Rust 1.88 native/WASM,
  strict clippy, tests, rustdoc, dependency policy, static UI and document checks.
- Pure Rust UI: 120 tests. Workflow/asset regressions: 42 tests.
- Workflow mutations: all five deliberately broken cases detected, no survivors.
- Runtime CI: [38771073204](https://github.com/haranishi/kamoku/actions/runs/38771073204),
  all ten jobs successful, including Linux/macOS, MSRV and real-server browser.
- The CI browser job runs smoke, 453 flows, 25 API contracts, 150 screenshots
  and seven Rust compatibility-preview cases against embedded production assets.
- Local flows: 452 succeeded; one browser-context setup timed out before the
  application ran. The identical case then passed three times without a source
  change. CI's complete same-source suite passed. Its cause remains unconfirmed.
- The earlier five application/DOM failures passed their eight targeted checks;
  the original multi-step dirty/history scenario passed three consecutive times.
- Public-source secret scan: 522 files, no findings. CI history scan passed.
- Independent source review, final pass: Standards 0 / Spec 0.

## Findings corrected during integration

- Verify mandatory WASM assets against the actual collected embedding manifest;
  reject missing files and symlink-only file/directory fixtures.
- Preserve combobox selection/caret behavior, same-route borrow lifetimes,
  drawer return history, latest-response rows and unchanged server field errors.
- Resolve modal decisions before closing removes their accessible role; retain
  delayed close-listener disposal and return notification portals on cleanup.
- Restore notification focus/hover lifetime, stable placement, dynamic bar
  observation and teardown; test the fixture under the actual CSP.
- Start inbox prerequisites concurrently and reveal the initial confirmation
  bar only after its list is rendered. Preserve submitted save/Undo operations.
- Restore narrow table/card classes, sidebar labels and nearest-field scrolling.

## Performance and limits

[Raw UI measurements](ui-performance.json) and [build provenance](builds.json)
compare independent release binaries with the same toolchain and flags.
See [method, numbers and payload tradeoff](../../../docs/performance-ui.md).
The measurement runner accepts both legacy `data-key` and Rust `data-entry-id`
and still requires all 50 journal IDs in API order. The initial CI measurement
failure was a runner selector mismatch; production readiness was not weakened.

The journal benchmark also passed in
[38771073171](https://github.com/haranishi/kamoku/actions/runs/38771073171).
Before merge, require every check on the final PR head, including the repaired
UI benchmark. Do not substitute an earlier green head for that final check.

Real OS IME conversion, assistive technology, Safari/Firefox and Windows remain
unverified. Synthetic Chromium success does not establish those platform results.
No books, credentials, server logs, traces or videos are published as evidence.
