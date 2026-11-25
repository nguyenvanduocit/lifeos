#!/bin/sh
# Install pinned frontend dependencies and build the release binary.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"

RUSTUP_TOOLCHAIN=${RUSTUP_TOOLCHAIN:-nightly}
export RUSTUP_TOOLCHAIN

for tool in cargo npm make wasm-bindgen; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        printf 'setup: %s is required. See docs/getting-started.md.\n' "$tool" >&2
        exit 1
    fi
done

(cd web && npm ci)
make build
printf 'Built LifeOS. Run ./target/release/lifeos init to create a vault.\n'
