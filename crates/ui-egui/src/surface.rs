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
//! keeps its own `list_hidden` terms, read after the drawing, for the command line's results and `run_action`, so a list key or act
//! never runs in the frame the list hides. A rule that needs this frame's layout belongs in `can_run`, not here.

use egui::Key;

use crate::keymap::{self, Keymap};

/// The most surfaces a frame holds: eight in front and the place, as many as exist; one more in front would drop the
/// lowest, never the top.
const MOST: usize = 9;

/// One thing that stands in front of the window's place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Surface {
    /// A float: a menu, an open list, a picker.
    Float,
    /// A confirmation over the dimmed window ([`crate::component::dialog`]): it holds the keys and the pointer.
    Dialog,
    /// The command line's results: they hold the keys.
    Command,
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
    /// A window of its own (`crate::draw::host`): its content, with its own `Esc` and keys. A window's surfaces
    /// ([`Surfaces::window`]) are its own, built in its pass from its input; the main window's never hold it.
    Window,
    /// The place itself (Home, the mail, the calendar, the settings): always the last.
    Place,
}

/// What the window holds this frame, read from its state before the keys.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[expect(clippy::struct_excessive_bools, reason = "one fact per surface")]
pub(crate) struct Facts {
    /// A float showed in this pass or the last.
    pub float: bool,
    /// A confirmation dialog shows over the dimmed window.
    pub dialog: bool,
    /// The command line's results show.
    pub command: bool,
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
    /// Home is the place shown.
    pub home: bool,
    /// A composer is drawn: in the reader's place or floating, on whatever place shows (the settings draw none).
    pub composer_drawn: bool,
}

/// What stands in front, top first, the place last. Built once a frame ([`Surfaces::new`]); copies, allocates nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Surfaces {
    stack: [Surface; MOST],
    len: usize,
    mail: bool,
    settings: bool,
    /// A place without mail is shown: the settings or Home ([`mail_only`] rests there).
    apart: bool,
    /// A composer is drawn ([`of_the_composer`] rests without one).
    composer_drawn: bool,
    /// These are a window of its own's ([`Surfaces::window`]): only [`WINDOW_KEYS`] and the composer's run there.
    window: bool,
}

impl Surfaces {
    /// The surfaces of `facts`, top first: a float over everything; the command line's results and a capture, which take the keys;
    /// the event editor; the reader's full width (its `Esc` comes before the composer's, as it did); the
    /// composer in front; the reader in one pane; the place.
    pub(crate) fn new(facts: Facts) -> Self {
        // The settings are a place of their own: the mail is not shown under them.
        let mail = facts.mail && !facts.settings;
        let order = [
            (facts.float, Surface::Float),
            (facts.dialog, Surface::Dialog),
            (facts.capture, Surface::Capture),
            (facts.command, Surface::Command),
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
        Self {
            stack,
            len: len + 1,
            mail,
            settings: facts.settings,
            apart: facts.settings || facts.home,
            composer_drawn: facts.composer_drawn,
            window: false,
        }
    }

    /// The surfaces of a window of its own, built in its pass: a `float` of its own over its content
    /// ([`Surface::Window`]), which one `Esc` closes. No mail, no list and no place of the main window stands in it, so
    /// a key pressed there never acts on the main window's ringed row: only the look of the app ([`WINDOW_KEYS`]) and,
    /// where it holds a `composer`, the composer's own run from it ([`Surfaces::allows`]).
    pub(crate) fn window(float: bool, composer: bool) -> Self {
        let mut stack = [Surface::Place; MOST];
        let mut len = 0;
        for (on, surface) in [(float, Surface::Float), (true, Surface::Window)] {
            if on && let Some(slot) = stack.get_mut(len) {
                *slot = surface;
                len += 1;
            }
        }
        Self { stack, len: len + 1, mail: false, settings: false, apart: true, composer_drawn: composer, window: true }
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

    /// Whether the keys are held by a surface that reads them itself: the command line's results, a confirmation
    /// dialog, or the Shortcuts page waiting.
    pub(crate) fn held(&self) -> bool {
        self.has(Surface::Command) || self.has(Surface::Dialog) || self.has(Surface::Capture)
    }

    /// Whether a modal surface stands in front: the command line's results or a confirmation dialog. Nothing of the
    /// place under them runs while they show, whatever pressed it (the dialog's own answer is not the place's). A
    /// capture holds the keys, not the pointer (a press on its page ends the wait); the event editor's own asks come
    /// with the calendar's, which it lies over.
    pub(crate) fn modal(&self) -> bool {
        self.has(Surface::Command) || self.has(Surface::Dialog)
    }

    /// Whether a key without Command, Ctrl or Alt runs nothing of the window, and Undo is not the window's: under the
    /// palette, a capture or the composer in front, whose fields and lists the letters are for, and on the settings,
    /// whose pages the letters are for.
    pub(crate) fn quiet(&self) -> bool {
        self.held() || self.settings || self.has(Surface::Composer)
    }

    /// Whether Undo rests, by every way in (its key, the command line's results's command, the toast's key and button): while the
    /// event editor is open, Undo would take back the event being edited.
    pub(crate) fn undo_rests(&self) -> bool {
        self.has(Surface::Editor)
    }

    /// The one surface an `Esc` pressed in this frame closes, or None when there was no `Esc` or it belongs to the
    /// place (the reader gives the keyboard back to the list). A float, the command line's results, a capture and the event editor
    /// take their `Esc` themselves; the window leaves the full width, and hands the rest to the place (the composer and
    /// the reader in one pane give the list its place back; the settings, a place, are not closed by it).
    pub(crate) fn escape(&self, ctx: &egui::Context) -> Option<Surface> {
        if !ctx.input(|input| input.key_pressed(Key::Escape)) {
            return None;
        }
        Some(self.top()).filter(|top| *top != Surface::Place)
    }

    /// Whether the window's `action` may run with these surfaces in front, as its key (`keymap`'s, with a modifier or
    /// without) is pressed. Not while the command line's results or a capture holds the keys; on the settings and on Home nothing of the mail
    /// ([`mail_only`]); the composer's own only where a composer is drawn ([`of_the_composer`]); under the event editor only what comes over it ([`EDITOR_KEYS`]); a key without a modifier not
    /// under a quiet surface ([`Surfaces::quiet`]), and Undo neither there nor while it rests
    /// ([`Surfaces::undo_rests`]); the list's keys only while it has them ([`Surfaces::list_has_keyboard`]). The acts
    /// on a row and the calendar's keys are not the window's: the list and the calendar take those.
    pub(crate) fn allows(&self, action: &str, keymap: &Keymap) -> bool {
        if self.held() || ROW_ACTS.contains(&action) || keymap::CALENDAR_KEYS.contains(&action) {
            return false;
        }
        if self.window {
            return WINDOW_KEYS.contains(&action) || (of_the_composer(action) && self.composer_drawn);
        }
        if self.apart && mail_only(action) {
            return false;
        }
        if of_the_composer(action) && !self.composer_drawn {
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

    /// Whether a press on the window's own control for `action` (the pointer's, or a screen reader's activate) may run
    /// it: as [`Surfaces::allows`] says for the action, without the rest of a key with no modifier under a quiet
    /// surface, which is for keys alone (the letters are the fields'; a press on a button is not a letter).
    pub(crate) fn allows_press(&self, action: &str) -> bool {
        self.allows(action, &Keymap::empty())
    }

    /// Whether the list may have the keys: on the mail, shown, with nothing in front that takes them (the command line's results, a
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

/// Whether `action` acts on the mail alone, so it rests while another place is shown that has no mail (the settings,
/// Home):
/// the list's keys and the acts on a row, Undo (of the mail's acts), the views of the inbox, Back, the held-back
/// message and Snooze. The ways to a place (Mail, Sent, Later, Find, the reader's full width) show the mail first, so
/// they are not among them; nor is Home's own way, which shows Home; nor the composer's ([`of_the_composer`]).
pub(crate) fn mail_only(action: &str) -> bool {
    keymap::LIST_KEYS.contains(&action) || ROW_ACTS.contains(&action) || MAIL_ONLY.contains(&action)
}

/// Whether `action` is the composer's own, Send or fold: it runs wherever a composer is drawn (in the reader's place on
/// the mail, floating over the mail, Home or the calendar) and rests where none is (the settings draw none), whatever
/// the place.
pub(crate) fn of_the_composer(action: &str) -> bool {
    action == keymap::SEND || action == keymap::FOLD
}

/// The window's actions that act on the mail shown, besides the list's keys and the acts on a row.
const MAIL_ONLY: [&str; 7] =
    [keymap::UNDO, keymap::VIEW_GROUPED, keymap::VIEW_SMART, keymap::VIEW_LIST, keymap::BACK, keymap::OPEN_HELD, keymap::SNOOZE];

/// The acts on the selected row: the list takes their keys.
const ROW_ACTS: [&str; 5] = [keymap::DONE, keymap::ARCHIVE, keymap::READ, keymap::FLAG, keymap::DELETE];

/// The keys that run under the event editor: what comes over it (the command line's results, the settings, Shortcuts) and the look
/// of the window (its mode and its text size). Nothing opens, shows or acts on what lies under it, nor leaves the calendar while it is open.
pub(crate) const EDITOR_KEYS: [&str; 10] = [
    keymap::PALETTE,
    keymap::SETTINGS,
    keymap::SHORTCUTS,
    keymap::LIGHT,
    keymap::DARK,
    keymap::SYSTEM_MODE,
    keymap::TEXT_LARGER,
    keymap::TEXT_LARGER_EQUALS,
    keymap::TEXT_SMALLER,
    keymap::TEXT_RESET,
];

/// The main window's keys that also run from a window of its own: the look of the app, which every window shows. What
/// opens, shows or acts on something (the palette, the settings, a place, the list's row, Undo) is the main window's.
pub(crate) const WINDOW_KEYS: [&str; 7] =
    [keymap::LIGHT, keymap::DARK, keymap::SYSTEM_MODE, keymap::TEXT_LARGER, keymap::TEXT_LARGER_EQUALS, keymap::TEXT_SMALLER, keymap::TEXT_RESET];

#[cfg(test)]
#[path = "surface_tests.rs"]
mod tests;
