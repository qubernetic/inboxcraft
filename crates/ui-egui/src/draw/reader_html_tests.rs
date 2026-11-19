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
    fitted_at(document, quotes, width, high, 1.0)
}

/// [`fitted`] at `ppp` physical pixels a point.
fn fitted_at(document: Document, quotes: bool, width: f32, high: f32, ppp: f32) -> Harness<'static, Seen> {
    let theme = Theme::new(Mode::Light, TextScale::default());
    let mut reader = crate::reader::Reader::new(Box::<crate::fetch::Offline>::default(), Box::new(NoSave));
    if quotes {
        reader.toggle_quotes(1);
    }
    let mut harness =
        Harness::builder().with_size(vec2(width, high)).with_pixels_per_point(ppp).with_context_setup(crate::theme::install_fonts).build_ui_state(
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
    // Each column's words start on a whole pixel and its width is whole pixels, at 1 and at 1.5 pixels a point (a
    // screen reader's box of a cell is its column's).
    for ppp in [1.0_f32, 1.5] {
        let harness = fitted_at(display(html), false, 561.0, 400.0, ppp);
        let cells: Vec<Rect> = ["Stoneware teapot", "1", "38.00"].iter().map(|words| harness.get_by_label(words).rect()).collect();
        let whole = |points: f32| ((points * ppp) - (points * ppp).round()).abs() < 1e-3;
        for cell in &cells {
            assert!(whole(cell.left()), "a column's left on a whole pixel at {ppp}: {cells:?}");
            assert!(whole(cell.width()), "a column whole pixels wide at {ppp}: {cells:?}");
        }
    }
    let harness = fitted(display(html), false, 561.0, 400.0);
    let p = Palette::of(Mode::Light);
    let texts: Vec<f32> =
        leaves(&harness).into_iter().filter_map(|shape| if let Shape::Text(text) = shape { Some(text.pos.x) } else { None }).collect();
    assert!(texts.len() >= 6, "{texts:?}");
    for x in &texts {
        assert_eq!(x.fract(), 0.0, "a cell's words stand on a whole pixel: {texts:?}");
    }
    // The rule between the rows is `panel_line`, the edge round the table `line`; no rule stands between columns.
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
        // Only what lies near sight is given to a screen reader: three panes of thousands of paragraphs.
        let heard = harness.query_all_by_role(Role::Label).count();
        assert!(heard < 200, "{scene}: {heard} labels");
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
    let mut app =
        InboxApp::with_outside_at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock()), Box::new(Recorder::default()), Box::new(NoSave));
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

/// Paragraphs of words without a link, `panes` panes of 600 high.
fn filler(panes: usize) -> String {
    "<p>Filler words that carry no link at all and stand between two links.</p>".repeat(panes * 600 / 34)
}

/// `document` in a pane 640 × 600 that scrolls, a button under the pane: what Tab reaches after the message.
fn fitted_then_button(document: Document) -> Harness<'static, Seen> {
    fitted_then_button_over(document, crate::reader::Reader::new(Box::<crate::fetch::Offline>::default(), Box::new(NoSave)))
}

/// [`fitted_then_button`] drawn over `reader` (its pictures as it holds them).
fn fitted_then_button_over(document: Document, reader: crate::reader::Reader) -> Harness<'static, Seen> {
    let theme = Theme::new(Mode::Light, TextScale::default());
    let mut harness =
        Harness::builder().with_size(vec2(640.0, 640.0)).with_max_steps(64).with_context_setup(crate::theme::install_fonts).build_ui_state(
            move |ui, seen: &mut Seen| {
                let room = Rect::from_min_size(pos2(0.0, 0.0), vec2(640.0, 600.0));
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
                ui.scope_builder(UiBuilder::new().max_rect(Rect::from_min_size(pos2(0.0, 604.0), vec2(200.0, 30.0))), |ui| {
                    let _ = ui.button("Outside the reader");
                });
            },
            Seen::default(),
        );
    harness.run();
    harness
}

#[test]
fn every_link_is_reached_by_tab_and_shift_tab_however_far_apart() {
    let html = format!(
        "<p><a href=\"https://example.org/a\">link A</a></p>{}<p><a href=\"https://example.org/b\">link B</a></p>{}\
         <p><a href=\"https://example.org/c\">link C</a></p>",
        filler(3),
        filler(3)
    );
    let mut harness = fitted_then_button(display(&html));
    let step = |harness: &mut Harness<'static, Seen>, back: bool| {
        if back {
            harness.key_press_modifiers(egui::Modifiers::SHIFT, Key::Tab);
        } else {
            harness.key_press(Key::Tab);
        }
        harness.run();
        harness.run();
        focused(harness).map(|(_, label)| label)
    };
    let forward: Vec<_> = (0..4).map(|_| step(&mut harness, false)).collect();
    assert_eq!(forward, ["link A", "link B", "link C", "Outside the reader"].map(|label| Some(label.to_owned())), "Tab");
    // The focused link was brought into sight.
    let back: Vec<_> = (0..3).map(|_| step(&mut harness, true)).collect();
    assert_eq!(back, ["link C", "link B", "link A"].map(|label| Some(label.to_owned())), "Shift Tab");
    let link = harness.get_by_role_and_label(Role::Link, "link A").rect();
    assert!(link.top() >= 0.0 && link.bottom() <= 600.0, "the focused link is in sight: {link:?}");
}

/// Where the words `label` of a table cell stand, if they are drawn.
fn cell_at(harness: &Harness<'static, Seen>, label: &str) -> Option<Rect> {
    harness.query_all_by_label(label).next().map(|node| node.rect())
}

#[test]
fn a_table_wider_than_its_room_is_a_tab_stop_whose_arrows_show_its_other_columns() {
    // The owner's ruling: the columns scrolled off are reached by the arrows while the table has the focus.
    let cells: String = (0..24).map(|index| format!("<td>column{index}</td>")).collect();
    // Far under the link: Tab finds it out of sight and brings it in.
    let filler = "<p>filler</p>".repeat(120);
    let html = format!("<p><a href=\"https://example.org/before\">before</a></p>{filler}<table><tr>{cells}</tr><tr>{cells}</tr></table>");
    let mut harness = fitted_then_button(display(&html));
    let last = |harness: &Harness<'static, Seen>| cell_at(harness, "column23").is_some_and(|rect| rect.right() <= 640.0 && rect.left() >= 0.0);
    let first = |harness: &Harness<'static, Seen>| cell_at(harness, "column0").is_some_and(|rect| rect.left() >= 0.0);
    assert!(cell_at(&harness, "column0").is_none_or(|rect| rect.top() > 600.0), "the table starts out of sight");
    harness.key_press(Key::Tab);
    harness.run();
    assert_eq!(focused(&harness).map(|(_, label)| label).as_deref(), Some("before"));
    harness.key_press(Key::Tab);
    harness.run();
    harness.run();
    assert!(!last(&harness) && first(&harness), "the table in sight, its last column not");
    let table = focused(&harness);
    assert_eq!(table.as_ref().map(|(role, _)| *role), Some(Role::ScrollView), "Tab reaches the table: {table:?}");
    for _ in 0..80 {
        harness.key_press(Key::ArrowRight);
        harness.run();
    }
    assert!(last(&harness), "Right brought the last column in: {:?}", cell_at(&harness, "column23"));
    assert_eq!(focused(&harness), table, "and the table keeps the focus");
    for _ in 0..80 {
        harness.key_press(Key::ArrowLeft);
        harness.run();
    }
    assert!(first(&harness) && !last(&harness), "Left took it back to the first");
    harness.key_press(Key::Tab);
    harness.run();
    assert_eq!(focused(&harness).map(|(_, label)| label).as_deref(), Some("Outside the reader"), "Tab goes on past it");
    harness.key_press_modifiers(egui::Modifiers::SHIFT, Key::Tab);
    harness.run();
    assert_eq!(focused(&harness), table, "and Shift Tab comes back to it");
}

#[test]
fn the_arrows_of_a_wide_table_of_links_scroll_it_and_never_take_the_focus_to_a_link() {
    // The table keeps the arrows while it has the focus: egui would move the focus to the link beside it.
    let cells: String = (0..24).map(|index| format!("<td><a href=\"https://example.org/{index}\">cell{index}</a></td>")).collect();
    let html = format!("<table><tr>{cells}</tr></table>");
    let mut harness = fitted_then_button(display(&html));
    harness.key_press(Key::Tab);
    harness.run();
    harness.run();
    let table = focused(&harness);
    assert_eq!(table.as_ref().map(|(role, _)| *role), Some(Role::ScrollView), "Tab reaches the table first: {table:?}");
    let start = cell_at(&harness, "cell0").map(|rect| rect.left());
    for _ in 0..3 {
        harness.key_press(Key::ArrowRight);
        harness.run();
        assert_eq!(focused(&harness), table, "Right leaves the focus on the table");
    }
    let moved = cell_at(&harness, "cell0").map(|rect| rect.left());
    assert!(moved < start, "Right scrolled the table: {start:?} to {moved:?}");
    harness.key_press(Key::ArrowLeft);
    harness.run();
    assert_eq!(focused(&harness), table, "Left too");
    // No trap: Tab and Shift Tab leave it.
    harness.key_press(Key::Tab);
    harness.run();
    assert!(focused(&harness).is_some_and(|now| Some(&now) != table.as_ref()), "Tab leaves the table");
    harness.key_press_modifiers(egui::Modifiers::SHIFT, Key::Tab);
    harness.run();
    assert_eq!(focused(&harness), table, "Shift Tab comes back to it");
    harness.key_press_modifiers(egui::Modifiers::SHIFT, Key::Tab);
    harness.run();
    assert_ne!(focused(&harness), table, "and leaves it backwards");
}

#[test]
fn a_focused_link_in_a_table_keeps_the_focus_far_from_sight() {
    let html = format!(
        "<table><tr><td><a href=\"https://example.org/1\">first</a></td><td><a href=\"https://example.org/2\">second</a></td></tr>\
         {}<tr><td><a href=\"https://example.org/3\">third</a></td><td>x</td></tr></table>",
        "<tr><td>row</td><td>row</td></tr>".repeat(200)
    );
    let mut harness = fitted_then_button(display(&html));
    harness.key_press(Key::Tab);
    harness.run();
    assert_eq!(focused(&harness).map(|(_, label)| label).as_deref(), Some("first"));
    // The wheel takes the pane far down the table.
    harness.hover_at(pos2(300.0, 300.0));
    harness.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: vec2(0.0, -2_500.0),
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Unknown,
        modifiers: egui::Modifiers::NONE,
    });
    harness.run();
    harness.run();
    assert_eq!(focused(&harness).map(|(_, label)| label).as_deref(), Some("first"), "the focus stays on the link");
    let mut next = Vec::new();
    for _ in 0..3 {
        harness.key_press(Key::Tab);
        harness.run();
        harness.run();
        next.push(focused(&harness).map(|(_, label)| label));
    }
    assert_eq!(next, ["second", "third", "Outside the reader"].map(|label| Some(label.to_owned())));
}

#[test]
fn a_fitted_table_and_preformatted_text_look_as_the_design_draws_them() {
    // The hairline table's sheet: a head 28 high on `panel`, rows 32 high, a `panel_line` between two rows and none
    // between the columns, the words 12 in and the columns 12 apart, a 1 px `line` edge; preformatted text in
    // `mono_s` 11/16, 8 × 12 inside its box.
    let html = "<table><tr><th>Week</th><th>Who</th></tr><tr><td>1</td><td>Odile</td></tr><tr><td>2</td><td>Mira</td></tr></table>\
                <pre>gate code: 4417</pre>";
    let harness = fitted(display(html), false, 640.0, 400.0);
    let p = Palette::of(Mode::Light);
    let theme = Theme::new(Mode::Light, TextScale::default());
    let edge = leaves(&harness)
        .into_iter()
        .find_map(|shape| match shape {
            Shape::Rect(rect) if rect.stroke.color == p.line && rect.fill == egui::Color32::TRANSPARENT => Some(rect.rect),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no edge"));
    assert_eq!(edge.height(), 28.0 + 1.0 + 32.0 + 1.0 + 32.0, "{edge:?}");
    let hairlines: Vec<_> = leaves(&harness)
        .into_iter()
        .filter_map(|shape| match shape {
            Shape::LineSegment { points: [a, b], stroke } if stroke.color == p.panel_line => Some((a, b)),
            _ => None,
        })
        .collect();
    assert_eq!(hairlines.len(), 2, "rules between the rows only: {hairlines:?}");
    assert!(hairlines.iter().all(|(a, b)| a.y == b.y), "no rule between columns: {hairlines:?}");
    let head = leaves(&harness).into_iter().find_map(|shape| match shape {
        Shape::Rect(rect) if rect.fill == p.panel => Some(rect.rect),
        _ => None,
    });
    assert_eq!(head.map(|head| head.height()), Some(28.0), "{head:?}");
    // A head's labels are set in capitals, and heard as they are shown.
    let week = harness.get_by_label("WEEK").rect();
    let who = harness.get_by_label("WHO").rect();
    let odile = harness.get_by_label("Odile").rect();
    assert_eq!(week.left() - edge.left(), 12.0, "the words 12 in");
    assert_eq!(who.left() - week.right(), 12.0, "the columns 12 apart: {week:?} {who:?}");
    assert_eq!(week.top() - edge.top(), 6.0, "a head's label 6 under the top");
    assert_eq!(odile.top() - edge.top(), 29.0 + 5.0, "a row's words 5 under its top");
    let pre = leaves(&harness)
        .into_iter()
        .find_map(|shape| match shape {
            Shape::Text(text) if text.galley.text().contains("gate code") => Some(text),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no preformatted text"));
    let size = pre.galley.job.sections.first().map(|section| section.format.font_id.size);
    assert_eq!(size, Some(theme.fonts.mono_s.size), "preformatted text in mono_s");
    let boxed = leaves(&harness)
        .into_iter()
        .find_map(|shape| match shape {
            Shape::Rect(rect) if rect.fill == p.term => Some(rect.rect),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no box"));
    assert_eq!(boxed.height(), 8.0 + theme.fonts.mono_s.line + 8.0, "{boxed:?}");
}

#[test]
fn the_plain_view_is_set_in_mono() {
    let mut harness = opened(None, "Your bike is due for a service");
    harness.get_by_label("plain text").click();
    harness.run();
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let mut sizes = Vec::new();
    fn walk(shape: &Shape, sizes: &mut Vec<f32>) {
        match shape {
            Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, sizes)),
            Shape::Text(text) if text.galley.text().contains("Book a slot") => {
                sizes.extend(text.galley.job.sections.iter().map(|section| section.format.font_id.size));
            }
            _ => {}
        }
    }
    harness.output().shapes.iter().for_each(|clipped| walk(&clipped.shape, &mut sizes));
    // The message's words in `mono` 13/20, not in the reading style (a row's preview of them stays in its caption).
    assert!(sizes.contains(&theme.fonts.mono.size) && !sizes.contains(&theme.fonts.read.size), "{sizes:?}");
}

#[test]
fn a_message_that_does_not_fit_beside_the_kept_layouts_is_shown_plain_and_says_why() {
    // With the kept layouts capped at one letter, no fitted message fits: the reader shows it plain, under a notice.
    let harness = super::reader::with_cap(1, || {
        let mut harness = opened(None, "Your bike is due for a service");
        harness.run();
        harness
    });
    harness.get_by_label("This message is shown as plain text: the reader holds as much fitted text as it keeps.");
    assert!(harness.query_by_label("Time for a check-up").is_none(), "the fitted heading is not drawn");
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let mut sizes = Vec::new();
    fn walk(shape: &Shape, sizes: &mut Vec<f32>) {
        match shape {
            Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, sizes)),
            Shape::Text(text) if text.galley.text().contains("Book a slot") => {
                sizes.extend(text.galley.job.sections.iter().map(|section| section.format.font_id.size));
            }
            _ => {}
        }
    }
    harness.output().shapes.iter().for_each(|clipped| walk(&clipped.shape, &mut sizes));
    assert!(sizes.contains(&theme.fonts.mono.size) && !sizes.contains(&theme.fonts.read.size), "{sizes:?}");
}

#[test]
fn a_message_s_headings_are_set_as_the_frames_set_them() {
    // The frames set a message's heading in Plex Sans SemiBold 22/29 and a deeper one as `strong`, 14/22.
    let harness = fitted(display("<h1>Thank you, Odile.</h1><h2>Your order</h2><h3>Delivery</h3>"), false, 640.0, 400.0);
    let theme = Theme::new(Mode::Light, TextScale::default());
    let set = |words: &str| {
        leaves(&harness).into_iter().find_map(|shape| match shape {
            Shape::Text(text) if text.galley.text() == words => {
                text.galley.job.sections.first().map(|section| (section.format.font_id.clone(), section.format.line_height))
            }
            _ => None,
        })
    };
    let title = theme.fonts.written_title();
    assert_eq!((title.size, title.line), (22.0, 29.0));
    assert_eq!(set("Thank you, Odile."), Some((title.id(), Some(29.0))));
    for words in ["Your order", "Delivery"] {
        assert_eq!(set(words), Some((theme.fonts.strong.id(), Some(22.0))), "{words}");
    }
}

/// The sample window with the message of `subject` open in `folder`, its remote pictures fetched through `fetch`.
fn opened_with(fetch: &Recorder, folder: u64, subject: &str) -> Harness<'static, InboxApp> {
    let mut app = InboxApp::with_outside_at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock()), Box::new(fetch.clone()), Box::new(NoSave));
    app.mailbox_mut().open(crate::mailbox::Place::Folder(folder));
    let id = app.mailbox().messages().iter().find(|row| row.subject == subject).unwrap().id;
    app.mailbox_mut().select(Some(id));
    let mut harness = Harness::builder().with_size(vec2(1280.0, 800.0)).with_max_steps(12).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    for _ in 0..4 {
        harness.run();
    }
    harness
}

/// Whether the node labelled `name` has the focus.
fn has_focus(harness: &Harness<'static, InboxApp>, name: &str) -> bool {
    harness.query_by_label(name).is_some_and(|node| node.accesskit_node().is_focused())
}

/// A fitted message's remote picture is the picture box in each of its states: not loaded, here, failed with `try
/// again` (reached by Tab, and it asks once more) and here but not drawable, whose box says so and offers no `try
/// again`, as trying again would get the same bytes.
#[test]
fn a_fitted_message_s_remote_picture_is_its_box_in_every_state() {
    let fetch = Recorder::default();
    let mut harness = opened_with(&fetch, 5, "You have won a lighthouse");
    let url = "https://track.example.net/open.png?id=4417";
    let settle = |harness: &mut Harness<'static, InboxApp>| {
        for _ in 0..4 {
            harness.run();
        }
    };
    // Not loaded: the dashed box, nothing asked.
    harness.get_by_label_contains("Remote picture not loaded");
    assert!(harness.query_by_label("try again").is_none());
    assert!(fetch.asked.lock().unwrap().is_empty());
    // Loaded: the picture itself, no box.
    harness.get_by_label("always for prize-desk@example.org").click();
    settle(&mut harness);
    assert_eq!(fetch.asked.lock().unwrap().len(), 1);
    harness.get_by_role_and_label(Role::Image, "Picture");
    assert!(harness.query_by_label_contains("Remote picture not loaded").is_none());
    // Failed: why, and `try again`, which Tab reaches and Enter presses: the picture is asked for once more.
    harness.state_mut().reader_mut().0.put_picture(url, crate::reader::PictureState::Failed("no answer".to_owned()));
    settle(&mut harness);
    harness.get_by_label_contains("Picture could not be shown: no answer");
    let mut reached = false;
    for _ in 0..60 {
        harness.key_press(Key::Tab);
        harness.run();
        if has_focus(&harness, "try again") {
            reached = true;
            break;
        }
    }
    assert!(reached, "Tab reaches try again");
    harness.key_press(Key::Enter);
    settle(&mut harness);
    assert_eq!(fetch.asked.lock().unwrap().len(), 2, "try again asks for the picture once more");
    harness.get_by_role_and_label(Role::Image, "Picture");
    // Here but not drawable: its own reason, no `try again`.
    let broken = crate::picture::Decoded { width: 2, height: 2, rgba: vec![0; 3] };
    harness.state_mut().reader_mut().0.put_picture(url, crate::reader::PictureState::Ready(std::sync::Arc::new(broken)));
    settle(&mut harness);
    harness.get_by_label_contains("Picture not shown: its data could not be drawn");
    assert!(harness.query_by_label_contains("could not be shown").is_none(), "not the failed box");
    assert!(harness.query_by_label("try again").is_none(), "a try again that can do nothing");
}

/// The remote-picture banner stands over a fitted message, not over the plain view the owner chose; over the plain
/// view a message falls back to when it does not fit beside the kept layouts it still stands, as the owner chose
/// nothing there.
#[test]
fn the_picture_banner_leaves_the_chosen_plain_view_and_stays_over_the_fallback() {
    const BANNER: &str = "1 remote picture is not shown. Loading it tells the sender that you opened this message.";
    let mut harness = opened(None, "Your bike is due for a service");
    harness.get_by_label(BANNER);
    harness.get_by_label("plain text").click();
    harness.run();
    assert!(harness.query_by_label(BANNER).is_none(), "the plain view the owner chose shows no banner");
    harness.get_by_label("fitted").click();
    harness.run();
    harness.get_by_label(BANNER);
    let harness = super::reader::with_cap(1, || {
        let mut harness = opened(None, "Your bike is due for a service");
        harness.run();
        harness
    });
    harness.get_by_label("This message is shown as plain text: the reader holds as much fitted text as it keeps.");
    harness.get_by_label(BANNER);
}

/// A failed picture whose box cannot hold `try again` (short and wide, or narrow and tall, as a sender's tag may ask)
/// offers no link and is no Tab stop: Tab from the link before it walks on to the link after it, however far.
#[test]
fn a_failed_box_too_short_or_too_narrow_for_try_again_is_no_tab_stop() {
    let url = "https://pictures.example.org/broken.png";
    for (width, height) in [(300, 20), (40, 400)] {
        let html = format!(
            "<p><a href=\"https://example.org/a\">link A</a></p>{}<img src=\"{url}\" alt=\"\" width={width} height={height}>{}\
             <p><a href=\"https://example.org/b\">link B</a></p>",
            filler(2),
            filler(2)
        );
        let document = inboxcraft_html::display(&html, &inboxcraft_html::Options { remote_content: inboxcraft_html::RemoteContent::Allowed });
        let mut reader = crate::reader::Reader::new(Box::<crate::fetch::Offline>::default(), Box::new(NoSave));
        reader.put_picture(url, crate::reader::PictureState::Failed("no answer".to_owned()));
        let mut harness = fitted_then_button_over(document, reader);
        let mut seen = Vec::new();
        for _ in 0..3 {
            harness.key_press(Key::Tab);
            harness.run();
            harness.run();
            seen.push(focused(&harness).map(|(_, label)| label));
        }
        assert_eq!(seen, ["link A", "link B", "Outside the reader"].map(|label| Some(label.to_owned())), "{width} × {height}");
        assert!(harness.query_by_label("try again").is_none(), "{width} × {height}: no try again where none is drawn");
    }
}
