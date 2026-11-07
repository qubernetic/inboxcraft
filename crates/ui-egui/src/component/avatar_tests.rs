// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::component::tests::builder;
use egui::accesskit::Role;
use egui::{Rect, pos2, vec2};
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::{avatar, colour, initials, paint};
use crate::sample;
use crate::shot;
use crate::theme::{AVATARS, Mode, PRESETS, Preset, Theme, contrast, ink_on, install_fonts};

fn all() -> Vec<Theme> {
    PRESETS.iter().flat_map(|preset| [Theme::new(Mode::Light, preset), Theme::new(Mode::Dark, preset)]).collect()
}

#[test]
fn the_same_address_always_gets_the_same_colour() {
    let theme = Theme::new(Mode::Light, &Preset::default());
    for (_, address) in sample::people() {
        let first = colour(&theme, address);
        for _ in 0..3 {
            assert_eq!(colour(&theme, address), first, "{address}");
        }
        assert_eq!(colour(&theme, &address.to_uppercase()), first, "case does not matter");
        assert_eq!(colour(&theme, &format!("  {address} ")), first, "spaces around do not matter");
        for other in all() {
            assert_eq!(colour(&other, address), first, "{address}: one colour in every mode and preset");
        }
    }
}

#[test]
fn the_colour_is_one_of_the_avatar_palette_and_people_spread_over_it() {
    let theme = Theme::new(Mode::Dark, &Preset::default());
    let mut used: Vec<_> = sample::people().iter().map(|(_, address)| colour(&theme, address)).collect();
    assert!(used.iter().all(|picked| AVATARS.contains(picked)));
    used.sort_by_key(|picked| picked.to_array());
    used.dedup();
    assert!(used.len() >= 6, "the sample's people spread over the palette: {}", used.len());
}

#[test]
fn a_known_address_keeps_its_colour_across_versions() {
    // FNV-1a of "odile@example.org" picks this slot; a changed hash would recolour everyone.
    let theme = Theme::new(Mode::Light, &Preset::default());
    let slot = AVATARS.iter().position(|picked| *picked == colour(&theme, "odile@example.org"));
    assert_eq!(slot, Some(fnv_slot("odile@example.org")));
}

fn fnv_slot(text: &str) -> usize {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    (hash % AVATARS.len() as u64) as usize
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
    let theme = Theme::new(Mode::Light, &Preset::default());
    for (name, address) in [("\0", "\0"), ("\u{202e}x", "\u{301}@\u{301}"), (long.as_str(), long.as_str()), ("e\u{301}", "")] {
        let _ = initials(name, address);
        let _ = colour(&theme, address);
    }
    assert!(initials(&long, "").chars().count() <= 2);
}

#[test]
fn every_avatar_s_initials_read_on_its_colour_in_every_theme() {
    for theme in all() {
        for (_, address) in sample::people() {
            let fill = colour(&theme, address);
            assert!(contrast(ink_on(fill), fill) >= 4.5, "{} {:?} {address}", theme.preset.id, theme.mode);
        }
    }
}

#[test]
fn the_avatar_is_an_image_named_for_the_person() {
    let theme = Theme::new(Mode::Light, &Preset::default());
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
fn the_avatar_paints_its_colour_with_the_initials_in_the_middle() {
    for theme in all() {
        let picture = shot::paint(vec2(40.0, 40.0), 1.0, theme.mode, |ui| {
            install_fonts(ui.ctx());
            paint(ui.painter(), &theme, Rect::from_min_size(pos2(0.0, 0.0), vec2(40.0, 40.0)), "Odile Brannock", "odile@example.org");
            paint(ui.painter(), &theme, Rect::NOTHING, "x", "x");
        })
        .unwrap();
        let fill = colour(&theme, "odile@example.org");
        let near = |x, y| {
            let pixel = picture.pixel(x, y).unwrap();
            contrast(egui::Color32::from_rgb(pixel.r(), pixel.g(), pixel.b()), fill) < 1.05
        };
        assert!(near(5, 20) && near(20, 3), "{} {:?}", theme.preset.id, theme.mode);
        let ink = ink_on(fill);
        let inked = (12..28).flat_map(|x| (12..28).map(move |y| (x, y))).any(|(x, y)| {
            let pixel = picture.pixel(x, y).unwrap();
            contrast(egui::Color32::from_rgb(pixel.r(), pixel.g(), pixel.b()), ink) < 1.2
        });
        assert!(inked, "the initials are drawn in their ink");
        assert_eq!(picture.pixel(0, 0).unwrap().a(), 0, "outside the circle nothing is painted");
    }
}
