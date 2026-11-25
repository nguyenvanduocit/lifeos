# LifeOS

**A personal AI assistant for tasks, habits, and journaling, with data stored on your machine.**

![LifeOS cover showing green hills at sunrise and an open notebook](docs/assets/cover.webp)

LifeOS uses Claude Code to chat with you and manage a Markdown and SQLite vault. See your plan for today, ask the agent to update it, and follow changes in the dashboard.

- **Plan your day** with tasks, goals, habits, journal entries, notes, and an inbox.
- **Keep track of what matters** with contact follow-ups, spending against budgets, and health trends.
- **Work with a crew** of specialist agents, each with its own role, tools, memory, and chat history.
- **Use LifeOS your way** through the web, terminal, Telegram, or MCP.

## Quick start

You’ll need Rust through rustup, a C toolchain, CMake, GNU Make, Node.js 22.12 or later, and npm. Install Claude Code and sign in before using AI features. See the [installation guide](docs/getting-started.md) for version requirements and build troubleshooting.

```sh
git clone https://github.com/nguyenvanduocit/lifeos.git
cd lifeos
rustup toolchain install nightly --target wasm32-unknown-unknown
cargo +nightly install wasm-bindgen-cli --version 0.2.130 --locked
./scripts/setup.sh
./target/release/lifeos init
./target/release/lifeos web
```

Open the [local dashboard](http://127.0.0.1:7420) and try asking **“Add a task to read tonight, then show me today’s plan.”** You can give the agent more context by editing `~/lifeos/CLAUDE.md`.

The web app listens on your machine by default. If you want access from another device, follow the [Tailscale deployment guide](docs/deployment.md). The web app and WebTransport connection have no built-in authentication.

## Documentation

| Guide | Contents |
| --- | --- |
| [Installation and usage](docs/getting-started.md) | Build, CLI, web, Telegram, MCP, and configuration |
| [Data and privacy](docs/data.md) | Vault layout, backups, and data shared with Claude Code |
| [Deployment](docs/deployment.md) | Local hosting, Tailscale, LaunchAgent, and WebTransport troubleshooting |
| [Contributing](CONTRIBUTING.md) | Development environment, tests, and pull requests |
| [Architecture](CLAUDE.md) | Crate layout, WebTransport protocol, and shared type definitions |

LifeOS is licensed under the [MIT License](LICENSE).
