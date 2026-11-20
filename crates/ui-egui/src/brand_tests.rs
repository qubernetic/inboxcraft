// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The brand's marks: each built-in file reads into its box in both looks, the path reader knows what the files hold,
//! the picture is filled where the shapes are, and a file that is not the brand's draws nothing and never panics.

use egui::{Color32, Rect, Vec2, pos2, vec2};

use super::{Fit, Mark, Picture, emblem, image, inside, placed, read};
use crate::theme::Mode;

const MARKS: [Mark; 3] = [Mark::Compact, Mark::Qubernetic, Mark::InboxCraft];

/// The union of the bounds of `picture`'s shapes.
fn reach(picture: &Picture) -> Rect {
    picture.shapes.iter().fold(Rect::NOTHING, |all, shape| all.union(shape.bounds))
}

/// Whether `a` and `b` are within a hundredth of a point of each other, each side.
fn near(a: Rect, b: Rect) -> bool {
    (a.min - b.min).length() < 0.01 && (a.max - b.max).length() < 0.01
}

#[test]
fn every_mark_reads_into_its_box_in_both_looks_in_the_brand_s_colours() {
    for mark in MARKS {
        for (mode, ink) in [(Mode::Dark, Color32::from_rgb(0xD4, 0xD4, 0xD4)), (Mode::Light, Color32::from_rgb(0x19, 0x19, 0x19))] {
            let picture = mark.picture(mode).unwrap_or_else(|| panic!("{mark:?} {mode:?} does not read"));
            assert_eq!(picture.size, mark.size(), "{mark:?}");
            let shapes = picture.shapes.len();
            let reach = reach(&picture);
            match mark {
                Mark::Compact => {
                    assert_eq!(shapes, 2, "the frame and the flap");
                    let frame = picture.shapes.first().unwrap();
                    assert!(
                        near(frame.bounds, Rect::from_min_max(pos2(0.0, 2.62), pos2(21.88, 19.44))),
                        "{mode:?}: the frame at 0, 2.62: {:?}",
                        frame.bounds
                    );
                    assert_eq!(frame.colour, ink, "{mode:?}: the frame in the look's ink");
                    let flap = picture.shapes.get(1).unwrap();
                    assert_eq!(flap.colour, Color32::from_rgb(0x22, 0xA8, 0xE0), "the flap in cyan");
                    // The frames: the flap at 6.32, 6.65, 11.59 × 8.84.
                    let drawn = Rect::from_min_size(pos2(6.32, 6.65), vec2(11.59, 8.84));
                    assert!(
                        (flap.bounds.min - drawn.min).length() < 0.2 && (flap.bounds.size() - drawn.size()).length() < 0.2,
                        "the flap at {:?}",
                        flap.bounds
                    );
                }
                Mark::Qubernetic | Mark::InboxCraft => {
                    assert_eq!(shapes, 10, "{mark:?}: ten letters");
                    assert!(picture.shapes.iter().all(|shape| shape.colour == ink), "{mark:?} {mode:?}: in the look's ink");
                    // The view box fills the box: the letters reach from its left to its right and from its top to its
                    // foot, to within half a point (a curve's lines fall short of its extremes).
                    let size = mark.size();
                    assert!(reach.left() > -0.01 && reach.left() < 0.5, "{mark:?} {mode:?}: from the left: {reach:?}");
                    assert!(reach.right() < size.x + 0.01 && reach.right() > size.x - 0.5, "{mark:?} {mode:?}: to the right: {reach:?}");
                    assert!(reach.top() > -0.01 && reach.top() < 0.5, "{mark:?} {mode:?}: from the top: {reach:?}");
                    assert!(reach.bottom() < size.y + 0.01 && reach.bottom() > size.y - 0.5, "{mark:?} {mode:?}: to the foot: {reach:?}");
                }
            }
        }
    }
}

/// The InboxCraft wordmark's letters stand left to right, each upright: the file's group flips them (`scale(s -s)`)
/// and places each by its own `translate`.
#[test]
fn the_inboxcraft_wordmark_s_letters_stand_in_order_and_upright() {
    let picture = Mark::InboxCraft.picture(Mode::Dark).unwrap();
    let lefts: Vec<f32> = picture.shapes.iter().map(|shape| shape.bounds.left()).collect();
    assert!(lefts.windows(2).all(|pair| pair[0] < pair[1]), "in order: {lefts:?}");
    // The `I` is a dot over a stem: its dot is the higher part, so the stem reaches the foot of the box.
    let i = picture.shapes.first().unwrap();
    assert_eq!(i.outlines.len(), 2, "the I's dot and stem");
    let dot = i.outlines.iter().map(|outline| outline.iter().map(|p| p.y).fold(f32::MAX, f32::min)).fold(f32::MAX, f32::min);
    assert!(dot < 0.5, "the dot at the top: {dot}");
    let stem_foot = i.outlines.iter().flatten().map(|p| p.y).fold(f32::MIN, f32::max);
    assert!(stem_foot > 12.5, "the stem at the foot: {stem_foot}");
    let (dot_outline, stem) = (i.outlines.first().unwrap(), i.outlines.get(1).unwrap());
    let top = |outline: &Vec<egui::Pos2>| outline.iter().map(|p| p.y).fold(f32::MAX, f32::min);
    assert!(top(dot_outline) < top(stem), "the file's first outline is the dot, above the stem");
}

/// Path data as the brand's files write it: letters glued to numbers, H and V, relative commands, a quadratic curve,
/// numbers after M drawing lines.
#[test]
fn the_reader_knows_the_brand_s_path_data() {
    let file = r##"<svg viewBox="0 0 10 10"><path d="M1 1H9V9H1Z" fill="#ff0000"/><path d="m2 2 l2 0 v2 h-2 z" fill="#00ff00"/><path d="M0 10Q5 0 10 10Z" fill="#0000ff"/><polygon points="0,0 1,0 1,1" fill="#ffffff"/></svg>"##;
    let picture = read(file, vec2(10.0, 10.0), Fit::ViewBox).unwrap();
    assert_eq!(picture.shapes.len(), 4);
    let square = picture.shapes.first().unwrap();
    assert_eq!(square.outlines, vec![vec![pos2(1.0, 1.0), pos2(9.0, 1.0), pos2(9.0, 9.0), pos2(1.0, 9.0)]], "H and V");
    assert_eq!(square.colour, Color32::from_rgb(255, 0, 0));
    let relative = picture.shapes.get(1).unwrap();
    assert_eq!(relative.outlines, vec![vec![pos2(2.0, 2.0), pos2(4.0, 2.0), pos2(4.0, 4.0), pos2(2.0, 4.0)]], "relative m, l, v, h");
    // The quadratic curve from 0, 10 over 5, 0 to 10, 10 peaks halfway, at 5, 5.
    let curve = picture.shapes.get(2).unwrap();
    let peak = curve.outlines.iter().flatten().map(|p| p.y).fold(f32::MAX, f32::min);
    assert!((peak - 5.0).abs() < 0.01, "the quadratic's peak at 5: {peak}");
    assert_eq!(picture.shapes.get(3).unwrap().outlines, vec![vec![pos2(0.0, 0.0), pos2(1.0, 0.0), pos2(1.0, 1.0)]], "a polygon");
    let glued = read(r##"<svg viewBox="0 0 4 4"><path d="M0 0L4-0 4 4 0 4z" fill="#000000"/></svg>"##, vec2(4.0, 4.0), Fit::ViewBox).unwrap();
    assert_eq!(glued.shapes.first().unwrap().outlines.first().unwrap().len(), 4, "numbers after L repeat it");
}

/// A group's fill and transform reach its paths, after the path's own transform: `translate(T) scale(S)` takes a
/// point to T + S × point.
#[test]
fn groups_give_their_fill_and_transform() {
    let file = r##"<svg viewBox="0 0 20 20"><g fill="#123456" transform="translate(0 20) scale(2 -2)"><path d="M0 0H1V1H0Z" transform="translate(3 0)"/></g></svg>"##;
    let picture = read(file, vec2(20.0, 20.0), Fit::ViewBox).unwrap();
    let shape = picture.shapes.first().unwrap();
    assert_eq!(shape.colour, Color32::from_rgb(0x12, 0x34, 0x56), "the group's fill");
    assert_eq!(shape.bounds, Rect::from_min_max(pos2(6.0, 18.0), pos2(8.0, 20.0)), "moved by 3, then doubled, flipped and moved down by 20");
}

#[test]
fn the_picture_is_filled_where_its_shapes_are_and_shifted_by_parts_of_a_pixel() {
    let file = r##"<svg viewBox="0 0 4 4"><path d="M0 0H2V4H0Z" fill="#ff0000"/><path d="M1 0H2V4H1Z" fill="#0000ff"/></svg>"##;
    let picture = read(file, vec2(4.0, 4.0), Fit::ViewBox).unwrap();
    let pixels = image(&picture, 1.0, Vec2::ZERO).unwrap();
    assert_eq!(pixels.size, [4, 4]);
    let at = |x: usize, y: usize| pixels.pixels.get(y * pixels.size[0] + x).copied().unwrap();
    assert_eq!(at(0, 0), Color32::from_rgb(255, 0, 0), "the first shape");
    assert_eq!(at(1, 3), Color32::from_rgb(0, 0, 255), "the later shape over the earlier one");
    assert_eq!(at(3, 0), Color32::TRANSPARENT, "nothing past the shapes");
    // At two pixels a point the picture is twice as wide; shifted by half a pixel it takes one more.
    assert_eq!(image(&picture, 2.0, Vec2::ZERO).unwrap().size, [8, 8]);
    let shifted = image(&picture, 1.0, vec2(0.5, 0.0)).unwrap();
    assert_eq!(shifted.size, [5, 4]);
    let half = shifted.pixels.first().copied().unwrap();
    assert_eq!(half.a(), 128, "half the first pixel covered: {half:?}");
    for ppp in [0.0, -1.0, f32::NAN, f32::INFINITY, 0.2, 9.0] {
        assert!(image(&picture, ppp, Vec2::ZERO).is_none(), "{ppp} pixels a point");
    }
    assert!(image(&picture, 1.0, vec2(f32::NAN, 0.0)).is_none(), "a shift that is not a number");
}

#[test]
fn even_odd_leaves_the_holes_open() {
    let ring = vec![
        vec![pos2(0.0, 0.0), pos2(10.0, 0.0), pos2(10.0, 10.0), pos2(0.0, 10.0)],
        vec![pos2(3.0, 3.0), pos2(7.0, 3.0), pos2(7.0, 7.0), pos2(3.0, 7.0)],
    ];
    assert!(inside(&ring, pos2(1.0, 1.0)));
    assert!(!inside(&ring, pos2(5.0, 5.0)), "the hole");
    assert!(!inside(&ring, pos2(11.0, 5.0)));
}

/// A box lands on the whole pixel it starts in, shifted by eighths of a pixel; a shift that rounds to a whole pixel
/// moves to the next.
#[test]
fn a_box_lands_on_whole_pixels_and_eighths() {
    assert_eq!(placed(pos2(17.0, 12.0), 1.0), (pos2(17.0, 12.0), Vec2::ZERO));
    assert_eq!(placed(pos2(18.5, 12.0), 1.0), (pos2(18.0, 12.0), vec2(0.5, 0.0)));
    assert_eq!(placed(pos2(18.5, 12.0), 2.0), (pos2(37.0, 24.0), Vec2::ZERO));
    assert_eq!(placed(pos2(10.99, 3.01), 1.0), (pos2(11.0, 3.0), Vec2::ZERO), "rounded to the next whole pixel");
    assert_eq!(placed(pos2(1.3, 0.0), 1.25), (pos2(1.0, 0.0), vec2(0.625, 0.0)));
}

/// What is not the brand's file reads as nothing, and nothing panics.
#[test]
fn a_file_that_is_not_the_brand_s_draws_nothing() {
    let path = |d: &str| format!(r##"<svg viewBox="0 0 4 4"><path d="{d}" fill="#000000"/></svg>"##);
    let deep = format!("<svg viewBox=\"0 0 4 4\">{}<path d=\"M0 0H1V1Z\" fill=\"#000000\"/></svg>", "<g>".repeat(17));
    let many = format!("<svg viewBox=\"0 0 4 4\">{}</svg>", "<path d=\"M0 0H1V1Z\" fill=\"#000000\"/>".repeat(33));
    let huge = format!("<svg viewBox=\"0 0 4 4\"><path d=\"M0 0{}Z\" fill=\"#000000\"/></svg>", " L1 1".repeat(16_385));
    let files = [
        String::new(),
        "<svg".to_owned(),
        path("M0 0 A1 1 0 0 1 2 2Z"),
        path("M0 0 L1"),
        path("1 2 3"),
        path("M0 0 L1e39 1Z"),
        path("M0 0 LNaN 1Z"),
        path("M0 0 L1 1 Z 3 4"),
        path("M0 0 L\u{202e}1 1Z"),
        r##"<svg viewBox="0 0 4 4"><path d="M0 0H1V1Z" fill="red"/></svg>"##.to_owned(),
        r##"<svg viewBox="0 0 4 4"><path d="M0 0H1V1Z"/></svg>"##.to_owned(),
        r##"<svg viewBox="0 0 4 4"><path d="M0 0H1V1Z" fill="#00000é"/></svg>"##.to_owned(),
        r##"<svg viewBox="0 0 4 4"><g transform="rotate(4)"><path d="M0 0H1V1Z" fill="#000000"/></g></svg>"##.to_owned(),
        r##"<svg viewBox="0 0 4 4"><g transform="scale(1"><path d="M0 0H1V1Z" fill="#000000"/></g></svg>"##.to_owned(),
        r##"<svg viewBox="0 0 0 4"><path d="M0 0H1V1Z" fill="#000000"/></svg>"##.to_owned(),
        r##"<svg><path d="M0 0H1V1Z" fill="#000000"/></svg>"##.to_owned(),
        deep,
        many,
        huge,
    ];
    for file in &files {
        let short: String = file.chars().take(80).collect();
        assert!(read(file, vec2(4.0, 4.0), Fit::ViewBox).is_none(), "{short}");
    }
    let flat = r##"<svg viewBox="0 0 4 4"><path d="M0 0H4" fill="#000000"/></svg>"##;
    assert!(read(flat, vec2(4.0, 4.0), Fit::First(Rect::from_min_size(pos2(0.0, 0.0), vec2(4.0, 4.0)))).is_none(), "nothing to fill");
}

/// The emblem is the frames' 76 × 40 cells, 547 × 500 points.
#[test]
fn the_emblem_is_the_frames_grid() {
    assert_eq!(emblem().grid(), (76, 40));
    assert_eq!(emblem().size(), vec2(547.0, 500.0));
}
