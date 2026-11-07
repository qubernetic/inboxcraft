// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The fake IMAP server without a socket: greeting, capabilities, sign-in and its own command reader.

use inboxcraft_testkit::imap::{Capability, FakeImap, PASSWORD, USER};

/// What the fake answers to `input`, line by line.
fn lines(fake: &FakeImap, input: &str) -> Vec<String> {
    let out = String::from_utf8_lossy(&fake.answer(input.as_bytes())).into_owned();
    out.split_terminator("\r\n").map(str::to_owned).collect()
}

const ALL: &str = "IMAP4rev1 AUTH=PLAIN SASL-IR LITERAL+ ENABLE CONDSTORE QRESYNC IDLE NOTIFY SPECIAL-USE UIDPLUS MOVE PREVIEW COMPRESS=DEFLATE";
const SIGNED_IN: &str = "IMAP4rev1 LITERAL+ ENABLE CONDSTORE QRESYNC IDLE NOTIFY SPECIAL-USE UIDPLUS MOVE PREVIEW COMPRESS=DEFLATE";

#[test]
fn the_greeting_and_capability_list_every_capability() {
    let got = lines(&FakeImap::new(), "a1 CAPABILITY\r\n");
    assert_eq!(
        got,
        [format!("* OK [CAPABILITY {ALL}] InboxCraft fake IMAP ready"), format!("* CAPABILITY {ALL}"), "a1 OK CAPABILITY completed".to_owned(),]
    );
}

#[test]
fn a_capability_switched_off_is_not_offered_and_its_command_is_refused() {
    let fake = FakeImap::new().without(Capability::AuthPlain).without(Capability::SaslIr).without(Capability::Move);
    let got = lines(&fake, "a1 CAPABILITY\r\na2 AUTHENTICATE PLAIN\r\n");
    let offered = "IMAP4rev1 LITERAL+ ENABLE CONDSTORE QRESYNC IDLE NOTIFY SPECIAL-USE UIDPLUS PREVIEW COMPRESS=DEFLATE";
    assert_eq!(got.get(1).map(String::as_str), Some(format!("* CAPABILITY {offered}").as_str()));
    assert_eq!(got.get(3).map(String::as_str), Some("a2 BAD AUTHENTICATE PLAIN is not offered"));
    let back = FakeImap::new().without(Capability::Move).with(Capability::Move);
    assert_eq!(lines(&back, "").first().map(String::as_str), Some(format!("* OK [CAPABILITY {ALL}] InboxCraft fake IMAP ready").as_str()));
}

#[test]
fn every_capability_has_its_name() {
    let names: Vec<&str> = Capability::ALL.iter().map(|cap| cap.name()).collect();
    assert_eq!(
        names.join(" "),
        "STARTTLS LOGINDISABLED AUTH=PLAIN SASL-IR LITERAL+ ENABLE CONDSTORE QRESYNC IDLE NOTIFY SPECIAL-USE UIDPLUS MOVE PREVIEW COMPRESS=DEFLATE"
    );
}

#[test]
fn login_takes_the_right_password_only() {
    let input = format!("a1 LOGIN {USER} wrong\r\na2 LOGIN \"{USER}\" \"{PASSWORD}\"\r\na3 LOGIN {USER} {PASSWORD}\r\n");
    let got = lines(&FakeImap::new(), &input);
    assert_eq!(
        got.get(1..),
        Some(
            [
                "a1 NO [AUTHENTICATIONFAILED] Invalid credentials".to_owned(),
                format!("a2 OK [CAPABILITY {SIGNED_IN}] Logged in"),
                "a3 BAD LOGIN is not available now".to_owned(),
            ]
            .as_slice()
        )
    );
}

#[test]
fn login_reads_literals_and_quoted_escapes() {
    let fake = FakeImap::new().login("ada@example.org", "pass \"word\" \\ jelszó");
    let input = format!("a1 LOGIN {{15}}\r\nada@example.org {{{}}}\r\n{}\r\n", "pass \"word\" \\ jelszó".len(), "pass \"word\" \\ jelszó");
    let got = lines(&fake, &input);
    assert_eq!(got.get(1..3), Some(["+ Ready for the literal".to_owned(), "+ Ready for the literal".to_owned()].as_slice()));
    assert!(got.get(3).is_some_and(|line| line.starts_with("a1 OK [CAPABILITY")), "{got:?}");
    let quoted = lines(&fake, "a1 LOGIN ada@example.org \"pass \\\"word\\\" \\\\ jelszó\"\r\n");
    assert!(quoted.get(1).is_some_and(|line| line.starts_with("a1 OK [CAPABILITY")), "{quoted:?}");
}

#[test]
fn a_non_synchronising_literal_needs_no_go_ahead() {
    let got = lines(&FakeImap::new(), &format!("a1 LOGIN {{15+}}\r\n{USER} {PASSWORD}\r\n"));
    assert_eq!(got.len(), 2, "{got:?}");
    assert!(got.get(1).is_some_and(|line| line.starts_with("a1 OK [CAPABILITY")), "{got:?}");
    let off = lines(&FakeImap::new().without(Capability::LiteralPlus), &format!("a1 LOGIN {{15+}}\r\n{USER} {PASSWORD}\r\n"));
    assert_eq!(off.get(1..), Some(["* BYE The literal is refused".to_owned()].as_slice()));
}

#[test]
fn authenticate_plain_with_an_initial_response() {
    // "\0ada@example.org\0not-a-real-password"
    let ok = "AGFkYUBleGFtcGxlLm9yZwBub3QtYS1yZWFsLXBhc3N3b3Jk";
    let got = lines(&FakeImap::new(), &format!("a1 AUTHENTICATE PLAIN {ok}\r\na2 AUTHENTICATE plain {ok}\r\n"));
    assert_eq!(got.get(1), Some(&format!("a1 OK [CAPABILITY {SIGNED_IN}] Logged in")));
    assert_eq!(got.get(2).map(String::as_str), Some("a2 BAD AUTHENTICATE is not available now"));
    // "ada@example.org\0ada@example.org\0not-a-real-password": the same user as authorisation identity
    let same = "YWRhQGV4YW1wbGUub3JnAGFkYUBleGFtcGxlLm9yZwBub3QtYS1yZWFsLXBhc3N3b3Jk";
    assert!(lines(&FakeImap::new(), &format!("a1 AUTHENTICATE PLAIN {same}\r\n")).get(1).is_some_and(|line| line.starts_with("a1 OK")));
}

#[test]
fn authenticate_plain_asks_for_the_response_without_sasl_ir() {
    let ok = "AGFkYUBleGFtcGxlLm9yZwBub3QtYS1yZWFsLXBhc3N3b3Jk";
    let fake = FakeImap::new().without(Capability::SaslIr);
    let got = lines(&fake, &format!("a1 AUTHENTICATE PLAIN\r\n{ok}\r\n"));
    assert_eq!(got.get(1).map(String::as_str), Some("+ "));
    assert!(got.get(2).is_some_and(|line| line.starts_with("a1 OK [CAPABILITY")), "{got:?}");
    let with_ir = lines(&fake, &format!("a1 AUTHENTICATE PLAIN {ok}\r\n"));
    assert_eq!(with_ir.get(1).map(String::as_str), Some("a1 BAD SASL-IR is not offered"));
}

#[test]
fn authenticate_plain_refuses_bad_responses() {
    let cases = [
        ("*", "a1 BAD Authentication cancelled"),
        ("=", "a1 NO [AUTHENTICATIONFAILED] Invalid credentials"),
        ("not base64!", "a1 BAD The response is not base64"),
        ("AGFkYQ", "a1 BAD The response is not base64"),
        ("AGF!", "a1 BAD The response is not base64"),
        ("AGFkYQ==", "a1 NO [AUTHENTICATIONFAILED] Invalid credentials"),
        // "bob\0ada@example.org\0not-a-real-password": another authorisation identity
        ("Ym9iAGFkYUBleGFtcGxlLm9yZwBub3QtYS1yZWFsLXBhc3N3b3Jk", "a1 NO [AUTHENTICATIONFAILED] Invalid credentials"),
    ];
    let fake = FakeImap::new().without(Capability::SaslIr);
    for (response, want) in cases {
        let got = lines(&fake, &format!("a1 AUTHENTICATE PLAIN\r\n{response}\r\n"));
        assert_eq!(got.get(2).map(String::as_str), Some(want), "{response}");
    }
    let other = lines(&fake, "a1 AUTHENTICATE CRAM-MD5\r\na2 AUTHENTICATE\r\n");
    assert_eq!(
        other.get(1..),
        Some(["a1 BAD AUTHENTICATE CRAM-MD5 is not offered".to_owned(), "a2 BAD AUTHENTICATE needs a mechanism".to_owned()].as_slice())
    );
}

#[test]
fn a_preauth_greeting_needs_no_sign_in() {
    let got = lines(&FakeImap::new().preauth(), "a1 LOGIN x y\r\n");
    assert_eq!(got, [format!("* PREAUTH [CAPABILITY {SIGNED_IN}] InboxCraft fake IMAP ready"), "a1 BAD LOGIN is not available now".to_owned()]);
}

#[test]
fn noop_and_logout() {
    let got = lines(&FakeImap::new(), "a1 NOOP\r\na2 LOGOUT\r\na3 NOOP\r\n");
    assert_eq!(got.get(1..), Some(["a1 OK NOOP completed", "* BYE Logging out", "a2 OK LOGOUT completed"].map(str::to_owned).as_slice()));
}

#[test]
fn hostile_command_lines_get_bad_answers() {
    let cases = [
        ("", "* BAD Empty line"),
        ("a1", "* BAD Missing command"),
        ("a+1 NOOP", "* BAD Bad tag"),
        ("a1 FROBNICATE", "a1 BAD FROBNICATE is not available now"),
        ("a1 UID", "a1 BAD Missing command"),
        ("a1 uid fetch 1 FLAGS", "a1 BAD UID FETCH is not available now"),
        ("a1 LOGIN x {}", "a1 BAD Unexpected {"),
        ("a1 LOGIN x {12345678901}", "a1 BAD Unexpected {"),
        ("a1 AUTHENTICATE \"PLAIN\"", "a1 BAD AUTHENTICATE needs a mechanism"),
        ("a1 \"NOOP\"", "a1 BAD Missing command"),
        ("a1 LOGIN \"unterminated", "a1 BAD Unterminated quoted string"),
        ("a1 LOGIN \"bad \\escape\"", "a1 BAD Bad escape in a quoted string"),
        ("a1 LOGIN ((((((((((x))))))))))", "a1 BAD Lists nested too deep"),
        ("a1 LOGIN (x", "a1 BAD Unclosed list"),
        ("a1 LOGIN x)", "a1 BAD Unexpected )"),
        ("a1 LOGIN {x} y", "a1 BAD Unexpected {"),
        ("a1 LOGIN a\u{e9} b", "a1 BAD Bad atom"),
        ("a1 LOGIN \"nul\0\" b", "a1 BAD Bad quoted string"),
        ("a1 LOGIN x", "a1 BAD LOGIN needs a user name and a password"),
        ("a1 LOGIN x (y)", "a1 BAD LOGIN needs a user name and a password"),
        ("a1 LOGIN {99999999}", "a1 BAD The literal is refused"),
        ("a1 CAPABILITY extra", "a1 OK CAPABILITY completed"),
    ];
    for (line, want) in cases {
        let got = lines(&FakeImap::new(), &format!("{line}\r\n"));
        assert_eq!(got.last().map(String::as_str), Some(want), "{line:?}");
    }
}

#[test]
fn too_many_literals_in_one_command_are_refused() {
    let input = format!("a1 LOGIN {}\r\n", "{1}\r\nx ".repeat(9));
    let got = lines(&FakeImap::new(), &input);
    assert_eq!(got.iter().filter(|line| *line == "+ Ready for the literal").count(), 8, "{got:?}");
    assert_eq!(got.get(9).map(String::as_str), Some("a1 BAD The literal is refused"), "{got:?}");
    assert_eq!(got.get(10).map(String::as_str), Some("x BAD Missing command"), "{got:?}");
}

#[test]
fn too_many_bad_commands_end_the_session() {
    let got = lines(&FakeImap::new(), &"x NOPE\r\n".repeat(30));
    assert_eq!(got.len(), 1 + 20 + 1, "{got:?}");
    assert_eq!(got.last().map(String::as_str), Some("* BYE Too many bad commands"));
}

#[test]
fn an_overlong_line_ends_the_session() {
    let got = lines(&FakeImap::new(), &format!("a1 {}\r\na2 NOOP\r\n", "x".repeat(9000)));
    assert_eq!(got.get(1..), Some(["* BYE Line too long".to_owned()].as_slice()));
}

#[test]
fn input_that_stops_in_a_literal_ends_the_session() {
    let got = lines(&FakeImap::new(), "a1 LOGIN {20}\r\nada");
    assert_eq!(got.get(1..), Some(["+ Ready for the literal".to_owned()].as_slice()));
}

#[test]
fn the_default_fake_is_the_new_one() {
    assert_eq!(FakeImap::default().answer(b""), FakeImap::new().answer(b""));
}

#[test]
fn authenticate_plain_decodes_the_whole_base64_alphabet() {
    // "\0ada@example.org\0~~~???>>>"
    let got = lines(&FakeImap::new().login(USER, "~~~???>>>"), "a1 AUTHENTICATE PLAIN AGFkYUBleGFtcGxlLm9yZwB+fn4/Pz8+Pj4=\r\n");
    assert!(got.get(1).is_some_and(|line| line.starts_with("a1 OK [CAPABILITY")), "{got:?}");
}

#[test]
fn a_client_gone_before_its_plain_response_ends_the_session() {
    let got = lines(&FakeImap::new().without(Capability::SaslIr), "a1 AUTHENTICATE PLAIN\r\n");
    assert_eq!(got.get(1..), Some(["+ ".to_owned()].as_slice()));
}

#[test]
fn too_many_unreadable_lines_end_the_session() {
    let got = lines(&FakeImap::new(), &"\r\n".repeat(25));
    assert_eq!(got.len(), 1 + 20 + 1, "{got:?}");
    assert_eq!(got.last().map(String::as_str), Some("* BYE Too many bad commands"));
}
