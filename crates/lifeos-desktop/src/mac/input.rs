//! Synthetic input through CGEvent: the pointer, the wheel and keys, posted as if from the hardware.

use std::ffi::c_void;
use std::thread::sleep;
use std::time::Duration;

use super::ax::CGPoint;
use crate::keys::{self, Chord};

type CGEventRef = *const c_void;
type CGEventSourceRef = *const c_void;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventSourceCreate(state: i32) -> CGEventSourceRef;
    fn CGEventCreateMouseEvent(source: CGEventSourceRef, kind: u32, at: CGPoint, button: u32) -> CGEventRef;
    fn CGEventCreateKeyboardEvent(source: CGEventSourceRef, code: u16, down: bool) -> CGEventRef;
    fn CGEventCreateScrollWheelEvent2(source: CGEventSourceRef, units: u32, count: u32, w1: i32, w2: i32, w3: i32) -> CGEventRef;
    fn CGEventKeyboardSetUnicodeString(e: CGEventRef, len: usize, s: *const u16);
    fn CGEventSetFlags(e: CGEventRef, flags: u64);
    fn CGEventSetIntegerValueField(e: CGEventRef, field: u32, value: i64);
    fn CGEventSetLocation(e: CGEventRef, at: CGPoint);
    fn CGEventPost(tap: u32, e: CGEventRef);
    fn CGEventSourceSecondsSinceLastEventType(state: i32, kind: u32) -> f64;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(cf: *const c_void);
}

const HID_TAP: u32 = 0;
const SOURCE_PRIVATE: i32 = -1;
const SOURCE_HID: i32 = 1;
const ANY_EVENT: u32 = !0;
const MOUSE_MOVED: u32 = 5;
const CLICK_STATE: u32 = 1;
/// Marks our events, so a tap could tell them from the owner's.
const USER_DATA: u32 = 42;
const OURS: i64 = 0x6c69_6665; // "life"

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Left,
    Right,
}

impl Button {
    /// (down, up, dragged) event types and the button number.
    fn kinds(self) -> (u32, u32, u32, u32) {
        match self {
            Button::Left => (1, 2, 6, 0),
            Button::Right => (3, 4, 7, 1),
        }
    }
}

/// Posts events from a private source, so the owner's modifier keys held now don't leak into ours, and remembers
/// when it last did: macOS counts our events as input too, so input more recent than our last is the owner's.
pub struct Input(CGEventSourceRef, std::cell::Cell<std::time::Instant>);

// SAFETY: a CGEventSource is a CFType; we use it from one thread at a time (the desktop's lock).
unsafe impl Send for Input {}

impl Drop for Input {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: we own the one reference.
            unsafe { CFRelease(self.0) }
        }
    }
}

/// Between a key's down and up, and after it.
const KEY_MS: u64 = 6;
/// Between a press and release of the pointer.
const CLICK_MS: u64 = 40;
/// Text is typed this many UTF-16 units per event.
const CHUNK: usize = 16;

impl Input {
    pub fn new() -> Input {
        // SAFETY: returns a +1 reference or null (then events come from no source, which also works).
        let long_ago = std::time::Instant::now().checked_sub(Duration::from_secs(3600)).unwrap_or_else(std::time::Instant::now);
        Input(unsafe { CGEventSourceCreate(SOURCE_PRIVATE) }, std::cell::Cell::new(long_ago))
    }

    fn post(&self, e: CGEventRef) {
        if e.is_null() {
            return;
        }
        // SAFETY: e is a +1 event we release after posting.
        unsafe {
            CGEventSetIntegerValueField(e, USER_DATA, OURS);
            CGEventPost(HID_TAP, e);
            CFRelease(e);
        }
        self.1.set(std::time::Instant::now());
    }

    /// Whether the owner moved the mouse or pressed a key in the last `within` seconds and after our last event.
    pub fn owner_since(&self, within: f64) -> bool {
        let idle = Self::owner_idle();
        idle < within && idle + 0.03 < self.1.get().elapsed().as_secs_f64()
    }

    /// Seconds since the last input, ours or the owner's.
    fn owner_idle() -> f64 {
        // SAFETY: a plain query.
        unsafe { CGEventSourceSecondsSinceLastEventType(SOURCE_HID, ANY_EVENT) }
    }

    pub fn move_to(&self, x: f64, y: f64) {
        // SAFETY: a +1 event handed to post.
        self.post(unsafe { CGEventCreateMouseEvent(self.0, MOUSE_MOVED, CGPoint { x, y }, 0) });
    }

    pub fn click(&self, x: f64, y: f64, button: Button, count: u32) {
        let (down, up, _, b) = button.kinds();
        let at = CGPoint { x, y };
        self.move_to(x, y);
        sleep(Duration::from_millis(30));
        for n in 1..=count {
            // SAFETY: +1 events handed to post.
            unsafe {
                let d = CGEventCreateMouseEvent(self.0, down, at, b);
                if !d.is_null() {
                    CGEventSetIntegerValueField(d, CLICK_STATE, i64::from(n));
                }
                self.post(d);
                sleep(Duration::from_millis(CLICK_MS));
                let u = CGEventCreateMouseEvent(self.0, up, at, b);
                if !u.is_null() {
                    CGEventSetIntegerValueField(u, CLICK_STATE, i64::from(n));
                }
                self.post(u);
            }
            sleep(Duration::from_millis(60));
        }
    }

    pub fn drag(&self, from: (f64, f64), to: (f64, f64)) {
        let (down, up, dragged, b) = Button::Left.kinds();
        self.move_to(from.0, from.1);
        sleep(Duration::from_millis(50));
        // SAFETY: +1 events handed to post.
        unsafe {
            self.post(CGEventCreateMouseEvent(self.0, down, CGPoint { x: from.0, y: from.1 }, b));
            sleep(Duration::from_millis(100));
            let steps = 20;
            for i in 1..=steps {
                let t = f64::from(i) / f64::from(steps);
                let at = CGPoint { x: from.0 + (to.0 - from.0) * t, y: from.1 + (to.1 - from.1) * t };
                self.post(CGEventCreateMouseEvent(self.0, dragged, at, b));
                sleep(Duration::from_millis(16));
            }
            sleep(Duration::from_millis(100));
            self.post(CGEventCreateMouseEvent(self.0, up, CGPoint { x: to.0, y: to.1 }, b));
        }
    }

    /// Scrolls at (x, y) by pixels: positive dy scrolls the content down (toward its end).
    pub fn scroll(&self, x: f64, y: f64, dx: i32, dy: i32) {
        self.move_to(x, y);
        sleep(Duration::from_millis(30));
        let step = 40;
        let n = (dx.abs().max(dy.abs()) / step).max(1);
        for _ in 0..n {
            // SAFETY: +1 event handed to post. Wheel 1 is vertical, positive up; wheel 2 horizontal, positive left.
            unsafe {
                let e = CGEventCreateScrollWheelEvent2(self.0, 0, 2, -dy.signum() * step.min(dy.abs()), -dx.signum() * step.min(dx.abs()), 0);
                if !e.is_null() {
                    CGEventSetLocation(e, CGPoint { x, y });
                }
                self.post(e);
            }
            sleep(Duration::from_millis(12));
        }
    }

    pub fn press(&self, c: Chord) {
        // SAFETY: +1 events handed to post.
        unsafe {
            let d = CGEventCreateKeyboardEvent(self.0, c.code, true);
            if !d.is_null() {
                CGEventSetFlags(d, c.flags);
            }
            self.post(d);
            sleep(Duration::from_millis(KEY_MS));
            let u = CGEventCreateKeyboardEvent(self.0, c.code, false);
            if !u.is_null() {
                CGEventSetFlags(u, c.flags);
            }
            self.post(u);
        }
        sleep(Duration::from_millis(KEY_MS));
    }

    /// Types text into whatever has the keyboard focus; stops between two chunks when the owner touches the mouse or
    /// keyboard, with how many characters went out.
    pub fn type_text(&self, text: &str) -> Result<(), usize> {
        let stop = || self.owner_since(f64::MAX);
        let mut done = 0;
        for line in text.split_inclusive(['\n', '\t']) {
            let (body, end) = match line.chars().last() {
                Some('\n') => (&line[..line.len() - 1], Some(keys::RETURN)),
                Some('\t') => (&line[..line.len() - 1], Some(keys::TAB)),
                _ => (line, None),
            };
            let units: Vec<u16> = body.encode_utf16().collect();
            for chunk in units.chunks(CHUNK) {
                if stop() {
                    return Err(done);
                }
                // SAFETY: +1 events handed to post; the slice outlives the calls.
                unsafe {
                    let d = CGEventCreateKeyboardEvent(self.0, 0, true);
                    if !d.is_null() {
                        CGEventSetFlags(d, 0);
                        CGEventKeyboardSetUnicodeString(d, chunk.len(), chunk.as_ptr());
                    }
                    self.post(d);
                    let u = CGEventCreateKeyboardEvent(self.0, 0, false);
                    if !u.is_null() {
                        CGEventSetFlags(u, 0);
                        CGEventKeyboardSetUnicodeString(u, chunk.len(), chunk.as_ptr());
                    }
                    self.post(u);
                }
                done += String::from_utf16_lossy(chunk).chars().count();
                sleep(Duration::from_millis(KEY_MS * 2));
            }
            if let Some(code) = end {
                self.press(Chord { code, flags: 0 });
                done += 1;
            }
        }
        Ok(())
    }
}
