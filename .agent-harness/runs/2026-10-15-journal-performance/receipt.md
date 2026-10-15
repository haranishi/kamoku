# Journal page performance receipt

Contract: [journal page](../../contracts/kamoku-journal-page-v1.md).
Implementation base: `0cd48f38895a87a5a36b6e8823f29f1147c103bb`.

## Scope and local verification

Ledger keeps derived posted-event positions and borrowed latest/last-valid
entry states. Successful append, verified reload, clone, void/revision, and
failure behavior preserve registration order. App iteration no longer rebuilds
history maps; page selection partitions and sorts only the page after checking
the complete filtered total and integer sum. Event bytes/hash/schema are intact.

Implementer passed rustfmt and offline ledger/app test-target clippy with
warnings denied. Ledger executed 15 internal tests, four new state tests
(including operation-sequence property testing), and two fixed-hash tests.
Five app reference/error/page-boundary tests were type-checked locally; their
execution and existing store rollback tests are assigned to CI.

The benchmark implementer passed offline example clippy and formatting, then
linked only the example against cached compatible libraries. Both 10k plain and
10k revised/voided fixtures matched the complete independent JSON oracle for
all six cases. Those debug/tool checks are not release performance evidence.

## Independent review

Separate read-only Standards and Spec passes found no change-related finding.
Derived-index failure/reload handling, original numbering, voided last-valid
entries, partition boundaries, deterministic ties, and all-row sum checks were
reviewed. Parent separately reviewed fixture generation, independent JSON
construction, timing boundaries, cleanup, error redaction, CLI, and CI builds.
One pre-existing books mutation search was stale at the base as well; it is
outside this change and not counted as a regression introduced here.

## Remote evidence and limits

Before merge, require the combined branch's original CI, real-server browser
suite, and same-runner release comparison. The benchmark source is copied
unchanged to base/head, compiled independently, and validates every result.
The PR validation section holds the actual run URLs/results. This receipt
records local evidence before CI and does not claim those gates have passed.

Synthetic warm sequential timings do not establish all-hardware or multi-user
SLOs. External SSD is unavailable; local checks reused cached dependencies and
bounded increments, while full builds run remotely. No accounting/tax rule or
external integration is added. See [rights record](../../../docs/legal/rights-check-journal-performance.md).
