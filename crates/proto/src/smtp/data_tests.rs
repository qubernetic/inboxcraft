// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use proptest::prelude::*;

use super::{MAX_DOMAIN, MAX_PATH, check_domain, check_path, dot_stuffed};
use crate::ProtoError;

#[test]
fn a_leading_dot_is_doubled_and_the_message_ends_with_a_lone_dot() {
    assert_eq!(dot_stuffed(b".hidden\r\nshown\r\n"), b"..hidden\r\nshown\r\n.\r\n");
    assert_eq!(dot_stuffed(b"a.b\r\n.\r\n"), b"a.b\r\n..\r\n.\r\n");
}

#[test]
fn every_line_ending_becomes_crlf() {
    assert_eq!(dot_stuffed(b"a\nb\rc\r\n\n\r"), b"a\r\nb\r\nc\r\n\r\n\r\n.\r\n");
    assert_eq!(dot_stuffed(b"no ending"), b"no ending\r\n.\r\n");
    assert_eq!(dot_stuffed(b""), b".\r\n");
}

/// What a server reads back: the lines up to the lone dot, each without one leading dot.
fn unstuffed(wire: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    for line in wire.strip_suffix(b"\r\n")?.split(|b| *b == b'\n') {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line == b"." {
            return Some(out);
        }
        out.extend_from_slice(line.strip_prefix(b".").unwrap_or(line));
        out.extend_from_slice(b"\r\n");
    }
    None
}

proptest! {
    #[test]
    fn the_server_reads_back_the_message_with_its_lines_ended(message in proptest::collection::vec(any::<u8>(), 0..200)) {
        let wire = dot_stuffed(&message);
        // Nothing but the end can end the data.
        let end = wire.windows(5).position(|w| w == b"\r\n.\r\n").map(|at| at + 5);
        prop_assert!(end.is_none_or(|end| end == wire.len()), "{}", wire.escape_ascii());
        prop_assert!(!wire.windows(2).any(|w| w[1] == b'\n' && w[0] != b'\r'));
        let mut expected: Vec<u8> = Vec::new();
        let mut lines: Vec<Vec<u8>> = vec![Vec::new()];
        let mut bytes = message.iter().copied().peekable();
        while let Some(byte) = bytes.next() {
            if byte == b'\r' || byte == b'\n' {
                if byte == b'\r' {
                    bytes.next_if_eq(&b'\n');
                }
                lines.push(Vec::new());
            } else if let Some(line) = lines.last_mut() {
                line.push(byte);
            }
        }
        if lines.last().is_some_and(Vec::is_empty) {
            lines.pop();
        }
        for line in lines {
            expected.extend_from_slice(&line);
            expected.extend_from_slice(b"\r\n");
        }
        prop_assert_eq!(unstuffed(&wire), Some(expected));
    }
}

#[test]
fn a_domain_or_address_literal_fits_and_nothing_else() {
    for domain in ["client.example.org", "[192.0.2.1]", "[IPv6:2001:db8::1]", "a"] {
        assert_eq!(check_domain(domain), Ok(()), "{domain}");
    }
    let long = "a".repeat(MAX_DOMAIN + 1);
    for domain in ["", "a b", "a\r\nQUIT", "árvíz.example.org", "a<b", &long] {
        assert_eq!(check_domain(domain), Err(ProtoError::BadArgument("EHLO domain")), "{domain}");
    }
    assert_eq!(check_domain(&long[1..]), Ok(()));
}

#[test]
fn an_address_is_capped_and_holds_nothing_that_ends_or_bends_the_line() {
    assert_eq!(check_path("ada@example.org", false), Ok(()));
    assert_eq!(check_path("", false), Ok(()));
    assert_eq!(check_path("ádám@example.org", true), Ok(()));
    assert_eq!(check_path("ádám@example.org", false), Err(ProtoError::BadArgument("address with non-ASCII characters, without SMTPUTF8")));
    let long = format!("{}@example.org", "a".repeat(MAX_PATH));
    assert_eq!(check_path(&long, true), Err(ProtoError::BadArgument("address, too long")));
    assert_eq!(check_path(&long[long.len() - MAX_PATH..], false), Ok(()));
    for address in ["a b@example.org", "a\0@example.org", "a\t@example.org", "<a@example.org>", "a\u{200b}@example.org", "a\u{a0}@example.org"] {
        assert_eq!(
            check_path(address, true),
            Err(ProtoError::BadArgument("address with a space, a control character, an invisible mark or an angle bracket")),
            "{address:?}"
        );
    }
}
