// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What goes on the wire: the message after DATA (RFC 5321 section 4.5.2) and the checks on command arguments.

use inboxcraft_model::is_invisible_mark;

use crate::ProtoError;

/// Longest reverse-path or forward-path, without its angle brackets (RFC 5321 section 4.5.3.1.3).
pub const MAX_PATH: usize = 256;

/// Longest domain or address literal EHLO sends (RFC 5321 section 4.5.3.1.2).
pub const MAX_DOMAIN: usize = 255;

/// The message ready to send after the go-ahead for DATA: every line ending (CRLF, a lone LF or a lone CR) becomes
/// CRLF, a line that starts with a dot gets one more, the last line is ended, and a line with a lone dot follows.
pub(crate) fn dot_stuffed(message: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(message.len().saturating_add(message.len() / 32).saturating_add(5));
    let mut line_start = true;
    let mut bytes = message.iter().copied().peekable();
    while let Some(byte) = bytes.next() {
        match byte {
            b'\r' | b'\n' => {
                if byte == b'\r' {
                    bytes.next_if_eq(&b'\n');
                }
                out.extend_from_slice(b"\r\n");
                line_start = true;
            }
            _ => {
                if line_start && byte == b'.' {
                    out.push(b'.');
                }
                out.push(byte);
                line_start = false;
            }
        }
    }
    if !line_start {
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(b".\r\n");
    out
}

/// Whether `domain` can follow EHLO (RFC 5321 section 4.1.3): labels of letters, digits and inner hyphens joined by
/// single dots, or a whole address literal, `[` IPv4 `]` or `[IPv6:` IPv6 `]`; nothing else.
pub(crate) fn check_domain(domain: &str) -> Result<(), ProtoError> {
    let fits = domain.len() <= MAX_DOMAIN
        && match domain.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')) {
            Some(literal) => is_address_literal(literal),
            None => domain.split('.').all(is_label),
        };
    if fits { Ok(()) } else { Err(ProtoError::BadArgument("EHLO domain")) }
}

/// `Let-dig [Ldh-str]`: not empty, letters, digits and hyphens, a hyphen neither first nor last.
fn is_label(label: &str) -> bool {
    let edge = |b: Option<u8>| b.is_some_and(|b| b.is_ascii_alphanumeric());
    edge(label.bytes().next()) && edge(label.bytes().last()) && label.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

/// What goes between the brackets of an address literal: an IPv4 address, or `IPv6:` (any case) and an IPv6 one.
fn is_address_literal(literal: &str) -> bool {
    match literal.get(..5).filter(|tag| tag.eq_ignore_ascii_case("IPv6:")) {
        Some(_) => literal.get(5..).is_some_and(|address| address.parse::<std::net::Ipv6Addr>().is_ok()),
        None => literal.parse::<std::net::Ipv4Addr>().is_ok(),
    }
}

/// Whether `address` can go between the angle brackets of MAIL FROM or RCPT TO: not too long, no white space,
/// control character, invisible mark or angle bracket, and ASCII unless the transaction is `utf8` (SMTPUTF8).
pub(crate) fn check_path(address: &str, utf8: bool) -> Result<(), ProtoError> {
    if address.len() > MAX_PATH {
        return Err(ProtoError::BadArgument("address, too long"));
    }
    if address.chars().any(|c| c.is_control() || c.is_whitespace() || is_invisible_mark(c) || matches!(c, '<' | '>')) {
        return Err(ProtoError::BadArgument("address with a space, a control character, an invisible mark or an angle bracket"));
    }
    if !utf8 && !address.is_ascii() {
        return Err(ProtoError::BadArgument("address with non-ASCII characters, without SMTPUTF8"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "data_tests.rs"]
mod tests;
