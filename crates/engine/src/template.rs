// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Filling in a template (`compose.template`).
//!
//! A template's subject and text may hold placeholders: `{name}` and `{first_name}` (the first `To` recipient's
//! name and the first word of it) and `{my_name}` (the account's display name). `{?prompt}` is a blank the owner
//! fills in. A filled-in template marks each blank, and each placeholder whose name is not known, as `[[prompt]]`
//! (`[[name]]`, `[[first name]]`, `[[my name]]`); the draft remembers the marks, and while one of them is still in its
//! subject or text it cannot be sent. A mark the draft or a filled-in name holds already (a quoted message may) is
//! made unique with a number (`[[day 2]]`), so the sender's words do not keep Send blocked. Anything else in braces
//! stays as it is.

use inboxcraft_model::{Account, Draft, Template};

use crate::draft;

/// The longest prompt of a blank, in characters; a longer one is cut.
const MAX_PROMPT_CHARS: usize = 60;

/// The most numbers tried to make a mark unique.
const MAX_TRIES: u32 = 100;

/// The longest placeholder looked for between braces, in bytes.
const MAX_PLACEHOLDER_BYTES: usize = 2 + MAX_PROMPT_CHARS * 4;

/// `template` put into `draft` of `account`: its text in front of the draft's text, its subject where the draft has
/// none, and the marks of its blanks remembered.
pub(crate) fn apply(template: &Template, draft: &mut Draft, account: &Account) {
    let recipient = draft.to.iter().find_map(|text| draft::recipient(text).ok().and_then(|(name, _)| name.map(str::to_owned)));
    let me = account.display_name().map(str::to_owned);
    // The names filled in are taken too: a mark found in one would keep Send blocked once the blank is filled.
    let taken = format!("{}\n{}\n{}\n{}", draft.subject, draft.text, recipient.as_deref().unwrap_or_default(), me.as_deref().unwrap_or_default());
    let mut fill = Fill { recipient, me, taken, marks: Vec::new() };
    let subject = fill.text(template.subject());
    let text = fill.text(template.text());
    if draft.subject.trim().is_empty() {
        draft.subject = subject;
    }
    draft.text = format!("{text}{}", draft.text);
    for mark in fill.marks.into_iter().map(|(_, mark)| mark) {
        if !draft.blanks.contains(&mark) {
            draft.blanks.push(mark);
        }
    }
}

/// The marks of `draft`'s blanks still in its subject or text.
pub(crate) fn open_blanks(draft: &Draft) -> Vec<&str> {
    draft.blanks.iter().filter(|mark| draft.subject.contains(mark.as_str()) || draft.text.contains(mark.as_str())).map(String::as_str).collect()
}

/// What a template is filled with, and the marks it gave so far.
struct Fill {
    recipient: Option<String>,
    me: Option<String>,
    /// The draft's own words and the names filled in, which a new mark must not be found in.
    taken: String,
    /// Each prompt and its mark.
    marks: Vec<(String, String)>,
}

impl Fill {
    /// `text` with its placeholders filled in and its blanks marked.
    fn text(&mut self, text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut rest = text;
        while let Some(open) = rest.find('{') {
            out.push_str(rest.get(..open).unwrap_or_default());
            let after = rest.get(open.saturating_add(1)..).unwrap_or_default();
            // A placeholder ends at the first `}`, on the same line, without another `{` before it.
            let close =
                after.find(['}', '{', '\n']).filter(|at| *at <= MAX_PLACEHOLDER_BYTES && after.get(*at..).is_some_and(|end| end.starts_with('}')));
            let filled = close.and_then(|close| Some((close, self.value(after.get(..close)?)?)));
            match filled {
                Some((close, value)) => {
                    out.push_str(&value);
                    rest = after.get(close.saturating_add(1)..).unwrap_or_default();
                }
                None => {
                    out.push('{');
                    rest = after;
                }
            }
        }
        out.push_str(rest);
        out
    }

    /// What the placeholder `inner` (between the braces) becomes; `None` for one that is no placeholder.
    fn value(&mut self, inner: &str) -> Option<String> {
        let first = |name: &str| name.split_whitespace().next().map(str::to_owned);
        let known = match inner {
            "name" => self.recipient.clone(),
            "first_name" => self.recipient.as_deref().and_then(first),
            "my_name" => self.me.clone(),
            _ => return Some(self.mark(inner.strip_prefix('?')?)),
        };
        Some(known.unwrap_or_else(|| self.mark(&inner.replace('_', " "))))
    }

    /// The mark of the blank `prompt`: the same for the same prompt, never one the draft holds already.
    fn mark(&mut self, prompt: &str) -> String {
        let prompt: String = prompt.chars().filter(|c| !c.is_control() && !matches!(c, '[' | ']')).take(MAX_PROMPT_CHARS).collect();
        let prompt = match prompt.trim() {
            "" => "blank".to_owned(),
            trimmed => trimmed.to_owned(),
        };
        if let Some((_, mark)) = self.marks.iter().find(|(held, _)| *held == prompt) {
            return mark.clone();
        }
        let mut mark = format!("[[{prompt}]]");
        // After so many tries the mark may be in the draft: Send stays blocked until the owner takes it out.
        let mut n = 1_u32;
        while n < MAX_TRIES && self.taken.contains(&mark) {
            n = n.saturating_add(1);
            mark = format!("[[{prompt} {n}]]");
        }
        self.marks.push((prompt, mark.clone()));
        mark
    }
}

#[cfg(test)]
#[path = "template_tests.rs"]
mod tests;
