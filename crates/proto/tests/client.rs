// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The client through its public interface: what the transcripts do not show well.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use inboxcraft_model::{FolderState, ModSeq, Uid, UidSet, UidValidity};
use inboxcraft_proto::ProtoError;
use inboxcraft_proto::imap::{
    Answer, Client, Event, Extension, FetchAttributes, FetchItem, MAX_CAPABILITY_LEN, MAX_LINE, MAX_MAILBOX_NAME, MAX_PREVIEW_CHARS, MAX_SET_RANGES,
    Moved, Notify, NotifyMailboxes, PreviewFetch, Refusal, SearchKey, SelectOptions, State, Transport,
};
use proptest::prelude::*;

/// A client over TLS, greeted and signed in with PREAUTH, with `INBOX` open.
fn selected() -> Client {
    let mut client = Client::new(Transport::Tls);
    client.receive(b"* PREAUTH [CAPABILITY IMAP4rev1] ready\r\n").unwrap();
    client.select("INBOX").unwrap();
    client.receive(b"A0001 OK done\r\n").unwrap();
    assert_eq!(client.state(), State::Selected);
    client.take_output();
    while client.next_event().is_some() {}
    client
}

#[test]
fn encryption_and_capabilities_as_the_session_goes() {
    let mut client = Client::new(Transport::Plain);
    assert!(!client.is_encrypted());
    assert_eq!(client.capabilities(), None);
    client.receive(b"* OK [CAPABILITY IMAP4rev1 STARTTLS] ready\r\n").unwrap();
    assert!(client.capabilities().is_some_and(|caps| caps.has("STARTTLS")));
    client.starttls().unwrap();
    client.receive(b"A0001 OK go\r\n").unwrap();
    client.tls_established().unwrap();
    assert!(client.is_encrypted());
    // What was said in the clear is forgotten.
    assert_eq!(client.capabilities(), None);
    assert!(Client::new(Transport::Tls).is_encrypted());
}

#[test]
fn compression_is_on_once_the_server_agreed_and_its_first_bytes_come_back_unread() {
    let mut client = Client::new(Transport::Tls);
    client.receive(b"* PREAUTH [CAPABILITY IMAP4rev1 COMPRESS=DEFLATE] ready\r\n").unwrap();
    assert!(!client.is_compressed());
    client.compress().unwrap();
    client.receive(b"A0001 NO [COMPRESSIONACTIVE] no\r\n").unwrap();
    assert!(!client.is_compressed());
    while client.next_event().is_some() {}
    let tag = client.compress().unwrap();
    // Raw DEFLATE bytes, a line feed among them, and no line ending at the end.
    client.receive(b"A0002 OK go\r\n\xf2\x0a\x00\x00").unwrap();
    assert!(client.is_compressed());
    assert_eq!(client.next_event(), Some(Event::Completed { tag, outcome: Ok(Answer::Compressed(b"\xf2\x0a\x00\x00".to_vec())) }));
    assert_eq!(client.next_event(), None);
    assert_eq!(client.compress(), Err(ProtoError::AlreadyCompressed));
    // The caller feeds what it inflated.
    client.receive(b"* OK still here\r\n").unwrap();
    assert_eq!(client.state(), State::Authenticated);
}

#[test]
fn arguments_that_cannot_be_sent_are_refused_unsent() {
    let mut client = Client::new(Transport::Tls);
    client.receive(b"* OK [CAPABILITY IMAP4rev1 AUTH=PLAIN] ready\r\n").unwrap();
    for (user, password) in [("", "x"), ("ada", ""), ("ada\0", "x"), ("ada", "x\0y")] {
        assert!(matches!(client.login(user, password), Err(ProtoError::BadArgument(_))), "{user:?}");
        assert!(matches!(client.authenticate_plain(user, password), Err(ProtoError::BadArgument(_))), "{user:?}");
    }
    assert!(client.take_output().is_empty());

    let mut client = selected();
    for name in ["", "a\r\nb", "a\0", "\u{7}", &"x".repeat(MAX_MAILBOX_NAME + 1)] {
        assert_eq!(client.select(name), Err(ProtoError::BadArgument("mailbox name")), "{name:?}");
        assert_eq!(client.examine(name), Err(ProtoError::BadArgument("mailbox name")), "{name:?}");
        assert_eq!(client.uid_copy(&UidSet::from_iter(Uid::new(1)), name), Err(ProtoError::BadArgument("mailbox name")), "{name:?}");
    }
    assert!(matches!(client.uid_fetch(&UidSet::default(), FetchAttributes::default()), Err(ProtoError::BadArgument(_))));
    assert!(matches!(client.uid_copy(&UidSet::default(), "Archive"), Err(ProtoError::BadArgument(_))));
    assert!(matches!(client.uid_search(&SearchKey::Subject("a\nb".into())), Err(ProtoError::BadArgument(_))));
    assert!(client.take_output().is_empty());
    assert_eq!(client.state(), State::Selected);
}

#[test]
fn bytes_in_any_pieces_give_the_same_events() {
    let input: &[u8] = b"* 1 FETCH (UID 7 FLAGS (\\Seen) BODY[] {5}\r\nHello)\r\n* 2 EXISTS\r\nA0002 OK done\r\n";
    let run = |pieces: &[&[u8]]| {
        let mut client = selected();
        client.uid_fetch(&UidSet::from_iter([Uid::new(7).unwrap()]), FetchAttributes::default()).unwrap();
        for piece in pieces {
            client.receive(piece).unwrap();
        }
        std::iter::from_fn(|| client.next_event()).collect::<Vec<_>>()
    };
    let whole = run(&[input]);
    assert_eq!(whole.len(), 3);
    assert!(matches!(whole.last(), Some(Event::Completed { outcome: Ok(Answer::Fetched), .. })));
    let bytes: Vec<&[u8]> = input.chunks(1).collect();
    assert_eq!(run(&bytes), whole);
    let (a, b) = input.split_at(30);
    assert_eq!(run(&[a, &[], b]), whole);
}

#[test]
fn a_line_without_end_is_refused_at_the_cap() {
    let mut client = selected();
    let chunk = vec![b'x'; 64 * 1024];
    let mut result = Ok(());
    for _ in 0..=(MAX_LINE / chunk.len()) {
        result = client.receive(&chunk);
        if result.is_err() {
            break;
        }
    }
    assert_eq!(result, Err(ProtoError::LineTooLong { limit: MAX_LINE }));
    assert_eq!(client.state(), State::Closed);
    assert_eq!(client.receive(b"\r\n"), Err(ProtoError::Closed));
    assert_eq!(client.logout(), Err(ProtoError::Closed));
}

#[test]
fn a_fatal_error_drops_what_was_not_written_yet() {
    let mut client = selected();
    client.uid_search(&SearchKey::All).unwrap();
    assert_eq!(client.receive(b"A9999 OK\r\n"), Err(ProtoError::Unexpected("tagged response")));
    assert!(client.take_output().is_empty());
}

/// The text of an event that the server wrote for people.
fn server_words(event: &Event) -> Option<&str> {
    match event {
        Event::Alert(text) | Event::Completed { outcome: Err(Refusal { text, .. }), .. } => Some(text),
        _ => None,
    }
}

#[test]
fn the_password_is_hidden_also_as_login_quotes_it() {
    let mut client = Client::new(Transport::Tls);
    client.receive(b"* OK [CAPABILITY IMAP4rev1] ready\r\n").unwrap();
    client.next_event();
    client.login("ada@example.org", "pa\"ss\\").unwrap();
    assert_eq!(client.take_output(), b"A0001 LOGIN \"ada@example.org\" \"pa\\\"ss\\\\\"\r\n");
    client.receive(b"A0001 NO [AUTHENTICATIONFAILED] bad password \"pa\\\"ss\\\\\" or pa\"ss\\\r\n").unwrap();
    let Some(Event::Completed { outcome: Err(refusal), .. }) = client.next_event() else { panic!("no refusal") };
    assert_eq!(refusal.text, "bad password \"***\" or ***");
}

/// A client over TLS, greeted, whose server offers LOGIN and AUTHENTICATE PLAIN with SASL-IR.
fn greeted() -> Client {
    let mut client = Client::new(Transport::Tls);
    client.receive(b"* OK [CAPABILITY IMAP4rev1 AUTH=PLAIN SASL-IR] ready\r\n").unwrap();
    client.next_event();
    client
}

#[test]
fn a_password_with_edge_white_space_is_hidden_first_and_last() {
    for password in [" secret", "secret ", "\tsecret"] {
        for plain in [false, true] {
            let mut client = greeted();
            if plain { client.authenticate_plain("tim", password) } else { client.login("tim", password) }.unwrap();
            client.receive(b"A0001 NO secret is wrong, wrong password secret\r\n").unwrap();
            let Some(Event::Completed { outcome: Err(refusal), .. }) = client.next_event() else { panic!("no refusal") };
            assert_eq!(refusal.text, "*** is wrong, wrong password ***", "{password:?}");

            let mut client = greeted();
            if plain { client.authenticate_plain("tim", password) } else { client.login("tim", password) }.unwrap();
            let error = client.receive(b"* BYE secret bad secret\r\n").unwrap_err();
            assert_eq!(error, ProtoError::Bye("*** bad ***".to_owned()), "{password:?}");
        }
    }
}

#[test]
fn a_response_code_is_kept_only_as_a_clean_word() {
    let refused_code = |line: &[u8]| {
        let mut client = greeted();
        client.login("tim", "hunter2").unwrap();
        client.receive(line).unwrap();
        let Some(Event::Completed { outcome: Err(refusal), .. }) = client.next_event() else { panic!("no refusal") };
        refusal.code
    };
    assert_eq!(refused_code(b"A0001 NO [AUTHENTICATIONFAILED] no\r\n"), Some("AUTHENTICATIONFAILED".to_owned()));
    assert_eq!(refused_code(b"A0001 NO [X\x07\x1b[31M] no\r\n"), None);
    assert_eq!(refused_code(format!("A0001 NO [{}] no\r\n", "X".repeat(MAX_CAPABILITY_LEN + 1)).as_bytes()), None);
    assert_eq!(refused_code(b"A0001 NO [HUNTER2] no\r\n"), None);
    assert_eq!(refused_code(b"A0001 NO [XHUNTER2Y] no\r\n"), None);
}

#[test]
fn a_refused_capability_after_the_sign_in_leaves_none_known() {
    let mut client = greeted();
    client.login("tim", "hunter2").unwrap();
    client.receive(b"A0001 OK signed in\r\n").unwrap();
    assert_eq!(client.take_output(), b"A0001 LOGIN \"tim\" \"hunter2\"\r\nA0002 CAPABILITY\r\n");
    client.receive(b"* CAPABILITY IMAP4rev1 X\r\nA0002 NO not now\r\n").unwrap();
    let Some(Event::Completed { outcome: Ok(Answer::Authenticated(capabilities)), .. }) = client.next_event() else { panic!("no sign-in") };
    assert_eq!(capabilities.iter().count(), 0);
    assert_eq!(client.capabilities(), None);
    assert_eq!(client.state(), State::Authenticated);
}

#[test]
fn only_what_was_asked_for_and_confirmed_is_enabled() {
    let enabled = |reply: &[u8], extension| {
        let mut client = Client::new(Transport::Tls);
        client.receive(b"* PREAUTH [CAPABILITY IMAP4rev1 ENABLE CONDSTORE QRESYNC] ready\r\n").unwrap();
        client.enable(extension).unwrap();
        client.receive(reply).unwrap();
        (client.is_enabled(Extension::CondStore), client.is_enabled(Extension::QResync))
    };
    assert_eq!(enabled(b"* ENABLED QRESYNC\r\nA0001 OK\r\n", Extension::QResync), (true, true));
    assert_eq!(enabled(b"* ENABLED condstore\r\nA0001 OK\r\n", Extension::CondStore), (true, false));
    // Not what was asked for, or refused in the end.
    assert_eq!(enabled(b"* ENABLED QRESYNC\r\nA0001 OK\r\n", Extension::CondStore), (false, false));
    assert_eq!(enabled(b"* ENABLED QRESYNC\r\nA0001 NO\r\n", Extension::QResync), (false, false));
    assert_eq!(enabled(b"A0001 OK\r\n", Extension::QResync), (false, false));
}

#[test]
fn known_uids_beyond_the_cap_are_refused_unsent() {
    let mut client = Client::new(Transport::Tls);
    client.receive(b"* PREAUTH [CAPABILITY IMAP4rev1 ENABLE QRESYNC] ready\r\n").unwrap();
    client.enable(Extension::QResync).unwrap();
    client.receive(b"* ENABLED QRESYNC\r\nA0001 OK\r\n").unwrap();
    client.select("INBOX").unwrap();
    client.receive(b"A0002 OK\r\n").unwrap();
    client.take_output();
    let known = FolderState::new(UidValidity::new(1).unwrap()).with_highest_modseq(ModSeq::new(9));
    let uids: UidSet = (1..=u32::try_from(MAX_SET_RANGES).unwrap() + 1).filter_map(|n| Uid::new(n * 2)).collect();
    let options = SelectOptions { known: Some(known), known_uids: Some(uids), ..SelectOptions::default() };
    assert_eq!(client.select_with("Archive", &options), Err(ProtoError::BadArgument("UID set with too many ranges")));
    assert!(client.take_output().is_empty());
    assert_eq!(client.state(), State::Selected);
}

/// Hostile bytes for a session in any state.
fn session_bytes() -> impl Strategy<Value = Vec<Vec<u8>>> {
    let line = prop_oneof![
        Just(
            b"* OK [CAPABILITY IMAP4rev1 AUTH=PLAIN SASL-IR STARTTLS LIST-EXTENDED SPECIAL-USE ENABLE CONDSTORE QRESYNC IDLE NOTIFY] hi\r\n".to_vec()
        ),
        Just(b"* CAPABILITY IMAP4rev1 MOVE UIDPLUS PREVIEW COMPRESS=DEFLATE\r\n".to_vec()),
        Just(b"* OK [COPYUID 7 1 9] moved\r\n".to_vec()),
        Just(b"* OK [COPYUID 7 1:2 9:10] moved\r\n".to_vec()),
        Just(b"A0002 OK [COPYUID 7 1 9] done\r\n".to_vec()),
        Just(b"* NO [UIDNOTSTICKY] gone\r\n".to_vec()),
        Just("* 1 FETCH (UID 1 PREVIEW \"pay\u{202e}fdp.exe\")\r\n".as_bytes().to_vec()),
        Just(b"* 1 FETCH (PREVIEW {3}\r\na\r\n)\r\n".to_vec()),
        Just(b"* ENABLED QRESYNC CONDSTORE\r\n".to_vec()),
        Just(b"* VANISHED (EARLIER) 1:5,9\r\n".to_vec()),
        Just(b"* VANISHED 1:*\r\n".to_vec()),
        Just(b"* OK [CLOSED]\r\n".to_vec()),
        Just(b"* OK [UIDVALIDITY 9] hunter2\r\n".to_vec()),
        Just(b"* OK [NOTIFICATIONOVERFLOW]\r\n".to_vec()),
        Just(b"* STATUS INBOX (MESSAGES 1 UIDNEXT 2 HIGHESTMODSEQ 3)\r\n".to_vec()),
        Just(b"* 2 FETCH (UID 4 MODSEQ (77))\r\n".to_vec()),
        Just(b"* PREAUTH hi\r\n".to_vec()),
        Just(b"+ \r\n".to_vec()),
        Just(b"A0001 OK [CAPABILITY IMAP4rev1] done\r\n".to_vec()),
        Just(b"A0002 NO hunter2 AHRpbQBodW50ZXIy\r\n".to_vec()),
        Just(b"* 3 FETCH (UID 9 FLAGS (\\Seen) BODY[] {4}\r\nabcd)\r\n".to_vec()),
        Just(b"* LIST (\\Sent) \"/\" {3}\r\nhun)\r\n".to_vec()),
        Just(b"* SEARCH 1 2 3\r\n".to_vec()),
        Just(b"* BYE hunter2\r\n".to_vec()),
        Just(b"A0003 NO [AUTHENTICATIONFAILED] bad password \"hun\\\"ter\\\\2\" hun\"ter\\2\r\n".to_vec()),
        Just(b"A0004 NO \"hun\\\\\\\\ter2\" hun\\\\ter2\r\n".to_vec()),
        Just("* OK [ALERT] abc\u{202e}gpj.exe\u{200b}x\r\n".as_bytes().to_vec()),
        Just("A0002 NO a\u{2066}b\u{feff}c\r\n".as_bytes().to_vec()),
        Just("* BYE \u{061c}gone\u{200f}\r\n".as_bytes().to_vec()),
        proptest::collection::vec(any::<u8>(), 0..64),
        "[ -~]{0,40}\r\n".prop_map(String::into_bytes),
    ];
    proptest::collection::vec(line, 0..12)
}

proptest! {
    #[test]
    fn hostile_bytes_never_panic_and_never_show_the_password(
        pieces in session_bytes(),
        commands in proptest::collection::vec(0_u8..22, 0..8),
        password in prop_oneof![Just("hunter2"), Just("hun\"ter\\2"), Just("hun\\\\ter2"), Just(" hunter2"), Just("hunter2\t"), Just(" hun\"ter\\2 ")],
    ) {
        // The password as LOGIN quotes it; also how `Debug` writes it.
        let quoted = password.replace('\\', "\\\\").replace('"', "\\\"");
        let mut client = Client::new(Transport::Tls);
        let known = FolderState::new(UidValidity::new(9).unwrap()).with_highest_modseq(ModSeq::new(5));
        let resync = SelectOptions { condstore: true, known: Some(known), known_uids: Some(UidSet::from_iter(Uid::new(3))) };
        let changed = FetchAttributes { changed_since: ModSeq::new(5), vanished: true, ..FetchAttributes::default() };
        let one = UidSet::from_iter([Uid::new(1).unwrap()]);
        let preview = FetchAttributes { preview: Some(PreviewFetch::Generate), ..FetchAttributes::default() };
        // The client can hide the password only once it has been given one.
        let mut given = false;
        let mut shown = Vec::new();
        for (n, piece) in pieces.iter().enumerate() {
            if let Some(command) = commands.get(n) {
                let result = match command {
                    13 => client.done().map(|()| None),
                    _ => match command {
                    0 => client.capability(),
                    1 => client.login("tim", password),
                    2 => client.authenticate_plain("tim", password),
                    3 => client.list(),
                    4 => client.select("INBOX"),
                    5 => client.uid_fetch(&UidSet::from_iter([Uid::new(1).unwrap()]), FetchAttributes::default()),
                    6 => client.uid_search(&SearchKey::Unseen),
                    7 => client.starttls(),
                    8 => client.tls_established(),
                    9 => client.enable(Extension::QResync),
                    10 => client.select_with("INBOX", &resync),
                    11 => client.uid_fetch(&UidSet::from_iter([Uid::new(1).unwrap()]), changed),
                    12 => client.idle(),
                    14 => client.notify(&Notify { selected: true, mailboxes: Some(NotifyMailboxes::Personal), mailbox_names: true, status: true }),
                    15 => client.notify_none(),
                    17 => client.uid_copy(&one, "Archive"),
                    18 => client.uid_move(&one, "Archive"),
                    19 => client.uid_expunge(&one),
                    20 => client.compress(),
                    21 => client.uid_fetch(&one, preview),
                    _ => client.logout(),
                    }
                    .map(Some),
                };
                match result {
                    Ok(_) if matches!(command, 1 | 2) => given = true,
                    Ok(_) => {}
                    Err(error) => shown.push(error.to_string()),
                }
            }
            let received = client.receive(piece);
            if let Err(error) = &received {
                prop_assert!(error.is_fatal());
                prop_assert_eq!(client.state(), State::Closed);
            }
            client.take_output();
            let events: Vec<Event> = std::iter::from_fn(|| client.next_event()).collect();
            for event in &events {
                match event {
                    // A COPYUID names only UIDs that were asked for.
                    Event::Completed { outcome: Ok(Answer::Copied(Some(copy_uid)) | Answer::Moved(Moved { copy_uid: Some(copy_uid), .. })), .. } => {
                        prop_assert!(copy_uid.pairs().all(|(source, _)| source == Uid::new(1).unwrap()), "{:?}", copy_uid);
                    }
                    Event::Fetched(FetchItem { preview: Some(preview), .. }) => {
                        prop_assert!(preview.chars().count() <= MAX_PREVIEW_CHARS, "{:?}", preview);
                        prop_assert!(!preview.chars().any(|c| c.is_control() || inboxcraft_model::is_invisible_mark(c)), "{:?}", preview);
                    }
                    _ => {}
                }
            }
            // Whatever the server wrote for people is shown without invisible marks.
            let mut words: Vec<&str> = events.iter().filter_map(server_words).collect();
            if let Err(ProtoError::Bye(text)) = &received {
                words.push(text);
            }
            for text in &words {
                prop_assert!(!text.chars().any(inboxcraft_model::is_invisible_mark), "{:?}", text);
            }
            if given {
                shown.extend(words.iter().map(|text| (*text).to_owned()));
                shown.extend(received.err().map(|error| error.to_string()));
                shown.extend(events.iter().map(|event| format!("{event:?}")));
            }
        }
        shown.push(format!("{client:?}"));
        for text in &shown {
            prop_assert!(!text.contains(password) && !text.contains(&quoted), "{}", text);
            // A server line is trimmed, so the password may come back without its edge white space.
            prop_assert!(!text.contains(password.trim()) && !text.contains(quoted.trim()), "{}", text);
            prop_assert!(!text.contains("AHRpbQBodW50ZXIy"), "{}", text);
        }
    }
}
