//! `lifeos`: one binary for every interface.

use std::io::Write as _;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use lifeos_agent::{AgentConfig, Effort, LifeAgent, Progress, SharedLife};
use lifeos_core::Life;
use tokio::io::{AsyncBufReadExt, BufReader};

const PROFILE_TEMPLATE: &str = include_str!("profile_template.md");
const CLI_SESSION_KEY: &str = "cli.session";

#[derive(Parser)]
#[command(name = "lifeos", version, about = "AI agent quản trị cuộc đời")]
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

impl From<EffortArg> for Effort {
    fn from(e: EffortArg) -> Effort {
        match e {
            EffortArg::Low => Effort::Low,
            EffortArg::Medium => Effort::Medium,
            EffortArg::High => Effort::High,
            EffortArg::Xhigh => Effort::Xhigh,
            EffortArg::Max => Effort::Max,
        }
    }
}

#[derive(Subcommand)]
enum Command {
    /// Create the vault and a CLAUDE.md for you to describe yourself.
    Init,
    /// Print today's overview (no AI).
    Today,
    /// Rebuild the search index from the Markdown files.
    Reindex,
    /// Talk with the agent in the terminal.
    Chat {
        /// Continue the last terminal conversation.
        #[arg(long, short)]
        resume: bool,
    },
    /// Ask one thing and print the answer.
    Ask { message: Vec<String> },
    /// Serve the tools over MCP on stdio (`claude mcp add lifeos -- lifeos mcp`).
    Mcp,
    /// Serve the web app (HTTP) and its WebTransport link (UDP).
    Web {
        #[arg(long, default_value = "127.0.0.1")]
        host: IpAddr,
        #[arg(long, default_value_t = 7420)]
        port: u16,
        #[arg(long, default_value_t = 7421)]
        link_port: u16,
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

#[tokio::main]
async fn main() -> Result<()> {
    // Logs go to stderr: stdout belongs to MCP in `lifeos mcp`.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cli = Cli::parse();
    let home = cli.home.clone().unwrap_or_else(lifeos_core::default_home);
    let agent_config = AgentConfig {
        model: cli.model.clone(),
        effort: cli.effort.map(Into::into),
        ..Default::default()
    };
    let open = || Life::open(&home).with_context(|| format!("cannot open lifeos home {}", home.display()));

    match cli.command {
        Command::Init => init(&home),
        Command::Today => {
            let life = open()?;
            let today = life.today(lifeos_core::life::today())?;
            println!("{}", serde_json::to_string_pretty(&today)?);
            Ok(())
        }
        Command::Reindex => {
            let stats = open()?.reindex()?;
            println!("indexed {} documents", stats.indexed);
            Ok(())
        }
        Command::Chat { resume } => chat(shared(open()?), agent_config, resume).await,
        Command::Ask { message } => {
            let agent = LifeAgent::spawn(shared(open()?), agent_config).await?;
            let reply = agent.ask(&message.join(" "), |_| {}).await;
            agent.close().await;
            println!("{}", reply?.text);
            Ok(())
        }
        Command::Mcp => lifeos_mcp::serve_stdio(&mut open()?),
        Command::Web { host, port, link_port } => {
            let config = lifeos_web::WebConfig {
                http: SocketAddr::new(host, port),
                link: SocketAddr::new(host, link_port),
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

async fn chat(life: SharedLife, mut config: AgentConfig, resume: bool) -> Result<()> {
    if resume {
        config.resume = life.lock().expect("life lock").kv_get(CLI_SESSION_KEY)?.filter(|s| !s.is_empty());
    }
    let agent = LifeAgent::spawn(life.clone(), config).await?;
    eprintln!("lifeos — gõ tin nhắn, Ctrl-D để thoát.");
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    loop {
        eprint!("\n› ");
        let Some(line) = lines.next_line().await? else { break };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let result = agent
            .ask(line, |p| match p {
                Progress::Text(t) => {
                    print!("{t}");
                    let _ = std::io::stdout().flush();
                }
                Progress::Tool(name) => eprintln!("  · {}", name.trim_start_matches("mcp__lifeos__")),
            })
            .await;
        match result {
            Ok(reply) => {
                println!();
                if let Some(id) = reply.session_id {
                    life.lock().expect("life lock").kv_set(CLI_SESSION_KEY, &id)?;
                }
            }
            Err(err) => eprintln!("lỗi: {err:#}"),
        }
    }
    agent.close().await;
    Ok(())
}
