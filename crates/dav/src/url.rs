// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The server's origin and the paths on it (RFC 3986): parsing, resolving the server's references, and one written
//! form for each path, so that two spellings of a path compare equal.

use std::fmt;

use crate::DavError;

/// The longest URL or reference read, in bytes.
pub const MAX_URL: usize = 2048;

/// The longest host name, in bytes.
const MAX_HOST: usize = 253;

/// Where the server is: `https` or `http`, the host and the port. Every request of an exchange goes to one origin;
/// a reference to another is never followed on its own ([`DavError::OtherOrigin`]).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Origin {
    tls: bool,
    host: String,
    port: u16,
}

impl Origin {
    /// Splits an absolute `https` or `http` URL into its origin and its path; the host is lowercased, the path
    /// written in its one form ([`Origin::resolve`]) and a fragment dropped.
    ///
    /// # Errors
    ///
    /// [`DavError::BadUrl`] for another scheme, a user name, a bad host or port, a control character, or a URL
    /// longer than [`MAX_URL`].
    ///
    /// ```
    /// use inboxcraft_dav::Origin;
    ///
    /// let (origin, path) = Origin::parse("https://Dav.Example.org/calendars/ada/")?;
    /// assert_eq!(origin.to_string(), "https://dav.example.org");
    /// assert_eq!(path, "/calendars/ada/");
    /// # Ok::<(), inboxcraft_dav::DavError>(())
    /// ```
    pub fn parse(url: &str) -> Result<(Self, String), DavError> {
        check(url)?;
        let (scheme, rest) = url.split_once("://").ok_or(DavError::BadUrl("a scheme other than https or http"))?;
        let tls = match scheme.to_ascii_lowercase().as_str() {
            "https" => true,
            "http" => false,
            _ => return Err(DavError::BadUrl("a scheme other than https or http")),
        };
        let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
        let (authority, path) = rest.split_at(end);
        if authority.contains('@') {
            return Err(DavError::BadUrl("a user name in the URL"));
        }
        let (host, port) = split_port(authority, tls)?;
        Ok((Self { tls, host, port }, normalize(path)))
    }

    /// Whether requests to this origin are encrypted (`https`).
    #[must_use]
    pub fn is_encrypted(&self) -> bool {
        self.tls
    }

    /// The host, lowercased; an IPv6 address in brackets.
    #[must_use]
    pub fn host(&self) -> &str {
        &self.host
    }

    /// The port.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.port
    }

    /// The `Host` header's value: the host, and the port when it is not the scheme's own.
    #[must_use]
    pub fn authority(&self) -> String {
        let default = if self.tls { 443 } else { 80 };
        if self.port == default { self.host.clone() } else { format!("{}:{}", self.host, self.port) }
    }

    /// The absolute URL of `path` on this origin.
    #[must_use]
    pub fn url(&self, path: &str) -> String {
        format!("{self}{path}")
    }

    /// Resolves the `reference` the server sent (an `href`, a `Location`) against `base`, a path on this origin, and
    /// writes it in its one form: bytes a path may not carry are percent-encoded, escapes of the characters it may
    /// carry (other than `/`, `?` and `%`) are decoded, the other escapes uppercased, and dot segments removed.
    ///
    /// # Errors
    ///
    /// [`DavError::OtherOrigin`] when the reference points to another origin, and [`DavError::BadUrl`] when it
    /// cannot be used at all.
    pub fn resolve(&self, base: &str, reference: &str) -> Result<String, DavError> {
        check(reference)?;
        let reference = reference.split_once('#').map_or(reference, |(before, _)| before);
        if has_scheme(reference) {
            return self.same_origin(reference);
        }
        if reference.starts_with("//") {
            let scheme = if self.tls { "https:" } else { "http:" };
            return self.same_origin(&format!("{scheme}{reference}"));
        }
        let base_path = base.split_once('?').map_or(base, |(path, _)| path);
        Ok(if reference.starts_with('/') {
            normalize(reference)
        } else if reference.is_empty() {
            normalize(base)
        } else if reference.starts_with('?') {
            normalize(&format!("{base_path}{reference}"))
        } else {
            let dir = base_path.rfind('/').and_then(|at| base_path.get(..=at)).unwrap_or("/");
            normalize(&format!("{dir}{reference}"))
        })
    }

    fn same_origin(&self, url: &str) -> Result<String, DavError> {
        let (origin, path) = Self::parse(url)?;
        if origin == *self { Ok(path) } else { Err(DavError::OtherOrigin { url: origin.url(&path) }) }
    }
}

impl fmt::Display for Origin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let scheme = if self.tls { "https" } else { "http" };
        write!(f, "{scheme}://{}", self.authority())
    }
}

fn check(url: &str) -> Result<(), DavError> {
    if url.len() > MAX_URL {
        return Err(DavError::BadUrl("a URL too long"));
    }
    if url.chars().any(char::is_control) {
        return Err(DavError::BadUrl("a control character"));
    }
    Ok(())
}

/// Whether `reference` starts with a scheme (RFC 3986 section 3.1) and a colon.
fn has_scheme(reference: &str) -> bool {
    let Some((scheme, _)) = reference.split_once(':') else { return false };
    let mut chars = scheme.chars();
    chars.next().is_some_and(|first| first.is_ascii_alphabetic()) && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

fn split_port(authority: &str, tls: bool) -> Result<(String, u16), DavError> {
    let default = if tls { 443 } else { 80 };
    let (host, port) = if let Some(v6) = authority.strip_prefix('[') {
        let (address, after) = v6.split_once(']').ok_or(DavError::BadUrl("a bad host"))?;
        if address.is_empty() || !address.chars().all(|c| c.is_ascii_hexdigit() || matches!(c, ':' | '.')) {
            return Err(DavError::BadUrl("a bad host"));
        }
        let port = if after.is_empty() { None } else { Some(after.strip_prefix(':').ok_or(DavError::BadUrl("a bad port"))?) };
        (format!("[{address}]"), port)
    } else {
        let (host, port) = authority.split_once(':').map_or((authority, None), |(host, port)| (host, Some(port)));
        let good = !host.is_empty() && host.len() <= MAX_HOST && host.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'));
        if !good {
            return Err(DavError::BadUrl("a bad host"));
        }
        (host.to_ascii_lowercase(), port)
    };
    let port = match port {
        None => default,
        Some(digits) => digits
            .parse::<u16>()
            .ok()
            .filter(|&port| port != 0 && digits.bytes().all(|byte| byte.is_ascii_digit()))
            .ok_or(DavError::BadUrl("a bad port"))?,
    };
    Ok((host, port))
}

/// Whether a path may carry `byte` as it is.
fn plain(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"-._~!$&'()*+,;=:@/?".contains(&byte)
}

/// Writes `path` in its one form, with a leading `/` and without a fragment.
fn normalize(path: &str) -> String {
    let path = path.split_once('#').map_or(path, |(before, _)| before);
    let (path, query) = path.split_once('?').map_or((path, None), |(path, query)| (path, Some(query)));
    let mut out = remove_dots(&encode(path));
    if !out.starts_with('/') {
        out.insert(0, '/');
    }
    if let Some(query) = query {
        out.push('?');
        out.push_str(&encode(query));
    }
    out
}

fn encode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    while let Some(&byte) = bytes.get(at) {
        let escape = bytes.get(at.saturating_add(1)..at.saturating_add(3)).and_then(hex_pair);
        match (byte, escape) {
            (b'%', Some(decoded)) => {
                if plain(decoded) && !matches!(decoded, b'/' | b'?') {
                    out.push(char::from(decoded));
                } else {
                    out.push_str(&format!("%{decoded:02X}"));
                }
                at = at.saturating_add(3);
                continue;
            }
            (b'%', None) => out.push_str("%25"),
            _ if plain(byte) => out.push(char::from(byte)),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
        at = at.saturating_add(1);
    }
    out
}

fn hex_pair(pair: &[u8]) -> Option<u8> {
    let text = std::str::from_utf8(pair).ok()?;
    if text.len() == 2 && text.bytes().all(|byte| byte.is_ascii_hexdigit()) { u8::from_str_radix(text, 16).ok() } else { None }
}

/// Removes the `.` and `..` segments (RFC 3986 section 5.2.4) of an encoded path.
fn remove_dots(path: &str) -> String {
    let mut kept: Vec<&str> = Vec::new();
    let segments: Vec<&str> = path.split('/').collect();
    let last = segments.len().saturating_sub(1);
    for (index, segment) in segments.iter().enumerate() {
        match *segment {
            "." | ".." => {
                if *segment == ".." && kept.len() > 1 {
                    kept.pop();
                }
                if index == last {
                    kept.push("");
                }
            }
            _ => kept.push(segment),
        }
    }
    let joined = kept.join("/");
    if joined.is_empty() { "/".to_owned() } else { joined }
}

#[cfg(test)]
#[path = "url_tests.rs"]
mod tests;
