// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::accesskit::Role;
use egui::{Key, KeyboardShortcut, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

use super::{bubble, bubble_noted};
use crate::component::button::IconButton;
use crate::component::tests::{Look, Own, builder, own, painted, quick_tooltips, staged, theme};
use crate::iconset::IconId;
use crate::keymap::{FIND, Keymap};
use crate::theme::{Mode, TextScale, Theme};

fn harness() -> Harness<'static, Keymap> {
    let theme = theme();
    let mut harness = builder().build_ui_state(
        move |ui, keymap: &mut Keymap| {
            IconButton::new(IconId::Search, "Search").action(FIND).show(ui, &theme, keymap);
            IconButton::new(IconId::Reply, "Reply").action("mail.reply").enabled(false).show(ui, &theme, keymap);
        },
        Keymap::default(),
    );
    quick_tooltips(&harness.ctx);
    harness.run();
    harness
}

#[test]
fn hovering_a_button_shows_its_name_and_key() {
    let mut harness = harness();
    assert!(harness.query_by_label("Ctrl F").is_none(), "no tooltip before the hover");
    harness.get_by_label("Search").hover();
    harness.run();
    harness.get_by_label("Ctrl F");
    assert_eq!(harness.query_all_by_label("Search").count(), 2, "the button and its tooltip's name");
}

#[test]
fn a_changed_key_shows_in_its_tooltip_at_once() {
    let mut harness = harness();
    harness.get_by_label("Search").hover();
    harness.run();
    harness.get_by_label("Ctrl F");
    let changed = KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, Key::K);
    let written = crate::component::key::key_words(&changed);
    assert_eq!(written, "Ctrl Shift K");
    harness.state_mut().bind(FIND, changed);
    harness.step();
    harness.get_by_label(&written);
    assert!(harness.query_by_label("Ctrl F").is_none());
    harness.state_mut().unbind(FIND);
    harness.step();
    assert!(harness.query_by_label(&written).is_none(), "an action without a key shows no chip");
    assert_eq!(harness.query_all_by_label("Search").count(), 2, "the tooltip still shows the name");
}

#[test]
fn a_disabled_button_still_explains_itself() {
    let mut harness = harness();
    harness.state_mut().bind("mail.reply", KeyboardShortcut::new(Modifiers::NONE, Key::R));
    harness.get_by_label("Reply").hover();
    harness.run();
    harness.get_by_label("R");
}

#[test]
fn the_bubble_alone_names_the_action_and_its_key() {
    let theme = theme();
    let mut harness = builder().build_ui(move |ui| {
        bubble(ui, &theme, &Keymap::default(), "Search", Some(FIND));
        bubble(ui, &theme, &Keymap::default(), "Flag", None);
        bubble(ui, &theme, &Keymap::empty(), "Find", Some(FIND));
    });
    harness.run();
    harness.get_by_label("Search");
    harness.get_by_label("Flag");
    harness.get_by_label("Find");
    assert_eq!(harness.query_all_by_label("Ctrl F").count(), 1);
}

/// A tooltip's words that wrap at its widest.
const LONG: &str = "Snooze is not there for a message that is still being sent. Wait until it has left the outbox, then try again.";

/// The states of the tooltip's sheet, in its order: its name, its action, its reason.
const STATES: [(&str, &str, Option<&str>, Option<&str>); 4] = [
    ("default", "Archive", None, None),
    ("with-key", "Archive", Some("mail.archive"), None),
    ("unavailable-reason", "Snooze", None, Some("Not yet: choose a message first")),
    ("long", LONG, None, None),
];

/// The bubble of `name` on a `bg` stage: its rect and its look.
fn bubble_in(mode: Mode, scale: TextScale, name: &'static str, action: Option<&'static str>, note: Option<&'static str>) -> (egui::Rect, Own) {
    let theme = Theme::new(mode, scale);
    let harness = staged(
        theme,
        |p| p.bg,
        egui::vec2(400.0, 160.0),
        move |ui, theme| {
            let mut keymap = Keymap::default();
            keymap.bind("mail.archive", KeyboardShortcut::new(Modifiers::NONE, Key::A));
            bubble_noted(ui, theme, &keymap, name, action, note);
        },
    );
    let rect = harness.get_by_role_and_label(Role::Tooltip, name).rect();
    (rect, own(&painted(&harness, name), rect))
}

#[test]
fn a_tooltip_looks_as_its_sheet_says_in_every_state() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let p = theme.palette;
        let font = theme.fonts.mono_s;
        let family = font.id().family.to_string();
        for (state, name, action, note) in STATES {
            let (rect, look) = bubble_in(mode, TextScale::default(), name, action, note);
            let at = format!("{mode:?} {state}: {look:#?}");
            let (width, height) = match state {
                "default" => (63.0, 24.0),
                "with-key" => (88.0, 28.0),
                "unavailable-reason" => (221.0, 40.0),
                _ => (280.0, 56.0),
            };
            assert!((rect.width() - width).abs() <= 1.0 && (rect.height() - height).abs() < 0.01, "{at} {rect:?}");
            assert_eq!(look.fill, Some(p.surface), "{at}");
            assert_eq!(look.edge, Some((1.0, p.line, false)), "{at}");
            assert_eq!(look.ring, None, "{at}");
            let mut texts = vec![(name.to_owned(), p.ink, family.clone(), font.size)];
            if action.is_some() {
                let key = theme.fonts.key;
                texts.push(("A".to_owned(), p.muted, key.id().family.to_string(), key.size));
            }
            if let Some(note) = note {
                texts.push((note.to_owned(), p.muted, family.clone(), font.size));
            }
            assert_eq!(look.texts, texts, "{at}");
            assert!(!look.overflows, "{at}");
            assert!(look.radius <= if action.is_some() { 2.0 } else { 0.0 }, "{at}");
        }
    }
}

#[test]
fn a_tooltip_casts_the_one_shadow() {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let harness = staged(
        theme,
        |p| p.bg,
        egui::vec2(300.0, 120.0),
        |ui, theme| {
            bubble_noted(ui, theme, &Keymap::default(), "Archive", None, None);
        },
    );
    let paint = painted(&harness, "Archive");
    let shadow = crate::theme::FLOAT_SHADOW;
    assert!(
        paint.shapes.iter().any(
            |shape| matches!(shape.look, Look::Shadow { offset, blur, .. } if offset == egui::vec2(0.0, 12.0) && blur == f32::from(shadow.blur))
        ),
        "{paint:#?}"
    );
}

#[test]
fn a_tooltip_at_130_percent_grows_with_its_text() {
    let theme = Theme::new(Mode::Light, TextScale::ALL[4]);
    let (rect, _) = bubble_in(Mode::Light, TextScale::ALL[4], "Archive", Some("mail.archive"), None);
    assert!((rect.height() - (theme.boxes().chip + 8.0)).abs() < 0.01, "{rect:?}");
}

#[test]
fn outside_its_box_a_tooltip_paints_the_one_shadow_and_nothing_else() {
    for mode in [Mode::Dark, Mode::Light] {
        for (state, name, action, note) in STATES {
            let theme = Theme::new(mode, TextScale::default());
            let harness = staged(
                theme,
                |p| p.bg,
                egui::vec2(400.0, 160.0),
                move |ui, theme| {
                    let mut keymap = Keymap::default();
                    keymap.bind("mail.archive", KeyboardShortcut::new(Modifiers::NONE, Key::A));
                    bubble_noted(ui, theme, &keymap, name, action, note);
                },
            );
            let rect = harness.get_by_role_and_label(Role::Tooltip, name).rect();
            let paint = painted(&harness, name);
            let outside: Vec<_> = paint.shapes.iter().filter(|shape| !rect.expand(0.01).contains_rect(shape.rect)).map(|shape| &shape.look).collect();
            let shadow = crate::theme::FLOAT_SHADOW;
            let one = Look::Shadow { offset: egui::vec2(0.0, 12.0), blur: f32::from(shadow.blur), spread: 0.0, colour: shadow.color };
            assert_eq!(outside, vec![&one], "{mode:?} {state}");
        }
    }
}

/// The pixels of the unavailable switch's dashed edge (its colour `muted`, the ground `bg`) with its reason tooltip
/// drawn under it as a board draws it: the count of pixels darker than `muted` in the edge's rows.
fn darkened_dashes(mode: Mode) -> usize {
    use crate::component::switch::{SwitchState, switch_noted};
    let theme = Theme::new(mode, TextScale::default());
    let keymap = Keymap::empty();
    let (muted, ground) = (theme.palette.muted, theme.palette.bg);
    let size = egui::vec2(240.0, 80.0);
    let picture = crate::shot::paint(size, 1.0, mode, |ui| {
        crate::theme::install_fonts(ui.ctx());
        ui.painter().rect_filled(egui::Rect::from_min_size(egui::Pos2::ZERO, size), 0.0, ground);
        let under = ui.painter().add(egui::Shape::Noop);
        switch_noted(ui, &theme, "Background", SwitchState::Unavailable, Some("Not there in the Flatpak yet"));
        super::bubble_noted_under(ui, &theme, &keymap, "Background", None, Some("Not there in the Flatpak yet"), under);
    })
    .unwrap();
    let close = |a: egui::Color32, b: egui::Color32| a.r().abs_diff(b.r()).max(a.g().abs_diff(b.g())).max(a.b().abs_diff(b.b())) <= 8;
    let mut darker = 0;
    for y in 0..20 {
        for x in 0..40 {
            let Some(pixel) = picture.pixel(x, y) else { continue };
            // A pixel of the dashes, but not the colour itself: the shadow has darkened it.
            if close(pixel, muted) && pixel != muted {
                darker += 1;
            }
        }
    }
    darker
}

#[test]
fn a_board_s_reason_tooltip_lays_its_shadow_under_the_control_above_it() {
    for mode in [Mode::Dark, Mode::Light] {
        assert_eq!(darkened_dashes(mode), 0, "{mode:?}: the switch's dashes keep their colour");
    }
}
