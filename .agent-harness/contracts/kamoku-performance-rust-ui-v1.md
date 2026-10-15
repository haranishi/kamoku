# Performance and Rust UI contract

## Goal and boundaries

Make latency a measured product property and move authored UI logic to Rust in
verified increments. Rust alone is not a performance result. Keep accounting
semantics, history, persistence durability, and existing supported workflows.

First remove repeated draft-page scans, then improve the journal query path and
build an isolated Rust/WASM journal UI.
The browser remains the renderer for the first slice so native input controls,
IME, semantic tables, and browser tests can be reused. Generated JS bindings are
expected; handwritten business/UI logic moves to Rust. The existing UI remains
available until functional, visual, and performance parity is demonstrated.

## First PR: pending draft queries

- Replace repeated 500-row scans with one bounded-memory amount query for the
  dashboard, and direct selected-ID lookup for confirmation. Retain integer
  overflow errors, status rules, order, transaction revalidation, and durability.
- Check mixed statuses, zero/empty input, duplicate/missing IDs, amount limits,
  and unrelated draft rows. Compare synthetic before/after on the same runner.

## Journal query follow-up

- Maintain ordered, derived entry-state references in the ledger instead of
  rebuilding the same history map for every listing. Do not change event bytes,
  hashes, persistence schema, or failure/rollback behavior.
- Select and sort only the requested page after filtering; retain exact totals,
  sums, dates, state filters, ordering, offset handling, and overflow errors.
- Compare public results with a simple reference on revised/voided/interleaved
  histories and page boundaries. Preserve fixed-hash and corruption tests.
- Benchmark identical synthetic data and build settings before/after, including
  10k/100k entries, empty search results, late pages, and revision-heavy histories.

## Rust UI slice and UX

Story: a bookkeeper opens the journal, searches a transaction, moves through
pages, and opens its detail without losing the query or keyboard position.
Strategy: quick, predictable recording on the local machine. Scope: journal
search/paging first. Structure: journal with explicit filters and detail links.
Skeleton: query above a bounded table, summary and pager visible. Surface: reuse
the existing typography, contrast, and responsive rules.

Use a separate workspace checkout for UI changes. Add actual WASM/browser tests,
three-width screenshots, a read-only review, and cold-start/query latency and
asset-size evidence before merging. A preview is not full Rust migration.

## Verification, authority, and loop

The maintainer explicitly authorized harness-driven development, PRs and merges.
Use signed-off feature-branch commits; never force-push or push directly to main.
Separate design, implementation, and read-only review. Use at most three fix
rounds per slice; return a repeated unresolved failure rather than declare pass.
Keep synthetic data and secrets out of published artifacts. Do not weaken
fsync, validation, hash checks, or input safety for benchmark scores.

Use existing local tools and remote CI while external storage is unavailable.
Large internal-disk dependency/build writes need the pending storage decision.
Record measured versus inferred results and all unverified platform/IME limits.
