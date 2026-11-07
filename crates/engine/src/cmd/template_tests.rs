// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Templates: `template.save`, `template.list` and `template.delete` keep them on the account; `compose.template`
//! puts one into a draft with its names filled in, and a blank left to fill blocks Send.

use inboxcraft_model::{AccountId, DraftId};
use inboxcraft_time::FakeClock;
use serde_json::{Value, json};

use super::compose_tests::session_at;
use super::reply_tests::store;
use crate::test_support::{NOW, at};
use crate::{AgentClass, Caller, EngineError, Session};

fn session() -> Session {
    session_at(store(), &FakeClock::new(at(NOW)))
}

fn bad(result: Result<Value, EngineError>) -> bool {
    matches!(result, Err(EngineError::BadParams { .. }))
}

const THANKS: &str = "Hi {first_name},\n\nthe {?day} works for me. {unknown} {\n{my_name}";

fn save(session: &mut Session, account: u64, name: &str, subject: &str, text: &str) -> Value {
    session.run("template.save", &json!({ "account": account, "name": name, "subject": subject, "text": text })).unwrap()
}

/// A new draft of account 1 to `to`; its id.
fn draft_to(session: &mut Session, to: &str) -> u64 {
    let id = session.run("compose.new", &json!({ "account": 1 })).unwrap()["id"].as_u64().unwrap();
    session.run("compose.update", &json!({ "id": id, "to": [to] })).unwrap();
    id
}

#[test]
fn templates_are_settings_and_using_one_is_safe() {
    let session = session();
    for (id, class) in [
        ("template.list", AgentClass::Safe),
        ("template.save", AgentClass::Owner),
        ("template.delete", AgentClass::Owner),
        ("compose.template", AgentClass::Safe),
    ] {
        assert_eq!(session.registry().get(id).map(|spec| spec.class), Some(Some(class)), "{id}");
    }
}

#[test]
fn templates_are_kept_on_their_account() {
    let mut session = session();
    assert_eq!(session.run_as(Caller::Agent, "template.list", &json!({})).unwrap(), json!({ "templates": [] }));
    assert!(matches!(
        session.run_as(Caller::Agent, "template.save", &json!({ "account": 1, "name": "x", "text": "" })),
        Err(EngineError::NotAllowed { .. })
    ));
    let saved = save(&mut session, 1, " Thanks ", "Rota {name}", THANKS);
    assert_eq!(saved, json!({ "account": 1, "name": "Thanks", "subject": "Rota {name}", "text": THANKS }));
    save(&mut session, 2, "Later", "", "Later.");
    // The same name without case replaces it, in its place.
    save(&mut session, 1, "Short", "", "Short.");
    save(&mut session, 1, "thanks", "", "Thanks!");
    let names = |list: &Value| list["templates"].as_array().unwrap().iter().map(|t| (t["account"].clone(), t["name"].clone())).collect::<Vec<_>>();
    let all = session.run("template.list", &json!({})).unwrap();
    assert_eq!(names(&all), [(json!(1), json!("thanks")), (json!(1), json!("Short")), (json!(2), json!("Later"))]);
    assert_eq!(all["templates"][0]["text"], json!("Thanks!"));
    let one = session.run("template.list", &json!({ "account": 2 })).unwrap();
    assert_eq!(names(&one), [(json!(2), json!("Later"))]);
    // The account record holds them, so they outlast the session.
    let account = session.store().accounts().unwrap().into_iter().find(|account| account.id() == AccountId::new(1)).unwrap();
    assert_eq!(account.templates().len(), 2);
    assert_eq!(session.run("template.delete", &json!({ "account": 1, "name": "SHORT" })).unwrap(), json!({ "account": 1, "deleted": "Short" }));
    assert_eq!(names(&session.run("template.list", &json!({ "account": 1 })).unwrap()), [(json!(1), json!("thanks"))]);
}

#[test]
fn template_commands_refuse_what_does_not_fit() {
    let mut session = session();
    let run = |session: &mut Session, id: &str, params: Value| session.run(id, &params);
    assert!(bad(run(&mut session, "template.save", json!({ "account": 1, "name": " \n", "text": "x" }))));
    assert!(bad(run(&mut session, "template.save", json!({ "account": 1, "name": "x".repeat(101), "text": "x" }))));
    assert!(bad(run(&mut session, "template.save", json!({ "account": 1, "name": "x", "subject": "s".repeat(501), "text": "x" }))));
    assert!(bad(run(&mut session, "template.save", json!({ "account": 1, "name": "x", "text": "é".repeat(16_001) }))));
    assert!(bad(run(&mut session, "template.save", json!({ "account": 1, "name": "x" }))));
    assert!(bad(run(&mut session, "template.save", json!({ "account": 1, "name": "x", "text": "x", "extra": 1 }))));
    assert!(matches!(run(&mut session, "template.save", json!({ "account": 9, "name": "x", "text": "x" })), Err(EngineError::NotFound { .. })));
    assert!(bad(run(&mut session, "template.delete", json!({ "account": 1, "name": "none" }))));
    assert!(matches!(run(&mut session, "template.delete", json!({ "account": 9, "name": "x" })), Err(EngineError::NotFound { .. })));
    assert!(matches!(run(&mut session, "template.list", json!({ "account": 9 })), Err(EngineError::NotFound { .. })));
    // An account holds at most so many.
    for n in 0..inboxcraft_model::Account::MAX_TEMPLATES {
        save(&mut session, 2, &format!("t{n}"), "", "x");
    }
    assert!(bad(run(&mut session, "template.save", json!({ "account": 2, "name": "one more", "text": "x" }))));
    // Replacing one is still fine.
    assert!(run(&mut session, "template.save", json!({ "account": 2, "name": "t0", "text": "y" })).is_ok());
}

#[test]
fn a_template_fills_in_the_names_and_its_blank_blocks_send() {
    let mut session = session();
    save(&mut session, 1, "Thanks", "Rota {name}", THANKS);
    let draft = draft_to(&mut session, "Odile Brannock <odile@example.org>");
    let answer = session.run_as(Caller::Agent, "compose.template", &json!({ "id": draft, "name": "thanks" })).unwrap();
    assert_eq!(answer["subject"], json!("Rota Odile Brannock"));
    let text = "Hi Odile,\n\nthe [[day]] works for me. {unknown} {\nAnna Example\n\n-- \nAnna\nExample Ltd.";
    assert_eq!(answer["text"], json!(text));
    assert_eq!(answer["blanks"], json!(["[[day]]"]));
    assert_eq!((answer["sendable"].clone(), answer["blocks"].clone()), (json!(false), json!(["blank"])));
    assert!(matches!(session.run("mail.send", &json!({ "id": draft })), Err(EngineError::BadParams { .. })));
    // Filled in, it can go.
    let filled = session.run("compose.update", &json!({ "id": draft, "text": text.replace("[[day]]", "Friday") })).unwrap();
    assert_eq!((filled["sendable"].clone(), filled["blanks"].clone()), (json!(true), json!([])));
    assert!(session.run("mail.send", &json!({ "id": draft })).is_ok());
}

#[test]
fn a_name_that_is_not_known_becomes_a_blank_too() {
    let mut session = session();
    save(&mut session, 2, "Hello", "Hello {first_name}", "Dear {name},\n{my_name}");
    let id = session.run("compose.new", &json!({ "account": 2 })).unwrap()["id"].as_u64().unwrap();
    let answer = session.run("compose.template", &json!({ "id": id, "name": "Hello" })).unwrap();
    // No recipient and no display name on account 2.
    assert_eq!(answer["subject"], json!("Hello [[first name]]"));
    assert_eq!(answer["text"], json!("Dear [[name]],\n[[my name]]"));
    assert_eq!(answer["blanks"], json!(["[[first name]]", "[[name]]", "[[my name]]"]));
    assert_eq!(answer["blocks"], json!(["no_recipient", "blank"]));
}

#[test]
fn a_template_goes_above_a_quote_and_keeps_the_reply_subject() {
    let mut session = session();
    save(&mut session, 1, "Ok", "Something else", "Fine by me, {first_name}. {?when}  {?}");
    let reply = session.run("compose.reply", &json!({ "id": 20 })).unwrap();
    let id = reply["id"].as_u64().unwrap();
    // The quote already holds the mark the blank would get.
    let quoted = format!("{}\n> [[when]]", reply["text"].as_str().unwrap());
    session.run("compose.update", &json!({ "id": id, "text": quoted })).unwrap();
    let answer = session.run("compose.template", &json!({ "id": id, "name": "OK" })).unwrap();
    assert_eq!(answer["subject"], json!("Re: Allotment rota"));
    let text = answer["text"].as_str().unwrap();
    assert!(text.starts_with("Fine by me, Odile. [[when 2]]  [[blank]]\n\nOdile Brannock <odile@example.org> wrote on"), "{text}");
    assert_eq!(answer["blanks"], json!(["[[when 2]]", "[[blank]]"]));
}

#[test]
fn compose_template_refuses_what_is_not_there() {
    let mut session = session();
    save(&mut session, 1, "Ok", "", "Ok");
    let draft = draft_to(&mut session, "odile@example.org");
    assert!(bad(session.run("compose.template", &json!({ "id": draft, "name": "none" }))));
    assert!(matches!(session.run("compose.template", &json!({ "id": 999, "name": "Ok" })), Err(EngineError::NotFound { .. })));
    assert!(bad(session.run("compose.template", &json!({ "id": draft }))));
    // The text would grow past the longest.
    let full = session.run("compose.new", &json!({ "account": 2 })).unwrap()["id"].as_u64().unwrap();
    session.run("compose.update", &json!({ "id": full, "text": "t".repeat(super::compose::MAX_TEXT_BYTES - 1) })).unwrap();
    save(&mut session, 2, "Ok", "", "Ok");
    assert!(bad(session.run("compose.template", &json!({ "id": full, "name": "Ok" }))));
    // A draft on its way is not changed.
    session.run("mail.send", &json!({ "id": draft })).unwrap();
    assert!(bad(session.run("compose.template", &json!({ "id": draft, "name": "Ok" }))));
    assert_eq!(session.store().draft(DraftId::new(draft)).unwrap().unwrap().text, "\n\n-- \nAnna\nExample Ltd.");
}

#[test]
fn a_name_that_looks_like_a_blank_does_not_keep_send_blocked() {
    let mut session = session();
    save(&mut session, 1, "when", "", "{name}, which {?date}?");
    let id = draft_to(&mut session, "[[date]] <odd@example.org>");
    let answer = session.run("compose.template", &json!({ "id": id, "name": "when" })).unwrap();
    assert_eq!(answer["blanks"], json!(["[[date 2]]"]));
    let text = answer["text"].as_str().unwrap().replace("[[date 2]]", "Friday");
    let filled = session.run("compose.update", &json!({ "id": id, "text": text })).unwrap();
    assert!(filled["text"].as_str().unwrap().starts_with("[[date]], which Friday?"));
    assert_eq!(filled["sendable"], json!(true));
}
