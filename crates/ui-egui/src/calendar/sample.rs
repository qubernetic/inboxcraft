// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The sample's calendars: Personal and Choir of the personal account, Studio of the studio account, with invented
//! events around the week of the sample's time now, so the day, week, month and agenda views have something to show:
//! overlapping events on one day, repeating ones, blocks over several days, and an event in a zone of its own.
//! Every name, place and booking is invented; the addresses are `example.org`.

use inboxcraft_model::{AccountId, Calendar, CalendarEvent, CalendarEventId, CalendarId};
use inboxcraft_store::Change;
use inboxcraft_time::{Offset, Timestamp};

use super::Day;
use crate::UiError;

/// The calendars: `(id, account, name, colour)`.
const CALENDARS: [(u64, u64, &str, &str); 3] = [(1, 1, "Personal", "#3b82f6ff"), (2, 2, "Studio", "#8b5cf6ff"), (3, 1, "Choir", "#e11d74ff")];

/// The zone of the ferry: the Azores, an hour behind UTC in winter (no summer time in November).
const AZORES: &str = "BEGIN:VTIMEZONE\r\nTZID:Atlantic/Azores\r\nBEGIN:STANDARD\r\nDTSTART:19701025T010000\r\n\
    TZOFFSETFROM:+0000\r\nTZOFFSETTO:-0100\r\nRRULE:FREQ=YEARLY;BYMONTH=10;BYDAY=-1SU\r\nEND:STANDARD\r\n\
    BEGIN:DAYLIGHT\r\nDTSTART:19700329T000000\r\nTZOFFSETFROM:-0100\r\nTZOFFSETTO:+0000\r\n\
    RRULE:FREQ=YEARLY;BYMONTH=3;BYDAY=-1SU\r\nEND:DAYLIGHT\r\nEND:VTIMEZONE\r\n";

/// An event: its calendar, its uid, the days after Monday it starts and ends on, and its lines between `UID` and
/// `END:VEVENT` with `{start}` and `{end}` for the two days (`20261105`).
struct Sample {
    calendar: u64,
    uid: &'static str,
    start: i64,
    end: i64,
    lines: &'static str,
}

const EVENTS: [Sample; 11] = [
    Sample {
        calendar: 2,
        uid: "standup",
        start: 0,
        end: 0,
        lines: "DTSTART:{start}T090000Z\r\nDTEND:{end}T100000Z\r\nSUMMARY:Studio stand-up\r\nRRULE:FREQ=WEEKLY;COUNT=6\r\n",
    },
    Sample {
        calendar: 2,
        uid: "print-run",
        start: 1,
        end: 1,
        lines: "DTSTART:{start}T100000Z\r\nDTEND:{end}T120000Z\r\nSUMMARY:Print run at Lumen\r\nLOCATION:Lumen Print Works\r\n",
    },
    Sample {
        calendar: 3,
        uid: "choir",
        start: 2,
        end: 2,
        lines: "DTSTART:{start}T190000Z\r\nDTEND:{end}T203000Z\r\nSUMMARY:Choir practice\r\nLOCATION:St Brendan's hall\r\n\
                RRULE:FREQ=WEEKLY;COUNT=8\r\n",
    },
    Sample {
        calendar: 2,
        uid: "petra",
        start: 3,
        end: 3,
        lines: "DTSTART:{start}T093000Z\r\nDTEND:{end}T103000Z\r\nSUMMARY:Call with Petra about the logo\r\n\
                ORGANIZER;CN=Mira Holloway:mailto:mira@holloway-studio.example.org\r\n\
                ATTENDEE;CN=Petra Lindqvist;PARTSTAT=ACCEPTED:mailto:petra@example.org\r\n",
    },
    Sample {
        calendar: 1,
        uid: "dentist",
        start: 3,
        end: 3,
        lines: "DTSTART:{start}T100000Z\r\nDTEND:{end}T113000Z\r\nSUMMARY:Dentist\r\nLOCATION:Harbour Dental\r\nCLASS:PRIVATE\r\n",
    },
    Sample {
        calendar: 1,
        uid: "allotment",
        start: 3,
        end: 3,
        lines: "DTSTART:{start}T130000Z\r\nDTEND:{end}T150000Z\r\nSUMMARY:Allotment work party\r\nLOCATION:Gull Lane allotments\r\n\
                ORGANIZER;CN=Teodor Lisk:mailto:teodor@example.org\r\n\
                ATTENDEE;CN=Mira Holloway;PARTSTAT=ACCEPTED:mailto:mira@example.org\r\n",
    },
    Sample {
        calendar: 2,
        uid: "book-fair",
        start: 3,
        end: 5,
        lines: "DTSTART;VALUE=DATE:{start}\r\nDTEND;VALUE=DATE:{end}\r\nSUMMARY:Small press book fair\r\nTRANSP:TRANSPARENT\r\n",
    },
    Sample {
        calendar: 1,
        uid: "odile",
        start: 5,
        end: 5,
        lines: "DTSTART:{start}T123000Z\r\nDTEND:{end}T133000Z\r\nSUMMARY:Lunch with Odile\r\nLOCATION:The Gull café\r\n\
                ORGANIZER;CN=Odile Brannock:mailto:odile@example.org\r\n\
                ATTENDEE;CN=Mira Holloway;PARTSTAT=NEEDS-ACTION:mailto:mira@example.org\r\n",
    },
    Sample {
        calendar: 1,
        uid: "ferry",
        start: 6,
        end: 6,
        lines: "DTSTART;TZID=Atlantic/Azores:{start}T070000\r\nDTEND;TZID=Atlantic/Azores:{end}T100000\r\n\
                SUMMARY:Ferry to the island\r\nLOCATION:Harbour pier\r\nDESCRIPTION:Grey Gull Ferries · Booking GG-2291\r\n",
    },
    Sample {
        calendar: 1,
        uid: "coast-trip",
        start: 14,
        end: 17,
        lines: "DTSTART;VALUE=DATE:{start}\r\nDTEND;VALUE=DATE:{end}\r\nSUMMARY:Coast trip\r\n",
    },
    Sample {
        calendar: 2,
        uid: "open-day",
        start: 20,
        end: 21,
        lines: "DTSTART;VALUE=DATE:{start}\r\nDTEND;VALUE=DATE:{end}\r\nSUMMARY:Studio open day\r\n",
    },
];

/// The day as an iCalendar date: `20261105`.
fn ics(day: Day) -> String {
    format!("{:04}{:02}{:02}", day.year(), day.month(), day.date())
}

/// The changes that put the sample's calendars and their events into a store, around the week of `now` (UTC).
///
/// # Errors
///
/// [`UiError::Sample`] when `now` has no day.
pub fn changes(now: Timestamp) -> Result<Vec<Change>, UiError> {
    let monday = Day::of(now, Offset::UTC).ok_or_else(|| UiError::sample("no day for the sample's calendars"))?.monday();
    let base = |calendar: u64| {
        let name = CALENDARS.iter().find(|(id, ..)| *id == calendar).map_or("personal", |(_, _, name, _)| name);
        format!("https://dav.example.org/calendars/mira/{}/", name.to_ascii_lowercase())
    };
    let mut changes: Vec<Change> = CALENDARS
        .iter()
        .map(|(id, account, name, colour)| {
            Change::PutCalendar(Calendar {
                id: CalendarId::new(*id),
                account: AccountId::new(*account),
                href: base(*id),
                name: (*name).to_owned(),
                colour: Some((*colour).to_owned()),
            })
        })
        .collect();
    for (index, sample) in EVENTS.iter().enumerate() {
        let lines = sample.lines.replace("{start}", &ics(monday.plus(sample.start))).replace("{end}", &ics(monday.plus(sample.end)));
        let zone = if sample.uid == "ferry" { AZORES } else { "" };
        let ical = format!(
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//InboxCraft//Sample//EN\r\n{zone}BEGIN:VEVENT\r\nUID:{uid}@example.org\r\n\
             DTSTAMP:20261101T000000Z\r\n{lines}END:VEVENT\r\nEND:VCALENDAR\r\n",
            uid = sample.uid
        );
        let id = u64::try_from(index).unwrap_or(0).saturating_add(1);
        changes.push(Change::PutCalendarEvent(CalendarEvent {
            id: CalendarEventId::new(id),
            calendar: CalendarId::new(sample.calendar),
            href: format!("{}{}.ics", base(sample.calendar), sample.uid),
            etag: Some(format!("\"{}\"", sample.uid)),
            ical,
        }));
    }
    Ok(changes)
}

#[cfg(test)]
#[path = "sample_tests.rs"]
mod tests;
