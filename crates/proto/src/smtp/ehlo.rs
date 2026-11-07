// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What the server can do: the service extensions its EHLO reply lists (RFC 5321 section 4.1.1.1).

/// Most extensions kept from one EHLO reply, and most parameters kept of one.
pub const MAX_EXTENSIONS: usize = 64;

/// Longest keyword or parameter kept.
pub const MAX_KEYWORD_LEN: usize = 64;

/// The extensions a server listed after EHLO: keywords in upper case, each with its parameters.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Extensions(Vec<(String, Vec<String>)>);

impl Extensions {
    /// From the lines of an EHLO reply after the first (the greeting line). A line whose keyword is not letters,
    /// digits and hyphens (not first), or is too long, is skipped; a keyword listed again is ignored.
    pub(crate) fn from_lines<'a>(lines: impl IntoIterator<Item = &'a str>) -> Self {
        let mut extensions: Vec<(String, Vec<String>)> = Vec::new();
        for line in lines {
            let mut words = line.split_ascii_whitespace();
            let Some(keyword) = words.next() else { continue };
            // `AUTH=PLAIN LOGIN`, written by servers from before RFC 4954.
            let (keyword, first) = match keyword.split_once('=') {
                Some((auth, mechanism)) if auth.eq_ignore_ascii_case("AUTH") => (auth, Some(mechanism)),
                _ => (keyword, None),
            };
            if !is_keyword(keyword) || extensions.len() >= MAX_EXTENSIONS {
                continue;
            }
            let keyword = keyword.to_ascii_uppercase();
            if extensions.iter().any(|(known, _)| *known == keyword) {
                continue;
            }
            let parameters = first.into_iter().chain(words).filter(|word| word.len() <= MAX_KEYWORD_LEN).take(MAX_EXTENSIONS);
            extensions.push((keyword, parameters.map(str::to_owned).collect()));
        }
        Self(extensions)
    }

    /// Whether the server lists `keyword` (any case).
    #[must_use]
    pub fn has(&self, keyword: &str) -> bool {
        self.parameters(keyword).is_some()
    }

    /// The keywords, in upper case, in the server's order.
    pub fn keywords(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(|(keyword, _)| keyword.as_str())
    }

    /// The largest message the server takes (SIZE, RFC 1870), `None` when it names no fixed limit.
    #[must_use]
    pub fn size_limit(&self) -> Option<u64> {
        self.parameters("SIZE")?.first()?.parse().ok().filter(|limit| *limit > 0)
    }

    /// Whether the server offers the SASL mechanism `mechanism` (any case) with AUTH (RFC 4954).
    #[must_use]
    pub fn offers_auth(&self, mechanism: &str) -> bool {
        self.parameters("AUTH").is_some_and(|mechanisms| mechanisms.iter().any(|m| m.eq_ignore_ascii_case(mechanism)))
    }

    fn parameters(&self, keyword: &str) -> Option<&[String]> {
        self.0.iter().find(|(known, _)| known.eq_ignore_ascii_case(keyword)).map(|(_, parameters)| parameters.as_slice())
    }
}

/// `ehlo-keyword = (ALPHA / DIGIT) *(ALPHA / DIGIT / "-")`, capped.
fn is_keyword(word: &str) -> bool {
    word.len() <= MAX_KEYWORD_LEN
        && word.bytes().next().is_some_and(|b| b.is_ascii_alphanumeric())
        && word.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

#[cfg(test)]
#[path = "ehlo_tests.rs"]
mod tests;
