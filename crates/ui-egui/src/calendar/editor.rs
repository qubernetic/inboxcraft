// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The event editor's form: a new event (`event.create`) or an event picked on the screen (`event.update`), with its
//! title, its start and end typed as `2026-11-06 13:00` (a day alone, `2026-11-06`, for an all-day event), all-day,
//! private, a repeat preset, busy or free, the alert, and the guests' addresses. An edit sends only what changed. Editing an
//! event that has guests asks whether to tell them of the change; the default is not to. A new event asks nothing:
//! inviting the guests is the invitations' work.
//!
//! The times are the computer's wall clock at its offset, sent to the engine in UTC. Changing the times of an
//! occurrence of a repeating event moves the whole series to them.

use inboxcraft_time::{CivilDateTime, Offset};
use serde_json::{Map, Value, json};

use super::{Day, EventRow, MAX_DESCRIPTION, Moment};
use crate::tr;

/// The repeat presets the editor offers, as `event.repeats` names them.
pub const REPEATS: [&str; 6] = ["none", "daily", "weekdays", "weekly", "monthly", "yearly"];

/// An event's alert, as `event.list` says it and the Alert field picks it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Alert {
    /// No alarm of its own: the default lead rings, ten minutes before a timed event and a day before an all-day one.
    #[default]
    Default,
    /// No alert.
    None,
    /// This many minutes before the start.
    Before(u32),
    /// An alarm the field cannot name (after the start, from the end, at a moment), kept as it is.
    Custom,
}

impl Alert {
    /// The alert `event.list` gives: `null` or nothing, `"none"`, the minutes, or anything else as custom.
    #[must_use]
    pub fn read(value: Option<&Value>) -> Self {
        match value {
            None | Some(Value::Null) => Self::Default,
            Some(Value::String(word)) if word == "none" => Self::None,
            Some(value) => value.as_u64().and_then(|minutes| u32::try_from(minutes).ok()).map_or(Self::Custom, Self::Before),
        }
    }

    /// What `event.create` and `event.update` take for it; `None` for the default and a custom alarm, which are never
    /// sent.
    fn param(self) -> Option<Value> {
        match self {
            Self::None => Some(json!("none")),
            Self::Before(minutes) => Some(json!(minutes)),
            Self::Default | Self::Custom => None,
        }
    }
}

/// The alerts the Alert field offers.
pub const ALERTS: [Alert; 11] = [
    Alert::None,
    Alert::Before(0),
    Alert::Before(5),
    Alert::Before(10),
    Alert::Before(15),
    Alert::Before(30),
    Alert::Before(60),
    Alert::Before(120),
    Alert::Before(1440),
    Alert::Before(2880),
    Alert::Before(10_080),
];

/// An alert in words, in its field: `None`, `10 min before`, `1 day before`; the default as the lead it rings with on
/// a timed or an `all_day` event.
#[must_use]
pub fn alert_text(alert: Alert, all_day: bool) -> String {
    const HOUR: u32 = 60;
    const DAY: u32 = 24 * HOUR;
    const WEEK: u32 = 7 * DAY;
    match alert {
        Alert::Default => alert_text(Alert::Before(if all_day { DAY } else { 10 }), all_day),
        Alert::None => tr!("None").to_owned(),
        Alert::Custom => tr!("Custom").to_owned(),
        Alert::Before(0) => tr!("At the start").to_owned(),
        Alert::Before(WEEK) => tr!("1 week before").to_owned(),
        Alert::Before(minutes) if minutes % WEEK == 0 => tr!("{count} weeks before", count = minutes / WEEK),
        Alert::Before(DAY) => tr!("1 day before").to_owned(),
        Alert::Before(minutes) if minutes % DAY == 0 => tr!("{count} days before", count = minutes / DAY),
        Alert::Before(HOUR) => tr!("1 hour before").to_owned(),
        Alert::Before(minutes) if minutes % HOUR == 0 => tr!("{count} hours before", count = minutes / HOUR),
        Alert::Before(minutes) => tr!("{count} min before", count = minutes),
    }
}

/// The longest text a field of the editor takes, in characters.
pub const MAX_FIELD: usize = 1000;

/// The longest guest list typed, in characters.
pub const MAX_GUESTS_TEXT: usize = 8000;

/// What the editor says when Esc or a press outside would lose what was typed: every way on from there.
fn warning() -> &'static str {
    tr!("What you typed is not saved. Save keeps it; Cancel, or Esc again, throws it away.")
}

/// A repeat preset in words, on its pill: `Never`, `Weekly`; `Custom` for a repeat the presets do not name.
#[must_use]
pub fn repeat_text(id: &str) -> &'static str {
    match id {
        "none" => tr!("Never"),
        "daily" => tr!("Daily"),
        "weekdays" => tr!("Weekdays"),
        "weekly" => tr!("Weekly"),
        "monthly" => tr!("Monthly"),
        "yearly" => tr!("Yearly"),
        _ => tr!("Custom"),
    }
}

/// What the form holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Form {
    /// The calendar a new event goes into; an edited event stays in its own.
    pub calendar: u64,
    /// The title.
    pub summary: String,
    /// The start: `2026-11-06 13:00`, or the first day of an all-day event.
    pub start: String,
    /// The end: `2026-11-06 15:00`, or the last day of an all-day event.
    pub end: String,
    /// A whole day or days, without times.
    pub all_day: bool,
    /// Private.
    pub private: bool,
    /// Busy (or free) time.
    pub busy: bool,
    /// The repeat preset ([`REPEATS`]), or `custom` for a repeat the presets do not name, kept as it is.
    pub repeat: String,
    /// The alert; the full editor and the window show it, the small editor does not.
    pub alert: Alert,
    /// The guests' addresses, separated by commas.
    pub guests: String,
    /// What the event says about itself, its lines kept.
    pub description: String,
}

/// The event editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Editor {
    /// The event edited; `None` for a new one.
    id: Option<u64>,
    /// The form as it was opened.
    was: Form,
    form: Form,
    /// The calendars a new event can go into: id and name.
    choices: Vec<(u64, String)>,
    /// Whether to tell the guests of the change.
    notify: bool,
    /// Why the last save was refused.
    problem: Option<String>,
    /// The link a press in the description copied last, until the status line says so.
    copied: Option<String>,
    /// Someone else's event, opened read-only: who organises it (empty when the event does not say).
    theirs: Option<String>,
    /// Whether the description read was cut to [`MAX_DESCRIPTION`]: it is then shown, never sent back cut.
    description_cut: bool,
    /// The list of choices open in the form, when one is.
    menu: Option<Menu>,
    /// The form and the guests' answer as they were when the editor said the typing is not saved, and whether Esc made
    /// it say so: an Esc after Esc's warning, with nothing changed since, throws the typing away; any change asks again.
    warned: Option<(Form, bool, bool)>,
}

/// A list of choices of the form that opens over it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Menu {
    /// Repeat's choices.
    Repeat,
    /// The calendars, where they do not fit side by side.
    Calendar,
    /// The alerts, in the full editor and the window.
    Alert,
}

impl Editor {
    /// A new event on `day` from 09:00 to 10:00, in `calendar`, which can be any of `choices`.
    #[must_use]
    pub fn new_event(calendar: u64, day: Day, choices: Vec<(u64, String)>) -> Self {
        let at = |minute: u16| typed(Moment { day, minute });
        let form = Form {
            calendar,
            summary: String::new(),
            start: at(9 * 60),
            end: at(10 * 60),
            all_day: false,
            private: false,
            busy: true,
            repeat: REPEATS[0].to_owned(),
            alert: Alert::Default,
            guests: String::new(),
            description: String::new(),
        };
        Self {
            id: None,
            was: form.clone(),
            form,
            choices,
            notify: false,
            problem: None,
            copied: None,
            theirs: None,
            description_cut: false,
            menu: None,
            warned: None,
        }
    }

    /// The occurrence `event` to edit: its times as the screen shows them.
    #[must_use]
    pub fn edit(event: &EventRow) -> Self {
        let (start, end) = if event.all_day { (iso_day(event.first), iso_day(event.last)) } else { (typed(event.start), typed(event.end)) };
        let addresses: Vec<&str> = event.guests.iter().map(|guest| guest.address.as_str()).filter(|address| !address.is_empty()).collect();
        let form = Form {
            calendar: event.calendar,
            summary: if event.untitled { String::new() } else { event.summary.clone() },
            start,
            end,
            all_day: event.all_day,
            private: event.private,
            busy: event.busy,
            repeat: event.repeat.clone().unwrap_or_else(|| REPEATS[0].to_owned()),
            alert: event.alert,
            guests: addresses.join(", "),
            description: event.description.clone(),
        };
        Self {
            id: Some(event.id),
            was: form.clone(),
            form,
            choices: Vec::new(),
            notify: false,
            problem: None,
            copied: None,
            theirs: (!event.own).then(|| event.organizer.clone().unwrap_or_default()),
            description_cut: event.description.chars().count() >= MAX_DESCRIPTION,
            menu: None,
            warned: None,
        }
    }

    /// The day its Starts names, while it names one (`2026-11-06`, with a time after it or not).
    #[must_use]
    pub fn day(&self) -> Option<Day> {
        self.form.start.trim().get(..10).and_then(Day::parse)
    }

    /// The list of choices open in the form, when one is.
    #[must_use]
    pub fn menu(&self) -> Option<Menu> {
        self.menu
    }

    /// Opens a list of choices, or with `None` closes the one open.
    pub fn set_menu(&mut self, menu: Option<Menu>) {
        self.menu = menu;
    }

    /// The event edited; `None` for a new one.
    #[must_use]
    pub fn id(&self) -> Option<u64> {
        self.id
    }

    /// The editor's title: `New event`, `Edit event`, or `Event` for someone else's.
    #[must_use]
    pub fn title(&self) -> &'static str {
        match (self.id, &self.theirs) {
            (None, _) => tr!("New event"),
            (Some(_), Some(_)) => tr!("Event"),
            (Some(_), None) => tr!("Edit event"),
        }
    }

    /// Whether the event is someone else's: the editor shows it and changes nothing.
    #[must_use]
    pub fn read_only(&self) -> bool {
        self.theirs.is_some()
    }

    /// Why someone else's event cannot be changed here, in words: `Teodor Lisk's event. …`; `None` for the owner's.
    #[must_use]
    pub fn read_only_note(&self) -> Option<String> {
        self.theirs.as_ref().map(|name| {
            if name.trim().is_empty() {
                tr!("Someone else's event. You can read it here; only the organiser can change it.").to_owned()
            } else {
                tr!("{name}'s event. You can read it here; only the organiser can change it.", name = name)
            }
        })
    }

    /// A press in the description at character `at` (with Ctrl or Cmd held: `command`): on a link, while Ctrl or Cmd
    /// is held or the event is someone else's, the link to copy, kept for [`Self::take_copied`].
    pub fn press_description(&mut self, at: usize, command: bool) -> Option<String> {
        if !command && !self.read_only() {
            return None;
        }
        let link = link_at(&self.form.description, at)?;
        self.copied = Some(link.clone());
        Some(link)
    }

    /// The link a press in the description copied, once.
    pub fn take_copied(&mut self) -> Option<String> {
        self.copied.take()
    }

    /// Whether the description can be changed: not in someone else's event, nor one read cut short.
    #[must_use]
    pub fn description_editable(&self) -> bool {
        !self.read_only() && !self.description_cut
    }

    /// What the form holds.
    #[must_use]
    pub fn form(&self) -> &Form {
        &self.form
    }

    /// The form, to type into.
    pub fn form_mut(&mut self) -> &mut Form {
        &mut self.form
    }

    /// Makes the event all-day, or no longer: the typed times become their days, or the days get 09:00 and 10:00.
    pub fn set_all_day(&mut self, all_day: bool) {
        let form = &mut self.form;
        if form.all_day == all_day {
            return;
        }
        form.all_day = all_day;
        for (text, clock) in [(&mut form.start, "09:00"), (&mut form.end, "10:00")] {
            let day = text.trim().get(..10).and_then(typed_day);
            if let Some(day) = day {
                *text = if all_day { day } else { format!("{day} {clock}") };
            }
        }
    }

    /// The calendars a new event can go into (none for an edit): id and name.
    #[must_use]
    pub fn choices(&self) -> &[(u64, String)] {
        &self.choices
    }

    /// Whether the editor asks to tell the guests of the change: an edit of an event with guests.
    #[must_use]
    pub fn asks_notify(&self) -> bool {
        self.id.is_some() && !self.read_only() && !guests_of(&self.form.guests).is_empty()
    }

    /// The question: `Tell the 2 guests about this change?`.
    #[must_use]
    pub fn notify_question(&self) -> String {
        match guests_of(&self.form.guests).len() {
            1 => tr!("Tell the 1 guest about this change?").to_owned(),
            count => tr!("Tell the {count} guests about this change?", count = count),
        }
    }

    /// Whether the guests are told of the change.
    #[must_use]
    pub fn notify(&self) -> bool {
        self.notify
    }

    /// Tells the guests of the change, or not.
    pub fn set_notify(&mut self, notify: bool) {
        self.notify = notify;
    }

    /// Why the last save was refused.
    #[must_use]
    pub fn problem(&self) -> Option<&str> {
        if self.warned() { Some(warning()) } else { self.problem.as_deref() }
    }

    /// Whether anything was typed or changed since the editor opened.
    #[must_use]
    pub fn edited(&self) -> bool {
        self.form != self.was
    }

    /// Whether the editor said that what was typed is not saved, and nothing changed since.
    #[must_use]
    pub fn warned(&self) -> bool {
        self.warned.as_ref().is_some_and(|(form, notify, _)| *form == self.form && *notify == self.notify)
    }

    /// Whether Esc made the editor say that what was typed is not saved, and nothing changed since: an Esc now throws
    /// the typing away.
    #[must_use]
    pub fn warned_by_escape(&self) -> bool {
        self.warned() && self.warned.as_ref().is_some_and(|(_, _, escape)| *escape)
    }

    /// Esc (`escape`), a press outside, New event or Edit with something typed: the editor stays open and says how to
    /// keep or throw away the typing. The warning holds until the form changes; only one Esc raised arms the next Esc.
    pub(crate) fn warn(&mut self, escape: bool) {
        self.warned = Some((self.form.clone(), self.notify, escape));
        self.problem = None;
    }

    /// The save was refused for `why`: the editor stays open and says so; the reason takes the warning's place, and
    /// the next Esc warns again.
    pub(crate) fn refused(&mut self, why: String) {
        self.problem = Some(why);
        self.warned = None;
    }

    /// The command that saves the form and its parameters, with the times in UTC from the computer's `offset`; `None`
    /// when an edit changed nothing.
    ///
    /// # Errors
    ///
    /// What cannot be read in the form, in words.
    pub fn changes(&self, offset: Offset) -> Result<Option<(&'static str, Value)>, String> {
        if self.read_only() {
            return Ok(None);
        }
        let form = &self.form;
        let times = || -> Result<(String, String), String> {
            if form.all_day {
                let start = typed_day(&form.start).ok_or_else(|| tr!("Starts is no day like 2026-11-06").to_owned())?;
                let end = typed_day(&form.end).ok_or_else(|| tr!("Ends is no day like 2026-11-06").to_owned())?;
                Ok((start, end))
            } else {
                let start = typed_moment(&form.start, offset).ok_or_else(|| tr!("Starts is no date and time like 2026-11-06 13:00").to_owned())?;
                let end = typed_moment(&form.end, offset).ok_or_else(|| tr!("Ends is no date and time like 2026-11-06 13:00").to_owned())?;
                Ok((start, end))
            }
        };
        let guests = guests_of(&form.guests);
        let Some(id) = self.id else {
            let (start, end) = times()?;
            let params = json!({
                "calendar": form.calendar,
                "summary": form.summary.trim(),
                "start": start,
                "end": end,
                "all_day": form.all_day,
                "busy": form.busy,
                "private": form.private,
                "repeat": form.repeat,
                "guests": guests,
            });
            let params = match (params, form.alert.param()) {
                (Value::Object(mut fields), Some(alert)) => {
                    fields.insert("alert".to_owned(), alert);
                    Value::Object(fields)
                }
                (params, _) => params,
            };
            let params = match params {
                Value::Object(mut fields) if !form.description.trim().is_empty() => {
                    fields.insert("description".to_owned(), json!(form.description));
                    Value::Object(fields)
                }
                params => params,
            };
            return Ok(Some(("event.create", params)));
        };
        let was = &self.was;
        let mut params = Map::new();
        if form.summary.trim() != was.summary.trim() {
            params.insert("summary".to_owned(), json!(form.summary.trim()));
        }
        if form.all_day != was.all_day {
            params.insert("all_day".to_owned(), json!(form.all_day));
        }
        if form.all_day != was.all_day || form.start.trim() != was.start.trim() || form.end.trim() != was.end.trim() {
            let (start, end) = times()?;
            params.insert("start".to_owned(), json!(start));
            params.insert("end".to_owned(), json!(end));
        }
        if form.busy != was.busy {
            params.insert("busy".to_owned(), json!(form.busy));
        }
        if form.private != was.private {
            params.insert("private".to_owned(), json!(form.private));
        }
        if form.repeat != was.repeat && REPEATS.contains(&form.repeat.as_str()) {
            params.insert("repeat".to_owned(), json!(form.repeat));
        }
        if form.alert != was.alert
            && let Some(alert) = form.alert.param()
        {
            params.insert("alert".to_owned(), alert);
        }
        if guests != guests_of(&was.guests) {
            params.insert("guests".to_owned(), json!(guests));
        }
        if form.description != was.description && !self.description_cut {
            params.insert("description".to_owned(), json!(form.description));
        }
        if params.is_empty() {
            return Ok(None);
        }
        params.insert("id".to_owned(), json!(id));
        if self.notify && self.asks_notify() {
            params.insert("notify".to_owned(), json!(true));
        }
        Ok(Some(("event.update", Value::Object(params))))
    }
}

/// The longest link the description gives to copy, in bytes.
pub const MAX_LINK: usize = 2048;

/// The link in `text` at character `at`, as a press there would copy it: a word starting `http://`, `https://` or
/// `mailto:` (in any letter case), with the brackets, quotes and full stops around it left off; a web link needs a
/// host. `None` where no link stands, or one longer than [`MAX_LINK`].
#[must_use]
pub fn link_at(text: &str, at: usize) -> Option<String> {
    let (start, word) = text
        .split(char::is_whitespace)
        .scan(0_usize, |from, word| {
            let start = *from;
            *from = from.saturating_add(word.chars().count()).saturating_add(1);
            Some((start, word))
        })
        .find(|(start, word)| at >= *start && at <= start.saturating_add(word.chars().count()))?;
    let lead = word.chars().take_while(|c| matches!(c, '(' | '<' | '[' | '"' | '\'' | '«')).count();
    let link =
        word.trim_start_matches(['(', '<', '[', '"', '\'', '«']).trim_end_matches(['.', ',', ';', ':', '!', '?', ')', '>', ']', '"', '\'', '»']);
    let first = start.saturating_add(lead);
    if at < first || at > first.saturating_add(link.chars().count()) || link.len() > MAX_LINK || link.chars().any(char::is_control) {
        return None;
    }
    let scheme = link.get(..link.find(':')?)?.to_ascii_lowercase();
    let rest = link.get(scheme.len().saturating_add(1)..)?;
    let usable = match scheme.as_str() {
        "http" | "https" => rest.strip_prefix("//").is_some_and(|host| host.split(['/', '?', '#']).next().is_some_and(|host| !host.is_empty())),
        "mailto" => !rest.is_empty(),
        _ => false,
    };
    usable.then(|| link.to_owned())
}

/// A wall-clock moment as the form shows it: `2026-11-06 13:00`.
fn typed(moment: Moment) -> String {
    format!("{} {}", iso_day(moment.day), moment.clock())
}

/// A day as ISO 8601 writes it: `2026-11-06`.
fn iso_day(day: Day) -> String {
    format!("{:04}-{:02}-{:02}", day.year(), day.month(), day.date())
}

/// The day typed (`2026-11-06`), as the engine reads it; nothing else.
#[must_use]
pub fn typed_day(text: &str) -> Option<String> {
    let text = text.trim();
    (text.len() == 10).then(|| Day::parse(text)).flatten().map(iso_day)
}

/// The wall-clock moment typed (`2026-11-06 13:00`, or with a `T`) at `offset`, in UTC as the engine reads it
/// (`2026-11-06T12:00:00Z`); nothing else.
#[must_use]
pub fn typed_moment(text: &str, offset: Offset) -> Option<String> {
    let text = text.trim();
    if text.len() != 16 || !matches!(text.get(10..11), Some(" " | "T")) {
        return None;
    }
    let day = Day::parse(text.get(..10)?)?;
    let moment = Moment::parse(&format!("{}T{}", text.get(..10)?, text.get(11..)?))?;
    let hour = u8::try_from(moment.minute / 60).ok()?;
    let minute = u8::try_from(moment.minute % 60).ok()?;
    let at = CivilDateTime::new(day.year(), day.month(), day.date(), hour, minute, 0)?.to_timestamp(offset)?;
    let utc = at.to_civil(Offset::UTC)?;
    Some(format!("{:04}-{:02}-{:02}T{:02}:{:02}:00Z", utc.year(), utc.month(), utc.day(), utc.hour(), utc.minute()))
}

/// The addresses of a guest list typed with commas, semicolons or lines between them.
#[must_use]
pub fn guests_of(text: &str) -> Vec<String> {
    text.split([',', ';', '\n']).map(str::trim).filter(|address| !address.is_empty()).map(str::to_owned).collect()
}

#[cfg(test)]
#[path = "editor_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "editor_alert_tests.rs"]
mod alert_tests;
#[cfg(test)]
#[path = "editor_owner_tests.rs"]
mod owner_tests;
