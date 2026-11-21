// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The Privacy page: what a remote picture tells its server, then the senders whose remote pictures load without
//! asking (`[ A ] PICTURES ALWAYS LOAD · 3`), a row per sender with "ask again"; the address stands at 16 while no
//! name is kept, after the frame's name column when one is. "ask again" takes the sender off the
//! list on the owner's press (`remote.forget`), at once, as the frames draw it, and the status line says so; a focus or
//! a hover runs nothing. The list is read once each time the page is shown.

use egui::{Align, Id, Layout, Rect, Ui, pos2, vec2};

use super::{Asked, body, group, head, inset, rule, words};
use crate::component::button::{LinkSize, link_sized};
use crate::keymap::Keymap;
use crate::privacy::{Hand, SenderRow};
use crate::settings::{Section, Settings};
use crate::theme::Theme;
use crate::tr;

/// A sender's row, in points at 100 %.
const ROW: f32 = 44.0;

/// The column the frame gives a sender's name, in points at 100 % (it grows with the type). `remote.list` keeps no name today: a row
/// without one starts its address at 16 and gives it the name's column too.
const NAME: f32 = 180.0;
/// The gap after a name, in points.
const NAME_GAP: f32 = 12.0;

/// The Privacy page.
pub(super) fn page(ui: &mut Ui, theme: &Theme, keymap: &Keymap, settings: &mut Settings) -> Option<Asked> {
    let mut asked = None;
    let intro = [
        tr!(
            "A remote picture tells its server that the message was opened, when and from where. InboxCraft asks every time before it loads one, unless you chose “always for this sender” in the reader."
        ),
        tr!("Nothing in a message runs, and a picture's request carries nothing but its address."),
    ];
    head(ui, theme, Section::Privacy, &intro, |_| {});
    let privacy = settings.privacy_mut();
    let mut hand = privacy.take_hand();
    if let Some(note) = privacy.note() {
        ui.add_space(12.0);
        inset(ui, |ui| words(ui, theme.fonts.body, note, theme.palette.warn));
    }
    let Some(rows) = privacy.rows() else {
        // Read once the page shows; the next frame draws the list.
        ui.ctx().request_repaint();
        return Some(Asked::ReadPrivacy);
    };
    body(ui, theme, "privacy", |ui| {
        let note = tr!(
            "The senders whose remote pictures load without asking. “ask again” takes one off the list: its pictures are held again, in every message."
        );
        if rows.is_empty() {
            group(ui, theme, "A", tr!("Pictures always load"), Some(note));
            ui.add_space(super::GROUP_TOP);
            crate::component::empty::empty_state(
                ui,
                theme,
                tr!("Privacy · nobody"),
                tr!("No sender loads pictures without asking."),
                tr!("“always for this sender” on a message's held pictures puts its sender here, and “ask again” takes it off."),
            );
            return;
        }
        group(ui, theme, "A", &tr!("Pictures always load · {n}", n = rows.len()), Some(note));
        let total = rows.len();
        for (at, row) in rows.iter().enumerate() {
            let ends = at == 0 || at.saturating_add(1) == total;
            if let Some(pressed) = sender_row(ui, theme, keymap, row, ends, &mut hand) {
                asked = Some(pressed);
            }
        }
    });
    asked
}

/// A sender's row; drawn only near the view unless it `ends` the list.
fn sender_row(ui: &mut Ui, theme: &Theme, keymap: &Keymap, row: &SenderRow, ends: bool, hand: &mut Option<Hand>) -> Option<Asked> {
    let width = ui.available_width();
    let high = ROW.max(theme.fonts.mono.line + 24.0).round();
    let top = ui.cursor().top();
    let rect = Rect::from_min_size(pos2(ui.max_rect().left(), top), vec2(width, high));
    let clip = ui.clip_rect();
    if !ends && !clip.expand2(vec2(0.0, clip.height())).intersects(rect) {
        ui.allocate_space(vec2(width, high + 1.0));
        return None;
    }
    let p = &theme.palette;
    let mut asked = None;
    let own = egui::UiBuilder::new()
        .scope_id(Id::unique("settings-always-load").with(&row.address))
        .max_rect(rect)
        .layout(Layout::left_to_right(Align::Center));
    ui.scope_builder(own, |ui| {
        ui.set_min_height(high);
        ui.spacing_mut().item_spacing.x = 0.0;
        let word = tr!("ask again");
        let link = crate::component::galley(ui, word, theme.fonts.mono_s, p.ink).size().x.ceil() + 16.0;
        ui.add_space(16.0);
        if let Some(name) = row.name.as_deref() {
            // The column grows with the type, as a text's cap does at 130 %.
            let column = Theme::size(theme, NAME);
            let name = crate::component::cut(ui, name, theme.fonts.strong, p.ink, column);
            let (place, _) = ui.allocate_exact_size(vec2(column, high), egui::Sense::hover());
            ui.painter().galley(pos2(place.left(), place.center().y - name.size().y / 2.0), name, p.ink);
            ui.add_space(NAME_GAP);
        }
        let room = (ui.available_width() - link - 12.0).max(1.0);
        let shown = crate::component::cut(ui, &crate::bidi::address(&row.shown), theme.fonts.mono, p.muted, room);
        let (place, _) = ui.allocate_exact_size(vec2(room, high), egui::Sense::hover());
        ui.painter().galley(pos2(place.left(), place.center().y - shown.size().y / 2.0), shown, p.muted);
        ui.add_space(12.0);
        let again = link_sized(ui, theme, keymap, word, LinkSize::Small);
        if hand.as_ref() == Some(&Hand::AskAgain(row.address.clone())) {
            *hand = None;
            again.request_focus();
        }
        let spoken = tr!("Ask again before the pictures of {address} load", address = row.shown);
        ui.ctx().accesskit_node_builder(again.id, |node| node.set_label(spoken.as_str()));
        if crate::component::button::owner_pressed(ui, &again) {
            asked = Some(Asked::AskAgain(row.address.clone()));
        }
    });
    rule(ui, theme);
    asked
}

/// What the Privacy page asked for, done: "ask again" through `remote.forget`, on the owner's press only.
impl super::super::InboxApp {
    /// Reads the senders whose pictures always load; a refusal leaves none and says why on the page.
    fn read_privacy(&mut self) {
        match self.mailbox.run("remote.list", &serde_json::json!({})) {
            Ok(answer) => {
                // A note from a read that failed before goes once the list reads well.
                self.settings.privacy_mut().read(&answer);
                self.settings.privacy_mut().set_note(None);
            }
            Err(error) => {
                self.settings.privacy_mut().read(&serde_json::Value::Null);
                self.settings.privacy_mut().set_note(Some(error.to_string()));
            }
        }
    }

    /// Does what the Privacy page asked for.
    pub(in crate::draw) fn apply_privacy(&mut self, asked: Asked) {
        match asked {
            Asked::ReadPrivacy => self.read_privacy(),
            Asked::AskAgain(address) => {
                let at = self.settings.privacy().rows().and_then(|rows| rows.iter().position(|row| row.address == address));
                let shown =
                    self.settings.privacy().rows().and_then(|rows| rows.iter().find(|row| row.address == address)).map(|row| row.shown.clone());
                let forgotten = self.mailbox.run("remote.forget", &serde_json::json!({ "address": address }));
                self.read_privacy();
                match forgotten {
                    Ok(_) => self.say(&tr!("{address} · pictures ask again", address = shown.unwrap_or_default())),
                    Err(error) => self.settings.privacy_mut().set_note(Some(error.to_string())),
                }
                // The keyboard goes to the row that took its place, or the one before; with none left, to the page's
                // row in the settings' own column.
                let rows = self.settings.privacy().rows().unwrap_or_default();
                let next = at.and_then(|at| rows.get(at).or_else(|| at.checked_sub(1).and_then(|before| rows.get(before))));
                match next.map(|row| row.address.clone()) {
                    Some(next) => self.settings.privacy_mut().hand_to(Hand::AskAgain(next)),
                    None => self.settings.hand_to_nav(),
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
#[path = "settings_privacy_tests.rs"]
mod tests;
