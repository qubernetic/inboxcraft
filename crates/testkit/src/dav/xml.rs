// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The fake's own reader of request bodies: XML with namespaces, small and strict, independent of `inboxcraft-dav`.
//! It returns `None` for anything it does not read (a document type declaration among them) and caps nesting and
//! the number of elements.

/// How deep elements may nest in a request body.
const MAX_DEPTH: usize = 32;
/// How many elements one request body may hold.
const MAX_ELEMENTS: usize = 10_000;

/// An element of a request body.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Element {
    pub(crate) ns: String,
    pub(crate) name: String,
    pub(crate) attrs: Vec<(String, String)>,
    pub(crate) children: Vec<Element>,
    pub(crate) text: String,
}

impl Element {
    pub(crate) fn is(&self, ns: &str, name: &str) -> bool {
        self.ns == ns && self.name == name
    }

    pub(crate) fn child(&self, ns: &str, name: &str) -> Option<&Self> {
        self.children.iter().find(|child| child.is(ns, name))
    }
}

/// An element whose end tag has not come yet: the element, its name as written, the prefixes it binds.
type Open = (Element, String, Vec<(String, String)>);

/// Reads a request body.
pub(crate) fn parse(bytes: &[u8]) -> Option<Element> {
    let text = std::str::from_utf8(bytes).ok()?;
    let mut rest = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut stack: Vec<Open> = Vec::new();
    let mut root = None;
    let mut elements = 0_usize;
    loop {
        let at = rest.find('<').unwrap_or(rest.len());
        let (before, after) = rest.split_at(at);
        add_text(&mut stack, &unescape(before)?)?;
        rest = after;
        if rest.is_empty() {
            break;
        }
        if let Some(comment) = rest.strip_prefix("<!--") {
            rest = comment.split_once("-->")?.1;
        } else if let Some(pi) = rest.strip_prefix("<?") {
            rest = pi.split_once("?>")?.1;
        } else if let Some(cdata) = rest.strip_prefix("<![CDATA[") {
            let (inner, next) = cdata.split_once("]]>")?;
            add_text(&mut stack, inner)?;
            rest = next;
        } else if rest.starts_with("<!") {
            return None;
        } else if let Some(end) = rest.strip_prefix("</") {
            let (name, next) = end.split_once('>')?;
            let (element, written, _) = stack.pop()?;
            if written != name.trim_end() {
                return None;
            }
            attach(&mut stack, &mut root, element);
            rest = next;
        } else {
            let close = tag_end(rest)?;
            let inner = rest.get(1..close)?;
            rest = rest.get(close.saturating_add(1)..)?;
            elements = elements.saturating_add(1);
            if elements > MAX_ELEMENTS || stack.len() >= MAX_DEPTH || root.is_some() {
                return None;
            }
            let (inner, closed) = inner.strip_suffix('/').map_or((inner, false), |inner| (inner, true));
            let open = start(inner, &stack)?;
            if closed {
                attach(&mut stack, &mut root, open.0);
            } else {
                stack.push(open);
            }
        }
    }
    if stack.is_empty() { root } else { None }
}

/// Where the tag that starts `rest` ends: its `>` outside quotes.
fn tag_end(rest: &str) -> Option<usize> {
    let mut quote = None;
    for (at, c) in rest.char_indices() {
        match (quote, c) {
            (None, '>') => return Some(at),
            (None, '"' | '\'') => quote = Some(c),
            (Some(open), _) if open == c => quote = None,
            _ => {}
        }
    }
    None
}

fn add_text(stack: &mut [Open], text: &str) -> Option<()> {
    match stack.last_mut() {
        Some((element, _, _)) => element.text.push_str(text),
        None if text.trim().is_empty() => {}
        None => return None,
    }
    Some(())
}

/// Adds `element` to its parent, or makes it the root: a second root never gets here, its start tag is refused.
fn attach(stack: &mut [Open], root: &mut Option<Element>, element: Element) {
    match stack.last_mut() {
        Some((parent, _, _)) => parent.children.push(element),
        None => *root = Some(element),
    }
}

/// Reads a start tag's name and attributes and binds its prefixes.
fn start(inner: &str, stack: &[Open]) -> Option<Open> {
    let inner = inner.trim_end();
    let name_end = inner.find(char::is_whitespace).unwrap_or(inner.len());
    let (written, mut rest) = inner.split_at(name_end);
    if !good_name(written) {
        return None;
    }
    let mut attrs = Vec::new();
    let mut bound = Vec::new();
    loop {
        rest = rest.trim_start();
        if rest.is_empty() {
            break;
        }
        let (name, after) = rest.split_once('=')?;
        let name = name.trim();
        let after = after.trim_start();
        let quote = after.chars().next().filter(|c| matches!(c, '"' | '\''))?;
        let (raw, next) = after.get(1..)?.split_once(quote)?;
        if !good_name(name) || raw.contains('<') {
            return None;
        }
        let value = unescape(raw)?;
        match name.strip_prefix("xmlns") {
            Some("") => bound.push((String::new(), value)),
            Some(prefixed) => bound.push((prefixed.strip_prefix(':')?.to_owned(), value)),
            None => attrs.push((name.to_owned(), value)),
        }
        rest = next;
    }
    let (prefix, name) = written.split_once(':').unwrap_or(("", written));
    let ns = lookup(prefix, &bound, stack)?;
    Some((Element { ns, name: name.to_owned(), attrs, ..Element::default() }, written.to_owned(), bound))
}

fn lookup(prefix: &str, bound: &[(String, String)], stack: &[Open]) -> Option<String> {
    let found = bound.iter().rev().chain(stack.iter().rev().flat_map(|(_, _, scope)| scope.iter().rev())).find(|(name, _)| name == prefix);
    match found {
        Some((_, uri)) => Some(uri.clone()),
        None if prefix.is_empty() => Some(String::new()),
        None if prefix == "xml" => Some("http://www.w3.org/XML/1998/namespace".to_owned()),
        None => None,
    }
}

fn good_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 256
        && name.split(':').count() <= 2
        && name.split(':').all(|part| part.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_'))
        && name.chars().all(|c| c.is_alphanumeric() || matches!(c, ':' | '-' | '.' | '_'))
}

/// Reads the predefined entities and character references; `None` for anything else after a `&`.
fn unescape(raw: &str) -> Option<String> {
    let mut out = String::with_capacity(raw.len());
    let mut parts = raw.split('&');
    out.push_str(parts.next().unwrap_or_default());
    for part in parts {
        let (name, after) = part.split_once(';')?;
        let c = match name {
            "lt" => '<',
            "gt" => '>',
            "amp" => '&',
            "quot" => '"',
            "apos" => '\'',
            _ => {
                let number = name.strip_prefix("#x").map_or_else(|| name.strip_prefix('#')?.parse().ok(), |hex| u32::from_str_radix(hex, 16).ok())?;
                char::from_u32(number).filter(|&c| c != '\0')?
            }
        };
        out.push(c);
        out.push_str(after);
    }
    Some(out)
}
