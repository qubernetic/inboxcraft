// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The settings, a place of the window in the body beside the index: the list of sections on the left, 232 wide (a head, "Settings" and how
//! many sections there are, then a row per section: the nine of the design, Appearance, Accounts, Notifications,
//! Triage, Sending, Calendar, Shortcuts, Privacy and About, then the two planned ones, AI and Plugins), and the open
//! section's page beside it in a column 720 wide. A page is its head (where it is, `[ 04 ] SETTINGS · APPEARANCE`, its
//! title and a sentence or two), then its groups: a section label (`[ A ] LOOK`) over settings rows.
//!
//! Appearance holds Look (mode, motion, text size) and Layout (the inbox view, where a message opens and is written,
//! the window frame). Accounts is drawn in `accounts.rs`. Notifications holds each account's level (all, people
//! only, none), whether returned snoozes and follow-up reminders notify, and the two switches of the app's
//! [`crate::desk::Desk`]: running in the background and starting with the computer. Triage holds the screener's mode,
//! whether a pin outlasts done and each account's grouping. Sending holds "Undo send", how long every send waits
//! before it goes, and each account's signature, saved with its Save button. Shortcuts lists every action of the window
//! with its key: a press on the key waits for the next key pressed, which becomes the action's. The sections and rows
//! not built yet are drawn as coming soon (`planned.rs`).
//!
//! The settings are a place, not a surface in front: the index and the window's keys work beside them, and `Tab` walks
//! from the index to the sections and on to the page.

use egui::{Align, Id, Layout, Rect, Response, Role, ScrollArea, Sense, Ui, UiBuilder, WidgetInfo, pos2, vec2};
use inboxcraft_engine::ScreenerMode;
use inboxcraft_engine::appearance::{Appearance, ThemeMode, WindowFrame};
use inboxcraft_engine::notify::Level;

use super::paint::{Ring, focus_ring};
use crate::component::button::{LinkSize, TextButton, link_sized};
use crate::component::field;
use crate::component::key::{Chip, ChipState};
use crate::component::label::section_label;
use crate::component::segmented::{segmented, segmented_but, segmented_for};
use crate::component::settings_row::{page as on_page, settings_row, settings_row_soon_with};
use crate::component::slider;
use crate::component::switch::switch;
use crate::draw::paint;
use crate::iconset::IconId;
use crate::keymap::{Action, Keymap};
use crate::mailbox::{InboxView, Mailbox};
use crate::onboard::Onboarding;
use crate::settings::{Section, Settings};
use crate::theme::{Font, TextScale, Theme};
use crate::tr;

#[path = "settings_planned.rs"]
mod planned;

/// The width of the sections' column, in points: as wide as the window's index.
const NAV: f32 = 232.0;

/// The height of the sections' head, in points.
const NAV_HEAD: f32 = 48.0;

/// The room between the head's hairline and the first section, in points.
const NAV_GAP: f32 = 8.0;

/// The width of a section's page, in points.
const COLUMN: f32 = 720.0;

/// The room above a page, in points.
const PAGE_TOP: f32 = 24.0;

/// The room left and right of a page, in points.
const PAGE_X: f32 = 40.0;

/// The least room left and right of a page, where the body has no room for the page's 720 and 40 either side (a narrow
/// window): the page keeps the width, the room gives way first, in points.
const PAGE_X_LEAST: f32 = 24.0;

/// The page's width the sections' column gives way for in a narrow body, in points.
const PAGE_LEAST: f32 = 520.0;

/// The room under a page, in points.
const PAGE_BOTTOM: f32 = 16.0;

/// The room under the About page, in points.
const ABOUT_BOTTOM: f32 = 24.0;

/// How far a page's words stand in from its column, as far as a settings row's, in points.
const INSET: f32 = 16.0;

/// The room above a group's label, in points.
const GROUP_TOP: f32 = 20.0;

/// The room between a group's label (and its sentence) and its hairline, in points.
const GROUP_UNDER: f32 = 8.0;

/// The height of a row of the Shortcuts page's list, in points (at 100 %).
const KEY_ROW: f32 = 36.0;

/// The room above the Shortcuts page's field and Reset all keys, in points.
const TOOLS_TOP: f32 = 16.0;

/// The room under them, above the list's first hairline, in points.
const TOOLS_UNDER: f32 = 12.0;

/// The width of the field that narrows the Shortcuts page's list, in points.
const FIND: f32 = 320.0;

/// The least width of that field beside Reset all keys, in points: narrower, the button goes under it.
const FIND_LEAST: f32 = 200.0;

/// The least room between that field and Reset all keys beside it, in points.
const FIND_GAP: f32 = 16.0;

/// What the owner asked for on the settings this frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Asked {
    Mode(ThemeMode),
    Scale(TextScale),
    ReduceMotion(bool),
    Frame(WindowFrame),
    /// Another page of the settings.
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
}

/// What the settings show and change.
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

/// A level of motion as the design names them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MotionLevel {
    /// The showpiece, with its text effects: not built yet.
    Full,
    /// What explains a change, without the text effects: the motion that runs today.
    Reduced,
    /// Every change at once: today's reduced motion.
    Off,
}

/// The inbox views in the order Appearance offers them, each with the name the design gives it: Grouped opens every
/// group, Smart shows the first rows of each, List is one list by time. The one place that names them.
pub(super) fn inbox_views() -> [(InboxView, &'static str); 3] {
    [(InboxView::Grouped, tr!("Grouped")), (InboxView::Smart, tr!("Smart")), (InboxView::List, tr!("List"))]
}

/// The icon of a section in the list.
fn section_icon(section: Section) -> IconId {
    match section {
        Section::Appearance => IconId::Palette,
        Section::Accounts => IconId::At,
        Section::Notifications => IconId::Notify,
        Section::Triage => IconId::Funnel,
        Section::Sending => IconId::Send,
        Section::Calendar => IconId::Calendar,
        Section::Shortcuts => IconId::Keyboard,
        Section::Privacy => IconId::Shield,
        Section::About => IconId::Info,
        Section::Ai => IconId::Compose,
        Section::Plugins => IconId::Add,
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

/// What the screener's `mode` does, as the help of its row.
fn screener_help(mode: ScreenerMode) -> &'static str {
    match mode {
        ScreenerMode::Before => {
            tr!("Before the inbox: the first mail of a sender you have not let in yet waits in New senders until you decide.")
        }
        ScreenerMode::Inside => tr!("Inside, marked: the first mail of a sender you have not let in yet shows in the inbox, marked."),
        ScreenerMode::Off => tr!("Off: the first mail of a new sender comes in as any other."),
    }
}

/// `text` in `font` and `colour`, wrapped to the room `ui` has.
fn words(ui: &mut Ui, font: Font, text: &str, colour: egui::Color32) {
    ui.add(egui::Label::new(font.rich(&crate::component::plain(text)).color(colour)).wrap());
}

/// Draws `add` in `ui` [`INSET`] in from either side, as far in as a settings row's words; what `add` gave.
pub(super) fn inset<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let mut room = ui.available_rect_before_wrap();
    room.min.x += INSET;
    room.max.x = (room.max.x - INSET).max(room.min.x + 1.0);
    let mut child = ui.new_child(UiBuilder::new().max_rect(room).layout(Layout::top_down(Align::Min)));
    child.spacing_mut().item_spacing.y = 0.0;
    let given = add(&mut child);
    let used = child.min_rect();
    ui.advance_cursor_after_rect(Rect::from_min_max(pos2(ui.max_rect().left(), used.top()), pos2(ui.max_rect().right(), used.bottom())));
    given
}

/// A `panel_line` hairline across `ui` at its cursor, and the pixel it takes.
pub(super) fn rule(ui: &mut Ui, theme: &Theme) {
    rule_in(ui, theme.palette.panel_line);
}

/// A hairline in `colour` across `ui` at its cursor, and the pixel it takes.
fn rule_in(ui: &mut Ui, colour: egui::Color32) {
    let y = ui.cursor().top();
    let x = ui.max_rect().x_range();
    paint::hairline(ui.painter(), pos2(x.min, y), pos2(x.max - 1.0, y), colour);
    ui.add_space(1.0);
}

/// The room between the lines of a page's head, in points.
const HEAD_GAP: f32 = 6.0;

/// The height of the line that says where a page is.
fn where_high(theme: &Theme) -> f32 {
    16.0_f32.max(theme.fonts.label.line).ceil()
}

/// The height of a page's title line.
fn title_high(theme: &Theme) -> f32 {
    32.0_f32.max(theme.fonts.title.line).ceil()
}

/// A page's head: where it is (`[ 04 ] SETTINGS · <SECTION>`), its title with `action` at its right end and, on a
/// planned section, the coming-soon mark there; then `intro`, a sentence a line.
pub(super) fn head(ui: &mut Ui, theme: &Theme, section: Section, intro: &[&str], action: impl FnOnce(&mut Ui)) {
    let p = &theme.palette;
    inset(ui, |ui| {
        let width = ui.available_width();
        ui.allocate_ui_with_layout(vec2(width, where_high(theme)), Layout::left_to_right(Align::Center), |ui| {
            section_label(ui, theme, "[ 04 ]", &tr!("Settings · {section}", section = section.name()), false);
        });
        ui.add_space(HEAD_GAP);
        let tall = title_high(theme);
        ui.allocate_ui_with_layout(vec2(width, tall), Layout::left_to_right(Align::Center), |ui| {
            ui.set_min_height(tall);
            let title = crate::component::label(ui, theme.fonts.title, section.name(), p.ink);
            title.widget_info(|| WidgetInfo::labeled(Role::Heading, true, section.name()));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if section.planned() {
                    crate::component::soon::soon_tag(ui, theme);
                }
                action(ui);
            });
        });
        for line in intro {
            ui.add_space(6.0);
            words(ui, theme.fonts.body, line, p.muted);
        }
    });
}

/// A group of settings: its label (`[ A ] LOOK`), the sentence it needs if any, and the hairline its first row hangs
/// from.
pub(super) fn group(ui: &mut Ui, theme: &Theme, index: &str, word: &str, note: Option<&str>) {
    ui.add_space(GROUP_TOP);
    inset(ui, |ui| {
        section_label(ui, theme, &format!("[ {index} ]"), word, false);
        if let Some(note) = note {
            ui.add_space(4.0);
            words(ui, theme.fonts.caption, note, theme.palette.muted);
        }
    });
    ui.add_space(GROUP_UNDER);
    rule(ui, theme);
}

/// The page's body under its head, scrolling when it is taller than the sheet. Its rows keep the column's whole width:
/// the scroll bar stands beside the column, in the page's room, never over a row or its focus edge.
pub(super) fn body<R>(ui: &mut Ui, theme: &Theme, salt: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    let width = ui.available_width().floor().max(1.0);
    let mut room = ui.available_rect_before_wrap();
    room.max.x += theme.sizes.scrollbar * 2.0;
    let mut child = ui.new_child(UiBuilder::new().max_rect(room).layout(Layout::top_down(Align::Min)));
    let area = ScrollArea::vertical().id_salt(("settings-page", salt)).auto_shrink([false, false]);
    let given = crate::component::scroll::show(&mut child, theme, area, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.set_width(width);
        let given = on_page(ui, add);
        follow_focus(ui);
        given
    })
    .inner;
    ui.advance_cursor_after_rect(child.min_rect());
    given
}

/// Called last inside a scrolled page: the keyboard just came to a control of it, so the page scrolls to it, wherever
/// it was. Only on the frame the focus arrives, so it never fights the wheel.
pub(super) fn follow_focus(ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    let arrived = ctx.memory(|memory| memory.focused().filter(|focused| !memory.had_focus_last_frame(*focused)));
    if let Some(control) = arrived.and_then(|focused| ctx.read_response(focused)).filter(|control| ui.min_rect().contains_rect(control.rect)) {
        ui.scroll_to_rect(control.rect, None);
    }
}

/// Draws `add`, one row of a page that has a row for every account, only where it is near enough to be seen: within a
/// view's height above or below what the page shows, so `Tab` still reaches the next row, which the page ([`body`])
/// then scrolls to. Elsewhere the row takes the height it last had and lays nothing out: a page with hundreds of
/// accounts builds the rows in sight only. The first time a row is drawn it is laid out wherever it is. A row at one
/// of the run's `ends` is always laid out, so `Tab` and `Shift+Tab` coming round the sheet reach the first and the last
/// account. A height kept at another width or text size is dropped: the row is laid out again.
pub(super) fn culled(ui: &mut Ui, theme: &Theme, id: Id, ends: bool, add: impl FnOnce(&mut Ui)) {
    let width = ui.available_width();
    let top = ui.cursor().top();
    let laid = Laid { width, text: theme.fonts.body.size, height: 0.0 };
    let known = ui.ctx().data(|data| data.get_temp::<Laid>(id)).filter(|kept| kept.width == laid.width && kept.text == laid.text);
    if let Some(height) = known.map(|kept| kept.height).filter(|_| !ends) {
        let clip = ui.clip_rect();
        let near = clip.expand2(vec2(0.0, clip.height()));
        let row = Rect::from_min_size(pos2(ui.max_rect().left(), top), vec2(width, height));
        if !near.intersects(row) {
            ui.allocate_space(vec2(width, height));
            return;
        }
    }
    add(ui);
    let height = (ui.cursor().top() - top).max(0.0);
    if known.is_none_or(|kept| kept.height != height) {
        ui.ctx().data_mut(|data| data.insert_temp(id, Laid { height, ..laid }));
    }
}

/// A row's height as [`culled`] last laid it out, with the width and the text size it was laid out at.
#[derive(Debug, Clone, Copy)]
struct Laid {
    /// The room's width, in points.
    width: f32,
    /// The body text's size, in points.
    text: f32,
    /// The row's height, in points.
    height: f32,
}

/// The Appearance page: Look, then Layout.
fn appearance(ui: &mut Ui, theme: &Theme, current: &Appearance, reduce: bool, view: InboxView) -> Option<Asked> {
    let mut asked = None;
    let f = &theme.fonts;
    head(ui, theme, Section::Appearance, &[], |_| {});
    body(ui, theme, "appearance", |ui| {
        group(ui, theme, "A", tr!("Look"), None);
        let modes = MODES.map(|mode| (mode, mode_name(mode), f.button));
        settings_row(ui, theme, tr!("Mode"), Some(tr!("Auto follows the computer's light or dark choice.")), |ui| {
            if let Some(mode) = segmented(ui, theme, &modes, current.theme) {
                asked = Some(Asked::Mode(mode));
            }
        });
        // The design's three levels. Full, with its text effects, is not built yet; Reduced is the motion that runs
        // today, Off lands every motion at its end at once (today's reduced motion).
        let levels =
            [(MotionLevel::Full, tr!("Full"), f.button), (MotionLevel::Reduced, tr!("Reduced"), f.button), (MotionLevel::Off, tr!("Off"), f.button)];
        let level = if reduce { MotionLevel::Off } else { MotionLevel::Reduced };
        let said =
            if reduce { tr!("Off: every change happens at once.") } else { tr!("Reduced keeps what explains a change and drops the text effects.") };
        // The second sentence stands whether or not the owner chose, as the design's help has it.
        let motion_help = tr!("{level} Until you choose, the computer's reduce-motion setting gives Off.", level = said);
        settings_row(ui, theme, tr!("Motion"), Some(&motion_help), |ui| {
            // Full, the first option, is planned: the coming-soon mark stands just before it (the row's control room
            // runs right to left).
            ui.spacing_mut().item_spacing.x = theme.spacing.s2;
            if let Some(chosen) = segmented_but(ui, theme, &levels, level, Some(tr!("Motion")), &[MotionLevel::Full]) {
                asked = Some(Asked::ReduceMotion(chosen == MotionLevel::Off));
            }
            crate::component::soon::soon_tag(ui, theme);
        });
        let now = TextScale::from_percent(current.text_scale).unwrap_or_default();
        settings_row(ui, theme, tr!("Text size"), Some(tr!("90 % to 130 %. Rows and bars follow the type.")), |ui| {
            if let Some(scale) = slider::text_size(ui, theme, now) {
                asked = Some(Asked::Scale(scale));
            }
        });
        group(ui, theme, "B", tr!("Layout"), None);
        let views = inbox_views().map(|(view, name)| (view, name, f.button));
        let view_help = tr!("Grouped opens every group, Smart shows the first three rows of each, List is one list by time.");
        settings_row(ui, theme, tr!("Inbox view"), Some(view_help), |ui| {
            if let Some(chosen) = segmented(ui, theme, &views, view) {
                asked = Some(Asked::InboxView(chosen));
            }
        });
        let places = [(0_u8, tr!("Pane"), f.button), (1, tr!("Window"), f.button)];
        settings_row_soon_with(ui, theme, tr!("Open a message"), tr!("In the pane beside the list, or in a window of its own."), |ui| {
            segmented_for(ui, theme, &places, 0, Some(tr!("Open a message")));
        });
        settings_row_soon_with(ui, theme, tr!("Write a message"), tr!("Docked in the reader's place, or in a window of its own."), |ui| {
            segmented_for(ui, theme, &places, 0, Some(tr!("Write a message")));
        });
        // The design's two frames: InboxCraft's own (the top row or the strip, whichever is on; Own from the desktop's
        // brings back the one last on, the strip at first) and the desktop's title bar.
        let own = Appearance::own_of(current.frame, Some(current.own_frame));
        let frames = [(own, tr!("Own"), f.button), (WindowFrame::System, tr!("System"), f.button)];
        let frame_help = tr!("InboxCraft's own frame, or the desktop's title bar above it.");
        settings_row(ui, theme, tr!("Window frame"), Some(frame_help), |ui| {
            if let Some(frame) = segmented(ui, theme, &frames, current.frame) {
                asked = Some(Asked::Frame(frame));
            }
        });
    });
    asked
}

/// The height of a section's row, in points (at 100 %).
const SECTION_ROW: f32 = 30.0;

/// Where a section row's icon starts, from the row's left edge (its 2 px bar, then 42), in points.
const SECTION_ICON_AT: f32 = 44.0;

/// The room between a section row's icon and its name, and right of the name, in points.
const SECTION_GAP: f32 = 10.0;

/// The room right of a section row's name, in points.
const SECTION_RIGHT: f32 = 14.0;

/// One section in the list, the design's `mailbox-row`: 30 high, the icon 16 and the name in `body`, both `muted`;
/// hovered, the `grid` fill and both `ink`; the section shown, the `stage` fill, a 2 px `cyan_edge` bar at its left
/// and both `ink`; focused, the inside ring.
fn section_row(ui: &mut Ui, theme: &Theme, section: Section, shown: bool) -> Response {
    let p = &theme.palette;
    let name = section.name();
    let height = Theme::size(theme, SECTION_ROW).max(theme.fonts.body.line + 8.0).ceil();
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    response.widget_info(|| WidgetInfo::selected(Role::Button, true, shown, tr!("{section}, settings page", section = name)));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let hovered = response.hovered() && !shown;
        if shown {
            painter.rect_filled(rect, 0.0, p.stage);
            painter.rect_filled(Rect::from_min_size(rect.min, vec2(2.0, rect.height())), 0.0, p.cyan_edge);
        } else if hovered {
            painter.rect_filled(rect, 0.0, p.grid);
        }
        let ink = if shown || hovered { p.ink } else { p.muted };
        let side = theme.icon_side();
        let left = rect.left() + SECTION_ICON_AT;
        let icon = Rect::from_min_size(pos2(left, (rect.center().y - side / 2.0).round()), vec2(side, side));
        crate::iconset::paint(painter, icon, section_icon(section), ink);
        let at = icon.right() + SECTION_GAP;
        let words = paint::line(ui, &[(name, theme.fonts.body, ink)], (rect.right() - SECTION_RIGHT - at).max(1.0));
        ui.painter().galley(pos2(at, (rect.center().y - words.size().y / 2.0).round()), words, ink);
        if response.has_focus() {
            focus_ring(ui.painter(), theme, rect, Ring::Inside);
        }
    }
    response
}

/// "<n> sections": every row the list shows, the two planned ones, AI and Plugins, among them (the design's list has
/// nine rows and says nine).
fn section_count() -> String {
    tr!("{count} sections", count = Section::ALL.len())
}

/// The list of sections: its head ("Settings" and how many sections there are), a `line` hairline, then a row per
/// section, the one shown selected. The section pressed.
fn nav(ui: &mut Ui, theme: &Theme, shown: Section) -> Option<Asked> {
    let p = &theme.palette;
    let f = &theme.fonts;
    let width = ui.available_width();
    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
    let tall = NAV_HEAD.max(f.heading.line);
    let mut asked = None;
    ui.allocate_ui_with_layout(vec2(width, tall), Layout::left_to_right(Align::Center), |ui| {
        ui.set_min_height(tall);
        ui.add_space(INSET);
        crate::component::label(ui, f.heading, tr!("Settings"), p.ink);
        ui.add_space(10.0);
        let count = section_count();
        // Cut only where there is no room (a window too small for the column).
        let room = ui.available_width() - INSET;
        let words = crate::component::cut(ui, &count, f.mono_s, p.muted, room);
        let (at, said) = ui.allocate_exact_size(words.size(), Sense::hover());
        said.widget_info(|| WidgetInfo::labeled(Role::Label, true, &count));
        ui.painter().galley(at.min, words, p.muted);
    });
    rule_in(ui, p.line);
    ui.add_space(NAV_GAP);
    for section in Section::ALL {
        if section_row(ui, theme, section, section == shown).clicked() && section != shown {
            asked = Some(Asked::Section(section));
        }
    }
    asked
}

/// The page shown.
fn page(ui: &mut Ui, theme: &Theme, view: &mut Sheet<'_>) -> Option<Asked> {
    let keymap = view.keymap;
    let mailbox = view.mailbox;
    match view.settings.section() {
        Section::Appearance => appearance(ui, theme, view.current, view.reduce, mailbox.inbox_view()),
        Section::Accounts => super::accounts::page(ui, theme, keymap, mailbox, view.onboarding, view.can_add),
        Section::Shortcuts => shortcuts(ui, theme, keymap, view.settings),
        Section::Sending => sending(ui, theme, keymap, view.settings, mailbox),
        Section::Triage => triage(ui, theme, mailbox),
        Section::Notifications => notifications(ui, theme, mailbox, view.settings, view.desk),
        Section::Calendar => planned::calendar(ui, theme, keymap, mailbox),
        Section::Privacy => planned::privacy(ui, theme, keymap),
        Section::About => planned::about(ui, theme),
        Section::Ai => planned::ai(ui, theme),
        Section::Plugins => planned::plugins(ui, theme),
    }
}

/// A key as a chip that can be pressed: `shown` in the chip `state` draws (the capture state while it waits).
fn key_button(ui: &mut Ui, theme: &Theme, shown: &str, spoken: &str, state: ChipState) -> Response {
    let chip = Chip::new(ui, theme, shown, state);
    let (rect, response) = ui.allocate_exact_size(chip.size(), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, spoken));
    if ui.is_rect_visible(rect) {
        if response.hovered() {
            ui.painter().rect_filled(rect, 0.0, theme.palette.stage);
        }
        chip.paint(ui.painter(), rect.min);
        if response.has_focus() {
            focus_ring(ui.painter(), theme, response.rect, Ring::Outside);
        }
    }
    response
}

/// The key of `action` on its row, drawn from the right in `ui`: the chip, which waits for a key when pressed, and
/// "remove" before it while it has one.
fn key_control(ui: &mut Ui, theme: &Theme, keymap: &Keymap, action: &Action, waits: bool) -> Option<Asked> {
    let mut asked = None;
    let chip = keymap.chip(ui.ctx(), action.id);
    let (shown, spoken, state) = match (&chip, waits) {
        (_, true) => {
            (tr!("press a key…").to_owned(), tr!("{name}: press a key, or Escape to keep the old one", name = action.name), ChipState::Capture)
        }
        (Some(chip), false) => (chip.clone(), tr!("Key of {name}: {key}", name = action.name, key = chip), ChipState::Default),
        (None, false) => (tr!("No key").to_owned(), tr!("{name} has no key", name = action.name), ChipState::Disabled),
    };
    if key_button(ui, theme, &shown, &spoken, state).clicked() && !waits {
        asked = Some(Asked::Capture(action.id));
    }
    if chip.is_some() && !waits {
        ui.add_space(12.0);
        let remove = tr!("Remove the key of {name}", name = action.name);
        let link = link_sized(ui, theme, keymap, tr!("remove"), LinkSize::Small);
        ui.ctx().accesskit_node_builder(link.id, |node| node.set_label(remove.as_str()));
        if link.clicked() {
            asked = Some(Asked::Unbind(action.id));
        }
    }
    asked
}

/// One action of the Shortcuts page: its name, and its key at the right end; a hairline under it, or, while it waits
/// for a key, a `cyan_edge` box round the whole row.
fn key_row(ui: &mut Ui, theme: &Theme, keymap: &Keymap, action: &Action, waits: bool) -> Option<Asked> {
    let height = Theme::size(theme, KEY_ROW).max(theme.fonts.body.line).ceil();
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    // The whole row high, its hairline included, so a chip 20 high stands 8 from its top as the design has it.
    let inner = Rect::from_min_max(pos2(rect.left() + INSET, rect.top()), pos2((rect.right() - INSET).max(rect.left() + INSET), rect.bottom()));
    let mut right = ui.new_child(UiBuilder::new().max_rect(inner).layout(Layout::right_to_left(Align::Center)));
    right.spacing_mut().item_spacing.x = 0.0;
    let asked = key_control(&mut right, theme, keymap, action, waits);
    let room = (right.min_rect().left() - inner.left() - 12.0).max(1.0);
    let name = crate::component::cut(ui, action.name, theme.fonts.body, theme.palette.ink, room);
    ui.painter().galley(pos2(inner.left(), (rect.center().y - name.size().y / 2.0).round()), name, theme.palette.ink);
    if waits {
        // The row waiting for a key is a box with a 1 px `cyan_edge` round it, as `settings-shortcuts/capture` has it.
        paint::outline(ui.painter(), rect, theme.palette.cyan_edge);
    } else {
        let y = rect.bottom() - 1.0;
        paint::hairline(ui.painter(), pos2(rect.left(), y), pos2(rect.right() - 1.0, y), theme.palette.panel_line);
    }
    asked
}

/// The Shortcuts page: its head, the field that narrows the list and Reset all keys, the note on the last key given,
/// then every action with its key, scrolling.
fn shortcuts(ui: &mut Ui, theme: &Theme, keymap: &Keymap, settings: &mut Settings) -> Option<Asked> {
    let p = &theme.palette;
    let mut asked = None;
    head(
        ui,
        theme,
        Section::Shortcuts,
        &[tr!("Every action can have a key. One key runs one action per screen: giving a key takes it from its holder there.")],
        |_| {},
    );
    ui.add_space(TOOLS_TOP);
    inset(ui, |ui| {
        // The field, its label over it, 320 wide where there is room; Reset all keys at the right end, 2 over its foot.
        // Where the row is too narrow to keep the field [`FIND_LEAST`] wide beside the button, the button goes under the
        // field, as a narrow settings row puts its control under its words.
        let reset = TextButton::new(tr!("Reset all keys")).secondary();
        let room = ui.available_width();
        let beside = room - reset.width(ui, theme, keymap) - FIND_GAP;
        let under = beside < FIND_LEAST;
        let width = if under { room } else { beside }.clamp(1.0, FIND);
        let field = ui
            .allocate_ui_with_layout(vec2(width, 0.0), Layout::top_down(Align::Min), |ui| {
                ui.set_width(width);
                let find = field::Field::new(tr!("Find a command"), tr!("a part of its name"));
                field::text_field(ui, theme, Id::unique("shortcuts-find"), settings.find_mut(), &find);
            })
            .response
            .rect;
        let pressed = if under {
            ui.add_space(8.0);
            reset.show(ui, theme, keymap).clicked()
        } else {
            let at = Rect::from_min_max(pos2(field.right(), field.top()), pos2(ui.max_rect().right(), field.bottom() - 2.0));
            ui.scope_builder(UiBuilder::new().max_rect(at).layout(Layout::right_to_left(Align::Max)), |ui| reset.show(ui, theme, keymap).clicked())
                .inner
        };
        if pressed {
            asked = Some(Asked::ResetKeys);
        }
        if let Some(text) = settings.keys_note() {
            ui.add_space(8.0);
            words(ui, theme.fonts.caption, text, p.muted);
        }
    });
    ui.add_space(TOOLS_UNDER);
    rule(ui, theme);
    let waiting = settings.capturing();
    let list = crate::palette::matching(settings.find());
    body(ui, theme, "shortcuts", |ui| {
        if list.is_empty() {
            ui.add_space(12.0);
            inset(ui, |ui| words(ui, theme.fonts.body, &tr!("No command matches “{text}”.", text = settings.find()), p.muted));
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

/// The room between two signatures side by side, in points.
const SIGNATURE_GAP: f32 = 24.0;

/// The least width of a signature's column beside another, in points: its label, its field and its Save with "nothing
/// changed" beside it, at 130 %. Narrower, the signatures stand one under the other.
const SIGNATURE_LEAST: f32 = 240.0;

/// One account's signature: its label, the field, and Save with "nothing changed" beside it while it is as kept.
fn signature(ui: &mut Ui, theme: &Theme, keymap: &Keymap, settings: &mut Settings, account: &crate::mailbox::AccountRow) -> Option<Asked> {
    let p = &theme.palette;
    let label = tr!("Signature of {account}", account = account.label);
    let typed = settings.signature_mut(account.id, &account.signature);
    // The field's own label over it, in capitals, wrapped where it is long; the model keeps a signature's characters:
    // what would pass them is refused whole and said, not cut.
    field::multiline_field_holding(
        ui,
        theme,
        Id::unique("signature").with(account.id),
        typed,
        &field::Field::new(&label, tr!("Anything they should know")),
        field::Cap::Chars(inboxcraft_model::Account::MAX_SIGNATURE_CHARS),
    );
    let changed = settings.signature(account.id).is_some_and(|typed| typed != account.signature);
    ui.add_space(8.0);
    let mut asked = None;
    ui.horizontal(|ui| {
        // Not the 24 between two signatures this row stands in: only the 12 below.
        ui.spacing_mut().item_spacing.x = 0.0;
        let save = tr!("Save the signature of {account}", account = account.label);
        let button = TextButton::new(tr!("Save")).secondary();
        let response = if changed { button.show(ui, theme, keymap) } else { button.unavailable(tr!("Nothing changed")).show(ui, theme, keymap) };
        ui.ctx().accesskit_node_builder(response.id, |node| node.set_label(save.as_str()));
        if changed && response.clicked() {
            asked = Some(Asked::Signature(account.id));
        }
        if !changed {
            ui.add_space(12.0);
            crate::component::label(ui, theme.fonts.mono_s, tr!("nothing changed"), p.muted);
        }
    });
    asked
}

/// The Sending page: Undo send, the signatures two by two, then the templates and the scheduled list (coming soon).
fn sending(ui: &mut Ui, theme: &Theme, keymap: &Keymap, settings: &mut Settings, mailbox: &Mailbox) -> Option<Asked> {
    let mut asked = None;
    head(ui, theme, Section::Sending, &[], |_| {});
    body(ui, theme, "sending", |ui| {
        group(ui, theme, "A", tr!("Before it goes"), None);
        let hold = mailbox.hold_seconds();
        let labels = HOLDS.map(|seconds| tr!("{seconds} s", seconds = seconds));
        let holds: Vec<(u8, &str, Font)> = HOLDS.iter().zip(&labels).map(|(seconds, label)| (*seconds, label.as_str(), theme.fonts.button)).collect();
        let now = tr!("Every message waits here this long before it goes, so Undo can take it back. Now {seconds} s.", seconds = hold);
        settings_row(ui, theme, tr!("Undo send"), Some(&now), |ui| {
            if let Some(seconds) = segmented(ui, theme, &holds, hold) {
                asked = Some(Asked::Hold(seconds));
            }
        });
        settings_row_soon_with(ui, theme, tr!("Send at once"), tr!("Undo send set to Off: every message goes the moment you send it."), |_| {});
        let note = tr!(
            "A signature goes under “-- ” at the end of every new message from its account. The drafts already started keep theirs. An empty field clears it."
        );
        group(ui, theme, "B", tr!("Signatures"), Some(note));
        if mailbox.accounts().is_empty() {
            ui.add_space(12.0);
            inset(ui, |ui| {
                words(ui, theme.fonts.body, tr!("Add an account first: a signature belongs to the account it is sent from."), theme.palette.muted)
            });
        }
        let pairs = mailbox.accounts().len().div_ceil(2);
        for (index, pair) in mailbox.accounts().chunks(2).enumerate() {
            let first = pair.first().map_or(0, |account| account.id);
            let ends = index == 0 || index.saturating_add(1) == pairs;
            culled(ui, theme, Id::unique("settings-signatures").with(first), ends, |ui| {
                ui.add_space(INSET);
                inset(ui, |ui| {
                    let column = ((ui.available_width() - SIGNATURE_GAP) / 2.0).floor().max(1.0);
                    // Two side by side while each keeps [`SIGNATURE_LEAST`]; one under the other below that.
                    if column < SIGNATURE_LEAST {
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 0.0;
                            for (index, account) in pair.iter().enumerate() {
                                if index > 0 {
                                    ui.add_space(SIGNATURE_GAP);
                                }
                                if let Some(pressed) = signature(ui, theme, keymap, settings, account) {
                                    asked = Some(pressed);
                                }
                            }
                        });
                        return;
                    }
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = SIGNATURE_GAP;
                        for account in pair {
                            ui.allocate_ui_with_layout(vec2(column, 0.0), Layout::top_down(Align::Min), |ui| {
                                ui.set_width(column);
                                ui.spacing_mut().item_spacing.y = 0.0;
                                if let Some(pressed) = signature(ui, theme, keymap, settings, account) {
                                    asked = Some(pressed);
                                }
                            });
                        }
                    });
                });
                ui.add_space(INSET);
                rule(ui, theme);
            });
        }
        group(ui, theme, "C", tr!("Templates and scheduled"), None);
        settings_row_soon_with(ui, theme, tr!("Templates"), tr!("Texts you write often, kept on their account, with a page to manage them."), |ui| {
            link_sized(ui, theme, keymap, tr!("manage templates →"), LinkSize::Small);
        });
        settings_row_soon_with(
            ui,
            theme,
            tr!("Scheduled"),
            tr!("Everything that waits for its time, sends, snoozes, reminders and calendar alerts, in one list."),
            |ui| {
                link_sized(ui, theme, keymap, tr!("open the list →"), LinkSize::Small);
            },
        );
    });
    asked
}

/// The Triage page: the screener, pins on done mail, each account's grouping.
fn triage(ui: &mut Ui, theme: &Theme, mailbox: &Mailbox) -> Option<Asked> {
    let mut asked = None;
    let f = &theme.fonts;
    head(ui, theme, Section::Triage, &[], |_| {});
    body(ui, theme, "triage", |ui| {
        group(
            ui,
            theme,
            "A",
            tr!("Screener"),
            Some(tr!("Each sender is judged once, across the accounts. Turning it on lets in everyone your mail already names.")),
        );
        let modes = ScreenerMode::ALL.map(|mode| (mode, screener_name(mode), f.button));
        let mode = mailbox.screener_mode();
        settings_row(ui, theme, tr!("New senders"), Some(screener_help(mode)), |ui| {
            if let Some(mode) = segmented(ui, theme, &modes, mode) {
                asked = Some(Asked::Screener(mode));
            }
        });
        group(ui, theme, "B", tr!("Pins"), None);
        let keep = mailbox.pins_outlast_done();
        let pins_note = tr!("Off, Done takes the pin off; on, a pinned message stays in Pins after it is done.");
        settings_row(ui, theme, tr!("Keep pins on done mail"), Some(pins_note), |ui| {
            if switch(ui, theme, tr!("Keep pins on done mail"), keep).clicked() {
                asked = Some(Asked::KeepPins(!keep));
            }
        });
        if !mailbox.accounts().is_empty() {
            let note = tr!("Each account's inbox in People, Notifications and Newsletters, sorted by InboxCraft or by your choice for a sender.");
            group(ui, theme, "C", tr!("Grouped inbox"), Some(note));
        }
        let count = mailbox.accounts().len();
        for (index, account) in mailbox.accounts().iter().enumerate() {
            let ends = index == 0 || index.saturating_add(1) == count;
            culled(ui, theme, Id::unique("settings-grouped").with(account.id), ends, |ui| {
                let name = tr!("Group the inbox of {account}", account = account.label);
                settings_row(ui, theme, &name, Some(&account.address), |ui| {
                    if switch(ui, theme, &name, account.grouped).clicked() {
                        asked = Some(Asked::Grouped(account.id, !account.grouped));
                    }
                });
            });
        }
    });
    asked
}

/// The Notifications page: each account's level, what else notifies, then running in the background and starting
/// with the computer, each switch unavailable where the app's desk cannot do it.
fn notifications(ui: &mut Ui, theme: &Theme, mailbox: &Mailbox, settings: &Settings, desk: DeskView) -> Option<Asked> {
    let mut asked = None;
    let f = &theme.fonts;
    let intro = [
        tr!(
            "Per account: all new mail, mail from people only, or none. Mail from your own addresses never notifies, and letting a sender in raises no burst of old mail."
        ),
        tr!("A notification shows who the mail is from and its subject; your desktop may keep it in its list and show it on the lock screen."),
    ];
    head(ui, theme, Section::Notifications, &intro, |_| {});
    body(ui, theme, "notifications", |ui| {
        group(ui, theme, "A", tr!("New mail"), None);
        if mailbox.accounts().is_empty() {
            ui.add_space(12.0);
            inset(ui, |ui| {
                words(ui, f.body, tr!("Add an account first: each account chooses which of its new mail notifies you."), theme.palette.muted)
            });
        }
        let levels = Level::ALL.map(|level| (level, level_name(level), f.button));
        let count = mailbox.accounts().len();
        for (index, account) in mailbox.accounts().iter().enumerate() {
            let ends = index == 0 || index.saturating_add(1) == count;
            culled(ui, theme, Id::unique("settings-level").with(account.id), ends, |ui| {
                let title = tr!("{account} · {address}", account = account.label, address = account.address);
                settings_row(ui, theme, &title, None, |ui| {
                    if let Some(level) = segmented_for(ui, theme, &levels, mailbox.notify_level(account.id), Some(&account.label)) {
                        asked = Some(Asked::Level(account.id, level));
                    }
                });
            });
        }
        group(ui, theme, "B", tr!("What else notifies"), None);
        let chosen = mailbox.notify_settings();
        let (returns, reminders) = (chosen.returns, chosen.reminders);
        settings_row(ui, theme, tr!("Returned snoozes"), Some(tr!("A snoozed message that comes back notifies you.")), |ui| {
            if switch(ui, theme, tr!("Returned snoozes"), returns).clicked() {
                asked = Some(Asked::Returns(!returns));
            }
        });
        settings_row(
            ui,
            theme,
            tr!("Follow-up reminders"),
            Some(tr!("A message you asked to be reminded of, when no reply came, notifies you.")),
            |ui| {
                if switch(ui, theme, tr!("Follow-up reminders"), reminders).clicked() {
                    asked = Some(Asked::Reminders(!reminders));
                }
            },
        );
        group(ui, theme, "C", tr!("In the background"), None);
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
    settings_row(ui, theme, tr!("Run in the background"), Some(&notes.join(" ")), |ui| {
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
    settings_row(ui, theme, tr!("Start with the computer"), Some(&notes.join(" ")), |ui| {
        if ui.add_enabled_ui(open, |ui| switch(ui, theme, tr!("Start with the computer"), login)).inner.clicked() {
            asked = Some(Asked::Login(!login));
        }
    });
    asked
}

/// Where the settings stand in `body`: the sections' column (232 wide, as the index) with the `line` hairline after it,
/// and the page's column, 720 wide at most, 40 in from either side (down to 24 where the body is narrow), 24 under the
/// top and 16 over the foot (the Shortcuts page's list runs to the foot, About keeps 24). On whole points; never inverted, however small the body.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Columns {
    /// The sections' column.
    pub(super) nav: Rect,
    /// The hairline's x, right of the sections' column.
    pub(super) divider: f32,
    /// The page's column.
    pub(super) page: Rect,
}

impl Columns {
    /// The columns in `body` with the page `shown`.
    pub(super) fn of(body: Rect, shown: Section) -> Self {
        let body = Rect::from_min_max(body.min.round(), body.max.round().max(body.min.round()));
        // The sections' column gives way only where the page would keep less than its least width beside it, and then
        // down to a quarter of the body.
        let nav_width = NAV.min((body.width() * 0.25).floor().max(body.width() - 1.0 - 2.0 * PAGE_X_LEAST - PAGE_LEAST)).max(0.0);
        let nav = Rect::from_min_size(body.min, vec2(nav_width, body.height()));
        let divider = nav.right();
        let bottom = match shown {
            Section::Shortcuts => 0.0,
            Section::About => ABOUT_BOTTOM,
            _ => PAGE_BOTTOM,
        };
        let side = ((body.right() - divider - 1.0 - COLUMN) / 2.0).clamp(PAGE_X_LEAST, PAGE_X).floor();
        let left = divider + 1.0 + side;
        let room = Rect::from_min_max(
            pos2(left, body.top() + PAGE_TOP),
            pos2((body.right() - side).max(left), (body.bottom() - bottom).max(body.top() + PAGE_TOP)),
        );
        let width = room.width().clamp(1.0, COLUMN).floor();
        let page = Rect::from_min_size(room.min, vec2(width, room.height().max(1.0)));
        Self { nav, divider, page }
    }
}

/// Draws the settings in `ui`'s whole rect, the window's body: the sections' column and the page shown; returns what
/// was asked for, applied by the caller after the frame's drawing.
pub(super) fn place(ui: &mut Ui, theme: &Theme, mut view: Sheet<'_>) -> Option<Asked> {
    let p = &theme.palette;
    let body = ui.max_rect();
    // One node a screen reader names the place by; it takes no press, the sections and the page lie over it.
    let place = ui.interact(body, Id::unique("settings-place"), Sense::hover());
    place.widget_info(|| WidgetInfo::labeled(Role::Pane, true, tr!("Settings")));
    ui.painter().rect_filled(body, 0.0, p.surface);
    let shown = view.settings.section();
    let columns = Columns::of(body, shown);
    paint::hairline(ui.painter(), pos2(columns.divider, body.top()), pos2(columns.divider, body.bottom() - 1.0), p.line);
    let mut asked = ui
        .scope_builder(UiBuilder::new().id_salt("settings-sections").max_rect(columns.nav).layout(Layout::top_down(Align::Min)), |ui| {
            nav(ui, theme, shown)
        })
        .inner;
    ui.scope_builder(UiBuilder::new().id_salt("settings-page").max_rect(columns.page).layout(Layout::top_down(Align::Min)), |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        if let Some(on_page) = page(ui, theme, &mut view) {
            asked = Some(on_page);
        }
    });
    asked
}

#[cfg(test)]
#[path = "settings_page_tests.rs"]
mod page_tests;
