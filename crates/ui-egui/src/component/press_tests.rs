// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What the toggles and choices share in their tests: a control held down by the pointer, or by Enter or Space while
//! it has the focus, read the way the state tests read it.

use egui::{Key, Rect, Ui, Vec2};
use egui_kittest::kittest::Queryable as _;

use super::Shown;
use super::tests::{Own, bring, own, painted, staged};
use crate::theme::{Mode, TextScale, Theme};

/// How a test draws one control.
pub(crate) type Draw = fn(&mut Ui, &Theme);

/// Where the part a test reads lies in the control's node `rect`, in `theme`.
pub(crate) type Part = fn(Rect, &Theme) -> Rect;

/// The control `label` that `draw` puts on a `surface` stage `size` large in `mode`, held down by the pointer (`key`
/// is `None`) or by `key` while it has the focus: its node rect and the look of its `part`.
pub(crate) fn held_by(mode: Mode, label: &'static str, key: Option<Key>, size: Vec2, part: Part, draw: Draw) -> (Rect, Own) {
    let theme = Theme::new(mode, TextScale::default());
    let mut harness = staged(theme, |p| p.surface, size, draw);
    match key {
        None => bring(&mut harness, label, Shown::Pressed),
        Some(key) => {
            bring(&mut harness, label, Shown::Focus);
            harness.key_down(key);
            harness.run();
        }
    }
    let rect = harness.get_by_label(label).rect();
    let at = part(rect, &theme);
    (rect, own(&painted(&harness, label), at))
}

/// Holds `label` down by Enter and by Space while it has the focus, in both modes: each gives the pointer's pressed
/// look (fill, edge, texts) on its `part`, which differs from the focused control at rest (a key that presses
/// nothing, `A`), and the focus ring stays where `ring` says from the node's rect.
pub(crate) fn keys_press_as_the_pointer(label: &'static str, size: Vec2, part: Part, ring: Part, draw: Draw) {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let (_, by_pointer) = held_by(mode, label, None, size, part, draw);
        let (_, at_rest) = held_by(mode, label, Some(Key::A), size, part, draw);
        for key in [Key::Enter, Key::Space] {
            let (rect, look) = held_by(mode, label, Some(key), size, part, draw);
            let what = format!("{mode:?} {label} {key:?}: {look:#?} pointer {by_pointer:#?} rest {at_rest:#?}");
            assert_eq!((look.fill, look.edge, &look.texts), (by_pointer.fill, by_pointer.edge, &by_pointer.texts), "{what}");
            assert_ne!((look.fill, look.edge, &look.texts), (at_rest.fill, at_rest.edge, &at_rest.texts), "pressed differs from rest: {what}");
            assert_eq!(look.ring, Some((ring(rect, &theme), 1.0, theme.palette.cyan_edge)), "{what}");
        }
    }
}
