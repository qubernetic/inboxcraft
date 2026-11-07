// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The service extensions the fake lists after EHLO.

/// A service extension a test can switch off with [`crate::smtp::FakeSmtp::without`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Extension {
    /// `STARTTLS` (RFC 3207), listed on a plain connection when [`crate::smtp::FakeSmtp::starttls`] gave a
    /// configuration.
    StartTls,
    /// `PIPELINING` (RFC 2920). The fake reads one command after the other either way; it only lists it.
    Pipelining,
    /// `SIZE` (RFC 1870) with the largest message, and the `SIZE=` parameter of MAIL.
    Size,
    /// `8BITMIME` (RFC 6152) and the `BODY=` parameter of MAIL.
    EightBitMime,
    /// `SMTPUTF8` (RFC 6531): the `SMTPUTF8` parameter of MAIL and addresses that are not ASCII.
    SmtpUtf8,
    /// `ENHANCEDSTATUSCODES` (RFC 2034): replies carry an RFC 3463 code after the reply code.
    EnhancedStatusCodes,
    /// The `PLAIN` mechanism of `AUTH` (RFC 4954, RFC 4616).
    AuthPlain,
    /// The `LOGIN` mechanism of `AUTH`.
    AuthLogin,
}

impl Extension {
    /// Every extension, in the order the fake lists them.
    pub const ALL: [Self; 8] = [
        Self::StartTls,
        Self::Pipelining,
        Self::Size,
        Self::EightBitMime,
        Self::SmtpUtf8,
        Self::EnhancedStatusCodes,
        Self::AuthPlain,
        Self::AuthLogin,
    ];

    /// The extension's keyword; both AUTH mechanisms are listed on one `AUTH` line.
    #[must_use]
    pub fn keyword(self) -> &'static str {
        match self {
            Self::StartTls => "STARTTLS",
            Self::Pipelining => "PIPELINING",
            Self::Size => "SIZE",
            Self::EightBitMime => "8BITMIME",
            Self::SmtpUtf8 => "SMTPUTF8",
            Self::EnhancedStatusCodes => "ENHANCEDSTATUSCODES",
            Self::AuthPlain | Self::AuthLogin => "AUTH",
        }
    }
}
