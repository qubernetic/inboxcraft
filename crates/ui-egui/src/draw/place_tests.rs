// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The settings as a place of the window: where their columns stand in the frames `settings-*/default`, what leaving
//! them keeps and drops, a check that goes on while another place shows, the palette over them, and nothing of the
//! mail running on them.

use egui::accesskit::Role;
use egui::{Key, Modifiers, Rect, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use inboxcraft_time::{FakeClock, Timestamp};

use super::InboxApp;
use super::settings::Columns;
use crate::keymap;
use crate::mailbox::Mailbox;
use crate::onboard::tests::{FakeWorkers, Kept, PASSWORD, mailbox, proposal};
use crate::onboard::{Proposed, Secret, Source, Stage};
use crate::sample;
use crate::settings::Section;

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

/// The frames' window, 1440 × 900, in the app's own frame.
fn window(mut app: InboxApp) -> Harness<'static, InboxApp> {
    app.set_own_frame(true);
    app.set_frame(inboxcraft_engine::appearance::WindowFrame::Own);
    let mut harness =
        Harness::builder().with_size(vec2(1440.0, 900.0)).with_theme(egui::Theme::Dark).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

fn sample_window() -> Harness<'static, InboxApp> {
    window(InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock())))
}

/// A window with no account whose checks are answered by `workers`.
fn adding_window(workers: FakeWorkers) -> Harness<'static, InboxApp> {
    let detect = Proposed::new(proposal(Source::Autoconfig));
    window(InboxApp::at_mail(mailbox()).with_accounts(std::sync::Arc::new(Kept::default()), Box::new(detect), Box::new(workers)))
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect::from_min_size(pos2(x, y), vec2(w, h))
}

/// The settings' key, Command and the comma.
fn settings_key(harness: &mut Harness<'static, InboxApp>) {
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Comma);
    harness.run();
}

#[test]
fn the_columns_stand_where_the_settings_frames_draw_them() {
    // `screen/settings-<section>/default` (accounts and privacy: `/list`) at 1440 × 900: the body 1205 × 826 at 234,46;
    // in it the sections' column 232 wide, its hairline at 466, then the page 40 in from it, 24 under the body's top,
    // 720 wide, 16 over the foot (Shortcuts' list runs to the foot, About's page keeps 24).
    let theme = crate::theme::Theme::new(crate::theme::Mode::Dark, crate::theme::TextScale::default());
    let body = super::shell::regions(vec2(1440.0, 900.0), &theme, 1.0).body;
    assert_eq!(body, rect(234.0, 46.0, 1205.0, 826.0));
    let mut harness = sample_window();
    for (index, section) in Section::ALL.into_iter().take(9).enumerate() {
        let foot = match section {
            Section::Shortcuts => 0.0,
            Section::About => 24.0,
            _ => 16.0,
        };
        let columns = Columns::of(body, section);
        assert_eq!(columns.nav, rect(234.0, 46.0, 232.0, 826.0), "{section:?}");
        assert_eq!(columns.divider, 466.0, "{section:?}");
        assert_eq!(columns.page, rect(507.0, 70.0, 720.0, 826.0 - 24.0 - foot), "{section:?}");
        // Drawn: the settings fill the window's body; the rows of the sections' column under its head 48, its hairline
        // and 8, 30 each; the page's head in the page's column, its words 16 in.
        super::tests::at_settings(harness.state_mut(), section);
        harness.run();
        let place = harness.get_by_role_and_label(Role::Pane, "Settings").rect();
        assert_eq!(Some(place), super::shell::laid(&harness.ctx).map(|laid| laid.body), "{section:?}: the settings fill the body");
        let row = harness.get_by_label(&format!("{}, settings page", section.name())).rect();
        assert_eq!(row, rect(place.left(), place.top() + 57.0 + 30.0 * index as f32, 232.0, 30.0), "{section:?}");
        let page = Columns::of(place, section).page;
        let title = harness.get_by_role_and_label(Role::Heading, section.name()).rect();
        assert!((title.left() - (page.left() + 16.0)).abs() <= 1.0, "{section:?}: the title {title:?} in {page:?}");
        assert!(title.top() >= page.top() && title.top() - page.top() <= 32.0, "{section:?}: the title {title:?} under the page's top");
    }
}

#[test]
fn leaving_settings_keeps_the_form_and_clears_the_password() {
    let mut harness = adding_window(FakeWorkers::passing());
    harness.state_mut().onboarding_mut().0.open();
    super::tests::at_settings(harness.state_mut(), Section::Accounts);
    let form = harness.state_mut().onboarding_mut().0.form_mut();
    form.address = "carol@example.org".to_owned();
    form.password = Secret::new(PASSWORD);
    harness.run();
    settings_key(&mut harness);
    assert!(!harness.state().settings().is_open(), "the settings are left");
    let onboarding = harness.state().onboarding();
    assert_eq!(onboarding.stage(), Stage::Form, "the form is kept as it was");
    assert_eq!(onboarding.form().address, "carol@example.org", "with what was typed");
    assert!(onboarding.form().password.is_empty(), "but not the password");
    settings_key(&mut harness);
    assert_eq!(harness.state().settings().section(), Section::Accounts);
    harness.get_by_label("Add an account");
    assert_eq!(harness.state().onboarding().form().address, "carol@example.org", "the form shows again as left");
}

#[test]
fn typing_in_the_command_line_keeps_the_settings_and_the_password_until_enter() {
    let mut harness = adding_window(FakeWorkers::passing());
    harness.state_mut().onboarding_mut().0.open();
    super::tests::at_settings(harness.state_mut(), Section::Accounts);
    let form = harness.state_mut().onboarding_mut().0.form_mut();
    form.address = "carol@example.org".to_owned();
    form.password = Secret::new(PASSWORD);
    harness.run();
    let kept = |harness: &Harness<'static, InboxApp>, step: &str| {
        assert!(harness.state().settings().is_open(), "{step}: still on the settings");
        assert!(!harness.state().onboarding().form().password.is_empty(), "{step}: the password kept");
    };
    harness.key_press_modifiers(Modifiers::COMMAND, Key::K);
    harness.run();
    harness.get_by_role_and_label(Role::TextInput, "Command line").type_text("a");
    harness.run();
    kept(&harness, "a word typed");
    assert_eq!(harness.state().search_text(), "", "nothing searched under the settings");
    harness.key_press(Key::Escape);
    harness.run();
    kept(&harness, "Esc");
    harness.get_by_role_and_label(Role::TextInput, "Command line").type_text("b");
    harness.run();
    kept(&harness, "typed again");
    harness.key_press(Key::Enter);
    harness.run();
    assert!(!harness.state().settings().is_open(), "Enter leaves the settings");
    assert_eq!((harness.state().screen(), harness.state().search_text()), (crate::Screen::Mail, "ab"), "for the mail searched");
    assert!(harness.state().onboarding().form().password.is_empty(), "leaving them clears the password, as ever");
}

/// The 6 × 6 squares of `cyan_edge` painted inside `within`.
fn squares(harness: &Harness<'static, InboxApp>, within: Rect) -> usize {
    let cyan = crate::theme::Theme::new(crate::theme::Mode::Dark, crate::theme::TextScale::default()).palette.cyan_edge;
    fn walk(shape: &egui::Shape, cyan: egui::Color32, within: Rect, found: &mut usize) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, cyan, within, found)),
            egui::Shape::Rect(rect) if rect.fill == cyan && rect.rect.size() == vec2(6.0, 6.0) && within.contains_rect(rect.rect) => *found += 1,
            _ => {}
        }
    }
    let mut found = 0;
    for clipped in &harness.output().shapes {
        walk(&clipped.shape, cyan, within, &mut found);
    }
    found
}

#[test]
fn a_check_goes_on_while_another_place_is_shown() {
    let passed = |step| (step, inboxcraft_model::StepStatus::Passed);
    let workers = FakeWorkers::answering(&[passed(inboxcraft_model::CheckStep::Dns)]).unfinished();
    let mut harness = adding_window(workers);
    harness.state_mut().onboarding_mut().0.open();
    super::tests::at_settings(harness.state_mut(), Section::Accounts);
    let form = harness.state_mut().onboarding_mut().0.form_mut();
    form.address = "carol@example.org".to_owned();
    form.password = Secret::new(PASSWORD);
    form.host = "imap.example.org".to_owned();
    let (onboarding, mailbox) = harness.state_mut().onboarding_mut();
    onboarding.add(mailbox);
    harness.state_mut().pump();
    harness.run();
    let settings_row = |harness: &Harness<'static, InboxApp>| harness.get_by_role_and_label(Role::Button, "Settings").rect();
    assert!(harness.state().onboarding().checking(), "the check runs");
    assert_eq!(squares(&harness, settings_row(&harness)), 0, "on the Accounts page SETTINGS carries no square");
    // The calendar shown: the check goes on, and SETTINGS says so with its square.
    let ctx = harness.ctx.clone();
    harness.state_mut().run_action(&ctx, keymap::CALENDAR);
    harness.run();
    assert!(!harness.state().settings().is_open());
    assert!(harness.state().onboarding().checking(), "the check goes on");
    assert_eq!(squares(&harness, settings_row(&harness)), 1, "SETTINGS carries the square while the check runs elsewhere");
    // Back on the settings, the check shows where it stands.
    settings_key(&mut harness);
    assert_eq!(harness.state().settings().section(), Section::Accounts);
    assert!(matches!(harness.state().onboarding().stage(), Stage::Checking(_)));
    harness.get_by_label("Checking carol@example.org");
    assert_eq!(squares(&harness, settings_row(&harness)), 0, "and the square goes");
}

#[test]
fn the_palette_no_longer_closes_settings() {
    let mut harness = sample_window();
    super::tests::at_settings(harness.state_mut(), Section::Triage);
    harness.run();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::K);
    harness.run();
    assert!(harness.state().command().results());
    assert!(harness.state().settings().is_open(), "the palette comes over the settings");
    harness.key_press(Key::Escape);
    harness.run();
    assert!(!harness.state().command().results(), "one Esc: the palette");
    assert!(harness.state().settings().is_open(), "the settings stay, on their page");
    assert_eq!(harness.state().settings().section(), Section::Triage);
}

/// Every action that touches a message, a thread, the list, the reader or the calendar: none runs on the settings,
/// which show none of them. The test's own list, so that an id dropped from the window's rule fails it.
pub(super) const OF_THE_MAIL: [&str; 29] = [
    keymap::DONE,
    keymap::SNOOZE,
    keymap::ARCHIVE,
    keymap::READ,
    keymap::FLAG,
    keymap::DELETE,
    keymap::UNDO,
    keymap::BACK,
    keymap::OPEN_HELD,
    keymap::VIEW_GROUPED,
    keymap::VIEW_SMART,
    keymap::VIEW_LIST,
    keymap::NEXT,
    keymap::PREVIOUS,
    keymap::DOWN,
    keymap::UP,
    keymap::PAGE_DOWN,
    keymap::PAGE_UP,
    keymap::FIRST,
    keymap::LAST,
    keymap::OPEN,
    keymap::MENU,
    keymap::MENU_KEY,
    keymap::CALENDAR_LEFT,
    keymap::CALENDAR_RIGHT,
    keymap::CALENDAR_UP,
    keymap::CALENDAR_DOWN,
    keymap::CALENDAR_OPEN,
    keymap::CALENDAR_CLOSE,
];

/// The composer's own actions, Send and fold: they run wherever a composer is drawn and rest where none is, as on the
/// settings, which draw none.
pub(super) const OF_THE_COMPOSER: [&str; 2] = [keymap::SEND, keymap::FOLD];

/// The window's own actions: they run on the settings as off them (a way to a place shows it, leaving the settings).
pub(super) const THE_WINDOW_S: [&str; 16] = [
    keymap::FIND,
    keymap::HOME,
    keymap::MAIL,
    keymap::SETTINGS,
    keymap::PALETTE,
    keymap::COMMAND_LINE,
    keymap::CALENDAR,
    keymap::SENT,
    keymap::LATER,
    keymap::SHORTCUTS,
    keymap::FULL_WIDTH,
    keymap::LIGHT,
    keymap::DARK,
    keymap::SYSTEM_MODE,
    keymap::MOTION,
    keymap::COMPOSE,
];

#[test]
fn every_action_is_of_the_mail_the_composer_or_the_window_s_and_none_twice() {
    for action in keymap::actions() {
        let lists = [OF_THE_MAIL.contains(&action.id), OF_THE_COMPOSER.contains(&action.id), THE_WINDOW_S.contains(&action.id)];
        assert_eq!(lists.iter().filter(|on| **on).count(), 1, "{}: classify it in exactly one of the three lists", action.id);
    }
}

#[test]
fn on_the_settings_no_key_and_no_press_runs_an_action_of_the_mail() {
    // The gates themselves, whatever the mail holds: a key asks `allows` with the owner's keys (the default ones and
    // each action on a key with Ctrl), a press or a screen reader's activate asks `allows_press`.
    let settings = crate::surface::Surfaces::new(crate::surface::Facts { settings: true, ..crate::surface::Facts::default() });
    for id in OF_THE_MAIL.into_iter().chain(OF_THE_COMPOSER) {
        let mut with_ctrl = keymap::Keymap::default();
        with_ctrl.bind(id, egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::F9));
        for keys in [keymap::Keymap::default(), with_ctrl] {
            assert!(!settings.allows(id, &keys), "{id}: its key runs nothing on the settings");
        }
        assert!(!settings.allows_press(id), "{id}: a press runs nothing on the settings");
    }
    for id in THE_WINDOW_S {
        assert!(settings.allows_press(id), "{id}: the window's own runs by a press on the settings");
    }
}

#[test]
fn nothing_of_the_mail_runs_on_the_settings_place() {
    let mut harness = sample_window();
    // An act to take back, so that Undo could run off the settings.
    let last = harness.state().mailbox().messages().last().map(|row| row.id).unwrap();
    harness.state_mut().mailbox_mut().act(last, crate::mailbox::Act::Archive);
    let first = harness.state().mailbox().messages().first().map(|row| row.id);
    harness.state_mut().mailbox_mut().select(first);
    harness.run();
    assert!(harness.state().can_run(keymap::UNDO), "Undo could run off the settings");
    let before: Vec<(&str, bool)> = keymap::actions().into_iter().map(|action| (action.id, harness.state().can_run(action.id))).collect();
    super::tests::at_settings(harness.state_mut(), Section::Appearance);
    harness.run();
    for (id, could) in before {
        let can = harness.state().can_run(id);
        if OF_THE_MAIL.contains(&id) || OF_THE_COMPOSER.contains(&id) {
            assert!(!can, "{id}: acts on the mail, the calendar or a composer, which the settings do not show");
        } else {
            assert_eq!(can, could, "{id}: the window's own runs on the settings as off them");
        }
    }
}
