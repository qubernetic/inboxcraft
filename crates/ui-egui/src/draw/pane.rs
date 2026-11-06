// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The navigation rail and the folder pane. The rail runs the preset's gradient at full strength down the window's
//! left edge, with Mail at its top and the appearance settings at its foot; the folder pane holds the account picker,
//! the unified views and each account's folders with their unread badges.

use egui::{Align, Color32, ComboBox, Layout, RichText, ScrollArea, Ui, pos2, vec2};

use super::paint::{NavRow, line_height, nav_row};
use crate::component::button::IconButton;
use crate::component::{control_side, tooltip};
use crate::gradient::{self, Gradient, Stop};
use crate::iconset::IconId;
use crate::keymap::{self, Keymap};
use crate::mailbox::{Mailbox, Place, role_text};
use crate::theme::{Theme, icon_ink_over};
use crate::tr;

/// The ink of the rail's icons: white where it reaches 3:1 on every stop of the gradient, otherwise the deep ink.
pub(super) fn rail_ink(theme: &Theme) -> Color32 {
    icon_ink_over(&theme.palette.gradient)
}

/// The rail's width: a button and room on either side.
pub(super) fn rail_width(ui: &Ui, theme: &Theme) -> f32 {
    control_side(ui, theme) + theme.spacing.s * 2.0
}

/// What the person asked for on the rail this frame.
#[derive(Debug, Default)]
pub(super) struct Rail {
    pub(super) mail: bool,
    pub(super) settings: bool,
}

/// Draws the rail over the whole of `ui`: Mail active unless `settings` or `adding` show another page.
pub(super) fn rail(ui: &mut Ui, theme: &Theme, keymap: &Keymap, settings: bool, adding: bool) -> Rail {
    let s = &theme.spacing;
    let rect = ui.max_rect();
    let [first, middle, last] = theme.palette.gradient;
    let gradient = Gradient::Linear {
        from: pos2(0.5, 0.0),
        to: pos2(0.5, 1.0),
        stops: vec![Stop::new(0.0, first), Stop::new(0.5, middle), Stop::new(1.0, last)],
    };
    gradient::fill(ui.painter(), rect, 0.0, &gradient);
    let ink = rail_ink(theme);
    let mut asked = Rail::default();
    ui.vertical_centered(|ui| {
        ui.add_space(s.m);
        asked.mail =
            IconButton::new(IconId::Inbox, tr!("Mail")).action(keymap::MAIL).active(!settings && !adding).ink(ink).show(ui, theme, keymap).clicked();
    });
    ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
        ui.add_space(s.m);
        asked.settings = IconButton::new(IconId::Settings, tr!("Appearance settings"))
            .action(keymap::SETTINGS)
            .active(settings)
            .ink(ink)
            .show(ui, theme, keymap)
            .clicked();
    });
    asked
}

/// What the person asked for in the pane this frame.
#[derive(Debug, Default)]
pub(super) struct Asked {
    pub(super) narrow: Option<Option<u64>>,
    pub(super) open: Option<Place>,
    pub(super) add: bool,
}

fn spoken(text: &str, account: Option<&str>, unread: u64) -> String {
    let named = match account {
        Some(account) => tr!("{folder} in {account}", folder = text, account = account),
        None => text.to_owned(),
    };
    if unread == 0 { named } else { tr!("{name}, {count} unread", name = named, count = unread) }
}

fn account_picker(ui: &mut Ui, theme: &Theme, mailbox: &Mailbox, asked: &mut Asked) {
    let label = ui.label(RichText::new(tr!("Accounts")).font(theme.fonts.caption.id()).color(theme.palette.muted));
    let current = match mailbox.narrowed().and_then(|id| mailbox.account(id)) {
        Some(account) => account.label.clone(),
        None => tr!("All accounts").to_owned(),
    };
    let width = ui.available_width();
    let response = ComboBox::from_id_salt("account-picker")
        .selected_text(current)
        .width(width)
        .show_ui(ui, |ui| {
            if ui.selectable_label(mailbox.narrowed().is_none(), tr!("All accounts")).clicked() {
                asked.narrow = Some(None);
            }
            for account in mailbox.accounts() {
                let text = tr!("{label} ({address})", label = account.label, address = account.address);
                if ui.selectable_label(mailbox.narrowed() == Some(account.id), text).clicked() {
                    asked.narrow = Some(Some(account.id));
                }
            }
        })
        .response;
    let _ = response.labelled_by(label.id);
}

fn heading(ui: &mut Ui, theme: &Theme, text: &str) {
    ui.add_space(theme.spacing.m);
    ui.label(RichText::new(text).font(theme.fonts.caption.id()).color(theme.palette.muted));
}

/// Draws the pane and returns what was asked for; the caller applies it after the frame's drawing. `can_add` shows
/// the Add account button.
pub(super) fn show(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, can_add: bool) -> Asked {
    let mut asked = Asked::default();
    let p = &theme.palette;
    ui.add_space(theme.spacing.s);
    ui.label(RichText::new("InboxCraft").font(theme.fonts.title.id()).color(p.text));
    if can_add {
        ui.add_space(theme.spacing.s);
        let button = ui.button(tr!("Add account"));
        asked.add = tooltip::attach(button, theme, keymap, tr!("Add account"), None).clicked();
    }
    if mailbox.accounts().is_empty() {
        return asked;
    }
    ui.add_space(theme.spacing.s);
    account_picker(ui, theme, mailbox, &mut asked);
    let searching = !mailbox.query().is_empty();
    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 2.0);
        heading(ui, theme, tr!("Mailboxes"));
        for view in mailbox.views() {
            let text = role_text(view.role);
            let row = NavRow {
                text,
                spoken: spoken(text, None, view.unread),
                badge: view.unread,
                selected: !searching && mailbox.place() == Place::View(view.role),
                depth: 0,
            };
            if nav_row(ui, theme, &row).clicked() {
                asked.open = Some(Place::View(view.role));
            }
        }
        for account in mailbox.accounts().iter().filter(|account| mailbox.narrowed().is_none_or(|id| id == account.id)) {
            ui.add_space(theme.spacing.m);
            let colour = theme.account_colour(u64::try_from(account.index).unwrap_or(0));
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = theme.spacing.xs;
                let (dot, _) = ui.allocate_exact_size(vec2(10.0, line_height(ui, theme.fonts.heading)), egui::Sense::hover());
                ui.painter().circle_filled(dot.center(), 4.0, colour);
                ui.label(RichText::new(&account.label).font(theme.fonts.heading.id()).color(p.text));
            });
            ui.label(RichText::new(&account.address).font(theme.fonts.caption.id()).color(p.muted));
            ui.add_space(theme.spacing.xs);
            for folder in mailbox.folders_of(account.id) {
                let text = match folder.role {
                    Some(role) => role_text(role),
                    None => folder.leaf.as_str(),
                };
                let row = NavRow {
                    text,
                    spoken: spoken(text, Some(&account.label), folder.unread),
                    badge: folder.unread,
                    selected: !searching && mailbox.place() == Place::Folder(folder.id),
                    depth: folder.depth,
                };
                if nav_row(ui, theme, &row).clicked() {
                    asked.open = Some(Place::Folder(folder.id));
                }
            }
        }
    });
    asked
}
