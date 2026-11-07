// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The bounce script: a message accepted for one recipient, then reported as failed in an RFC 3464 delivery report
//! that arrives in the fake IMAP server's mailbox.

use crate::imap::{Message, Postbox};
use crate::smtp::DOMAIN;
use crate::smtp::outbox::Submission;
use crate::smtp::rules::clean;

/// The boundary of the report's parts.
const BOUNDARY: &str = "inboxcraft-fake-report";
/// The date every report carries.
const DATE: &str = "Thu, 01 Jan 2026 00:00:00 +0000";

/// A recipient the fake accepts mail for and then reports as failed.
///
/// When a message for the recipient has been accepted, the fake writes a delivery report (RFC 3464 in an RFC 6522
/// `multipart/report`) from `MAILER-DAEMON@smtp.example.org` to the sender, and delivers it to the mailbox of the
/// [`Postbox`] a [`FakeImap`](crate::imap::FakeImap) reads. The report's parts: a line of text, the
/// `message/delivery-status` with the recipient, `Action: failed`, the status and the diagnostic, and the header block
/// of the message as `text/rfc822-headers`, its `Message-ID` among them. A message from the null path `<>` is not
/// reported, as RFC 5321 section 6.1 asks.
#[derive(Debug, Clone)]
pub struct Bounce {
    recipient: String,
    status: String,
    diagnostic: String,
    mailbox: String,
    postbox: Postbox,
}

impl Bounce {
    /// Reports `recipient` with `5.1.1` and `550 5.1.1 The mailbox does not exist`, into `INBOX` through `postbox`.
    #[must_use]
    pub fn new(recipient: &str, postbox: &Postbox) -> Self {
        Self {
            recipient: recipient.to_ascii_lowercase(),
            status: "5.1.1".to_owned(),
            diagnostic: "550 5.1.1 The mailbox does not exist".to_owned(),
            mailbox: "INBOX".to_owned(),
            postbox: postbox.clone(),
        }
    }

    /// The RFC 3463 `status` (`5.2.2`) and the remote server's reply as `diagnostic` (`552 5.2.2 Mailbox full`).
    #[must_use]
    pub fn status(self, status: &str, diagnostic: &str) -> Self {
        Self { status: clean(status), diagnostic: clean(diagnostic), ..self }
    }

    /// The fake IMAP mailbox the report is delivered to.
    #[must_use]
    pub fn mailbox(self, name: &str) -> Self {
        Self { mailbox: clean(name), ..self }
    }

    /// Whether this bounce is for `recipient`.
    pub(super) fn matches(&self, recipient: &str) -> bool {
        self.recipient.eq_ignore_ascii_case(recipient)
    }

    /// Reports `submission` as failed for this bounce's recipient, unless it came from the null path.
    pub(super) fn report(&self, submission: &Submission) {
        if submission.from.is_empty() {
            return;
        }
        let headers = submission.data.windows(4).position(|window| window == b"\r\n\r\n").map_or(submission.data.len(), |at| at + 2);
        let headers = String::from_utf8_lossy(submission.data.get(..headers).unwrap_or_default());
        let id = submission.id;
        let (from, recipient, status, diagnostic) = (&submission.from, &self.recipient, &self.status, &self.diagnostic);
        let text = format!(
            "From: Mail Delivery System <MAILER-DAEMON@{DOMAIN}>\n\
             To: <{from}>\n\
             Subject: Undelivered Mail Returned to Sender\n\
             Date: {DATE}\n\
             Message-ID: <report.{id}@{DOMAIN}>\n\
             Auto-Submitted: auto-replied\n\
             MIME-Version: 1.0\n\
             Content-Type: multipart/report; report-type=delivery-status; boundary=\"{BOUNDARY}\"\n\
             \n\
             --{BOUNDARY}\n\
             Content-Type: text/plain; charset=utf-8\n\
             \n\
             The fake mail system could not deliver your message to <{recipient}>: {diagnostic}\n\
             \n\
             --{BOUNDARY}\n\
             Content-Type: message/delivery-status\n\
             \n\
             Reporting-MTA: dns; {DOMAIN}\n\
             Arrival-Date: {DATE}\n\
             \n\
             Final-Recipient: rfc822; {recipient}\n\
             Action: failed\n\
             Status: {status}\n\
             Remote-MTA: dns; mx.example.org\n\
             Diagnostic-Code: smtp; {diagnostic}\n\
             \n\
             --{BOUNDARY}\n\
             Content-Type: text/rfc822-headers\n\
             \n\
             {headers}\n\
             --{BOUNDARY}--\n"
        );
        self.postbox.deliver(&self.mailbox, Message::new(&text));
    }
}
