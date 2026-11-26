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
    let card = || ASSISTANT.to_string();
    round_trip(Call::Read { tool: "dashboard".into(), input: "{}".into() });
    round_trip(Call::Agent { card: card() });
    round_trip(Call::Events { card: card(), before: None });
    round_trip(Call::Events { card: "huan-luyen-vien".into(), before: Some(42) });
    for delivery in [Delivery::Queue, Delivery::Steer, Delivery::Interrupt] {
        round_trip(Call::Say { card: card(), text: "Hôm nay nên làm gì?".into(), images: vec![], delivery });
    }
    round_trip(Call::Brief);
    round_trip(Call::Interrupt { card: card() });
    round_trip(Call::Restart { card: card() });
    round_trip(Call::Draft { card: card(), text: "đang gõ".into(), images: vec![] });
    round_trip(Call::Watch);
    round_trip(Call::BlockPick { event: 12, part: "when".into(), picked: vec!["am".into(), "pm".into()] });
    round_trip(Call::BlockPick { event: 12, part: "when".into(), picked: vec![] });
    round_trip(Call::Overturn { question: 3, word: "Không, giữ tiền mặt".into() });
    for mark in [TurnMark::Helpful, TurnMark::NotHelpful, TurnMark::Correct] {
        round_trip(Call::Teach { event: 7, mark, words: "Ngắn thôi".into() });
    }
    round_trip(Call::Stop { card: card(), reason: String::new() });
    round_trip(Reply::Json("{\"tasks\":[]}".into()));
    round_trip(Reply::Changed(u64::MAX));
    round_trip(Reply::Done);
    round_trip(Reply::Failed("lỗi".into()));
    round_trip(LinkInfo {
        url: "https://127.0.0.1:7421/link".into(),
        certificate_hashes: vec!["ab".into()],
        certificate_expires: "2026-12-08T00:00:00Z".into(),
    });
}

fn event(id: i64, kind: EventKind, status: &str) -> Event {
    Event {
        id,
        card: ASSISTANT.into(),
        kind,
        status: status.into(),
        tool_call_id: String::new(),
        tool_kind: String::new(),
        tool_name: String::new(),
        title: String::new(),
        text: "Mua sữa".into(),
        by: None,
        data: String::new(),
        created: "2026-11-25T09:00:00.000+07:00".into(),
        updated: "2026-11-25T09:00:00.000+07:00".into(),
        rev: 1,
    }
}

/// A block of each part kind, picked by `by` (none: still the agent's).
fn block(by: Option<Actor>) -> Block {
    let option = |id: &str, detail: Option<&str>| BlockOption { id: id.into(), label: id.to_uppercase(), detail: detail.map(String::from) };
    Block {
        spec: BlockSpec {
            title: "Lịch tập".into(),
            parts: vec![
                BlockPart::Choice {
                    id: "when".into(),
                    label: "Giờ".into(),
                    options: vec![option("am", Some("mát hơn")), option("pm", None)],
                    many: false,
                    picked: vec!["am".into()],
                },
                BlockPart::Choice { id: "days".into(), label: "Ngày".into(), options: vec![option("t2", None), option("t4", None)], many: true, picked: vec![] },
                BlockPart::Chart {
                    id: "kg".into(),
                    label: "Cân nặng".into(),
                    kind: ChartKind::Line,
                    x: vec!["T2".into(), "T3".into()],
                    series: vec![ChartSeries { name: "kg".into(), values: vec![70.5, -1.25] }],
                    unit: Some("kg".into()),
                },
                BlockPart::Ui {
                    id: "panel".into(),
                    document: r#"{"kind":"ui","root":"a","elements":{"a":{"component":"shadcn","name":"Card"}}}"#.into(),
                },
                BlockPart::Chart { id: "h".into(), label: "Giờ ngủ".into(), kind: ChartKind::Bar, x: vec!["T2".into()], series: vec![], unit: None },
            ],
        },
        picked_at: by.as_ref().map(|_| "2026-11-25T09:00:00Z".to_owned()),
        picked_by: by,
    }
}

/// A block is stored as JSON in its event's data, which the page parses again: it survives JSON as it does postcard.
#[test]
fn a_stored_block_of_each_part_kind_reads_across_json() {
    let b = block(None);
    let back: Block = serde_json::from_str(&serde_json::to_string(&b).unwrap()).unwrap();
    assert_eq!(back, b);
    // What an agent writes may leave out `many`, `unit` and `detail`.
    let written = r#"{"title":"T","parts":[{"choice":{"id":"c","label":"L","options":[{"id":"a","label":"A"},{"id":"b","label":"B"}],"picked":["a"]}},{"chart":{"id":"x","label":"X","kind":"bar","x":["1"],"series":[{"name":"s","values":[1]}]}}]}"#;
    let spec: BlockSpec = serde_json::from_str(written).unwrap();
    assert!(matches!(&spec.parts[0], BlockPart::Choice { many: false, .. }));
    assert!(matches!(&spec.parts[1], BlockPart::Chart { unit: None, .. }));
}

#[test]
fn streams_and_agents() {
    let owner = Actor { kind: ActorKind::Owner, id: String::new(), name: "You".into() };
    let kinds = [
        EventKind::Message,
        EventKind::Thought,
        EventKind::Tool,
        EventKind::Prompt,
        EventKind::Stop,
        EventKind::System,
        EventKind::Task,
    ];
    for (i, kind) in kinds.into_iter().enumerate() {
        round_trip(Reply::Event(EventRead { event: event(i as i64, kind, ""), activity: None }));
    }
    let prompt = Event { by: Some(owner.clone()), ..event(9, EventKind::Prompt, "queued") };
    let activities = [
        Activity::Tool { group: ToolGroup::Lifeos, name: "add_task".into(), summary: "Mua sữa".into(), status: ToolState::Running },
        Activity::Sent { to: "coach".into(), kind: "question".into(), label: "Hỏi".into(), text: "Ngủ đủ chưa?".into() },
        Activity::TurnEnd {
            ms: Some(1200),
            turn: Some(TurnSummary {
                touched: vec![Touched { group: ToolGroup::Read, name: "Read".into(), summary: "a.md".into(), count: 2 }],
                failures: vec![TurnFailure { event_id: 3, kind: FailureKind::Refused, name: "Edit".into(), summary: String::new() }],
                partial: true,
            }),
        },
        Activity::TurnEnd { ms: None, turn: None },
        Activity::StartedOver,
        Activity::Block { block: block(None) },
        Activity::Block { block: block(Some(owner.clone())) },
        Activity::Marked {
            marks: vec![ReplyMark { by: owner.clone(), mark: TurnMark::NotHelpful, words: String::new(), at: "2026-11-25T09:00:00Z".into() }],
        },
        Activity::Stopped { reason: "đổi ý".into(), dropped: 2 },
    ];
    for a in activities {
        round_trip(Reply::Event(EventRead { event: prompt.clone(), activity: Some(a) }));
    }
    round_trip(Reply::Delta(EventDelta { id: 4, card: ASSISTANT.into(), kind: EventKind::Message, text: "ữa".into(), offset: 5 }));
    let view = AgentView {
        card: Card {
            id: ASSISTANT.into(),
            name: "Assistant".into(),
            status: CardStatus::Working,
            plan: vec![PlanEntry { content: "Đọc lịch".into(), priority: "high".into(), status: "in_progress".into() }],
            ..Card::default()
        },
        standing: AgentStanding {
            card: ASSISTANT.into(),
            label: "Working".into(),
            tone: AgentTone::Running,
            doing_tone: DoingTone::Busy,
            asks: vec![Ask::Message, Ask::Steer, Ask::Interrupt, Ask::Restart, Ask::Stop],
            stop_says: "Its turn ends.".into(),
            on_message: Some(OnMessage { event_id: 9, answering: true, at: "2026-11-25T09:00:00Z".into() }),
            on_send: Some("Waits for the current turn (+1 queued)".into()),
            since: Some("2026-11-25T09:00:00Z".into()),
            waiting: vec![1],
            ..AgentStanding::default()
        },
        draft: Draft { card: ASSISTANT.into(), text: "đang gõ".into(), images: vec![], rev: 3 },
    };
    round_trip(Reply::Agent(view.clone()));
    for status in [CardStatus::Idle, CardStatus::Starting, CardStatus::Failed, CardStatus::Waiting] {
        round_trip(Reply::Agent(AgentView { card: Card { status, ..view.card.clone() }, ..view.clone() }));
    }
    for tone in [AgentTone::Needs, AgentTone::Failed, AgentTone::Waiting, AgentTone::Done, AgentTone::Idle] {
        round_trip(AgentStanding { tone, ..view.standing.clone() });
    }
    for tone in [DoingTone::Needs, DoingTone::Waiting, DoingTone::Idle, DoingTone::Done, DoingTone::Error] {
        round_trip(AgentStanding { doing_tone: tone, ..view.standing.clone() });
    }
    for kind in [ActorKind::Assistant, ActorKind::Agent, ActorKind::Lifeos, ActorKind::Routine, ActorKind::Workflow] {
        round_trip(Actor { kind, id: "x".into(), name: "X".into() });
    }
    for group in [ToolGroup::Edit, ToolGroup::Execute, ToolGroup::Search, ToolGroup::Fetch, ToolGroup::Other] {
        round_trip(Touched { group, name: String::new(), summary: String::new(), count: 1 });
    }
    round_trip(Reply::Events(vec![EventRead { event: prompt, activity: None }]));
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

    let tasks = TasksView { overdue: vec![], today: vec![], upcoming: vec![], later: vec![doc], anytime: vec![], done_today: vec![] };
    let json = serde_json::to_string(&tasks).unwrap();
    assert_eq!(serde_json::from_str::<TasksView>(&json).unwrap(), tasks);
    assert!(json.contains("\"later\":[{"), "{json}");
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
        workflow: Some("ban-tin".into()),
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
            secrets: vec!["GITHUB_TOKEN".into()],
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
    });
}

#[test]
fn the_vault_survives_both_encodings() {
    let view = VaultView {
        items: vec![
            VaultItem {
                id: 1,
                kind: VaultKind::Key,
                title: "GitHub token".into(),
                description: "Repositories of the owner".into(),
                website: None,
                username: None,
                key_name: Some("GITHUB_TOKEN".into()),
                key_hint: Some("a1b2".into()),
                has_secret: true,
                contact: None,
                all_agents: false,
                agents: vec!["assistant".into(), "coder".into()],
                used_in: vec!["github".into()],
                last_used: Some("2026-11-25T22:00:00+07:00".into()),
                created: "2026-11-25T21:00:00+07:00".into(),
                updated: "2026-11-25T21:00:00+07:00".into(),
            },
            VaultItem {
                id: 2,
                kind: VaultKind::Contact,
                title: "Nhà".into(),
                description: String::new(),
                website: None,
                username: None,
                key_name: None,
                key_hint: None,
                has_secret: false,
                contact: Some(VaultContact { name: "Được".into(), address: "Hà Nội".into(), ..Default::default() }),
                all_agents: true,
                agents: vec![],
                used_in: vec![],
                last_used: None,
                created: "2026-11-25T21:00:00+07:00".into(),
                updated: "2026-11-25T21:00:00+07:00".into(),
            },
        ],
        uses: vec![VaultUse {
            item: 1,
            action: "key".into(),
            agent: Some("coder".into()),
            detail: "github".into(),
            at: "2026-11-25T22:00:00+07:00".into(),
        }],
    };
    round_trip(view.clone());
    json_round_trip(view);
}

#[test]
fn agent_accounts_survive_json() {
    json_round_trip(GoogleView {
        client: Some(GoogleClient { client_id: "id.apps.googleusercontent.com".into(), kind: "web".into(), project: None }),
        account: Some(GoogleAccount {
            email: "owner@example.com".into(),
            services: vec!["gmail".into(), "calendar".into()],
            since: "2026-11-25T10:00:00+07:00".into(),
        }),
        signin: Some(GoogleSignin { url: "https://accounts.google.com/".into(), status: "failed".into(), error: Some("denied".into()) }),
        redirect: "http://127.0.0.1:7420/api/google/callback".into(),
        members: vec!["scout".into()],
    });
    json_round_trip(AgentAccounts {
        accounts: vec![AgentAccount {
            id: "default".into(), label: "Tài khoản của máy".into(), kind: "default".into(), status: "near".into(),
            reason: None, quota: Some("five_hour".into()), utilization: Some(0.8), resets_at: Some(1770000000),
            retry_at: None,
        }],
        signin: Some(AccountSignin { id: "a-test".into(), url: None, status: "waiting".into(), error: None }),
    });
}

#[test]
fn questions() {
    let assistant = Actor { kind: ActorKind::Assistant, id: "assistant".into(), name: "Assistant".into() };
    for (status, by) in [
        (QuestionStatus::Open, None),
        (QuestionStatus::Decided, Some(assistant.clone())),
        (QuestionStatus::Left, Some(assistant)),
        (QuestionStatus::Answered, None),
        (QuestionStatus::Withdrawn, None),
    ] {
        assert_eq!(QuestionStatus::parse(status.as_str()), Some(status));
        let q = Question {
            id: 3,
            asker: "fin".into(),
            asker_name: "Fin".into(),
            text: "Có trả trước khoản vay không?".into(),
            status,
            answer: String::new(),
            why: String::new(),
            decided_by: by,
            overturned: String::new(),
            created: "2026-11-25T10:00:00.000+07:00".into(),
            decided: String::new(),
        };
        round_trip(q.clone());
        json_round_trip(q);
    }
}

#[test]
fn handoffs() {
    for kind in [HandoffKind::Ask, HandoffKind::Task, HandoffKind::News] {
        assert_eq!(HandoffKind::parse(kind.as_str()), Some(kind));
    }
    for status in [
        HandoffStatus::Queued,
        HandoffStatus::Working,
        HandoffStatus::Waiting,
        HandoffStatus::Done,
        HandoffStatus::Failed,
        HandoffStatus::Cancelled,
    ] {
        assert_eq!(HandoffStatus::parse(status.as_str()), Some(status));
        let h = Handoff {
            id: 4,
            kind: HandoffKind::Task,
            sender: "assistant".into(),
            sender_name: "Assistant".into(),
            target: "fin".into(),
            target_name: "Fin".into(),
            text: "Kiểm tra ngân sách tháng này".into(),
            status,
            result: String::new(),
            parent: Some(2),
            chain: vec!["assistant".into()],
            attempts: 1,
            note: String::new(),
            created: "2026-11-25T10:00:00.000+07:00".into(),
            updated: "2026-11-25T10:01:00.000+07:00".into(),
            closed: String::new(),
            reported: false,
        };
        round_trip(h.clone());
        json_round_trip(h);
    }
}

#[test]
fn workflows() {
    let step = WorkflowStep {
        seq: 0,
        verb: "agent".into(),
        label: "Fin".into(),
        phase: "Hỏi".into(),
        input: "{\"prompt\":\"Số dư?\"}".into(),
        status: "done".into(),
        result: "\"42\"".into(),
        error: String::new(),
        created: "2026-11-25T18:00:00.000+07:00".into(),
        finished: String::new(),
    };
    let run = WorkflowRun {
        id: 1,
        name: "thu".into(),
        description: String::new(),
        status: "done".into(),
        result: "{}".into(),
        error: String::new(),
        started_by: "You".into(),
        created: String::new(),
        finished: String::new(),
        steps: vec![step],
    };
    round_trip(run.clone());
    json_round_trip(Workflows { scripts: vec!["thu".into()], runs: vec![run] });
    let t = Teaching {
        id: 1,
        holder: ASSISTANT.into(),
        card: ASSISTANT.into(),
        role: "teaching".into(),
        source: "a conversation".into(),
        teacher: "owner".into(),
        from_event: 3,
        to_event: 5,
        text: String::new(),
        status: "open".into(),
        note: String::new(),
        written: vec![7],
        created: String::new(),
        settled: String::new(),
    };
    round_trip(t.clone());
    json_round_trip(t);
}

#[test]
fn context_and_telemetry() {
    let card = || ASSISTANT.to_string();
    round_trip(Call::Context { card: card() });
    round_trip(Call::Telemetry { card: card() });
    round_trip(Reply::Context(CardContext { card: card(), ..CardContext::default() }));
    let item = |name: &str, deferred| ContextItem { name: name.into(), source: "project".into(), tokens: 812, deferred };
    let context = CardContext {
        card: card(),
        given: Some(ContextGiven {
            skills: vec![],
            claude_tools: Some(vec!["Read".into(), "TodoWrite".into()]),
            setting_sources: vec!["project".into()],
            mcp_servers: vec!["lifeos".into(), "google".into()],
            tools: vec![GivenTool { name: "dashboard".into(), description: "Hôm nay".into() }],
            allowed_tools: vec!["mcp__lifeos".into()],
            deny_rules: vec!["Edit(./crew/**)".into()],
            opening: Some("[2026-11-25 Wednesday 09:00]\n\nChào".into()),
        }),
        loaded: Some(InitReport {
            init: SessionInit {
                session_id: "s1".into(),
                version: "2.1.0".into(),
                model: "claude-fable".into(),
                permission_mode: "dontAsk".into(),
                output_style: "default".into(),
                tools: vec!["Read".into()],
                mcp_servers: vec![McpServerState { name: "lifeos".into(), status: "connected".into(), source: "sdk".into() }],
                skills: vec![],
                plugins: vec![PluginInfo { name: "p".into(), path: "/p".into(), version: "1".into() }],
                plugin_errors: vec![PluginError { plugin: "q".into(), kind: "load".into(), message: "no".into() }],
                agents: vec![],
                slash_commands: vec!["/context".into()],
            },
            at: "2026-11-25T09:00:00+07:00".into(),
        }),
        context: Some(ContextCount {
            context: SessionContext {
                session_id: "s1".into(),
                model: "claude-fable".into(),
                total: 21_000,
                size: 200_000,
                compact_at: 167_000,
                parts: vec![ContextPart { id: "messages".into(), name: "Messages".into(), tokens: 9000, kind: "used".into() }],
                memory_files: vec![item("CLAUDE.md", false)],
                skills: vec![],
                agents: vec![],
                mcp_tools: vec![item("mcp__lifeos__dashboard", true)],
                tool_calls: vec![item("Read", false)],
            },
            at: "2026-11-25T09:00:01+07:00".into(),
        }),
        context_error: "the agent's process did not answer".into(),
    };
    round_trip(Reply::Context(context.clone()));
    json_round_trip(context);

    round_trip(Reply::Telemetry(CardTelemetry::default()));
    let t = CardTelemetry {
        requests: 3,
        errors: 1,
        input_tokens: 10,
        output_tokens: 20,
        cache_read_tokens: 30,
        cache_creation_tokens: 40,
        cost_usd: 0.0125,
        latency_p50_ms: 900,
        latency_p95_ms: 2100,
        ttft_p50_ms: 400,
        models: vec![ModelUse { model: "claude-fable".into(), effort: "high".into(), requests: 3, cost_usd: 0.0125 }],
        tools: vec![ToolUse { name: "Read".into(), runs: 2, failed: 1, total_ms: 30, p95_ms: 20 }],
        recent_errors: vec![ApiError { at_ms: 1_764_000_000_000, model: "claude-fable".into(), status: "529".into(), error: "Overloaded".into() }],
        first_at_ms: 1_764_000_000_000,
        last_at_ms: 1_764_000_060_000,
    };
    round_trip(Reply::Telemetry(t.clone()));
    json_round_trip(t);
}

#[test]
fn images() {
    let card = || ASSISTANT.to_string();
    let shot = |id: &str, media_type| ImageRef { id: id.into(), media_type };
    let images = vec![
        shot("im_1", ImageType::Png),
        shot("im_2", ImageType::Jpeg),
        shot("im_3", ImageType::Gif),
        shot("im_4", ImageType::Webp),
    ];
    round_trip(Call::Say { card: card(), text: String::new(), images: images.clone(), delivery: Delivery::Steer });
    round_trip(Call::Draft { card: card(), text: "xem ảnh".into(), images: images.clone() });
    round_trip(Draft { card: card(), text: String::new(), images: images.clone(), rev: 2 });
    round_trip(Call::ImagePut { id: "im_1".into(), image: Image { media_type: ImageType::Webp, data: "UklGRg==".into() } });
    round_trip(Call::ImageGet { id: "im_1".into() });
    round_trip(Reply::Image(Image { media_type: ImageType::Gif, data: "R0lGOA==".into() }));
    for t in [ImageType::Png, ImageType::Jpeg, ImageType::Gif, ImageType::Webp] {
        assert_eq!(ImageType::parse(t.as_str()), Some(t));
        assert_eq!(serde_json::to_string(&t).unwrap(), format!("\"{}\"", t.as_str()));
    }
    json_round_trip(images);
}

/// The largest image the server keeps, as base64 in its call or reply, is one frame.
#[test]
fn the_largest_image_fits_a_frame() {
    let data = "A".repeat(IMAGE_MAX_BYTES.div_ceil(3) * 4);
    let image = Image { media_type: ImageType::Png, data };
    let put = Call::ImagePut { id: format!("im_{}", "f".repeat(32)), image: image.clone() };
    let frame = encode(&put).unwrap();
    assert!(frame.len() <= frame::MAX_FRAME + 4, "{}", frame.len());
    let mut reader = Reader::default();
    reader.push(&frame);
    assert_eq!(reader.next_frame::<Call>().unwrap(), Some(put));
    let got = Reply::Image(image);
    let mut reader = Reader::default();
    reader.push(&encode(&got).unwrap());
    assert_eq!(reader.next_frame::<Reply>().unwrap(), Some(got));
}

#[test]
fn a_computer_stream_survives_postcard() {
    let hello = ComputerHello {
        id: "c_1".into(),
        name: "Studio".into(),
        version: "0.1.0".into(),
        accessibility: true,
        screen_recording: false,
        width: 1512,
        height: 982,
        paused: false,
    };
    round_trip(Call::Computer(hello.clone()));
    round_trip(Reply::ComputerAsk(ComputerAsk { id: 7, op: "desktop_look".into(), input: "{}".into() }));
    round_trip(ComputerSays::Status(hello));
    for jpeg in [None, Some("/9j/".to_owned())] {
        round_trip(ComputerSays::Answer(ComputerAnswer { id: 7, ok: true, text: "window".into(), jpeg }));
    }
    round_trip(ComputerView { id: "c_1".into(), name: "Studio".into(), online: true, seen: "2026-11-25T09:00:00Z".into(), ..Default::default() });
}
