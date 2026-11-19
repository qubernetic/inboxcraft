// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The window at the smallest and the largest text scale: every text fits the control it belongs to, is not cut by
//! the pane it sits in, and lies over no other text.

use egui::accesskit::Role;
use egui::{Rect, Shape, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
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

fn app(scene: Scene, scale: TextScale) -> InboxApp {
    let mut app = InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    app.set_text_scale(scale);
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

/// Every text of the last frame with the clip it was drawn in, groups opened. The scrim of a sheet covers every text
/// drawn before it: those are left out.
fn texts<S>(harness: &Harness<'_, S>) -> Vec<(Rect, Rect, String)> {
    fn open(clip: Rect, shape: &Shape, out: &mut Vec<(Rect, Rect, String)>) {
        match shape {
            Shape::Vec(inner) => inner.iter().for_each(|shape| open(clip, shape, out)),
            Shape::Text(text) if !text.galley.is_empty() => {
                out.push((clip, shape.visual_bounding_rect(), text.galley.text().to_owned()));
            }
            // A plain translucent fill over the whole window, from its corner: a sheet's scrim.
            Shape::Rect(rect)
                if rect.brush.is_none() && rect.rect.min == egui::Pos2::ZERO && rect.rect.width() > 500.0 && (1..255).contains(&rect.fill.a()) =>
            {
                out.clear();
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
/// text cut at the top or bottom of a scrolling pane is only scrolled), or lies over another text. While a dialog (the
/// settings sheet) is open, only what lies in it counts: the window under it is covered on purpose.
pub(super) fn misfits<S>(harness: &Harness<'_, S>) -> Vec<String> {
    misfits_within(harness, Rect::EVERYTHING)
}

/// [`misfits`] among the texts and controls that lie in `area` (a place of the window, the settings: the index beside
/// them has its own checks).
pub(super) fn misfits_within<S>(harness: &Harness<'_, S>, area: Rect) -> Vec<String> {
    let mut found = Vec::new();
    // A dialog covers the controls made before it.
    let nodes: Vec<_> = harness.root().children_recursive().collect();
    let first = nodes.iter().position(|node| node.accesskit_node().role() == Role::Dialog).map_or(0, |index| index + 1);
    let counted = nodes.get(first..).unwrap_or_default();
    let texts = texts(harness);
    let visible: Vec<(Rect, &str)> = texts
        .iter()
        .filter(|(clip, rect, _)| clip.y_range().intersects(rect.y_range()) && area.contains(rect.center()))
        .map(|(clip, rect, text)| {
            if rect.left() < clip.left() - SLACK || rect.right() > clip.right() + SLACK {
                found.push(format!("cut at the side: {text:?} {rect:?} in {clip:?}"));
            }
            (rect.intersect(*clip), text.as_str())
        })
        .filter(|(rect, _)| rect.is_positive())
        .collect();
    for role in [Role::ListBoxOption, Role::Button] {
        for node in counted.iter().filter(|node| node.accesskit_node().role() == role && area.contains(node.rect().center())) {
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
fn the_list_the_reader_and_the_settings_fit_at_90_and_130_percent() {
    for percent in [90, 100, 130] {
        let scale = TextScale::from_percent(percent).unwrap();
        for scene in SCENES {
            let mut harness = Harness::builder()
                .with_size(vec2(1200.0, 760.0))
                .with_theme(egui::Theme::Light)
                .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app(scene, scale));
            harness.run();
            assert_eq!(harness.state().text_scale(), scale);
            let found = misfits(&harness);
            assert!(found.is_empty(), "{scene:?} at {percent} %:\n{}", found.join("\n"));
        }
    }
}

#[test]
fn the_settings_fit_a_narrow_window() {
    for percent in [90, 130] {
        let scale = TextScale::from_percent(percent).unwrap();
        let mut harness = Harness::builder()
            .with_size(vec2(820.0, 760.0))
            .with_theme(egui::Theme::Dark)
            .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app(Scene::Settings, scale));
        harness.run();
        let found = misfits(&harness);
        assert!(found.is_empty(), "narrow at {percent} %:\n{}", found.join("\n"));
    }
}

#[test]
fn the_fit_check_finds_text_that_does_not_fit() {
    let mut harness = Harness::builder().with_size(vec2(400.0, 200.0)).build_ui(|ui| {
        let font = crate::theme::Theme::new(crate::theme::Mode::Light, crate::theme::TextScale::default()).fonts.body.id();
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

/// The roles of the controls a person presses or types into.
const CONTROLS: [Role; 11] = [
    Role::Button,
    Role::TextInput,
    Role::MultilineTextInput,
    Role::PasswordInput,
    Role::ComboBox,
    Role::CheckBox,
    Role::RadioButton,
    Role::Switch,
    Role::Slider,
    Role::Link,
    Role::Tab,
];

/// What is wrong with the controls on the settings: two that lie over each other (the part of each in sight), or one
/// that reaches past the side of the settings. A control inside another (a field's own button) is not two.
fn crowded(harness: &Harness<'_, InboxApp>) -> Vec<String> {
    // The settings in the window: what reaches past its side is cut.
    let settings = harness.get_by_role_and_label(Role::Pane, "Settings").rect().intersect(harness.ctx.content_rect());
    let controls: Vec<(Rect, String)> = harness
        .root()
        .children_recursive()
        .filter(|node| CONTROLS.contains(&node.accesskit_node().role()) && settings.contains(node.rect().center()))
        .map(|node| (node.rect(), format!("{:?} {:?}", node.accesskit_node().role(), node.accesskit_node().label())))
        .collect();
    let mut found = Vec::new();
    for (rect, name) in &controls {
        if rect.left() < settings.left() - SLACK || rect.right() > settings.right() + SLACK {
            found.push(format!("past the side: {name} {rect:?} in {settings:?}"));
        }
    }
    for (index, (a, name_a)) in controls.iter().enumerate() {
        for (b, name_b) in controls.iter().skip(index + 1) {
            let (a, b) = (a.intersect(settings), b.intersect(settings));
            let both = a.intersect(b);
            if both.width() > SLACK && both.height() > SLACK && !a.contains_rect(b) && !b.contains_rect(a) {
                found.push(format!("over each other: {name_a} {a:?} and {name_b} {b:?}"));
            }
        }
    }
    found
}

#[test]
fn every_settings_page_keeps_its_controls_apart_and_on_the_settings_at_every_width() {
    let mut found = Vec::new();
    for percent in [100, 130] {
        let scale = TextScale::from_percent(percent).unwrap();
        for width in [800.0, 1000.0, 1300.0, 1440.0] {
            for section in crate::settings::Section::ALL {
                let mut app = app(Scene::List, scale);
                app.open_settings_page(Some(section.id()));
                let mut harness = Harness::builder()
                    .with_size(vec2(width, 900.0))
                    .with_theme(egui::Theme::Dark)
                    .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
                harness.run();
                found.extend(crowded(&harness).into_iter().map(|line| format!("{section:?} at {width} wide, {percent} %: {line}")));
            }
        }
    }
    assert!(found.is_empty(), "{}", found.join("\n"));
}
