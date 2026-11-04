// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Property tests of the message reader: no bytes make it panic, whatever reads holds no more parts than the cap,
//! header text for people holds no control character and no invisible direction mark, and a base64 body reads back
//! as the bytes it was made of.

use inboxcraft_mime::{ContentType, DeliveryStatus, Disposition, Headers, MAX_PARTS, Part, Status, TransferEncoding, parse};
use proptest::prelude::*;

/// Lines a message is made of: header fields of every kind the reader looks at, delimiters of a few boundaries,
/// encoded words, broken encodings and noise.
fn message_lines() -> impl Strategy<Value = Vec<u8>> {
    let line = prop_oneof![
        Just("Content-Type: multipart/mixed; boundary=a".to_owned()),
        Just("Content-Type: multipart/alternative; boundary=\"b\"".to_owned()),
        Just("Content-Type: multipart/digest; boundary=a".to_owned()),
        Just("Content-Type: multipart/report; report-type=delivery-status; boundary=b".to_owned()),
        Just("Content-Type: message/rfc822".to_owned()),
        Just("Content-Type: message/delivery-status".to_owned()),
        Just("Content-Type: text/plain; charset=iso-8859-2".to_owned()),
        Just("Content-Type: text/html; charset=x-unknown".to_owned()),
        Just("Content-Type: image/png; name*0*=utf-8''k%C3%A9p; name*1=.png".to_owned()),
        Just("Content-Transfer-Encoding: base64".to_owned()),
        Just("Content-Transfer-Encoding: quoted-printable".to_owned()),
        Just("Content-Transfer-Encoding: x-uue".to_owned()),
        Just("Content-Disposition: attachment; filename=\"=?UTF-8?Q?a=0Ab?=\"".to_owned()),
        Just("Content-Disposition: attachment; filename=\"a\u{202e}b.exe\"".to_owned()),
        Just("Content-ID: <x@example.org>".to_owned()),
        Just("Subject: =?UTF-8?B?w6l=?= =?ISO-8859-1?Q?=E9?=".to_owned()),
        Just("Reporting-MTA: dns; mx.example.org".to_owned()),
        Just("Final-Recipient: rfc822; a@example.org".to_owned()),
        Just("Action: failed".to_owned()),
        Just("Status: 5.1.1".to_owned()),
        Just("--a".to_owned()),
        Just("--a--".to_owned()),
        Just("--b".to_owned()),
        Just("--b--".to_owned()),
        Just(String::new()),
        Just(" folded".to_owned()),
        Just("w6FydsOtenTFsXLFkQ==".to_owned()),
        Just("=E1rv=EDz=\t".to_owned()),
        "\\PC{0,30}",
    ];
    proptest::collection::vec((line, prop::bool::ANY), 0..60).prop_map(|lines| {
        lines.into_iter().flat_map(|(line, crlf)| [line.into_bytes(), if crlf { b"\r\n".to_vec() } else { b"\n".to_vec() }].concat()).collect()
    })
}

/// A character that text for people must not hold: a control character, or a mark that changes how text runs or
/// joins without showing itself.
fn is_hidden(c: char) -> bool {
    c.is_control() || matches!(c, '\u{061c}' | '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{2069}' | '\u{feff}')
}

/// Everything the reader offers, on every part: none of it may panic, and header text holds no control character and
/// no invisible direction mark.
fn touch_everything(message: &Part<'_>) -> Result<(), TestCaseError> {
    let parts = message.parts();
    prop_assert!(parts.len() <= MAX_PARTS);
    for part in parts {
        let _ = part.decoded();
        let _ = part.text();
        let _ = part.disposition();
        let _ = part.delivery_status();
        let _ = part.is_attachment();
        for text in [part.file_name(), part.content_id(), part.headers().text("subject")].into_iter().flatten() {
            prop_assert!(!text.chars().any(is_hidden), "{text:?}");
        }
    }
    let _ = message.attachments();
    let _ = message.text_body();
    let _ = message.html_body();
    Ok(())
}

/// Base64 in lines of 76 characters.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = Vec::new();
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, b)| n | u32::from(*b) << (16 - 8 * i));
        for i in 0..4 {
            encoded.push(if i <= chunk.len() { ALPHABET[(n >> (18 - 6 * i) & 63) as usize] } else { b'=' });
        }
    }
    encoded.chunks(76).map(|line| String::from_utf8_lossy(line).into_owned()).collect::<Vec<_>>().join("\r\n")
}

proptest! {
    #[test]
    fn no_bytes_make_the_reader_panic(bytes in proptest::collection::vec(any::<u8>(), 0..2048)) {
        if let Ok(message) = parse(&bytes) {
            touch_everything(&message)?;
        }
        let _ = Headers::parse(&bytes);
        let _ = DeliveryStatus::parse(&bytes);
    }

    #[test]
    fn no_message_shaped_lines_make_the_reader_panic(bytes in message_lines()) {
        if let Ok(message) = parse(&bytes) {
            touch_everything(&message)?;
        }
        let _ = DeliveryStatus::parse(&bytes);
    }

    #[test]
    fn no_header_value_makes_the_value_readers_panic(value in "\\PC{0,80}") {
        let _ = ContentType::parse(&value);
        let _ = Disposition::parse(&value).file_name();
        let _ = TransferEncoding::parse(&value);
        let _ = Status::parse(&value);
    }

    #[test]
    fn header_text_holds_no_control_character(
        name in "[A-Za-z-]{1,12}",
        value in "[\\PC\u{061c}\u{200b}-\u{200f}\u{202a}-\u{202e}\u{2060}-\u{2069}\u{feff}]{0,80}",
    ) {
        let block = format!("{name}: {value}\r\n");
        if let Ok(headers) = Headers::parse(block.as_bytes())
            && let Some(text) = headers.text(&name)
        {
            prop_assert!(!text.chars().any(is_hidden), "{text:?}");
        }
    }

    #[test]
    fn a_base64_attachment_reads_back_as_its_bytes(bytes in proptest::collection::vec(any::<u8>(), 0..600)) {
        let message = format!(
            "Content-Type: multipart/mixed; boundary=b\r\n\r\n--b\r\nContent-Type: application/octet-stream\r\nContent-Transfer-Encoding: base64\r\n\r\n{}\r\n--b--\r\n",
            base64(&bytes)
        );
        let message = parse(message.as_bytes()).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let attachments = message.attachments();
        prop_assert_eq!(attachments.len(), 1);
        prop_assert_eq!(attachments[0].decoded().map_err(|e| TestCaseError::fail(e.to_string()))?.into_owned(), bytes);
    }
}
