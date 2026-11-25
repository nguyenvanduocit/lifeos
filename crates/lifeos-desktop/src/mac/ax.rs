//! The Accessibility API: elements of other apps, their attributes and actions.

use std::ffi::c_void;
use std::time::Instant;

use core_foundation::array::{CFArray, CFArrayRef};
use core_foundation::base::{CFGetTypeID, CFRelease, CFRetain, CFType, CFTypeRef, TCFType};
use core_foundation::boolean::{CFBoolean, CFBooleanGetTypeID};
use core_foundation::number::{CFNumber, CFNumberGetTypeID};
use core_foundation::string::{CFString, CFStringGetTypeID};

use crate::tree::{Node, Rect};

type AXUIElementRef = *const c_void;
type AXError = i32;
const AX_OK: AXError = 0;
const POINT: u32 = 1;
const SIZE: u32 = 2;

#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct CGPoint {
    pub x: f64,
    pub y: f64,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct CGSize {
    w: f64,
    h: f64,
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXUIElementCreateSystemWide() -> AXUIElementRef;
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(e: AXUIElementRef, attr: CFTypeRef, value: *mut CFTypeRef) -> AXError;
    fn AXUIElementCopyMultipleAttributeValues(e: AXUIElementRef, attrs: CFArrayRef, options: u32, values: *mut CFArrayRef) -> AXError;
    fn AXUIElementSetAttributeValue(e: AXUIElementRef, attr: CFTypeRef, value: CFTypeRef) -> AXError;
    fn AXUIElementIsAttributeSettable(e: AXUIElementRef, attr: CFTypeRef, settable: *mut u8) -> AXError;
    fn AXUIElementPerformAction(e: AXUIElementRef, action: CFTypeRef) -> AXError;
    fn AXUIElementCopyActionNames(e: AXUIElementRef, names: *mut CFArrayRef) -> AXError;
    fn AXUIElementGetPid(e: AXUIElementRef, pid: *mut i32) -> AXError;
    fn AXUIElementSetMessagingTimeout(e: AXUIElementRef, seconds: f32) -> AXError;
    fn AXValueGetValue(v: CFTypeRef, kind: u32, out: *mut c_void) -> u8;
    fn AXValueGetTypeID() -> usize;
    fn AXUIElementGetTypeID() -> usize;
}

/// One element of another app, retained while held.
pub struct Ax(AXUIElementRef);

// SAFETY: AXUIElementRef is a CFType: retain and release are thread safe, and AX calls may come from any thread.
unsafe impl Send for Ax {}
unsafe impl Sync for Ax {}

impl Drop for Ax {
    fn drop(&mut self) {
        // SAFETY: we hold one retain on it.
        unsafe { CFRelease(self.0) }
    }
}

impl Clone for Ax {
    fn clone(&self) -> Self {
        // SAFETY: a live CFType.
        unsafe { CFRetain(self.0) };
        Ax(self.0)
    }
}

/// How long one app may take to answer: a hung app must not hang the look.
const TIMEOUT_S: f32 = 1.5;

fn s(name: &str) -> CFString {
    CFString::new(name)
}

impl Ax {
    pub fn system() -> Ax {
        // SAFETY: returns a +1 reference.
        let e = Ax(unsafe { AXUIElementCreateSystemWide() });
        unsafe { AXUIElementSetMessagingTimeout(e.0, TIMEOUT_S) };
        e
    }

    pub fn app(pid: i32) -> Ax {
        // SAFETY: returns a +1 reference.
        let e = Ax(unsafe { AXUIElementCreateApplication(pid) });
        unsafe { AXUIElementSetMessagingTimeout(e.0, TIMEOUT_S) };
        e
    }

    /// An app asked only for a little: a busy one is skipped rather than waited for.
    pub fn app_quick(pid: i32) -> Ax {
        let e = Ax::app(pid);
        // SAFETY: a live element.
        unsafe { AXUIElementSetMessagingTimeout(e.0, 0.25) };
        e
    }

    pub fn pid(&self) -> Option<i32> {
        let mut pid = 0;
        // SAFETY: out pointer to a local.
        (unsafe { AXUIElementGetPid(self.0, &mut pid) } == AX_OK).then_some(pid)
    }

    /// The attribute `name`, as a CFType, if the element has it.
    pub fn get(&self, name: &str) -> Option<CFType> {
        let mut out: CFTypeRef = std::ptr::null();
        // SAFETY: Copy rule: we own what comes back.
        let err = unsafe { AXUIElementCopyAttributeValue(self.0, s(name).as_CFTypeRef(), &mut out) };
        (err == AX_OK && !out.is_null()).then(|| unsafe { CFType::wrap_under_create_rule(out) })
    }

    pub fn element(&self, name: &str) -> Option<Ax> {
        self.get(name).and_then(|v| as_element(&v))
    }

    pub fn elements(&self, name: &str) -> Vec<Ax> {
        self.get(name).map(|v| as_elements(&v)).unwrap_or_default()
    }

    pub fn text(&self, name: &str) -> Option<String> {
        self.get(name).and_then(|v| as_text(&v))
    }

    pub fn frame(&self) -> Option<Rect> {
        let (p, z) = (self.get("AXPosition")?, self.get("AXSize")?);
        frame_of(&p, &z)
    }

    /// Sets `name` to a CF value; whether the app took it.
    pub fn set(&self, name: &str, value: &CFType) -> bool {
        // SAFETY: both live CFTypes.
        unsafe { AXUIElementSetAttributeValue(self.0, s(name).as_CFTypeRef(), value.as_CFTypeRef()) == AX_OK }
    }

    pub fn set_bool(&self, name: &str, v: bool) -> bool {
        let b = if v { CFBoolean::true_value() } else { CFBoolean::false_value() };
        self.set(name, &b.as_CFType())
    }

    pub fn settable(&self, name: &str) -> bool {
        let mut out = 0_u8;
        // SAFETY: out pointer to a local.
        unsafe { AXUIElementIsAttributeSettable(self.0, s(name).as_CFTypeRef(), &mut out) == AX_OK && out != 0 }
    }

    pub fn actions(&self) -> Vec<String> {
        let mut out: CFArrayRef = std::ptr::null();
        // SAFETY: Copy rule.
        if unsafe { AXUIElementCopyActionNames(self.0, &mut out) } != AX_OK || out.is_null() {
            return Vec::new();
        }
        let arr: CFArray<CFType> = unsafe { CFArray::wrap_under_create_rule(out) };
        arr.iter().filter_map(|v| as_text(&v)).collect()
    }

    pub fn perform(&self, action: &str) -> bool {
        // SAFETY: a live element and string.
        unsafe { AXUIElementPerformAction(self.0, s(action).as_CFTypeRef()) == AX_OK }
    }
}

fn as_element(v: &CFType) -> Option<Ax> {
    // SAFETY: type checked before the cast; retained for the new owner.
    unsafe {
        (CFGetTypeID(v.as_CFTypeRef()) == AXUIElementGetTypeID()).then(|| {
            CFRetain(v.as_CFTypeRef());
            Ax(v.as_CFTypeRef())
        })
    }
}

fn as_elements(v: &CFType) -> Vec<Ax> {
    let Some(arr) = v.downcast::<CFArray>() else { return Vec::new() };
    let arr: CFArray<CFType> = unsafe { CFArray::wrap_under_get_rule(arr.as_concrete_TypeRef()) };
    arr.iter().filter_map(|e| as_element(&e)).collect()
}

/// A CF value as an agent reads it: strings, numbers and booleans.
fn as_text(v: &CFType) -> Option<String> {
    // SAFETY: type ids checked before each downcast.
    unsafe {
        let id = CFGetTypeID(v.as_CFTypeRef());
        if id == CFStringGetTypeID() {
            return v.downcast::<CFString>().map(|s| s.to_string());
        }
        if id == CFBooleanGetTypeID() {
            return v.downcast::<CFBoolean>().map(|b| if bool::from(b) { "1" } else { "0" }.to_owned());
        }
        if id == CFNumberGetTypeID() {
            let n = v.downcast::<CFNumber>()?;
            return n.to_i64().map(|i| i.to_string()).or_else(|| n.to_f64().map(|f| format!("{f:.2}")));
        }
    }
    None
}

fn as_bool(v: &CFType) -> Option<bool> {
    v.downcast::<CFBoolean>().map(bool::from)
}

fn frame_of(p: &CFType, z: &CFType) -> Option<Rect> {
    // SAFETY: AXValue type checked; out pointers sized for the asked kinds.
    unsafe {
        let ax = AXValueGetTypeID();
        if CFGetTypeID(p.as_CFTypeRef()) != ax || CFGetTypeID(z.as_CFTypeRef()) != ax {
            return None;
        }
        let (mut pt, mut sz) = (CGPoint::default(), CGSize::default());
        let ok = AXValueGetValue(p.as_CFTypeRef(), POINT, (&raw mut pt).cast()) != 0
            && AXValueGetValue(z.as_CFTypeRef(), SIZE, (&raw mut sz).cast()) != 0;
        ok.then_some(Rect { x: pt.x, y: pt.y, w: sz.w, h: sz.h })
    }
}

/// The attributes read of every node, in one call each.
const ATTRS: &[&str] = &[
    "AXRole", "AXSubrole", "AXTitle", "AXDescription", "AXValue", "AXPlaceholderValue", "AXPosition", "AXSize",
    "AXFocused", "AXEnabled", "AXSelected", "AXExpanded", "AXChildren", "AXLabel",
];

/// Reads trees under elements, keeping each node's element so a ref can be acted on.
pub struct Reader {
    pub elements: Vec<Ax>,
    attrs: CFArray<CFString>,
    started: Instant,
    budget_ms: u128,
    nodes_max: usize,
}

/// The deepest a tree is read: web pages nest deep, but not this deep in anything an agent needs.
const DEPTH_MAX: usize = 64;

impl Reader {
    pub fn new(budget_ms: u128, nodes_max: usize) -> Reader {
        let names: Vec<CFString> = ATTRS.iter().map(|a| s(a)).collect();
        Reader { elements: Vec::new(), attrs: CFArray::from_CFTypes(&names), started: Instant::now(), budget_ms, nodes_max }
    }

    fn spent(&self) -> bool {
        self.elements.len() >= self.nodes_max || self.started.elapsed().as_millis() > self.budget_ms
    }

    /// The tree under e, with `clip` to skip reading what lies wholly outside the screen.
    pub fn read(&mut self, e: &Ax, clip: Rect) -> Node {
        self.read_at(e, clip, 0)
    }

    fn read_at(&mut self, e: &Ax, clip: Rect, depth: usize) -> Node {
        let handle = self.elements.len();
        self.elements.push(e.clone());
        let mut out: CFArrayRef = std::ptr::null();
        // SAFETY: Copy rule; options 0 puts an AXValue error in place of a missing attribute.
        let err = unsafe { AXUIElementCopyMultipleAttributeValues(e.0, self.attrs.as_concrete_TypeRef(), 0, &mut out) };
        let mut node = Node { handle, enabled: true, ..Node::default() };
        if err != AX_OK || out.is_null() {
            return node;
        }
        let values: CFArray<CFType> = unsafe { CFArray::wrap_under_create_rule(out) };
        let at = |i: usize| values.get(isize::try_from(i).unwrap_or(0)).map(|v| (*v).clone());
        let text = |i: usize| at(i).and_then(|v| as_text(&v)).unwrap_or_default();
        node.role = text(0);
        node.subrole = text(1);
        let (title, desc, label) = (text(2), text(3), text(13));
        node.name = [title, label, desc].into_iter().find(|t| !t.is_empty()).unwrap_or_default();
        node.value = at(4).and_then(|v| as_text(&v));
        node.placeholder = text(5);
        node.frame = match (at(6), at(7)) {
            (Some(p), Some(z)) => frame_of(&p, &z),
            _ => None,
        };
        node.focused = at(8).and_then(|v| as_bool(&v)).unwrap_or(false);
        node.enabled = at(9).and_then(|v| as_bool(&v)).unwrap_or(true);
        node.selected = at(10).and_then(|v| as_bool(&v)).unwrap_or(false);
        node.expanded = at(11).and_then(|v| as_bool(&v));
        let off = node.frame.is_some_and(|f| f.w > 0.0 && f.h > 0.0 && clip.intersect(f).is_none());
        // A closed menu still lists its items; reading them all costs much and shows nothing.
        let closed_menu = node.role == "AXMenuBarItem" && !node.selected;
        if off || closed_menu || depth >= DEPTH_MAX {
            return node;
        }
        if let Some(kids) = at(12) {
            for k in as_elements(&kids) {
                if self.spent() {
                    break;
                }
                node.children.push(self.read_at(&k, clip, depth + 1));
            }
        }
        node
    }
}
