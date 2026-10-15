# Complete authored-Rust UI migration

## Goal and authority

The maintainer explicitly requests the complete Rust UI migration, harness and
skills, and PR creation through merge. The main agent owns integration and Git
operations. Implementation agents may edit only their assigned modules; a
separate read-only review is required before merge. Maximum three fix rounds;
return an unresolved repeated finding after two rounds.

Baseline: main `9ad343060b821b9d9789c10766664c8689893120`.
Migrate every existing production screen and shared UI behavior to Rust/WASM,
then make Rust the default application. Keep browser-native input and semantic
DOM, existing hash URLs, CSS, accessibility, API and persistence contracts.
Generated wasm-bindgen JS, a minimal loader, and JS development/test tools are
allowed. Authored production business/UI JS, embedded old JS, eval, and a JS
interpreter masquerading as a Rust migration are not allowed.

## User story and UX decisions

A bookkeeper sets up a book, records/corrects/voids transactions, imports and
confirms statements, maintains masters, and reads/exports reports without
losing existing keyboard, Japanese input, error recovery or saved records.

- Strategy: fast, predictable local bookkeeping; language alone proves no speed.
- Requirements: complete current functionality, exact integer yen, read-only
  enforcement, dirty guards, history/focus, safe text and same-origin requests.
- Structure: preserve all current routes, navigation and primary task sequences.
- Skeleton: preserve the existing labelled fields, tables, dialogs and CSS.
- Surface: reuse tokens/themes/density and 390/768/1440 responsive behavior;
  do not redesign or copy another accounting product during the migration.

## Boundaries

Allowed: Rust UI crate/common components/screens, app asset embedding/allowlist,
WASM build/runner/CI, equivalent behavioral tests, documentation and rights
records. No accounting/tax/schema/durability/authentication changes or new
external service. Use synthetic data only; never upload auth URLs, logs, trace,
video or books. Keep heavy builds/caches on the restored external SSD and use
separate baseline/head release targets for performance comparisons.

## Acceptance and verification

- All existing production routes and task flows work through the default URL.
- Production assets no longer serve authored legacy JS business/UI modules.
- Build real WASM before embedding and reject missing generated default assets.
- Keep the existing 447 behavioral flows, smoke, contract and screenshot suites;
  replace direct JS-module probes with equivalent Rust UI behavior checks.
- Port relevant pure UI logic tests into Rust; do not drop behavioral coverage
  to obtain a green result. Keep strict specification traceability meaningful.
- Run formatting, native/WASM Rust tests/checks/clippy, Rust 1.88, dependency
  policy, REUSE, links, secrets and all real-server browser suites in CI.
- Run equivalent synthetic before/after performance measurements with limits.
- Inspect actual images at three widths and independently review source and
  task interaction evidence. Record actual platform gaps without calling them
  tested. Merge only the final tested source tree; update resumption notes.
