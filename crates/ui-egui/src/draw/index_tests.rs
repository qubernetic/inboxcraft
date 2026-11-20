// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The index on its own: its frames' order and places, what does not fit, the keyboard, hostile names.

use std::collections::BTreeSet;

use egui::accesskit::Role;
use egui::{Rect, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_model::{Account, AccountId, EmailAddress, Folder, FolderId, FolderListing, FolderPath, Host, Security, Server};
use inboxcraft_store::{Batch, Change, Storage as _};
use inboxcraft_time::{FakeClock, Timestamp};

use super::{Asked, MOST_NAME, Place, clean, open_id, show};
use crate::component::tests::{keyboard_walk, rests};
use crate::keymap::Keymap;
use crate::mailbox::Mailbox;
use crate::sample;
use crate::theme::{Mode, TextScale, Theme, install_fonts};

fn now() -> Timestamp {
    Timestamp::from_unix(1_793_869_200, 0).unwrap()
}

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(now()))
}

/// The sample's two accounts.
fn two() -> Mailbox {
    Mailbox::new(sample::session(clock()).unwrap(), clock())
}

/// A 60-character folder name with a bidi override in it (the store refuses control characters in a path).
const LONG: &str = "Clients / Bakery on the corner / Shop\u{202e} front lettering 26";

/// The sample with four accounts more (six), the last one's folders 40, one of them named [`LONG`].
fn six() -> Mailbox {
    let mut store = sample::mailbox(now()).unwrap();
    let mut batch = Batch::new();
    for id in 3..=6_u64 {
        let address = format!("box{id}@example.org");
        let imap = Server::new(Host::new("imap.example.org").unwrap(), Security::Tls.imap_port(), Security::Tls);
        let label = if id == 4 { "Studio Collective, shared orders and deliveries\u{202e}" } else { "Garden" };
        let account = Account::new(AccountId::new(id), label, EmailAddress::new(&address).unwrap(), &address, imap).unwrap();
        batch.push(Change::PutAccount(account));
    }
    for n in 0..40_u64 {
        let name = if n == 3 { LONG.to_owned() } else { format!("Folder {n}") };
        let path = FolderPath::new(&name, Some('|')).unwrap();
        batch.push(Change::PutFolder(Folder::new(FolderId::new(1000 + n), AccountId::new(6), FolderListing::new(path, vec![], true))));
    }
    store.apply(&batch).unwrap();
    Mailbox::new(sample::session_over(store, clock()).unwrap(), clock())
}

/// What a test harness keeps: the mailbox, the place, narrow or not, and the last frame's ask.
struct State {
    mailbox: Mailbox,
    at: Place,
    narrow: bool,
    asked: Asked,
    keymap: Keymap,
    theme: Theme,
}

/// The index alone in a `width` × `height` column, motion reduced, every account open when `open`.
fn harness(mailbox: Mailbox, at: Place, narrow: bool, scale: TextScale, height: f32, open: bool) -> Harness<'static, State> {
    harness_in(mailbox, at, narrow, scale, height, open, 0.0)
}

/// [`harness`] with `beside` points of window on the index's right, where its floats may reach.
fn harness_in(mailbox: Mailbox, at: Place, narrow: bool, scale: TextScale, height: f32, open: bool, beside: f32) -> Harness<'static, State> {
    let theme = Theme::new(Mode::Dark, scale);
    let width = if narrow { 56.0 } else { 232.0 };
    let ids: BTreeSet<u64> = mailbox.accounts().iter().map(|account| account.id).collect();
    let state = State { mailbox, at, narrow, asked: Asked::default(), keymap: Keymap::default(), theme };
    let mut harness = Harness::builder()
        .with_size(vec2(width + beside, height))
        .with_context_setup(move |ctx| {
            install_fonts(ctx);
            crate::motion::set_reduced(ctx, true);
            if open {
                ctx.data_mut(|data| data.insert_temp(open_id(), ids.clone()));
            }
        })
        .build_ui_state(
            move |ui, state: &mut State| {
                let window = ui.ctx().content_rect();
                let whole = Rect::from_min_size(window.min, vec2(width, window.height()));
                let asked = ui
                    .scope_builder(egui::UiBuilder::new().max_rect(whole), |ui| {
                        show(ui, &state.theme, &state.keymap, &state.mailbox, state.at, state.narrow, (true, false))
                    })
                    .inner;
                if let Some(id) = asked.toggle {
                    super::toggle(ui.ctx(), id);
                }
                if asked != Asked::default() {
                    state.asked = asked;
                }
            },
            state,
        );
    harness.run();
    harness
}

/// The rect of the button named `name`.
fn at(harness: &Harness<'_, State>, name: &str) -> Rect {
    harness.get_by_role_and_label(Role::Button, name).rect()
}

#[test]
fn the_index_stands_as_its_frame_draws_with_mail_open() {
    let harness = harness(two(), Place::Mail, false, TextScale::default(), 826.0, false);
    let home = at(&harness, "Home");
    assert_eq!((home.top(), home.size()), (52.0, vec2(232.0, 34.0)), "HOME under New message's 40 high row, 12 down");
    assert_eq!(at(&harness, "Mail").top(), 86.0);
    let inbox = harness.query_all_by_role(Role::Button).find(|node| node.accesskit_node().label().is_some_and(|l| l.starts_with("Inbox"))).unwrap();
    assert_eq!((inbox.rect().top(), inbox.rect().height()), (120.0, 30.0), "the mailboxes under MAIL");
    assert!(inbox.accesskit_node().toggled() == Some(egui::accesskit::Toggled::True), "the inbox is the mailbox shown");
    assert!(
        harness.query_by_label("New senders").is_some()
            || harness.query_all_by_role(Role::Button).any(|n| n.accesskit_node().label().is_some_and(|l| l.starts_with("New senders")))
    );
    let calendar = at(&harness, "Calendar");
    let later = at(&harness, "Later");
    let settings = at(&harness, "Settings");
    assert_eq!((later.top() - calendar.top(), settings.top() - later.top()), (34.0, 34.0), "the places after the mailboxes");
    let owner = harness.query_all_by_role(Role::Button).find(|n| n.accesskit_node().label().is_some_and(|l| l.starts_with("Show mail of"))).unwrap();
    assert_eq!((owner.rect().top(), owner.rect().height()), (786.0, 40.0), "the owner's row at the foot");
    assert!(!harness.get_by_role_and_label(Role::Button, "Home").accesskit_node().is_disabled(), "HOME is a place");
    assert_eq!(harness.query_all_by_label("coming soon").count(), 0, "a row inside soon carries no tag of its own");
}

/// The labels of the index's buttons from top to bottom, each cut at its first comma (a count or "open" after it).
fn rows_down(harness: &Harness<'_, State>) -> Vec<String> {
    let mut rows: Vec<(f32, String)> = harness
        .query_all_by_role(Role::Button)
        .map(|node| (node.rect().top(), node.accesskit_node().label().unwrap_or_default().split(',').next().unwrap_or_default().to_owned()))
        .collect();
    rows.sort_by(|a, b| a.0.total_cmp(&b.0));
    rows.into_iter().map(|(_, label)| label).collect()
}

#[test]
fn the_shared_mailboxes_are_the_frames_and_no_hidden_folder_of_our_own() {
    // The sample's Personal holds `InboxCraft/Screener`, whose mail New senders shows: no row of its own (#325, 3.1).
    let harness = harness(two(), Place::Mail, false, TextScale::default(), 826.0, false);
    let rows = rows_down(&harness);
    let from = rows.iter().position(|label| label == "Mail").unwrap() + 1;
    let to = rows.iter().position(|label| label == "Personal").unwrap();
    assert_eq!(rows[from..to], ["Inbox", "New senders", "Drafts", "Sent", "Archive", "Junk", "Bin"], "{rows:?}");
    assert!(!rows.iter().any(|label| label.starts_with("Screener")), "{rows:?}");
}

/// The sample with `InboxCraft/Snoozed` and `InboxCraft/Blocked` in Personal, beside its `InboxCraft/Screener`.
fn with_own_folders() -> Mailbox {
    let mut store = sample::mailbox(now()).unwrap();
    let mut batch = Batch::new();
    for (id, name) in [(901_u64, "InboxCraft/Snoozed"), (902, "InboxCraft/Blocked")] {
        let path = FolderPath::new(name, Some('/')).unwrap();
        batch.push(Change::PutFolder(Folder::new(FolderId::new(id), AccountId::new(1), FolderListing::new(path, vec![], true))));
    }
    store.apply(&batch).unwrap();
    Mailbox::new(sample::session_over(store, clock()).unwrap(), clock())
}

#[test]
fn blocked_keeps_its_row_and_opens_where_screener_and_snoozed_have_places_of_their_own() {
    // Screener's mail is in New senders and Snoozed's in Later; Blocked has no other place, so its row stays (#325, I-1).
    let mut harness = harness(with_own_folders(), Place::Mail, false, TextScale::default(), 826.0, false);
    let rows = rows_down(&harness);
    assert!(rows.iter().any(|label| label.starts_with("Blocked")), "{rows:?}");
    assert!(!rows.iter().any(|label| label.starts_with("Screener") || label.starts_with("Snoozed")), "{rows:?}");
    let blocked = harness.query_all_by_role(Role::Button).find(|n| n.accesskit_node().label().is_some_and(|l| l.starts_with("Blocked"))).unwrap();
    blocked.click();
    harness.run();
    assert_eq!(harness.state().asked.mailbox, Some(crate::mailbox::Place::View(inboxcraft_model::Role::Blocked)));
}

#[test]
fn new_senders_counts_the_senders_held_back() {
    // screen/index/with-counts: New senders carries a count, as Home's TO SCREEN (#325, 3.8).
    let mailbox = two();
    let waiting = mailbox.waiting().len();
    assert!(waiting > 0, "the sample holds senders back");
    let harness = harness(mailbox, Place::Mail, false, TextScale::default(), 826.0, false);
    harness.get_by_role_and_label(Role::Button, &format!("New senders, {waiting} waiting"));
}

#[test]
fn the_place_shown_is_the_selected_row() {
    for place in [Place::Calendar, Place::Later, Place::Settings] {
        let harness = harness(two(), place, false, TextScale::default(), 826.0, false);
        for (name, which) in [("Calendar", Place::Calendar), ("Later", Place::Later), ("Settings", Place::Settings)] {
            let selected = harness.get_by_role_and_label(Role::Button, name).accesskit_node().toggled() == Some(egui::accesskit::Toggled::True);
            assert_eq!(selected, which == place, "{place:?}: {name}");
        }
        let inbox = harness.query_all_by_role(Role::Button).find(|n| n.accesskit_node().label().is_some_and(|l| l.starts_with("Inbox"))).unwrap();
        assert!(inbox.accesskit_node().toggled() != Some(egui::accesskit::Toggled::True), "{place:?}: no mailbox is shown");
    }
}

#[test]
fn an_index_that_does_not_fit_scrolls_only_its_mailboxes() {
    for narrow in [false, true] {
        for scale in [TextScale::default(), TextScale::ALL[4]] {
            let mut harness = harness(six(), Place::Mail, narrow, scale, 520.0 - 72.0, true);
            let what = format!("narrow {narrow}, {scale:?}");
            let places = |harness: &Harness<'_, State>| ["Home", "Mail", "Calendar", "Later", "Settings"].map(|name| at(harness, name));
            let owner = |harness: &Harness<'_, State>| {
                harness
                    .query_all_by_role(Role::Button)
                    .find(|n| n.accesskit_node().label().is_some_and(|l| l.starts_with("Show mail of")))
                    .unwrap()
                    .rect()
            };
            let before = places(&harness);
            let foot = owner(&harness);
            assert_eq!(foot.bottom(), 448.0, "{what}: the owner's row at the foot");
            for pair in before.windows(2) {
                assert!(pair[0].bottom() <= pair[1].top(), "{what}: the places do not overlap: {pair:?}");
            }
            assert!(before[4].bottom() <= foot.top() - 1.0, "{what}: SETTINGS stays over the owner's row");
            // The mailboxes' rows that show lie between MAIL and CALENDAR.
            let band = Rect::from_min_max(egui::pos2(before[1].left(), before[1].bottom()), egui::pos2(before[1].right(), before[2].top()));
            assert!(band.height() > 0.0, "{what}: the mailboxes keep some room");
            harness.hover_at(band.center());
            harness.event(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: vec2(0.0, -600.0),
                modifiers: egui::Modifiers::NONE,
                phase: egui::TouchPhase::Move,
                source: egui::MouseWheelSource::Unknown,
            });
            harness.run();
            assert_eq!(places(&harness), before, "{what}: the places stay where they were");
            assert_eq!(owner(&harness), foot, "{what}: the owner's row stays");
            let rows: Vec<Rect> = harness
                .query_all_by_role(Role::Button)
                .filter(|n| {
                    let label = n.accesskit_node().label().unwrap_or_default();
                    label.starts_with("Folder") || label.starts_with("Garden") || label.starts_with("Clients")
                })
                .map(|n| n.rect())
                .filter(|r| r.intersects(band))
                .collect();
            assert!(!rows.is_empty(), "{what}: folders scrolled into sight");
            for row in rows {
                let seen = row.intersect(band);
                assert!(seen.height() <= row.height(), "{what}");
                assert!(!before.iter().any(|place| place.intersects(seen.shrink(0.5))), "{what}: a mailbox row over a place: {row:?}");
            }
        }
    }
}

#[test]
fn a_hostile_name_is_cleaned_capped_and_cut_inside_its_row() {
    assert_eq!(clean("Shop\u{202e} front\nlettering\u{0}"), "Shop front lettering ");
    let long: String = "x".repeat(10_000);
    assert_eq!(clean(&long).chars().count(), MOST_NAME);
    let mut harness = harness(six(), Place::Mail, false, TextScale::default(), 3000.0, true);
    harness.run();
    let shown =
        harness.query_all_by_role(Role::Button).find(|n| n.accesskit_node().label().is_some_and(|l| l.starts_with("Clients / Bakery"))).unwrap();
    let label = shown.accesskit_node().label().unwrap_or_default();
    assert!(!label.contains('\u{202e}'), "{label:?}");
    assert!(shown.rect().right() <= 232.0, "{:?}", shown.rect());
}

#[test]
fn tab_walks_places_mailboxes_accounts_and_the_owner_s_row_in_that_order() {
    let mut harness = harness(two(), Place::Mail, false, TextScale::default(), 826.0, false);
    let mut order = Vec::new();
    for _ in 0..40 {
        harness.key_press(egui::Key::Tab);
        harness.run();
        let Some(node) = harness.query_all_by_role(Role::Button).find(|n| n.accesskit_node().is_focused()) else { continue };
        let label = node.accesskit_node().label().unwrap_or_default();
        if order.last() != Some(&label) && !order.contains(&label) {
            order.push(label);
        }
    }
    let first = |prefix: &str| order.iter().position(|label| label.starts_with(prefix)).unwrap_or_else(|| panic!("{prefix}: {order:?}"));
    assert!(first("Mail") < first("Inbox"), "{order:?}");
    assert!(first("Inbox") < first("Personal") || first("Inbox") < first("Studio"), "{order:?}");
    assert!(first("Calendar") > first("Inbox"), "the places after the mailboxes come after them: {order:?}");
    assert!(first("Settings") < first("Show mail of"), "the owner's row last: {order:?}");
    assert!(first("Home") < first("Mail"), "HOME is the first stop: {order:?}");
}

#[test]
fn every_row_and_the_account_picker_are_reached_by_the_keyboard() {
    let mut harness = harness(two(), Place::Mail, false, TextScale::default(), 826.0, true);
    let missed = keyboard_walk(&mut harness);
    assert!(missed.is_empty(), "{missed:?}");
    let owner = harness.query_all_by_role(Role::Button).find(|n| n.accesskit_node().label().is_some_and(|l| l.starts_with("Show mail of"))).unwrap();
    owner.focus();
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
    let all = harness.get_by_label("All accounts");
    all.click();
    harness.run();
    assert_eq!(harness.state().asked.account, Some(None), "the picker's All accounts");
}

#[test]
fn the_account_picker_stands_on_the_owner_s_row_as_its_frame_draws_it() {
    // popup/account-picker/open: 230 wide, 3 in from the index's left, its foot on the owner's row (over the hairline);
    // a SHOW MAIL OF head 28 high, then All accounts and each account with its bar and address, 32 high (#325, 3.6).
    let mut harness = harness_in(two(), Place::Mail, false, TextScale::default(), 826.0, false, 400.0);
    harness.query_all_by_role(Role::Button).find(|n| n.accesskit_node().label().is_some_and(|l| l.starts_with("Show mail of"))).unwrap().click();
    harness.run();
    harness.run();
    let head = harness.get_by_label("Show mail of").rect();
    let all = harness.get_by_role_and_label(Role::MenuItemRadio, "All accounts").rect();
    let personal = harness.get_by_role_and_label(Role::MenuItemRadio, "Personal, mira@example.org").rect();
    let studio = harness.get_by_role_and_label(Role::MenuItemRadio, "Studio, holloway.studio@example.org").rect();
    assert_eq!(head.height(), 28.0, "{head:?}");
    assert_eq!([all.top() - head.bottom(), personal.top() - all.bottom(), studio.top() - personal.bottom()], [0.0; 3]);
    assert_eq!([all.height(), personal.height(), studio.height()], [32.0; 3]);
    // The float's own 1 px edge lies around its rows.
    assert_eq!((head.left() - 1.0, head.right() + 1.0), (3.0, 233.0), "{head:?}");
    assert_eq!(studio.bottom() + 1.0, 786.0, "the picker's foot on the owner's row: {studio:?}");
}

#[test]
fn all_accounts_leaves_the_account_bar_s_room_in_the_picker() {
    // popup/account-picker/open: every name of the picker starts 47 into its row, All accounts' too ("no bar" keeps
    // the bar's room).
    let mut harness = harness_in(two(), Place::Mail, false, TextScale::default(), 826.0, false, 400.0);
    harness.query_all_by_role(Role::Button).find(|n| n.accesskit_node().label().is_some_and(|l| l.starts_with("Show mail of"))).unwrap().click();
    harness.run();
    harness.run();
    let all = harness.get_by_role_and_label(Role::MenuItemRadio, "All accounts").rect();
    let personal = harness.get_by_role_and_label(Role::MenuItemRadio, "Personal, mira@example.org").rect();
    fn texts(shape: &egui::Shape, found: &mut Vec<(String, egui::Pos2)>) {
        match shape {
            egui::Shape::Text(text) => found.push((text.galley.job.text.clone(), text.pos)),
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| texts(shape, found)),
            _ => {}
        }
    }
    let mut found = Vec::new();
    harness.output().shapes.iter().for_each(|clipped| texts(&clipped.shape, &mut found));
    let at = |words: &str, row: egui::Rect| {
        found.iter().find(|(text, pos)| text == words && row.contains(*pos + egui::vec2(0.0, 1.0))).map(|(_, pos)| pos.x - row.left())
    };
    assert_eq!(at("Personal", personal), Some(47.0), "{found:?}");
    assert_eq!(at("All accounts", all), Some(47.0), "{found:?}");
}

#[test]
fn with_one_account_the_picker_ticks_it_and_says_where_another_is_added() {
    // popup/account-picker/one-account: no All accounts; the account ticked, a hairline and the note under it.
    let mut store = sample::mailbox(now()).unwrap();
    store.apply(&Batch::new().with(Change::RemoveAccount(AccountId::new(2)))).unwrap();
    let one = Mailbox::new(sample::session_over(store, clock()).unwrap(), clock());
    let mut harness = harness_in(one, Place::Mail, false, TextScale::default(), 826.0, false, 400.0);
    harness.query_all_by_role(Role::Button).find(|n| n.accesskit_node().label().is_some_and(|l| l.starts_with("Show mail of"))).unwrap().click();
    harness.run();
    harness.run();
    assert!(harness.query_by_role_and_label(Role::MenuItemRadio, "All accounts").is_none(), "nothing to narrow");
    let personal = harness.get_by_role_and_label(Role::MenuItemRadio, "Personal, mira@example.org");
    assert_eq!(personal.accesskit_node().toggled(), Some(egui::accesskit::Toggled::True), "the only account is the one shown");
    assert!(harness.query_by_label("The only account. Another is added in Settings.").is_some());
}

#[test]
fn the_index_rests() {
    let mut harness = harness(two(), Place::Mail, false, TextScale::default(), 826.0, true);
    assert!(rests(&mut harness, 8));
}

#[test]
fn a_press_asks_for_a_place_or_a_mailbox() {
    let mut harness = harness(two(), Place::Mail, false, TextScale::default(), 826.0, false);
    harness.get_by_role_and_label(Role::Button, "Calendar").click();
    harness.run();
    assert_eq!(harness.state().asked.place, Some(Place::Calendar));
    let sent = harness.query_all_by_role(Role::Button).find(|n| n.accesskit_node().label().is_some_and(|l| l.starts_with("Sent"))).unwrap();
    sent.click();
    harness.run();
    assert_eq!(harness.state().asked.mailbox, Some(crate::mailbox::Place::View(inboxcraft_model::Role::Sent)));
}

/// The sample with no account.
fn none() -> Mailbox {
    let session =
        inboxcraft_engine::Session::new(Box::new(inboxcraft_store::Memory::new(inboxcraft_store::StoreConfig::default())), clock()).unwrap();
    Mailbox::new(session, clock())
}

#[test]
fn new_message_stands_first_on_every_place_and_the_places_move_down_40() {
    // screen/index/*: New message 12 down, 12 in, 208 × 32; HOME 40 under its row's top, at 52 (#339).
    for place in [Place::Home, Place::Mail, Place::Calendar, Place::Later, Place::Settings] {
        let harness = harness(two(), place, false, TextScale::default(), 826.0, false);
        let new = at(&harness, "New message");
        assert_eq!(new, Rect::from_min_size(egui::pos2(12.0, 12.0), vec2(208.0, 32.0)), "{place:?}");
        assert_eq!(at(&harness, "Home").top(), 52.0, "{place:?}: HOME under New message's row");
        assert!(!harness.get_by_role_and_label(Role::Button, "New message").accesskit_node().is_disabled(), "{place:?}");
    }
}

#[test]
fn new_message_draws_its_key_at_the_right_edge() {
    // text-button/with-icon-key: the glyph at 12, the words at 36, the key's chip ending 12 in from the button's right.
    let harness = harness(two(), Place::Mail, false, TextScale::default(), 826.0, false);
    let mut found = Vec::new();
    fn texts(shape: &egui::Shape, found: &mut Vec<(String, egui::Rect)>) {
        match shape {
            egui::Shape::Text(text) => found.push((text.galley.job.text.clone(), egui::Rect::from_min_size(text.pos, text.galley.size()))),
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| texts(shape, found)),
            _ => {}
        }
    }
    harness.output().shapes.iter().for_each(|clipped| texts(&clipped.shape, &mut found));
    let words = found.iter().find(|(text, _)| text == "New message").map(|(_, rect)| rect.left()).unwrap();
    assert_eq!(words, 12.0 + 36.0, "the words 36 into the button");
    // The chip 179 into the button, 17 wide: its letter 5 into it.
    let key = found.iter().find(|(text, rect)| text == "C" && rect.top() < 44.0).map(|(_, rect)| rect.left()).unwrap();
    assert_eq!(key, 12.0 + 179.0 + 5.0, "the key's chip ending 12 in from the button's right edge");
}

#[test]
fn a_press_on_new_message_asks_to_write() {
    let mut harness = harness(two(), Place::Calendar, false, TextScale::default(), 826.0, false);
    harness.get_by_role_and_label(Role::Button, "New message").click();
    harness.run();
    assert!(harness.state().asked.compose, "New message asks for a new message");
    assert_eq!(harness.state().asked.place, None, "and for nothing else");
}

#[test]
fn without_an_account_new_message_cannot_be_pressed_and_says_why() {
    for narrow in [false, true] {
        let mut harness = harness(none(), Place::Home, narrow, TextScale::default(), 826.0, false);
        crate::component::tests::quick_tooltips(&harness.ctx);
        let new = harness.get_by_role_and_label(Role::Button, "New message");
        assert!(new.accesskit_node().is_disabled(), "narrow {narrow}");
        new.hover();
        harness.run();
        harness.run();
        assert!(harness.query_by_label("no account yet").is_some(), "narrow {narrow}: its tooltip says why");
        harness.get_by_role_and_label(Role::Button, "New message").click();
        harness.run();
        assert!(!harness.state().asked.compose, "narrow {narrow}: nothing asked");
    }
}

#[test]
fn narrow_new_message_is_its_glyph_in_a_32_square_named_in_its_tooltip() {
    // screen/index/narrowed: the glyph alone in a 32 square 12 in and 12 down; HOME at 52.
    let mut harness = harness(two(), Place::Mail, true, TextScale::default(), 826.0, false);
    crate::component::tests::quick_tooltips(&harness.ctx);
    harness.run();
    assert_eq!(at(&harness, "New message"), Rect::from_min_size(egui::pos2(12.0, 12.0), vec2(32.0, 32.0)));
    assert_eq!(at(&harness, "Home").top(), 52.0);
    assert!(harness.query_all_by_label("New message").count() == 1, "no tooltip before the hover");
    harness.get_by_role_and_label(Role::Button, "New message").hover();
    harness.run();
    harness.run();
    assert_eq!(harness.query_all_by_label("New message").count(), 2, "the button and its tooltip's name");
    assert!(harness.query_by_label("C").is_some(), "and its key");
}

#[test]
fn new_message_is_the_index_s_first_tab_stop() {
    let mut harness = harness(two(), Place::Mail, false, TextScale::default(), 826.0, false);
    let mut order = Vec::new();
    for _ in 0..3 {
        harness.key_press(egui::Key::Tab);
        harness.run();
        if let Some(node) = harness.query_all_by_role(Role::Button).find(|n| n.accesskit_node().is_focused()) {
            order.push(node.accesskit_node().label().unwrap_or_default());
        }
    }
    assert_eq!(order.first().map(String::as_str), Some("New message"), "{order:?}");
    assert!(order.get(1).is_some_and(|label| label.starts_with("Home")), "{order:?}");
}
