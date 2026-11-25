//! Questions agents put to the owner (remit's ADR-0087: no agent waits on the owner). An agent asks with `ask_owner`;
//! the Assistant decides it for him (`decide`) or leaves it to him (`leave`) when only he can do it; he may answer or
//! overturn through the Assistant (`decide` with his word). One question per asker waits at a time: asking again
//! withdraws the one before.

use anyhow::{Result, bail};
use rusqlite::{OptionalExtension, Row, params};

use super::Life;
use super::cards::now;
use lifeos_wire::{Actor, ActorKind, Question, QuestionStatus};

const COLUMNS: &str = "id, asker, asker_name, text, status, answer, why, decided_by, overturned, created, decided";

input! {
    pub struct ListQuestions {
        /// Only those still waiting: not decided yet, or left to the owner.
        #[serde(default)]
        pub waiting: bool,
    }
}

/// The questions a list shows, at most.
const LISTED: u32 = 200;

fn question(r: &Row) -> rusqlite::Result<Question> {
    let status: String = r.get(4)?;
    let by: Option<String> = r.get(7)?;
    Ok(Question {
        id: r.get(0)?,
        asker: r.get(1)?,
        asker_name: r.get(2)?,
        text: r.get(3)?,
        status: QuestionStatus::parse(&status).unwrap_or_default(),
        answer: r.get(5)?,
        why: r.get(6)?,
        decided_by: by.and_then(|b| serde_json::from_str(&b).ok()),
        overturned: r.get(8)?,
        created: r.get(9)?,
        decided: r.get(10)?,
    })
}

impl Life {
    /// Agent `asker` asks the owner `text`; a question it asked before that still waits is withdrawn.
    pub fn ask_owner(&self, asker: &str, asker_name: &str, text: &str) -> Result<Question> {
        let text = text.trim();
        if text.is_empty() {
            bail!("say what you ask");
        }
        self.conn.execute(
            "UPDATE questions SET status = 'withdrawn', decided = ?2 WHERE asker = ?1 AND status IN ('open', 'left')",
            params![asker, now()],
        )?;
        self.conn.execute(
            "INSERT INTO questions(asker, asker_name, text, created) VALUES (?1, ?2, ?3, ?4)",
            params![asker, asker_name, text, now()],
        )?;
        self.question(self.conn.last_insert_rowid())
    }

    pub fn question(&self, id: i64) -> Result<Question> {
        self.conn
            .query_row(&format!("SELECT {COLUMNS} FROM questions WHERE id = ?1"), [id], question)
            .optional()?
            .ok_or_else(|| anyhow::anyhow!("no question {id}; list_questions shows them"))
    }

    /// The newest questions first; only those still waiting (open or left to the owner) when `waiting`.
    pub fn questions(&self, waiting: bool) -> Result<Vec<Question>> {
        let filter = if waiting { "WHERE status IN ('open', 'left')" } else { "" };
        let mut stmt =
            self.conn.prepare(&format!("SELECT {COLUMNS} FROM questions {filter} ORDER BY id DESC LIMIT {LISTED}"))?;
        Ok(stmt.query_map([], question)?.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// The ids of the questions agent `asker` waits on.
    pub fn waiting_on(&self, asker: &str) -> Result<Vec<i64>> {
        let mut stmt =
            self.conn.prepare("SELECT id FROM questions WHERE asker = ?1 AND status IN ('open', 'left') ORDER BY id")?;
        Ok(stmt.query_map([asker], |r| r.get(0))?.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Answers question `id`, by the Assistant for the owner or by the owner himself (`by` an owner). An answer the
    /// owner gives to one already decided overturns it. The question as it now is.
    pub fn decide(&self, id: i64, answer: &str, why: &str, by: &Actor) -> Result<Question> {
        let q = self.question(id)?;
        let answer = answer.trim();
        if answer.is_empty() {
            bail!("say the answer");
        }
        let owner = by.kind == ActorKind::Owner;
        let overturned = match q.status {
            QuestionStatus::Open | QuestionStatus::Left => String::new(),
            QuestionStatus::Decided | QuestionStatus::Answered if owner => q.answer.clone(),
            QuestionStatus::Decided | QuestionStatus::Answered => {
                bail!("question {id} is answered already; only the owner's word changes it")
            }
            QuestionStatus::Withdrawn => bail!("question {id} was withdrawn by {}", q.asker_name),
        };
        let status = if owner { QuestionStatus::Answered } else { QuestionStatus::Decided };
        self.conn.execute(
            "UPDATE questions SET status = ?2, answer = ?3, why = ?4, decided_by = ?5, overturned = ?6, decided = ?7
             WHERE id = ?1",
            params![id, status.as_str(), answer, why.trim(), serde_json::to_string(by)?, overturned, now()],
        )?;
        self.question(id)
    }

    /// Leaves open question `id` to the owner, saying why only he can decide it.
    pub fn leave(&self, id: i64, why: &str, by: &Actor) -> Result<Question> {
        let q = self.question(id)?;
        if q.status != QuestionStatus::Open {
            bail!("question {id} is {}, not open", q.status.as_str());
        }
        self.conn.execute(
            "UPDATE questions SET status = 'left', why = ?2, decided_by = ?3, decided = ?4 WHERE id = ?1",
            params![id, why.trim(), serde_json::to_string(by)?, now()],
        )?;
        self.question(id)
    }

    /// Takes back what agent `asker` still waits on; how many it took back.
    pub fn withdraw(&self, asker: &str, why: &str) -> Result<usize> {
        Ok(self.conn.execute(
            "UPDATE questions SET status = 'withdrawn', why = ?2, decided = ?3 WHERE asker = ?1 AND status IN ('open', 'left')",
            params![asker, why.trim(), now()],
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assistant() -> Actor {
        Actor { kind: ActorKind::Assistant, id: "assistant".into(), name: "Assistant".into() }
    }

    fn owner() -> Actor {
        Actor { kind: ActorKind::Owner, id: String::new(), name: "You".into() }
    }

    #[test]
    fn the_assistant_decides_and_only_the_owner_overturns() {
        let (_dir, life) = crate::life::test_support::life();
        let first = life.ask_owner("fin", "Fin", "Trả trước khoản vay?").unwrap();
        let q = life.ask_owner("fin", "Fin", "Trả trước khoản vay nhà?").unwrap();
        assert_eq!(life.question(first.id).unwrap().status, QuestionStatus::Withdrawn);
        assert_eq!(life.waiting_on("fin").unwrap(), [q.id]);

        let q = life.decide(q.id, "Có, 10 triệu", "lãi cao hơn tiết kiệm", &assistant()).unwrap();
        assert_eq!((q.status, q.decided_by), (QuestionStatus::Decided, Some(assistant())));
        assert!(life.decide(q.id, "Không", "", &assistant()).is_err());
        assert!(life.waiting_on("fin").unwrap().is_empty());

        let q = life.decide(q.id, "Không, giữ tiền mặt", "", &owner()).unwrap();
        assert_eq!((q.status, q.overturned.as_str()), (QuestionStatus::Answered, "Có, 10 triệu"));
    }

    #[test]
    fn a_question_left_to_the_owner_waits_until_he_answers() {
        let (_dir, life) = crate::life::test_support::life();
        let q = life.ask_owner("fin", "Fin", "Mật khẩu ngân hàng?").unwrap();
        let q = life.leave(q.id, "chỉ anh ấy gõ được mật khẩu", &assistant()).unwrap();
        assert_eq!(life.questions(true).unwrap().len(), 1);
        assert!(life.leave(q.id, "", &assistant()).is_err());
        let q = life.decide(q.id, "Đã nhập", "", &owner()).unwrap();
        assert_eq!((q.status, q.overturned.as_str()), (QuestionStatus::Answered, ""));
        assert!(life.questions(true).unwrap().is_empty());
    }
}
