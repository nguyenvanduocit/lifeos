//! The computers online now (LifeOS Computer on each of the owner's Macs, `lifeos_wire::computer`), and the asks
//! the desktop tools send them. The link holds each computer's stream: it attaches the computer here, writes what
//! comes out of [`Attached::asks`], and hands each answer back to [`Computers::answer`].

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use lifeos_wire::{ComputerAnswer, ComputerAsk, ComputerHello};
use tokio::sync::{mpsc, oneshot};

/// The longest a computer may take over one ask: typing a long text with settling is the slowest.
const ASK_TIMEOUT: Duration = Duration::from_secs(120);

struct Live {
    hello: ComputerHello,
    /// Which attachment this is: a computer that connects again replaces its older stream.
    epoch: u64,
    asks: mpsc::UnboundedSender<ComputerAsk>,
    waiting: HashMap<u64, oneshot::Sender<ComputerAnswer>>,
}

#[derive(Default)]
pub struct Computers {
    live: Mutex<HashMap<String, Live>>,
    next: AtomicU64,
}

/// A computer's stream, attached: what to write to it.
pub struct Attached {
    pub epoch: u64,
    pub asks: mpsc::UnboundedReceiver<ComputerAsk>,
}

impl Computers {
    fn lock(&self) -> MutexGuard<'_, HashMap<String, Live>> {
        self.live.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn next(&self) -> u64 {
        self.next.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// A computer connected; an older stream of the same computer is let go (its asks fail).
    pub fn attach(&self, hello: ComputerHello) -> Attached {
        let (tx, rx) = mpsc::unbounded_channel();
        let epoch = self.next();
        self.lock().insert(hello.id.clone(), Live { hello, epoch, asks: tx, waiting: HashMap::new() });
        Attached { epoch, asks: rx }
    }

    /// The computer told what changed (paused, a permission granted).
    pub fn update(&self, epoch: u64, hello: ComputerHello) {
        if let Some(l) = self.lock().get_mut(&hello.id).filter(|l| l.epoch == epoch) {
            l.hello = hello;
        }
    }

    /// The stream `epoch` of computer `id` ended; whether it was the computer's current one (it is away now).
    pub fn detach(&self, id: &str, epoch: u64) -> bool {
        let mut live = self.lock();
        if live.get(id).is_some_and(|l| l.epoch == epoch) {
            live.remove(id);
            return true;
        }
        false
    }

    /// An answer from computer `id` to its ask.
    pub fn answer(&self, id: &str, answer: ComputerAnswer) {
        if let Some(tx) = self.lock().get_mut(id).and_then(|l| l.waiting.remove(&answer.id)) {
            let _ = tx.send(answer);
        }
    }

    /// The computers online, by name.
    pub fn online(&self) -> Vec<ComputerHello> {
        let mut out: Vec<ComputerHello> = self.lock().values().map(|l| l.hello.clone()).collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    /// The online computer `which` names (its id or name, any case), or the only one online.
    fn pick(&self, which: Option<&str>) -> Result<String> {
        let live = self.lock();
        let which = which.map(str::trim).filter(|w| !w.is_empty());
        let found: Vec<&Live> = match which {
            Some(w) => live.values().filter(|l| l.hello.id == w || l.hello.name.eq_ignore_ascii_case(w)).collect(),
            None => live.values().collect(),
        };
        match (found.as_slice(), which) {
            ([one], _) => Ok(one.hello.id.clone()),
            ([], None) => bail!(
                "no computer is online: the owner runs LifeOS Computer on a Mac to let you use it (Integrations → Desktop)"
            ),
            ([], Some(w)) => bail!("no computer {w:?} is online; desktop_computers lists them"),
            (_, _) => bail!(
                "{} computers are online; say which with computer: {}",
                found.len(),
                found.iter().map(|l| format!("{:?}", l.hello.name)).collect::<Vec<_>>().join(", ")
            ),
        }
    }

    /// Runs desktop tool `op` with `input` (JSON) on the computer `which` names, and waits for its answer.
    pub async fn ask(&self, which: Option<&str>, op: &str, input: String) -> Result<ComputerAnswer> {
        let id = self.pick(which)?;
        let (tx, rx) = oneshot::channel();
        let n = self.next();
        {
            let mut live = self.lock();
            let l = live.get_mut(&id).context("the computer went away")?;
            if l.hello.paused {
                bail!("the owner paused agents on {:?}; ask him before using it", l.hello.name);
            }
            l.waiting.insert(n, tx);
            l.asks.send(ComputerAsk { id: n, op: op.to_owned(), input }).ok().context("the computer went away")?;
        }
        let answer = tokio::time::timeout(ASK_TIMEOUT, rx).await;
        if let Some(l) = self.lock().get_mut(&id) {
            l.waiting.remove(&n);
        }
        match answer {
            Ok(Ok(a)) => Ok(a),
            Ok(Err(_)) => bail!("the computer went away before it answered"),
            Err(_) => bail!("the computer did not answer in {} s", ASK_TIMEOUT.as_secs()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hello(id: &str, name: &str) -> ComputerHello {
        ComputerHello { id: id.into(), name: name.into(), ..Default::default() }
    }

    #[tokio::test]
    async fn an_ask_goes_to_the_computer_named_and_its_answer_comes_back() {
        let c = std::sync::Arc::new(Computers::default());
        let mut studio = c.attach(hello("c1", "Studio"));
        let _air = c.attach(hello("c2", "Air"));
        assert!(c.ask(None, "desktop_look", "{}".into()).await.unwrap_err().to_string().contains("say which"));
        let asking = {
            let c = c.clone();
            tokio::spawn(async move { c.ask(Some("studio"), "desktop_look", "{}".into()).await })
        };
        let ask = studio.asks.recv().await.unwrap();
        assert_eq!(ask.op, "desktop_look");
        c.answer("c1", ComputerAnswer { id: ask.id, ok: true, text: "window".into(), jpeg: None });
        assert_eq!(asking.await.unwrap().unwrap().text, "window");
    }

    #[tokio::test]
    async fn a_computer_that_connects_again_replaces_its_old_stream_and_a_paused_one_refuses() {
        let c = Computers::default();
        let old = c.attach(hello("c1", "Studio"));
        let new = c.attach(hello("c1", "Studio"));
        assert!(!c.detach("c1", old.epoch), "the old stream ending leaves the new one");
        assert_eq!(c.online().len(), 1);
        c.update(new.epoch, ComputerHello { paused: true, ..hello("c1", "Studio") });
        assert!(c.ask(None, "desktop_look", "{}".into()).await.unwrap_err().to_string().contains("paused"));
        assert!(c.detach("c1", new.epoch));
        assert!(c.ask(None, "desktop_look", "{}".into()).await.unwrap_err().to_string().contains("no computer"));
    }
}
