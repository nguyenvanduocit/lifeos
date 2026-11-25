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

fn doc(due: Option<&str>) -> DocSummary {
    DocSummary {
        id: "goi-me".into(),
        kind: Kind::Task,
        title: "Gọi mẹ".into(),
        status: Some("open".into()),
        due: due.map(Into::into),
        goal: None,
        horizon: None,
        path: "tasks/goi-me.md".into(),
    }
}

#[test]
fn calls_and_replies() {
    round_trip(Call::Today);
    round_trip(Call::Chat { message: "Hôm nay nên làm gì?".into() });
    round_trip(Call::ResetChat);
    round_trip(Reply::Progress(Progress::Text("Để mình xem…".into())));
    round_trip(Reply::Progress(Progress::Tool("mcp__lifeos__today".into())));
    round_trip(Reply::ChatDone(ChatDone { text: "Xong".into(), tools: vec!["today".into()], cost_usd: None }));
    round_trip(Reply::Done);
    round_trip(Reply::Failed("lỗi".into()));
}

#[test]
fn today_with_every_optional_both_ways() {
    round_trip(Reply::Today(Today {
        date: "2026-11-25".into(),
        weekday: "Wednesday".into(),
        overdue_tasks: vec![doc(Some("2026-11-20"))],
        due_today: vec![doc(None)],
        open_tasks_without_due: 3,
        active_goals: vec![],
        habits: vec![HabitStatus { habit: "thiền".into(), done_today: true, streak: 4, days_done_in_window: 5, window_days: 7 }],
        money_today: vec![CurrencySummary {
            currency: "VND".into(),
            income: 0.0,
            spending: 75_000.0,
            net: -75_000.0,
            spending_by_category: vec![CategoryAmount { category: "ăn uống".into(), amount: 75_000.0 }],
        }],
        journal_written: false,
    }));
}

#[test]
fn oversized_length_is_refused() {
    let mut reader = Reader::default();
    reader.push(&u32::MAX.to_be_bytes());
    assert!(matches!(reader.next_frame::<Call>(), Err(FrameError::TooLarge(_))));
}
