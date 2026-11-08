// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The event editor's form: a new event (`event.create`) or an event picked on the screen (`event.update`), with its
//! title, its start and end typed as `2026-11-06 13:00` (a day alone, `2026-11-06`, for an all-day event), all-day,
//! private, a repeat preset, busy or free, and the guests' addresses. An edit sends only what changed. Editing an
//! event that has guests asks whether to tell them of the change; the default is not to. A new event asks nothing:
//! inviting the guests is the invitations' work.
//!
//! The times are the computer's wall clock at its offset, sent to the engine in UTC. Changing the times of an
//! occurrence of a repeating event moves the whole series to them.

use inboxcraft_time::{CivilDateTime, Offset};
use serde_json::{Map, Value, json};

use super::{Day, EventRow, Moment};
use crate::tr;

/// The repeat presets the editor offers, as `event.repeats` names them.
pub const REPEATS: [&str; 6] = ["none", "daily", "weekdays", "weekly", "monthly", "yearly"];

/// The longest text a field of the editor takes, in characters.
pub const MAX_FIELD: usize = 1000;

/// The longest guest list typed, in characters.
pub const MAX_GUESTS_TEXT: usize = 8000;

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
    /// The guests' addresses, separated by commas.
    pub guests: String,
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
            guests: String::new(),
        };
        Self { id: None, was: form.clone(), form, choices, notify: false, problem: None }
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
            guests: addresses.join(", "),
        };
        Self { id: Some(event.id), was: form.clone(), form, choices: Vec::new(), notify: false, problem: None }
    }

    /// The event edited; `None` for a new one.
    #[must_use]
    pub fn id(&self) -> Option<u64> {
        self.id
    }

    /// The editor's title: `New event` or `Edit event`.
    #[must_use]
    pub fn title(&self) -> &'static str {
        if self.id.is_none() { tr!("New event") } else { tr!("Edit event") }
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
        self.id.is_some() && !guests_of(&self.form.guests).is_empty()
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
        self.problem.as_deref()
    }

    /// The save was refused for `why`: the editor stays open and says so.
    pub(crate) fn refused(&mut self, why: String) {
        self.problem = Some(why);
    }

    /// The command that saves the form and its parameters, with the times in UTC from the computer's `offset`; `None`
    /// when an edit changed nothing.
    ///
    /// # Errors
    ///
    /// What cannot be read in the form, in words.
    pub fn changes(&self, offset: Offset) -> Result<Option<(&'static str, Value)>, String> {
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
        if guests != guests_of(&was.guests) {
            params.insert("guests".to_owned(), json!(guests));
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
