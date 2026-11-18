// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The inside of a fitted message: what its layouts keep, and where its table's columns stand.

use egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use inboxcraft_html::Document;

use crate::save::NoSave;
use crate::theme::{Mode, TextScale, Theme};

use super::{KEPT_GLYPHS, kept_now, made_now};

/// `html` as the reader's cleaner gives it.
fn display(html: &str) -> Document {
    inboxcraft_html::display(html, &inboxcraft_html::Options::default())
}

#[test]
fn opening_nine_large_messages_keeps_the_layout_of_the_one_shown_only() {
    // Nine messages of one long paragraph each, opened one after another.
    let documents: Vec<Document> =
        (0..9).map(|index| display(&format!("<p>{index} {}</p>", "The minutes of the allotment meeting, read again. ".repeat(4_000)))).collect();
    let theme = Theme::new(Mode::Light, TextScale::default());
    let mut harness =
        Harness::builder().with_size(vec2(800.0, 600.0)).with_max_steps(16).with_context_setup(crate::theme::install_fonts).build_ui_state(
            move |ui, which: &mut usize| {
                let reader = crate::reader::Reader::new(Box::<crate::fetch::Offline>::default(), Box::new(NoSave));
                let Some(document) = documents.get(*which) else { return };
                crate::component::scroll::show(ui, &theme, egui::ScrollArea::vertical(), |ui| {
                    let mut textures = super::Textures::default();
                    let mut drawn = super::Drawn::default();
                    let id = u64::try_from(*which).unwrap_or(0) + 1;
                    super::blocks(ui, &theme, &reader, id, 0, document, &document.blocks, 0, &mut textures, &mut drawn);
                });
            },
            0_usize,
        );
    harness.run();
    for which in 1..9 {
        *harness.state_mut() = which;
        harness.run();
        let (count, glyphs) = kept_now(&harness.ctx);
        assert_eq!(count, 1, "message {which}: {count} layouts kept");
        assert!(glyphs > 100_000 && glyphs <= KEPT_GLYPHS, "message {which}: {glyphs} letters kept");
    }
}

#[test]
fn a_thread_over_the_cap_keeps_its_letters_under_it_and_lays_nothing_out_again_at_rest() {
    // Three unfolded messages of about 1.4 million letters each, one under another: more than the kept layouts hold.
    let documents: Vec<Document> =
        (0..3).map(|index| display(&format!("<p>{index} {}</p>", "The minutes of the allotment meeting, read again. ".repeat(33_000)))).collect();
    let theme = Theme::new(Mode::Light, TextScale::default());
    let mut harness =
        Harness::builder().with_size(vec2(800.0, 600.0)).with_max_steps(16).with_context_setup(crate::theme::install_fonts).build_ui_state(
            move |ui, fitted: &mut Vec<bool>| {
                let reader = crate::reader::Reader::new(Box::<crate::fetch::Offline>::default(), Box::new(NoSave));
                crate::component::scroll::show(ui, &theme, egui::ScrollArea::vertical(), |ui| {
                    let mut textures = super::Textures::default();
                    let mut drawn = super::Drawn::default();
                    fitted.clear();
                    for (id, document) in (1_u64..).zip(&documents) {
                        fitted.push(super::blocks(ui, &theme, &reader, id, 1, document, &document.blocks, 0, &mut textures, &mut drawn));
                    }
                });
            },
            Vec::new(),
        );
    harness.run();
    let (count, glyphs) = kept_now(&harness.ctx);
    assert!(glyphs > 2_000_000 && glyphs <= KEPT_GLYPHS, "{count} layouts kept, {glyphs} letters");
    // The one that does not fit beside the others is shown plain, and says so (the reader's body draws the notice).
    assert_eq!(harness.state(), &vec![true, true, false]);
    // At rest nothing is laid out again: not the two kept, not the one refused.
    let made = made_now(&harness.ctx);
    for frame in 0..10 {
        harness.step();
        assert_eq!(made_now(&harness.ctx), made, "frame {frame}: laid out again");
        assert_eq!(kept_now(&harness.ctx), (count, glyphs), "frame {frame}");
    }
}

#[test]
fn a_message_shown_plain_over_the_cap_is_fitted_again_once_a_big_one_is_folded() {
    // Three messages of about 1.4 million letters each; the first is folded once the third has been shown plain.
    let documents: Vec<Document> =
        (0..3).map(|index| display(&format!("<p>{index} {}</p>", "The minutes of the allotment meeting, read again. ".repeat(33_000)))).collect();
    let theme = Theme::new(Mode::Light, TextScale::default());
    let mut harness =
        Harness::builder().with_size(vec2(800.0, 600.0)).with_max_steps(16).with_context_setup(crate::theme::install_fonts).build_ui_state(
            move |ui, (first, fitted): &mut (usize, Vec<bool>)| {
                let reader = crate::reader::Reader::new(Box::<crate::fetch::Offline>::default(), Box::new(NoSave));
                crate::component::scroll::show(ui, &theme, egui::ScrollArea::vertical(), |ui| {
                    let mut textures = super::Textures::default();
                    let mut drawn = super::Drawn::default();
                    fitted.clear();
                    for (id, document) in (1_u64..).zip(&documents).skip(*first) {
                        fitted.push(super::blocks(ui, &theme, &reader, id, 1, document, &document.blocks, 0, &mut textures, &mut drawn));
                    }
                });
            },
            (0, Vec::new()),
        );
    harness.run();
    assert_eq!(harness.state().1, vec![true, true, false]);
    harness.state_mut().0 = 1;
    harness.run();
    // The folded one's room comes back: the third is fitted, and the first's letters are let go.
    assert_eq!(harness.state().1, vec![true, true]);
    let (count, glyphs) = kept_now(&harness.ctx);
    assert!(count == 2 && glyphs <= KEPT_GLYPHS, "{count} layouts kept, {glyphs} letters");
    let made = made_now(&harness.ctx);
    for frame in 0..10 {
        harness.step();
        assert_eq!(made_now(&harness.ctx), made, "frame {frame}: laid out again");
        assert_eq!(harness.state().1, vec![true, true], "frame {frame}");
    }
}

#[test]
fn a_message_laid_out_again_is_drawn_from_its_new_blocks_wherever_they_lie() {
    // The blocks change in place (the same list at the same address, as a freed list's place is taken again by the
    // next one), and the reader gives the message a new generation: the new words are drawn.
    struct Shown {
        document: Document,
        generation: u64,
    }
    let theme = Theme::new(Mode::Light, TextScale::default());
    let reader = crate::reader::Reader::new(Box::<crate::fetch::Offline>::default(), Box::new(NoSave));
    let mut harness = Harness::builder().with_size(vec2(800.0, 600.0)).with_context_setup(crate::theme::install_fonts).build_ui_state(
        move |ui, shown: &mut Shown| {
            let mut textures = super::Textures::default();
            let mut drawn = super::Drawn::default();
            let list = &shown.document.blocks;
            super::blocks(ui, &theme, &reader, 1, shown.generation, &shown.document, list, 0, &mut textures, &mut drawn);
        },
        Shown { document: display("<p>The first words</p>"), generation: 1 },
    );
    harness.run();
    let _ = harness.get_by_label("The first words");
    let fresh = display("<p>The second words</p>");
    let state = harness.state_mut();
    if let (Some(slot), Some(block)) = (state.document.blocks.first_mut(), fresh.blocks.into_iter().next()) {
        *slot = block;
    }
    state.generation = 2;
    harness.run();
    let _ = harness.get_by_label("The second words");
}

thread_local! {
    /// Whether the next harness lays out a line of other letters before the message.
    static DECOY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[test]
fn a_message_keeps_its_letters_when_egui_builds_its_font_atlas_afresh() {
    // A kept layout's letters point into the font atlas as it was; egui builds it afresh when it fills or when the text
    // options change. The first frame lays the message out after a line of other letters, which take the atlas's
    // first places; after the atlas is built afresh without them, the message's letters land elsewhere.
    let html = "<p>Plain words, <b>bold words</b> and <a href=\"https://example.org/\">a link</a> in one paragraph.</p>";
    let document = display(html);
    let theme = Theme::new(Mode::Light, TextScale::default());
    let reader = crate::reader::Reader::new(Box::<crate::fetch::Offline>::default(), Box::new(NoSave));
    DECOY.with(|decoy| decoy.set(true));
    let mut harness = Harness::builder().with_size(vec2(640.0, 400.0)).with_context_setup(crate::theme::install_fonts).build_ui(move |ui| {
        if DECOY.with(std::cell::Cell::get) {
            ui.label(egui::RichText::new("ZQXJWVKYPBGM 0123456789 ¶§ÆØÅ").size(64.0));
        }
        let mut textures = super::Textures::default();
        let mut drawn = super::Drawn::default();
        super::blocks(ui, &theme, &reader, 1, 1, &document, &document.blocks, 0, &mut textures, &mut drawn);
    });
    harness.run();
    DECOY.with(|decoy| decoy.set(false));
    // Two changes of the text options: each makes egui build its font atlas afresh.
    for _ in 0..2 {
        harness.ctx.all_styles_mut(|style| style.visuals.text_options.font_hinting = !style.visuals.text_options.font_hinting);
        harness.run();
    }
    harness.run();
    fn walk(shape: &egui::Shape, out: &mut Vec<std::sync::Arc<egui::Galley>>) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, out)),
            egui::Shape::Text(text) => out.push(text.galley.clone()),
            _ => {}
        }
    }
    let mut texts = Vec::new();
    harness.output().shapes.iter().for_each(|clipped| walk(&clipped.shape, &mut texts));
    let painted: Vec<_> = texts.iter().filter(|galley| galley.text().contains("bold")).collect();
    assert!(!painted.is_empty(), "the paragraph is painted");
    for galley in painted {
        let now = harness.ctx.fonts_mut(|fonts| fonts.layout_job((*galley.job).clone()));
        let places = |galley: &egui::Galley| -> Vec<_> {
            galley.rows.iter().flat_map(|row| row.glyphs.iter().map(|glyph| (glyph.chr, glyph.uv_rect.min, glyph.uv_rect.max))).collect()
        };
        assert_eq!(places(galley), places(&now), "painted with the old atlas's places");
    }
}

#[test]
fn a_paragraph_of_ten_thousand_links_registers_only_those_near_sight() {
    // As many links as the cleaner keeps, in one paragraph: each frame only those within reach take the keyboard.
    let html = format!("<p>{}</p>", "<a href=\"https://allotments.example.org/rota\">rota</a> ".repeat(inboxcraft_html::MAX_LINKS));
    let document = display(&html);
    let theme = Theme::new(Mode::Light, TextScale::default());
    let reader = crate::reader::Reader::new(Box::<crate::fetch::Offline>::default(), Box::new(NoSave));
    let mut harness = Harness::builder().with_size(vec2(800.0, 600.0)).with_context_setup(crate::theme::install_fonts).build_ui(move |ui| {
        crate::component::scroll::show(ui, &theme, egui::ScrollArea::vertical().auto_shrink([false, false]), |ui| {
            let mut textures = super::Textures::default();
            let mut drawn = super::Drawn::default();
            super::blocks(ui, &theme, &reader, 1, 1, &document, &document.blocks, 0, &mut textures, &mut drawn);
        });
    });
    harness.run();
    let links = harness.query_all_by_role(egui::accesskit::Role::Link).count();
    // Three panes of 26-point rows, about fifteen links a row: about a thousand, not ten.
    assert!((50..2_000).contains(&links), "{links} links registered");
}
