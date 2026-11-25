# LifeOS

LifeOS is a personal AI assistant for tasks, goals, habits, relationships, finances, health, journals, notes, and inbox items. Data lives on the user’s machine. Users can interact with LifeOS through the terminal, web, Telegram, and MCP.

## Architecture

```text
crates/
  lifeos-wire        Wire types and Postcard framing, without I/O. Builds for wasm32.
  lifeos-core        Markdown vault, SQLite, and the tools! registry.
                      src/life/ contains domain modules, crew, memory, chats, and dashboards.
                      Tool inputs are declared beside their handlers.
  lifeos-agent       Claude Code session via claude-code-agent, with in-process tools.
  lifeos-mcp         LifeOS tools exposed as an MCP server over stdio.
  lifeos-web         Axum serves the embedded app and /link.json.
                      web-transport-quinn serves WebTransport over UDP.
  lifeos-web-client  Browser-side link built to Wasm with web-transport and wasm-bindgen.
  lifeos             Binary with init, today, reindex, chat, ask, mcp, web, and telegram.
                      today prints dashboard JSON without invoking the agent.
  lifeos-telegram    Telegram bot using long polling.
web/                 Vue 3, Vite, and Tailwind 4. Renders data from the Wasm client.
```

The browser path runs from Vue to the Wasm client, through WebTransport to `lifeos-web`, then to `Life` or `LifeAgent`. Frames use a four-byte big-endian length prefix followed by a Postcard payload. Each call uses its own bidirectional stream. The client writes one `Call` and closes its send side. The server sends any `Reply::Progress(Step)` messages for text, thinking, tool calls, and tool results, followed by exactly one final reply, then closes its send side. `Call::Watch` keeps the stream open and sends `Reply::Changed` when data changes, including changes from agent writes, completed chats, and other processes editing the vault or database.

## Agent-centered interface

- Dashboard pages read through `Call::Read` using tools marked `read`. The server rejects calls to tools marked `write` through this route. Users ask the agent to change their data.
- Page actions either send a prompt to the agent through `ask` or put a draft in the Assistant composer through `say` and `AskChip`.
- The Assistant panel is available on every page. The protocol supports `Call::Chat { agent }`, `Stop`, `History`, and `ResetChat`. Each agent has a Claude Code session and its own history in the `turns` table. The chat renders user messages as bubbles, collapses completed steps, groups adjacent tool calls, and keeps errors visible.
- **Crew.** Member files in `crew/` define roles, second-person instructions, models, effort, and tool access. Assistant manages members through `add_member`, `update_member`, and `retire_member`. Changing a member’s configuration recreates its session.
- **Memory.** The `memories` and `memories_fts` tables associate each memory with a `holder`, either `assistant` or a crew member ID. Agents use `remember` and `forget`. Before each message, LifeOS recalls up to five relevant memories from the current agent and Assistant and adds them in a `<recalled>` block. The Memory page sends requests to forget a memory through the agent.
- The Assistant panel shows running tools. Dashboard reads refresh when `revision` changes through `useRead`.
- The morning brief uses `save_review` with kind `brief`. Schedule it with `lifeos web --brief-at 06:30`.

## Development rules

- **Define each type once.** Wire and view types live in `lifeos-wire`. Tool inputs use the `input!` macro in `lifeos-core`. Run `make gen` to generate `web/src/core/generated/` with ts-rs, including `tools.ts`, which maps each read tool to its input and output. Edit the Rust definitions and regenerate the TypeScript.
- **Keep the wire types compatible with Postcard.** Avoid `skip_serializing_if`, `flatten`, untagged or internally tagged enums, and `serde_json::Value` in `lifeos-wire`. Add new types to `crates/lifeos-wire/tests/round_trip.rs`.
- **Register each tool once.** The `tools!` registry in `crates/lifeos-core/src/tools.rs` defines access level, name, input, output, description, and handler. The agent, MCP, and web layers use `specs()` and dispatch through `call()`. `call()` runs `sync_index()` first.
- **Use `choice!` for fixed choices** such as status and priority to keep the schema, parser, and TypeScript definitions in sync.
- **Keep one source of truth for each data type.** Vault Markdown holds documents such as tasks, goals, journal entries, and notes. The `docs` and `docs_fts` tables are derived and can be rebuilt with `lifeos reindex`. SQLite stores structured records in tables such as `habits`, `habit_logs`, `interactions`, `transactions`, `budgets`, `metrics`, `inbox`, `memories`, `turns`, and `kv`. Amounts use the currency’s smallest unit in `amount_minor`.
- Users can edit vault files directly. Call `sync_index()` before reading the index. `tools::call` already does this.
- The agent has no Bash access. Built-in tools are limited to the vault working directory, and `PermissionMode::DontAsk` rejects unapproved actions.
- The web app and link have no built-in authentication. Run them on loopback or within a trusted tailnet using `scripts/install-tailscale.sh`. Telegram serves only allowed chat IDs.
- The WebTransport link uses a self-signed certificate pinned by hash. Browsers accept a pinned certificate with a validity period under 14 days, so `cert::rotate` installs a new certificate on the QUIC endpoint every six days. Existing connections keep their certificate.
- LifeOS is early in development. Refactors may change compatibility.

## Development commands

```sh
make deps        # install npm dependencies
make gen         # generate TypeScript from wire types and tool inputs
make core        # build Wasm into web/src/core/pkg with the pinned wasm-bindgen CLI
make test        # run Rust tests, build Wasm, and type-check Vue
make dev         # backend TCP 7420 and UDP 7421, plus Vite on TCP 5173
make build       # build a release binary with the web app embedded
```

Run `cargo test -p lifeos-web --test link` to exercise a real server over QUIC without invoking AI.
