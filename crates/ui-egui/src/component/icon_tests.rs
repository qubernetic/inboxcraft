// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::{icon, name};
use crate::component::tests::{Own, icons, own, painted, staged};
use crate::iconset::IconId;
use crate::theme::{Mode, TextScale, Theme};

/// The icon's sheet, in its order: each state's name, the id the code gives it and the glyph of Phosphor Regular.
const SHEET: [(&str, IconId, char); 45] = [
    ("inbox", IconId::Inbox, '\u{e4aa}'),
    ("done", IconId::Done, '\u{e184}'),
    ("archive", IconId::Archive, '\u{e00c}'),
    ("delete", IconId::Delete, '\u{e4a6}'),
    ("snooze", IconId::Snooze, '\u{e19a}'),
    ("pin", IconId::Pin, '\u{e3e2}'),
    ("move", IconId::Move, '\u{e25a}'),
    ("reply", IconId::Reply, '\u{e024}'),
    ("reply-all", IconId::ReplyAll, '\u{e03a}'),
    ("forward", IconId::Forward, '\u{e026}'),
    ("send", IconId::Send, '\u{e398}'),
    ("search", IconId::Search, '\u{e30c}'),
    ("compose", IconId::Compose, '\u{ebc6}'),
    ("calendar", IconId::Calendar, '\u{e10a}'),
    ("attach", IconId::Attach, '\u{e39a}'),
    ("settings", IconId::Settings, '\u{e272}'),
    ("notify", IconId::Notify, '\u{e0ce}'),
    ("people", IconId::People, '\u{e4d6}'),
    ("flag", IconId::Flag, '\u{e244}'),
    ("more", IconId::More, '\u{e1fe}'),
    ("file", IconId::File, '\u{e230}'),
    ("failed", IconId::Failed, '\u{e4f8}'),
    ("waiting", IconId::Waiting, '\u{e18a}'),
    ("skipped", IconId::Skipped, '\u{e32c}'),
    ("home", IconId::Home, '\u{e2c2}'),
    ("mail", IconId::Mail, '\u{e218}'),
    ("add", IconId::Add, '\u{e3d4}'),
    ("junk", IconId::Junk, '\u{e4e2}'),
    ("mark-read", IconId::MarkRead, '\u{e21a}'),
    ("lock", IconId::Lock, '\u{e308}'),
    ("palette", IconId::Palette, '\u{e6c8}'),
    ("at", IconId::At, '\u{e0ac}'),
    ("funnel", IconId::Funnel, '\u{e266}'),
    ("keyboard", IconId::Keyboard, '\u{e2d8}'),
    ("shield", IconId::Shield, '\u{e40c}'),
    ("info", IconId::Info, '\u{e2ce}'),
    ("close", IconId::Close, '\u{e4f6}'),
    ("back", IconId::Back, '\u{e058}'),
    ("caret", IconId::Caret, '\u{e136}'),
    ("newspaper", IconId::Newspaper, '\u{e344}'),
    ("previous", IconId::Previous, '\u{e138}'),
    ("next", IconId::Next, '\u{e13a}'),
    ("globe", IconId::Globe, '\u{e288}'),
    ("changed", IconId::Changed, '\u{e094}'),
    ("older", IconId::Older, '\u{e1a0}'),
];

/// The icon `id` named `label` on a `surface` stage: its rect, its own look and its role.
fn icon_in(mode: Mode, scale: TextScale, id: IconId, label: &'static str) -> (egui::Rect, Own, Role) {
    let theme = Theme::new(mode, scale);
    let harness = staged(
        theme,
        |p| p.surface,
        egui::vec2(100.0, 80.0),
        move |ui, theme| {
            icon(ui, theme, id, label);
        },
    );
    let node = harness.get_by_label(label);
    let rect = node.rect();
    (rect, own(&painted(&harness, label), rect), node.accesskit_node().role())
}

#[test]
fn the_sheet_holds_every_icon_once_in_the_codes_order() {
    assert_eq!(SHEET.map(|(_, id, _)| id).as_slice(), &IconId::ALL[..IconId::SHEET]);
    for (state, id, _) in SHEET {
        assert_eq!(name(id), state, "the sheet's name of {id:?}");
    }
}

#[test]
fn an_icon_looks_as_its_sheet_says_in_every_state() {
    for mode in [Mode::Dark, Mode::Light] {
        let p = Theme::new(mode, TextScale::default()).palette;
        for (state, id, glyph) in SHEET {
            let (rect, look, role) = icon_in(mode, TextScale::default(), id, state);
            let at = format!("{mode:?} {state}: {look:#?}");
            assert!((rect.width() - 16.0).abs() < 0.01 && (rect.height() - 16.0).abs() < 0.01, "{at} {rect:?}");
            assert_eq!(role, Role::Image, "{at}");
            assert_eq!(look.texts, vec![(glyph.to_string(), p.muted, icons(), 16.0)], "Phosphor Regular 16 in muted: {at}");
            assert_eq!(look.fill, None, "never in a tile: {at}");
            assert_eq!(look.edge, None, "{at}");
            assert_eq!(look.ring, None, "{at}");
            assert!(look.radius <= 0.0, "{at}");
            assert!(!look.overflows, "{at}");
        }
    }
}

#[test]
fn an_icon_at_130_percent_is_the_scaled_side() {
    let theme = Theme::new(Mode::Light, TextScale::ALL[4]);
    for (state, id, glyph) in SHEET {
        let (rect, look, _) = icon_in(Mode::Light, TextScale::ALL[4], id, state);
        let side = theme.size(16.0);
        assert!((rect.width() - side).abs() < 0.01 && (rect.height() - side).abs() < 0.01, "{state} {rect:?}");
        assert_eq!(look.texts, vec![(glyph.to_string(), theme.palette.muted, icons(), side)], "{state}");
    }
}
