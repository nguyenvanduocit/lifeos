//! Account metadata and health live in SQLite. The Claude profiles and their
//! credentials are managed by the agent runtime outside the vault.

use std::path::PathBuf;

use anyhow::{Context, Result, ensure};
use lifeos_wire::AgentAccount;
use rusqlite::{OptionalExtension, params};

use super::Life;

pub const DEFAULT_ACCOUNT: &str = "default";

impl Life {
    pub fn account_profiles_dir(&self) -> Result<PathBuf> {
        self.private_config_dir("LIFEOS_ACCOUNTS_DIR", "accounts")?
            .context("configure a private account directory outside the vault")
    }

    pub fn agent_accounts(&self) -> Result<Vec<AgentAccount>> {
        let mut stmt = self.conn.prepare(
            "SELECT id,label,kind,status,reason,quota,utilization,resets_at,retry_at
             FROM agent_accounts ORDER BY kind != 'default', rowid",
        )?;
        Ok(stmt
            .query_map([], |row| {
                Ok(AgentAccount {
                    id: row.get(0)?,
                    label: row.get(1)?,
                    kind: row.get(2)?,
                    status: row.get(3)?,
                    reason: row.get(4)?,
                    quota: row.get(5)?,
                    utilization: row.get(6)?,
                    resets_at: row.get(7)?,
                    retry_at: row.get(8)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// A successful sign-in for the same address replaces its previous login.
    pub fn keep_agent_account(&self, id: &str, label: &str, kind: &str) -> Result<Option<String>> {
        let tx = self.conn.unchecked_transaction()?;
        let previous: Option<String> = if kind == "login" {
            tx.query_row(
                "SELECT id FROM agent_accounts WHERE kind='login' AND label=?1",
                [label],
                |row| row.get(0),
            )
            .optional()?
        } else {
            None
        };
        if let Some(old) = previous.as_deref() {
            tx.execute("DELETE FROM agent_accounts WHERE id=?1", [old])?;
        }
        tx.execute(
            "INSERT INTO agent_accounts(id,label,kind) VALUES (?1,?2,?3)",
            params![id, label, kind],
        )?;
        tx.commit()?;
        Ok(previous)
    }

    pub fn label_default_account(&self, label: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE agent_accounts SET label=?1 WHERE id='default'",
            [label],
        )?;
        Ok(())
    }

    pub fn replace_token_account(&self, previous: &str, id: &str, label: &str) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        let changed = tx.execute(
            "DELETE FROM agent_accounts WHERE id=?1 AND kind='connection'",
            [previous],
        )?;
        ensure!(changed == 1, "unknown Claude connection");
        tx.execute(
            "INSERT INTO agent_accounts(id,label,kind) VALUES (?1,?2,'connection')",
            params![id, label],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn remove_agent_account(&self, id: &str) -> Result<()> {
        ensure!(
            id != DEFAULT_ACCOUNT,
            "the default account belongs to this machine"
        );
        let changed = self
            .conn
            .execute("DELETE FROM agent_accounts WHERE id=?1", [id])?;
        ensure!(changed != 0, "unknown account");
        Ok(())
    }

    pub fn save_agent_account_health(&self, account: &AgentAccount) -> Result<()> {
        self.conn.execute(
            "UPDATE agent_accounts SET status=?2,reason=?3,quota=?4,utilization=?5,resets_at=?6,retry_at=?7 WHERE id=?1",
            params![account.id, account.status, account.reason, account.quota, account.utilization, account.resets_at, account.retry_at],
        )?;
        Ok(())
    }
}
