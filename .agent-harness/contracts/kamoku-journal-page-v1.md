# Journal page performance contract

Improve journal latency without changing event encoding, hashes, durability,
report bounds, search semantics, totals, ordering, or public API output.

## Design

- Keep ordered derived entry-state positions in Ledger, rebuilt from verified
  events and maintained only after a successful append. Preserve const new,
  clone, load, void/revision, failed append, and store rollback behavior.
- Expose borrowed current states including voided entries and their last valid
  entry. Iterate in original registration order, not HashMap order. Numbering
  must remain stable across revisions, voids, and reload.
- Avoid rebuilding history maps for each list/export/opening call. Filter once,
  check the full filtered sum, partition for the requested page and sort that
  page in descending date then original entry number. Handle empty results,
  zero/huge offsets, all limits, same dates, and partial final pages exactly.
- Keep a simple full-history/full-sort reference in meaningful tests. Compare
  complete JSON results for varied histories, filters, and page boundaries.

## Evidence and scope

Fixed hash/corruption tests, store rollback tests, full Rust and real-server
browser CI must pass. Benchmark base/head release on identical synthetic data
and the same runner, including 10k/100k entries and revision-heavy histories.
Use existing dependencies and small cached local checks; full builds run in CI
while external storage remains unavailable. Never publish auth output or DBs.

The maintainer authorized signed-off feature commits, PR creation and merging
after successful checks and independent read-only review. No direct main push
or force push. At most three fix rounds; report repeated unresolved failures.
