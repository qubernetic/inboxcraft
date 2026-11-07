// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::DavError;

#[test]
fn every_error_says_what_went_wrong() {
    let all = [
        (DavError::NotEncrypted, "a password goes over an encrypted connection only"),
        (DavError::BadUrl("a user name"), "the URL cannot be used: a user name"),
        (DavError::BadArgument("a line break"), "the argument cannot be sent: a line break"),
        (DavError::OtherOrigin { url: "https://other.example.org/".into() }, "the server points to another server: https://other.example.org/"),
        (DavError::TooManyRedirects, "the server redirects too often"),
        (DavError::Unauthorized, "the server did not take the user name and password"),
        (DavError::Refused { status: 500 }, "the server answered 500"),
        (DavError::Missing("principal"), "the server's answer lacks the principal"),
        (DavError::BadResponse("no status line"), "the server's answer is not HTTP: no status line"),
        (DavError::BadXml("an unclosed element"), "the server's XML cannot be read: an unclosed element"),
        (DavError::TooLarge { what: "body", limit: 3 }, "the server's body is larger than 3"),
        (DavError::Finished, "the exchange is over"),
    ];
    for (error, shown) in all {
        assert_eq!(error.to_string(), shown);
        let _: &dyn std::error::Error = &error;
    }
}
