//! What the agents tell every page as they work (remit's `chains::notify`): an event added or changed, a chunk of a
//! growing message, an agent whose standing changed. A page that falls behind reads its agent again.

use lifeos_wire::{EventDelta, EventRead};
use tokio::sync::broadcast;

#[derive(Debug, Clone, PartialEq)]
#[expect(clippy::large_enum_variant, reason = "a notice is told once and read soon; boxing would only add a step")]
pub enum Notice {
    Event(EventRead),
    Delta(EventDelta),
    /// The card of this agent changed: read its view again.
    Agent(String),
}

/// Notices a page has not read, at most, before it is told it fell behind.
const BACKLOG: usize = 4096;

#[derive(Clone)]
pub struct Notifier(broadcast::Sender<Notice>);

impl Default for Notifier {
    fn default() -> Self {
        Self(broadcast::channel(BACKLOG).0)
    }
}

impl Notifier {
    pub fn tell(&self, n: Notice) {
        // No page listening is not an error.
        let _sent = self.0.send(n);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Notice> {
        self.0.subscribe()
    }
}
