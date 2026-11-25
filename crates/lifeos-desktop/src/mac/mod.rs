//! Desktop use on macOS: what [`Desktop`] does with each tool, through the Accessibility API, CGEvent,
//! `screencapture` and Vision.

mod ax;
mod input;
mod ocr;
mod screen;

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread::sleep;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail, ensure};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use core_foundation::base::TCFType;
use core_foundation::string::CFString;

use crate::tree::{Look, Node, Rect, RefBook, Target, quote};
use crate::{Act, Action, Answer, At, Capture, Direction, keys};
use ax::{Ax, Reader};
use input::{Button, Input};

/// What macOS lets this app do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Permissions {
    pub accessibility: bool,
    pub screen_recording: bool,
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> u8;
    fn AXIsProcessTrustedWithOptions(options: *const std::ffi::c_void) -> u8;
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGRequestScreenCaptureAccess() -> bool;
}

impl Permissions {
    /// What macOS grants now; `ask` shows its prompts for what is missing (macOS shows each once per app).
    pub fn check(ask: bool) -> Permissions {
        use core_foundation::boolean::CFBoolean;
        use core_foundation::dictionary::CFDictionary;
        // SAFETY: plain calls; the options dictionary outlives the call.
        unsafe {
            let accessibility = if ask {
                let options = CFDictionary::from_CFType_pairs(&[(
                    CFString::new("AXTrustedCheckOptionPrompt").as_CFType(),
                    CFBoolean::true_value().as_CFType(),
                )]);
                AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef().cast()) != 0
            } else {
                AXIsProcessTrusted() != 0
            };
            let screen_recording = CGPreflightScreenCaptureAccess() || (ask && CGRequestScreenCaptureAccess());
            Permissions { accessibility, screen_recording }
        }
    }
}

/// The main display's size, in points.
pub fn screen_size() -> (u32, u32) {
    let d = screen::main_display();
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    (d.w.round() as u32, d.h.round() as u32)
}

/// How long a look may read for, and how many elements.
const LOOK_MS: u128 = 4000;
const NODES_MAX: usize = 3000;
/// The owner's hands on the mouse or keyboard this recently mean he is using the computer.
const OWNER_ACTIVE_S: f64 = 1.5;
/// How long after an action the screen is given to settle before the next look.
const SETTLE_MS: u64 = 500;
const LAUNCH_SETTLE_MS: u64 = 2500;
/// Names of menu items that end or lock the owner's session.
const SESSION_ENDING: &[&str] = &["log out", "shut down", "restart", "lock screen", "sleep", "force quit"];

/// One look, kept so the next action can find its refs and tell whether they moved.
struct Seen {
    targets: HashMap<String, Target>,
    elements: Vec<Ax>,
    /// The window each target is in, by handle, to bring it forward or say what covers it.
    pid_of: HashMap<usize, i32>,
}

/// The desktop as one computer runs it. Not shared: the app runs one call at a time.
pub struct Desktop {
    book: RefBook,
    seen: Option<Seen>,
    input: Input,
    /// Where `open_file` may open files from.
    roots: Vec<PathBuf>,
    /// Text never shown to an agent.
    redact: Vec<String>,
}

impl Default for Desktop {
    fn default() -> Self {
        Desktop::new(dirs_home().into_iter().collect())
    }
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

impl Desktop {
    pub fn new(roots: Vec<PathBuf>) -> Desktop {
        Desktop {
            book: RefBook::default(),
            seen: None,
            input: Input::new(),
            roots,
            redact: Vec::new(),
        }
    }

    /// Runs desktop tool `op` with its JSON input.
    pub fn run(&mut self, op: &str, input: &str) -> Result<Answer> {
        let p = Permissions::check(false);
        match op {
            "desktop_look" => {
                ensure!(p.accessibility, NEEDS_ACCESSIBILITY);
                Ok(Answer { text: self.look()?, jpeg: None })
            }
            "desktop_act" => {
                ensure!(p.accessibility, NEEDS_ACCESSIBILITY);
                let act: Act = serde_json::from_str(input).context("bad input for desktop_act")?;
                self.act(act)
            }
            "desktop_capture" => {
                ensure!(p.screen_recording, NEEDS_SCREEN);
                let c: Capture = serde_json::from_str(input).context("bad input for desktop_capture")?;
                self.capture(c)
            }
            "desktop_apps" => Ok(Answer { text: apps().join("\n"), jpeg: None }),
            "desktop_text" => {
                ensure!(p.screen_recording, NEEDS_SCREEN);
                Ok(Answer { text: self.text()?, jpeg: None })
            }
            other => bail!("no desktop tool {other:?}"),
        }
    }

    /// Whether the owner's hands are on the mouse or keyboard now.
    fn owner_active(&self) -> bool {
        self.input.owner_since(OWNER_ACTIVE_S)
    }

    fn check_owner(&self) -> Result<()> {
        ensure!(
            !self.owner_active(),
            "The owner is using this computer right now (mouse or keyboard). Nothing was done: wait, or ask him."
        );
        Ok(())
    }

    // ------------------------------------------------------------------ look

    fn look(&mut self) -> Result<String> {
        let display = screen::main_display();
        let system = Ax::system();
        let windows = screen::windows();
        let front_pid = std::env::var("LIFEOS_DESKTOP_PID")
            .ok()
            .and_then(|p| p.parse().ok())
            .or_else(|| screen::front_pid(&system))
            .or_else(|| windows.iter().find(|w| w.layer == 0).map(|w| w.pid));
        let mut reader = Reader::new(LOOK_MS, NODES_MAX);
        let mut out = String::new();
        let mut targets = HashMap::new();
        let mut pid_of = HashMap::new();
        let _ = writeln!(out, "Screen {}×{} points.", display.w.round(), display.h.round());
        let app_name = |pid: i32| windows.iter().find(|w| w.pid == pid).map(|w| w.app.clone()).unwrap_or_default();

        if let Some(pid) = front_pid {
            let app = Ax::app(pid);
            // Chromium and Electron apps show their web content to accessibility only when asked.
            let _ = app.set_bool("AXManualAccessibility", true);
            let _ = app.set_bool("AXEnhancedUserInterface", true);
            let name = app.text("AXTitle").unwrap_or_else(|| app_name(pid));
            let _ = writeln!(out, "Front app: {name}");
            let focused = app.element("AXFocusedWindow");
            let wins = app.elements("AXWindows");
            let mut trees: Vec<Node> = Vec::new();
            // The focused window in full; the app's other windows as one line each.
            let t0 = Instant::now();
            if let Some(w) = focused.as_ref().or_else(|| wins.first()) {
                trees.push(reader.read(w, display));
            }
            trace(|| format!("front window: {} nodes in {} ms", reader.elements.len(), t0.elapsed().as_millis()));
            if std::env::var_os("LIFEOS_DESKTOP_DUMP").is_some() {
                fn dump(n: &Node, d: usize) {
                    eprintln!("{}{} {:?} {:?} {:?} kids={}", "  ".repeat(d), n.role, n.subrole, n.name, n.frame.map(|f| (f.x as i64, f.y as i64, f.w as i64, f.h as i64)), n.children.len());
                    for c in &n.children { dump(c, d + 1); }
                }
                for t in &trees { dump(t, 0); }
            }
            let mut look = Look { book: &mut self.book, out: String::new(), targets: HashMap::new(), clip: display, redact: &self.redact };
            for t in &trees {
                look.node(t, 0, &format!("{name}/window"));
            }
            let mut behind: HashMap<String, u32> = HashMap::new();
            for w in wins.iter().filter(|w| focused.as_ref().is_none_or(|f| !same_element(f, w))) {
                let title = w.text("AXTitle").unwrap_or_default();
                let handle = reader.elements.len();
                reader.elements.push(w.clone());
                let nth = {
                    let n = behind.entry(title.clone()).or_insert(0);
                    *n += 1;
                    *n
                };
                let r = look.book.get(&format!("{name}/other-window/{title}#{nth}"));
                let _ = writeln!(look.out, "window {} [{r}] (behind)", quote(&title));
                look.targets.insert(
                    r,
                    Target { handle, role: "AXWindow".into(), name: title, frame: w.frame(), checked: None, editable: false },
                );
            }
            // The menu bar: its menus, and an open menu in full.
            if let Some(bar) = app.element("AXMenuBar") {
                let node = reader.read(&bar, display);
                let _ = writeln!(look.out, "Menu bar:");
                look.node(&node, 1, &format!("{name}/menubar"));
            }
            out.push_str(&look.out);
            for (r, t) in look.targets {
                pid_of.insert(t.handle, pid);
                targets.insert(r, t);
            }
        } else {
            out.push_str("No app is in front.\n");
        }

        trace(|| format!("front app done: {} nodes", reader.elements.len()));
        // Other apps' windows on the screen, front to back: a focus brings one forward.
        let others: Vec<&screen::Window> =
            windows.iter().filter(|w| w.layer == 0 && Some(w.pid) != front_pid && w.frame.intersect(display).is_some()).collect();
        if !others.is_empty() {
            out.push_str("Other windows:\n");
            let mut seen_apps: HashMap<i32, Vec<Ax>> = HashMap::new();
            let mut named: HashMap<String, u32> = HashMap::new();
            for w in others.into_iter().take(30) {
                let wins = seen_apps.entry(w.pid).or_insert_with(|| Ax::app_quick(w.pid).elements("AXWindows"));
                let found = wins.iter().find(|a| a.frame().is_some_and(|f| f.same(w.frame))).cloned();
                let label = if w.title.is_empty() { w.app.clone() } else { format!("{} — {}", w.title, w.app) };
                let nth = {
                    let n = named.entry(label.clone()).or_insert(0);
                    *n += 1;
                    *n
                };
                match found {
                    Some(el) => {
                        let handle = reader.elements.len();
                        reader.elements.push(el);
                        let r = self.book.get(&format!("{}/window/{}#{nth}", w.app, w.title));
                        let _ = writeln!(out, "  window {} [{r}]", quote(&label));
                        pid_of.insert(handle, w.pid);
                        targets.insert(
                            r,
                            Target { handle, role: "AXWindow".into(), name: label, frame: Some(w.frame), checked: None, editable: false },
                        );
                    }
                    None => {
                        let _ = writeln!(out, "  window {}", quote(&label));
                    }
                }
            }
        }
        trace(|| "other windows done".to_owned());
        if self.owner_active() {
            out.push_str("The owner is using this computer right now: wait before acting.\n");
        }
        self.seen = Some(Seen { targets, elements: reader.elements, pid_of });
        Ok(out)
    }

    // ------------------------------------------------------------------ act

    fn act(&mut self, act: Act) -> Result<Answer> {
        let settle_ms = match &act.action {
            Action::Launch { .. } | Action::OpenFile { .. } => LAUNCH_SETTLE_MS,
            Action::Pause { .. } => 0,
            _ => SETTLE_MS,
        };
        let said = self.act_now(act.action)?;
        if !act.settle {
            return Ok(Answer { text: said.unwrap_or_else(|| "Done.".into()), jpeg: None });
        }
        sleep(Duration::from_millis(settle_ms));
        let look = self.look()?;
        Ok(Answer { text: match said { Some(s) => format!("{s}\n{look}"), None => look }, jpeg: None })
    }

    /// Does the action; what it has to say beyond the look, if anything.
    fn act_now(&mut self, action: Action) -> Result<Option<String>> {
        match action {
            Action::Click { at } => {
                let spot = self.spot(&at, true)?;
                if let Some(el) = spot.element.as_ref().filter(|e| e.actions().iter().any(|a| a == "AXPress")) {
                    refuse_session(&spot)?;
                    if el.perform("AXPress") {
                        return Ok(None);
                    }
                }
                self.pointer(&spot)?;
                self.check_owner()?;
                self.input.click(spot.x, spot.y, Button::Left, 1);
            }
            Action::DoubleClick { at } => {
                let spot = self.pointer_spot(&at)?;
                self.input.click(spot.x, spot.y, Button::Left, 2);
            }
            Action::RightClick { at } => {
                let spot = self.spot(&at, true)?;
                if let Some(el) = spot.element.as_ref().filter(|e| e.actions().iter().any(|a| a == "AXShowMenu")) {
                    if el.perform("AXShowMenu") {
                        return Ok(None);
                    }
                }
                self.pointer(&spot)?;
                self.check_owner()?;
                self.input.click(spot.x, spot.y, Button::Right, 1);
            }
            Action::Hover { at } => {
                let spot = self.pointer_spot(&at)?;
                self.input.move_to(spot.x, spot.y);
            }
            Action::Focus { at } => {
                let spot = self.spot(&at, false)?;
                let el = spot.element.context("focus takes a window's ref")?;
                let pid = el.pid().context("the window's app is gone")?;
                let app = Ax::app(pid);
                let _ = app.set_bool("AXFrontmost", true);
                let _ = el.perform("AXRaise");
                let _ = el.set_bool("AXMain", true);
            }
            Action::Check { at } => return self.toggle(&at, true),
            Action::Uncheck { at } => return self.toggle(&at, false),
            Action::Drag { from, to } => {
                let a = self.spot(&from, true)?;
                let b = self.spot(&to, true)?;
                self.check_owner()?;
                self.input.drag((a.x, a.y), (b.x, b.y));
            }
            Action::Fill { at, text } => {
                let spot = self.spot(&at, true)?;
                let el = spot.element.clone();
                if let Some(el) = el.filter(|e| e.settable("AXValue")) {
                    let _ = el.set_bool("AXFocused", true);
                    if el.set("AXValue", &CFString::new(&text).as_CFType()) {
                        return Ok(None);
                    }
                }
                self.focus_field(&spot)?;
                self.input.press(keys::chord("cmd+a").map_err(|e| anyhow!(e))?);
                if text.is_empty() {
                    self.input.press(keys::chord("backspace").map_err(|e| anyhow!(e))?);
                } else {
                    self.type_text(&text)?;
                }
            }
            Action::Type { text, at } => {
                ensure!(!text.is_empty() && text.chars().count() <= 4000, "type takes 1 to 4000 characters");
                if let Some(at) = at {
                    let spot = self.spot(&at, true)?;
                    self.focus_field(&spot)?;
                }
                self.check_owner()?;
                self.type_text(&text)?;
            }
            Action::Select { at, value } => return self.select(&at, &value),
            Action::Press { key } => {
                let chord = keys::chord(&key).map_err(|e| anyhow!(e))?;
                self.check_owner()?;
                self.input.press(chord);
            }
            Action::Scroll { direction, pixels, at } => {
                let (x, y) = match at {
                    Some(at) => {
                        let s = self.spot(&at, true)?;
                        (s.x, s.y)
                    }
                    None => front_window_center()?,
                };
                self.check_owner()?;
                let px = i32::try_from(pixels.clamp(1, 5000)).unwrap_or(400);
                let (dx, dy) = match direction {
                    Direction::Up => (0, -px),
                    Direction::Down => (0, px),
                    Direction::Left => (-px, 0),
                    Direction::Right => (px, 0),
                };
                self.input.scroll(x, y, dx, dy);
            }
            Action::Launch { app } => {
                ensure!(!app.trim().is_empty() && !app.contains('/'), "launch takes an app's name from desktop_apps");
                let out = Command::new("/usr/bin/open").arg("-a").arg(app.trim()).output().context("open")?;
                ensure!(out.status.success(), "couldn't open {app}: {}", String::from_utf8_lossy(&out.stderr).trim());
            }
            Action::OpenFile { path } => {
                let file = self.openable(&path)?;
                let out = Command::new("/usr/bin/open").arg(&file).output().context("open")?;
                ensure!(out.status.success(), "couldn't open {path}: {}", String::from_utf8_lossy(&out.stderr).trim());
            }
            Action::Pause { ms } => sleep(Duration::from_millis(u64::from(ms.min(10_000)))),
        }
        Ok(None)
    }

    fn toggle(&mut self, at: &At, on: bool) -> Result<Option<String>> {
        let spot = self.spot(at, true)?;
        if spot.target.as_ref().and_then(|t| t.checked) == Some(on) {
            return Ok(Some(format!("Already {}.", if on { "checked" } else { "unchecked" })));
        }
        if let Some(el) = &spot.element {
            if el.perform("AXPress") {
                return Ok(None);
            }
        }
        self.pointer(&spot)?;
        self.check_owner()?;
        self.input.click(spot.x, spot.y, Button::Left, 1);
        Ok(None)
    }

    fn select(&mut self, at: &At, value: &str) -> Result<Option<String>> {
        let spot = self.spot(at, true)?;
        let el = spot.element.clone().context("select takes an element's ref")?;
        let role = el.text("AXRole").unwrap_or_default();
        if role == "AXComboBox" && el.settable("AXValue") && el.set("AXValue", &CFString::new(value).as_CFType()) {
            return Ok(None);
        }
        ensure!(el.perform("AXPress") || el.perform("AXShowMenu"), "couldn't open {}", spot.label());
        sleep(Duration::from_millis(300));
        let want = plain(value);
        let mut stack: Vec<Ax> = el.elements("AXChildren");
        let mut names = Vec::new();
        let mut steps = 0;
        while let Some(e) = stack.pop() {
            steps += 1;
            if steps > 2000 {
                break;
            }
            if e.text("AXRole").as_deref() == Some("AXMenuItem") {
                let title = e.text("AXTitle").unwrap_or_default();
                if plain(&title) == want {
                    ensure!(e.perform("AXPress"), "couldn't pick {title:?}");
                    return Ok(None);
                }
                if !title.is_empty() {
                    names.push(title);
                }
            }
            stack.extend(e.elements("AXChildren"));
        }
        self.input.press(keys::chord("escape").map_err(|e| anyhow!(e))?);
        bail!("{} has no option {value:?}; it has: {}", spot.label(), names.join(", "))
    }

    fn type_text(&mut self, text: &str) -> Result<()> {
        let result = self.input.type_text(text);
        match result {
            Ok(()) => Ok(()),
            Err(done) => bail!("The owner took the keyboard or mouse: stopped after {done} characters. Wait, or ask him."),
        }
    }

    fn openable(&self, path: &str) -> Result<PathBuf> {
        let p = Path::new(path.trim());
        ensure!(p.is_absolute(), "open_file takes an absolute path");
        let real = p.canonicalize().with_context(|| format!("there is no file {path}"))?;
        ensure!(
            self.roots.iter().filter_map(|r| r.canonicalize().ok()).any(|r| real.starts_with(r)),
            "open_file opens files under the owner's home only"
        );
        Ok(real)
    }

    // ------------------------------------------------------------------ where actions land

    /// The point and element `at` names, checked against what is there now: a ref that moved or went away is
    /// stale, a point off the screen is refused.
    fn spot(&self, at: &At, needs_point: bool) -> Result<Spot> {
        let display = screen::main_display();
        if let Some(r) = at.reference.as_deref() {
            let seen = self.seen.as_ref().ok_or_else(|| anyhow!("stale ref {r}: look at the screen first"))?;
            let t = seen.targets.get(r).ok_or_else(|| anyhow!("stale ref {r}: it isn't in the last look. Look again."))?;
            let el = seen.elements.get(t.handle).cloned().context("stale ref")?;
            let now = el.frame();
            if needs_point {
                let (Some(was), Some(now)) = (t.frame, now) else {
                    bail!("stale ref {r}: {} isn't on the screen any more. Look again.", t.name)
                };
                ensure!(was.same(now), "stale ref {r}: it moved since the last look. Look again.");
            }
            let (x, y) = now.map(Rect::center).unwrap_or((0.0, 0.0));
            if needs_point {
                ensure!(display.contains((x, y)), "{r} is off the screen; scroll to it first");
            }
            return Ok(Spot {
                x,
                y,
                element: Some(el),
                target: Some(t.clone()),
                pid: seen.pid_of.get(&t.handle).copied(),
                reference: Some(r.to_owned()),
            });
        }
        let (Some(x), Some(y)) = (at.x, at.y) else { bail!("an action lands on an element (ref, like e12) or a point (x and y)") };
        ensure!(display.contains((x, y)), "{x}, {y} is off the screen ({}×{})", display.w, display.h);
        Ok(Spot { x, y, element: None, target: None, pid: None, reference: None })
    }

    fn pointer_spot(&mut self, at: &At) -> Result<Spot> {
        let spot = self.spot(at, true)?;
        self.pointer(&spot)?;
        self.check_owner()?;
        Ok(spot)
    }

    /// Refuses a pointer action on a ref another window covers now.
    fn pointer(&mut self, spot: &Spot) -> Result<()> {
        refuse_session(spot)?;
        let Some(pid) = spot.pid else { return Ok(()) };
        let windows = screen::windows();
        if let Some(top) = screen::top_at(&windows, (spot.x, spot.y)) {
            // The menu bar (24) and its status items (25) are drawn by other processes over the app's own menus.
            ensure!(
                top.pid == pid || matches!(top.layer, 24 | 25),
                "{} is covered by {}; focus its window first",
                spot.label(),
                if top.title.is_empty() { top.app.clone() } else { format!("{:?} of {}", top.title, top.app) }
            );
        }
        Ok(())
    }

    /// Puts the keyboard in a field: through accessibility when the app takes it, else by clicking it.
    fn focus_field(&mut self, spot: &Spot) -> Result<()> {
        if let Some(el) = &spot.element {
            if el.set_bool("AXFocused", true) && el.text("AXFocused").as_deref() == Some("1") {
                return Ok(());
            }
        }
        self.pointer(spot)?;
        self.check_owner()?;
        self.input.click(spot.x, spot.y, Button::Left, 1);
        sleep(Duration::from_millis(80));
        Ok(())
    }

    // ------------------------------------------------------------------ capture and text

    fn capture(&mut self, c: Capture) -> Result<Answer> {
        let display = screen::main_display();
        let region = c.region.map(|r| Rect { x: r.x, y: r.y, w: r.width, h: r.height });
        if let Some(r) = region {
            ensure!(r.w >= 1.0 && r.h >= 1.0, "a region is at least 1 point a side");
            ensure!(display.intersect(r).is_some(), "the region is off the screen");
        }
        let dir = tempfile::tempdir()?;
        let file = dir.path().join("shot.jpg");
        let (w, h) = screen::capture(region, 70, &file)?;
        let jpeg = STANDARD.encode(std::fs::read(&file)?);
        let origin = region.map_or((0.0, 0.0), |r| (r.x, r.y));
        let text = format!(
            "Screenshot {w}×{h}; one pixel is one screen point, its top left at {}, {} on the screen (add that to a pixel's \
             position for desktop_act's x, y).",
            origin.0, origin.1
        );
        Ok(Answer { text, jpeg: Some(jpeg) })
    }

    fn text(&mut self) -> Result<String> {
        let display = screen::main_display();
        let dir = tempfile::tempdir()?;
        let file = dir.path().join("shot.jpg");
        screen::capture(None, 90, &file)?;
        let lines = ocr::read(&file, display.w, display.h)?;
        if lines.is_empty() {
            return Ok("No text found on the screen.".into());
        }
        let mut out = String::new();
        for l in lines {
            let mut t = l.text;
            for s in self.redact.iter().filter(|s| s.len() >= 4) {
                t = t.replace(s.as_str(), "[secret]");
            }
            let _ = writeln!(out, "{t:?} at {:.0}, {:.0} ({:.0}×{:.0})", l.x + l.w / 2.0, l.y + l.h / 2.0, l.w, l.h);
        }
        Ok(out)
    }
}

const NEEDS_ACCESSIBILITY: &str = "macOS hasn't let LifeOS Computer use Accessibility on this computer: the owner turns it \
                                   on in System Settings → Privacy & Security → Accessibility.";
const NEEDS_SCREEN: &str = "macOS hasn't let LifeOS Computer record the screen on this computer: the owner turns it on in \
                            System Settings → Privacy & Security → Screen & System Audio Recording.";

/// Where an action lands.
struct Spot {
    x: f64,
    y: f64,
    element: Option<Ax>,
    target: Option<Target>,
    pid: Option<i32>,
    reference: Option<String>,
}

impl Spot {
    fn label(&self) -> String {
        match (&self.reference, &self.target) {
            (Some(r), Some(t)) if !t.name.is_empty() => format!("{r} ({:?})", t.name),
            (Some(r), _) => r.clone(),
            _ => format!("{:.0}, {:.0}", self.x, self.y),
        }
    }
}

/// Refuses menu items that log out, lock, restart or shut down.
fn refuse_session(spot: &Spot) -> Result<()> {
    if let Some(t) = &spot.target {
        let name = plain(&t.name);
        if t.role == "AXMenuItem" && SESSION_ENDING.iter().any(|s| name.starts_with(s)) {
            bail!("{:?} would end or lock the owner's session: an agent never does that", t.name);
        }
    }
    Ok(())
}

/// Timings on stderr when `LIFEOS_DESKTOP_TRACE` is set.
fn trace(what: impl FnOnce() -> String) {
    if std::env::var_os("LIFEOS_DESKTOP_TRACE").is_some() {
        eprintln!("[desktop] {}", what());
    }
}

fn plain(s: &str) -> String {
    s.trim().trim_end_matches(['…', '.']).to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ")
}

fn same_element(a: &Ax, b: &Ax) -> bool {
    a.frame() == b.frame() && a.text("AXTitle") == b.text("AXTitle")
}

fn front_window_center() -> Result<(f64, f64)> {
    let display = screen::main_display();
    let windows = screen::windows();
    let front = windows.iter().find(|w| w.layer == 0).context("no window to scroll")?;
    let visible = front.frame.intersect(display).context("the front window is off the screen")?;
    Ok(visible.center())
}

/// The apps installed, by name, sorted.
fn apps() -> Vec<String> {
    let mut dirs = vec![
        PathBuf::from("/Applications"),
        PathBuf::from("/Applications/Utilities"),
        PathBuf::from("/System/Applications"),
        PathBuf::from("/System/Applications/Utilities"),
    ];
    if let Some(h) = dirs_home() {
        dirs.push(h.join("Applications"));
    }
    let mut out: Vec<String> = dirs
        .iter()
        .filter_map(|d| std::fs::read_dir(d).ok())
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().to_str().and_then(|n| n.strip_suffix(".app")).map(String::from))
        .collect();
    out.sort_by_key(|n| n.to_lowercase());
    out.dedup();
    out
}
