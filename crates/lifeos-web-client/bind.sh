#!/bin/sh
# Builds the page's link client for wasm32 and binds it with wasm-bindgen-cli
# into web/src/core/pkg/web, the module the Vue app loads.
# wasm-bindgen-cli must be the version the crate pins (Cargo.toml: wasm-bindgen = "=X").
# WASM_PROFILE=dev builds fast; the default `wasm` profile builds small.
set -eu

root=$(cd "$(dirname "$0")/../.." && pwd)
target=${CARGO_TARGET_DIR:-$root/target}
out=$root/web/src/core/pkg/web
profile=${WASM_PROFILE:-wasm}
dir=$profile
[ "$profile" = dev ] && dir=debug
wasm=$target/wasm32-unknown-unknown/$dir/lifeos_web_client.wasm

want=$(sed -n 's/^wasm-bindgen = "=\(.*\)"/\1/p' "$root/crates/lifeos-web-client/Cargo.toml")
have=$(wasm-bindgen --version | cut -d' ' -f2)
if [ "$want" != "$have" ]; then
  echo "bind.sh: wasm-bindgen-cli is $have, the crate pins $want: cargo install wasm-bindgen-cli --version $want --locked" >&2
  exit 1
fi

cargo build --manifest-path "$root/crates/lifeos-web-client/Cargo.toml" --profile "$profile" --target wasm32-unknown-unknown
rm -rf "$out"
wasm-bindgen --target web --out-dir "$out" "$wasm"
printf 'bind.sh: %s bytes, %s gzipped\n' "$(wc -c < "$out/lifeos_web_client_bg.wasm" | tr -d ' ')" \
  "$(gzip -9 -c "$out/lifeos_web_client_bg.wasm" | wc -c | tr -d ' ')"
