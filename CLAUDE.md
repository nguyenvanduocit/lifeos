# LifeOS

LifeOS is a personal AI assistant for tasks, goals, habits, relationships, finances, health, journals, notes, and inbox items. Data lives on the user’s machine. Users can interact with LifeOS through the terminal, web, Telegram, and MCP.

## Architecture

```text
crates/
  lifeos-wire        Wire types and Postcard framing, without I/O. Builds for wasm32.
  lifeos-core        Markdown vault, SQLite, and the tools! registry.
                      src/life/ contains domain modules, crew, memory, chats, and dashboards.
                      Tool inputs are declared beside their handlers.
  lifeos-agent       remit's agent backbone (docs/adr/0001): one card and stream per agent, sessions through
                      the agent crate, lifeos tools over a loopback MCP door, messages, questions, memory
                      recall, the librarian, workflow runs.
  agent, claudecode, claudehooks, workflow, jail, procgroup, childenv, urlescape, openrouter, model, telemetry
                     Copied from remit. Carry remit's changes over; lifeos's own changes are listed in the ADR.
  lifeos-google      The built-in Google app: OAuth sign-in (PKCE, the owner's own client) and Gmail, Calendar,
                      Drive and Contacts tools, served to agents through the door as the MCP server `google`.
  lifeos-mcp         LifeOS tools exposed as an MCP server over stdio.
  lifeos-web         Axum serves the embedded app and /link.json.
                      web-transport-quinn serves WebTransport over UDP.
  lifeos-web-client  Browser-side link built to Wasm with web-transport and wasm-bindgen.
  lifeos             Binary with init, today, reindex, chat, ask, mcp, web, and telegram.
                      today prints dashboard JSON without invoking the agent.
  lifeos-telegram    Telegram bot using long polling.
apps/ios/            SwiftUI iPhone app (XcodeGen): reads Apple Health and POSTs daily values to /api/health/apple
                      (`metrics` with source `apple-health`); a WebView of the web app. docs/ios.md.
web/                 Vue 3, Vite, and Tailwind 4. Renders data from the Wasm client.
```

The browser path runs from Vue to the Wasm client, through WebTransport to `lifeos-web`, then to `Life` or `Agents`. Frames use a four-byte big-endian length prefix followed by a Postcard payload. Each call uses its own bidirectional stream. The client writes one `Call` and closes its send side. The server sends exactly one reply, then closes its send side. `Call::Watch` keeps the stream open and sends `Reply::Changed` when data changes (agent writes, other processes editing the vault or database), and `Reply::Event`, `Reply::Delta` and `Reply::Agent` as agents work.

## Agent-centered interface

- Dashboard pages read through `Call::Read` using tools marked `read`. The server rejects calls to tools marked `write` through this route. Users ask the agent to change their data.
- Pages may write the owner's own acts on agents, as remit's do (docs/adr/0001): `Say`, `BlockPick` (a pick in a block an agent showed with `show_block`), `Overturn` (his word on a question the Assistant decided or left to him), `Teach` (Helpful / Not helpful / Correct reply on an agent's reply, a teaching for the librarian) and `Stop { card, reason }` (drops what waits, ends the turn, keeps his why in the stream for the agent's next message). None of them runs a tool.
- Page actions either send a prompt to the agent through `ask` or put a draft in the Assistant composer through `say` and `AskChip`.
- The Assistant panel is available on every page and is remit's drawer. Each agent (the Assistant, each crew member, the hidden librarian) has one card and one stream of events (`cards`, `events`): its conversation, and while prompts wait in it, its outbox. The protocol has `Call::Agent`, `Events`, `Say { images, delivery: queue | steer | interrupt }`, `Interrupt`, `Restart`, `Draft`, `ImagePut`, `ImageGet` (images are stored once by id in SQLite and named by reference, remit's ADR-0111).
- **Crew.** Member files in `crew/` define roles, second-person instructions, models, effort, and tool access. Assistant manages members through `add_member`, `update_member`, and `retire_member`. Any agent asks another with `ask_agent` or hands it work with `hand_off` (`heads_up` for news); each is a row in `handoffs`, and what one piece of work handed out comes back to the sender together, once, as `[what came of what you handed off]`. An agent that handed part of its hand-off on takes a turn on what came back (check, fix) before its answer goes up. Chains hold at most 4 agents and never loop back; `team_status`, `cancel_handoff` and `<handed-off>` in every turn show where the work is, and a health check hands lost work again once and reports what failed (docs/adr/0002). A member asks the owner with `ask_owner`. No agent waits on the owner: the Assistant decides `ask_owner` questions (`answer_question`, `leave_to_owner`), and the owner overturns through the Assistant.
- **Memory.** Only the librarian writes memory, from teachings (each turn on the owner's words, overturns, memories that misled). Agents read: before each message LifeOS gives at most five relevant memories in a `<recalled>` block, more beside lifeos tool calls, ranked as remit's ADR-0117 (words, embeddings and a reranker through OpenRouter when `<vault>/.lifeos/openrouter.key` exists, words alone when not). Agents rate what they were given with `rate_memory`; the Assistant may `forget` on the owner's word.
- **Vault.** The owner's logins, keys and contacts (`vault_items`, after agentes.chat's ADR 0144), each shared with every agent or the ones he picks; values live one file per item in the private secrets directory. A saved value comes back to no one: the owner's page (`/api/vault`, never the link) shows that one is saved, agents' `vault_list` shows names, sites, usernames and contacts. A connection's `${KEY}` resolves only in the sessions of agents the key is shared with, each use recorded (`vault_uses`), and every value is redacted from sessions. Only the owner saves, shares and deletes (page, `lifeos vault`).
- **Workflows.** Scripts in `workflows/*.js` (remit's workflow engine, a replayed journal) put agents to work, wait and ask the owner; the Assistant runs them with `run_workflow`, and a routine may run one.
- Dashboard reads refresh when `revision` changes through `useRead`.
- The morning brief uses `save_review` with kind `brief`. Schedule it with `lifeos web --brief-at 06:30`.

## Development rules

- **Define each type once.** Wire and view types live in `lifeos-wire`. Tool inputs use the `input!` macro in `lifeos-core`. Run `make gen` to generate `web/src/core/generated/` with ts-rs, including `tools.ts`, which maps each read tool to its input and output. Edit the Rust definitions and regenerate the TypeScript.
- **Keep the wire types compatible with Postcard.** Avoid `skip_serializing_if`, `flatten`, untagged or internally tagged enums, and `serde_json::Value` in `lifeos-wire`. Add new types to `crates/lifeos-wire/tests/round_trip.rs`.
- **Register each tool once.** The `tools!` registry in `crates/lifeos-core/src/tools.rs` defines access level, name, input, output, description, and handler. The agent, MCP, and web layers use `specs()` and dispatch through `call()`. `call()` runs `sync_index()` first.
- **Use `choice!` for fixed choices** such as status and priority to keep the schema, parser, and TypeScript definitions in sync.
- **Keep one source of truth for each data type.** Vault Markdown holds documents such as tasks, goals, journal entries, and notes. The `docs` and `docs_fts` tables are derived and can be rebuilt with `lifeos reindex`. SQLite stores structured records in tables such as `habits`, `habit_logs`, `interactions`, `transactions`, `budgets`, `metrics`, `inbox`, `memories`, `handoffs`, `turns`, and `kv`. Amounts use the currency’s smallest unit in `amount_minor`.
- Users can edit vault files directly. Call `sync_index()` before reading the index. `tools::call` already does this.
- Agents have no Bash access. On macOS each session runs in remit's jail (`jail::init()` first in `main`), which hides `.lifeos/`, the Vault's secrets directory and the Google sign-in. Built-in tools are limited to the vault working directory, and `PermissionMode::DontAsk` rejects unapproved actions.
- The web app and link have no built-in authentication. Run them on loopback or within a trusted tailnet using `scripts/install-tailscale.sh`. Telegram serves only allowed chat IDs.
- The WebTransport link uses a self-signed certificate pinned by hash. Browsers accept a pinned certificate with a validity period under 14 days, so `cert::rotate` installs a new certificate on the QUIC endpoint every six days. Existing connections keep their certificate.
- LifeOS is early in development. Refactors may change compatibility.

## Ship every change to production

The owner uses the installed build, not `make dev`, so a change he cannot use until it is installed is not done. After every change, once the tests pass:

```sh
make build && scripts/install-tailscale.sh
```

This builds the release binary with the web app inside, copies it to `~/.local/bin/lifeos`, and restarts the `com.lifeos.web` LaunchAgent. Check that the service came back (`launchctl print gui/$(id -u)/com.lifeos.web`, log in `~/Library/Logs/lifeos-web.log`) and say in your report that it is installed. The owner opens it at https://lifeos.aiocean.dev (Caddy in `~/.config/caddy-remit/Caddyfile` proxies it to `127.0.0.1:7420`, tailnet only); give that address, not the `ts.net` one the install script prints. If the build or install fails, say so; do not report the change as done.

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
