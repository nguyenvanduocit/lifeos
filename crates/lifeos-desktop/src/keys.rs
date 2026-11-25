//! Key names as agents write them (`cmd+s`, `Enter`, `ctrl+shift+Tab`) turned into a Mac virtual key code and
//! modifier flags. Pure.

/// Modifier flags of a CGEvent.
pub const SHIFT: u64 = 0x0002_0000;
pub const CONTROL: u64 = 0x0004_0000;
pub const OPTION: u64 = 0x0008_0000;
pub const COMMAND: u64 = 0x0010_0000;
pub const FN: u64 = 0x0080_0000;

/// The virtual key of Return and Tab, typed for newlines and tabs in text.
pub const RETURN: u16 = 36;
pub const TAB: u16 = 48;

/// One chord: a key and the modifiers held with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chord {
    pub code: u16,
    pub flags: u64,
}

/// Chords that end or lock the owner's session: an agent never presses them.
const SESSION: &[(&str, u64)] = &[
    ("q", COMMAND | SHIFT),          // Log Out
    ("q", COMMAND | SHIFT | OPTION), // Log Out at once
    ("q", COMMAND | CONTROL),        // Lock Screen
    ("power", 0),
    ("eject", 0),
];

/// The chord `input` names, or why it names none.
pub fn chord(input: &str) -> Result<Chord, String> {
    let raw = input.trim();
    if raw.is_empty() || raw.len() > 64 {
        return Err("press takes key names joined by +, like cmd+s, Enter or cmd+shift+t".into());
    }
    let parts: Vec<&str> = if raw == "+" { vec!["+"] } else { raw.split('+').map(str::trim).collect() };
    let (key, mods) = parts.split_last().ok_or("no key")?;
    let mut flags = 0;
    for m in mods {
        flags |= match m.to_ascii_lowercase().as_str() {
            "cmd" | "command" | "meta" | "super" | "win" => COMMAND,
            "ctrl" | "control" => CONTROL,
            "alt" | "option" | "opt" => OPTION,
            "shift" => SHIFT,
            "fn" => FN,
            other => return Err(format!("{other:?} is not a modifier: use cmd, ctrl, alt (option), shift or fn")),
        };
    }
    let lower = key.to_ascii_lowercase();
    if SESSION.iter().any(|(k, f)| *k == lower && *f == flags) {
        return Err(format!("{raw} would log the owner out or lock the screen: an agent never presses it"));
    }
    let code = code_of(&lower).ok_or_else(|| format!("{key:?} is not a key name"))?;
    Ok(Chord { code, flags })
}

fn code_of(key: &str) -> Option<u16> {
    let letters = "asdfhgzxcv\0bqweryt";
    if key.len() == 1 {
        let c = key.as_bytes()[0];
        if let Some(i) = letters.bytes().position(|b| b == c) {
            return u16::try_from(i).ok();
        }
    }
    Some(match key {
        "1" => 18,
        "2" => 19,
        "3" => 20,
        "4" => 21,
        "6" => 22,
        "5" => 23,
        "=" | "equal" => 24,
        "9" => 25,
        "7" => 26,
        "-" | "minus" => 27,
        "8" => 28,
        "0" => 29,
        "]" | "bracketright" => 30,
        "o" => 31,
        "u" => 32,
        "[" | "bracketleft" => 33,
        "i" => 34,
        "p" => 35,
        "enter" | "return" => RETURN,
        "l" => 37,
        "j" => 38,
        "'" | "apostrophe" | "quote" => 39,
        "k" => 40,
        ";" | "semicolon" => 41,
        "\\" | "backslash" => 42,
        "," | "comma" => 43,
        "/" | "slash" => 44,
        "n" => 45,
        "m" => 46,
        "." | "period" => 47,
        "tab" => TAB,
        "space" | " " => 49,
        "`" | "grave" => 50,
        "backspace" => 51,
        "esc" | "escape" => 53,
        "capslock" => 57,
        "f17" => 64,
        "f18" => 79,
        "f19" => 80,
        "f20" => 90,
        "f5" => 96,
        "f6" => 97,
        "f7" => 98,
        "f3" => 99,
        "f8" => 100,
        "f9" => 101,
        "f11" => 103,
        "f13" => 105,
        "f16" => 106,
        "f14" => 107,
        "f10" => 109,
        "f12" => 111,
        "f15" => 113,
        "home" => 115,
        "pageup" | "page_up" => 116,
        "delete" | "del" | "forwarddelete" => 117,
        "f4" => 118,
        "end" => 119,
        "f2" => 120,
        "pagedown" | "page_down" => 121,
        "f1" => 122,
        "left" | "arrowleft" => 123,
        "right" | "arrowright" => 124,
        "down" | "arrowdown" => 125,
        "up" | "arrowup" => 126,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chords_read_as_agents_write_them() {
        assert_eq!(chord("cmd+s"), Ok(Chord { code: 1, flags: COMMAND }));
        assert_eq!(chord("Enter"), Ok(Chord { code: RETURN, flags: 0 }));
        assert_eq!(chord("ctrl+shift+Tab"), Ok(Chord { code: TAB, flags: CONTROL | SHIFT }));
        assert_eq!(chord("cmd+option+esc"), Ok(Chord { code: 53, flags: COMMAND | OPTION }));
        assert_eq!(chord("b").map(|c| c.code), Ok(11));
        assert!(chord("hyper+x").is_err());
        assert!(chord("cmd+nothing").is_err());
    }

    #[test]
    fn chords_that_end_the_session_are_refused() {
        assert!(chord("cmd+shift+q").unwrap_err().contains("log the owner out"));
        assert!(chord("ctrl+cmd+q").is_err());
        assert!(chord("cmd+q").is_ok());
    }
}
