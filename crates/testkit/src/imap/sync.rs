// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! ENABLE (RFC 5161) and the CONDSTORE and QRESYNC parameters of SELECT and EXAMINE (RFC 7162).

use crate::imap::Capability;
use crate::imap::fixture::Mailbox;
use crate::imap::reader::Token;
use crate::imap::session::Session;
use crate::imap::set::{UidSet, compact};
use crate::imap::write::{flags, line, ok};

/// What the last session knew, from a QRESYNC SELECT parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Resync {
    uid_validity: u32,
    modseq: u64,
    known: Option<UidSet>,
}

impl Resync {
    fn read(token: Option<&Token>) -> Option<Self> {
        let Some(Token::List(values)) = token else { return None };
        let [uid_validity, modseq, rest @ ..] = values.as_slice() else { return None };
        let known = match rest.first() {
            None => None,
            Some(known) => Some(UidSet::parse(known.atom()?, u32::MAX)?),
        };
        Some(Self { uid_validity: uid_validity.atom()?.parse().ok()?, modseq: modseq.atom()?.parse().ok()?, known })
    }

    /// The VANISHED (EARLIER) and FETCH lines that bring the client up to date, when its UIDVALIDITY is still right.
    pub(super) fn report(&self, mailbox: &Mailbox, out: &mut Vec<u8>) {
        if self.uid_validity != mailbox.uid_validity {
            return;
        }
        let mut vanished = mailbox.vanished_since(self.modseq);
        if let Some(known) = &self.known {
            vanished.retain(|uid| known.contains(*uid));
        }
        if !vanished.is_empty() {
            line(out, &format!("* VANISHED (EARLIER) {}", compact(&vanished)));
        }
        for (seq, message) in mailbox.messages.iter().enumerate().filter(|(_, message)| message.modseq > self.modseq) {
            line(out, &format!("* {} FETCH (UID {} FLAGS {} MODSEQ ({}))", seq + 1, message.uid, flags(&message.flags), message.modseq));
        }
    }
}

impl Session {
    pub(super) fn enable(&mut self, tag: &str, args: &[Token], out: &mut Vec<u8>) {
        if args.is_empty() {
            return self.bad(out, Some(tag), "ENABLE needs capabilities");
        }
        let mut enabled = vec!["* ENABLED"];
        for word in args.iter().filter_map(Token::atom).map(str::to_ascii_uppercase) {
            if word == "CONDSTORE" && self.fake.offers(Capability::CondStore) {
                self.condstore = true;
                enabled.push("CONDSTORE");
            }
            if word == "QRESYNC" && self.fake.offers(Capability::QResync) {
                self.condstore = true;
                self.qresync = true;
                enabled.push("QRESYNC");
            }
        }
        line(out, &enabled.join(" "));
        ok(out, tag, "ENABLE completed");
    }

    /// Reads the parameters of SELECT or EXAMINE: CONDSTORE, and QRESYNC once enabled.
    pub(super) fn select_parameters(&mut self, command: &str, params: &[Token]) -> Result<Option<Resync>, String> {
        let mut resync = None;
        let mut params = params.iter();
        while let Some(param) = params.next() {
            match param.atom().map(str::to_ascii_uppercase).as_deref() {
                Some("CONDSTORE") if self.fake.offers(Capability::CondStore) => self.condstore = true,
                Some("QRESYNC") if self.qresync => resync = Some(Resync::read(params.next()).ok_or("Bad QRESYNC parameter")?),
                Some("QRESYNC") if self.fake.offers(Capability::QResync) => return Err("QRESYNC is not enabled".to_owned()),
                _ => return Err(format!("Unknown {command} parameter")),
            }
        }
        Ok(resync)
    }
}
