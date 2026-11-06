// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_model::{Flag, FolderId, MessageId};
use serde_json::{Value, json};

use crate::test_support::{sample_session, session};
use crate::{AgentClass, Caller, EngineError, Session};

const SET: &str = "sample.mail.set";

/// Message `id`'s folder, UID and whether it is read.
fn held(session: &Session, id: u64) -> (u64, u32, bool) {
    let entry = session.store().message(MessageId::new(id)).unwrap().unwrap();
    let summary = entry.message().summary();
    (entry.message().folder().get(), summary.uid().get(), summary.flags().contains(&Flag::Seen))
}

fn bad(result: Result<Value, EngineError>) -> bool {
    matches!(result, Err(EngineError::BadParams { .. }))
}

#[test]
fn sample_mail_set_is_safe_and_runs_only_in_a_sample_session() {
    let mut session = session();
    assert!(!session.is_sample());
    assert_eq!(session.registry().get(SET).unwrap().class, Some(AgentClass::Safe));
    for caller in [Caller::Owner, Caller::Agent] {
        let refused = session.run_as(caller, SET, &json!({ "id": 1, "seen": true }));
        assert_eq!(refused, Err(EngineError::SampleOnly(SET)));
    }
    assert_eq!(held(&session, 1), (1, 10, false), "a refused command changes nothing");
    assert!(sample_session().is_sample());
}

#[test]
fn it_marks_a_message_read_and_unread_and_answers_what_it_was() {
    let mut session = sample_session();
    let answer = session.run_as(Caller::Agent, SET, &json!({ "id": 1, "seen": true })).unwrap();
    assert_eq!(answer, json!({ "id": 1, "folder": 1, "seen": true, "was": { "folder": 1, "seen": false } }));
    assert_eq!(held(&session, 1), (1, 10, true));
    let answer = session.run(SET, &json!({ "id": 1, "seen": false })).unwrap();
    assert_eq!(answer["was"], json!({ "folder": 1, "seen": true }));
    assert_eq!(held(&session, 1), (1, 10, false));
    assert!(session.take_jobs().is_empty(), "nothing goes to a server");
}

#[test]
fn it_moves_a_message_to_a_folder_of_its_account_and_back() {
    let mut session = sample_session();
    let answer = session.run(SET, &json!({ "id": 2, "seen": true, "folder": 3 })).unwrap();
    assert_eq!(answer, json!({ "id": 2, "folder": 3, "seen": true, "was": { "folder": 1, "seen": false } }));
    assert_eq!(held(&session, 2), (3, 1, true), "the first UID of an empty folder");
    assert!(session.store().messages(FolderId::new(1)).unwrap().iter().all(|entry| entry.message().id().get() != 2));
    session.run(SET, &json!({ "id": 2, "seen": false, "folder": 1 })).unwrap();
    assert_eq!(held(&session, 2), (1, 11, false), "past the UIDs the folder holds");
    session.run(SET, &json!({ "id": 1, "folder": 1 })).unwrap();
    assert_eq!(held(&session, 1), (1, 10, false), "a message moved to its own folder keeps its UID");
    assert!(session.take_jobs().is_empty(), "nothing goes to a server");
}

#[test]
fn it_refuses_what_is_not_there_or_changes_nothing() {
    let mut session = sample_session();
    assert_eq!(session.run(SET, &json!({ "id": 99, "seen": true })), Err(EngineError::NotFound { what: "message", id: 99 }));
    assert_eq!(session.run(SET, &json!({ "id": 1, "folder": 99 })), Err(EngineError::NotFound { what: "folder", id: 99 }));
    for junk in [
        json!({ "id": 1 }),
        json!({ "id": 1, "folder": 4 }),
        json!({}),
        json!({ "id": -1, "seen": true }),
        json!({ "id": 1, "seen": "yes" }),
        json!({ "id": 1, "seen": true, "x": 1 }),
        json!([1]),
    ] {
        assert!(bad(session.run(SET, &junk)), "{junk}");
    }
    assert_eq!(held(&session, 1), (1, 10, false), "a refused command changes nothing");
}
