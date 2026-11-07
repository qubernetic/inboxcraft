// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The capabilities a fake IMAP server can offer, each of which a test can switch off.

/// A capability of the fake beyond `IMAP4rev1`. [`crate::imap::FakeImap::new`] offers all of them; a capability
/// switched off is not listed and its command or argument is answered `BAD`, as by a server that lacks it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Capability {
    /// STARTTLS (RFC 3501 6.2.1), offered before TLS when the fake has a STARTTLS configuration.
    StartTls,
    /// LOGINDISABLED: no sign-in before TLS. Switched off, the fake offers AUTH=PLAIN and takes a password in clear.
    LoginDisabled,
    /// AUTH=PLAIN (RFC 4616).
    AuthPlain,
    /// SASL-IR (RFC 4959): the PLAIN response on the AUTHENTICATE line.
    SaslIr,
    /// LITERAL+ (RFC 7888): non-synchronising literals.
    LiteralPlus,
    /// ENABLE (RFC 5161).
    Enable,
    /// CONDSTORE (RFC 7162).
    CondStore,
    /// QRESYNC (RFC 7162).
    QResync,
    /// IDLE (RFC 2177).
    Idle,
    /// NOTIFY (RFC 5465).
    Notify,
    /// SPECIAL-USE (RFC 6154).
    SpecialUse,
    /// UIDPLUS (RFC 4315): UID EXPUNGE, COPYUID.
    UidPlus,
    /// MOVE (RFC 6851).
    Move,
    /// PREVIEW (RFC 8970).
    Preview,
    /// COMPRESS=DEFLATE (RFC 4978).
    CompressDeflate,
}

impl Capability {
    /// Every capability, in the order the fake lists them.
    pub const ALL: [Self; 15] = [
        Self::StartTls,
        Self::LoginDisabled,
        Self::AuthPlain,
        Self::SaslIr,
        Self::LiteralPlus,
        Self::Enable,
        Self::CondStore,
        Self::QResync,
        Self::Idle,
        Self::Notify,
        Self::SpecialUse,
        Self::UidPlus,
        Self::Move,
        Self::Preview,
        Self::CompressDeflate,
    ];

    /// The capability as the server lists it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::StartTls => "STARTTLS",
            Self::LoginDisabled => "LOGINDISABLED",
            Self::AuthPlain => "AUTH=PLAIN",
            Self::SaslIr => "SASL-IR",
            Self::LiteralPlus => "LITERAL+",
            Self::Enable => "ENABLE",
            Self::CondStore => "CONDSTORE",
            Self::QResync => "QRESYNC",
            Self::Idle => "IDLE",
            Self::Notify => "NOTIFY",
            Self::SpecialUse => "SPECIAL-USE",
            Self::UidPlus => "UIDPLUS",
            Self::Move => "MOVE",
            Self::Preview => "PREVIEW",
            Self::CompressDeflate => "COMPRESS=DEFLATE",
        }
    }
}
