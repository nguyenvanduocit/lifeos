//! `lifeos`: one binary for every interface.

use std::io::{IsTerminal, Read as _, Write as _};
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use lifeos_agent::{Agents, AgentsConfig, Notice, SharedLife, owner};
use lifeos_wire::{ASSISTANT, Activity, EventKind, ToolState};
use lifeos_core::Life;
use tokio::io::{AsyncBufReadExt, BufReader};

#[cfg(test)]
mod stamp;

const PROFILE_TEMPLATE: &str = include_str!("profile_template.md");

/// What `--version` prints: the package version and the git commit this binary was built from, as build.rs read it.
const VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), " (", env!("LIFEOS_BUILD_COMMIT"), ")");

#[derive(Parser)]
#[command(name = "lifeos", version = VERSION, about = "AI agent quản trị cuộc đời")]
struct Cli {
    /// The vault: Markdown files plus `.lifeos/lifeos.db`.
    #[arg(long, global = true, env = "LIFEOS_HOME")]
    home: Option<PathBuf>,
    /// Claude Code model alias or id (default: your Claude Code default).
    #[arg(long, global = true, env = "LIFEOS_MODEL")]
    model: Option<String>,
    #[arg(long, global = true, env = "LIFEOS_EFFORT", value_enum)]
    effort: Option<EffortArg>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Clone, Copy, ValueEnum)]
enum EffortArg {
    Low,
    Medium,
    High,
    Xhigh,
    Max,
}

impl EffortArg {
    fn as_str(self) -> &'static str {
        match self {
            EffortArg::Low => "low",
            EffortArg::Medium => "medium",
            EffortArg::High => "high",
            EffortArg::Xhigh => "xhigh",
            EffortArg::Max => "max",
        }
    }
}

#[derive(Subcommand)]
enum Command {
    /// Create the vault and a CLAUDE.md for you to describe yourself.
    Init,
    /// Print today's dashboard as JSON (no AI).
    Today,
    /// Rebuild the search index from the Markdown files.
    Reindex,
    /// Talk with the Assistant in the terminal: the same conversation the web
    /// app and Telegram show. `/new` starts a new session.
    Chat,
    /// Ask one thing and print the answer.
    Ask { message: Vec<String> },
    /// Serve the tools over MCP on stdio (`claude mcp add lifeos -- lifeos mcp`).
    Mcp {
        /// Serve the enabled functions of this installed plugin.
        #[arg(long)]
        plugin: Option<String>,
    },
    /// Serve the web app (HTTP) and its WebTransport link (UDP).
    ///
    /// Behind `tailscale serve` (the page needs HTTPS for WebTransport), keep
    /// HTTP on loopback and put the link on the machine's Tailscale IP:
    /// `lifeos web --link-host 100.x.y.z`.
    Web {
        /// Address for HTTP (the app and /link.json).
        #[arg(long, default_value = "127.0.0.1", env = "LIFEOS_WEB_HOST")]
        host: IpAddr,
        #[arg(long, default_value_t = 7420, env = "LIFEOS_WEB_PORT")]
        port: u16,
        /// Address for the WebTransport link (UDP); defaults to --host.
        #[arg(long, env = "LIFEOS_LINK_HOST")]
        link_host: Option<IpAddr>,
        #[arg(long, default_value_t = 7421, env = "LIFEOS_LINK_PORT")]
        link_port: u16,
        /// Have the agent write the morning brief daily at this local time, e.g. 06:30.
        #[arg(long, env = "LIFEOS_BRIEF_AT")]
        brief_at: Option<chrono::NaiveTime>,
    },
    /// Queue a routine by id.
    RunRoutine { id: String },
    /// Generate a new bearer token for POST /hooks/<id>.
    HookKey { id: String },
    /// The Vault's keys: what connections send as `${NAME}`. Logins and contacts are kept in Settings › Vault.
    Vault {
        #[command(subcommand)]
        command: VaultCommand,
    },
    /// Configure an external MCP server locally.
    Connection {
        #[command(subcommand)]
        command: ConnectionCommand,
    },
    /// Run the Telegram bot (long polling).
    Telegram {
        #[arg(long, env = "LIFEOS_TELEGRAM_TOKEN", hide_env_values = true)]
        token: String,
        /// Chat ids allowed to use the bot, comma separated.
        #[arg(long, env = "LIFEOS_TELEGRAM_ALLOWED_CHATS", value_delimiter = ',')]
        allowed_chats: Vec<i64>,
    },
}

#[derive(Subcommand)]
enum VaultCommand {
    /// Save a key's value, read from stdin. A new key is the Assistant's; share it with others in Settings › Vault.
    Set { name: String },
    /// Delete a key.
    Remove { name: String },
    /// List what the Vault holds, never a value.
    List,
}

#[derive(Subcommand)]
enum ConnectionCommand {
    /// Read an MCP config object (url/headers or command/args/env) from stdin.
    Set { name: String },
    /// Remove a connection and every crew member's grant of it.
    Remove { name: String },
}

fn main() -> Result<()> {
    // Before any thread starts: this binary is also the jail's helper, which sets an agent's sandbox up.
    jail::init();
    tokio::runtime::Builder::new_multi_thread().enable_all().build()?.block_on(run())
}

async fn run() -> Result<()> {
    // Logs go to stderr: stdout belongs to MCP in `lifeos mcp`.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        // No colour codes in launchd log files.
        .with_ansi(std::io::IsTerminal::is_terminal(&std::io::stderr()))
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cli = Cli::parse();
    let home = cli.home.clone().unwrap_or_else(lifeos_core::default_home);
    let agent_config = AgentsConfig {
        model: cli.model.clone(),
        effort: cli.effort.map(|e| e.as_str().to_owned()),
        ..Default::default()
    };
    let open = || Life::open(&home).with_context(|| format!("cannot open lifeos home {}", home.display()));

    match cli.command {
        Command::Init => init(&home),
        Command::Today => {
            let life = open()?;
            let today = life.dashboard(lifeos_core::life::today())?;
            println!("{}", serde_json::to_string_pretty(&today)?);
            Ok(())
        }
        Command::Reindex => {
            let stats = open()?.reindex()?;
            println!("indexed {} documents", stats.indexed);
            Ok(())
        }
        Command::Chat => chat(shared(open()?), agent_config).await,
        Command::Ask { message } => {
            let agents = Agents::start(shared(open()?), agent_config).await?;
            let reply = agents.ask(ASSISTANT, &message.join(" "), owner()).await;
            agents.close();
            println!("{}", reply?.shown());
            Ok(())
        }
        Command::RunRoutine { id } => {
            println!("{}", open()?.queue_manual(&id)?.id);
            Ok(())
        }
        Command::HookKey { id } => {
            println!("{}", open()?.rotate_hook_key(&id)?);
            Ok(())
        }
        Command::Vault { command } => match command {
            VaultCommand::Set { name } => {
                anyhow::ensure!(!std::io::stdin().is_terminal(), "pipe the key's value to stdin");
                let mut value = String::new();
                std::io::stdin().take(65538).read_to_string(&mut value)?;
                open()?.set_vault_key(&name, value.trim_end_matches(['\r', '\n']))
            }
            VaultCommand::Remove { name } => open()?.remove_vault_key(&name),
            VaultCommand::List => {
                for item in open()?.vault_view()?.items {
                    let what = item.key_name.clone().or(item.website.clone()).unwrap_or_default();
                    let who = if item.all_agents { "every agent".to_owned() } else if item.agents.is_empty() { "no agent".to_owned() } else { item.agents.join(", ") };
                    println!("{:?}\t{}\t{}\t{}", item.kind, item.title, what, who);
                }
                Ok(())
            }
        },
        Command::Connection { command: ConnectionCommand::Set { name } } => {
            let config = serde_json::from_reader(std::io::stdin().lock()).context("invalid MCP configuration")?;
            open()?.set_custom_connection(&name, &config)
        }
        Command::Connection { command: ConnectionCommand::Remove { name } } => open()?.remove_connection(&name),
        Command::Mcp { plugin: Some(id) } => lifeos_mcp::plugin::serve(&home, id).await,
        Command::Mcp { plugin: None } => lifeos_mcp::serve_stdio(&mut open()?),
        Command::Web { host, port, link_host, link_port, brief_at } => {
            let config = lifeos_web::WebConfig {
                http: SocketAddr::new(host, port),
                link: SocketAddr::new(link_host.unwrap_or(host), link_port),
                brief_at,
            };
            eprintln!("lifeos: http://{}", config.http);
            lifeos_web::serve(shared(open()?), agent_config, config).await
        }
        Command::Telegram { token, allowed_chats } => {
            let config = lifeos_telegram::BotConfig {
                token,
                allowed_chats,
                agent: agent_config,
            };
            lifeos_telegram::run(shared(open()?), config).await
        }
    }
}

fn shared(life: Life) -> SharedLife {
    Arc::new(Mutex::new(life))
}

fn init(home: &std::path::Path) -> Result<()> {
    let life = Life::open(home)?;
    let profile = home.join("CLAUDE.md");
    if !profile.exists() {
        std::fs::write(&profile, PROFILE_TEMPLATE)?;
    }
    println!("lifeos home: {}", life.home().display());
    println!("Hãy mở {} và viết về bản thân bạn — agent đọc nó mỗi lần trò chuyện.", profile.display());
    Ok(())
}

async fn chat(life: SharedLife, config: AgentsConfig) -> Result<()> {
    let agents = Agents::start(life, config).await?;
    let mut notices = agents.subscribe();
    // What the Assistant does, as it does it: its words on stdout, its tools on stderr.
    tokio::spawn(async move {
        while let Ok(n) = notices.recv().await {
            match n {
                Notice::Delta(d) if d.card == ASSISTANT && d.kind == EventKind::Message => {
                    print!("{}", d.text);
                    let _ = std::io::stdout().flush();
                }
                Notice::Event(e) if e.event.card == ASSISTANT && e.event.rev == 1 && e.event.kind == EventKind::Message => {
                    print!("{}", e.event.text);
                    let _ = std::io::stdout().flush();
                }
                Notice::Event(e) if e.event.card == ASSISTANT => {
                    if let Some(Activity::Tool { name, summary, status: ToolState::Failed, .. }) = &e.activity {
                        eprintln!("  ✗ {name} {summary}");
                    } else if let Some(Activity::Tool { name, status: ToolState::Running, .. }) = &e.activity
                        && e.event.rev == 1
                    {
                        eprintln!("  · {name}");
                    }
                }
                _ => {}
            }
        }
    });
    eprintln!("lifeos — gõ tin nhắn, /new để bắt đầu phiên mới, Ctrl-D để thoát.");
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    loop {
        eprint!("\n› ");
        let Some(line) = lines.next_line().await? else { break };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line == "/new" {
            agents.restart(ASSISTANT).await?;
            eprintln!("Đã bắt đầu phiên mới.");
            continue;
        }
        match agents.ask(ASSISTANT, line, owner()).await {
            Ok(answer) => {
                println!();
                if let Some(err) = answer.error {
                    eprintln!("lỗi: {err}");
                }
            }
            Err(err) => eprintln!("lỗi: {err:#}"),
        }
    }
    agents.close();
    Ok(())
}
