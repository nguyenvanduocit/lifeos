//! What an agent reads of the screen: the accessibility tree of the front window as indented lines, each element it
//! can act on marked with a ref (`e12`). Pure: the macOS part reads the tree into [`Node`]s, this turns them into
//! words and remembers which ref is which node.

use std::collections::HashMap;
use std::fmt::Write as _;

/// A rectangle on the screen, in points from the main display's top left.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn center(self) -> (f64, f64) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    pub fn contains(self, (x, y): (f64, f64)) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }

    pub fn intersect(self, o: Rect) -> Option<Rect> {
        let (x0, y0) = (self.x.max(o.x), self.y.max(o.y));
        let (x1, y1) = ((self.x + self.w).min(o.x + o.w), (self.y + self.h).min(o.y + o.h));
        (x1 > x0 && y1 > y0).then(|| Rect { x: x0, y: y0, w: x1 - x0, h: y1 - y0 })
    }

    /// Whether `o` is where this was, give or take a point (layout rounding).
    pub fn same(self, o: Rect) -> bool {
        (self.x - o.x).abs() <= 2.0 && (self.y - o.y).abs() <= 2.0 && (self.w - o.w).abs() <= 2.0 && (self.h - o.h).abs() <= 2.0
    }
}

/// One element of an app's accessibility tree, as read.
#[derive(Debug, Clone, Default)]
pub struct Node {
    /// AXButton, AXTextField…
    pub role: String,
    pub subrole: String,
    /// Its title, description or label.
    pub name: String,
    /// Its value, as text: a field's contents, a checkbox's 0/1, a slider's number.
    pub value: Option<String>,
    pub placeholder: String,
    pub frame: Option<Rect>,
    pub focused: bool,
    pub enabled: bool,
    pub selected: bool,
    pub expanded: Option<bool>,
    pub children: Vec<Node>,
    /// The backend's handle on the element (an index into what it read).
    pub handle: usize,
}

/// What a ref points at.
#[derive(Debug, Clone, PartialEq)]
pub struct Target {
    pub handle: usize,
    pub role: String,
    pub name: String,
    pub frame: Option<Rect>,
    /// A checkbox's or radio button's state, when it has one.
    pub checked: Option<bool>,
    pub editable: bool,
}

/// Refs that stay the same from look to look for the same element (by app, place and name), so an agent's earlier
/// words about `e12` keep meaning the same button.
#[derive(Debug, Default)]
pub struct RefBook {
    ids: HashMap<String, u32>,
    next: u32,
}

impl RefBook {
    const MAX: usize = 50_000;

    pub fn get(&mut self, key: &str) -> String {
        if self.ids.len() >= Self::MAX {
            self.ids.clear();
        }
        let next = &mut self.next;
        let id = *self.ids.entry(key.to_owned()).or_insert_with(|| {
            *next += 1;
            *next
        });
        format!("e{id}")
    }
}

/// The most of a field's text shown.
const VALUE_MAX: usize = 2000;
/// The most of a name shown.
const NAME_MAX: usize = 200;
/// The most a look says, all told.
pub const LOOK_MAX: usize = 40_000;

/// Roles that only arrange others: shown as their children when they have no name of their own.
const STRUCTURAL: &[&str] = &[
    "AXGroup", "AXScrollArea", "AXSplitGroup", "AXLayoutArea", "AXLayoutItem", "AXUnknown", "AXGenericElement",
    "AXSplitter", "AXMatte", "AXRuler", "AXGrowArea", "AXValueIndicator", "AXColumn",
];

/// Roles an agent acts on: each gets a ref.
const ACTIONABLE: &[&str] = &[
    "AXButton", "AXCheckBox", "AXRadioButton", "AXPopUpButton", "AXMenuButton", "AXComboBox", "AXTextField",
    "AXTextArea", "AXSearchField", "AXLink", "AXMenuItem", "AXMenuBarItem", "AXSlider", "AXIncrementor",
    "AXDisclosureTriangle", "AXRow", "AXCell", "AXColorWell", "AXDateField", "AXTimeField", "AXWindow", "AXSheet",
    "AXTab", "AXDockItem", "AXStepper", "AXToggle", "AXSwitch",
];

/// Roles whose value is text someone types.
const EDITABLE: &[&str] = &["AXTextField", "AXTextArea", "AXSearchField", "AXComboBox", "AXDateField", "AXTimeField"];

/// The short word an agent reads for a role.
pub fn role_word(role: &str, subrole: &str) -> String {
    let word = match (role, subrole) {
        (_, "AXSecureTextField") => "password",
        (_, "AXSearchField") => "searchbox",
        (_, "AXSwitch") => "switch",
        (_, "AXToggle") => "toggle",
        (_, "AXTabButton") => "tab",
        ("AXButton", _) => "button",
        ("AXCheckBox", _) => "checkbox",
        ("AXRadioButton", _) => "radio",
        ("AXPopUpButton", _) => "popup",
        ("AXMenuButton", _) => "menubutton",
        ("AXComboBox", _) => "combobox",
        ("AXTextField", _) => "textbox",
        ("AXTextArea", _) => "textarea",
        ("AXStaticText", _) => "text",
        ("AXLink", _) => "link",
        ("AXImage", _) => "image",
        ("AXMenuItem", _) => "menuitem",
        ("AXMenuBarItem", _) => "menu",
        ("AXMenu", _) => "menulist",
        ("AXMenuBar", _) => "menubar",
        ("AXWebArea", _) => "document",
        ("AXHeading", _) => "heading",
        ("AXToolbar", _) => "toolbar",
        ("AXTabGroup", _) => "tabs",
        ("AXTable", _) => "table",
        ("AXOutline", _) => "tree",
        ("AXList", _) => "list",
        ("AXRow", _) => "row",
        ("AXCell", _) => "cell",
        ("AXSlider", _) => "slider",
        ("AXWindow", _) => "window",
        ("AXSheet", _) => "sheet",
        ("AXDialog", _) => "dialog",
        ("AXPopover", _) => "popover",
        ("AXDisclosureTriangle", _) => "disclosure",
        ("AXProgressIndicator", _) => "progress",
        ("AXIncrementor" | "AXStepper", _) => "stepper",
        ("AXGroup", _) => "group",
        _ => return role.strip_prefix("AX").unwrap_or(role).to_lowercase(),
    };
    word.to_owned()
}

/// One element's line, and what of it becomes a target.
pub struct Look<'a> {
    pub book: &'a mut RefBook,
    pub out: String,
    pub targets: HashMap<String, Target>,
    /// What is visible: elements wholly outside it are left out.
    pub clip: Rect,
    /// Text hidden wherever it appears (the owner's secrets).
    pub redact: &'a [String],
}

impl Look<'_> {
    /// Writes node and what is under it at `depth`, keyed under `scope` (the app and window) for stable refs.
    pub fn node(&mut self, node: &Node, depth: usize, scope: &str) {
        let mut seen: HashMap<String, u32> = HashMap::new();
        self.walk(node, depth, scope, &mut seen, self.clip);
    }

    fn walk(&mut self, n: &Node, depth: usize, scope: &str, seen: &mut HashMap<String, u32>, clip: Rect) {
        if self.out.len() >= LOOK_MAX {
            return;
        }
        let visible = match n.frame {
            Some(f) if f.w > 0.0 && f.h > 0.0 => clip.intersect(f),
            // Menus and some web nodes give no frame: their children may.
            _ => Some(clip),
        };
        let Some(visible) = visible else { return };
        // A scrolled list clips what is under it to what shows.
        let inner = if matches!(n.role.as_str(), "AXScrollArea" | "AXWebArea" | "AXList" | "AXTable" | "AXOutline") {
            visible
        } else {
            clip
        };
        let name = self.clean(&n.name, NAME_MAX);
        let structural = STRUCTURAL.contains(&n.role.as_str()) && name.is_empty();
        if structural {
            for c in &n.children {
                self.walk(c, depth, scope, seen, inner);
            }
            return;
        }
        let word = role_word(&n.role, &n.subrole);
        let actionable = ACTIONABLE.contains(&n.role.as_str()) || ACTIONABLE.contains(&n.subrole.as_str());
        let editable = EDITABLE.contains(&n.role.as_str());
        let secure = n.subrole == "AXSecureTextField";
        let value = n.value.as_deref().filter(|_| !secure).map(|v| self.clean(v, if editable { VALUE_MAX } else { NAME_MAX }));
        // Text whose words are its value, and that says nothing else, is shown as its words.
        let (name, value) = match (n.role.as_str(), value) {
            ("AXStaticText" | "AXHeading", Some(v)) if name.is_empty() || name == v => (v, None),
            (_, v) => (name, v),
        };
        if !actionable && name.is_empty() && value.is_none() && n.children.is_empty() {
            return;
        }
        // A tab is a radio button whose value says which is shown: it reads as selected, not checked.
        let tab = n.subrole == "AXTabButton";
        let checked = (matches!(n.role.as_str(), "AXCheckBox" | "AXRadioButton" | "AXSwitch" | "AXToggle") && !tab)
            .then(|| value.as_deref().map(|v| v == "1"))
            .flatten();
        // A heading's value is its level; a tab's says whether it shows.
        let value = if tab || n.role == "AXHeading" { None } else { value };
        let selected = n.selected || tab && n.value.as_deref() == Some("1");
        let mut line = format!("{}{word}", "  ".repeat(depth));
        if !name.is_empty() {
            let _ = write!(line, " {}", quote(&name));
        }
        match (&value, checked) {
            (_, Some(c)) => line.push_str(if c { " checked" } else { " unchecked" }),
            (Some(v), None) if !v.is_empty() && v != &name => {
                let _ = write!(line, " = {}", quote(v));
            }
            _ => {}
        }
        if secure && n.value.as_deref().is_some_and(|v| !v.is_empty()) {
            line.push_str(" (filled)");
        }
        if editable && value.as_deref().is_none_or(str::is_empty) && !n.placeholder.is_empty() {
            let _ = write!(line, " placeholder {}", quote(&self.clean(&n.placeholder, NAME_MAX)));
        }
        if actionable {
            let key = format!("{scope}/{}/{}", n.role, name);
            let nth = seen.entry(key.clone()).or_default();
            *nth += 1;
            let r = self.book.get(&format!("{key}#{nth}"));
            let _ = write!(line, " [{r}]");
            self.targets.insert(
                r,
                Target {
                    handle: n.handle,
                    role: n.role.clone(),
                    name: name.clone(),
                    frame: n.frame,
                    checked,
                    editable,
                },
            );
        }
        if n.focused {
            line.push_str(" focused");
        }
        if !n.enabled && actionable {
            line.push_str(" disabled");
        }
        if selected {
            line.push_str(" selected");
        }
        match n.expanded {
            Some(true) => line.push_str(" expanded"),
            // Menus and bars say collapsed all the time; it only tells something on what opens in place.
            Some(false) if matches!(n.role.as_str(), "AXDisclosureTriangle" | "AXRow" | "AXComboBox" | "AXPopUpButton") => {
                line.push_str(" collapsed");
            }
            _ => {}
        }
        line.push('\n');
        self.out.push_str(&line);
        let scope = if name.is_empty() { scope.to_owned() } else { format!("{scope}/{word}:{name}") };
        let mut seen_below = HashMap::new();
        // A button or heading whose one child is its own words says them once.
        let echo = |c: &Node| c.role == "AXStaticText" && c.children.is_empty() && {
            let words = c.value.as_deref().filter(|v| !v.is_empty()).unwrap_or(&c.name);
            !name.is_empty() && words.trim() == name
        };
        for c in n.children.iter().filter(|c| !echo(c)) {
            self.walk(c, depth + 1, &scope, &mut seen_below, inner);
        }
    }

    /// Text on one line, at most `max` characters, secrets hidden.
    fn clean(&self, s: &str, max: usize) -> String {
        let mut t: String = s.chars().map(|c| if c.is_control() && c != '\n' { ' ' } else { c }).collect();
        for secret in self.redact.iter().filter(|s| s.len() >= 4) {
            t = t.replace(secret.as_str(), "[secret]");
        }
        let t = t.trim();
        if t.chars().count() <= max {
            return t.to_owned();
        }
        let mut cut: String = t.chars().take(max).collect();
        cut.push('…');
        cut
    }
}

/// A name in quotes, its newlines shown as ⏎ so each element stays on one line.
pub fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "⏎"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(role: &str, name: &str, children: Vec<Node>) -> Node {
        Node {
            role: role.into(),
            name: name.into(),
            enabled: true,
            frame: Some(Rect { x: 10.0, y: 10.0, w: 50.0, h: 20.0 }),
            children,
            ..Node::default()
        }
    }

    fn look(node: &Node, book: &mut RefBook) -> (String, HashMap<String, Target>) {
        let mut l = Look {
            book,
            out: String::new(),
            targets: HashMap::new(),
            clip: Rect { x: 0.0, y: 0.0, w: 1000.0, h: 800.0 },
            redact: &[],
        };
        l.node(node, 0, "app");
        (l.out, l.targets)
    }

    #[test]
    fn groups_without_names_fold_into_their_children_and_buttons_get_refs() {
        let tree = n("AXWindow", "Notes", vec![n("AXGroup", "", vec![n("AXButton", "Save", vec![])])]);
        let (out, targets) = look(&tree, &mut RefBook::default());
        assert_eq!(out, "window \"Notes\" [e1]\n  button \"Save\" [e2]\n");
        assert_eq!(targets["e2"].name, "Save");
    }

    #[test]
    fn refs_stay_the_same_for_the_same_element_from_look_to_look() {
        let mut book = RefBook::default();
        let a = n("AXWindow", "W", vec![n("AXButton", "OK", vec![]), n("AXButton", "OK", vec![])]);
        let (first, _) = look(&a, &mut book);
        let b = n("AXWindow", "W", vec![n("AXButton", "Cancel", vec![]), n("AXButton", "OK", vec![]), n("AXButton", "OK", vec![])]);
        let (second, _) = look(&b, &mut book);
        assert!(first.contains("\"OK\" [e2]") && first.contains("\"OK\" [e3]"), "{first}");
        assert!(second.contains("\"Cancel\" [e4]") && second.contains("\"OK\" [e2]"), "{second}");
    }

    #[test]
    fn what_lies_outside_the_screen_is_left_out_and_passwords_never_show() {
        let mut off = n("AXButton", "Far", vec![]);
        off.frame = Some(Rect { x: 5000.0, y: 0.0, w: 10.0, h: 10.0 });
        let mut pw = n("AXTextField", "Password", vec![]);
        pw.subrole = "AXSecureTextField".into();
        pw.value = Some("hunter2".into());
        let (out, _) = look(&n("AXWindow", "W", vec![off, pw]), &mut RefBook::default());
        assert!(!out.contains("Far") && !out.contains("hunter2"), "{out}");
        assert!(out.contains("password \"Password\" (filled)"), "{out}");
    }

    #[test]
    fn checkboxes_say_their_state_and_text_says_its_words() {
        let mut cb = n("AXCheckBox", "Wrap", vec![]);
        cb.value = Some("1".into());
        let mut t = n("AXStaticText", "", vec![]);
        t.value = Some("Hello\nthere".into());
        let (out, targets) = look(&n("AXWindow", "W", vec![cb, t]), &mut RefBook::default());
        assert!(out.contains("checkbox \"Wrap\" checked [e2]"), "{out}");
        assert!(out.contains("text \"Hello⏎there\"\n"), "{out}");
        assert_eq!(targets["e2"].checked, Some(true));
    }
}

#[cfg(test)]
mod web_tests {
    use super::*;

    #[test]
    fn a_web_page_inside_a_scroll_area_shows() {
        let f = |x: f64, y: f64, w: f64, h: f64| Some(Rect { x, y, w, h });
        let node = |role: &str, name: &str, frame, children| Node { role: role.into(), name: name.into(), enabled: true, frame, children, ..Node::default() };
        let heading = node("AXHeading", "Log in", f(752.0, 320.0, 416.0, 56.0), vec![]);
        let web = node("AXWebArea", "Page", f(6.0, 112.0, 1908.0, 872.0), vec![node("AXGroup", "", f(6.0, 112.0, 1908.0, 872.0), vec![heading])]);
        let scroll = node("AXScrollArea", "", f(6.0, 112.0, 1908.0, 872.0), vec![web]);
        let tree = node("AXWindow", "W", f(0.0, 30.0, 1920.0, 960.0), vec![node("AXGroup", "", f(6.0, 112.0, 1908.0, 0.0), vec![]), scroll]);
        let mut book = RefBook::default();
        let mut l = Look { book: &mut book, out: String::new(), targets: HashMap::new(), clip: Rect { x: 0.0, y: 0.0, w: 1920.0, h: 1080.0 }, redact: &[] };
        l.node(&tree, 0, "app");
        assert!(l.out.contains("heading \"Log in\""), "{}", l.out);
    }
}
