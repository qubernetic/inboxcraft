// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The inside of a fitted message: what its layouts keep, and where its table's columns stand.

use egui::vec2;
use egui_kittest::Harness;
use inboxcraft_html::Document;

use crate::save::NoSave;
use crate::theme::{Mode, TextScale, Theme};

use super::{KEPT_GLYPHS, kept_now};

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
                    super::blocks(ui, &theme, &reader, id, document, &document.blocks, 0, &mut textures, &mut drawn);
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
