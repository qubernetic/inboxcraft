// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Home, the page (`docs/design.md`, "Home"): at the left the brief of today, 600 wide, 56 in, 36 down and 24 above
//! its foot, its parts 20 apart (the section label, the headline in `display` on two lines, the lede in `read` on at
//! most two lines or the failing banner in its place, the row of four count tiles of one height, the agenda's block,
//! the drafts' block, two buttons); before the first account the label, the headline, the lede on three lines, the two
//! buttons and the block of what adding one does. At the right the emblem's place: the character field (the ground of
//! Home), and on it the emblem only where it fits whole (never cut, never smaller).
//!
//! The page takes what it shows as data ([`crate::home::Home`]) and gives back what was asked ([`Asked`]): it runs
//! no command, reads no mailbox and opens nothing. Where the body is narrower than the brief and its place, the
//! emblem's place goes first (the emblem whole, then the field); under 600 the brief takes the whole width; a brief
//! taller than the body scrolls. Every pressable part takes Tab in the drawn order: the banner's links, the four
//! tiles, the agenda's link, the draft rows, the drafts' link, the two buttons; the rest is information and takes none.

use egui::{Align, Id, Layout, Rect, Response, Role, ScrollArea, Sense, Stroke, StrokeKind, Ui, UiBuilder, WidgetInfo, pos2, vec2};

use crate::component::banner::{BannerKind, BannerLink, banner};
use crate::component::block::{Block, Inset, block};
use crate::component::block_line::{BlockLine, block_line};
use crate::component::button::TextButton;
use crate::component::check_step::{StepState, check_step};
use crate::component::count_tile::{CountTile, Tone, count_tile, height as tile_height};
use crate::component::draft_row::{DraftRow, draft_row};
use crate::component::{self, label};
use crate::draw::paint;
use crate::field::{self, Emblem, Field, Level, Opts};
use crate::home::{AGENDA, Buttons, Count, Counted, DRAFTS, Home, Kind};
use crate::keymap::{self, Keymap};
use crate::theme::Theme;
use crate::tr;

/// The brief's width, in points.
pub const BRIEF: f32 = 600.0;

/// The room round the brief's parts, in points: above, at the left, under.
const PAD_TOP: f32 = 36.0;
const PAD_LEFT: f32 = 56.0;
const PAD_FOOT: f32 = 24.0;

/// The room between the brief's parts, in points.
const GAP: f32 = 20.0;

/// The room between the two buttons, in points.
const BUTTON_GAP: f32 = 8.0;

/// An agenda line's height at 100 % text, in points.
const AGENDA_LINE: f32 = 40.0;

/// The room inside a line's ends, and between its parts, in points.
const LINE_PAD: f32 = 16.0;
const LINE_GAP: f32 = 12.0;

/// The account bar of an agenda line, in points.
const BAR: egui::Vec2 = vec2(3.0, 12.0);

/// The block of what adding an account does: the room round its steps and between them, in points.
const STEPS_PAD_Y: f32 = 10.0;
const STEPS_GAP: f32 = 2.0;

/// The largest side of a page Home draws, in points: a larger rect is cut to it.
pub const MOST_SIDE: f32 = 16_384.0;

/// What the owner asked of Home this frame. Home runs none of it; its caller does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Asked {
    /// A count's tile: its place (the inbox, the screener, Later, the outbox).
    Count(Counted),
    /// The agenda's link: the week.
    Week,
    /// The agenda's link when more is left today than it shows: the rest of today.
    Today,
    /// A draft row, or its "open": the draft in the composer.
    OpenDraft(u64),
    /// A draft row's "discard".
    DiscardDraft(u64),
    /// "all drafts (N)": every draft.
    AllDrafts,
    /// Read mail.
    ReadMail,
    /// Write.
    Write,
    /// Open the calendar.
    Calendar,
    /// Add an account.
    AddAccount,
    /// Look around with sample mail.
    SampleMail,
    /// The failing banner's "open account".
    OpenAccount,
    /// The failing banner's "open outbox".
    OpenOutbox,
}

/// Where the parts of Home stand this frame (what the tests measure).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Parts {
    /// The brief.
    pub brief: Option<Rect>,
    /// The section label.
    pub label: Option<Rect>,
    /// The headline.
    pub headline: Option<Rect>,
    /// The lede.
    pub lede: Option<Rect>,
    /// The failing banner.
    pub banner: Option<Rect>,
    /// The row of count tiles.
    pub counts: Option<Rect>,
    /// The agenda's block.
    pub agenda: Option<Rect>,
    /// The drafts' block.
    pub drafts: Option<Rect>,
    /// The block of what adding an account does.
    pub steps: Option<Rect>,
    /// The two buttons.
    pub buttons: Option<Rect>,
    /// The emblem's place.
    pub stage: Option<Rect>,
    /// The emblem, where it stands whole.
    pub emblem: Option<Rect>,
}

/// What Home drew and what it was asked this frame.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Drawn {
    /// What was asked.
    pub asked: Option<Asked>,
    /// Where its parts stand.
    pub parts: Parts,
}

/// The emblem's place as the caller gives it: the character field Home stands on, the emblem on it (`None`: none),
/// and the motion level.
pub struct Stage<'a> {
    /// The field: the ground of the emblem's place. The emblem it draws is the one given to it
    /// ([`Field::with_emblem`]).
    pub field: &'a mut Field,
    /// The emblem, for its size: where it does not fit whole it is not drawn.
    pub emblem: Option<&'a Emblem>,
    /// How much the field moves.
    pub level: Level,
}

/// Where `emblem` stands in `stage`: centred, at its one size, and only if it fits whole.
#[must_use]
pub fn emblem_place(stage: Rect, emblem: &Emblem) -> Option<Rect> {
    let size = emblem.size();
    (size.x > 0.0 && size.y > 0.0 && size.x <= stage.width() && size.y <= stage.height())
        .then(|| Rect::from_min_size((stage.center() - size / 2.0).round(), size))
}

/// `rect` as Home draws in it: cut to [`MOST_SIDE`]; `None` for one that is empty or not a number.
fn usable(rect: Rect) -> Option<Rect> {
    let fine = rect.min.x.is_finite() && rect.min.y.is_finite() && rect.max.x.is_finite() && rect.max.y.is_finite();
    let rect = fine.then(|| Rect::from_min_size(rect.min, vec2(rect.width().min(MOST_SIDE), rect.height().min(MOST_SIDE))))?;
    (rect.width() >= 1.0 && rect.height() >= 1.0).then_some(rect)
}

/// Home in `rect`: the brief of `home` at the left, the emblem's place at the right. What was asked, and where its
/// parts stand; nothing at all in a rect that is empty or not a number.
pub fn show(ui: &mut Ui, theme: &Theme, keymap: &Keymap, home: &Home, rect: Rect, stage: Stage<'_>) -> Drawn {
    let Some(rect) = usable(rect) else { return Drawn::default() };
    let mut parts = Parts::default();
    ui.painter().rect_filled(rect, 0.0, theme.palette.bg);
    let brief = Rect::from_min_size(rect.min, vec2(rect.width().min(BRIEF), rect.height()));
    parts.brief = Some(brief);
    if rect.width() > BRIEF {
        let place = Rect::from_min_max(pos2(brief.right(), rect.top()), rect.max);
        let emblem = stage.emblem.and_then(|emblem| emblem_place(place, emblem));
        field::show(ui, theme, stage.field, place, stage.level, Opts { on_home: true, on_pane: false, emblem });
        parts.stage = Some(place);
        parts.emblem = emblem;
    }
    let mut asked = None;
    ui.scope_builder(UiBuilder::new().max_rect(brief).id_salt("home"), |ui| {
        ui.set_clip_rect(brief.intersect(ui.clip_rect()));
        let area = ScrollArea::vertical().id_salt("home-brief").auto_shrink([false, false]).max_height(brief.height());
        crate::component::scroll::show(ui, theme, area, |ui| {
            let margin = egui::Margin { left: PAD_LEFT as i8, right: 0, top: PAD_TOP as i8, bottom: PAD_FOOT as i8 };
            egui::Frame::NONE.inner_margin(margin).show(ui, |ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, GAP);
                // As wide as the brief leaves, less the scroll bar while it shows.
                ui.set_width(ui.available_width().clamp(1.0, (brief.width() - PAD_LEFT).max(1.0)));
                asked = brief_parts(ui, theme, keymap, home, &mut parts);
            });
        });
    });
    Drawn { asked, parts }
}

/// The brief's parts, top down, in `ui` (as wide as the brief's content).
fn brief_parts(ui: &mut Ui, theme: &Theme, keymap: &Keymap, home: &Home, parts: &mut Parts) -> Option<Asked> {
    let mut asked = None;
    let index = "[ 00 ]";
    // The date cut once to what the line leaves after the number (8 apart, as the label sets them).
    let number = component::galley(ui, index, theme.fonts.label, theme.palette.cyan_text).size().x.ceil();
    let room = (ui.available_width() - number - 8.0).max(1.0);
    let date = component::fit_capped(ui, &component::plain(component::capped(&home.date, component::MOST).0), theme.fonts.label, room);
    parts.label = Some(label::section_label(ui, theme, index, &date, false).rect);
    parts.headline = Some(headline(ui, theme, home).rect);
    let first = home.kind == Kind::FirstRun;
    if let (Some(failing), false) = (&home.failing, first) {
        let mut links = Vec::new();
        if failing.account {
            links.push(BannerLink { name: tr!("open account"), id: "account" });
        }
        if failing.outbox {
            links.push(BannerLink { name: tr!("open outbox"), id: "outbox" });
        }
        let top = ui.cursor().min.y;
        let text = crate::home::kept(&failing.text);
        if let Some(id) = banner(ui, theme, keymap, BannerKind::Failing, &text, &links) {
            asked = Some(if id == "account" { Asked::OpenAccount } else { Asked::OpenOutbox });
        }
        parts.banner = Some(Rect::from_min_max(pos2(ui.min_rect().left(), top), pos2(ui.min_rect().right(), ui.min_rect().bottom())));
    } else {
        parts.lede = Some(lede(ui, theme, &home.lede, if first { 3 } else { 2 }).rect);
    }
    if first {
        let (rect, pressed) = buttons(ui, theme, keymap, home.buttons);
        parts.buttons = Some(rect);
        asked = asked.or(pressed);
        parts.steps = Some(steps(ui, theme));
        return asked;
    }
    let (rect, pressed) = counts(ui, theme, &home.counts);
    parts.counts = Some(rect);
    asked = asked.or(pressed);
    let (rect, pressed) = agenda(ui, theme, keymap, home);
    parts.agenda = Some(rect);
    asked = asked.or(pressed);
    let (rect, pressed) = drafts(ui, theme, keymap, home);
    parts.drafts = Some(rect);
    asked = asked.or(pressed);
    let (rect, pressed) = buttons(ui, theme, keymap, home.buttons);
    parts.buttons = Some(rect);
    asked.or(pressed)
}

/// The headline: the greeting over the name in `display` and `ink`, each line cut once at the brief's width.
fn headline(ui: &mut Ui, theme: &Theme, home: &Home) -> Response {
    let font = theme.fonts.display;
    let ink = theme.palette.ink;
    let width = ui.available_width().max(1.0);
    let (first, second) = home.headline();
    let second = second.map(|name| match home.kind {
        Kind::Brief => tr!("{name}.", name = component::plain(component::capped(name, component::MOST).0)),
        Kind::FirstRun => name.to_owned(),
    });
    let lines = if second.is_some() { 2.0 } else { 1.0 };
    let (rect, response) = ui.allocate_exact_size(vec2(width, (font.line * lines).ceil()), Sense::hover());
    let spoken = second.as_ref().map_or_else(|| first.to_owned(), |second| format!("{first} {second}"));
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, &spoken));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter_at(rect);
        let fitted = component::fit_capped(ui, first, font, width);
        paint::run_at(&painter, rect.min, component::galley(ui, &fitted, font, ink), ink);
        if let Some(second) = second {
            let fitted = component::fit_capped(ui, &second, font, width);
            paint::run_at(&painter, pos2(rect.left(), rect.top() + font.line), component::galley(ui, &fitted, font, ink), ink);
        }
    }
    response
}

/// The lede in `read` and `muted`, wrapped onto at most `rows` lines, the last cut with one `…`.
fn lede(ui: &mut Ui, theme: &Theme, text: &str, rows: usize) -> Response {
    let font = theme.fonts.read;
    let width = ui.available_width().max(1.0);
    let text = crate::home::kept(text);
    let galley = component::wrapped(ui, &text, font, theme.palette.muted, width, rows, Align::LEFT);
    let lines = galley.rows.len().clamp(1, rows.max(1));
    let high = (font.line * lines as f32).ceil();
    let (rect, response) = ui.allocate_exact_size(vec2(width, high), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, &text));
    if ui.is_rect_visible(rect) {
        ui.painter_at(rect).galley(rect.min, galley, theme.palette.muted);
    }
    response
}

/// The words of a tile counting `what`, plain when it could not be read, the "nothing" form when it is 0.
fn tile_words(count: &Count) -> (&'static str, Tone) {
    let none = count.count == Some(0);
    match (count.what, none) {
        (Counted::ToRead, false) => (tr!("To read"), Tone::Accent),
        (Counted::ToRead, true) => (tr!("Nothing to read"), Tone::Accent),
        (Counted::ToScreen, false) => (tr!("To screen"), Tone::Plain),
        (Counted::ToScreen, true) => (tr!("Nobody to screen"), Tone::Plain),
        (Counted::BackToday, false) => (tr!("Back today"), Tone::Warn),
        (Counted::BackToday, true) => (tr!("Nothing back today"), Tone::Warn),
        (Counted::Outbox, false) => (tr!("In the outbox"), Tone::Plain),
        (Counted::Outbox, true) => (tr!("Nothing to send"), Tone::Plain),
        (Counted::NotSent, _) => (tr!("Not sent"), Tone::Minus),
    }
}

/// The row of the four count tiles on `surface` in a `line` edge, 1 px hairlines between them, of one height (the
/// tallest tile's). Its rect, and the count whose tile was pressed.
fn counts(ui: &mut Ui, theme: &Theme, counts: &[Count; 4]) -> (Rect, Option<Asked>) {
    let p = &theme.palette;
    let width = ui.available_width().max(8.0).round();
    // The tiles' edges, rounded: 135, 135, 134, 135 across 544.
    let inner = width - 2.0 - 3.0;
    let edge = |at: f32| (inner * at / 4.0).round();
    let words: Vec<_> = counts.iter().map(tile_words).collect();
    let high = (0..4)
        .zip(&words)
        .map(|(at, (label, _))| tile_height(ui, theme, label, edge(at as f32 + 1.0) - edge(at as f32)))
        .fold(Theme::size(theme, 76.0), f32::max);
    let (rect, _) = ui.allocate_exact_size(vec2(width, high + 2.0), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, p.surface);
    let mut asked = None;
    for (at, (count, (label, tone))) in counts.iter().zip(&words).enumerate() {
        let at = at as f32;
        let left = rect.left() + 1.0 + edge(at) + at;
        let tile = Rect::from_min_size(pos2(left, rect.top() + 1.0), vec2(edge(at + 1.0) - edge(at), high));
        let shown = CountTile { id: Id::unique(("home-count", at as u8)), count: count.count.unwrap_or(0), label, tone: *tone, row: Some(high) };
        // A child that takes no room of the row's own: the row has taken it.
        let mut child = ui.new_child(UiBuilder::new().max_rect(tile).id_salt(("home-count", at as u8)));
        let pressed = count_tile(&mut child, theme, &shown, None).clicked();
        if pressed {
            asked = Some(Asked::Count(count.what));
        }
        if at < 3.0 {
            let x = tile.right();
            ui.painter().rect_filled(Rect::from_min_max(pos2(x, tile.top()), pos2(x + 1.0, tile.bottom())), 0.0, p.line);
        }
    }
    ui.painter().rect_stroke(rect, 0.0, Stroke::new(1.0, p.line), StrokeKind::Inside);
    (rect, asked)
}

/// The agenda's block: the rest of today's lines (at most [`AGENDA`]), or why there are none, then its link.
fn agenda(ui: &mut Ui, theme: &Theme, keymap: &Keymap, home: &Home) -> (Rect, Option<Asked>) {
    let head = Block {
        id: Id::unique("home-agenda"),
        index: "[ 01 ]",
        word: tr!("Agenda · rest of today"),
        inset: Inset::Flush,
        presses: false,
        selected: false,
    };
    let drawn = block(ui, theme, &head, None, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for line in home.agenda.iter().take(AGENDA) {
            agenda_line(ui, theme, line);
        }
        if home.agenda.is_empty() {
            let empty = match &home.tomorrow {
                Some((time, title)) => tr!(
                    "No more events today. Tomorrow starts with {title}, {time}.",
                    title = crate::home::kept(title),
                    time = crate::home::kept(time)
                ),
                None => tr!("No more events today.").to_owned(),
            };
            block_line(ui, theme, keymap, &BlockLine::Empty(&empty), None);
        }
        let shown = u64::try_from(home.agenda.len().min(AGENDA)).unwrap_or(u64::MAX);
        let more = home.agenda_total.saturating_sub(shown);
        if more > 0 {
            let words = tr!("{n} more today →", n = label::count(more));
            block_line(ui, theme, keymap, &BlockLine::More(&words), None).then_some(Asked::Today)
        } else {
            block_line(ui, theme, keymap, &BlockLine::More(tr!("open the week →")), None).then_some(Asked::Week)
        }
    });
    (drawn.rect, drawn.inner)
}

/// A line of the agenda, 40 high, as the event block's row draws one: its time in `mono` and `muted`, its account's
/// bar, its title in `body` and `ink`, where and how long in `mono_s` and `muted` at the right end, a `panel_line`
/// hairline under it. Information: no Tab stop.
fn agenda_line(ui: &mut Ui, theme: &Theme, line: &crate::home::AgendaLine) {
    let p = &theme.palette;
    let f = &theme.fonts;
    let width = ui.available_width().max(LINE_PAD * 2.0 + 1.0);
    let (rect, response) = ui.allocate_exact_size(vec2(width, Theme::size(theme, AGENDA_LINE)), Sense::hover());
    let [time, title, detail] = [&line.time, &line.title, &line.detail].map(|text| crate::home::kept(text));
    let spoken = format!("{time} {title} {detail}");
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, &spoken));
    if !ui.is_rect_visible(rect) {
        return;
    }
    let painter = ui.painter_at(rect);
    let middle = rect.top() + (rect.height() - 1.0) / 2.0;
    let at = |high: f32| (middle - high / 2.0).round();
    let right = rect.right() - LINE_PAD;
    let room = right - rect.left() - LINE_PAD;
    let time = component::cut(ui, &time, f.mono, p.muted, (room / 4.0).max(1.0));
    let mut left = rect.left() + LINE_PAD;
    let time_wide = time.size().x.ceil();
    paint::run_at(&painter, pos2(left, at(f.mono.line)), time, p.muted);
    left += time_wide + LINE_GAP;
    let bar_high = Theme::size(theme, BAR.y);
    painter.rect_filled(Rect::from_min_size(pos2(left, at(bar_high)), vec2(BAR.x, bar_high)), 0.0, theme.account_colour(line.account));
    left += BAR.x + LINE_GAP;
    let mut title_right = right;
    if !detail.is_empty() {
        let galley = component::cut(ui, &detail, f.mono_s, p.muted, ((right - left) / 2.0).max(1.0));
        title_right = paint::run_right(&painter, right, at(f.mono_s.line), galley, p.muted) - LINE_GAP;
    }
    let title = component::cut(ui, &title, f.body, p.ink, (title_right - left).max(1.0));
    paint::run_at(&painter, pos2(left, at(f.body.line)), title, p.ink);
    let line_y = rect.bottom() - 1.0;
    painter.rect_filled(Rect::from_min_max(pos2(rect.left(), line_y), rect.max), 0.0, p.panel_line);
}

/// The drafts' block: the newest drafts (at most [`DRAFTS`]) and "all drafts (N)", or why there are none.
fn drafts(ui: &mut Ui, theme: &Theme, keymap: &Keymap, home: &Home) -> (Rect, Option<Asked>) {
    let head = Block {
        id: Id::unique("home-drafts"),
        index: "[ 02 ]",
        word: tr!("Pick up where you left"),
        inset: Inset::Flush,
        presses: false,
        selected: false,
    };
    let drawn = block(ui, theme, &head, None, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        let mut asked = None;
        for draft in home.drafts.iter().take(DRAFTS) {
            let subject = draft.subject.as_deref().map(crate::home::kept);
            let [to, when] = [&draft.to, &draft.when].map(|text| crate::home::kept(text));
            let spoken = format!("{} {to} {when}", subject.as_deref().unwrap_or(tr!("(no subject)")));
            let row = DraftRow {
                id: Id::unique(("home-draft", draft.id)),
                spoken: &spoken,
                subject: subject.as_deref(),
                to: &to,
                when: &when,
                last: false,
            };
            // In a child of its own: what the row lays down inside itself (its links) moves no cursor of the block's.
            let (place, _) =
                ui.allocate_exact_size(vec2(ui.available_width(), Theme::size(theme, crate::component::draft_row::HEIGHT)), Sense::hover());
            let mut child = ui.new_child(UiBuilder::new().max_rect(place).id_salt(("home-draft", draft.id)));
            let drawn = draft_row(&mut child, theme, keymap, &row, false, None);
            if drawn.discard {
                asked = Some(Asked::DiscardDraft(draft.id));
            } else if drawn.open || drawn.row.response.clicked() {
                asked = Some(Asked::OpenDraft(draft.id));
            }
        }
        if home.drafts.is_empty() {
            block_line(ui, theme, keymap, &BlockLine::Empty(tr!("No drafts. What you start and do not send is kept here.")), None);
        } else {
            let words = tr!("all drafts ({n}) →", n = label::count(home.drafts_total.max(1)));
            if block_line(ui, theme, keymap, &BlockLine::More(&words), None) {
                asked = Some(Asked::AllDrafts);
            }
        }
        asked
    });
    (drawn.rect, drawn.inner)
}

/// The two buttons, 8 apart: the primary one with its arrow (and its key's chip), the secondary one.
fn buttons(ui: &mut Ui, theme: &Theme, keymap: &Keymap, buttons: Buttons) -> (Rect, Option<Asked>) {
    let (first, second) = match buttons {
        Buttons::ReadAndWrite => ((tr!("Read mail"), Some(keymap::MAIL), Asked::ReadMail), (tr!("Write"), Some(keymap::COMPOSE), Asked::Write)),
        Buttons::WriteAndCalendar => {
            ((tr!("Write"), Some(keymap::COMPOSE), Asked::Write), (tr!("Open the calendar"), Some(keymap::CALENDAR), Asked::Calendar))
        }
        Buttons::FirstRun => ((tr!("Add an account"), None, Asked::AddAccount), (tr!("Look around with sample mail"), None, Asked::SampleMail)),
    };
    let width = ui.available_width().max(1.0);
    let inner = ui.allocate_ui_with_layout(vec2(width, 0.0), Layout::left_to_right(Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = BUTTON_GAP;
        let primary = TextButton::new(first.0).primary().arrow().key(true).within(width);
        let primary = match first.1 {
            Some(action) => primary.action(action),
            None => primary,
        };
        let one = primary.show(ui, theme, keymap).clicked();
        let room = (ui.available_width()).max(1.0);
        let secondary = TextButton::new(second.0).secondary().within(room);
        let secondary = match second.1 {
            Some(action) => secondary.action(action),
            None => secondary,
        };
        let two = secondary.show(ui, theme, keymap).clicked();
        if one { Some(first.2) } else { two.then_some(second.2) }
    });
    (inner.response.rect, inner.inner)
}

/// The block of what adding an account does: its four steps, waiting.
fn steps(ui: &mut Ui, theme: &Theme) -> Rect {
    let head = Block {
        id: Id::unique("home-steps"),
        index: "[ 01 ]",
        word: tr!("What happens when you add one"),
        inset: Inset::Flush,
        presses: false,
        selected: false,
    };
    block(ui, theme, &head, None, |ui| {
        let width = ui.available_width();
        ui.add_space(STEPS_PAD_Y);
        let room = Rect::from_min_size(ui.cursor().min + vec2(LINE_PAD, 0.0), vec2((width - LINE_PAD * 2.0).max(1.0), 0.0));
        ui.scope_builder(UiBuilder::new().max_rect(room).layout(Layout::top_down(Align::Min)), |ui| {
            ui.spacing_mut().item_spacing.y = STEPS_GAP;
            check_step(ui, theme, tr!("Find"), tr!("the servers of your address"), tr!("looked up for you"), StepState::Waiting, None);
            check_step(ui, theme, tr!("Check"), tr!("the sign-in to IMAP and SMTP"), tr!("TLS, or refused"), StepState::Waiting, None);
            check_step(ui, theme, tr!("Keep"), tr!("the password in the keyring"), tr!("never in a file"), StepState::Waiting, None);
            check_step(ui, theme, tr!("Fetch"), tr!("your mail to this computer"), tr!("readable offline"), StepState::Waiting, None);
        });
        ui.add_space(STEPS_PAD_Y);
    })
    .rect
}

/// The gallery's page of Home in `state` (see [`crate::home::sample`]) across `size`, on a still field without an
/// emblem; nothing for a state it does not know.
pub fn gallery(ui: &mut Ui, theme: &Theme, keymap: &Keymap, state: &str, field: &mut Field, size: egui::Vec2) {
    let Some(home) = crate::home::sample(state) else { return };
    let rect = Rect::from_min_size(ui.max_rect().min, size);
    show(ui, theme, keymap, &home, rect, Stage { field, emblem: None, level: Level::Off });
}

#[cfg(test)]
#[path = "home_tests.rs"]
mod tests;
