// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The settings sheet over the dimmed window: the sections on the left (Appearance, Accounts, Notifications, Triage,
//! Sending and Shortcuts; the others not built yet), the chosen page on the right. Notifications holds each account's
//! level (all, people only, none), whether returned snoozes and follow-up reminders notify, and the two switches of
//! the app's [`crate::desk::Desk`]: running in the background and starting with the computer. Accounts is drawn in `accounts.rs`. Triage holds
//! the screener's mode, how the inbox shows its groups, whether a pin outlasts done and each account's grouping. Sending holds
//! "Undo send", how long every send waits before it goes (5, 10, 30 or 60 seconds), and each account's signature, saved
//! with its Save button. Shortcuts lists every action of
//! the window with its key: a press on the key waits for the next key pressed, which becomes the action's. Each setting is a row: its name and a note on the left, its control on the right (or
//! under them where the row is narrow): a segmented choice for the mode (Auto, Dark, Light), cards for the
//! window frame, a slider for the text size and a switch for "Reduce motion"; a preview in the look the controls
//! choose closes the page.

use egui::{
    Align, Area, Frame, Id, Key, Layout, Order, Painter, Rect, Response, RichText, Role, ScrollArea, Sense, Stroke, StrokeKind, TextEdit, Ui,
    UiBuilder, WidgetInfo, pos2, vec2,
};
use inboxcraft_engine::ScreenerMode;
use inboxcraft_engine::appearance::{Appearance, Motion, ThemeMode, WindowFrame};
use inboxcraft_engine::notify::Level;

use super::paint::{NavRow, focus_ring, line, line_height, nav_row, selection};
use crate::component::avatar;
use crate::component::button::{IconButton, TextButton};
use crate::component::field;
use crate::component::segmented::{segmented, segmented_for};
use crate::component::switch::switch;
use crate::component::tooltip;
use crate::draw::paint::{self, FloatEdge};
use crate::iconset::{IconId, Style};
use crate::keymap::{Action, Keymap};
use crate::mailbox::{InboxView, Mailbox};
use crate::onboard::Onboarding;
use crate::settings::{Section, Settings};
use crate::theme::{Face, Font, TextScale, Theme, Weight};
use crate::tr;

/// The sheet's size where the window has room for it, in points.
const SHEET: [f32; 2] = [1100.0, 780.0];

/// The width of the sections' column, in points.
const NAV: f32 = 240.0;

/// The narrowest a row's name and note get beside its control, in points; narrower, the control goes under them.
const TEXT_MIN: f32 = 240.0;

/// A window on a window-frame card, in points.
const THUMB: [f32; 2] = [84.0, 52.0];

/// The slider's track, in points.
const TRACK: [f32; 2] = [200.0, 6.0];

/// What the owner asked for on the sheet this frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Asked {
    Mode(ThemeMode),
    Scale(TextScale),
    ReduceMotion(bool),
    Frame(WindowFrame),
    /// Another page of the sheet.
    Section(Section),
    /// Add account on the Accounts page: the form opens on it.
    Add,
    /// An account on the Accounts page: its form opens there.
    Edit(u64),
    /// Back from the form or the check to the list of accounts.
    Back,
    /// Something on the form or the check.
    Page(super::onboard::Asked),
    /// Wait for the next key pressed, for the action.
    Capture(&'static str),
    /// Take the action's key away.
    Unbind(&'static str),
    /// Give every action its first key again.
    ResetKeys,
    /// Save the signature of the account as typed.
    Signature(u64),
    /// The screener's mode.
    Screener(ScreenerMode),
    /// How the inbox shows its groups.
    InboxView(InboxView),
    /// Group the account's inbox, or not.
    Grouped(u64, bool),
    /// Whether a pin stays on a message when it is done.
    KeepPins(bool),
    /// How long every send is held before it goes, in seconds.
    Hold(u8),
    /// Which new mail of the account notifies.
    Level(u64, Level),
    /// Whether returned snoozes notify.
    Returns(bool),
    /// Whether follow-up reminders notify.
    Reminders(bool),
    /// Whether closing the window leaves InboxCraft running.
    Background(bool),
    /// Whether InboxCraft starts with the computer.
    Login(bool),
    Close,
}

/// What the sheet shows and changes.
pub(super) struct Sheet<'a> {
    pub(super) keymap: &'a Keymap,
    pub(super) settings: &'a mut Settings,
    pub(super) current: &'a Appearance,
    pub(super) reduce: bool,
    pub(super) mailbox: &'a Mailbox,
    pub(super) onboarding: &'a mut Onboarding,
    /// Whether the window can add accounts.
    pub(super) can_add: bool,
    /// What the app's desk says now.
    pub(super) desk: DeskView,
}

/// What the app's [`crate::desk::Desk`] says this frame; all `None` without one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct DeskView {
    /// Whether closing the window leaves InboxCraft running, where it can.
    pub(super) background: Option<bool>,
    /// Whether InboxCraft starts with the computer, where it can.
    pub(super) login: Option<bool>,
    /// A change of starting with the computer waits for the desktop's answer.
    pub(super) asking: bool,
}

/// The modes in the order the page shows them.
const MODES: [ThemeMode; 3] = [ThemeMode::System, ThemeMode::Dark, ThemeMode::Light];

/// The name of a mode.
fn mode_name(mode: ThemeMode) -> &'static str {
    match mode {
        ThemeMode::System => tr!("Auto"),
        ThemeMode::Light => tr!("Light"),
        ThemeMode::Dark => tr!("Dark"),
    }
}

/// The name of a window frame.
fn frame_name(frame: WindowFrame) -> &'static str {
    match frame {
        WindowFrame::System => tr!("Title bar"),
        WindowFrame::TopRow => tr!("Top row"),
        WindowFrame::Strip => tr!("Strip"),
    }
}

/// The sections of the settings, with their icons and the page of those that are built.
fn sections() -> [(IconId, &'static str, Option<Section>); 9] {
    [
        (IconId::Palette, tr!("Appearance"), Some(Section::Appearance)),
        (IconId::At, tr!("Accounts"), Some(Section::Accounts)),
        (IconId::Notify, tr!("Notifications"), Some(Section::Notifications)),
        (IconId::Funnel, tr!("Triage"), Some(Section::Triage)),
        (IconId::Send, tr!("Sending"), Some(Section::Sending)),
        (IconId::Calendar, tr!("Calendar"), None),
        (IconId::Keyboard, tr!("Shortcuts"), Some(Section::Shortcuts)),
        (IconId::Shield, tr!("Privacy"), None),
        (IconId::Info, tr!("About"), None),
    ]
}

/// The name of a section.
fn section_name(section: Section) -> &'static str {
    match section {
        Section::Appearance => tr!("Appearance"),
        Section::Accounts => tr!("Accounts"),
        Section::Shortcuts => tr!("Shortcuts"),
        Section::Sending => tr!("Sending"),
        Section::Triage => tr!("Triage"),
        Section::Notifications => tr!("Notifications"),
    }
}

/// The name of a notification level.
fn level_name(level: Level) -> &'static str {
    match level {
        Level::All => tr!("All"),
        Level::People => tr!("People only"),
        Level::None => tr!("None"),
    }
}

/// The name of a screener mode.
pub(super) fn screener_name(mode: ScreenerMode) -> &'static str {
    match mode {
        ScreenerMode::Before => tr!("Before the inbox"),
        ScreenerMode::Inside => tr!("Inside, marked"),
        ScreenerMode::Off => tr!("Off"),
    }
}

/// `font` in the SemiBold weight.
fn semibold(font: Font) -> Font {
    Font { face: Face::Sans(Weight::SemiBold), ..font }
}

fn note(ui: &mut Ui, theme: &Theme, text: &str) {
    ui.add(egui::Label::new(RichText::new(text).font(theme.fonts.caption.id()).color(theme.palette.muted)).wrap());
}

/// A row's name and note.
fn texts(ui: &mut Ui, theme: &Theme, title: &str, notes: &[&str]) {
    ui.spacing_mut().item_spacing.y = 2.0;
    ui.label(RichText::new(title).font(semibold(theme.fonts.body).id()).color(theme.palette.ink));
    for text in notes {
        note(ui, theme, text);
    }
}

/// A setting: its `title` and `notes` on the left and `control` at the right end, or under them when the row has no
/// room for both, and a line under it. The control is measured in a pass that draws nothing and takes no press.
fn row(ui: &mut Ui, theme: &Theme, title: &str, notes: &[&str], control: &mut dyn FnMut(&mut Ui)) {
    row_with(ui, theme, None, title, notes, control);
}

/// The name and notes of a row, `width` wide, after its `icon` in the accent colour when it has one.
fn heading(ui: &mut Ui, theme: &Theme, icon: Option<IconId>, title: &str, notes: &[&str], width: f32) {
    let Some(icon) = icon else {
        ui.vertical(|ui| texts(ui, theme, title, notes));
        return;
    };
    ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
        let side = line_height(ui, theme.fonts.body) * 1.2;
        let (rect, _) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
        crate::iconset::paint(ui.painter(), rect, icon, Style::Regular, theme.palette.cyan_text);
        ui.add_space(theme.spacing.s1);
        let used = side + theme.spacing.s1 + ui.spacing().item_spacing.x * 2.0;
        ui.vertical(|ui| {
            ui.set_width((width - used).max(1.0));
            texts(ui, theme, title, notes);
        });
    });
}

/// A row as [`row`] draws it, with an `icon` before its name.
pub(super) fn row_with(ui: &mut Ui, theme: &Theme, icon: Option<IconId>, title: &str, notes: &[&str], control: &mut dyn FnMut(&mut Ui)) {
    let s = &theme.spacing;
    ui.add_space(s.s4);
    let width = ui.available_width();
    let mut sizing = ui
        .new_child(UiBuilder::new().max_rect(ui.available_rect_before_wrap()).layout(Layout::left_to_right(Align::Center)).sizing_pass().invisible());
    control(&mut sizing);
    let need = sizing.min_rect().width();
    let room = width - need - s.s4;
    if room < TEXT_MIN {
        heading(ui, theme, icon, title, notes, width);
        ui.add_space(s.s1);
        ui.horizontal_wrapped(|ui| control(ui));
    } else {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.set_width(room);
                heading(ui, theme, icon, title, notes, room);
            });
            ui.add_space((width - room - need - 2.0 * ui.spacing().item_spacing.x).max(0.0));
            control(ui);
        });
    }
    ui.add_space(s.s3);
    let y = ui.cursor().top();
    ui.painter().hline(ui.max_rect().x_range(), y, Stroke::new(1.0, theme.palette.line));
    ui.add_space(1.0);
}

/// A card: a picture `swatch` big drawn by `paint`, its `name` under it; framed in the focus colour while `on`.
fn card(ui: &mut Ui, theme: &Theme, name: &str, on: bool, swatch: [f32; 2], paint: impl FnOnce(&Painter, Rect)) -> Response {
    let p = &theme.palette;
    let s = &theme.spacing;
    let caption = semibold(theme.fonts.caption);
    let galley = line(ui, &[(name, caption, p.ink)], f32::INFINITY);
    let size = vec2(swatch[0].max(galley.size().x) + s.s2 * 2.0, s.s2 + swatch[1] + s.s1 + galley.size().y + s.s2);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| WidgetInfo::selected(Role::Button, true, on, name));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        if response.hovered() {
            painter.rect_filled(rect, 0.0, p.stage);
        }
        let stroke = Stroke::new(1.0, if on { p.cyan_edge } else { p.line });
        painter.rect_stroke(rect, 0.0, stroke, StrokeKind::Inside);
        let picture = Rect::from_min_size(pos2(rect.center().x - swatch[0] / 2.0, rect.top() + s.s2), vec2(swatch[0], swatch[1]));
        paint(painter, picture);
        painter.galley(pos2(rect.center().x - galley.size().x / 2.0, picture.bottom() + s.s1), galley, p.ink);
        focus_ring(ui, theme, &response);
    }
    response
}

/// A small window as `frame` draws it: the desktop's bar, or the window controls in the top row or on a strip.
fn thumb(painter: &Painter, theme: &Theme, rect: Rect, frame: WindowFrame) {
    let p = &theme.palette;
    painter.rect_filled(rect, 0.0, p.bg);
    let bar = if frame == WindowFrame::System { 9.0 } else { 0.0 };
    if bar > 0.0 {
        painter.rect_filled(Rect::from_min_size(rect.min, vec2(rect.width(), bar)), 0.0, p.muted);
    }
    let rail = Rect::from_min_size(pos2(rect.left(), rect.top() + bar), vec2(9.0, rect.height() - bar));
    painter.rect_filled(rail, 0.0, p.panel);
    painter.vline(rail.right() - 0.5, rail.y_range(), Stroke::new(1.0, p.line));
    let top = rect.top() + bar + if frame == WindowFrame::Strip { 9.0 } else { 3.0 };
    for (x, w) in [(12.0, 22.0), (37.0, 44.0)] {
        painter.rect_filled(Rect::from_min_max(pos2(rect.left() + x, top), pos2(rect.left() + x + w, rect.bottom() - 3.0)), 0.0, p.surface);
    }
    if frame != WindowFrame::System {
        let y = if frame == WindowFrame::Strip { rect.top() + 3.5 } else { top + 3.5 };
        for i in 0..3_u8 {
            painter.circle_filled(pos2(rect.right() - 7.5 - f32::from(i) * 5.0, y), 1.5, p.ink);
        }
    }
    painter.rect_stroke(rect, 0.0, Stroke::new(1.0, p.line), StrokeKind::Inside);
}

/// The text size as a slider over the steps from 90 % to 130 %, a small and a large A at its ends and the size after
/// it. A press or a drag sets the step under the pointer; the arrow keys move it one step.
fn text_size(ui: &mut Ui, theme: &Theme, now: TextScale) -> Option<TextScale> {
    let p = &theme.palette;
    let steps = TextScale::ALL;
    let last = steps.len().saturating_sub(1).max(1);
    let index = steps.iter().position(|step| *step == now).unwrap_or(1);
    ui.label(RichText::new("A").font(theme.fonts.caption.id()).color(p.muted));
    let knob = 16.0;
    let (rect, response) = ui.allocate_exact_size(vec2(TRACK[0], knob), Sense::click_and_drag());
    let name = tr!("Text size");
    response.widget_info(|| WidgetInfo::slider(true, f64::from(now.percent()), name));
    let mut wanted = None;
    if response.clicked() || response.dragged() {
        response.request_focus();
        if let Some(at) = response.interact_pointer_pos() {
            let t = ((at.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
            // The nearest step; the cast is of a small index.
            wanted = Some((t * last as f32).round() as usize);
        }
    }
    if response.has_focus() {
        let (left, right) = ui.input(|input| (input.key_pressed(Key::ArrowLeft), input.key_pressed(Key::ArrowRight)));
        if left {
            wanted = Some(index.saturating_sub(1));
        } else if right {
            wanted = Some((index + 1).min(last));
        }
    }
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let track = Rect::from_center_size(rect.center(), vec2(TRACK[0], TRACK[1]));
        painter.rect_filled(track, 0.0, p.stage);
        // An index of five steps.
        let x = track.left() + track.width() * index as f32 / last as f32;
        let done = Rect::from_min_max(track.min, pos2(x.max(track.left() + TRACK[1]), track.bottom()));
        painter.rect_filled(done, 0.0, p.cyan);
        let knob_rect = Rect::from_center_size(pos2(x, rect.center().y), vec2(knob, knob));
        painter.rect_filled(knob_rect, 0.0, p.term);
        painter.rect_stroke(knob_rect, 0.0, Stroke::new(1.0, p.muted), StrokeKind::Inside);
        if response.has_focus() {
            painter.rect_stroke(knob_rect.expand(2.0), 0.0, Stroke::new(1.0, p.cyan_edge), StrokeKind::Outside);
        }
    }
    ui.label(RichText::new("A").font(theme.fonts.title.id()).color(p.muted));
    ui.label(RichText::new(tr!("{percent} %", percent = now.percent())).font(semibold(theme.fonts.body).id()).color(p.ink));
    wanted.and_then(|at| steps.get(at).copied()).filter(|step| *step != now)
}

/// The rows of the Appearance page; what was asked for.
fn rows(ui: &mut Ui, theme: &Theme, current: &Appearance, reduce: bool) -> Option<Asked> {
    let mut asked = None;
    let f = &theme.fonts;
    let modes = MODES.map(|mode| (mode, mode_name(mode), f.button));
    row(ui, theme, tr!("Mode"), &[tr!("Auto follows the computer's light or dark choice.")], &mut |ui| {
        if let Some(mode) = segmented(ui, theme, &modes, current.theme) {
            asked = Some(Asked::Mode(mode));
        }
    });
    row(ui, theme, tr!("Window frame"), &[tr!("The system's title bar, or a frame in the app's own colours.")], &mut |ui| {
        for frame in WindowFrame::ALL {
            if card(ui, theme, frame_name(frame), current.frame == frame, THUMB, |painter, rect| thumb(painter, theme, rect, frame)).clicked() {
                asked = Some(Asked::Frame(frame));
            }
        }
    });
    let now = TextScale::from_percent(current.text_scale).unwrap_or_default();
    row(ui, theme, tr!("Text size"), &[tr!("90 % to 130 %. Rows and toolbars follow the font.")], &mut |ui| {
        if let Some(scale) = text_size(ui, theme, now) {
            asked = Some(Asked::Scale(scale));
        }
    });
    let mut motion_notes = vec![tr!("Every motion lands at its end at once.")];
    if current.motion == Motion::System {
        motion_notes.push(tr!("Until you choose, motion follows the system where InboxCraft can read it, and runs elsewhere."));
    }
    row(ui, theme, tr!("Reduce motion"), &motion_notes, &mut |ui| {
        if switch(ui, theme, tr!("Reduce motion"), reduce).clicked() {
            asked = Some(Asked::ReduceMotion(!reduce));
        }
    });
    asked
}

/// A message row of the preview, drawn like the list's.
fn preview_row(ui: &mut Ui, theme: &Theme, sender: (&str, &str), subject: &str, time: &str, selected: bool, unread: bool) {
    let p = &theme.palette;
    let s = &theme.spacing;
    let f = &theme.fonts;
    let first = line_height(ui, f.sender_unread).max(line_height(ui, f.caption));
    let second = line_height(ui, f.subject);
    let height = first + second + s.s2 * 1.5;
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let painter = ui.painter();
    if selected {
        selection(painter, theme, rect, 0.0);
    }
    let side = (first + second).min(height - s.s2);
    let face = Rect::from_min_size(pos2(rect.left() + s.s3 + s.s1, rect.center().y - side / 2.0), vec2(side, side));
    avatar::paint(painter, theme, face, sender.0, sender.1);
    if unread {
        painter.circle_filled(pos2(rect.left() + s.s2, rect.center().y), 3.5, p.cyan_edge);
    }
    let left = face.right() + s.s3;
    let time = line(ui, &[(time, f.caption, p.muted)], rect.width());
    let right = rect.right() - s.s3;
    let top = rect.top() + s.s2 * 0.75;
    painter.galley(pos2(right - time.size().x, top + (first - time.size().y) / 2.0), time.clone(), p.muted);
    let sender_font = if unread { f.sender_unread } else { f.sender };
    let name = line(ui, &[(sender.0, sender_font, p.ink)], (right - time.size().x - s.s2 - left).max(1.0));
    painter.galley(pos2(left, top + (first - name.size().y) / 2.0), name, p.ink);
    let subject = line(ui, &[(subject, if unread { f.subject_unread } else { f.subject }, p.ink)], (right - left).max(1.0));
    painter.galley(pos2(left, top + first), subject, p.ink);
}

/// The preview: two rows of the list, the first one selected, in `theme`.
fn preview(ui: &mut Ui, theme: &Theme) {
    let p = &theme.palette;
    let s = &theme.spacing;
    ui.add_space(s.s3);
    ui.label(RichText::new(tr!("Preview")).font(semibold(theme.fonts.caption).id()).color(p.muted));
    ui.add_space(s.s1);
    Frame::new().fill(p.bg).stroke(Stroke::new(1.0, p.line)).corner_radius(0.0).inner_margin(s.s2).show(ui, |ui| {
        ui.set_width(ui.available_width());
        preview_row(ui, theme, ("Odile Brannock", "odile@example.org"), tr!("Allotment rota for November"), "08:35", true, true);
        preview_row(ui, theme, ("Teodor Lisk", "teodor@example.org"), tr!("Photos from the coast walk"), tr!("Yesterday"), false, false);
    });
}

/// The sections on the left: the title, then a row each, the one shown selected; those not built yet are faded and
/// say so. The section pressed.
fn nav(ui: &mut Ui, theme: &Theme, keymap: &Keymap, shown: Section) -> Option<Section> {
    ui.spacing_mut().item_spacing.y = 2.0;
    ui.label(RichText::new(tr!("Settings")).font(theme.fonts.title.id()).color(theme.palette.ink));
    ui.add_space(theme.spacing.s2);
    let mut pressed = None;
    for (icon, name, section) in sections() {
        let row = NavRow {
            text: name,
            spoken: tr!("{section}, settings page", section = name),
            badge: 0,
            selected: section == Some(shown),
            depth: 0,
            icon: Some(icon),
            enabled: section.is_some(),
        };
        let response = nav_row(ui, theme, &row);
        match section {
            Some(section) if response.clicked() && section != shown => pressed = Some(section),
            Some(_) => {}
            None => {
                tooltip::attach_note(response, theme, keymap, name, None, Some(tr!("Not available yet")));
            }
        }
    }
    pressed
}

/// The page shown: its title with Close, then Appearance's rows and preview, scrolling, or the Accounts page.
fn page(ui: &mut Ui, theme: &Theme, view: &mut Sheet<'_>) -> Option<Asked> {
    let mut asked = None;
    let keymap = view.keymap;
    let section = view.settings.section();
    super::frame::beside(ui, theme, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(section_name(section)).font(theme.fonts.title.id()).color(theme.palette.ink));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if IconButton::new(IconId::Close, tr!("Close")).show(ui, theme, keymap).clicked() {
                    asked = Some(Asked::Close);
                }
            });
        });
    });
    ui.add_space(theme.spacing.s2);
    let on_page = match section {
        Section::Appearance => appearance(ui, theme, view.current, view.reduce),
        Section::Accounts => super::accounts::page(ui, theme, keymap, view.mailbox, view.onboarding, view.can_add),
        Section::Shortcuts => shortcuts(ui, theme, keymap, view.settings),
        Section::Sending => sending(ui, theme, keymap, view.settings, view.mailbox),
        Section::Triage => triage(ui, theme, view.mailbox),
        Section::Notifications => notifications(ui, theme, view.mailbox, view.settings, view.desk),
    };
    asked.or(on_page)
}

/// The Appearance page under its title: the rows and the preview, scrolling.
fn appearance(ui: &mut Ui, theme: &Theme, current: &Appearance, reduce: bool) -> Option<Asked> {
    let mut asked = None;
    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.set_max_width(ui.available_width() - theme.spacing.s3);
        asked = rows(ui, theme, current, reduce);
        preview(ui, theme);
    });
    asked
}

/// A key as a chip that can be pressed: `shown` in a rounded frame, ringed in the focus colour while it `waits`.
fn key_button(ui: &mut Ui, theme: &Theme, shown: &str, spoken: &str, waits: bool) -> Response {
    let p = &theme.palette;
    let s = &theme.spacing;
    let ink = if waits { p.cyan_text } else { p.ink };
    let galley = line(ui, &[(shown, theme.fonts.caption, ink)], f32::INFINITY);
    let size = galley.size() + vec2(s.s2 * 2.0, s.s1 * 2.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, spoken));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        painter.rect_filled(rect, 0.0, if response.hovered() { p.grid } else { p.panel });
        let edge = if waits { Stroke::new(2.0, p.cyan_edge) } else { Stroke::new(1.0, p.line) };
        painter.rect_stroke(rect, 0.0, edge, StrokeKind::Inside);
        painter.galley(rect.center() - galley.size() / 2.0, galley, ink);
        focus_ring(ui, theme, &response);
    }
    response
}

/// The key of `action` on its row, drawn from the right in `ui`: the chip, which waits for a key when pressed, and
/// Remove before it while it has one.
fn key_control(ui: &mut Ui, theme: &Theme, keymap: &Keymap, action: &Action, waits: bool) -> Option<Asked> {
    let mut asked = None;
    let chip = keymap.chip(ui.ctx(), action.id);
    let (shown, spoken) = match (&chip, waits) {
        (_, true) => (tr!("press a key…").to_owned(), tr!("{name}: press a key, or Escape to keep the old one", name = action.name)),
        (Some(chip), false) => (chip.clone(), tr!("Key of {name}: {key}", name = action.name, key = chip)),
        (None, false) => (tr!("No key").to_owned(), tr!("{name} has no key", name = action.name)),
    };
    if key_button(ui, theme, &shown, &spoken, waits).clicked() && !waits {
        asked = Some(Asked::Capture(action.id));
    }
    if chip.is_some() && !waits {
        let remove = tr!("Remove the key of {name}", name = action.name);
        if IconButton::new(IconId::Close, &remove).show(ui, theme, keymap).clicked() {
            asked = Some(Asked::Unbind(action.id));
        }
    }
    asked
}

/// One action of the Shortcuts page: its name, and its key at the right end; a line under it.
fn key_row(ui: &mut Ui, theme: &Theme, keymap: &Keymap, action: &Action, waits: bool) -> Option<Asked> {
    let s = &theme.spacing;
    let height = line_height(ui, theme.fonts.body) + s.s3 * 2.0;
    let asked = ui
        .allocate_ui_with_layout(vec2(ui.available_width(), height), Layout::left_to_right(Align::Center), |ui| {
            ui.set_min_height(height);
            ui.label(RichText::new(action.name).font(theme.fonts.body.id()).color(theme.palette.ink));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| key_control(ui, theme, keymap, action, waits)).inner
        })
        .inner;
    ui.painter().hline(ui.max_rect().x_range(), ui.cursor().top(), Stroke::new(1.0, theme.palette.line));
    ui.add_space(1.0);
    asked
}

/// The Shortcuts page under its title: a field that narrows the list and Reset all keys, the note on the last key
/// given, then every action with its key, scrolling.
fn shortcuts(ui: &mut Ui, theme: &Theme, keymap: &Keymap, settings: &mut Settings) -> Option<Asked> {
    let s = &theme.spacing;
    let mut asked = None;
    note(ui, theme, tr!("Every action can have a key. One key runs one action: giving a key takes it from the action that had it."));
    ui.add_space(s.s2);
    ui.horizontal(|ui| {
        let width = (ui.available_width() * 0.6).max(1.0);
        let find = field::search(ui, theme, Id::unique("shortcuts-find"), settings.find_mut(), tr!("Find a command"), width);
        ui.ctx().accesskit_node_builder(find.id, |node| node.set_label(tr!("Find a command")));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if TextButton::new(tr!("Reset all keys")).show(ui, theme, keymap).clicked() {
                asked = Some(Asked::ResetKeys);
            }
        });
    });
    if let Some(text) = settings.keys_note() {
        ui.add_space(s.s1);
        note(ui, theme, text);
    }
    let waiting = settings.capturing();
    let list = crate::palette::matching(settings.find());
    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.set_max_width(ui.available_width() - s.s3);
        if list.is_empty() {
            ui.add_space(s.s4);
            note(ui, theme, &tr!("No command matches “{text}”.", text = settings.find()));
        }
        for action in &list {
            if let Some(pressed) = key_row(ui, theme, keymap, action, waiting == Some(action.id)) {
                asked = Some(pressed);
            }
        }
    });
    asked
}

/// The holds "Undo send" offers, in seconds.
const HOLDS: [u8; 4] = [5, 10, 30, 60];

/// The Sending page under its title: "Undo send" with the hold's seconds, then each account with its signature in a
/// field, and Save while it differs from the one the account has; scrolling.
fn sending(ui: &mut Ui, theme: &Theme, keymap: &Keymap, settings: &mut Settings, mailbox: &Mailbox) -> Option<Asked> {
    let s = &theme.spacing;
    let mut asked = None;
    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.set_max_width(ui.available_width() - s.s3);
        let hold = mailbox.hold_seconds();
        let labels = HOLDS.map(|seconds| tr!("{seconds} s", seconds = seconds));
        let holds: Vec<(u8, &str, Font)> = HOLDS.iter().zip(&labels).map(|(seconds, label)| (*seconds, label.as_str(), theme.fonts.button)).collect();
        let now = tr!("Every message waits here this long before it goes, so Undo can take it back. Now {seconds} s.", seconds = hold);
        row(ui, theme, tr!("Undo send"), &[tr!("Hold mail for a few seconds before it goes."), &now], &mut |ui| {
            if let Some(seconds) = segmented(ui, theme, &holds, hold) {
                asked = Some(Asked::Hold(seconds));
            }
        });
        ui.add_space(s.s4);
        note(
            ui,
            theme,
            tr!("A signature goes under “-- ” at the end of every new message from its account. The drafts already started keep theirs."),
        );
        if mailbox.accounts().is_empty() {
            ui.add_space(s.s4);
            note(ui, theme, tr!("Add an account first: a signature belongs to the account it is sent from."));
        }
        for account in mailbox.accounts() {
            ui.add_space(s.s4);
            heading(ui, theme, Some(IconId::At), &account.label, &[&account.address], ui.available_width());
            ui.add_space(s.s1);
            let label = tr!("Signature of {account}", account = account.label);
            let typed = settings.signature_mut(account.id, &account.signature);
            let edit = TextEdit::multiline(typed).id(Id::unique("signature").with(account.id)).desired_rows(3).desired_width(ui.available_width());
            let field = ui.add(edit);
            ui.ctx().accesskit_node_builder(field.id, |node| node.set_label(&label));
            let changed = settings.signature(account.id).is_some_and(|typed| typed != account.signature);
            ui.add_space(s.s1);
            let save = tr!("Save the signature of {account}", account = account.label);
            let button = TextButton::new(tr!("Save"));
            let response = if changed { button.show(ui, theme, keymap) } else { button.unavailable(tr!("Nothing changed")).show(ui, theme, keymap) };
            ui.ctx().accesskit_node_builder(response.id, |node| node.set_label(&save));
            if changed && response.clicked() {
                asked = Some(Asked::Signature(account.id));
            }
            ui.add_space(s.s3);
            ui.painter().hline(ui.max_rect().x_range(), ui.cursor().top(), Stroke::new(1.0, theme.palette.line));
            ui.add_space(1.0);
        }
    });
    asked
}

/// The Triage page under its title: the screener's mode, the inbox view, and a switch for each account's grouping;
/// scrolling.
fn triage(ui: &mut Ui, theme: &Theme, mailbox: &Mailbox) -> Option<Asked> {
    let mut asked = None;
    let f = &theme.fonts;
    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.set_max_width(ui.available_width() - theme.spacing.s3);
        let modes = ScreenerMode::ALL.map(|mode| (mode, screener_name(mode), f.button));
        let screener_notes = [
            tr!("The first mail of a sender you have not let in yet waits in New senders, shows in the inbox marked, or comes in as any other."),
            tr!("Each sender is judged once, across the accounts. Turning it on lets in everyone your mail already names."),
        ];
        row(ui, theme, tr!("Screener"), &screener_notes, &mut |ui| {
            if let Some(mode) = segmented(ui, theme, &modes, mailbox.screener_mode()) {
                asked = Some(Asked::Screener(mode));
            }
        });
        let views = InboxView::ALL.map(|view| (view, view.text(), f.button));
        let view_note = tr!("People, Updates, Newsletters and Invitations as cards, as one list under headings, or one list by date.");
        row(ui, theme, tr!("Inbox"), &[view_note], &mut |ui| {
            if let Some(view) = segmented(ui, theme, &views, mailbox.inbox_view()) {
                asked = Some(Asked::InboxView(view));
            }
        });
        let keep = mailbox.pins_outlast_done();
        let pins_note = tr!("Off, Done takes the pin off; on, a pinned message stays in Pins after it is done.");
        row(ui, theme, tr!("Keep pins on done mail"), &[pins_note], &mut |ui| {
            if switch(ui, theme, tr!("Keep pins on done mail"), keep).clicked() {
                asked = Some(Asked::KeepPins(!keep));
            }
        });
        if !mailbox.accounts().is_empty() {
            ui.add_space(theme.spacing.s6);
            let note =
                tr!("Each account's inbox in People, Updates, Newsletters and Invitations, sorted by InboxCraft or by your choice for a sender.");
            texts(ui, theme, tr!("Grouped inbox"), &[note]);
        }
        for account in mailbox.accounts() {
            let name = tr!("Group the inbox of {account}", account = account.label);
            let note = tr!("Off, the mail of {address} stays out of the groups.", address = account.address);
            row_with(ui, theme, Some(IconId::At), &account.label, &[&note], &mut |ui| {
                if switch(ui, theme, &name, account.grouped).clicked() {
                    asked = Some(Asked::Grouped(account.id, !account.grouped));
                }
            });
        }
    });
    asked
}

/// The Notifications page under its title: each account's level, returned snoozes and reminders, then running in
/// the background and starting with the computer, each switch unavailable where the app's desk cannot do it;
/// scrolling.
fn notifications(ui: &mut Ui, theme: &Theme, mailbox: &Mailbox, settings: &Settings, desk: DeskView) -> Option<Asked> {
    let mut asked = None;
    let f = &theme.fonts;
    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.set_max_width(ui.available_width() - theme.spacing.s3);
        note(
            ui,
            theme,
            tr!("Per account: all new mail, mail from people only, or none. Mail from your own addresses never notifies, and letting a sender in raises no burst of old mail."),
        );
        note(
            ui,
            theme,
            tr!("A notification shows who the mail is from and its subject; your desktop may keep it in its list and show it on the lock screen."),
        );
        if mailbox.accounts().is_empty() {
            ui.add_space(theme.spacing.s4);
            note(ui, theme, tr!("Add an account first: each account chooses which of its new mail notifies you."));
        }
        let levels = Level::ALL.map(|level| (level, level_name(level), f.button));
        for account in mailbox.accounts() {
            let title = tr!("{account} · {address}", account = account.label, address = account.address);
            row(ui, theme, &title, &[], &mut |ui| {
                if let Some(level) = segmented_for(ui, theme, &levels, mailbox.notify_level(account.id), Some(&account.label)) {
                    asked = Some(Asked::Level(account.id, level));
                }
            });
        }
        let chosen = mailbox.notify_settings();
        let (returns, reminders) = (chosen.returns, chosen.reminders);
        row(ui, theme, tr!("Returned snoozes"), &[tr!("A snoozed message that comes back notifies you.")], &mut |ui| {
            if switch(ui, theme, tr!("Returned snoozes"), returns).clicked() {
                asked = Some(Asked::Returns(!returns));
            }
        });
        row(ui, theme, tr!("Follow-up reminders"), &[tr!("A message you asked to be reminded of, when no reply came, notifies you.")], &mut |ui| {
            if switch(ui, theme, tr!("Follow-up reminders"), reminders).clicked() {
                asked = Some(Asked::Reminders(!reminders));
            }
        });
        if let Some(pressed) = in_the_background(ui, theme, settings, desk) {
            asked = Some(pressed);
        }
    });
    asked
}

/// The rows of running in the background and starting with the computer, as the app's desk allows them.
fn in_the_background(ui: &mut Ui, theme: &Theme, settings: &Settings, desk: DeskView) -> Option<Asked> {
    let mut asked = None;
    let elsewhere = tr!("Not available in this window");
    let mut notes = vec![tr!("Closing the window leaves InboxCraft running, so new mail, send later and snooze keep their times.")];
    if desk.background.is_none() {
        notes.push(elsewhere);
    }
    let background = desk.background.unwrap_or(false);
    row(ui, theme, tr!("Run in the background"), &notes, &mut |ui| {
        let pressed = ui.add_enabled_ui(desk.background.is_some(), |ui| switch(ui, theme, tr!("Run in the background"), background)).inner;
        if pressed.clicked() {
            asked = Some(Asked::Background(!background));
        }
    });
    let mut notes = vec![tr!("InboxCraft starts in the background when you log in.")];
    if desk.login.is_none() {
        notes.push(elsewhere);
    } else if desk.asking {
        notes.push(tr!("Waiting for the desktop's answer…"));
    } else if desk.background == Some(false) {
        notes.push(tr!("Turn on Run in the background first: a start at login opens no window."));
    }
    if let Some(text) = settings.login_note() {
        notes.push(text);
    }
    let login = desk.login.unwrap_or(false);
    let open = desk.login.is_some() && desk.background == Some(true) && !desk.asking;
    row(ui, theme, tr!("Start with the computer"), &notes, &mut |ui| {
        if ui.add_enabled_ui(open, |ui| switch(ui, theme, tr!("Start with the computer"), login)).inner.clicked() {
            asked = Some(Asked::Login(!login));
        }
    });
    asked
}

/// The sheet's place in `screen`: its size where there is room, and two gaps from each edge where there is not.
fn place(theme: &Theme, screen: Rect) -> Rect {
    let least = theme.spacing.s2 * 2.0;
    let margin = vec2(((screen.width() - SHEET[0]) / 2.0).max(least), ((screen.height() - SHEET[1]) / 2.0).max(least));
    screen.shrink2(margin)
}

/// Draws the settings sheet over the window, which it dims and keeps from taking presses; returns what was asked
/// for, applied by the caller after the frame's drawing.
pub(super) fn sheet(ctx: &egui::Context, theme: &Theme, mut view: Sheet<'_>) -> Option<Asked> {
    let p = &theme.palette;
    let s = &theme.spacing;
    let screen = ctx.content_rect();
    // A press beside the sheet leaves it, and reaches nothing under it.
    let beside = Area::new(Id::unique("settings-dim"))
        .order(Order::Middle)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let (rect, response) = ui.allocate_exact_size(screen.size(), Sense::click());
            ui.painter().rect_filled(rect, 0.0, p.scrim);
            // On the press, not the release: what lies under the press is no longer covered when the button comes up.
            response.clicked() || (response.contains_pointer() && ui.input(|input| input.pointer.primary_pressed()))
        })
        .inner;
    let rect = place(theme, screen);
    let mut asked = beside.then_some(Asked::Close);
    Area::new(Id::unique("settings-sheet")).order(Order::Foreground).fixed_pos(rect.min).show(ctx, |ui| {
        let (whole, response) = ui.allocate_exact_size(rect.size(), Sense::hover());
        response.widget_info(|| WidgetInfo::labeled(Role::Dialog, true, tr!("Settings")));
        // The float's shadow and edge, then solid surfaces: nothing of the window shows through the text.
        paint::float(ui.painter(), theme, whole, FloatEdge::Keyboard);
        ui.painter().rect_filled(whole.shrink(1.0), 0.0, p.surface);
        let nav_width = NAV.min(whole.width() * 0.3);
        let left = Rect::from_min_size(whole.min, vec2(nav_width, whole.height())).shrink(1.0);
        ui.painter().rect_filled(left, 0.0, p.panel);
        let inner = left.shrink2(vec2(s.s3, s.s4));
        let shown = view.settings.section();
        let pressed =
            ui.scope_builder(UiBuilder::new().max_rect(inner).layout(Layout::top_down(Align::Min)), |ui| nav(ui, theme, view.keymap, shown)).inner;
        if let Some(section) = pressed {
            asked = Some(Asked::Section(section));
        }
        let body = Rect::from_min_max(pos2(whole.left() + nav_width, whole.top()), whole.max).shrink2(vec2(s.s6 + s.s1, s.s4 + s.s1));
        ui.scope_builder(UiBuilder::new().max_rect(body).layout(Layout::top_down(Align::Min)), |ui| {
            if let Some(on_page) = page(ui, theme, &mut view) {
                asked = Some(on_page);
            }
        });
    });
    asked
}
