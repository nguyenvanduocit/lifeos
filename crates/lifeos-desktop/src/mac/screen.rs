//! The screen: its size, the windows on it front to back, and pictures of it.

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use core_foundation::array::{CFArray, CFArrayRef};
use core_foundation::base::{CFType, TCFType};
use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;

use crate::tree::Rect;

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct CGRect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGMainDisplayID() -> u32;
    fn CGDisplayBounds(display: u32) -> CGRect;
    fn CGWindowListCopyWindowInfo(option: u32, relative_to: u32) -> CFArrayRef;
    fn CGRectMakeWithDictionaryRepresentation(dict: CFDictionaryRef, rect: *mut CGRect) -> bool;
}

/// The main display, in points.
pub fn main_display() -> Rect {
    // SAFETY: plain queries.
    let r = unsafe { CGDisplayBounds(CGMainDisplayID()) };
    Rect { x: r.x, y: r.y, w: r.w, h: r.h }
}

/// A window on the screen.
#[derive(Debug, Clone)]
pub struct Window {
    pub pid: i32,
    pub app: String,
    pub title: String,
    pub frame: Rect,
    /// 0 for app windows; menus, the menu bar, the Dock and overlays sit higher.
    pub layer: i64,
}

const ON_SCREEN_ONLY: u32 = 1;
const EXCLUDE_DESKTOP: u32 = 16;

/// The windows on the screen, front to back.
pub fn windows() -> Vec<Window> {
    // SAFETY: Copy rule on the array; each item is a dictionary of CF values.
    let raw = unsafe { CGWindowListCopyWindowInfo(ON_SCREEN_ONLY | EXCLUDE_DESKTOP, 0) };
    if raw.is_null() {
        return Vec::new();
    }
    let list: CFArray<CFDictionary<CFString, CFType>> = unsafe { CFArray::wrap_under_create_rule(raw) };
    let mut out = Vec::new();
    for d in list.iter() {
        let num = |k: &str| d.find(CFString::new(k)).and_then(|v| v.downcast::<CFNumber>()).and_then(|n| n.to_i64());
        let text = |k: &str| d.find(CFString::new(k)).and_then(|v| v.downcast::<CFString>()).map(|s| s.to_string());
        let Some(bounds) = d.find(CFString::new("kCGWindowBounds")).and_then(|v| v.downcast::<CFDictionary>()) else {
            continue;
        };
        let mut r = CGRect::default();
        // SAFETY: a live dictionary and an out pointer to a local.
        if !unsafe { CGRectMakeWithDictionaryRepresentation(bounds.as_concrete_TypeRef(), &mut r) } {
            continue;
        }
        let alpha = d
            .find(CFString::new("kCGWindowAlpha"))
            .and_then(|v| v.downcast::<CFNumber>())
            .and_then(|n| n.to_f64())
            .unwrap_or(1.0);
        if alpha <= 0.01 || r.w < 2.0 || r.h < 2.0 {
            continue;
        }
        out.push(Window {
            pid: num("kCGWindowOwnerPID").and_then(|p| i32::try_from(p).ok()).unwrap_or(0),
            app: text("kCGWindowOwnerName").unwrap_or_default(),
            title: text("kCGWindowName").unwrap_or_default(),
            frame: Rect { x: r.x, y: r.y, w: r.w, h: r.h },
            layer: num("kCGWindowLayer").unwrap_or(0),
        });
    }
    out
}

/// The window drawn on top at `point`, among app windows, panels, the menu bar, the Dock and menus (not overlays
/// above them, which take no clicks).
pub fn top_at(windows: &[Window], point: (f64, f64)) -> Option<&Window> {
    windows.iter().find(|w| w.frame.contains(point) && (0..=101).contains(&w.layer))
}

/// A JPEG of `region` (or the main display) at one pixel per point, quality 0–100, into `file`.
pub fn capture(region: Option<Rect>, quality: u8, file: &Path) -> Result<(u32, u32)> {
    let mut cmd = Command::new("/usr/sbin/screencapture");
    cmd.args(["-x", "-t", "jpg"]);
    match region {
        Some(r) => {
            cmd.arg(format!("-R{},{},{},{}", r.x.round(), r.y.round(), r.w.round(), r.h.round()));
        }
        None => {
            cmd.arg("-m");
        }
    }
    let out = cmd.arg(file).output().context("screencapture")?;
    if !out.status.success() || !file.exists() {
        bail!("screencapture failed: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    let area = region.unwrap_or_else(main_display);
    let (w, h) = (area.w.round().max(1.0), area.h.round().max(1.0));
    // Retina screens capture two pixels a point; the agent's coordinates are points.
    let out = Command::new("/usr/bin/sips")
        .args(["-s", "format", "jpeg", "-s", "formatOptions", &quality.to_string(), "-z"])
        .arg(format!("{h}"))
        .arg(format!("{w}"))
        .arg(file)
        .output()
        .context("sips")?;
    if !out.status.success() {
        bail!("sips failed: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Ok((w as u32, h as u32))
}

/// The pid of the frontmost app.
pub fn front_pid(system: &super::ax::Ax) -> Option<i32> {
    system.element("AXFocusedApplication").and_then(|a| a.pid())
}
