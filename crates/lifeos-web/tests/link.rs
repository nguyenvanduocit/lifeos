//! The link end to end over real QUIC: a native WebTransport client pins the
//! certificate hash, as a browser does, and makes calls that need no AI.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use lifeos_core::Life;
use lifeos_wire::{Call, Reader, Reply, encode};
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

#[tokio::test]
async fn today_over_webtransport() {
    let dir = tempfile::tempdir().unwrap();
    let life = Life::open_with_memory_db(dir.path()).unwrap();
    life.add_task("Gọi mẹ", Some(lifeos_core::life::today()), None, vec![], None).unwrap();
    let life = Arc::new(Mutex::new(life));

    let any: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let config = lifeos_web::WebConfig { http: any, link: any };
    let running = lifeos_web::start(life, Default::default(), config).await.unwrap();

    let session = dial(running.link, &running.certificate_hash).await.unwrap();

    let Reply::Today(today) = call(&session, &Call::Today).await else { panic!("not Today") };
    assert_eq!(today.due_today.len(), 1);
    assert_eq!(today.due_today[0].title, "Gọi mẹ");

    // Resetting a conversation that never started is fine and needs no Claude.
    assert_eq!(call(&session, &Call::ResetChat).await, Reply::Done);
}

#[tokio::test]
async fn rotating_the_certificate_keeps_open_sessions_and_pins_the_new_hash() {
    let dir = tempfile::tempdir().unwrap();
    let life = Arc::new(Mutex::new(Life::open_with_memory_db(dir.path()).unwrap()));
    let any: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let running = lifeos_web::start(life, Default::default(), lifeos_web::WebConfig { http: any, link: any })
        .await
        .unwrap();

    let old = running.certificate_hash.clone();
    let open = dial(running.link, &old).await.unwrap();
    let new = running.rotate_certificate().unwrap();
    assert_ne!(old, new);

    // A session opened before the swap goes on working.
    assert!(matches!(call(&open, &Call::Today).await, Reply::Today(_)));
    // New sessions get the new certificate: the old pin no longer matches.
    assert!(dial(running.link, &old).await.is_err());
    let fresh = dial(running.link, &new).await.unwrap();
    assert!(matches!(call(&fresh, &Call::Today).await, Reply::Today(_)));
}
