# Test workflow contract

## Purpose

Make kamoku's documented local and CI checks fail when a required check did not
run. A contributor must be able to select the checks for a change and distinguish
a complete run, an explicitly reduced run, and an incomplete run.

## Scope

- Update `tools/check-all.sh`, `e2e/run-real.sh`, their regression/mutation tests,
  generated-file exclusions, CI, and contributor documentation. Keep the
  application and accounting rules unchanged.
- Reject unknown options and missing prerequisites before reporting success.
- Use locked dependencies. Never install missing tools or browsers implicitly.
- Rebuild the real server by default before E2E; an explicit `KAMOKU_BIN` selects
  a caller-provided executable. Respect Cargo's actual artifact location.
- Make missing automated acceptance-condition references fail the checks.
- Preserve isolated synthetic test data and sequential E2E stages.

## Acceptance and verification

- Regression tests demonstrate missing tools, invalid modes, failed child checks,
  explicit quick-mode omissions, and real-server artifact selection.
- Local shell syntax, tooling regression tests, UI tests, document links, golden
  values, and strict specification trace checks pass.
- GitHub CI checks Linux, macOS, Rust 1.88, dependencies, licensing, secrets, UI,
  and real-server E2E on the submitted commit before merge.
- A separate read-only agent reviews the final diff; material findings are fixed
  and the affected checks rerun.

## Authorization and limits

The maintainer requested commits, pushes, PR creation, and merging for kamoku in
this session. Use a feature branch, signed-off commits, and the personal account;
do not push directly to main or force-push. Do not change global skills/harnesses
or another project while its requested scope is still being clarified.

Use existing local dependencies and remote CI while external storage is
unavailable. Do not install large dependencies on the internal disk. Limit fixes
and independent review to three rounds; return a repeated unresolved failure.
