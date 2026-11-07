// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The navigation rail and the folder pane. The rail runs the preset's gradient at full strength down the window's
//! left edge, with the app's mark and the views at its top and the appearance settings and the owner at its foot; the folder pane holds New message,
//! the account picker, the unified views and each account's folders with their unread badges.

use egui::{Align, Color32, ComboBox, Layout, Rect, RichText, Role, ScrollArea, Sense, Stroke, Ui, WidgetInfo, pos2, vec2};

use super::Screen;
use super::paint::{NavRow, line_height, nav_row};
use crate::component::avatar;
use crate::component::button::IconButton;
use crate::gradient::{self, Gradient, Stop};
use crate::iconset::{IconId, Style};
use crate::keymap::{self, Keymap};
use inboxcraft_model::Role as FolderRole;

use crate::mailbox::{Mailbox, Place, role_text};
use crate::theme::{Font, Theme, icon_ink_over, rail_avatar};
use crate::tr;

/// The ink of the rail's icons: white where it reaches 3:1 on every stop of the gradient, otherwise the deep ink.
pub(super) fn rail_ink(theme: &Theme) -> Color32 {
    icon_ink_over(&theme.palette.gradient)
}

/// The side of the rail's buttons and its mark, in points: the rail keeps its size at every text scale, so the
/// pages beside it keep their room.
const RAIL_TILE: f32 = 40.0;

/// The rail's width: a tile and room on either side.
pub(super) fn rail_width(theme: &Theme) -> f32 {
    RAIL_TILE + theme.spacing.m * 2.0
}

/// What the person asked for on the rail this frame.
#[derive(Debug, Default)]
pub(super) struct Rail {
    pub(super) mail: bool,
    pub(super) sent: bool,
    pub(super) later: bool,
    pub(super) search: bool,
    pub(super) calendar: bool,
    pub(super) settings: bool,
}

impl Rail {
    /// The actions of the buttons pressed, besides the settings, which the rail's button opens and closes.
    pub(super) fn actions(&self) -> impl Iterator<Item = &'static str> {
        [
            (self.mail, keymap::MAIL),
            (self.sent, keymap::SENT),
            (self.later, keymap::LATER),
            (self.search, keymap::FIND),
            (self.calendar, keymap::CALENDAR),
        ]
        .into_iter()
        .filter_map(|(pressed, action)| pressed.then_some(action))
    }
}

/// The app's mark at the rail's top: the mail icon on a tile of the ink, faint.
fn mark(ui: &mut Ui, theme: &Theme, ink: Color32) {
    let side = RAIL_TILE;
    let (rect, response) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Image, true, "InboxCraft"));
    ui.painter().rect_filled(rect, side * 0.25, ink.gamma_multiply(MARK_TILE));
    let icon = side * 0.5;
    theme.icons.paint(ui.painter(), Rect::from_center_size(rect.center(), vec2(icon, icon)), IconId::Mail, Style::Fill, ink);
}

/// How strong the ink is in the tile under the app's mark.
const MARK_TILE: f32 = 0.22;

/// The owner's avatar at the rail's foot: the first account's initials on a white disc, in the middle stop.
fn owner(ui: &mut Ui, theme: &Theme, mailbox: &Mailbox) {
    let Some(account) = mailbox.accounts().first() else { return };
    let side = RAIL_TILE * 0.9;
    let (rect, response) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
    let spoken = tr!("Signed in as {address}", address = account.address);
    response.widget_info(|| WidgetInfo::labeled(Role::Image, true, &spoken));
    let (disc, ink) = rail_avatar(theme.palette.gradient);
    ui.painter().circle_filled(rect.center(), side / 2.0, disc);
    let font = Font { size: side * 0.36, ..theme.fonts.strong };
    let galley = ui.painter().layout_no_wrap(avatar::initials("", &account.address), font.id(), ink);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, ink);
}

/// Where the rail sits in the window: the window's corner radius, which its left corners follow, and how far down the
/// cards start, where its mark starts too.
#[derive(Debug, Clone, Copy)]
pub(super) struct RailPlace {
    pub(super) corner: f32,
    pub(super) top: f32,
}

/// Draws the rail over the whole of `ui`: the app's mark, the views (the `screen` shown active unless `settings` or
/// `adding` show another page; the views not built yet disabled, saying so), and at the foot the settings and the
/// owner. Its left corners follow the window's `corner`.
#[expect(clippy::too_many_arguments, reason = "the rail reads the window's state, which is not one value")]
pub(super) fn rail(
    ui: &mut Ui,
    theme: &Theme,
    keymap: &Keymap,
    mailbox: &Mailbox,
    screen: Screen,
    settings: bool,
    adding: bool,
    place: RailPlace,
) -> Rail {
    let corner = place.corner;
    let s = &theme.spacing;
    let rect = ui.max_rect();
    let [first, middle, last] = theme.palette.gradient;
    let gradient = Gradient::Linear {
        from: pos2(0.5, 0.0),
        to: pos2(0.5, 1.0),
        stops: vec![Stop::new(0.0, first), Stop::new(0.5, middle), Stop::new(1.0, last)],
    };
    gradient::fill(ui.painter(), rect, super::frame::left_corners(corner), &gradient);
    let ink = rail_ink(theme);
    let not_yet = tr!("Not available yet");
    let tile = RAIL_TILE;
    let shown = !settings && !adding;
    let mail_shown = shown && screen == Screen::Mail;
    let in_sent = mail_shown && mailbox.place() == Place::View(FolderRole::Sent);
    let in_later = mail_shown && matches!(mailbox.place(), Place::Later(_));
    let mut asked = Rail::default();
    ui.vertical_centered(|ui| {
        ui.spacing_mut().item_spacing.y = s.s;
        // The mark's top lines up with the cards'.
        ui.add_space((place.top - (ui.cursor().top() - rect.top())).max(0.0));
        mark(ui, theme, ink);
        ui.add_space(s.m);
        IconButton::new(IconId::Home, tr!("Start")).unavailable(not_yet).ink(ink).side(tile).show(ui, theme, keymap);
        asked.mail = IconButton::new(IconId::Inbox, tr!("Mail"))
            .action(keymap::MAIL)
            .active(mail_shown && !in_sent && !in_later)
            .ink(ink)
            .side(tile)
            .show(ui, theme, keymap)
            .clicked();
        asked.calendar = IconButton::new(IconId::Calendar, tr!("Calendar"))
            .action(keymap::CALENDAR)
            .active(shown && screen == Screen::Calendar)
            .ink(ink)
            .side(tile)
            .show(ui, theme, keymap)
            .clicked();
        asked.later =
            IconButton::new(IconId::Pin, tr!("Later")).action(keymap::LATER).active(in_later).ink(ink).side(tile).show(ui, theme, keymap).clicked();
        asked.sent = IconButton::new(IconId::Send, tr!("Sent mail"))
            .action(keymap::SENT)
            .active(in_sent)
            .ink(ink)
            .side(tile)
            .show(ui, theme, keymap)
            .clicked();
        asked.search = IconButton::new(IconId::Search, tr!("Search mail")).action(keymap::FIND).ink(ink).side(tile).show(ui, theme, keymap).clicked();
    });
    ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
        ui.spacing_mut().item_spacing.y = s.s;
        ui.add_space(s.m);
        owner(ui, theme, mailbox);
        asked.settings = IconButton::new(IconId::Settings, tr!("Appearance settings"))
            .action(keymap::SETTINGS)
            .active(settings)
            .ink(ink)
            .side(tile)
            .show(ui, theme, keymap)
            .clicked();
    });
    asked
}

/// What the person asked for in the pane this frame.
#[derive(Debug, Default)]
pub(super) struct Asked {
    pub(super) narrow: Option<Option<u64>>,
    pub(super) open: Option<Place>,
    /// New message.
    pub(super) compose: bool,
}

fn spoken(text: &str, account: Option<&str>, unread: u64) -> String {
    let named = match account {
        Some(account) => tr!("{folder} in {account}", folder = text, account = account),
        None => text.to_owned(),
    };
    if unread == 0 { named } else { tr!("{name}, {count} unread", name = named, count = unread) }
}

/// The account picker: the account shown (or All accounts) in an outlined field with a chevron.
fn account_picker(ui: &mut Ui, theme: &Theme, mailbox: &Mailbox, asked: &mut Asked) {
    let current = match mailbox.narrowed().and_then(|id| mailbox.account(id)) {
        Some(account) => account.label.clone(),
        None => tr!("All accounts").to_owned(),
    };
    let width = ui.available_width();
    let ink = theme.palette.muted;
    // A field, not a button: the field colour at rest.
    let visuals = &mut ui.visuals_mut().widgets;
    visuals.inactive.weak_bg_fill = theme.palette.field;
    visuals.inactive.bg_fill = theme.palette.field;
    let response = ComboBox::from_id_salt("account-picker")
        .selected_text(RichText::new(current).font(theme.fonts.body.id()).color(theme.palette.text))
        .width(width)
        .icon(move |ui, rect, _, _| {
            // A chevron pointing down, drawn in lines like the icons.
            let c = rect.center();
            let w = rect.width().min(rect.height()) * 0.3;
            let stroke = Stroke::new(1.5, ink);
            ui.painter().line_segment([pos2(c.x - w, c.y - w / 2.0), pos2(c.x, c.y + w / 2.0)], stroke);
            ui.painter().line_segment([pos2(c.x, c.y + w / 2.0), pos2(c.x + w, c.y - w / 2.0)], stroke);
        })
        .show_ui(ui, |ui| {
            if ui.selectable_label(mailbox.narrowed().is_none(), tr!("All accounts")).clicked() {
                asked.narrow = Some(None);
            }
            for account in mailbox.accounts() {
                let text = tr!("{label} ({address})", label = account.label, address = account.address);
                if ui.selectable_label(mailbox.narrowed() == Some(account.id), text).clicked() {
                    asked.narrow = Some(Some(account.id));
                }
            }
        })
        .response;
    response.widget_info(|| WidgetInfo::labeled(Role::ComboBox, true, tr!("Accounts")));
}

/// A section's heading: small capitals in the muted colour.
fn heading(ui: &mut Ui, theme: &Theme, text: &str) {
    ui.add_space(theme.spacing.m);
    ui.label(RichText::new(text.to_uppercase()).font(theme.fonts.caption.id()).color(theme.palette.muted).extra_letter_spacing(0.6));
    ui.add_space(theme.spacing.xs);
}

/// The icon of a folder of `role` (a plain folder without one).
fn folder_icon(role: Option<FolderRole>) -> IconId {
    match role {
        Some(FolderRole::Inbox) => IconId::Inbox,
        Some(FolderRole::Sent) => IconId::Send,
        Some(FolderRole::Drafts) => IconId::File,
        Some(FolderRole::Trash) => IconId::Delete,
        Some(FolderRole::Junk) => IconId::Junk,
        Some(FolderRole::Archive) => IconId::Archive,
        _ => IconId::Move,
    }
}

/// Draws the pane and returns what was asked for; the caller applies it after the frame's drawing. Accounts are added
/// in the settings, not here: most people have one, and a link here would hold its room for nothing.
pub(super) fn show(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox) -> Asked {
    let mut asked = Asked::default();
    let p = &theme.palette;
    ui.add_space(theme.spacing.s);
    ui.horizontal(|ui| {
        ui.label(RichText::new("InboxCraft").font(theme.fonts.title.id()).color(p.text));
        if !mailbox.accounts().is_empty() {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                asked.compose = IconButton::new(IconId::Compose, tr!("New message")).action(keymap::COMPOSE).show(ui, theme, keymap).clicked();
            });
        }
    });
    if !mailbox.accounts().is_empty() {
        ui.add_space(theme.spacing.s);
        ui.scope(|ui| account_picker(ui, theme, mailbox, &mut asked));
    }
    if mailbox.accounts().is_empty() {
        return asked;
    }
    let searching = !mailbox.query().is_empty();
    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 2.0);
        heading(ui, theme, tr!("Mailboxes"));
        for view in mailbox.views() {
            let text = role_text(view.role);
            let row = NavRow {
                text,
                spoken: spoken(text, None, view.unread),
                badge: view.unread,
                selected: !searching && mailbox.place() == Place::View(view.role),
                depth: 0,
                icon: Some(folder_icon(Some(view.role))),
                enabled: true,
            };
            if nav_row(ui, theme, &row).clicked() {
                asked.open = Some(Place::View(view.role));
            }
        }
        for account in mailbox.accounts().iter().filter(|account| mailbox.narrowed().is_none_or(|id| id == account.id)) {
            ui.add_space(theme.spacing.m);
            let colour = theme.account_colour(u64::try_from(account.index).unwrap_or(0));
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = theme.spacing.xs;
                let (dot, _) = ui.allocate_exact_size(vec2(10.0, line_height(ui, theme.fonts.strong)), Sense::hover());
                ui.painter().circle_filled(dot.center(), 4.0, colour);
                ui.label(RichText::new(&account.label).font(theme.fonts.strong.id()).color(p.text));
            });
            ui.horizontal(|ui| {
                ui.add_space(10.0 + theme.spacing.xs);
                ui.label(RichText::new(&account.address).font(theme.fonts.caption.id()).color(p.muted));
            });
            ui.add_space(theme.spacing.xs);
            // The unified views above hold every account's inbox, sent mail and the like: an account lists only the
            // folders of its own.
            for folder in mailbox.folders_of(account.id).iter().filter(|folder| folder.role.is_none()) {
                let text = match folder.role {
                    Some(role) => role_text(role),
                    None => folder.leaf.as_str(),
                };
                let row = NavRow {
                    text,
                    spoken: spoken(text, Some(&account.label), folder.unread),
                    badge: folder.unread,
                    selected: !searching && mailbox.place() == Place::Folder(folder.id),
                    depth: folder.depth,
                    icon: Some(folder_icon(folder.role)),
                    enabled: true,
                };
                if nav_row(ui, theme, &row).clicked() {
                    asked.open = Some(Place::Folder(folder.id));
                }
            }
        }
    });
    asked
}
