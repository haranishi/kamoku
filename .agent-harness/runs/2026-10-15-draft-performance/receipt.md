# Pending draft query performance receipt

## Scope and implementation

Base: `0cd48f38895a87a5a36b6e8823f29f1147c103bb`.
Contract: [performance and Rust UI](../../contracts/kamoku-performance-rust-ui-v1.md).
Replace repeated draft pagination in dashboard/selected confirmation with one
integer-column scan and prepared primary-key lookups. No new runtime dependency,
schema change, durability change, automatic classification, or tax-rule change.

## Local evidence

- Implementer: targeted rustfmt, offline store test-target cargo check, and
  clippy with warnings denied passed. No newly linked application was run locally.
- New store cases: mixed direction/status, empty/duplicate/missing IDs, 2,048
  unrelated rows, exact safe-integer boundary, overflow classification, and
  status changes before transactional confirmation. App cases preserve mixed
  result order/dedup and the existing 422 message. Execution is assigned to CI.
- Links (91 documents), independent golden hashes (7), strict trace references
  (78 conditions, 6 manual), and workflow regressions (30) passed.
- Benchmark tool: actual cached debug binary with 10k rows completed; bounded
  100k timeout was recorded with no successful samples; require-complete failed
  as intended. Wrong HTTP-200 amounts and malformed JSON were rejected. Server
  cleanup and authentication-free JSON were checked. These are tool checks,
  not a release comparison or evidence for the new app binary.
- Japanese documentation lint retained one rhythm heuristic per edited guide;
  the technical descriptions and measurement limitations are intentional.

## Independent review and fix

Read-only backend review used the separate Standards and Spec passes. One
Standards finding: B5 mutation still targeted the old dashboard amount guard.
Its file/search string now targets the store guard; exact-match count is one.
Spec/runtime review found no additional issue. Parent separately reviewed the
benchmark, workflow, cleanup, integer semantics, and documentation.

## Merge gates and remote evidence

The PR checks must pass for the final head before merging: full Linux/macOS Rust
tests/lints/docs, MSRV 1.88, dependency policy, REUSE, docs/trace, UI checks,
real-server smoke/flows/contract/screens, secrets, and release performance.
The performance workflow builds base/head on one runner, checks 10k/100k rows,
and runs the retained B5 guard mutation. Machine check records and the PR's
validation section hold the final run URLs and actual timing results; they are
not claimed as passed by this pre-CI local receipt.

## Limits

External SSD unavailable. Full new builds run in CI; cached local binaries may
predate current dependencies. Synthetic measurements do not establish all-OS
SLOs, real-accounting performance, concurrent-user gains, or UI migration speed.
