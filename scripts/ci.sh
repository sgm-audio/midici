#!/usr/bin/env bash
# Local mirror of the portable CI gates. Set WITH_ALSA=1 on a Linux box with
# alsa-lib >= 1.2.13 to include the ALSA transport and virtual-responder.
set -euo pipefail
cd "$(dirname "$0")/.."
EXCL=(--exclude midici-transport-alsa --exclude virtual-responder)
[[ "${WITH_ALSA:-0}" == 1 ]] && EXCL=()
cargo fmt --all -- --check
cargo clippy --workspace "${EXCL[@]}" --all-targets -- -D warnings
cargo test --workspace "${EXCL[@]}" --all-targets
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps "${EXCL[@]}"
rustup target add thumbv7em-none-eabihf >/dev/null
cargo build -p midici-core -p midici-pe --no-default-features --target thumbv7em-none-eabihf
command -v cargo-deny >/dev/null && cargo deny check || echo "SKIPPED: cargo-deny not installed"
echo "ci.sh: passed"
