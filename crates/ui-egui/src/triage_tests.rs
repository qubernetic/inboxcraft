// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The screener and the grouped inbox in the mailbox, read from the engine's `screener.*`, `group.*` and `mail.list`
//! over the sample, which holds two new senders before the inbox and groups its inbox in People and Updates.

use inboxcraft_engine::ScreenerMode;
use inboxcraft_model::Role;
use inboxcraft_time::{FakeClock, Timestamp};
use serde_json::json;

use super::{Act, GroupKind, InboxView, Mailbox, Place};
use crate::sample;

const SWIM: &str = "Winter swim at the harbour steps";
const PLANTS: &str = "Bare-root fruit trees are in";
const ROTA: &str = "Allotment rota for November";
const LOAN: &str = "Your library loan is due on Friday";

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

fn mailbox() -> Mailbox {
    Mailbox::new(sample::session(clock()).unwrap(), clock())
}

fn id_of(mailbox: &Mailbox, subject: &str) -> u64 {
    mailbox.messages().iter().find(|row| row.subject == subject).map(|row| row.id).unwrap()
}

fn group_of(mailbox: &Mailbox, subject: &str) -> Option<GroupKind> {
    mailbox.messages().iter().find(|row| row.subject == subject).and_then(|row| row.group)
}

fn senders(mailbox: &Mailbox) -> Vec<&str> {
    mailbox.waiting().iter().map(|waiting| waiting.address.as_str()).collect()
}

#[test]
fn the_rows_carry_their_group_and_the_inbox_groups_them() {
    let mailbox = mailbox();
    assert_eq!(group_of(&mailbox, ROTA), Some(GroupKind::People));
    assert_eq!(group_of(&mailbox, LOAN), Some(GroupKind::Updates));
    let groups: Vec<(String, usize)> = mailbox.groups().into_iter().map(|group| (group.name, group.messages.len())).collect();
    assert_eq!(groups, [("People".to_owned(), 8), ("Updates".to_owned(), 3)]);
    assert!(mailbox.messages().iter().all(|row| !row.new_sender));
}

#[test]
fn groups_show_in_the_unified_inbox_only() {
    let mut mailbox = mailbox();
    mailbox.open(Place::View(Role::Sent));
    assert!(mailbox.groups().is_empty());
    mailbox.open(Place::View(Role::Inbox));
    mailbox.search("rota");
    assert!(mailbox.groups().is_empty(), "a search is one list");
}

#[test]
fn the_groups_have_their_names_and_icons() {
    let names: Vec<(&str, &str)> = GroupKind::ALL.iter().map(|kind| (kind.id(), kind.text())).collect();
    assert_eq!(names, [("people", "People"), ("updates", "Updates"), ("newsletters", "Newsletters"), ("invitations", "Invitations")]);
    assert_eq!(GroupKind::named("newsletters"), Some(GroupKind::Newsletters));
    assert_eq!(GroupKind::named("spam"), None);
    assert_ne!(GroupKind::Newsletters.icon(), GroupKind::People.icon());
}

#[test]
fn the_screener_lists_the_senders_waiting_with_their_mail() {
    let mailbox = mailbox();
    assert_eq!(mailbox.screener_mode(), ScreenerMode::Before);
    assert_eq!(senders(&mailbox), ["club@example.org", "plants@example.org"]);
    let first = &mailbox.waiting()[0];
    assert_eq!(first.name, "Harbour Swim Club");
    assert_eq!(first.messages.len(), 1);
}

#[test]
fn new_senders_is_a_place_of_its_own_with_the_held_mail() {
    let mut mailbox = mailbox();
    mailbox.open(Place::Senders);
    assert_eq!(mailbox.title(), "New senders");
    let subjects: Vec<&str> = mailbox.messages().iter().map(|row| row.subject.as_str()).collect();
    assert_eq!(subjects, [SWIM, PLANTS]);
    assert!(mailbox.messages().iter().all(|row| row.new_sender));
    assert!(mailbox.groups().is_empty());
}

#[test]
fn a_preview_of_held_mail_leaves_it_unread() {
    let mut mailbox = mailbox();
    mailbox.open(Place::Senders);
    let swim = id_of(&mailbox, SWIM);
    mailbox.open_message(swim).unwrap();
    assert_eq!(mailbox.last_run(), Some("mail.read"));
    assert!(!mailbox.messages().iter().any(|row| row.id == swim && row.seen));
    assert!(!mailbox.can_undo());
}

#[test]
fn accept_lets_the_sender_in_with_a_group_and_undo_holds_it_again() {
    let mut mailbox = mailbox();
    mailbox.open(Place::Senders);
    let swim = id_of(&mailbox, SWIM);
    mailbox.act(swim, Act::Accept { group: Some(GroupKind::Updates), open: false });
    assert_eq!(mailbox.error(), None);
    assert_eq!(mailbox.undoable().map(|undo| undo.text.as_str()), Some("Accepted Harbour Swim Club into Updates"));
    assert_eq!(senders(&mailbox), ["plants@example.org"]);
    assert!(mailbox.messages().iter().all(|row| row.id != swim));
    mailbox.open(Place::View(Role::Inbox));
    assert_eq!(group_of(&mailbox, SWIM), Some(GroupKind::Updates));
    mailbox.undo();
    assert_eq!(senders(&mailbox), ["club@example.org", "plants@example.org"]);
    assert!(mailbox.messages().iter().all(|row| row.subject != SWIM));
}

#[test]
fn accept_without_a_group_says_so_and_accept_and_open_opens_it_in_the_inbox() {
    let mut mailbox = mailbox();
    mailbox.open(Place::Senders);
    let plants = id_of(&mailbox, PLANTS);
    mailbox.act(plants, Act::Accept { group: None, open: true });
    assert_eq!(mailbox.undoable().map(|undo| undo.text.as_str()), Some("Accepted Fernhill Nursery"));
    assert_eq!(mailbox.place(), Place::View(Role::Inbox));
    assert_eq!(mailbox.selected(), Some(plants));
    assert!(mailbox.messages().iter().any(|row| row.id == plants && row.seen), "the newest is opened, so read");
}

#[test]
fn block_files_the_held_mail_away_and_undo_brings_it_back() {
    let mut mailbox = mailbox();
    mailbox.open(Place::Senders);
    let plants = id_of(&mailbox, PLANTS);
    mailbox.act(plants, Act::Block);
    assert_eq!(mailbox.undoable().map(|undo| undo.text.as_str()), Some("Blocked Fernhill Nursery"));
    assert_eq!(senders(&mailbox), ["club@example.org"]);
    mailbox.open(Place::View(Role::Blocked));
    assert_eq!(mailbox.messages().iter().map(|row| row.subject.as_str()).collect::<Vec<_>>(), [PLANTS]);
    mailbox.undo();
    assert_eq!(senders(&mailbox), ["club@example.org", "plants@example.org"]);
}

#[test]
fn a_sender_moves_to_another_group_and_undo_moves_them_back() {
    let mut mailbox = mailbox();
    let rota = id_of(&mailbox, ROTA);
    mailbox.act(rota, Act::Group(Some(GroupKind::Newsletters)));
    assert_eq!(mailbox.undoable().map(|undo| undo.text.as_str()), Some("Odile Brannock now goes to Newsletters"));
    assert_eq!(group_of(&mailbox, ROTA), Some(GroupKind::Newsletters));
    assert_eq!(group_of(&mailbox, "Re: Soup recipe"), Some(GroupKind::Newsletters), "all her mail moves");
    mailbox.undo();
    assert_eq!(group_of(&mailbox, ROTA), Some(GroupKind::People));
    let loan = id_of(&mailbox, LOAN);
    mailbox.act(loan, Act::Group(Some(GroupKind::People)));
    mailbox.act(loan, Act::Group(None));
    assert_eq!(mailbox.undoable().map(|undo| undo.text.as_str()), Some("Riverside Library goes where InboxCraft sorts it"));
    assert_eq!(group_of(&mailbox, LOAN), Some(GroupKind::Updates));
}

#[test]
fn an_act_on_a_message_that_is_gone_shows_an_error() {
    let mut mailbox = mailbox();
    mailbox.act(9999, Act::Block);
    assert!(mailbox.error().is_some());
    mailbox.act(9999, Act::Group(None));
    assert!(mailbox.error().is_some());
}

#[test]
fn the_screener_mode_and_the_grouping_of_an_account_change_through_the_engine() {
    let mut mailbox = mailbox();
    mailbox.set_screener_mode(ScreenerMode::Inside);
    assert_eq!(mailbox.screener_mode(), ScreenerMode::Inside);
    assert!(mailbox.account(1).unwrap().grouped);
    mailbox.set_grouped(1, false);
    assert!(!mailbox.account(1).unwrap().grouped);
    assert_eq!(group_of(&mailbox, ROTA), None);
    assert_eq!(group_of(&mailbox, "Draft logo for the bakery"), Some(GroupKind::People), "the Studio stays grouped");
    let groups: Vec<String> = mailbox.groups().into_iter().map(|group| group.name).collect();
    assert_eq!(groups, ["People", "Updates"]);
    mailbox.set_grouped(9, false);
    assert!(mailbox.error().is_some());
}

#[test]
fn the_inbox_view_is_cards_until_changed() {
    let mut mailbox = mailbox();
    assert_eq!(mailbox.inbox_view(), InboxView::Cards);
    mailbox.set_inbox_view(InboxView::Simple);
    assert_eq!(mailbox.inbox_view(), InboxView::Simple);
    assert!(mailbox.groups().is_empty(), "the simple list has no groups");
    mailbox.set_inbox_view(InboxView::List);
    assert_eq!(mailbox.groups().len(), 2);
    let names: Vec<(&str, &str)> = InboxView::ALL.iter().map(|view| (view.id(), view.text())).collect();
    assert_eq!(names, [("cards", "Cards"), ("list", "Grouped list"), ("simple", "Simple list")]);
    assert_eq!(InboxView::named("list"), Some(InboxView::List));
    assert_eq!(InboxView::named("tiles"), None);
}

#[test]
fn a_new_sender_inside_the_inbox_is_marked_and_read_aloud_so() {
    let mut mailbox = mailbox();
    mailbox.open(Place::Senders);
    let row = mailbox.messages()[0].clone();
    assert_eq!(mailbox.spoken(&row), "Harbour Swim Club, Winter swim at the harbour steps, 06:00, unread, new sender");
    mailbox.run("screener.mode", &json!({ "mode": "off" })).unwrap();
    mailbox.refresh();
    assert!(mailbox.messages().iter().all(|row| !row.new_sender), "off lets everyone in");
}
