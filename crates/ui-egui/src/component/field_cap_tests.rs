// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The cap of a field in one frame: whatever the frame's events and their order, the text never passes its cap by
//! growing, and what would carry it past is refused whole (`guard` before the edit, `hold` after it).

use egui::accesskit::{Action, ActionData, ActionRequest, TreeId};
use egui::text::CCursorRange;
use egui::text_edit::TextEditState;
use egui::{Event, ImeEvent, Key, Modifiers, PointerButton, Pos2, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::tests::{Kind, ONE_FRAME, at_the_cap, cap_of, field_of, kind_name, one_frame, press, says_too_long};
use super::{Cap, Field, LAID_OUT, multiline_field_holding, password_field, text_field_limited};
use crate::component::tests::{builder, theme};

/// The id the field of `kind` has in `field_of`.
fn id_of(kind: Kind) -> egui::Id {
    match kind {
        Kind::Many => egui::Id::unique("many"),
        Kind::Password => egui::Id::unique("secret"),
        Kind::PlainText => egui::Id::unique("body"),
        _ => egui::Id::unique("one"),
    }
}

/// A screen reader's `action` on the field `id` with `value`.
fn reader(id: egui::Id, action: Action, value: &str) -> Event {
    Event::AccessKitActionRequest(ActionRequest {
        action,
        target_tree: TreeId::ROOT,
        target_node: id.accesskit_id(),
        data: Some(ActionData::Value(value.into())),
    })
}

/// A middle click at `at` that pastes `text` (the PRIMARY selection on X11 and Wayland), as egui-winit sends it.
fn middle(at: Pos2, text: &str) -> Vec<Event> {
    let button = |pressed| Event::PointerButton { pos: at, button: PointerButton::Middle, pressed, modifiers: Modifiers::NONE };
    vec![Event::PointerMoved(at), button(true), Event::MiddleClickPaste { pos: at, text: text.to_owned() }, button(false)]
}

/// A point inside the part of the field of `kind` that is in the window (a long text's node reaches past it).
fn inside(harness: &Harness<'static, String>, kind: Kind) -> Pos2 {
    let rect = harness.get_by_label(kind_name(kind)).rect();
    pos2(rect.center().x, (rect.top().max(0.0) + rect.bottom().min(500.0)) / 2.0)
}

/// The field's selection as it is stored.
fn selection(harness: &Harness<'static, String>, kind: Kind) -> Option<CCursorRange> {
    TextEditState::load(&harness.ctx, id_of(kind)).and_then(|state| state.cursor.char_range())
}

/// The text and the selection before a frame.
struct Before {
    text: String,
    selection: Option<CCursorRange>,
}

fn before(harness: &Harness<'static, String>, kind: Kind) -> Before {
    Before { text: harness.state().clone(), selection: selection(harness, kind) }
}

/// The size of `text` in the unit of the cap of `kind`.
fn size(kind: Kind, text: &str) -> usize {
    if cap_of(kind).1 { text.len() } else { text.chars().count() }
}

/// After a frame: the text is within its cap or no longer than it was; and when the field refused, the text and the
/// selection are as they were and it says so.
fn sound(harness: &Harness<'static, String>, kind: Kind, was: &Before, case: &str) {
    let (most, _) = cap_of(kind);
    let now = harness.state();
    let grown = size(kind, now);
    assert!(grown <= most || grown <= size(kind, &was.text), "{kind:?} {case}: {grown} past the cap {most}");
}

/// The frame was refused whole: text and selection as before, and the field says why.
fn refused(harness: &Harness<'static, String>, kind: Kind, was: &Before, case: &str) {
    sound(harness, kind, was, case);
    assert!(harness.state() == &was.text, "{kind:?} {case}: the text changed ({} for {})", harness.state().len(), was.text.len());
    assert_eq!(selection(harness, kind), was.selection, "{kind:?} {case}: the selection is as it was");
    assert!(says_too_long(harness), "{kind:?} {case}: the field says why");
}

/// The frame was taken: the text holds `whole` whole and the field says nothing.
fn taken(harness: &Harness<'static, String>, kind: Kind, was: &Before, whole: &str, case: &str) {
    sound(harness, kind, was, case);
    assert!(harness.state().contains(whole), "{kind:?} {case}: {:?} is not in the text whole", whole.get(..8));
    assert!(!says_too_long(harness), "{kind:?} {case}: nothing was refused");
}

#[test]
fn a_screen_reader_s_value_and_a_paste_in_one_frame_are_judged_in_egui_s_order() {
    for kind in ONE_FRAME {
        let (most, _) = cap_of(kind);
        let id = id_of(kind);
        let value = "v".repeat(most);
        // egui sets the value first, whatever its place in the list, and then pastes after it: one past the cap.
        for value_first in [false, true] {
            let mut harness = field_of(kind, "kept");
            let was = before(&harness, kind);
            let set = reader(id, Action::SetValue, &value);
            let paste = Event::Paste("p".to_owned());
            one_frame(&mut harness, if value_first { vec![set, paste] } else { vec![paste, set] });
            refused(&harness, kind, &was, &format!("value first {value_first}"));
        }
        // A value as long as the cap alone is taken; one longer is refused.
        let mut harness = field_of(kind, "kept");
        let was = before(&harness, kind);
        one_frame(&mut harness, vec![reader(id, Action::SetValue, &value)]);
        assert_eq!(harness.state(), &value, "{kind:?}: a value of the cap is taken");
        sound(&harness, kind, &was, "value of the cap");
        let was = before(&harness, kind);
        one_frame(&mut harness, vec![reader(id, Action::SetValue, &"w".repeat(most + 1))]);
        refused(&harness, kind, &was, "value past the cap");
    }
}

#[test]
fn a_middle_click_paste_is_held_to_the_cap_with_the_focus_and_without() {
    for kind in ONE_FRAME {
        for focused in [true, false] {
            // Far below the cap it goes in whole.
            let mut harness = field_of(kind, "kept");
            let at = inside(&harness, kind);
            if !focused {
                one_frame(&mut harness, press(Key::Escape, Modifiers::NONE).into());
                assert!(!harness.get_by_label(kind_name(kind)).is_focused(), "{kind:?}: Escape lets go of the focus");
            }
            one_frame(&mut harness, vec![Event::PointerMoved(at)]);
            let was = before(&harness, kind);
            one_frame(&mut harness, middle(at, &"m".repeat(10)));
            taken(&harness, kind, &was, &"m".repeat(10), &format!("focus {focused}, below the cap"));
            // At the cap it is refused whole.
            let (mut harness, most) = at_the_cap(kind);
            let at = inside(&harness, kind);
            if !focused {
                one_frame(&mut harness, press(Key::Escape, Modifiers::NONE).into());
            }
            one_frame(&mut harness, vec![Event::PointerMoved(at)]);
            let was = before(&harness, kind);
            one_frame(&mut harness, middle(at, "m"));
            refused(&harness, kind, &was, &format!("focus {focused}, at the cap"));
            // And one longer than the cap is refused in any field.
            let mut harness = field_of(kind, "");
            let at = inside(&harness, kind);
            if !focused {
                one_frame(&mut harness, press(Key::Escape, Modifiers::NONE).into());
            }
            let was = before(&harness, kind);
            one_frame(&mut harness, vec![Event::PointerMoved(at)]);
            one_frame(&mut harness, middle(at, &"m".repeat(most + 1)));
            assert_eq!(harness.state(), &was.text, "{kind:?} focus {focused}: past the cap alone");
            assert!(says_too_long(&harness), "{kind:?} focus {focused}");
        }
    }
}

#[test]
fn a_collapsed_select_all_then_a_paste_of_the_cap_is_refused_whole() {
    for kind in ONE_FRAME {
        let (mut harness, most) = at_the_cap(kind);
        let was = before(&harness, kind);
        let mut events: Vec<Event> = press(Key::A, Modifiers::COMMAND).into();
        events.extend(press(Key::ArrowRight, Modifiers::NONE));
        events.push(Event::Paste("z".repeat(most)));
        one_frame(&mut harness, events);
        refused(&harness, kind, &was, "select all, arrow, paste");
    }
}

#[test]
fn a_character_typed_in_the_frame_of_an_undo_or_a_redo_is_judged_by_where_the_text_ends() {
    for kind in ONE_FRAME {
        for undo in [Key::Z, Key::Y] {
            // Far below the cap: taken.
            let mut harness = field_of(kind, "kept");
            let was = before(&harness, kind);
            let mut events: Vec<Event> = press(undo, Modifiers::COMMAND).into();
            events.push(Event::Text("a".to_owned()));
            one_frame(&mut harness, events);
            taken(&harness, kind, &was, "a", &format!("{undo:?}, far below"));
            // At the cap, opened so with nothing to undo or redo: refused.
            let (most, _) = cap_of(kind);
            let mut harness = field_of(kind, &"y".repeat(most));
            one_frame(&mut harness, press(Key::End, Modifiers::CTRL).into());
            let was = before(&harness, kind);
            let mut events: Vec<Event> = press(undo, Modifiers::COMMAND).into();
            events.push(Event::Text("a".to_owned()));
            one_frame(&mut harness, events);
            refused(&harness, kind, &was, &format!("{undo:?}, typed at the cap"));
            // A paste in the frame of a redo, at the cap.
            let was = before(&harness, kind);
            let mut events: Vec<Event> = press(Key::Y, Modifiers::COMMAND).into();
            events.push(Event::Paste("z".repeat(most)));
            one_frame(&mut harness, events);
            refused(&harness, kind, &was, "Ctrl Y, paste");
        }
    }
}

#[test]
fn a_screen_reader_s_replacement_of_a_selection_is_held_to_the_cap() {
    for kind in ONE_FRAME {
        let id = id_of(kind);
        // The whole text selected and replaced by as much: taken.
        let (mut harness, most) = at_the_cap(kind);
        one_frame(&mut harness, press(Key::A, Modifiers::COMMAND).into());
        let was = before(&harness, kind);
        one_frame(&mut harness, vec![reader(id, Action::ReplaceSelectedText, &"r".repeat(most))]);
        assert_eq!(harness.state(), &"r".repeat(most), "{kind:?}: the selection gives its room");
        sound(&harness, kind, &was, "replace all");
        // One character selected and replaced by two: refused.
        one_frame(&mut harness, press(Key::End, Modifiers::CTRL).into());
        one_frame(&mut harness, press(Key::ArrowLeft, Modifiers::SHIFT).into());
        let was = before(&harness, kind);
        one_frame(&mut harness, vec![reader(id, Action::ReplaceSelectedText, "rr")]);
        refused(&harness, kind, &was, "replace one by two");
    }
}

#[test]
fn an_ime_composition_across_frames_with_an_undo_between_is_held_to_the_cap() {
    for kind in ONE_FRAME {
        for undo in [false, true] {
            let (most, bytes) = cap_of(kind);
            // Four of the cap left: "ëñ" fits as a composition in either unit, "ëñë" does not.
            let room = if bytes { 4 } else { 2 };
            let start = "y".repeat(most - room);
            let mut harness = field_of(kind, &start);
            one_frame(&mut harness, press(Key::End, Modifiers::CTRL).into());
            one_frame(&mut harness, vec![Event::Ime(ImeEvent::Preedit { text: "ëñ".to_owned(), active_range_chars: Some(0..2) })]);
            assert_eq!(harness.state(), &format!("{start}ëñ"), "{kind:?}: the composition stands in the text");
            if undo {
                let was = before(&harness, kind);
                one_frame(&mut harness, press(Key::Z, Modifiers::COMMAND).into());
                sound(&harness, kind, &was, "undo in a composition");
            }
            // A commit longer than the composition passes the cap: refused, and the composition goes with it.
            let was = before(&harness, kind);
            one_frame(&mut harness, vec![Event::Ime(ImeEvent::Commit("ëñë".to_owned()))]);
            sound(&harness, kind, &was, "commit");
            assert_eq!(harness.state(), &start, "{kind:?} undo {undo}: nothing of the composition stays");
            assert!(says_too_long(&harness), "{kind:?} undo {undo}: the field says why");
            // A commit that fits goes in whole.
            one_frame(&mut harness, vec![Event::Ime(ImeEvent::Preedit { text: "ë".to_owned(), active_range_chars: Some(0..1) })]);
            one_frame(&mut harness, vec![Event::Ime(ImeEvent::Commit("ëñ".to_owned()))]);
            assert_eq!(harness.state(), &format!("{start}ëñ"), "{kind:?} undo {undo}: the commit that fits");
        }
    }
}

#[test]
fn what_an_edit_lays_out_in_one_frame_is_at_most_twice_the_cap() {
    for kind in ONE_FRAME {
        let (most, _) = cap_of(kind);
        let id = id_of(kind);
        let (mut harness, _) = at_the_cap(kind);
        let at = inside(&harness, kind);
        one_frame(&mut harness, vec![Event::PointerMoved(at)]);
        LAID_OUT.with(|laid| laid.set(0));
        // Every event at most the cap, together many times it, in the order egui likes least.
        let mut events = vec![Event::Paste("a".repeat(most)), reader(id, Action::SetValue, &"b".repeat(most))];
        events.extend(middle(at, &"c".repeat(most)));
        events.push(reader(id, Action::ReplaceSelectedText, &"d".repeat(most)));
        events.push(Event::Text("e".repeat(most)));
        one_frame(&mut harness, events);
        // And one event far past the cap.
        one_frame(&mut harness, middle(at, &"f".repeat(most * 10)));
        one_frame(&mut harness, vec![Event::Paste("g".repeat(most * 10))]);
        let laid = LAID_OUT.with(std::cell::Cell::get);
        assert!(laid <= 2 * most, "{kind:?}: {laid} laid out, the cap {most}");
        assert_eq!(harness.state().len(), most, "{kind:?}: still at the cap");
    }
}

#[test]
fn a_text_loaded_over_its_cap_opens_whole_edits_down_and_cannot_grow() {
    for kind in ONE_FRAME {
        let (most, _) = cap_of(kind);
        let over = "y".repeat(most + 2);
        let mut harness = field_of(kind, &over);
        assert_eq!(harness.state(), &over, "{kind:?}: opened whole");
        one_frame(&mut harness, press(Key::End, Modifiers::CTRL).into());
        let was = before(&harness, kind);
        one_frame(&mut harness, vec![Event::Text("q".to_owned())]);
        refused(&harness, kind, &was, "grow over the cap");
        let was = before(&harness, kind);
        one_frame(&mut harness, vec![reader(id_of(kind), Action::SetValue, &"w".repeat(most + 3))]);
        refused(&harness, kind, &was, "a longer value over the cap");
        one_frame(&mut harness, press(Key::Backspace, Modifiers::NONE).into());
        assert_eq!(harness.state().len(), most + 1, "{kind:?}: deleting is taken");
        one_frame(&mut harness, press(Key::Backspace, Modifiers::NONE).into());
        assert_eq!(harness.state().len(), most, "{kind:?}: it fits");
        assert!(!says_too_long(&harness), "{kind:?}: the line goes once it fits");
        let was = before(&harness, kind);
        one_frame(&mut harness, vec![Event::Text("q".to_owned())]);
        refused(&harness, kind, &was, "at the cap");
    }
}

#[test]
fn a_refused_password_leaves_nothing_in_the_tree_the_memory_or_the_clipboard() {
    const SECRET: &str = "Sekret-kettle";
    let theme = theme();
    let id = egui::Id::unique("password");
    let mut harness = builder().build_ui_state(
        move |ui, text: &mut String| {
            password_field(ui, &theme, id, text, &Field::new("Password", ""), super::Secret::Typed);
        },
        String::new(),
    );
    harness.run();
    harness.get_by_label("Password").focus();
    harness.run();
    let pad = "s".repeat(super::MAX_SECRET - SECRET.len() - 2);
    one_frame(&mut harness, vec![Event::Paste(format!("{SECRET}{pad}"))]);
    // Five more do not fit in two: refused after the edit took them, by the snapshot.
    one_frame(&mut harness, vec![Event::Paste("QQQQQ".to_owned())]);
    assert!(harness.state().starts_with(SECRET) && !harness.state().contains('Q'), "refused");
    assert!(harness.query_all(egui_kittest::kittest::by().label_contains("Too long")).count() == 1, "said");
    let leaks = |text: &str| text.contains(SECRET) || text.contains("QQQ");
    assert!(harness.root().children_recursive().all(|node| !leaks(&format!("{:?}", node.accesskit_node()))), "a node holds the password");
    let copied =
        harness.output().platform_output.commands.iter().any(|command| matches!(command, egui::OutputCommand::CopyText(text) if leaks(text)));
    assert!(!copied, "the clipboard was handed the password");
    // The edit's state, which egui keeps in its persisted memory, holds no history to bring a text back.
    let state = TextEditState::load(&harness.ctx, id);
    let history = state.map(|state| format!("{:?}", state.undoer())).unwrap_or_default();
    assert!(history.contains("undo count: 0") && history.contains("redo count: 0"), "{history}");
    assert!(harness.ctx.data_mut(|data| data.get_persisted::<String>(id)).is_none(), "no text kept by the field's id");
}

/// A deterministic stream of numbers (xorshift), so that the property test needs no clock and no crate.
struct Seeded(u64);

impl Seeded {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// A number in `0..below`.
    fn below(&mut self, below: usize) -> usize {
        usize::try_from(self.next() % u64::try_from(below.max(1)).unwrap_or(1)).unwrap_or(0)
    }
}

/// Characters of one to four bytes, each one event's marker in a frame ('x' when none is left, which is not checked).
const MARKERS: [char; 24] =
    ['a', 'b', 'c', 'd', 'g', 'h', 'é', 'ß', 'ü', 'ő', 'ñ', 'ø', '€', '→', '✓', 'あ', '中', '한', '𝄞', '😀', '🦀', '𐍈', '🌍', '🎵'];

/// What a random frame holds.
fn random_frame(seed: &mut Seeded, id: egui::Id, at: Pos2, most: usize, absent: &[char]) -> (Vec<Event>, Vec<(char, usize)>, bool) {
    let mut events = Vec::new();
    let mut carried = Vec::new();
    let mut keys = false;
    for slot in 0..=seed.below(4) {
        let marker = absent.get(slot).copied().unwrap_or('x');
        let count = seed.below(most * 2 + 2);
        let text: String = std::iter::repeat_n(marker, count).collect();
        let pick = seed.below(14);
        let carries = match pick {
            0 => Some(Event::Text(text)),
            1 => Some(Event::Paste(text)),
            2 => Some(reader(id, Action::SetValue, &text)),
            3 => Some(reader(id, Action::ReplaceSelectedText, &text)),
            4 => {
                events.push(Event::PointerMoved(at));
                Some(Event::MiddleClickPaste { pos: at, text })
            }
            5 => Some(Event::Ime(ImeEvent::Preedit { text, active_range_chars: None })),
            6 => Some(Event::Ime(ImeEvent::Commit(text))),
            _ => None,
        };
        if let Some(event) = carries {
            carried.push((marker, count));
            events.push(event);
            continue;
        }
        keys = true;
        let (key, modifiers) = match pick {
            7 => (Key::A, Modifiers::COMMAND),
            8 => (Key::ArrowLeft, if seed.below(2) == 0 { Modifiers::SHIFT } else { Modifiers::NONE }),
            9 => (Key::ArrowRight, Modifiers::NONE),
            10 => (Key::Backspace, Modifiers::NONE),
            11 => (Key::Enter, Modifiers::NONE),
            12 => (Key::Z, Modifiers::COMMAND),
            _ => (Key::Y, Modifiers::COMMAND),
        };
        events.extend(press(key, modifiers));
    }
    (events, carried, keys)
}

/// 2 000 random frames of one to four events each in a field holding at most `cap`: after each, the text is within
/// the cap or not longer than before, and in a frame without keys every marker's text is in whole or not at all.
fn random_frames(cap: Cap, lines: bool) {
    let theme = theme();
    let id = egui::Id::unique("random");
    let mut harness = builder().with_size(vec2(600.0, 500.0)).build_ui_state(
        move |ui, text: &mut String| {
            let field = Field::new("Random", "");
            if lines {
                multiline_field_holding(ui, &theme, id, text, &field, cap);
            } else if let Cap::Chars(most) = cap {
                text_field_limited(ui, &theme, id, text, &field, most);
            }
        },
        String::new(),
    );
    harness.run_ok();
    harness.get_by_label("Random").focus();
    harness.run_ok();
    let rect = harness.get_by_label("Random").rect();
    let at = pos2(rect.center().x, (rect.top().max(0.0) + rect.bottom().min(500.0)) / 2.0);
    let most = cap.most();
    let measure = |text: &str| cap.measure(text);
    let mut seed = Seeded(0x1234_5678_9abc_def1);
    for frame in 0..2_000 {
        let was = harness.state().clone();
        let absent: Vec<char> = MARKERS.iter().copied().filter(|marker| !was.contains(*marker)).collect();
        let (events, carried, keys) = random_frame(&mut seed, id, at, most, &absent);
        LAID_OUT.with(|laid| laid.set(0));
        harness.input_mut().events.extend(events.clone());
        harness.step();
        let now = harness.state();
        let laid = LAID_OUT.with(std::cell::Cell::get);
        assert!(measure(now) <= most || measure(now) <= measure(&was), "frame {frame}: {} past {most}: {events:?}", measure(now));
        // Twice the cap: in bytes for a text of several lines, at most four bytes a character for a line.
        let bound = if lines { 2 * most } else { 2 * 4 * most };
        assert!(laid <= bound, "frame {frame}: {laid} bytes laid out: {events:?}");
        if !keys {
            for (marker, count) in carried {
                let n = now.matches(marker).count();
                // A one-line field writes a line break as a space; the markers have none.
                assert!(marker == 'x' || n == 0 || n == count, "frame {frame}: {n} of {count} {marker:?} went in: {events:?}");
            }
        }
    }
}

#[test]
fn two_thousand_random_frames_never_pass_the_cap_of_a_line() {
    random_frames(Cap::Chars(16), false);
}

#[test]
fn two_thousand_random_frames_never_pass_the_cap_of_a_text_of_several_lines() {
    random_frames(Cap::Bytes(64), true);
}
