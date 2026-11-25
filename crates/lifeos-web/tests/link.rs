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

#[tokio::test]
async fn today_over_webtransport() {
    let dir = tempfile::tempdir().unwrap();
    let life = Life::open_with_memory_db(dir.path()).unwrap();
    life.add_task("Gọi mẹ", Some(lifeos_core::life::today()), None, vec![], None).unwrap();
    let life = Arc::new(Mutex::new(life));

    let any: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let config = lifeos_web::WebConfig { http: any, link: any };
    let running = lifeos_web::start(life, Default::default(), config).await.unwrap();

    let hash: Vec<u8> = (0..running.certificate_hash.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&running.certificate_hash[i..i + 2], 16).unwrap())
        .collect();
    let client = ClientBuilder::new().with_server_certificate_hashes(vec![hash]).unwrap();
    let url = format!("https://{}{}", running.link, lifeos_wire::PATH);
    let session = client.connect(url.parse::<url::Url>().unwrap()).await.unwrap();

    let Reply::Today(today) = call(&session, &Call::Today).await else { panic!("not Today") };
    assert_eq!(today.due_today.len(), 1);
    assert_eq!(today.due_today[0].title, "Gọi mẹ");

    // Resetting a conversation that never started is fine and needs no Claude.
    assert_eq!(call(&session, &Call::ResetChat).await, Reply::Done);
}
