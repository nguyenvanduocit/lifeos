//! Desktop use: agents see and work the owner's Macs through LifeOS Computer, an app on each that dials the link
//! (`lifeos_wire::computer`). The tools are `lifeos-desktop`'s; the agents' server sends each call to its computer.
//! Here: the computers that ever connected, kept so the owner sees one that is away, and the connection name crew are
//! granted them by.

use anyhow::Result;
use lifeos_wire::{ComputerHello, ComputerView};
use rusqlite::params;

use super::Life;
use super::cards::now;

input! {
    pub struct ComputerName {
        /// The computer's name or id.
        pub computer: String,
    }
}

/// The connection name crew are granted the computers by; their tools reach agents as `mcp__desktop__<tool>`.
pub const CONNECTION: &str = "desktop";

impl Life {
    /// A computer connected, or told what changed: kept as it says, online.
    pub fn computer_hello(&self, hello: &ComputerHello) -> Result<()> {
        self.conn.execute(
            "INSERT INTO computers(id, name, version, accessibility, screen_recording, width, height, paused, online, seen)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9)
             ON CONFLICT(id) DO UPDATE SET name=excluded.name, version=excluded.version,
                accessibility=excluded.accessibility, screen_recording=excluded.screen_recording, width=excluded.width,
                height=excluded.height, paused=excluded.paused, online=1, seen=excluded.seen",
            params![hello.id, hello.name, hello.version, hello.accessibility, hello.screen_recording, hello.width,
                hello.height, hello.paused, now()],
        )?;
        Ok(())
    }

    /// Computer `id` went away.
    pub fn computer_gone(&self, id: &str) -> Result<()> {
        self.conn.execute("UPDATE computers SET online=0, seen=?2 WHERE id=?1", params![id, now()])?;
        Ok(())
    }

    /// Every computer away: the server starts with none connected.
    pub fn computers_all_gone(&self) -> Result<()> {
        self.conn.execute("UPDATE computers SET online=0 WHERE online=1", [])?;
        Ok(())
    }

    /// Forgets computer `id`; it comes back if it connects again.
    pub fn forget_computer(&self, id: &str) -> Result<()> {
        let changed = self.conn.execute("DELETE FROM computers WHERE id=?1 OR name=?1", [id])?;
        anyhow::ensure!(changed != 0, "no computer {id}");
        Ok(())
    }

    /// The computers, online first, then by name.
    pub fn computers(&self) -> Result<Vec<ComputerView>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, online, version, accessibility, screen_recording, width, height, paused, seen
             FROM computers ORDER BY online DESC, name",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(ComputerView {
                id: r.get(0)?,
                name: r.get(1)?,
                online: r.get(2)?,
                version: r.get(3)?,
                accessibility: r.get(4)?,
                screen_recording: r.get(5)?,
                width: r.get(6)?,
                height: r.get(7)?,
                paused: r.get(8)?,
                seen: r.get(9)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_computer_is_kept_as_it_says_and_marked_away_when_it_goes() {
        let dir = tempfile::tempdir().unwrap();
        let life = Life::open_with_memory_db(dir.path()).unwrap();
        let mut hello = ComputerHello { id: "c1".into(), name: "Studio".into(), accessibility: true, ..Default::default() };
        life.computer_hello(&hello).unwrap();
        hello.name = "Studio 2".into();
        life.computer_hello(&hello).unwrap();
        let all = life.computers().unwrap();
        assert_eq!((all.len(), all[0].name.as_str(), all[0].online), (1, "Studio 2", true));
        life.computer_gone("c1").unwrap();
        assert!(!life.computers().unwrap()[0].online);
        life.forget_computer("Studio 2").unwrap();
        assert!(life.computers().unwrap().is_empty());
    }
}
