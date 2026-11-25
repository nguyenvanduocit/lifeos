//! What a Claude Code session reports (`ToolCall`, `Note`, `Task`, `Limit`, `Failure`, …), declared once: the session
//! port (`agent::Handler`, `agent::Session`) speaks these types. Taken from remit's runtime contract (`wire::runtime`);
//! lifeos runs its sessions in process, so only what a session reports is kept, not the link's calls.

use std::fmt;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{SystemNote, TaskStatus, Time, ToolStatus, text_enum};
text_enum! {
    /// What part of the provider failed a turn.
    pub enum FailureCategory {
        /// The account cannot be used (it may not use the model, it is signed out).
        Access = "access",
        /// A rate or usage limit.
        Limit = "limit",
        /// The provider's service is down or overloaded.
        Service = "service",
    }
}

/// A turn that ended because of the provider behind the agent: its account, its limits or its service. It is what the
/// turn's error is when the provider is at fault, so a caller can have the work wait for the provider rather than for the
/// owner. `title` is what the provider said; `resets_at` when the limit it hit resets, none when it did not say.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Failure {
    pub category: FailureCategory,
    pub title: String,
    pub resets_at: Option<Time>,
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.title)
    }
}

impl std::error::Error for Failure {}

text_enum! {
    /// Whether the account's limit still takes requests.
    pub enum LimitStatus {
        /// It takes them.
        Allowed = "allowed",
        /// It takes them, and the provider warns the limit is near.
        Warning = "warning",
        /// It takes none until it resets.
        Rejected = "rejected",
    }
}

/// What the provider said of the account's limit at a turn (Claude Code's rate_limit_event), each time it said it:
/// `window` is the limit it speaks of as it names it (`five_hour`, `seven_day`, `seven_day_opus`, …; empty when it did not
/// say), `utilization` how much of it is used from 0 to 1 and `resets_at` when it resets; none when the provider did not
/// say.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Limit {
    pub status: LimitStatus,
    pub window: String,
    pub utilization: Option<f64>,
    pub resets_at: Option<Time>,
}

text_enum! {
    /// The kind of a tool call, as the stream records it (`model::Event::tool_kind`) and the page groups it.
    pub enum ToolKind {
        Read = "read",
        Edit = "edit",
        Delete = "delete",
        Move = "move",
        Search = "search",
        Execute = "execute",
        Think = "think",
        Fetch = "fetch",
        SwitchMode = "switch_mode",
        Other = "other",
    }
}

/// One use of a tool by the agent, as it grows over entries (the same `id` is the same call): Claude Code's tool name
/// (Bash, Edit, mcp__remit__update_plan, …) and input, and what they read as (`kind`, `title`). `output` is the text the
/// tool answered with, once it has. The input is JSON Claude Code defines, carried as `model::ForeignJson` is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub kind: ToolKind,
    pub title: String,
    pub status: ToolStatus,
    #[serde(with = "crate::stored_json::foreign_object")]
    pub input: Map<String, Value>,
    pub output: String,
}

text_enum! {
    /// Which of the things Claude Code does to a session a [`Note`] tells.
    pub enum NoteKind {
        /// A request to the model failed and Claude Code tries it again after a wait.
        Retrying = "retrying",
        /// Claude Code compacted the conversation: what the agent was told before is a summary now.
        Compacted = "compacted",
        /// Claude Code tried to compact the conversation and could not: it holds what it held.
        CompactFailed = "compact_failed",
        /// The session read a CLAUDE.md or rule file.
        InstructionsRead = "instructions_read",
        /// Claude Code changed the session's model on its own.
        ModelSwitched = "model_switched",
        /// A rail kept something out of the session: a tool call while its turn is being cancelled, a settings change.
        Refused = "refused",
    }
}

/// Something Claude Code did to a session beside the agent's work, which the owner follows the agent by: `kind` which it
/// is, `title` what it is about when it is about one thing (the instructions file read; empty otherwise), `text` says it
/// in words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub kind: NoteKind,
    pub title: String,
    pub text: String,
}

impl From<NoteKind> for SystemNote {
    /// The system event a note is recorded as.
    fn from(kind: NoteKind) -> Self {
        match kind {
            NoteKind::Retrying => SystemNote::Retrying,
            NoteKind::Compacted | NoteKind::CompactFailed => SystemNote::Compacted,
            NoteKind::InstructionsRead => SystemNote::InstructionsRead,
            NoteKind::ModelSwitched => SystemNote::ModelSwitched,
            NoteKind::Refused => SystemNote::Refused,
        }
    }
}

impl Note {
    /// A note about nothing in particular: its words alone.
    pub fn says(kind: NoteKind, text: String) -> Self {
        Note {
            kind,
            title: String::new(),
            text,
        }
    }
}

/// How full the session's context is: `used` of `size` tokens.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub used: u64,
    pub size: u64,
}

/// The task type of a subagent.
pub const TASK_SUBAGENT: &str = "local_agent";

/// The task type of a run of Claude Code's Workflow tool.
pub const TASK_WORKFLOW: &str = "local_workflow";

/// Work a session's process runs beside the turn that started it: a shell command run in the background, a Monitor
/// watching one, a subagent, a workflow run. While it runs, the agent can be told of it (a Monitor's events, the task's
/// end) and take a turn of its own on that news. A subagent or a workflow run is reported from its start, whether or not
/// it runs in the background; the session's other tasks only once they are in the background.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    /// The process's name for it; a subagent's is its agent id, the one `agent::Handler::on_subagent` is asked for.
    pub id: String,
    /// Claude Code's task type: `TASK_SUBAGENT`, `TASK_WORKFLOW`, local_bash (a background shell or a Monitor), ...
    pub kind: String,
    pub title: String,
    /// The tool call that started it, once known.
    pub tool_call_id: String,
    pub output_file: String,
    pub status: TaskStatus,
    /// How it ended, in Claude Code's words.
    pub summary: String,
    /// A subagent's and a workflow run's, from their start: the subagent type it runs as, what it was asked (a workflow
    /// run's script, as Claude Code ran it), how deep its spawn is (1 for one the session's own agent spawned), and
    /// whether it runs beside the turn that spawned it, which went on.
    pub agent_type: String,
    pub prompt: String,
    pub depth: i64,
    pub background: bool,
    /// A workflow run's: its name, the handle that resumes it (`Workflow({scriptPath, resumeFromRunId})`) once its launch
    /// is answered, where its script and its agents' transcripts are, and its agents as its progress last said.
    pub name: String,
    pub run_id: String,
    pub script_path: String,
    pub transcript_dir: String,
    pub agents: Vec<WorkflowAgent>,
}

/// One agent() call of a workflow run, as the run's progress says it. A workflow's agents run inside the run: they are not
/// subagents of the session, and what they write does not reach it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowAgent {
    /// Its place among the run's agents, from 1.
    pub index: i64,
    /// The label the script gave it.
    pub label: String,
    /// The phase it ran in.
    pub phase: String,
    pub agent_id: String,
    /// Its subagent type.
    pub agent_type: String,
    /// running, done or error.
    pub state: String,
    /// The start of its prompt.
    pub prompt: String,
    /// The start of its result.
    pub result: String,
    pub error: String,
}

/// The end of what a background shell or a Monitor of a session's process has written: its last 8 KiB (`text`, as Claude
/// Code keeps it), all it wrote (`total_bytes`), and whether `text` starts partway through.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskOutput {
    pub text: String,
    pub total_bytes: u64,
    pub truncated: bool,
}

/// The agent asking to make a tool call, and why Claude Code asks rather than decides (empty when it gives no reason).
/// `classifier` is the auto-mode classifier's reason when the call is one it refused, come back to be asked (ADR-0083);
/// empty otherwise. `agent_id` is the subagent that asks; empty when the session's own agent does.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRequest {
    pub tool: ToolCall,
    pub reason: String,
    pub classifier: String,
    pub agent_id: String,
}

/// The answer to a [`PermissionRequest`]: the call is allowed, or refused with a message, which the agent reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(remote = "Self", rename_all = "snake_case")]
pub enum PermissionAnswer {
    Allow,
    Deny { message: String },
}

crate::json_tagged!(PermissionAnswer, "answer");

/// An MCP server the session reaches, with the headers (the run's token) it is called with.
/// A moment of an agent's work at which Claude Code takes what the core adds to what the agent reads (its hooks'
/// `additionalContext`): a call of a tool that succeeded, by its name and input, or the agent's start, a subagent's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Moment {
    ToolUsed {
        tool: String,
        #[serde(with = "crate::stored_json::foreign_object")]
        input: Map<String, Value>,
    },
    Started,
}

/// What the agent reads beside a moment of its work: empty when nothing.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Told {
    pub text: String,
}
