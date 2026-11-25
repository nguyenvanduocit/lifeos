//! What lifeos shows: the records tools return and pages draw. Dates are
//! `YYYY-MM-DD` strings so every side reads them the same way.

use serde::{Deserialize, Serialize};
#[cfg(feature = "ts")]
use ts_rs::TS;

/// Days of history a habit carries for its heatmap.
pub const HABIT_RECENT_DAYS: usize = 84;

/// What a document is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub enum Kind {
    Task,
    Goal,
    Person,
    Journal,
    Note,
    /// A brief or a review the agent wrote.
    Review,
    /// A crew member: an agent with its own role and instructions.
    Crew,
    /// A routine: words an agent acts on at a time, on a webhook, or after a tool.
    Routine,
}

impl Kind {
    pub const ALL: [Kind; 8] = [
        Kind::Task,
        Kind::Goal,
        Kind::Person,
        Kind::Journal,
        Kind::Note,
        Kind::Review,
        Kind::Crew,
        Kind::Routine,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Task => "task",
            Kind::Goal => "goal",
            Kind::Person => "person",
            Kind::Journal => "journal",
            Kind::Note => "note",
            Kind::Review => "review",
            Kind::Crew => "crew",
            Kind::Routine => "routine",
        }
    }

    pub fn parse(s: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.as_str() == s)
    }
}

// ------------------------------------------------------------------ documents

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct DocSummary {
    /// File stem: the handle tools take.
    pub id: String,
    pub kind: Kind,
    pub title: String,
    pub status: Option<String>,
    pub due: Option<String>,
    /// Task: the day it is planned for (not a deadline).
    pub scheduled: Option<String>,
    /// Task: "high", "normal" or "low".
    pub priority: Option<String>,
    pub goal: Option<String>,
    pub horizon: Option<String>,
    /// Goal: 0–100.
    pub progress: Option<u8>,
    pub tags: Vec<String>,
    /// Relative to the vault root.
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct DocFull {
    pub summary: DocSummary,
    /// Markdown, without the frontmatter.
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct SearchHit {
    pub id: String,
    pub kind: Kind,
    pub title: String,
    pub path: String,
    pub snippet: String,
}

// ---------------------------------------------------------------------- tasks

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct TasksView {
    pub overdue: Vec<DocSummary>,
    /// Due or scheduled today.
    pub today: Vec<DocSummary>,
    /// Due or scheduled in the next 7 days.
    pub upcoming: Vec<DocSummary>,
    /// Open, with neither a due nor a scheduled date.
    pub anytime: Vec<DocSummary>,
    pub done_today: Vec<DocSummary>,
}

// ---------------------------------------------------------------------- goals

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct GoalView {
    pub summary: DocSummary,
    /// The goal's "why", first paragraph.
    pub why: String,
    pub open_tasks: u32,
    pub done_tasks: u32,
}

// --------------------------------------------------------------------- habits

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct HabitStatus {
    #[serde(default)]
    pub id: Option<String>,
    pub habit: String,
    /// What `value` counts (minutes, pages, km…); none for done/not done.
    pub unit: Option<String>,
    /// Days a week the user aims for.
    pub target_per_week: u32,
    pub done_today: bool,
    pub streak: u32,
    /// Days done since Monday.
    pub done_this_week: u32,
    /// The last `HABIT_RECENT_DAYS` days, oldest first: the value logged, 0 when not done.
    pub recent: Vec<f64>,
}

// --------------------------------------------------------------------- people

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct PersonView {
    pub id: String,
    pub name: String,
    /// e.g. "gia đình", "bạn", "đồng nghiệp".
    pub relation: Option<String>,
    /// `YYYY-MM-DD`, or `MM-DD` when the year is unknown.
    pub birthday: Option<String>,
    pub days_to_birthday: Option<u32>,
    /// How often the user wants to be in touch.
    pub contact_every_days: Option<u32>,
    pub last_contact: Option<String>,
    pub days_since_contact: Option<u32>,
    /// Past `contact_every_days` without contact.
    pub overdue: bool,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct Interaction {
    pub id: i64,
    pub person: String,
    pub day: String,
    /// e.g. "gặp", "gọi", "nhắn".
    pub channel: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct PersonDetail {
    pub person: PersonView,
    /// Markdown notes about them.
    pub body: String,
    /// Newest first.
    pub interactions: Vec<Interaction>,
}

// ---------------------------------------------------------------------- money

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct Transaction {
    pub id: i64,
    pub day: String,
    /// Signed: negative is money out.
    pub amount: f64,
    pub currency: String,
    pub category: String,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct CategoryAmount {
    pub category: String,
    pub amount: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct CurrencySummary {
    pub currency: String,
    pub income: f64,
    pub spending: f64,
    pub net: f64,
    /// Spending per category (positive amounts), largest first.
    pub spending_by_category: Vec<CategoryAmount>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct Budget {
    pub category: String,
    pub currency: String,
    /// Per month, positive.
    pub monthly: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct BudgetLine {
    pub category: String,
    pub currency: String,
    pub budget: f64,
    pub spent: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct DayAmount {
    pub day: String,
    pub amount: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct MoneyMonth {
    /// `YYYY-MM`.
    pub month: String,
    pub currencies: Vec<CurrencySummary>,
    pub budgets: Vec<BudgetLine>,
    /// The currency with the most transactions this month.
    pub primary_currency: Option<String>,
    /// Spending in `primary_currency` for every day of the month so far.
    pub daily_spending: Vec<DayAmount>,
    /// Newest first.
    pub recent: Vec<Transaction>,
}

// --------------------------------------------------------------------- health

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct Metric {
    pub id: i64,
    pub day: String,
    pub name: String,
    pub value: f64,
    pub unit: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct MetricPoint {
    pub day: String,
    pub value: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct MetricSeries {
    pub name: String,
    pub unit: Option<String>,
    /// One point per day (the day's last value), oldest first.
    pub points: Vec<MetricPoint>,
    pub latest: Option<f64>,
    /// The value before `latest`, to show the change.
    pub previous: Option<f64>,
}

// ------------------------------------------------------------ journal, inbox

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct JournalExcerpt {
    pub day: String,
    pub excerpt: String,
    pub words: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct InboxItem {
    pub id: i64,
    /// RFC 3339.
    pub created: String,
    pub text: String,
}

// ------------------------------------------------------------------ dashboard

/// Everything the overview shows, in one read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct Dashboard {
    pub date: String,
    pub weekday: String,
    /// Today's brief, when the agent wrote one.
    pub brief: Option<DocFull>,
    pub tasks: TasksView,
    pub goals: Vec<GoalView>,
    pub habits: Vec<HabitStatus>,
    /// Overdue for contact, most overdue first.
    pub people_due: Vec<PersonView>,
    /// Birthdays in the next 30 days, soonest first.
    pub birthdays: Vec<PersonView>,
    pub money: MoneyMonth,
    /// Last 30 days.
    pub health: Vec<MetricSeries>,
    /// Last 7 entries, newest first.
    pub journal: Vec<JournalExcerpt>,
    pub journal_written_today: bool,
    /// Last 6 notes changed.
    pub notes: Vec<DocSummary>,
    pub inbox: Vec<InboxItem>,
}

// ----------------------------------------------------------------------- crew

/// An agent the user asked for, with its own role, instructions, memory and
/// conversation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct CrewMember {
    pub id: String,
    pub name: String,
    pub role: Option<String>,
    /// Added to its system prompt, Markdown.
    pub instructions: String,
    pub model: Option<String>,
    pub effort: Option<String>,
    /// Tools it may use; empty means every lifeos tool.
    pub tools: Vec<String>,
    /// Connections (outside MCP servers) it may use; the Assistant has them all.
    pub connections: Vec<String>,
    /// active | retired
    pub status: String,
    /// How many things it remembers.
    pub memories: u32,
}

// --------------------------------------------------------------------- memory

/// Something an agent remembers across conversations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct Memory {
    pub id: i64,
    /// `assistant`, or the crew member who holds it.
    pub holder: String,
    /// fact | preference | principle | lesson | decision | intent
    pub kind: String,
    pub title: String,
    pub body: String,
    /// `YYYY-MM-DD HH:MM`.
    pub created: String,
    pub updated: String,
    /// Set once forgotten (or replaced), with why.
    pub ended: Option<String>,
    pub why_ended: Option<String>,
    /// How many times it was brought back into a conversation.
    pub recalled: u32,
    /// `auto` (an agent wrote it) or `confirmed` (the owner stood behind it).
    pub trust: String,
    /// What agents said of it when it was given: it helped, or it misled.
    pub helped: u32,
    pub misled: u32,
    /// When a conversation was last given it, `YYYY-MM-DD HH:MM`.
    pub last_recalled: Option<String>,
    /// Its links to other memories: what it supersedes, rests on, relates to or contradicts.
    pub links: Vec<MemoryLink>,
    /// The teaching the librarian learned it from; none for one written before the librarian.
    pub source: Option<i64>,
    /// What the teaching said that it rests on, quoted.
    pub evidence: String,
}

/// Something the owner taught an agent, which the librarian reads and writes memory from (remit's ADR-0117).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct Teaching {
    pub id: i64,
    /// Whose memory it teaches: `assistant` or a crew member's id.
    pub holder: String,
    /// The card whose conversation it is, when it reads one.
    pub card: String,
    /// `teaching`: what the owner said to an agent in a turn; `review`: memories lifeos found to check again.
    pub role: String,
    /// What it came from, in words.
    pub source: String,
    /// Who taught it: `owner`, or empty for lifeos.
    pub teacher: String,
    /// The range of the card's stream it reads; both 0 for a text.
    pub from_event: i64,
    pub to_event: i64,
    /// What it holds when it reads no stream.
    pub text: String,
    /// `open` until the librarian settles it.
    pub status: String,
    /// The librarian's note on settling: which memories, or why none.
    pub note: String,
    /// The memories it wrote.
    pub written: Vec<i64>,
    pub created: String,
    pub settled: String,
}

/// One memory's link to another.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct MemoryLink {
    pub to: i64,
    /// supersedes | because | related | contradicts
    pub kind: String,
}

// ------------------------------------------------------------------- routines

/// Words an agent acts on by itself: on a schedule, once at a time, when a
/// webhook is posted, or after a lifeos tool ran.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct Routine {
    pub id: String,
    pub title: String,
    /// `assistant` or a crew member id: who does it, in its own conversation.
    pub agent: String,
    /// The script in `workflows/` it runs in place of asking its agent.
    pub workflow: Option<String>,
    /// What to do each time, Markdown.
    pub instructions: String,
    /// Repeats: five cron fields, local time.
    pub cron: Option<String>,
    /// Once: `YYYY-MM-DD HH:MM`, local time.
    pub at: Option<String>,
    /// Runs after any of these lifeos write tools (not when a routine ran them).
    pub on: Vec<String>,
    /// Runs when `POST /hooks/<id>` is sent with its key.
    pub hook: bool,
    /// A webhook key has been set locally.
    pub hook_ready: bool,
    /// active | paused | done (a one-time routine that ran)
    pub status: String,
    /// When the schedule fires next, `YYYY-MM-DD HH:MM`.
    pub next: Option<String>,
    pub last: Option<Run>,
    pub runs: u32,
}

/// One time a routine ran (or waits to).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct Run {
    pub id: i64,
    pub routine: String,
    pub title: String,
    pub agent: String,
    /// schedule | hook | event | manual
    pub trigger: String,
    /// What set it off, in a line: the tool and its subject, the webhook's first words.
    pub detail: Option<String>,
    /// `YYYY-MM-DD HH:MM:SS`.
    pub queued: String,
    pub started: Option<String>,
    pub finished: Option<String>,
    pub ms: Option<u64>,
    pub error: Option<String>,
    /// The agent's reply, shortened.
    pub reply: Option<String>,
}

// ---------------------------------------------------------------- connections

/// An outside MCP server (GitHub, X…), or lifeos's own Google app, that lifeos's agents can use.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct Connection {
    /// Its tools reach the agent as `mcp__<name>__<tool>`.
    pub name: String,
    /// The preset it came from, if any.
    pub preset: Option<String>,
    /// http | stdio | built-in
    pub transport: String,
    /// The URL, the command line, or the account a built-in app is signed in to.
    pub target: String,
    pub enabled: bool,
    /// Every Vault key it names (`${NAME}`), saved or not.
    pub secrets: Vec<String>,
    /// Keys it names that the Vault does not hold yet: it stays off until it does.
    pub missing: Vec<String>,
    /// Crew members allowed to use it (the Assistant always is).
    pub members: Vec<String>,
}

/// A connection lifeos knows how to set up.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct Preset {
    pub id: String,
    pub name: String,
    pub description: String,
    /// What the user does once on this machine, Markdown.
    pub setup: String,
    /// Vault keys it needs (`${NAME}` in its configuration).
    pub secrets: Vec<String>,
    /// Already added.
    pub added: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct Connections {
    pub connections: Vec<Connection>,
    pub presets: Vec<Preset>,
}

// ---------------------------------------------------------------- vault

/// What a Vault item is (after agentes.chat's ADR 0144): a website's login, a key a connection sends, or contact
/// details agents type in forms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub enum VaultKind {
    Login,
    Key,
    Contact,
}

/// A contact's details. Not a secret: the agents it is shared with read them.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct VaultContact {
    pub name: String,
    pub company: String,
    pub tax_id: String,
    pub email: String,
    pub phone: String,
    pub address: String,
}

/// One thing in the owner's Vault, as anyone sees it: never the password or the key itself. A saved value goes back
/// to no one, the owner included; only lifeos uses it, where an agent the item is shared with needs it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct VaultItem {
    pub id: i64,
    pub kind: VaultKind,
    pub title: String,
    pub description: String,
    /// Login: the website (`https://host`).
    pub website: Option<String>,
    /// Login: the account's username.
    pub username: Option<String>,
    /// Key: the name a connection uses it by (`${GITHUB_TOKEN}`), `UPPER_SNAKE`.
    pub key_name: Option<String>,
    /// Key: its last four characters, to tell keys apart.
    pub key_hint: Option<String>,
    /// A password or a key is saved (logins and keys).
    pub has_secret: bool,
    /// Contact: the details.
    pub contact: Option<VaultContact>,
    /// Every agent may use it, those added later too.
    pub all_agents: bool,
    /// Otherwise the agents (cards) that may: `assistant`, crew member ids.
    pub agents: Vec<String>,
    /// Connections that name it (keys).
    pub used_in: Vec<String>,
    pub last_used: Option<String>,
    pub created: String,
    pub updated: String,
}

/// One use of a Vault item, never its value: a key given to an agent's connection (`key`), a contact an agent read
/// (`read`), or the owner's `create`, `edit`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct VaultUse {
    pub item: i64,
    pub action: String,
    /// The agent's card, for an agent's use.
    pub agent: Option<String>,
    /// Where: the connection a key went to.
    pub detail: String,
    pub at: String,
}

/// The Vault as the owner's Settings page reads it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct VaultView {
    pub items: Vec<VaultItem>,
    /// The latest uses, newest first.
    pub uses: Vec<VaultUse>,
}

/// Public account state. Authentication material stays outside the vault.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct AgentAccount {
    pub id: String,
    pub label: String,
    /// default | login | connection
    pub kind: String,
    /// unchecked | ready | near | limited | access | checking
    pub status: String,
    pub reason: Option<String>,
    pub quota: Option<String>,
    pub utilization: Option<f64>,
    /// UNIX seconds, when reported by Claude Code.
    pub resets_at: Option<i64>,
    /// Earliest automatic retry when the provider gave no reset time.
    pub retry_at: Option<i64>,
}

/// A private sign-in flow, shown to the owner only on the account screen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct AccountSignin {
    pub id: String,
    pub url: Option<String>,
    /// waiting | complete | failed
    pub status: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct AgentAccounts {
    pub accounts: Vec<AgentAccount>,
    pub signin: Option<AccountSignin>,
}

// --------------------------------------------------------------------- google

/// lifeos's Google app as the owner's Google page shows it. Tokens never leave the server.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct GoogleView {
    /// The OAuth client the owner pasted, if any.
    pub client: Option<GoogleClient>,
    /// The Google account signed in, if any.
    pub account: Option<GoogleAccount>,
    /// A sign-in in progress, or the one that just failed.
    pub signin: Option<GoogleSignin>,
    /// Where Google sends the browser back after sign-in.
    pub redirect: String,
    /// Crew members allowed to use Google (the Assistant always is).
    pub members: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct GoogleClient {
    pub client_id: String,
    /// desktop (sign-in comes back to loopback only) | web (comes back to the redirect URIs registered for it).
    pub kind: String,
    pub project: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct GoogleAccount {
    pub email: String,
    /// The services granted: gmail | calendar | drive | contacts.
    pub services: Vec<String>,
    /// RFC 3339.
    pub since: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct GoogleSignin {
    /// Google's consent page.
    pub url: String,
    /// waiting | failed
    pub status: String,
    pub error: Option<String>,
}

// ------------------------------------------------------------------ workflows

/// The workflow scripts in the vault and the newest runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct Workflows {
    /// Script names: `workflows/<name>.js`.
    pub scripts: Vec<String>,
    pub runs: Vec<WorkflowRun>,
}

/// One run of a workflow script (remit's ADR-0073): a script re-evaluated against the journal of its steps' results.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct WorkflowRun {
    pub id: i64,
    /// The script's name: its file in `workflows/`, or its meta's name for one given inline.
    pub name: String,
    pub description: String,
    /// running | waiting | done | failed | stopped
    pub status: String,
    /// What the script returned, as JSON text; empty until it did.
    pub result: String,
    pub error: String,
    /// Who started it, in words.
    pub started_by: String,
    pub created: String,
    pub finished: String,
    pub steps: Vec<WorkflowStep>,
}

/// One call a run's script made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct WorkflowStep {
    pub seq: i64,
    /// agent | wait | ask_owner | log | phase | …
    pub verb: String,
    pub label: String,
    pub phase: String,
    /// Its input, as JSON text.
    pub input: String,
    /// pending | done | failed
    pub status: String,
    /// Its result, as JSON text; empty while pending.
    pub result: String,
    pub error: String,
    pub created: String,
    pub finished: String,
}
