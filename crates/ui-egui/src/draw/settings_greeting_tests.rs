// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! How Home greets you, on Settings › Sending: four segments, the one chosen marked, a press sets it through
//! `greeting.set`, and what the row says follows the choice.

use egui::accesskit::{Role, Toggled};
use egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_time::{FakeClock, Timestamp};

use crate::mailbox::{GreetBy, Mailbox};
use crate::settings::Section;
use crate::{InboxApp, sample};

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

/// The sample window on Settings › Sending, motion off, tall enough for the whole page.
fn window() -> Harness<'static, InboxApp> {
    let mut app = InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    app.set_reduce_motion(true);
    let mut harness = Harness::builder().with_size(vec2(1300.0, 1600.0)).with_max_steps(40).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    crate::draw::tests::at_settings(harness.state_mut(), Section::Sending);
    harness.run();
    harness
}

/// The segment named `name`, as the screen reader has it.
fn segment(name: &str) -> String {
    format!("{name} for how Home greets you")
}

/// Which of the four segments is marked chosen.
fn marked(harness: &Harness<'static, InboxApp>) -> Vec<&'static str> {
    ["First word", "Last word", "Full name", "No name"]
        .into_iter()
        .filter(|name| harness.get_by_role_and_label(Role::RadioButton, &segment(name)).accesskit_node().toggled() == Some(Toggled::True))
        .collect()
}

#[test]
fn the_sending_page_shows_and_sets_how_home_greets_you() {
    let mut harness = window();
    assert_eq!(marked(&harness), ["First word"], "the first word by default");
    assert!(harness.query_all_by_label_contains("the first word of your sending name").next().is_some());
    for (name, by, says) in [
        ("Last word", GreetBy::Last, "the last word of your sending name"),
        ("Full name", GreetBy::Full, "your whole sending name"),
        ("No name", GreetBy::None, "without a name"),
        ("First word", GreetBy::First, "the first word of your sending name"),
    ] {
        harness.get_by_role_and_label(Role::RadioButton, &segment(name)).click();
        harness.run();
        assert_eq!(harness.state().mailbox().greet_by(), by, "{name}");
        assert_eq!(marked(&harness), [name]);
        assert!(harness.query_all_by_label_contains(says).next().is_some(), "{name}: {says}");
    }
}

#[test]
fn a_hover_or_a_focus_changes_nothing() {
    let mut harness = window();
    harness.get_by_role_and_label(Role::RadioButton, &segment("Full name")).hover();
    harness.run();
    harness.get_by_role_and_label(Role::RadioButton, &segment("Full name")).focus();
    harness.run();
    assert_eq!(harness.state().mailbox().greet_by(), GreetBy::First);
}

#[test]
fn a_greeting_kept_from_an_earlier_run_is_the_one_marked() {
    let mut harness = window();
    harness.state_mut().mailbox_mut().restore_greet_by(GreetBy::Full);
    harness.run();
    assert_eq!(marked(&harness), ["Full name"]);
}
