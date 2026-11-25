//! The morning brief, written by the agent once a day at a set time (when
//! today's is not written yet), so the dashboard opens on it.

use std::sync::Arc;

use chrono::{Duration, Local, NaiveTime};

use crate::AppState;

pub async fn daily(state: Arc<AppState>, at: NaiveTime) {
    loop {
        tokio::time::sleep(until(at)).await;
        let today = Local::now().date_naive();
        let written = state.life.lock().expect("life lock").brief(today).ok().flatten();
        if written.is_some_and(|b| b.summary.id.starts_with(&today.to_string())) {
            continue;
        }
        tracing::info!("writing the morning brief");
        let by = lifeos_agent::routine("brief", "Morning brief · scheduled");
        if let Err(err) = state.agents.ask(lifeos_wire::ASSISTANT, lifeos_agent::BRIEF_PROMPT, by).await {
            tracing::warn!("morning brief failed: {err:#}");
        }
        // Do not run twice within the same minute.
        tokio::time::sleep(std::time::Duration::from_secs(61)).await;
    }
}

/// How long until the next `at`, local time.
fn until(at: NaiveTime) -> std::time::Duration {
    let now = Local::now().naive_local();
    let mut next = now.date().and_time(at);
    if next <= now {
        next += Duration::days(1);
    }
    (next - now).to_std().unwrap_or_default()
}
