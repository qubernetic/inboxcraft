// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::{Color32, Key, Rect, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::{BannerKind, BannerLink, banner};
use crate::component::tests::{Look, builder, own, painted, stage};
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
            let name = super::spoken(&theme, *kind, text);
            let node = harness.get_by_label(&name);
            let rect = node.rect();
            let paint = painted(&harness, &name);
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

/// The bidi format characters no name may carry: embeddings, overrides and isolates, and the marks.
const BIDI: [char; 12] =
    ['\u{202A}', '\u{202B}', '\u{202C}', '\u{202D}', '\u{202E}', '\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}', '\u{200E}', '\u{200F}', '\u{061C}'];

/// A bounce banner 640 wide saying `text` with one link `link`, on a stage.
fn bounce(text: &'static str, link: &'static str) -> egui_kittest::Harness<'static> {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    // The app draws a box for a character no face has (egui's default); the test harness would panic instead.
    let mut harness = builder().allow_missing_glyphs().with_size(vec2(700.0, 200.0)).build_ui(move |ui| {
        motion::set_reduced(ui.ctx(), true);
        stage(ui, theme.palette.surface, |ui| {
            ui.set_width(640.0);
            banner(ui, &theme, &Keymap::default(), BannerKind::Bounce, text, &[BannerLink { name: link, id: "report" }]);
        });
    });
    harness.run();
    harness
}

/// The characters the words of a banner 640 wide show for `text`, as laid out.
fn shown_words(text: &'static str) -> String {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let mut harness = builder().allow_missing_glyphs().with_size(vec2(700.0, 100.0)).build_ui_state(
        move |ui, shown: &mut String| {
            let galley = super::words(ui, &theme, &crate::component::plain(text), 545.0);
            // What is laid out, in the written order (egui keeps right-to-left glyphs in the order they are drawn).
            *shown = galley.job.text.clone();
        },
        String::new(),
    );
    harness.run();
    harness.state().clone()
}

/// Hostile words: a 2 000 letter word, 10 000 combining marks, control characters, right-to-left text with overrides.
fn hostile_texts() -> Vec<&'static str> {
    let texts = [
        "x".repeat(2000),
        format!("e{}", "\u{301}".repeat(10_000)),
        "550\u{0}\r\n5.1.1 ".repeat(200) + "é\u{200D}👩\u{200D}👩\u{200D}👧",
        "שלום עולם, ".repeat(100) + "\u{202E}gro.elpmaxe@ eve",
        "مرحبا ".repeat(300),
    ];
    texts.into_iter().map(|text| &*Box::leak(text.into_boxed_str())).collect()
}

#[test]
fn hostile_words_keep_the_banner_at_its_sheet_size_cut_once_at_a_character() {
    for text in hostile_texts() {
        let harness = bounce(text, "open the report");
        let name = super::spoken(&Theme::new(Mode::Dark, TextScale::default()), BannerKind::Bounce, text);
        let rect = harness.get_by_label(&name).rect();
        assert_eq!(rect.size(), vec2(640.0, 54.0), "{:?}…: the sheet's box", text.chars().take(12).collect::<String>());
        let paint = painted(&harness, &name);
        assert!(paint.shapes.iter().all(|shape| !shape.overflows), "nothing leaves the banner");
        for shape in &paint.shapes {
            if let Look::Text { text: words, .. } = &shape.look {
                assert!(rect.contains_rect(shape.rect), "{:?} at {:?}", words.chars().take(12).collect::<String>(), shape.rect);
                assert!(!words.contains(['\0', '\r', '\n']), "no control character is drawn");
                assert!(!words.contains(BIDI), "no bidi control is drawn: egui would apply it");
            }
        }
        let shown = shown_words(text);
        assert_eq!(shown.matches('…').count(), 1, "one ellipsis: {:?}", shown.chars().rev().take(12).collect::<String>());
        assert!(shown.ends_with('…'), "it ends the line");
        assert!(
            whole_prefix(&crate::component::plain(text), &shown),
            "cut between grapheme clusters, nothing else changed: {:?}",
            shown.chars().rev().take(12).collect::<String>()
        );
    }
}

#[test]
fn no_bidi_control_reaches_a_name_and_a_long_link_name_is_cut() {
    let link_words: &'static str = Box::leak(format!("always for \u{2066}{}@example.org\u{2069} \u{202E}", "a".repeat(300)).into_boxed_str());
    for text in hostile_texts() {
        let harness = bounce(text, link_words);
        let name = super::spoken(&Theme::new(Mode::Dark, TextScale::default()), BannerKind::Bounce, text);
        let banner = harness.get_by_label(&name);
        let rect = banner.rect();
        let names: Vec<String> = harness.root().children_recursive().filter_map(|node| node.accesskit_node().label()).collect();
        for name in &names {
            assert!(!name.contains(BIDI), "a name carries a bidi control: {:?}", name.chars().take(20).collect::<String>());
        }
        let link_text = link_words;
        let link = harness.query_all_by_role(egui::accesskit::Role::Button).next().unwrap_or_else(|| panic!("no link"));
        assert!(rect.contains_rect(link.rect()), "the link {:?} leaves the banner {rect:?}", link.rect());
        // Its name is the whole address, without the bidi controls; the words drawn are cut.
        let label = link.accesskit_node().label().unwrap_or_default();
        assert_eq!(label, crate::component::spoken(link_text), "the whole name");
    }
}

#[test]
fn a_banner_is_as_high_as_its_lines_at_130_percent() {
    let theme = Theme::new(Mode::Light, TextScale::ALL[4]);
    let harness = harness(theme, 4);
    let rect = harness.get_by_label(&super::spoken(&theme, STATES[4].0, STATES[4].2)).rect();
    assert_eq!(rect.height(), (8.0 + theme.fonts.caption.line + 8.0).ceil(), "whole points: {rect:?}");
}

/// Whether `kept`, without a closing ellipsis, ends at a grapheme boundary of `full` and is its logical prefix.
fn whole_prefix(full: &str, kept: &str) -> bool {
    use unicode_segmentation::UnicodeSegmentation as _;
    let kept = kept.strip_suffix('…').unwrap_or(kept).trim_end();
    full.starts_with(kept) && (kept.is_empty() || full.grapheme_indices(true).any(|(at, grapheme)| at + grapheme.len() == kept.len()))
}

#[test]
fn an_override_in_mail_cannot_reorder_the_words_after_it() {
    let text = "abc \u{202E}def ghi\u{202C} jkl \u{2067}mno\u{2069}";
    assert_eq!(crate::component::plain(text), "abc def ghi jkl mno");
    // Drawn, the glyphs come in the order the words are written.
    assert_eq!(shown_words(Box::leak(text.to_owned().into_boxed_str())), "abc def ghi jkl mno");
}

#[test]
fn right_to_left_names_keep_their_direction_marks() {
    for name in ["\u{200F}דנה כהן (עבודה) 2\u{200F}", "\u{061C}سارة (العمل) 3\u{061C}", "\u{200E}Ines (Work)\u{200E}"] {
        assert_eq!(crate::component::plain(name), name, "the marks stay: {name:?}");
        assert_eq!(crate::component::spoken(name), name, "and in names: {name:?}");
    }
}

/// A banner's link called `name` cut to `room` points: the words drawn.
fn cut_link(name: &'static str, room: f32) -> String {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let mut harness = builder()
        .allow_missing_glyphs()
        .with_size(vec2(700.0, 100.0))
        .build_ui_state(move |ui, cut: &mut String| *cut = super::cut_name(ui, &theme, name, room), String::new());
    harness.run();
    harness.state().clone()
}

#[test]
fn a_cut_keeps_every_grapheme_whole_and_never_mirrors_a_right_to_left_name() {
    let names: [&'static str; 4] = [
        Box::leak(format!("always for {}", "👩\u{200D}👩\u{200D}👧".repeat(60)).into_boxed_str()),
        Box::leak(format!("always for {}", "e\u{301}\u{302}".repeat(200)).into_boxed_str()),
        Box::leak(format!("always for {}", "דנה כהן ".repeat(60)).into_boxed_str()),
        Box::leak(format!("always for {}", "🇭🇺🇳🇴".repeat(80)).into_boxed_str()),
    ];
    for name in names {
        for room in [90.0, 151.0, 230.0] {
            let cut = cut_link(name, room);
            assert!(cut.ends_with('…') && cut.matches('…').count() == 1, "{room}: {cut:?}");
            assert!(whole_prefix(name, &cut), "{room}: {cut:?} is not a whole-grapheme prefix in the written order");
        }
    }
    // The 400 character bound of a banner's words keeps graphemes whole too.
    let text: &'static str = Box::leak(format!("{}{}", "a".repeat(398), "👩\u{200D}👩\u{200D}👧".repeat(4)).into_boxed_str());
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let mut harness = builder()
        .allow_missing_glyphs()
        .with_size(vec2(700.0, 100.0))
        .build_ui_state(move |ui, laid: &mut String| *laid = super::words(ui, &theme, text, 10_000.0).job.text.clone(), String::new());
    harness.run();
    assert!(whole_prefix(text, harness.state()), "{:?}", harness.state().chars().rev().take(8).collect::<String>());
}

#[test]
fn a_cut_links_name_is_heard_whole() {
    let link: &'static str = Box::leak(format!("always for {}@example.org", "a".repeat(300)).into_boxed_str());
    let harness = bounce("Pictures held back.", link);
    let names: Vec<String> = harness.query_all_by_role(egui::accesskit::Role::Button).filter_map(|node| node.accesskit_node().label()).collect();
    assert_eq!(names, vec![link.to_owned()], "the link's name is the whole address");
}

#[test]
fn a_link_name_is_capped_before_it_is_laid_out() {
    // A sender's name of 100 000 characters: what is laid out to fit it is a capped beginning, never the whole.
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let long = "a".repeat(100_000);
    let mut harness = builder().build_ui_state(
        move |ui, shown: &mut String| {
            *shown = super::cut_name(ui, &theme, &long, 1.0e9);
        },
        String::new(),
    );
    harness.run();
    let shown = harness.state();
    assert!(shown.chars().count() <= crate::component::MOST + 1, "{} characters", shown.chars().count());
    assert!(shown.ends_with('…'), "the cap says the rest was left out");
    // In a banner, the link draws a capped beginning and is heard whole.
    let words: &'static str = Box::leak(format!("always for {}@example.org", "b".repeat(100_000)).into_boxed_str());
    let harness = bounce("Not delivered.", words);
    let link = harness.query_all_by_role(egui::accesskit::Role::Button).next().unwrap_or_else(|| panic!("no link"));
    assert_eq!(link.accesskit_node().label().unwrap_or_default(), words, "heard whole");
    let drawn = painted(&harness, words);
    let text = drawn.shapes.iter().find_map(|shape| match &shape.look {
        Look::Text { text, .. } => Some(text.clone()),
        _ => None,
    });
    assert!(text.is_some_and(|text| text.chars().count() <= crate::component::MOST + 1), "the drawn link is capped");
}
