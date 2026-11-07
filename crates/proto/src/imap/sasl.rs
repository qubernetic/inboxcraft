// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The SASL PLAIN response (RFC 4616) in base64 (RFC 4648 section 4).

use crate::ProtoError;

/// Longest user name and password, together, one sign-in carries.
pub const MAX_CREDENTIALS: usize = 8 * 1024;

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Whether a user name or password can go into a sign-in: not empty, no NUL, and together not too long.
pub(crate) fn check_credentials(user: &str, password: &str) -> Result<(), ProtoError> {
    if user.is_empty() || password.is_empty() {
        return Err(ProtoError::BadArgument("empty user name or password"));
    }
    if user.contains('\0') || password.contains('\0') {
        return Err(ProtoError::BadArgument("user name or password with a NUL"));
    }
    if user.len().saturating_add(password.len()) > MAX_CREDENTIALS {
        return Err(ProtoError::BadArgument("user name or password, too long"));
    }
    Ok(())
}

/// `\0user\0password` in base64, with an empty authorisation identity: the server derives it from the user.
pub(crate) fn plain_response(user: &str, password: &str) -> Result<String, ProtoError> {
    check_credentials(user, password)?;
    let mut message = Vec::with_capacity(user.len().saturating_add(password.len()).saturating_add(2));
    message.push(0);
    message.extend_from_slice(user.as_bytes());
    message.push(0);
    message.extend_from_slice(password.as_bytes());
    Ok(base64(&message))
}

/// Every base64 form a server may write the credentials back in, whichever way they were sent: the PLAIN token and
/// the password alone, each with and without its padding.
pub(crate) fn base64_forms(user: &str, password: &str) -> Result<Vec<String>, ProtoError> {
    let forms = [plain_response(user, password)?, base64(password.as_bytes())];
    Ok(forms.into_iter().flat_map(|form| [form.trim_end_matches('=').to_owned(), form]).collect())
}

/// Base64 with padding.
fn base64(bytes: &[u8]) -> String {
    let sextet = |group: u32, shift: u32| ALPHABET.get(((group >> shift) & 0x3F) as usize).copied().map_or('A', char::from);
    let mut out = String::with_capacity(bytes.len().div_ceil(3).saturating_mul(4));
    for chunk in bytes.chunks(3) {
        let at = |i: usize| u32::from(chunk.get(i).copied().unwrap_or(0));
        let group = (at(0) << 16) | (at(1) << 8) | at(2);
        for (index, shift) in [18_u32, 12, 6, 0].into_iter().enumerate() {
            out.push(if index <= chunk.len() { sextet(group, shift) } else { '=' });
        }
    }
    out
}

#[cfg(test)]
#[path = "sasl_tests.rs"]
mod tests;
