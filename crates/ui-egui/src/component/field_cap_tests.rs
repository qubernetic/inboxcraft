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

use super::tests::{Kind, ONE_FRAME, UNDRAWN, at_the_cap, cap_of, field_built, field_of, inside, kind_name, one_frame, press, says_too_long};
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
fn a_text_loaded_over_its_cap_opens_whole_edits_down_and_never_grows_past_its_opening_size() {
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
        // Up to the size it opened with it may grow again (an undo of the deletion would): past it, never.
        one_frame(&mut harness, vec![Event::Text("q".to_owned())]);
        one_frame(&mut harness, vec![Event::Text("q".to_owned())]);
        assert_eq!(harness.state().len(), most + 2, "{kind:?}: back to its opening size");
        let was = before(&harness, kind);
        one_frame(&mut harness, vec![Event::Text("q".to_owned())]);
        refused(&harness, kind, &was, "past its opening size");
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
        let pick = seed.below(15);
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
        if pick == 14 {
            events.push(Event::Cut);
            continue;
        }
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

/// 2 000 random frames of one to four events each in a field holding at most `cap` (several `lines` or one), opening
/// with `start`, kept `focused` or never focused by the test (a middle click or a screen reader may focus it): after
/// each frame the text is within its limit (the cap, or the size it opened with if that is more), what was laid out
/// is within twice that, and in a frame without keys every marker's text is in whole or not at all.
fn random_frames(cap: Cap, lines: bool, start: &str, focused: bool) {
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
        start.to_owned(),
    );
    harness.run_ok();
    let rect = harness.get_by_label("Random").rect();
    let at = pos2(rect.center().x, if rect.top() >= 0.0 { rect.top() + 8.0 } else { rect.bottom() - 16.0 });
    let most = cap.most();
    let measure = |text: &str| cap.measure(text);
    let limit = most.max(measure(start));
    let mut seed = Seeded(0x1234_5678_9abc_def1);
    for frame in 0..2_000 {
        if focused {
            // A one-line field lets go of the focus on Enter: it is given back before every frame.
            harness.ctx.memory_mut(|memory| memory.request_focus(id));
            assert!(harness.ctx.memory(|memory| memory.has_focus(id)), "frame {frame}: focused");
        }
        let was = harness.state().clone();
        let absent: Vec<char> = MARKERS.iter().copied().filter(|marker| !was.contains(*marker)).collect();
        let (events, carried, keys) = random_frame(&mut seed, id, at, most, &absent);
        LAID_OUT.with(|laid| laid.set(0));
        harness.input_mut().events.extend(events.clone());
        harness.step();
        let now = harness.state();
        let laid = LAID_OUT.with(std::cell::Cell::get);
        assert!(measure(now) <= limit, "frame {frame}: {} past {limit}: {events:?}", measure(now));
        // Twice the limit: in bytes for a text of several lines, at most four bytes a character for a line.
        let bound = if lines { 2 * limit } else { 2 * 4 * limit };
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
    random_frames(Cap::Chars(16), false, "", true);
    random_frames(Cap::Chars(16), false, &"y".repeat(20), true);
    random_frames(Cap::Chars(16), false, "", false);
}

#[test]
fn two_thousand_random_frames_never_pass_the_cap_of_a_text_of_several_lines() {
    random_frames(Cap::Bytes(64), true, "", true);
    random_frames(Cap::Bytes(64), true, &"y".repeat(80), true);
    random_frames(Cap::Bytes(64), true, "", false);
}

#[test]
fn a_refusal_on_a_field_never_focused_loses_nothing_to_a_later_undo() {
    for kind in ONE_FRAME {
        let (most, _) = cap_of(kind);
        let full = "y".repeat(most);
        for set_value in [false, true] {
            let case = if set_value { "a screen reader's value" } else { "a middle click" };
            let mut harness = field_built(kind, &full, theme());
            let at = inside(&harness, kind);
            one_frame(&mut harness, vec![Event::PointerMoved(at)]);
            let refused_frame = if set_value { vec![reader(id_of(kind), Action::SetValue, &"w".repeat(most + 1))] } else { middle(at, "m") };
            one_frame(&mut harness, refused_frame);
            assert_eq!(harness.state(), &full, "{kind:?} {case}: refused whole");
            assert!(says_too_long(&harness), "{kind:?} {case}: and said");
            // Focused later: one character deleted, time for egui's history to keep it, then undo and redo, one with a
            // character typed in its frame.
            harness.get_by_label(kind_name(kind)).focus();
            harness.run_ok();
            one_frame(&mut harness, press(Key::End, Modifiers::CTRL).into());
            one_frame(&mut harness, press(Key::Backspace, Modifiers::NONE).into());
            harness.run_steps(90);
            let undo: Vec<Event> = press(Key::Z, Modifiers::COMMAND).into();
            let mut undo_and_type = undo.clone();
            undo_and_type.push(Event::Text("a".to_owned()));
            let redo: Vec<Event> = press(Key::Y, Modifiers::COMMAND).into();
            for (step, events) in [undo.clone(), undo.clone(), undo_and_type, undo, redo.clone(), redo].into_iter().enumerate() {
                one_frame(&mut harness, events);
                let text = harness.state();
                assert!(text.starts_with(&full[..most - 1]) && size(kind, text) <= most, "{kind:?} {case} step {step}: {} left", text.len());
            }
        }
    }
}

#[test]
fn a_text_opened_at_twice_its_cap_can_undo_a_deletion_and_never_grows_past_its_opening_size() {
    for kind in ONE_FRAME {
        let (most, _) = cap_of(kind);
        let opened = "y".repeat(2 * most);
        let mut harness = field_of(kind, &opened);
        one_frame(&mut harness, press(Key::A, Modifiers::COMMAND).into());
        one_frame(&mut harness, press(Key::Backspace, Modifiers::NONE).into());
        assert_eq!(harness.state(), "", "{kind:?}: all of it deleted");
        one_frame(&mut harness, press(Key::Z, Modifiers::COMMAND).into());
        if kind == Kind::Password {
            // A password keeps no history: nothing comes back, and nothing is refused.
            assert_eq!(harness.state(), "", "{kind:?}");
            one_frame(&mut harness, vec![Event::Paste(opened.clone())]);
        }
        assert_eq!(harness.state(), &opened, "{kind:?}: the deletion is undone");
        one_frame(&mut harness, press(Key::End, Modifiers::CTRL).into());
        let was = before(&harness, kind);
        one_frame(&mut harness, vec![Event::Paste("z".to_owned())]);
        refused(&harness, kind, &was, "past its opening size");
        // Changed from outside to a text within the cap: the limit is the cap again.
        *harness.state_mut() = "y".repeat(most - 1);
        harness.run_ok();
        one_frame(&mut harness, press(Key::End, Modifiers::CTRL).into());
        let was = before(&harness, kind);
        one_frame(&mut harness, vec![Event::Paste("zz".to_owned())]);
        refused(&harness, kind, &was, "past the cap after a change from outside");
        one_frame(&mut harness, vec![Event::Paste("z".to_owned())]);
        assert_eq!(size(kind, harness.state()), most, "{kind:?}: up to the cap");
    }
}

#[test]
fn a_text_opened_over_its_cap_undoes_a_deletion_after_the_field_went_undrawn() {
    // A password keeps no history: the three others are the composer's message text, a field of several lines and a
    // line, which another screen undraws (Mail, then Calendar, then Mail again).
    for kind in [Kind::Text, Kind::Many, Kind::PlainText] {
        let (most, _) = cap_of(kind);
        let opened = "y".repeat(2 * most);
        let mut harness = field_of(kind, &opened);
        one_frame(&mut harness, press(Key::A, Modifiers::COMMAND).into());
        one_frame(&mut harness, press(Key::Backspace, Modifiers::NONE).into());
        assert_eq!(harness.state(), "", "{kind:?}: all of it deleted");
        UNDRAWN.with(|undrawn| undrawn.set(true));
        harness.run_steps(5);
        UNDRAWN.with(|undrawn| undrawn.set(false));
        harness.run_ok();
        harness.get_by_label(kind_name(kind)).focus();
        harness.run_ok();
        harness.run_steps(90);
        one_frame(&mut harness, press(Key::Z, Modifiers::COMMAND).into());
        assert!(harness.state() == &opened, "{kind:?}: the deletion is undone ({} of {})", harness.state().len(), opened.len());
        assert!(!says_too_long(&harness), "{kind:?}: and nothing was refused");
        one_frame(&mut harness, press(Key::End, Modifiers::CTRL).into());
        let was = before(&harness, kind);
        one_frame(&mut harness, vec![Event::Paste("z".to_owned())]);
        refused(&harness, kind, &was, "past its opening size after the field went undrawn");
    }
}

#[test]
fn a_change_from_outside_clears_the_field_s_history() {
    // Another text under the same field (a new draft): Ctrl Z never brings back the one before it.
    for kind in [Kind::Text, Kind::Many, Kind::PlainText] {
        let mut harness = field_of(kind, "first");
        one_frame(&mut harness, press(Key::End, Modifiers::CTRL).into());
        one_frame(&mut harness, vec![Event::Text("!".to_owned())]);
        harness.run_steps(90);
        *harness.state_mut() = "another".to_owned();
        harness.run_ok();
        one_frame(&mut harness, press(Key::Z, Modifiers::COMMAND).into());
        assert_eq!(harness.state(), "another", "{kind:?}: the history of the text before is gone");
    }
}

#[test]
fn a_password_s_buffer_never_moves_nor_grows() {
    use crate::onboard::Secret;
    let theme = theme();
    let id = egui::Id::unique("password");
    let mut harness = builder().build_ui_state(
        move |ui, secret: &mut Secret| {
            password_field(ui, &theme, id, secret.expose_mut(), &Field::new("Password", ""), super::Secret::Typed);
        },
        Secret::default(),
    );
    harness.run();
    harness.get_by_label("Password").focus();
    harness.run();
    let buffer = |harness: &mut Harness<'static, Secret>| {
        let text = harness.state_mut().expose_mut();
        (text.as_ptr(), text.capacity())
    };
    let frame = |harness: &mut Harness<'static, Secret>, events: Vec<Event>| {
        harness.input_mut().events.extend(events);
        harness.step();
        harness.run();
    };
    let first = buffer(&mut harness);
    // Typed to the cap, four bytes a character: the buffer is full.
    for _ in 0..super::MAX_SECRET {
        frame(&mut harness, vec![Event::Text("𝄞".to_owned())]);
    }
    assert_eq!(harness.state().expose().chars().count(), super::MAX_SECRET);
    assert_eq!(buffer(&mut harness), first, "typing to the cap");
    frame(&mut harness, vec![Event::Paste("QQQQQ".to_owned())]);
    assert_eq!(buffer(&mut harness), first, "a refused paste");
    frame(&mut harness, vec![reader(id, Action::SetValue, &"W".repeat(300))]);
    assert_eq!(buffer(&mut harness), first, "a refused value");
    // One character selected and replaced by two: the deletion is rolled back with the refused insert.
    frame(&mut harness, press(Key::ArrowLeft, Modifiers::SHIFT).into());
    frame(&mut harness, vec![Event::Paste("ab".to_owned())]);
    assert_eq!(buffer(&mut harness), first, "a rolled-back replacement");
    assert_eq!(harness.state().expose(), "𝄞".repeat(super::MAX_SECRET), "the password is as it was");
    // A deletion and a refused insert in one frame, at the start of the text.
    let mut events: Vec<Event> = press(Key::Home, Modifiers::NONE).into();
    events.extend(press(Key::Delete, Modifiers::NONE));
    events.push(Event::Text("ab".to_owned()));
    frame(&mut harness, events);
    assert_eq!(buffer(&mut harness), first, "a rolled-back frame at the start");
    assert_eq!(harness.state().expose(), "𝄞".repeat(super::MAX_SECRET), "the password is as it was");
}
