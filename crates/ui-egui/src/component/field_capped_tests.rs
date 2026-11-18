// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::TextBuffer as _;
use egui::text::CharIndex;

use super::{Cap, Capped, JOURNAL_BYTES, JOURNAL_STEPS, Outcome};

const START: &str = "aé€😀b, then more";

/// One change through the trait, as egui makes them.
#[derive(Debug, Clone, Copy)]
enum Op {
    Insert,
    Delete,
    Replace,
    Clear,
    ReplaceWith,
}

const OPS: [Op; 5] = [Op::Insert, Op::Delete, Op::Replace, Op::Clear, Op::ReplaceWith];

fn apply(buffer: &mut Capped<'_>, op: Op) {
    match op {
        Op::Insert => {
            let _ = buffer.insert_text("ő→", CharIndex(2));
        }
        Op::Delete => buffer.delete_char_range(CharIndex(1)..CharIndex(4)),
        Op::Replace => {
            buffer.delete_char_range(CharIndex(0)..CharIndex(2));
            let _ = buffer.insert_text("🦀x", CharIndex(0));
        }
        Op::Clear => buffer.clear(),
        Op::ReplaceWith => buffer.replace_with("ünïcödé"),
    }
}

#[test]
fn every_order_of_changes_rolls_back_to_the_text_the_frame_began_with() {
    for cap in [Cap::Bytes(64), Cap::Chars(32)] {
        for first in OPS {
            for second in OPS {
                for third in OPS {
                    let mut text = START.to_owned();
                    text.reserve(256);
                    let (pointer, room) = (text.as_ptr(), text.capacity());
                    let mut buffer = Capped::new(&mut text, cap, cap_most(cap), true, false);
                    for op in [first, second, third] {
                        apply(&mut buffer, op);
                        assert!(buffer.as_str().is_char_boundary(buffer.as_str().len()));
                    }
                    // Past the limit: refused, and the whole frame goes back.
                    assert_eq!(buffer.insert_text(&"z".repeat(100), CharIndex(0)), 0);
                    assert_eq!(buffer.as_str(), START, "{cap:?} {first:?} {second:?} {third:?}");
                    // After the refusal the frame changes nothing more.
                    apply(&mut buffer, Op::Clear);
                    assert_eq!(buffer.insert_text("q", CharIndex(0)), 0);
                    assert_eq!(buffer.finish(), Outcome { refused: true, changed: false });
                    drop(buffer);
                    assert_eq!(text, START);
                    assert_eq!((text.as_ptr(), text.capacity()), (pointer, room), "the same buffer");
                }
            }
        }
    }
}

fn cap_most(cap: Cap) -> usize {
    match cap {
        Cap::Bytes(most) | Cap::Chars(most) => most,
    }
}

#[test]
fn changes_within_the_limit_are_kept_and_said() {
    let mut text = START.to_owned();
    let mut buffer = Capped::new(&mut text, Cap::Chars(32), 32, true, false);
    assert_eq!(buffer.insert_text("ő→", CharIndex(2)), 2);
    buffer.delete_char_range(CharIndex(0)..CharIndex(1));
    assert_eq!(buffer.finish(), Outcome { refused: false, changed: true });
    drop(buffer);
    assert_eq!(text, "éő→€😀b, then more");
}

#[test]
fn indices_past_the_text_or_backwards_never_panic_nor_split_a_character() {
    let mut text = START.to_owned();
    let mut buffer = Capped::new(&mut text, Cap::Bytes(64), 64, true, false);
    buffer.delete_char_range(CharIndex(5)..CharIndex(2));
    buffer.delete_char_range(CharIndex(100)..CharIndex(200));
    assert_eq!(buffer.as_str(), START);
    assert_eq!(buffer.insert_text("!", CharIndex(1000)), 1);
    assert!(buffer.as_str().ends_with('!'));
    buffer.delete_char_range(CharIndex(3)..CharIndex(usize::MAX));
    assert_eq!(buffer.as_str(), "aé€");
    assert!(buffer.finish().changed);
}

#[test]
fn a_buffer_that_is_not_mutable_takes_nothing() {
    let mut text = START.to_owned();
    let mut buffer = Capped::new(&mut text, Cap::Bytes(64), 64, false, false);
    assert!(!buffer.is_mutable());
    for op in OPS {
        apply(&mut buffer, op);
    }
    assert_eq!(buffer.finish(), Outcome { refused: false, changed: false });
    drop(buffer);
    assert_eq!(text, START);
}

#[test]
fn a_frame_of_10000_changes_keeps_a_bounded_journal_and_still_rolls_back() {
    let mut text = START.to_owned();
    let mut buffer = Capped::new(&mut text, Cap::Bytes(64), 64, true, false);
    for _ in 0..10_000 {
        let _ = buffer.insert_text("ab", CharIndex(0));
        buffer.delete_char_range(CharIndex(0)..CharIndex(2));
        assert!(buffer.journal.len() <= JOURNAL_STEPS, "{} steps", buffer.journal.len());
        assert!(buffer.removed <= START.len().max(JOURNAL_BYTES) + 64, "{} bytes", buffer.removed);
    }
    assert!(buffer.base.is_some(), "past the bound it keeps one copy of the text instead");
    assert_eq!(buffer.insert_text(&"z".repeat(100), CharIndex(0)), 0);
    assert_eq!(buffer.as_str(), START);
    drop(buffer);
    // An undo of a long text again and again: the removed bytes stay bounded too.
    let long = "é".repeat(30_000);
    let mut text = long.clone();
    let mut buffer = Capped::new(&mut text, Cap::Bytes(1 << 20), 1 << 20, true, false);
    for round in 0..200 {
        buffer.replace_with(if round % 2 == 0 { "short" } else { &long });
        assert!(buffer.removed <= long.len().max(JOURNAL_BYTES) + long.len(), "{} bytes", buffer.removed);
    }
    assert_eq!(buffer.insert_text(&"z".repeat(1 << 20), CharIndex(0)), 0);
    assert_eq!(buffer.as_str(), long);
}

#[test]
fn a_password_buffer_grows_only_into_a_new_one_and_never_moves_while_it_has_room() {
    let mut text = String::with_capacity(16);
    let pointer = text.as_ptr();
    let mut buffer = Capped::new(&mut text, Cap::Chars(256), 256, true, true);
    assert_eq!(buffer.insert_text("0123456789", CharIndex(0)), 10);
    drop(buffer);
    assert_eq!(text.as_ptr(), pointer, "room enough: the same buffer");
    let mut buffer = Capped::new(&mut text, Cap::Chars(256), 256, true, true);
    assert_eq!(buffer.insert_text("0123456789", CharIndex(10)), 10);
    drop(buffer);
    assert!(text.capacity() >= 256 * 4, "it grew once to hold the whole cap: {}", text.capacity());
    let (pointer, room) = (text.as_ptr(), text.capacity());
    let mut buffer = Capped::new(&mut text, Cap::Chars(256), 256, true, true);
    for _ in 0..236 {
        let _ = buffer.insert_text("𝄞", CharIndex(0));
    }
    assert_eq!(buffer.insert_text("x", CharIndex(0)), 0, "past the cap");
    drop(buffer);
    assert_eq!((text.as_ptr(), text.capacity()), (pointer, room), "rolled back in place");
    assert_eq!(text, "01234567890123456789");
}
