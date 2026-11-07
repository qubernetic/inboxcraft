// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_time::{CivilDateTime, MailDate, Offset, Zone};

use super::{Attachment, Mailbox, MessageBuilder, within_limit};
use crate::{Body, MAX_MESSAGE_BYTES, MimeError, Part, TransferEncoding, parse};

fn anna() -> Mailbox {
    Mailbox::named("Anna Kovács", "anna@example.org")
}

/// The names of the fields of a part, in order, lower case.
fn names(part: &Part<'_>) -> Vec<String> {
    part.headers().fields().iter().map(|field| field.name().to_ascii_lowercase()).collect()
}

/// Every line of a built message is short and ends in CRLF, and no bare CR or LF is left.
fn assert_lines(bytes: &[u8]) {
    let text = String::from_utf8_lossy(bytes);
    for line in text.split("\r\n") {
        assert!(line.len() <= 998, "{line}");
        assert!(!line.contains(['\r', '\n', '\0']), "{line:?}");
    }
}

#[test]
fn a_text_message_has_its_fields_in_order() {
    let date = MailDate::from_local(CivilDateTime::new(2026, 11, 4, 13, 5, 0).unwrap(), Zone::Fixed(Offset::UTC)).unwrap();
    let bytes = MessageBuilder::new()
        .date(date)
        .from(anna())
        .sender(Mailbox::new("office@example.org"))
        .reply_to(Mailbox::new("lists@example.org"))
        .to(Mailbox::new("bela@example.org"))
        .to(Mailbox::named("Csaba \"Csabi\" Nagy", "csaba@example.org"))
        .cc(Mailbox::named("Dóra", "dora@example.org"))
        .subject("Árvíztűrő tükörfúrógép")
        .message_id("<1@example.org>")
        .in_reply_to("0@example.org")
        .reference("<a@example.org>")
        .reference("0@example.org")
        .header("X-Mailer", "InboxCraft")
        .text("Szia!\nLine two.\n")
        .build()
        .unwrap();
    assert_lines(&bytes);
    let message = parse(&bytes).unwrap();
    assert_eq!(
        names(&message),
        [
            "date",
            "from",
            "sender",
            "reply-to",
            "to",
            "cc",
            "subject",
            "message-id",
            "in-reply-to",
            "references",
            "x-mailer",
            "mime-version",
            "content-type",
            "content-transfer-encoding"
        ]
    );
    let headers = message.headers();
    assert_eq!(headers.get("date"), Some("Wed, 4 Nov 2026 13:05:00 +0000"));
    assert_eq!(headers.get("sender"), Some("office@example.org"));
    assert_eq!(headers.text("from").as_deref(), Some("Anna Kovács <anna@example.org>"));
    assert_eq!(headers.get("to"), Some(r#"bela@example.org, "Csaba \"Csabi\" Nagy" <csaba@example.org>"#));
    assert_eq!(headers.text("subject").as_deref(), Some("Árvíztűrő tükörfúrógép"));
    assert_eq!(headers.get("message-id"), Some("<1@example.org>"));
    assert_eq!(headers.get("in-reply-to"), Some("<0@example.org>"));
    assert_eq!(headers.get("references"), Some("<a@example.org> <0@example.org>"));
    assert_eq!(headers.get("mime-version"), Some("1.0"));
    assert_eq!(message.content_type().essence(), "text/plain");
    assert_eq!(message.content_type().param("charset"), Some("utf-8"));
    assert_eq!(message.transfer_encoding(), &TransferEncoding::SevenBit);
    assert_eq!(message.text().unwrap(), "Szia!\r\nLine two.\r\n");
}

#[test]
fn an_empty_builder_writes_an_empty_text_message() {
    let bytes = MessageBuilder::default().build().unwrap();
    let message = parse(&bytes).unwrap();
    assert_eq!(names(&message), ["mime-version", "content-type", "content-transfer-encoding"]);
    assert_eq!(message.text().unwrap(), "");
}

#[test]
fn text_that_is_not_plain_ascii_is_quoted_printable() {
    for text in ["Árvíztűrő", "nul\0byte", "trailing space ", &"x".repeat(1200)] {
        let bytes = MessageBuilder::new().text(text).build().unwrap();
        assert_lines(&bytes);
        let message = parse(&bytes).unwrap();
        assert_eq!(message.transfer_encoding(), &TransferEncoding::QuotedPrintable, "{text}");
        assert_eq!(message.text().unwrap(), text);
    }
}

#[test]
fn text_and_html_are_alternatives() {
    let bytes = MessageBuilder::new().text("plain").html("<p>rich</p>").build().unwrap();
    let message = parse(&bytes).unwrap();
    assert!(message.content_type().is("multipart", "alternative"));
    let Body::Multipart(multipart) = message.body() else { panic!("no multipart") };
    assert!(multipart.closed());
    assert_eq!(multipart.parts().len(), 2);
    assert_eq!(message.text_body().unwrap().text().unwrap(), "plain");
    assert_eq!(message.html_body().unwrap().text().unwrap(), "<p>rich</p>");
    assert!(message.attachments().is_empty());
}

#[test]
fn html_alone_is_a_single_part() {
    let message = MessageBuilder::new().html("<b>x</b>").build().unwrap();
    let message = parse(&message).unwrap();
    assert!(message.content_type().is("text", "html"));
    assert!(message.text_body().is_none());
}

#[test]
fn attachments_make_a_mixed_message() {
    let bytes = MessageBuilder::new()
        .text("see attached")
        .html("<p>see attached</p>")
        .attach(Attachment::new("kép.png", "Image/PNG", b"\x89PNG\r\n\x1a\n".to_vec()))
        .attach(Attachment::new("notes.txt", "text/plain", b"plain text file".to_vec()))
        .build()
        .unwrap();
    assert_lines(&bytes);
    let message = parse(&bytes).unwrap();
    assert!(message.content_type().is("multipart", "mixed"));
    assert_eq!(message.text_body().unwrap().text().unwrap(), "see attached");
    assert_eq!(message.html_body().unwrap().text().unwrap(), "<p>see attached</p>");
    let attachments = message.attachments();
    assert_eq!(attachments.len(), 2);
    assert_eq!(attachments[0].file_name().as_deref(), Some("kép.png"));
    assert_eq!(attachments[0].content_type().essence(), "image/png");
    assert_eq!(attachments[0].transfer_encoding(), &TransferEncoding::Base64);
    assert_eq!(&*attachments[0].decoded().unwrap(), b"\x89PNG\r\n\x1a\n");
    assert_eq!(attachments[1].file_name().as_deref(), Some("notes.txt"));
    assert_eq!(&*attachments[1].decoded().unwrap(), b"plain text file");
}

#[test]
fn attachments_alone_and_odd_file_names() {
    let long = format!("{}.pdf", "a".repeat(200));
    let bytes = MessageBuilder::new()
        .attach(Attachment::new("", "application/pdf", vec![1, 2, 3]))
        .attach(Attachment::new(&long, "application/pdf", vec![]))
        .attach(Attachment::new(r#"say "hi"\.txt"#, "text/plain", vec![]))
        .attach(Attachment::new("=?utf-8?q?x?=", "text/plain", vec![]))
        .build()
        .unwrap();
    assert_lines(&bytes);
    let message = parse(&bytes).unwrap();
    assert!(message.text_body().is_none());
    let attachments = message.attachments();
    assert_eq!(attachments.len(), 4);
    assert_eq!(attachments[0].file_name(), None);
    assert_eq!(attachments[1].file_name(), Some(long));
    assert_eq!(attachments[2].file_name().as_deref(), Some(r#"say "hi"\.txt"#));
    assert_eq!(attachments[3].file_name().as_deref(), Some("=?utf-8?q?x?="));
}

#[test]
fn the_boundary_is_not_in_the_parts() {
    let text = "--=_inboxcraft_0_0\r\n--=_inboxcraft_0_1--\r\n";
    let bytes = MessageBuilder::new().text(text).attach(Attachment::new("a.bin", "application/octet-stream", vec![0])).build().unwrap();
    let message = parse(&bytes).unwrap();
    assert_eq!(message.text_body().unwrap().text().unwrap(), text);
    assert_eq!(message.attachments().len(), 1);
}

#[test]
fn a_long_subject_without_spaces_is_folded_as_encoded_words() {
    let subject = "x".repeat(2000);
    let bytes = MessageBuilder::new().subject(&subject).build().unwrap();
    assert_lines(&bytes);
    assert!(String::from_utf8_lossy(&bytes).lines().all(|line| line.len() <= 78));
    assert_eq!(parse(&bytes).unwrap().headers().text("subject"), Some(subject));
}

#[test]
fn display_names_that_are_not_plain_are_encoded_words() {
    let long = "x".repeat(100);
    for name in ["Kovács Anna", "=?utf-8?q?x?=", "tab\there", long.as_str()] {
        let bytes = MessageBuilder::new().from(Mailbox::named(name, "a@example.org")).build().unwrap();
        assert_lines(&bytes);
        let message = parse(&bytes).unwrap();
        let from = message.headers().get("from").unwrap();
        assert!(from.starts_with("=?UTF-8?B?") && from.ends_with(" <a@example.org>"), "{from}");
    }
    let bytes = MessageBuilder::new().from(Mailbox::named(" \r\n ", "a@example.org")).build().unwrap();
    assert_eq!(parse(&bytes).unwrap().headers().get("from"), Some("a@example.org"));
}

#[test]
fn addresses_of_every_form_are_written_as_given() {
    for address in
        ["a.b+c@example.org", "\"a b\\\"c\"@example.org", "user@[192.0.2.1]", "ünnep@példa.example", " padded@example.org ", "x@a-b.example"]
    {
        let bytes = MessageBuilder::new().to(Mailbox::new(address)).build().unwrap();
        assert_eq!(parse(&bytes).unwrap().headers().get("to"), Some(address.trim()), "{address}");
    }
}

#[test]
fn a_message_the_reader_could_not_read_back_is_refused() {
    let big = Attachment::new("big.bin", "application/octet-stream", vec![0; MAX_MESSAGE_BYTES / 4 * 3 + 3]);
    assert_eq!(MessageBuilder::new().attach(big).build(), Err(MimeError::TooLarge { limit: MAX_MESSAGE_BYTES }));
    // Zeroed and never touched, so this costs no memory.
    assert_eq!(within_limit(vec![0; MAX_MESSAGE_BYTES + 1]), Err(MimeError::TooLarge { limit: MAX_MESSAGE_BYTES }));
    let subject = ["word"; 60_000].join(" ");
    assert_eq!(MessageBuilder::new().subject(&subject).build(), Err(MimeError::FieldTooLong { limit: crate::MAX_FIELD_BYTES }));
}

// The injection guard: one test per kind of header value. Text for people loses CR, LF and NUL; values with a
// grammar of their own (addresses, message ids, field names, media types) are refused when they hold them.

const BREAKS: [&str; 5] =
    ["\r\nBcc: evil@example.org", "\nBcc: evil@example.org", "\rBcc: evil@example.org", "\0Bcc: evil@example.org", "\r\n\r\n--body"];

/// The built message has exactly the fields named, and no `Bcc`.
fn assert_no_injection(bytes: &[u8]) {
    assert_lines(bytes);
    let message = parse(bytes).unwrap();
    assert!(message.headers().get("bcc").is_none(), "{}", String::from_utf8_lossy(bytes));
    assert!(!String::from_utf8_lossy(bytes).contains("\r\nBcc"), "{}", String::from_utf8_lossy(bytes));
}

#[test]
fn injection_into_the_subject_is_stripped() {
    for evil in BREAKS {
        let bytes = MessageBuilder::new().subject(&format!("Hi{evil}")).build().unwrap();
        assert_no_injection(&bytes);
        let expected = format!("Hi{evil}").replace(['\r', '\n', '\0'], "");
        assert_eq!(parse(&bytes).unwrap().headers().text("subject"), Some(expected));
    }
}

#[test]
fn injection_into_a_custom_field_value_is_stripped() {
    for evil in BREAKS {
        let bytes = MessageBuilder::new().header("X-Note", &format!("note{evil}")).build().unwrap();
        assert_no_injection(&bytes);
        assert_eq!(names(&parse(&bytes).unwrap())[0], "x-note");
    }
}

#[test]
fn injection_into_a_display_name_is_stripped() {
    for evil in BREAKS {
        let name = format!("Anna{evil}");
        let builder = MessageBuilder::new()
            .from(Mailbox::named(&name, "a@example.org"))
            .sender(Mailbox::named(&name, "a@example.org"))
            .reply_to(Mailbox::named(&name, "a@example.org"))
            .to(Mailbox::named(&name, "a@example.org"))
            .cc(Mailbox::named(&name, "a@example.org"));
        assert_no_injection(&builder.build().unwrap());
    }
}

#[test]
fn injection_into_an_address_is_refused() {
    let bad = MimeError::BadHeader("address");
    for evil in BREAKS {
        let address = format!("a@example.org{evil}");
        assert_eq!(MessageBuilder::new().from(Mailbox::new(&address)).build(), Err(bad.clone()));
        assert_eq!(MessageBuilder::new().sender(Mailbox::new(&address)).build(), Err(bad.clone()));
        assert_eq!(MessageBuilder::new().reply_to(Mailbox::new(&address)).build(), Err(bad.clone()));
        assert_eq!(MessageBuilder::new().to(Mailbox::new(&address)).build(), Err(bad.clone()));
        assert_eq!(MessageBuilder::new().cc(Mailbox::named("A", &address)).build(), Err(bad.clone()));
    }
}

#[test]
fn an_address_outside_the_grammar_is_refused() {
    for address in [
        "",
        "no-at-sign",
        "@example.org",
        "a@",
        "a b@example.org",
        "a@example.org>, evil@example.org",
        "<a@example.org>",
        "a,b@example.org",
        ".a@example.org",
        "a..b@example.org",
        "a.@example.org",
        "a@exa mple.org",
        "a@.example.org",
        "a@example..org",
        "a@[1.2.3.4",
        "a@[a]b]",
        "\"a\"b\"@example.org",
        "\"a\\\"@example.org",
        "\"ab@example.org",
        "\"a\u{7}\"@example.org",
        "a\u{202e}b@example.org",
        "a@exam\u{200b}ple.org",
    ] {
        assert_eq!(MessageBuilder::new().to(Mailbox::new(address)).build(), Err(MimeError::BadHeader("address")), "{address:?}");
    }
    let long = format!("{}@example.org", "a".repeat(250));
    assert_eq!(MessageBuilder::new().to(Mailbox::new(&long)).build(), Err(MimeError::BadHeader("address")));
}

#[test]
fn injection_into_a_message_id_is_refused() {
    let bad = MimeError::BadHeader("message id");
    for evil in BREAKS {
        let id = format!("<1@example.org>{evil}");
        assert_eq!(MessageBuilder::new().message_id(&id).build(), Err(bad.clone()));
        assert_eq!(MessageBuilder::new().in_reply_to(&id).build(), Err(bad.clone()));
        assert_eq!(MessageBuilder::new().reference(&id).build(), Err(bad.clone()));
    }
}

#[test]
fn a_message_id_outside_the_grammar_is_refused() {
    for id in ["", "<>", "no-at", "a@b@c", "<a b@example.org>", "a@example.org>", "<a@[x]y]>", "a@[x\\]", "a..b@example.org", "é@example.org"] {
        assert_eq!(MessageBuilder::new().message_id(id).build(), Err(MimeError::BadHeader("message id")), "{id:?}");
    }
    let long = format!("{}@example.org", "a".repeat(250));
    assert_eq!(MessageBuilder::new().message_id(&long).build(), Err(MimeError::BadHeader("message id")));
    let bytes = MessageBuilder::new().message_id("a.b@[192.0.2.1]").build().unwrap();
    assert_eq!(parse(&bytes).unwrap().headers().get("message-id"), Some("<a.b@[192.0.2.1]>"));
}

#[test]
fn injection_into_a_field_name_is_refused() {
    for evil in BREAKS {
        let name = format!("X-Note{evil}");
        assert_eq!(MessageBuilder::new().header(&name, "x").build(), Err(MimeError::BadHeader("field name")));
    }
}

#[test]
fn a_field_name_outside_the_grammar_or_one_the_builder_writes_is_refused() {
    let long = "X".repeat(77);
    for name in ["", "X Note", "X:Note", "Ünnep", long.as_str(), "Subject", "BCC", "content-type", "Content-Whatever", "MIME-Version", "from"] {
        assert_eq!(MessageBuilder::new().header(name, "x").build(), Err(MimeError::BadHeader("field name")), "{name:?}");
    }
}

#[test]
fn injection_into_a_file_name_is_stripped() {
    for evil in BREAKS {
        let name = format!("a{evil}.txt");
        let bytes = MessageBuilder::new().attach(Attachment::new(&name, "text/plain", vec![])).build().unwrap();
        assert_no_injection(&bytes);
        let message = parse(&bytes).unwrap();
        assert_eq!(message.attachments()[0].file_name(), Some(name.replace(['\r', '\n', '\0'], "")));
    }
}

#[test]
fn injection_into_a_media_type_is_refused() {
    for evil in BREAKS {
        let media_type = format!("text/plain{evil}");
        assert_eq!(MessageBuilder::new().attach(Attachment::new("a", &media_type, vec![])).build(), Err(MimeError::BadHeader("media type")));
    }
    for media_type in ["", "text", "text/", "/plain", "te xt/plain", "text/plain; charset=x", "multipart/mixed", "text/pl\u{e1}in"] {
        let attachment = Attachment::new("a", media_type, vec![]);
        assert_eq!(MessageBuilder::new().attach(attachment).build(), Err(MimeError::BadHeader("media type")), "{media_type:?}");
    }
}

#[test]
fn a_media_type_with_a_name_over_127_characters_is_refused() {
    let long = "x".repeat(128);
    for media_type in [format!("application/{long}"), format!("{long}/plain"), format!("application/{}", "x".repeat(2000))] {
        let attachment = Attachment::new("a.bin", &media_type, vec![]);
        assert_eq!(MessageBuilder::new().attach(attachment).build(), Err(MimeError::BadHeader("media type")));
    }
    let longest = format!("{}/{}", "a".repeat(127), "b".repeat(127));
    let bytes = MessageBuilder::new().attach(Attachment::new("a.bin", &longest, vec![])).build().unwrap();
    assert!(bytes.split(|b| *b == b'\n').all(|line| line.len() <= 1000));
    assert_eq!(parse(&bytes).unwrap().attachments()[0].content_type().essence(), longest);
}

#[test]
fn an_attached_message_type_other_than_message_global_is_refused() {
    for media_type in ["message/rfc822", "Message/RFC822", "message/partial", "message/external-body", "message/delivery-status"] {
        let attachment = Attachment::new("fwd.eml", media_type, b"Subject: hi\r\n\r\nhi\r\n".to_vec());
        assert_eq!(MessageBuilder::new().attach(attachment).build(), Err(MimeError::BadHeader("media type")), "{media_type:?}");
    }
    let bytes = MessageBuilder::new().attach(Attachment::new("fwd.eml", "message/global", b"hi".to_vec())).build().unwrap();
    assert_eq!(parse(&bytes).unwrap().attachments()[0].decoded().unwrap().as_ref(), b"hi");
}
