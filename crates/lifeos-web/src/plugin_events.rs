//! Persistent fan-out. A delivery keeps its request id through retries and restarts.
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;
use std::{path::Path, sync::Mutex};
use uuid::Uuid;

pub(super) struct Queue(Mutex<Connection>);

pub(super) struct Event {
    pub key: String,
    pub body: Value,
    pub dedupe: String,
    pub subscribers: Vec<(String, String)>,
}

pub(super) struct Job {
    pub id: String,
    pub key: String,
    pub body: Value,
    pub plugin: String,
    pub processor: String,
    root: String,
    depth: u32,
    pub at: i64,
    pub attempts: u32,
}

impl Queue {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let file = std::fs::File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL;
            CREATE TABLE IF NOT EXISTS seen(event TEXT, token TEXT, at INTEGER, PRIMARY KEY(event, token));
            CREATE INDEX IF NOT EXISTS seen_age ON seen(at);
            CREATE TABLE IF NOT EXISTS signal_history(id INTEGER PRIMARY KEY AUTOINCREMENT, event TEXT NOT NULL, at INTEGER NOT NULL, subscribers INTEGER NOT NULL);
            CREATE TABLE IF NOT EXISTS chains(id TEXT PRIMARY KEY, events INTEGER NOT NULL);
            CREATE TABLE IF NOT EXISTS jobs(id TEXT PRIMARY KEY, root TEXT, depth INTEGER, event TEXT,
                body TEXT, plugin TEXT, processor TEXT, at INTEGER, attempts INTEGER DEFAULT 0,
                due INTEGER DEFAULT 0);
            CREATE INDEX IF NOT EXISTS jobs_due ON jobs(due);
            CREATE INDEX IF NOT EXISTS jobs_root ON jobs(root);")?;
        Ok(Self(Mutex::new(conn)))
    }

    // Commit the parent ACK and all resulting deliveries together.
    pub fn commit(&self, parent: Option<&Job>, events: Vec<Event>) -> anyhow::Result<()> {
        let mut conn = self.0.lock().expect("plugin queue lock");
        let tx = conn.transaction()?;
        let root = parent
            .map(|j| j.root.clone())
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let depth = parent.map_or(0, |j| j.depth + 1);
        let now = chrono::Utc::now().timestamp_millis();
        tx.execute("DELETE FROM seen WHERE at < ?1", [now - 30 * 86400000])?;
        if let Some(job) = parent {
            tx.execute("DELETE FROM jobs WHERE id=?1", [&job.id])?;
        }
        tx.execute("INSERT OR IGNORE INTO chains VALUES (?1,0)", [&root])?;
        for event in events {
            if depth >= 8 {
                break;
            }
            if !event.dedupe.is_empty()
                && tx.execute(
                    "INSERT OR IGNORE INTO seen VALUES (?1,?2,?3)",
                    params![event.key, event.dedupe, now],
                )? == 0
            {
                continue;
            }
            if tx.execute(
                "UPDATE chains SET events=events+1 WHERE id=?1 AND events<64",
                [&root],
            )? == 0
            {
                break;
            }
            let body = serde_json::to_string(&event.body)?;
            tx.execute(
                "INSERT INTO signal_history(event,at,subscribers) VALUES (?1,?2,?3)",
                params![event.key, now, event.subscribers.len() as i64],
            )?;
            for (plugin, processor) in event.subscribers {
                tx.execute(
                    "INSERT INTO jobs(id,root,depth,event,body,plugin,processor,at)
                    VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                    params![
                        Uuid::new_v4().to_string(),
                        root,
                        depth,
                        event.key,
                        body,
                        plugin,
                        processor,
                        now
                    ],
                )?;
            }
        }
        tx.execute(
            "DELETE FROM chains WHERE id=?1 AND NOT EXISTS (SELECT 1 FROM jobs WHERE root=?1)",
            [&root],
        )?;
        tx.execute(
            "DELETE FROM signal_history WHERE id <= (SELECT max(id)-1000 FROM signal_history)",
            [],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn next(&self) -> anyhow::Result<Option<Job>> {
        let conn = self.0.lock().expect("plugin queue lock");
        Ok(conn
            .query_row(
                "SELECT id,root,depth,event,body,plugin,processor,at,attempts FROM jobs
            WHERE due <= ?1 ORDER BY rowid LIMIT 1",
                [chrono::Utc::now().timestamp()],
                |r| {
                    let body: String = r.get(4)?;
                    let body = serde_json::from_str(&body).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            4,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?;
                    Ok(Job {
                        id: r.get(0)?,
                        root: r.get(1)?,
                        depth: r.get(2)?,
                        key: r.get(3)?,
                        body,
                        plugin: r.get(5)?,
                        processor: r.get(6)?,
                        at: r.get(7)?,
                        attempts: r.get(8)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn observed_keys(&self) -> anyhow::Result<Vec<String>> {
        let conn = self.0.lock().expect("plugin queue lock");
        let mut query = conn.prepare("SELECT DISTINCT event FROM signal_history")?;
        Ok(query
            .query_map([], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?)
    }

    pub fn history(&self) -> anyhow::Result<Vec<Value>> {
        let conn = self.0.lock().expect("plugin queue lock");
        let mut query = conn.prepare(
            "SELECT id,event,at,subscribers FROM signal_history ORDER BY id DESC LIMIT 100",
        )?;
        Ok(query
            .query_map([], |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, i64>(0)?, "event": row.get::<_, String>(1)?,
                    "at": row.get::<_, i64>(2)?, "subscribers": row.get::<_, i64>(3)?
                }))
            })?
            .collect::<rusqlite::Result<_>>()?)
    }

    pub fn retry_counts(&self) -> anyhow::Result<std::collections::BTreeMap<String, i64>> {
        let conn = self.0.lock().expect("plugin queue lock");
        let mut query =
            conn.prepare("SELECT plugin,count(*) FROM jobs WHERE attempts>0 GROUP BY plugin")?;
        Ok(query
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?)
    }

    pub fn retry(&self, job: &Job) -> anyhow::Result<()> {
        let delay = 1_i64 << job.attempts.min(8);
        self.0.lock().expect("plugin queue lock").execute(
            "UPDATE jobs SET attempts=attempts+1,due=?2 WHERE id=?1",
            params![job.id, chrono::Utc::now().timestamp() + delay],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn event(key: &str, token: &str) -> Event {
        Event {
            key: key.into(),
            body: serde_json::json!({"ok":true}),
            dedupe: token.into(),
            subscribers: vec![("sample".into(), "remember".into())],
        }
    }
    #[test]
    fn history_keeps_events_without_subscribers_across_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.sqlite");
        let queue = Queue::open(&path).unwrap();
        let mut started = event("lifeos.started", "");
        started.subscribers.clear();
        queue.commit(None, vec![started]).unwrap();
        assert!(queue.next().unwrap().is_none());
        drop(queue);
        let history = Queue::open(&path).unwrap().history().unwrap();
        assert_eq!(history[0]["event"], "lifeos.started");
        assert_eq!(history[0]["subscribers"], 0);
        assert!(history[0].get("body").is_none());
    }

    #[test]
    fn redelivery_and_restart_preserve_identity_and_atomic_fanout() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.sqlite");
        let q = Queue::open(&path).unwrap();
        q.commit(None, vec![event("plugin.github.push", "123")])
            .unwrap();
        let job = q.next().unwrap().unwrap();
        q.retry(&job).unwrap();
        drop(q);
        let q = Queue::open(&path).unwrap();
        q.commit(None, vec![event("plugin.github.push", "123")])
            .unwrap();
        q.0.lock()
            .unwrap()
            .execute("UPDATE jobs SET due=0", [])
            .unwrap();
        let retried = q.next().unwrap().unwrap();
        assert_eq!(retried.id, job.id);
        assert_eq!(retried.attempts, 1);
        q.commit(Some(&retried), vec![event("plugin.sample.done", "123")])
            .unwrap();
        assert_eq!(q.history().unwrap().len(), 2);
        let child = q.next().unwrap().unwrap();
        assert_eq!(child.key, "plugin.sample.done");
        q.commit(Some(&child), vec![]).unwrap();
        assert!(q.next().unwrap().is_none());
    }
}
