# AGENTS.md — midici build rules (binding for all agent sessions)

## Session protocol
1. START: read this file, docs/ARD-001.md, internal/PROGRESS.md. Work ONLY the current phase.
2. END: append to internal/PROGRESS.md: phase, what was done, deviations from ARD (with reason),
   DoD command outputs (verbatim), open items, next phase. This file is the single state object.
3. On any tool/test failure: compact the failure to <10 lines in internal/PROGRESS.md (root cause, not logs).
4. Never start the next phase in the same session.

## Source of truth — anti-fabrication (HIGHEST PRIORITY)
5. Protocol facts come ONLY from internal/specs/* and docs/ARD-001.md. If a required byte layout,
   constant, status code, or field is not derivable from those, STOP the phase and report the
   exact gap in internal/PROGRESS.md. NEVER guess protocol bytes. NEVER fill gaps from training memory.
6. Every spec constant carries a doc reference comment: // M2-101 §<section>.
7. crates/midici-conformance/goldens/ is APPEND-ONLY. Modifying or deleting a golden requires
   the commit message line "HUMAN-APPROVED-GOLDEN-CHANGE: <reason>" authored by Scott.
8. Tests, assertions, and lint levels may never be weakened, #[ignore]d, or deleted to make a
   phase pass. If a test is genuinely wrong, STOP and report.

## Engineering rules
9. RT contract = ARD §6, verbatim. Any allocation, lock, syscall, or logging on the RT path is a
   defect, not a style issue.
10. Dependency policy = ARD §2. Any new dependency requires a justification entry in internal/PROGRESS.md.
11. No stubs, no todo!(), no placeholder implementations in committed code. Scope not yet built
    simply does not exist in the API surface.
12. Conventional Commits (feat/fix/chore/docs/test/refactor). PR-sized branches; merge to main
    only with CI green.
13. Current build host is Scott's Windows machine, per `internal/ENVIRONMENT.md`.
    `internal/ENVIRONMENT.md` is authoritative for which environment a given phase ran in.
14. rustfmt + clippy -D warnings are gates, not suggestions.

## Definition-of-done protocol
15. A phase's DoD is a list of shell commands. Done = all exit 0 AND outputs pasted in
 internal/PROGRESS.md. Screenshots of green CI are not a substitute for local command output.

## Environment
AGENTS.md is binding on every agent/session, but the *host environment varies*.
Current build host and per-phase environment history live in
`internal/ENVIRONMENT.md` (not public-facing, but tracked with the repo).
