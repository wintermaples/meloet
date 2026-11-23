//! A small, strict XML reader that keeps where everything is in the text, so that an edit
//! changes only the bytes it means and leaves the rest of the document as it was. Enough for
//! Surge XT's patches: elements, attributes, text, comments, CDATA and processing
//! instructions; no DOCTYPE (and so no entities but the five of XML and character
//! references).

use std::borrow::Cow;
use std::ops::Range;

/// Elements inside one another, at most.
const MAX_DEPTH: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attr {
    pub name: Range<usize>,
    /// The value as written, without its quotes.
    pub value: Range<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Element {
    pub name: Range<usize>,
    /// From `<` to the `>` of the start tag (`/>` for an empty one).
    pub start: Range<usize>,
    pub attrs: Vec<Attr>,
    pub children: Vec<Element>,
    /// The end tag; `None` for `<name/>`.
    pub end: Option<Range<usize>>,
}

impl Element {
    /// The whole element, from its start tag to its end tag.
    pub fn span(&self) -> Range<usize> {
        self.start.start..self.end.as_ref().map_or(self.start.end, |e| e.end)
    }

    pub fn name<'t>(&self, text: &'t str) -> &'t str {
        &text[self.name.clone()]
    }

    pub fn attr(&self, text: &str, name: &str) -> Option<&Attr> {
        self.attrs.iter().find(|a| &text[a.name.clone()] == name)
    }

    /// The value of an attribute, its references replaced.
    pub fn value<'t>(&self, text: &'t str, name: &str) -> Option<Cow<'t, str>> {
        let attr = self.attr(text, name)?;
        Some(unescape(&text[attr.value.clone()]))
    }

    pub fn child<'e>(&'e self, text: &str, name: &str) -> Option<&'e Element> {
        self.children.iter().find(|c| c.name(text) == name)
    }

    /// Where a new attribute goes: right after the last one, or after the name.
    pub fn attr_place(&self) -> usize {
        self.attrs.last().map_or(self.name.end, |a| a.value.end + 1)
    }
}

/// Parses a whole document; `Err` says what is wrong and where (a byte offset).
pub fn parse(text: &str) -> Result<Element, String> {
    let mut p = Parser {
        text,
        b: text.as_bytes(),
        at: 0,
    };
    p.misc(true)?;
    if p.at >= p.b.len() || p.b[p.at] != b'<' {
        return Err(p.error("no root element"));
    }
    let root = p.element(0)?;
    p.misc(false)?;
    if p.at != p.b.len() {
        return Err(p.error("more after the root element"));
    }
    Ok(root)
}

struct Parser<'t> {
    text: &'t str,
    b: &'t [u8],
    at: usize,
}

impl Parser<'_> {
    fn error(&self, what: &str) -> String {
        format!("{what} (at byte {})", self.at)
    }

    fn starts(&self, s: &str) -> bool {
        self.b[self.at..].starts_with(s.as_bytes())
    }

    fn spaces(&mut self) -> usize {
        let from = self.at;
        while self.at < self.b.len() && matches!(self.b[self.at], b' ' | b'\t' | b'\r' | b'\n') {
            self.at += 1;
        }
        self.at - from
    }

    /// Past `until`, or an error.
    fn skip_past(&mut self, until: &str, what: &str) -> Result<Range<usize>, String> {
        match self.text[self.at..].find(until) {
            Some(i) => {
                let inside = self.at..self.at + i;
                self.at += i + until.len();
                Ok(inside)
            }
            None => Err(self.error(&format!("{what} does not end"))),
        }
    }

    /// Spaces, comments and processing instructions outside the root (and the XML
    /// declaration before it).
    fn misc(&mut self, before: bool) -> Result<(), String> {
        loop {
            self.spaces();
            if self.starts("<?") {
                self.at += 2;
                let target_at = self.at;
                let inside = self.skip_past("?>", "a processing instruction")?;
                let target = self.text[inside].split_ascii_whitespace().next();
                if target.is_some_and(|t| t.eq_ignore_ascii_case("xml"))
                    && (!before || target_at != 2)
                {
                    return Err(self.error("an XML declaration that is not at the start"));
                }
            } else if self.starts("<!--") {
                self.comment()?;
            } else if self.starts("<!") {
                return Err(self.error("a DOCTYPE or other declaration, which is not taken"));
            } else {
                return Ok(());
            }
        }
    }

    fn comment(&mut self) -> Result<(), String> {
        self.at += 4;
        let inside = self.skip_past("-->", "a comment")?;
        if self.text[inside].contains("--") {
            return Err(self.error("\"--\" inside a comment"));
        }
        Ok(())
    }

    fn name(&mut self) -> Result<Range<usize>, String> {
        let from = self.at;
        let rest = &self.text[self.at..];
        for (i, c) in rest.char_indices() {
            let ok = match i {
                0 => c.is_alphabetic() || c == '_' || c == ':' || !c.is_ascii(),
                _ => {
                    c.is_alphanumeric()
                        || matches!(c, '_' | ':' | '-' | '.')
                        || (!c.is_ascii() && !c.is_whitespace())
                }
            };
            if !ok {
                break;
            }
            self.at = from + i + c.len_utf8();
        }
        if self.at == from {
            return Err(self.error("a name is missing"));
        }
        Ok(from..self.at)
    }

    fn element(&mut self, depth: usize) -> Result<Element, String> {
        if depth >= MAX_DEPTH {
            return Err(self.error("elements are nested too deep"));
        }
        let start_at = self.at;
        self.at += 1;
        let name = self.name()?;
        let mut attrs: Vec<Attr> = Vec::new();
        loop {
            let spaced = self.spaces() > 0;
            if self.at >= self.b.len() {
                return Err(self.error("a start tag does not end"));
            }
            match self.b[self.at] {
                b'>' => {
                    self.at += 1;
                    break;
                }
                b'/' if self.starts("/>") => {
                    self.at += 2;
                    return Ok(Element {
                        name,
                        start: start_at..self.at,
                        attrs,
                        children: Vec::new(),
                        end: None,
                    });
                }
                _ if !spaced => return Err(self.error("no space before an attribute")),
                _ => {}
            }
            let attr_name = self.name()?;
            self.spaces();
            if self.at >= self.b.len() || self.b[self.at] != b'=' {
                return Err(self.error("an attribute without a value"));
            }
            self.at += 1;
            self.spaces();
            let quote = match self.b.get(self.at) {
                Some(q @ (b'"' | b'\'')) => *q,
                _ => return Err(self.error("an attribute value without quotes")),
            };
            self.at += 1;
            let from = self.at;
            let Some(len) = self.b[from..].iter().position(|c| *c == quote) else {
                return Err(self.error("an attribute value does not end"));
            };
            let value = from..from + len;
            let raw = &self.text[value.clone()];
            if raw.contains('<') {
                return Err(self.error("\"<\" inside an attribute value"));
            }
            check_references(raw).map_err(|e| self.error(&e))?;
            let new = &self.text[attr_name.clone()];
            if attrs.iter().any(|a| &self.text[a.name.clone()] == new) {
                return Err(self.error(&format!("the attribute {new} twice")));
            }
            self.at = value.end + 1;
            attrs.push(Attr {
                name: attr_name,
                value,
            });
        }
        let start = start_at..self.at;
        let mut children = Vec::new();
        loop {
            if self.at >= self.b.len() {
                let open = &self.text[name.clone()];
                return Err(self.error(&format!("<{open}> is not closed")));
            }
            if self.starts("</") {
                let end_at = self.at;
                self.at += 2;
                let closing = self.name()?;
                self.spaces();
                if self.b.get(self.at) != Some(&b'>') {
                    return Err(self.error("an end tag does not end"));
                }
                self.at += 1;
                if self.text[closing] != self.text[name.clone()] {
                    let open = &self.text[name.clone()];
                    return Err(self.error(&format!("<{open}> is closed by another end tag")));
                }
                return Ok(Element {
                    name,
                    start,
                    attrs,
                    children,
                    end: Some(end_at..self.at),
                });
            }
            if self.starts("<!--") {
                self.comment()?;
            } else if self.starts("<![CDATA[") {
                self.at += 9;
                self.skip_past("]]>", "a CDATA section")?;
            } else if self.starts("<?") {
                self.at += 2;
                self.skip_past("?>", "a processing instruction")?;
            } else if self.starts("<!") {
                return Err(self.error("a declaration inside an element"));
            } else if self.b[self.at] == b'<' {
                children.push(self.element(depth + 1)?);
            } else {
                let from = self.at;
                let len = self.b[from..].iter().position(|c| *c == b'<');
                self.at = len.map_or(self.b.len(), |len| from + len);
                let text = &self.text[from..self.at];
                if text.contains("]]>") {
                    return Err(self.error("\"]]>\" in text"));
                }
                check_references(text).map_err(|e| self.error(&e))?;
            }
        }
    }
}

/// Every `&` starts one of the five entities or a character reference.
fn check_references(raw: &str) -> Result<(), String> {
    let mut rest = raw;
    while let Some(at) = rest.find('&') {
        rest = &rest[at + 1..];
        let Some(end) = rest.find(';') else {
            return Err("a \"&\" that is no reference".into());
        };
        if reference(&rest[..end]).is_none() {
            return Err(format!("the unknown reference &{};", &rest[..end]));
        }
        rest = &rest[end + 1..];
    }
    Ok(())
}

fn reference(name: &str) -> Option<char> {
    match name {
        "lt" => Some('<'),
        "gt" => Some('>'),
        "amp" => Some('&'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        _ => {
            let code = match name.strip_prefix("#x") {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => name.strip_prefix('#')?.parse().ok()?,
            };
            char::from_u32(code).filter(|c| *c != '\0')
        }
    }
}

/// An attribute's value or a text with its references replaced (both checked by `parse`).
pub fn unescape(raw: &str) -> Cow<'_, str> {
    if !raw.contains('&') {
        return Cow::Borrowed(raw);
    }
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at + 1..];
        let end = rest.find(';').unwrap_or(rest.len());
        out.extend(reference(&rest[..end]));
        rest = rest.get(end + 1..).unwrap_or("");
    }
    out.push_str(rest);
    Cow::Owned(out)
}

/// A value to write between double quotes.
pub fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\n' => out.push_str("&#10;"),
            '\r' => out.push_str("&#13;"),
            '\t' => out.push_str("&#9;"),
            c => out.push(c),
        }
    }
    out
}

/// Replaces spans of `text`; the spans must not overlap. An empty span inserts.
pub fn apply(text: &str, mut edits: Vec<(Range<usize>, String)>) -> String {
    edits.sort_by_key(|(span, _)| (span.start, span.end));
    let mut out = String::with_capacity(text.len() + 256);
    let mut at = 0;
    for (span, new) in edits {
        debug_assert!(span.start >= at, "edits overlap");
        out.push_str(&text[at..span.start]);
        out.push_str(&new);
        at = span.end;
    }
    out.push_str(&text[at..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_reads_elements_attributes_and_where_they_are() {
        let text = "<?xml version=\"1.0\" ?><patch revision=\"24\"><a x='1' y=\"&lt;2&gt;\"/>\
                    <!-- c --><b>t&amp;t<![CDATA[<raw>]]><c/></b></patch>";
        let root = parse(text).unwrap();
        assert_eq!(root.name(text), "patch");
        assert_eq!(root.value(text, "revision").as_deref(), Some("24"));
        assert_eq!(root.children.len(), 2);
        let a = &root.children[0];
        assert_eq!(&text[a.span()], "<a x='1' y=\"&lt;2&gt;\"/>");
        assert_eq!(a.value(text, "y").as_deref(), Some("<2>"));
        assert_eq!(a.end, None);
        let b = root.child(text, "b").unwrap();
        assert_eq!(b.children[0].name(text), "c");
        assert_eq!(&text[b.end.clone().unwrap()], "</b>");
        assert_eq!(root.span(), 22..text.len());
        assert_eq!(&text[a.attr_place()..a.attr_place() + 2], "/>");
    }

    #[test]
    fn it_refuses_what_is_not_well_formed() {
        for bad in [
            "",
            "text",
            "<a>",
            "<a></b>",
            "<a x=1/>",
            "<a x='1' x='2'/>",
            "<a x='<'/>",
            "<a>&nope;</a>",
            "<a>&amp</a>",
            "<a/><b/>",
            "<!DOCTYPE a [<!ENTITY e 'x'>]><a>&e;</a>",
            "<a><!-- -- --></a>",
            "<a x='1'y='2'/>",
            "<a>&#0;</a>",
            "<a/><?xml version='1.0'?>",
            "<a>]]></a>",
        ] {
            assert!(parse(bad).is_err(), "{bad:?}");
        }
        let deep = "<a>".repeat(MAX_DEPTH + 1) + &"</a>".repeat(MAX_DEPTH + 1);
        assert!(parse(&deep).is_err());
        assert!(parse(" <a>&#x41;&#66;</a>\n").is_ok());
    }

    #[test]
    fn edits_change_only_their_spans() {
        let text = "<p><a v=\"1\" /><b/></p>";
        let root = parse(text).unwrap();
        let a = &root.children[0];
        let v = a.attr(text, "v").unwrap().value.clone();
        let place = root.children[1].attr_place();
        let out = apply(
            text,
            vec![(place..place, " w=\"x\"".into()), (v, "2".into())],
        );
        assert_eq!(out, "<p><a v=\"2\" /><b w=\"x\"/></p>");
        assert_eq!(escape("a\"<&>\n"), "a&quot;&lt;&amp;&gt;&#10;");
        assert_eq!(unescape("a&quot;&#x41;&amp;"), "a\"A&");
    }
}
