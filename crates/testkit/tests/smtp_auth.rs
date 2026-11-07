// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The fake SMTP server's AUTH (RFC 4954): PLAIN with and without an initial response, LOGIN, wrong credentials, a
//! cancelled exchange and AUTH out of place.

use inboxcraft_testkit::smtp::{Extension, FakeSmtp};

/// `\0ada@example.org\0not-a-real-password` in base64.
const PLAIN: &str = "AGFkYUBleGFtcGxlLm9yZwBub3QtYS1yZWFsLXBhc3N3b3Jk";
/// `\0ada@example.org\0wrong` in base64.
const PLAIN_WRONG: &str = "AGFkYUBleGFtcGxlLm9yZwB3cm9uZw==";
/// `bob@example.org\0ada@example.org\0not-a-real-password` in base64: someone else's authorisation identity.
const PLAIN_OTHER_AUTHZ: &str = "Ym9iQGV4YW1wbGUub3JnAGFkYUBleGFtcGxlLm9yZwBub3QtYS1yZWFsLXBhc3N3b3Jk";
/// `ada@example.org` in base64.
const LOGIN_USER: &str = "YWRhQGV4YW1wbGUub3Jn";
/// `not-a-real-password` in base64.
const LOGIN_PASSWORD: &str = "bm90LWEtcmVhbC1wYXNzd29yZA==";

/// The server's lines after the greeting and EHLO, for `input`.
fn after_ehlo(fake: &FakeSmtp, input: &str) -> Vec<String> {
    let out = String::from_utf8_lossy(&fake.answer(format!("EHLO c.example.org\r\n{input}").as_bytes())).into_owned();
    out.split_terminator("\r\n").skip_while(|line| !line.starts_with("250 ")).skip(1).map(str::to_owned).collect()
}

#[test]
fn plain_with_an_initial_response_signs_in() {
    let out = after_ehlo(&FakeSmtp::new(), &format!("AUTH PLAIN {PLAIN}\r\nAUTH PLAIN {PLAIN}\r\n"));
    assert_eq!(out, ["235 2.7.0 Authentication successful", "503 5.5.1 Already signed in"]);
}

#[test]
fn plain_after_a_challenge_signs_in() {
    let out = after_ehlo(&FakeSmtp::new(), &format!("auth plain\r\n{PLAIN}\r\n"));
    assert_eq!(out, ["334 ", "235 2.7.0 Authentication successful"]);
}

#[test]
fn login_asks_for_the_user_and_the_password() {
    let out = after_ehlo(&FakeSmtp::new(), &format!("AUTH LOGIN\r\n{LOGIN_USER}\r\n{LOGIN_PASSWORD}\r\n"));
    assert_eq!(out, ["334 VXNlcm5hbWU6", "334 UGFzc3dvcmQ6", "235 2.7.0 Authentication successful"]);
    let out = after_ehlo(&FakeSmtp::new(), &format!("AUTH LOGIN {LOGIN_USER}\r\n{LOGIN_PASSWORD}\r\n"));
    assert_eq!(out, ["334 UGFzc3dvcmQ6", "235 2.7.0 Authentication successful"]);
}

#[test]
fn wrong_credentials_are_refused() {
    let wrong = "535 5.7.8 Authentication credentials invalid";
    let out = after_ehlo(
        &FakeSmtp::new(),
        &format!("AUTH PLAIN {PLAIN_WRONG}\r\nAUTH PLAIN {PLAIN_OTHER_AUTHZ}\r\nAUTH PLAIN =\r\nAUTH LOGIN {LOGIN_USER}\r\n=\r\n"),
    );
    assert_eq!(out, [wrong, wrong, wrong, "334 UGFzc3dvcmQ6", wrong]);
    let out = after_ehlo(&FakeSmtp::new().login("bea@example.org", "another-invented-one"), &format!("AUTH PLAIN {PLAIN}\r\n"));
    assert_eq!(out, [wrong]);
}

#[test]
fn a_cancelled_or_unreadable_response_ends_the_exchange() {
    let cancelled = "501 5.7.0 Authentication cancelled";
    let unreadable = "501 5.5.2 Cannot decode the response";
    let out = after_ehlo(
        &FakeSmtp::new(),
        "AUTH PLAIN\r\n*\r\nAUTH PLAIN not!base64\r\nAUTH LOGIN\r\n*\r\nAUTH LOGIN\r\nYWRh\r\n*\r\nAUTH LOGIN ***\r\nAUTH LOGIN\r\nx\r\n",
    );
    assert_eq!(
        out,
        [
            "334 ",
            cancelled,
            unreadable,
            "334 VXNlcm5hbWU6",
            cancelled,
            "334 VXNlcm5hbWU6",
            "334 UGFzc3dvcmQ6",
            cancelled,
            unreadable,
            "334 VXNlcm5hbWU6",
            unreadable
        ]
    );
}

#[test]
fn a_mechanism_not_offered_is_refused() {
    let refused = "504 5.5.4 Unrecognised authentication type";
    let out = after_ehlo(&FakeSmtp::new(), "AUTH CRAM-MD5\r\n");
    assert_eq!(out, [refused]);
    let out = after_ehlo(&FakeSmtp::new().without(Extension::AuthPlain), &format!("AUTH PLAIN {PLAIN}\r\n"));
    assert_eq!(out, [refused]);
    let out = after_ehlo(&FakeSmtp::new().without(Extension::AuthLogin), "AUTH LOGIN\r\n");
    assert_eq!(out, [refused]);
}

#[test]
fn auth_is_refused_in_a_mail_transaction() {
    let fake = FakeSmtp::new().without(Extension::AuthPlain).without(Extension::AuthLogin);
    let out = after_ehlo(&fake, "MAIL FROM:<ada@example.org>\r\nAUTH PLAIN\r\n");
    assert_eq!(out, ["250 2.1.0 Sender OK", "503 5.5.1 AUTH is not allowed in a mail transaction"]);
}

#[test]
fn a_client_that_goes_during_the_exchange_ends_the_session() {
    for input in ["AUTH PLAIN\r\n", "AUTH LOGIN\r\n", "AUTH LOGIN\r\nYWRh\r\n"] {
        let out = after_ehlo(&FakeSmtp::new(), input);
        assert!(out.last().is_some_and(|line| line.starts_with("334 ")), "{out:?}");
    }
    let long = "A".repeat(9000);
    let out = after_ehlo(&FakeSmtp::new(), &format!("AUTH PLAIN\r\n{long}\r\n"));
    assert_eq!(out.last().map(String::as_str), Some("421 4.7.0 Line too long"));
}
