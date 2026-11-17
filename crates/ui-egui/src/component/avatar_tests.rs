// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::component::tests::builder;
use egui::accesskit::Role;
use egui::{Rect, pos2, vec2};
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::{CORNER, avatar, initials, paint};
use crate::shot;
use crate::theme::{Mode, TextScale, Theme, contrast, install_fonts};

fn all() -> Vec<Theme> {
    [Mode::Light, Mode::Dark].map(|mode| Theme::new(mode, TextScale::default())).to_vec()
}

#[test]
fn an_avatar_is_quiet_and_not_quite_square() {
    assert_eq!(CORNER, 2.0, "radius 2: one of the two shapes that are not square");
    for theme in all() {
        assert!(contrast(theme.palette.muted, theme.palette.stage) >= 4.5, "{:?}: the initials read", theme.mode);
    }
}

#[test]
fn initials_come_from_the_first_and_last_word_or_the_address() {
    assert_eq!(initials("Mira Holloway", "mira@example.org"), "MH");
    assert_eq!(initials("Lumen Print accounts", "accounts@example.org"), "LA");
    assert_eq!(initials("Paper & Pine", "orders@example.org"), "PP");
    assert_eq!(initials("odile", "odile@example.org"), "O");
    assert_eq!(initials("", "loans@example.org"), "L");
    assert_eq!(initials("  ", "@example.org"), "?");
    assert_eq!(initials("", ""), "?");
    assert_eq!(initials("\"Ölga\" (ßtrand)", ""), "ÖSS");
    assert_eq!(initials("🎉 🎈", "x9@example.org"), "X");
    assert_eq!(initials("2nd Floor", ""), "2F");
}

#[test]
fn hostile_names_and_addresses_never_panic() {
    let long = "a ".repeat(50_000);
    for (name, address) in [("\0", "\0"), ("\u{202e}x", "\u{301}@\u{301}"), (long.as_str(), long.as_str()), ("e\u{301}", "")] {
        let _ = initials(name, address);
    }
    assert!(initials(&long, "").chars().count() <= 2);
}

#[test]
fn the_avatar_is_an_image_named_for_the_person() {
    let theme = Theme::new(Mode::Light, TextScale::default());
    let mut harness = builder().build_ui(move |ui| {
        avatar(ui, &theme, "Odile Brannock", "odile@example.org", 32.0);
        avatar(ui, &theme, "", "loans@example.org", 32.0);
        avatar(ui, &theme, "x", "x@example.org", f32::NAN);
    });
    harness.run();
    assert_eq!(harness.get_by_label("Odile Brannock").accesskit_node().role(), Role::Image);
    harness.get_by_label("loans@example.org");
}

#[test]
fn the_avatar_paints_the_stage_with_the_muted_initials_in_the_middle() {
    for theme in all() {
        let picture = shot::paint(vec2(40.0, 40.0), 1.0, theme.mode, |ui| {
            install_fonts(ui.ctx());
            paint(ui.painter(), &theme, Rect::from_min_size(pos2(0.0, 0.0), vec2(40.0, 40.0)), "Odile Brannock", "odile@example.org");
            paint(ui.painter(), &theme, Rect::NOTHING, "x", "x");
        })
        .unwrap();
        let fill = theme.palette.stage;
        let near = |x, y| {
            let pixel = picture.pixel(x, y).unwrap();
            contrast(egui::Color32::from_rgb(pixel.r(), pixel.g(), pixel.b()), fill) < 1.05
        };
        assert!(near(5, 20) && near(20, 3), "{:?}", theme.mode);
        let ink = theme.palette.muted;
        let inked = (12..28).flat_map(|x| (12..28).map(move |y| (x, y))).any(|(x, y)| {
            let pixel = picture.pixel(x, y).unwrap();
            contrast(egui::Color32::from_rgb(pixel.r(), pixel.g(), pixel.b()), ink) < 1.2
        });
        assert!(inked, "the initials are drawn in their ink");
        assert!(near(3, 3) && near(36, 36), "{:?}: the square reaches its corners", theme.mode);
    }
}
