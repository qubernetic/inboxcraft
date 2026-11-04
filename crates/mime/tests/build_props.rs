// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Property tests of the message builder: what it builds reads back as what went in, every line it writes is short
//! and ends in CRLF, and no value, whatever it holds, adds a field or a part.

use inboxcraft_mime::{Attachment, Mailbox, MessageBuilder, MimeError, parse};
use proptest::prelude::*;

fn address() -> impl Strategy<Value = String> {
    "[a-z0-9+_-]{1,20}(\\.[a-z0-9]{1,5})?@[a-z]{1,10}\\.example"
}

/// Text with the characters that could break a header: CR, LF, NUL, tabs and other control characters among
/// anything else.
fn hostile_text() -> impl Strategy<Value = String> {
    proptest::collection::vec(
        prop_oneof![Just("\r\n".to_owned()), Just("\r".to_owned()), Just("\n".to_owned()), Just("\0".to_owned()), "\\PC{0,8}", ".{0,4}"],
        0..12,
    )
    .prop_map(|pieces| pieces.concat())
}

/// A media type: hostile text, or tokens long enough to break the 998 line limit if nothing capped them.
fn media_type() -> impl Strategy<Value = String> {
    prop_oneof![hostile_text(), "[a-z]{1,200}/[a-z0-9.+-]{1,1200}"]
}

/// Line ends as the builder writes them.
fn crlf(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n").replace('\n', "\r\n")
}

/// Every line ends in CRLF, holds no other CR, LF or NUL and is at most 998 characters.
fn lines_are_clean(bytes: &[u8]) -> Result<(), TestCaseError> {
    for line in bytes.split(|b| *b == b'\n') {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        prop_assert!(line.len() <= 998);
        prop_assert!(!line.iter().any(|b| matches!(b, b'\r' | b'\0')), "{:?}", String::from_utf8_lossy(line));
    }
    let text = String::from_utf8_lossy(bytes);
    prop_assert_eq!(text.matches('\n').count(), text.matches("\r\n").count());
    Ok(())
}

fn fail(error: MimeError) -> TestCaseError {
    TestCaseError::fail(error.to_string())
}

proptest! {
    #[test]
    fn a_built_message_reads_back_as_what_went_in(
        subject in "\\PC{0,200}",
        from_name in "\\PC{0,40}",
        from in address(),
        to in proptest::collection::vec(address(), 0..4),
        text in any::<String>(),
        html in proptest::option::of("\\PC{0,300}"),
        attachments in proptest::collection::vec(("\\PC{0,90}", proptest::collection::vec(any::<u8>(), 0..300)), 0..4),
    ) {
        let mut builder = MessageBuilder::new().subject(&subject).from(Mailbox::named(&from_name, &from)).text(&text);
        for address in &to {
            builder = builder.to(Mailbox::new(address));
        }
        if let Some(html) = &html {
            builder = builder.html(html);
        }
        for (name, data) in &attachments {
            builder = builder.attach(Attachment::new(name, "application/octet-stream", data.clone()));
        }
        let bytes = builder.build().map_err(fail)?;
        lines_are_clean(&bytes)?;
        let message = parse(&bytes).map_err(fail)?;
        let headers = message.headers();
        prop_assert_eq!(headers.text("subject"), Some(subject.trim().to_owned()));
        let from_field = headers.text("from").unwrap_or_default();
        prop_assert!(from_field == from || from_field.ends_with(&format!(" <{from}>")), "{from_field}");
        if !from_name.trim().is_empty() && !from_field.starts_with('"') {
            prop_assert_eq!(from_field, format!("{} <{from}>", from_name.trim()));
        }
        prop_assert_eq!(headers.get("to").map(str::to_owned), (!to.is_empty()).then(|| to.join(", ")));
        let body = message.text_body().map(|part| part.text()).transpose().map_err(fail)?;
        prop_assert_eq!(body, Some(crlf(&text)));
        let html_body = message.html_body().map(|part| part.text()).transpose().map_err(fail)?;
        prop_assert_eq!(html_body, html.as_deref().map(crlf));
        let read = message.attachments();
        prop_assert_eq!(read.len(), attachments.len());
        for (part, (name, data)) in read.iter().zip(&attachments) {
            let name = name.trim();
            prop_assert_eq!(part.file_name(), (!name.is_empty()).then(|| name.to_owned()));
            prop_assert_eq!(&*part.decoded().map_err(fail)?, data.as_slice());
        }
    }

    #[test]
    fn no_value_adds_a_field_or_a_part(
        text in hostile_text(),
        address_text in hostile_text(),
        id in hostile_text(),
        name in hostile_text(),
        media_type in media_type(),
    ) {
        let built = MessageBuilder::new()
            .subject(&text)
            .from(Mailbox::named(&text, "a@example.org"))
            .to(Mailbox::new(&address_text))
            .message_id(&id)
            .header("X-Note", &text)
            .text(&text)
            .attach(Attachment::new(&text, "text/plain", text.clone().into_bytes()))
            .build();
        match built {
            Ok(bytes) => {
                lines_are_clean(&bytes)?;
                let message = parse(&bytes).map_err(fail)?;
                let names: Vec<String> = message.headers().fields().iter().map(|f| f.name().to_ascii_lowercase()).collect();
                prop_assert_eq!(names, ["from", "to", "subject", "message-id", "x-note", "mime-version", "content-type"]);
                prop_assert_eq!(message.attachments().len(), 1);
            }
            Err(error) => prop_assert!(matches!(error, MimeError::BadHeader("address" | "message id")), "{error}"),
        }
        let header = MessageBuilder::new().header(&name, &text).build();
        prop_assert!(header.as_ref().is_ok_and(|bytes| parse(bytes).is_ok_and(|m| m.headers().fields().len() == 4)) || header == Err(MimeError::BadHeader("field name")));
        let attached = MessageBuilder::new().attach(Attachment::new("a", &media_type, vec![])).build();
        match attached {
            Ok(bytes) => {
                lines_are_clean(&bytes)?;
                prop_assert_eq!(parse(&bytes).map_err(fail)?.attachments().len(), 1);
            }
            Err(error) => prop_assert_eq!(error, MimeError::BadHeader("media type")),
        }
    }
}
