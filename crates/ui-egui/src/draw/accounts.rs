// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The Accounts page of the settings sheet: its head with Add account at the title's right end where the window can
//! add one, then "Your accounts" with a row for each account: the bar of its colour, its name, its address and the
//! link "open".
//! With no account the page says so. What is not built yet follows as coming soon: each connection's state, removing
//! an account and setting up a phone. Adding happens on the same page: "Accounts" at the top leads back to the list,
//! and the form, Detect and the check follow under it. An account's row opens its settings the same way: the add
//! form's fields, without Detect.

use egui::{CursorIcon, Frame, Rect, RichText, Role, Sense, Stroke, Ui, WidgetInfo, pos2, vec2};

use super::onboard;
use super::paint::{self, Ring, focus_ring, line};
use super::settings::{Asked, body, group, head, inset};
use crate::component::button::TextButton;
use crate::component::button::{LinkSize, link_sized};
use crate::component::label::count;
use crate::component::settings_row::settings_row_soon_with;
use crate::iconset::IconId;
use crate::keymap::Keymap;
use crate::mailbox::{AccountRow, Mailbox};
use crate::onboard::{Onboarding, Stage};
use crate::settings::Section;
use crate::theme::Theme;
use crate::tr;

/// The height of an account's row, in points (at 100 %).
const ROW: f32 = 48.0;

/// The width of an account's name, in points; its address follows 12 after it.
const NAME: f32 = 148.0;

/// The room between an account row's parts, in points.
const ROW_GAP: f32 = 12.0;

/// One account: the bar of its colour (3 × 12, 18 from the top of a 48 row), its name 148 wide, its address in `mono`,
/// and "open", a small link at the right end; the row is one button a screen reader reads, and it and the link open
/// the account's settings; a hairline under it. Whether it was pressed.
fn account(ui: &mut Ui, theme: &Theme, keymap: &Keymap, row: &AccountRow) -> bool {
    let p = &theme.palette;
    let f = &theme.fonts;
    let height = Theme::size(theme, ROW).max(f.strong.line).ceil();
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    let response = response.on_hover_cursor(CursorIcon::PointingHand);
    let spoken = tr!("{name}, {address}", name = row.label, address = row.address);
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, &spoken));
    let mut pressed = response.clicked();
    if response.hovered() && ui.is_rect_visible(rect) {
        ui.painter().rect_filled(rect, 0.0, p.stage);
    }
    let mid = rect.center().y;
    let right = rect.right() - 16.0;
    // "open", a link of its own at the right end, standing in the row's middle.
    let link = ui
        .scope_builder(
            egui::UiBuilder::new()
                .max_rect(Rect::from_min_max(pos2(rect.left(), rect.top()), pos2(right, rect.bottom())))
                .layout(egui::Layout::right_to_left(egui::Align::Center)),
            |ui| link_sized(ui, theme, keymap, tr!("open"), LinkSize::Small),
        )
        .inner;
    let spoken = tr!("Open the settings of {name}", name = row.label);
    ui.ctx().accesskit_node_builder(link.id, |node| node.set_label(spoken.as_str()));
    pressed |= link.clicked();
    if !ui.is_rect_visible(rect) {
        return pressed;
    }
    let painter = ui.painter();
    let left = rect.left() + 16.0;
    let bar = Rect::from_min_size(pos2(left, rect.top() + ((rect.height() - 12.0) / 2.0).round()), vec2(3.0, 12.0));
    painter.rect_filled(bar, 0.0, theme.account_colour(u64::try_from(row.index).unwrap_or(0)));
    let words = bar.right() + ROW_GAP;
    let ends = link.rect.left() - ROW_GAP;
    let address_at = (words + NAME + ROW_GAP).min(ends - 1.0).max(words + 1.0);
    let name = crate::component::cut(ui, &row.label, f.strong, p.ink, (address_at - words - ROW_GAP).max(1.0));
    let address = crate::component::cut(ui, &row.address, f.mono, p.muted, (ends - address_at).max(1.0));
    painter.galley(pos2(words, (mid - name.size().y / 2.0).round()), name, p.ink);
    painter.galley(pos2(address_at, (mid - address.size().y / 2.0).round()), address, p.muted);
    let y = rect.bottom() - 1.0;
    paint::hairline(painter, pos2(rect.left(), y), pos2(rect.right() - 1.0, y), p.panel_line);
    if response.has_focus() {
        focus_ring(ui.painter(), theme, Rect::from_min_max(rect.min, pos2(rect.right(), rect.bottom() - 1.0)), Ring::Inside);
    }
    pressed
}

/// What the page says where the accounts would be when there are none.
fn none_yet(ui: &mut Ui, theme: &Theme) {
    let p = &theme.palette;
    let s = &theme.spacing;
    Frame::new().stroke(Stroke::new(1.0, p.line)).corner_radius(0.0).inner_margin(s.s6).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.vertical_centered(|ui| {
            let side = theme.fonts.body.line * 2.4;
            let (rect, _) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
            crate::iconset::paint(ui.painter(), rect, IconId::Inbox, p.cyan_text);
            ui.add_space(s.s2);
            ui.label(RichText::new(tr!("Your accounts show up here")).font(theme.fonts.heading.id()).color(p.ink));
            ui.add_space(s.s1);
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
    let galley = line(ui, &[(name, theme.fonts.button, p.cyan_text)], f32::INFINITY);
    let icon = theme.fonts.body.size * 1.1;
    let (rect, response) = ui.allocate_exact_size(vec2(icon + s.s1 + galley.size().x + s.s2, galley.size().y + s.s1 * 2.0), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(Role::Link, true, tr!("Back to the accounts")));
    if ui.is_rect_visible(rect) {
        if response.hovered() {
            ui.painter().rect_filled(rect, 0.0, p.stage);
        }
        let at = Rect::from_center_size(pos2(rect.left() + icon / 2.0, rect.center().y), vec2(icon, icon));
        crate::iconset::paint(ui.painter(), at, IconId::Back, p.cyan_text);
        ui.painter().galley(pos2(at.right() + s.s1, rect.center().y - galley.size().y / 2.0), galley, p.cyan_text);
        if response.has_focus() {
            focus_ring(ui.painter(), theme, response.rect, Ring::Outside);
        }
    }
    response.clicked()
}

/// Draws the page; what was asked for. `can_add` offers Add account.
pub(super) fn page(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, onboarding: &mut Onboarding, can_add: bool) -> Option<Asked> {
    if onboarding.stage() != Stage::Closed {
        let went_back = inset(ui, |ui| back(ui, theme));
        ui.add_space(theme.spacing.s2);
        let asked = inset(ui, |ui| onboard::show(ui, theme, keymap, mailbox, onboarding).map(Asked::Page));
        return if went_back { Some(Asked::Back) } else { asked };
    }
    let accounts = mailbox.accounts();
    let mut asked = None;
    let note = if accounts.is_empty() {
        if can_add { tr!("No account yet. Add one to read and send mail here.") } else { tr!("No account yet.") }
    } else {
        tr!("The mail accounts this computer reads and sends from.")
    };
    head(ui, theme, Section::Accounts, &[note], |ui| {
        if can_add {
            let button = TextButton::new(tr!("Add account")).icon(IconId::Add);
            let button = if accounts.is_empty() { button.primary() } else { button.secondary() };
            if button.show(ui, theme, keymap).clicked() {
                asked = Some(Asked::Add);
            }
        }
    });
    body(ui, theme, "accounts", |ui| {
        if accounts.is_empty() && can_add {
            ui.add_space(theme.spacing.s4);
            none_yet(ui, theme);
        }
        if !accounts.is_empty() {
            let word = tr!("Your accounts · {count}", count = count(u64::try_from(accounts.len()).unwrap_or(u64::MAX)));
            group(ui, theme, "A", &word, None);
            for row in accounts {
                if account(ui, theme, keymap, row) {
                    asked = Some(Asked::Edit(row.id));
                }
            }
        }
        group(ui, theme, if accounts.is_empty() { "A" } else { "B" }, tr!("More"), None);
        settings_row_soon_with(
            ui,
            theme,
            tr!("Connection state"),
            tr!("Each account's row says how its connection stands: online, signing in or offline."),
            |_| {},
        );
        settings_row_soon_with(
            ui,
            theme,
            tr!("Remove account"),
            tr!("Takes an account and its mail off this computer, after a question that says it cannot be undone."),
            |ui| {
                TextButton::new(tr!("Remove account")).danger().show(ui, theme, keymap);
            },
        );
        settings_row_soon_with(
            ui,
            theme,
            tr!("Set up a phone"),
            tr!("A code on this screen sets up InboxCraft on your phone with the same accounts."),
            |ui| {
                link_sized(ui, theme, keymap, tr!("show the code →"), LinkSize::Small);
            },
        );
    });
    asked
}
