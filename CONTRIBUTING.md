# Contributing to LifeOS

Follow the [installation guide](docs/getting-started.md) to set up Rust nightly, wasm-bindgen, and npm. Read the [architecture notes](CLAUDE.md) before changing wire types, tools, or chat behavior.

```sh
export RUSTUP_TOOLCHAIN=nightly
(cd web && npm ci)
LIFEOS_HOME="$(mktemp -d)" make dev
```

Open the Vite development server at `http://localhost:5173`. Use a test vault and fictional data in bug reports, screenshots, and logs.

```sh
make test                           # Rust tests, Wasm build, and Vue type check
make web                            # production frontend build
cargo test -p lifeos-web --test link # WebTransport integration test
```

If you change Rust wire types or tool inputs, run `make gen` and commit the generated TypeScript with the Rust changes. In your pull request, describe the behavior that changed and list the checks you ran.
