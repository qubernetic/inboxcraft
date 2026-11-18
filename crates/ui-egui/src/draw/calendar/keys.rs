// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The calendar's keys. The arrows move the picked day (month, mini month) or the picked event (week, day, agenda),
//! `Enter` opens the picked day or takes the keyboard into the picked event's details, `Esc` leaves them. They are
//! keymap actions ([`crate::keymap::CALENDAR_KEYS`]): named, rebindable on the Shortcuts page, and resting while a
//! text field has the keyboard, while a float or the editor is open, and while the keyboard is on a control that is not
//! the picked day or event (a toolbar button keeps its own keys).
//!
//! The keyboard's focus and the pick are one thing: a key that moves the pick hands the focus to the new day's or
//! event's button ([`wanted`]), and Tab onto an event picks it, so a screen reader hears what is picked.

use egui::{Context, Id};

use crate::calendar::{Calendar, Target};
use crate::keymap::{self, Keymap};

/// Where the buttons of the days and events drawn last frame are kept: the keyboard on one of them is on the pick.
fn items_id() -> Id {
    Id::unique("calendar-key-items")
}

/// Where the button the next frame hands the keyboard to is kept.
fn wanted_id() -> Id {
    Id::unique("calendar-key-wanted")
}

/// The most buttons kept as the pick's: more than a frame draws.
const MOST_ITEMS: usize = 4096;

/// Forgets last frame's day and event buttons: called before the screen is drawn.
pub(in crate::draw) fn begin(ctx: &Context) {
    ctx.data_mut(|data| data.insert_temp::<Vec<Id>>(items_id(), Vec::new()));
}

/// Notes `response` as the button of a day or an event drawn this frame. While it has the keyboard, the arrows are
/// the calendar's, not egui's (which would move the focus to the widget beside it).
pub(super) fn item(ctx: &Context, response: &egui::Response) {
    if response.has_focus() {
        let filter = egui::EventFilter { horizontal_arrows: true, vertical_arrows: true, tab: false, escape: true };
        ctx.memory_mut(|memory| memory.set_focus_lock_filter(response.id, filter));
    }
    let id = response.id;
    ctx.data_mut(|data| {
        let items = data.get_temp_mut_or_default::<Vec<Id>>(items_id());
        if items.len() < MOST_ITEMS {
            items.push(id);
        }
    });
}

/// Whether the keyboard should go to `target` now: the key that moved the pick asked for it; asked once.
pub(super) fn wanted(ctx: &Context, target: Target) -> bool {
    ctx.data_mut(|data| {
        let wanted = data.get_temp::<Target>(wanted_id());
        if wanted == Some(target) {
            data.remove::<Target>(wanted_id());
            true
        } else {
            false
        }
    })
}

/// Asks the next frame to hand the keyboard to `target`'s button.
pub(in crate::draw) fn want(ctx: &Context, target: Target) {
    ctx.data_mut(|data| data.insert_temp(wanted_id(), target));
}

/// The calendar's keys pressed this frame that are the calendar's to take; each press is taken, so nothing else sees
/// it. `open` says whether a float, the editor, the settings sheet or the palette is open: then every key rests.
#[must_use]
pub(in crate::draw) fn pressed(ctx: &Context, keymap: &Keymap, calendar: &Calendar, open: bool) -> Vec<&'static str> {
    let focused = ctx.memory(|memory| memory.focused());
    // A text field keeps its keys: the focused widget holds a text field's state.
    let typing = focused.is_some_and(|id| egui::text_edit::TextEditState::load(ctx, id).is_some());
    let modal = ctx.memory(|memory| memory.top_modal_layer().is_some());
    if open || typing || modal || calendar.editor().is_some() || calendar.more().is_some() {
        return Vec::new();
    }
    let on_pick = focused.is_none_or(|focused| ctx.data(|data| data.get_temp::<Vec<Id>>(items_id())).is_some_and(|items| items.contains(&focused)));
    keymap::CALENDAR_KEYS
        .into_iter()
        // Esc leaves the details from anywhere on the screen; the arrows and Enter only from the pick.
        .filter(|id| on_pick || *id == keymap::CALENDAR_CLOSE)
        .filter(|id| keymap.pressed(ctx, id))
        .collect()
}
