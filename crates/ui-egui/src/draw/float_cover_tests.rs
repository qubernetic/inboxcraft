// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A float closed by a choice is closed: after the palette or the settings came over the window and went, one press
//! opens it again and it stays open. Every kind of float the window has: a menu (the reader's More and Move), a select
//! (on a settings page), the time pickers (Snooze, Send later) and the index's account picker.

use egui::accesskit::Role;
use egui::{Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_time::{FakeClock, Timestamp};

use crate::keymap;
use crate::mailbox::Mailbox;
use crate::settings::Section;
use crate::{InboxApp, sample};

/// 2026-11-05T09:00:00Z.
const NOW: i64 = 1_793_869_200;

/// The index's account picker opener.
const OWNER: &str = "Show mail of: mira@example.org";

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(NOW, 0).unwrap()))
}

fn harness() -> Harness<'static, InboxApp> {
    let app = InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    let mut harness = Harness::builder()
        .with_size(vec2(1440.0, 900.0))
        .with_theme(egui::Theme::Dark)
        .with_max_steps(40)
        .with_step_dt(1.0 / 60.0)
        .with_context_setup(crate::theme::install_fonts)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

fn settle(harness: &mut Harness<'static, InboxApp>) {
    for _ in 0..4 {
        harness.run();
    }
}

/// A step of a test on the app.
type Step = fn(&mut Harness<'static, InboxApp>);

/// A kind of float: what makes its opener show, the one press that opens it, a choice in it, and whether it shows.
struct Kind {
    name: &'static str,
    ready: fn(&mut Harness<'static, InboxApp>),
    open: fn(&mut Harness<'static, InboxApp>),
    choose: fn(&mut Harness<'static, InboxApp>),
    shown: fn(&Harness<'static, InboxApp>) -> bool,
    /// The ways its place stops being drawn and comes back.
    away: &'static [(&'static str, Step)],
}

/// Ctrl+2 and Ctrl+1: the calendar shown in the mail's place, and the mail again.
fn calendar_and_back(harness: &mut Harness<'static, InboxApp>) {
    for key in [Key::Num2, Key::Num1] {
        harness.key_press_modifiers(Modifiers::COMMAND, key);
        settle(harness);
    }
}

/// Ctrl+2, Ctrl+, twice and Ctrl+1: another place, the settings coming and going over it, and the mail again.
fn calendar_settings_and_back(harness: &mut Harness<'static, InboxApp>) {
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Num2);
    settle(harness);
    settings_come_and_go(harness);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Num1);
    settle(harness);
}

/// Ctrl+1 from the settings, and Ctrl+, back to them.
fn mail_and_settings_again(harness: &mut Harness<'static, InboxApp>) {
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Num1);
    settle(harness);
    assert!(!harness.state().settings().is_open(), "Ctrl+1 leaves the settings");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Comma);
    settle(harness);
}

/// The covers alone: the palette, the settings.
const COVERS: &[(&str, Step)] = &[("the palette", palette_comes_and_goes), ("the settings", settings_come_and_go)];

/// A float on the mail's place: another place and back, then the covers.
const OFF_THE_MAIL: &[(&str, Step)] = &[
    ("the calendar and back", calendar_and_back),
    ("the calendar, the settings and back", calendar_settings_and_back),
    ("the palette", palette_comes_and_goes),
    ("the settings", settings_come_and_go),
];

/// A float on a settings page: the mail and back, then the covers.
const OFF_THE_SETTINGS: &[(&str, Step)] =
    &[("the mail and back", mail_and_settings_again), ("the palette", palette_comes_and_goes), ("the settings", settings_come_and_go)];

/// The calendar's week with New event open.
fn editing(harness: &mut Harness<'static, InboxApp>) {
    if harness.state().calendar().editor().is_none() {
        harness.state_mut().show_calendar();
        let (calendar, mailbox) = harness.state_mut().calendar_mut();
        calendar.set_view(mailbox, crate::calendar::View::Week);
        settle(harness);
        harness.get_by_role_and_label(Role::Button, "New event").click();
    }
    settle(harness);
}

/// The composer's From select, whatever account it shows.
fn from_field<'h>(harness: &'h Harness<'static, InboxApp>) -> egui_kittest::Node<'h> {
    harness.query_all_by_role(Role::ComboBox).find(|node| node.accesskit_node().label().is_some_and(|label| label.starts_with("From "))).unwrap()
}

/// The first message open in the reader.
fn reading(harness: &mut Harness<'static, InboxApp>) {
    let first = harness.state().mailbox().messages().first().map(|row| row.id).unwrap();
    harness.state_mut().mailbox_mut().select(Some(first));
    settle(harness);
}

fn press_button(harness: &mut Harness<'static, InboxApp>, label: &str) {
    harness.get_by_role_and_label(Role::Button, label).click();
    harness.run();
}

/// The settings on Accounts with the new account's form, its IMAP security select in sight.
fn on_the_form(harness: &mut Harness<'static, InboxApp>) {
    if !harness.state().settings().is_open() {
        harness.state_mut().onboarding_mut().0.open();
        super::tests::at_settings(harness.state_mut(), Section::Accounts);
    }
    settle(harness);
    harness.get_by_label("IMAP security").scroll_to_me();
    settle(harness);
}

fn composing(harness: &mut Harness<'static, InboxApp>) {
    if harness.state().composer().is_none() {
        let ctx = harness.ctx.clone();
        harness.state_mut().run_action(&ctx, keymap::COMPOSE);
    }
    settle(harness);
}

const KINDS: [Kind; 8] = [
    Kind {
        name: "the reader's More menu",
        ready: reading,
        open: |harness| press_button(harness, "More"),
        choose: |harness| {
            let name = if harness.query_by_label("Show the full header").is_some() { "Show the full header" } else { "Hide the full header" };
            harness.get_by_role_and_label(Role::MenuItem, name).click();
        },
        shown: |harness| harness.query_by_label("Show the full header").or_else(|| harness.query_by_label("Hide the full header")).is_some(),
        away: OFF_THE_MAIL,
    },
    Kind {
        name: "the Move menu",
        ready: reading,
        open: |harness| press_button(harness, "Move to a folder"),
        choose: |harness| {
            let row = harness.query_all_by_role(Role::MenuItem).last().unwrap();
            row.click();
        },
        shown: |harness| harness.query_by_label("Move to").is_some(),
        away: OFF_THE_MAIL,
    },
    Kind {
        name: "the Snooze picker",
        ready: reading,
        open: |harness| press_button(harness, "Snooze"),
        choose: |harness| harness.get_by_label_contains("Tomorrow morning").click(),
        shown: |harness| harness.query_by_label("Snooze until").is_some(),
        away: OFF_THE_MAIL,
    },
    Kind {
        name: "the Send later picker",
        ready: composing,
        open: |harness| press_button(harness, "Send later"),
        choose: |harness| harness.get_by_label_contains("Tomorrow morning").click(),
        shown: |harness| harness.query_by_label_contains("Tomorrow morning").is_some(),
        away: COVERS,
    },
    Kind {
        name: "the account picker",
        ready: settle,
        open: |harness| press_button(harness, OWNER),
        choose: |harness| harness.get_by_role_and_label(Role::MenuItemRadio, "Studio").click(),
        shown: |harness| harness.query_by_role_and_label(Role::MenuItemRadio, "All accounts").is_some(),
        away: COVERS,
    },
    Kind {
        name: "a select on a settings page",
        ready: on_the_form,
        open: |harness| {
            harness.get_by_label("IMAP security").click();
            harness.run();
        },
        choose: |harness| harness.get_by_role_and_label(Role::MenuItemRadio, "STARTTLS").click(),
        shown: |harness| harness.query_by_role_and_label(Role::MenuItemRadio, "STARTTLS").is_some(),
        away: OFF_THE_SETTINGS,
    },
    Kind {
        name: "the composer's From menu",
        ready: composing,
        open: |harness| {
            from_field(harness).click();
            harness.run();
        },
        choose: |harness| harness.get_by_label("Studio, holloway.studio@example.org").click(),
        shown: |harness| harness.query_by_label("Studio, holloway.studio@example.org").is_some(),
        away: COVERS,
    },
    Kind {
        name: "the event editor's Repeat select",
        ready: editing,
        open: |harness| {
            harness.get_by_role_and_label(Role::ComboBox, "Repeat").click();
            harness.run();
        },
        choose: |harness| harness.get_by_label("Weekdays").click(),
        shown: |harness| harness.query_by_label("Weekdays").is_some(),
        away: COVERS,
    },
];

/// The palette comes over the window and goes.
fn palette_comes_and_goes(harness: &mut Harness<'static, InboxApp>) {
    harness.key_press_modifiers(Modifiers::COMMAND, Key::K);
    settle(harness);
    assert!(harness.state().palette().is_open());
    harness.key_press(Key::Escape);
    settle(harness);
    assert!(!harness.state().palette().is_open());
}

/// The settings show and leave by their key (from a settings page: they leave and show again).
fn settings_come_and_go(harness: &mut Harness<'static, InboxApp>) {
    for _ in 0..2 {
        harness.key_press_modifiers(Modifiers::COMMAND, Key::Comma);
        settle(harness);
    }
}

#[test]
fn every_float_closed_by_a_choice_opens_again_with_one_press_after_a_cover_came_and_went() {
    let mut faults = Vec::new();
    for kind in &KINDS {
        for (cover, come_and_go) in COVERS {
            let mut harness = harness();
            (kind.ready)(&mut harness);
            (kind.open)(&mut harness);
            settle(&mut harness);
            assert!((kind.shown)(&harness), "{}: open", kind.name);
            (kind.choose)(&mut harness);
            settle(&mut harness);
            assert!(!(kind.shown)(&harness), "{}: closed by the choice", kind.name);
            come_and_go(&mut harness);
            (kind.ready)(&mut harness);
            (kind.open)(&mut harness);
            settle(&mut harness);
            if !(kind.shown)(&harness) {
                faults.push(format!("{} after {cover} came and went", kind.name));
            }
        }
    }
    assert!(faults.is_empty(), "one press opens it and it stays open: {faults:#?}");
}

#[test]
fn every_float_open_when_its_place_stopped_being_drawn_is_closed_on_return_and_opens_with_one_press() {
    let mut faults = Vec::new();
    for kind in &KINDS {
        for (way, away_and_back) in kind.away {
            let mut harness = harness();
            (kind.ready)(&mut harness);
            (kind.open)(&mut harness);
            settle(&mut harness);
            assert!((kind.shown)(&harness), "{}: open", kind.name);
            away_and_back(&mut harness);
            (kind.ready)(&mut harness);
            settle(&mut harness);
            if (kind.shown)(&harness) {
                faults.push(format!("{} after {way}: open by itself", kind.name));
                continue;
            }
            (kind.open)(&mut harness);
            settle(&mut harness);
            if !(kind.shown)(&harness) {
                faults.push(format!("{} after {way}: one press does not open it", kind.name));
            }
        }
    }
    assert!(faults.is_empty(), "{faults:#?}");
}
