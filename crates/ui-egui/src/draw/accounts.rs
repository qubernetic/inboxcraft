// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The Accounts page of the settings sheet. "Your accounts" heads it, with Add account at its right end where the
//! window can add one, then a row for each account: its colour, its avatar, its name and its address. With no account
//! the page says so, and Add account is the primary button. Adding happens on the same page: "Accounts" at the top
//! leads back to the list, and the form, Detect and the check follow under it.

use egui::{Align, Frame, Layout, Rect, RichText, Role, Sense, Stroke, Ui, WidgetInfo, pos2, vec2};

use super::onboard;
use super::paint::{focus_ring, line, line_height};
use super::settings::{Asked, row_with};
use crate::component::avatar;
use crate::component::button::TextButton;
use crate::iconset::{IconId, Style};
use crate::keymap::Keymap;
use crate::mailbox::{AccountRow, Mailbox};
use crate::onboard::{Onboarding, Stage};
use crate::theme::Theme;
use crate::tr;

/// One account: the dot of its colour, its avatar, its name and its address, as one line a screen reader reads.
fn account(ui: &mut Ui, theme: &Theme, row: &AccountRow) {
    let p = &theme.palette;
    let s = &theme.spacing;
    let f = &theme.fonts;
    let tall = line_height(ui, f.strong).max(line_height(ui, f.body));
    let face = tall * 1.4;
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), face + s.s * 1.5), Sense::hover());
    let spoken = tr!("{name}, {address}", name = row.label, address = row.address);
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, &spoken));
    if !ui.is_rect_visible(rect) {
        return;
    }
    let painter = ui.painter();
    let mid = rect.center().y;
    painter.circle_filled(pos2(rect.left() + s.s, mid), 4.0, theme.account_colour(u64::try_from(row.index).unwrap_or(0)));
    let picture = Rect::from_min_size(pos2(rect.left() + s.s * 2.0, mid - face / 2.0), vec2(face, face));
    avatar::paint(painter, theme, picture, &row.label, &row.address);
    let left = picture.right() + s.m;
    let room = (rect.right() - left).max(1.0);
    let name = line(ui, &[(row.label.as_str(), f.strong, p.text)], room);
    let after = left + name.size().x + s.s;
    let address = line(ui, &[(row.address.as_str(), f.body, p.muted)], (rect.right() - after).max(1.0));
    painter.galley(pos2(left, mid - name.size().y / 2.0), name, p.text);
    painter.galley(pos2(after, mid - address.size().y / 2.0), address, p.muted);
}

/// What the page says where the accounts would be when there are none.
fn none_yet(ui: &mut Ui, theme: &Theme) {
    let p = &theme.palette;
    let s = &theme.spacing;
    ui.add_space(s.m);
    Frame::new().stroke(Stroke::new(1.0, p.field_border)).corner_radius(s.card).inner_margin(s.xl).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.vertical_centered(|ui| {
            let side = line_height(ui, theme.fonts.body) * 2.4;
            let (rect, _) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
            theme.icons.paint(ui.painter(), rect, IconId::Inbox, Style::Duotone, p.accent);
            ui.add_space(s.s);
            ui.label(RichText::new(tr!("Your accounts show up here")).font(theme.fonts.heading.id()).color(p.text));
            ui.add_space(s.xs);
            let about = tr!("IMAP for the mail. Detect proposes the servers from the address; nothing is sent until you have seen them.");
            ui.add(egui::Label::new(RichText::new(about).font(theme.fonts.body.id()).color(p.muted)).wrap());
        });
    });
}

/// "Accounts" with the back arrow before it, as a link to the list.
fn back(ui: &mut Ui, theme: &Theme) -> bool {
    let p = &theme.palette;
    let s = &theme.spacing;
    let name = tr!("Accounts");
    let galley = line(ui, &[(name, theme.fonts.button, p.accent)], f32::INFINITY);
    let icon = theme.fonts.body.size * 1.1;
    let (rect, response) = ui.allocate_exact_size(vec2(icon + s.xs + galley.size().x + s.s, galley.size().y + s.xs * 2.0), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(Role::Link, true, tr!("Back to the accounts")));
    if ui.is_rect_visible(rect) {
        if response.hovered() {
            ui.painter().rect_filled(rect, s.corner, p.hover);
        }
        let at = Rect::from_center_size(pos2(rect.left() + icon / 2.0, rect.center().y), vec2(icon, icon));
        theme.icons.paint(ui.painter(), at, IconId::Back, Style::Regular, p.accent);
        ui.painter().galley(pos2(at.right() + s.xs, rect.center().y - galley.size().y / 2.0), galley, p.accent);
        focus_ring(ui, theme, &response);
    }
    response.clicked()
}

/// Draws the page under its title; what was asked for. `can_add` offers Add account.
pub(super) fn page(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, onboarding: &mut Onboarding, can_add: bool) -> Option<Asked> {
    if onboarding.stage() != Stage::Closed {
        let went_back = back(ui, theme);
        ui.add_space(theme.spacing.s);
        let asked = onboard::show(ui, theme, keymap, mailbox, onboarding).map(Asked::Page);
        return if went_back { Some(Asked::Back) } else { asked };
    }
    let accounts = mailbox.accounts();
    let mut asked = None;
    let note = if accounts.is_empty() {
        if can_add { tr!("No account yet. Add one to read and send mail here.") } else { tr!("No account yet.") }
    } else {
        tr!("The mail accounts this computer reads and sends from.")
    };
    row_with(ui, theme, Some(IconId::At), tr!("Your accounts"), &[note], &mut |ui| {
        if can_add {
            let button = TextButton::new(tr!("Add account")).icon(IconId::Add);
            let button = if accounts.is_empty() { button.primary() } else { button.secondary() };
            if button.show(ui, theme, keymap).clicked() {
                asked = Some(Asked::Add);
            }
        }
    });
    if accounts.is_empty() {
        if can_add {
            none_yet(ui, theme);
        }
        return asked;
    }
    ui.add_space(theme.spacing.xs);
    ui.with_layout(Layout::top_down(Align::Min), |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for row in accounts {
            account(ui, theme, row);
        }
    });
    asked
}
