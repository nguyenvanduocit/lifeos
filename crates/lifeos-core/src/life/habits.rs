//! Habits: definitions in `habits`, one record per habit per day in `habit_logs`.

use std::collections::HashMap;

use anyhow::{Result, bail};
use chrono::{Datelike, Duration, NaiveDate};
use rusqlite::{OptionalExtension, params};

use super::{Life, key, today};
use lifeos_wire::{HABIT_RECENT_DAYS, HabitStatus};

input! {
    /// Creates the habit, or changes it (and brings it back if archived).
    pub struct DefineHabit {
        /// Short stable name, e.g. "thiền", "chạy bộ", "đọc sách".
        pub name: String,
        /// What a logged value counts: "phút", "trang", "km"… Omit for done/not done.
        pub unit: Option<String>,
        /// Days a week to aim for, 1–7. Defaults to 7.
        pub target_per_week: Option<u32>,
    }
}

input! {
    pub struct HabitName {
        pub name: String,
    }
}

input! {
    pub struct LogHabit {
        pub habit: String,
        /// YYYY-MM-DD. Defaults to today.
        pub date: Option<NaiveDate>,
        /// Amount done (minutes, pages, km…). Defaults to 1. 0 un-marks the day.
        pub value: Option<f64>,
        pub note: Option<String>,
    }
}

input! {
    pub struct RenameHabit {
        /// A habit ID or its existing name.
        pub id: String,
        pub name: String,
    }
}

input! {
    pub struct ListHabits {
        /// Also habits the user stopped.
        #[serde(default)]
        pub include_archived: bool,
    }
}

impl Life {
    fn habit_name(&self, reference: &str) -> Result<Option<String>> {
        let name = key(reference);
        Ok(self.conn.query_row(
            "SELECT name FROM habits
             WHERE entity_id = ?1 OR name = ?1 OR entity_id = (SELECT entity_id FROM habit_aliases WHERE name = ?1)
             ORDER BY CASE WHEN entity_id = ?1 THEN 0 WHEN name = ?1 THEN 1 ELSE 2 END LIMIT 1",
            [&name],
            |row| row.get(0),
        ).optional()?)
    }

    fn habit_name_for_write(&self, reference: &str) -> Result<String> {
        if let Some(name) = self.habit_name(reference)? {
            return Ok(name);
        }
        let name = key(reference);
        anyhow::ensure!(!name.is_empty(), "habit name is empty");
        anyhow::ensure!(!name.starts_with("habit_"), "unknown habit ID or reserved habit name");
        Ok(name)
    }

    pub fn habit(&self, reference: &str) -> Result<HabitStatus> {
        let name = self.habit_name(reference)?.ok_or_else(|| anyhow::anyhow!("no habit '{reference}'"))?;
        self.habit_status(&name, today())
    }

    pub fn rename_habit(&self, input: RenameHabit) -> Result<HabitStatus> {
        let old = self.habit_name(&input.id)?.ok_or_else(|| anyhow::anyhow!("no habit '{}'", input.id))?;
        let name = key(&input.name);
        anyhow::ensure!(!name.is_empty() && !name.starts_with("habit_"), "invalid habit name");
        if old != name {
            let occupied = self.habit_name(&name)?;
            anyhow::ensure!(occupied.as_ref().is_none_or(|owner| owner == &old), "habit name is already in use");
            self.conn.execute("UPDATE habits SET name = ?1 WHERE name = ?2", params![name, old])?;
        }
        self.habit_status(&name, today())
    }

    pub fn define_habit(&self, h: DefineHabit) -> Result<HabitStatus> {
        let name = self.habit_name_for_write(&h.name)?;
        let target = h.target_per_week.unwrap_or(7);
        if !(1..=7).contains(&target) {
            bail!("target_per_week is 1–7");
        }
        self.conn.execute(
            "INSERT INTO habits(name, unit, target_per_week, created, entity_id)
             VALUES (?1, ?2, ?3, ?4, 'habit_' || lower(hex(randomblob(12))))
             ON CONFLICT(name) DO UPDATE SET unit = COALESCE(?2, unit), target_per_week = ?3, archived = 0",
            params![name, h.unit, target, today().to_string()],
        )?;
        self.habit_status(&name, today())
    }

    pub fn archive_habit(&self, name: &str) -> Result<()> {
        let resolved = self.habit_name(name)?;
        if self.conn.execute("UPDATE habits SET archived = 1 WHERE name = ?1", [resolved])? == 0 {
            bail!("no habit '{name}'");
        }
        Ok(())
    }

    pub fn log_habit(&self, l: LogHabit) -> Result<HabitStatus> {
        let name = self.habit_name_for_write(&l.habit)?;
        let day = l.date.unwrap_or_else(today);
        self.conn.execute(
            "INSERT INTO habits(name, created, entity_id) VALUES (?1, ?2, 'habit_' || lower(hex(randomblob(12)))) ON CONFLICT(name) DO UPDATE SET archived = 0",
            params![name, day.to_string()],
        )?;
        self.conn.execute(
            "INSERT INTO habit_logs(habit, day, value, note) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(habit, day) DO UPDATE SET value = ?3, note = ?4",
            params![name, day.to_string(), l.value.unwrap_or(1.0), l.note],
        )?;
        self.habit_status(&name, today())
    }

    pub fn habits(&self, q: &ListHabits, on: NaiveDate) -> Result<Vec<HabitStatus>> {
        let names: Vec<String> = {
            let mut stmt = self.conn.prepare(
                "SELECT name FROM habits WHERE ?1 OR archived = 0 ORDER BY created, name",
            )?;
            stmt.query_map([q.include_archived], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?
        };
        names.iter().map(|h| self.habit_status(h, on)).collect()
    }

    pub(super) fn habit_status(&self, habit: &str, on: NaiveDate) -> Result<HabitStatus> {
        let (id, unit, target): (String, Option<String>, u32) = self.conn.query_row(
            "SELECT entity_id, unit, target_per_week FROM habits WHERE name = ?1",
            [habit],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        let logs: HashMap<NaiveDate, f64> = {
            let mut stmt = self.conn.prepare(
                "SELECT day, value FROM habit_logs WHERE habit = ?1 AND day <= ?2 AND value > 0",
            )?;
            stmt.query_map(params![habit, on.to_string()], |r| Ok((r.get::<_, String>(0)?, r.get(1)?)))?
                .filter_map(|row| {
                    let (day, value) = row.ok()?;
                    Some((day.parse().ok()?, value))
                })
                .collect()
        };
        let done_today = logs.contains_key(&on);
        // A streak survives until the end of today: count from today if done,
        // otherwise from yesterday.
        let mut cursor = if done_today { on } else { on - Duration::days(1) };
        let mut streak = 0;
        while logs.contains_key(&cursor) {
            streak += 1;
            cursor -= Duration::days(1);
        }
        let monday = on - Duration::days(on.weekday().num_days_from_monday() as i64);
        let done_this_week = logs.keys().filter(|d| **d >= monday).count() as u32;
        let recent = (0..HABIT_RECENT_DAYS as i64)
            .rev()
            .map(|back| logs.get(&(on - Duration::days(back))).copied().unwrap_or(0.0))
            .collect();
        Ok(HabitStatus {
            id: Some(id),
            habit: habit.to_string(),
            unit,
            target_per_week: target,
            done_today,
            streak,
            done_this_week,
            recent,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::life::test_support::*;

    fn log(life: &Life, habit: &str, day: &str) {
        life.log_habit(LogHabit { habit: habit.into(), date: Some(d(day)), ..Default::default() }).unwrap();
    }

    #[test]
    fn streaks_week_counts_and_heatmap() {
        let (_dir, life) = life();
        life.define_habit(DefineHabit { name: "Thiền".into(), unit: Some("phút".into()), target_per_week: Some(5) })
            .unwrap();
        for day in ["2026-11-21", "2026-11-23", "2026-11-24"] {
            log(&life, "thiền", day);
        }
        // 2026-11-25 is a Wednesday.
        let s = life.habit_status("thiền", d("2026-11-25")).unwrap();
        assert!(!s.done_today);
        assert_eq!(s.streak, 2);
        assert_eq!(s.done_this_week, 2); // Mon 23, Tue 24
        assert_eq!(s.target_per_week, 5);
        assert_eq!(s.unit.as_deref(), Some("phút"));
        assert_eq!(s.recent.len(), HABIT_RECENT_DAYS);
        assert_eq!(&s.recent[HABIT_RECENT_DAYS - 3..], &[1.0, 1.0, 0.0]);

        log(&life, "thiền", "2026-11-25");
        assert_eq!(life.habit_status("thiền", d("2026-11-25")).unwrap().streak, 3);
    }

    #[test]
    fn archived_habits_hide_until_logged_again() {
        let (_dir, life) = life();
        log(&life, "Đọc sách", "2026-11-25");
        assert!(life.define_habit(DefineHabit { name: "x".into(), target_per_week: Some(8), ..Default::default() }).is_err());
        life.archive_habit("đọc sách").unwrap();
        let on = d("2026-11-25");
        assert!(life.habits(&ListHabits::default(), on).unwrap().is_empty());
        assert_eq!(life.habits(&ListHabits { include_archived: true }, on).unwrap().len(), 1);
        log(&life, "đọc sách", "2026-11-25");
        assert_eq!(life.habits(&ListHabits::default(), on).unwrap().len(), 1);
    }
}
