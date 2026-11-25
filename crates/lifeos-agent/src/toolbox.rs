//! The tools one card's server gives (remit's `Tools`): each once, by name, with what it does to a life declared
//! (`readOnlyHint`, which Claude Code reads to run readers side by side). What an agent may do is decided by which tools
//! its toolbox holds.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use rmcp::ErrorData;
use rmcp::model::{MetaObject, Tool, ToolAnnotations};
use serde_json::{Map, Value};

/// What a tool answers: its text, or why it refused.
pub type Answer = Pin<Box<dyn Future<Output = anyhow::Result<String>> + Send>>;

/// What a tool that may show pictures answers.
pub type Shown = Pin<Box<dyn Future<Output = anyhow::Result<Output>> + Send>>;

/// A tool's answer: its words, and the pictures beside them (a screenshot).
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Output {
    pub text: String,
    /// Each picture's media type and bytes, base64.
    pub images: Vec<(String, String)>,
}

type Run = Box<dyn Fn(Map<String, Value>) -> Shown + Send + Sync>;

/// The tools an agent calls turn after turn, kept loaded when Claude Code defers the rest behind ToolSearch.
const ALWAYS_LOAD: &[&str] = &["dashboard", "ask_agent", "hand_off", "ask_owner", "answer_question"];

#[derive(Default)]
pub struct Toolbox {
    tools: Vec<(Tool, Run)>,
}

impl Toolbox {
    /// Adds the tool `name`; one added again replaces the one before.
    pub fn add(
        &mut self,
        name: &str,
        description: &str,
        schema: Map<String, Value>,
        reads: bool,
        run: impl Fn(Map<String, Value>) -> Answer + Send + Sync + 'static,
    ) {
        self.add_shown(name, description, schema, reads, move |args| {
            let answer = run(args);
            Box::pin(async move { answer.await.map(|text| Output { text, images: Vec::new() }) })
        });
    }

    /// Adds the tool `name`, whose answer may carry pictures.
    pub fn add_shown(
        &mut self,
        name: &str,
        description: &str,
        schema: Map<String, Value>,
        reads: bool,
        run: impl Fn(Map<String, Value>) -> Shown + Send + Sync + 'static,
    ) {
        let mut tool = Tool::new(name.to_owned(), description.to_owned(), Arc::new(schema))
            .with_annotations(ToolAnnotations::new().read_only(reads));
        if ALWAYS_LOAD.contains(&name) {
            let mut meta = MetaObject::new();
            meta.0.insert("anthropic/alwaysLoad".to_owned(), Value::Bool(true));
            tool = tool.with_meta(meta);
        }
        let run: Run = Box::new(run);
        match self.tools.iter_mut().find(|(t, _)| t.name == name) {
            Some(slot) => *slot = (tool, run),
            None => self.tools.push((tool, run)),
        }
    }

    /// The tools by name, as the server lists them.
    pub fn listed(&self) -> Vec<Tool> {
        let mut out: Vec<Tool> = self.tools.iter().map(|(t, _)| t.clone()).collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    pub async fn call(&self, name: &str, args: Option<Map<String, Value>>) -> Result<anyhow::Result<Output>, ErrorData> {
        let Some((_, run)) = self.tools.iter().find(|(t, _)| t.name == name) else {
            return Err(ErrorData::invalid_params(format!("unknown tool {name:?}"), None));
        };
        Ok(run(args.unwrap_or_default()).await)
    }
}
