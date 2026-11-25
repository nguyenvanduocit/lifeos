//! Goals: `goals/<id>.md`, with a "why", a progress log and a 0–100 progress.

use anyhow::{Result, bail};
use chrono::{Local, NaiveDate};
use rusqlite::params;

use super::{Life, today};
use crate::vault::{Frontmatter, Kind};
use lifeos_wire::{DocSummary, GoalView};

choice! {
    pub enum GoalStatus {
        Active => "active",
        Paused => "paused",
        Done => "done",
        Dropped => "dropped",
    }
}

input! {
    pub struct AddGoal {
        pub title: String,
        /// Time frame in words, e.g. "2026", "Q1 2027", "5 năm".
        pub horizon: Option<String>,
        /// Target date, YYYY-MM-DD.
        pub due: Option<NaiveDate>,
        /// Why this goal matters to the user.
        pub why: Option<String>,
    }
}

input! {
    /// Only the fields given change.
    pub struct UpdateGoal {
        pub id: String,
        pub title: Option<String>,
        pub status: Option<GoalStatus>,
        /// 0–100.
        pub progress: Option<u8>,
        pub horizon: Option<String>,
        pub due: Option<NaiveDate>,
        /// A dated line added to the goal's progress log.
        pub progress_note: Option<String>,
    }
}

input! {
    pub struct ListGoals {
        /// Omit for every goal.
        pub status: Option<GoalStatus>,
    }
}

const PROGRESS_HEADING: &str = "## Tiến độ";

impl Life {
    pub fn add_goal(&self, g: AddGoal) -> Result<DocSummary> {
        let meta = Frontmatter {
            title: g.title.trim().to_string(),
            status: Some("active".into()),
            horizon: g.horizon,
            due: g.due,
            progress: Some(0),
            created: Some(Local::now()),
            ..Default::default()
        };
        let why = g.why.unwrap_or_default();
        let body = format!("## Vì sao\n\n{why}\n\n{PROGRESS_HEADING}\n\n");
        Ok(super::summary(&self.create(Kind::Goal, meta, body)?))
    }

    pub fn update_goal(&self, u: UpdateGoal) -> Result<DocSummary> {
        let mut doc = self.find(Kind::Goal, &u.id)?;
        if let Some(title) = u.title {
            doc.meta.title = title.trim().to_string();
        }
        if let Some(status) = u.status {
            doc.meta.status = Some(status.as_str().into());
            if status == GoalStatus::Done {
                doc.meta.completed = Some(today());
                doc.meta.progress = Some(100);
            }
        }
        if let Some(p) = u.progress {
            if p > 100 {
                bail!("progress is 0–100");
            }
            doc.meta.progress = Some(p);
        }
        if u.horizon.is_some() {
            doc.meta.horizon = u.horizon;
        }
        if u.due.is_some() {
            doc.meta.due = u.due;
        }
        if let Some(note) = u.progress_note {
            let line = format!("- {}: {}", today(), note.trim());
            if !doc.body.contains(PROGRESS_HEADING) {
                doc.body.push_str(&format!("\n{PROGRESS_HEADING}\n\n"));
            }
            if !doc.body.ends_with('\n') {
                doc.body.push('\n');
            }
            doc.body.push_str(&line);
            doc.body.push('\n');
        }
        self.save(&doc)?;
        Ok(super::summary(&doc))
    }

    pub fn list_goals(&self, q: &ListGoals) -> Result<Vec<DocSummary>> {
        match q.status {
            Some(s) => self.docs_where(Kind::Goal, &format!("status = '{}'", s.as_str()), "due IS NULL, due, title"),
            None => self.docs_where(Kind::Goal, "1", "status <> 'active', due IS NULL, due, title"),
        }
    }

    pub fn goal_views(&self, q: &ListGoals) -> Result<Vec<GoalView>> {
        self.list_goals(q)?
            .into_iter()
            .map(|summary| {
                let (open, done): (u32, u32) = self.conn.query_row(
                    "SELECT COALESCE(SUM(status = 'open'), 0), COALESCE(SUM(status = 'done'), 0)
                     FROM docs WHERE kind = 'task' AND goal = ?1",
                    params![summary.id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )?;
                let why = self
                    .find(Kind::Goal, &summary.id)
                    .map(|doc| first_paragraph_after(&doc.body, "## Vì sao"))
                    .unwrap_or_default();
                Ok(GoalView { summary, why, open_tasks: open, done_tasks: done })
            })
            .collect()
    }
}

/// The first non-empty paragraph under `heading`, or of the body when absent.
fn first_paragraph_after(body: &str, heading: &str) -> String {
    let rest = body.find(heading).map(|i| &body[i + heading.len()..]).unwrap_or(body);
    rest.split("\n\n")
        .map(str::trim)
        .find(|p| !p.is_empty() && !p.starts_with('#'))
        .unwrap_or_default()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::life::tasks::AddTask;
    use crate::life::test_support::*;

    #[test]
    fn goal_progress_and_linked_tasks() {
        let (_dir, life) = life();
        let g = life
            .add_goal(AddGoal {
                title: "Khoẻ mạnh hơn".into(),
                horizon: Some("2026".into()),
                why: Some("Đủ sức chơi với con.".into()),
                ..Default::default()
            })
            .unwrap();
        for title in ["Khám sức khoẻ", "Mua giày chạy"] {
            life.add_task(AddTask { title: title.into(), goal: Some(g.id.clone()), ..Default::default() })
                .unwrap();
        }
        life.complete_task("kham-suc-khoe").unwrap();
        life.update_goal(UpdateGoal {
            id: g.id.clone(),
            progress: Some(30),
            progress_note: Some("Đã khám xong".into()),
            ..Default::default()
        })
        .unwrap();

        let views = life.goal_views(&ListGoals { status: Some(GoalStatus::Active) }).unwrap();
        assert_eq!(views.len(), 1);
        assert_eq!(views[0].summary.progress, Some(30));
        assert_eq!(views[0].why, "Đủ sức chơi với con.");
        assert_eq!((views[0].open_tasks, views[0].done_tasks), (1, 1));
        assert!(life.read_doc(&g.id).unwrap().body.contains("Đã khám xong"));

        assert!(life.update_goal(UpdateGoal { id: g.id, progress: Some(101), ..Default::default() }).is_err());
    }
}
