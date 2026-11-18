// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::{Rect, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

use super::{PictureAsked, PictureLook, picture_box, shown_size, spoken};
use crate::component::tests::{builder, stage};
use crate::keymap::Keymap;
use crate::motion;
use crate::theme::{Mode, TextScale, Theme};

const KILN: &str = "The new kiln, lit for the first time";

/// A box `room` wide on the reader's `surface`; the state keeps what was asked.
fn boxed(look: PictureLook<'static>, alt: &'static str, size: Option<(u32, u32)>, room: f32) -> Harness<'static, Vec<PictureAsked>> {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let mut harness = builder().allow_missing_glyphs().with_size(vec2(room + 60.0, 640.0)).build_ui_state(
        move |ui, asked: &mut Vec<PictureAsked>| {
            motion::set_reduced(ui.ctx(), true);
            stage(ui, theme.palette.surface, |ui| {
                ui.set_width(room);
                asked.extend(picture_box(ui, &theme, &Keymap::default(), look, alt, size).1);
            });
        },
        Vec::new(),
    );
    harness.run();
    harness
}

/// Every text the last frame painted: its rect, its words and the rect it was clipped to.
fn texts(harness: &Harness<'static, Vec<PictureAsked>>) -> Vec<(Rect, String, Rect)> {
    fn walk(shape: &egui::Shape, clip: Rect, found: &mut Vec<(Rect, String, Rect)>) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, clip, found)),
            egui::Shape::Text(text) => found.push((Rect::from_min_size(text.pos, text.galley.size()), text.galley.job.text.clone(), clip)),
            _ => {}
        }
    }
    let mut found = Vec::new();
    harness.output().shapes.iter().for_each(|clipped| walk(&clipped.shape, clipped.clip_rect, &mut found));
    found
}

#[test]
fn a_remote_picture_not_loaded_is_a_box_of_its_real_size() {
    let harness = boxed(PictureLook::NotLoaded, KILN, Some((600, 240)), 684.0);
    let label = spoken(PictureLook::NotLoaded, KILN, Some((600, 240)));
    assert_eq!(label, format!("Remote picture not loaded: {KILN}, 600 × 240"));
    let rect = harness.get_by_label(&label).rect();
    assert_eq!(rect.size(), vec2(600.0, 240.0), "the frames' 600 × 240");
    let said: Vec<String> = texts(&harness).into_iter().map(|(_, text, _)| text).collect();
    for words in ["Remote picture not loaded", KILN, "600 × 240"] {
        assert!(said.iter().any(|text| text == words), "{words}: {said:?}");
    }
    // In a narrower room it keeps its shape, as the picture would.
    let harness = boxed(PictureLook::NotLoaded, KILN, Some((600, 240)), 300.0);
    assert_eq!(harness.get_by_label(&label).rect().size(), vec2(300.0, 120.0));
}

#[test]
fn every_look_says_why_and_keeps_inside_its_box() {
    for (look, words) in [
        (PictureLook::NotLoaded, "Remote picture not loaded"),
        (PictureLook::NotShown, "Picture not shown: InboxCraft does not draw this kind of picture"),
        (PictureLook::Failed("no connection"), "Picture could not be shown: no connection"),
        (PictureLook::Loading, "loading picture"),
    ] {
        for (size, room) in [(Some((600, 240)), 684.0), (None, 684.0), (Some((600, 240)), 260.0), (None, 200.0)] {
            let harness = boxed(look, KILN, size, room);
            let rect = harness.get_by_label(&spoken(look, KILN, size)).rect();
            let at = format!("{look:?} {size:?} {room}");
            assert!(rect.width() <= room + 0.5, "{at}: in the room: {rect:?}");
            let painted = texts(&harness);
            assert!(painted.iter().any(|(_, text, _)| text == words || text.starts_with(&words[..words.len().min(12)])), "{at}: {painted:?}");
            for (text_rect, text, clip) in painted.iter().filter(|(_, text, _)| !text.is_empty()) {
                assert!(rect.expand(0.5).contains_rect(text_rect.intersect(*clip)), "{at}: {text:?} at {text_rect:?} past {rect:?}");
            }
        }
    }
}

#[test]
fn a_one_pixel_picture_is_a_one_pixel_box_that_says_nothing() {
    let harness = boxed(PictureLook::NotLoaded, "", Some((1, 1)), 684.0);
    let rect = harness.get_by_label(&spoken(PictureLook::NotLoaded, "", Some((1, 1)))).rect();
    assert_eq!(rect.size(), vec2(1.0, 1.0));
    assert!(texts(&harness).is_empty(), "no words past a one-pixel box");
}

#[test]
fn try_again_is_the_owners_press() {
    let mut harness = boxed(PictureLook::Failed("no connection"), KILN, Some((600, 240)), 684.0);
    assert!(harness.state().is_empty(), "the box asks for nothing by itself");
    harness.get_by_label("try again").click();
    harness.run();
    assert_eq!(harness.state(), &vec![PictureAsked::Retry]);
}

#[test]
fn the_senders_text_is_drawn_flat_and_heard_whole() {
    const HOSTILE: &str = "Kiln\r\n\u{202e}gnp.exe\u{0}\u{7} and a very long text that goes on and on past the box's width and further still";
    let harness = boxed(PictureLook::NotShown, HOSTILE, Some((300, 200)), 684.0);
    for (_, text, _) in texts(&harness) {
        assert!(!text.contains(['\r', '\n', '\u{202e}', '\u{0}', '\u{7}']), "{text:?}");
    }
    let heard = spoken(PictureLook::NotShown, HOSTILE, Some((300, 200)));
    assert!(heard.contains("and further still") && !heard.contains(['\r', '\n', '\u{202e}']), "{heard:?}");
}

#[test]
fn any_size_from_a_tag_stands_in_the_room() {
    for size in [(0, 0), (1, u32::MAX), (u32::MAX, 1), (u32::MAX, u32::MAX), (4096, 4096)] {
        for room in [0.0, 1.0, 684.0, f32::NAN, f32::INFINITY, -5.0] {
            let shown = shown_size(size, room);
            assert!(shown.x >= 1.0 && shown.y >= 1.0 && shown.y <= super::MOST_HIGH, "{size:?} {room}: {shown:?}");
            assert!(shown.x <= room.max(1.0) || !room.is_finite(), "{size:?} {room}: {shown:?}");
        }
    }
}

/// A failed picture narrower than the room keeps every word inside its box, and `try again` stands inside it; in a box
/// too narrow for the link whole, the link is not offered.
#[test]
fn a_narrow_failed_picture_keeps_its_parts_inside_the_box() {
    let look = PictureLook::Failed("no connection");
    for (size, link) in [((140, 200), true), ((60, 200), false)] {
        let harness = boxed(look, KILN, Some(size), 684.0);
        let rect = harness.get_by_label(&spoken(look, KILN, Some(size))).rect();
        for (text_rect, text, _) in texts(&harness) {
            assert!(rect.expand(0.5).contains_rect(text_rect), "{size:?}: {text:?} at {text_rect:?} paints outside the box {rect:?}");
        }
        match harness.query_by_label("try again") {
            Some(node) => {
                assert!(link, "{size:?}: a link too wide for the box is offered");
                assert!(rect.contains_rect(node.rect()), "{size:?}: try again at {:?} outside the box {rect:?}", node.rect());
            }
            None => assert!(!link, "{size:?}: try again is missing"),
        }
    }
}
