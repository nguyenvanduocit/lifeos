//! The page's side of the link, compiled to wasm: it dials the server's
//! WebTransport, writes `Call` frames and reads `Reply` frames (postcard, from
//! `lifeos_wire`), and hands Vue plain objects — the generated TypeScript in
//! `web/src/core/generated/` describes them. Vue never sees a byte.
//!
//! Built by `crates/lifeos-web-client/bind.sh` into `web/src/core/pkg/`.

#![cfg(target_arch = "wasm32")]

use js_sys::{Function, Promise};
use lifeos_wire::{Call, Delivery, Image, ImageRef, LinkInfo, Reader, Reply, TurnMark, encode};
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
            finish(call(&session, Call::Read { tool, input }).await?)
        })
    }

    /// Resolves to agent `card` as its drawer shows it, an `AgentView`.
    pub fn agent(&self, card: String) -> Promise {
        let session = self.session.clone();
        future_to_promise(async move { finish(call(&session, Call::Agent { card }).await?) })
    }

    /// Resolves to the newest events of agent `card`'s stream, or those before event `before`, an `EventRead[]`
    /// oldest first.
    pub fn events(&self, card: String, before: Option<f64>) -> Promise {
        let session = self.session.clone();
        #[expect(clippy::as_conversions, reason = "an event id is an integer JavaScript carries exactly")]
        let before = before.map(|b| b as i64);
        future_to_promise(async move { finish(call(&session, Call::Events { card, before }).await?) })
    }

    /// Sends the owner's message to agent `card` with the images put for it (an `ImageRef[]`); `delivery` is `queue`,
    /// `steer` or `interrupt`. Resolves once it is queued: what the agent does arrives through `watch`.
    pub fn say(&self, card: String, text: String, images: JsValue, delivery: String) -> Promise {
        let session = self.session.clone();
        future_to_promise(async move {
            let images = image_refs(images)?;
            let delivery = match delivery.as_str() {
                "steer" => Delivery::Steer,
                "interrupt" => Delivery::Interrupt,
                _ => Delivery::Queue,
            };
            finish(call(&session, Call::Say { card, text, images, delivery }).await?)
        })
    }

    /// Asks the Assistant to write today's morning brief.
    pub fn brief(&self) -> Promise {
        let session = self.session.clone();
        future_to_promise(async move { finish(call(&session, Call::Brief).await?) })
    }

    /// Ends the turn agent `card` is in.
    pub fn interrupt(&self, card: String) -> Promise {
        let session = self.session.clone();
        future_to_promise(async move { finish(call(&session, Call::Interrupt { card }).await?) })
    }

    /// Start over: agent `card`'s next message opens a new session.
    pub fn restart(&self, card: String) -> Promise {
        let session = self.session.clone();
        future_to_promise(async move { finish(call(&session, Call::Restart { card }).await?) })
    }

    /// Keeps what the owner is typing to agent `card` on the server, with the images put for it (an `ImageRef[]`).
    pub fn draft(&self, card: String, text: String, images: JsValue) -> Promise {
        let session = self.session.clone();
        future_to_promise(async move {
            let images = image_refs(images)?;
            finish(call(&session, Call::Draft { card, text, images }).await?)
        })
    }

    /// Keeps `image` (an `Image`: its kind and base64 bytes) as `id` on the server, for a draft to name.
    pub fn image_put(&self, id: String, image: JsValue) -> Promise {
        let session = self.session.clone();
        future_to_promise(async move {
            let image: Image = serde_wasm_bindgen::from_value(image)?;
            finish(call(&session, Call::ImagePut { id, image }).await?)
        })
    }

    /// Resolves to the image kept as `id`, an `Image`.
    pub fn image_get(&self, id: String) -> Promise {
        let session = self.session.clone();
        future_to_promise(async move { finish(call(&session, Call::ImageGet { id }).await?) })
    }

    /// Resolves to what fills agent `card`'s context and what it was given, a `CardContext`.
    pub fn context(&self, card: String) -> Promise {
        let session = self.session.clone();
        future_to_promise(async move { finish(call(&session, Call::Context { card }).await?) })
    }

    /// Resolves to what agent `card`'s model requests and tools did, a `CardTelemetry`.
    pub fn telemetry(&self, card: String) -> Promise {
        let session = self.session.clone();
        future_to_promise(async move { finish(call(&session, Call::Telemetry { card }).await?) })
    }

    /// The owner picks `picked` (option ids, a `string[]`) in the choice `part` of block event `event`; resolves once
    /// the agent's message is queued.
    pub fn block_pick(&self, event: f64, part: String, picked: JsValue) -> Promise {
        let session = self.session.clone();
        future_to_promise(async move {
            let picked: Vec<String> = serde_wasm_bindgen::from_value(picked)?;
            finish(call(&session, Call::BlockPick { event: whole(event)?, part, picked }).await?)
        })
    }

    /// The owner overturns the answer given to question `question` with `word`, or answers one left to him.
    pub fn overturn(&self, question: f64, word: String) -> Promise {
        let session = self.session.clone();
        future_to_promise(async move { finish(call(&session, Call::Overturn { question: whole(question)?, word }).await?) })
    }

    /// The owner marks agent reply `event` (`mark` a `TurnMark`: helpful | not_helpful | correct).
    pub fn teach(&self, event: f64, mark: JsValue, words: String) -> Promise {
        let session = self.session.clone();
        future_to_promise(async move {
            let mark: TurnMark = serde_wasm_bindgen::from_value(mark)?;
            finish(call(&session, Call::Teach { event: whole(event)?, mark, words }).await?)
        })
    }

    /// The owner stops agent `card`, saying why (`reason`, may be empty).
    pub fn stop(&self, card: String, reason: String) -> Promise {
        let session = self.session.clone();
        future_to_promise(async move { finish(call(&session, Call::Stop { card, reason }).await?) })
    }

    /// Calls `onReply(reply)` with every reply of the watch: `{Changed: n}` now and whenever a life's data changes,
    /// and `{Event: EventRead}`, `{Delta: EventDelta}`, `{Agent: AgentView}` as the agents work. Resolves when the
    /// server stops sending (the link is gone).
    pub fn watch(&self, on_reply: Function) -> Promise {
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
                    if let Reply::Failed(message) = reply {
                        return Err(JsError::new(&message).into());
                    }
                    on_reply.call1(&JsValue::NULL, &to_js(&reply)?)?;
                }
            }
            Ok(JsValue::UNDEFINED)
        })
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

/// A JS number that is an id, which JavaScript carries exactly.
#[expect(clippy::as_conversions, reason = "an id is an integer JavaScript carries exactly")]
fn whole(n: f64) -> Result<i64, JsError> {
    if n.fract() != 0.0 || !n.is_finite() {
        return Err(JsError::new("lifeos: an id is a whole number"));
    }
    Ok(n as i64)
}

/// The images a page names, an `ImageRef[]`; none for undefined.
fn image_refs(images: JsValue) -> Result<Vec<ImageRef>, JsError> {
    if images.is_undefined() || images.is_null() {
        return Ok(Vec::new());
    }
    Ok(serde_wasm_bindgen::from_value(images)?)
}

/// The final reply as the promise's value; `Failed` rejects it.
fn finish(reply: Reply) -> Result<JsValue, JsValue> {
    match reply {
        Reply::Json(json) => js_sys::JSON::parse(&json),
        Reply::Agent(view) => Ok(to_js(&view)?),
        Reply::Events(events) => Ok(to_js(&events)?),
        Reply::Context(context) => Ok(to_js(&context)?),
        Reply::Telemetry(telemetry) => Ok(to_js(&telemetry)?),
        Reply::Image(image) => Ok(to_js(&image)?),
        Reply::Done => Ok(JsValue::UNDEFINED),
        Reply::Failed(message) => Err(JsError::new(&message).into()),
        Reply::Event(_) | Reply::Delta(_) | Reply::Changed(_) | Reply::ComputerAsk(_) => {
            Err(JsError::new("lifeos: unexpected final reply").into())
        }
    }
}

/// One call on its own stream: write the call and finish, then read its one reply.
async fn call(session: &Session, call: Call) -> Result<Reply, JsError> {
    let err = |e: web_transport::Error| JsError::new(&format!("lifeos: link: {e}"));
    let (mut send, mut recv) = session.open_bi().await.map_err(err)?;
    let frame = encode(&call).map_err(|e| JsError::new(&e.to_string()))?;
    send.write(&frame).await.map_err(err)?;
    send.finish().map_err(err)?;

    let mut reader = Reader::default();
    while let Some(chunk) = recv.read(CHUNK).await.map_err(err)? {
        reader.push(&chunk);
        if let Some(reply) = reader.next_frame::<Reply>().map_err(|e| JsError::new(&e.to_string()))? {
            return Ok(reply);
        }
    }
    Err(JsError::new("lifeos: the server closed the call without replying"))
}
