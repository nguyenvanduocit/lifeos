//! What lifeos shows: the records tools return and pages draw. Dates are
//! `YYYY-MM-DD` strings so every side reads them the same way.

use serde::{Deserialize, Serialize};
#[cfg(feature = "ts")]
use ts_rs::TS;

/// What a document is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub enum Kind {
    Task,
    Goal,
    Journal,
    Note,
}

impl Kind {
    pub const ALL: [Kind; 4] = [Kind::Task, Kind::Goal, Kind::Journal, Kind::Note];

    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Task => "task",
            Kind::Goal => "goal",
            Kind::Journal => "journal",
            Kind::Note => "note",
        }
    }

    pub fn parse(s: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.as_str() == s)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct DocSummary {
    /// File stem: the handle tools take.
    pub id: String,
    pub kind: Kind,
    pub title: String,
    pub status: Option<String>,
    pub due: Option<String>,
    pub goal: Option<String>,
    pub horizon: Option<String>,
    /// Relative to the vault root.
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct DocFull {
    pub summary: DocSummary,
    pub tags: Vec<String>,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct HabitStatus {
    pub habit: String,
    pub done_today: bool,
    pub streak: u32,
    pub days_done_in_window: u32,
    pub window_days: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct Transaction {
    // The page gets plain JSON numbers (serde-wasm-bindgen `json_compatible`).
    #[cfg_attr(feature = "ts", ts(type = "number"))]
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
pub struct Metric {
    pub day: String,
    pub name: String,
    pub value: f64,
    pub unit: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct Today {
    pub date: String,
    pub weekday: String,
    pub overdue_tasks: Vec<DocSummary>,
    pub due_today: Vec<DocSummary>,
    pub open_tasks_without_due: u32,
    pub active_goals: Vec<DocSummary>,
    pub habits: Vec<HabitStatus>,
    pub money_today: Vec<CurrencySummary>,
    pub journal_written: bool,
}
