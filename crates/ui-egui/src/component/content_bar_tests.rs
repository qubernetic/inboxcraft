// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::accesskit::Toggled;
use egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::{BarKind, ContentBar, View, content_bar, spoken};
use crate::component::banner::BannerLink;
use crate::component::tests::{Look, own, painted, staged};
use crate::keymap::Keymap;
use crate::theme::{Mode, TextScale, Theme};

/// A state of the sheet: its name, kind, view, words, whether they wait, and its links' words.
type State = (&'static str, BarKind, View, &'static str, bool, &'static [&'static str]);

const STATES: [State; 7] = [
    ("text", BarKind::Text, View::Fitted, "plain text, shown as sent", false, &[]),
    ("html", BarKind::Html, View::Fitted, "nothing held back", false, &[]),
    ("list", BarKind::List, View::Fitted, "craft-dev@lists.example.org", false, &["unsubscribe"]),
    ("invite", BarKind::Invite, View::Fitted, "read into the card below", false, &[]),
    ("pictures-held", BarKind::Html, View::Fitted, "6 pictures held back", true, &["load once", "always"]),
    ("trackers-removed", BarKind::Html, View::Fitted, "2 trackers removed · 6 pictures loaded once", false, &[]),
    ("plain-text", BarKind::Html, View::Plain, "as a text-only program shows it", false, &[]),
];

/// The bar 720 wide on the reader's `surface`, in `state`.
fn harness(theme: Theme, state: State, views: bool) -> Harness<'static> {
    let (_, kind, view, held, waiting, words) = state;
    staged(
        theme,
        |p| p.surface,
        vec2(720.0 + 48.0, 100.0),
        move |ui, theme| {
            ui.set_width(720.0);
            let links: Vec<BannerLink<'_>> = words.iter().map(|name| BannerLink { name, id: name }).collect();
            let bar = ContentBar { kind, view, views, held, waiting, links: &links };
            content_bar(ui, theme, &Keymap::default(), &bar);
        },
    )
}

/// The coming-soon tag of a bar whose choice of view works: once at most, 12 before the browser's link and apart from
/// the `group` of the choice; there for sure where `room` says the words leave it room.
fn tag_beside_browser(harness: &Harness<'static>, room: bool, group: egui::Rect, browser: egui::Rect, at: &str) -> Option<egui::Rect> {
    let tags: Vec<egui::Rect> = harness.query_all_by_label("coming soon").map(|tag| tag.rect()).collect();
    assert!(tags.len() <= 1, "{at}");
    let tag = tags.first().copied();
    assert!(!room || tag.is_some(), "where the words leave room the tag stands: {at}");
    if let Some(tag) = tag {
        assert_eq!(tag.right(), browser.left() - 12.0, "the tag beside the browser's link: {at}");
        assert!(tag.left() > group.right() + 12.0, "the tag apart from the choice of view: {at}");
    }
    tag
}

#[test]
fn the_content_bar_looks_as_its_sheet_says_in_every_state() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let p = theme.palette;
        let f = theme.fonts;
        for state in STATES {
            let (name, kind, view, held, waiting, words) = state;
            let harness = harness(theme, state, true);
            let label = spoken(kind, held);
            let rect = harness.get_by_label(&label).rect();
            let paint = painted(&harness, &label);
            let at = format!("{mode:?} {name}: {paint:#?}");
            assert_eq!(rect.size(), vec2(720.0, 40.0), "{at}");
            let look = own(&paint, rect);
            assert_eq!((look.fill, look.edge, look.radius), (Some(p.panel), Some((1.0, p.panel_line, false)), 0.0), "{at}");
            let text = |start: &str| {
                paint.shapes.iter().find_map(|shape| match &shape.look {
                    Look::Text { text, colour, size, .. } if text.starts_with(start) => Some((shape.rect, *colour, *size)),
                    _ => None,
                })
            };
            let tag = ["TEXT", "HTML", "LIST", "INVITE"][kind as usize];
            let (tag_rect, tag_ink, _) = text(tag).unwrap_or_else(|| panic!("the tag: {at}"));
            assert_eq!((tag_rect.left() - rect.left(), tag_ink), (18.0, p.cyan_text), "the tag's words 6 inside its box at 12: {at}");
            let (words_rect, ink, size) = text(&held[..8]).unwrap_or_else(|| panic!("the words: {at}"));
            assert_eq!((ink, size), (if waiting { p.warn } else { p.muted }, f.mono_s.size), "{at}");
            if kind == BarKind::Text {
                assert_eq!(words_rect.left() - rect.left(), 63.0, "the words follow the tag: {at}");
                assert_eq!(harness.query_all_by_label("open in browser ↗").count(), 0, "{at}");
                assert_eq!(harness.query_all_by_role(egui::accesskit::Role::RadioGroup).count(), 0, "{at}");
                continue;
            }
            // The choice of view, 176 wide at 63, the chosen one selected.
            let (chosen, other) = if view == View::Fitted { ("fitted", "plain text") } else { ("plain text", "fitted") };
            assert_eq!(harness.get_by_label(chosen).accesskit_node().toggled(), Some(Toggled::True), "{at}");
            assert_eq!(harness.get_by_label(other).accesskit_node().toggled(), Some(Toggled::False), "{at}");
            let group = harness.get_by_role(egui::accesskit::Role::RadioGroup).rect();
            // 12 after the tag (39 wide, INVITE 52).
            let after_tag = if kind == BarKind::Invite { 76.0 } else { 63.0 };
            assert_eq!((group.left() - rect.left(), group.width(), group.height()), (after_tag, 176.0, 32.0), "{at}");
            // The browser's copy is not built: its link at the right end where the sheet puts it (595), unavailable.
            let browser = harness.get_by_label("open in browser ↗");
            assert!(browser.accesskit_node().is_disabled(), "{at}");
            assert_eq!(browser.rect().right(), rect.right() - 12.0, "{at}");
            assert_eq!(browser.rect().left() - rect.left(), 595.0, "{at}");
            // The choice of view works: the coming-soon tag stands once at most, 12 before the browser's link, the one
            // thing here not built yet, where the words stay whole; never beside the choice.
            let tag = tag_beside_browser(&harness, name == "html", group, browser.rect(), &at);
            // The links stand 12 before the tag (or the browser's link); the words 12 before the links, right-aligned,
            // as the sheet has them (the words end at 583, or 12 before the links), whole.
            let before = tag.map_or(browser.rect().left(), |tag| tag.left());
            let mut end = before - 12.0;
            for word in words.iter().rev() {
                let link = harness.get_by_label(word).rect();
                assert_eq!(link.right(), end, "{word}: {at}");
                assert!(!harness.get_by_label(word).accesskit_node().is_disabled(), "{at}");
                end = link.left() - 10.0;
            }
            let words_end = if words.is_empty() { before - 12.0 } else { end + 10.0 - 12.0 };
            let cut = paint.shapes.iter().any(|shape| matches!(&shape.look, Look::Text { text, .. } if text.ends_with('…')));
            assert!(!cut, "the words whole: {at}");
            assert!((words_rect.right() - words_end).abs() <= 1.0, "the words end at {words_end}: {at}");
        }
    }
}

#[test]
fn the_choice_of_view_stands_unavailable_until_the_view_is_built() {
    let theme = Theme::new(Mode::Light, TextScale::default());
    let harness = harness(theme, STATES[1], false);
    assert!(harness.get_by_role(egui::accesskit::Role::RadioGroup).accesskit_node().is_disabled());
    assert_eq!(harness.query_all_by_label("coming soon").count(), 1, "the tag once for the bar");
    // Unavailable, the choice has the tag 12 after it.
    let group = harness.get_by_role(egui::accesskit::Role::RadioGroup).rect();
    assert_eq!(harness.get_by_label("coming soon").rect().left(), group.right() + 12.0);
}

#[test]
fn the_content_bar_is_as_high_as_the_scaled_size_at_130_percent() {
    let theme = Theme::new(Mode::Dark, TextScale::ALL[4]);
    let state = STATES[4];
    let harness = harness(theme, state, true);
    let rect = harness.get_by_label(&spoken(state.1, state.3)).rect();
    assert_eq!(rect.height(), theme.size(40.0));
}

#[test]
fn a_hostile_list_address_stays_inside_the_bar_and_is_heard_whole() {
    let theme = Theme::new(Mode::Dark, TextScale::ALL[4]);
    let held: &'static str = Box::leak(format!("\u{202E}{}@lists.example.org", "x".repeat(500)).into_boxed_str());
    let harness = harness(theme, ("list", BarKind::List, View::Fitted, held, false, &["unsubscribe"]), true);
    let label = spoken(BarKind::List, held);
    assert!(!label.contains('\u{202E}'));
    let rect = harness.get_by_label(&label).rect();
    let paint = painted(&harness, &label);
    let run = paint.shapes.iter().find(|shape| matches!(&shape.look, Look::Text { text, .. } if text.starts_with('x')));
    let run = run.unwrap_or_else(|| panic!("{paint:#?}"));
    assert!(matches!(&run.look, Look::Text { text, .. } if text.ends_with('…')), "{run:?}");
    assert!(run.rect.right() < harness.get_by_label("unsubscribe").rect().left() && run.rect.left() > rect.left(), "{run:?}");
}

#[test]
fn in_a_narrow_bar_the_browser_s_link_gives_way_and_nothing_lies_over_another() {
    let theme = Theme::new(Mode::Light, TextScale::default());
    let harness = staged(
        theme,
        |p| p.surface,
        vec2(420.0 + 48.0, 100.0),
        move |ui, theme| {
            ui.set_width(420.0);
            let bar = ContentBar { kind: BarKind::Html, view: View::Fitted, views: false, held: "nothing held back", waiting: false, links: &[] };
            content_bar(ui, theme, &Keymap::default(), &bar);
        },
    );
    assert_eq!(harness.query_all_by_label("open in browser ↗").count(), 0);
    let group = harness.get_by_role(egui::accesskit::Role::RadioGroup).rect();
    let label = spoken(BarKind::Html, "nothing held back");
    let paint = painted(&harness, &label);
    let words = paint.shapes.iter().find(|shape| matches!(&shape.look, Look::Text { text, .. } if text.starts_with("nothing")));
    let words = words.unwrap_or_else(|| panic!("{paint:#?}"));
    assert!(words.rect.left() >= group.right() + 12.0 - 1.0, "{words:?} {group:?}");
}
