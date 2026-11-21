// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! An agent's `ui.home.open` and `ui.settings.open` leave the owner where they are while the owner holds the keys: the
//! command line's results, a dialog, an open menu, the composer with the keyboard (docked or floating), the event
//! editor (also under the settings); with none of them, they go through.

use std::sync::Arc;

use egui::{Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use inboxcraft_engine::shown::{Asked, HomeAsked, SettingsAsked};
use inboxcraft_time::{FakeClock, Timestamp};

use super::super::{InboxApp, Screen};
use crate::component::popup;
use crate::mailbox::Mailbox;
use crate::onboard::NoDetect;
use crate::onboard::tests::Kept;
use crate::sample;
use crate::workers::NoWorkers;

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

/// The sample window on the mail, `set` done to it before its first frame.
fn window(set: impl FnOnce(&mut InboxApp)) -> Harness<'static, InboxApp> {
    let mut app = InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock())).with_accounts(
        Arc::new(Kept::default()),
        Box::new(NoDetect::default()),
        Box::new(NoWorkers),
    );
    app.set_reduce_motion(true);
    set(&mut app);
    let mut harness = Harness::builder()
        .with_size(vec2(1440.0, 900.0))
        .with_max_steps(40)
        .with_theme(egui::Theme::Dark)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

const HOME: Asked = Asked::Home(HomeAsked { by_agent: true });
const ABOUT: Asked = Asked::Settings(SettingsAsked { page: Some("about"), bare: false, by_agent: true });

/// Where the owner is: the place, whether the settings show and on which page.
fn place(harness: &Harness<'static, InboxApp>) -> (Screen, bool, crate::settings::Section) {
    let app = harness.state();
    (app.screen(), app.settings().is_open(), app.settings().section())
}

/// `harness` with the keyboard in its composer's text, as the owner writing.
fn typing(mut harness: Harness<'static, InboxApp>) -> Harness<'static, InboxApp> {
    harness.ctx.memory_mut(|memory| memory.request_focus(super::super::compose::text_id()));
    harness.run();
    harness
}

/// An agent's `asked`, held and acted on at the frame's end as the window does, then a frame.
fn agent_asks(harness: &mut Harness<'static, InboxApp>, asked: Asked) {
    harness.state_mut().mailbox.hold_agent_ask(asked);
    harness.state_mut().act_on_agent_ask(false);
    harness.run();
}

/// Each of an agent's asks leaves the owner where they are in the window `held` makes, and `still` holds after it.
fn both_pass_over(held: impl Fn() -> Harness<'static, InboxApp>, still: impl Fn(&Harness<'static, InboxApp>) -> bool, what: &str) {
    for asked in [HOME, ABOUT] {
        let mut harness = held();
        assert!(still(&harness), "{what}: held before the ask");
        let before = place(&harness);
        agent_asks(&mut harness, asked);
        assert_eq!(place(&harness), before, "{what}: {asked:?} left the owner where they were");
        assert!(still(&harness), "{what}: {asked:?} took it away");
    }
}

#[test]
fn an_agent_s_ask_passes_over_the_command_line_s_results() {
    let held = || {
        let mut harness = window(|_| {});
        harness.key_press_modifiers(Modifiers::COMMAND, Key::K);
        harness.run();
        harness
    };
    both_pass_over(held, |harness| harness.state().command().results(), "the results");
}

#[test]
fn an_agent_s_ask_passes_over_a_dialog() {
    let held = || {
        let mut harness = window(|app| {
            app.open_accounts();
            let (onboarding, mailbox) = app.onboarding_mut();
            onboarding.edit(mailbox, 2);
        });
        harness.get_by_label("Remove account…").scroll_to_me();
        harness.run();
        harness.get_by_label("Remove account…").click();
        harness.run();
        harness
    };
    both_pass_over(held, |harness| harness.state().onboarding().asking_remove(), "the dialog");
}

#[test]
fn an_agent_s_ask_passes_over_an_open_menu() {
    let held = || {
        let mut harness = window(|_| {});
        harness.key_press(Key::J);
        harness.run();
        harness.key_press_modifiers(Modifiers::SHIFT, Key::F10);
        harness.run();
        harness
    };
    let open = |harness: &Harness<'static, InboxApp>| {
        harness.state().mailbox().selected().is_some_and(|row| popup::is_open(&harness.ctx, super::super::list::group_menu_id(row)))
    };
    both_pass_over(held, open, "the menu");
}

#[test]
fn an_agent_s_ask_passes_over_the_docked_composer_with_the_keyboard() {
    let held = || typing(window(InboxApp::compose));
    let docked = |harness: &Harness<'static, InboxApp>| harness.state().docked() && super::super::compose::has_keys(&harness.ctx);
    both_pass_over(held, docked, "the docked composer");
}

#[test]
fn an_agent_s_ask_passes_over_the_floating_composer_with_the_keyboard() {
    let held = || {
        typing(window(|app| {
            app.compose();
            app.show_home();
        }))
    };
    let floating = |harness: &Harness<'static, InboxApp>| {
        harness.state().composer().is_some() && !harness.state().docked() && super::super::compose::has_keys(&harness.ctx)
    };
    // On Home an agent's Home changes nothing anyway; the settings' ask is the one that would move the owner.
    let mut harness = held();
    assert_eq!(harness.state().screen(), Screen::Home);
    assert!(floating(&harness), "the floating composer has the keyboard");
    let before = place(&harness);
    agent_asks(&mut harness, ABOUT);
    assert_eq!(place(&harness), before, "the settings did not come over the composer");
    assert!(floating(&harness));
    // On the calendar, both would.
    let held = || {
        typing(window(|app| {
            app.compose();
            app.show_calendar();
        }))
    };
    both_pass_over(held, floating, "the floating composer on the calendar");
}

#[test]
fn an_agent_s_ask_passes_over_the_event_editor_also_under_the_settings() {
    let held = || {
        window(|app| {
            app.show_calendar();
            app.calendar_mut().0.new_event();
            app.open_settings_page(Some("appearance"));
        })
    };
    let editing = |harness: &Harness<'static, InboxApp>| harness.state().calendar().editor().is_some() && harness.state().settings().is_open();
    both_pass_over(held, editing, "the editor under the settings");
}

#[test]
fn with_nothing_held_an_agent_s_asks_go_through() {
    let mut harness = window(|_| {});
    assert_eq!(harness.state().screen(), Screen::Mail);
    assert!(!harness.state().owner_keys, "nothing held on the mail");
    agent_asks(&mut harness, HOME);
    assert_eq!(harness.state().screen(), Screen::Home, "Home shows");
    agent_asks(&mut harness, ABOUT);
    assert!(harness.state().settings().is_open(), "the settings show");
    // The composer kept but without the keyboard: the ask goes through, and the composer stays.
    let mut harness = typing(window(InboxApp::compose));
    assert!(super::super::compose::has_keys(&harness.ctx));
    harness.ctx.memory_mut(|memory| memory.stop_text_input());
    harness.run();
    assert!(!super::super::compose::has_keys(&harness.ctx), "the keyboard left the composer");
    agent_asks(&mut harness, HOME);
    assert_eq!(harness.state().screen(), Screen::Home, "Home shows");
    assert!(harness.state().composer().is_some(), "the composer is kept");
}
