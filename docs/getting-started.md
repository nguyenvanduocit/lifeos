# Installation and usage

## Build from source

Install Git, rustup, a C compiler, CMake, GNU Make, Node.js 22.12 or later, and npm. On macOS, `xcode-select --install` installs the C toolchain. Install Claude Code 2.1.329 or later and sign in before using chat or generating briefs.

From the repository root, run:

```sh
rustup toolchain install nightly --target wasm32-unknown-unknown
cargo +nightly install wasm-bindgen-cli --version 0.2.130 --locked
./scripts/setup.sh
```

The script runs `npm ci` and builds a release binary with the dashboard embedded. If the Wasm build reports a version mismatch, compare `wasm-bindgen --version` with the version pinned in `crates/lifeos-web-client/Cargo.toml`.

## Create a vault and use the terminal

```sh
./target/release/lifeos init
./target/release/lifeos today
./target/release/lifeos chat
./target/release/lifeos chat --resume
./target/release/lifeos ask "Summarize the tasks I need to do today"
```

`today` prints the dashboard as JSON and runs without a Claude Code login. To use a different vault, pass `--home` or set `LIFEOS_HOME`.

## Web dashboard

```sh
./target/release/lifeos web --brief-at 06:30
```

Open the [dashboard](http://127.0.0.1:7420). The brief runs at the specified local time each day. Omit `--brief-at` to disable the schedule. Use the Assistant panel to add or update data. If you have created a crew, select an agent in the Assistant panel to switch conversations.

## Telegram and MCP

Set `LIFEOS_TELEGRAM_TOKEN` in the process environment and run `./target/release/lifeos telegram`. With an empty allowlist, the bot replies with the chat ID. Send it a message to get your ID, then set `LIFEOS_TELEGRAM_ALLOWED_CHATS` to a comma-separated list of allowed IDs and restart the bot.

Register the MCP server over stdio from the repository root:

```sh
claude mcp add lifeos -- "$PWD/target/release/lifeos" mcp
```

To use a different vault, add `--home "$HOME/my-lifeos-vault"` before `mcp` in the registration command.

## Configuration

| Environment variable | Default or purpose |
| --- | --- |
| `LIFEOS_HOME` | `~/lifeos` |
| `LIFEOS_MODEL` | Claude Code’s default model |
| `LIFEOS_EFFORT` | `low`, `medium`, `high`, `xhigh`, or `max` |
| `LIFEOS_WEB_HOST` / `LIFEOS_WEB_PORT` | HTTP on `127.0.0.1:7420` |
| `LIFEOS_LINK_HOST` / `LIFEOS_LINK_PORT` | WebTransport uses the HTTP host and UDP port `7421` by default |
| `LIFEOS_BRIEF_AT` | Local time for the daily brief, such as `06:30` |
| `LIFEOS_TELEGRAM_TOKEN` | Telegram bot token |
| `LIFEOS_TELEGRAM_ALLOWED_CHATS` | Comma-separated chat IDs authorized to use the agent |
