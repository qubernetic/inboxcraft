// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_engine::{CommandSpec, Context, EngineError, Outcome, Registry, Session};
use inboxcraft_model::Role;
use inboxcraft_store::{Memory, StoreConfig};
use inboxcraft_time::{FakeClock, Offset, Timestamp};
use serde_json::{Value, json};

use super::{Act, After, MAX_QUERY, MAX_SHOWN, Mailbox, Place, Showcase, shown, when};
use crate::sample;

/// 2026-11-05T09:00:00Z, a Thursday.
const NOW: i64 = 1_793_869_200;

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_unix(seconds, 0).unwrap()
}

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(at(NOW)))
}

fn mailbox() -> Mailbox {
    Mailbox::new(sample::session(clock()).unwrap(), clock())
}

fn subjects(mailbox: &Mailbox) -> Vec<&str> {
    mailbox.messages().iter().map(|row| row.subject.as_str()).collect()
}

#[test]
fn it_opens_on_the_unified_inbox_of_every_account() {
    let mailbox = mailbox();
    assert_eq!(mailbox.error(), None);
    assert_eq!(mailbox.place(), Place::View(Role::Inbox));
    assert_eq!(mailbox.narrowed(), None);
    assert_eq!(mailbox.title(), "Inbox");
    assert_eq!(mailbox.subtitle(), "All accounts");
    assert_eq!(format!("{mailbox:?}"), "Mailbox { narrowed: None, place: View(Inbox), messages: 11, .. }");
    assert_eq!(mailbox.messages().len(), 11);
    assert_eq!(mailbox.messages()[0].subject, "Allotment rota for November");
    assert_eq!(mailbox.messages()[0].sender, "Odile Brannock");
    assert!(!mailbox.messages()[0].seen);
    assert!(mailbox.messages().iter().any(|row| row.account == 2));
}

#[test]
fn accounts_carry_their_label_address_and_colour_index() {
    let mailbox = mailbox();
    let accounts: Vec<(u64, &str, &str, usize)> = mailbox.accounts().iter().map(|a| (a.id, a.label.as_str(), a.address.as_str(), a.index)).collect();
    assert_eq!(accounts, [(1, "Personal", "mira@example.org", 0), (2, "Studio", "holloway.studio@example.org", 1)]);
    assert_eq!(mailbox.account(2).map(|a| a.label.as_str()), Some("Studio"));
    assert!(mailbox.account(9).is_none());
}

#[test]
fn the_unified_views_sum_the_unread_of_their_folders() {
    let mailbox = mailbox();
    let views: Vec<(Role, u64)> = mailbox.views().iter().map(|view| (view.role, view.unread)).collect();
    assert_eq!(&views[..3], &[(Role::Inbox, 4), (Role::Drafts, 0), (Role::Sent, 0)]);
    assert!(views.iter().any(|(role, unread)| *role == Role::Junk && *unread == 1));
}

#[test]
fn folders_come_by_account_roles_first_then_by_path_with_depth() {
    let mailbox = mailbox();
    let first: Vec<(&str, usize, Option<Role>)> = mailbox.folders_of(1).iter().map(|f| (f.leaf.as_str(), f.depth, f.role)).collect();
    assert_eq!(
        first,
        [
            ("INBOX", 0, Some(Role::Inbox)),
            ("Drafts", 0, Some(Role::Drafts)),
            ("Sent", 0, Some(Role::Sent)),
            ("Archive", 0, Some(Role::Archive)),
            ("Junk", 0, Some(Role::Junk)),
            ("Trash", 0, Some(Role::Trash)),
            ("Receipts", 0, None),
            ("Travel", 0, None),
            ("2026", 1, None),
        ]
    );
    let inbox = &mailbox.folders_of(2)[0];
    assert_eq!((inbox.id, inbox.unread, inbox.messages), (10, 2, 4));
    assert!(mailbox.folders_of(9).is_empty());
}

#[test]
fn narrowing_shows_one_account_with_its_address_in_the_header() {
    let mut mailbox = mailbox();
    mailbox.narrow(Some(2));
    assert_eq!(mailbox.narrowed(), Some(2));
    assert_eq!(mailbox.subtitle(), "holloway.studio@example.org");
    assert!(mailbox.messages().iter().all(|row| row.account == 2));
    assert_eq!(mailbox.messages().len(), 4);
    let inbox = mailbox.views().iter().find(|view| view.role == Role::Inbox).map(|view| view.unread);
    assert_eq!(inbox, Some(2));
    assert!(mailbox.views().iter().all(|view| view.role != Role::Junk), "account 2 has no junk folder");
    mailbox.narrow(None);
    assert_eq!(mailbox.messages().len(), 11);
}

#[test]
fn narrowing_away_from_an_open_folder_goes_back_to_the_inbox() {
    let mut mailbox = mailbox();
    mailbox.open(Place::Folder(7));
    assert_eq!(subjects(&mailbox), ["Receipt for your order 4417"]);
    assert_eq!(mailbox.title(), "Receipts");
    mailbox.narrow(Some(1));
    assert_eq!(mailbox.place(), Place::Folder(7), "the folder belongs to the account");
    mailbox.narrow(Some(2));
    assert_eq!(mailbox.place(), Place::View(Role::Inbox));
    mailbox.narrow(Some(42));
    assert_eq!(mailbox.narrowed(), None, "an account that is not there narrows nothing");
}

#[test]
fn a_view_or_a_folder_can_be_opened() {
    let mut mailbox = mailbox();
    mailbox.open(Place::View(Role::Sent));
    assert_eq!(mailbox.title(), "Sent");
    assert_eq!(subjects(&mailbox), ["Re: Allotment rota for November", "Re: Draft logo for the bakery", "Weekend plans"]);
    mailbox.open(Place::Folder(9));
    assert_eq!(mailbox.title(), "2026");
    assert_eq!(subjects(&mailbox), ["Your ferry booking is confirmed"]);
    mailbox.open(Place::Folder(4242));
    assert!(mailbox.error().is_some(), "a folder that is not there is an error");
    assert_eq!(mailbox.place(), Place::Folder(9), "and the list stays where it was");
    assert_eq!(subjects(&mailbox), ["Your ferry booking is confirmed"]);
    mailbox.open(Place::View(Role::Inbox));
    assert_eq!(mailbox.error(), None, "a good command clears the error");
}

#[test]
fn the_search_looks_at_subject_sender_and_recipient_across_folders() {
    let mut mailbox = mailbox();
    mailbox.search("logo");
    assert_eq!(mailbox.query(), "logo");
    assert_eq!(subjects(&mailbox), ["Re: Draft logo for the bakery", "Draft logo for the bakery"]);
    assert_eq!(mailbox.title(), "Search");
    mailbox.search("ODILE");
    assert_eq!(subjects(&mailbox), ["Re: Allotment rota for November", "Allotment rota for November", "Re: Soup recipe"]);
    mailbox.narrow(Some(2));
    assert_eq!(mailbox.query(), "ODILE", "narrowing keeps the search");
    assert!(mailbox.messages().is_empty());
    mailbox.search("   ");
    assert_eq!(mailbox.query(), "");
    assert_eq!(mailbox.title(), "Inbox");
    assert_eq!(mailbox.messages().len(), 4);
}

#[test]
fn opening_a_place_ends_the_search() {
    let mut mailbox = mailbox();
    mailbox.search("logo");
    mailbox.open(Place::View(Role::Archive));
    assert_eq!(mailbox.query(), "");
    assert_eq!(subjects(&mailbox), ["Minutes of the studio meeting", "Tenancy renewal"]);
}

#[test]
fn a_hostile_query_is_cut_and_never_panics() {
    let mut mailbox = mailbox();
    let long = "é".repeat(MAX_QUERY * 3);
    mailbox.search(&long);
    assert_eq!(mailbox.query().chars().count(), MAX_QUERY);
    assert!(mailbox.messages().is_empty());
    for junk in ["\0", "\r\n", "%", "_", "\u{202e}gol", "'; DROP TABLE messages; --", "\"", "*"] {
        mailbox.search(junk);
        assert_eq!(mailbox.error(), None, "{junk:?}");
    }
}

#[test]
fn a_row_can_be_selected_and_the_selection_survives_only_while_shown() {
    let mut mailbox = mailbox();
    let id = mailbox.messages()[1].id;
    mailbox.select(Some(id));
    assert_eq!(mailbox.selected(), Some(id));
    mailbox.select(Some(9999));
    assert_eq!(mailbox.selected(), Some(id), "a message not in the list is not selected");
    mailbox.open(Place::View(Role::Sent));
    assert_eq!(mailbox.selected(), None);
    mailbox.select(None);
    assert_eq!(mailbox.selected(), None);
}

#[test]
fn a_row_reads_aloud_with_sender_subject_date_and_state() {
    let mailbox = mailbox();
    let row = &mailbox.messages()[0];
    assert_eq!(mailbox.when(row), "08:35");
    assert_eq!(mailbox.spoken(row), "Odile Brannock, Allotment rota for November, 08:35, unread");
    let flagged = mailbox.messages().iter().find(|row| row.subject == "Photos from the coast walk").unwrap();
    assert_eq!(mailbox.spoken(flagged), "Teodor Lisk, Photos from the coast walk, 04:00, flagged");
}

#[test]
fn an_empty_mailbox_has_no_accounts_and_no_error() {
    let session = Session::new(Box::new(Memory::new(StoreConfig::default())), clock()).unwrap();
    let mailbox = Mailbox::new(session, clock());
    assert_eq!(mailbox.error(), None);
    assert!(mailbox.accounts().is_empty());
    assert!(mailbox.views().is_empty());
    assert!(mailbox.messages().is_empty());
    assert_eq!(mailbox.subtitle(), "All accounts");
}

fn broken(_: &Context<'_>, _: &Value) -> Result<Outcome, EngineError> {
    Ok(Outcome::new(json!({ "unexpected": true })))
}

fn failing(_: &Context<'_>, _: &Value) -> Result<Outcome, EngineError> {
    Err(EngineError::NotFound { what: "account", id: 1 })
}

fn session_with(commands: &[(&'static str, inboxcraft_engine::Run)]) -> Session {
    let mut registry = Registry::new();
    for (id, run) in commands {
        registry.register(CommandSpec { id, class: Some(inboxcraft_engine::AgentClass::Safe), summary: "test", run: *run }).unwrap();
    }
    Session::with_registry(Box::new(sample::mailbox(at(NOW)).unwrap()), clock(), registry).unwrap()
}

#[test]
fn an_answer_of_the_wrong_shape_becomes_an_error_not_a_panic() {
    for id in ["account.list", "folder.list", "mail.list"] {
        let commands: Vec<(&'static str, inboxcraft_engine::Run)> = ["account.list", "folder.list", "mail.list", "mail.search"]
            .iter()
            .map(|c| (*c, if *c == id { broken as _ } else { failing as _ }))
            .collect();
        let mailbox = Mailbox::new(session_with(&commands), clock());
        assert!(mailbox.error().is_some(), "{id}");
    }
    let mut mailbox = Mailbox::new(session_with(&[("mail.search", broken)]), clock());
    mailbox.search("x");
    assert!(mailbox.error().unwrap().contains("unexpected shape"), "{:?}", mailbox.error());
}

#[test]
fn a_failing_command_is_shown_and_can_be_dismissed() {
    let mut mailbox = Mailbox::new(session_with(&[("account.list", failing)]), clock());
    assert!(mailbox.error().is_some());
    mailbox.dismiss_error();
    assert_eq!(mailbox.error(), None);
}

#[test]
fn dates_read_by_how_long_ago_they_are() {
    let now = at(NOW);
    let utc = Offset::UTC;
    assert_eq!(when(at(NOW - 25 * 60), now, utc), "08:35");
    assert_eq!(when(at(NOW - 9 * 3600 - 1), now, utc), "Yesterday", "23:59:59 the day before");
    assert_eq!(when(at(NOW - 3 * 86_400), now, utc), "Monday");
    assert_eq!(when(at(NOW - 6 * 86_400), now, utc), "Friday");
    assert_eq!(when(at(NOW - 7 * 86_400), now, utc), "29 Oct");
    assert_eq!(when(at(NOW - 300 * 86_400), now, utc), "9 Jan");
    assert_eq!(when(at(NOW - 400 * 86_400), now, utc), "1 Oct 2025");
    assert_eq!(when(at(NOW + 3600), now, utc), "10:00", "later today, from a clock running ahead");
    assert_eq!(when(at(NOW + 3 * 86_400), now, utc), "8 Nov", "a future day shows its date");
    let east = Offset::from_minutes(5 * 60).unwrap();
    assert_eq!(when(at(NOW - 25 * 60), now, east), "13:35");
    let far_east = Offset::from_minutes(15 * 60).unwrap();
    assert_eq!(when(at(NOW - 25 * 60), now, far_east), "Yesterday", "just before midnight there, and now is past it");
    assert_eq!(when(Timestamp::MIN, now, utc), "1 Jan 0");
    for month in 1..=12 {
        let civil = inboxcraft_time::CivilDateTime::new(2024, month, 15, 12, 0, 0).unwrap();
        assert!(!when(civil.to_timestamp(utc).unwrap(), now, utc).is_empty());
    }
}

#[test]
fn a_message_without_a_date_shows_no_date() {
    let mailbox = mailbox();
    let mut row = mailbox.messages()[0].clone();
    row.date = None;
    assert_eq!(mailbox.when(&row), "");
    assert_eq!(mailbox.spoken(&row), "Odile Brannock, Allotment rota for November, unread");
}

#[test]
fn a_sender_without_a_name_shows_the_address_and_no_sender_shows_a_placeholder() {
    let mailbox = mailbox();
    assert_eq!(super::sender(&json!([{ "name": null, "address": "x@example.org" }])), "x@example.org");
    assert_eq!(super::sender(&json!([{ "name": "", "address": "x@example.org" }])), "x@example.org");
    assert_eq!(super::sender(&json!([])), "(no sender)");
    let mut row = mailbox.messages()[0].clone();
    row.subject = String::new();
    assert_eq!(mailbox.spoken(&row), "Odile Brannock, (no subject), 08:35, unread");
}

#[test]
fn server_text_is_flattened_to_one_line_without_bidi_controls_and_cut() {
    assert_eq!(shown("Hello\r\nBcc: thief@example.org\t\0end"), "Hello  Bcc: thief@example.org  end");
    assert_eq!(shown("invoice\u{202e}fdp.exe"), "invoicefdp.exe");
    assert_eq!(shown("\u{2066}a\u{2069}\u{200f}b\u{061c}"), "ab");
    assert_eq!(shown(&"x".repeat(MAX_SHOWN * 2)).len(), MAX_SHOWN);
    assert_eq!(shown("Ünnep ✓ 🌊"), "Ünnep ✓ 🌊");
    assert_eq!(super::sender(&json!([{ "name": "Eve\nAdmin", "address": "eve@example.org" }])), "Eve Admin");
}

/// The sample inbox with the library notice selected, then that notice expunged on the server and the list read
/// again under `after`.
fn removed_under(after: After) -> Mailbox {
    use inboxcraft_model::{AccountId, Event, EventKind, FolderId, FolderState, Uid, UidSet, UidValidity};
    let mut mailbox = mailbox();
    let library = mailbox.messages().iter().find(|row| row.subject.starts_with("Your library loan")).unwrap().id;
    mailbox.select(Some(library));
    mailbox.set_after(after);
    assert_eq!(mailbox.after(), after);
    let validity = UidValidity::new(1).unwrap();
    let synced = EventKind::FolderSynced { folder: FolderId::new(1), state: FolderState::new(validity) };
    mailbox.session.apply(&Event::new(AccountId::new(1), None, synced)).unwrap();
    let gone = EventKind::MessagesExpunged { folder: FolderId::new(1), uid_validity: validity, uids: UidSet::from_iter([Uid::new(2).unwrap()]) };
    mailbox.session.apply(&Event::new(AccountId::new(1), None, gone)).unwrap();
    mailbox.refresh();
    assert!(mailbox.messages().iter().all(|row| row.id != library));
    mailbox
}

fn selected_subject(mailbox: &Mailbox) -> Option<String> {
    let id = mailbox.selected()?;
    mailbox.messages().iter().find(|row| row.id == id).map(|row| row.subject.clone())
}

#[test]
fn a_message_that_leaves_the_list_gives_way_by_the_next_message_rule() {
    assert_eq!(Mailbox::new(sample::session(clock()).unwrap(), clock()).after(), After::Next, "next by default");
    assert_eq!(selected_subject(&removed_under(After::Next)).as_deref(), Some("Invoice 2026-031 is paid"));
    assert_eq!(selected_subject(&removed_under(After::Previous)).as_deref(), Some("Draft logo for the bakery"));
    assert_eq!(removed_under(After::List).selected(), None);
}

#[test]
fn the_rule_stays_inside_the_list_at_either_end() {
    let mut mailbox = mailbox();
    mailbox.open(Place::Folder(9));
    let only = mailbox.messages()[0].id;
    mailbox.select(Some(only));
    mailbox.refresh();
    assert_eq!(mailbox.selected(), Some(only), "a message still there stays selected");
}

#[test]
fn step_moves_the_selection_down_and_up_and_stops_at_the_ends() {
    let mut mailbox = mailbox();
    mailbox.step(false);
    assert_eq!(selected_subject(&mailbox).as_deref(), Some("Allotment rota for November"), "the first row without a selection");
    mailbox.step(false);
    assert_eq!(selected_subject(&mailbox).as_deref(), Some("Allotment rota for November"));
    mailbox.step(true);
    assert_eq!(selected_subject(&mailbox).as_deref(), Some("Draft logo for the bakery"));
    for _ in 0..20 {
        mailbox.step(true);
    }
    assert_eq!(mailbox.selected(), mailbox.messages().last().map(|row| row.id));
    mailbox.open(Place::Folder(8));
    mailbox.step(true);
    assert_eq!(mailbox.selected(), None, "an empty list selects nothing");
}

#[test]
fn the_expanded_header_gives_the_date_in_full() {
    let mailbox = mailbox();
    assert_eq!(mailbox.full_date(at(NOW - 25 * 60)), "Thursday 5 Nov 2026, 08:35");
    assert_eq!(mailbox.short_date(at(NOW - 25 * 60)), "08:35");
}

#[test]
fn a_row_keeps_its_sender_s_address_for_the_avatar() {
    let mailbox = mailbox();
    let first = mailbox.messages().first().unwrap();
    assert_eq!((first.sender.as_str(), first.address.as_str()), ("Odile Brannock", "odile@example.org"));
}

#[test]
fn a_keyboard_step_marks_the_selection_as_keyed_and_a_click_does_not() {
    let mut mailbox = mailbox();
    assert!(!mailbox.keyed());
    mailbox.step(true);
    assert!(mailbox.keyed());
    let second = mailbox.messages().get(1).unwrap().id;
    mailbox.select(Some(second));
    assert!(!mailbox.keyed());
    mailbox.step(false);
    assert!(mailbox.keyed());
    mailbox.select(None);
    assert!(!mailbox.keyed());
}

#[test]
fn the_showcase_is_empty_until_ui_shot_fills_it() {
    let mut mailbox = mailbox();
    assert_eq!(mailbox.showcase(), &Showcase::default());
    assert!(mailbox.showcase().is_empty());
    let showcase = sample::showcase(&mailbox);
    assert!(!showcase.screener.is_empty() && !showcase.groups.is_empty());
    mailbox.set_showcase(showcase.clone());
    assert_eq!(mailbox.showcase(), &showcase);
    assert!(!mailbox.showcase().is_empty());
}

#[test]
fn the_sample_showcase_groups_people_and_updates_and_leaves_one_row_out() {
    let mailbox = mailbox();
    let showcase = sample::showcase(&mailbox);
    let names: Vec<&str> = showcase.groups.iter().map(|group| group.name.as_str()).collect();
    assert_eq!(names, ["People", "Updates"]);
    let grouped: Vec<u64> = showcase.groups.iter().flat_map(|group| group.messages.iter().copied()).collect();
    assert_eq!(grouped.len(), mailbox.messages().len() - 1);
    let left: Vec<&str> = mailbox.messages().iter().filter(|row| !grouped.contains(&row.id)).map(|row| row.sender.as_str()).collect();
    assert_eq!(left, ["Kasimir Vell"]);
    assert!(showcase.screener.iter().all(|waiting| waiting.address.ends_with("@example.org")));
}

fn id_of(mailbox: &Mailbox, subject: &str) -> u64 {
    mailbox.messages().iter().find(|row| row.subject == subject).map(|row| row.id).unwrap()
}

/// Whether the row of `id` is listed, and whether it is read.
fn listed(mailbox: &Mailbox, id: u64) -> Option<bool> {
    mailbox.messages().iter().find(|row| row.id == id).map(|row| row.seen)
}

fn flagged(mailbox: &Mailbox, id: u64) -> Option<bool> {
    mailbox.messages().iter().find(|row| row.id == id).map(|row| row.flagged)
}

fn inbox_unread(mailbox: &Mailbox) -> u64 {
    mailbox.views().iter().find(|view| view.role == Role::Inbox).map_or(0, |view| view.unread)
}

fn folder_named(mailbox: &Mailbox, account: u64, leaf: &str) -> u64 {
    mailbox.folders_of(account).iter().find(|folder| folder.leaf == leaf).map(|folder| folder.id).unwrap()
}

fn toast(mailbox: &Mailbox) -> Option<(Act, u64, u64, &str, bool)> {
    mailbox.undoable().map(|undo| (undo.act, undo.id, undo.serial, undo.text.as_str(), undo.undoable))
}

const ROTA: &str = "Allotment rota for November";
const PHOTOS: &str = "Photos from the coast walk";
const LOGO: &str = "Draft logo for the bakery";

#[test]
fn read_and_unread_change_the_row_and_undo_brings_its_state_back() {
    let mut mailbox = mailbox();
    let (rota, photos) = (id_of(&mailbox, ROTA), id_of(&mailbox, PHOTOS));
    let unread = inbox_unread(&mailbox);
    mailbox.act(rota, Act::Read);
    assert_eq!(mailbox.last_run(), Some("mail.list"));
    assert_eq!(listed(&mailbox, rota), Some(true));
    assert_eq!(inbox_unread(&mailbox), unread - 1);
    assert_eq!(toast(&mailbox), Some((Act::Read, rota, 1, "Marked as read", true)));
    assert!(mailbox.can_undo());
    mailbox.undo();
    assert_eq!(listed(&mailbox, rota), Some(false));
    assert_eq!(inbox_unread(&mailbox), unread);
    assert_eq!(mailbox.undoable(), None);
    assert!(!mailbox.can_undo(), "the history is empty again");
    mailbox.act(photos, Act::Unread);
    assert_eq!(listed(&mailbox, photos), Some(false));
    assert_eq!(toast(&mailbox), Some((Act::Unread, photos, 2, "Marked as unread", true)));
    mailbox.undo();
    assert_eq!(listed(&mailbox, photos), Some(true));
    assert_eq!(mailbox.error(), None);
}

#[test]
fn flag_and_unflag_change_the_row_and_undo_takes_the_flag_back() {
    let mut mailbox = mailbox();
    let rota = id_of(&mailbox, ROTA);
    assert_eq!(flagged(&mailbox, rota), Some(false));
    mailbox.act(rota, Act::Flag);
    assert_eq!(flagged(&mailbox, rota), Some(true));
    assert_eq!(toast(&mailbox).map(|toast| toast.3), Some("Flagged"));
    mailbox.act(rota, Act::Unflag);
    assert_eq!(flagged(&mailbox, rota), Some(false));
    assert_eq!(toast(&mailbox).map(|toast| toast.3), Some("Flag taken off"));
    mailbox.undo();
    assert_eq!(flagged(&mailbox, rota), Some(true), "the newest act is taken back first");
    mailbox.undo();
    assert_eq!(flagged(&mailbox, rota), Some(false), "then the one before it, through the engine's history");
    assert!(!mailbox.can_undo());
}

#[test]
fn an_act_that_changes_nothing_shows_its_toast_without_undo() {
    let mut mailbox = mailbox();
    let photos = id_of(&mailbox, PHOTOS);
    mailbox.act(photos, Act::Read);
    assert_eq!(toast(&mailbox), Some((Act::Read, photos, 1, "Marked as read", false)), "it was read already");
    assert!(!mailbox.can_undo());
    let runs = mailbox.runs();
    mailbox.undo();
    assert_eq!(mailbox.runs(), runs, "nothing to take back, no command");
    assert_eq!(mailbox.undoable(), None);
}

#[test]
fn done_reads_and_archives_and_undo_brings_the_row_back_selected_and_unread() {
    let mut mailbox = mailbox();
    let rota = id_of(&mailbox, ROTA);
    let below = mailbox.messages()[1].id;
    mailbox.select(Some(rota));
    mailbox.act(rota, Act::Done);
    assert_eq!(listed(&mailbox, rota), None);
    assert_eq!(mailbox.messages().len(), 10);
    assert_eq!(mailbox.selected(), Some(below), "the next-message rule picks the row below");
    assert_eq!(toast(&mailbox).map(|toast| toast.3), Some("Marked as done"));
    mailbox.open(Place::View(Role::Archive));
    assert_eq!(listed(&mailbox, rota), Some(true), "in the Archive, read");
    mailbox.open(Place::View(Role::Inbox));
    mailbox.undo();
    assert_eq!(listed(&mailbox, rota), Some(false), "both steps of Done are taken back");
    assert_eq!(mailbox.messages().len(), 11);
    assert_eq!(mailbox.selected(), Some(rota));
    assert!(!mailbox.can_undo());
    assert_eq!(mailbox.error(), None);
}

#[test]
fn archive_moves_the_row_to_the_archive_as_it_is() {
    let mut mailbox = mailbox();
    let rota = id_of(&mailbox, ROTA);
    mailbox.act(rota, Act::Archive);
    assert_eq!(listed(&mailbox, rota), None);
    assert_eq!(toast(&mailbox).map(|toast| toast.3), Some("Moved to Archive"));
    mailbox.open(Place::View(Role::Archive));
    assert_eq!(listed(&mailbox, rota), Some(false), "in the Archive, still unread");
    mailbox.undo();
    mailbox.open(Place::View(Role::Inbox));
    assert_eq!(listed(&mailbox, rota), Some(false));
}

#[test]
fn delete_moves_the_row_to_the_trash_and_undo_brings_it_back() {
    let mut mailbox = mailbox();
    let logo = id_of(&mailbox, LOGO);
    mailbox.act(logo, Act::Delete);
    assert_eq!(listed(&mailbox, logo), None);
    assert_eq!(toast(&mailbox).map(|toast| (toast.3, toast.4)), Some(("Moved to Trash", true)));
    mailbox.narrow(Some(2));
    mailbox.open(Place::View(Role::Trash));
    assert_eq!(listed(&mailbox, logo), Some(false), "in the Studio Trash, still unread");
    mailbox.undo();
    assert_eq!(listed(&mailbox, logo), None, "out of the Trash");
    mailbox.open(Place::View(Role::Inbox));
    assert_eq!(listed(&mailbox, logo), Some(false));
    assert_eq!(mailbox.selected(), None, "a row that was not selected is not selected after");
}

#[test]
fn delete_in_the_trash_deletes_for_good_and_cannot_be_undone() {
    let mut mailbox = mailbox();
    let logo = id_of(&mailbox, LOGO);
    mailbox.act(logo, Act::Delete);
    mailbox.forget_undo();
    mailbox.narrow(Some(2));
    mailbox.open(Place::View(Role::Trash));
    mailbox.act(logo, Act::Delete);
    assert_eq!(listed(&mailbox, logo), None);
    assert_eq!(toast(&mailbox).map(|toast| (toast.3, toast.4)), Some(("Deleted for good", false)));
    assert!(mailbox.can_undo(), "the move to the Trash before it still can be");
    mailbox.undo();
    assert_eq!(mailbox.undoable(), None);
    assert_eq!(listed(&mailbox, logo), None, "gone for good");
    assert_eq!(mailbox.error(), None, "{:?}", mailbox.error());
}

#[test]
fn move_takes_the_row_to_a_folder_of_its_account_and_undo_brings_it_back() {
    let mut mailbox = mailbox();
    let rota = id_of(&mailbox, ROTA);
    let receipts = folder_named(&mailbox, 1, "Receipts");
    assert_eq!(
        mailbox.move_targets(rota).iter().map(|folder| folder.leaf.as_str()).collect::<Vec<_>>(),
        ["Drafts", "Sent", "Archive", "Junk", "Trash", "Receipts", "Travel", "2026"]
    );
    assert!(mailbox.move_targets(999).is_empty());
    mailbox.act(rota, Act::Move(receipts));
    assert_eq!(listed(&mailbox, rota), None);
    assert_eq!(toast(&mailbox).map(|toast| toast.3), Some("Moved to Receipts"));
    mailbox.open(Place::Folder(receipts));
    assert_eq!(listed(&mailbox, rota), Some(false));
    mailbox.undo();
    assert_eq!(listed(&mailbox, rota), None);
    mailbox.open(Place::View(Role::Inbox));
    assert_eq!(listed(&mailbox, rota), Some(false));
}

#[test]
fn a_move_to_another_account_s_folder_is_refused_and_shown() {
    let mut mailbox = mailbox();
    let rota = id_of(&mailbox, ROTA);
    let studio_trash = folder_named(&mailbox, 2, "Trash");
    mailbox.act(rota, Act::Move(studio_trash));
    assert!(mailbox.error().is_some_and(|error| error.contains("another account")), "{:?}", mailbox.error());
    assert_eq!(listed(&mailbox, rota), Some(false));
    assert_eq!(mailbox.undoable(), None);
}

#[test]
fn acts_that_take_the_row_out_slide_first_and_a_new_act_finishes_the_sliding_one() {
    assert!([Act::Done, Act::Archive, Act::Delete, Act::Move(7)].iter().all(|act| act.slides()));
    assert!(![Act::Read, Act::Unread, Act::Flag, Act::Unflag].iter().any(|act| act.slides()));
    let mut mailbox = mailbox();
    let (rota, photos) = (id_of(&mailbox, ROTA), id_of(&mailbox, PHOTOS));
    mailbox.start(rota, Act::Done);
    assert_eq!(mailbox.leaving(), Some((rota, Act::Done)));
    assert_eq!(listed(&mailbox, rota), Some(false), "still in the list while it slides");
    assert_eq!(mailbox.undoable(), None);
    mailbox.start(photos, Act::Delete);
    assert_eq!(listed(&mailbox, rota), None, "done before the next one starts");
    assert_eq!(mailbox.leaving(), Some((photos, Act::Delete)));
    mailbox.left();
    assert_eq!(listed(&mailbox, photos), None);
    assert_eq!(mailbox.leaving(), None);
    mailbox.left();
    let kasimir = id_of(&mailbox, "Studio open day: volunteers?");
    mailbox.start(kasimir, Act::Read);
    assert_eq!(mailbox.leaving(), None, "read is done at once");
    assert_eq!(listed(&mailbox, kasimir), Some(true));
    mailbox.forget_undo();
    assert_eq!(mailbox.undoable(), None);
    assert!(mailbox.can_undo(), "the toast has gone, the history stays");
    mailbox.undo();
    assert_eq!(listed(&mailbox, kasimir), Some(false), "Ctrl+Z still walks back, one act at a time");
    mailbox.undo();
    assert_eq!(listed(&mailbox, photos), Some(true), "the delete before it");
    mailbox.undo();
    assert_eq!(listed(&mailbox, rota), Some(true), "Done is two acts in the history: first the archive");
    assert!(mailbox.can_undo());
    mailbox.undo();
    assert_eq!(listed(&mailbox, rota), Some(false), "then the read");
    assert!(!mailbox.can_undo());
    assert_eq!(mailbox.messages().len(), 11);
}

#[test]
fn a_mailbox_over_a_real_store_acts_and_queues_the_server_s_job() {
    use inboxcraft_model::{AccountId, Event, EventKind, FolderId, FolderState, JobKind, UidValidity};
    let session = Session::new(Box::new(sample::mailbox(at(NOW)).unwrap()), clock()).unwrap();
    let mut mailbox = Mailbox::new(session, clock());
    let synced = EventKind::FolderSynced { folder: FolderId::new(1), state: FolderState::new(UidValidity::new(1).unwrap()) };
    mailbox.session.apply(&Event::new(AccountId::new(1), None, synced)).unwrap();
    let rota = id_of(&mailbox, ROTA);
    mailbox.start(rota, Act::Flag);
    assert_eq!(flagged(&mailbox, rota), Some(true));
    mailbox.start(rota, Act::Archive);
    mailbox.left();
    assert_eq!(listed(&mailbox, rota), None);
    let jobs: Vec<JobKind> = mailbox.session.take_jobs().into_iter().map(|job| job.kind).collect();
    assert!(matches!(jobs.as_slice(), [JobKind::StoreFlags { .. }, JobKind::Move { .. }]), "{jobs:?}");
    mailbox.undo();
    assert_eq!(listed(&mailbox, rota), Some(false), "back while its move is on its way");
    assert_eq!(mailbox.error(), None);
}

#[test]
fn opening_marks_read_through_the_history_and_an_undone_message_opens_as_it_was() {
    let mut mailbox = mailbox();
    let rota = id_of(&mailbox, ROTA);
    mailbox.select(Some(rota));
    let answer = mailbox.open_message(rota).unwrap();
    assert_eq!(answer.get("marked_seen"), Some(&json!(true)));
    assert_eq!(listed(&mailbox, rota), Some(true), "opening marks read");
    assert!(mailbox.can_undo());
    mailbox.act(rota, Act::Unread);
    mailbox.act(rota, Act::Archive);
    mailbox.undo();
    assert_eq!(mailbox.selected(), Some(rota));
    assert_eq!(listed(&mailbox, rota), Some(false));
    mailbox.open_message(rota).unwrap();
    assert_eq!(mailbox.last_run(), Some("mail.read"), "the message back from an undo is shown, not marked");
    assert_eq!(listed(&mailbox, rota), Some(false));
    mailbox.open_message(rota).unwrap();
    assert_eq!(listed(&mailbox, rota), Some(true), "opened again later, it is marked read");
    assert!(mailbox.open_message(999).is_err());
}

#[test]
fn a_history_shorter_than_counted_stops_the_undo_without_an_error() {
    let mut mailbox = mailbox();
    mailbox.history = 3;
    assert!(mailbox.can_undo());
    mailbox.undo();
    assert!(!mailbox.can_undo(), "the engine had nothing: the count starts again");
    assert_eq!(mailbox.error(), None);
}

#[test]
fn an_act_on_a_row_not_listed_shows_an_error_and_changes_nothing() {
    let mut mailbox = mailbox();
    mailbox.act(999, Act::Done);
    assert!(mailbox.error().is_some_and(|error| error.contains("there is no message 999")), "{:?}", mailbox.error());
    mailbox.act(999, Act::Flag);
    assert!(mailbox.error().is_some_and(|error| error.contains("there is no message 999")), "{:?}", mailbox.error());
    assert_eq!(mailbox.undoable(), None);
    assert!(!mailbox.can_undo());
    assert_eq!(mailbox.messages().len(), 11);
}

#[test]
fn a_failing_undo_shows_its_error() {
    let mut mailbox = Mailbox::new(session_with(&[("history.undo", failing)]), clock());
    mailbox.history = 1;
    mailbox.undo();
    assert!(mailbox.error().is_some());
}

#[test]
fn the_sample_has_nothing_to_tell_the_owner() {
    let mut mailbox = mailbox();
    assert_eq!(mailbox.take_notices(), vec![], "no mail arrives in the sample, so nothing notifies");
}
