// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The design file's own sample mailbox, for the pictures that are measured against its frames: Odile Brannock's
//! Personal and Studio accounts on Monday 16 November 2026 at 14:04 (UTC), synced at 14:02, with the people,
//! subjects, times and counts the frames show. Seven to read in the inbox (two from people, four notifications, one
//! newsletter), three first-time senders in the screener, four snoozed (two back today), three pinned, two parked,
//! three done, one message in the outbox waiting to go (no time of its own), a sent message that bounced, and the
//! week of 16 November in four calendars, the design review at 15:00 next.
//!
//! As the present sample, every name, address, subject and text is invented for InboxCraft (the senders are on
//! `example.org` and its subdomains), nothing connects anywhere, and it is built afresh at every start; `just
//! run-sample` keeps the present sample.

use inboxcraft_engine::Session;
use inboxcraft_model::{AccountId, Calendar, CalendarEvent, CalendarEventId, CalendarId, Rsvp, SpecialUse};
use inboxcraft_store::{Change, Memory, Storage, StoreConfig};
use inboxcraft_time::{Clock, FakeClock, Timestamp};

use super::{
    Away, DAY, HOUR, Invited, Lookup, Meeting, Person, Sample, Schedule, Text, Thread, bounce_of, id_in, invitation, mail, person, put_away_of,
    sample, scheduled_of, session_over, showcase_of, submission,
};
use crate::UiError;
use crate::iconset::IconId;
use crate::mailbox::Showcase;

/// The time the frames show: Monday 16 November 2026, 14:04 UTC (the sync two minutes before, at 14:02).
pub const TIME: i64 = 1_794_837_840;

const ODILE: Person = person("Odile Brannock", "odile@example.org");
const STUDIO: Person = person("Odile Brannock", "studio@example.org");
const TEODOR: Person = person("Teodor Vasquez", "teodor@example.org");
const FORGEHUB: Person = person("Forgehub", "forgehub@example.org");
const MARIT: Person = person("Marit Solheim", "marit@example.org");
const PARCELWAY: Person = person("Parcelway", "parcels@example.org");
const BANKLINE: Person = person("Bankline", "statements@example.org");
const INES: Person = person("Ines Okafor", "ines@example.org");
const COLLECTIVE: Person = person("Studio Collective", "news@studio-collective.example.org");
const PAVEL: Person = person("Pavel Hruska", "pavel@example.org");
const HOSTWELL: Person = person("Hostwell", "certificates@example.org");
const TYPE_WEEKLY: Person = person("Type Weekly", "hello@typeweekly.example.org");
const QUIET_WEB: Person = person("The Quiet Web", "finds@quietweb.example.org");
const KETTLE: Person = person("Kettle & Co", "orders@kettle.example.org");
const BRIGHTSHELF: Person = person("Brightshelf", "news@brightshelf.example.org");
const LUCIAN: Person = person("Lucian Albrecht", "lucian@example.org");
const DEALS: Person = person("Deals Daily", "offers@example.org");
const NORA: Person = person("Nora Keane", "nora@example.org");
const HALVARD: Person = person("Halvard Engen", "halvard@example.org");
const CITYWORKS: Person = person("Cityworks", "permits@example.org");
const SUNNIVA: Person = person("Sunniva Aalto", "sunniva@example.org");
const ANNELISE: Person = person("Annelise Vandenberghe-Okonkwo-Halvorssen", "annelise@example.org");
const TOBIAS: Person = person("Tobias Wren", "tobias@example.org");
const DMITRI: Person = person("Dmitri Vale", "dmitri@example.org");
const ANSEL: Person = person("Ansel", "ansel@example.org");
const MIRELA: Person = person("Mirela Anton", "mirela@example.org");
const ANDERS: Person = person("Anders Holm", "anders@example.org");
const HANNE: Person = person("Hanne Dahl", "hanne@example.org");

/// The two accounts: `(id, label, owner)`.
const ACCOUNTS: [(u64, &str, Person); 2] = [(1, "Personal", ODILE), (2, "Studio", STUDIO)];

/// The folders: `(id, account, path, special use)`. The ids of the present sample's Personal folders stay; the
/// Studio account has the frames' Clients, Invoices 2026, Kiln rota (empty) and Open day.
const FOLDERS: &[(u64, u64, &str, &[SpecialUse])] = &[
    (1, 1, "INBOX", &[]),
    (2, 1, "Drafts", &[SpecialUse::Drafts]),
    (3, 1, "Sent", &[SpecialUse::Sent]),
    (4, 1, "Archive", &[SpecialUse::Archive]),
    (6, 1, "Bin", &[SpecialUse::Trash]),
    (10, 2, "INBOX", &[]),
    (11, 2, "Sent", &[SpecialUse::Sent]),
    (12, 2, "Archive", &[SpecialUse::Archive]),
    (13, 2, "Bin", &[SpecialUse::Trash]),
    (14, 2, "Clients", &[]),
    (15, 2, "Invoices 2026", &[]),
    (16, 1, "InboxCraft/Screener", &[]),
    (17, 2, "Kiln rota", &[]),
    (18, 2, "Open day", &[]),
];

/// The Kiln rota folder: empty, as the frame of an empty folder shows it.
pub const EMPTY_FOLDER: u64 = 17;

/// Minutes before 14:04 on Monday: `days` back and then to `hour`:`minute` of that day.
const fn at(days: u64, hour: u64, minute: u64) -> u64 {
    let today = 14 * HOUR + 4;
    days * DAY + today - hour * HOUR - minute
}

/// The messages. The inbox (folders 1 and 10) holds the thirteen rows of the frames' inbox (with the invitation
/// added after), then the three done, four snoozed and two parked that [`put_away`] takes out of it.
const MESSAGES: &[Sample] = &[
    sample(
        10,
        TEODOR,
        STUDIO,
        "Draft logo for the bakery",
        "Hi Odile, two directions attached. The first is close to the sign they have now. The second keeps the wheat but drops the frame.",
        at(0, 14, 2),
        false,
        false,
    ),
    sample(1, FORGEHUB, ODILE, "[inboxcraft] CI passed on develop", "All 12 checks passed in 6 min 40 s.", at(0, 13, 55), false, false),
    sample(10, MARIT, STUDIO, "Re: Thursday workshop", "Room 3 is free from ten. I can bring the projector.", at(0, 13, 40), false, false),
    sample(1, PARCELWAY, ODILE, "Your parcel arrives tomorrow", "Between 9:00 and 12:00, no signature needed.", at(0, 12, 30), false, false),
    sample(1, BANKLINE, ODILE, "Statement for October", "Your statement is ready to read.", at(0, 11, 48), false, false),
    sample(10, INES, STUDIO, "Invoice 2026-114", "Attached, due on the 30th.", at(0, 11, 15), true, false),
    sample(1, FORGEHUB, ODILE, "Review requested: #221", "A second pair of eyes on the screener keys, please.", at(0, 9, 31), false, false),
    sample(10, COLLECTIVE, STUDIO, "Studio Collective news · November", "Open day on the 22nd, and a new kiln.", at(0, 9, 10), false, false),
    sample(1, PAVEL, ODILE, "Photos from the harvest festival", "Forty-two picked, link inside.", at(1, 17, 20), true, false),
    sample(1, HOSTWELL, ODILE, "Certificate renewed", "example.org is covered until 14 Feb 2027.", at(1, 10, 15), true, false),
    sample(1, TYPE_WEEKLY, ODILE, "Type Weekly #88", "Five monospaced faces worth a look.", at(1, 8, 0), true, false),
    sample(1, QUIET_WEB, ODILE, "The Quiet Web", "Small sites, no trackers: this week's finds.", at(2, 9, 0), true, false),
    // Done.
    sample(10, INES, STUDIO, "Re: Invoice 2026-113", "Paid this morning, thank you.", at(0, 12, 50), true, false),
    sample(1, KETTLE, ODILE, "Your order KC-20418", "Thank you. Your teapot is on its way.", at(0, 10, 31), true, false),
    sample(1, TYPE_WEEKLY, ODILE, "Type Weekly #87", "Why italics lean.", at(7, 8, 0), true, false),
    // Snoozed.
    sample(
        10,
        HALVARD,
        STUDIO,
        "Kiln service: two dates to choose from",
        "Good morning, the kiln is due its yearly service. I can come on Tuesday 24 or on Thursday 26 November, between nine and noon.",
        at(3, 11, 20),
        true,
        false,
    ),
    sample(
        10,
        CITYWORKS,
        STUDIO,
        "Parking permit for the open day",
        "Your permit for two bays on Saturday 21 November is ready.",
        at(2, 10, 0),
        true,
        false,
    ),
    sample(
        10,
        SUNNIVA,
        STUDIO,
        "Glaze order, second batch",
        "The celadon is back in stock; the second batch goes out on Wednesday.",
        at(4, 15, 30),
        true,
        false,
    ),
    sample(
        1,
        BRIGHTSHELF,
        ODILE,
        "Your warranty runs out in December",
        "The two-year cover on your shelving ends on 14 December.",
        at(5, 9, 0),
        true,
        false,
    ),
    // Parked.
    sample(
        10,
        TOBIAS,
        STUDIO,
        "Spare keys for the studio",
        "Two spare keys are cut; they are in the drawer under the till.",
        at(6, 16, 30),
        true,
        false,
    ),
    sample(1, DMITRI, ODILE, "Kiln rota for December", "The first draft of the rota; swap any day you cannot do.", at(6, 11, 0), true, false),
    // Pinned, kept in the Studio archive.
    sample(12, MARIT, STUDIO, "Workshop: what to bring", "Aprons, an old towel and the glaze notes from last time.", at(7, 10, 0), true, false),
    sample(12, COLLECTIVE, STUDIO, "Door code and opening hours", "The new door code is on the board by the kiln room.", at(46, 9, 0), true, false),
    sample(
        12,
        ANNELISE,
        STUDIO,
        "Minutes of the quarterly studio planning meeting, with the kiln rota and the open day plan",
        "We agreed on the kiln rota for December and on two shifts for the open day.",
        at(49, 17, 0),
        true,
        false,
    ),
    // The workshop thread: Marit asks, Odile answers, Ines adds; Marit's last word is in the inbox.
    sample(
        12,
        MARIT,
        STUDIO,
        "Thursday workshop",
        "Can we hold the workshop on Thursday? I would like two hours for glazing and one for the wheels.",
        at(4, 9, 30),
        true,
        false,
    ),
    sample(11, STUDIO, MARIT, "Re: Thursday workshop", "Thursday works. Which room, and do we need the projector?", at(4, 16, 10), true, false),
    sample(
        12,
        INES,
        STUDIO,
        "Re: Thursday workshop",
        "Room 3 has the long table. I have the keys to the store room, so the wheels can come out the night before.",
        at(3, 10, 5),
        true,
        false,
    ),
    // The sent message the report says did not arrive.
    sample(
        11,
        STUDIO,
        ANSEL,
        "Shelving for the store room: two options",
        "Hi Ansel, both fit the back wall: four bays of 90 cm in steel, or three bays of 120 cm in pine.",
        at(1, 16, 40),
        true,
        false,
    ),
    // New senders, held in the screener.
    sample(
        16,
        LUCIAN,
        STUDIO,
        "Prices for the sign, oak or steel",
        "We can start in the first week of December. The price for the lettering is the same in oak or in steel.",
        at(0, 13, 20),
        false,
        false,
    ),
    sample(16, LUCIAN, STUDIO, "Sizes for the sign", "Could you send the width of the fascia? Then I can price both.", at(1, 15, 10), false, false),
    sample(16, DEALS, ODILE, "48 hours only", "Everything must go. Unsubscribe at the bottom.", at(0, 12, 41), false, false),
    sample(16, NORA, STUDIO, "Introduction from Marit", "Marit said you might help with a sign for our studio.", at(0, 11, 7), false, false),
    // The Personal archive: a marketing message and the people the composer suggests.
    sample(4, BRIGHTSHELF, ODILE, "48 hours: winter lamps −30 %", "Winter lamps at thirty per cent off, two days only.", at(0, 9, 2), true, false),
    sample(
        4,
        MIRELA,
        ODILE,
        "Lettering for the café menu",
        "The menu board is up; the chalk lettering survived the first week.",
        at(20, 10, 0),
        true,
        false,
    ),
    sample(4, ANDERS, ODILE, "Frames for the exhibition", "Twelve oak frames are ready to collect on Friday.", at(25, 12, 0), true, false),
    sample(4, HANNE, ODILE, "Book club in December", "We read the lighthouse novel; bring a dish if you like.", at(30, 18, 0), true, false),
];

/// The thread of the workshop: the four messages and what each answers.
const THREADS: [Thread; 4] = [
    Thread { subject: "Thursday workshop", from: "marit@example.org", id: "workshop.1@example.org", answers: None },
    Thread { subject: "Re: Thursday workshop", from: "studio@example.org", id: "workshop.2@example.org", answers: Some("workshop.1@example.org") },
    Thread { subject: "Re: Thursday workshop", from: "ines@example.org", id: "workshop.3@example.org", answers: Some("workshop.2@example.org") },
    Thread { subject: "Re: Thursday workshop", from: "marit@example.org", id: "workshop.4@example.org", answers: Some("workshop.3@example.org") },
];

const TEXTS: &[Text] = &[
    Text {
        subject: "Draft logo for the bakery",
        from: None,
        plain: "Hi Odile,\n\ntwo directions attached. The first is close to the sign they have now. The second keeps the wheat but \
                drops the frame.\n\nTell me which one you want to take further and I will draw the shop front with it.\n\nTeodor\n",
        html: None,
        files: &["bakery-logo-v2.pdf"],
    },
    Text {
        subject: "Re: Thursday workshop",
        from: Some("marit@example.org"),
        plain: "Room 3 is free from ten. I can bring the projector.\n\nSee you on Thursday,\nMarit\n\nInes Okafor wrote:\n\
                > Room 3 has the long table. I have the keys to the store room, so the\n> wheels can come out the night before.\n>\n\
                > Ines\n>\n> Odile Brannock wrote:\n>> Thursday works. Which room, and do we need the\n>> projector?\n>>\n>> Odile\n",
        html: None,
        files: &[],
    },
    Text {
        subject: "Kiln service: two dates to choose from",
        from: None,
        plain: "Good morning,\n\nthe kiln is due its yearly service. I can come on Tuesday 24 or on Thursday 26 November, between \
                nine and noon.\n\nWhich of the two suits you?\n\nHalvard Engen\n",
        html: None,
        files: &[],
    },
    Text {
        subject: "Shelving for the store room: two options",
        from: None,
        plain: "Hi Ansel,\n\nboth fit the back wall: four bays of 90 cm in steel, or three bays of 120 cm in pine. The pine is \
                warmer, the steel takes more weight.\n\nOdile Brannock, Studio Collective\n",
        html: None,
        files: &[],
    },
    Text {
        subject: "Studio Collective news · November",
        from: None,
        plain: "Open day on the 22nd, and a new kiln.\n\nRead it in the browser: https://news.studio-collective.example.org/november\n",
        html: Some(
            "<h1>Studio Collective · November</h1><p><img src=\"https://pictures.studio-collective.example.org/kiln.png\" \
             alt=\"The new kiln\"></p><p>Open day on the <b>22nd</b>, and a new kiln in the back room.</p>\
             <p><a href=\"https://news.studio-collective.example.org/november\">Read it in the browser</a></p>\
             <img src=\"https://track.example.net/open.png?id=88\" width=\"1\" height=\"1\" alt=\"\">",
        ),
        files: &[],
    },
    Text {
        subject: "48 hours: winter lamps −30 %",
        from: None,
        plain: "Winter lamps at thirty per cent off, two days only.\n\nShop now: https://brightshelf.example.org/winter\n",
        html: Some(
            "<h2>Winter lamps −30 %</h2><p><img src=\"https://pictures.brightshelf.example.org/lamps.png\" alt=\"Three lamps\"></p>\
             <p>Two days only: every winter lamp at thirty per cent off.</p><ul><li>Desk lamps</li><li>Floor lamps</li>\
             <li>Clip lights</li></ul><p><a href=\"https://brightshelf.example.org/winter\">Shop now</a></p>",
        ),
        files: &[],
    },
];

/// The senders whose mail says a machine sent it: Notifications.
const MACHINES: [Person; 6] = [FORGEHUB, PARCELWAY, BANKLINE, HOSTWELL, KETTLE, CITYWORKS];

/// The senders whose mail comes through a list: Newsletters.
const LISTS: [Person; 4] = [COLLECTIVE, TYPE_WEEKLY, QUIET_WEB, BRIGHTSHELF];

const LOOKUP: Lookup = Lookup { links: &[], threads: &THREADS, texts: TEXTS, machines: &MACHINES, lists: &LISTS };

/// The design review Marit moved: today, 15:00 to 16:00 in Room 3.
const REVIEW: Meeting = Meeting {
    uid: "design-review@example.org",
    summary: "Design review",
    location: "Room 3",
    organiser: MARIT,
    attendee: STUDIO,
    text: "Moved by an hour, Ines has a call until three.\n\nMarit\n",
    account: 2,
    day: 0,
    minutes: 60,
};

/// The first invitation to the review (answered Going, in the Studio archive) and the update in the inbox.
const INVITATIONS: [Invited; 2] = [
    Invited {
        folder: 12,
        subject: "Invitation: Design review, {when}",
        minutes_ago: at(3, 16, 0),
        message_id: "review.1@example.org",
        follows: None,
        sequence: 0,
        start: (14, 0),
        answer: Some(Rsvp::Accepted),
        seen: true,
    },
    Invited {
        folder: 10,
        subject: "Updated invitation: Design review, {when}",
        minutes_ago: at(0, 10, 2),
        message_id: "review.2@example.org",
        follows: Some("review.1@example.org"),
        sequence: 1,
        start: (15, 0),
        answer: None,
        seen: true,
    },
];

/// The calendars: `(id, account, name, colour)`.
const CALENDARS: [(u64, u64, &str, &str); 4] =
    [(1, 1, "Personal", "#3b82f6ff"), (2, 2, "Studio", "#8b5cf6ff"), (3, 1, "Birthdays", "#e11d74ff"), (4, 2, "Kiln rota", "#f59e0bff")];

/// The week's events: `(calendar, uid, day after Monday, lines)`; `{day}` is that day (`20261116`), `{after}` two
/// days after it, the `DTEND` of a two-day all-day event (the open day, Saturday and Sunday).
const EVENTS: [(u64, &str, i64, &str); 12] = [
    (2, "workshop-prep", 0, "DTSTART:{day}T100000Z\r\nDTEND:{day}T110000Z\r\nSUMMARY:Workshop prep\r\nLOCATION:Room 3\r\n"),
    (
        2,
        "design-review",
        0,
        "DTSTART:{day}T150000Z\r\nDTEND:{day}T160000Z\r\nSUMMARY:Design review\r\nLOCATION:Room 3\r\n\
         DESCRIPTION:Second round on the bakery logo. Teodor brings both directions.\r\n\
         ORGANIZER;CN=Marit Solheim:mailto:marit@example.org\r\n\
         ATTENDEE;CN=Odile Brannock;PARTSTAT=ACCEPTED:mailto:studio@example.org\r\n\
         ATTENDEE;CN=Teodor Vasquez;PARTSTAT=ACCEPTED:mailto:teodor@example.org\r\n\
         ATTENDEE;CN=Ines Okafor;PARTSTAT=TENTATIVE:mailto:ines@example.org\r\n",
    ),
    (1, "dinner", 0, "DTSTART:{day}T183000Z\r\nDTEND:{day}T200000Z\r\nSUMMARY:Dinner with Marit\r\n"),
    (4, "kiln-loading", 1, "DTSTART:{day}T083000Z\r\nDTEND:{day}T090000Z\r\nSUMMARY:Kiln loading\r\n"),
    (2, "shop-front", 1, "DTSTART:{day}T090000Z\r\nDTEND:{day}T110000Z\r\nSUMMARY:Bakery: shop front\r\n"),
    (1, "choir", 1, "DTSTART:{day}T190000Z\r\nDTEND:{day}T203000Z\r\nSUMMARY:Choir\r\n"),
    (1, "dentist", 2, "DTSTART:{day}T130000Z\r\nDTEND:{day}T140000Z\r\nSUMMARY:Dentist\r\n"),
    (2, "thursday-workshop", 3, "DTSTART:{day}T100000Z\r\nDTEND:{day}T130000Z\r\nSUMMARY:Thursday workshop\r\nLOCATION:Room 3\r\n"),
    (2, "standup", 4, "DTSTART:{day}T090000Z\r\nDTEND:{day}T093000Z\r\nSUMMARY:Studio stand-up\r\n"),
    (2, "invoices", 4, "DTSTART:{day}T160000Z\r\nDTEND:{day}T170000Z\r\nSUMMARY:Invoices\r\n"),
    (2, "open-day", 5, "DTSTART;VALUE=DATE:{day}\r\nDTEND;VALUE=DATE:{after}\r\nSUMMARY:Open day\r\n"),
    (1, "lunch-pavel", 6, "DTSTART:{day}T130000Z\r\nDTEND:{day}T143000Z\r\nSUMMARY:Lunch with Pavel\r\n"),
];

/// The day `days` after the Monday of `now` as iCalendar writes a date: `20261116`.
fn ics_day(now: Timestamp, days: i64) -> Option<String> {
    let today = now.unix_seconds().div_euclid(86_400);
    // 1970-01-01 was a Thursday: three days after a Monday.
    let monday = today.checked_sub(today.checked_add(3)?.rem_euclid(7))?;
    let at = Timestamp::from_unix(monday.checked_add(days)?.checked_mul(86_400)?, 0)?;
    let civil = at.to_civil(inboxcraft_time::Offset::UTC)?;
    Some(format!("{:04}{:02}{:02}", civil.year(), civil.month(), civil.day()))
}

/// The calendars and the week's events around `now`.
fn calendars(now: Timestamp) -> Result<Vec<Change>, UiError> {
    let base = |calendar: u64| {
        let name = CALENDARS.iter().find(|(id, ..)| *id == calendar).map_or("personal", |(_, _, name, _)| name);
        format!("https://dav.example.org/calendars/odile/{}/", name.to_ascii_lowercase().replace(' ', "-"))
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
    for (index, (calendar, uid, day, lines)) in EVENTS.iter().enumerate() {
        let out = || UiError::sample("a calendar day out of range");
        let lines =
            lines.replace("{day}", &ics_day(now, *day).ok_or_else(out)?).replace("{after}", &ics_day(now, day.saturating_add(2)).ok_or_else(out)?);
        let ical = format!(
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//InboxCraft//Sample//EN\r\nBEGIN:VEVENT\r\nUID:{uid}@example.org\r\n\
             DTSTAMP:20261101T000000Z\r\n{lines}END:VEVENT\r\nEND:VCALENDAR\r\n"
        );
        changes.push(Change::PutCalendarEvent(CalendarEvent {
            id: CalendarEventId::new(u64::try_from(index).unwrap_or(0).saturating_add(1)),
            calendar: CalendarId::new(*calendar),
            href: format!("{}{uid}.ics", base(*calendar)),
            etag: Some(format!("\"{uid}\"")),
            ical,
        }));
    }
    Ok(changes)
}

/// The frames' mailbox at `now`: its accounts (both able to send through an invented server), folders, messages,
/// the two invitations, the bounce and the calendars.
///
/// # Errors
///
/// None in practice: the sample is fixed. A value the model or the store refuses becomes [`UiError::Sample`].
pub fn mailbox(now: Timestamp) -> Result<Memory, UiError> {
    let mut batch = mail(&ACCOUNTS, FOLDERS, MESSAGES, &LOOKUP, now)?;
    let first_id = u64::try_from(MESSAGES.len()).unwrap_or(u64::MAX).saturating_add(1);
    for (index, invited) in INVITATIONS.iter().enumerate() {
        let n = u32::try_from(index).unwrap_or(0);
        for change in invitation(first_id.saturating_add(u64::from(n)), invitation_uid(&INVITATIONS, index), &REVIEW, invited, now)? {
            batch.push(change);
        }
    }
    for change in calendars(now)? {
        batch.push(change);
    }
    let mut store = Memory::new(StoreConfig::default());
    store.apply(&batch).map_err(UiError::sample)?;
    for (id, label, owner) in ACCOUNTS {
        submission(&mut store, id, label, owner)?;
    }
    // No delivery report is in the sample: the engine shows the reason and names none.
    let report = id_in(MESSAGES, "Your order KC-20418")?;
    bounce_of(&mut store, id_in(MESSAGES, BOUNCED)?, report, "ansel@example.org: 550 5.1.1 The address ansel@example.org does not exist.")?;
    Ok(store)
}

/// The UID of the invitation at `index` of `invitations` in its folder: after the folder's messages and the
/// invitations before it in the same folder, so no two meet.
fn invitation_uid(invitations: &[Invited], index: usize) -> u32 {
    let Some(invited) = invitations.get(index) else { return u32::MAX };
    let earlier = invitations.iter().take(index).filter(|other| other.folder == invited.folder).count();
    let messages = MESSAGES.iter().filter(|sample| sample.folder == invited.folder).count();
    u32::try_from(messages.saturating_add(earlier)).unwrap_or(u32::MAX).saturating_add(1)
}

/// The sent message the report says did not arrive.
pub const BOUNCED: &str = "Shelving for the store room: two options";

/// A sample session over the frames' [`mailbox`] at `clock`'s time now, the screener on. Run [`prepare`] on the
/// window's mailbox before the picture.
///
/// # Errors
///
/// As [`mailbox`], and [`UiError::Engine`] when the session refuses the store.
pub fn session(clock: Box<dyn Clock>) -> Result<Session, UiError> {
    session_over(mailbox(clock.now())?, clock)
}

/// [`session`] with the Hebrew and the Arabic message of the present sample, newest in the Personal inbox.
///
/// # Errors
///
/// As [`session`].
pub fn right_to_left(clock: Box<dyn Clock>) -> Result<Session, UiError> {
    let now = clock.now();
    let mut store = mailbox(now)?;
    let skip = MESSAGES.len().saturating_add(INVITATIONS.len());
    let mut batch = inboxcraft_store::Batch::new();
    for change in super::right_to_left_after(MESSAGES, skip, now)? {
        batch.push(change);
    }
    store.apply(&batch).map_err(UiError::sample)?;
    session_over(store, clock)
}

/// What the frames put away: two snoozed until today 16:00 and 18:00, two until Wednesday and Friday 08:00, three
/// pinned, two parked and three done.
const AWAY: Away = Away {
    snoozed: &[
        ("Kiln service: two dates to choose from", 116),
        ("Parking permit for the open day", 236),
        ("Glaze order, second batch", 2 * 24 * 60 - 6 * 60 - 4),
        ("Your warranty runs out in December", 4 * 24 * 60 - 6 * 60 - 4),
    ],
    pinned: &[
        "Workshop: what to bring",
        "Door code and opening hours",
        "Minutes of the quarterly studio planning meeting, with the kiln rota and the open day plan",
    ],
    parked: &["Spare keys for the studio", "Kiln rota for December"],
    done: &["Re: Invoice 2026-113", "Your order KC-20418", "Type Weekly #87"],
    returned: None,
};

/// The message in the outbox: Greta's proof, from the Studio account.
const WAITING_SEND: (&str, &str, &str) = ("Greta Lindqvist <greta@example.org>", "Thank-you cards: the proof", "The proof is attached.");

/// What the frames' outbox holds when the window is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outbox {
    /// Nothing: a window with no drafts.
    Empty,
    /// [`WAITING_SEND`], sent yesterday and on its way since, with no time of its own: the status line's "outbox 1".
    Waiting,
    /// [`WAITING_SEND`] to go on Tuesday at 08:00, as the Scheduled page lists it.
    Scheduled,
}

/// Sets the window's `mailbox` on `clock` as the frames show it: [`AWAY`] put away and the message `outbox` asks for;
/// nothing to undo. The clock comes back to its time.
///
/// # Errors
///
/// [`UiError`] when the session refuses one of the acts or commands.
pub fn prepare(mailbox: &mut crate::mailbox::Mailbox, clock: &FakeClock, outbox: Outbox) -> Result<(), UiError> {
    put_away_of(mailbox, &AWAY, MESSAGES)?;
    if outbox != Outbox::Empty {
        let now = clock.now();
        let queued = queue(mailbox, clock, now, outbox);
        clock.set(now);
        queued?;
    }
    mailbox.forget_undo();
    Ok(())
}

/// Writes [`WAITING_SEND`] yesterday and sends it: then, so when its hold is over the engine hands it to the account's
/// worker and, as nothing connects in a picture, it stays on its way with no time to go at (the status line says
/// "outbox 1", no "one goes at"); or, [`Outbox::Scheduled`], later, tomorrow at 08:00.
fn queue(mailbox: &mut crate::mailbox::Mailbox, clock: &FakeClock, now: Timestamp, outbox: Outbox) -> Result<(), UiError> {
    let written = now.checked_sub(std::time::Duration::from_secs(WRITTEN * 60)).ok_or_else(|| UiError::sample("no time"))?;
    clock.set(written);
    let (to, subject, text) = WAITING_SEND;
    let draft = mailbox.run("compose.new", &serde_json::json!({ "account": 2 }))?;
    let id = draft.get("id").and_then(serde_json::Value::as_u64).ok_or_else(|| UiError::sample("compose.new gave no draft"))?;
    mailbox.run("compose.update", &serde_json::json!({ "id": id, "to": [to], "subject": subject, "text": text }))?;
    if outbox == Outbox::Scheduled {
        let at = morning(now, 1).ok_or_else(|| UiError::sample("no morning"))?;
        mailbox.run("mail.send_later", &serde_json::json!({ "id": id, "at": at }))?;
        return Ok(());
    }
    mailbox.run("mail.send", &serde_json::json!({ "id": id }))?;
    // Past the hold (5 s unless chosen otherwise) and well before the engine counts the send late (a minute): it
    // goes on time, so no "sent late" notice follows.
    clock.set(written.checked_add(std::time::Duration::from_secs(HELD)).ok_or_else(|| UiError::sample("no time"))?);
    mailbox.pump(&mut crate::workers::NoWorkers);
    Ok(())
}

/// 08:00 UTC `days` after the day of `now`, as RFC 3339.
fn morning(now: Timestamp, days: i64) -> Option<String> {
    let day = now.unix_seconds().div_euclid(86_400).checked_add(days)?;
    let civil = Timestamp::from_unix(day.checked_mul(86_400)?.checked_add(8 * 3_600)?, 0)?.to_civil(inboxcraft_time::Offset::UTC)?;
    Some(format!("{:04}-{:02}-{:02}T08:00:00Z", civil.year(), civil.month(), civil.day()))
}

/// How long before the frames' time the message in the outbox was written and sent, in minutes: yesterday.
const WRITTEN: u64 = 22 * 60;

/// How many seconds after it was sent the message's hold is looked at: past the default hold of five.
const HELD: u64 = 20;

/// The screener's senders and the groups of the frames' showcase: three waiting, People and Notifications.
const WAITING: [Person; 3] = [LUCIAN, DEALS, NORA];
const GROUPS: [(IconId, &str, &[Person]); 2] =
    [(IconId::People, "People", &[TEODOR, MARIT, INES, PAVEL]), (IconId::Notify, "Notifications", &[FORGEHUB, PARCELWAY, BANKLINE, HOSTWELL])];

/// The frames' screener and groups over `mailbox`'s list, as [`super::showcase`] gives the present sample's.
#[must_use]
pub fn showcase(mailbox: &crate::mailbox::Mailbox) -> Showcase {
    showcase_of(mailbox, &WAITING, &GROUPS)
}

/// Schedules, for the Scheduled page: the invoice snoozed, a reminder on the bounced shelving message, the studio's
/// closing reminder sent on Friday at 09:00 and a glaze test tonight; as [`super::scheduled`] does for the present
/// sample.
///
/// # Errors
///
/// [`UiError`] when the session refuses one.
pub fn scheduled(mailbox: &mut crate::mailbox::Mailbox) -> Result<(), UiError> {
    let plan = Schedule {
        snooze: ("Invoice 2026-114", 7),
        reminder: (BOUNCED, 46),
        send: ("Tobias Wren <tobias@example.org>", "Reminder: the studio is closed on the 24th", 91),
        event: ("Glaze test", 6),
        account: 2,
    };
    scheduled_of(mailbox, &plan, MESSAGES)
}

#[cfg(test)]
#[path = "sample_frames_tests.rs"]
mod tests;
