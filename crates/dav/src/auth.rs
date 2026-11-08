// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The user name and password for HTTP Basic auth (RFC 7617).

use std::fmt;

use crate::DavError;

/// The user name and password a request carries, as `Authorization: Basic`. They are written only into a request to
/// an `https` origin ([`crate::Request::encode`]); `Debug` hides the password.
#[derive(Clone, PartialEq, Eq)]
pub struct Credentials {
    user: String,
    password: String,
}

impl Credentials {
    /// The user name and password.
    ///
    /// # Errors
    ///
    /// [`DavError::BadArgument`] when the user name is empty or holds a colon (Basic auth cannot carry one), or
    /// either holds a control character.
    pub fn new(user: &str, password: &str) -> Result<Self, DavError> {
        if user.is_empty() {
            return Err(DavError::BadArgument("an empty user name"));
        }
        if user.contains(':') {
            return Err(DavError::BadArgument("a colon in the user name"));
        }
        if user.chars().chain(password.chars()).any(char::is_control) {
            return Err(DavError::BadArgument("a control character"));
        }
        Ok(Self { user: user.to_owned(), password: password.to_owned() })
    }

    /// The user name.
    #[must_use]
    pub fn user(&self) -> &str {
        &self.user
    }

    /// The `Authorization` header's value.
    pub(crate) fn authorization(&self) -> String {
        format!("Basic {}", base64(format!("{}:{}", self.user, self.password).as_bytes()))
    }
}

impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Credentials").field("user", &self.user).field("password", &"***").finish()
    }
}

/// Base64 with padding (RFC 4648 section 4).
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3).saturating_mul(4));
    for group in bytes.chunks(3) {
        let [a, b, c] = [0, 1, 2].map(|at| group.get(at).copied().unwrap_or(0));
        let word = u32::from(a) << 16 | u32::from(b) << 8 | u32::from(c);
        for (index, shift) in [18_u32, 12, 6, 0].into_iter().enumerate() {
            if index > group.len() {
                out.push('=');
            } else {
                let sextet = usize::try_from(word >> shift & 0x3f).unwrap_or(0);
                out.push(ALPHABET.get(sextet).map_or('=', |&byte| char::from(byte)));
            }
        }
    }
    out
}

#[cfg(test)]
#[path = "auth_tests.rs"]
mod tests;
