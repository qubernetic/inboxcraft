// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::{Color32, Key, Rect, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

use super::{BannerKind, BannerLink, banner};
use crate::component::tests::{Look, builder, own, painted, stage, staged};
use crate::keymap::Keymap;
use crate::motion;
use crate::theme::{Mode, Palette, TextScale, Theme};

/// A state of the sheet: kind, label, text, links (words and id) and height.
type State = (BannerKind, &'static str, &'static str, &'static [(&'static str, &'static str)], f32);

/// The sheet's six states.
const STATES: [State; 6] = [
    (
        BannerKind::RemotePictures,
        "PICTURES",
        "3 remote pictures are not shown. Loading them tells the sender that you opened this message.",
        &[("load once", "pictures.once"), ("always for news@brightshelf.example.org", "pictures.always")],
        54.0,
    ),
    (
        BannerKind::Bounce,
        "NOT DELIVERED",
        "This message did not arrive: 550 5.1.1 The address ansel@example.org does not exist.",
        &[("open the report", "bounce.report"), ("edit and send again", "bounce.again")],
        54.0,
    ),
    (
        BannerKind::Error,
        "ERROR",
        "The message could not be moved: the server refused (NO [CANNOT]).",
        &[("try again", "error.again"), ("dismiss", "error.dismiss")],
        54.0,
    ),
    (
        BannerKind::Failing,
        "ERROR",
        "Studio refused the sign-in, and one message was not sent.",
        &[("open account", "failing.account"), ("open outbox", "failing.outbox")],
        54.0,
    ),
    (BannerKind::Notice, "NOTE", "The text is cut here: the rest is longer than the reader shows.", &[], 34.0),
    (BannerKind::Careful, "CAREFUL", "The text says “example.org”, but the link goes to example.net.", &[], 34.0),
];

/// The banner's own colours of `kind`: its edge and its label.
fn colours(p: &Palette, kind: BannerKind) -> (Color32, Color32) {
    match kind {
        BannerKind::RemotePictures => (p.line, p.warn),
        BannerKind::Notice => (p.panel_line, p.muted),
        BannerKind::Bounce | BannerKind::Error | BannerKind::Failing | BannerKind::Careful => (p.minus, p.minus),
    }
}

/// A harness with one banner 640 wide on the reader's `surface`; the state keeps the ids pressed.
fn harness(theme: Theme, index: usize) -> Harness<'static, Vec<String>> {
    let (kind, _, text, links, _) = STATES[index];
    let mut harness = builder().with_size(vec2(700.0, 160.0)).build_ui_state(
        move |ui, pressed: &mut Vec<String>| {
            motion::set_reduced(ui.ctx(), true);
            stage(ui, theme.palette.surface, |ui| {
                ui.set_width(640.0);
                let links: Vec<BannerLink<'_>> = links.iter().map(|(name, id)| BannerLink { name, id }).collect();
                pressed.extend(banner(ui, &theme, &Keymap::default(), kind, text, &links));
            });
        },
        Vec::new(),
    );
    harness.run();
    harness
}

#[test]
fn a_banner_looks_as_its_sheet_says_in_every_state() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let p = theme.palette;
        for (index, (kind, label, text, links, height)) in STATES.iter().enumerate() {
            let harness = harness(theme, index);
            let node = harness.get_by_label(text);
            let rect = node.rect();
            let paint = painted(&harness, text);
            let at = format!("{mode:?} {kind:?}: {paint:#?}");
            assert_eq!(rect.size(), vec2(640.0, *height), "{at}");
            let look = own(&paint, rect);
            let (edge, ink) = colours(p, *kind);
            assert_eq!(look.fill, Some(p.panel), "{at}");
            assert_eq!(look.edge, Some((1.0, edge, false)), "{at}");
            assert!(look.radius <= 0.0, "{at}");
            assert!(!look.overflows, "{at}");
            let texts: Vec<(Rect, String, Color32, String, f32)> = paint
                .shapes
                .iter()
                .filter_map(|shape| match &shape.look {
                    Look::Text { text, colour, family, size, .. } => Some((shape.rect, text.clone(), *colour, family.clone(), *size)),
                    _ => None,
                })
                .collect();
            let (mono, sans) = (theme.fonts.label.id().family.to_string(), theme.fonts.caption.id().family.to_string());
            let mut wanted = vec![(label.to_string(), ink, mono.clone(), 11.0), (text.to_string(), p.ink, sans, 12.0)];
            wanted.extend(links.iter().map(|(name, _)| (name.to_string(), p.cyan_text, mono.clone(), 11.0)));
            let got: Vec<_> = texts.iter().map(|(_, text, colour, family, size)| (text.clone(), *colour, family.clone(), *size)).collect();
            assert_eq!(got, wanted, "{at}");
            // The sheet's places: the label at 12,9; the text 12 right of the label at 8; the links at 30, 16 apart.
            let label_rect = texts.first().map_or(Rect::NOTHING, |text| text.0);
            assert_eq!(label_rect.min - rect.min, vec2(12.0, 9.0), "{at}");
            let text_rect = texts.get(1).map_or(Rect::NOTHING, |text| text.0);
            assert_eq!(text_rect.min - rect.min, vec2(12.0 + label_rect.width().ceil() + 12.0, 8.0), "{at}");
            assert_eq!(text_rect.height(), 18.0, "one line: {at}");
            let mut x = text_rect.left();
            for (link, ..) in texts.iter().skip(2) {
                assert_eq!((link.left(), link.top() - rect.top()), (x, 30.0), "{at}");
                x = link.left() + link.width().ceil() + 16.0;
            }
        }
    }
}

#[test]
fn a_banner_link_gives_its_id_on_a_click_and_on_enter_and_on_space() {
    let theme = Theme::new(Mode::Light, TextScale::default());
    let mut harness = harness(theme, 1);
    harness.get_by_label("edit and send again").click();
    harness.run();
    assert_eq!(harness.state(), &["bounce.again".to_owned()]);
    for key in [Key::Enter, Key::Space] {
        harness.state_mut().clear();
        harness.get_by_label("open the report").focus();
        harness.run();
        harness.key_press(key);
        harness.run();
        assert_eq!(harness.state(), &["bounce.report".to_owned()], "{key:?}");
    }
}

#[test]
fn a_long_or_hostile_text_wraps_inside_the_banner_and_ends_with_one_ellipsis() {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let hostile = "550\u{0}\r\n5.1.1 ".repeat(200) + "é‍👩‍👩‍👧";
    let text: &'static str = Box::leak(hostile.into_boxed_str());
    let harness = staged(
        theme,
        |p| p.surface,
        vec2(700.0, 300.0),
        move |ui, theme| {
            ui.set_width(640.0);
            banner(ui, theme, &Keymap::default(), BannerKind::Bounce, text, &[BannerLink { name: "open the report", id: "r" }]);
        },
    );
    let node = harness.get_by_label(text);
    let rect = node.rect();
    let paint = painted(&harness, text);
    let body = paint.shapes.iter().find_map(|shape| match &shape.look {
        Look::Text { text: words, .. } if words.contains("5.1.1") => Some((shape.rect, words.clone())),
        _ => None,
    });
    let (body, words) = body.unwrap_or((Rect::NOTHING, String::new()));
    assert!(rect.contains_rect(body), "{rect:?} {body:?}");
    assert!(body.height() <= 3.0 * 18.0 + 0.01, "at most three lines: {body:?}");
    assert!(!words.contains(['\0', '\r', '\n']), "no control characters are drawn");
    assert_eq!(rect.height(), (8.0 + body.height() + 4.0 + 16.0 + 8.0).ceil());
    harness.get_by_label("open the report");
}

#[test]
fn a_banner_is_as_high_as_its_lines_at_130_percent() {
    let theme = Theme::new(Mode::Light, TextScale::ALL[4]);
    let harness = harness(theme, 4);
    let rect = harness.get_by_label(STATES[4].2).rect();
    assert_eq!(rect.height(), (8.0 + theme.fonts.caption.line + 8.0).ceil(), "whole points: {rect:?}");
}
