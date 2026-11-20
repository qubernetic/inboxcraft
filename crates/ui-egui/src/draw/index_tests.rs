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
    let theme = Theme::new(Mode::Dark, scale);
    let width = if narrow { 56.0 } else { 232.0 };
    let ids: BTreeSet<u64> = mailbox.accounts().iter().map(|account| account.id).collect();
    let state = State { mailbox, at, narrow, asked: Asked::default(), keymap: Keymap::default(), theme };
    let mut harness = Harness::builder()
        .with_size(vec2(width, height))
        .with_context_setup(move |ctx| {
            install_fonts(ctx);
            crate::motion::set_reduced(ctx, true);
            if open {
                ctx.data_mut(|data| data.insert_temp(open_id(), ids.clone()));
            }
        })
        .build_ui_state(
            |ui, state: &mut State| {
                let whole = ui.ctx().content_rect();
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
    assert_eq!((home.top(), home.size()), (12.0, vec2(232.0, 34.0)), "HOME 12 down");
    assert_eq!(at(&harness, "Mail").top(), 46.0);
    let inbox = harness.query_all_by_role(Role::Button).find(|node| node.accesskit_node().label().is_some_and(|l| l.starts_with("Inbox"))).unwrap();
    assert_eq!((inbox.rect().top(), inbox.rect().height()), (80.0, 30.0), "the mailboxes under MAIL");
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
    assert_eq!(rows[from..to], ["Inbox", "New senders", "Drafts", "Sent", "Archive", "Junk", "Trash"], "{rows:?}");
    assert!(!rows.iter().any(|label| label.starts_with("Screener")), "{rows:?}");
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
