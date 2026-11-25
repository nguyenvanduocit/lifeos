//! The link end to end over real QUIC: a native WebTransport client pins the
//! certificate hash, as a browser does, and makes calls that need no AI.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use lifeos_core::Life;
use lifeos_core::life::tasks::AddTask;
use lifeos_wire::{Call, Dashboard, Reader, Reply, VaultView, encode};
use web_transport_quinn::ClientBuilder;

async fn call(session: &web_transport_quinn::Session, call: &Call) -> Reply {
    let (mut send, mut recv) = session.open_bi().await.unwrap();
    send.write_all(&encode(call).unwrap()).await.unwrap();
    send.finish().unwrap();
    let mut reader = Reader::default();
    reader.push(&recv.read_to_end(lifeos_wire::frame::MAX_FRAME + 4).await.unwrap());
    let reply = reader.next_frame::<Reply>().unwrap().expect("a whole reply");
    assert_eq!(reader.pending(), 0, "nothing after the final reply");
    reply
}

fn unhex(hex: &str) -> Vec<u8> {
    (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap()).collect()
}

async fn dial(link: SocketAddr, hash: &str) -> Result<web_transport_quinn::Session, String> {
    let client = ClientBuilder::new().with_server_certificate_hashes(vec![unhex(hash)]).unwrap();
    let url = format!("https://{link}{}", lifeos_wire::PATH);
    let connect = client.connect(url.parse::<url::Url>().unwrap());
    match tokio::time::timeout(std::time::Duration::from_secs(5), connect).await {
        Ok(r) => r.map_err(|e| e.to_string()),
        Err(_) => Err("timed out".into()),
    }
}

fn config() -> lifeos_web::WebConfig {
    let any: SocketAddr = "127.0.0.1:0".parse().unwrap();
    lifeos_web::WebConfig { http: any, link: any, brief_at: None }
}

fn read(tool: &str, input: &str) -> Call {
    Call::Read { tool: tool.into(), input: input.into() }
}

#[tokio::test]
async fn the_page_reads_but_never_writes() {
    let dir = tempfile::tempdir().unwrap();
    let life = Life::open_with_memory_db(dir.path()).unwrap();
    let due = Some(lifeos_core::life::today());
    life.add_task(AddTask { title: "Gọi mẹ".into(), due, ..Default::default() }).unwrap();
    let life = Arc::new(Mutex::new(life));
    let running = lifeos_web::start(life, Default::default(), config()).await.unwrap();
    let session = dial(running.link, &running.certificate_hash).await.unwrap();

    let Reply::Json(json) = call(&session, &read("dashboard", "")).await else { panic!("not Json") };
    let dashboard: Dashboard = serde_json::from_str(&json).unwrap();
    assert_eq!(dashboard.tasks.today.len(), 1);
    assert_eq!(dashboard.tasks.today[0].title, "Gọi mẹ");

    let Reply::Failed(why) = call(&session, &read("add_task", r#"{"title":"x"}"#)).await else {
        panic!("a write went through")
    };
    assert!(why.contains("ask the agent"), "{why}");
    assert!(matches!(call(&session, &read("nope", "{}")).await, Reply::Failed(_)));
    assert!(matches!(call(&session, &read("list_tasks", "{bad")).await, Reply::Failed(_)));

    // No conversation yet: the Assistant's card is made on first read, its stream empty, and needs no Claude.
    let Reply::Agent(view) = call(&session, &Call::Agent { card: "assistant".into() }).await else { panic!("not Agent") };
    assert_eq!((view.card.id.as_str(), view.card.turns), ("assistant", 0));
    assert!(view.standing.asks.contains(&lifeos_wire::Ask::Message));
    assert_eq!(call(&session, &Call::Events { card: "assistant".into(), before: None }).await, Reply::Events(vec![]));
    // An image is put first, by an id the page makes, then named by the draft; the largest one crosses in one call.
    let shot = lifeos_wire::ImageRef { id: "im_1".into(), media_type: lifeos_wire::ImageType::Png };
    let draft = |images: Vec<lifeos_wire::ImageRef>| Call::Draft { card: "assistant".into(), text: "chào".into(), images };
    assert!(matches!(call(&session, &draft(vec![shot.clone()])).await, Reply::Failed(_)), "an image not put");
    let image = lifeos_wire::Image { media_type: lifeos_wire::ImageType::Png, data: "iVBORw0KGgo=".into() };
    assert_eq!(call(&session, &Call::ImagePut { id: "im_1".into(), image: image.clone() }).await, Reply::Done);
    assert_eq!(call(&session, &Call::ImageGet { id: "im_1".into() }).await, Reply::Image(image));
    let largest = lifeos_wire::Image {
        media_type: lifeos_wire::ImageType::Webp,
        data: "A".repeat(lifeos_wire::IMAGE_MAX_BYTES / 3 * 4),
    };
    assert_eq!(call(&session, &Call::ImagePut { id: "im_big".into(), image: largest.clone() }).await, Reply::Done);
    assert_eq!(call(&session, &Call::ImageGet { id: "im_big".into() }).await, Reply::Image(largest));
    let over = lifeos_wire::Image { media_type: lifeos_wire::ImageType::Png, data: "A".repeat(lifeos_wire::IMAGE_MAX_BYTES / 3 * 4 + 4) };
    assert!(matches!(call(&session, &Call::ImagePut { id: "im_over".into(), image: over }).await, Reply::Failed(_)));
    assert_eq!(call(&session, &draft(vec![shot.clone()])).await, Reply::Done);
    let Reply::Agent(view) = call(&session, &Call::Agent { card: "assistant".into() }).await else { panic!("not Agent") };
    assert_eq!((view.draft.text.as_str(), view.draft.images.as_slice()), ("chào", [shot].as_slice()));
    assert_eq!(call(&session, &Call::Interrupt { card: "assistant".into() }).await, Reply::Done);
    // A crew member that does not exist cannot be talked to.
    let say = Call::Say { card: "nobody".into(), text: "chào".into(), images: vec![], delivery: lifeos_wire::Delivery::Queue };
    assert!(matches!(call(&session, &say).await, Reply::Failed(_)));
    // The owner's own acts on the agents go through, and say why when they cannot.
    let stop = Call::Stop { card: "assistant".into(), reason: "đổi ý".into() };
    assert_eq!(call(&session, &stop).await, Reply::Done);
    let teach = Call::Teach { event: 999, mark: lifeos_wire::TurnMark::Helpful, words: String::new() };
    assert!(matches!(call(&session, &teach).await, Reply::Failed(_)));
    let pick = Call::BlockPick { event: 999, part: "when".into(), picked: vec![] };
    assert!(matches!(call(&session, &pick).await, Reply::Failed(_)));
    let overturn = Call::Overturn { question: 999, word: "Không".into() };
    assert!(matches!(call(&session, &overturn).await, Reply::Failed(_)));
}

#[tokio::test]
async fn watch_streams_the_current_revision() {
    let dir = tempfile::tempdir().unwrap();
    let life = Arc::new(Mutex::new(Life::open_with_memory_db(dir.path()).unwrap()));
    let running = lifeos_web::start(life, Default::default(), config()).await.unwrap();
    let session = dial(running.link, &running.certificate_hash).await.unwrap();

    let (mut send, mut recv) = session.open_bi().await.unwrap();
    send.write_all(&encode(&Call::Watch).unwrap()).await.unwrap();
    send.finish().unwrap();
    let mut reader = Reader::default();
    let first = loop {
        if let Some(reply) = reader.next_frame::<Reply>().unwrap() {
            break reply;
        }
        let chunk = recv.read_chunk(1024, true).await.unwrap().expect("stream open");
        reader.push(&chunk.bytes);
    };
    assert_eq!(first, Reply::Changed(0));
}

#[tokio::test]
async fn rotating_the_certificate_keeps_open_sessions_and_pins_the_new_hash() {
    let dir = tempfile::tempdir().unwrap();
    let life = Arc::new(Mutex::new(Life::open_with_memory_db(dir.path()).unwrap()));
    let running = lifeos_web::start(life, Default::default(), config()).await.unwrap();

    let old = running.certificate_hash.clone();
    let open = dial(running.link, &old).await.unwrap();
    let new = running.rotate_certificate().unwrap();
    assert_ne!(old, new);

    // A session opened before the swap goes on working.
    assert!(matches!(call(&open, &read("dashboard", "{}")).await, Reply::Json(_)));
    // New sessions get the new certificate: the old pin no longer matches.
    assert!(dial(running.link, &old).await.is_err());
    let fresh = dial(running.link, &new).await.unwrap();
    assert!(matches!(call(&fresh, &read("dashboard", "{}")).await, Reply::Json(_)));
}

#[tokio::test]
async fn authenticated_hook_queues_and_labels_a_run_and_accounts_are_private() {
    use lifeos_core::life::{crew::AddMember, routines::AddRoutine};
    use lifeos_wire::{AgentAccounts, Run};
    use reqwest::StatusCode;

    let dir = tempfile::tempdir().unwrap();
    let life = Life::open(dir.path()).unwrap();
    let member = life.add_member(AddMember { name: "Test crew".into(), ..Default::default() }).unwrap();
    let routine = life.add_routine(AddRoutine {
        title: "Webhook thử".into(), instructions: "Chạy thử".into(), hook: true,
        agent: Some(member.id.clone()), ..Default::default()
    }).unwrap();
    let key = life.rotate_hook_key(&routine.id).unwrap();
    // This avoids invoking Claude while still verifying worker completion
    // and the failure turn's source through real WebTransport.
    life.retire_member(&member.id).unwrap();
    let running = lifeos_web::start(Arc::new(Mutex::new(life)), Default::default(), config()).await.unwrap();
    let http = reqwest::Client::new();
    let hook = format!("http://{}/hooks/{}", running.http, routine.id);
    assert_eq!(http.post(&hook).send().await.unwrap().status(), StatusCode::UNAUTHORIZED);
    assert_eq!(http.post(&hook).bearer_auth("wrong").send().await.unwrap().status(), StatusCode::UNAUTHORIZED);
    assert_eq!(http.post(&hook).bearer_auth(&key).body("a".repeat(65537)).send().await.unwrap().status(), StatusCode::PAYLOAD_TOO_LARGE);
    let accepted = http.post(&hook).bearer_auth(key).body("fixture").send().await.unwrap();
    assert_eq!(accepted.status(), StatusCode::ACCEPTED);
    let run_id = accepted.json::<serde_json::Value>().await.unwrap()["run_id"].as_i64().unwrap();

    let session = dial(running.link, &running.certificate_hash).await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(8), async {
        loop {
            let Reply::Json(json) = call(&session, &read("list_runs", "{}")).await else { panic!("not Json") };
            let runs: Vec<Run> = serde_json::from_str(&json).unwrap();
            assert_eq!(runs.len(), 1);
            assert_eq!(runs[0].id, run_id);
            if runs[0].finished.is_some() { break; }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }).await.unwrap();
    let Reply::Json(json) = call(&session, &read("list_runs", "{}")).await else { panic!("not Json") };
    let runs: Vec<Run> = serde_json::from_str(&json).unwrap();
    let error = runs[0].error.as_deref().unwrap_or_default();
    assert!(error.contains("retired"), "a retired member's run fails saying so: {error}");
    assert!(matches!(call(&session, &Call::Events { card: member.id, before: None }).await, Reply::Failed(_)));

    let url = format!("http://{}/api/accounts", running.http);
    assert_eq!(http.get(&url).send().await.unwrap().status(), StatusCode::FORBIDDEN);
    assert_eq!(http.post(&url).header("x-lifeos-account", "1").header("sec-fetch-site", "cross-site")
        .json(&serde_json::json!({"action":"login"})).send().await.unwrap().status(), StatusCode::FORBIDDEN);
    let response = http.get(url).header("x-lifeos-account", "1").send().await.unwrap();
    assert_eq!(response.headers()["cache-control"], "no-store");
    let accounts: AgentAccounts = response.json().await.unwrap();
    assert_eq!(accounts.accounts.len(), 1);
    assert_eq!(accounts.accounts[0].kind, "default");
    assert!(accounts.signin.is_none());
}

#[tokio::test]
async fn google_sign_in_starts_from_the_page_and_takes_back_only_its_own_answer() {
    use lifeos_wire::GoogleView;
    use reqwest::StatusCode;

    let dir = tempfile::tempdir().unwrap();
    let life = Arc::new(Mutex::new(Life::open_with_memory_db(dir.path()).unwrap()));
    let google = Arc::new(lifeos_agent::Google::new(Some(dir.path().join("private-google"))));
    let agents = lifeos_agent::AgentsConfig { google: Some(google), ..Default::default() };
    let running = lifeos_web::start(life, agents, config()).await.unwrap();
    let http = reqwest::Client::new();
    let url = format!("http://{}/api/google", running.http);
    let act = |body: serde_json::Value| http.post(&url).header("x-lifeos-account", "1").json(&body).send();

    assert_eq!(http.get(&url).send().await.unwrap().status(), StatusCode::FORBIDDEN);
    let response = act(serde_json::json!({"action": "login"})).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST, "no sign-in before a client is pasted");

    let client = r#"{"installed":{"client_id":"1-a.apps.googleusercontent.com","client_secret":"s"}}"#;
    let view: GoogleView = act(serde_json::json!({"action": "client", "json": client})).await.unwrap().json().await.unwrap();
    assert_eq!(view.client.unwrap().client_id, "1-a.apps.googleusercontent.com");
    assert_eq!(view.redirect, format!("http://127.0.0.1:{}/api/google/callback", running.http.port()));

    let view: GoogleView = act(serde_json::json!({"action": "login"})).await.unwrap().json().await.unwrap();
    let signin = view.signin.unwrap();
    assert_eq!(signin.status, "waiting");
    let consent = url::Url::parse(&signin.url).unwrap();
    let state = consent.query_pairs().find(|(k, _)| k == "state").unwrap().1.into_owned();
    assert!(consent.query_pairs().any(|(k, v)| k == "redirect_uri" && v == view.redirect));

    let callback = format!("http://{}/api/google/callback", running.http);
    let wrong = http.get(&callback).query(&[("code", "c"), ("state", "forged")]).send().await.unwrap();
    assert_eq!(wrong.status(), StatusCode::BAD_REQUEST);
    let denied = http.get(&callback).query(&[("error", "access_denied"), ("state", state.as_str())]).send().await.unwrap();
    assert_eq!(denied.status(), StatusCode::OK);
    assert!(denied.text().await.unwrap().contains("cancelled"));
    let view: GoogleView =
        http.get(&url).header("x-lifeos-account", "1").send().await.unwrap().json().await.unwrap();
    assert!(view.account.is_none());
    assert_eq!(view.signin.unwrap().status, "failed");
}

#[tokio::test]
async fn the_vault_is_the_pages_alone_and_never_echoes_a_value() {
    use reqwest::StatusCode;

    let dir = tempfile::tempdir().unwrap();
    let life = Arc::new(Mutex::new(Life::open_with_memory_db(dir.path()).unwrap()));
    let running = lifeos_web::start(life, Default::default(), config()).await.unwrap();
    let http = reqwest::Client::new();
    let url = format!("http://{}/api/vault", running.http);
    let key = |name: &str, value: &str| {
        serde_json::json!({"action": "save", "item": {"kind": "key", "title": "GitHub", "key_name": name, "secret": value}})
    };

    let foreign = http.post(&url).header("x-lifeos-account", "1").header("sec-fetch-site", "cross-site");
    assert_eq!(foreign.json(&key("GITHUB_TOKEN", "x")).send().await.unwrap().status(), StatusCode::FORBIDDEN);
    assert_eq!(http.post(&url).json(&key("GITHUB_TOKEN", "x")).send().await.unwrap().status(), StatusCode::FORBIDDEN);
    assert_eq!(http.get(&url).send().await.unwrap().status(), StatusCode::FORBIDDEN);

    let refused = http.post(&url).header("x-lifeos-account", "1").json(&key("GITHUB_TOKEN", "fixture\nvalue")).send().await.unwrap();
    assert_eq!(refused.status(), StatusCode::BAD_REQUEST);
    assert!(!refused.text().await.unwrap().contains("fixture"), "the value never comes back");
    let refused = http.post(&url).header("x-lifeos-account", "1").json(&key("not a name", "fixture-v")).send().await.unwrap();
    assert_eq!(refused.status(), StatusCode::BAD_REQUEST);
    assert!(!refused.text().await.unwrap().contains("fixture-v"));

    // A contact keeps no value file: safe to save from a test.
    let contact = serde_json::json!({"action": "save", "item": {"kind": "contact", "title": "Home", "all_agents": true,
        "contact": {"address": "Hà Nội"}}});
    let saved = http.post(&url).header("x-lifeos-account", "1").json(&contact).send().await.unwrap();
    assert_eq!(saved.status(), StatusCode::OK);
    let view: VaultView = http.get(&url).header("x-lifeos-account", "1").send().await.unwrap().json().await.unwrap();
    assert_eq!(view.items.len(), 1);
    assert_eq!(view.items[0].contact.as_ref().unwrap().address, "Hà Nội");
}

#[tokio::test]
async fn the_phone_sends_apple_health_days_and_a_day_sent_again_replaces_itself() {
    use reqwest::StatusCode;

    let dir = tempfile::tempdir().unwrap();
    let life = Arc::new(Mutex::new(Life::open_with_memory_db(dir.path()).unwrap()));
    let running = lifeos_web::start(life.clone(), Default::default(), config()).await.unwrap();
    let http = reqwest::Client::new();
    let url = format!("http://{}/api/health/apple", running.http);
    let upload = |steps: f64| {
        serde_json::json!({"readings": [
            {"day": "2026-11-24", "measure": "sleep_hours", "value": 7.25},
            {"day": "2026-11-25", "measure": "steps", "value": steps},
        ]})
    };

    assert_eq!(http.post(&url).json(&upload(1.0)).send().await.unwrap().status(), StatusCode::FORBIDDEN);
    let foreign = http.post(&url).header("x-lifeos-account", "1").header("sec-fetch-site", "cross-site");
    assert_eq!(foreign.json(&upload(1.0)).send().await.unwrap().status(), StatusCode::FORBIDDEN);

    for steps in [4000.0, 9000.0] {
        let sent = http.post(&url).header("x-lifeos-account", "1").json(&upload(steps)).send().await.unwrap();
        assert_eq!(sent.status(), StatusCode::OK);
    }
    let bad = serde_json::json!({"readings": [{"day": "2026-11-25", "measure": "mood", "value": 1}]});
    let refused = http.post(&url).header("x-lifeos-account", "1").json(&bad).send().await.unwrap();
    assert!(refused.status().is_client_error());

    let kept = life
        .lock()
        .unwrap()
        .metrics(&lifeos_core::life::health::ListMetrics {
            from: Some("2026-11-20".parse().unwrap()),
            to: Some("2026-11-25".parse().unwrap()),
            ..Default::default()
        })
        .unwrap();
    let pairs: Vec<_> = kept.iter().map(|m| (m.name.as_str(), m.value)).collect();
    assert_eq!(pairs, [("giấc ngủ", 7.25), ("bước chân", 9000.0)]);
}

#[tokio::test]
async fn a_computer_connects_answers_the_agents_and_is_marked_away_when_it_goes() {
    use lifeos_wire::{ComputerAnswer, ComputerHello, ComputerSays, ComputerView};
    let dir = tempfile::tempdir().unwrap();
    let life = Arc::new(Mutex::new(Life::open_with_memory_db(dir.path()).unwrap()));
    let running = lifeos_web::start(life, Default::default(), config()).await.unwrap();
    let session = dial(running.link, &running.certificate_hash).await.unwrap();

    // The app: says who it is, then answers what it is asked, and keeps its side open.
    let (mut send, mut recv) = session.open_bi().await.unwrap();
    let hello = ComputerHello { id: "c_test".into(), name: "Studio".into(), accessibility: true, width: 1512, height: 982, ..Default::default() };
    send.write_all(&encode(&Call::Computer(hello)).unwrap()).await.unwrap();
    let app = tokio::spawn(async move {
        let mut reader = Reader::default();
        let mut buf = vec![0_u8; 4096];
        loop {
            while let Some(reply) = reader.next_frame::<Reply>().unwrap() {
                let Reply::ComputerAsk(ask) = reply else { panic!("not an ask: {reply:?}") };
                let text = format!("{} {}", ask.op, ask.input);
                let says = ComputerSays::Answer(ComputerAnswer { id: ask.id, ok: true, text, jpeg: Some("/9j/".into()) });
                send.write_all(&encode(&says).unwrap()).await.unwrap();
                if ask.op == "desktop_look" {
                    // Gone after its answer.
                    send.finish().unwrap();
                    return;
                }
            }
            match recv.read(&mut buf).await.unwrap() {
                Some(n) => reader.push(&buf[..n]),
                None => return,
            }
        }
    });

    let computers = |session| async move {
        let Reply::Json(json) = call(session, &read("list_computers", "")).await else { panic!("not Json") };
        serde_json::from_str::<Vec<ComputerView>>(&json).unwrap()
    };
    for _ in 0..50 {
        if !running.computers.online().is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    let seen = computers(&session).await;
    assert_eq!((seen.len(), seen[0].name.as_str(), seen[0].online), (1, "Studio", true));

    let answer = running.computers.ask(Some("studio"), "desktop_look", "{}".into()).await.unwrap();
    assert_eq!((answer.text.as_str(), answer.jpeg.as_deref()), ("desktop_look {}", Some("/9j/")));
    app.await.unwrap();
    for _ in 0..100 {
        if running.computers.online().is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert!(running.computers.online().is_empty());
    assert!(!computers(&session).await[0].online);
}

#[tokio::test]
async fn signals_show_configured_receivers_even_when_disabled() {
    let dir = tempfile::tempdir().unwrap();
    let life = Life::open_with_memory_db(dir.path()).unwrap();
    let registry = dir.path().join(".lifeos");
    std::fs::create_dir_all(&registry).unwrap();
    let config_json = serde_json::json!({
        "receiver": {
            "binary": "/unused", "disabled": true, "sha256": "0".repeat(64),
            "events": {"plugin.producer.push": "consume"}
        }
    });
    std::fs::write(registry.join("plugins.json"), config_json.to_string()).unwrap();
    let running = lifeos_web::start(Arc::new(Mutex::new(life)), Default::default(), config()).await.unwrap();
    let view: serde_json::Value = reqwest::get(format!("http://{}/api/signals", running.http))
        .await.unwrap().error_for_status().unwrap().json().await.unwrap();
    assert_eq!(view["subscriptions"], serde_json::json!([{
        "event": "plugin.producer.push", "plugin": "receiver", "processor": "consume", "enabled": false
    }]));
}
