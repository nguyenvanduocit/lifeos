//! The crew: agents the user asked for, each `crew/<id>.md` (frontmatter:
//! name, role, model, effort, tools; body: its instructions). Each runs as
//! its own Claude Code conversation with its own memory.

use anyhow::{Context, Result, bail};
use chrono::Local;

use super::Life;
use crate::vault::{Doc, Frontmatter, Kind};
use lifeos_wire::{ASSISTANT, CrewMember, LIBRARIAN};

choice! {
    pub enum Effort {
        Low => "low",
        Medium => "medium",
        High => "high",
        Xhigh => "xhigh",
        Max => "max",
    }
}

input! {
    pub struct AddMember {
        /// What the user calls it, e.g. "Coach Minh".
        pub name: String,
        /// What it does for the user, e.g. "huấn luyện viên sức khoẻ".
        pub role: String,
        /// How it works: its focus, tone, what to watch, when to speak up. Markdown, in second person.
        pub instructions: String,
        /// Claude Code model alias (e.g. "sonnet", "opus") or id. Omit for the default.
        pub model: Option<String>,
        pub effort: Option<Effort>,
        /// lifeos tools it may use (names from the tool list). Omit for every tool.
        #[serde(default)]
        pub tools: Vec<String>,
        /// Connection names this member may use: external MCP servers, and the built-in `google` and `desktop`.
        #[serde(default)]
        pub connections: Vec<String>,
    }
}

input! {
    /// Only the fields given change.
    pub struct UpdateMember {
        pub id: String,
        pub name: Option<String>,
        pub role: Option<String>,
        /// Replaces its instructions.
        pub instructions: Option<String>,
        pub model: Option<String>,
        pub effort: Option<Effort>,
        pub tools: Option<Vec<String>>,
        pub connections: Option<Vec<String>>,
    }
}

input! {
    pub struct MemberId {
        /// Crew member id (from list_crew).
        pub id: String,
    }
}

fn check_tools(tools: &[String]) -> Result<()> {
    for t in tools {
        if crate::tools::spec(t).is_none() {
            bail!("no lifeos tool '{t}'");
        }
    }
    Ok(())
}

impl Life {
    pub fn add_member(&self, m: AddMember) -> Result<CrewMember> {
        check_tools(&m.tools)?;
        self.check_connections(&m.connections)?;
        let name = m.name.trim().to_string();
        let plain: String = name.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase();
        if name.is_empty() || plain == ASSISTANT || plain == LIBRARIAN {
            bail!("pick another name");
        }
        let meta = Frontmatter {
            title: name,
            status: Some("active".into()),
            role: Some(m.role.trim().to_string()),
            model: m.model,
            effort: m.effort.map(|e| e.as_str().to_string()),
            tools: m.tools,
            connections: m.connections,
            created: Some(Local::now()),
            ..Default::default()
        };
        let doc = self.create(Kind::Crew, meta, m.instructions)?;
        self.crew_member(&doc)
    }

    pub fn update_member(&self, u: UpdateMember) -> Result<CrewMember> {
        let mut doc = self.find(Kind::Crew, &u.id)?;
        if let Some(name) = u.name {
            doc.meta.title = name.trim().to_string();
        }
        if let Some(role) = u.role {
            doc.meta.role = Some(role.trim().to_string());
        }
        if let Some(instructions) = u.instructions {
            doc.body = instructions;
        }
        if u.model.is_some() {
            doc.meta.model = u.model.filter(|m| !m.trim().is_empty());
        }
        if let Some(effort) = u.effort {
            doc.meta.effort = Some(effort.as_str().into());
        }
        if let Some(tools) = u.tools {
            check_tools(&tools)?;
            doc.meta.tools = tools;
        }
        if let Some(connections) = u.connections {
            self.check_connections(&connections)?;
            doc.meta.connections = connections;
        }
        doc.meta.status = Some("active".into());
        self.save(&doc)?;
        self.crew_member(&doc)
    }

    /// Stops it taking messages; its file and memories stay.
    pub fn retire_member(&self, id: &str) -> Result<CrewMember> {
        let mut doc = self.find(Kind::Crew, id)?;
        doc.meta.status = Some("retired".into());
        self.save(&doc)?;
        self.crew_member(&doc)
    }

    /// Active members first, then by name.
    pub fn crew(&self) -> Result<Vec<CrewMember>> {
        let mut crew = self.docs_where(Kind::Crew, "1", "status = 'retired', title")?
            .iter().map(|d| self.member(&d.id)).collect::<Result<Vec<_>>>()?;
        for doc in self.contributed_docs(Kind::Crew)? {crew.push(self.crew_member(&doc)?);}
        crew.sort_by(|a,b| (a.status=="retired",&a.name).cmp(&(b.status=="retired",&b.name)));
        Ok(crew)
    }

    pub fn member(&self, id: &str) -> Result<CrewMember> {
        let doc = self.find(Kind::Crew, id).with_context(|| format!("no crew member '{id}'; list_crew shows ids"))?;
        self.crew_member(&doc)
    }

    fn crew_member(&self, doc: &Doc) -> Result<CrewMember> {
        let m = &doc.meta;
        Ok(CrewMember {
            id: doc.id.clone(),
            name: m.title.clone(),
            role: m.role.clone(),
            instructions: doc.body.clone(),
            model: m.model.clone(),
            effort: m.effort.clone(),
            tools: m.tools.clone(),
            connections: m.connections.clone(),
            status: m.status.clone().unwrap_or_else(|| "active".into()),
            memories: self.memory_count(&doc.id)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::life::test_support::*;

    #[test]
    fn hire_change_retire() {
        let (_dir, life) = life();
        let hire = |name: &str, tools: Vec<String>| AddMember {
            name: name.into(),
            role: "huấn luyện viên sức khoẻ".into(),
            instructions: "Theo dõi giấc ngủ và chạy bộ.".into(),
            tools,
            ..Default::default()
        };
        assert!(life.add_member(hire("x", vec!["nope".into()])).is_err());
        assert!(life.add_member(hire("Assistant", vec![])).is_err());
        let m = life.add_member(hire("Coach Minh", vec!["habits".into(), "log_habit".into()])).unwrap();
        assert_eq!(m.id, "coach-minh");
        assert_eq!(m.tools.len(), 2);
        let m = life
            .update_member(UpdateMember { id: m.id, effort: Some(Effort::High), ..Default::default() })
            .unwrap();
        assert_eq!(m.effort.as_deref(), Some("high"));
        assert_eq!(life.retire_member(&m.id).unwrap().status, "retired");
        assert_eq!(life.crew().unwrap().len(), 1);
        assert!(life.member(&m.id).unwrap().instructions.contains("giấc ngủ"));
    }
}
