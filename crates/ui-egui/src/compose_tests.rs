// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_engine::Session;
use inboxcraft_model::{AccountId, Host, Security, Server};
use inboxcraft_store::{Batch, Change, Storage as _};
use inboxcraft_time::{FakeClock, Timestamp};
use serde_json::{Value, json};

use super::{Chip, Composer, Line, MAX_RECIPIENT_BYTES, MAX_RECIPIENTS, MAX_SUBJECT_BYTES, MAX_SUGGESTIONS, Suggestion};
use crate::mailbox::Mailbox;
use crate::sample;

/// 2026-11-05T09:00:00Z.
const NOW: i64 = 1_793_869_200;

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(NOW, 0).unwrap()))
}

fn mailbox() -> Mailbox {
    Mailbox::new(sample::session(clock()).unwrap(), clock())
}

/// A mailbox over the sample mail whose first account can send: a real session, not the sample's.
fn sending() -> Mailbox {
    let mut store = sample::mailbox(Timestamp::from_unix(NOW, 0).unwrap()).unwrap();
    let account = store.accounts().unwrap().into_iter().find(|account| account.id() == AccountId::new(1)).unwrap();
    let server = Server::new(Host::new("smtp.example.org").unwrap(), Security::Tls.submission_port(), Security::Tls);
    let mut batch = Batch::new();
    batch.push(Change::PutAccount(account.with_submission(Some(server))));
    store.apply(&batch).unwrap();
    Mailbox::new(Session::new(Box::new(store), clock()).unwrap(), clock())
}

fn shown(mailbox: &mut Mailbox, id: u64) -> Value {
    mailbox.run("compose.show", &json!({ "id": id })).unwrap()
}

fn type_in(composer: &mut Composer, mailbox: &mut Mailbox, line: Line, text: &str) {
    text.clone_into(composer.typing_mut(line).unwrap());
    composer.typed(mailbox, line).unwrap();
}

fn texts(composer: &Composer, line: Line) -> Vec<&str> {
    composer.chips(line).iter().map(|chip| chip.text.as_str()).collect()
}

#[test]
fn a_new_composer_keeps_its_draft_at_once() {
    let mut mailbox = mailbox();
    let composer = Composer::start(&mut mailbox).unwrap();
    let drafts = mailbox.run("compose.list", &json!({})).unwrap();
    assert_eq!(drafts["drafts"][0]["id"], json!(composer.draft()));
    assert_eq!(composer.account(), 1, "the account that last sent");
    assert_eq!(composer.title(), "New message");
    assert!(!composer.sendable(), "no recipient yet");
    assert_eq!(composer.problem().as_deref(), Some("Add a recipient to send"));
    assert!(!composer.copies() && !composer.floating() && !composer.changed());
}

#[test]
fn an_address_taken_becomes_a_chip_the_engine_checked() {
    let mut mailbox = mailbox();
    let mut composer = Composer::start(&mut mailbox).unwrap();
    "odile@example.org".clone_into(composer.typing_mut(Line::To).unwrap());
    composer.take(&mut mailbox, Line::To).unwrap();
    assert_eq!(composer.chips(Line::To), [Chip { text: "odile@example.org".to_owned(), ok: true }]);
    assert_eq!(composer.typing(Line::To), "");
    assert!(composer.sendable());
    assert_eq!(composer.problem(), None);
    assert_eq!(shown(&mut mailbox, composer.draft())["to"][0]["text"], json!("odile@example.org"));
}

#[test]
fn an_impossible_address_is_marked_at_once_and_blocks_send() {
    let mut mailbox = sending();
    let mut composer = Composer::start(&mut mailbox).unwrap();
    type_in(&mut composer, &mut mailbox, Line::To, "odile@example.org, ansel.example.org,");
    assert_eq!(texts(&composer, Line::To), ["odile@example.org", "ansel.example.org"]);
    assert!(composer.chips(Line::To)[0].ok);
    assert!(!composer.chips(Line::To)[1].ok);
    assert_eq!(composer.chips(Line::To)[1].spoken(), "ansel.example.org, not an address");
    assert!(!composer.sendable());
    assert_eq!(composer.problem().as_deref(), Some("ansel.example.org is not an address"));
    assert!(!composer.send(&mut mailbox).unwrap(), "a blocked draft is not sent");
    assert_ne!(mailbox.last_run(), Some("mail.send"));
    composer.remove(&mut mailbox, Line::To, 1).unwrap();
    assert!(composer.sendable());
}

#[test]
fn commas_semicolons_and_line_breaks_end_a_recipient_and_the_rest_stays_typed() {
    let mut mailbox = mailbox();
    let mut composer = Composer::start(&mut mailbox).unwrap();
    type_in(&mut composer, &mut mailbox, Line::To, "a@example.org; b@example.org\nc@exa");
    assert_eq!(texts(&composer, Line::To), ["a@example.org", "b@example.org"]);
    assert_eq!(composer.typing(Line::To), "c@exa");
    type_in(&mut composer, &mut mailbox, Line::To, "c@example.org");
    assert_eq!(texts(&composer, Line::To), ["a@example.org", "b@example.org"], "no separator, no chip");
}

#[test]
fn a_recipient_is_there_once_across_the_lines() {
    let mut mailbox = mailbox();
    let mut composer = Composer::start(&mut mailbox).unwrap();
    type_in(&mut composer, &mut mailbox, Line::To, "Odile <odile@example.org>, ODILE@example.org,");
    composer.show_copies();
    type_in(&mut composer, &mut mailbox, Line::Cc, "odile@example.org;");
    assert_eq!(texts(&composer, Line::To), ["Odile <odile@example.org>"]);
    assert!(composer.chips(Line::Cc).is_empty());
    assert!(composer.copies());
}

#[test]
fn suggestions_are_the_people_in_the_mail_and_enter_takes_the_highlighted_one() {
    let mut mailbox = mailbox();
    let mut composer = Composer::start(&mut mailbox).unwrap();
    type_in(&mut composer, &mut mailbox, Line::To, "odi");
    let (line, found) = composer.suggestions();
    assert_eq!(line, Some(Line::To));
    assert_eq!(found, [Suggestion { name: "Odile Brannock".to_owned(), address: "odile@example.org".to_owned() }]);
    assert_eq!(composer.highlighted(), None);
    composer.step(true);
    assert_eq!(composer.highlighted(), Some(0));
    composer.step(true);
    assert_eq!(composer.highlighted(), Some(0), "stays at the end");
    composer.take(&mut mailbox, Line::To).unwrap();
    assert_eq!(texts(&composer, Line::To), ["Odile Brannock <odile@example.org>"]);
    assert_eq!(composer.chips(Line::To)[0].shown(), "Odile Brannock");
    assert_eq!(composer.chips(Line::To)[0].spoken(), "Odile Brannock, odile@example.org");
    assert_eq!(composer.typing(Line::To), "");
    assert_eq!(composer.suggestions().1, []);
}

#[test]
fn without_a_highlight_enter_takes_what_was_typed() {
    let mut mailbox = mailbox();
    let mut composer = Composer::start(&mut mailbox).unwrap();
    type_in(&mut composer, &mut mailbox, Line::To, "odi");
    composer.step(true);
    composer.step(false);
    assert_eq!(composer.highlighted(), None, "up from the first takes the highlight off");
    composer.take(&mut mailbox, Line::To).unwrap();
    assert_eq!(texts(&composer, Line::To), ["odi"]);
    assert!(!composer.chips(Line::To)[0].ok);
}

#[test]
fn the_owners_own_addresses_and_the_recipients_there_are_not_suggested() {
    let mut mailbox = mailbox();
    let mut composer = Composer::start(&mut mailbox).unwrap();
    type_in(&mut composer, &mut mailbox, Line::To, "example.org");
    let addresses: Vec<String> = composer.suggestions().1.iter().map(|found| found.address.clone()).collect();
    assert_eq!(addresses.len(), MAX_SUGGESTIONS);
    assert!(!addresses.iter().any(|address| address == "mira@example.org" || address == "holloway.studio@example.org"));
    composer.pick(&mut mailbox, 0).unwrap();
    type_in(&mut composer, &mut mailbox, Line::To, "example.org");
    assert!(!composer.suggestions().1.iter().any(|found| found.address == addresses[0]));
    composer.pick(&mut mailbox, 99).unwrap();
    assert_eq!(composer.chips(Line::To).len(), 1, "no suggestion there, nothing taken");
    type_in(&mut composer, &mut mailbox, Line::To, "nobody-at-all");
    assert_eq!(composer.suggestions(), (None, &[][..]));
    type_in(&mut composer, &mut mailbox, Line::To, "  ");
    assert_eq!(composer.suggestions(), (None, &[][..]));
}

#[test]
fn a_suggestion_whose_name_could_move_the_address_is_taken_as_the_address_alone() {
    let plain = Suggestion { name: "Odile Brannock".to_owned(), address: "odile@example.org".to_owned() };
    assert_eq!(plain.recipient(), "Odile Brannock <odile@example.org>");
    assert_eq!(plain.shown(), "Odile Brannock <odile@example.org>");
    for name in ["", "  ", "Eve <eve@example.net>", "Eve, Mallory", "\"Eve\"", "eve@example.net"] {
        let odd = Suggestion { name: name.to_owned(), address: "odile@example.org".to_owned() };
        assert_eq!(odd.recipient(), "odile@example.org", "{name:?}");
    }
    assert_eq!(Suggestion { name: String::new(), address: "odile@example.org".to_owned() }.shown(), "odile@example.org");
}

#[test]
fn backspace_in_an_empty_line_removes_its_last_chip() {
    let mut mailbox = mailbox();
    let mut composer = Composer::start(&mut mailbox).unwrap();
    type_in(&mut composer, &mut mailbox, Line::To, "a@example.org, b@example.org,");
    composer.remove_last(&mut mailbox, Line::To).unwrap();
    assert_eq!(texts(&composer, Line::To), ["a@example.org"]);
    composer.remove_last(&mut mailbox, Line::To).unwrap();
    composer.remove_last(&mut mailbox, Line::To).unwrap();
    assert!(composer.chips(Line::To).is_empty());
    composer.remove(&mut mailbox, Line::Bcc, 3).unwrap();
    assert_eq!(shown(&mut mailbox, composer.draft())["to"], json!([]));
}

#[test]
fn closing_takes_what_is_typed_and_keeps_every_word() {
    let mut mailbox = mailbox();
    let mut composer = Composer::start(&mut mailbox).unwrap();
    "teodor@example.org".clone_into(composer.typing_mut(Line::To).unwrap());
    composer.show_copies();
    "petra@example.org".clone_into(composer.typing_mut(Line::Bcc).unwrap());
    "Work party".clone_into(composer.subject_mut());
    "We meet at ten.".clone_into(composer.text_mut());
    composer.edited();
    assert!(composer.changed());
    composer.close(&mut mailbox).unwrap();
    assert!(!composer.changed());
    let kept = shown(&mut mailbox, composer.draft());
    assert_eq!(kept["to"][0]["text"], json!("teodor@example.org"));
    assert_eq!(kept["bcc"][0]["text"], json!("petra@example.org"));
    assert_eq!(kept["subject"], json!("Work party"));
    assert_eq!(kept["text"], json!("We meet at ten."));
    let again = Composer::open(&mut mailbox, composer.draft()).unwrap();
    assert_eq!(again.title(), "Work party");
    assert!(again.copies(), "a draft with a Bcc shows the copy lines");
    assert_eq!(again.text(), "We meet at ten.");
}

#[test]
fn another_account_brings_its_signature() {
    let mut mailbox = mailbox();
    mailbox.run("compose.signature", &json!({ "account": 2, "signature": "Mira\nStudio" })).unwrap();
    let mut composer = Composer::start(&mut mailbox).unwrap();
    "Hello".clone_into(composer.text_mut());
    composer.edited();
    composer.set_account(&mut mailbox, 2).unwrap();
    assert_eq!(composer.account(), 2);
    assert_eq!(composer.text(), "Hello\n\n-- \nMira\nStudio");
    assert!(composer.set_account(&mut mailbox, 9).is_err());
}

#[test]
fn send_keeps_the_draft_first_and_sends_it() {
    let mut mailbox = sending();
    let mut composer = Composer::start(&mut mailbox).unwrap();
    "odile@example.org".clone_into(composer.typing_mut(Line::To).unwrap());
    "Hi".clone_into(composer.subject_mut());
    composer.edited();
    assert!(composer.send(&mut mailbox).unwrap());
    assert_eq!(mailbox.last_run(), Some("mail.send"));
    assert!(composer.send(&mut mailbox).is_err(), "a draft on its way is not changed");
}

#[test]
fn the_sample_cannot_send_and_says_why() {
    let mut mailbox = mailbox();
    let mut composer = Composer::start(&mut mailbox).unwrap();
    "odile@example.org".clone_into(composer.typing_mut(Line::To).unwrap());
    let error = composer.send(&mut mailbox).unwrap_err().to_string();
    assert!(error.contains("submission"), "{error}");
}

#[test]
fn discard_removes_the_draft() {
    let mut mailbox = mailbox();
    let composer = Composer::start(&mut mailbox).unwrap();
    composer.discard(&mut mailbox).unwrap();
    assert_eq!(mailbox.run("compose.list", &json!({})).unwrap()["drafts"], json!([]));
    assert!(Composer::open(&mut mailbox, composer.draft()).is_err());
    assert!(composer.discard(&mut mailbox).is_err());
}

#[test]
fn what_does_not_fit_is_cut_to_what_the_engine_keeps() {
    let mut mailbox = mailbox();
    let mut composer = Composer::start(&mut mailbox).unwrap();
    *composer.subject_mut() = format!("a{}", "ő".repeat(MAX_SUBJECT_BYTES));
    composer.edited();
    assert!(composer.subject().len() <= MAX_SUBJECT_BYTES);
    assert!(composer.subject().chars().skip(1).all(|c| c == 'ő'), "cut at a character's start");
    let long = format!("{}@example.org", "a".repeat(MAX_RECIPIENT_BYTES * 2));
    let many: Vec<String> = (0..MAX_RECIPIENTS + 5).map(|n| format!("p{n}@example.org")).collect();
    type_in(&mut composer, &mut mailbox, Line::To, &format!("{long},{},", many.join(",")));
    assert_eq!(composer.chips(Line::To).len(), MAX_RECIPIENTS);
    assert!(composer.chips(Line::To).iter().all(|chip| chip.text.len() <= MAX_RECIPIENT_BYTES));
    type_in(&mut composer, &mut mailbox, Line::Cc, "x\u{0}y\r@example.org;");
    assert_eq!(texts(&composer, Line::Cc), ["xy", "@example.org"], "control characters go; a line break ends a recipient");
}

#[test]
fn an_answer_without_a_draft_is_refused() {
    assert!(Composer::over("compose.new", &json!({ "account": 1 })).is_err());
    let composer = Composer::over("compose.new", &json!({ "id": 4, "to": [{ "ok": true }, 7], "blocks": [3] })).unwrap();
    assert!(composer.chips(Line::To).is_empty());
    assert!(composer.sendable());
}

#[test]
fn chips_show_the_name_or_the_address_fit_for_one_line() {
    let chip = |text: &str| Chip { text: text.to_owned(), ok: true };
    assert_eq!(chip("\"Odile B\" <odile@example.org>").shown(), "Odile B");
    assert_eq!(chip("<odile@example.org>").shown(), "odile@example.org");
    assert_eq!(chip("odile@example.org").address(), "odile@example.org");
    assert_eq!(chip("Odile\u{202E} <odile@example.org>").shown(), "Odile");
    assert_eq!(Line::Bcc.name(), "Bcc");
    assert_eq!(Line::Cc.key(), "cc");
}

#[test]
fn the_highlight_moves_within_the_suggestions_and_without_any_there_is_none() {
    let mut mailbox = mailbox();
    let mut composer = Composer::start(&mut mailbox).unwrap();
    composer.step(true);
    assert_eq!(composer.highlighted(), None, "nothing to highlight");
    type_in(&mut composer, &mut mailbox, Line::To, "example.org");
    composer.step(false);
    assert_eq!(composer.highlighted(), None, "up from none stays at none");
    composer.step(true);
    composer.step(true);
    composer.step(true);
    assert_eq!(composer.highlighted(), Some(2));
    composer.step(false);
    assert_eq!(composer.highlighted(), Some(1));
    composer.dismiss_suggestions();
    assert_eq!((composer.suggestions(), composer.highlighted()), ((None, &[][..]), None));
}
