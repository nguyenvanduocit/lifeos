//! Tasks: `tasks/<id>.md`, status open | done.

use anyhow::Result;
use chrono::{Duration, Local, NaiveDate};

use super::{Life, today};
use crate::vault::{Frontmatter, Kind};
use lifeos_wire::{DocSummary, TasksView};

choice! {
    pub enum Priority {
        High => "high",
        Normal => "normal",
        Low => "low",
    }
}

choice! {
    pub enum TaskFilter {
        Open => "open",
        Done => "done",
        All => "all",
    }
}

input! {
    pub struct AddTask {
        pub title: String,
        /// Deadline, YYYY-MM-DD.
        pub due: Option<NaiveDate>,
        /// The day the user plans to do it, YYYY-MM-DD.
        pub scheduled: Option<NaiveDate>,
        pub priority: Option<Priority>,
        /// Id of the goal this task serves (from list_goals).
        pub goal: Option<String>,
        #[serde(default)]
        pub tags: Vec<String>,
        /// Details, Markdown.
        pub notes: Option<String>,
    }
}

input! {
    /// Only the fields given change.
    pub struct UpdateTask {
        pub id: String,
        pub title: Option<String>,
        pub due: Option<NaiveDate>,
        pub scheduled: Option<NaiveDate>,
        pub priority: Option<Priority>,
        pub goal: Option<String>,
        pub tags: Option<Vec<String>>,
        /// Appended to the task's notes.
        pub add_notes: Option<String>,
        /// Remove the due date.
        #[serde(default)]
        pub clear_due: bool,
        /// Remove the scheduled day.
        #[serde(default)]
        pub clear_scheduled: bool,
    }
}

input! {
    pub struct ListTasks {
        /// Defaults to open.
        pub status: Option<TaskFilter>,
        /// Only tasks for this goal id.
        pub goal: Option<String>,
    }
}

/// Soonest date first, then priority, then title.
const TASK_ORDER: &str = "COALESCE(MIN(due, scheduled), due, scheduled) IS NULL,
    COALESCE(MIN(due, scheduled), due, scheduled),
    CASE priority WHEN 'high' THEN 0 WHEN 'low' THEN 2 ELSE 1 END, title";

impl Life {
    pub fn add_task(&self, t: AddTask) -> Result<DocSummary> {
        let goal = t.goal.as_deref().map(|id| self.find(Kind::Goal, id).map(|doc| doc.id)).transpose()?;
        let meta = Frontmatter {
            title: t.title.trim().to_string(),
            status: Some("open".into()),
            due: t.due,
            scheduled: t.scheduled,
            priority: t.priority.map(|p| p.as_str().to_string()),
            goal,
            tags: t.tags,
            created: Some(Local::now()),
            ..Default::default()
        };
        Ok(super::summary(&self.create(Kind::Task, meta, t.notes.unwrap_or_default())?))
    }

    pub fn update_task(&self, u: UpdateTask) -> Result<DocSummary> {
        let mut doc = self.find(Kind::Task, &u.id)?;
        let m = &mut doc.meta;
        if let Some(title) = u.title {
            m.title = title.trim().to_string();
        }
        if u.clear_due {
            m.due = None;
        }
        if u.clear_scheduled {
            m.scheduled = None;
        }
        m.due = u.due.or(m.due);
        m.scheduled = u.scheduled.or(m.scheduled);
        if let Some(p) = u.priority {
            m.priority = Some(p.as_str().to_string());
        }
        if let Some(goal) = u.goal {
            doc.meta.goal = Some(self.find(Kind::Goal, &goal)?.id);
        }
        if let Some(tags) = u.tags {
            doc.meta.tags = tags;
        }
        if let Some(notes) = u.add_notes {
            if !doc.body.is_empty() && !doc.body.ends_with('\n') {
                doc.body.push('\n');
            }
            doc.body.push_str(&format!("\n{}\n", notes.trim()));
        }
        self.save(&doc)?;
        Ok(super::summary(&doc))
    }

    pub fn complete_task(&self, id: &str) -> Result<DocSummary> {
        let mut doc = self.find(Kind::Task, id)?;
        doc.meta.status = Some("done".into());
        doc.meta.completed = Some(today());
        self.save(&doc)?;
        Ok(super::summary(&doc))
    }

    pub fn reopen_task(&self, id: &str) -> Result<DocSummary> {
        let mut doc = self.find(Kind::Task, id)?;
        doc.meta.status = Some("open".into());
        doc.meta.completed = None;
        self.save(&doc)?;
        Ok(super::summary(&doc))
    }

    pub fn list_tasks(&self, q: &ListTasks) -> Result<Vec<DocSummary>> {
        let status = match q.status.unwrap_or(TaskFilter::Open) {
            TaskFilter::Open => "status = 'open'",
            TaskFilter::Done => "status = 'done'",
            TaskFilter::All => "1",
        };
        let mut tasks = self.docs_where(Kind::Task, status, TASK_ORDER)?;
        if let Some(goal) = &q.goal {
            let id = self.find(Kind::Goal, goal).map(|doc| doc.id).unwrap_or_else(|_| goal.clone());
            tasks.retain(|t| t.goal.as_deref() == Some(id.as_str()));
        }
        Ok(tasks)
    }

    pub fn tasks_view(&self, day: NaiveDate) -> Result<TasksView> {
        let d = day.to_string();
        let week = (day + Duration::days(7)).to_string();
        let open = |cond: String| self.docs_where(Kind::Task, &format!("status = 'open' AND ({cond})"), TASK_ORDER);
        Ok(TasksView {
            overdue: open(format!("due < '{d}'"))?,
            today: open(format!(
                "(due IS NULL OR due >= '{d}') AND (due = '{d}' OR scheduled <= '{d}')"
            ))?,
            upcoming: open(format!(
                "(due IS NULL OR due > '{d}') AND (scheduled IS NULL OR scheduled > '{d}')
                 AND COALESCE(MIN(due, scheduled), due, scheduled) BETWEEN '{d}' AND '{week}'"
            ))?,
            anytime: open("due IS NULL AND scheduled IS NULL".into())?,
            done_today: self.docs_where(Kind::Task, &format!("status = 'done' AND completed = '{d}'"), "title")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::life::test_support::*;

    fn add(life: &Life, title: &str, due: Option<&str>, scheduled: Option<&str>) -> DocSummary {
        life.add_task(AddTask {
            title: title.into(),
            due: due.map(d),
            scheduled: scheduled.map(d),
            ..Default::default()
        })
        .unwrap()
    }

    #[test]
    fn tasks_fall_into_the_right_buckets() {
        let (_dir, life) = life();
        add(&life, "Quá hạn", Some("2026-11-20"), None);
        add(&life, "Hạn hôm nay", Some("2026-11-25"), None);
        add(&life, "Định làm hôm nay", None, Some("2026-11-25"));
        add(&life, "Dời từ hôm qua", None, Some("2026-11-24"));
        add(&life, "Tuần này", None, Some("2026-11-28"));
        add(&life, "Lúc nào cũng được", None, None);
        let v = life.tasks_view(d("2026-11-25")).unwrap();
        let titles = |l: &[DocSummary]| l.iter().map(|t| t.title.clone()).collect::<Vec<_>>();
        assert_eq!(titles(&v.overdue), ["Quá hạn"]);
        assert_eq!(titles(&v.today).len(), 3);
        assert_eq!(titles(&v.upcoming), ["Tuần này"]);
        assert_eq!(titles(&v.anytime), ["Lúc nào cũng được"]);
    }

    #[test]
    fn task_lifecycle_and_goal_link() {
        let (_dir, life) = life();
        let bad = AddTask { title: "x".into(), goal: Some("nope".into()), ..Default::default() };
        assert!(life.add_task(bad).is_err());
        let t = add(&life, "Đọc chương 1", None, None);
        assert_eq!(t.id, "doc-chuong-1");
        assert_eq!(add(&life, "Đọc chương 1", None, None).id, "doc-chuong-1-2");

        life.update_task(UpdateTask {
            id: t.id.clone(),
            priority: Some(Priority::High),
            add_notes: Some("trang 1-30".into()),
            ..Default::default()
        })
        .unwrap();
        let full = life.read_doc(&t.id).unwrap();
        assert_eq!(full.summary.priority.as_deref(), Some("high"));
        assert!(full.body.contains("trang 1-30"));

        life.complete_task(&t.id).unwrap();
        let open = ListTasks::default();
        assert_eq!(life.list_tasks(&open).unwrap().len(), 1);
        let done = ListTasks { status: Some(TaskFilter::Done), ..Default::default() };
        assert_eq!(life.list_tasks(&done).unwrap().len(), 1);
        assert_eq!(life.tasks_view(today()).unwrap().done_today.len(), 1);
        life.reopen_task(&t.id).unwrap();
        assert_eq!(life.list_tasks(&open).unwrap().len(), 2);
    }
}
