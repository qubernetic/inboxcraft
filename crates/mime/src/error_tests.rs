// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::MimeError;
use inboxcraft_charset::Charset;

#[test]
fn every_error_says_what_went_wrong() {
    let charset = Charset::for_label("x-unknown").unwrap_err();
    let cases = [
        (MimeError::TooLarge { limit: 9 }, "the message is longer than 9 bytes"),
        (MimeError::TooManyFields { limit: 9 }, "a header has more than 9 fields"),
        (MimeError::FieldTooLong { limit: 9 }, "a header field is longer than 9 bytes"),
        (MimeError::TooDeep { limit: 9 }, "the message parts are nested deeper than 9"),
        (MimeError::TooManyParts { limit: 9 }, "the message has more than 9 parts"),
        (MimeError::MissingBoundary, "a multipart has no boundary"),
        (MimeError::BadBoundary, "a multipart boundary is not valid"),
        (MimeError::BoundaryNotFound, "a multipart boundary is not in its body"),
        (MimeError::UnknownEncoding("x-uue".to_owned()), "unknown transfer encoding \"x-uue\""),
        (MimeError::BadBase64, "a base64 body is broken"),
        (MimeError::from(charset.clone()), &charset.to_string()),
        (MimeError::NotAReport, "the message is no delivery report"),
        (MimeError::MalformedReport, "a delivery report has a line that is no field"),
        (MimeError::MissingField("Action"), "a delivery report lacks the Action field"),
        (MimeError::BadField("Status"), "a delivery report has a bad Status field"),
        (MimeError::TooManyRecipients { limit: 9 }, "a delivery report names more than 9 recipients"),
        (MimeError::BadHeader("address"), "the address cannot be written into a header"),
    ];
    for (error, text) in cases {
        assert_eq!(error.to_string(), text);
    }
    let error: Box<dyn std::error::Error> = Box::new(MimeError::BadBase64);
    assert!(error.source().is_none());
}
