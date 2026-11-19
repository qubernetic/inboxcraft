// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The frame's four regions: the command line, the index, the body and the status line, measured from the frame sheet
//! of the shell, on whole pixels at every scale, never negative or no number whatever size the desktop reports.

use egui::accesskit::{Role, Toggled};
use egui::{Rect, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_model::Role as Folder;
use inboxcraft_time::{FakeClock, Timestamp};

use super::{self as shell, COMMAND_LINE, INDEX, INDEX_NARROW, NARROW_INDEX_UNDER, Regions, STATUS_LINE};
use crate::mailbox::{Mailbox, Pile, Place};
use crate::theme::{Mode, TextScale, Theme};
use crate::{InboxApp, sample};

fn theme(percent: u16) -> Theme {
    Theme::new(Mode::Dark, TextScale::from_percent(percent).unwrap())
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect::from_min_size(pos2(x, y), vec2(w, h))
}

fn every(regions: &Regions) -> [(&'static str, Rect); 4] {
    [("command", regions.command), ("index", regions.index), ("body", regions.body), ("status", regions.status)]
}

#[test]
fn the_regions_are_the_frame_s() {
    assert_eq!((COMMAND_LINE, STATUS_LINE, INDEX, INDEX_NARROW, NARROW_INDEX_UNDER), (44.0, 26.0, 232.0, 56.0, 1280.0));
    // The sheet at 1440 × 900: the window's outline (1) round everything; the command line 1438 × 44, a hairline under
    // it; the index 232 and the body 1205 wide, 826 high, one hairline between them; a hairline over the status line,
    // 1438 × 26.
    let regions = shell::regions(vec2(1440.0, 900.0), &theme(100), 1.0);
    assert_eq!(regions.command, rect(1.0, 1.0, 1438.0, 44.0));
    assert_eq!(regions.index, rect(1.0, 46.0, 232.0, 826.0));
    assert_eq!(regions.body, rect(234.0, 46.0, 1205.0, 826.0));
    assert_eq!(regions.status, rect(1.0, 873.0, 1438.0, 26.0));
    // At 130 %: the heights × 1.3 to the nearest even pixel, the widths stay.
    let large = shell::regions(vec2(1440.0, 900.0), &theme(130), 1.0);
    assert_eq!((large.command.height(), large.status.height()), (58.0, 34.0));
    assert_eq!((large.index.width(), large.body.width()), (232.0, 1205.0));
    assert_eq!(large.body.top(), 1.0 + 58.0 + 1.0);
    assert_eq!(large.status.bottom(), 899.0);
    // Under 1280 the index is the narrow one.
    let narrow = shell::regions(vec2(1100.0, 900.0), &theme(100), 1.0);
    assert_eq!(narrow.index.width(), 56.0);
    assert_eq!(narrow.body, rect(58.0, 46.0, 1100.0 - 58.0 - 1.0, 826.0));
    assert_eq!(shell::regions(vec2(1280.0, 900.0), &theme(100), 1.0).index.width(), 232.0);
    // Never narrower than 800.
    assert_eq!(shell::regions(vec2(799.0, 900.0), &theme(100), 1.0), shell::regions(vec2(800.0, 900.0), &theme(100), 1.0));
}

#[test]
fn every_edge_is_on_a_whole_pixel() {
    for ppp in [1.0, 1.25, 1.5, 2.0] {
        for percent in [100, 130] {
            for size in [vec2(1440.0, 900.0), vec2(1100.0, 700.0), vec2(1283.3, 811.7), vec2(800.0, 600.0)] {
                let regions = shell::regions(size, &theme(percent), ppp);
                for (name, rect) in every(&regions) {
                    for edge in [rect.left(), rect.right(), rect.top(), rect.bottom()] {
                        let pixels = edge * ppp;
                        assert!((pixels - pixels.round()).abs() < 1e-3, "{ppp} {percent} {size:?}: {name} edge {edge} is not whole");
                    }
                }
                // The regions touch nothing of each other: one hairline at least between them.
                assert!(regions.index.right() < regions.body.left(), "{ppp} {percent}: a line between index and body");
                assert!(regions.command.bottom() < regions.body.top() && regions.body.bottom() < regions.status.top(), "{ppp} {percent}");
            }
        }
    }
}

#[test]
fn any_size_from_the_desktop_lays_out_as_at_least_800_wide() {
    let least = shell::regions(vec2(800.0, 0.0), &theme(100), 1.0);
    let sizes = [
        vec2(0.0, 0.0),
        vec2(-50.0, -50.0),
        vec2(f32::NAN, f32::NAN),
        vec2(f32::INFINITY, f32::INFINITY),
        vec2(f32::NEG_INFINITY, 900.0),
        vec2(f32::MAX, f32::MAX),
        vec2(1.0e-30, 3.0),
        vec2(799.0, 1.0),
    ];
    for size in sizes {
        for ppp in [1.0, 1.5, 0.0, -1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 1.0e-6, 1.0e-30, f32::MIN_POSITIVE, 1.0e37, f32::MAX] {
            for percent in [90, 100, 130] {
                let regions = shell::regions(size, &theme(percent), ppp);
                for (name, rect) in every(&regions) {
                    assert!(rect.is_finite(), "{size:?} {ppp}: {name} is a number");
                    assert!(rect.width() >= 0.0 && rect.height() >= 0.0, "{size:?} {ppp}: {name} {rect:?} is not negative");
                }
                let at_least = shell::regions(vec2(800.0, 0.0), &theme(percent), ppp);
                assert!(regions.command.right() >= at_least.command.right(), "{size:?} {ppp}: laid out 800 wide at least");
                assert!(regions.status.bottom() >= at_least.status.bottom(), "{size:?} {ppp}: as high as the regions need at least");
                assert!(regions.status.bottom() >= regions.command.bottom(), "{size:?} {ppp}: the status line under the command line");
            }
        }
    }
    assert!(least.status.top() >= least.body.bottom(), "the least height holds every region");
}

/// The sample window `size` points large, drawn once.
fn window(size: egui::Vec2) -> Harness<'static, InboxApp> {
    let clock = || Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()));
    let app = InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    let mut harness =
        Harness::builder().with_size(size).with_context_setup(crate::theme::install_fonts).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

#[test]
fn the_window_does_not_narrow_its_index_yet() {
    let harness = window(vec2(1100.0, 760.0));
    let laid = shell::laid(&harness.ctx).unwrap();
    assert_eq!(laid.index.width(), INDEX, "the window keeps the wide index until the widths slice turns the breakpoint on");
    assert_eq!(laid.body.left() - laid.index.left(), INDEX + 1.0, "the body one hairline right of the index");
    // The index's rows stand in the index, the list in the body.
    let home = harness.get_by_role_and_label(Role::Button, "Home").rect();
    assert!(laid.index.contains_rect(home), "{home:?} in {:?}", laid.index);
    let head = harness.get_by_label("the inbox").rect();
    assert!(laid.body.contains_rect(head), "{head:?} in {:?}", laid.body);
}

#[test]
fn the_status_line_is_reserved_and_holds_nothing_yet() {
    let mut harness = window(vec2(1300.0, 820.0));
    let status = shell::laid(&harness.ctx).unwrap().status;
    let inside: Vec<String> = harness
        .root()
        .children_recursive()
        .filter(|node| {
            node.accesskit_node().bounding_box().is_some_and(|b| {
                let ppp = harness.ctx.pixels_per_point();
                let rect = Rect::from_min_max(pos2(b.x0 as f32 / ppp, b.y0 as f32 / ppp), pos2(b.x1 as f32 / ppp, b.y1 as f32 / ppp));
                rect.intersects(status.shrink(0.5)) && !rect.contains_rect(status)
            })
        })
        .map(|node| format!("{:?} {:?}", node.accesskit_node().role(), node.accesskit_node().label()))
        .collect();
    assert!(inside.is_empty(), "nothing stands in the status line: {inside:?}");
    // A press there reaches nothing: nothing is selected, opened or shown.
    let (place, selected) = (harness.state().mailbox().place(), harness.state().mailbox().selected());
    let at = status.center();
    harness.hover_at(at);
    harness.run();
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
        harness.run();
    }
    assert_eq!((harness.state().mailbox().place(), harness.state().mailbox().selected()), (place, selected));
    assert!(!harness.state().settings().is_open());
}

#[test]
fn the_index_s_places_and_mailboxes_go_where_the_rail_and_the_folder_pane_went() {
    let mut harness = window(vec2(1300.0, 820.0));
    let press = |harness: &mut Harness<'static, InboxApp>, name: &str| {
        harness.get_by_role_and_label(Role::Button, name).click();
        harness.run();
    };
    let shown = |harness: &Harness<'static, InboxApp>, name: &str| harness.get_by_role_and_label(Role::Button, name).accesskit_node().toggled();
    press(&mut harness, "Later");
    assert_eq!(harness.state().mailbox().place(), Place::Later(Pile::Pins), "Later opens on the pins");
    assert_eq!(shown(&harness, "Later"), Some(Toggled::True));
    press(&mut harness, "Mail");
    assert_eq!(harness.state().mailbox().place(), Place::View(Folder::Inbox), "Mail leaves Later for the inbox");
    press(&mut harness, "Calendar");
    assert_eq!(harness.state().screen(), crate::Screen::Calendar);
    assert_eq!(shown(&harness, "Calendar"), Some(Toggled::True));
    // A mailbox pressed on the calendar shows the mail, on that mailbox.
    press(&mut harness, "Sent");
    assert_eq!((harness.state().screen(), harness.state().mailbox().place()), (crate::Screen::Mail, Place::View(Folder::Sent)));
    // Home is a place: a press shows it, the mail kept as it was under it.
    press(&mut harness, "Home");
    assert_eq!((harness.state().screen(), harness.state().mailbox().place()), (crate::Screen::Home, Place::View(Folder::Sent)));
    assert_eq!(shown(&harness, "Home"), Some(Toggled::True));
    // Settings is a place, the one shown while it shows; Esc does not close it, SETTINGS again shows the place left.
    press(&mut harness, "Settings");
    assert!(harness.state().settings().is_open());
    assert_eq!(shown(&harness, "Settings"), Some(Toggled::True));
    harness.key_press(egui::Key::Escape);
    harness.run();
    assert!(harness.state().settings().is_open());
    press(&mut harness, "Settings");
    assert!(!harness.state().settings().is_open());
    assert_eq!(
        (harness.state().screen(), harness.state().mailbox().place()),
        (crate::Screen::Home, Place::View(Folder::Sent)),
        "Home, left for them"
    );
    assert_eq!(shown(&harness, "Settings"), Some(Toggled::False));
}

/// What a press under a modal surface must not change: the screen, the mailbox shown, the account narrowed to, the
/// index's account rows (open or not) and whether the account picker is open.
fn under(harness: &Harness<'static, InboxApp>) -> (crate::Screen, Place, Option<u64>, Vec<String>, bool) {
    let rows = harness
        .query_all_by_role(Role::Button)
        .filter_map(|node| node.accesskit_node().label())
        .filter(|label| label.starts_with("Personal"))
        .collect();
    let picker = harness.query_by_label("All accounts").is_some();
    (harness.state().screen(), harness.state().mailbox().place(), harness.state().mailbox().narrowed(), rows, picker)
}

#[test]
fn a_screen_reader_s_activate_on_the_index_runs_nothing_under_a_modal_surface() {
    // The settings are not one of them: they are a place (below).
    use crate::keymap;
    let presses = ["Mail", "Calendar", "Later", "Sent", "Inbox", "Personal", "Show mail of: mira@example.org", "New message"];
    for surface in ["editor", "palette"] {
        let mut harness = window(vec2(1300.0, 820.0));
        let ctx = harness.ctx.clone();
        match surface {
            "editor" => {
                harness.state_mut().run_action(&ctx, keymap::CALENDAR);
                harness.run();
                harness.get_by_role_and_label(Role::Button, "New event").click();
                harness.run();
                harness.get_by_role_and_label(Role::TextInput, "Title").focus();
                harness.get_by_role_and_label(Role::TextInput, "Title").type_text("Kiln");
                harness.run();
            }
            _ => {
                harness.state_mut().run_action(&ctx, keymap::PALETTE);
                harness.run();
                harness.state_mut().command_mut().set_text(">zz");
                harness.run();
            }
        }
        let before = under(&harness);
        for name in presses {
            let Some(node) = harness
                .query_all_by_role(Role::Button)
                .find(|node| node.accesskit_node().label().is_some_and(|label| label == name || label.starts_with(&format!("{name},"))))
            else {
                panic!("{surface}: no button {name}");
            };
            node.click_accesskit();
            harness.run();
            assert_eq!(under(&harness), before, "{surface}: an activate on {name} changed what lies under it");
            assert!(harness.state().composer().is_none(), "{surface}: {name} opened no composer");
            match surface {
                "editor" => {
                    let summary = harness.state().calendar().editor().map(|editor| editor.form().summary.clone());
                    assert_eq!(summary.as_deref(), Some("Kiln"), "{surface}: {name}: the editor stays with its typing");
                }
                _ => {
                    // The command line's results are no modal layer: the keyboard taken from its field closes them
                    // (one surface); what was typed stays, and nothing under them ran (`under` above).
                    assert_eq!(harness.state().command().text(), "zz", "{surface}: {name}: its typing whole");
                }
            }
        }
    }
}

#[test]
fn on_the_settings_a_place_or_mailbox_of_the_index_does_what_its_key_does() {
    // The settings are a place, not a surface in front: a press, a screen reader's activate and the key alike leave
    // them and show what was asked for.
    let mut harness = window(vec2(1300.0, 820.0));
    let ctx = harness.ctx.clone();
    harness.state_mut().run_action(&ctx, crate::keymap::SETTINGS);
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Calendar").click_accesskit();
    harness.run();
    assert!(!harness.state().settings().is_open(), "the settings are left, as with Ctrl 2");
    assert_eq!(harness.state().screen(), crate::Screen::Calendar);
    harness.state_mut().run_action(&ctx, crate::keymap::SETTINGS);
    harness.run();
    let sent = harness.query_all_by_role(Role::Button).find(|node| node.accesskit_node().label().is_some_and(|label| label == "Sent")).unwrap();
    sent.click_accesskit();
    harness.run();
    assert!(!harness.state().settings().is_open(), "a mailbox pressed leaves the settings");
    assert_eq!((harness.state().screen(), harness.state().mailbox().place()), (crate::Screen::Mail, Place::View(Folder::Sent)));
}

#[test]
fn tab_walks_new_message_in_the_command_line_before_the_index_under_it() {
    let mut harness = window(vec2(1300.0, 820.0));
    let focused = |harness: &Harness<'static, InboxApp>| {
        harness.root().children_recursive().find(|node| node.accesskit_node().is_focused()).and_then(|node| node.accesskit_node().label())
    };
    let mut walked = Vec::new();
    for _ in 0..3 {
        harness.key_press(egui::Key::Tab);
        harness.run();
        walked.push(focused(&harness).unwrap_or_default());
    }
    // Home is a place and a stop, first of the index.
    assert_eq!(walked, ["New message", "Command line", "Home"], "the drawn order, top down");
    harness.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::Tab);
    harness.run();
    harness.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::Tab);
    harness.run();
    assert_eq!(focused(&harness).as_deref(), Some("New message"), "Shift Tab walks it back");
}

#[test]
fn a_press_on_the_index_from_the_docked_composer_keeps_the_draft_and_the_keyboard_on_a_drawn_control() {
    let focused = |harness: &Harness<'static, InboxApp>| {
        harness.root().children_recursive().find(|node| node.accesskit_node().is_focused()).and_then(|node| node.accesskit_node().label())
    };
    // Beside the docked composer the list does not take keys: the keyboard goes back to the Subject; an account row
    // shows no list and keeps it.
    for (name, on) in [("Sent", "Subject"), ("Later", "Subject"), ("Mail", "Subject"), ("Studio", "Studio")] {
        let mut harness = window(vec2(1300.0, 820.0));
        let ctx = harness.ctx.clone();
        harness.state_mut().run_action(&ctx, crate::keymap::COMPOSE);
        harness.run();
        harness.get_by_role_and_label(Role::TextInput, "Subject").focus();
        harness.get_by_role_and_label(Role::TextInput, "Subject").type_text("Plot");
        harness.run();
        let row = harness
            .query_all_by_role(Role::Button)
            .find(|node| node.accesskit_node().label().is_some_and(|label| label == name || label.starts_with(&format!("{name},"))));
        let Some(row) = row else { panic!("no row {name}") };
        row.click();
        harness.run();
        let at = focused(&harness);
        assert!(at.as_deref().is_some_and(|label| label.starts_with(on)), "{name}: the keyboard on {on}, not {at:?}");
        assert_eq!(harness.state().composer().map(|composer| composer.subject().to_owned()).as_deref(), Some("Plot"), "{name}: the draft kept");
    }
}

#[test]
fn after_a_press_on_the_index_the_list_takes_its_keys_at_once() {
    for name in ["Inbox", "Mail", "Sent"] {
        let mut harness = window(vec2(1300.0, 820.0));
        let row = harness
            .query_all_by_role(Role::Button)
            .find(|node| node.accesskit_node().label().is_some_and(|label| label == name || label.starts_with(&format!("{name},"))));
        let Some(row) = row else { panic!("no row {name}") };
        row.click();
        harness.run();
        let mut picked = Vec::new();
        for key in [egui::Key::J, egui::Key::ArrowDown, egui::Key::J] {
            harness.key_press(key);
            harness.run();
            picked.push(harness.state().mailbox().selected());
        }
        assert!(picked.iter().all(Option::is_some), "{name}: the list took J, Down, J: {picked:?}");
        assert!(picked[0] != picked[1] && picked[1] != picked[2], "{name}: each key moved the selection: {picked:?}");
    }
}
