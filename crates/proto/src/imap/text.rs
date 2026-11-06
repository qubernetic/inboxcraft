// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-FileCopyrightText: MailCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Server text made fit to show, and the secrets it must not show.
//!
//! Adapted from UCS-Care-Group/mailcraft `crates/proto/src/lib.rs` (`tidy`, `Secrets`; see `PROVENANCE.md`).

use inboxcraft_model::is_invisible_mark;

/// Most characters kept of a text the server wrote for people.
pub const MAX_TEXT_CHARS: usize = 300;

/// Server text made fit for an error or a refusal: one line, no control characters, no invisible direction or
/// zero-width marks ([`is_invisible_mark`]), at most [`MAX_TEXT_CHARS`] characters and a `...` when it was longer.
pub(crate) fn tidy(text: &str) -> String {
    one_line(text, MAX_TEXT_CHARS, "...")
}

/// `text` as one line as [`tidy`] makes it, of at most `max` characters, and `cut` after it when it was longer.
pub(crate) fn one_line(text: &str, max: usize, cut: &str) -> String {
    let mut out = String::new();
    let mut space = false;
    for (n, c) in text.trim().chars().enumerate() {
        if n >= max {
            out.push_str(cut);
            break;
        }
        if is_invisible_mark(c) {
            continue;
        }
        if c.is_control() || c.is_whitespace() {
            space = !out.is_empty();
        } else {
            if space {
                out.push(' ');
                space = false;
            }
            out.push(c);
        }
    }
    out
}

/// What must never be shown: the password and every form it was sent in.
#[derive(Clone, Default)]
pub(crate) struct Secrets(Vec<String>);

impl Secrets {
    /// Adds `secret`, and `secret` without its edge white space: a server line is trimmed before it is searched, so
    /// a password written back first or last has lost it.
    pub(crate) fn add(&mut self, secret: &str) {
        for secret in [secret, secret.trim()] {
            if !secret.is_empty() && !self.0.iter().any(|s| s == secret) {
                self.0.push(secret.to_owned());
            }
        }
        // Longest first, so a secret inside another one cannot leave a piece behind.
        self.0.sort_by_key(|s| std::cmp::Reverse(s.len()));
    }

    /// Whether `text` holds a secret, in any case of ASCII letters: for words the server upper-cases.
    pub(crate) fn shows(&self, text: &str) -> bool {
        let text = text.to_ascii_uppercase();
        self.0.iter().any(|secret| text.contains(&secret.to_ascii_uppercase()))
    }

    /// `text` with every secret replaced by `***`, then tidied. Hidden before it is shortened: a secret cut in half
    /// would no longer be recognised. A secret's edge white space stays, so that `***` does not run into a word.
    pub(crate) fn tidy(&self, text: &str) -> String {
        let mut out = text.to_owned();
        for secret in &self.0 {
            if out.contains(secret.as_str()) {
                let before = if secret.starts_with(char::is_whitespace) { " " } else { "" };
                let after = if secret.ends_with(char::is_whitespace) { " " } else { "" };
                out = out.replace(secret.as_str(), &format!("{before}***{after}"));
            }
        }
        tidy(&out)
    }
}

/// Neither the secrets nor how many there are.
impl std::fmt::Debug for Secrets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secrets(..)")
    }
}

#[cfg(test)]
#[path = "text_tests.rs"]
mod tests;
