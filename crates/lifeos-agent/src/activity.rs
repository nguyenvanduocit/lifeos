//! What the server types an event as, for the page to render without reading a tool's raw input (remit's
//! `chains::activity`), and what a turn did, told once per thing it touched. Pure.

use lifeos_wire::{Activity, Event, EventKind, FailureKind, ToolGroup, ToolState, Touched, TurnFailure, TurnSummary};
use serde_json::Value;

/// What the stop event of a turn keeps (`Event::data`).
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StopData {
    /// The prompt the turn answered.
    pub prompt: Option<i64>,
    pub ms: Option<i64>,
    pub turn: Option<TurnSummary>,
}

/// What a prompt keeps: whether the answering turn's message goes back to the sender (one agent's message to another),
/// what the turn was given of memory, and the images it holds.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PromptData {
    /// The hand-off a turn on this prompt works for (docs/adr/0002): its own message, or what came back of what was
    /// handed on for it. The turn's last message is what comes of it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handoff: Option<i64>,
    /// The hand-offs whose results this prompt brings back to their sender.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub results: Vec<i64>,
    /// The memories the turn on it was given, as the owner sees them beside the message.
    #[serde(default)]
    pub recalled: Vec<Recalled>,
    /// The images the message holds (remit's ADR-0111): the session reads their bytes with its words.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<lifeos_wire::ImageRef>,
    /// The block event whose pick the owner's message tells (remit's ADR-0091): a later pick rewrites it while unread.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block: Option<i64>,
}

/// The status of a block event (a `System` event, its data `BlockData`).
pub const BLOCK: &str = "block";

/// The status of the note the owner's stop leaves in the stream (a `System` event, its data `StopNote`).
pub const STOPPED: &str = "stopped";

/// What a block event keeps.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BlockData {
    pub block: lifeos_wire::Block,
}

/// What a reply the owner marked keeps (remit's ADR-0093).
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MarkData {
    #[serde(default)]
    pub marks: Vec<lifeos_wire::ReplyMark>,
}

/// What the owner's stop keeps: his why and how many waiting messages it dropped.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StopNote {
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub dropped: i64,
}

/// A memory a turn was given.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Recalled {
    pub id: i64,
    /// Whose memory it is: the agent's own, or (for a crew member) the Assistant's.
    #[serde(default)]
    pub holder: String,
    pub kind: String,
    pub title: String,
}

/// What the system event of a new session keeps.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct NewSessionData {
    /// The owner asked for it (Start over), as against a lost or full session.
    pub started_over: bool,
}

/// `mcp__lifeos__add_task` → `add_task`; another server's tool → `server · tool`; a built-in tool keeps its name.
pub fn shown_name(tool_name: &str) -> String {
    if let Some(tool) = tool_name.strip_prefix("mcp__lifeos__") {
        return tool.to_owned();
    }
    match tool_name.strip_prefix("mcp__").and_then(|rest| rest.split_once("__")) {
        Some((server, tool)) => format!("{server} · {tool}"),
        None => tool_name.to_owned(),
    }
}

pub fn group_of(tool_kind: &str, tool_name: &str) -> ToolGroup {
    if tool_name.starts_with("mcp__lifeos__") {
        return ToolGroup::Lifeos;
    }
    match tool_kind {
        "read" => ToolGroup::Read,
        "edit" | "delete" | "move" => ToolGroup::Edit,
        "execute" => ToolGroup::Execute,
        "search" => ToolGroup::Search,
        "fetch" => ToolGroup::Fetch,
        _ => ToolGroup::Other,
    }
}

fn state_of(status: &str) -> ToolState {
    match status {
        "completed" => ToolState::Completed,
        "failed" => ToolState::Failed,
        _ => ToolState::Running,
    }
}

pub(crate) fn data_of<T: serde::de::DeserializeOwned + Default>(e: &Event) -> T {
    if e.data.is_empty() { T::default() } else { serde_json::from_str(&e.data).unwrap_or_default() }
}

pub fn activity(e: &Event) -> Option<Activity> {
    match e.kind {
        EventKind::Tool if e.status == "completed" && sent_of(e).is_some() => sent_of(e),
        EventKind::Tool => Some(Activity::Tool {
            group: group_of(&e.tool_kind, &e.tool_name),
            name: shown_name(&e.tool_name),
            summary: e.title.clone(),
            status: state_of(&e.status),
        }),
        EventKind::Stop => {
            let stop: StopData = data_of(e);
            Some(Activity::TurnEnd { ms: stop.ms, turn: stop.turn })
        }
        EventKind::System if e.status == "new_session" => {
            let data: NewSessionData = data_of(e);
            data.started_over.then_some(Activity::StartedOver)
        }
        EventKind::System if e.status == BLOCK => block_of(e).map(|block| Activity::Block { block }),
        EventKind::System if e.status == STOPPED => {
            let note: StopNote = data_of(e);
            Some(Activity::Stopped { reason: note.reason, dropped: note.dropped })
        }
        EventKind::Message => {
            let data: MarkData = data_of(e);
            (!data.marks.is_empty()).then_some(Activity::Marked { marks: data.marks })
        }
        _ => None,
    }
}

/// The block a block event holds; none for any other event.
pub fn block_of(e: &Event) -> Option<lifeos_wire::Block> {
    if e.kind != EventKind::System || e.status != BLOCK || e.data.is_empty() {
        return None;
    }
    serde_json::from_str::<BlockData>(&e.data).ok().map(|d| d.block)
}

/// A message a call sent to another agent or to the owner, once the tool took it (remit's `sent_of`).
fn sent_of(e: &Event) -> Option<Activity> {
    let data: Value = if e.data.is_empty() { Value::Null } else { serde_json::from_str(&e.data).ok()? };
    let field = |k: &str| data.pointer(&format!("/input/{k}")).and_then(Value::as_str).unwrap_or_default().trim().to_owned();
    let (to, kind, label, text) = match e.tool_name.strip_prefix("mcp__lifeos__")? {
        "ask_agent" => (field("agent"), "message", "Asked", field("question")),
        "hand_off" if data.pointer("/input/heads_up").and_then(Value::as_bool) == Some(true) => {
            (field("agent"), "message", "Told", field("task"))
        }
        "hand_off" => (field("agent"), "message", "Handed off", field("task")),
        "ask_owner" => ("owner".to_owned(), "question", "Asked you", field("question")),
        "answer_question" => (String::new(), "answer", "Answered a question", field("answer")),
        _ => return None,
    };
    (!text.is_empty()).then(|| Activity::Sent { to, kind: kind.into(), label: label.into(), text })
}

pub fn read(e: lifeos_wire::Event) -> lifeos_wire::EventRead {
    let activity = activity(&e);
    lifeos_wire::EventRead { event: e, activity }
}

/// Events of one turn `turn_summary` reads at most (remit's `TURN_EVENTS_READ`); a turn with more is summed up from the
/// first of them, and says so (`partial`).
pub const TURN_EVENTS_READ: usize = 5000;

/// What the tool calls of one turn touched, each once with its count, in the order first touched; a thing every call
/// on which failed is only a failure. A turn of more than `TURN_EVENTS_READ` events is told by its first ones.
pub fn turn_summary(events: &[Event]) -> TurnSummary {
    let mut summary = TurnSummary { partial: events.len() > TURN_EVENTS_READ, ..TurnSummary::default() };
    for e in events.iter().take(TURN_EVENTS_READ).filter(|e| e.kind == EventKind::Tool) {
        let group = group_of(&e.tool_kind, &e.tool_name);
        let name = shown_name(&e.tool_name);
        if e.status == "failed" {
            let refused = refused(e);
            summary.failures.push(TurnFailure {
                event_id: e.id,
                kind: if refused { FailureKind::Refused } else { FailureKind::Failed },
                name,
                summary: e.title.clone(),
            });
            continue;
        }
        match summary.touched.iter_mut().find(|t| t.group == group && t.name == name && t.summary == e.title) {
            Some(t) => t.count = t.count.saturating_add(1),
            None => summary.touched.push(Touched { group, name, summary: e.title.clone(), count: 1 }),
        }
    }
    summary
}

/// A call a rule refused before it ran says so in its output.
fn refused(e: &Event) -> bool {
    let data: Value = if e.data.is_empty() { Value::Null } else { serde_json::from_str(&e.data).unwrap_or(Value::Null) };
    data.get("output")
        .and_then(Value::as_str)
        .is_some_and(|o| o.contains("not allowed") || o.contains("denied") || o.contains("File tools are limited"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool(id: i64, name: &str, kind: &str, title: &str, status: &str) -> Event {
        Event {
            id,
            card: "assistant".into(),
            kind: EventKind::Tool,
            status: status.into(),
            tool_call_id: format!("t{id}"),
            tool_kind: kind.into(),
            tool_name: name.into(),
            title: title.into(),
            text: String::new(),
            by: None,
            data: String::new(),
            created: String::new(),
            updated: String::new(),
            rev: 1,
        }
    }

    #[test]
    fn a_turn_tells_each_thing_it_touched_once() {
        let events = [
            tool(1, "mcp__lifeos__add_task", "other", "Mua sữa", "completed"),
            tool(2, "Read", "read", "notes/a.md", "completed"),
            tool(3, "mcp__lifeos__add_task", "other", "Mua sữa", "completed"),
            tool(4, "Edit", "edit", "notes/b.md", "failed"),
        ];
        let s = turn_summary(&events);
        let touched: Vec<_> = s.touched.iter().map(|t| (t.group, t.name.as_str(), t.count)).collect();
        assert_eq!(touched, [(ToolGroup::Lifeos, "add_task", 2), (ToolGroup::Read, "Read", 1)]);
        assert_eq!(s.failures.len(), 1);
        assert_eq!(s.failures[0].event_id, 4);
        assert!(!s.partial);
    }

    #[test]
    fn a_turn_longer_than_is_read_is_told_by_its_first_events_and_says_so() {
        let mut events: Vec<Event> =
            (0..TURN_EVENTS_READ as i64).map(|i| tool(i, "Read", "read", "notes/a.md", "completed")).collect();
        let exactly = turn_summary(&events);
        assert!(!exactly.partial);
        events.push(tool(TURN_EVENTS_READ as i64, "Edit", "edit", "notes/b.md", "failed"));
        let s = turn_summary(&events);
        assert!(s.partial);
        assert_eq!(s.touched.len(), 1);
        assert_eq!(s.touched[0].count, TURN_EVENTS_READ as i64);
        assert!(s.failures.is_empty(), "an event past those read is not told");
    }
}
