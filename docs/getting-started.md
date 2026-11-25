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

## Routines and webhooks

In **Tự động**, ask the Assistant to create a routine. Each routine has instructions and a target agent. Triggers include five-field cron, a one-time local date and time, a webhook, a LifeOS write tool, and a manual run. The web process reads the SQLite queue and saves each result to the history and to the target agent’s chat. While the server is down, the queue remains on disk. A missed recurring schedule is queued once on startup.

For a webhook-enabled routine, run `lifeos hook-key <routine-id>` on the server to generate a key. Send `POST /hooks/<routine-id>` with an `Authorization: Bearer …` header and up to 64 KiB of UTF-8 text. A successful request returns HTTP 202 and a run ID. Run `lifeos run-routine <routine-id>` for a manual trigger. Pass the same `--home` as the web process when targeting a separate vault.

## Connections and Claude accounts

Under **Integrations → Connections**, select Browser or X and follow the preset’s setup instructions on the machine that runs LifeOS. GitHub is available as an [optional plugin](github-plugin.md). Use **Edit crew access** to ask the Assistant to grant access to a crew member.

Browser uses `agent-browser` over MCP. Install an MCP-capable `agent-browser` on the LifeOS host’s `PATH` and run `agent-browser install` as the user that runs LifeOS. Select **Add Browser** to give the Assistant access to navigation, page snapshots, form controls, and screenshots. The Assistant and any Crew members granted access can use the shared `lifeos` browser session on that host.

Google (Gmail, Calendar, Drive, Contacts) is built in. Open **Integrations → Google**, paste the JSON of a Desktop-app OAuth client from your own Google Cloud project once (the page lists the steps), then press **Sign in with Google**. Google sends the browser back to `/api/google/callback` at the address you opened LifeOS at. Opened on loopback, a **Desktop app** client works as is. Opened at a domain (for example behind Caddy or Tailscale Serve), create a **Web application** client and add the redirect URI the Google page shows under Authorized redirect URIs; LifeOS reads the domain from `Host` and `X-Forwarded-Proto`. Publish the OAuth app in Google Cloud, or Google ends the sign-in after 7 days. LifeOS keeps the client and the refresh token outside the vault, out of every agent's reach, and serves the tools to agents itself as `mcp__google__*`. The Assistant can always use them; grant the `google` connection to crew members that should. Agents can read, send and delete with full access, so the Assistant only sends mail or deletes when you say so.

For a custom MCP server, pipe an object into `lifeos connection set <name>`. Remove one with `lifeos connection remove <name>`. Use `url` and `headers` for HTTP or `command`, `args`, and `env` for stdio. Reference credentials in header or environment values using `${KEY_NAME}`, and save the key in the Vault (Integrations → Vault, or pipe it into `lifeos vault set KEY_NAME` on the host). A connection reaches only the agents its keys are shared with. LifeOS keeps passwords' and keys' values in a private directory outside the vault folder; see [The Vault](data.md#the-vault). The Connections status describes the saved configuration. Provider sign-in and tool availability depend on the MCP server.

Under **Tích hợp → Tài khoản**, the machine’s Claude login is listed alongside added logins and Claude OAuth token connections. To add a login, open the sign-in link and paste the code into the account page if Claude asks for it. Token connections use tokens produced by `claude setup-token`. **Kiểm tra lại** makes a short Claude request on each available account to check its status. LifeOS can move work to another account when Claude rejects a turn before tools run. A turn with tool activity stays in history so its effects can be reviewed.

## Telegram and MCP

In the web app, open **Integrations → Plugins**, install **Telegram**, and open **Details**. Save your @BotFather token, send `/start` to the bot to get your chat ID, then add the ID to **Allowed chat IDs** and save. Messages in that chat now reach the same Assistant as the web app. Disable the plugin to stop polling. Run only one poller per token.

For the standalone CLI, set `LIFEOS_TELEGRAM_TOKEN` in the process environment and run `./target/release/lifeos telegram`. With an empty allowlist, the bot replies with the chat ID. Send it a message to get your ID, then set `LIFEOS_TELEGRAM_ALLOWED_CHATS` to a comma-separated list of allowed IDs and restart the bot.

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
| `LIFEOS_SECRETS_DIR` | Optional absolute directory outside the vault folder for the Vault's passwords and keys |
| `LIFEOS_GOOGLE_DIR` | Optional absolute directory outside the vault for the Google OAuth client and sign-in |
| `LIFEOS_ACCOUNTS_DIR` | Optional absolute directory outside the vault for isolated Claude profiles |

### Memory models

LifeOS reads `<vault>/.lifeos/memory.json` when the agent service starts. To enable OpenRouter retrieval and optional Jev review, save this file, put the OpenRouter API key in `<vault>/.lifeos/openrouter.key`, and restart LifeOS. The key file can also be selected with `LIFEOS_OPENROUTER_KEY_FILE`.

```json
{
  "openrouter_enabled": true,
  "embedding_model": "qwen/qwen3-embedding-8b",
  "rerank_model": "qwen/qwen3-reranker-8b",
  "decision_model": "typesafe/jev-1.14"
}
```

With no configuration file, retrieval uses the Qwen models when a key is present. Set `openrouter_enabled` to `false` to use word and link search locally even with a key installed. A missing key also selects local retrieval. Set `decision_model` to `null` (the default) to skip decision calls. When enabled, Jev checks up to six nearby memories for duplication or contradiction before the librarian writes. Unanswered pairs retain the cosine check. Changing the embedding model starts background indexing under the new model ID. The similarity thresholds were calibrated for the default Qwen model.

Open `/api/memory/models` on your LifeOS instance to see the loaded model IDs and whether remote calls are enabled with a key available.
