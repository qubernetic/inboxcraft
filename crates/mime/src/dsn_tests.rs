// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{Action, DeliveryStatus, MAX_RECIPIENTS, RecipientStatus, Status, TypedValue};
use crate::{MimeError, parse};

fn eml(lines: &[&str]) -> Vec<u8> {
    lines.iter().flat_map(|line| [line.as_bytes(), b"\r\n"].concat()).collect()
}

fn typed(kind: &str, value: &str) -> TypedValue {
    TypedValue { kind: kind.to_owned(), value: value.to_owned() }
}

fn status(class: u8, subject: u16, detail: u16) -> Status {
    Status { class, subject, detail }
}

/// A bounce for two recipients, one failed and one delayed, as a mail server sends it.
fn bounce() -> Vec<u8> {
    eml(&[
        "From: Mail Delivery System <mailer-daemon@mx.example.org>",
        "To: anna@example.org",
        "Subject: Undelivered Mail Returned to Sender",
        "Content-Type: multipart/report; report-type=delivery-status; boundary=\"r\"",
        "",
        "--r",
        "Content-Type: text/plain",
        "",
        "Your message could not be delivered.",
        "--r",
        "Content-Type: message/delivery-status",
        "",
        "Reporting-MTA: dns; mx.example.org",
        "Original-Envelope-Id: env-42",
        "Received-From-MTA: dns; client.example.org",
        "Arrival-Date: Wed, 4 Nov 2026 10:00:00 +0000",
        "",
        "Final-Recipient: rfc822; bela@example.net",
        "Original-Recipient: rfc822;Bela@example.net",
        "Action: failed",
        "Status: 5.1.1 (user unknown)",
        "Remote-MTA: dns; mx.example.net",
        "Diagnostic-Code: smtp; 550 5.1.1 <bela@example.net>:",
        "  Recipient address rejected",
        "Last-Attempt-Date: Wed, 4 Nov 2026 10:00:05 +0000",
        "",
        "",
        "Final-Recipient: RFC822; csilla@example.net",
        "Action: Delayed (still trying)",
        "Status: 4.4.1",
        "Will-Retry-Until: Fri, 6 Nov 2026 10:00:00 +0000",
        "",
        "--r",
        "Content-Type: text/rfc822-headers",
        "",
        "Subject: Hello",
        "--r--",
    ])
}

#[test]
fn a_bounce_gives_its_delivery_status() {
    let bytes = bounce();
    let message = parse(&bytes).unwrap();
    let report = message.delivery_status().unwrap();
    assert_eq!(report.reporting_mta, Some(typed("dns", "mx.example.org")));
    assert_eq!(report.original_envelope_id.as_deref(), Some("env-42"));
    assert_eq!(report.received_from_mta, Some(typed("dns", "client.example.org")));
    assert_eq!(report.arrival_date.as_deref(), Some("Wed, 4 Nov 2026 10:00:00 +0000"));
    assert_eq!(
        report.recipients,
        [
            RecipientStatus {
                final_recipient: typed("rfc822", "bela@example.net"),
                original_recipient: Some(typed("rfc822", "Bela@example.net")),
                action: Action::Failed,
                status: status(5, 1, 1),
                remote_mta: Some(typed("dns", "mx.example.net")),
                diagnostic_code: Some(typed("smtp", "550 5.1.1 <bela@example.net>:  Recipient address rejected")),
                last_attempt_date: Some("Wed, 4 Nov 2026 10:00:05 +0000".to_owned()),
                will_retry_until: None,
            },
            RecipientStatus {
                final_recipient: typed("rfc822", "csilla@example.net"),
                original_recipient: None,
                action: Action::Delayed,
                status: status(4, 4, 1),
                remote_mta: None,
                diagnostic_code: None,
                last_attempt_date: None,
                will_retry_until: Some("Fri, 6 Nov 2026 10:00:00 +0000".to_owned()),
            },
        ]
    );
}

#[test]
fn a_global_delivery_status_is_read_too() {
    let bytes = eml(&[
        "Content-Type: multipart/report; report-type=global-delivery-status; boundary=r",
        "",
        "--r",
        "Content-Type: message/global-delivery-status",
        "",
        "Reporting-MTA: dns; mx.example.org",
        "",
        "Final-Recipient: utf-8; józsef@example.org",
        "Action: delivered",
        "Status: 2.0.0",
        "--r--",
    ]);
    let report = parse(&bytes).unwrap().delivery_status().unwrap();
    assert_eq!(report.recipients[0].final_recipient, typed("utf-8", "józsef@example.org"));
    assert_eq!(report.recipients[0].action, Action::Delivered);
}

#[test]
fn a_message_that_is_no_report_has_no_delivery_status() {
    assert_eq!(parse(b"Subject: x\r\n\r\nbody").unwrap().delivery_status(), Err(MimeError::NotAReport));
    let receipt = eml(&[
        "Content-Type: multipart/report; report-type=disposition-notification; boundary=r",
        "",
        "--r",
        "Content-Type: message/disposition-notification",
        "",
        "Final-Recipient: rfc822; anna@example.org",
        "--r--",
    ]);
    assert_eq!(parse(&receipt).unwrap().delivery_status(), Err(MimeError::NotAReport));
}

#[test]
fn a_report_inside_an_attached_message_is_not_read() {
    let bytes = eml(&[
        "Content-Type: multipart/report; report-type=delivery-status; boundary=outer",
        "",
        "--outer",
        "Content-Type: text/plain",
        "",
        "See the attached message.",
        "--outer",
        "Content-Type: message/rfc822",
        "",
        "Content-Type: multipart/report; report-type=delivery-status; boundary=inner",
        "",
        "--inner",
        "Content-Type: message/delivery-status",
        "",
        "Reporting-MTA: dns; evil.example.org",
        "",
        "Final-Recipient: rfc822; anna@example.org",
        "Action: delivered",
        "Status: 2.0.0",
        "--inner--",
        "--outer--",
    ]);
    let message = parse(&bytes).unwrap();
    assert_eq!(message.delivery_status(), Err(MimeError::NotAReport));
}

#[test]
fn a_status_in_a_nested_multipart_of_the_report_is_not_read() {
    let bytes = eml(&[
        "Content-Type: multipart/report; report-type=delivery-status; boundary=outer",
        "",
        "--outer",
        "Content-Type: multipart/mixed; boundary=inner",
        "",
        "--inner",
        "Content-Type: message/delivery-status",
        "",
        "Reporting-MTA: dns; mx.example.org",
        "",
        "Final-Recipient: rfc822; anna@example.org",
        "Action: failed",
        "Status: 5.1.1",
        "--inner--",
        "--outer--",
    ]);
    assert_eq!(parse(&bytes).unwrap().delivery_status(), Err(MimeError::NotAReport));
}

#[test]
fn a_delivery_status_in_base64_is_decoded_first() {
    let bytes = eml(&[
        "Content-Type: multipart/report; boundary=r",
        "",
        "--r",
        "Content-Type: message/delivery-status",
        "Content-Transfer-Encoding: base64",
        "",
        "UmVwb3J0aW5nLU1UQTogZG5zOyBteAoKRmluYWwtUmVjaXBpZW50OiByZmM4MjI7IGFAZXhhbXBsZS5vcmcKQWN0aW9uOiByZWxheWVkClN0YXR1czogMi4wLjAK",
        "--r",
        "Content-Type: message/delivery-status",
        "Content-Transfer-Encoding: base64",
        "",
        "!!",
        "--r--",
    ]);
    let report = parse(&bytes).unwrap().delivery_status().unwrap();
    assert_eq!(report.reporting_mta, Some(typed("dns", "mx")));
    assert_eq!(report.recipients[0].action, Action::Relayed);
}

#[test]
fn a_broken_delivery_status_part_is_an_error() {
    let bytes = eml(&[
        "Content-Type: multipart/report; boundary=r",
        "",
        "--r",
        "Content-Type: message/delivery-status",
        "Content-Transfer-Encoding: base64",
        "",
        "!!",
        "--r--",
    ]);
    assert_eq!(parse(&bytes).unwrap().delivery_status(), Err(MimeError::BadBase64));
}

#[test]
fn every_action_is_read() {
    for (text, action) in [
        ("failed", Action::Failed),
        ("DELAYED", Action::Delayed),
        ("delivered", Action::Delivered),
        ("relayed", Action::Relayed),
        ("expanded", Action::Expanded),
    ] {
        let report =
            DeliveryStatus::parse(format!("\r\nFinal-Recipient: rfc822; a@example.org\r\nAction: {text}\r\nStatus: 2.0.0\r\n").as_bytes()).unwrap();
        assert_eq!(report.recipients[0].action, action);
        assert_eq!(report.reporting_mta, None);
    }
}

#[test]
fn a_status_says_what_kind_of_result_it_is() {
    assert!(status(5, 1, 1).is_permanent_failure());
    assert!(status(4, 4, 1).is_temporary_failure());
    assert!(status(2, 0, 0).is_success());
    assert!(!status(5, 0, 0).is_success() && !status(5, 0, 0).is_temporary_failure());
    assert!(!status(2, 0, 0).is_permanent_failure());
    assert_eq!(status(5, 7, 26).to_string(), "5.7.26");
    assert_eq!(Status::parse(" 5.7.26 (spf)"), Some(status(5, 7, 26)));
    for bad in ["", "5.1", "5.1.1.1", "3.1.1", "5.x.1", "5.1000.1", "5.1.1000", "5..1", "-5.1.1", "5.1.+1"] {
        assert_eq!(Status::parse(bad), None, "{bad:?}");
    }
}

fn recipient_error(fields: &str) -> MimeError {
    DeliveryStatus::parse(format!("Reporting-MTA: dns; mx\r\n\r\n{fields}").as_bytes()).unwrap_err()
}

#[test]
fn a_recipient_without_a_field_it_must_have_is_an_error() {
    assert_eq!(recipient_error("Action: failed\r\nStatus: 5.0.0\r\n"), MimeError::MissingField("Final-Recipient"));
    assert_eq!(recipient_error("Final-Recipient: rfc822; a@example.org\r\nStatus: 5.0.0\r\n"), MimeError::MissingField("Action"));
    assert_eq!(recipient_error("Final-Recipient: rfc822; a@example.org\r\nAction: failed\r\n"), MimeError::MissingField("Status"));
    assert_eq!(DeliveryStatus::parse(b"Reporting-MTA: dns; mx\r\n\r\n"), Err(MimeError::MissingField("Final-Recipient")));
    assert_eq!(DeliveryStatus::parse(b""), Err(MimeError::MissingField("Final-Recipient")));
}

#[test]
fn a_field_that_cannot_be_read_is_an_error() {
    let fields = |action: &str, status: &str, recipient: &str| format!("Final-Recipient: {recipient}\r\nAction: {action}\r\nStatus: {status}\r\n");
    assert_eq!(recipient_error(&fields("bounced", "5.0.0", "rfc822; a@example.org")), MimeError::BadField("Action"));
    assert_eq!(recipient_error(&fields("failed", "550", "rfc822; a@example.org")), MimeError::BadField("Status"));
    assert_eq!(recipient_error(&fields("failed", "5.0.0", "a@example.org")), MimeError::BadField("Final-Recipient"));
    assert_eq!(recipient_error(&fields("failed", "5.0.0", "; a@example.org")), MimeError::BadField("Final-Recipient"));
    assert_eq!(DeliveryStatus::parse(b"Reporting-MTA: mx\r\n\r\n"), Err(MimeError::BadField("Reporting-MTA")));
    let remote = format!("{}Remote-MTA: nothing\r\n", fields("failed", "5.0.0", "rfc822; a@example.org"));
    assert_eq!(recipient_error(&remote), MimeError::BadField("Remote-MTA"));
}

#[test]
fn a_line_that_is_no_field_is_an_error() {
    assert_eq!(recipient_error("this is prose\r\n"), MimeError::MalformedReport);
    assert_eq!(DeliveryStatus::parse(b"prose first\r\n"), Err(MimeError::MalformedReport));
}

#[test]
fn typed_values_are_cleaned() {
    let report = DeliveryStatus::parse(b"\r\nFinal-Recipient: RFC822 ;\ta@example.org\x01\r\nAction: failed\r\nStatus: 5.0.0\r\n").unwrap();
    assert_eq!(report.recipients[0].final_recipient, typed("rfc822", "a@example.org"));
}

#[test]
fn recipients_are_capped() {
    let one = "Final-Recipient: rfc822; a@example.org\r\nAction: failed\r\nStatus: 5.0.0\r\n\r\n";
    let report = DeliveryStatus::parse(format!("Reporting-MTA: dns; mx\r\n\r\n{}", one.repeat(MAX_RECIPIENTS)).as_bytes()).unwrap();
    assert_eq!(report.recipients.len(), MAX_RECIPIENTS);
    let too_many = format!("Reporting-MTA: dns; mx\r\n\r\n{}", one.repeat(MAX_RECIPIENTS + 1));
    assert_eq!(DeliveryStatus::parse(too_many.as_bytes()), Err(MimeError::TooManyRecipients { limit: MAX_RECIPIENTS }));
}

#[test]
fn a_header_error_in_a_report_is_an_error() {
    let long = format!("Reporting-MTA: dns; {}\r\n", "a".repeat(crate::MAX_FIELD_BYTES));
    assert_eq!(DeliveryStatus::parse(long.as_bytes()), Err(MimeError::FieldTooLong { limit: crate::MAX_FIELD_BYTES }));
    let long = format!("Reporting-MTA: dns; mx\r\n\r\nX: {}\r\n", "a".repeat(crate::MAX_FIELD_BYTES));
    assert_eq!(DeliveryStatus::parse(long.as_bytes()), Err(MimeError::FieldTooLong { limit: crate::MAX_FIELD_BYTES }));
}

/// A report about the message `<returned.1@example.org>`, its third part of `media_type` with `headers`.
fn returned(media_type: &str, headers: &[&str]) -> Vec<u8> {
    let mut lines = vec![
        "From: mailer-daemon@mx.example.org",
        "Content-Type: multipart/report; report-type=delivery-status; boundary=r",
        "",
        "--r",
        "Content-Type: message/delivery-status",
        "",
        "Final-Recipient: rfc822; bela@example.net",
        "Action: failed",
        "Status: 5.1.1",
        "",
        "--r",
    ];
    let content_type = format!("Content-Type: {media_type}");
    lines.push(&content_type);
    lines.push("");
    lines.extend(headers);
    lines.push("--r--");
    eml(&lines)
}

#[test]
fn a_report_names_the_message_it_is_about_by_its_returned_header() {
    let headers = ["Subject: Hello", "Message-ID:", " <returned.1@example.org>", "From: anna@example.org", ""];
    for media_type in ["text/rfc822-headers", "message/rfc822-headers", "message/global-headers", "message/rfc822", "message/global"] {
        let bytes = returned(media_type, &headers);
        assert_eq!(parse(&bytes).unwrap().reported_message_id().as_deref(), Some("<returned.1@example.org>"), "{media_type}");
    }
    // The whole message, its body after the header.
    let whole = returned("message/rfc822", &["Message-ID: <returned.1@example.org>", "", "Hello"]);
    assert_eq!(parse(&whole).unwrap().reported_message_id().as_deref(), Some("<returned.1@example.org>"));
    // The fixture's returned header has no Message-ID.
    assert_eq!(parse(&bounce()).unwrap().reported_message_id(), None);
}

#[test]
fn a_report_without_a_returned_header_names_no_message() {
    // Another media type in the third place.
    let other = returned("text/plain", &["Message-ID: <returned.1@example.org>", ""]);
    assert_eq!(parse(&other).unwrap().reported_message_id(), None);
    // No report at all.
    assert_eq!(parse(b"Message-ID: <x@example.org>\r\n\r\nbody").unwrap().reported_message_id(), None);
    // A multipart that is no report.
    let mixed = eml(&[
        "Content-Type: multipart/mixed; boundary=r",
        "",
        "--r",
        "Content-Type: text/rfc822-headers",
        "",
        "Message-ID: <x@example.org>",
        "--r--",
    ]);
    assert_eq!(parse(&mixed).unwrap().reported_message_id(), None);
    // A returned header that cannot be read.
    let broken = returned("text/rfc822-headers", &["Content-Transfer-Encoding: base64", "not a header line", ""]);
    assert_eq!(parse(&broken).unwrap().reported_message_id(), None);
    let encoded = eml(&[
        "Content-Type: multipart/report; report-type=delivery-status; boundary=r",
        "",
        "--r",
        "Content-Type: text/rfc822-headers",
        "Content-Transfer-Encoding: base64",
        "",
        "!!!",
        "--r--",
    ]);
    assert_eq!(parse(&encoded).unwrap().reported_message_id(), None);
}
