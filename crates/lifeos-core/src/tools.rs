//! The tool surface: the one list of operations on a life.
//!
//! Each tool is declared once in `tools!` below: its access (`read` or
//! `write`), name, typed input and output, description and handler. The JSON
//! schema comes from the input type, so schema and parser cannot drift. The
//! agent (in-process MCP), the stdio MCP server and the web link all read
//! `specs()` and dispatch through `call()`.
//!
//! Only the agent writes. The web link calls `read` tools to draw the
//! dashboard; every change goes through a conversation with the agent.

use anyhow::{Context, Result};
use chrono::NaiveDate;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::life::crew::*;
use crate::life::connections::*;
use crate::life::routines::*;
use crate::life::goals::*;
use crate::life::habits::*;
use crate::life::health::*;
use crate::life::inbox::*;
use crate::life::memory::*;
use crate::life::money::*;
use crate::life::notes::*;
use crate::life::people::*;
use crate::life::tasks::*;
use crate::life::{Life, today};
use lifeos_wire::*;

#[derive(Debug, Clone)]
pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: Value,
    /// Only reads: safe to call without the user's say-so, and from the web app.
    pub read_only: bool,
}

input! {
    pub struct Day {
        /// YYYY-MM-DD. Defaults to today.
        pub date: Option<NaiveDate>,
    }
}

input! {
    pub struct DocId {
        /// Document id (file name without .md), or its path in the vault.
        pub id: String,
    }
}

input! {
    /// A row in one of lifeos's tables (a transaction, measurement, interaction, inbox item).
    pub struct RowId {
        pub id: i64,
    }
}

input! {
    pub struct Search {
        pub query: String,
        /// Defaults to 10.
        pub limit: Option<u32>,
    }
}

input! {
    pub struct ListReviews {
        /// Omit for every kind.
        pub kind: Option<ReviewKind>,
        /// Defaults to 10.
        pub limit: Option<u32>,
    }
}

input! {
    pub struct Nothing {}
}

fn schema_of<T: JsonSchema>() -> Value {
    // Inline enums instead of `$defs`/`$ref`: some MCP clients don't resolve refs.
    let generator = schemars::generate::SchemaSettings::draft2020_12()
        .with(|s| s.inline_subschemas = true)
        .into_generator();
    let mut v = serde_json::to_value(generator.into_root_schema_for::<T>()).expect("schema serializes");
    if let Some(obj) = v.as_object_mut() {
        obj.remove("$schema");
        obj.remove("title");
        obj.remove("description");
    }
    v
}

fn parse<T: for<'de> Deserialize<'de>>(name: &str, input: Value) -> Result<T> {
    // Some clients send `null` instead of `{}` for tools without arguments.
    let input = if input.is_null() { json!({}) } else { input };
    serde_json::from_value(input).with_context(|| format!("invalid input for {name}"))
}

fn on(d: Option<NaiveDate>) -> NaiveDate {
    d.unwrap_or_else(today)
}

macro_rules! access {
    (read) => {
        true
    };
    (write) => {
        false
    };
}

/// Assistant-only configuration tools. Automated turns cannot change their
/// own triggers, enqueue another run, or edit crew and connection permissions.
pub fn manages_automation(name: &str) -> bool {
    matches!(name, "add_routine" | "update_routine" | "remove_routine" | "run_routine"
        | "add_connection" | "set_connection" | "remove_connection"
        | "add_member" | "update_member" | "retire_member")
}

macro_rules! tools {
    ($( $access:ident $name:literal: $input:ty => $output:ty, $desc:literal, |$life:ident, $arg:ident $(, $who:ident)?| $body:expr; )*) => {
        pub fn specs() -> Vec<ToolSpec> {
            vec![$(ToolSpec {
                name: $name,
                description: $desc,
                input_schema: schema_of::<$input>(),
                read_only: access!($access),
            },)*]
        }

        pub fn spec(name: &str) -> Option<ToolSpec> {
            specs().into_iter().find(|s| s.name == name)
        }

        /// Runs a tool by name for `caller` (`ASSISTANT`, or a crew member
        /// id), after picking up hand edits to the vault. Errors are meant to
        /// be shown to the model.
        pub fn call(life: &mut Life, caller: &str, name: &str, input: Value) -> Result<Value> {
            call_with_origin(life, caller, name, input, false)
        }

        pub fn event_tool(name: &str) -> bool {
            match name {
                $($name => !access!($access) && !manages_automation(name),)*
                _ => false,
            }
        }

        pub fn call_with_origin(life: &mut Life, caller: &str, name: &str, input: Value, automated: bool) -> Result<Value> {
            anyhow::ensure!(!manages_automation(name) || (caller == ASSISTANT && !automated),
                "only the Assistant in a user conversation may manage automation, crew and connections");
            life.sync_index()?;
            match name {
                $($name => {
                    let $arg: $input = parse($name, input)?;
                    let $life = &*life;
                    $(let $who = caller;)?
                    let out: Result<$output> = $body;
                    let out = out?;
                    let value = serde_json::to_value(out)?;
                    let value = if value.is_null() && std::any::TypeId::of::<$output>() == std::any::TypeId::of::<()>() {
                        json!({ "ok": true })
                    } else {
                        value
                    };
                    if !automated && !access!($access) && !manages_automation(name)
                        && let Err(err) = life.record_event(caller, name, &value) {
                        // The write has already succeeded; reporting failure would invite a duplicate write.
                        tracing::warn!("could not record event for {name}: {err}");
                    }
                    Ok(value)
                })*
                other => anyhow::bail!("unknown tool '{other}'"),
            }
        }

        /// `ReadTools` for the web app: each read tool's input and output type.
        #[cfg(feature = "ts")]
        pub fn read_tools_ts() -> String {
            use ts_rs::{Config, TS};
            let cfg = Config::from_env();
            let mut entries = String::new();
            $(if access!($access) {
                entries.push_str(&format!(
                    "  {}: {{ input: {}; output: {} }};\n",
                    $name,
                    <$input as TS>::name(&cfg),
                    <$output as TS>::name(&cfg),
                ));
            })*
            // Every exported type is in its own file next to this one; import
            // the ones the entries name.
            let mut imports: Vec<&str> = entries
                .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                .filter(|w| w.starts_with(|c: char| c.is_ascii_uppercase()) && *w != "Array")
                .collect();
            imports.sort_unstable();
            imports.dedup();
            let mut out = String::from("// Generated by `make gen` from crates/lifeos-core/src/tools.rs. Do not edit.\n");
            for name in imports {
                out.push_str(&format!("import type {{ {name} }} from \"./{name}\";\n"));
            }
            out.push_str("\nexport interface ReadTools {\n");
            out.push_str(&entries);
            out.push_str("}\n\nexport type ReadTool = keyof ReadTools;\n");
            out
        }
    };
}

tools! {
    // ------------------------------------------------------------- overview
    read "dashboard": Day => Dashboard,
        "Everything at a glance for a day: the morning brief, tasks by bucket, active goals, habits, people to get in touch with, birthdays, this month's money, health trends, recent journal and notes, the inbox. Start here when the user asks what to do or how things are going.",
        |life, i| life.dashboard(on(i.date));
    read "search": Search => Vec<SearchHit>,
        "Full-text search across tasks, goals, journal, notes, people and reviews (accents optional).",
        |life, i| life.search(&i.query, i.limit.unwrap_or(10) as usize);
    read "read_doc": DocId => DocFull,
        "Read any task, goal, journal day, note, person or review in full, by id or vault path.",
        |life, i| life.read_doc(&i.id);

    // ---------------------------------------------------------------- tasks
    read "list_tasks": ListTasks => Vec<DocSummary>,
        "List tasks, soonest date first, then priority.",
        |life, i| life.list_tasks(&i);
    read "tasks_view": Day => TasksView,
        "Open tasks of a day in buckets: overdue, today (due or planned), the next 7 days, anytime; plus what was done that day.",
        |life, i| life.tasks_view(on(i.date));
    write "add_task": AddTask => DocSummary,
        "Create a task (tasks/<id>.md). Use `scheduled` for the day the user plans to do it and `due` for a real deadline. Link it to a goal when it clearly serves one.",
        |life, i| life.add_task(i);
    write "update_task": UpdateTask => DocSummary,
        "Change a task: reschedule, reprioritise, retitle, relink, add notes. Only given fields change.",
        |life, i| life.update_task(i);
    write "complete_task": DocId => DocSummary,
        "Mark a task done.",
        |life, i| life.complete_task(&i.id);
    write "reopen_task": DocId => DocSummary,
        "Mark a done task open again.",
        |life, i| life.reopen_task(&i.id);

    // ---------------------------------------------------------------- goals
    read "list_goals": ListGoals => Vec<GoalView>,
        "Goals with progress, why they matter and their linked task counts. Active first.",
        |life, i| life.goal_views(&i);
    write "add_goal": AddGoal => DocSummary,
        "Create a goal (goals/<id>.md) with its horizon and the reason behind it.",
        |life, i| life.add_goal(i);
    write "update_goal": UpdateGoal => DocSummary,
        "Change a goal: status (active, paused, done, dropped), progress 0–100, horizon, due, or add a dated line to its progress log.",
        |life, i| life.update_goal(i);

    // --------------------------------------------------------------- habits
    read "habits": ListHabits => Vec<HabitStatus>,
        "Habits with today's state, streak, days done this week against the weekly target, and the last 12 weeks day by day.",
        |life, i| life.habits(&i, today());
    write "define_habit": DefineHabit => HabitStatus,
        "Start tracking a habit, or change its unit or weekly target.",
        |life, i| life.define_habit(i);
    write "log_habit": LogHabit => HabitStatus,
        "Record that a habit was done on a day (logging the same day again overwrites; value 0 un-marks it).",
        |life, i| life.log_habit(i);
    write "archive_habit": HabitName => (),
        "Stop tracking a habit (its history stays).",
        |life, i| life.archive_habit(&i.name);

    // --------------------------------------------------------------- people
    read "list_people": ListPeople => Vec<PersonView>,
        "People in the user's life: relation, birthday, how long since last contact, who is overdue for contact.",
        |life, i| life.people(&i, today());
    read "person": DocId => PersonDetail,
        "One person in full: what lifeos remembers about them and every logged interaction.",
        |life, i| life.person_detail(&i.id);
    write "add_person": AddPerson => DocSummary,
        "Remember a person (people/<id>.md): relation, birthday, how often to be in touch, notes.",
        |life, i| life.add_person(i);
    write "update_person": UpdatePerson => DocSummary,
        "Change what lifeos knows about a person, or add a dated note about them.",
        |life, i| life.update_person(i);
    write "log_interaction": LogInteraction => Interaction,
        "Record contact with a person: met, called, messaged… and what about.",
        |life, i| life.log_interaction(i);
    write "delete_interaction": RowId => (),
        "Delete a logged interaction (to fix a mistake).",
        |life, i| life.delete_interaction(i.id);

    // ---------------------------------------------------------------- money
    read "money_month": MonthInput => MoneyMonth,
        "A month of money: totals per currency, spending by category, budgets against spending, daily spending, latest transactions.",
        |life, i| life.money_month(i.month.as_deref(), today());
    read "list_transactions": DateRange => Vec<Transaction>,
        "Transactions in a date range (default: this month), oldest first.",
        |life, i| { let (from, to) = i.resolve(); life.transactions(from, to) };
    read "finance_summary": DateRange => Vec<CurrencySummary>,
        "Income, spending, net and spending by category per currency in a date range (default: this month).",
        |life, i| { let (from, to) = i.resolve(); life.finance_summary(from, to) };
    read "list_budgets": Nothing => Vec<Budget>,
        "Monthly budgets per category.",
        |life, _i| life.budgets();
    write "add_transaction": AddTransaction => Transaction,
        "Record money in (positive) or out (negative).",
        |life, i| life.add_transaction(i);
    write "delete_transaction": RowId => (),
        "Delete a transaction (to fix a mistake).",
        |life, i| life.delete_transaction(i.id);
    write "set_budget": SetBudget => Vec<Budget>,
        "Set a monthly budget for a spending category; 0 removes it.",
        |life, i| life.set_budget(i);

    // --------------------------------------------------------------- health
    read "metric_series": SeriesInput => Vec<MetricSeries>,
        "Each health measurement over the last days (default 30): one point per day, latest and previous value.",
        |life, i| life.metric_series(i.days.unwrap_or(SERIES_DAYS).max(1), today());
    read "list_metrics": ListMetrics => Vec<Metric>,
        "Recorded measurements in a date range (default: last 30 days).",
        |life, i| life.metrics(&i);
    write "log_metric": LogMetric => Metric,
        "Record a health or life measurement: weight, sleep hours, steps, mood, energy…",
        |life, i| life.log_metric(i);
    write "delete_metric": RowId => (),
        "Delete a measurement (to fix a mistake).",
        |life, i| life.delete_metric(i.id);

    // ---------------------------------------------------- journal and notes
    read "read_journal": Day => Option<DocFull>,
        "The journal of a day, or null when nothing was written.",
        |life, i| life.read_journal(on(i.date));
    read "journal_recent": Recent => Vec<JournalExcerpt>,
        "The latest journal days with a short excerpt, newest first.",
        |life, i| life.journal_recent(i.limit.unwrap_or(10));
    read "recent_notes": Recent => Vec<DocSummary>,
        "Most recently changed notes.",
        |life, i| life.recent_notes(i.limit.unwrap_or(10));
    read "reviews": ListReviews => Vec<DocSummary>,
        "Briefs and weekly or monthly reviews, newest first.",
        |life, i| life.reviews(i.kind, i.limit.unwrap_or(10));
    write "write_journal": WriteJournal => DocSummary,
        "Append a timestamped entry to a day's journal (journal/YYYY/YYYY-MM-DD.md), in the user's voice.",
        |life, i| life.write_journal(i);
    write "save_note": SaveNote => DocSummary,
        "Save a knowledge note (notes/): an idea, a lesson, a reference, a book.",
        |life, i| life.save_note(i);
    write "save_review": SaveReview => DocSummary,
        "Save a review you wrote: the morning brief (kind brief, shown on top of the dashboard) or a weekly or monthly review. One per kind per date; saving again replaces it.",
        |life, i| life.save_review(i);

    // ---------------------------------------------------------------- inbox
    read "list_inbox": Nothing => Vec<InboxItem>,
        "Things captured but not yet sorted, oldest first.",
        |life, _i| life.inbox();
    write "capture": Capture => InboxItem,
        "Drop something into the inbox when it is not yet clear what it is.",
        |life, i| life.capture(i);
    write "resolve_inbox": RowId => (),
        "Mark an inbox item sorted, once it became a task, note, person… or was dropped.",
        |life, i| life.resolve_inbox(i.id);

    // --------------------------------------------------------------- memory
    read "recall": Recall => Vec<Memory>,
        "Search what you remember (your own memories, and for a crew member the Assistant's memories about the user too). lifeos already puts the most relevant ones in front of each message; search when you need more.",
        |life, i, who| life.recall(who, &i.query, i.limit.unwrap_or(8));
    read "list_memories": ListMemories => Vec<Memory>,
        "Every memory, newest first, or those matching words: for reviewing or tidying what is remembered.",
        |life, i| life.memories(&i);
    write "remember": Remember => Memory,
        "Remember something for future conversations: a fact about the user's life, a preference, a principle they live by, a lesson, a decision with its reason, an intent. One claim per memory; check `recall` first and pass `replaces` instead of adding a duplicate.",
        |life, i, who| life.remember(who, i);
    write "forget": Forget => (),
        "Forget a memory that is wrong or no longer holds, saying why.",
        |life, i, who| life.forget(who, i);

    // ---------------------------------------------------------------- automation
    read "list_routines": Nothing => Vec<Routine>,
        "List routines, schedules, webhook readiness, and the latest run.",
        |life, _i| life.routines();
    read "list_runs": ListRuns => Vec<Run>,
        "Read routine run history, including queued and running work.",
        |life, i| life.runs(i.routine.as_deref(), i.limit.unwrap_or(30));
    write "add_routine": AddRoutine => Routine,
        "Create a routine with a prompt for an agent. Use a five-field local cron, a single local time, write-tool events, a webhook, or manual runs. Webhook keys are set in the terminal with lifeos hook-key.",
        |life, i| life.add_routine(i);
    write "update_routine": UpdateRoutine => Routine,
        "Edit or pause a routine. Set cron or at to an empty string to clear it, or on to an empty array to clear all event triggers.",
        |life, i| life.update_routine(i);
    write "remove_routine": DocId => (),
        "Delete a routine and cancel its waiting runs. Run history stays.",
        |life, i| life.remove_routine(&i.id);
    write "run_routine": DocId => Run,
        "Queue a manual run. The LifeOS web worker runs it in the target agent's conversation.",
        |life, i| life.queue_manual(&i.id);

    // ---------------------------------------------------------------- connections
    read "list_connections": Nothing => Connections,
        "List external MCP connections, crew access, missing secret names and setup instructions. Never returns secret values.",
        |life, _i| life.connections();
    write "add_connection": AddConnection => Connection,
        "Add a built-in external MCP preset (github, gmail, x). Set credentials locally per the preset instructions. Custom MCP servers are configured in the terminal with lifeos connection set.",
        |life, i| life.add_connection(&i.preset);
    write "set_connection": SetConnection => (),
        "Enable or disable a configured MCP connection.",
        |life, i| life.enable_connection(&i.name, i.enabled);
    write "remove_connection": ConnectionName => (),
        "Remove an external MCP connection.",
        |life, i| life.remove_connection(&i.name);

    // ----------------------------------------------------------------- crew
    read "list_crew": Nothing => Vec<CrewMember>,
        "The crew: agents the user asked for, each with a role, instructions, tools and its own memory and conversation.",
        |life, _i| life.crew();
    read "crew_member": MemberId => CrewMember,
        "One crew member in full, with its instructions.",
        |life, i| life.member(&i.id);
    write "add_member": AddMember => CrewMember,
        "Add a crew member (crew/<id>.md) when the user wants a dedicated agent for part of their life, e.g. a health coach or a money adviser. Write its instructions in second person: its focus, tone, what to watch, when to speak up. Limit its tools to its area when that makes sense.",
        |life, i| life.add_member(i);
    write "update_member": UpdateMember => CrewMember,
        "Change a crew member: name, role, instructions, model, effort, tools or its list of granted connection names.",
        |life, i| life.update_member(i);
    write "retire_member": MemberId => CrewMember,
        "Retire a crew member: it stops taking messages; its file and memories stay.",
        |life, i| life.retire_member(&i.id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_schema_is_an_object_without_meta_keys() {
        let specs = specs();
        let mut names: Vec<_> = specs.iter().map(|s| s.name).collect();
        names.dedup();
        assert_eq!(names.len(), specs.len());
        for s in &specs {
            assert_eq!(s.input_schema["type"], "object", "{}", s.name);
            assert!(s.input_schema.get("$schema").is_none());
            assert!(!s.input_schema.to_string().contains("$ref"), "{}", s.name);
        }
        assert!(spec("dashboard").unwrap().read_only);
        assert!(!spec("add_task").unwrap().read_only);
    }

    #[test]
    fn call_dispatches_and_rejects_bad_input() {
        let dir = tempfile::tempdir().unwrap();
        let mut life = Life::open_with_memory_db(dir.path()).unwrap();
        let out = call(&mut life, ASSISTANT, "add_task", json!({ "title": "Gọi mẹ", "due": "2026-12-01", "priority": "high" })).unwrap();
        assert_eq!(out["id"], "goi-me");
        let list = call(&mut life, ASSISTANT, "list_tasks", Value::Null).unwrap();
        assert_eq!(list.as_array().unwrap().len(), 1);
        assert!(call(&mut life, ASSISTANT, "add_task", json!({ "title": "x", "bogus": 1 })).is_err());
        assert!(call(&mut life, ASSISTANT, "add_task", json!({ "title": "x", "priority": "urgent" })).is_err());
        assert!(call(&mut life, ASSISTANT, "nope", json!({})).is_err());
        let item = call(&mut life, ASSISTANT, "capture", json!({ "text": "ý tưởng" })).unwrap();
        assert_eq!(call(&mut life, ASSISTANT, "resolve_inbox", json!({ "id": item["id"] })).unwrap(), json!({ "ok": true }));
        let dash = call(&mut life, ASSISTANT, "dashboard", json!({})).unwrap();
        assert!(dash["tasks"].is_object());
    }

    #[cfg(feature = "ts")]
    #[test]
    fn export_read_tools_ts() {
        let dir = std::env::var("TS_RS_EXPORT_DIR").unwrap_or_else(|_| "bindings".into());
        let ts = read_tools_ts();
        assert!(ts.contains("dashboard: { input: Day; output: Dashboard }"), "{ts}");
        assert!(!ts.contains("add_task"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(std::path::Path::new(&dir).join("tools.ts"), ts).unwrap();
    }
}
