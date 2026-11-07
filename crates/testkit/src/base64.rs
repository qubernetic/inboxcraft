// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Base64 (RFC 4648 section 4) for the IMAP AUTHENTICATE and SMTP AUTH responses, strict: padded, no white space.

/// Decodes `text`, or `None` when it is not padded base64.
pub(crate) fn decode(text: &[u8]) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(4) {
        return None;
    }
    let body = text.strip_suffix(b"==").or_else(|| text.strip_suffix(b"=")).unwrap_or(text);
    let mut out = Vec::with_capacity(body.len() / 4 * 3 + 2);
    let mut acc: u32 = 0;
    let mut bits = 0;
    for &byte in body {
        acc = ((acc << 6) | value(byte)?) & 0xFFFF;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(u8::try_from((acc >> bits) & 0xFF).ok()?);
        }
    }
    Some(out)
}

fn value(byte: u8) -> Option<u32> {
    let value = match byte {
        b'A'..=b'Z' => byte - b'A',
        b'a'..=b'z' => byte - b'a' + 26,
        b'0'..=b'9' => byte - b'0' + 52,
        b'+' => 62,
        b'/' => 63,
        _ => return None,
    };
    Some(u32::from(value))
}
