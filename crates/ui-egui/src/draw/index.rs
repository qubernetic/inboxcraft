// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The index: the window's left column on the `panel`, 232 wide (56 narrow). From its top, 12 down: the five places
//! `[ 00 ] HOME` … `[ 04 ] SETTINGS`, and under MAIL the mailboxes: the shared ones (Inbox, New senders, Drafts, the
//! Outbox while anything is in it, Sent, Archive, Junk, Bin), 8 apart, then one row per account that opens to its own
//! folders. Only the mailboxes scroll: when they do not fit, they take the room the places and the owner's row leave,
//! between two hairlines, and the rest stay where they are. At the foot, over a hairline, the owner's row (avatar and
//! address) opens the account picker, a float that narrows the mail to one account or shows them all.
//!
//! HOME shows Home, MAIL the mail, and so on, each as its key does. A mailbox or account name is outside data: it is cleaned of control and bidi characters and
//! capped at [`MOST_NAME`] characters when a row is built, and cut with one `…` inside its row. Only the rows near the
//! mailboxes' viewport are built and laid out (a screen above and below it, so Tab walks on into rows out of sight
//! and the row it lands on is scrolled into sight): 40 000 folders cost their offsets, not their rows.

use std::collections::BTreeSet;

use egui::{Id, Rect, Response, Role as AxRole, ScrollArea, Ui, UiBuilder, WidgetInfo, pos2, vec2};
use inboxcraft_model::Role;

use crate::component::account_row::{self, AccountRow, Sync};
use crate::component::mailbox_row::{self, MailboxRow};
use crate::component::nav_row::{self, NavRow, Standing, base};
use crate::component::{avatar, popup, tooltip};
use crate::iconset::{self, IconId};
use crate::keymap::{self, Keymap};
use crate::mailbox::{self, Mailbox, Pile, role_text};
use crate::theme::Theme;
use crate::tr;

use super::paint;

/// The places of the index, `[ 00 ]` … `[ 04 ]`, in this order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Place {
    Home,
    Mail,
    Calendar,
    Later,
    Settings,
}

/// What the owner asked for in the index this frame; the caller applies it after the frame's drawing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Asked {
    /// A place.
    pub place: Option<Place>,
    /// A mailbox (a shared one, a folder or the Outbox), which also shows Mail.
    pub mailbox: Option<mailbox::Place>,
    /// The account picker's choice: one account, or all of them.
    pub account: Option<Option<u64>>,
    /// An account row pressed: its folders open or close ([`toggle`]).
    pub toggle: Option<u64>,
}

/// The most characters of a name a row keeps.
pub(crate) const MOST_NAME: usize = 64;

/// The room above the first place, in points.
const TOP: f32 = 12.0;

/// The gap between the shared mailboxes and the accounts, in points.
const GAP: f32 = 8.0;

/// The owner's row: its height, the avatar's side and left edge, the address's left edge, in points.
const OWNER: f32 = 40.0;
const AVATAR: f32 = 24.0;
const AVATAR_LEFT: f32 = 16.0;
const ADDRESS_LEFT: f32 = 50.0;

/// The account picker's width, in points.
const PICKER: f32 = 230.0;

/// How far a menu over its button stands over it (`popup::menu_up`), in points: the picker stands on the owner's row.
const MENU_DROP: f32 = 4.0;

/// The phosphor glyph of the Outbox (tray-arrow-up).
const OUTBOX: char = '\u{ee52}';

/// A row of the mailboxes, built only near the viewport.
#[derive(Debug, Clone, Copy)]
enum Line<'a> {
    /// A shared mailbox: what it opens, its name, its icon, how many wait.
    Shared(mailbox::Place, &'a str, char, u64),
    /// The room between the shared mailboxes and the accounts.
    Gap,
    /// An account, open or not.
    Account(&'a mailbox::AccountRow, bool),
    /// A folder of an account, and the account's label, which a screen reader hears with it.
    Folder(&'a mailbox::FolderRow, &'a str),
}

impl Line<'_> {
    /// Its height at `theme`'s text scale.
    fn height(&self, theme: &Theme) -> f32 {
        match self {
            Self::Shared(..) | Self::Folder(..) => Theme::size(theme, mailbox_row::HEIGHT),
            Self::Gap => GAP,
            Self::Account(..) => Theme::size(theme, account_row::HEIGHT),
        }
    }
}

/// `name` as a row shows it: control and bidi characters out, at most [`MOST_NAME`] characters.
pub(crate) fn clean(name: &str) -> String {
    let (kept, _) = crate::component::capped(name, MOST_NAME);
    crate::component::plain(kept).into_owned()
}

/// The icon of a shared mailbox or folder of `role`.
fn icon(role: Option<Role>) -> char {
    iconset::glyph(match role {
        Some(Role::Inbox) => IconId::Inbox,
        Some(Role::Sent) => IconId::Send,
        Some(Role::Drafts) => IconId::Compose,
        Some(Role::Trash) => IconId::Delete,
        Some(Role::Junk) => IconId::Junk,
        Some(Role::Archive) => IconId::Archive,
        _ => IconId::Move,
    })
}

/// Where the accounts open in the index are kept.
fn open_id() -> Id {
    Id::unique("index-open-accounts")
}

/// The mailboxes' rows, in order: the shared ones, the gap, each account and, open, its folders. Our own hidden folders
/// (the screener's, snoozed and blocked mail) have no row: New senders, Later and the settings show what is in them.
fn lines<'a>(mailbox: &'a Mailbox, open: &BTreeSet<u64>) -> Vec<Line<'a>> {
    let mut lines = Vec::new();
    let sends = u64::try_from(mailbox.outbox().len()).unwrap_or(u64::MAX);
    for view in mailbox.views().iter().filter(|view| !view.role.is_hidden()) {
        lines.push(Line::Shared(mailbox::Place::View(view.role), role_text(view.role), icon(Some(view.role)), view.unread));
        if view.role == Role::Inbox {
            // New senders counts the senders the screener holds back, as Home's TO SCREEN does.
            let waiting = u64::try_from(mailbox.waiting().len()).unwrap_or(u64::MAX);
            lines.push(Line::Shared(mailbox::Place::Senders, tr!("New senders"), iconset::glyph(IconId::People), waiting));
        }
        if view.role == Role::Drafts && sends > 0 {
            lines.push(Line::Shared(mailbox::Place::Outbox, tr!("Outbox"), OUTBOX, sends));
        }
    }
    if mailbox.accounts().is_empty() {
        return lines;
    }
    lines.push(Line::Gap);
    for account in mailbox.accounts() {
        let opened = open.contains(&account.id);
        lines.push(Line::Account(account, opened));
        if opened {
            lines.extend(
                mailbox.folders_of(account.id).into_iter().filter(|folder| folder.role.is_none()).map(|folder| Line::Folder(folder, &account.label)),
            );
        }
    }
    lines
}

/// The mailbox shown, when Mail is the place and no search runs.
fn shown(mailbox: &Mailbox, at: Place) -> Option<mailbox::Place> {
    (at == Place::Mail && mailbox.query().is_empty()).then(|| mailbox.place())
}

/// Draws one row of the mailboxes and notes what it asked for.
fn line(ui: &mut Ui, theme: &Theme, line: &Line<'_>, active: Option<mailbox::Place>, narrow: bool, asked: &mut Asked, toggled: &mut Option<u64>) {
    let response = match *line {
        Line::Shared(place, name, glyph, count) => {
            // The Outbox counts sends waiting and New senders the senders held back, not unread mail.
            let spoken = match (place, count) {
                (mailbox::Place::Outbox, 1) => tr!("Outbox, 1 send").to_owned(),
                (mailbox::Place::Outbox, _) => tr!("Outbox, {count} sends", count = count),
                (mailbox::Place::Senders, 0) => name.to_owned(),
                (mailbox::Place::Senders, _) => tr!("{name}, {count} waiting", name = name, count = count),
                _ => spoken(name, None, count),
            };
            let row = MailboxRow {
                id: Id::unique("index-shared").with(format!("{place:?}")),
                spoken: &spoken,
                icon: glyph,
                name,
                count,
                active: active == Some(place),
            };
            let response = mailbox_row::mailbox_row(ui, theme, &row, narrow, None);
            if response.clicked() {
                asked.mailbox = Some(place);
            }
            response
        }
        Line::Gap => {
            ui.allocate_space(vec2(ui.available_width().max(0.0), GAP));
            return;
        }
        Line::Account(account, open) => {
            let name = clean(&account.label);
            let spoken = if open { tr!("{name}, open", name = name) } else { name.clone() };
            let index = u64::try_from(account.index).unwrap_or(0);
            let row = AccountRow {
                id: Id::unique("index-account").with(account.id),
                spoken: &spoken,
                name: &name,
                account: index,
                open,
                sync: Sync::Quiet,
            };
            let response = account_row::account_row(ui, theme, &row, narrow, None);
            if response.clicked() {
                *toggled = Some(account.id);
            }
            response
        }
        Line::Folder(folder, account) => {
            let name = clean(&folder.name);
            let spoken = spoken(&name, Some(&clean(account)), folder.unread);
            let place = mailbox::Place::Folder(folder.id);
            let row = MailboxRow {
                id: Id::unique("index-folder").with(folder.id),
                spoken: &spoken,
                icon: icon(folder.role),
                name: &name,
                count: folder.unread,
                active: active == Some(place),
            };
            let response = mailbox_row::mailbox_row(ui, theme, &row, narrow, None);
            if response.clicked() {
                asked.mailbox = Some(place);
            }
            response
        }
    };
    // A press on an account row shows no list: the keyboard stays on the row, never on nothing. A mailbox shows its
    // list, which takes the keys (J, K, the arrows) as before.
    if matches!(line, Line::Account(..)) && response.clicked() {
        response.request_focus();
    }
    if response.gained_focus() {
        response.scroll_to_me(None);
    }
}

/// What a screen reader hears for a mailbox `name` with `count` waiting.
fn spoken(name: &str, account: Option<&str>, count: u64) -> String {
    let named = match account {
        Some(account) => tr!("{folder} in {account}", folder = name, account = account),
        None => name.to_owned(),
    };
    if count == 0 { named } else { tr!("{name}, {count} unread", name = named, count = count) }
}

/// A place's row; hovered, it says its name and its key, as the rail's buttons did.
fn place(ui: &mut Ui, theme: &Theme, keymap: &Keymap, which: Place, at: Place, count: u64, narrow: bool) -> Response {
    let (index, label, icon) = match which {
        Place::Home => ("[ 00 ]", tr!("Home"), IconId::Home),
        Place::Mail => ("[ 01 ]", tr!("Mail"), IconId::Mail),
        Place::Calendar => ("[ 02 ]", tr!("Calendar"), IconId::Calendar),
        Place::Later => ("[ 03 ]", tr!("Later"), IconId::Snooze),
        Place::Settings => ("[ 04 ]", tr!("Settings"), IconId::Settings),
    };
    let standing = match (which, at) {
        (Place::Mail, Place::Mail) => Standing::Open,
        (which, at) if which == at => Standing::Active,
        _ => Standing::Rest,
    };
    let row = NavRow { id: Id::unique("index-place").with(index), spoken: label, index, label, icon, count, standing };
    let action = match which {
        Place::Home => keymap::HOME,
        Place::Mail => keymap::MAIL,
        Place::Calendar => keymap::CALENDAR,
        Place::Later => keymap::LATER,
        Place::Settings => keymap::SETTINGS,
    };
    let response = tooltip::attach(nav_row::nav_row(ui, theme, &row, narrow, None), theme, keymap, label, Some(action));
    // A press on Calendar or Home shows no list: the keyboard stays on the place, never on nothing. Mail and Later show
    // a list, which takes the keys as before.
    if matches!(which, Place::Calendar | Place::Home) && response.clicked() {
        response.request_focus();
    }
    response
}

/// The side of the square SETTINGS carries while a check runs elsewhere, as the unread square's, in points.
const CHECK_MARK: f32 = 6.0;

/// The square on SETTINGS's `row` while the check of an added account runs elsewhere: `cyan_edge`, at the right end where
/// a count stands (narrow, at the icon's top right).
fn check_mark(ui: &Ui, theme: &Theme, row: Rect, narrow: bool) {
    if !ui.is_rect_visible(row) {
        return;
    }
    let right = if narrow { row.center().x + nav_row::ICON / 2.0 + CHECK_MARK } else { row.right() - nav_row::RIGHT };
    let top = if narrow { row.center().y - nav_row::ICON / 2.0 - CHECK_MARK / 2.0 } else { row.center().y - CHECK_MARK / 2.0 };
    let square = Rect::from_min_size(pos2((right - CHECK_MARK).round(), top.round()), vec2(CHECK_MARK, CHECK_MARK));
    ui.painter().rect_filled(square, 0.0, theme.palette.cyan_edge);
}

/// Opens the folders of account `id` in the index, or closes them when they are open.
pub(crate) fn toggle(ctx: &egui::Context, id: u64) {
    let mut open: BTreeSet<u64> = ctx.data(|data| data.get_temp::<BTreeSet<u64>>(open_id())).unwrap_or_default();
    if !open.remove(&id) {
        open.insert(id);
    }
    ctx.data_mut(|data| data.insert_temp(open_id(), open));
}

/// Draws the index into `ui`'s whole rect and returns what was asked for (the caller applies it, [`toggle`] too). `at` is the place shown; `narrow` draws the
/// 56 column (icons, a square where something waits, the accounts' bars).
///
/// While not `live` (a modal surface in front) the owner's row does not open the account picker: a screen reader's
/// activate reaches it under egui's modal layers, and a float opened there would come over the surface. The rest of
/// what is pressed is returned and gated by the caller. While `checking` (the check of an added account runs and the
/// Accounts page does not show) SETTINGS carries a `cyan_edge` square at its right end.
pub(crate) fn show(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, at: Place, narrow: bool, (live, checking): (bool, bool)) -> Asked {
    let p = &theme.palette;
    let rect = ui.max_rect();
    ui.painter().rect_filled(rect, 0.0, p.panel);
    let mut asked = Asked::default();
    let place_high = Theme::size(theme, nav_row::HEIGHT);
    let owner_high = Theme::size(theme, OWNER);
    // The room the mailboxes may take: what the places, the owner's row and the hairlines leave.
    let room = (rect.height() - TOP - place_high * 5.0 - owner_high - 3.0).max(0.0);
    let open: BTreeSet<u64> = ui.data(|data| data.get_temp::<BTreeSet<u64>>(open_id())).unwrap_or_default();
    let lines = lines(mailbox, &open);
    let whole: f32 = lines.iter().map(|line| line.height(theme)).sum();
    let active = shown(mailbox, at);
    let mail_count = mailbox.views().iter().find(|view| view.role == Role::Inbox).map_or(0, |view| view.unread);
    let later = Pile::ALL.iter().fold(0_u64, |sum, pile| sum.saturating_add(mailbox.pile_count(*pile)));
    let mut toggled = None;
    let column = Rect::from_min_max(pos2(rect.left(), rect.top() + TOP), rect.max);
    ui.scope_builder(UiBuilder::new().max_rect(column).id_salt("index-column"), |ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        if place(ui, theme, keymap, Place::Home, at, 0, narrow).clicked() {
            asked.place = Some(Place::Home);
        }
        if place(ui, theme, keymap, Place::Mail, at, mail_count, narrow).clicked() {
            asked.place = Some(Place::Mail);
        }
        if mailbox.accounts().is_empty() {
            no_account(ui, theme, keymap, narrow, &mut asked);
        } else {
            let height = whole.min(room);
            let scrolls = whole > room + 0.5;
            let top = ui.cursor().top();
            if scrolls {
                paint::hairline(ui.painter(), pos2(rect.left(), top), pos2(rect.right() - 1.0, top), p.panel_line);
                ui.add_space(1.0);
            }
            let area = ScrollArea::vertical().id_salt("index-mailboxes").max_height(height).min_scrolled_height(height).auto_shrink([false, true]);
            crate::component::scroll::show_viewport_over(ui, theme, area, |ui, viewport| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                ui.set_min_height(whole);
                let near = viewport.expand2(vec2(0.0, viewport.height().max(1.0)));
                let mut y = 0.0;
                let left = ui.max_rect().left();
                let wide = ui.available_width();
                for each in &lines {
                    let high = each.height(theme);
                    if y + high >= near.top() && y <= near.bottom() {
                        let at = Rect::from_min_size(pos2(left, ui.max_rect().top() + y), vec2(wide, high));
                        ui.scope_builder(UiBuilder::new().max_rect(at), |ui| {
                            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                            line(ui, theme, each, active, narrow, &mut asked, &mut toggled);
                        });
                    }
                    y += high;
                }
            });
            if scrolls {
                let bottom = ui.cursor().top();
                paint::hairline(ui.painter(), pos2(rect.left(), bottom), pos2(rect.right() - 1.0, bottom), p.panel_line);
                ui.add_space(1.0);
            }
        }
        for (which, count) in [(Place::Calendar, 0), (Place::Later, later), (Place::Settings, 0)] {
            let response = place(ui, theme, keymap, which, at, count, narrow);
            if which == Place::Settings && checking {
                check_mark(ui, theme, response.rect, narrow);
            }
            if response.clicked() {
                asked.place = Some(which);
            }
        }
    });
    asked.toggle = toggled;
    owner(ui, theme, mailbox, rect, (narrow, live), &mut asked);
    asked
}

/// The mailboxes' place without an account: "No account yet." and the link that opens the settings.
fn no_account(ui: &mut Ui, theme: &Theme, keymap: &Keymap, narrow: bool, asked: &mut Asked) {
    if narrow {
        return;
    }
    let f = &theme.fonts;
    egui::Frame::NONE.inner_margin(egui::Margin { left: 44, right: 16, top: 6, bottom: 12 }).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 6.0;
        crate::component::label(ui, f.caption, tr!("No account yet."), theme.palette.muted);
        if crate::component::button::link_with(ui, theme, keymap, tr!("add an account"), crate::component::button::LinkSize::Small, true, None)
            .clicked()
        {
            asked.place = Some(Place::Settings);
        }
    });
}

/// The owner's row at the foot of `rect`, over a hairline, and the account picker it opens.
fn owner(ui: &mut Ui, theme: &Theme, mailbox: &Mailbox, rect: Rect, (narrow, live): (bool, bool), asked: &mut Asked) {
    let p = &theme.palette;
    let f = &theme.fonts;
    let at = Rect::from_min_max(pos2(rect.left(), rect.bottom() - Theme::size(theme, OWNER)), rect.max);
    paint::hairline(ui.painter(), pos2(rect.left(), at.top() - 1.0), pos2(rect.right() - 1.0, at.top() - 1.0), p.panel_line);
    let first = mailbox.accounts().first();
    ui.scope_builder(UiBuilder::new().max_rect(at).id_salt("index-owner"), |ui| {
        let Some(account) = first else {
            if !narrow {
                let words = crate::component::fitted(ui, tr!("no account"), f.mono_s, p.muted, (at.width() - 32.0).max(0.0));
                paint::run_at(ui.painter(), pos2(at.left() + 16.0, at.top() + ((at.height() - f.mono_s.line) / 2.0).round()), words, p.muted);
            }
            return;
        };
        let address = clean(&account.address);
        let name = clean(&account.name);
        let drawn = base(ui, theme, Id::unique("index-owner-row"), OWNER, false, true, None);
        let spoken = tr!("Show mail of: {address}", address = address);
        drawn.response.widget_info(|| WidgetInfo::labeled(AxRole::Button, true, &spoken));
        let left = if narrow { at.center().x - AVATAR / 2.0 } else { at.left() + AVATAR_LEFT };
        let face = Rect::from_min_size(pos2(left, drawn.middle(AVATAR)), vec2(AVATAR, AVATAR));
        avatar::paint(ui.painter(), theme, face, &name, &address);
        if !narrow {
            let ink = if drawn.hovered { p.ink } else { p.muted };
            let words = crate::component::fitted(ui, &address, f.mono_s, ink, (at.right() - 16.0 - (at.left() + ADDRESS_LEFT)).max(0.0));
            paint::run_at(ui.painter(), pos2(at.left() + ADDRESS_LEFT, drawn.middle(f.mono_s.line)), words, ink);
        }
        drawn.finish(ui, theme);
        if live && let Some(choice) = picker(ui, theme, mailbox, &drawn.response) {
            asked.account = Some(choice);
        }
    });
}

/// The account picker over the owner's row, as its frame draws it: a SHOW MAIL OF head, All accounts, then each
/// account with its bar and address; the one shown chosen. Its foot stands on the owner's row (over the hairline above
/// it) and its right edge 1 past the index's, on the hairline beside it: 3 in from the index's left.
fn picker(ui: &Ui, theme: &Theme, mailbox: &Mailbox, opener: &Response) -> Option<Option<u64>> {
    let narrowed = mailbox.narrowed().filter(|id| mailbox.account(*id).is_some());
    let accounts: Vec<(u64, String, String, u64)> = mailbox
        .accounts()
        .iter()
        .map(|account| (account.id, clean(&account.label), clean(&account.address), u64::try_from(account.index).unwrap_or(0)))
        .collect();
    // `menu_up` stands a menu `MENU_DROP` over its opener, right edges together: the place it is given is the owner's
    // row moved that much down and 1 to the right.
    let mut on_row = opener.clone();
    on_row.rect = opener.rect.translate(vec2(1.0, MENU_DROP));
    // With one account there is nothing to narrow: its row is the one shown, and a note says where another is added.
    let only = accounts.len() == 1;
    popup::menu_up(ui.ctx(), theme, &on_row, Id::unique("index-account-picker"), PICKER, |ui| {
        popup::head(ui, theme, tr!("Show mail of"), None);
        let mut chosen = None;
        if !only && popup::choice(ui, theme, tr!("All accounts"), narrowed.is_none()).clicked() {
            chosen = Some(None);
        }
        for (id, name, address, index) in &accounts {
            if popup::account_choice(ui, theme, name, address, *index, only || narrowed == Some(*id)).clicked() {
                chosen = Some(Some(*id));
            }
        }
        if only {
            popup::note(ui, theme, tr!("The only account. Another is added in Settings."));
        }
        (chosen, chosen.is_some())
    })
    .flatten()
}

#[cfg(test)]
#[path = "index_tests.rs"]
mod tests;
