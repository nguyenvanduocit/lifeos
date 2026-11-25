#!/bin/sh
# The backend (`lifeos web`, debug build) and Vite's dev server, together.
# Vite proxies /link.json to the backend; the page dials the backend's
# WebTransport link directly. Ctrl-C stops both.
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"

[ -d web/node_modules ] || (cd web && npm install)
WASM_PROFILE=dev crates/lifeos-web-client/bind.sh
cargo build -q -p lifeos

./target/debug/lifeos web --port 7420 --link-port 7421 &
backend=$!
trap 'kill $backend 2>/dev/null' EXIT INT TERM

cd web && LIFEOS_BACKEND=http://127.0.0.1:7420 npm run dev -- --port 5173 --strictPort
