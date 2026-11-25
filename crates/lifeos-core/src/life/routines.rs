//! Routine definitions live in the vault. Schedule cursors, events and runs
//! live in SQLite so queuing and advancing a trigger can commit together.

use anyhow::{Context, Result, bail, ensure};
use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
use croner::{Cron, parser::{CronParser, Seconds}};
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde_json::Value;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use super::Life;
use crate::vault::{Doc, Frontmatter};
use lifeos_wire::{ASSISTANT, Kind, Routine, Run};

const MINUTE: &str = "%Y-%m-%d %H:%M";
const RUN_COLUMNS: &str =
    "id, routine, title, agent, trigger, detail, queued, started, finished, ms, error, reply";

input! {
    pub struct AddRoutine {
        pub title: String,
        pub instructions: String,
        /// Crew member id, or assistant by default.
        pub agent: Option<String>,
        /// Five cron fields in the server's local time.
        pub cron: Option<String>,
        /// YYYY-MM-DD HH:MM in the server's local time.
        pub at: Option<String>,
        /// Names of LifeOS write tools that trigger this routine.
        #[serde(default)]
        pub on: Vec<String>,
        #[serde(default)]
        pub hook: bool,
    }
}

input! {
    pub struct UpdateRoutine {
        pub id: String,
        pub title: Option<String>,
        pub instructions: Option<String>,
        pub agent: Option<String>,
        /// Set to an empty string to clear.
        pub cron: Option<String>,
        /// Set to an empty string to clear.
        pub at: Option<String>,
        pub on: Option<Vec<String>>,
        pub hook: Option<bool>,
        pub paused: Option<bool>,
    }
}

input! {
    pub struct ListRuns {
        pub routine: Option<String>,
        /// Defaults to 30, maximum 200.
        pub limit: Option<u32>,
    }
}

pub struct PendingRun {
    pub run: Run,
    pub instructions: String,
    pub payload: Option<String>,
}

fn cron(expression: &str) -> Result<Cron> {
    ensure!(expression.split_whitespace().count() == 5, "cron must have five fields");
    Ok(CronParser::builder().seconds(Seconds::Disallowed).build().parse(expression)?)
}

fn once(at: &str) -> Result<DateTime<Local>> {
    let time = NaiveDateTime::parse_from_str(at, MINUTE).context("expected YYYY-MM-DD HH:MM")?;
    Local.from_local_datetime(&time).earliest().context("this local time does not exist")
}

fn stamp(now: DateTime<Local>) -> String {
    now.format("%Y-%m-%d %H:%M:%S%.3f").to_string()
}

fn preview(value: &str, limit: usize) -> String {
    value.chars().take(limit).collect()
}

fn is_active(doc: &Doc) -> bool {
    doc.meta.status.as_deref().unwrap_or("active") == "active"
}

fn agent(doc: &Doc) -> &str {
    doc.meta.agent.as_deref().unwrap_or(ASSISTANT)
}

fn run_row(r: &Row<'_>) -> rusqlite::Result<Run> {
    Ok(Run {
        id: r.get(0)?, routine: r.get(1)?, title: r.get(2)?, agent: r.get(3)?,
        trigger: r.get(4)?, detail: r.get(5)?, queued: r.get(6)?, started: r.get(7)?,
        finished: r.get(8)?, ms: r.get::<_, Option<i64>>(9)?.map(|n| n as u64), error: r.get(10)?, reply: r.get(11)?,
    })
}

fn enqueue(
    conn: &Connection, doc: &Doc, trigger: &str, detail: Option<&str>, payload: Option<&str>, now: DateTime<Local>,
) -> Result<i64> {
    Ok(conn.query_row(
        "INSERT INTO runs(routine,title,agent,trigger,detail,payload,queued,instructions)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8) RETURNING id",
        params![doc.id, doc.meta.title, agent(doc), trigger, detail, payload, stamp(now), doc.body],
        |row| row.get(0),
    )?)
}

fn schedule(
    conn: &Connection, doc: &Doc, now: DateTime<Local>,
) -> Result<(Option<i64>, bool)> {
    let previous: Option<(String, Option<i64>)> = conn.query_row(
        "SELECT expression, next FROM routine_schedules WHERE routine=?1",
        [&doc.id], |r| Ok((r.get(0)?, r.get(1)?)),
    ).optional()?;
    let from = if previous.is_none() { doc.meta.created.unwrap_or(now) } else { now };
    let (expression, repeated) = match (&doc.meta.cron, &doc.meta.at) {
        (Some(expression), None) => (format!("cron:{expression}"), true),
        (None, Some(at)) => (format!("at:{at}"), false),
        (None, None) => return Ok((None, false)),
        _ => bail!("choose cron or at"),
    };
    let next = match previous {
        Some((was, next)) if was == expression => next,
        _ if repeated => Some(cron(&expression[5..])?.find_next_occurrence(&from, false)?.timestamp()),
        _ => Some(once(&expression[3..])?.timestamp()),
    };
    Ok((next, !repeated && next.is_none()))
}

impl Life {
    fn validate_routine(&self, meta: &Frontmatter, instructions: &str) -> Result<()> {
        ensure!(!meta.title.trim().is_empty() && !instructions.trim().is_empty(), "title and instructions are required");
        ensure!(meta.cron.is_none() || meta.at.is_none(), "choose cron or at");
        if let Some(cron_expression) = &meta.cron {
            cron(cron_expression)?.find_next_occurrence(&Local::now(), false)?;
        }
        if let Some(at) = &meta.at {
            once(at)?;
        }
        if let Some(agent) = meta.agent.as_deref().filter(|a| *a != ASSISTANT) {
            ensure!(self.member(agent)?.status == "active", "crew member '{agent}' is retired");
        }
        for tool in &meta.on {
            ensure!(crate::tools::event_tool(tool), "'{tool}' does not emit events");
        }
        Ok(())
    }

    pub fn add_routine(&self, input: AddRoutine) -> Result<Routine> {
        let meta = Frontmatter {
            title: input.title,
            agent: input.agent,
            cron: input.cron,
            at: input.at,
            on: input.on,
            hook: input.hook,
            status: Some("active".into()),
            created: Some(Local::now()),
            ..Default::default()
        };
        self.validate_routine(&meta, &input.instructions)?;
        let doc = self.create(Kind::Routine, meta, input.instructions)?;
        self.routine_view(&doc)
    }

    pub fn update_routine(&self, input: UpdateRoutine) -> Result<Routine> {
        let mut doc = self.find(Kind::Routine, &input.id)?;
        let old_schedule = (doc.meta.cron.clone(), doc.meta.at.clone());
        if let Some(title) = input.title { doc.meta.title = title; }
        if let Some(agent) = input.agent { doc.meta.agent = Some(agent); }
        if let Some(instructions) = input.instructions { doc.body = instructions; }
        if let Some(cron) = input.cron { doc.meta.cron = Some(cron).filter(|s| !s.trim().is_empty()); }
        if let Some(at) = input.at { doc.meta.at = Some(at).filter(|s| !s.trim().is_empty()); }
        if let Some(on) = input.on { doc.meta.on = on; }
        if let Some(hook) = input.hook { doc.meta.hook = hook; }
        if let Some(paused) = input.paused {
            doc.meta.status = Some(if paused { "paused" } else { "active" }.into());
        }
        self.validate_routine(&doc.meta, &doc.body)?;
        if old_schedule != (doc.meta.cron.clone(), doc.meta.at.clone()) {
            // Keep the definition's creation time for event filtering, but start a new
            // cron schedule from the point where the user changed it.
            let mut scheduled = doc.clone();
            let now = Local::now();
            scheduled.meta.created = Some(now);
            let (next, _) = schedule(&self.conn, &scheduled, now)?;
            persist_schedule(&self.conn, &doc, next)?;
        }
        self.save(&doc)?;
        self.routine_view(&doc)
    }

    pub fn remove_routine(&self, id: &str) -> Result<()> {
        let doc = self.find(Kind::Routine, id)?;
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("DELETE FROM routine_hooks WHERE routine=?1", [id])?;
        tx.execute("DELETE FROM routine_schedules WHERE routine=?1", [id])?;
        tx.execute("UPDATE runs SET finished=?2,error='Routine deleted' WHERE routine=?1 AND started IS NULL AND finished IS NULL", params![id, stamp(Local::now())])?;
        std::fs::remove_file(self.vault.abs(&doc.rel_path))?;
        self.unindex(&doc.rel_path)?;
        tx.commit()?;
        Ok(())
    }

    pub fn routines(&self) -> Result<Vec<Routine>> {
        self.routine_docs()?.iter().map(|doc| self.routine_view(doc)).collect()
    }

    fn routine_docs(&self) -> Result<Vec<Doc>> {
        self.docs_where(Kind::Routine, "1", "title, path")?.into_iter()
            .map(|d| self.vault.read(Kind::Routine, &d.path)).collect()
    }

    fn routine_view(&self, doc: &Doc) -> Result<Routine> {
        let (next, done) = schedule(&self.conn, doc, Local::now()).unwrap_or_default();
        let count = self.conn.query_row("SELECT count(*) FROM runs WHERE routine=?1", [&doc.id], |r| r.get(0))?;
        let last = self.runs(Some(&doc.id), 1)?.pop();
        let status = if !is_active(doc) { "paused" } else if done { "done" } else { "active" };
        Ok(Routine {
            id: doc.id.clone(), title: doc.meta.title.clone(), agent: agent(doc).into(),
            instructions: doc.body.clone(), cron: doc.meta.cron.clone(), at: doc.meta.at.clone(),
            on: doc.meta.on.clone(), hook: doc.meta.hook,
            hook_ready: doc.meta.hook && self.hook_digest(&doc.id)?.is_some(),
            status: status.into(), last, runs: count,
            next: next.and_then(|at| Local.timestamp_opt(at, 0).single())
                .filter(|_| is_active(doc)).map(|at| at.format(MINUTE).to_string()),
        })
    }

    pub fn runs(&self, routine: Option<&str>, limit: u32) -> Result<Vec<Run>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {RUN_COLUMNS} FROM runs WHERE ?1 IS NULL OR routine=?1 ORDER BY id DESC LIMIT ?2"
        ))?;
        Ok(stmt.query_map(params![routine, limit.clamp(1, 200)], run_row)?.collect::<rusqlite::Result<_>>()?)
    }

    pub fn queue_manual(&self, id: &str) -> Result<Run> {
        let doc = self.find(Kind::Routine, id)?;
        ensure!(is_active(&doc), "routine is paused");
        let id = enqueue(&self.conn, &doc, "manual", None, None, Local::now())?;
        Ok(self.conn.query_row(&format!("SELECT {RUN_COLUMNS} FROM runs WHERE id=?1"), [id], run_row)?)
    }

    /// Generate or rotate a key locally. Only its digest is stored.
    pub fn rotate_hook_key(&self, id: &str) -> Result<String> {
        ensure!(self.find(Kind::Routine, id)?.meta.hook, "routine has no webhook");
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).map_err(|err| anyhow::anyhow!("cannot generate webhook key: {err}"))?;
        let key = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
        let digest = Sha256::digest(key.as_bytes());
        self.conn.execute(
            "INSERT INTO routine_hooks VALUES (?1,?2) ON CONFLICT(routine) DO UPDATE SET digest=excluded.digest",
            params![id, digest.as_slice()],
        )?;
        Ok(key)
    }

    fn hook_digest(&self, id: &str) -> Result<Option<Vec<u8>>> {
        Ok(self.conn.query_row("SELECT digest FROM routine_hooks WHERE routine=?1", [id], |r| r.get(0)).optional()?)
    }

    pub fn queue_hook(&self, id: &str, token: &str, payload: &str) -> Result<Option<i64>> {
        let Some(expected) = self.hook_digest(id)? else { return Ok(None) };
        if !bool::from(expected.ct_eq(Sha256::digest(token.as_bytes()).as_slice())) {
            return Ok(None);
        }
        let Ok(doc) = self.find(Kind::Routine, id) else { return Ok(None) };
        if !doc.meta.hook || !is_active(&doc) {
            return Ok(None);
        }
        let id = enqueue(&self.conn, &doc, "hook", None, Some(payload), Local::now())?;
        Ok(Some(id))
    }

    pub(crate) fn record_event(&self, caller: &str, tool: &str, output: &Value) -> Result<()> {
        // Input is omitted to avoid persisting extra copies of user-supplied arguments.
        self.conn.execute(
            "INSERT INTO events(at,caller,tool,input,output) VALUES (?1,?2,?3,'{}',?4)",
            params![stamp(Local::now()), caller, tool, preview(&output.to_string(), 4000)],
        )?;
        Ok(())
    }

    /// Queues missed recurring schedules once, and consumes events atomically with their runs.
    pub fn tick_routines(&mut self, now: DateTime<Local>) -> Result<bool> {
        self.sync_index()?;
        let docs = self.routine_docs()?;
        let tx = self.conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let mut changed = false;
        for doc in &docs {
            let (next, _) = match schedule(&tx, doc, now) {
                Ok(schedule) => schedule,
                Err(err) => {
                    tracing::warn!(routine = %doc.id, "invalid routine schedule: {err}");
                    continue;
                }
            };
            if doc.meta.cron.is_none() && doc.meta.at.is_none() { continue; }
            let mut next_run = next;
            if is_active(doc) && let Some(at) = next.filter(|at| *at <= now.timestamp()) {
                let detail = Local.timestamp_opt(at, 0).single().map(|t| t.format(MINUTE).to_string());
                next_run = doc.meta.cron.as_deref().map(|expression| {
                    cron(expression).and_then(|cron| Ok(cron.find_next_occurrence(&now, false)?.timestamp()))
                }).transpose()?;
                enqueue(&tx, doc, "schedule", detail.as_deref(), None, now)?;
                changed = true;
            }
            persist_schedule(&tx, doc, next_run)?;
        }
        let events = {
            let mut stmt = tx.prepare("SELECT id,at,caller,tool,output FROM events WHERE routine IS NULL ORDER BY id LIMIT 250")?;
            stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, String>(4)?)))?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        for (id, at, caller, tool, output) in events {
            for doc in docs.iter().filter(|d| is_active(d) && d.meta.on.contains(&tool)) {
                if doc.meta.created.is_some_and(|created| stamp(created) > at) { continue; }
                let payload = serde_json::json!({ "at": at, "caller": caller, "tool": tool, "result": output });
                enqueue(&tx, doc, "event", Some(&tool), Some(&payload.to_string()), now)?;
                changed = true;
            }
            tx.execute("DELETE FROM events WHERE id=?1", [id])?;
        }
        tx.commit()?;
        Ok(changed)
    }

    /// Called once after acquiring the exclusive worker lock for this vault.
    /// Interrupted runs stay visible and are never replayed automatically.
    pub fn recover_routines(&self) -> Result<()> {
        self.conn.execute(
            "UPDATE runs SET finished=?1,error='Worker stopped before the run completed'
             WHERE started IS NOT NULL AND finished IS NULL", [stamp(Local::now())],
        )?;
        Ok(())
    }

    pub fn claim_run(&mut self) -> Result<Option<PendingRun>> {
        let tx = self.conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let run = tx.query_row(&format!(
            "SELECT {RUN_COLUMNS},instructions,payload FROM runs AS r
             WHERE started IS NULL AND finished IS NULL AND NOT EXISTS (
               SELECT 1 FROM runs AS busy WHERE busy.started IS NOT NULL AND busy.finished IS NULL
               AND (busy.routine=r.routine OR busy.agent=r.agent))
             ORDER BY id LIMIT 1"
        ), [], |row| Ok(PendingRun { run: run_row(row)?, instructions: row.get(12)?, payload: row.get(13)? }))
            .optional()?;
        let run = run.map(|mut work| {
            let started = stamp(Local::now());
            tx.execute("UPDATE runs SET started=?2 WHERE id=?1", params![work.run.id, started])?;
            work.run.started = Some(started);
            Ok::<_, rusqlite::Error>(work)
        }).transpose()?;
        tx.commit()?;
        Ok(run)
    }

    pub fn finish_run(&self, id: i64, ms: u64, error: Option<&str>, reply: Option<&str>) -> Result<()> {
        self.conn.execute(
            "UPDATE runs SET finished=?2,ms=?3,error=?4,reply=?5,payload=NULL WHERE id=?1 AND finished IS NULL",
            params![id, stamp(Local::now()), i64::try_from(ms)?, error.map(|s| preview(s, 4000)), reply.map(|s| preview(s, 2000))],
        )?;
        Ok(())
    }

    pub fn check_queued_routine(&self, id: &str) -> Result<()> {
        ensure!(is_active(&self.find(Kind::Routine, id)?), "routine is paused");
        Ok(())
    }
}

fn persist_schedule(conn: &Connection, doc: &Doc, next: Option<i64>) -> Result<()> {
    let expression = if let Some(cron) = &doc.meta.cron {
        format!("cron:{cron}")
    } else if let Some(at) = &doc.meta.at {
        format!("at:{at}")
    } else {
        conn.execute("DELETE FROM routine_schedules WHERE routine=?1", [&doc.id])?;
        return Ok(());
    };
    conn.execute(
        "INSERT INTO routine_schedules VALUES (?1,?2,?3) ON CONFLICT(routine) DO UPDATE
         SET expression=excluded.expression,next=excluded.next
         WHERE expression IS NOT excluded.expression OR next IS NOT excluded.next",
        params![doc.id, expression, next],
    )?;
    Ok(())
}
