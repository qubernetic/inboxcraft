// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What the fitted view keeps of a message's look: backgrounds, alignment, widths and buttons, and that hostile
//! looks stay bounded and run nothing.

use crate::{
    Align, Block, DARK_INK, Document, LEAST_CONTRAST, LIGHT_INK, LinkTarget, Look, MAX_SIDE, MAX_TABLE_DEPTH, Options, Paint, Rgb, Width, display,
};

fn doc(html: &str) -> Document {
    display(html, &Options::default())
}

fn rgb(hex: u32) -> Rgb {
    let [_, r, g, b] = hex.to_be_bytes();
    Rgb { r, g, b }
}

/// The look of every block, in order, with its text (or `[table]`, `[img]`, `[hr]`).
fn looks(blocks: &[Block]) -> Vec<(String, Look)> {
    blocks
        .iter()
        .map(|block| match block {
            Block::Text { look, .. } => (block.text(), *look),
            Block::Image { look, .. } => ("[img]".to_owned(), *look),
            Block::Table { look, .. } => ("[table]".to_owned(), *look),
            Block::Rule { .. } => ("[hr]".to_owned(), Look::default()),
        })
        .collect()
}

fn background_of(d: &Document, text: &str) -> Option<Paint> {
    looks(&d.blocks).into_iter().find(|(said, _)| said == text).and_then(|(_, look)| look.background)
}

fn align_of(d: &Document, text: &str) -> Option<Align> {
    looks(&d.blocks).into_iter().find(|(said, _)| said == text).map(|(_, look)| look.align)
}

#[test]
fn a_coloured_background_is_kept_with_the_ink_that_reads_on_it() {
    let d = doc(r##"<div style="background:#2D5B45"><h1>Thank you, Odile.</h1></div>
           <div style="background-color: #FF6A00 !important"><p>Winter lamps</p></div>
           <table><tr><td bgcolor="FFE600"><p>Sale</p></td></tr></table>"##);
    assert_eq!(background_of(&d, "Thank you, Odile."), Some(Paint { fill: rgb(0x2D5B45), ink: LIGHT_INK }));
    assert_eq!(background_of(&d, "Winter lamps"), Some(Paint { fill: rgb(0xFF6A00), ink: DARK_INK }));
    // A table of one column is laid out as its cell's blocks: they keep the cell's background.
    assert_eq!(background_of(&d, "Sale"), Some(Paint { fill: rgb(0xFFE600), ink: DARK_INK }));
}

#[test]
fn a_white_grey_or_black_background_is_the_sender_s_page_and_is_dropped() {
    for colour in ["#ffffff", "white", "#f4f4f4", "#191919", "black", "rgb(128, 128, 128)", "#f0f4ff"] {
        let d = doc(&format!(r##"<div style="background:{colour}"><p>words</p></div>"##));
        assert_eq!(background_of(&d, "words"), None, "{colour}");
    }
}

#[test]
fn a_background_no_ink_can_be_read_on_is_dropped() {
    // Pure red: neither the near black (4.4:1) nor white (4.0:1) reaches 4.5:1 on it.
    let d = doc(r##"<div bgcolor="red"><p>alarm</p></div><div style="background:rgb(255,0,0)"><p>again</p></div>"##);
    assert_eq!(background_of(&d, "alarm"), None);
    assert_eq!(background_of(&d, "again"), None);
}

#[test]
fn a_background_the_colour_of_its_text_never_hides_the_text() {
    // Review focus 4: text set in its background's own colour, to vanish. The text's colour is never read: a kept
    // background carries an ink that reaches 4.5:1 on it, else it is dropped.
    for colour in ["#2D5B45", "#191919", "#FFE600", "#ff0000", "#D4D4D4", "#22A8E0", "#4B3FB5", "#808000"] {
        let d = doc(&format!(r##"<div style="background:{colour};color:{colour}"><p>hidden words</p></div>"##));
        assert_eq!(d.blocks.len(), 1, "{colour}: the words are there");
        if let Some(paint) = background_of(&d, "hidden words") {
            assert!(paint.fill.contrast(paint.ink) >= LEAST_CONTRAST, "{colour}: {paint:?}");
            assert_ne!(paint.ink, paint.fill, "{colour}");
        }
    }
}

#[test]
fn a_background_ends_with_its_element_and_a_white_one_inside_clears_it() {
    let d =
        doc(r##"<div style="background:#2D5B45"><p>on green</p><div bgcolor="#ffffff"><p>on white</p></div><p>green again</p></div><p>after</p>"##);
    let green = Some(Paint { fill: rgb(0x2D5B45), ink: LIGHT_INK });
    assert_eq!(background_of(&d, "on green"), green);
    assert_eq!(background_of(&d, "on white"), None);
    assert_eq!(background_of(&d, "green again"), green);
    assert_eq!(background_of(&d, "after"), None);
    // Transparent says nothing: what is around it holds.
    let d = doc(r##"<div style="background:#2D5B45"><div style="background:transparent">still green</div></div>"##);
    assert_eq!(background_of(&d, "still green"), green);
}

#[test]
fn a_table_cell_keeps_its_background_and_its_blocks_do_not_repeat_it() {
    let d = doc(r##"<table bgcolor="#4B3FB5"><tr><td>run</td><td bgcolor="#2D5B45">passed</td><td bgcolor="#fff">plain</td></tr></table>"##);
    let Some(Block::Table { rows, .. }) = d.blocks.first() else { panic!("{:?}", d.blocks) };
    let cells: Vec<Option<Rgb>> = rows.iter().flatten().map(|cell| cell.background.map(|paint| paint.fill)).collect();
    assert_eq!(cells, [Some(rgb(0x4B3FB5)), Some(rgb(0x2D5B45)), None]);
    for cell in rows.iter().flatten() {
        assert!(looks(&cell.blocks).iter().all(|(_, look)| look.background.is_none()), "{cell:?}");
    }
}

#[test]
fn text_align_left_centre_and_right_are_kept_and_inherited() {
    let d = doc(r##"<p align="center">one</p><div style="text-align: right"><p>two</p><p style="text-align:left">three</p></div>
           <center><h2>four</h2></center><p>five</p><div style="text-align:-webkit-center"><img alt="six"></div>"##);
    assert_eq!(align_of(&d, "one"), Some(Align::Centre));
    assert_eq!(align_of(&d, "two"), Some(Align::Right));
    assert_eq!(align_of(&d, "three"), Some(Align::Left));
    assert_eq!(align_of(&d, "four"), Some(Align::Centre));
    assert_eq!(align_of(&d, "five"), Some(Align::Left));
    assert_eq!(align_of(&d, "[img]"), Some(Align::Centre));
}

#[test]
fn a_cell_s_alignment_holds_for_its_blocks_and_a_centred_table_stands_in_the_middle() {
    let d = doc(
        r##"<table align="center"><tr><td align="right">a</td><td>b</td></tr></table><table style="margin:0 auto"><tr><td>c</td><td>d</td></tr></table>"##,
    );
    let tables: Vec<&Block> = d.blocks.iter().filter(|block| matches!(block, Block::Table { .. })).collect();
    assert_eq!(tables.len(), 2);
    for table in &tables {
        let Block::Table { look, .. } = table else { continue };
        assert_eq!(look.align, Align::Centre);
    }
    let Some(Block::Table { rows, .. }) = tables.first() else { return };
    let aligns: Vec<Vec<Align>> = rows.iter().flatten().map(|cell| looks(&cell.blocks).into_iter().map(|(_, look)| look.align).collect()).collect();
    // The table's own `align` moves the table, not the words in its cells.
    assert_eq!(aligns, [vec![Align::Right], vec![Align::Left]]);
}

#[test]
fn table_and_cell_widths_are_kept_as_pixels_or_shares_within_their_caps() {
    let d = doc(r##"<table width="560"><tr><td width="280">a</td><td style="width: 50%">b</td></tr></table>
           <table style="width:100%"><tr><td width="99999999">c</td><td width="250%">d</td></tr></table>
           <table width="0"><tr><td width="-5">e</td><td width="1e9">f</td></tr></table>"##);
    let widths: Vec<(Option<Width>, Vec<Option<Width>>)> = d
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Table { width, rows, .. } => Some((*width, rows.iter().flatten().map(|cell| cell.width).collect())),
            _ => None,
        })
        .collect();
    assert_eq!(
        widths,
        [
            (Some(Width::Pixels(560)), vec![Some(Width::Pixels(280)), Some(Width::Share(50))]),
            (Some(Width::Share(100)), vec![Some(Width::Pixels(MAX_SIDE)), Some(Width::Share(100))]),
            (None, vec![None, None]),
        ]
    );
}

#[test]
fn a_link_styled_as_a_button_is_a_button_link_in_a_block_of_its_own() {
    let d = doc(
        r##"<p>Hello <a href="https://shop.example/sale" style="display:inline-block;background:#FFE600;padding:10px 16px">Shop the sale</a> now</p>
           <p><a role="button" href="https://parcel.example/track">Track parcel</a></p>
           <p><a href="https://plain.example/" style="padding:4px">plain link</a> and <a href="https://x.example/" style="background:#FFE600">marked</a></p>"##,
    );
    assert_eq!(
        looks(&d.blocks).into_iter().map(|(text, _)| text).collect::<Vec<_>>(),
        ["Hello", "Shop the sale", "now", "Track parcel", "plain link and marked"]
    );
    let buttons: Vec<(bool, Option<Paint>)> = d.links.iter().map(|link| (link.button, link.fill)).collect();
    assert_eq!(buttons, [(true, Some(Paint { fill: rgb(0xFFE600), ink: DARK_INK })), (true, None), (false, None), (false, None)]);
}

#[test]
fn a_button_with_a_script_action_is_a_link_that_goes_nowhere_and_nothing_of_the_script_is_kept() {
    // Review focus 4: a `<button>` with a `javascript:` action, alone, in a form and inside a link.
    let d = doc(r##"<button onclick="javascript:steal()" formaction="javascript:steal()">Claim</button>
           <form action="javascript:steal()"><button type=submit>Send</button></form>
           <a href="javascript:steal()"><button>Open</button></a>
           <a href="https://forge.example/run/7"><button style="background:#4B3FB5">Open run</button></a>"##);
    assert!(!format!("{d:?}").contains("steal"), "nothing of the script is kept");
    let links: Vec<(&str, bool, bool)> = d.links.iter().map(|link| (link.text.as_str(), link.button, link.target == LinkTarget::Refused)).collect();
    assert_eq!(links, [("Claim", true, true), ("Send", true, true), ("Open", true, true), ("Open run", true, false)]);
    assert_eq!(d.links.last().and_then(|link| link.fill).map(|paint| paint.fill), Some(rgb(0x4B3FB5)));
    // Each button's words are a block of their own, all of whose words are the button.
    for block in &d.blocks {
        let Block::Text { spans, .. } = block else { continue };
        let link = spans.first().and_then(|span| span.link);
        assert!(link.is_some() && spans.iter().all(|span| span.link == link), "{spans:?}");
    }
}

#[test]
fn ten_thousand_nested_tables_as_wide_as_can_be_stay_bounded() {
    // Review focus 4: 10 000 nested tables, each `width="99999999"` on a coloured ground.
    let open = r##"<table width="99999999" bgcolor="#2D5B45" align="center"><tr><td width="99999999" style="text-align:center">x</td><td>"##;
    let html = format!("{}deep{}", open.repeat(10_000), "</td></tr></table>".repeat(10_000));
    let d = doc(&html);
    fn deepest(blocks: &[Block], depth: usize) -> usize {
        blocks
            .iter()
            .map(|block| match block {
                Block::Table { width, rows, .. } => {
                    assert!(matches!(width, Some(Width::Pixels(MAX_SIDE)) | None), "{width:?}");
                    for cell in rows.iter().flatten() {
                        assert!(matches!(cell.width, Some(Width::Pixels(MAX_SIDE)) | None), "{:?}", cell.width);
                    }
                    rows.iter().flatten().map(|cell| deepest(&cell.blocks, depth + 1)).max().unwrap_or(depth + 1)
                }
                _ => depth,
            })
            .max()
            .unwrap_or(depth)
    }
    assert!(deepest(&d.blocks, 0) <= MAX_TABLE_DEPTH);
    assert!(format!("{d:?}").contains("deep"), "the innermost words stay");
}

#[test]
fn text_align_on_a_mebibyte_of_text_and_a_mebibyte_of_style_is_read_in_one_pass() {
    // Review focus 4: CSS `text-align` on 1 MiB of text, and a style attribute of a mebibyte.
    let words = "Long evenings, winter lamps. ".repeat(1024 * 1024 / 29);
    let style = format!("text-align:center;{}", "color:red;".repeat(100_000));
    let html = format!(r##"<div style="{style}"><p>{words}</p></div>"##);
    let d = doc(&html);
    assert_eq!(d.blocks.len(), 1);
    assert!(looks(&d.blocks).iter().all(|(_, look)| look.align == Align::Centre));
}

#[test]
fn a_row_whose_end_tags_are_left_out_keeps_its_own_look_and_the_table_s() {
    // `</td>` and `</tr>` may be left out: each `<tr>` closes the row before it before it takes its own look.
    let d = doc(r##"<table bgcolor="#2D5B45"><tr><td>a<td>b<tr bgcolor="#4B3FB5" align="right"><td>c<td>d<tr><td>e<td>f</table>"##);
    let Some(Block::Table { rows, .. }) = d.blocks.first() else { panic!("{:?}", d.blocks) };
    let seen: Vec<Vec<(Option<Rgb>, Option<Align>)>> = rows
        .iter()
        .map(|row| {
            row.iter().map(|cell| (cell.background.map(|paint| paint.fill), looks(&cell.blocks).first().map(|(_, look)| look.align))).collect()
        })
        .collect();
    let green = (Some(rgb(0x2D5B45)), Some(Align::Left));
    let purple = (Some(rgb(0x4B3FB5)), Some(Align::Right));
    assert_eq!(seen, [vec![green, green], vec![purple, purple], vec![green, green]]);
}
