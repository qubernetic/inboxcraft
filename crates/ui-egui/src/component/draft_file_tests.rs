// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::accesskit::Role;
use egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

use super::{DraftFile, draft_file_with};
use crate::component::Shown;
use crate::component::gallery::keymap;
use crate::component::tests::{bring, own, painted, staged};
use crate::theme::{Mode, TextScale, Theme};

/// The file's row of the sheet, 320 wide, on `surface`, in `shown` (rest when `None`).
fn row(mode: Mode, file: DraftFile<'static>, shown: Option<Shown>) -> Harness<'static> {
    let theme = Theme::new(mode, TextScale::default());
    let keymap = keymap();
    let mut harness = staged(
        theme,
        |p| p.surface,
        vec2(400.0, 120.0),
        move |ui, theme| {
            draft_file_with(ui, theme, &keymap, &file, 320.0, None);
        },
    );
    if let Some(shown) = shown {
        bring(&mut harness, "shop-front-quote.pdf, 184 KB", shown);
    }
    harness
}

const QUOTE: DraftFile<'static> = DraftFile { name: "shop-front-quote.pdf", size: "184 KB", too_large: false };

#[test]
fn a_file_on_a_draft_looks_as_its_sheet_says_in_every_state() {
    for mode in [Mode::Dark, Mode::Light] {
        let p = Theme::new(mode, TextScale::default()).palette;
        let label = "shop-front-quote.pdf, 184 KB";
        let rest = row(mode, QUOTE, None);
        let rect = rest.get_by_role_and_label(Role::ListItem, label).rect();
        assert_eq!(rect.size(), vec2(320.0, 44.0), "{mode:?}");
        let look = own(&painted(&rest, label), rect);
        assert_eq!(look.edge, Some((1.0, p.line, false)), "{mode:?}");
        assert!(
            look.texts.iter().any(|(text, colour, family, _)| text == "shop-front-quote.pdf" && *colour == p.ink && family.contains("Mono")),
            "{mode:?} {:?}",
            look.texts
        );
        assert!(look.texts.iter().any(|(text, colour, ..)| text == "184 KB" && *colour == p.muted), "{mode:?}");
        let hover = row(mode, QUOTE, Some(Shown::Hover));
        assert_eq!(own(&painted(&hover, label), rect).fill, Some(p.stage), "{mode:?} hover");
        let focus = row(mode, QUOTE, Some(Shown::Focus));
        assert_eq!(own(&painted(&focus, label), rect).ring.map(|(_, _, colour)| colour), Some(p.cyan_edge), "{mode:?} focus");
        let large = DraftFile { name: "shop-front-quote.pdf", size: "184 KB", too_large: true };
        let large = row(mode, large, None);
        let look = own(&painted(&large, label), rect);
        assert_eq!(look.edge.map(|(_, colour, _)| colour), Some(p.minus), "{mode:?} too large");
        assert!(look.texts.iter().any(|(text, colour, ..)| text == "184 KB" && *colour == p.minus), "{mode:?}");
    }
}

#[test]
fn a_long_name_is_cut_and_remove_is_a_link_named_for_its_file() {
    let long = DraftFile { name: "the-whole-open-day-with-every-picture-taken-on-the-day.zip", size: "48 MB", too_large: true };
    let harness = row(Mode::Dark, long, None);
    let label = "the-whole-open-day-with-every-picture-taken-on-the-day.zip, 48 MB";
    let rect = harness.get_by_role_and_label(Role::ListItem, label).rect();
    let look = own(&painted(&harness, label), rect);
    // Cut with its ellipsis inside the row: nothing reaches past it.
    assert!(!look.overflows, "{:?}", look.texts);
    let remove = harness.get_by_role_and_label(Role::Button, "Remove the-whole-open-day-with-every-picture-taken-on-the-day.zip").rect();
    assert!(rect.contains_rect(remove));
}
