// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What a fitted message keeps of its look, as the Messages page draws it: bands of kept backgrounds, aligned blocks,
//! tables at their widths, button-links; the eight kinds of mail the page shows; hostile looks laid out bounded.

use egui::{Color32, Rect, vec2};
use egui_kittest::Harness;
use inboxcraft_html::Document;

use crate::save::NoSave;
use crate::theme::{Mode, TextScale, Theme};

use super::{Context, Kind, Laid, Style, lay};

/// A placed block as a test sees it.
#[derive(Debug, Clone, PartialEq)]
enum Seen {
    /// Words: their rect, the galley's origin into the box, its alignment, and the colour of its first run.
    Text { rect: Rect, words: String, shift: f32, halign: egui::Align, colour: Color32 },
    /// A button-link: its rect, its words, its own background and its ink.
    Button { rect: Rect, words: String, fill: Option<Color32>, ink: Color32 },
    /// A table: its rect, how wide it is laid out, its columns' widths, its cells' backgrounds row by row.
    Table { rect: Rect, wide: f32, widths: Vec<f32>, fills: Vec<Vec<Option<Color32>>> },
    /// Anything else, with its rect.
    Other { rect: Rect },
}

/// A message laid out: its blocks as seen, the bands of kept backgrounds, its height.
#[derive(Debug, Default)]
struct Look {
    items: Vec<Seen>,
    grounds: Vec<(Rect, Color32)>,
    height: f32,
}

fn seen(laid: &Laid) -> Look {
    let items = laid
        .items
        .iter()
        .map(|item| match &item.kind {
            Kind::Text { galley, shift, .. } => Seen::Text {
                rect: item.rect,
                words: galley.text().to_owned(),
                shift: *shift,
                halign: galley.job.halign,
                colour: galley.job.sections.first().map_or(Color32::TRANSPARENT, |section| section.format.color),
            },
            Kind::ButtonLink { galley, fill, ink, .. } => Seen::Button { rect: item.rect, words: galley.text().to_owned(), fill: *fill, ink: *ink },
            Kind::Table(grid) => Seen::Table {
                rect: item.rect,
                wide: grid.size.x,
                widths: grid.widths.clone(),
                fills: grid.rows.iter().map(|row| row.fills.clone()).collect(),
            },
            _ => Seen::Other { rect: item.rect },
        })
        .collect();
    Look { items, grounds: laid.grounds.clone(), height: laid.height }
}

/// `html` laid out `width` wide in `mode`, as the reader lays a message out.
fn laid_out(html: &str, width: f32, mode: Mode) -> Look {
    let document = inboxcraft_html::display(html, &inboxcraft_html::Options::default());
    laid_document(document, width, mode)
}

fn laid_document(document: Document, width: f32, mode: Mode) -> Look {
    let theme = Theme::new(mode, TextScale::default());
    let mut harness = Harness::builder().with_size(vec2(width + 40.0, 600.0)).with_context_setup(crate::theme::install_fonts).build_ui_state(
        move |ui, look: &mut Look| {
            let reader = crate::reader::Reader::new(Box::<crate::fetch::Offline>::default(), Box::new(NoSave));
            let context = Context { ui, theme: &theme, reader: &reader, document: &document, id: 1, quotes: false, ppp: 1.0 };
            *look = seen(&lay(&context, &document.blocks, width, Style { text: theme.fonts.read, head: false, ground: None }, 0));
        },
        Look::default(),
    );
    harness.run();
    std::mem::take(harness.state_mut())
}

fn rgb(hex: u32) -> Color32 {
    let [_, r, g, b] = hex.to_be_bytes();
    Color32::from_rgb(r, g, b)
}

const DARK: u32 = 0x19_19_19;
const WHITE: u32 = 0xFF_FF_FF;

fn texts(look: &Look) -> Vec<&Seen> {
    look.items.iter().filter(|item| matches!(item, Seen::Text { .. })).collect()
}

fn text<'l>(look: &'l Look, words: &str) -> &'l Seen {
    look.items.iter().find(|item| matches!(item, Seen::Text { words: said, .. } if said == words)).unwrap_or_else(|| panic!("no {words}: {look:#?}"))
}

fn buttons(look: &Look) -> Vec<&Seen> {
    look.items.iter().filter(|item| matches!(item, Seen::Button { .. })).collect()
}

fn tables(look: &Look) -> Vec<&Seen> {
    look.items.iter().filter(|item| matches!(item, Seen::Table { .. })).collect()
}

/// The Messages page's marketing mail, invented: a band of the sender's orange round a centred heading, a line and
/// the mail's button, then three columns of 192.
const MARKETING: &str = r##"<div style="background:#FF6A00;text-align:center">
    <h1>Light up the long evenings.</h1><p>Winter lamps, 30 % off for 48 hours.</p>
    <a href="https://brightshelf.example/sale" style="display:inline-block;background:#FFE600;padding:10px 16px">Shop the sale</a></div>
    <table align="center"><tr><td width="192">Arc lamp</td><td width="192">Dome lamp</td><td width="192">Reed lamp</td></tr></table>"##;

/// The receipt: a band of green with a white line, the words, a table of two columns of 280 in 560, the mail's button.
const RECEIPT: &str = r##"<table width="100%"><tr><td bgcolor="#2D5B45" align="center"><h1>Thank you, Odile.</h1></td></tr></table>
    <p>Your order KC-20418 is packed and on its way.</p>
    <table width="560"><tr><td width="280">Delivered to</td><td width="280">Paid with</td></tr><tr><td>Odile Brannock</td><td>Card ending 4417</td></tr></table>
    <p><a href="https://kettle.example/track" role="button" style="background:#2D5B45;padding:10px 16px">Track parcel</a></p>"##;

/// The forge notification: a table of the mail's own widths with a status cell on its own colour, the mail's button.
const FORGE: &str = r##"<p>The run finished.</p>
    <table width="420"><tr><th>Job</th><th>Result</th></tr><tr><td>build</td><td bgcolor="#2D5B45">passed</td></tr><tr><td>test</td><td>failed</td></tr></table>
    <p><a href="https://forge.example/run/7" style="display:inline-block;background-color:#4B3FB5;padding:10px 16px">Open run</a></p>"##;

/// The newsletter: a centred heading, an issue's words, a button-link of no colour of its own.
const NEWSLETTER: &str = r##"<center><h1>The Allotment Letter</h1><p>Issue 41 · November</p></center>
    <p>Frost is coming early this year.</p><p><a role="button" href="https://letter.example/41">Read it on the site</a></p>"##;

/// The mailing list, the pictures, the invitation and the bounce: plain mail that keeps no look of its own.
const PLAIN_KINDS: [&str; 4] = [
    "<p>On the list, Mira wrote:</p><blockquote><p>The patch looks good.</p></blockquote><p>Merged, thanks.</p>",
    r##"<p>The plot in spring:</p><img src="cid:plot@example.org" alt="The plot" width="320" height="240"><p>Two more attached.</p>"##,
    "<p>You are invited to the allotment meeting on Thursday.</p><p>Bring the rota.</p>",
    "<p>Your message could not be delivered to odile@example.org.</p><pre>550 5.1.1 No such user</pre>",
];

#[test]
fn the_marketing_mail_is_a_band_of_its_orange_with_its_button_and_three_columns_as_sent() {
    let look = laid_out(MARKETING, 684.0, Mode::Dark);
    // One band of the sender's orange across the room, its blocks 32 in, 14 under its top; their words in the near
    // black that reads on it.
    let [(band, colour)] = look.grounds.as_slice() else { panic!("{:#?}", look.grounds) };
    assert_eq!(*colour, rgb(0xFF6A00));
    assert_eq!((band.left(), band.width()), (0.0, 684.0));
    let Seen::Text { rect, colour, halign, shift, .. } = text(&look, "Light up the long evenings.") else { return };
    assert_eq!((*colour, *halign), (rgb(DARK), egui::Align::Center));
    assert_eq!((rect.left(), rect.top() - band.top()), (32.0, 14.0));
    assert_eq!(*shift, (684.0 - 64.0) / 2.0, "the words stand in the band's middle");
    // The mail's button: its own yellow, its words in the near black, in the middle of the band.
    let [Seen::Button { rect, words, fill, ink }] = buttons(&look).as_slice() else { panic!("{look:#?}") };
    assert_eq!((words.as_str(), *fill, *ink), ("Shop the sale", Some(rgb(0xFFE600)), rgb(DARK)));
    assert_eq!(rect.height(), 36.0);
    assert!((rect.center().x - 342.0).abs() <= 1.0, "{rect:?}");
    assert!(rect.bottom() + 16.0 <= band.bottom() + 0.5, "the band holds the button and 16 under it");
    // Three columns of 192 each, the table in the middle of the room.
    let [Seen::Table { rect, widths, .. }] = tables(&look).as_slice() else { panic!("{look:#?}") };
    assert_eq!(widths, &vec![192.0; 3]);
    assert!((rect.center().x - 342.0).abs() <= 1.0, "{rect:?}");
    assert!(rect.top() >= band.bottom(), "the table stands under the band");
}

#[test]
fn the_receipt_keeps_its_green_band_its_white_words_and_a_table_of_its_width() {
    let look = laid_out(RECEIPT, 684.0, Mode::Light);
    let [(_, colour)] = look.grounds.as_slice() else { panic!("{:#?}", look.grounds) };
    assert_eq!(*colour, rgb(0x2D5B45));
    let Seen::Text { colour, halign, .. } = text(&look, "Thank you, Odile.") else { return };
    assert_eq!((*colour, *halign), (rgb(WHITE), egui::Align::Center));
    // The words after the band are the app's own again.
    let Seen::Text { colour, halign, .. } = text(&look, "Your order KC-20418 is packed and on its way.") else { return };
    assert_eq!((*colour, *halign), (Theme::new(Mode::Light, TextScale::default()).palette.ink, egui::Align::Min));
    let [Seen::Table { rect, wide, widths, .. }] = tables(&look).as_slice() else { panic!("{look:#?}") };
    assert_eq!((rect.width(), *wide), (560.0, 560.0));
    assert_eq!(widths.first(), widths.last(), "two columns alike");
    let [Seen::Button { words, fill, ink, .. }] = buttons(&look).as_slice() else { panic!("{look:#?}") };
    assert_eq!((words.as_str(), *fill, *ink), ("Track parcel", Some(rgb(0x2D5B45)), rgb(WHITE)));
}

#[test]
fn the_forge_mail_keeps_its_table_s_width_and_its_status_cell_s_colour() {
    let look = laid_out(FORGE, 684.0, Mode::Dark);
    assert!(look.grounds.is_empty());
    let [Seen::Table { rect, fills, .. }] = tables(&look).as_slice() else { panic!("{look:#?}") };
    assert_eq!(rect.width(), 420.0);
    assert_eq!(fills, &vec![vec![None, None], vec![None, Some(rgb(0x2D5B45))], vec![None, None]]);
    let [Seen::Button { words, fill, ink, .. }] = buttons(&look).as_slice() else { panic!("{look:#?}") };
    assert_eq!((words.as_str(), *fill, *ink), ("Open run", Some(rgb(0x4B3FB5)), rgb(WHITE)));
}

#[test]
fn the_newsletter_s_heading_stands_in_the_middle_and_its_button_link_is_a_secondary_button() {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let look = laid_out(NEWSLETTER, 684.0, Mode::Dark);
    let Seen::Text { halign, colour, .. } = text(&look, "The Allotment Letter") else { return };
    assert_eq!((*halign, *colour), (egui::Align::Center, theme.palette.ink));
    let Seen::Text { halign, .. } = text(&look, "Frost is coming early this year.") else { return };
    assert_eq!(*halign, egui::Align::Min);
    let [Seen::Button { rect, words, fill, ink }] = buttons(&look).as_slice() else { panic!("{look:#?}") };
    assert_eq!((words.as_str(), *fill, *ink), ("Read it on the site", None, theme.palette.ink));
    assert_eq!((rect.left(), rect.height()), (0.0, 36.0), "a left-aligned button hugs its words");
}

#[test]
fn the_list_the_pictures_the_invitation_and_the_bounce_keep_no_look_of_their_own() {
    for html in PLAIN_KINDS {
        for mode in [Mode::Light, Mode::Dark] {
            let look = laid_out(html, 684.0, mode);
            assert!(look.grounds.is_empty() && buttons(&look).is_empty(), "{html}");
            for item in texts(&look) {
                let Seen::Text { halign, shift, .. } = item else { continue };
                assert_eq!((*halign, *shift), (egui::Align::Min, 0.0), "{html}");
            }
        }
    }
}

#[test]
fn a_right_aligned_block_and_picture_stand_at_the_right() {
    let look = laid_out(r##"<div align="right"><p>Kind regards</p><img alt="signature" width="120" height="40"></div>"##, 600.0, Mode::Light);
    let Seen::Text { halign, shift, .. } = text(&look, "Kind regards") else { return };
    assert_eq!((*halign, *shift), (egui::Align::Max, 600.0));
    let Some(Seen::Other { rect }) = look.items.last() else { panic!("{look:#?}") };
    assert_eq!(rect.right(), 600.0, "{rect:?}");
}

#[test]
fn every_word_on_a_kept_background_reaches_the_contrast_the_app_asks_for() {
    // Review focus 4: backgrounds the colour of the text, and the app's own ink in either mode.
    let palette_inks = [Mode::Light, Mode::Dark].map(|mode| Theme::new(mode, TextScale::default()).palette.ink);
    for colour in ["#191919", "#D4D4D4", "#FF0000", "#2D5B45", "#FFE600", "#22A8E0"] {
        let html =
            format!(r##"<div style="background:{colour};color:{colour}"><p>hidden words</p><a role=button href="https://x.example/">go</a></div>"##);
        for mode in [Mode::Light, Mode::Dark] {
            let look = laid_out(&html, 600.0, mode);
            let ground = look.grounds.first().map(|(_, colour)| *colour);
            for item in &look.items {
                let (Some(ground), Seen::Text { colour: ink, .. } | Seen::Button { ink, .. }) = (ground, item) else { continue };
                assert!(crate::theme::contrast(*ink, ground) >= 4.5, "{colour} in {mode:?}: {item:?}");
            }
            if ground.is_none() {
                // Dropped: the words are the app's own on the app's own ground.
                let Seen::Text { colour: ink, .. } = text(&look, "hidden words") else { return };
                assert!(palette_inks.contains(ink), "{colour}: {ink:?}");
            }
        }
    }
}

#[test]
fn ten_thousand_nested_tables_ninety_nine_million_wide_lay_out_within_the_room() {
    // Review focus 4: bounded layout whatever the widths and the nesting.
    let open = r##"<table width="99999999" bgcolor="#2D5B45" align="center"><tr><td width="99999999">x</td><td width="99999999">"##;
    let html = format!("{}deep{}", open.repeat(10_000), "</td></tr></table>".repeat(10_000));
    let look = laid_out(&html, 600.0, Mode::Light);
    let [Seen::Table { rect, wide, .. }] = tables(&look).as_slice() else { panic!("{look:#?}") };
    assert!(rect.left() >= 0.0 && rect.right() <= 600.0, "{rect:?}");
    assert!(*wide <= 600.0 || (*wide - rect.width()) > 0.5, "wider than its room only to scroll sideways: {wide}");
    let single = laid_out(r##"<table width="99999999"><tr><td>a</td><td width="99999999">b</td></tr></table>"##, 600.0, Mode::Light);
    let [Seen::Table { rect, wide, widths, .. }] = tables(&single).as_slice() else { panic!("{single:#?}") };
    assert!(rect.width() <= 600.0 && (599.0..=600.0).contains(wide), "{rect:?} {wide} {widths:?}");
    assert!(widths.iter().all(|width| *width >= 1.0), "{widths:?}");
}

#[test]
fn a_mebibyte_of_centred_text_lays_out_centred_once() {
    // Review focus 4: CSS `text-align` on 1 MiB of text.
    let words = "Long evenings, winter lamps. ".repeat(1024 * 1024 / 29);
    let look = laid_out(&format!(r##"<p style="text-align:center">{words}</p>"##), 640.0, Mode::Light);
    let [Seen::Text { halign, shift, .. }] = texts(&look).as_slice() else { panic!("one paragraph") };
    assert_eq!((*halign, *shift), (egui::Align::Center, 320.0));
    assert!(look.height > 10_000.0, "{}", look.height);
}
