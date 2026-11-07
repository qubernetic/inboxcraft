// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What happens at scripted requests: answers of the script's own, and faults.

use crate::Fault;

/// A request at which [`crate::dav::FakeDav::on`] plays its actions: its method and, when given, its path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct When {
    method: String,
    path: Option<String>,
}

impl When {
    /// Any request with `method` (`PROPFIND`, `REPORT`, `PUT`), in any case.
    #[must_use]
    pub fn method(method: &str) -> Self {
        Self { method: method.to_ascii_uppercase(), path: None }
    }

    /// A request with `method` for `path`, as decoded.
    #[must_use]
    pub fn at(method: &str, path: &str) -> Self {
        Self { method: method.to_ascii_uppercase(), path: Some(path.to_owned()) }
    }

    fn matches(&self, method: &str, path: &str) -> bool {
        self.method == method && self.path.as_deref().is_none_or(|wanted| wanted == path)
    }
}

/// Something the server does at a [`When`], before it reads the request's body or checks the password.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Action {
    /// Drop the connection, wait, or send garbage. A dropped connection ends the session.
    Fault(Fault),
    /// Answer with this instead of the server's own answer, and do not do what was asked.
    Reply(Reply),
}

/// An answer the script gives: its status, header fields and body. `Content-Length` is added.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub(crate) status: u16,
    pub(crate) fields: Vec<(String, String)>,
    pub(crate) body: Vec<u8>,
}

impl Reply {
    /// An answer with `status` (kept between 100 and 599), no fields and no body.
    #[must_use]
    pub fn new(status: u16) -> Self {
        Self { status: status.clamp(100, 599), fields: Vec::new(), body: Vec::new() }
    }

    /// Adds a header field; CR, LF and NUL are stripped from it.
    #[must_use]
    pub fn field(mut self, name: &str, value: &str) -> Self {
        self.fields.push((clean(name), clean(value)));
        self
    }

    /// Sets the body, sent as it is.
    #[must_use]
    pub fn body(self, body: impl Into<Vec<u8>>) -> Self {
        Self { body: body.into(), ..self }
    }
}

/// `text` without CR, LF and NUL.
fn clean(text: &str) -> String {
    text.chars().filter(|c| !matches!(c, '\r' | '\n' | '\0')).collect()
}

/// Actions waiting for their [`When`]; each rule is played once, at the first matching request after the rules
/// before it.
#[derive(Debug, Clone, Default)]
pub(crate) struct Rules {
    waiting: Vec<(When, Vec<Action>)>,
}

impl Rules {
    pub(crate) fn add(&mut self, when: When, actions: Vec<Action>) {
        self.waiting.push((when, actions));
    }

    /// The actions of the first rule waiting for a request with `method` for `path`, which is then played.
    pub(crate) fn take(&mut self, method: &str, path: &str) -> Vec<Action> {
        match self.waiting.iter().position(|(when, _)| when.matches(method, path)) {
            Some(index) => self.waiting.remove(index).1,
            None => Vec::new(),
        }
    }
}
