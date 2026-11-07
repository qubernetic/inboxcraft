// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The token a client shows on every line: 32 random bytes, written as 64 lowercase hex digits.

/// How many bytes a token holds.
pub const TOKEN_BYTES: usize = 32;

/// The control channel's token. It compares in constant time and never shows in `Debug` output; [`Token::text`] is
/// the only way to see it.
#[derive(Clone)]
pub struct Token([u8; TOKEN_BYTES]);

impl Token {
    /// The token made of `bytes`, which the caller draws from the operating system's random source.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; TOKEN_BYTES]) -> Self {
        Self(bytes)
    }

    /// The token written in `text`: exactly 64 hex digits, either case; `None` for anything else.
    ///
    /// ```
    /// use inboxcraft_control_key::Token;
    ///
    /// let token = Token::from_bytes([1; 32]);
    /// assert_eq!(Token::parse(&token.text()), Some(token));
    /// assert_eq!(Token::parse("0101"), None);
    /// ```
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let digits = text.as_bytes();
        if digits.len() != TOKEN_BYTES.saturating_mul(2) {
            return None;
        }
        let mut bytes = [0_u8; TOKEN_BYTES];
        for (byte, [high, low]) in bytes.iter_mut().zip(digits.as_chunks::<2>().0) {
            let (high, low) = (hex(*high)?, hex(*low)?);
            *byte = high << 4 | low;
        }
        Some(Self(bytes))
    }

    /// The token as 64 lowercase hex digits.
    #[must_use]
    pub fn text(&self) -> String {
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        let mut text = String::with_capacity(TOKEN_BYTES.saturating_mul(2));
        for byte in self.0 {
            for nibble in [byte >> 4, byte & 0x0f] {
                text.push(char::from(DIGITS.get(usize::from(nibble)).copied().unwrap_or(b'0')));
            }
        }
        text
    }
}

/// The value of one hex digit.
fn hex(digit: u8) -> Option<u8> {
    match digit {
        b'0'..=b'9' => Some(digit - b'0'),
        b'a'..=b'f' => Some(digit - b'a' + 10),
        b'A'..=b'F' => Some(digit - b'A' + 10),
        _ => None,
    }
}

impl PartialEq for Token {
    /// Every byte is looked at, whatever the first difference: how long a comparison takes says nothing about where a
    /// guess went wrong.
    fn eq(&self, other: &Self) -> bool {
        self.0.iter().zip(other.0.iter()).fold(0_u8, |differs, (a, b)| differs | (a ^ b)) == 0
    }
}

impl Eq for Token {}

impl std::fmt::Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Token(..)")
    }
}

#[cfg(test)]
#[path = "token_tests.rs"]
mod tests;
