use std::borrow::Cow;

pub fn first_image_source(html: &str) -> Option<String> {
    let bytes = html.as_bytes();
    let mut at = 0;
    while let Some(offset) = bytes[at..].iter().position(|&byte| byte == b'<') {
        let tag_start = at + offset + 1;
        if bytes[tag_start..].starts_with(b"!--") {
            let comment_body = tag_start + 3;
            let end = html[comment_body..].find("-->")?;
            at = comment_body + end + 3;
        } else if starts_image_tag(&bytes[tag_start..]) {
            match scan_image_tag(html, tag_start + 3) {
                Scan::Found(source) => return Some(source),
                Scan::Missing(end) => at = end,
            }
        } else {
            at = tag_start;
        }
    }
    None
}

fn starts_image_tag(rest: &[u8]) -> bool {
    rest.len() > 3
        && rest[..3].eq_ignore_ascii_case(b"img")
        && (rest[3].is_ascii_whitespace() || matches!(rest[3], b'/' | b'>'))
}

enum Scan {
    Found(String),
    Missing(usize),
}

fn scan_image_tag(html: &str, mut at: usize) -> Scan {
    let bytes = html.as_bytes();
    let end = bytes.len();
    loop {
        while at < end && (bytes[at].is_ascii_whitespace() || bytes[at] == b'/') {
            at += 1;
        }
        if at >= end {
            return Scan::Missing(end);
        }
        if bytes[at] == b'>' {
            return Scan::Missing(at + 1);
        }
        let name_start = at;
        while at < end && !is_name_terminator(bytes[at]) {
            at += 1;
        }
        let name = &html[name_start..at];
        while at < end && bytes[at].is_ascii_whitespace() {
            at += 1;
        }
        if at >= end || bytes[at] != b'=' {
            continue;
        }
        at += 1;
        while at < end && bytes[at].is_ascii_whitespace() {
            at += 1;
        }
        let (value, next) = read_value(html, at);
        at = next;
        if name.eq_ignore_ascii_case("src") {
            let decoded = decode_entities(value);
            let source = decoded.trim();
            if !source.is_empty() {
                return Scan::Found(source.to_owned());
            }
        }
    }
}

fn is_name_terminator(byte: u8) -> bool {
    byte.is_ascii_whitespace() || matches!(byte, b'=' | b'>' | b'/')
}

fn read_value(html: &str, at: usize) -> (&str, usize) {
    let bytes = html.as_bytes();
    match bytes.get(at) {
        Some(&quote @ (b'"' | b'\'')) => {
            let start = at + 1;
            match html[start..].find(char::from(quote)) {
                Some(length) => (&html[start..start + length], start + length + 1),
                None => (&html[start..], bytes.len()),
            }
        }
        Some(_) => {
            let length = bytes[at..]
                .iter()
                .position(|&byte| byte.is_ascii_whitespace() || byte == b'>')
                .unwrap_or(bytes.len() - at);
            (&html[at..at + length], at + length)
        }
        None => ("", bytes.len()),
    }
}

fn decode_entities(text: &str) -> Cow<'_, str> {
    if !text.contains('&') {
        return Cow::Borrowed(text);
    }
    let mut decoded = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(index) = rest.find('&') {
        decoded.push_str(&rest[..index]);
        rest = &rest[index..];
        match decode_entity(rest) {
            Some((character, length)) => {
                decoded.push(character);
                rest = &rest[length..];
            }
            None => {
                decoded.push('&');
                rest = &rest[1..];
            }
        }
    }
    decoded.push_str(rest);
    Cow::Owned(decoded)
}

fn decode_entity(rest: &str) -> Option<(char, usize)> {
    const LONGEST_ENTITY: usize = 12;
    let semicolon = rest
        .bytes()
        .take(LONGEST_ENTITY)
        .position(|byte| byte == b';')?;
    let name = &rest[1..semicolon];
    let character = match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        _ => {
            let digits = name.strip_prefix('#')?;
            let code = match digits.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => digits.parse().ok()?,
            };
            char::from_u32(code)?
        }
    };
    Some((character, semicolon + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(html: &str) -> Option<String> {
        first_image_source(html)
    }

    #[test]
    fn finds_double_quoted_source() {
        assert_eq!(
            source(r#"<p><img alt="x" src="https://a.test/i.png"></p>"#).as_deref(),
            Some("https://a.test/i.png")
        );
    }

    #[test]
    fn finds_single_quoted_and_unquoted_sources() {
        assert_eq!(source("<img src='a.png'>").as_deref(), Some("a.png"));
        assert_eq!(source("<IMG SRC=b.png width=3>").as_deref(), Some("b.png"));
    }

    #[test]
    fn keeps_only_the_first_image() {
        assert_eq!(
            source(r#"<img src="one.png"><img src="two.png">"#).as_deref(),
            Some("one.png")
        );
    }

    #[test]
    fn skips_images_without_source() {
        assert_eq!(
            source(r#"<img alt="a"><img src="" ><img src="real.png">"#).as_deref(),
            Some("real.png")
        );
    }

    #[test]
    fn skips_comments_and_lookalike_tags() {
        assert_eq!(
            source(r#"<!-- <img src="no.png"> --><imgx src="no2.png"><img src="yes.png">"#)
                .as_deref(),
            Some("yes.png")
        );
    }

    #[test]
    fn decodes_entities_in_sources() {
        assert_eq!(
            source(r#"<img src="https://a.test/i?a=1&amp;b=2&#38;c=&#x33;">"#).as_deref(),
            Some("https://a.test/i?a=1&b=2&c=3")
        );
    }

    #[test]
    fn survives_hostile_markup() {
        assert_eq!(
            source("<img src=\"unterminated"),
            Some("unterminated".into())
        );
        assert_eq!(source("<img"), None);
        assert_eq!(source("<<<<img<img src"), None);
        assert_eq!(source("<!-- never closed <img src=x>"), None);
        assert_eq!(source("<img =\"x\" / = >"), None);
        assert_eq!(
            source("&&&&&&&&;;;;<img src=\"é&é;\">").as_deref(),
            Some("é&é;")
        );
    }

    #[test]
    fn stays_linear_on_large_input() {
        let html = "<img ".repeat(200_000);
        assert_eq!(source(&html), None);
        let html = "<".repeat(1_000_000);
        assert_eq!(source(&html), None);
    }
}
