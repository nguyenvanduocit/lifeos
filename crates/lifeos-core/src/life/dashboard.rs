//! The dashboard: every area of a life at a glance, in one read.

use anyhow::Result;
use chrono::NaiveDate;

use super::Life;
use super::goals::{GoalStatus, ListGoals};
use super::habits::ListHabits;
use super::health::SERIES_DAYS;
use super::people::ListPeople;
use lifeos_wire::Dashboard;

/// Birthdays this close (in days) show on the dashboard.
pub const BIRTHDAYS_WITHIN: u32 = 30;
const JOURNAL_DAYS: u32 = 7;
const NOTES: u32 = 6;

impl Life {
    pub fn dashboard(&self, day: NaiveDate) -> Result<Dashboard> {
        let people = self.people(&ListPeople::default(), day)?;
        let mut birthdays: Vec<_> = people
            .iter()
            .filter(|p| p.days_to_birthday.is_some_and(|d| d <= BIRTHDAYS_WITHIN))
            .cloned()
            .collect();
        birthdays.sort_by_key(|p| p.days_to_birthday);
        Ok(Dashboard {
            date: day.to_string(),
            weekday: day.format("%A").to_string(),
            brief: self.brief(day)?,
            tasks: self.tasks_view(day)?,
            goals: self.goal_views(&ListGoals { status: Some(GoalStatus::Active) })?,
            habits: self.habits(&ListHabits::default(), day)?,
            people_due: people.into_iter().filter(|p| p.overdue).collect(),
            birthdays,
            money: self.money_month(None, day)?,
            health: self.metric_series(SERIES_DAYS, day)?,
            journal: self.journal_recent(JOURNAL_DAYS)?,
            journal_written_today: self.journal_written(day),
            notes: self.recent_notes(NOTES)?,
            inbox: self.inbox()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::life::people::AddPerson;
    use crate::life::test_support::*;

    #[test]
    fn an_empty_life_and_a_birthday() {
        let (_dir, life) = life();
        let empty = life.dashboard(d("2026-11-25")).unwrap();
        assert_eq!(empty.weekday, "Wednesday");
        assert!(empty.brief.is_none() && empty.habits.is_empty() && empty.money.primary_currency.is_none());

        life.add_person(AddPerson { name: "Mẹ".into(), birthday: Some("12-01".into()), ..Default::default() })
            .unwrap();
        life.add_person(AddPerson { name: "Bố".into(), birthday: Some("06-01".into()), ..Default::default() })
            .unwrap();
        let dash = life.dashboard(d("2026-11-25")).unwrap();
        assert_eq!(dash.birthdays.len(), 1);
        assert_eq!(dash.birthdays[0].days_to_birthday, Some(6));
    }
}
