// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The navigation rail: the preset's gradient at full strength down the window's left edge, with Mail and the
//! appearance settings on it.

use egui::accesskit::{Role, Toggled};
use egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_time::{FakeClock, Timestamp};

use super::InboxApp;
use crate::mailbox::Mailbox;
use crate::sample;
use crate::shot::render;
use crate::theme::{Mode, PRESETS, Theme, contrast, install_fonts};

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

fn app() -> InboxApp {
    InboxApp::new(Mailbox::new(sample::session(clock()).unwrap(), clock()))
}

fn close(a: egui::Color32, b: egui::Color32) -> bool {
    a.to_array().iter().zip(b.to_array()).all(|(a, b)| a.abs_diff(b) <= 6)
}

#[test]
fn the_rail_runs_the_preset_s_gradient_at_full_strength_down_the_left_edge() {
    for preset in PRESETS {
        for mode in [Mode::Light, Mode::Dark] {
            let mut app = app();
            app.set_preset(preset);
            let picture = render(&mut app, vec2(1100.0, 700.0), 1.0, mode).unwrap();
            let [first, _, last] = Theme::new(mode, &preset).palette.gradient;
            let (top, bottom) = (picture.pixel(2, 1).unwrap(), picture.pixel(2, 698).unwrap());
            assert!(close(top, first), "{} {mode:?}: top {top:?}, first stop {first:?}", preset.id);
            assert!(close(bottom, last), "{} {mode:?}: bottom {bottom:?}, last stop {last:?}", preset.id);
        }
    }
}

#[test]
fn the_rail_s_buttons_say_where_the_window_is_and_read_on_the_gradient() {
    let mut harness = Harness::builder()
        .with_size(vec2(1100.0, 700.0))
        .with_context_setup(install_fonts)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app());
    harness.run();
    let toggled = |harness: &Harness<'static, InboxApp>, name: &str| harness.get_by_role_and_label(Role::Button, name).accesskit_node().toggled();
    assert_eq!(toggled(&harness, "Mail"), Some(Toggled::True));
    assert_eq!(toggled(&harness, "Appearance settings"), Some(Toggled::False));
    let mail = harness.get_by_role_and_label(Role::Button, "Mail").rect();
    let settings = harness.get_by_role_and_label(Role::Button, "Appearance settings").rect();
    assert!(mail.left() < 30.0 && settings.top() > 600.0, "Mail on top, the settings at the foot: {mail:?} {settings:?}");
    harness.get_by_label("Appearance settings").click();
    harness.run();
    assert_eq!(toggled(&harness, "Mail"), Some(Toggled::False));
    assert_eq!(toggled(&harness, "Appearance settings"), Some(Toggled::True));
    for preset in PRESETS {
        for mode in [Mode::Light, Mode::Dark] {
            let theme = Theme::new(mode, &preset);
            let ink = super::pane::rail_ink(&theme);
            for stop in theme.palette.gradient {
                assert!(contrast(ink, stop) >= 3.0, "{} {mode:?}: {ink:?} on {stop:?}", preset.id);
            }
        }
    }
}
