//! The link end to end over real QUIC: a native WebTransport client pins the
//! certificate hash, as a browser does, and makes calls that need no AI.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use lifeos_core::Life;
use lifeos_core::life::tasks::AddTask;
use lifeos_wire::{Call, Dashboard, Reader, Reply, encode};
use web_transport_quinn::ClientBuilder;

async fn call(session: &web_transport_quinn::Session, call: &Call) -> Reply {
    let (mut send, mut recv) = session.open_bi().await.unwrap();
    send.write_all(&encode(call).unwrap()).await.unwrap();
    send.finish().unwrap();
    let mut reader = Reader::default();
    reader.push(&recv.read_to_end(1 << 20).await.unwrap());
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

    // No conversation yet, and resetting one that never started needs no Claude.
    assert_eq!(call(&session, &Call::History).await, Reply::History(vec![]));
    assert_eq!(call(&session, &Call::ResetChat).await, Reply::Done);
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
