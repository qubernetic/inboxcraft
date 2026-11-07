// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{Answer, Client, Event, State, Transport, Verb};
use crate::ProtoError;

fn signed_in(transport: Transport) -> Client {
    let mut client = Client::new(transport);
    let _ = client.receive(b"220 smtp.example.org\r\n");
    let _ = client.ehlo("client.example.org");
    let _ = client.receive(b"250-smtp.example.org\r\n250-STARTTLS\r\n250 AUTH PLAIN\r\n");
    while client.next_event().is_some() {}
    client.take_output();
    client
}

#[test]
fn the_session_tells_what_it_knows() {
    let mut client = signed_in(Transport::Tls);
    assert!(client.is_encrypted());
    assert!(!client.is_signed_in());
    assert!(client.extensions().is_some_and(|extensions| extensions.offers_auth("PLAIN")));
    assert_eq!(client.auth_plain("ada@example.org", "hunter2"), Ok(()));
    assert_eq!(client.receive(b"235 OK\r\n"), Ok(()));
    assert!(client.is_signed_in());
    assert!(!signed_in(Transport::Plain).is_encrypted());
}

#[test]
fn starttls_forgets_the_extensions_until_ehlo_is_answered_again() {
    let mut client = signed_in(Transport::Plain);
    assert_eq!(client.starttls(), Ok(()));
    assert_eq!(client.take_output(), b"STARTTLS\r\n");
    assert_eq!(client.receive(b"220 go\r\n"), Ok(()));
    assert_eq!(client.receive(b""), Ok(()));
    assert_eq!(client.tls_established(), Ok(()));
    assert_eq!(client.extensions(), None);
    assert_eq!(client.take_output(), b"EHLO client.example.org\r\n");
    assert_eq!(client.next_event(), Some(Event::Completed { command: Verb::StartTls, outcome: Ok(Answer::StartTls) }));
}

#[test]
fn neither_debug_nor_an_error_shows_the_password() {
    let mut client = signed_in(Transport::Tls);
    assert_eq!(client.auth_plain("ada@example.org", "hunter2"), Ok(()));
    let shown = format!("{client:?}");
    assert!(!shown.contains("hunter2") && shown.contains("Auth"), "{shown}");
    let error = client.receive(b"421 hunter2 is wrong\r\n");
    assert_eq!(error, Err(ProtoError::Bye("*** is wrong".into())));
    assert_eq!(client.state(), State::Closed);
    assert_eq!(client.take_output(), b"");
}

#[test]
fn a_refusal_knows_whether_it_is_for_good() {
    let mut client = signed_in(Transport::Tls);
    assert_eq!(client.rset(), Ok(()));
    assert_eq!(client.receive(b"451 later\r\n"), Ok(()));
    let Some(Event::Completed { outcome: Err(refusal), .. }) = client.next_event() else { panic!("no refusal") };
    assert!(!refusal.is_permanent());
    assert_eq!(refusal.code, 451);
    assert_eq!(refusal.text, "later");
}

#[test]
fn every_refusal_is_4yz_or_5yz() {
    type Command = fn(&mut Client) -> Result<(), ProtoError>;
    let commands: [Command; 2] = [Client::rset, Client::quit];
    for command in commands {
        for code in [200, 250, 299, 300, 335, 354, 399, 400, 451, 500, 554, 599] {
            let mut client = signed_in(Transport::Tls);
            assert_eq!(command(&mut client), Ok(()));
            let _ = client.receive(format!("{code} words\r\n").as_bytes());
            while let Some(event) = client.next_event() {
                if let Event::Completed { outcome: Err(refusal), .. } = event {
                    assert!((400..600).contains(&refusal.code), "{refusal:?}");
                }
            }
        }
    }
}

#[test]
fn every_state_and_verb_has_a_name() {
    let states = [State::Greeting, State::Connected, State::Ready, State::StartingTls, State::Mail, State::Quitting, State::Closed];
    let mut names: Vec<String> = states.iter().map(ToString::to_string).collect();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), states.len());
    let verbs = [Verb::Ehlo, Verb::StartTls, Verb::Auth, Verb::Mail, Verb::Rcpt, Verb::Data, Verb::Rset, Verb::Quit];
    let names: Vec<String> = verbs.iter().map(ToString::to_string).collect();
    assert_eq!(names, ["EHLO", "STARTTLS", "AUTH", "MAIL", "RCPT", "DATA", "RSET", "QUIT"]);
}
