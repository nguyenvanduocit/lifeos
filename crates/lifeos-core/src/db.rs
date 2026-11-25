//! SQLite at `<home>/.lifeos/lifeos.db`.
//!
//! Two roles, kept apart:
//! - **Index** (`docs`, `docs_fts`): derived from the Markdown vault, safe to
//!   drop and rebuild with `lifeos reindex`.
//! - **Records** (`habit_logs`, `transactions`, `metrics`, `kv`): numeric data
//!   whose source of truth is this database. Back it up.

use std::path::Path;

use anyhow::Result;
use rusqlite::Connection;

const MIGRATIONS: &[&str] = &[
    // v1
    r#"
    CREATE TABLE docs(
        path    TEXT PRIMARY KEY,
        kind    TEXT NOT NULL,
        id      TEXT NOT NULL,
        title   TEXT NOT NULL,
        status  TEXT,
        due     TEXT,
        goal    TEXT,
        horizon TEXT,
        tags    TEXT NOT NULL DEFAULT '',
        mtime   INTEGER NOT NULL
    );
    CREATE INDEX docs_kind_status ON docs(kind, status);
    CREATE VIRTUAL TABLE docs_fts USING fts5(
        path UNINDEXED, title, body,
        tokenize = 'unicode61 remove_diacritics 2'
    );

    CREATE TABLE habit_logs(
        habit TEXT NOT NULL,
        day   TEXT NOT NULL,
        value REAL NOT NULL DEFAULT 1,
        note  TEXT,
        PRIMARY KEY(habit, day)
    );

    -- amount_minor: signed, in the currency's minor unit (VND: đồng, USD: cents).
    -- Negative = money out, positive = money in.
    CREATE TABLE transactions(
        id           INTEGER PRIMARY KEY,
        day          TEXT NOT NULL,
        amount_minor INTEGER NOT NULL,
        currency     TEXT NOT NULL,
        category     TEXT NOT NULL,
        note         TEXT
    );
    CREATE INDEX transactions_day ON transactions(day);

    CREATE TABLE metrics(
        id    INTEGER PRIMARY KEY,
        day   TEXT NOT NULL,
        name  TEXT NOT NULL,
        value REAL NOT NULL,
        unit  TEXT,
        note  TEXT
    );
    CREATE INDEX metrics_name_day ON metrics(name, day);

    CREATE TABLE kv(key TEXT PRIMARY KEY, value TEXT NOT NULL);
    "#,
];

pub fn open(path: &Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")?;
    migrate(&mut conn)?;
    Ok(conn)
}

pub fn open_in_memory() -> Result<Connection> {
    let mut conn = Connection::open_in_memory()?;
    migrate(&mut conn)?;
    Ok(conn)
}

fn migrate(conn: &mut Connection) -> Result<()> {
    let current: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.execute_batch(&format!("PRAGMA user_version = {}", i + 1))?;
        tx.commit()?;
    }
    Ok(())
}
