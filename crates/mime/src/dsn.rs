// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Delivery status notifications: the `message/delivery-status` part of a `multipart/report` (RFC 3464, RFC 6522),
//! and its UTF-8 form `message/global-delivery-status` (RFC 6533). Status codes per RFC 3463.

use std::fmt;

use crate::header::{clean, line_at, split};
use crate::{Body, Headers, MimeError, Part};

/// The most recipients one delivery report may name.
pub const MAX_RECIPIENTS: usize = 1000;

/// A delivery report: what one mail server says about the delivery of one message to its recipients.
///
/// Field values are as written, unfolded, control characters as spaces; dates are kept as text.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeliveryStatus {
    /// `Reporting-MTA`: the server that wrote the report.
    pub reporting_mta: Option<TypedValue>,
    /// `Original-Envelope-Id`: the envelope id the sender gave (RFC 3461).
    pub original_envelope_id: Option<String>,
    /// `Received-From-MTA`: where the reporting server had the message from.
    pub received_from_mta: Option<TypedValue>,
    /// `Arrival-Date`.
    pub arrival_date: Option<String>,
    /// One status per recipient, at least one.
    pub recipients: Vec<RecipientStatus>,
}

/// What happened with one recipient.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecipientStatus {
    /// `Final-Recipient`: the address the server tried.
    pub final_recipient: TypedValue,
    /// `Original-Recipient`: the address as the sender gave it (RFC 3461).
    pub original_recipient: Option<TypedValue>,
    /// `Action`.
    pub action: Action,
    /// `Status`.
    pub status: Status,
    /// `Remote-MTA`: the server that answered.
    pub remote_mta: Option<TypedValue>,
    /// `Diagnostic-Code`: that server's answer, such as `smtp; 550 5.1.1 user unknown`.
    pub diagnostic_code: Option<TypedValue>,
    /// `Last-Attempt-Date`.
    pub last_attempt_date: Option<String>,
    /// `Will-Retry-Until`.
    pub will_retry_until: Option<String>,
}

/// A value with its type: `rfc822; anna@example.org`, `dns; mx.example.org`, `smtp; 550 ...`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypedValue {
    /// The type, lower case (`rfc822`, `utf-8`, `dns`, `smtp`, `x-...`).
    pub kind: String,
    /// The value.
    pub value: String,
}

/// What the reporting server did for a recipient (RFC 3464 section 2.3.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// The message could not be delivered.
    Failed,
    /// Not delivered yet; the server keeps trying.
    Delayed,
    /// Delivered.
    Delivered,
    /// Passed on to a server that sends no reports.
    Relayed,
    /// Delivered and passed on to more addresses (a list).
    Expanded,
}

/// An enhanced status code, `class.subject.detail` (RFC 3463).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Status {
    /// 2 success, 4 temporary failure, 5 permanent failure.
    pub class: u8,
    /// What it is about (1 addressing, 2 mailbox, 4 network, 7 security ...), at most 999.
    pub subject: u16,
    /// The detail, at most 999.
    pub detail: u16,
}

impl Status {
    /// Reads `class.subject.detail`; a comment after it is ignored. `None` when it is no status code.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        let code = value.split(|c: char| c.is_whitespace() || c == '(').find(|t| !t.is_empty())?;
        let mut numbers = code.split('.');
        let (Some(class), Some(subject), Some(detail), None) = (numbers.next(), numbers.next(), numbers.next(), numbers.next()) else {
            return None;
        };
        let number = |n: &str| ((1..=3).contains(&n.len()) && n.bytes().all(|b| b.is_ascii_digit())).then(|| n.parse::<u16>().ok()).flatten();
        let class = number(class).and_then(|c| u8::try_from(c).ok()).filter(|c| matches!(c, 2 | 4 | 5))?;
        Some(Self { class, subject: number(subject)?, detail: number(detail)? })
    }

    /// Class 2.
    #[must_use]
    pub fn is_success(self) -> bool {
        self.class == 2
    }

    /// Class 4: trying again later may work.
    #[must_use]
    pub fn is_temporary_failure(self) -> bool {
        self.class == 4
    }

    /// Class 5: the message will not get through as it is.
    #[must_use]
    pub fn is_permanent_failure(self) -> bool {
        self.class == 5
    }
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.class, self.subject, self.detail)
    }
}

impl DeliveryStatus {
    /// Reads the body of a `message/delivery-status` part: the per-message fields, then an empty line, then the
    /// fields of each recipient, one group per recipient, empty lines between them.
    ///
    /// # Errors
    ///
    /// The errors of [`Headers::parse`]; [`MimeError::MalformedReport`] for a line that is no field;
    /// [`MimeError::MissingField`] when there is no recipient or a recipient lacks `Final-Recipient`, `Action` or
    /// `Status`; [`MimeError::BadField`] for a value that cannot be read; [`MimeError::TooManyRecipients`] past
    /// [`MAX_RECIPIENTS`].
    pub fn parse(bytes: &[u8]) -> Result<Self, MimeError> {
        let (fields, mut rest) = split(bytes, false)?;
        let mut report = Self {
            reporting_mta: typed(&fields, "Reporting-MTA")?,
            original_envelope_id: text(&fields, "Original-Envelope-Id"),
            received_from_mta: typed(&fields, "Received-From-MTA")?,
            arrival_date: text(&fields, "Arrival-Date"),
            recipients: Vec::new(),
        };
        loop {
            rest = skip_empty_lines(rest);
            if rest.is_empty() {
                break;
            }
            let (fields, after) = split(rest, false)?;
            if fields.fields().is_empty() {
                return Err(MimeError::MalformedReport);
            }
            if report.recipients.len() >= MAX_RECIPIENTS {
                return Err(MimeError::TooManyRecipients { limit: MAX_RECIPIENTS });
            }
            report.recipients.push(recipient(&fields)?);
            rest = after;
        }
        if report.recipients.is_empty() {
            return Err(MimeError::MissingField("Final-Recipient"));
        }
        Ok(report)
    }
}

impl Part<'_> {
    /// The delivery report of a bounce: the first `message/delivery-status` or `message/global-delivery-status`
    /// part among the direct parts of this `multipart/report`, read. Parts further down, those of an attached
    /// message above all, are not looked at: a status there was written by someone else.
    ///
    /// # Errors
    ///
    /// [`MimeError::NotAReport`] when this is no `multipart/report` with such a part; the errors of
    /// [`Part::decoded`] and [`DeliveryStatus::parse`].
    pub fn delivery_status(&self) -> Result<DeliveryStatus, MimeError> {
        let Body::Multipart(multipart) = self.body() else { return Err(MimeError::NotAReport) };
        if !self.content_type().is("multipart", "report") {
            return Err(MimeError::NotAReport);
        }
        let is_status =
            |part: &&Self| part.content_type().is("message", "delivery-status") || part.content_type().is("message", "global-delivery-status");
        let part = multipart.parts().iter().find(is_status).ok_or(MimeError::NotAReport)?;
        DeliveryStatus::parse(&part.decoded()?)
    }

    /// The `Message-ID` of the message a report is about, as its returned header writes it (`<id@example.org>`,
    /// unfolded, control characters as spaces): from the first direct part of this `multipart/report` that returns the
    /// message's header (`text/rfc822-headers`, `message/rfc822-headers`, `message/global-headers`, RFC 6522 and
    /// RFC 6533) or the whole message (`message/rfc822`, `message/global`). `None` when there is no such part, it
    /// cannot be read, or its header has no `Message-ID`.
    #[must_use]
    pub fn reported_message_id(&self) -> Option<String> {
        let Body::Multipart(multipart) = self.body() else { return None };
        if !self.content_type().is("multipart", "report") {
            return None;
        }
        let returned = |part: &&Self| {
            let kind = part.content_type();
            RETURNED.iter().any(|(kind_type, subtype)| kind.is(kind_type, subtype))
        };
        let part = multipart.parts().iter().find(returned)?;
        let id = match part.body() {
            Body::Message(message) => message.headers().get("Message-ID").map(clean),
            _ => split(&part.decoded().ok()?, false).ok()?.0.get("Message-ID").map(clean),
        };
        id.map(|id| id.trim().to_owned()).filter(|id| !id.is_empty())
    }
}

/// The media types of a report's part that returns the message's header or the whole message.
const RETURNED: [(&str, &str); 5] =
    [("text", "rfc822-headers"), ("message", "rfc822-headers"), ("message", "global-headers"), ("message", "rfc822"), ("message", "global")];

fn recipient(fields: &Headers) -> Result<RecipientStatus, MimeError> {
    let action = text(fields, "Action").ok_or(MimeError::MissingField("Action"))?;
    let action = match action.split(|c: char| c.is_whitespace() || c == '(').next().unwrap_or_default().to_ascii_lowercase().as_str() {
        "failed" => Action::Failed,
        "delayed" => Action::Delayed,
        "delivered" => Action::Delivered,
        "relayed" => Action::Relayed,
        "expanded" => Action::Expanded,
        _ => return Err(MimeError::BadField("Action")),
    };
    let status = text(fields, "Status").ok_or(MimeError::MissingField("Status"))?;
    Ok(RecipientStatus {
        final_recipient: typed(fields, "Final-Recipient")?.ok_or(MimeError::MissingField("Final-Recipient"))?,
        original_recipient: typed(fields, "Original-Recipient")?,
        action,
        status: Status::parse(&status).ok_or(MimeError::BadField("Status"))?,
        remote_mta: typed(fields, "Remote-MTA")?,
        diagnostic_code: typed(fields, "Diagnostic-Code")?,
        last_attempt_date: text(fields, "Last-Attempt-Date"),
        will_retry_until: text(fields, "Will-Retry-Until"),
    })
}

/// A field as cleaned text.
fn text(fields: &Headers, name: &str) -> Option<String> {
    fields.get(name).map(|value| clean(value).trim().to_owned())
}

/// A `type; value` field; [`MimeError::BadField`] without the `;` or the type.
fn typed(fields: &Headers, name: &'static str) -> Result<Option<TypedValue>, MimeError> {
    let Some(value) = text(fields, name) else { return Ok(None) };
    let (kind, value) = value.split_once(';').ok_or(MimeError::BadField(name))?;
    let kind = kind.trim().to_ascii_lowercase();
    if kind.is_empty() {
        return Err(MimeError::BadField(name));
    }
    Ok(Some(TypedValue { kind, value: value.trim().to_owned() }))
}

fn skip_empty_lines(mut bytes: &[u8]) -> &[u8] {
    loop {
        let (line, next) = line_at(bytes, 0);
        if !line.is_empty() || next == 0 {
            return bytes;
        }
        bytes = bytes.get(next..).unwrap_or_default();
    }
}

#[cfg(test)]
#[path = "dsn_tests.rs"]
mod tests;
