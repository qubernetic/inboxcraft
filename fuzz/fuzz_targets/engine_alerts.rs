// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Event alerts: any bytes a calendar sync brings as an event's iCalendar text are read for its alerts when it comes,
//! when the session starts and at every sweep, and its alerts fire and tell the owner as the clock moves on: none
//! panics, so the session's panic net is never used.

#![no_main]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::time::Duration;

use inboxcraft_engine::{EngineError, Session};
use inboxcraft_model::{
    Account, AccountId, Calendar, CalendarId, CalendarReport, EmailAddress, Event, EventKind, FetchedEvent, Host, Security, Server,
};
use inboxcraft_store::{Batch, Change, Memory, Storage, StoreConfig};
use inboxcraft_time::{FakeClock, Offset, OffsetTable, Timestamp};
use libfuzzer_sys::fuzz_target;

const HOME: &str = "https://dav.example.org/calendars/anna/home/";

/// One account with a calendar.
fn calendar() -> Option<Memory> {
    let imap = Server::new(Host::new("imap.example.org").ok()?, Security::Tls.imap_port(), Security::Tls);
    let account =
        Account::new(AccountId::new(1), "Home", EmailAddress::new("anna@example.org").ok()?, "anna", imap).ok()?.with_caldav(Some("https://dav.example.org/")).ok()?;
    let calendar = Calendar { id: CalendarId::new(1), account: AccountId::new(1), href: HOME.to_owned(), name: "Home".to_owned(), colour: None };
    let mut store = Memory::new(StoreConfig::default());
    store.apply(&Batch::new().with(Change::PutAccount(account)).with(Change::PutCalendar(calendar))).ok()?;
    Some(store)
}

/// A clock that is put forward an hour in spring and back in autumn 2026.
fn summer() -> Option<OffsetTable> {
    let at = |seconds: i64| Timestamp::from_unix(seconds, 0);
    Some(OffsetTable::new(Offset::from_minutes(60)?).with_change(at(1_774_746_000)?, Offset::from_minutes(120)?).with_change(at(1_792_890_000)?, Offset::from_minutes(60)?))
}

fn checked<T>(result: Result<T, EngineError>, what: &str) {
    assert!(!matches!(result, Err(EngineError::Panicked(_))), "{what} panicked");
}

fuzz_target!(|data: &[u8]| {
    let Some(store) = calendar() else { return };
    let Some(start) = Timestamp::from_unix(1_792_000_000, 0) else { return };
    let clock = FakeClock::new(start);
    let Ok(mut session) = Session::new(Box::new(store), Box::new(clock.clone())) else { return };
    if let Some(summer) = summer() {
        session.set_offsets(Box::new(summer));
    }
    let ical = String::from_utf8_lossy(data).into_owned();
    let fetched = FetchedEvent { url: format!("{HOME}e.ics"), etag: Some("\"1\"".to_owned()), ical };
    let synced = CalendarReport::Synced { calendar: CalendarId::new(1), changed: vec![fetched], removed: vec![], token: None, full: true };
    checked(session.apply(&Event::new(AccountId::new(1), None, EventKind::Calendar(synced))), "the sync");
    session.start();
    for step in [600, 3600, 86_400, 86_400, 15 * 86_400] {
        clock.advance(Duration::from_secs(step));
        checked(session.tick(), "a tick");
        drop(session.take_notices());
    }
});
