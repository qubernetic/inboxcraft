// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::{Color32, Rect, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::{FileAsked, FileState, attachment, attachment_with, spoken};
use crate::component::Shown;
use crate::component::tests::{Look, Paint, own, painted, staged};
use crate::keymap::Keymap;
use crate::theme::{Mode, TextScale, Theme};

const NAME: &str = "bakery-logo-v2.pdf";
const SIZE: &str = "1.2 MB";

/// A state of the sheet: its name, the state, the look forced, the box's width, and its links' words with their left
/// edges.
type State = (&'static str, FileState<'static>, Shown, f32, Vec<(&'static str, f32)>);

/// The sheet's states.
fn states() -> [State; 6] {
    let ready = || vec![("open", 269.0), ("save", 312.0)];
    [
        ("default", FileState::Ready, Shown::Rest, 351.0, ready()),
        ("hover", FileState::Ready, Shown::Hover, 351.0, ready()),
        ("focus", FileState::Ready, Shown::Focus, 351.0, ready()),
        ("saving", FileState::Saving("saving [=====>  ]"), Shown::Rest, 394.0, vec![]),
        ("saved", FileState::Saved("saved to Downloads"), Shown::Rest, 505.0, vec![("open folder", 420.0)]),
        ("failed", FileState::Failed("not saved: the disk is full"), Shown::Rest, 532.0, vec![("try again", 460.0)]),
    ]
}

/// The attachment on the reader's `surface`.
fn harness(theme: Theme, state: FileState<'static>, shown: Shown) -> Harness<'static> {
    staged(
        theme,
        |p| p.surface,
        vec2(640.0, 100.0),
        move |ui, theme| {
            attachment_with(ui, theme, &Keymap::default(), NAME, SIZE, state, Some(shown));
        },
    )
}

/// The text painted that starts with `start`: its rect and colour.
fn text(paint: &Paint, start: &str) -> Option<(Rect, Color32)> {
    paint.shapes.iter().find_map(|shape| match &shape.look {
        Look::Text { text, colour, .. } if text.starts_with(start) => Some((shape.rect, *colour)),
        _ => None,
    })
}

#[test]
fn the_attachment_looks_as_its_sheet_says_in_every_state() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let p = theme.palette;
        for (state_name, state, shown, width, links) in states() {
            let harness = harness(theme, state, shown);
            let name = spoken(NAME, SIZE, state);
            let rect = harness.get_by_label(&name).rect();
            let paint = painted(&harness, &name);
            let at = format!("{mode:?} {state_name}: {paint:#?}");
            assert_eq!(rect.size(), vec2(width, 44.0), "{at}");
            let look = own(&paint, rect);
            let edge = if matches!(state, FileState::Failed(_)) { p.minus } else { p.line };
            assert_eq!(look.edge, Some((1.0, edge, false)), "{at}");
            assert_eq!(look.fill, (shown == Shown::Hover).then_some(p.stage), "{at}");
            assert_eq!(look.radius, 0.0, "{at}");
            assert_eq!(look.ring, (shown == Shown::Focus).then(|| (rect.expand(3.0), 1.0, p.cyan_edge)), "{at}");
            let icon = if shown == Shown::Hover { p.ink } else { p.muted };
            assert!(look.texts.iter().any(|(text, colour, ..)| text == "\u{e230}" && *colour == icon), "the file icon in {icon:?}: {at}");
            let (name_rect, name_ink) = text(&paint, NAME).unwrap_or_else(|| panic!("{at}"));
            assert_eq!((name_rect.left() - rect.left(), name_ink), (40.0, p.ink), "{at}");
            let (size_rect, size_ink) = text(&paint, SIZE).unwrap_or_else(|| panic!("{at}"));
            assert_eq!((size_rect.left() - rect.left(), size_ink), (193.0, p.muted), "{at}");
            match state {
                FileState::Ready => {}
                FileState::Saving(how) => assert_eq!(text(&paint, how).map(|(r, c)| (r.left() - rect.left(), c)), Some((269.0, p.muted)), "{at}"),
                FileState::Saved(how) => {
                    assert_eq!(text(&paint, how).map(|(r, c)| (r.left() - rect.left(), c)), Some((289.0, p.plus)), "{at}");
                    assert!(look.texts.iter().any(|(text, colour, ..)| text == "\u{e182}" && *colour == p.plus), "the check: {at}");
                }
                FileState::Failed(how) => assert_eq!(text(&paint, how).map(|(r, c)| (r.left() - rect.left(), c)), Some((269.0, p.minus)), "{at}"),
            }
            for (word, left) in links {
                let found = text(&paint, word).unwrap_or_else(|| panic!("{word}: {at}"));
                assert_eq!(found.0.left() - rect.left(), left, "{word}: {at}");
                // Opening a file or its folder is not built: those links stand unavailable.
                let colour = if word.starts_with("open") { p.muted } else { p.cyan_text };
                assert_eq!(found.1, colour, "{word}: {at}");
            }
        }
    }
}

#[test]
fn enter_on_the_box_saves_it_and_its_name_decides_nothing_but_its_label() {
    let theme = Theme::new(Mode::Light, TextScale::default());
    // A name that looks like a path and carries an override: it is a label and nothing more.
    let name = "\u{202E}fdp.exe/../../etc/passwd";
    let asked = std::rc::Rc::new(std::cell::Cell::new(None));
    let kept = asked.clone();
    let mut harness = staged(
        theme,
        |p| p.surface,
        vec2(640.0, 100.0),
        move |ui, theme| {
            let (_, got) = attachment(ui, theme, &Keymap::default(), name, SIZE, FileState::Ready);
            if got.is_some() {
                kept.set(got);
            }
        },
    );
    let spoken_name = spoken(name, SIZE, FileState::Ready);
    assert!(!spoken_name.contains('\u{202E}'));
    harness.get_by_label(&spoken_name).focus();
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert_eq!(asked.get(), Some(FileAsked::Save));
    let paint = painted(&harness, &spoken_name);
    assert!(own(&paint, harness.get_by_label(&spoken_name).rect()).texts.iter().any(|(text, ..)| text == "\u{e230}"), "the same icon for every file");
}

#[test]
fn a_long_name_without_a_dot_is_cut_at_240_and_the_box_keeps_its_height_at_130_percent() {
    let theme = Theme::new(Mode::Dark, TextScale::ALL[4]);
    let name: &'static str = Box::leak("a".repeat(500).into_boxed_str());
    let harness = staged(
        theme,
        |p| p.surface,
        vec2(900.0, 100.0),
        move |ui, theme| {
            attachment(ui, theme, &Keymap::default(), name, SIZE, FileState::Ready);
        },
    );
    let label = spoken(name, SIZE, FileState::Ready);
    let rect = harness.get_by_label(&label).rect();
    assert_eq!(rect.height(), theme.size(44.0));
    let paint = painted(&harness, &label);
    let (cut, _) = text(&paint, "aaa").unwrap_or_else(|| panic!("{paint:#?}"));
    assert!(cut.width() <= 240.0, "{cut:?}");
    assert!(paint.shapes.iter().any(|shape| matches!(&shape.look, Look::Text { text, .. } if text.starts_with("aaa") && text.ends_with('…'))));
    assert!(harness.get_by_label(&format!("Save {name}")).accesskit_node().label().is_some(), "the link is heard with the whole name");
}

#[test]
fn in_a_narrow_pane_nothing_paints_outside_the_box() {
    let long: &'static str = Box::leak("quarterly-report-with-a-very-long-name-".repeat(20).into_boxed_str());
    for scale in [TextScale::default(), TextScale::ALL[4]] {
        let theme = Theme::new(Mode::Dark, scale);
        for room in [330.0, 240.0, 160.0, 90.0, 40.0, 1.0] {
            for (name, state) in [
                (NAME, FileState::Ready),
                (long, FileState::Ready),
                (long, FileState::Failed("not saved: the disk is full")),
                (long, FileState::Saved("saved to Downloads")),
            ] {
                let harness = staged(
                    theme,
                    |p| p.surface,
                    vec2(room + 48.0, 120.0),
                    move |ui, theme| {
                        ui.set_max_width(room);
                        attachment(ui, theme, &Keymap::default(), name, "12 345.6 MB", state);
                    },
                );
                let label = spoken(name, "12 345.6 MB", state);
                let rect = harness.get_by_label(&label).rect();
                let at = format!("{scale:?} {room} {state:?}");
                assert!(rect.width() <= room.max(1.0) + 0.5, "the box keeps to the room: {rect:?} {at}");
                let paint = painted(&harness, &label);
                for shape in &paint.shapes {
                    if let Look::Text { text, .. } = &shape.look {
                        // What shows of a word stays inside the box: cut whole, or clipped at its edge.
                        assert!(shape.inside || shape.clipped, "{text:?} at {:?} paints past the box {rect:?}: {at}", shape.rect);
                    }
                }
                // A link that stands keeps inside the box.
                for link in harness.query_all_by_role(egui::accesskit::Role::Button) {
                    let link_rect = link.rect();
                    if link.accesskit_node().label().is_some_and(|label| label.starts_with("Save ") || label.starts_with("Open ")) {
                        assert!(link_rect.is_positive() && rect.expand(0.5).contains_rect(link_rect), "{link_rect:?} in {rect:?}: {at}");
                    }
                }
            }
        }
    }
}

#[test]
fn a_press_on_the_box_saves_nothing_only_save_does() {
    let theme = Theme::new(Mode::Light, TextScale::default());
    let asked = std::rc::Rc::new(std::cell::Cell::new(None));
    let kept = asked.clone();
    let mut harness = staged(
        theme,
        |p| p.surface,
        vec2(640.0, 100.0),
        move |ui, theme| {
            let (_, got) = attachment(ui, theme, &Keymap::default(), NAME, SIZE, FileState::Ready);
            if got.is_some() {
                kept.set(got);
            }
        },
    );
    harness.get_by_label(&spoken(NAME, SIZE, FileState::Ready)).click();
    harness.run();
    assert_eq!(asked.get(), None, "a press on the name saves nothing");
    // A press on the coming-soon `open` falls through to nothing either.
    harness.get_by_label(&format!("Open {NAME}")).click();
    harness.run();
    assert_eq!(asked.get(), None, "a press on the unavailable open saves nothing");
    harness.get_by_label(&format!("Save {NAME}")).click();
    harness.run();
    assert_eq!(asked.get(), Some(FileAsked::Save));
}
