// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The window at the smallest and the largest text scale, in every typeface: every text fits the control it belongs to, is not cut by
//! the pane it sits in, and lies over no other text.

use egui::accesskit::Role;
use egui::{Rect, Shape, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_engine::appearance::Typeface;
use inboxcraft_time::{FakeClock, Timestamp};

use super::InboxApp;
use crate::mailbox::{Mailbox, Place};
use crate::sample;
use crate::theme::TextScale;

/// How far a text may reach past an edge, in points: the glyphs' antialiasing.
const SLACK: f32 = 1.0;

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

/// What the window shows.
#[derive(Debug, Clone, Copy)]
enum Scene {
    /// The unified inbox with a message with attachments open beside it.
    List,
    /// A thread with its quotes shown.
    Thread,
    /// A newsletter with its remote-content banner and the full header.
    Details,
    /// The reader filling the window.
    Full,
    /// The add-account page.
    Onboard,
    /// The appearance settings page.
    Settings,
}

const SCENES: [Scene; 6] = [Scene::List, Scene::Thread, Scene::Details, Scene::Full, Scene::Onboard, Scene::Settings];

fn app(scene: Scene, scale: TextScale, typeface: Typeface) -> InboxApp {
    let mut app = InboxApp::new(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    app.set_text_scale(scale);
    app.set_typeface(typeface);
    let (folder, subject) = match scene {
        Scene::List | Scene::Onboard | Scene::Settings => (None, "Photos from the coast walk"),
        Scene::Thread => (Some(3), "Re: Allotment rota for November"),
        Scene::Details => (None, "Your bike is due for a service"),
        Scene::Full => (None, "Draft logo for the bakery"),
    };
    if let Some(folder) = folder {
        app.mailbox_mut().open(Place::Folder(folder));
    }
    let id = app.mailbox().messages().iter().find(|row| row.subject == subject).unwrap().id;
    app.mailbox_mut().select(Some(id));
    let (reader, _) = app.reader_mut();
    match scene {
        Scene::Thread => reader.toggle_quotes(id),
        Scene::Details => reader.toggle_details(id),
        Scene::Full => reader.set_full_width(true),
        Scene::List | Scene::Onboard | Scene::Settings => {}
    }
    match scene {
        Scene::Onboard => app.onboarding_mut().0.open(),
        Scene::Settings => app.open_settings(),
        _ => {}
    }
    app
}

/// Every text of the last frame with the clip it was drawn in, groups opened.
fn texts<S>(harness: &Harness<'_, S>) -> Vec<(Rect, Rect, String)> {
    fn open(clip: Rect, shape: &Shape, out: &mut Vec<(Rect, Rect, String)>) {
        match shape {
            Shape::Vec(inner) => inner.iter().for_each(|shape| open(clip, shape, out)),
            Shape::Text(text) if !text.galley.is_empty() => {
                out.push((clip, shape.visual_bounding_rect(), text.galley.text().to_owned()));
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    harness.output().shapes.iter().for_each(|clipped| open(clipped.clip_rect, &clipped.shape, &mut out));
    out
}

fn inside(inner: Rect, outer: Rect) -> bool {
    outer.expand(SLACK).contains_rect(inner)
}

/// What does not fit: a text that sticks out of the row or button it belongs to, is cut at the side of its pane (a
/// text cut at the top or bottom of a scrolling pane is only scrolled), or lies over another text.
fn misfits<S>(harness: &Harness<'_, S>) -> Vec<String> {
    let mut found = Vec::new();
    let texts = texts(harness);
    let visible: Vec<(Rect, &str)> = texts
        .iter()
        .filter(|(clip, rect, _)| clip.y_range().intersects(rect.y_range()))
        .map(|(clip, rect, text)| {
            if rect.left() < clip.left() - SLACK || rect.right() > clip.right() + SLACK {
                found.push(format!("cut at the side: {text:?} {rect:?} in {clip:?}"));
            }
            (rect.intersect(*clip), text.as_str())
        })
        .filter(|(rect, _)| rect.is_positive())
        .collect();
    for role in [Role::ListBoxOption, Role::Button] {
        for node in harness.query_all_by_role(role) {
            let control = node.rect();
            for (rect, text) in visible.iter().filter(|(rect, _)| control.contains(rect.center())) {
                if !inside(*rect, control) {
                    found.push(format!("sticks out of {role:?} {:?}: {text:?} {rect:?} in {control:?}", node.accesskit_node().label()));
                }
            }
        }
    }
    for (index, (a, text_a)) in visible.iter().enumerate() {
        for (b, text_b) in visible.iter().skip(index + 1) {
            let both = a.intersect(*b);
            if both.width() > SLACK && both.height() > SLACK {
                found.push(format!("overlap: {text_a:?} {a:?} and {text_b:?} {b:?}"));
            }
        }
    }
    found
}

#[test]
fn the_list_the_reader_and_the_settings_fit_at_90_and_130_percent_in_every_typeface() {
    for typeface in Typeface::ALL {
        for percent in [90, 100, 130] {
            let scale = TextScale::from_percent(percent).unwrap();
            for scene in SCENES {
                let mut harness = Harness::builder()
                    .with_size(vec2(1200.0, 760.0))
                    .with_theme(egui::Theme::Light)
                    .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app(scene, scale, typeface));
                harness.run();
                assert_eq!((harness.state().text_scale(), harness.state().typeface()), (scale, typeface));
                let found = misfits(&harness);
                assert!(found.is_empty(), "{scene:?} in {typeface:?} at {percent} %:\n{}", found.join("\n"));
            }
        }
    }
}

#[test]
fn the_settings_fit_a_narrow_window_with_the_preview_above_the_controls() {
    for percent in [90, 130] {
        let scale = TextScale::from_percent(percent).unwrap();
        let mut harness = Harness::builder()
            .with_size(vec2(820.0, 760.0))
            .with_theme(egui::Theme::Dark)
            .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app(Scene::Settings, scale, Typeface::Inter));
        harness.run();
        let found = misfits(&harness);
        assert!(found.is_empty(), "narrow at {percent} %:\n{}", found.join("\n"));
    }
}

#[test]
fn the_fit_check_finds_text_that_does_not_fit() {
    let mut harness = Harness::builder().with_size(vec2(400.0, 200.0)).build_ui(|ui| {
        let font = crate::theme::Theme::new(crate::theme::Mode::Light, &crate::theme::Preset::default()).fonts.body.id();
        let painter = ui.painter();
        let colour = ui.visuals().text_color();
        painter.text(egui::pos2(10.0, 10.0), egui::Align2::LEFT_TOP, "one text", font.clone(), colour);
        painter.text(egui::pos2(14.0, 12.0), egui::Align2::LEFT_TOP, "over another", font.clone(), colour);
        painter.text(egui::pos2(380.0, 100.0), egui::Align2::LEFT_TOP, "past the side", font, colour);
    });
    harness.run();
    let found = misfits(&harness);
    assert!(found.iter().any(|line| line.starts_with("overlap")), "{found:?}");
    assert!(found.iter().any(|line| line.starts_with("cut at the side")), "{found:?}");
}
