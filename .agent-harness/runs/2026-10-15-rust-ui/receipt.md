# Authored-Rust journal preview receipt

## Scope

Contract: [Rust journal preview](../../contracts/kamoku-rust-ui-preview-v1.md).
Base at implementation: `0cd48f38895a87a5a36b6e8823f29f1147c103bb`.
Rust owns query state, read-only API calls, DOM, IME checks, keyboard navigation,
pagination, history, and focus. A minimal loader and generated JS/WASM bindings
remain. The established UI is still the default; full parity is not claimed.

## Local checks before CI

Implementer ran four native model tests, offline wasm32 check, host/WASM clippy
with warnings denied, Rust formatting, existing JS unit tests (168), policy
checks (124 authored files), contrast (72), links, and strict trace references.
Script syntax and actionlint passed. The small checking target stayed around
202MB; full app/WASM generation is assigned to remote CI while SSD is absent.

## Independent review and changes

Separate Standards/Spec review found two P2 layout issues: a maximum valid row
amount could overlap a neighboring cell, and the 768px header link was shorter
than the 44px touch target. The implementer widened desktop amount cells, changed
the <=1023px layout to cards, wrapped long totals, and sized the header link.
Browser regressions now create two maximum-amount entries, check real text
fragments against cell/table bounds, check link dimensions, and capture all
three widths. Spec/runtime review found no additional blocker. Parent reviewed
the build/runner, static serving, CSP/MIME, and artifact boundaries separately.

## Required remote and visual evidence

Before merge: original CI; Rust 1.88 WASM check; current WASM clippy; actual
release WASM/binding generation; newly built server with embedded assets; seven
Playwright cases covering operations, composition, races/retry, safe text,
integer precision, history/focus, maximum values, and responsive screenshots.
CI saves only the synthetic PNGs and metrics JSON. Parent must view all PNGs.
The PR validation section and machine checks record final runs and actual
measurements; this pre-CI receipt does not claim those checks have passed.

## Limits and rights

No speedup from the language change is claimed. Readiness timing contains test
polling and one CI environment; compare repeated equivalent runs before choosing
a default UI. Native IME, screen readers, Safari, Firefox, and Windows remain
human/platform checks. See the [creation record](../../../docs/legal/rights-check-rust-ui.md).
