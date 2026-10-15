# Test workflow verification

## Scope and review baseline

The maintainer requested continued kamoku development and PR creation through
merge. This change repairs false-success paths in the test workflow and documents
the commands contributors should run. Application behavior and tax rules are
outside this change. The separate request for another project's test procedure
still needs its target to be identified.

The implementation and read-only review compared the working diff against
`3ac426755be910cbad12b8088140df48e280235e`. That base includes the reviewed sha2
and Playwright dependency updates. PR 1 was merged as
`fc56c5c9f9b91fb7234ac17220e0c784dd836410` after all nine checks passed on its
updated head `6d707e41c05fb6ec318841980ff596d0f0fa1c70`.

## Changes and acceptance evidence

- Required tools and unknown arguments fail before testing; quick omissions are
  explicit and cannot be reported as a complete run.
- Cargo operations use locked dependencies; missing acceptance-condition
  references fail local checks and CI.
- E2E builds the server by default, reads its Cargo executable artifact, and runs
  smoke, browser flows, contract tests, and screenshots through the same CI/local
  entry point. An explicit binary override is reported as such.
- Playwright must already be installed. Startup failures do not print secret
  server logs. Each stage still uses isolated synthetic data and stops servers.
- Preparation, change-specific commands, partial checks, and reporting of manual
  gaps are documented in `docs/testing-procedure.md`.

## Local results

| Verification | Result |
|---|---|
| Python workflow regression tests | 30 passed; isolated command stubs, no downloads |
| Workflow mutations | 4 detected; zero surviving mutations |
| Independent read-only review | Standards 0 findings; Spec 0 findings |
| Shell compatibility | Bash 3.2.57 syntax and regression tests passed |
| Real-server contract runner | 25 passed using the existing cached server binary |
| Compiler failure diagnostics | Tiny offline crate confirmed readable stderr and artifact JSON on stdout |
| UI unit tests | 168 passed |
| Contrast / UI policy | 72 pairs passed / 121 files passed |
| Ledger golden values | All 7 matched the independent Python calculation |
| Strict specification trace | 78 conditions, 6 manual; zero missing references |
| Document links / diff whitespace | Passed |
| REUSE / Japanese document lint | Passed / zero findings in six changed guides |

The implementation and review were separate agents. The parent selected the
design, integrated CI/documentation, and ran the cached real-server contract
check. Full Rust compilation and browser coverage for the submitted commit are
delegated to GitHub CI; the cached-binary check does not establish dependency
compatibility. No new dependencies or browsers were installed locally.

## Remaining checks

PR 3 and this change must pass their submitted-commit CI before merge. Actual
human IME, screen reader, Windows, Safari, and Firefox verification remain open
as recorded in `docs/testing.md`. Screenshot generation alone is not a visual
quality assessment. No new visual score or complete Phase 1 claim is made.
