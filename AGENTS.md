# AGENTS.md — midici build rules (binding for all agent sessions)

## Session protocol
1. START: read this file, docs/ARD-001.md, docs/PROGRESS.md. Work ONLY the current phase.
2. END: append to docs/PROGRESS.md: phase, what was done, deviations from ARD (with reason),
   DoD command outputs (verbatim), open items, next phase. This file is the single state object.
3. On any tool/test failure: compact the failure to <10 lines in PROGRESS.md (root cause, not logs).
4. Never start the next phase in the same session.

## Source of truth — anti-fabrication (HIGHEST PRIORITY)
5. Protocol facts come ONLY from docs/specs/* and docs/ARD-001.md. If a required byte layout,
   constant, status code, or field is not derivable from those, STOP the phase and report the
   exact gap in PROGRESS.md. NEVER guess protocol bytes. NEVER fill gaps from training memory.
6. Every spec constant carries a doc reference comment: // M2-101 §<section>.
7. crates/midici-conformance/goldens/ is APPEND-ONLY. Modifying or deleting a golden requires
   the commit message line "HUMAN-APPROVED-GOLDEN-CHANGE: <reason>" authored by Scott.
8. Tests, assertions, and lint levels may never be weakened, #[ignore]d, or deleted to make a
   phase pass. If a test is genuinely wrong, STOP and report.

## Engineering rules
9. RT contract = ARD §6, verbatim. Any allocation, lock, syscall, or logging on the RT path is a
   defect, not a style issue.
10. Dependency policy = ARD §2. Any new dependency requires a justification entry in PROGRESS.md.
11. No stubs, no todo!(), no placeholder implementations in committed code. Scope not yet built
    simply does not exist in the API surface.
12. Conventional Commits (feat/fix/chore/docs/test/refactor). PR-sized branches; merge to main
    only with CI green.
13. All commands run inside distrobox `midici-dev`. Never modify the host OS.
14. rustfmt + clippy -D warnings are gates, not suggestions.

## Definition-of-done protocol
15. A phase's DoD is a list of shell commands. Done = all exit 0 AND outputs pasted in
    PROGRESS.md. Screenshots of green CI are not a substitute for local command output.
