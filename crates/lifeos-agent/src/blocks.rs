//! A block an agent shows in its stream (remit's ADR-0091): a spec of parts the page draws with its own components (a
//! choice, a chart), checked once as the agent writes it (`check_block`) and stored as an event of its card (a `System`
//! event of status `block`, its data `{block}`). The owner's pick in a choice (`pick_in_block`) changes the block and
//! reaches the agent as one queued message from him (`pick_words`), which replaces one from the same block the agent has
//! not read yet. Nothing waits on the pick: the agent goes on with its own.

use std::collections::HashSet;
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use lifeos_core::life::widgets::WidgetUi;
use lifeos_core::life::cards::{NewEvent, now};
use lifeos_wire::{Block, BlockOption, BlockPart, BlockSpec, Delivery, EventKind};
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::activity::{BLOCK, BlockData, PromptData, block_of};
use crate::toolbox::Toolbox;
use crate::{Agents, Inner, owner};

/// The most one block may weigh as JSON, so a block fits the stream.
pub(crate) const MOST_BYTES: usize = 16 * 1024;

/// show_block's input.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ShowBlockInput {
    spec: BlockSpec,
}

/// show_block's input schema: the wire's `BlockSpec`, its parts externally tagged (`{"choice": {…}}`, `{"chart": {…}}`).
fn schema() -> Map<String, Value> {
    let text = |d: &str| json!({ "type": "string", "description": d });
    let choice = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["id", "label", "options", "picked"],
        "properties": {
            "id": text("Unique within the block."),
            "label": text("What is chosen, in a few words."),
            "options": {
                "type": "array",
                "minItems": 2,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["id", "label"],
                    "properties": {
                        "id": text("Unique within the block."),
                        "label": text("The option, in a few words."),
                        "detail": text("One line that compares it with the others.")
                    }
                }
            },
            "many": { "type": "boolean", "description": "Several may be picked. Default: one." },
            "picked": { "type": "array", "items": { "type": "string" }, "description": "The option ids you go ahead with: one unless `many`." }
        }
    });
    let chart = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["id", "label", "kind", "x", "series"],
        "properties": {
            "id": text("Unique within the block."),
            "label": text("What the chart shows."),
            "kind": { "type": "string", "enum": ["bar", "line"] },
            "x": { "type": "array", "items": { "type": "string" }, "description": "The labels along the bottom." },
            "series": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["name", "values"],
                    "properties": {
                        "name": { "type": "string" },
                        "values": { "type": "array", "items": { "type": "number" }, "description": "One per label of x." }
                    }
                }
            },
            "unit": text("The unit of the values (kg, VND, hours).")
        }
    });
    let part = json!({
        "oneOf": [
            { "type": "object", "additionalProperties": false, "required": ["ui"], "properties": {
                "ui": {"type":"object","required":["id","document"],"additionalProperties":false,"properties":{
                    "id": text("Unique within the block."),
                    "document": lifeos_core::tools::schema_of::<WidgetUi>()
                }}
            }},
            { "type": "object", "additionalProperties": false, "required": ["choice"], "properties": { "choice": choice } },
            { "type": "object", "additionalProperties": false, "required": ["chart"], "properties": { "chart": chart } }
        ]
    });
    let props = json!({
        "spec": {
            "type": "object",
            "additionalProperties": false,
            "required": ["title", "parts"],
            "properties": {
                "title": text("What the block is about, in a few words."),
                "parts": { "type": "array", "minItems": 1, "items": part, "description": "Each part contains a choice, chart, or ui." }
            }
        }
    });
    match json!({ "type": "object", "properties": props, "required": ["spec"], "additionalProperties": false }) {
        Value::Object(m) => m,
        _ => Map::new(),
    }
}

impl Inner {
    /// Gives card show_block: it writes only in the agent's own stream, so no tool access gates it.
    pub(crate) fn add_block_tool(self: &Arc<Self>, toolbox: &mut Toolbox, card: &str) {
        let (weak, me) = (Arc::downgrade(self), card.to_owned());
        toolbox.add(
            "show_block",
            &format!(
                "Show the owner a block in your stream: options laid side by side (a `choice`, each option with a line of \
                 `detail` that compares it) or numbers drawn as a `chart` (bar or line, one value per label of `x` in each \
                 series), or a JSON component tree (ui with id and document; use list_ui_components for names). A block never waits: set `picked` to the option you go ahead with, and go on. When the owner picks \
                 another, you get his pick as a message from him, which you act on in your next turn. What the work cannot \
                 go on without is no block: ask it (ask_owner, or in your answer). A table needs no block: write it in \
                 Markdown. Labels are plain text. At most {MOST_BYTES} bytes as JSON. Answers the block's event id."
            ),
            schema(),
            false,
            move |args| {
                let (weak, me) = (weak.clone(), me.clone());
                Box::pin(async move {
                    let i = weak.upgrade().context("lifeos stopped")?;
                    let mut args = Value::Object(args);
                    if let Some(parts) = args.pointer_mut("/spec/parts").and_then(Value::as_array_mut) {
                        for part in parts {
                            if let Some(doc) = part.pointer_mut("/ui/document") {
                                let ui: WidgetUi = serde_json::from_value(doc.take())?;
                                *doc = Value::String(serde_json::to_string(&ui)?);
                            }
                        }
                    }
                    let input: ShowBlockInput = serde_json::from_value(args)?;
                    let id = i.show_block(&me, input.spec)?;
                    Ok(format!(
                        "Shown as block {id}. Go on with what you picked: a change of the owner's reaches you as his message."
                    ))
                })
            },
        );
    }

    /// Stores spec, once it passes `check_block`, as a block event of card, and pushes it to where the owner watches it.
    /// The block event's id.
    pub(crate) fn show_block(&self, card: &str, spec: BlockSpec) -> Result<i64> {
        check_block(&spec).map_err(anyhow::Error::msg)?;
        let title = spec.title.clone();
        let data = serde_json::to_string(&BlockData { block: Block { spec, picked_by: None, picked_at: None } })?;
        let e = self.add_event(NewEvent {
            card: card.into(),
            kind: Some(EventKind::System),
            status: BLOCK.into(),
            title,
            by: Some(self.actor_of(card)),
            data,
            ..NewEvent::default()
        });
        if e.id == 0 {
            bail!("could not show the block");
        }
        Ok(e.id)
    }

    /// The owner picks `picked` in the choice `part` of block event `event`: the block holds his pick, and its agent is
    /// told in one queued message from him; one from this block it has not read yet is rewritten instead.
    pub(crate) async fn pick_in_block(self: &Arc<Self>, event: i64, part: &str, picked: Vec<String>) -> Result<()> {
        // Held from the read to the store: two picks at once would each find no unread message and queue two.
        let _picking = self.picking.lock().await;
        let e = self.life().event(event)?;
        let block = block_of(&e).with_context(|| format!("event {event} is no block"))?;
        // The agent's own rule for taking a message refuses before anything is stored: the block keeps its pick.
        self.check_agent(&e.card)?;
        let spec = picked_in(&block.spec, part, picked).map_err(anyhow::Error::msg)?;
        let words = pick_words(&spec);
        let mut shown = e.clone();
        shown.data = serde_json::to_string(&BlockData {
            block: Block { spec, picked_by: Some(owner()), picked_at: Some(now()) },
        })?;
        let shown = self.life().update_event(&shown)?;
        self.tell_event(shown);
        let unread = self
            .life()
            .events_after(&e.card, event)?
            .into_iter()
            .find(|q| q.kind == EventKind::Prompt && q.status == "queued" && from_block(q, event));
        if let Some(mut m) = unread {
            m.text = words;
            let m = self.life().update_event(&m)?;
            self.tell_event(m);
            return Ok(());
        }
        let data = serde_json::to_string(&PromptData { block: Some(event), ..PromptData::default() })?;
        let agents = Agents { inner: Arc::clone(self) };
        agents.say_with(&e.card, &words, &[], owner(), Delivery::Queue, data).await?;
        Ok(())
    }
}

/// Whether queued message m is the owner's pick in block event.
fn from_block(m: &lifeos_wire::Event, event: i64) -> bool {
    !m.data.is_empty() && serde_json::from_str::<PromptData>(&m.data).is_ok_and(|d| d.block == Some(event))
}

/// Whether spec may be shown; the refusal names the place that is wrong (`parts[1].series[0].values: 9 values, x has
/// 10`), so the agent fixes it in the same turn. Pure.
pub(crate) fn check_block(spec: &BlockSpec) -> std::result::Result<(), String> {
    let bytes = serde_json::to_vec(spec).map_err(|e| e.to_string())?.len();
    if bytes > MOST_BYTES {
        return Err(format!("the block is {bytes} bytes as JSON: at most {MOST_BYTES}"));
    }
    named("title", &spec.title)?;
    if spec.parts.is_empty() {
        return Err("parts: none: a block holds at least one".to_owned());
    }
    let mut ids = HashSet::new();
    for (n, part) in spec.parts.iter().enumerate() {
        let at = format!("parts[{n}]");
        match part {
            BlockPart::Choice { id, label, options, many, picked } => {
                unique(&mut ids, &format!("{at}.id"), id)?;
                named(&format!("{at}.label"), label)?;
                if options.len() < 2 {
                    return Err(format!("{at}.options: {} options, a choice offers at least 2", options.len()));
                }
                for (k, o) in options.iter().enumerate() {
                    unique(&mut ids, &format!("{at}.options[{k}].id"), &o.id)?;
                    named(&format!("{at}.options[{k}].label"), &o.label)?;
                }
                picks_fit(options, *many, picked).map_err(|why| format!("{at}.picked: {why}"))?;
            }
            BlockPart::Ui { id, document } => {
                unique(&mut ids, &format!("{at}.id"), id)?;
                let ui: WidgetUi = serde_json::from_str(document).map_err(|e| format!("{at}.document: {e}"))?;
                ui.validate().map_err(|e| format!("{at}.document: {e}"))?;
            }
            BlockPart::Chart { id, label, x, series, .. } => {
                unique(&mut ids, &format!("{at}.id"), id)?;
                named(&format!("{at}.label"), label)?;
                if x.is_empty() {
                    return Err(format!("{at}.x: none: a chart has at least one label"));
                }
                for (k, l) in x.iter().enumerate() {
                    named(&format!("{at}.x[{k}]"), l)?;
                }
                if series.is_empty() {
                    return Err(format!("{at}.series: none: a chart has at least one"));
                }
                for (k, s) in series.iter().enumerate() {
                    named(&format!("{at}.series[{k}].name"), &s.name)?;
                    if s.values.len() != x.len() {
                        return Err(format!("{at}.series[{k}].values: {} values, x has {}", s.values.len(), x.len()));
                    }
                    if let Some(v) = s.values.iter().position(|v| !v.is_finite()) {
                        return Err(format!("{at}.series[{k}].values[{v}]: not a finite number"));
                    }
                }
            }
        }
    }
    Ok(())
}

fn named(at: &str, label: &str) -> std::result::Result<(), String> {
    if label.trim().is_empty() {
        return Err(format!("{at}: empty"));
    }
    Ok(())
}

fn unique(ids: &mut HashSet<String>, at: &str, id: &str) -> std::result::Result<(), String> {
    named(at, id)?;
    if !ids.insert(id.to_owned()) {
        return Err(format!("{at}: {id:?} is used twice: ids are unique within a block"));
    }
    Ok(())
}

/// Whether picked names options of a choice, each once, and one of them unless it takes many.
fn picks_fit(options: &[BlockOption], many: bool, picked: &[String]) -> std::result::Result<(), String> {
    if !many && picked.len() != 1 {
        return Err(format!("{} picked: a choice that is not `many` has one", picked.len()));
    }
    let mut seen = HashSet::new();
    for p in picked {
        if !options.iter().any(|o| &o.id == p) {
            return Err(format!("{p:?} is no option of it"));
        }
        if !seen.insert(p) {
            return Err(format!("{p:?} is picked twice"));
        }
    }
    Ok(())
}

/// spec with picked as the picks of its choice part.
fn picked_in(spec: &BlockSpec, part: &str, picked: Vec<String>) -> std::result::Result<BlockSpec, String> {
    let mut spec = spec.clone();
    let Some((options, many, picks)) = spec.parts.iter_mut().find_map(|p| match p {
        BlockPart::Choice { id, options, many, picked: now, .. } if id == part => Some((&*options, *many, now)),
        _ => None,
    }) else {
        return Err(format!("{part:?} is no choice of this block"));
    };
    picks_fit(options, many, &picked)?;
    *picks = picked;
    Ok(spec)
}

/// What the agent reads of the owner's picks in spec: the block's title and, for every choice, its label and the labels
/// of what is picked. Every choice, so a message rewritten by a later pick in another choice still carries the first.
pub(crate) fn pick_words(spec: &BlockSpec) -> String {
    let picks: Vec<String> = spec
        .parts
        .iter()
        .filter_map(|p| match p {
            BlockPart::Choice { label, options, picked, .. } => {
                let labels: Vec<&str> =
                    options.iter().filter(|o| picked.contains(&o.id)).map(|o| o.label.as_str()).collect();
                let pick = if labels.is_empty() { "none of them".to_owned() } else { labels.join(", ") };
                Some(format!("for \"{label}\" I pick: {pick}"))
            }
            BlockPart::Chart { .. } | BlockPart::Ui { .. } => None,
        })
        .collect();
    let go_on = if picks.len() == 1 { "Go on with this pick." } else { "Go on with these picks." };
    format!("In your block \"{}\", {}. {go_on}", spec.title, picks.join("; "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lifeos_wire::{ChartKind, ChartSeries};

    fn option(id: &str) -> BlockOption {
        BlockOption { id: id.into(), label: format!("Option {id}"), detail: Some("cheaper".into()) }
    }

    fn choice(id: &str, many: bool, picked: &[&str]) -> BlockPart {
        BlockPart::Choice {
            id: id.into(),
            label: "Which way".into(),
            options: vec![option("a"), option("b"), option("c")],
            many,
            picked: picked.iter().map(|p| (*p).to_owned()).collect(),
        }
    }

    fn chart(values: Vec<f64>) -> BlockPart {
        BlockPart::Chart {
            id: "weight".into(),
            label: "Weight".into(),
            kind: ChartKind::Line,
            x: vec!["Mon".into(), "Tue".into(), "Wed".into()],
            series: vec![ChartSeries { name: "kg".into(), values }],
            unit: Some("kg".into()),
        }
    }

    fn spec(parts: Vec<BlockPart>) -> BlockSpec {
        BlockSpec { title: "Plan".into(), parts }
    }

    #[test]
    fn a_block_is_checked_and_a_refusal_names_the_place() {
        assert_eq!(check_block(&spec(vec![choice("way", false, &["a"]), chart(vec![1.0, -2.5, 3.0])])), Ok(()));
        assert_eq!(check_block(&spec(vec![choice("way", true, &[])])), Ok(()), "a `many` choice may pick none");
        let refused = |parts, why: &str| {
            let got = check_block(&spec(parts)).unwrap_err();
            assert!(got.starts_with(why), "{got}");
        };
        refused(vec![], "parts: none");
        refused(vec![chart(vec![1.0, 2.0])], "parts[0].series[0].values: 2 values, x has 3");
        refused(vec![choice("way", false, &["a"]), chart(vec![1.0, f64::NAN, 3.0])], "parts[1].series[0].values[1]: not a finite");
        refused(vec![choice("way", false, &["a", "b"])], "parts[0].picked: 2 picked");
        refused(vec![choice("way", false, &["z"])], "parts[0].picked: \"z\" is no option");
        refused(vec![choice("way", true, &["a", "a"])], "parts[0].picked: \"a\" is picked twice");
        refused(vec![choice("a", false, &["a"])], "parts[0].options[0].id: \"a\" is used twice");
        refused(vec![choice("x", false, &["a"]), choice("x", false, &["a"])], "parts[1].id: \"x\" is used twice");
        let mut one = choice("way", false, &["a"]);
        if let BlockPart::Choice { options, .. } = &mut one {
            options.truncate(1);
        }
        refused(vec![one], "parts[0].options: 1 options");
        assert!(check_block(&BlockSpec { title: " ".into(), parts: vec![choice("way", false, &["a"])] }).unwrap_err().starts_with("title: empty"));
        let big = BlockSpec { title: "x".repeat(MOST_BYTES), parts: vec![choice("way", false, &["a"])] };
        assert!(check_block(&big).unwrap_err().contains("at most 16384"));
    }

    #[test]
    fn a_pick_is_told_in_words_naming_every_choice() {
        let s = spec(vec![choice("way", false, &["b"]), chart(vec![1.0, 2.0, 3.0])]);
        assert_eq!(pick_words(&s), "In your block \"Plan\", for \"Which way\" I pick: Option b. Go on with this pick.");
        let s = picked_in(&s, "way", vec!["c".into()]).unwrap();
        assert!(pick_words(&s).contains("Option c"));
        assert!(picked_in(&s, "weight", vec![]).is_err(), "a chart is no choice");
        assert!(picked_in(&s, "way", vec!["a".into(), "b".into()]).is_err());
    }
}
