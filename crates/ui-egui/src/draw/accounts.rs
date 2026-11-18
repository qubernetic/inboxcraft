// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The Accounts page of the settings sheet: its head with Add account at the title's right end where the window can
//! add one, then "Your accounts" with a row for each account: the bar of its colour, its name, its address and the
//! link "open".
//! With no account the page is one box in the middle of its room, with Add account in it. What is not built yet follows as coming soon: each connection's state, removing
//! an account and setting up a phone. Adding happens on the same page: "Accounts" at the top leads back to the list,
//! and the form, Detect and the check follow under it. An account's row opens its settings the same way: the add
//! form's fields, without Detect.

use egui::{CursorIcon, Rect, Role, Sense, Ui, WidgetInfo, pos2, vec2};

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

/// The box of the page with no account, in points: 520 wide, its lines 40 under its 1 px edge and 24 in from it, 12
/// apart, and Add account 8 under the gap after the help.
const NONE_WIDE: f32 = 520.0;
const NONE_PAD_Y: f32 = 40.0;
const NONE_PAD_X: f32 = 24.0;
const NONE_GAP: f32 = 12.0;
const NONE_ACTION_TOP: f32 = 8.0;

/// The most lines the box's title and help each take.
const NONE_ROWS: usize = 4;

/// What the page says where the accounts would be when there are none, the design's `settings-accounts/empty`: a box
/// 520 wide in the middle of the page's room, `surface` with a 1 px `line` edge, holding the empty state of no account
/// with Add account where the window can add one (`can_add`). Whether Add account was pressed.
fn none_yet(ui: &mut Ui, theme: &Theme, keymap: &Keymap, can_add: bool) -> bool {
    let p = &theme.palette;
    let f = &theme.fonts;
    let room = ui.available_rect_before_wrap();
    let width = NONE_WIDE.min(room.width()).floor().max(1.0);
    let inner = (width - 2.0 - 2.0 * NONE_PAD_X).max(1.0);
    let (title, help) = if can_add {
        (
            tr!("Add one to read and send mail here."),
            tr!(
                "IMAP for the mail, SMTP to send, CalDAV for calendars. Detect proposes the servers from the address; nothing is sent until you have seen them."
            ),
        )
    } else {
        (tr!("This window cannot add an account."), tr!("Start InboxCraft with --sample to look around an invented mailbox."))
    };
    let label = tr!("No account yet");
    let lines = [
        (crate::component::wrapped(ui, label, f.label, p.muted, inner, 1, egui::Align::LEFT), label),
        (crate::component::wrapped(ui, title, f.title, p.muted, inner, NONE_ROWS, egui::Align::LEFT), title),
        (crate::component::wrapped(ui, help, f.caption, p.muted, inner, NONE_ROWS, egui::Align::LEFT), help),
    ];
    let text_high: f32 = lines.iter().map(|(line, _)| line.size().y).sum::<f32>() + NONE_GAP * 2.0;
    let action = if can_add { NONE_GAP + NONE_ACTION_TOP + crate::component::control_side(theme) } else { 0.0 };
    let high = (2.0 + NONE_PAD_Y + text_high + action + NONE_PAD_Y).ceil();
    // In the middle of the room under the head, on whole points; at its top where the room is shorter than the box.
    let free = if room.height().is_finite() { (room.height() - high).max(0.0) } else { 0.0 };
    let min = pos2(room.left() + ((room.width() - width) / 2.0).max(0.0), room.top() + free / 2.0).round();
    let card = Rect::from_min_size(min, vec2(width, high));
    ui.advance_cursor_after_rect(card);
    let group = ui.interact(card, ui.scope_id().with("no-account"), Sense::hover());
    group.widget_info(|| WidgetInfo::labeled(Role::Group, true, label));
    if ui.is_rect_visible(card) {
        paint::boxed(ui.painter(), card, p.surface, p.line);
    }
    let left = card.left() + 1.0 + NONE_PAD_X;
    let mut top = card.top() + 1.0 + NONE_PAD_Y;
    for (line, text) in lines {
        let high = line.size().y;
        onboard::said(ui, pos2(left, top), inner, line, p.muted, text);
        top += high + NONE_GAP;
    }
    if !can_add {
        return false;
    }
    let at = Rect::from_min_size(pos2(left, top + NONE_ACTION_TOP), vec2(inner, crate::component::control_side(theme)));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(at).layout(egui::Layout::left_to_right(egui::Align::Center)));
    TextButton::new(tr!("Add account")).primary().arrow().show(&mut child, theme, keymap).clicked()
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
    head(ui, theme, Section::Accounts, &[tr!("The mail accounts this computer reads and sends from.")], |ui| {
        if can_add && !accounts.is_empty() {
            let button = TextButton::new(tr!("Add account")).secondary();
            if button.show(ui, theme, keymap).clicked() {
                asked = Some(Asked::Add);
            }
        }
    });
    // With no account the page is the one box, as its frame draws it: nothing to scroll and nothing else to say.
    if accounts.is_empty() {
        if none_yet(ui, theme, keymap, can_add) {
            asked = Some(Asked::Add);
        }
        return asked;
    }
    body(ui, theme, "accounts", |ui| {
        let word = tr!("Your accounts · {count}", count = count(u64::try_from(accounts.len()).unwrap_or(u64::MAX)));
        group(ui, theme, "A", &word, None);
        for row in accounts {
            if account(ui, theme, keymap, row) {
                asked = Some(Asked::Edit(row.id));
            }
        }
        group(ui, theme, "B", tr!("More"), None);
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
