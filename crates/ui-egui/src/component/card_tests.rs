// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::accesskit::{Role, Toggled};
use egui::{Context, Id, RawInput, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::card;
use crate::component::tests::{builder, theme};
use crate::motion::{self, tests::FRAME};

/// Two cards whose open state is the harness state; a click on a header opens or closes it.
fn harness() -> Harness<'static, [bool; 2]> {
    let theme = theme();
    let mut harness = builder().build_ui_state(
        move |ui, open: &mut [bool; 2]| {
            for (index, title) in ["Newsletters", "Screener"].into_iter().enumerate() {
                let shown = card(ui, &theme, Id::unique(title), title, open[index], |ui| ui.label(format!("{title} body")));
                if shown.header.clicked() {
                    open[index] = !open[index];
                }
            }
        },
        [true, false],
    );
    harness.run();
    harness
}

#[test]
fn an_open_card_shows_its_body_and_a_closed_one_only_its_header() {
    let harness = harness();
    harness.get_by_label("Newsletters body");
    assert!(harness.query_by_label("Screener body").is_none());
    let header = harness.get_by_label("Screener");
    assert_eq!(header.accesskit_node().role(), Role::Button);
    assert_eq!(header.accesskit_node().toggled(), Some(Toggled::False));
    assert_eq!(harness.get_by_label("Newsletters").accesskit_node().toggled(), Some(Toggled::True));
}

#[test]
fn a_click_on_the_header_opens_and_closes_the_card() {
    let mut harness = harness();
    harness.get_by_label("Screener").click();
    harness.run();
    assert_eq!(*harness.state(), [true, true]);
    harness.get_by_label("Screener body");
    harness.get_by_label("Newsletters").click();
    harness.run();
    assert!(harness.query_by_label("Newsletters body").is_none(), "closed when the motion has run");
}

/// How far a card that opens at the second frame is open, frame by frame, and its height.
fn opening(reduce: bool) -> Vec<(f32, f32)> {
    let ctx = Context::default();
    let theme = theme();
    (0..30)
        .map(|n| {
            let mut out = (0.0, 0.0);
            let input = RawInput {
                time: Some(f64::from(n) * FRAME),
                predicted_dt: FRAME as f32,
                screen_rect: Some(egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 400.0))),
                ..RawInput::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                motion::set_reduced(ui.ctx(), reduce);
                let shown = card(ui, &theme, Id::unique("card"), "Card", n > 0, |ui| {
                    ui.label("One");
                    ui.label("Two");
                    ui.label("Three");
                });
                out = (shown.openness, shown.rect.height());
            });
            output.textures_delta.clear();
            out
        })
        .collect()
}

#[test]
fn a_card_unrolls_over_the_card_token() {
    let frames = opening(false);
    let (closed, open) = (frames[0], frames[29]);
    assert_eq!(closed.0, 0.0);
    assert_eq!(open.0, 1.0);
    assert!(open.1 > closed.1 + 40.0, "{frames:?}");
    assert!(frames.iter().any(|(openness, height)| *openness > 0.0 && *openness < 1.0 && *height > closed.1 && *height < open.1), "{frames:?}");
}

#[test]
fn with_reduced_motion_a_card_opens_at_once() {
    let frames = opening(true);
    assert_eq!(frames[1].0, 1.0);
    assert!(frames.iter().skip(2).all(|frame| *frame == frames[2]), "{frames:?}");
}
