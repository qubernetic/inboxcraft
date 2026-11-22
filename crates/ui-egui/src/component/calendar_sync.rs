// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The calendars' sync line at the foot of the calendar's side panel, 222 wide, as its four frames draw it:
//!
//! - Idle (40 high): a status item 7 down, the `plus` `●` and "calendars read 14:02" in `muted` (without a read yet,
//!   "calendars not read yet" and no mark), and the small link "sync now" at the right, 12 down.
//! - Syncing (40): the progress line 6 in and 8 down, running: "calendars", the bar of the jobs heard back, the
//!   account read now as its note. Its ring turns a dot every 125 ms, the only frames it asks for.
//! - Synced (40): the progress line done, the time as its note.
//! - Failed (58): a status item 6 down, the `minus` `●` and "Studio calendars: refused" in `minus`, and under it,
//!   6 in, the small link "try again" and 16 after it "open log", coming soon (no log to open yet).
//!
//! One control takes the press in every state (the link, or the progress line while there is none), Enter and Space
//! included, under one id, so the keyboard stays on it as the state changes; its name says the state to a screen
//! reader. Texts are one line cut with `…` inside the line; a failure is in the window's words only.

use egui::{Id, Rect, Sense, Ui, UiBuilder, WidgetInfo, pos2, vec2};

use crate::calendar::Moment;
use crate::calendar::sync::{SyncState, SyncView};
use crate::component::button::{LinkSize, link_sized};
use crate::component::progress::{ProgressState, bar, progress_with};
use crate::component::soon::soon;
use crate::draw::paint::{Ring, focus_ring, run_at};
use crate::keymap::Keymap;
use crate::theme::Theme;
use crate::tr;

/// The line's height at 100 %, in points, idle, syncing and synced; at other text sizes to the nearest even pixel.
pub const HIGH: f32 = 40.0;

/// The line's height with a failure at 100 %, in points: the failure's line and the ways out under it.
pub const HIGH_FAILED: f32 = 58.0;

/// A status item's height at 100 %, in points.
const ITEM: f32 = 26.0;

/// A small link's height at 100 %, in points.
const LINK: f32 = 16.0;

/// Over and under a failure's item and links, in points, at every text size.
const FAILED_PAD: [f32; 2] = [6.0, 10.0];

/// The line's width on the gallery's boards, in points: the side panel's room.
const BOARD: f32 = 222.0;

/// How the line is drawn: the control's `id`; `resting` under another surface (it takes neither the pointer nor the
/// keyboard); `still`, the running ring standing (a picture of the frames).
#[derive(Debug, Clone, Copy)]
pub struct Drawn {
    /// The control's id.
    pub id: Id,
    /// Under another surface.
    pub resting: bool,
    /// The running ring drawn whole and standing.
    pub still: bool,
}

/// The height of the line for `view` at `theme`'s text size: the heights grow with the text (to the nearest even
/// pixel), the paddings stay (`text-130/calendar`).
#[must_use]
pub fn height(theme: &Theme, view: &SyncView) -> f32 {
    if matches!(view.state, SyncState::Failed { .. }) {
        FAILED_PAD[0] + Theme::size(theme, ITEM) + Theme::size(theme, LINK) + FAILED_PAD[1]
    } else {
        Theme::size(theme, HIGH)
    }
}

/// The sync line over `ui`'s whole room for `view`, drawn as `drawn` says. Whether the control was pressed.
pub fn calendar_sync(ui: &mut Ui, theme: &Theme, keymap: &Keymap, view: &SyncView, drawn: Drawn) -> bool {
    let p = &theme.palette;
    let font = theme.fonts.mono_s;
    let area = ui.max_rect();
    let read = view.read.map(Moment::clock);
    let (item_high, link_high) = (Theme::size(theme, ITEM), Theme::size(theme, LINK));
    // Idle and the progress line stand centred in the line.
    let centred = |high: f32| area.top() + ((Theme::size(theme, HIGH) - high) / 2.0).round();
    let (press, link, spoken) = match &view.state {
        SyncState::Idle => {
            let galley = super::galley(ui, tr!("sync now"), font, p.cyan_text);
            let wide = galley.size().x.ceil();
            let link = Rect::from_min_size(pos2(area.right() - wide, centred(link_high)), vec2(wide, link_high));
            let status = read.as_ref().map_or_else(|| tr!("calendars not read yet").to_owned(), |time| tr!("calendars read {time}", time = time));
            let top = centred(item_high);
            let item = Rect::from_min_max(pos2(area.left(), top), pos2((link.left() - 8.0).max(area.left()), top + item_high));
            status_item(ui, theme, item, &status, read.is_some().then_some(p.plus), p.muted);
            (link, Some(galley), format!("{status}, {}", tr!("sync now")))
        }
        SyncState::Syncing { done, of, account } => {
            // Of the jobs handed out so far: the bar fills as they come back.
            let fraction = if *of == 0 { 0.0 } else { (*done as f32 / *of as f32).min(1.0) };
            let line = progress_line(ui, theme, keymap, area, (ProgressState::Running(fraction), account), drawn.still, centred);
            (line, None, tr!("syncing calendars: {account}, {done} of {of}", account = account, done = done, of = of))
        }
        SyncState::Synced { at } => {
            let time = (*at).or(view.read).map(Moment::clock).unwrap_or_default();
            let line = progress_line(ui, theme, keymap, area, (ProgressState::Done, &time), drawn.still, centred);
            (line, None, format!("{}, {}", tr!("calendars synced {time}", time = time), tr!("sync now")))
        }
        SyncState::Failed { account, why } => {
            let status = account.as_ref().map_or_else(
                || tr!("calendars: {why}", why = why.words()),
                |account| tr!("{account} calendars: {why}", account = account, why = why.words()),
            );
            let item = Rect::from_min_size(pos2(area.left(), area.top() + FAILED_PAD[0]), vec2(area.width(), item_high));
            status_item(ui, theme, item, &status, Some(p.minus), p.minus);
            let galley = super::galley(ui, tr!("try again"), font, p.cyan_text);
            let link = Rect::from_min_size(pos2(area.left() + 6.0, item.bottom()), vec2(galley.size().x.ceil(), link_high));
            // No log to open yet: drawn, coming soon, 16 after "try again" (its tag leaves no room on the line).
            let log = Rect::from_min_max(pos2(link.right() + 16.0, link.top()), pos2(area.right().max(link.right() + 17.0), link.bottom()));
            ui.scope_builder(UiBuilder::new().max_rect(log).layout(egui::Layout::left_to_right(egui::Align::Center)), |ui| {
                soon(ui, theme, |ui| link_sized(ui, theme, keymap, tr!("open log"), LinkSize::Small));
            });
            (link, Some(galley), format!("{status}, {}", tr!("try again")))
        }
    };
    let sense = if drawn.resting { Sense::hover() } else { Sense::click() };
    let response = ui.interact(press, drawn.id, sense);
    response.widget_info(|| WidgetInfo::labeled(egui::accesskit::Role::Button, !drawn.resting, &spoken));
    let response = if drawn.resting { response } else { response.on_hover_cursor(egui::CursorIcon::PointingHand) };
    let held = response.is_pointer_button_down_on();
    if let Some(galley) = link {
        if held {
            ui.painter().rect_filled(press, 0.0, p.cyan_muted);
        }
        // Under the pointer and while pressed, the font's own underline, as a link draws it.
        if held || response.hovered() {
            let baseline = super::button::baseline(font, press.center().y);
            let (below, thick) = font.face.underline();
            let middle = baseline + below * font.size;
            let band = (middle - thick * font.size / 2.0, middle + thick * font.size / 2.0);
            crate::draw::paint::sampled_band(ui.painter(), press.left()..=press.right(), band, p.cyan_text);
        }
        ui.painter().galley(pos2(press.left(), (press.center().y - galley.size().y / 2.0).round()), galley, p.cyan_text);
    }
    if response.has_focus() {
        focus_ring(ui.painter(), theme, press, Ring::Outside);
    }
    !drawn.resting && response.clicked()
}

/// A status item over `rect` as the status-item sheet draws it: 6 in, the `●` in `mark`'s colour when there is one
/// and the text 6 after it, in `ink`, on one line cut with `…` 6 before the item's end.
fn status_item(ui: &Ui, theme: &Theme, rect: Rect, text: &str, mark: Option<egui::Color32>, ink: egui::Color32) {
    let font = theme.fonts.mono_s;
    let ppp = ui.pixels_per_point();
    let top = ((rect.center().y - font.line / 2.0) * ppp).round() / ppp;
    let mut left = rect.left() + 6.0;
    if let Some(colour) = mark {
        // The sheet's `●` drawn as the progress line draws its own: a disc 0.75 of the size across, Plex Mono holds no
        // such sign.
        let centre = pos2(left + 5.0 / 11.0 * font.size, top + font.line / 2.0 + font.size / 88.0);
        ui.painter().circle_filled(centre, 0.75 * font.size / 2.0, colour);
        left += 11.0 + 6.0;
    }
    let galley = super::cut(ui, &super::plain(text), font, ink, (rect.right() - 6.0 - left).max(1.0));
    run_at(ui.painter(), pos2(left, top), galley, ink);
}

/// The sync's progress line 6 in from `area`'s left, its top where `centred` puts a strip (8 down at 100 %): `state`
/// with its note cut to what is left of the line. Its box.
fn progress_line(
    ui: &mut Ui,
    theme: &Theme,
    keymap: &Keymap,
    area: Rect,
    (state, note): (ProgressState, &str),
    still: bool,
    centred: impl Fn(f32) -> f32,
) -> Rect {
    let font = theme.fonts.mono_s;
    let ink = theme.palette.muted;
    let word = tr!("calendars");
    let measure = |text: &str| crate::draw::paint::run_box(super::galley(ui, text, font, ink).size().x);
    // The mark, the word and the bar, 8 apart, and 8 before the note.
    let taken = 11.0 + 8.0 + measure(word) + 8.0 + measure(&bar(state)) + 8.0;
    let room = (area.width() - 6.0 - taken).max(1.0);
    let note = fit(ui, theme, note, room);
    let strip = theme.boxes().strip;
    let line = Rect::from_min_size(pos2(area.left() + 6.0, centred(strip)), vec2((area.width() - 6.0).max(1.0), strip));
    let mut child = ui.new_child(UiBuilder::new().max_rect(line).layout(egui::Layout::left_to_right(egui::Align::Center)));
    progress_with(&mut child, theme, keymap, word, state, &note, still);
    child.min_rect()
}

/// The most characters of a note laid out: an account's name, kept to [`crate::calendar::sync::MOST_NAME`] already.
const MOST_NOTE: usize = 64;

/// `text` as one plain line in mono 11 that fits `room`: whole when it fits, else the longest start of it that fits
/// with one `…` after it (the progress line lays its note out as given). At most [`MOST_NOTE`] characters are measured,
/// a few layouts of them, while the sync runs.
fn fit(ui: &Ui, theme: &Theme, text: &str, room: f32) -> String {
    let font = theme.fonts.mono_s;
    let plain = super::plain(text);
    let (kept, cut) = super::capped(&plain, MOST_NOTE);
    let width = |text: &str| crate::draw::paint::run_box(super::galley(ui, text, font, theme.palette.muted).size().x);
    if !cut && width(kept) <= room {
        return kept.to_owned();
    }
    let ends: Vec<usize> = kept.char_indices().map(|(at, _)| at).skip(1).chain(std::iter::once(kept.len())).collect();
    // The longest start whose letters and the ellipsis fit: found by halving.
    let (mut low, mut high) = (0_usize, ends.len());
    while low < high {
        let middle = low + (high - low).div_ceil(2);
        let start = ends.get(middle.saturating_sub(1)).and_then(|end| kept.get(..*end)).unwrap_or_default().trim_end();
        if width(&format!("{start}…")) <= room {
            low = middle;
        } else {
            high = middle.saturating_sub(1);
        }
    }
    let start = low.checked_sub(1).and_then(|index| ends.get(index)).and_then(|end| kept.get(..*end)).unwrap_or_default().trim_end();
    format!("{start}…")
}

/// The gallery's board of the line in `view`, 222 wide: the control under `id`, its press running nothing, the ring
/// standing.
pub fn board(ui: &mut Ui, theme: &Theme, keymap: &Keymap, view: &SyncView, id: Id) {
    let failed = SyncView { state: SyncState::Failed { account: None, why: crate::calendar::sync::Why::Failed }, read: None };
    let (rect, _) = ui.allocate_exact_size(vec2(BOARD, height(theme, &failed)), Sense::hover());
    let rect = Rect::from_min_size(rect.min, vec2(BOARD, height(theme, view)));
    ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| calendar_sync(ui, theme, keymap, view, Drawn { id, resting: false, still: true }));
}

#[cfg(test)]
#[path = "calendar_sync_tests.rs"]
mod tests;
