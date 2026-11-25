//! Plain text from what Google gives.

/// The text of an HTML mail or document, roughly: no tags, scripts or styles; breaks kept.
pub(crate) fn html_to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len() / 2);
    let mut rest = html;
    while let Some(open) = rest.find('<') {
        out.push_str(&rest[..open]);
        let tag_and_after = &rest[open..];
        let Some(close) = tag_and_after.find('>') else {
            rest = "";
            break;
        };
        let tag = tag_and_after[1..close].trim().to_ascii_lowercase();
        let name: String = tag.trim_start_matches('/').chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
        rest = &tag_and_after[close + 1..];
        if (name == "script" || name == "style") && !tag.starts_with('/') {
            let end = format!("</{name}");
            match rest.to_ascii_lowercase().find(&end) {
                Some(at) => {
                    rest = &rest[at..];
                    if let Some(gt) = rest.find('>') {
                        rest = &rest[gt + 1..];
                    }
                }
                None => rest = "",
            }
            continue;
        }
        if matches!(name.as_str(), "br" | "p" | "div" | "tr" | "li" | "h1" | "h2" | "h3" | "h4" | "table" | "blockquote") {
            out.push('\n');
        }
    }
    out.push_str(rest);
    let decoded = decode_entities(&out);
    let mut text = String::with_capacity(decoded.len());
    let mut blank = 0;
    for line in decoded.lines() {
        let line = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if line.is_empty() {
            blank += 1;
            if blank > 1 {
                continue;
            }
        } else {
            blank = 0;
        }
        text.push_str(&line);
        text.push('\n');
    }
    text.trim().to_owned()
}

fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let after = &rest[at..];
        let Some(end) = after.find(';').filter(|e| *e <= 10) else {
            out.push('&');
            rest = &after[1..];
            continue;
        };
        let entity = &after[1..end];
        let ch = match entity {
            "nbsp" => Some(' '),
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"))
                .and_then(|h| u32::from_str_radix(h, 16).ok())
                .or_else(|| entity.strip_prefix('#').and_then(|d| d.parse().ok()))
                .and_then(char::from_u32),
        };
        match ch {
            Some(c) => {
                out.push(c);
                rest = &after[end + 1..];
            }
            None => {
                out.push('&');
                rest = &after[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// At most `most` characters of `text`, saying when there was more.
pub(crate) fn clip(text: &str, most: usize) -> String {
    match text.char_indices().nth(most) {
        Some((at, _)) => format!("{}\n\n[… {} more characters]", &text[..at], text[at..].chars().count()),
        None => text.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_becomes_readable_text() {
        let html = "<html><style>p{color:red}</style><p>Hi&nbsp;Anna,</p><p>See <a href='x'>this</a> &amp; that.<br>Bye</p>\
                    <script>alert(1)</script>&#39;ok&#x27;";
        assert_eq!(html_to_text(html), "Hi Anna,\n\nSee this & that.\nBye\n'ok'");
    }

    #[test]
    fn clipping_says_how_much_was_left() {
        assert_eq!(clip("héllo", 10), "héllo");
        assert_eq!(clip("héllo", 2), "hé\n\n[… 3 more characters]");
    }
}
