//! Writing: the journal (`journal/YYYY/YYYY-MM-DD.md`, one file a day),
//! notes (`notes/<id>.md`) and reviews the agent writes (`reviews/`): the
//! morning brief and weekly or monthly reviews.

use anyhow::Result;
use chrono::{Duration, Local, NaiveDate};

use super::{Life, full, summary, today};
use crate::vault::{Doc, Frontmatter, Kind, Vault};
use lifeos_wire::{DocFull, DocSummary, JournalExcerpt};

input! {
    pub struct WriteJournal {
        /// Markdown, in the user's own voice. Appended under a time heading.
        pub text: String,
        /// YYYY-MM-DD. Defaults to today.
        pub date: Option<NaiveDate>,
    }
}

input! {
    pub struct SaveNote {
        pub title: String,
        /// Markdown.
        pub body: String,
        #[serde(default)]
        pub tags: Vec<String>,
    }
}

input! {
    pub struct UpdateDocument {
        pub id: String,
        pub title: Option<String>,
        pub body: Option<String>,
        pub tags: Option<Vec<String>>,
    }
}

input! {
    pub struct Recent {
        /// How many. Defaults to 10.
        pub limit: Option<u32>,
    }
}

choice! {
    pub enum ReviewKind {
        /// The short morning brief shown at the top of the dashboard.
        Brief => "brief",
        Weekly => "weekly",
        Monthly => "monthly",
    }
}

input! {
    /// One review per kind per date: saving again replaces it.
    pub struct SaveReview {
        /// Defaults to brief.
        pub kind: Option<ReviewKind>,
        pub title: String,
        /// Markdown.
        pub body: String,
        /// YYYY-MM-DD. Defaults to today.
        pub date: Option<NaiveDate>,
    }
}

const EXCERPT_CHARS: usize = 280;

fn excerpt(body: &str) -> String {
    let text = body
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect::<Vec<_>>()
        .join(" ");
    match text.char_indices().nth(EXCERPT_CHARS) {
        Some((i, _)) => format!("{}…", text[..i].trim_end()),
        None => text,
    }
}

impl Life {
    pub fn write_journal(&self, j: WriteJournal) -> Result<DocSummary> {
        let day = j.date.unwrap_or_else(today);
        let rel = Vault::journal_rel_path(day);
        let mut doc = if self.vault.abs(&rel).exists() {
            self.vault.read(Kind::Journal, &rel)?
        } else {
            Doc {
                kind: Kind::Journal,
                id: day.to_string(),
                rel_path: rel,
                meta: Frontmatter {
                    title: format!("Nhật ký {day}"),
                    created: Some(Local::now()),
                    ..Default::default()
                },
                body: String::new(),
            }
        };
        if !doc.body.is_empty() && !doc.body.ends_with('\n') {
            doc.body.push('\n');
        }
        doc.body.push_str(&format!("\n## {}\n\n{}\n", Local::now().format("%H:%M"), j.text.trim()));
        self.save(&doc)?;
        Ok(summary(&doc))
    }

    pub fn read_journal(&self, day: NaiveDate) -> Result<Option<DocFull>> {
        let rel = Vault::journal_rel_path(day);
        if !self.vault.abs(&rel).exists() {
            return Ok(None);
        }
        Ok(Some(full(self.vault.read(Kind::Journal, &rel)?)))
    }

    pub fn journal_written(&self, day: NaiveDate) -> bool {
        self.vault.abs(&Vault::journal_rel_path(day)).exists()
    }

    /// The latest journal days, newest first.
    pub fn journal_recent(&self, limit: u32) -> Result<Vec<JournalExcerpt>> {
        self.docs_where(Kind::Journal, "1", &format!("id DESC LIMIT {limit}"))?
            .into_iter()
            .map(|d| {
                let doc = self.vault.read(Kind::Journal, &d.path)?;
                Ok(JournalExcerpt {
                    day: d.id,
                    excerpt: excerpt(&doc.body),
                    words: doc
                        .body
                        .lines()
                        .filter(|l| !l.trim_start().starts_with('#'))
                        .flat_map(str::split_whitespace)
                        .count() as u32,
                })
            })
            .collect()
    }

    pub fn save_note(&self, n: SaveNote) -> Result<DocSummary> {
        let meta = Frontmatter {
            title: n.title.trim().to_string(),
            tags: n.tags,
            created: Some(Local::now()),
            ..Default::default()
        };
        Ok(summary(&self.create(Kind::Note, meta, n.body)?))
    }

    pub fn update_document(&self, input: UpdateDocument) -> Result<DocSummary> {
        let mut doc = self.resolve_doc(&input.id)?;
        anyhow::ensure!(
            matches!(doc.kind, Kind::Note | Kind::Journal | Kind::Review),
            "Use the entity's own update tool for {}",
            doc.kind.as_str()
        );
        if let Some(title) = input.title {
            doc.meta.title = title.trim().to_owned();
        }
        if let Some(body) = input.body {
            doc.body = body;
        }
        if let Some(tags) = input.tags {
            doc.meta.tags = tags;
        }
        self.save(&doc)?;
        Ok(summary(&doc))
    }

    /// Most recently changed first.
    pub fn recent_notes(&self, limit: u32) -> Result<Vec<DocSummary>> {
        self.docs_where(Kind::Note, "1", &format!("mtime DESC LIMIT {limit}"))
    }

    /// The review's kind is kept in `status`.
    pub fn save_review(&self, r: SaveReview) -> Result<DocSummary> {
        let day = r.date.unwrap_or_else(today);
        let kind = r.kind.unwrap_or(ReviewKind::Brief);
        let rel = format!("{}/{day}-{}.md", crate::vault::folder(Kind::Review), kind.as_str());
        let meta = Frontmatter {
            title: r.title.trim().to_string(),
            status: Some(kind.as_str().into()),
            created: Some(Local::now()),
            ..Default::default()
        };
        Ok(summary(&self.create_at(Kind::Review, rel, meta, r.body)?))
    }

    /// Newest first.
    pub fn reviews(&self, kind: Option<ReviewKind>, limit: u32) -> Result<Vec<DocSummary>> {
        let cond = match kind {
            Some(k) => format!("status = '{}'", k.as_str()),
            None => "1".into(),
        };
        self.docs_where(Kind::Review, &cond, &format!("id DESC LIMIT {limit}"))
    }

    /// The brief written for `day`, or yesterday's when today's is not written yet.
    pub fn brief(&self, day: NaiveDate) -> Result<Option<DocFull>> {
        for d in [day, day - Duration::days(1)] {
            let rel = format!("{}/{d}-brief.md", crate::vault::folder(Kind::Review));
            if self.vault.abs(&rel).exists() {
                return Ok(Some(full(self.vault.read(Kind::Review, &rel)?)));
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::life::test_support::*;

    #[test]
    fn journal_appends_and_excerpts() {
        let (_dir, life) = life();
        let day = Some(d("2026-11-24"));
        life.write_journal(WriteJournal { text: "Sáng chạy bộ.".into(), date: day }).unwrap();
        life.write_journal(WriteJournal { text: "Tối đọc sách.".into(), date: day }).unwrap();
        life.write_journal(WriteJournal { text: "Hôm nay.".into(), date: Some(d("2026-11-25")) }).unwrap();
        let body = life.read_journal(d("2026-11-24")).unwrap().unwrap().body;
        assert!(body.contains("Sáng chạy bộ.") && body.contains("Tối đọc sách."));
        let recent = life.journal_recent(7).unwrap();
        assert_eq!(recent.iter().map(|j| j.day.as_str()).collect::<Vec<_>>(), ["2026-11-25", "2026-11-24"]);
        assert_eq!(recent[1].excerpt, "Sáng chạy bộ. Tối đọc sách.");
        assert_eq!(recent[1].words, 6);
        assert!(!life.journal_written(d("2026-11-23")));
        assert_eq!(excerpt(&"á".repeat(400)).chars().count(), EXCERPT_CHARS + 1);
    }

    #[test]
    fn a_brief_per_day_replaces_itself() {
        let (_dir, life) = life();
        let brief = |body: &str, day: &str| SaveReview {
            kind: None,
            title: "Sáng nay".into(),
            body: body.into(),
            date: Some(d(day)),
        };
        life.save_review(brief("cũ", "2026-11-24")).unwrap();
        assert_eq!(life.brief(d("2026-11-25")).unwrap().unwrap().body, "cũ");
        life.save_review(brief("một", "2026-11-25")).unwrap();
        life.save_review(brief("hai", "2026-11-25")).unwrap();
        assert_eq!(life.brief(d("2026-11-25")).unwrap().unwrap().body, "hai");
        assert_eq!(life.reviews(Some(ReviewKind::Brief), 10).unwrap().len(), 2);
        assert!(life.reviews(Some(ReviewKind::Weekly), 10).unwrap().is_empty());
        life.save_note(SaveNote { title: "Ý tưởng".into(), body: "x".into(), tags: vec![] }).unwrap();
        assert_eq!(life.recent_notes(5).unwrap().len(), 1);
    }
}
