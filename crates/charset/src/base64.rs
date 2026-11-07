// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Base64 (RFC 4648 section 4) for the B encoding of encoded words.

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// The length of the base64 text of `len` bytes, padding included.
pub(crate) fn encoded_len(len: usize) -> usize {
    len.div_ceil(3).saturating_mul(4)
}

/// Encodes bytes, with padding.
pub(crate) fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(encoded_len(bytes.len()));
    for chunk in bytes.chunks(3) {
        let b0 = chunk.first().copied().unwrap_or(0);
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        let group = (u32::from(b0) << 16) | (u32::from(b1) << 8) | u32::from(b2);
        for (index, shift) in [18_u32, 12, 6, 0].into_iter().enumerate() {
            if index <= chunk.len() {
                out.push(char::from(sextet_char(group >> shift)));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Decodes base64. Missing padding is forgiven; anything else that is not base64 gives `None`.
pub(crate) fn decode(text: &[u8]) -> Option<Vec<u8>> {
    let data = text.strip_suffix(b"==").or_else(|| text.strip_suffix(b"=")).unwrap_or(text);
    let (tail, padding) = (data.len() % 4, text.len() - data.len());
    // One character cannot hold a byte; padding belongs only after a short last group, and at most up to four.
    if tail == 1 || (padding > 0 && (tail == 0 || padding > 4 - tail)) {
        return None;
    }
    let mut out = Vec::with_capacity(data.len() / 4 * 3 + 2);
    for chunk in data.chunks(4) {
        let mut group = 0_u32;
        for &byte in chunk {
            group = (group << 6) | sextet_value(byte)?;
        }
        // A short last group of n characters holds n - 1 bytes.
        group <<= 6 * (4 - chunk.len());
        let [_, b0, b1, b2] = group.to_be_bytes();
        out.extend([b0, b1, b2].into_iter().take(chunk.len().saturating_sub(1)));
    }
    Some(out)
}

fn sextet_char(sextet: u32) -> u8 {
    ALPHABET.get((sextet & 0x3f) as usize).copied().unwrap_or(b'A')
}

fn sextet_value(byte: u8) -> Option<u32> {
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

#[cfg(test)]
#[path = "base64_tests.rs"]
mod tests;
