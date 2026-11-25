//! The tool surface: the one list of operations exposed to models.
//!
//! Each tool is declared once in `tools!` below — its name, description, typed
//! input and handler. The JSON schema comes from the input type, so schema
//! and parser cannot drift. The agent (in-process MCP), the stdio MCP server
//! and the web API all read `specs()` and dispatch through `call()`.

use anyhow::{Context, Result};
use chrono::{Datelike, Duration, NaiveDate};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::life::{Life, today};

#[derive(Debug, Clone)]
pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: Value,
    /// Only reads; safe to call without the user's say-so.
    pub read_only: bool,
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
    }
    v
}

fn parse<T: for<'de> Deserialize<'de>>(name: &str, input: Value) -> Result<T> {
    // Some clients send `null` instead of `{}` for tools without arguments.
    let input = if input.is_null() { json!({}) } else { input };
    serde_json::from_value(input).with_context(|| format!("invalid input for {name}"))
}

macro_rules! access {
    (read) => { true };
    (write) => { false };
}

macro_rules! tools {
    ($( $access:ident $name:literal => $input:ty, $desc:literal, |$life:ident, $arg:ident| $body:expr; )*) => {
        pub fn specs() -> Vec<ToolSpec> {
            vec![$(ToolSpec {
                name: $name,
                description: $desc,
                input_schema: schema_of::<$input>(),
                read_only: access!($access),
            },)*]
        }

        /// Runs a tool by name. Errors are meant to be shown to the model.
        pub fn call(life: &mut Life, name: &str, input: Value) -> Result<Value> {
            match name {
                $($name => {
                    let $arg: $input = parse($name, input)?;
                    let $life = life;
                    Ok(serde_json::to_value($body?)?)
                })*
                other => anyhow::bail!("unknown tool '{other}'"),
            }
        }
    };
}

fn day_or_today(d: Option<NaiveDate>) -> NaiveDate {
    d.unwrap_or_else(today)
}

fn month_start(d: NaiveDate) -> NaiveDate {
    d.with_day(1).expect("day 1 exists")
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DateInput {
    /// YYYY-MM-DD. Defaults to today.
    pub date: Option<NaiveDate>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddTask {
    pub title: String,
    /// Due date, YYYY-MM-DD.
    pub due: Option<NaiveDate>,
    /// Id of the goal this task serves (from list_goals).
    pub goal: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Extra details, Markdown.
    pub notes: Option<String>,
}

#[derive(Deserialize, JsonSchema, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum TaskFilter {
    Open,
    Done,
    All,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListTasks {
    /// Defaults to open.
    pub status: Option<TaskFilter>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IdInput {
    pub id: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddGoal {
    pub title: String,
    /// Time frame in words, e.g. "2026", "Q1 2027", "5 năm".
    pub horizon: Option<String>,
    /// Target date, YYYY-MM-DD.
    pub due: Option<NaiveDate>,
    /// Why this goal matters — kept in the goal file.
    pub why: Option<String>,
}

#[derive(Deserialize, JsonSchema, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum GoalStatus {
    Active,
    Paused,
    Done,
    Dropped,
}

impl GoalStatus {
    fn as_str(self) -> &'static str {
        match self {
            GoalStatus::Active => "active",
            GoalStatus::Paused => "paused",
            GoalStatus::Done => "done",
            GoalStatus::Dropped => "dropped",
        }
    }
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListGoals {
    /// Omit for all goals.
    pub status: Option<GoalStatus>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetGoalStatus {
    pub id: String,
    pub status: GoalStatus,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WriteJournal {
    /// The entry, in the user's own words where possible. Markdown.
    pub text: String,
    /// YYYY-MM-DD. Defaults to today.
    pub date: Option<NaiveDate>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SaveNote {
    pub title: String,
    /// Markdown.
    pub body: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Search {
    /// Words to look for; accents are optional ("ca phe" finds "cà phê").
    pub query: String,
    /// Defaults to 10.
    pub limit: Option<u32>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LogHabit {
    /// Short stable name, e.g. "thiền", "chạy bộ", "đọc sách".
    pub habit: String,
    /// YYYY-MM-DD. Defaults to today.
    pub date: Option<NaiveDate>,
    /// Amount done (minutes, pages, km…). Defaults to 1. 0 un-marks the day.
    pub value: Option<f64>,
    pub note: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Habits {
    /// Days to count back from today. Defaults to 7.
    pub window_days: Option<u32>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddTransaction {
    /// Signed: negative for money out (spending), positive for money in.
    pub amount: f64,
    /// ISO code, e.g. VND, USD.
    pub currency: String,
    /// Short lowercase category, e.g. "ăn uống", "đi lại", "lương".
    pub category: String,
    pub note: Option<String>,
    /// YYYY-MM-DD. Defaults to today.
    pub date: Option<NaiveDate>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DateRange {
    /// YYYY-MM-DD, inclusive. Defaults to the first day of this month.
    pub from: Option<NaiveDate>,
    /// YYYY-MM-DD, inclusive. Defaults to today.
    pub to: Option<NaiveDate>,
}

impl DateRange {
    fn resolve(&self) -> (NaiveDate, NaiveDate) {
        let to = day_or_today(self.to);
        (self.from.unwrap_or_else(|| month_start(to)), to)
    }
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DeleteTransaction {
    pub id: i64,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LogMetric {
    /// Lowercase name, e.g. "cân nặng", "giấc ngủ", "năng lượng".
    pub name: String,
    pub value: f64,
    /// e.g. "kg", "giờ", "1-10".
    pub unit: Option<String>,
    pub note: Option<String>,
    /// YYYY-MM-DD. Defaults to today.
    pub date: Option<NaiveDate>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListMetrics {
    /// Omit for every metric.
    pub name: Option<String>,
    /// YYYY-MM-DD. Defaults to 30 days ago.
    pub from: Option<NaiveDate>,
    /// YYYY-MM-DD. Defaults to today.
    pub to: Option<NaiveDate>,
}

tools! {
    read "today" => DateInput,
        "Overview of a day: overdue and due tasks, active goals, habit streaks, money moved, whether the journal is written. Start here when the user asks what to do or how things are going.",
        |life, i| life.today(day_or_today(i.date));

    write "add_task" => AddTask,
        "Create a task (a Markdown file in tasks/). Link it to a goal when it clearly serves one.",
        |life, i| life.add_task(&i.title, i.due, i.goal, i.tags, i.notes);
    read "list_tasks" => ListTasks,
        "List tasks, sorted by due date.",
        |life, i| life.list_tasks(match i.status.unwrap_or(TaskFilter::Open) {
            TaskFilter::Open => Some("open"),
            TaskFilter::Done => Some("done"),
            TaskFilter::All => None,
        });
    write "complete_task" => IdInput,
        "Mark a task done by its id.",
        |life, i| life.complete_task(&i.id);

    write "add_goal" => AddGoal,
        "Create a goal (a Markdown file in goals/) with its horizon and the reason behind it.",
        |life, i| life.add_goal(&i.title, i.horizon, i.due, i.why);
    read "list_goals" => ListGoals,
        "List goals.",
        |life, i| life.list_goals(i.status.map(GoalStatus::as_str));
    write "set_goal_status" => SetGoalStatus,
        "Change a goal's status: active, paused, done or dropped.",
        |life, i| life.set_goal_status(&i.id, i.status.as_str());

    write "write_journal" => WriteJournal,
        "Append a timestamped entry to the journal file of a day (journal/YYYY/YYYY-MM-DD.md).",
        |life, i| life.journal_append(day_or_today(i.date), &i.text);
    read "read_journal" => DateInput,
        "Read the journal of a day. Returns null when nothing was written.",
        |life, i| life.journal_read(day_or_today(i.date));

    write "save_note" => SaveNote,
        "Save a knowledge note (notes/): an idea, a lesson, something about a person, a reference.",
        |life, i| life.save_note(&i.title, &i.body, i.tags);
    read "read_doc" => IdInput,
        "Read any task, goal, journal day or note in full, by id or vault-relative path.",
        |life, i| life.read_doc(&i.id);
    read "search" => Search,
        "Full-text search across tasks, goals, journal and notes.",
        |life, i| life.search(&i.query, i.limit.unwrap_or(10) as usize);

    write "log_habit" => LogHabit,
        "Record that a habit was done on a day (one record per habit per day; logging again overwrites). Returns the streak.",
        |life, i| life.log_habit(&i.habit, day_or_today(i.date), i.value.unwrap_or(1.0), i.note);
    read "habits" => Habits,
        "Every habit ever logged, with today's state, current streak and count in the window.",
        |life, i| life.habits(i.window_days.unwrap_or(7).max(1));

    write "add_transaction" => AddTransaction,
        "Record money in or out.",
        |life, i| life.add_transaction(day_or_today(i.date), i.amount, &i.currency, &i.category, i.note);
    read "list_transactions" => DateRange,
        "List transactions in a date range (default: this month).",
        |life, i| { let (from, to) = i.resolve(); life.transactions(from, to) };
    write "delete_transaction" => DeleteTransaction,
        "Delete a transaction by id (to fix a mistake).",
        |life, i| life.delete_transaction(i.id).map(|()| json!({ "deleted": i.id }));
    read "finance_summary" => DateRange,
        "Income, spending, net and spending by category per currency, in a date range (default: this month).",
        |life, i| { let (from, to) = i.resolve(); life.finance_summary(from, to) };

    write "log_metric" => LogMetric,
        "Record a health or life measurement: weight, sleep hours, energy, mood, steps…",
        |life, i| life.log_metric(day_or_today(i.date), &i.name, i.value, i.unit, i.note);
    read "list_metrics" => ListMetrics,
        "List recorded measurements in a date range (default: last 30 days).",
        |life, i| {
            let to = day_or_today(i.to);
            life.metrics(i.name.as_deref(), i.from.unwrap_or(to - Duration::days(30)), to)
        };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_schema_is_an_object_without_meta_keys() {
        let specs = specs();
        assert_eq!(specs.len(), 20);
        for s in &specs {
            assert_eq!(s.input_schema["type"], "object", "{}", s.name);
            assert!(s.input_schema.get("$schema").is_none());
            assert!(!s.input_schema.to_string().contains("$ref"), "{}", s.name);
        }
    }

    #[test]
    fn call_dispatches_and_rejects_bad_input() {
        let dir = tempfile::tempdir().unwrap();
        let mut life = Life::open_with_memory_db(dir.path()).unwrap();
        let out = call(&mut life, "add_task", json!({ "title": "Gọi mẹ", "due": "2026-12-01" })).unwrap();
        assert_eq!(out["id"], "goi-me");
        let list = call(&mut life, "list_tasks", Value::Null).unwrap();
        assert_eq!(list.as_array().unwrap().len(), 1);
        assert!(call(&mut life, "add_task", json!({ "title": "x", "bogus": 1 })).is_err());
        assert!(call(&mut life, "nope", json!({})).is_err());
    }
}
