use std::sync::{Arc, Mutex};

use plugin_sdk::{Definition, Effect, ProcessAnswer, Processor, Reading, Slot, Tool, Ui};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Deserialize, JsonSchema)]
struct Message {
    message: String,
}

#[derive(Deserialize, JsonSchema)]
struct Empty {}

fn main() -> std::process::ExitCode {
    let last_event = Arc::new(Mutex::new(String::from("Waiting for an event")));
    plugin_sdk::serve(Definition {
        label: "Hello from LifeOS".into(),
        capability: "lifeos".into(),
        tools: vec![Tool::new(
            "latest_event",
            "Read the latest event received by this plugin.",
            Effect::Read,
            {
                let last_event = last_event.clone();
                move |_, _: Empty| {
                    let message = last_event.lock().unwrap().clone();
                    async move { Ok(message) }
                }
            },
        )],
        processors: vec![
            Processor::new(
                "echo",
                "Receive a message",
                |_, input: plugin_sdk::ProcessInput<Message>| async move {
                    Ok(ProcessAnswer {
                        text: input.params.message.clone(),
                        emits: vec![Reading {
                            key_suffix: "message".into(),
                            value: serde_json::json!({ "message": input.params.message }).into(),
                            ..Default::default()
                        }],
                    })
                },
            ),
            Processor::new(
                "remember",
                "Remember an event",
                move |_, input: plugin_sdk::ProcessInput<Empty>| {
                    let last_event = last_event.clone();
                    async move {
                        if let Some(reading) = input.reading {
                            let message = reading.value["message"].as_str().unwrap_or(&reading.key);
                            *last_event.lock().unwrap() = message.to_owned();
                        }
                        Ok(ProcessAnswer::default())
                    }
                },
            ),
        ],
        ui: Some(Ui {
            entry: "hello.html".into(),
            slots: ["page", "dashboard"]
                .into_iter()
                .map(|slot| Slot {
                    slot: slot.into(),
                    id: "hello".into(),
                    title: "Plugin event monitor".into(),
                    expose: String::new(),
                })
                .collect(),
        }),
        assets: [("hello.html".into(), include_bytes!("hello.html").to_vec())].into(),
        ..Default::default()
    })
}
