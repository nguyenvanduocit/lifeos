//! Every type that crosses the wire survives postcard, including its `None`s
//! and every enum variant — postcard has no field names to fall back on.

use lifeos_wire::*;

fn round_trip<T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug>(value: T) {
    let bytes = encode(&value).unwrap();
    // Feed it one byte at a time: frames must not depend on how chunks fall.
    let mut reader = Reader::default();
    for (i, b) in bytes.iter().enumerate() {
        reader.push(&[*b]);
        let got: Option<T> = reader.next_frame().unwrap();
        if i + 1 < bytes.len() {
            assert!(got.is_none());
        } else {
            assert_eq!(got.as_ref(), Some(&value));
        }
    }
    assert_eq!(reader.pending(), 0);
}

#[test]
fn calls_and_replies() {
    round_trip(Call::Read { tool: "dashboard".into(), input: "{}".into() });
    round_trip(Call::Chat { message: "Hôm nay nên làm gì?".into() });
    round_trip(Call::Brief);
    round_trip(Call::History);
    round_trip(Call::ResetChat);
    round_trip(Call::Watch);
    round_trip(Reply::Json("{\"tasks\":[]}".into()));
    round_trip(Reply::Progress(Progress::Text("Để mình xem…".into())));
    round_trip(Reply::Progress(Progress::Tool("add_task".into())));
    let done = ChatDone { text: "Xong".into(), tools: vec!["add_task".into()], cost_usd: None };
    round_trip(Reply::ChatDone(done.clone()));
    round_trip(Reply::ChatDone(ChatDone { cost_usd: Some(0.01), ..done.clone() }));
    round_trip(Reply::History(vec![Turn { ask: "Thêm việc".into(), reply: done }]));
    round_trip(Reply::Changed(u64::MAX));
    round_trip(Reply::Done);
    round_trip(Reply::Failed("lỗi".into()));
    round_trip(LinkInfo {
        url: "https://127.0.0.1:7421/link".into(),
        certificate_hashes: vec!["ab".into()],
        certificate_expires: "2026-12-08T00:00:00Z".into(),
    });
}

/// Views cross as JSON inside `Reply::Json`; they must survive it too.
#[test]
fn views_survive_json() {
    let doc = DocSummary {
        id: "goi-me".into(),
        kind: Kind::Task,
        title: "Gọi mẹ".into(),
        status: Some("open".into()),
        due: Some("2026-11-20".into()),
        scheduled: None,
        priority: Some("high".into()),
        goal: None,
        horizon: None,
        progress: None,
        tags: vec!["gia đình".into()],
        path: "tasks/goi-me.md".into(),
    };
    let json = serde_json::to_string(&doc).unwrap();
    assert_eq!(serde_json::from_str::<DocSummary>(&json).unwrap(), doc);
    assert!(json.contains("\"kind\":\"task\""), "{json}");
}

#[test]
fn oversized_length_is_refused() {
    let mut reader = Reader::default();
    reader.push(&u32::MAX.to_be_bytes());
    assert!(matches!(reader.next_frame::<Call>(), Err(FrameError::TooLarge(_))));
}
