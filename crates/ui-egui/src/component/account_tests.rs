// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::{Color32, Rect, Shape, vec2};

use super::account_bar;
use crate::component::tests::builder;
use crate::theme::{Mode, TextScale, Theme};

/// A filled rect painted: its rect, fill, largest corner radius and whether it has a stroke.
type Fill = (Rect, Color32, f32, bool);

/// The account bars of `accounts` drawn in `theme`: each one's rect and the filled rects painted inside it.
fn bars(theme: Theme, accounts: &'static [u64]) -> Vec<(Rect, Vec<Fill>)> {
    let mut harness = builder().with_size(vec2(200.0, 300.0)).build_ui_state(
        move |ui, rects: &mut Vec<Rect>| {
            *rects = accounts.iter().map(|account| account_bar(ui, &theme, *account).rect).collect();
        },
        Vec::new(),
    );
    harness.run();
    let mut shapes = Vec::new();
    for clipped in &harness.output().shapes {
        if let Shape::Rect(rect) = &clipped.shape {
            shapes.push((rect.rect, rect.fill, f32::from(rect.corner_radius.ne), rect.stroke.width > 0.0));
        }
    }
    harness.state().iter().map(|bar| (*bar, shapes.iter().filter(|(rect, ..)| bar.expand(0.01).contains_rect(*rect)).copied().collect())).collect()
}

#[test]
fn an_account_bar_looks_as_its_sheet_says_for_each_of_the_six_accounts() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let colours = theme.palette.accounts;
        for (account, (rect, painted)) in bars(theme, &[0, 1, 2, 3, 4, 5]).into_iter().enumerate() {
            let at = format!("{mode:?} acct{}: {rect:?} {painted:?}", account + 1);
            assert!((rect.width() - 3.0).abs() < 0.01 && (rect.height() - 12.0).abs() < 0.01, "3 × 12: {at}");
            assert_eq!(painted, vec![(rect, colours[account], 0.0, false)], "one square fill in the account's colour: {at}");
        }
    }
}

#[test]
fn the_seventh_account_takes_the_first_colour_again_and_the_bar_keeps_its_size_at_130_percent() {
    let theme = Theme::new(Mode::Light, TextScale::ALL[4]);
    let colours = theme.palette.accounts;
    let drawn = bars(theme, &[6, 13, u64::MAX]);
    for ((rect, painted), colour) in drawn.into_iter().zip([colours[0], colours[1], colours[(u64::MAX % 6) as usize]]) {
        assert!((rect.width() - 3.0).abs() < 0.01 && (rect.height() - 12.0).abs() < 0.01, "a fixed mark: {rect:?}");
        assert_eq!(painted.iter().map(|(_, fill, ..)| *fill).collect::<Vec<_>>(), vec![colour], "{rect:?}");
    }
}
