//! The owner's own acts on the agents from a page: a pick in a block (remit's ADR-0091), an overturn, a mark on a reply
//! (remit's ADR-0093), and a stop with a reason.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use agent::{BoxFuture, Handler, Launcher, Session, Spec};
use lifeos_core::Life;
use lifeos_core::life::cards::NewEvent;
use lifeos_core::life::crew::AddMember;
use lifeos_wire::{
    ASSISTANT, Activity, ActorKind, Ask, BlockOption, BlockPart, BlockSpec, Delivery, EventKind, QuestionStatus, TurnMark,
};
use model::runtime::{ToolCall, ToolKind};
use model::{Prompt, StopReason, ToolStatus};
use tokio::sync::Notify;

use crate::{Agents, AgentsConfig, Launch, owner};

/// What each session was given, in order; the first prompt holds its turn until `release` (or a cancel).
#[derive(Default, Clone)]
struct Fake {
    given: Arc<Mutex<Vec<String>>>,
    release: Arc<Notify>,
    handler: Arc<Mutex<Option<Arc<dyn Handler>>>>,
}

struct FakeSession(Fake);

impl Launcher for Fake {
    fn start(&self, _: Spec, handler: Arc<dyn Handler>) -> BoxFuture<'_, Result<Box<dyn Session>, agent::Error>> {
        *self.handler.lock().unwrap() = Some(handler);
        let fake = self.clone();
        Box::pin(async move { Ok(Box::new(FakeSession(fake)) as Box<dyn Session>) })
    }
}

impl Session for FakeSession {
    fn id(&self) -> &str {
        "fake"
    }

    fn prompt(&self, p: Prompt) -> BoxFuture<'_, Result<StopReason, agent::Error>> {
        Box::pin(async move {
            let first = {
                let mut given = self.0.given.lock().unwrap();
                given.push(p.text);
                given.len() == 1
            };
            if first {
                self.0.release.notified().await;
            }
            Ok(StopReason::EndTurn)
        })
    }

    fn steer(&self, _: Prompt) -> BoxFuture<'_, Result<bool, agent::Error>> {
        Box::pin(async { Ok(false) })
    }

    fn cancel(&self) -> BoxFuture<'_, Result<(), agent::Error>> {
        Box::pin(async { Ok(()) })
    }

    fn close(&self) {}

    fn context_lost(&self) -> bool {
        false
    }

    fn move_to(&self, _: String) -> BoxFuture<'_, Result<(), agent::Error>> {
        Box::pin(async { Ok(()) })
    }

    fn context(&self) -> BoxFuture<'_, Result<model::SessionContext, agent::Error>> {
        Box::pin(async { Ok(model::SessionContext::default()) })
    }
}

async fn until(what: &str, mut done: impl FnMut() -> bool) {
    for _ in 0..500 {
        if done() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("{what} never happened");
}

async fn start(life: Life) -> (Agents, Fake, Arc<Mutex<Life>>) {
    let fake = Fake::default();
    let life = Arc::new(Mutex::new(life));
    let config = AgentsConfig { launcher: Some(Launch(Arc::new(fake.clone()))), ..Default::default() };
    (Agents::start(Arc::clone(&life), config).await.unwrap(), fake, life)
}

fn queued(agents: &Agents) -> Vec<lifeos_wire::Event> {
    let stream = agents.events(ASSISTANT, None).unwrap();
    stream.into_iter().map(|e| e.event).filter(|e| e.kind == EventKind::Prompt && e.status == "queued").collect()
}

#[tokio::test]
async fn a_run_of_picks_reaches_the_agent_as_one_message_with_the_last() {
    let dir = tempfile::tempdir().unwrap();
    let (agents, fake, _life) = start(Life::open_with_memory_db(dir.path()).unwrap()).await;
    agents.say(ASSISTANT, "Lên kế hoạch tập", owner(), Delivery::Queue).await.unwrap();
    until("the first turn", || fake.given.lock().unwrap().len() == 1).await;

    let option = |id: &str, label: &str| BlockOption { id: id.into(), label: label.into(), detail: None };
    let spec = BlockSpec {
        title: "Lịch tập".into(),
        parts: vec![BlockPart::Choice {
            id: "when".into(),
            label: "Giờ tập".into(),
            options: vec![option("am", "Sáng"), option("pm", "Tối")],
            many: false,
            picked: vec!["am".into()],
        }],
    };
    let bad = BlockSpec { parts: vec![], ..spec.clone() };
    assert!(agents.inner.show_block(ASSISTANT, bad).is_err());
    let block = agents.inner.show_block(ASSISTANT, spec).unwrap();
    let shown = agents.events(ASSISTANT, None).unwrap().into_iter().find(|e| e.event.id == block).unwrap();
    assert!(matches!(shown.activity, Some(Activity::Block { .. })));

    assert!(agents.block_pick(block, "when", vec!["noon".into()]).await.is_err(), "no option of it");
    assert!(agents.block_pick(block, "nope", vec!["pm".into()]).await.is_err(), "no choice of it");
    agents.block_pick(block, "when", vec!["pm".into()]).await.unwrap();
    assert_eq!(queued(&agents).len(), 1);
    agents.block_pick(block, "when", vec!["am".into()]).await.unwrap();
    let waiting = queued(&agents);
    assert_eq!(waiting.len(), 1, "a later pick replaces the unread one");
    assert!(waiting[0].text.contains("I pick: Sáng"), "{}", waiting[0].text);
    assert_eq!(waiting[0].by.as_ref().map(|b| b.kind), Some(ActorKind::Owner));

    let shown = agents.events(ASSISTANT, None).unwrap().into_iter().find(|e| e.event.id == block).unwrap();
    let Some(Activity::Block { block: b }) = shown.activity else { panic!("no block") };
    assert_eq!(b.picked_by.map(|a| a.kind), Some(ActorKind::Owner));
    assert!(matches!(&b.spec.parts[0], BlockPart::Choice { picked, .. } if picked == &["am"]));

    fake.release.notify_one();
    until("the pick's turn", || fake.given.lock().unwrap().len() == 2).await;
    let given = fake.given.lock().unwrap().clone();
    assert!(given[1].ends_with("In your block \"Lịch tập\", for \"Giờ tập\" I pick: Sáng. Go on with this pick."), "{}", given[1]);
    // Read now: the next pick is a message of its own.
    until("the pick read", || queued(&agents).is_empty()).await;
    agents.block_pick(block, "when", vec!["pm".into()]).await.unwrap();
    until("the second pick's turn", || fake.given.lock().unwrap().len() == 3).await;
    agents.close();
}

#[tokio::test]
async fn the_owners_overturn_answers_the_asker_and_teaches_the_assistant() {
    let dir = tempfile::tempdir().unwrap();
    let life = Life::open_with_memory_db(dir.path()).unwrap();
    life.add_member(AddMember { name: "Fin".into(), instructions: "Tiền bạc.".into(), ..Default::default() }).unwrap();
    let (agents, fake, life) = start(life).await;
    let q = {
        let life = life.lock().unwrap();
        let q = life.ask_owner("fin", "Fin", "Trả trước khoản vay?").unwrap();
        let assistant = agents.inner.actor_of(ASSISTANT);
        life.decide(q.id, "Có, 10 triệu", "lãi cao", &assistant).unwrap()
    };
    assert!(agents.overturn(q.id, "  ").await.is_err(), "a word is needed");
    agents.overturn(q.id, "Không, giữ tiền mặt").await.unwrap();

    let q = life.lock().unwrap().question(q.id).unwrap();
    assert_eq!((q.status, q.overturned.as_str()), (QuestionStatus::Answered, "Có, 10 triệu"));
    assert_eq!(q.decided_by.map(|b| b.kind), Some(ActorKind::Owner));
    let teachings = life.lock().unwrap().teachings(Some(ASSISTANT), 10).unwrap();
    assert_eq!(teachings.len(), 1);
    assert!(teachings[0].text.contains("The owner overturned it: Không, giữ tiền mặt"), "{}", teachings[0].text);
    until("Fin's turn on the answer", || !fake.given.lock().unwrap().is_empty()).await;
    assert!(fake.given.lock().unwrap()[0].contains("[the owner overturned the answer to your question"));
    agents.close();
}

#[tokio::test]
async fn a_mark_on_a_reply_stays_on_it_and_teaches_its_agent() {
    let dir = tempfile::tempdir().unwrap();
    let (agents, _fake, life) = start(Life::open_with_memory_db(dir.path()).unwrap()).await;
    let (reply, prompt) = {
        let life = life.lock().unwrap();
        life.ensure_card(ASSISTANT, "Assistant").unwrap();
        let prompt = life
            .add_event(NewEvent { card: ASSISTANT.into(), kind: Some(EventKind::Prompt), status: "delivered".into(), text: "Hỏi".into(), by: Some(owner()), ..NewEvent::default() })
            .unwrap();
        let reply = life
            .add_event(NewEvent { card: ASSISTANT.into(), kind: Some(EventKind::Message), text: "Trả lời dài dòng".into(), ..NewEvent::default() })
            .unwrap();
        (reply.id, prompt.id)
    };
    assert!(agents.teach(prompt, TurnMark::Helpful, "").is_err(), "the owner's own words are no reply");
    assert!(agents.teach(reply, TurnMark::Correct, " ").is_err(), "a correction says the reply");
    let teachings = || life.lock().unwrap().teachings(Some(ASSISTANT), 10).unwrap();

    agents.teach(reply, TurnMark::NotHelpful, "quá dài").unwrap();
    agents.teach(reply, TurnMark::NotHelpful, "quá dài").unwrap();
    assert_eq!(teachings().len(), 1, "the same mark again changes nothing");
    assert!(teachings()[0].text.contains("not helpful. Why:\nquá dài"), "{}", teachings()[0].text);

    agents.teach(reply, TurnMark::Correct, "Ngắn thôi").unwrap();
    assert_eq!(teachings().len(), 2);
    let read = agents.events(ASSISTANT, None).unwrap().into_iter().find(|e| e.event.id == reply).unwrap();
    let Some(Activity::Marked { marks }) = read.activity else { panic!("no mark") };
    assert_eq!(marks.len(), 1, "his newer mark replaces the one before");
    assert_eq!((marks[0].mark, marks[0].words.as_str()), (TurnMark::Correct, "Ngắn thôi"));
    agents.close();
}

#[tokio::test]
async fn a_stop_drops_what_waits_and_the_agent_reads_why_next() {
    let dir = tempfile::tempdir().unwrap();
    let (agents, fake, _life) = start(Life::open_with_memory_db(dir.path()).unwrap()).await;
    agents.say(ASSISTANT, "Dọn hộp thư", owner(), Delivery::Queue).await.unwrap();
    until("the first turn", || fake.given.lock().unwrap().len() == 1).await;
    let handler = fake.handler.lock().unwrap().clone().unwrap();
    let tool = |id: &str| agent::Update::ToolCall(ToolCall {
        id: id.into(),
        name: "mcp__lifeos__add_routine".into(),
        kind: ToolKind::Other,
        title: String::new(),
        status: ToolStatus::InProgress,
        input: Default::default(),
        output: String::new(),
    });
    handler.on_update(tool("before-stop"));
    agents.say(ASSISTANT, "một", owner(), Delivery::Queue).await.unwrap();
    agents.say(ASSISTANT, "hai", owner(), Delivery::Queue).await.unwrap();
    let standing = agents.view(ASSISTANT).unwrap().standing;
    assert!(standing.asks.contains(&Ask::Stop) && !standing.stop_says.is_empty());

    agents.stop(ASSISTANT, "đổi ý").await.unwrap();
    until("the turn ended", || agents.view(ASSISTANT).unwrap().card.turns == 1).await;
    handler.on_working(true);
    handler.on_update(tool("after-stop"));
    let stream = agents.events(ASSISTANT, None).unwrap();
    let calls: Vec<_> = stream.iter().filter(|e| e.event.kind == EventKind::Tool).collect();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].event.status, "failed");
    assert!(agents.view(ASSISTANT).unwrap().standing.doing.is_empty());
    let dropped = stream.iter().filter(|e| e.event.kind == EventKind::Prompt && e.event.status == "dropped").count();
    assert_eq!(dropped, 2);
    let note = stream.iter().find_map(|e| match &e.activity {
        Some(Activity::Stopped { reason, dropped }) => Some((reason.clone(), *dropped)),
        _ => None,
    });
    assert_eq!(note, Some(("đổi ý".into(), 2)));
    assert!(!agents.view(ASSISTANT).unwrap().standing.asks.contains(&Ask::Stop), "nothing left to stop");

    agents.say(ASSISTANT, "tiếp", owner(), Delivery::Queue).await.unwrap();
    until("the next turn", || fake.given.lock().unwrap().len() == 2).await;
    let next = fake.given.lock().unwrap()[1].clone();
    assert!(next.contains("[the owner stopped you, and dropped 2 messages that waited for you] Why: đổi ý"), "{next}");
    until("the turn ended", || agents.view(ASSISTANT).unwrap().card.turns == 2).await;
    agents.say(ASSISTANT, "nữa", owner(), Delivery::Queue).await.unwrap();
    until("a third turn", || fake.given.lock().unwrap().len() == 3).await;
    assert!(!fake.given.lock().unwrap()[2].contains("stopped you"), "read once");
    agents.close();
}

#[tokio::test]
async fn a_card_without_a_live_session_does_not_show_stale_activity() {
    let dir = tempfile::tempdir().unwrap();
    let life = Life::open_with_memory_db(dir.path()).unwrap();
    let mut card = life.ensure_card(ASSISTANT, "Assistant").unwrap();
    card.status = lifeos_wire::CardStatus::Working;
    card.activity = "Using add_routine".into();
    life.save_card(&mut card).unwrap();

    let (agents, _, _) = start(life).await;
    let view = agents.view(ASSISTANT).unwrap();
    assert_eq!(view.card.status, lifeos_wire::CardStatus::Idle);
    assert!(view.standing.doing.is_empty());
    assert!(!view.standing.asks.contains(&Ask::Stop));
    agents.close();
}
