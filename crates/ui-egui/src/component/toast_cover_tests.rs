// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Where the window's toasts stand over the controls under them, and what a press there reaches: the "Draft saved"
//! toast (no control of its own) and an act's toast (with Undo) over the reader, at 1300 × 872, 1000 × 700 and
//! 800 × 600, at 100 % and 130 % text; and the keyboard, which reaches the reader's answers while a toast shows.

use egui::accesskit::Role;
use egui::{Event, Id, Key, LayerId, Modifiers, Order, PointerButton, Pos2, Rect, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_time::{FakeClock, Timestamp};

use crate::InboxApp;
use crate::compose::Answer;
use crate::mailbox::{Act, Mailbox};
use crate::sample;
use crate::theme::TextScale;

const SIZES: [(f32, f32); 3] = [(1300.0, 872.0), (1000.0, 700.0), (800.0, 600.0)];
const PERCENTS: [u16; 2] = [100, 130];

/// The reader's answers at its foot.
const ANSWERS: [&str; 3] = ["Reply", "Reply all", "Forward"];

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

/// Which toast shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shown {
    /// "Draft saved": Reply opened the composer and it was closed again.
    DraftSaved,
    /// An act's toast, with Undo: the open message was flagged.
    Act,
}

/// The sample window `size` big at `percent` text, its first message open in the reader and the toast `shown` risen
/// (a second of steps: it shows for six).
fn with_toast(size: (f32, f32), percent: u16, shown: Shown) -> Harness<'static, InboxApp> {
    let mut app = InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    app.set_text_scale(TextScale::from_percent(percent).unwrap());
    let id = app.mailbox().messages().first().map(|row| row.id).unwrap();
    app.mailbox_mut().select(Some(id));
    let mut harness = Harness::builder().with_size(vec2(size.0, size.1)).with_max_steps(12).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    match shown {
        Shown::DraftSaved => {
            assert!(harness.state_mut().answer(id, Answer::Reply));
            harness.run();
            harness.state_mut().close_composer();
        }
        Shown::Act => harness.state_mut().mailbox_mut().act(id, Act::Flag),
    }
    for _ in 0..4 {
        harness.step();
    }
    match shown {
        Shown::DraftSaved => assert_eq!(harness.state().notice(), Some("Draft saved")),
        Shown::Act => assert!(harness.state().mailbox().undoable().is_some_and(|undo| undo.undoable)),
    }
    assert!(harness.state().composer().is_none());
    harness
}

/// The layer of the toast that shows.
fn toast_layer(harness: &Harness<'static, InboxApp>, shown: Shown) -> LayerId {
    let id = match shown {
        Shown::DraftSaved => Id::unique("compose-toast"),
        Shown::Act => Id::unique("act-toast").with(harness.state().mailbox().undoable().map_or(0, |undo| undo.serial)),
    };
    LayerId::new(Order::Foreground, id)
}

/// The rect of the toast's float.
fn toast_rect(harness: &Harness<'static, InboxApp>, shown: Shown) -> Rect {
    let id = toast_layer(harness, shown).id;
    harness.ctx.memory(|memory| memory.area_rect(id)).unwrap()
}

/// Every control of the frame but the toast's own: its name (up to its first comma) and rect.
fn controls(harness: &Harness<'static, InboxApp>) -> Vec<(String, Rect)> {
    harness
        .root()
        .children_recursive()
        .filter(|node| matches!(node.accesskit_node().role(), Role::Button | Role::TextInput | Role::ComboBox | Role::Link | Role::CheckBox))
        .map(|node| {
            let name = node.accesskit_node().label().unwrap_or_default();
            (name.split(',').next().unwrap_or_default().to_owned(), node.rect())
        })
        .filter(|(name, _)| name != "Undo")
        .collect()
}

/// The controls the toast `shown` stands over.
fn covered(harness: &Harness<'static, InboxApp>, shown: Shown) -> Vec<(String, Rect)> {
    let toast = toast_rect(harness, shown);
    controls(harness).into_iter().filter(|(_, rect)| rect.intersects(toast)).collect()
}

/// A press and release of the primary button at `at`, and the frames after it.
fn press(harness: &mut Harness<'static, InboxApp>, at: Pos2) {
    harness.event(Event::PointerMoved(at));
    harness.event(Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    harness.event(Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    harness.step();
    harness.step();
}

/// Where the toast `shown` stands and which controls it covers, one line per window size and text size.
fn table(shown: Shown) -> Vec<String> {
    let mut lines = Vec::new();
    for size in SIZES {
        for percent in PERCENTS {
            let harness = with_toast(size, percent, shown);
            let at = toast_rect(&harness, shown);
            let names: Vec<String> = covered(&harness, shown).into_iter().map(|(name, _)| name).collect();
            lines.push(format!("{}x{} {percent}: {:.0},{:.0} to {:.0},{:.0} over {names:?}", size.0, size.1, at.min.x, at.min.y, at.max.x, at.max.y));
        }
    }
    lines
}

#[test]
fn the_draft_saved_toast_stands_at_the_foot_of_the_window_s_middle_and_covers_these_controls() {
    // Centred at the foot of the body beside the index, over the reader's answers at 1300 × 872 and over the thread's
    // folded row or the answers in the one pane of 800 × 600; clear of them at 1000 × 700.
    let expected = [
        r#"1300x872 100: 715,770 to 818,808 over ["Reply", "Reply all"]"#,
        r#"1300x872 130: 704,755 to 829,800 over ["Reply", "Reply all"]"#,
        r#"1000x700 100: 565,598 to 668,636 over []"#,
        r#"1000x700 130: 554,583 to 679,628 over []"#,
        r#"800x600 100: 473,498 to 576,536 over ["Mira Holloway", "Forward"]"#,
        r#"800x600 130: 462,483 to 587,528 over ["Reply all", "Forward"]"#,
    ];
    assert_eq!(table(Shown::DraftSaved), expected);
}

/// Words that name a control a stray press must never reach: one that deletes, files as done, junks or sends.
const DESTRUCTIVE: [&str; 4] = ["delete", "done", "junk", "send"];

#[test]
fn no_control_that_deletes_files_junks_or_sends_lies_under_the_draft_saved_toast() {
    // A press on the "Draft saved" float reaches what lies under it: at every tested size and text size that is an
    // answer or a folded row of the thread, never a control whose name says delete, done, junk or send.
    for size in SIZES {
        for percent in PERCENTS {
            let harness = with_toast(size, percent, Shown::DraftSaved);
            let under: Vec<String> = covered(&harness, Shown::DraftSaved).into_iter().map(|(name, _)| name).collect();
            let found: Vec<&String> = under.iter().filter(|name| DESTRUCTIVE.iter().any(|word| name.to_lowercase().contains(word))).collect();
            assert!(found.is_empty(), "{size:?} {percent}: {found:?} under the toast");
        }
    }
}

#[test]
fn an_act_s_toast_with_undo_stands_where_the_draft_saved_toast_does_and_covers_these_controls() {
    // Wider, with Undo and its key: at 1000 × 700 and 130 % it reaches the answers too.
    let expected = [
        r#"1300x872 100: 673,776 to 861,808 over ["Reply", "Reply all"]"#,
        r#"1300x872 130: 655,760 to 879,800 over ["Reply", "Reply all"]"#,
        r#"1000x700 100: 523,604 to 711,636 over []"#,
        r#"1000x700 130: 505,588 to 729,628 over ["Reply", "Forward"]"#,
        r#"800x600 100: 431,504 to 619,536 over ["Mira Holloway", "Reply all", "Forward"]"#,
        r#"800x600 130: 413,488 to 637,528 over ["Reply all", "Forward"]"#,
    ];
    assert_eq!(table(Shown::Act), expected);
}

/// Each answer under the toast `shown` with a point to press it at: inside both, 2 in from the side of their overlap,
/// and not on the toast's Undo; at every size and scale.
fn answers_under(shown: Shown) -> Vec<((f32, f32), u16, String, Pos2)> {
    let mut found = Vec::new();
    for size in SIZES {
        for percent in PERCENTS {
            let harness = with_toast(size, percent, shown);
            let toast = toast_rect(&harness, shown);
            let undo = harness.query_all_by_label("Undo").map(|node| node.rect()).collect::<Vec<_>>();
            for (name, rect) in covered(&harness, shown).into_iter().filter(|(name, _)| ANSWERS.contains(&name.as_str())) {
                let both = rect.intersect(toast);
                let points = [Pos2::new(both.left() + 2.0, both.center().y), Pos2::new(both.right() - 2.0, both.center().y)];
                if let Some(at) = points.into_iter().find(|at| !undo.iter().any(|undo| undo.expand(1.0).contains(*at))) {
                    found.push((size, percent, name, at));
                }
            }
        }
    }
    found
}

#[test]
fn a_press_on_an_answer_under_the_draft_saved_toast_reaches_the_answer() {
    // The toast has no control of its own: it takes no press, so an answer under it still answers.
    let under = answers_under(Shown::DraftSaved);
    assert!(under.len() >= 4, "the toast covers answers at two sizes: {under:?}");
    for (size, percent, name, at) in under {
        let mut harness = with_toast(size, percent, Shown::DraftSaved);
        assert_ne!(harness.ctx.layer_id_at(at), Some(toast_layer(&harness, Shown::DraftSaved)), "{size:?} {percent}: {name}");
        press(&mut harness, at);
        assert!(harness.state().composer().is_some(), "{size:?} {percent}: {name} under the toast at {at:?} answers");
    }
}

#[test]
fn a_press_on_an_answer_under_an_act_s_toast_with_undo_reaches_the_toast_and_does_nothing() {
    // Today's fault: a toast with Undo takes presses over its whole float, so an answer under it cannot be pressed by
    // the pointer until the toast goes (six seconds). The keyboard still reaches it (the next test).
    let under = answers_under(Shown::Act);
    assert!(!under.is_empty(), "the toast covers answers");
    for (size, percent, name, at) in under {
        let mut harness = with_toast(size, percent, Shown::Act);
        assert_eq!(harness.ctx.layer_id_at(at), Some(toast_layer(&harness, Shown::Act)), "{size:?} {percent}: {name}");
        press(&mut harness, at);
        assert!(harness.state().composer().is_none(), "{size:?} {percent}: {name} under the toast at {at:?}");
        assert!(harness.state().mailbox().undoable().is_some(), "{size:?} {percent}: the press did not undo either");
    }
}

#[test]
fn reply_is_reached_by_the_keyboard_while_either_toast_shows() {
    // Reply has no key of its own: the focus reaches it, and Enter presses it.
    for shown in [Shown::DraftSaved, Shown::Act] {
        for size in SIZES {
            for percent in PERCENTS {
                let mut harness = with_toast(size, percent, shown);
                harness.get_by_role_and_label(Role::Button, "Reply").focus();
                harness.step();
                harness.key_press(Key::Enter);
                harness.step();
                assert!(harness.state().composer().is_some(), "{shown:?} {size:?} {percent}: Enter on Reply");
            }
        }
    }
}
