// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Writing the fake's answers: lines, tagged results, strings and UID sets.

/// Adds `text` and CRLF.
pub(super) fn line(out: &mut Vec<u8>, text: &str) {
    out.extend_from_slice(text.as_bytes());
    out.extend_from_slice(b"\r\n");
}

pub(super) fn ok(out: &mut Vec<u8>, tag: &str, text: &str) {
    line(out, &format!("{tag} OK {text}"));
}

pub(super) fn no(out: &mut Vec<u8>, tag: &str, text: &str) {
    line(out, &format!("{tag} NO {text}"));
}

/// `bytes` as an IMAP string: quoted when it is printable ASCII, a literal otherwise.
pub(super) fn string(bytes: &[u8]) -> Vec<u8> {
    if !bytes.iter().all(|&byte| (0x20..0x7f).contains(&byte)) {
        return literal(bytes);
    }
    let mut out = vec![b'"'];
    for &byte in bytes {
        if matches!(byte, b'"' | b'\\') {
            out.push(b'\\');
        }
        out.push(byte);
    }
    out.push(b'"');
    out
}

/// `bytes` as an IMAP literal.
pub(super) fn literal(bytes: &[u8]) -> Vec<u8> {
    let mut out = format!("{{{}}}\r\n", bytes.len()).into_bytes();
    out.extend_from_slice(bytes);
    out
}

/// A parenthesised flag list.
pub(super) fn flags(flags: &[String]) -> String {
    format!("({})", flags.join(" "))
}

/// `text` without CR, LF and NUL.
pub(super) fn clean(text: &str) -> String {
    text.chars().filter(|c| !matches!(c, '\r' | '\n' | '\0')).collect()
}
