# Sanitize manifest: midici (2026-10-31)

Branch `chore/sanitize`, cut from `main` @ `95a9677`. 4 local commits, including this file. Nothing pushed. PR #17 (@a5c5874) is included as the first commit.

## Removed
| Path | Why |
|---|---|
| `AGENTS.md` | Agent instructions. (From PR #17.) |
| `internal/ANNOUNCE.md`, `ENVIRONMENT.md`, `PROGRESS.md` | Internal and agent progress notes. `ENVIRONMENT.md` had 3 lines with local paths. (From PR #17.) |

## Moved
`internal/VERIFY.md` → `docs/VERIFY.md`.

## Added
`CONTRIBUTING.md` and `scripts/ci.sh` (mirrors the portable CI gates). LICENSE was already MIT.

## Code fixes (tested)
- `controller.rs:637,643`: E0502 borrow error fixed. `main` didn't compile. (From PR #17.)
- `cargo fmt`.
- `ci.yml`: a no-std job was added (thumbv7em build), and the `cursor/**` push trigger was dropped.

## Doc claims changed
- The README was rewritten plain.
- The goldens are described as midici-generated regression fixtures, not hardware captures.
- The no_std claim is scoped to what was built (thumbv7em).
- The status gives the 2026-10-31 test run and MSRV = the pinned 1.97.1.
- The ARD-001 status line marks it as a design record and lists the open §8 items.
- The CLAP guide lost its agent-rules reference and a dead `autoprop.cast` link.

## Tests, before and after (shared 8-vCPU Intel Xeon VM, Linux, Rust 1.97.1)
| Check | Before (`main` @ 95a9677) | After |
|---|---|---|
| `cargo test --workspace` | compile error (E0502 in midici-pe) | 108 passed, 1 ignored |
| `cargo build` for `thumbv7em-none-eabihf`, no_std crates | not checked | builds |
| `scripts/ci.sh` | n/a | passes |
Logs: `_logs/midici-before-test.log`, `midici-after-test.log`, `midici-after-ci.log`, `midici-thumb.log`.

## Needs Scott
- **crates.io:** midici 0.1.0 is published, and the `midici-*-v0.1.0` tags point at commits that a rewrite would change. The crates.io tarballs are unaffected, but the tags would be force-moved. Decide whether to rewrite at all; the only path hits are 15 lines in old docs.
- **MIDI spec PDFs:** history holds MIDI Association spec PDFs (`docs/specs/M2-10x-*.pdf`, about 8.7 MB). These are usually not licensed for redistribution, so check whether they should stay in history. The tree no longer has them.
- **CI billing.**

## History rewrite (prepared, not run on the real repo)

Nothing has been rewritten or pushed. The commands below were tried on a throwaway `--mirror` clone in `/tmp` on 2026-10-31. They exited 0, and afterwards `git log --all -p` and the commit messages had 0 matches for the local-path patterns. Pack size went from 9.48 MiB to 378 KiB. That scratch clone was deleted.

The secret scans (gitleaks `git` and `dir`, and trufflehog, over all history) found no secrets. So this rewrite is about local paths and large or unneeded files, not live keys. Scott says any exposed keys were rotated anyway.

`paths-midici.txt` (paths removed from every commit):
```text
alsa-utils-1.2.15.2-1.fc42.src.rpm
AGENTS.md
internal/ANNOUNCE.md
internal/ENVIRONMENT.md
internal/PROGRESS.md
glob:docs/specs/*.pdf
```

`replace.txt` is kept outside the repo, so the paths it removes aren't republished here. Use `/workspace/handoff/_history/replace-paths.txt`. It has 8 literal lines covering the Linux, Fedora Atomic (`/var/home`), macOS and Windows home-directory prefixes and the `x:` workspace URI. The `/var/home` line comes before the plain `/home` line.

Commands:
```bash
# 1. Merge chore/sanitize into the default branch first, so the rewrite covers the cleaned tree.
git clone --mirror https://github.com/sgm-audio/midici.git midici-rewrite.git
cd midici-rewrite.git
git filter-repo --invert-paths --paths-from-file ../paths-midici.txt --replace-text ../replace.txt
# 2. Check before pushing
git log --all -p | grep -cEf ../path-patterns.txt   # expect 0 (patterns file is next to replace.txt)
gitleaks git . --no-banner --redact                                                       # expect no leaks
git count-objects -vH
# 3. Push. Needs Scott's OK: this rewrites every commit hash.
git push --force --all origin && git push --force --tags origin
```

Caveats:
- GitHub keeps `refs/pull/*` for every PR, and a force-push can't touch them. The old objects stay reachable through those PR refs and cached commit URLs until GitHub Support runs a GC; ask them to purge after the push.
- Forks and existing clones keep the old history. Collaborators need to re-clone, and open PRs have to be recreated.
- Tags (`midici-*-v0.1.0`, `phase-*`) are rewritten too. `git push --force --tags` moves them.
