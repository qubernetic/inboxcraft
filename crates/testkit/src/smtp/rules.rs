// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What happens at scripted points of a session: refusals with their enhanced codes, faults, and lines sent as they
//! are.

use crate::Fault;

/// A point in a session at which [`crate::smtp::FakeSmtp::on`] plays its actions.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum When {
    /// Before the greeting; a [`Action::Reply`] here is the greeting.
    Greeting,
    /// When the client has sent this command (`EHLO`, `AUTH`, `MAIL`, `RCPT`, `DATA`), before the server answers it.
    Command(String),
    /// When the client names this recipient in RCPT TO, in any case, before the server answers; after the rules of
    /// [`When::Command`] for `RCPT`.
    Rcpt(String),
    /// When the client has sent a whole message after DATA, before the server answers its final dot.
    Message,
    /// After the server's answer to this command.
    Answered(String),
}

impl When {
    /// [`When::Command`] for `verb`, in any case.
    #[must_use]
    pub fn command(verb: &str) -> Self {
        Self::Command(verb.to_ascii_uppercase())
    }

    /// [`When::Rcpt`] for `address`, in any case.
    #[must_use]
    pub fn rcpt(address: &str) -> Self {
        Self::Rcpt(address.to_ascii_lowercase())
    }

    /// [`When::Answered`] for `verb`, in any case.
    #[must_use]
    pub fn answered(verb: &str) -> Self {
        Self::Answered(verb.to_ascii_uppercase())
    }
}

/// Something the server does at a [`When`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Action {
    /// Drop the connection, wait, or send garbage. A dropped connection ends the session.
    Fault(Fault),
    /// Send this line as it is; CR, LF and NUL are stripped from it.
    Send(String),
    /// Answer with this reply instead of the server's own, and do not do what was asked. At [`When::Answered`] it is
    /// sent after the answer, as an unasked reply.
    Reply(Reply),
}

/// A reply the script gives: its code, its enhanced status code and its text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    code: u16,
    enhanced: String,
    lines: Vec<String>,
}

impl Reply {
    /// A one-line reply: `code` (kept between 200 and 599), the RFC 3463 code `enhanced` (`5.1.1`; empty for none),
    /// sent only when the server offers ENHANCEDSTATUSCODES, and `text`. CR, LF and NUL are stripped.
    #[must_use]
    pub fn new(code: u16, enhanced: &str, text: &str) -> Self {
        Self { code: code.clamp(200, 599), enhanced: clean(enhanced), lines: vec![clean(text)] }
    }

    /// Adds a line of text: the reply becomes a multiline one, each line with the code and the enhanced code.
    #[must_use]
    pub fn line(mut self, text: &str) -> Self {
        self.lines.push(clean(text));
        self
    }

    /// The reply code.
    #[must_use]
    pub fn code(&self) -> u16 {
        self.code
    }

    /// Whether the reply says yes (2xx or 3xx).
    pub(super) fn positive(&self) -> bool {
        self.code < 400
    }

    /// Adds the reply's lines to `out`, the enhanced code on each when `enhanced` is set.
    pub(super) fn write(&self, out: &mut Vec<u8>, enhanced: bool) {
        let code = (!self.enhanced.is_empty() && enhanced).then_some(self.enhanced.as_str());
        let lines: Vec<&str> = self.lines.iter().map(String::as_str).collect();
        write_reply(out, self.code, code, &lines);
    }
}

/// Adds a reply to `out`: one line per text, `-` after the code on all but the last.
pub(super) fn write_reply(out: &mut Vec<u8>, code: u16, enhanced: Option<&str>, lines: &[&str]) {
    let last = lines.len().saturating_sub(1);
    for (at, text) in lines.iter().enumerate() {
        let mark = if at == last { ' ' } else { '-' };
        let line = match enhanced {
            Some(enhanced) => format!("{code}{mark}{enhanced} {text}"),
            None => format!("{code}{mark}{text}"),
        };
        out.extend_from_slice(line.trim_end().as_bytes());
        out.extend_from_slice(b"\r\n");
    }
}

/// `text` without CR, LF and NUL.
pub(super) fn clean(text: &str) -> String {
    text.chars().filter(|c| !matches!(c, '\r' | '\n' | '\0')).collect()
}

/// Actions waiting for their [`When`]; each rule is played once, at the first matching point after the rules before
/// it with the same point.
#[derive(Debug, Clone, Default)]
pub(crate) struct Rules {
    waiting: Vec<(When, Vec<Action>)>,
}

impl Rules {
    pub(crate) fn add(&mut self, when: When, actions: Vec<Action>) {
        self.waiting.push((when, actions));
    }

    /// The actions of the first rule waiting for `when`, which is then played.
    pub(crate) fn take(&mut self, when: &When) -> Vec<Action> {
        match self.waiting.iter().position(|(waiting, _)| waiting == when) {
            Some(index) => self.waiting.remove(index).1,
            None => Vec::new(),
        }
    }
}
