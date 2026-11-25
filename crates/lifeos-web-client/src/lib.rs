//! The page's side of the link, compiled to wasm: it dials the server's
//! WebTransport, writes `Call` frames and reads `Reply` frames (postcard, from
//! `lifeos_wire`), and hands Vue plain objects — the generated TypeScript in
//! `web/src/core/generated/` describes them. Vue never sees a byte.
//!
//! Built by `crates/lifeos-web-client/bind.sh` into `web/src/core/pkg/`.

#![cfg(target_arch = "wasm32")]

use js_sys::{Function, Promise};
use lifeos_wire::{Call, LinkInfo, Reader, Reply, encode};
use serde::Serialize;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;
use web_transport::{ClientBuilder, Session};

/// Large enough for a frame's head and a good part of its body in one read.
const CHUNK: usize = 64 * 1024;

/// One open link to the server.
#[wasm_bindgen]
pub struct Link {
    session: Session,
}

fn to_js<T: Serialize>(value: &T) -> Result<JsValue, JsError> {
    value
        .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .map_err(|e| JsError::new(&format!("lifeos: {e}")))
}

fn unhex(text: &str) -> Result<Vec<u8>, JsError> {
    let bad = || JsError::new(&format!("lifeos: {text:?} is not a hex hash"));
    if text.len() % 2 != 0 {
        return Err(bad());
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).map_err(|_| bad()))
        .collect()
}

#[wasm_bindgen]
impl Link {
    /// Dials the link `/link.json` describes (pass its parsed JSON).
    pub async fn connect(info: JsValue) -> Result<Link, JsError> {
        let info: LinkInfo = serde_wasm_bindgen::from_value(info)?;
        let url = url::Url::parse(&info.url).map_err(|e| JsError::new(&format!("lifeos: the link url: {e}")))?;
        let hashes = info
            .certificate_hashes
            .iter()
            .map(|h| unhex(h))
            .collect::<Result<Vec<_>, _>>()?;
        let client = ClientBuilder::new()
            .with_server_certificate_hashes(hashes)
            .map_err(|e| JsError::new(&format!("lifeos: {e}")))?;
        let session = client
            .connect(url)
            .await
            .map_err(|e| JsError::new(&format!("lifeos: cannot reach the link: {e}")))?;
        Ok(Link { session })
    }

    /// Runs a `read` tool. `input` is a plain object (or undefined); resolves
    /// to the tool's output, typed by `ReadTools` in the generated TypeScript.
    pub fn read(&self, tool: String, input: JsValue) -> Promise {
        let session = self.session.clone();
        future_to_promise(async move {
            let input = if input.is_undefined() || input.is_null() {
                "{}".to_string()
            } else {
                js_sys::JSON::stringify(&input)?.as_string().unwrap_or_default()
            };
            finish(call(&session, Call::Read { tool, input }, None).await?)
        })
    }

    /// Sends one message to an agent (`assistant` or a crew member id).
    /// `onStep` receives each `Step` while it works; resolves to a `ChatDone`.
    pub fn chat(&self, agent: String, message: String, on_step: Function) -> Promise {
        let session = self.session.clone();
        future_to_promise(async move {
            finish(call(&session, Call::Chat { agent, message }, Some(on_step)).await?)
        })
    }

    /// Stops what an agent is doing now.
    pub fn stop(&self, agent: String) -> Promise {
        let session = self.session.clone();
        future_to_promise(async move { finish(call(&session, Call::Stop { agent }, None).await?) })
    }

    /// Asks the agent to write today's morning brief; like `chat`.
    pub fn brief(&self, on_progress: Function) -> Promise {
        let session = self.session.clone();
        future_to_promise(async move { finish(call(&session, Call::Brief, Some(on_progress)).await?) })
    }

    /// Resolves to an agent's conversation so far, a `Turn[]`.
    pub fn history(&self, agent: String) -> Promise {
        let session = self.session.clone();
        future_to_promise(async move { finish(call(&session, Call::History { agent }, None).await?) })
    }

    /// Calls `onChange(revision)` now and whenever the data changes. Resolves
    /// when the server stops sending (the link is gone).
    pub fn watch(&self, on_change: Function) -> Promise {
        let session = self.session.clone();
        future_to_promise(async move {
            let err = |e: web_transport::Error| JsError::new(&format!("lifeos: link: {e}"));
            let (mut send, mut recv) = session.open_bi().await.map_err(err)?;
            send.write(&encode(&Call::Watch).map_err(|e| JsError::new(&e.to_string()))?).await.map_err(err)?;
            send.finish().map_err(err)?;
            let mut reader = Reader::default();
            while let Some(chunk) = recv.read(CHUNK).await.map_err(err)? {
                reader.push(&chunk);
                while let Some(reply) = reader.next_frame::<Reply>().map_err(|e| JsError::new(&e.to_string()))? {
                    match reply {
                        Reply::Changed(revision) => {
                            on_change.call1(&JsValue::NULL, &JsValue::from_f64(revision as f64))?;
                        }
                        Reply::Failed(message) => return Err(JsError::new(&message).into()),
                        _ => return Err(JsError::new("lifeos: unexpected reply on the watch stream").into()),
                    }
                }
            }
            Ok(JsValue::UNDEFINED)
        })
    }

    /// Starts an agent's conversation over.
    #[wasm_bindgen(js_name = resetChat)]
    pub fn reset_chat(&self, agent: String) -> Promise {
        let session = self.session.clone();
        future_to_promise(async move { finish(call(&session, Call::ResetChat { agent }, None).await?) })
    }

    /// Resolves (to an error message) once the link is gone, so the page can redial.
    pub fn closed(&self) -> Promise {
        let session = self.session.clone();
        future_to_promise(async move { Ok(JsValue::from_str(&session.closed().await.to_string())) })
    }

    pub fn close(&self) {
        self.session.close(0, "bye");
    }
}

/// The final reply as the promise's value; `Failed` rejects it.
fn finish(reply: Reply) -> Result<JsValue, JsValue> {
    match reply {
        Reply::Json(json) => js_sys::JSON::parse(&json),
        Reply::ChatDone(done) => Ok(to_js(&done)?),
        Reply::History(turns) => Ok(to_js(&turns)?),
        Reply::Done => Ok(JsValue::UNDEFINED),
        Reply::Failed(message) => Err(JsError::new(&message).into()),
        Reply::Progress(_) | Reply::Changed(_) => Err(JsError::new("lifeos: unexpected final reply").into()),
    }
}

/// One call on its own stream: write the call and finish, then read replies
/// until the final one.
async fn call(session: &Session, call: Call, on_progress: Option<Function>) -> Result<Reply, JsError> {
    let err = |e: web_transport::Error| JsError::new(&format!("lifeos: link: {e}"));
    let (mut send, mut recv) = session.open_bi().await.map_err(err)?;
    let frame = encode(&call).map_err(|e| JsError::new(&e.to_string()))?;
    send.write(&frame).await.map_err(err)?;
    send.finish().map_err(err)?;

    let mut reader = Reader::default();
    while let Some(chunk) = recv.read(CHUNK).await.map_err(err)? {
        reader.push(&chunk);
        while let Some(reply) = reader.next_frame::<Reply>().map_err(|e| JsError::new(&e.to_string()))? {
            match reply {
                Reply::Progress(progress) => {
                    if let Some(f) = &on_progress {
                        f.call1(&JsValue::NULL, &to_js(&progress)?)
                            .map_err(|_| JsError::new("lifeos: onProgress threw"))?;
                    }
                }
                last => return Ok(last),
            }
        }
    }
    Err(JsError::new("lifeos: the server closed the call without replying"))
}
