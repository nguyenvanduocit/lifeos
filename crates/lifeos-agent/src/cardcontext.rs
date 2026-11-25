//! What one agent has in its context, read for the owner on request (remit's `cardcontext.rs` in chains and orch): what
//! lifeos gave its session as it started it, what Claude Code loaded, and what fills its context as Claude Code counts it
//! (`/context`). Each is what was recorded or counted; a part with nothing recorded yet is absent, never filled in.

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use lifeos_core::life::cards::now;
use lifeos_core::life::sessions::SessionLaunch;
use lifeos_wire::{
    CardContext, ContextCount, ContextGiven, ContextItem, ContextPart, GivenTool, InitReport, McpServerState, PluginError,
    PluginInfo, SessionContext, SessionInit,
};

use crate::Inner;

/// How long a running session is given to count its context (remit's `CANCEL_WAIT`).
const COUNT_WAIT: Duration = Duration::from_secs(5);

/// The settings layers every session reads: `agent::options` reads the vault's project settings only.
const SETTING_SOURCES: &[&str] = &["project"];

impl Inner {
    /// What spec gives card's session: its tools, skills, settings and servers, and the tools lifeos's server lists for
    /// it. Read before the spec goes to the launcher; recorded once the session's id is known (`record_launch`).
    pub(crate) fn launch_given(self: &Arc<Self>, card: &str, spec: &agent::Spec) -> ContextGiven {
        let tools = match self.toolbox(card) {
            Ok(t) => t
                .listed()
                .into_iter()
                .map(|t| GivenTool {
                    name: t.name.to_string(),
                    description: t.description.as_deref().unwrap_or_default().to_owned(),
                })
                .collect(),
            Err(err) => {
                tracing::warn!("list the tools of {card}: {err:#}");
                Vec::new()
            }
        };
        ContextGiven {
            skills: spec.skills.clone(),
            claude_tools: spec.tools.clone(),
            setting_sources: SETTING_SOURCES.iter().map(|s| (*s).to_owned()).collect(),
            mcp_servers: spec.mcp.iter().map(|m| m.name.clone()).chain(spec.outside.keys().cloned()).collect(),
            tools,
            allowed_tools: spec.allowed_tools.clone(),
            deny_rules: spec.deny_tools.clone(),
            opening: None,
        }
    }

    /// Records what card's session `session_id` was started with.
    pub(crate) fn record_launch(&self, card: &str, session_id: &str, resumed: bool, given: ContextGiven) {
        let launch = SessionLaunch { card: card.to_owned(), session_id: session_id.to_owned(), resumed, at: now(), given };
        if let Err(err) = self.life().put_session_launch(&launch) {
            tracing::error!("record the start of {card}'s session: {err:#}");
        }
    }

    /// Records text, sent as the first message of card's session `session_id`, as its opening.
    pub(crate) fn record_opening(&self, card: &str, session_id: &str, text: &str) {
        if let Err(err) = self.life().record_opening(card, session_id, text) {
            tracing::error!("record the opening of {card}'s session: {err:#}");
        }
    }

    /// Keeps what Claude Code says card's session loaded at the start of this turn, over what it said before.
    pub(crate) fn on_init(&self, card: &str, init: model::SessionInit) {
        let report = InitReport { init: init_of(init), at: now() };
        if let Err(err) = self.life().put_session_init(card, &report) {
            tracing::error!("{card}'s session init: {err:#}");
        }
    }

    /// Keeps what fills card's session's context, counted as it went to rest, over the count before.
    pub(crate) fn on_context(&self, card: &str, context: model::SessionContext) {
        let count = ContextCount { context: context_of(context), at: now() };
        if let Err(err) = self.life().put_session_context(card, &count) {
            tracing::error!("{card}'s session context: {err:#}");
        }
    }

    /// Reads card's context. A part that cannot be read (no init yet, no count yet) is reported as such, never as the
    /// whole read failing.
    pub(crate) async fn card_context(&self, card: &str) -> Result<CardContext> {
        let c = self.card(card)?;
        let mut out = CardContext { card: c.id.clone(), ..CardContext::default() };
        let (launch, init, stored) = {
            let life = self.life();
            (life.session_launch(card)?, life.session_init(card)?, life.session_context(card)?)
        };
        let ours = |session_id: &str| !c.session_id.is_empty() && session_id == c.session_id;
        out.given = launch.filter(|l| ours(&l.session_id)).map(|l| l.given);
        out.loaded = init.filter(|i| ours(&i.init.session_id));
        match self.count_context(card).await {
            Some(Ok(context)) => out.context = Some(context),
            counted => {
                if let Some(Err(why)) = counted {
                    out.context_error = why;
                }
                out.context = stored.filter(|s| s.context.session_id == c.session_id);
            }
        }
        Ok(out)
    }

    /// Has card's running session count its context now, giving it `COUNT_WAIT` to answer; none when no session of it
    /// runs.
    async fn count_context(&self, card: &str) -> Option<std::result::Result<ContextCount, String>> {
        let session = self.run_of(card)?.session()?;
        Some(match tokio::time::timeout(COUNT_WAIT, session.context()).await {
            Ok(Ok(context)) => Ok(ContextCount { context: context_of(context), at: now() }),
            Ok(Err(err)) => Err(err.to_string()),
            Err(_) => Err("the agent's process did not answer".to_owned()),
        })
    }
}

fn init_of(i: model::SessionInit) -> SessionInit {
    SessionInit {
        session_id: i.session_id,
        version: i.version,
        model: i.model,
        permission_mode: i.permission_mode,
        output_style: i.output_style,
        tools: i.tools,
        mcp_servers: i
            .mcp_servers
            .into_iter()
            .map(|m| McpServerState { name: m.name, status: m.status, source: m.source })
            .collect(),
        skills: i.skills,
        plugins: i.plugins.into_iter().map(|p| PluginInfo { name: p.name, path: p.path, version: p.version }).collect(),
        plugin_errors: i
            .plugin_errors
            .into_iter()
            .map(|e| PluginError { plugin: e.plugin, kind: e.r#type, message: e.message })
            .collect(),
        agents: i.agents,
        slash_commands: i.slash_commands,
    }
}

fn context_of(c: model::SessionContext) -> SessionContext {
    let items = |xs: Vec<model::ContextItem>| -> Vec<ContextItem> {
        xs.into_iter()
            .map(|x| ContextItem { name: x.name, source: x.source, tokens: x.tokens, deferred: x.deferred })
            .collect()
    };
    SessionContext {
        session_id: c.session_id,
        model: c.model,
        total: c.total,
        size: c.size,
        compact_at: c.compact_at,
        parts: c
            .parts
            .into_iter()
            .map(|p| ContextPart { id: p.id, name: p.name, tokens: p.tokens, kind: p.kind })
            .collect(),
        memory_files: items(c.memory_files),
        skills: items(c.skills),
        agents: items(c.agents),
        mcp_tools: items(c.mcp_tools),
        tool_calls: items(c.tool_calls),
    }
}
