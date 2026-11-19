// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What stands in front of the window's place, top first, and the rules that follow from it: which surface one `Esc`
//! closes, which of the window's keys may run, and whether the mail list has the keys. [`Surfaces`] is built once a
//! frame from [`Facts`], before any key is read, and the window asks it instead of asking each surface in turn.
//!
//! What the screens settled stays where it is and is only named here:
//! - a float (a menu, a list, a picker) is [`crate::component::popup::any_open`]; it takes its own `Esc`
//!   ([`crate::component::popup::escape`] is the one gate for what lies under it) and a press beside it;
//! - a preview (a row the keys or the focus walked to, an Undo's restored selection) never floats a docked composer;
//!   only the owner's open does (`InboxApp::float_behind`);
//! - the floating composer is two areas, a shadow that takes no pointer under the panel, and stands 16 above the
//!   reader's answers.
//!
//! Built before the keys, from the frame before: a float is one shown "in this pass or the last", so after one closes
//! a single `Esc` may still find [`Surface::Float`] on top and close nothing. Whether the list is hidden beside the
//! composer is known only once this frame lays the panes out, after the keys; it is no fact here. The composer in
//! front ([`Surface::Composer`]) already takes the list's keys whenever the list is hidden, and `InboxApp::can_run`
//! keeps its own `list_hidden` terms, read after the drawing, for the palette and `run_action`, so a list key or act
//! never runs in the frame the list hides. A rule that needs this frame's layout belongs in `can_run`, not here.

use egui::Key;

use crate::keymap::{self, Keymap};

/// The most surfaces a frame holds: seven in front and the place. Eight surfaces exist, and the palette and a capture
/// never stand together with all the rest; an eighth in front would drop the lowest, never the top.
const MOST: usize = 8;

/// One thing that stands in front of the window's place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Surface {
    /// A float: a menu, an open list, a picker.
    Float,
    /// The command palette.
    Palette,
    /// The Shortcuts page waiting for a key.
    Capture,
    /// The calendar's event editor.
    Editor,
    /// The composer in the reader's place, in front.
    Composer,
    /// The reader over the whole width.
    FullWidth,
    /// In one pane, the open message in the list's place.
    OnePaneReader,
    /// The place itself (the mail, the calendar, the settings): always the last.
    Place,
}

/// What the window holds this frame, read from its state before the keys.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[expect(clippy::struct_excessive_bools, reason = "one fact per surface")]
pub(crate) struct Facts {
    /// A float showed in this pass or the last.
    pub float: bool,
    /// The palette is open.
    pub palette: bool,
    /// The Shortcuts page waits for a key.
    pub capture: bool,
    /// The calendar shows its event editor.
    pub editor: bool,
    /// The composer stands in the reader's place, in front of the list.
    pub composer: bool,
    /// The reader is set to the whole width (on the mail).
    pub full_width: bool,
    /// In one pane, the open message stands in the list's place.
    pub one_pane_reader: bool,
    /// The mail is the place shown.
    pub mail: bool,
    /// The settings are the place shown.
    pub settings: bool,
}

/// What stands in front, top first, the place last. Built once a frame ([`Surfaces::new`]); copies, allocates nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Surfaces {
    stack: [Surface; MOST],
    len: usize,
    mail: bool,
    settings: bool,
}

impl Surfaces {
    /// The surfaces of `facts`, top first: a float over everything; the palette and a capture, which take the keys;
    /// the event editor; the reader's full width (its `Esc` comes before the composer's, as it did); the
    /// composer in front; the reader in one pane; the place.
    pub(crate) fn new(facts: Facts) -> Self {
        // The settings are a place of their own: the mail is not shown under them.
        let mail = facts.mail && !facts.settings;
        let order = [
            (facts.float, Surface::Float),
            (facts.capture, Surface::Capture),
            (facts.palette, Surface::Palette),
            (facts.editor, Surface::Editor),
            (facts.full_width && mail, Surface::FullWidth),
            (facts.composer, Surface::Composer),
            (facts.one_pane_reader, Surface::OnePaneReader),
        ];
        let mut stack = [Surface::Place; MOST];
        let mut len = 0;
        for (on, surface) in order {
            // The last slot is the place's.
            if on && let Some(slot) = stack.get_mut(len).filter(|_| len + 1 < MOST) {
                *slot = surface;
                len += 1;
            }
        }
        Self { stack, len: len + 1, mail, settings: facts.settings }
    }

    /// The surfaces, top first, the place last.
    pub(crate) fn all(&self) -> &[Surface] {
        self.stack.get(..self.len).unwrap_or(&[Surface::Place])
    }

    /// The surface on top: the place when nothing stands in front.
    pub(crate) fn top(&self) -> Surface {
        self.all().first().copied().unwrap_or(Surface::Place)
    }

    /// Whether `surface` stands in front this frame.
    pub(crate) fn has(&self, surface: Surface) -> bool {
        self.all().contains(&surface)
    }

    /// Whether the keys are held by a surface that reads them itself: the palette, or the Shortcuts page waiting.
    pub(crate) fn held(&self) -> bool {
        self.has(Surface::Palette) || self.has(Surface::Capture)
    }

    /// Whether a key without Command, Ctrl or Alt runs nothing of the window, and Undo is not the window's: under the
    /// palette, a capture or the composer in front, whose fields and lists the letters are for, and on the settings,
    /// whose pages the letters are for.
    pub(crate) fn quiet(&self) -> bool {
        self.held() || self.settings || self.has(Surface::Composer)
    }

    /// Whether Undo rests, by every way in (its key, the palette's command, the toast's key and button): while the
    /// event editor is open, Undo would take back the event being edited.
    pub(crate) fn undo_rests(&self) -> bool {
        self.has(Surface::Editor)
    }

    /// The one surface an `Esc` pressed in this frame closes, or None when there was no `Esc` or it belongs to the
    /// place (the reader gives the keyboard back to the list). A float, the palette, a capture and the event editor
    /// take their `Esc` themselves; the window leaves the full width, and hands the rest to the place (the composer and
    /// the reader in one pane give the list its place back; the settings, a place, are not closed by it).
    pub(crate) fn escape(&self, ctx: &egui::Context) -> Option<Surface> {
        if !ctx.input(|input| input.key_pressed(Key::Escape)) {
            return None;
        }
        Some(self.top()).filter(|top| *top != Surface::Place)
    }

    /// Whether the window's `action` may run with these surfaces in front, as its key (`keymap`'s, with a modifier or
    /// without) is pressed. Not while the palette or a capture holds the keys; on the settings nothing of the mail
    /// ([`mail_only`]); under the event editor only what comes over it ([`EDITOR_KEYS`]); a key without a modifier not
    /// under a quiet surface ([`Surfaces::quiet`]), and Undo neither there nor while it rests
    /// ([`Surfaces::undo_rests`]); the list's keys only while it has them ([`Surfaces::list_has_keyboard`]). The acts
    /// on a row and the calendar's keys are not the window's: the list and the calendar take those.
    pub(crate) fn allows(&self, action: &str, keymap: &Keymap) -> bool {
        if self.held() || ROW_ACTS.contains(&action) || keymap::CALENDAR_KEYS.contains(&action) {
            return false;
        }
        if self.settings && mail_only(action) {
            return false;
        }
        if action == keymap::UNDO && (self.quiet() || self.undo_rests()) {
            return false;
        }
        if self.has(Surface::Editor) && !EDITOR_KEYS.contains(&action) {
            return false;
        }
        if keymap::LIST_KEYS.contains(&action) {
            return self.list_has_keyboard();
        }
        let bare = keymap.shortcut(action).is_some_and(|key| {
            let held = key.modifiers;
            !(held.command || held.ctrl || held.mac_cmd || held.alt)
        });
        !(self.quiet() && bare)
    }

    /// Whether the list may have the keys: on the mail, shown, with nothing in front that takes them (the palette, a
    /// capture, the composer in the reader's place, a float, whose arrows are its own). Whether a row of it has them is
    /// the list's to say.
    pub(crate) fn list_has_keyboard(&self) -> bool {
        self.mail && !self.held() && !self.has(Surface::Composer) && !self.has(Surface::Float)
    }

    /// Whether the settings are the place shown.
    pub(crate) fn on_settings(&self) -> bool {
        self.settings
    }
}

/// Whether `action` acts on the mail alone, so it rests while another place is shown that has no mail (the settings):
/// the list's keys and the acts on a row, Undo (of the mail's acts), the views of the inbox, Back, the held-back
/// message, Snooze and the composer's Send and fold. The ways to a place (Mail, Sent, Later, Find, the reader's full
/// width) show the mail first, so they are not among them.
pub(crate) fn mail_only(action: &str) -> bool {
    keymap::LIST_KEYS.contains(&action) || ROW_ACTS.contains(&action) || MAIL_ONLY.contains(&action)
}

/// The window's actions that act on the mail shown, besides the list's keys and the acts on a row.
const MAIL_ONLY: [&str; 9] = [
    keymap::UNDO,
    keymap::VIEW_GROUPED,
    keymap::VIEW_SMART,
    keymap::VIEW_LIST,
    keymap::BACK,
    keymap::OPEN_HELD,
    keymap::SNOOZE,
    keymap::SEND,
    keymap::FOLD,
];

/// The acts on the selected row: the list takes their keys.
const ROW_ACTS: [&str; 5] = [keymap::DONE, keymap::ARCHIVE, keymap::READ, keymap::FLAG, keymap::DELETE];

/// The keys that run under the event editor: what comes over it (the palette, the settings, Shortcuts) and the look
/// of the window. Nothing opens, shows or acts on what lies under it, nor leaves the calendar while it is open.
const EDITOR_KEYS: [&str; 7] =
    [keymap::PALETTE, keymap::SETTINGS, keymap::SHORTCUTS, keymap::LIGHT, keymap::DARK, keymap::SYSTEM_MODE, keymap::MOTION];

#[cfg(test)]
#[path = "surface_tests.rs"]
mod tests;
