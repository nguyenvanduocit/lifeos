//! People: `people/<id>.md` holds who they are (relation, birthday, how often
//! to be in touch, notes); the `interactions` table holds every contact.

use std::collections::HashMap;

use anyhow::{Context, Result, bail};
use chrono::{Datelike, Local, NaiveDate};
use rusqlite::params;

use super::{Life, today};
use crate::vault::{Frontmatter, Kind};
use lifeos_wire::{DocSummary, Interaction, PersonDetail, PersonView};

input! {
    pub struct AddPerson {
        pub name: String,
        /// e.g. "gia đình", "bạn", "đồng nghiệp", "mentor".
        pub relation: Option<String>,
        /// YYYY-MM-DD, or MM-DD when the year is unknown.
        pub birthday: Option<String>,
        /// Be in touch at least this often, in days.
        pub contact_every_days: Option<u32>,
        #[serde(default)]
        pub tags: Vec<String>,
        /// What to remember about them, Markdown.
        pub notes: Option<String>,
    }
}

input! {
    /// Only the fields given change.
    pub struct UpdatePerson {
        pub id: String,
        pub name: Option<String>,
        pub relation: Option<String>,
        pub birthday: Option<String>,
        pub contact_every_days: Option<u32>,
        pub tags: Option<Vec<String>>,
        /// Appended to what lifeos remembers about them.
        pub add_notes: Option<String>,
    }
}

input! {
    pub struct LogInteraction {
        /// Person id (from list_people).
        pub person: String,
        /// e.g. "gặp", "gọi", "nhắn", "email".
        pub channel: Option<String>,
        /// What happened or was talked about.
        pub note: Option<String>,
        /// YYYY-MM-DD. Defaults to today.
        pub date: Option<NaiveDate>,
    }
}

input! {
    pub struct ListPeople {
        /// Only this relation.
        pub relation: Option<String>,
        /// Only people overdue for contact.
        #[serde(default)]
        pub overdue_only: bool,
    }
}

/// Month and day of a `YYYY-MM-DD` or `MM-DD` birthday.
fn birthday_month_day(s: &str) -> Option<(u32, u32)> {
    let parts: Vec<&str> = s.trim().split('-').collect();
    let (m, d) = match parts.as_slice() {
        [_, m, d] => (m, d),
        [m, d] => (m, d),
        _ => return None,
    };
    let (m, d) = (m.parse().ok()?, d.parse().ok()?);
    // 2024 is a leap year, so 02-29 is valid here.
    NaiveDate::from_ymd_opt(2024, m, d).map(|_| (m, d))
}

/// Days until the next birthday on or after `from` (29 Feb falls on 28 Feb in other years).
fn days_to_birthday(birthday: &str, from: NaiveDate) -> Option<u32> {
    let (m, d) = birthday_month_day(birthday)?;
    let on = |year: i32| NaiveDate::from_ymd_opt(year, m, d).or_else(|| NaiveDate::from_ymd_opt(year, m, d - 1));
    let this_year = on(from.year())?;
    let next = if this_year >= from { this_year } else { on(from.year() + 1)? };
    Some((next - from).num_days() as u32)
}

fn check_birthday(b: &Option<String>) -> Result<()> {
    if let Some(b) = b
        && birthday_month_day(b).is_none()
    {
        bail!("birthday must be YYYY-MM-DD or MM-DD, not '{b}'");
    }
    Ok(())
}

impl Life {
    pub fn add_person(&self, p: AddPerson) -> Result<DocSummary> {
        check_birthday(&p.birthday)?;
        let meta = Frontmatter {
            title: p.name.trim().to_string(),
            relation: p.relation,
            birthday: p.birthday,
            contact_every_days: p.contact_every_days,
            tags: p.tags,
            created: Some(Local::now()),
            ..Default::default()
        };
        Ok(super::summary(&self.create(Kind::Person, meta, p.notes.unwrap_or_default())?))
    }

    pub fn update_person(&self, u: UpdatePerson) -> Result<DocSummary> {
        check_birthday(&u.birthday)?;
        let mut doc = self.find(Kind::Person, &u.id)?;
        if let Some(name) = u.name {
            doc.meta.title = name.trim().to_string();
        }
        if u.relation.is_some() {
            doc.meta.relation = u.relation;
        }
        if u.birthday.is_some() {
            doc.meta.birthday = u.birthday;
        }
        if u.contact_every_days.is_some() {
            doc.meta.contact_every_days = u.contact_every_days;
        }
        if let Some(tags) = u.tags {
            doc.meta.tags = tags;
        }
        if let Some(notes) = u.add_notes {
            if !doc.body.is_empty() && !doc.body.ends_with('\n') {
                doc.body.push('\n');
            }
            doc.body.push_str(&format!("\n- {}: {}\n", today(), notes.trim()));
        }
        self.save(&doc)?;
        Ok(super::summary(&doc))
    }

    pub fn log_interaction(&self, i: LogInteraction) -> Result<Interaction> {
        let person = self.find(Kind::Person, &i.person)
            .with_context(|| format!("no person '{}'; list_people shows ids", i.person))?;
        let day = i.date.unwrap_or_else(today);
        self.conn.execute(
            "INSERT INTO interactions(id, person, day, channel, note)
                 VALUES ((SELECT MAX(COALESCE(MAX(id), 0),
                    COALESCE((SELECT highest FROM retired_row_ids WHERE table_name = 'interactions'), 0)) + 1 FROM interactions), ?1, ?2, ?3, ?4)",
            params![person.id, day.to_string(), i.channel, i.note],
        )?;
        Ok(Interaction {
            id: self.conn.last_insert_rowid(),
            person: person.id,
            day: day.to_string(),
            channel: i.channel,
            note: i.note,
        })
    }

    pub fn delete_interaction(&self, id: i64) -> Result<()> {
        if self.conn.execute("DELETE FROM interactions WHERE id = ?1", [id])? == 0 {
            bail!("no interaction {id}");
        }
        Ok(())
    }

    /// Everyone, overdue first (longest without contact first), then by name.
    pub fn people(&self, q: &ListPeople, on: NaiveDate) -> Result<Vec<PersonView>> {
        let last: HashMap<String, String> = {
            let mut stmt = self.conn.prepare("SELECT person, MAX(day) FROM interactions GROUP BY person")?;
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<rusqlite::Result<_>>()?
        };
        let mut stmt = self.conn.prepare(
            "SELECT id, title, relation, birthday, contact_every, tags FROM docs WHERE kind = 'person'",
        )?;
        let mut people: Vec<PersonView> = stmt
            .query_map([], |r| {
                let id: String = r.get(0)?;
                let birthday: Option<String> = r.get(3)?;
                let every: Option<u32> = r.get(4)?;
                let tags: String = r.get(5)?;
                let last_contact = last.get(&id).cloned();
                let days_since = last_contact
                    .as_deref()
                    .and_then(|d| d.parse::<NaiveDate>().ok())
                    .map(|d| (on - d).num_days().max(0) as u32);
                let overdue = match (every, days_since) {
                    (Some(every), Some(since)) => since >= every,
                    (Some(_), None) => true,
                    (None, _) => false,
                };
                Ok(PersonView {
                    days_to_birthday: birthday.as_deref().and_then(|b| days_to_birthday(b, on)),
                    id,
                    name: r.get(1)?,
                    relation: r.get(2)?,
                    birthday,
                    contact_every_days: every,
                    last_contact,
                    days_since_contact: days_since,
                    overdue,
                    tags: tags.split(',').filter(|t| !t.is_empty()).map(str::to_owned).collect(),
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        if let Some(rel) = &q.relation {
            people.retain(|p| p.relation.as_deref().is_some_and(|r| r.eq_ignore_ascii_case(rel)));
        }
        if q.overdue_only {
            people.retain(|p| p.overdue);
        }
        let lateness = |p: &PersonView| match (p.contact_every_days, p.days_since_contact) {
            (Some(every), Some(since)) => since as i64 - every as i64,
            (Some(_), None) => i64::MAX,
            _ => i64::MIN,
        };
        people.sort_by(|a, b| {
            b.overdue
                .cmp(&a.overdue)
                .then(lateness(b).cmp(&lateness(a)))
                .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        Ok(people)
    }

    pub fn person_detail(&self, id: &str) -> Result<PersonDetail> {
        let doc = self.find(Kind::Person, id)?;
        let person = self
            .people(&ListPeople::default(), today())?
            .into_iter()
            .find(|p| p.id == doc.id)
            .context("person not in the index")?;
        let mut stmt = self.conn.prepare(
            "SELECT id, person, day, channel, note FROM interactions WHERE person = ?1 ORDER BY day DESC, id DESC",
        )?;
        let interactions = stmt
            .query_map([&doc.id], |r| {
                Ok(Interaction { id: r.get(0)?, person: r.get(1)?, day: r.get(2)?, channel: r.get(3)?, note: r.get(4)? })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(PersonDetail { person, body: doc.body, interactions })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::life::test_support::*;

    #[test]
    fn birthdays_wrap_the_year_and_handle_29_february() {
        assert_eq!(days_to_birthday("1990-11-27", d("2026-11-25")), Some(2));
        assert_eq!(days_to_birthday("01-01", d("2026-12-31")), Some(1));
        assert_eq!(days_to_birthday("11-25", d("2026-11-25")), Some(0));
        assert_eq!(days_to_birthday("2000-02-29", d("2026-02-01")), Some(27)); // 28 Feb
        assert_eq!(birthday_month_day("13-01"), None);
        assert_eq!(birthday_month_day("hôm qua"), None);
    }

    #[test]
    fn overdue_contacts_come_first() {
        let (_dir, life) = life();
        let person = |name: &str, every: Option<u32>| AddPerson {
            name: name.into(),
            contact_every_days: every,
            ..Default::default()
        };
        life.add_person(person("Mẹ", Some(3))).unwrap();
        life.add_person(person("Bạn thân", Some(14))).unwrap();
        life.add_person(person("Đồng nghiệp", None)).unwrap();
        assert!(life.add_person(AddPerson { name: "X".into(), birthday: Some("bad".into()), ..Default::default() }).is_err());

        let log = |p: &str, day: &str| {
            life.log_interaction(LogInteraction { person: p.into(), date: Some(d(day)), ..Default::default() })
                .unwrap()
        };
        log("me", "2026-11-20"); // 5 days: over its 3
        log("ban-than", "2026-11-24"); // 1 day: fine
        let all = life.people(&ListPeople::default(), d("2026-11-25")).unwrap();
        assert_eq!(all.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Mẹ", "Bạn thân", "Đồng nghiệp"]);
        assert!(all[0].overdue && !all[1].overdue);
        assert_eq!(all[0].days_since_contact, Some(5));

        let due = life.people(&ListPeople { overdue_only: true, ..Default::default() }, d("2026-11-25")).unwrap();
        assert_eq!(due.len(), 1);
        assert!(life.log_interaction(LogInteraction { person: "nobody".into(), ..Default::default() }).is_err());
        assert_eq!(life.person_detail("me").unwrap().interactions.len(), 1);
    }
}
