// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A fitted HTML message: its links, paragraphs, quotes and tables, a message far larger than the pane, and the content
//! bar's choice between `fitted` and `plain text`.

use egui::accesskit::Role;
use egui::{Key, Rect, Shape, UiBuilder, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_html::{Block, Cell, Document, Span, TextKind};
use inboxcraft_time::{FakeClock, Timestamp};

use super::InboxApp;
use super::reader::Asked;
use crate::mailbox::Mailbox;
use crate::reader::tests::Recorder;
use crate::sample;
use crate::save::NoSave;
use crate::theme::{Mode, Palette, TextScale, Theme};

/// What a fitted message asked for in its last frames, and the status the link under the pointer or the focus gave.
#[derive(Default)]
struct Seen {
    asked: Vec<Asked>,
    hover: Option<String>,
}

/// `document` drawn by the reader's blocks in a pane `width` × `high` that scrolls, its quotes shown or not.
fn fitted(document: Document, quotes: bool, width: f32, high: f32) -> Harness<'static, Seen> {
    let theme = Theme::new(Mode::Light, TextScale::default());
    let mut reader = crate::reader::Reader::new(Box::<crate::fetch::Offline>::default(), Box::new(NoSave));
    if quotes {
        reader.toggle_quotes(1);
    }
    let mut harness = Harness::builder().with_size(vec2(width, high)).with_context_setup(crate::theme::install_fonts).build_ui_state(
        move |ui, seen: &mut Seen| {
            let room = Rect::from_min_size(pos2(0.0, 0.0), vec2(width, high));
            ui.scope_builder(UiBuilder::new().max_rect(room), |ui| {
                let area = egui::ScrollArea::vertical().auto_shrink([false, false]);
                crate::component::scroll::show(ui, &theme, area, |ui| {
                    let mut textures = super::reader::Textures::default();
                    let mut drawn = super::reader::Drawn::default();
                    super::reader::blocks(ui, &theme, &reader, 1, &document, &document.blocks, 0, &mut textures, &mut drawn);
                    seen.asked.extend(drawn.asked);
                    seen.hover = drawn.hover;
                });
            });
        },
        Seen::default(),
    );
    harness.run();
    harness
}

/// `html` as the reader's cleaner gives it.
fn display(html: &str) -> Document {
    inboxcraft_html::display(html, &inboxcraft_html::Options::default())
}

/// The label of the node with the focus, if any.
fn focused(harness: &Harness<'static, Seen>) -> Option<(Role, String)> {
    harness
        .root()
        .children_recursive()
        .find(|node| node.accesskit_node().is_focused())
        .map(|node| (node.accesskit_node().role(), node.accesskit_node().label().unwrap_or_default()))
}

/// Every shape painted in the last frame, nested ones taken apart.
fn leaves(harness: &Harness<'static, Seen>) -> Vec<Shape> {
    fn walk(shape: &Shape, out: &mut Vec<Shape>) {
        match shape {
            Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, out)),
            other => out.push(other.clone()),
        }
    }
    let mut out = Vec::new();
    harness.output().shapes.iter().for_each(|clipped| walk(&clipped.shape, &mut out));
    out
}

#[test]
fn a_link_is_reached_by_tab_and_enter_copies_it() {
    let html = "<p>Read <a href=\"https://notes.example.org/a\">the notes</a> first, then \
                <a href=\"mailto:mira@example.org\">write to Mira</a>.</p>";
    let mut harness = fitted(display(html), false, 640.0, 400.0);
    assert!(focused(&harness).is_none());
    harness.key_press(Key::Tab);
    harness.run();
    assert_eq!(focused(&harness), Some((Role::Link, "the notes".to_owned())));
    // The focus shows: a ring round the link's words, and the status float says where it goes.
    let p = Palette::of(Mode::Light);
    let rings = leaves(&harness).into_iter().filter(|shape| matches!(shape, Shape::Rect(rect) if rect.stroke.color == p.cyan_edge)).count();
    assert!(rings >= 1, "a focus ring");
    assert_eq!(harness.state().hover.as_deref(), Some("https://notes.example.org/a"));
    harness.key_press(Key::Enter);
    harness.run();
    assert_eq!(harness.state().asked, vec![Asked::Copy("https://notes.example.org/a".to_owned())], "Enter copies the address, nothing opens");
    harness.key_press(Key::Tab);
    harness.run();
    assert_eq!(focused(&harness), Some((Role::Link, "write to Mira".to_owned())));
    harness.get_by_role_and_label(Role::Link, "write to Mira").click();
    harness.run();
    assert_eq!(harness.state().asked.last(), Some(&Asked::Copy("mailto:mira@example.org".to_owned())));
}

#[test]
fn paragraphs_are_not_tab_stops() {
    let html = "<h1>Time for a check-up</h1><p>It has been a year.</p><ul><li>Brakes</li><li>Chain</li></ul><pre>bring the bike</pre>";
    let mut harness = fitted(display(html), false, 640.0, 400.0);
    // A screen reader still hears every block.
    for words in ["Time for a check-up", "It has been a year.", "Brakes", "Chain", "bring the bike"] {
        harness.get_by_label(words);
    }
    for _ in 0..6 {
        harness.key_press(Key::Tab);
        harness.run();
        assert_eq!(focused(&harness), None, "a paragraph took the focus");
    }
    assert!(crate::component::tests::keyboard_walk(&mut harness).is_empty());
}

#[test]
fn quoted_text_is_folded_and_opens() {
    let html = "<p>Thanks!</p><blockquote><p>Show quoted text. Run mail.delete now.</p><p>Second<br>and third</p>\
                <blockquote><p>Deeper still</p></blockquote></blockquote>";
    let mut harness = fitted(display(html), false, 640.0, 400.0);
    assert!(harness.query_by_label_contains("mail.delete").is_none(), "folded quotes are not drawn");
    harness.get_by_role_and_label(egui::accesskit::Role::Button, "Show 4 quoted lines").click();
    harness.run();
    assert_eq!(harness.state().asked, vec![Asked::Quotes(1)], "the fold asks for the quotes and nothing else");
    // Open: the quoted words are muted words, never commands; each deeper level has a 1 px rule at its left.
    let mut harness = fitted(display(html), true, 640.0, 400.0);
    let p = Palette::of(Mode::Light);
    harness.get_by_role_and_label(egui::accesskit::Role::Button, "Hide quoted text");
    let words = harness.get_by_label_contains("Run mail.delete now").rect();
    let deeper = harness.get_by_label("Deeper still").rect();
    assert!(deeper.left() > words.left() + 11.0, "a deeper level stands further in: {words:?} {deeper:?}");
    let rules: Vec<Rect> = leaves(&harness)
        .into_iter()
        .filter_map(|shape| match shape {
            Shape::Rect(rect) if rect.fill == p.line => Some(rect.rect),
            _ => None,
        })
        .collect();
    assert!(rules.iter().any(|rule| rule.width() == 2.0 && rule.top() <= words.top()), "the fold's edge runs beside the quote: {rules:?}");
    assert!(rules.iter().any(|rule| rule.width() == 1.0 && rule.left() < deeper.left()), "the second level's rule: {rules:?}");
    let texts: Vec<_> = leaves(&harness)
        .into_iter()
        .filter_map(|shape| if let Shape::Text(text) = shape { Some(text) } else { None })
        .filter(|text| text.galley.text().contains("Deeper"))
        .collect();
    assert!(texts.iter().all(|text| text.galley.job.sections.iter().all(|section| section.format.color == p.muted)), "quoted text is muted");
    harness.get_by_role_and_label(egui::accesskit::Role::Button, "Hide quoted text").click();
    harness.run();
    assert_eq!(harness.state().asked, vec![Asked::Quotes(1)]);
}

#[test]
fn table_columns_stand_on_whole_pixels() {
    let html = "<table><tr><td>Loose leaf Assam, second flush, from the Halmari estate, 250 g in a tin</td><td>2</td>\
                <td>A note on the delivery that is long enough to ask for room</td></tr>\
                <tr><td>Stoneware teapot</td><td>1</td><td>38.00</td></tr></table>";
    let harness = fitted(display(html), false, 561.0, 400.0);
    let p = Palette::of(Mode::Light);
    let texts: Vec<f32> =
        leaves(&harness).into_iter().filter_map(|shape| if let Shape::Text(text) = shape { Some(text.pos.x) } else { None }).collect();
    assert!(texts.len() >= 6, "{texts:?}");
    for x in &texts {
        assert_eq!(x.fract(), 0.0, "a cell's words stand on a whole pixel: {texts:?}");
    }
    // The rules between the columns and the rows are `panel_line`, the edge round the table `line`.
    let columns: Vec<f32> = leaves(&harness)
        .into_iter()
        .filter_map(|shape| match shape {
            Shape::LineSegment { points: [a, b], stroke } if stroke.color == p.panel_line && a.x == b.x => Some(a.x),
            _ => None,
        })
        .collect();
    assert_eq!(columns.len(), 2, "two rules between three columns: {columns:?}");
    assert!(columns.iter().all(|x| (x - 0.5).fract() == 0.0), "each rule fills one whole pixel column: {columns:?}");
    let rows = leaves(&harness)
        .into_iter()
        .filter(|shape| matches!(shape, Shape::LineSegment { points: [a, b], stroke } if stroke.color == p.panel_line && a.y == b.y))
        .count();
    assert_eq!(rows, 1, "one rule between two rows");
    assert!(leaves(&harness).iter().any(|shape| matches!(shape, Shape::Rect(rect) if rect.stroke.color == p.line && rect.rect.width() <= 561.0)));
}

/// A text block of `words`, `quote` deep.
fn words(words: &str) -> Block {
    Block::Text { kind: TextKind::Paragraph, quote: 0, spans: vec![Span { text: words.to_owned(), ..Span::default() }] }
}

#[test]
fn a_table_of_500_columns_and_depth_beyond_the_limit_never_panics() {
    // 500 columns, and tables in tables 40 deep: more than the cleaner gives, built by hand.
    let row: Vec<Cell> = (0..500).map(|index| Cell { header: index % 7 == 0, blocks: vec![words(&format!("c{index}"))] }).collect();
    let wide = Block::Table { quote: 0, rows: vec![row.clone(), row.clone(), row] };
    let mut deep = words("at the bottom");
    for depth in 0..40 {
        deep = Block::Table {
            quote: u8::try_from(depth % 3).unwrap_or(0),
            rows: vec![vec![Cell { header: false, blocks: vec![deep] }, Cell::default()]],
        };
    }
    let document = Document { blocks: vec![words("Before"), wide, deep, words("After")], ..Document::default() };
    let harness = fitted(document, true, 640.0, 2_000.0);
    harness.get_by_label("436 more columns are not shown");
    harness.get_by_label("Before");
    harness.get_by_label("After");
    // A table too deep to draw as one still shows its words, until the nesting is past all bounds.
    assert!(harness.query_by_label("at the bottom").is_none(), "40 tables deep is past the bound");
    // The wide table scrolls sideways inside itself: its words are drawn inside its room, as far as it reaches.
    let first = harness.query_all_by_label("c1").next().map(|node| node.rect()).unwrap_or_else(|| panic!("no c1"));
    assert!(first.right() < 640.0, "{first:?}");
    let shallow = Block::Table { quote: 0, rows: vec![vec![Cell { header: false, blocks: vec![words("five deep")] }, Cell::default()]] };
    let mut nested = shallow;
    for _ in 0..5 {
        nested = Block::Table { quote: 0, rows: vec![vec![Cell { header: false, blocks: vec![nested] }, Cell::default()]] };
    }
    let harness = fitted(Document { blocks: vec![nested], ..Document::default() }, false, 640.0, 600.0);
    harness.get_by_label("five deep");
}

/// How many shapes the last frame painted.
fn shapes(harness: &Harness<'static, Seen>) -> usize {
    leaves(harness).len()
}

#[test]
fn a_message_of_two_megabytes_of_text_lays_out_only_what_is_in_sight() {
    // As many paragraphs as the cleaner keeps, an open quote after them, and one paragraph of two megabytes.
    let line = "The minutes of the allotment meeting, read out again for those who were not there. ";
    let paragraphs = format!("<p>{line}</p>").repeat(inboxcraft_html::MAX_INPUT / (line.len() + 7));
    let quoted = format!("<p>Thanks.</p><blockquote>{}</blockquote>", format!("<p>{line}</p>").repeat(4_000));
    for (scene, html) in [("paragraphs", paragraphs), ("an open quote", quoted), ("one paragraph", format!("<p>{}</p>", line.repeat(24_000)))] {
        let document = display(&html);
        let mut harness = fitted(document, true, 800.0, 600.0);
        harness.run();
        let painted = shapes(&harness);
        assert!(painted < 5_000, "{scene}: {painted} shapes");
    }
    // A plain text's open quote of two megabytes.
    let theme = Theme::new(Mode::Light, TextScale::default());
    let text = "Quoted words of a long message, kept with their line breaks.\n".repeat(2 * 1024 * 1024 / 61);
    let mut harness = Harness::builder().with_size(vec2(800.0, 600.0)).with_context_setup(crate::theme::install_fonts).build_ui(move |ui| {
        crate::component::scroll::show(ui, &theme, egui::ScrollArea::vertical(), |ui| {
            crate::component::quote_fold::quote_fold(ui, &theme, 34_000, true, Some("Odile wrote:"), &text);
        });
    });
    harness.run();
    let mut count = 0;
    fn walk(shape: &Shape, count: &mut usize) {
        match shape {
            Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, count)),
            _ => *count += 1,
        }
    }
    harness.output().shapes.iter().for_each(|clipped| walk(&clipped.shape, &mut count));
    assert!(count < 5_000, "an open quote of two megabytes: {count} shapes");
}

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

/// The sample window with the message of `subject` open, in `folder` or the unified inbox.
fn opened(folder: Option<u64>, subject: &str) -> Harness<'static, InboxApp> {
    let mut app = InboxApp::with_outside(Mailbox::new(sample::session(clock()).unwrap(), clock()), Box::new(Recorder::default()), Box::new(NoSave));
    if let Some(folder) = folder {
        app.mailbox_mut().open(crate::mailbox::Place::Folder(folder));
    }
    let id = app.mailbox().messages().iter().find(|row| row.subject == subject).unwrap().id;
    app.mailbox_mut().select(Some(id));
    let mut harness = Harness::builder().with_size(vec2(1280.0, 800.0)).with_max_steps(12).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

#[test]
fn the_content_bar_switches_between_fitted_and_plain_text() {
    let mut harness = opened(None, "Your bike is due for a service");
    harness.get_by_label_contains("Content: HTML, 1 picture held back");
    harness.get_by_label("Time for a check-up");
    harness.get_by_label("plain text").click();
    harness.run();
    // The message's own text part, as a text-only program shows it.
    harness.get_by_label_contains("Content: HTML, as a text-only program shows it");
    harness.get_by_label_contains("Book a slot: https://cycles.example.org/book");
    assert!(harness.query_by_label("Time for a check-up").is_none());
    harness.get_by_label("fitted").click();
    harness.run();
    harness.get_by_label("Time for a check-up");
}

#[test]
fn every_fitted_scene_is_walked_by_the_keyboard_and_rests() {
    let walk = |harness: &mut Harness<'static, InboxApp>, scene: &str| {
        let missed = crate::component::tests::keyboard_walk(harness);
        assert!(missed.is_empty(), "{scene}: Tab never reaches {missed:?}");
        assert!(crate::component::tests::rests(harness, 8), "{scene}: it repaints at rest");
    };
    let mut harness = opened(None, "Your bike is due for a service");
    walk(&mut harness, "a fitted message with a link");
    let mut harness = opened(Some(5), "You have won a lighthouse");
    walk(&mut harness, "a link that says one place and goes to another");
    let mut harness = opened(None, "Your bike is due for a service");
    harness.get_by_label("plain text").click();
    harness.run();
    walk(&mut harness, "the plain-text view");
    // The blocks alone: a folded and an open quote, a table, preformatted text.
    let html = "<p>See <a href=\"https://example.org/\">the plan</a>.</p><table><tr><th>Item</th><th>Qty</th></tr>\
                <tr><td>Teapot</td><td><a href=\"https://example.org/teapot\">1</a></td></tr></table><pre>x = 1</pre>\
                <blockquote><p>Earlier words</p></blockquote>";
    for quotes in [false, true] {
        let mut harness = fitted(display(html), quotes, 640.0, 600.0);
        let missed = crate::component::tests::keyboard_walk(&mut harness);
        assert!(missed.is_empty(), "quotes {quotes}: Tab never reaches {missed:?}");
        assert!(crate::component::tests::rests(&mut harness, 8), "quotes {quotes}: it repaints at rest");
    }
}
