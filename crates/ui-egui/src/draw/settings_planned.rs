// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The settings pages that are designed and not built whole yet: Calendar (the calendar alerts switch and the hour
//! all-day alerts ring at work; each
//! account's calendars with "read again" are coming soon; the week's first day and first week and where a new event
//! opens are the owner's), and the three sections that
//! are planned whole, with the coming-soon mark on their head: About (the program, its licences, its makers and
//! contributors), AI (where a model runs, and the consent to send mail to one elsewhere) and Plugins. Nothing coming
//! soon on them can be pressed or focused.

use egui::{Align, Layout, Rect, Sense, Ui, UiBuilder, pos2, vec2};

use super::{Asked, body, group, head};
use crate::calendar::all_day_alert_text;
use crate::component::button::{LinkSize, link_sized};
use crate::component::segmented::{segmented, segmented_for};
use crate::component::select::Select;
use crate::component::settings_row::settings_row;
use crate::component::soon::soon;
use crate::component::switch::{SwitchState, switch_in};
use crate::iconset::IconId;
use crate::keymap::Keymap;
use crate::mailbox::{AccountRow, Mailbox};
use crate::settings::Section;
use crate::theme::Theme;
use crate::tr;
use inboxcraft_engine::all_day_alert::AllDayAlert;
use inboxcraft_engine::appearance::EventEditorHost;
use inboxcraft_time::{FirstWeek, Weekday};

/// The Calendar page: alerts, each account's calendars, the week. The alerts switch changes `notify.set`'s `alerts` on
/// the owner's press only.
pub(super) fn calendar(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox) -> Option<Asked> {
    let mut asked = None;
    head(ui, theme, Section::Calendar, &[], |_| {});
    body(ui, theme, "calendar", |ui| {
        group(ui, theme, "A", tr!("Alerts"), None);
        let on = mailbox.notify_settings().alerts;
        let help = if on {
            tr!("An event's alert shows as a desktop notification at its time.")
        } else {
            tr!("Off: no event notifies. The alerts stay on their events and come back with the switch.")
        };
        settings_row(ui, theme, tr!("Calendar alerts"), Some(help), |ui| {
            let pressed = crate::component::switch::switch(ui, theme, tr!("Calendar alerts"), on);
            if crate::component::button::owner_pressed(ui, &pressed) {
                asked = Some(Asked::Alerts(!on));
            }
        });
        if let Some(choice) = all_day_alert(ui, theme, mailbox.all_day_alert()) {
            asked = Some(Asked::AllDayAlert(choice));
        }
        let note = tr!("A calendar comes from its account's CalDAV address. Which of them the calendar shows is chosen in its side panel.");
        group(ui, theme, "B", tr!("Calendars"), Some(note));
        let count = mailbox.accounts().len();
        for (index, account) in mailbox.accounts().iter().enumerate() {
            let ends = index == 0 || index.saturating_add(1) == count;
            super::culled(ui, theme, egui::Id::unique("settings-calendars").with(account.id), ends, |ui| calendars(ui, theme, keymap, account));
        }
        group(ui, theme, "C", tr!("Week"), None);
        let rule = mailbox.week_rule();
        let note = tr!("The calendar's weeks and months start on this day; until you choose one, your system's region suggests it.");
        settings_row(ui, theme, tr!("Week starts on"), Some(note), |ui| {
            if let Some(day) = first_day(ui, theme, rule.first_day) {
                asked = Some(Asked::FirstDay(day));
            }
        });
        let firsts = [
            (FirstWeek::FirstFourDayWeek, tr!("Four-day week"), theme.fonts.button),
            (FirstWeek::WeekOf1January, tr!("Week of 1 January"), theme.fonts.button),
        ];
        let note = match rule.first_week {
            FirstWeek::FirstFourDayWeek => tr!("Week 1 is the first with four days in the new year (ISO 8601)."),
            FirstWeek::WeekOf1January => tr!("Week 1 is the week that holds 1 January."),
        };
        settings_row(ui, theme, tr!("First week of the year"), Some(note), |ui| {
            if let Some(first) = segmented(ui, theme, &firsts, rule.first_week) {
                asked = Some(Asked::FirstWeek(first));
            }
        });
        group(ui, theme, "D", tr!("Editor"), None);
        if let Some(host) = event_editor(ui, theme, mailbox.appearance().layout.event_editor) {
            asked = Some(Asked::EventEditor(host));
        }
    });
    asked
}

/// "New event opens": the small editor, the full one or a window of its own, its help saying where the one chosen
/// opens; another host on the owner's press, never on a held key's repeat (Edit follows it too).
fn event_editor(ui: &mut Ui, theme: &Theme, now: EventEditorHost) -> Option<EventEditorHost> {
    let font = theme.fonts.button;
    let hosts =
        [(EventEditorHost::Small, tr!("Small"), font), (EventEditorHost::Full, tr!("Full"), font), (EventEditorHost::Window, tr!("Window"), font)];
    let help = match now {
        EventEditorHost::Small => tr!("Small: a panel over the calendar, for a title and a time. Edit opens the same way."),
        EventEditorHost::Full => tr!("Full: in the calendar's place, with the event's day beside it. Edit opens the same way."),
        EventEditorHost::Window => tr!("Window: in a window of its own. Edit opens the same way."),
    };
    let mut chosen = None;
    settings_row(ui, theme, tr!("New event opens"), Some(help), |ui| {
        let pressed = segmented_for(ui, theme, &hosts, now, Some(tr!("New event opens")));
        chosen = pressed.filter(|host| *host != now && !crate::component::button::key_repeated(ui));
    });
    chosen
}

/// The least width of the all-day alert's choice, in points.
const ALL_DAY_ALERT: f32 = 220.0;

/// The room a select's field takes around its words: 12 before them, 8 to the caret, the caret's 8 and 12 after it,
/// in points.
const SELECT_ROOM: f32 = 40.0;

/// The width of the all-day alert's choice: [`ALL_DAY_ALERT`], or more where the widest choice needs it to read whole
/// (large text), on whole points.
fn all_day_alert_width(ui: &Ui, theme: &Theme) -> f32 {
    let widest = AllDayAlert::ALL
        .iter()
        .map(|choice| crate::component::galley(ui, all_day_alert_text(*choice), theme.fonts.mono, theme.palette.ink).size().x)
        .fold(0.0_f32, f32::max);
    (widest.ceil() + SELECT_ROOM).max(ALL_DAY_ALERT)
}

/// "All-day alerts ring at": a select of the hours a new all-day event's alert rings at; another one on the owner's
/// press, never on a held key's repeat.
fn all_day_alert(ui: &mut Ui, theme: &Theme, now: AllDayAlert) -> Option<AllDayAlert> {
    let options = AllDayAlert::ALL.map(|choice| (choice, all_day_alert_text(choice)));
    let id = ui.make_persistent_id("settings-all-day-alert-open");
    let mut open = ui.data(|data| data.get_temp::<bool>(id)).unwrap_or(false);
    let help = tr!("A new all-day event's alert, and the alert of one that brings none. Events already saved keep theirs.");
    let mut chosen = None;
    settings_row(ui, theme, tr!("All-day alerts ring at"), Some(help), |ui| {
        let select = Select {
            name: tr!("All-day alerts ring at"),
            shown: all_day_alert_text(now),
            options: &options,
            current: Some(now),
            icon: IconId::Notify,
            width: all_day_alert_width(ui, theme),
        };
        let (_, picked) = select.show(ui, theme, &mut open);
        chosen = picked.filter(|choice| *choice != now && !crate::component::button::key_repeated(ui));
    });
    ui.data_mut(|data| data.insert_temp(id, open));
    chosen
}

/// The width of the first day's choice, in points.
const FIRST_DAY: f32 = 160.0;

/// The choice of the week's first day: a select of the seven days; the day picked, on the owner's press of it.
fn first_day(ui: &mut Ui, theme: &Theme, now: Weekday) -> Option<Weekday> {
    let options = Weekday::ALL.map(|day| (day, crate::mailbox::weekday_text(day)));
    let id = ui.make_persistent_id("settings-first-day-open");
    let mut open = ui.data(|data| data.get_temp::<bool>(id)).unwrap_or(false);
    let select = Select {
        name: tr!("Week starts on"),
        shown: crate::mailbox::weekday_text(now),
        options: &options,
        current: Some(now),
        icon: IconId::Calendar,
        width: FIRST_DAY,
    };
    let (_, chosen) = select.show(ui, theme, &mut open);
    ui.data_mut(|data| data.insert_temp(id, open));
    chosen.filter(|day| *day != now)
}

/// The height of an account's row of calendars, in points (at 100 %).
const CALENDARS_ROW: f32 = 56.0;

/// The room the coming-soon tag takes, in points (at 100 %).
const TAG: f32 = 85.0;

/// One account's calendars, coming soon, as the design's row: the account's bar (3 × 12, 16 in), its name in `strong`
/// with the coming-soon tag, a `mono_s` line of where its calendars come from under it, and "read again", a small link
/// 16 from the end; a hairline under it.
fn calendars(ui: &mut Ui, theme: &Theme, keymap: &Keymap, account: &AccountRow) {
    let p = &theme.palette;
    let f = &theme.fonts;
    let height = Theme::size(theme, CALENDARS_ROW).max(f.strong.line + 2.0 + f.mono_s.line + 16.0).ceil();
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    soon(ui, theme, |ui| {
        let link = ui
            .scope_builder(
                UiBuilder::new()
                    .max_rect(Rect::from_min_max(rect.min, pos2(rect.right() - 16.0, rect.bottom())))
                    .layout(Layout::right_to_left(Align::Center)),
                |ui| link_sized(ui, theme, keymap, tr!("read again"), LinkSize::Small),
            )
            .inner;
        let bar = Rect::from_min_size(pos2(rect.left() + 16.0, rect.top() + ((rect.height() - 12.0) / 2.0).round()), vec2(3.0, 12.0));
        ui.painter().rect_filled(bar, 0.0, theme.account_colour(u64::try_from(account.index).unwrap_or(0)));
        let words = bar.right() + 12.0;
        let room = (link.rect.left() - 12.0 - words).max(1.0);
        let lines = f.strong.line + 2.0 + f.mono_s.line;
        let top = rect.top() + ((rect.height() - lines) / 2.0).round();
        let column = Rect::from_min_size(pos2(words, top), vec2(room, lines));
        ui.scope_builder(UiBuilder::new().max_rect(column).layout(Layout::top_down(Align::Min)), |ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 2.0);
            ui.horizontal(|ui| {
                ui.set_height(f.strong.line);
                // The name takes what the tag after it leaves of the column, cut there.
                let tag = Theme::size(theme, TAG) + 8.0;
                let name = crate::component::cut(ui, &account.label, f.strong, p.muted, (room - tag).max(1.0));
                let (at, _) = ui.allocate_exact_size(vec2(name.size().x, f.strong.line), Sense::hover());
                ui.painter().galley(pos2(at.left(), (at.center().y - name.size().y / 2.0).round()), name, p.muted);
                crate::component::soon::soon_tag(ui, theme);
            });
            let line = tr!("Calendars from the CalDAV address of {address}", address = account.address);
            let line = crate::component::cut(ui, &line, f.mono_s, p.muted, room);
            let (at, _) = ui.allocate_exact_size(line.size(), Sense::hover());
            ui.painter().galley(at.min, line, p.muted);
        });
    });
    let y = rect.bottom() - 1.0;
    super::paint::hairline(ui.painter(), pos2(rect.left(), y), pos2(rect.right() - 1.0, y), p.panel_line);
}

/// The About page, planned whole: the program, Qubernetic, the contributors.
pub(super) fn about(ui: &mut Ui, theme: &Theme) -> Option<Asked> {
    head(ui, theme, Section::About, &[], |_| {});
    body(ui, theme, "about", |ui| {
        soon(ui, theme, |ui| {
            group(ui, theme, "A", tr!("This program"), None);
            let version = tr!(
                "{version}. A mail and calendar program for IMAP, SMTP and CalDAV that keeps your mail on this computer.",
                version = env!("CARGO_PKG_VERSION")
            );
            settings_row(ui, theme, tr!("Version"), Some(&version), |_| {});
            settings_row(
                ui,
                theme,
                tr!("Licence"),
                Some(tr!("MIT or Apache-2.0, at your choice. The parts it is built from keep their own licences.")),
                |_| {},
            );
            settings_row(
                ui,
                theme,
                tr!("Type, icons"),
                Some(tr!("IBM Plex Mono, IBM Plex Sans, Audiowide · Phosphor. The typefaces under the SIL Open Font Licence, the icons under MIT.")),
                |_| {},
            );
            group(ui, theme, "B", tr!("Qubernetic"), None);
            settings_row(ui, theme, tr!("Made by"), Some(tr!("Qubernetic and the InboxCraft contributors.")), |_| {});
            group(ui, theme, "C", tr!("Contributors"), None);
            settings_row(ui, theme, tr!("With"), Some(tr!("Everyone who asked to be named, in the order they joined.")), |_| {});
        });
    });
    None
}

/// The AI page, planned whole: where a model runs, and the consent to send mail to one elsewhere.
pub(super) fn ai(ui: &mut Ui, theme: &Theme) -> Option<Asked> {
    let intro = [tr!(
        "InboxCraft has no AI of its own. A model you bring can summarise a message and draft a reply for you to change and send. It never sends, moves or deletes anything, and what it writes is marked."
    )];
    head(ui, theme, Section::Ai, &intro, |_| {});
    body(ui, theme, "ai", |ui| {
        soon(ui, theme, |ui| {
            group(ui, theme, "A", tr!("Model"), None);
            let f = theme.fonts.button;
            let places = [(0_u8, tr!("None"), f), (1, tr!("On this computer"), f), (2, tr!("Elsewhere"), f)];
            settings_row(ui, theme, tr!("Where the model runs"), Some(tr!("No model: Summarise and Draft a reply are not offered.")), |ui| {
                segmented_for(ui, theme, &places, 0, Some(tr!("Where the model runs")));
            });
            let consent = tr!("A model elsewhere sees the messages you ask it about. It is asked once, before anything leaves this computer.");
            settings_row(ui, theme, tr!("Send mail to a model elsewhere"), Some(consent), |ui| {
                switch_in(ui, theme, tr!("Send mail to a model elsewhere"), SwitchState::Unavailable);
            });
        });
    });
    None
}

/// The Plugins page, planned whole.
pub(super) fn plugins(ui: &mut Ui, theme: &Theme) -> Option<Asked> {
    let intro = [tr!(
        "A plugin is code from someone else that adds to InboxCraft through the app's own commands. It asks before it is added and does only what you allowed."
    )];
    head(ui, theme, Section::Plugins, &intro, |_| {});
    body(ui, theme, "plugins", |ui| {
        soon(ui, theme, |ui| {
            group(ui, theme, "A", tr!("Plugins"), None);
            settings_row(
                ui,
                theme,
                tr!("Installed plugins"),
                Some(tr!("None. Here you will see each plugin, what it may do, and take it back.")),
                |_| {},
            );
        });
    });
    None
}

#[cfg(test)]
#[path = "settings_calendar_alerts_tests.rs"]
mod alerts_tests;
