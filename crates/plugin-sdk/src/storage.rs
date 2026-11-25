//! Versioned schema migrations for trusted plugins sharing the LifeOS vault.
use crate::Error;
use rusqlite::{Connection, OpenFlags, OptionalExtension, Transaction, TransactionBehavior};
use std::{path::Path, time::Duration};

/// Apply missing versions in a single write transaction. Use plugin-prefixed SQL
/// names and leave the vault's PRAGMA user_version to LifeOS core.
pub fn open(
    path: &Path,
    plugin: &str,
    version: u32,
    mut migrate: impl FnMut(&Transaction<'_>, u32) -> Result<(), Error>,
) -> Result<Connection, Error> {
    let mut db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    db.busy_timeout(Duration::from_secs(5))?;
    db.pragma_update(None, "foreign_keys", true)?;
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    tx.execute_batch("CREATE TABLE IF NOT EXISTS lifeos_plugin_schemas(plugin TEXT PRIMARY KEY, version INTEGER NOT NULL)")?;
    let current: u32 = tx
        .query_row(
            "SELECT version FROM lifeos_plugin_schemas WHERE plugin=?",
            [plugin],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or(0);
    if current > version {
        return Err(format!("Schema for {plugin} is newer than supported").into());
    }
    for next in (current..version).map(|v| v + 1) {
        migrate(&tx, next)?;
    }
    if current < version {
        tx.execute("INSERT INTO lifeos_plugin_schemas(plugin,version) VALUES(?,?) ON CONFLICT(plugin) DO UPDATE SET version=excluded.version", (plugin,version))?;
    }
    tx.commit()?;
    Ok(db)
}
