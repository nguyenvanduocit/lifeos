//! An agent's work as the page reads it, in remit's shapes (docs/adr/0001): its card, the events of its stream, what the
//! server types each event as (`Activity`), the deltas a growing message arrives in, and where the agent stands
//! (`AgentStanding`) with what the owner can ask of it.
//!
//! A card is one agent's working record, the Assistant's or a crew member's; its id is the agent's id.

use serde::{Deserialize, Serialize};
#[cfg(feature = "ts")]
use ts_rs::TS;

/// Who sent a message to an agent, or caused a note in its stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct Actor {
    pub kind: ActorKind,
    /// The agent's id for `Assistant` and `Agent`, the routine's for `Routine`; empty otherwise.
    pub id: String,
    /// How the page names it.
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(TS), ts(export, rename_all = "snake_case"))]
pub enum ActorKind {
    /// The person whose life it is.
    Owner,
    Assistant,
    /// A crew member.
    Agent,
    /// lifeos itself: a brief it opens a session with, a recovery.
    Lifeos,
    Routine,
    /// A workflow run's step: its id is the run's.
    Workflow,
}

/// What kind of entry an event is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(TS), ts(export, rename_all = "snake_case"))]
pub enum EventKind {
    /// What the agent said, its chunks joined.
    Message,
    /// What the agent thought, its chunks joined.
    Thought,
    /// A tool call, as it grows.
    Tool,
    /// A message sent to the agent; while queued, the outbox.
    Prompt,
    /// A turn ended.
    Stop,
    /// A lifecycle note or an error.
    System,
    /// Work the agent's process runs in the background.
    Task,
}

/// One entry of a card's stream. `status` is its kind's (remit `model::EventStatus`): a tool call's
/// `pending|in_progress|completed|failed`, a prompt's `queued|delivered|steered|dropped`, a stop's
/// `end_turn|cancelled`, a system note's (`new_session`, `error`, `compacted`, …), a task's
/// `running|completed|failed|stopped`; empty for a message or a thought.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct Event {
    pub id: i64,
    pub card: String,
    pub kind: EventKind,
    pub status: String,
    pub tool_call_id: String,
    /// read | edit | delete | move | search | execute | think | fetch | switch_mode | other
    pub tool_kind: String,
    /// Claude Code's name for the tool (`Read`, `mcp__lifeos__add_task`).
    pub tool_name: String,
    pub title: String,
    pub text: String,
    pub by: Option<Actor>,
    /// What the event's kind keeps beside its text, as JSON text; empty for none.
    pub data: String,
    /// RFC 3339.
    pub created: String,
    pub updated: String,
    /// Counts its stored changes: of two copies, the page keeps the one with the higher.
    pub rev: i64,
}

/// What kind of work a tool call did, as the stream groups and counts it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(TS), ts(export, rename_all = "snake_case"))]
pub enum ToolGroup {
    Read,
    Edit,
    Execute,
    Search,
    Fetch,
    /// lifeos's own tools.
    Lifeos,
    Other,
}

/// Where a tool call stands as the page shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(TS), ts(export, rename_all = "snake_case"))]
pub enum ToolState {
    Running,
    Completed,
    Failed,
}

/// An event as the server types it, for the page to render without reading a tool's raw input.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(TS), ts(export, rename_all = "snake_case"))]
pub enum Activity {
    /// A tool call: its group, the name it is shown by, what it did in words, and where it stands.
    Tool { group: ToolGroup, name: String, summary: String, status: ToolState },
    /// A message the agent sent another agent through a tool: `to` the agent it went to.
    Sent { to: String, kind: String, label: String, text: String },
    /// Where a turn ended, how long it took, and what it touched.
    TurnEnd { ms: Option<i64>, turn: Option<TurnSummary> },
    /// The owner's Start over: the conversation shown begins at the newest one.
    StartedOver,
    /// A block the agent showed (`show_block`, remit's ADR-0091): its parts as it drew them, with the picks as they stand.
    Block { block: Block },
    /// An agent's reply the owner marked (remit's ADR-0093): his mark as it stands.
    Marked { marks: Vec<ReplyMark> },
    /// The owner stopped the agent (`Call::Stop`): its turn ended, the messages waiting for it were dropped, and why.
    Stopped { reason: String, dropped: i64 },
}

/// What an agent draws in its stream with `show_block` (remit's ADR-0091): a title and the parts the page draws with its
/// own components. Checked once at the tool (`check_block`) and trusted after.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct BlockSpec {
    /// What the block is about, in a few words.
    pub title: String,
    pub parts: Vec<BlockPart>,
}

/// One part of a block. A new kind is a new variant: stored blocks keep reading. Externally tagged (`{"choice": {…}}`),
/// as postcard needs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(TS), ts(export, rename_all = "snake_case"))]
pub enum BlockPart {
    /// Options the owner may pick from: one, or several when `many`. `picked` holds option ids: the agent's own pick from
    /// the start, which it goes ahead with; the owner's once he changed it.
    Choice {
        id: String,
        label: String,
        options: Vec<BlockOption>,
        #[serde(default)]
        many: bool,
        picked: Vec<String>,
    },
    /// Numbers drawn as bars or lines: one value of each series per label of `x`.
    Chart {
        id: String,
        label: String,
        kind: ChartKind,
        x: Vec<String>,
        series: Vec<ChartSeries>,
        #[serde(default)]
        unit: Option<String>,
    },
}

/// An option of a choice; `detail` is a line that compares it with the others.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct BlockOption {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub detail: Option<String>,
}

/// How a chart draws its series.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(TS), ts(export, rename_all = "snake_case"))]
pub enum ChartKind {
    Bar,
    Line,
}

/// One named series of a chart, a value for each label of its `x`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct ChartSeries {
    pub name: String,
    pub values: Vec<f64>,
}

/// A block as stored and shown: the spec, its choices' picks as they stand, and who changed them last and when (none:
/// they are still the agent's).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct Block {
    pub spec: BlockSpec,
    #[serde(default)]
    pub picked_by: Option<Actor>,
    /// RFC 3339.
    #[serde(default)]
    pub picked_at: Option<String>,
}

/// What the owner teaches by marking an agent's reply (remit's ADR-0093).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(TS), ts(export, rename_all = "snake_case"))]
pub enum TurnMark {
    Helpful,
    NotHelpful,
    /// The reply was wrong; the words given are the correct reply.
    Correct,
}

impl TurnMark {
    pub fn as_str(self) -> &'static str {
        match self {
            TurnMark::Helpful => "helpful",
            TurnMark::NotHelpful => "not_helpful",
            TurnMark::Correct => "correct",
        }
    }
}

/// The owner's mark on an agent's reply as it stands: his newer mark on it replaces it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct ReplyMark {
    pub by: Actor,
    pub mark: TurnMark,
    /// Why, or the correct reply; empty when he said nothing.
    pub words: String,
    /// RFC 3339.
    pub at: String,
}

/// What one turn did: what it touched, each told once with how many times, and its failures and refusals apart.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct TurnSummary {
    pub touched: Vec<Touched>,
    pub failures: Vec<TurnFailure>,
    /// The turn had more events than the server read: the row tells the first of them.
    #[serde(default)]
    pub partial: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct Touched {
    pub group: ToolGroup,
    pub name: String,
    pub summary: String,
    pub count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct TurnFailure {
    pub event_id: i64,
    pub kind: FailureKind,
    pub name: String,
    pub summary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(TS), ts(export, rename_all = "snake_case"))]
pub enum FailureKind {
    Failed,
    Refused,
}

/// An event as the page reads it: the stored event and what the server types it as.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct EventRead {
    pub event: Event,
    pub activity: Option<Activity>,
}

/// A chunk of a growing message or thought; `offset` is the length of the event's text before it, in UTF-16 units.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct EventDelta {
    pub id: i64,
    pub card: String,
    pub kind: EventKind,
    pub text: String,
    pub offset: i64,
}

/// A step of an agent's plan (Claude Code's TodoWrite).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct PlanEntry {
    pub content: String,
    pub priority: String,
    /// pending | in_progress | completed
    pub status: String,
}

/// Where a card's work stands.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(TS), ts(export, rename_all = "snake_case"))]
pub enum CardStatus {
    /// No turn runs; a message starts one.
    #[default]
    Idle,
    /// Its session is being opened.
    Starting,
    /// A turn runs.
    Working,
    /// Its last turn failed.
    Failed,
    /// Waiting for a Claude account to take its work.
    Waiting,
}

/// One agent's working record.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct Card {
    /// The agent's id: `assistant`, or a crew member's.
    pub id: String,
    pub name: String,
    pub model: String,
    pub effort: String,
    pub account: String,
    /// Claude Code's session id; empty until its first session opens, and after Start over.
    pub session_id: String,
    pub status: CardStatus,
    /// What it is doing now, in words; empty when idle.
    pub activity: String,
    pub plan: Vec<PlanEntry>,
    pub ctx_used: i64,
    pub ctx_size: i64,
    pub turns: i64,
    /// Messages waiting for its next turn.
    pub queued: i64,
    /// The final message of its last finished turn.
    pub output: String,
    pub error: String,
    pub cwd: String,
    pub created: String,
    pub updated: String,
    pub rev: i64,
}

/// Something the owner can ask of an agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(TS), ts(export, rename_all = "snake_case"))]
pub enum Ask {
    /// A message, read when its turn ends (or now when none runs).
    Message,
    /// A message put into the turn in progress.
    Steer,
    /// The turn in progress ended.
    Interrupt,
    /// A new session.
    Restart,
    /// The turn in progress ended and the messages waiting dropped, with the owner's why kept in the stream.
    Stop,
}

/// How a message the owner sends meets a turn in progress: `queue` is read after it (or now when none runs), `steer`
/// goes into it (queued when it ended meanwhile), `interrupt` ends it and is read next.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(TS), ts(export, rename_all = "snake_case"))]
pub enum Delivery {
    Queue,
    Steer,
    Interrupt,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(TS), ts(export, rename_all = "snake_case"))]
pub enum AgentTone {
    Needs,
    Failed,
    Waiting,
    Running,
    Done,
    #[default]
    Idle,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(TS), ts(export, rename_all = "snake_case"))]
pub enum DoingTone {
    Busy,
    Needs,
    Waiting,
    #[default]
    Idle,
    Done,
    Error,
}

/// The message an agent in a turn is on: the newest it took (`event_id`), and whether it has begun to answer it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct OnMessage {
    pub event_id: i64,
    pub answering: bool,
    /// When its session took it.
    pub at: String,
}

/// Where an agent stands, as its header and composer show it; the server derives every word of it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct AgentStanding {
    pub card: String,
    /// Its badge: Working, Needs you, Failed, Idle…
    pub label: String,
    pub tone: AgentTone,
    /// What it is doing right now.
    pub doing: String,
    pub doing_tone: DoingTone,
    /// The step of its plan in progress while it runs; empty otherwise.
    pub work: String,
    /// Since when it stands as it does: its turn's start while it runs.
    pub since: Option<String>,
    /// The open questions with it, pinned under its stream.
    pub waiting: Vec<i64>,
    pub asks: Vec<Ask>,
    /// What stopping it does, in the server's words: what the owner reads before he stops it; empty when he may not.
    pub stop_says: String,
    /// Whether it has a memory to read.
    pub remembers: bool,
    pub on_message: Option<OnMessage>,
    /// What a message sent now does, when it is not read at once ("Waits for the current turn (+2 queued)").
    pub on_send: Option<String>,
}

/// What the owner is typing to an agent, kept on the server so it outlives a reload and shows in every tab.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct Draft {
    pub card: String,
    pub text: String,
    /// The attachments in the box.
    pub images: Vec<ImageRef>,
    pub rev: i64,
}

/// The attachment kind. Supported images go directly to the model.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub enum ImageType {
    #[default]
    #[serde(rename = "image/png")]
    Png,
    #[serde(rename = "image/jpeg")]
    Jpeg,
    #[serde(rename = "image/gif")]
    Gif,
    #[serde(rename = "image/webp")]
    Webp,
    #[serde(rename = "application/octet-stream")]
    File,
}

impl ImageType {
    pub fn as_str(self) -> &'static str {
        match self {
            ImageType::Png => "image/png",
            ImageType::Jpeg => "image/jpeg",
            ImageType::Gif => "image/gif",
            ImageType::Webp => "image/webp",
            ImageType::File => "application/octet-stream",
        }
    }

    pub fn parse(text: &str) -> Option<ImageType> {
        Some(match text {
            "image/png" => ImageType::Png,
            "image/jpeg" => ImageType::Jpeg,
            "image/gif" => ImageType::Gif,
            "image/webp" => ImageType::Webp,
            "application/octet-stream" => ImageType::File,
            _ => return None,
        })
    }
}

/// The most bytes one image may hold: the model refuses a larger one.
pub const IMAGE_MAX_BYTES: usize = 5 * 1024 * 1024;

/// An attachment a message holds. Images use their stored id. Other files use an id of the form `uuid/filename`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct ImageRef {
    pub id: String,
    pub media_type: ImageType,
}

/// An image's bytes, base64, the form a model and a page both take them in.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct Image {
    pub media_type: ImageType,
    pub data: String,
}

/// One agent as its drawer shows it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct AgentView {
    pub card: Card,
    pub standing: AgentStanding,
    pub draft: Draft,
}

/// A question an agent put to the owner with `ask_owner` (remit's ADR-0087: no agent waits on the owner). The Assistant
/// decides it for him first; he reads the decision and may overturn it. Only what no agent can do is left to him.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct Question {
    pub id: i64,
    /// The card of the agent that asked.
    pub asker: String,
    /// How the page names the asker.
    pub asker_name: String,
    pub text: String,
    pub status: QuestionStatus,
    /// The answer the asker was given; empty until one is.
    pub answer: String,
    /// Why it was decided so, or why it was left to the owner.
    pub why: String,
    /// Who decided it: the Assistant, or the owner (his word through the Assistant).
    pub decided_by: Option<Actor>,
    /// An answer given before this one, which the owner overturned; empty when none was.
    pub overturned: String,
    pub created: String,
    pub decided: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(TS), ts(export, rename_all = "snake_case"))]
pub enum QuestionStatus {
    /// Asked; the Assistant has not decided it yet.
    #[default]
    Open,
    /// The Assistant answered it for the owner; he may overturn it.
    Decided,
    /// Left to the owner: only he can do it.
    Left,
    /// The owner answered it, or overturned the Assistant's answer.
    Answered,
    /// The asker took it back.
    Withdrawn,
}

impl QuestionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            QuestionStatus::Open => "open",
            QuestionStatus::Decided => "decided",
            QuestionStatus::Left => "left",
            QuestionStatus::Answered => "answered",
            QuestionStatus::Withdrawn => "withdrawn",
        }
    }

    pub fn parse(text: &str) -> Option<QuestionStatus> {
        Some(match text {
            "open" => QuestionStatus::Open,
            "decided" => QuestionStatus::Decided,
            "left" => QuestionStatus::Left,
            "answered" => QuestionStatus::Answered,
            "withdrawn" => QuestionStatus::Withdrawn,
            _ => return None,
        })
    }
}

/// Work or a question one agent handed another (docs/adr/0002, after agentes.chat's ADR 0034 and 0095): a row from the
/// moment it is sent until what came of it went back to the sender, so the sender, the owner and lifeos's health check
/// all see who has it and how it stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct Handoff {
    pub id: i64,
    pub kind: HandoffKind,
    /// The card of the agent that handed it, and how the page names it.
    pub sender: String,
    pub sender_name: String,
    /// The card of the agent that has it.
    pub target: String,
    pub target_name: String,
    pub text: String,
    pub status: HandoffStatus,
    /// What came of it: the target's last message, or why it failed; empty while it is out.
    pub result: String,
    /// The hand-off the sender was working on when it sent this one: this one's result goes back into it.
    pub parent: Option<i64>,
    /// The agents from the one the work started with to the sender, in order.
    pub chain: Vec<String>,
    /// How many times it was given to the target: a second when lifeos handed it again after losing it.
    pub attempts: i64,
    /// What the health check last saw: quiet for a while, handed again, given up on. Empty when nothing.
    pub note: String,
    pub created: String,
    pub updated: String,
    pub closed: String,
    /// Its result went back to the sender.
    pub reported: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(TS), ts(export, rename_all = "snake_case"))]
pub enum HandoffKind {
    /// A question whose answer the sender needs for its work (`ask_agent`).
    #[default]
    Ask,
    /// Work the target does with its own tools (`hand_off`).
    Task,
    /// News with nothing to do (`hand_off` with `heads_up`): read with the target's next message, no turn, no reply.
    News,
}

impl HandoffKind {
    pub fn as_str(self) -> &'static str {
        match self {
            HandoffKind::Ask => "ask",
            HandoffKind::Task => "task",
            HandoffKind::News => "news",
        }
    }

    pub fn parse(text: &str) -> Option<HandoffKind> {
        Some(match text {
            "ask" => HandoffKind::Ask,
            "task" => HandoffKind::Task,
            "news" => HandoffKind::News,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(TS), ts(export, rename_all = "snake_case"))]
pub enum HandoffStatus {
    /// In the target's outbox, not taken yet.
    #[default]
    Queued,
    /// The target is at work on it.
    Working,
    /// The target answered, but handed part of it on: it waits for that to come back.
    Waiting,
    /// Done: its result is the target's answer.
    Done,
    /// The target could not do it: its turn failed, the owner stopped it, or the health check gave up on it.
    Failed,
    /// The sender took it back.
    Cancelled,
}

impl HandoffStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            HandoffStatus::Queued => "queued",
            HandoffStatus::Working => "working",
            HandoffStatus::Waiting => "waiting",
            HandoffStatus::Done => "done",
            HandoffStatus::Failed => "failed",
            HandoffStatus::Cancelled => "cancelled",
        }
    }

    pub fn parse(text: &str) -> Option<HandoffStatus> {
        Some(match text {
            "queued" => HandoffStatus::Queued,
            "working" => HandoffStatus::Working,
            "waiting" => HandoffStatus::Waiting,
            "done" => HandoffStatus::Done,
            "failed" => HandoffStatus::Failed,
            "cancelled" => HandoffStatus::Cancelled,
            _ => return None,
        })
    }

    /// Still out: nothing has come of it yet.
    pub fn open(self) -> bool {
        matches!(self, HandoffStatus::Queued | HandoffStatus::Working | HandoffStatus::Waiting)
    }
}
