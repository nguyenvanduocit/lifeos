//! Google Calendar: the owner's calendars and events.

use anyhow::{Context, Result, bail, ensure};
use chrono::{DateTime, Duration, Local, NaiveDate, NaiveDateTime, TimeZone};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::api::{count, url};
use crate::tools::Nothing;
use crate::{Google, random_token};

const BASE: &str = "https://www.googleapis.com/calendar/v3/";

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CalendarEvents {
    /// "primary" when not given.
    pub calendar_id: Option<String>,
    /// RFC 3339 date-time or YYYY-MM-DD; now when not given.
    pub from: Option<String>,
    /// RFC 3339 date-time or YYYY-MM-DD (exclusive); 7 days after from when not given.
    pub to: Option<String>,
    /// Free text to match in titles, descriptions, places and attendees.
    pub query: Option<String>,
    /// 1–250; 50 when not given.
    pub max_results: Option<u32>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CalendarCreate {
    /// "primary" when not given.
    pub calendar_id: Option<String>,
    pub summary: String,
    /// YYYY-MM-DD for an all-day event, or a date-time (RFC 3339, or local without an offset).
    pub start: String,
    /// Same form as start; one hour (or one day) after it when not given. All-day ends are exclusive.
    pub end: Option<String>,
    /// IANA time zone such as "Asia/Ho_Chi_Minh", for date-times and recurrence.
    pub time_zone: Option<String>,
    pub description: Option<String>,
    pub location: Option<String>,
    /// Guest email addresses.
    #[serde(default)]
    pub attendees: Vec<String>,
    /// RRULE lines, e.g. "RRULE:FREQ=WEEKLY;BYDAY=MO".
    #[serde(default)]
    pub recurrence: Vec<String>,
    /// Add a Google Meet link.
    #[serde(default)]
    pub google_meet: bool,
    /// Email the guests about it.
    #[serde(default)]
    pub notify: bool,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CalendarUpdate {
    /// "primary" when not given.
    pub calendar_id: Option<String>,
    pub event_id: String,
    pub summary: Option<String>,
    pub start: Option<String>,
    pub end: Option<String>,
    pub time_zone: Option<String>,
    pub description: Option<String>,
    pub location: Option<String>,
    /// Replaces the guest list.
    pub attendees: Option<Vec<String>>,
    /// Email the guests about the change.
    #[serde(default)]
    pub notify: bool,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CalendarDelete {
    /// "primary" when not given.
    pub calendar_id: Option<String>,
    pub event_id: String,
    /// Email the guests that it is cancelled.
    #[serde(default)]
    pub notify: bool,
}

fn calendar(id: &Option<String>) -> &str {
    id.as_deref().filter(|c| !c.trim().is_empty()).unwrap_or("primary")
}

fn send_updates(notify: bool) -> &'static str {
    if notify { "all" } else { "none" }
}

/// A moment the owner gave: a date (local midnight) or a date-time.
fn moment(text: &str) -> Result<DateTime<Local>> {
    let text = text.trim();
    if let Ok(d) = NaiveDate::parse_from_str(text, "%Y-%m-%d") {
        return local(d.and_hms_opt(0, 0, 0).expect("midnight"));
    }
    if let Ok(t) = DateTime::parse_from_rfc3339(text) {
        return Ok(t.with_timezone(&Local));
    }
    for form in ["%Y-%m-%dT%H:%M:%S", "%Y-%m-%dT%H:%M", "%Y-%m-%d %H:%M:%S", "%Y-%m-%d %H:%M"] {
        if let Ok(t) = NaiveDateTime::parse_from_str(text, form) {
            return local(t);
        }
    }
    bail!("{text:?} is not a date (YYYY-MM-DD) or a date-time")
}

fn local(t: NaiveDateTime) -> Result<DateTime<Local>> {
    Local.from_local_datetime(&t).earliest().with_context(|| format!("{t} does not exist in the local time zone"))
}

/// An event's start or end as Calendar takes it.
fn when(text: &str, time_zone: Option<&str>) -> Result<Value> {
    let text = text.trim();
    if let Ok(d) = NaiveDate::parse_from_str(text, "%Y-%m-%d") {
        return Ok(json!({ "date": d.to_string() }));
    }
    let mut out = json!({ "dateTime": moment(text)?.to_rfc3339() });
    if let Some(tz) = time_zone {
        out["timeZone"] = json!(tz);
    }
    Ok(out)
}

/// One hour after a date-time, or the day after a date.
fn default_end(start: &str, time_zone: Option<&str>) -> Result<Value> {
    let start = start.trim();
    if let Ok(d) = NaiveDate::parse_from_str(start, "%Y-%m-%d") {
        return Ok(json!({ "date": (d + Duration::days(1)).to_string() }));
    }
    when(&(moment(start)? + Duration::hours(1)).to_rfc3339(), time_zone)
}

fn guests(emails: &[String]) -> Value {
    json!(emails.iter().map(|e| json!({ "email": e.trim() })).collect::<Vec<_>>())
}

fn event(e: &Value) -> Value {
    let time = |key: &str| e.get(key).and_then(|t| t.get("dateTime").or_else(|| t.get("date"))).cloned();
    json!({
        "id": e.get("id"),
        "summary": e.get("summary"),
        "start": time("start"),
        "end": time("end"),
        "all_day": e.pointer("/start/date").is_some(),
        "location": e.get("location"),
        "description": e.get("description").and_then(Value::as_str).map(|d| crate::text::clip(d, 2000)),
        "status": e.get("status"),
        "organizer": e.pointer("/organizer/email"),
        "attendees": e.get("attendees").and_then(Value::as_array).map(|a| a.iter().map(|p| json!({
            "email": p.get("email"),
            "response": p.get("responseStatus"),
        })).collect::<Vec<_>>()),
        "meet": e.get("hangoutLink"),
        "recurring_event_id": e.get("recurringEventId"),
        "link": e.get("htmlLink"),
    })
}

pub(crate) async fn calendar_list(g: &Google, _: Nothing) -> Result<Value> {
    let list = g.json(|http| http.get(format!("{BASE}users/me/calendarList"))).await?;
    Ok(json!(
        list.get("items")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|c| json!({
                "id": c.get("id"),
                "name": c.get("summaryOverride").or(c.get("summary")),
                "primary": c.get("primary").and_then(Value::as_bool).unwrap_or(false),
                "time_zone": c.get("timeZone"),
                "access": c.get("accessRole"),
            }))
            .collect::<Vec<_>>()
    ))
}

pub(crate) async fn calendar_events(g: &Google, i: CalendarEvents) -> Result<Value> {
    let from = match &i.from {
        Some(f) => moment(f)?,
        None => Local::now(),
    };
    let to = match &i.to {
        Some(t) => moment(t)?,
        None => from + Duration::days(7),
    };
    ensure!(to > from, "to must be after from");
    let (from, to, max) = (from.to_rfc3339(), to.to_rfc3339(), count(i.max_results, 50, 250).to_string());
    let list = g
        .json(|http| {
            let mut q = vec![
                ("timeMin", from.as_str()),
                ("timeMax", to.as_str()),
                ("singleEvents", "true"),
                ("orderBy", "startTime"),
                ("maxResults", max.as_str()),
            ];
            if let Some(query) = i.query.as_deref().filter(|q| !q.trim().is_empty()) {
                q.push(("q", query));
            }
            http.get(url(BASE, &["calendars", calendar(&i.calendar_id), "events"])).query(&q)
        })
        .await?;
    Ok(json!({
        "time_zone": list.get("timeZone"),
        "events": list.get("items").and_then(Value::as_array).into_iter().flatten().map(event).collect::<Vec<_>>(),
    }))
}

pub(crate) async fn calendar_create(g: &Google, i: CalendarCreate) -> Result<Value> {
    ensure!(!i.summary.trim().is_empty(), "give the event a title");
    let tz = i.time_zone.as_deref();
    let start = when(&i.start, tz)?;
    let end = match &i.end {
        Some(e) => when(e, tz)?,
        None => default_end(&i.start, tz)?,
    };
    ensure!(start.get("date").is_some() == end.get("date").is_some(), "start and end must both be dates or both date-times");
    let mut body = Map::new();
    body.insert("summary".into(), json!(i.summary));
    body.insert("start".into(), start);
    body.insert("end".into(), end);
    if let Some(d) = &i.description {
        body.insert("description".into(), json!(d));
    }
    if let Some(l) = &i.location {
        body.insert("location".into(), json!(l));
    }
    if !i.attendees.is_empty() {
        body.insert("attendees".into(), guests(&i.attendees));
    }
    if !i.recurrence.is_empty() {
        body.insert("recurrence".into(), json!(i.recurrence));
    }
    if i.google_meet {
        body.insert(
            "conferenceData".into(),
            json!({ "createRequest": { "requestId": random_token(12)?, "conferenceSolutionKey": { "type": "hangoutsMeet" } } }),
        );
    }
    let body = Value::Object(body);
    let created = g
        .json(|http| {
            http.post(url(BASE, &["calendars", calendar(&i.calendar_id), "events"]))
                .query(&[("sendUpdates", send_updates(i.notify)), ("conferenceDataVersion", "1")])
                .json(&body)
        })
        .await?;
    Ok(event(&created))
}

pub(crate) async fn calendar_update(g: &Google, i: CalendarUpdate) -> Result<Value> {
    let tz = i.time_zone.as_deref();
    let mut body = Map::new();
    if let Some(s) = &i.summary {
        body.insert("summary".into(), json!(s));
    }
    if let Some(s) = &i.start {
        body.insert("start".into(), when(s, tz)?);
        if i.end.is_none() {
            body.insert("end".into(), default_end(s, tz)?);
        }
    }
    if let Some(e) = &i.end {
        body.insert("end".into(), when(e, tz)?);
    }
    if let Some(d) = &i.description {
        body.insert("description".into(), json!(d));
    }
    if let Some(l) = &i.location {
        body.insert("location".into(), json!(l));
    }
    if let Some(a) = &i.attendees {
        body.insert("attendees".into(), guests(a));
    }
    ensure!(!body.is_empty(), "say what to change");
    let body = Value::Object(body);
    let changed = g
        .json(|http| {
            http.patch(url(BASE, &["calendars", calendar(&i.calendar_id), "events", &i.event_id]))
                .query(&[("sendUpdates", send_updates(i.notify))])
                .json(&body)
        })
        .await?;
    Ok(event(&changed))
}

pub(crate) async fn calendar_delete(g: &Google, i: CalendarDelete) -> Result<Value> {
    g.json(|http| {
        http.delete(url(BASE, &["calendars", calendar(&i.calendar_id), "events", &i.event_id]))
            .query(&[("sendUpdates", send_updates(i.notify))])
    })
    .await?;
    Ok(json!({ "deleted": i.event_id }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_are_all_day_and_times_carry_an_offset() {
        assert_eq!(when("2026-12-01", None).unwrap(), json!({ "date": "2026-12-01" }));
        assert_eq!(default_end("2026-12-31", None).unwrap(), json!({ "date": "2027-01-01" }));
        let t = when("2026-12-01T09:30:00+07:00", Some("Asia/Ho_Chi_Minh")).unwrap();
        assert_eq!(t["timeZone"], "Asia/Ho_Chi_Minh");
        let at = DateTime::parse_from_rfc3339(t["dateTime"].as_str().unwrap()).unwrap();
        assert_eq!(at, DateTime::parse_from_rfc3339("2026-12-01T09:30:00+07:00").unwrap());
        let end = default_end("2026-12-01T09:30:00+07:00", None).unwrap();
        let end = DateTime::parse_from_rfc3339(end["dateTime"].as_str().unwrap()).unwrap();
        assert_eq!(end - at, Duration::hours(1));
        assert!(when("next tuesday", None).is_err());
        assert!(moment("2026-12-01 14:00").is_ok());
    }
}
