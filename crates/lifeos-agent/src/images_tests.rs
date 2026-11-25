//! The owner's images reach the session with his words (remit's ADR-0111): steered into the turn in progress, or kept
//! named on a queued prompt until its turn reads their bytes.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use agent::{BoxFuture, Handler, Launcher, Session, Spec};
use lifeos_core::Life;
use lifeos_wire::{ASSISTANT, Delivery, EventKind, Image, ImageRef, ImageType};
use model::{Prompt, StopReason};
use tokio::sync::Notify;

use crate::{Agents, AgentsConfig, Launch, owner};

/// What each session was given, prompts and steers, in order; the first prompt holds its turn until `release`.
#[derive(Default, Clone)]
struct Fake {
    given: Arc<Mutex<Vec<(&'static str, Prompt)>>>,
    release: Arc<Notify>,
}

struct FakeSession(Fake);

impl Launcher for Fake {
    fn start(&self, _: Spec, _: Arc<dyn Handler>) -> BoxFuture<'_, Result<Box<dyn Session>, agent::Error>> {
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
                given.push(("prompt", p));
                given.len() == 1
            };
            if first {
                self.0.release.notified().await;
            }
            Ok(StopReason::EndTurn)
        })
    }

    fn steer(&self, p: Prompt) -> BoxFuture<'_, Result<bool, agent::Error>> {
        self.0.given.lock().unwrap().push(("steer", p));
        Box::pin(async { Ok(true) })
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

#[tokio::test]
async fn attachments_reach_the_session_steered_and_after_queueing() {
    let dir = tempfile::tempdir().unwrap();
    let life = Life::open_with_memory_db(dir.path()).unwrap();
    let fake = Fake::default();
    let config = AgentsConfig { launcher: Some(Launch(Arc::new(fake.clone()))), ..Default::default() };
    let agents = Agents::start(Arc::new(Mutex::new(life)), config).await.unwrap();

    let png = Image { media_type: ImageType::Png, data: "iVBORw0KGgo=".into() };
    let webp = Image { media_type: ImageType::Webp, data: "UklGRg==".into() };
    agents.put_image("im_png", &png).unwrap();
    agents.put_image("im_webp", &webp).unwrap();
    let shot = |id: &str, media_type| ImageRef { id: id.into(), media_type };
    let missing = [shot("im_none", ImageType::Png)];
    assert!(agents.say_images(ASSISTANT, "xem", &missing, owner(), Delivery::Queue).await.is_err(), "an image not put");

    let file_id = "00112233445566778899aabbccddeeff/report.txt";
    let file = shot(file_id, ImageType::File);
    assert!(agents.set_draft(ASSISTANT, "", std::slice::from_ref(&file)).is_err());
    let relative = lifeos_core::life::images::attachment_path(file_id).unwrap();
    let path = dir.path().join(&relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, b"example report").unwrap();
    agents.set_draft(ASSISTANT, "", std::slice::from_ref(&file)).unwrap();

    // A turn in progress: a message queued behind it keeps its images named until its own turn.
    agents.say(ASSISTANT, "Chào", owner(), Delivery::Queue).await.unwrap();
    until("the first turn", || fake.given.lock().unwrap().len() == 1).await;
    let queued = agents
        .say_images(ASSISTANT, "", &[shot("im_png", ImageType::Png), file.clone()], owner(), Delivery::Queue)
        .await
        .unwrap();
    assert_eq!(queued.status, "queued");
    assert!(queued.data.contains("im_png"), "{}", queued.data);
    let steered = agents
        .say_images(ASSISTANT, "cái này nữa", &[shot("im_webp", ImageType::Webp), file], owner(), Delivery::Steer)
        .await
        .unwrap();
    assert_eq!((steered.kind, steered.status.as_str()), (EventKind::Prompt, "steered"));
    assert!(steered.data.contains("im_webp"), "{}", steered.data);
    fake.release.notify_one();
    until("the queued message's turn", || fake.given.lock().unwrap().len() == 3).await;

    let given = fake.given.lock().unwrap().clone();
    let model_image = |i: &Image, t| model::Image { media_type: t, data: i.data.clone() };
    assert!(given[0].1.images.is_empty());
    assert_eq!(given[1].0, "steer");
    assert!(given[1].1.text.contains("cái này nữa"), "{}", given[1].1.text);
    let relative = relative.to_str().unwrap();
    assert!(given[1].1.text.contains(relative));
    assert_eq!(given[1].1.images, [model_image(&webp, model::ImageType::Webp)]);
    assert_eq!(given[2].0, "prompt");
    assert!(given[2].1.text.contains(relative));
    assert_eq!(given[2].1.images, [model_image(&png, model::ImageType::Png)]);
    // The stream keeps the names, never the bytes.
    let stream = agents.events(ASSISTANT, None).unwrap();
    let delivered = stream.iter().find(|e| e.event.id == queued.id).unwrap();
    assert_eq!(delivered.event.status, "delivered");
    assert!(!delivered.event.data.contains(&png.data));
}
