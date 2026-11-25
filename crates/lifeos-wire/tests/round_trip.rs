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
    let agent = || ASSISTANT.to_string();
    round_trip(Call::Read { tool: "dashboard".into(), input: "{}".into() });
    round_trip(Call::Chat { agent: agent(), message: "Hôm nay nên làm gì?".into() });
    round_trip(Call::Brief);
    round_trip(Call::Stop { agent: "huan-luyen-vien".into() });
    round_trip(Call::History { agent: agent() });
    round_trip(Call::ResetChat { agent: agent() });
    round_trip(Call::Watch);
    round_trip(Reply::Json("{\"tasks\":[]}".into()));
    let steps = vec![
        Step::Thinking("Xem lịch…".into()),
        Step::Tool(ToolCall { id: "t1".into(), name: "add_task".into(), input: "{\"title\":\"x\"}".into() }),
        Step::ToolDone(ToolOutcome { id: "t1".into(), output: "ok".into(), failed: false }),
        Step::Text("Xong".into()),
    ];
    for step in &steps {
        round_trip(Reply::Progress(step.clone()));
    }
    let done = ChatDone { steps, ms: 1200, cost_usd: None, error: None };
    round_trip(Reply::ChatDone(done.clone()));
    round_trip(Reply::ChatDone(ChatDone { cost_usd: Some(0.01), error: Some("dừng".into()), ..done.clone() }));
    round_trip(Reply::History(vec![Turn { ask: "Thêm việc".into(), at: "2026-11-25 09:00".into(), reply: done.clone(), source: None }]));
    round_trip(Reply::History(vec![Turn {
        ask: "Viết bản tin".into(),
        at: "2026-11-26 06:30".into(),
        reply: done,
        source: Some("Bản tin buổi sáng · theo lịch".into()),
    }]));
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

fn json_round_trip<T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug>(value: T) {
    let json = serde_json::to_string(&value).unwrap();
    assert_eq!(serde_json::from_str::<T>(&json).unwrap(), value);
}

#[test]
fn routines_and_connections_survive_json() {
    let run = Run {
        id: 1,
        routine: "ban-tin".into(),
        title: "Bản tin".into(),
        agent: ASSISTANT.into(),
        trigger: "schedule".into(),
        detail: None,
        queued: "2026-11-26 06:30:00".into(),
        started: Some("2026-11-26 06:30:02".into()),
        finished: None,
        ms: Some(4000),
        error: None,
        reply: Some("Đã viết".into()),
    };
    json_round_trip(Routine {
        id: "ban-tin".into(),
        title: "Bản tin".into(),
        agent: ASSISTANT.into(),
        instructions: "Viết bản tin.".into(),
        cron: Some("30 6 * * *".into()),
        at: None,
        on: vec!["add_transaction".into()],
        hook: true,
        hook_ready: true,
        status: "active".into(),
        next: Some("2026-11-27 06:30".into()),
        last: Some(run.clone()),
        runs: 3,
    });
    json_round_trip(run);
    json_round_trip(Connections {
        connections: vec![Connection {
            name: "github".into(),
            preset: Some("github".into()),
            transport: "http".into(),
            target: "https://api.githubcopilot.com/mcp/".into(),
            enabled: true,
            missing: vec!["GITHUB_TOKEN".into()],
            members: vec![],
        }],
        presets: vec![Preset {
            id: "github".into(),
            name: "GitHub".into(),
            description: "Issues".into(),
            setup: "1. Tạo token".into(),
            secrets: vec!["GITHUB_TOKEN".into()],
            added: true,
        }],
        secrets: vec!["GITHUB_TOKEN".into()],
    });
}
