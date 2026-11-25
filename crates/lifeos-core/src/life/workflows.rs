//! Workflow runs and their journals (remit's ADR-0073): the store side. A script lives in the vault as
//! `workflows/<name>.js` (or is given inline); a run keeps its script and arguments, and one row per call its script made:
//! pending until its result arrives, then done or failed, numbered by arrival. lifeos-agent evaluates the script against
//! that journal (the `workflow` crate) and issues its calls.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use rusqlite::{OptionalExtension, Row, params};

use super::Life;
use super::cards::now;
use lifeos_wire::{WorkflowRun, WorkflowStep, Workflows};

/// The vault folder workflow scripts live in.
pub const FOLDER: &str = "workflows";

/// Runs a list shows, at most.
const LISTED: u32 = 50;

input! {
    pub struct ListWorkflows {
        /// Also the runs, newest first. Defaults to true.
        pub runs: Option<bool>,
    }
}

input! {
    pub struct GetWorkflowRun {
        pub id: i64,
    }
}

/// The waits due, each as (run, step), and when the soonest other one comes due.
pub type DueWaits = (Vec<(i64, i64)>, Option<String>);

/// One settled call, as the engine's journal reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settled {
    pub seq: i64,
    pub verb: String,
    pub hash: String,
    /// Its result as JSON text, empty for null.
    pub result: String,
    /// Why it failed; empty when it did not.
    pub error: String,
}

const RUN_COLUMNS: &str = "id, name, description, status, result, error, started_by, created, finished";
const STEP_COLUMNS: &str = "seq, verb, label, phase, input, status, result, error, created, finished";

fn run_of(r: &Row) -> rusqlite::Result<WorkflowRun> {
    Ok(WorkflowRun {
        id: r.get(0)?,
        name: r.get(1)?,
        description: r.get(2)?,
        status: r.get(3)?,
        result: r.get(4)?,
        error: r.get(5)?,
        started_by: r.get(6)?,
        created: r.get(7)?,
        finished: r.get(8)?,
        steps: Vec::new(),
    })
}

fn step_of(r: &Row) -> rusqlite::Result<WorkflowStep> {
    Ok(WorkflowStep {
        seq: r.get(0)?,
        verb: r.get(1)?,
        label: r.get(2)?,
        phase: r.get(3)?,
        input: r.get(4)?,
        status: r.get(5)?,
        result: r.get(6)?,
        error: r.get(7)?,
        created: r.get(8)?,
        finished: r.get(9)?,
    })
}

/// Whether name is a script's name: letters, digits, `-` and `_`.
fn plain(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}

impl Life {
    fn workflow_dir(&self) -> PathBuf {
        self.home().join(FOLDER)
    }

    /// The names of the scripts in `workflows/`.
    pub fn workflow_scripts(&self) -> Result<Vec<String>> {
        let Ok(dir) = std::fs::read_dir(self.workflow_dir()) else { return Ok(Vec::new()) };
        let mut names: Vec<String> = dir
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().to_str().and_then(|n| n.strip_suffix(".js")).map(str::to_owned))
            .filter(|n| plain(n))
            .collect();
        names.sort();
        Ok(names)
    }

    /// The script `workflows/<name>.js`.
    pub fn workflow_script(&self, name: &str) -> Result<String> {
        if !plain(name) {
            bail!("a workflow's name is its file's in workflows/, without .js");
        }
        let path = self.workflow_dir().join(format!("{name}.js"));
        std::fs::read_to_string(&path).with_context(|| format!("no workflow {name}: write it as workflows/{name}.js"))
    }

    pub fn new_workflow_run(&self, name: &str, description: &str, script: &str, args: &str, by: &str) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO workflow_runs(name, description, script, args, started_by, created) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![name, description, script, args, by, now()],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Run `id` with its steps.
    pub fn workflow_run(&self, id: i64) -> Result<WorkflowRun> {
        let mut run = self
            .conn
            .query_row(&format!("SELECT {RUN_COLUMNS} FROM workflow_runs WHERE id = ?1"), [id], run_of)
            .optional()?
            .with_context(|| format!("no workflow run {id}"))?;
        let mut stmt = self.conn.prepare(&format!("SELECT {STEP_COLUMNS} FROM workflow_steps WHERE run = ?1 ORDER BY seq"))?;
        run.steps = stmt.query_map([id], step_of)?.collect::<rusqlite::Result<_>>()?;
        Ok(run)
    }

    pub fn workflows(&self, l: &ListWorkflows) -> Result<Workflows> {
        let runs = if l.runs.unwrap_or(true) {
            let mut stmt = self.conn.prepare(&format!("SELECT id FROM workflow_runs ORDER BY id DESC LIMIT {LISTED}"))?;
            let ids = stmt.query_map([], |r| r.get::<_, i64>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
            ids.into_iter().map(|id| self.workflow_run(id)).collect::<Result<_>>()?
        } else {
            Vec::new()
        };
        Ok(Workflows { scripts: self.workflow_scripts()?, runs })
    }

    /// Run `id`'s script and arguments.
    pub fn workflow_source(&self, id: i64) -> Result<(String, String)> {
        Ok(self.conn.query_row("SELECT script, args FROM workflow_runs WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))?)
    }

    /// The runs not ended.
    pub fn open_workflow_runs(&self) -> Result<Vec<i64>> {
        let mut stmt =
            self.conn.prepare("SELECT id FROM workflow_runs WHERE status IN ('running', 'waiting') ORDER BY id")?;
        Ok(stmt.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?)
    }

    /// The journal of run `id`: each settled call, in the order its result arrived.
    pub fn workflow_journal(&self, id: i64) -> Result<Vec<Settled>> {
        let mut stmt = self.conn.prepare(
            "SELECT seq, verb, hash, result, error FROM workflow_steps WHERE run = ?1 AND status != 'pending' ORDER BY arrived",
        )?;
        let rows = stmt.query_map([id], |r| {
            Ok(Settled { seq: r.get(0)?, verb: r.get(1)?, hash: r.get(2)?, result: r.get(3)?, error: r.get(4)? })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Records a call run `run` made; false when it is recorded already.
    #[allow(clippy::too_many_arguments, reason = "one row's columns")]
    pub fn add_workflow_step(
        &self,
        run: i64,
        seq: i64,
        verb: &str,
        label: &str,
        phase: &str,
        input: &str,
        hash: &str,
        due: &str,
    ) -> Result<bool> {
        Ok(self.conn.execute(
            "INSERT OR IGNORE INTO workflow_steps(run, seq, verb, label, phase, input, hash, due, created)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![run, seq, verb, label, phase, input, hash, due, now()],
        )? > 0)
    }

    /// Settles a pending call with its result (JSON text) or why it failed; numbered as the next to arrive. False when
    /// it was settled already.
    pub fn settle_workflow_step(&self, run: i64, seq: i64, outcome: std::result::Result<String, String>) -> Result<bool> {
        let (status, result, error) = match outcome {
            Ok(r) => ("done", r, String::new()),
            Err(e) => ("failed", String::new(), if e.is_empty() { "failed".to_owned() } else { e }),
        };
        Ok(self.conn.execute(
            "UPDATE workflow_steps SET status = ?3, result = ?4, error = ?5, finished = ?6,
                 arrived = (SELECT COALESCE(MAX(arrived), 0) + 1 FROM workflow_steps WHERE run = ?1)
             WHERE run = ?1 AND seq = ?2 AND status = 'pending'",
            params![run, seq, status, result, error, now()],
        )? > 0)
    }

    /// What the script's meta says of itself, kept on the run once read.
    pub fn set_workflow_meta(&self, id: i64, name: &str, description: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE workflow_runs SET name = CASE WHEN ?2 = '' THEN name ELSE ?2 END, description = ?3 WHERE id = ?1",
            params![id, name, description],
        )?;
        Ok(())
    }

    pub fn set_workflow_status(&self, id: i64, status: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE workflow_runs SET status = ?2 WHERE id = ?1 AND status IN ('running', 'waiting')",
            params![id, status],
        )?;
        Ok(())
    }

    /// Ends run `id` as `status` (done, failed or stopped), failing its pending calls. False when it had ended.
    pub fn end_workflow_run(&self, id: i64, status: &str, result: &str, error: &str) -> Result<bool> {
        let at = now();
        let ended = self.conn.execute(
            "UPDATE workflow_runs SET status = ?2, result = ?3, error = ?4, finished = ?5
             WHERE id = ?1 AND status IN ('running', 'waiting')",
            params![id, status, result, error, at],
        )? > 0;
        if ended {
            self.conn.execute(
                "UPDATE workflow_steps SET status = 'failed', error = ?2, finished = ?3 WHERE run = ?1 AND status = 'pending'",
                params![id, format!("the run {status}"), at],
            )?;
        }
        Ok(ended)
    }

    /// The pending calls of run `id`.
    pub fn pending_workflow_steps(&self, id: i64) -> Result<Vec<WorkflowStep>> {
        Ok(self.workflow_run(id)?.steps.into_iter().filter(|s| s.status == "pending").collect())
    }

    /// The waits due by `at` (`YYYY-MM-DDTHH:MM:SS`), and the soonest one after.
    pub fn due_workflow_waits(&self, at: &str) -> Result<DueWaits> {
        let mut stmt = self.conn.prepare(
            "SELECT run, seq FROM workflow_steps WHERE verb = 'wait' AND status = 'pending' AND due != '' AND due <= ?1",
        )?;
        let due = stmt.query_map([at], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
        let next = self
            .conn
            .query_row(
                "SELECT MIN(due) FROM workflow_steps WHERE verb = 'wait' AND status = 'pending' AND due > ?1",
                [at],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        Ok((due, next))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::life::test_support::life;

    #[test]
    fn a_journal_lists_results_in_the_order_they_arrived() {
        let (dir, life) = life();
        std::fs::create_dir_all(dir.path().join(FOLDER)).unwrap();
        std::fs::write(dir.path().join("workflows/sang-thu-hai.js"), "export const meta = {name: 'x'}\nreturn 1").unwrap();
        assert_eq!(life.workflow_scripts().unwrap(), ["sang-thu-hai"]);
        assert!(life.workflow_script("../x").is_err());

        let id = life.new_workflow_run("x", "", "return 1", "", "You").unwrap();
        assert!(life.add_workflow_step(id, 0, "agent", "a", "", "{}", "h0", "").unwrap());
        assert!(life.add_workflow_step(id, 1, "wait", "", "", "{}", "h1", "2026-01-01T08:00:00").unwrap());
        assert!(!life.add_workflow_step(id, 1, "wait", "", "", "{}", "h1", "").unwrap(), "once");
        assert_eq!(life.due_workflow_waits("2026-01-01T07:00:00").unwrap(), (vec![], Some("2026-01-01T08:00:00".into())));
        assert_eq!(life.due_workflow_waits("2026-01-01T09:00:00").unwrap().0, [(id, 1)]);

        assert!(life.settle_workflow_step(id, 1, Ok("null".into())).unwrap());
        assert!(life.settle_workflow_step(id, 0, Err("no".into())).unwrap());
        assert!(!life.settle_workflow_step(id, 0, Ok("1".into())).unwrap(), "settled once");
        let journal = life.workflow_journal(id).unwrap();
        assert_eq!(journal.iter().map(|s| s.seq).collect::<Vec<_>>(), [1, 0]);
        assert_eq!(journal[1].error, "no");

        assert!(life.end_workflow_run(id, "done", "1", "").unwrap());
        assert!(!life.end_workflow_run(id, "failed", "", "x").unwrap());
        assert!(life.open_workflow_runs().unwrap().is_empty());
        assert_eq!(life.workflow_run(id).unwrap().status, "done");
    }
}
