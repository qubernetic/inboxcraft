// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Link targets, picture sources and the text/target comparison.
//!
//! Only `http`, `https` and `mailto` targets are kept, and only `http`, `https` and `cid` picture
//! sources. A target is read the way a browser would: surrounding white space trimmed, tab,
//! carriage return and line feed removed anywhere, the scheme compared in any letter case. Whatever
//! is still unusual after that (another control character, no host) is refused rather than guessed.

use crate::{ImageSource, LinkTarget, MAX_URL, Options, RemoteContent};

/// A link target from an `href` value (character references already decoded).
pub(crate) fn target(href: &str) -> LinkTarget {
    let Some(url) = clean(href) else { return LinkTarget::Refused };
    match scheme(&url).as_deref() {
        Some("http" | "https") => match web_host(&url) {
            Some(host) => LinkTarget::Web { url, host },
            None => LinkTarget::Refused,
        },
        Some("mailto") => {
            let rest = url.get("mailto:".len()..).unwrap_or("");
            let address = percent_decode(rest.split('?').next().unwrap_or(""));
            if address.trim().is_empty() || address.chars().any(char::is_control) {
                return LinkTarget::Refused;
            }
            LinkTarget::Mail { url, address }
        }
        _ => LinkTarget::Refused,
    }
}

/// A picture source from a `src` value (character references already decoded).
pub(crate) fn image_source(src: &str, options: &Options) -> ImageSource {
    let Some(url) = clean(src) else { return ImageSource::Unsupported };
    match scheme(&url).as_deref() {
        Some("cid") => {
            let id = percent_decode(url.get("cid:".len()..).unwrap_or(""));
            let id = id.trim().trim_start_matches('<').trim_end_matches('>').trim();
            if id.is_empty() || id.chars().any(char::is_control) {
                return ImageSource::Unsupported;
            }
            ImageSource::Inline(id.to_owned())
        }
        Some("http" | "https") if web_host(&url).is_some() => match options.remote_content {
            RemoteContent::Blocked => ImageSource::Blocked,
            RemoteContent::Allowed => ImageSource::Remote(url),
        },
        _ => ImageSource::Unsupported,
    }
}

/// Whether a link's visible text names another place than its target. A web target with a user
/// name (`https://bank.example@evil.example/`) is always a mismatch: a reader takes the part before
/// the `@` for the host, and a real link almost never carries one.
pub(crate) fn mismatch(text: &str, target: &LinkTarget) -> bool {
    if let LinkTarget::Web { url, .. } = target
        && has_user_info(url)
    {
        return true;
    }
    let text = comparable(text);
    let Some(named) = named_place(&text) else { return false };
    match (named, target) {
        (Place::Host(named), LinkTarget::Web { host, .. }) => !same_site(&named, host),
        (Place::Mail(named), LinkTarget::Mail { address, .. }) => named != address.trim().to_lowercase(),
        _ => true,
    }
}

/// Whether a character takes no room when shown: Unicode's default ignorable code points (format
/// characters such as the soft hyphen, the zero-width space and joiners, the bidi marks, the byte
/// order mark, fillers and variation selectors).
pub(crate) fn invisible(c: char) -> bool {
    matches!(
        c,
        '\u{ad}'
            | '\u{34f}'
            | '\u{61c}'
            | '\u{115f}'..='\u{1160}'
            | '\u{17b4}'..='\u{17b5}'
            | '\u{180b}'..='\u{180f}'
            | '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{206f}'
            | '\u{3164}'
            | '\u{fe00}'..='\u{fe0f}'
            | '\u{feff}'
            | '\u{ffa0}'
            | '\u{fff0}'..='\u{fff8}'
            | '\u{1bca0}'..='\u{1bca3}'
            | '\u{1d173}'..='\u{1d17a}'
            | '\u{e0000}'..='\u{e0fff}'
    )
}

/// The invisible characters shown text keeps, because emoji sequences, flags and some scripts need
/// them: the zero-width joiner and non-joiner, the variation selectors and the tag characters.
pub(crate) fn joins(c: char) -> bool {
    matches!(
        c,
        '\u{200c}' | '\u{200d}' | '\u{180b}'..='\u{180d}' | '\u{180f}' | '\u{fe00}'..='\u{fe0f}' | '\u{e0020}'..='\u{e007f}' | '\u{e0100}'..='\u{e01ef}'
    )
}

/// A link text as a reader takes it: invisible characters dropped, the ideographic, fullwidth and
/// halfwidth full stops read as dots (a browser's host parser does the same), trimmed, in lower case.
fn comparable(text: &str) -> String {
    let text: String =
        text.chars().filter(|c| !invisible(*c)).map(|c| if matches!(c, '\u{3002}' | '\u{ff0e}' | '\u{ff61}') { '.' } else { c }).collect();
    text.trim().to_lowercase()
}

/// A place a link text names.
enum Place {
    Host(String),
    Mail(String),
    /// A web address with a user name: it names one host and goes to another.
    Disguised,
}

/// The web host or mail address a link text names, if it looks like one at all.
fn named_place(text: &str) -> Option<Place> {
    if text.is_empty() || text.chars().any(char::is_whitespace) {
        return None;
    }
    if let Some(rest) = text.strip_prefix("mailto:") {
        return Some(Place::Mail(rest.split('?').next().unwrap_or("").to_owned()));
    }
    if text.starts_with("http://") || text.starts_with("https://") {
        if has_user_info(text) {
            return Some(Place::Disguised);
        }
        return web_host(text).map(Place::Host);
    }
    if let Some((local, domain)) = text.split_once('@') {
        return (!local.is_empty() && looks_like_domain(domain)).then(|| Place::Mail(text.to_owned()));
    }
    let host = text.split(['/', '?', '#']).next().unwrap_or("");
    let host = host.split(':').next().unwrap_or("");
    looks_like_domain(host).then(|| Place::Host(host.trim_end_matches('.').to_owned()))
}

/// Whether `s` reads as a domain name: dotted labels of letters, digits and hyphens, the last one
/// letters only and at least two of them.
fn looks_like_domain(s: &str) -> bool {
    let s = s.trim_end_matches('.');
    let mut labels = s.split('.');
    let Some(last) = labels.next_back() else { return false };
    let dotted = s.contains('.');
    let tld = last.len() >= 2 && last.chars().all(|c| c.is_alphabetic());
    dotted && tld && s.split('.').all(|l| !l.is_empty() && l.chars().all(|c| c.is_alphanumeric() || c == '-'))
}

/// Whether the host a text names and the target's host are the same site: equal, or the target a
/// subdomain of the named host, with a leading `www.` ignored on both.
fn same_site(named: &str, host: &str) -> bool {
    let named = named.strip_prefix("www.").unwrap_or(named);
    let host = host.strip_prefix("www.").unwrap_or(host);
    host == named || host.strip_suffix(named).is_some_and(|sub| sub.ends_with('.'))
}

/// A target as a browser would read it, or `None` when it is empty, too long or carries a control
/// character other than the tab, carriage return and line feed a browser drops.
fn clean(raw: &str) -> Option<String> {
    let trimmed = raw.trim_matches(|c: char| c.is_ascii_whitespace() || c.is_control());
    let mut out = String::with_capacity(trimmed.len().min(MAX_URL));
    for c in trimmed.chars() {
        match c {
            '\t' | '\r' | '\n' => {}
            ' ' => out.push_str("%20"),
            c if c.is_control() => return None,
            c => out.push(c),
        }
        if out.len() > MAX_URL {
            return None;
        }
    }
    (!out.is_empty()).then_some(out)
}

/// A target's scheme in lower case, if it has one.
fn scheme(url: &str) -> Option<String> {
    let (scheme, _) = url.split_once(':')?;
    let mut chars = scheme.chars();
    let first = chars.next()?;
    let valid = first.is_ascii_alphabetic() && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    valid.then(|| scheme.to_ascii_lowercase())
}

/// The authority of an `http` or `https` address: what follows `//` up to the path, query or
/// fragment. A browser ends it at a backslash too.
fn authority(url: &str) -> Option<&str> {
    let (_, rest) = url.split_once(':')?;
    let rest = rest.strip_prefix("//")?;
    rest.split(['/', '?', '#', '\\']).next()
}

/// Whether an `http` or `https` address carries a user name (an `@` in its authority).
fn has_user_info(url: &str) -> bool {
    authority(url).is_some_and(|a| a.contains('@'))
}

/// The host of an `http` or `https` address in lower case, without user name, port or trailing dot.
fn web_host(url: &str) -> Option<String> {
    let authority = authority(url)?;
    let host_port = authority.rsplit('@').next().unwrap_or("");
    let host =
        if let Some(v6) = host_port.strip_prefix('[') { v6.split(']').next().unwrap_or("") } else { host_port.split(':').next().unwrap_or("") };
    let host = host.trim_end_matches('.').to_lowercase();
    let valid = !host.is_empty() && host.chars().all(|c| c.is_alphanumeric() || matches!(c, '-' | '.' | '_' | ':'));
    valid.then_some(host)
}

/// `%XX` escapes decoded; a bad escape stays as written, bytes that are not UTF-8 become U+FFFD.
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while let Some(&c) = b.get(i) {
        let hex = |at: usize| b.get(at).and_then(|h| char::from(*h).to_digit(16));
        match (c, hex(i.saturating_add(1)), hex(i.saturating_add(2))) {
            (b'%', Some(hi), Some(lo)) => {
                out.push(u8::try_from(hi.saturating_mul(16).saturating_add(lo)).unwrap_or(b'?'));
                i = i.saturating_add(3);
            }
            _ => {
                out.push(c);
                i = i.saturating_add(1);
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
#[path = "link_tests.rs"]
mod tests;
