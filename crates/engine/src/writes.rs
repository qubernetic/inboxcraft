// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! One calendar write at a time for each event. A write (`Put` or `Delete`) names the entity tag of the copy it
//! replaces, and the server refuses it (412) when the copy has changed since. A second write to an event whose first
//! one is still out would name the tag from before the first, so the owner's own change would conflict with their
//! own earlier one: it waits here until the server has taken the first, then goes out with the tag the first one got.
//! A write the server took without sending its tag lets the next one go when a sync of its calendar brings the tag,
//! or with the last tag known when the account fails.

use std::collections::BTreeMap;

use inboxcraft_model::{AccountId, CalendarEventId, CalendarId, CalendarJob, CalendarReport, Event, EventKind, Job, JobId, JobKind};
use inboxcraft_store::Change;
use inboxcraft_time::Timestamp;

/// The calendar writes out to the workers, one for each event, and the ones waiting for them.
#[derive(Debug, Clone, Default)]
pub(crate) struct Writes {
    out: BTreeMap<CalendarEventId, Out>,
    /// Kept writes waiting for the write out to their event, in the order they came.
    waiting: Vec<Job>,
}

/// The write out to one event.
#[derive(Debug, Clone)]
struct Out {
    /// The job; `None` once the server took it without a tag, until a sync of its calendar brings the tag.
    job: Option<JobId>,
    account: AccountId,
    calendar: CalendarId,
    url: String,
}

/// What an event from a worker lets go.
#[derive(Debug, Default)]
pub(crate) struct Settled {
    /// The writes to hand to the workers now.
    pub(crate) jobs: Vec<Job>,
    /// The store's copies of the waiting and held writes that took a new tag.
    pub(crate) changes: Vec<Change>,
}

/// The event, calendar and URL a calendar write is about.
fn target(job: &Job) -> Option<(CalendarEventId, CalendarId, &str)> {
    match &job.kind {
        JobKind::Calendar(CalendarJob::Put { calendar, event, url, .. } | CalendarJob::Delete { calendar, event, url, .. }) => {
            Some((*event, *calendar, url))
        }
        _ => None,
    }
}

/// Whether `job` writes `event`.
fn writes(job: &Job, event: CalendarEventId) -> bool {
    target(job).is_some_and(|(of, ..)| of == event)
}

impl Writes {
    /// `job` to hand to its worker now, or `None` when it waits for the write out to its event.
    pub(crate) fn hand_out(&mut self, job: Job) -> Option<Job> {
        let Some((event, calendar, url)) = target(&job) else { return Some(job) };
        if self.out.contains_key(&event) {
            self.waiting.push(job);
            return None;
        }
        let out = Out { job: Some(job.id), account: job.account, calendar, url: url.to_owned() };
        self.out.insert(event, out);
        Some(job)
    }

    /// Whether a write to the event at `url` of `calendar` is out: the store holds the owner's copy, newer than the
    /// one a sync brings meanwhile.
    pub(crate) fn busy(&self, calendar: CalendarId, url: &str) -> bool {
        self.out.values().any(|out| out.calendar == calendar && out.url == url)
    }

    /// The writes waiting, in the order they came.
    pub(crate) fn waiting(&self) -> impl Iterator<Item = &Job> {
        self.waiting.iter()
    }

    /// Takes the waiting write `job` back.
    pub(crate) fn withdraw(&mut self, job: JobId) {
        self.waiting.retain(|waiting| waiting.id != job);
    }

    /// Forgets the writes of `account`, removed.
    pub(crate) fn forget(&mut self, account: AccountId) {
        self.out.retain(|_, out| out.account != account);
        self.waiting.retain(|job| job.account != account);
    }

    /// What `event` from a worker lets go: the write it ends, its tag given to the writes of the same event that
    /// wait or are `held`, and the next one of them handed out.
    pub(crate) fn settle(&mut self, held: &mut [(Timestamp, Job)], event: &Event) -> Settled {
        let mut settled = Settled::default();
        let ended = event.job.and_then(|job| self.out.iter().find(|(_, out)| out.job == Some(job)).map(|(id, _)| *id));
        if let Some(id) = ended {
            match &event.kind {
                EventKind::Calendar(CalendarReport::Written { etag: Some(tag), .. }) => {
                    self.retag(held, id, tag, &mut settled);
                    self.free(id, &mut settled);
                }
                EventKind::Calendar(CalendarReport::Written { etag: None, .. }) if self.follows(held, id) => {
                    if let Some(out) = self.out.get_mut(&id) {
                        out.job = None;
                    }
                }
                kind if crate::session::ends_job(kind) => self.free(id, &mut settled),
                _ => {}
            }
        }
        let awaiting = |out: &Out| out.job.is_none() && out.account == event.account;
        match &event.kind {
            EventKind::Calendar(CalendarReport::Synced { calendar, changed, .. }) => {
                let synced: Vec<(CalendarEventId, Option<String>)> = self
                    .out
                    .iter()
                    .filter(|(_, out)| awaiting(out) && out.calendar == *calendar)
                    .map(|(id, out)| (*id, changed.iter().find(|fetched| fetched.url == out.url).and_then(|fetched| fetched.etag.clone())))
                    .collect();
                for (id, tag) in synced {
                    if let Some(tag) = tag {
                        self.retag(held, id, &tag, &mut settled);
                    }
                    self.free(id, &mut settled);
                }
            }
            // The sync that would bring the tag may be the one that failed: the next write goes with the last tag known.
            EventKind::Calendar(CalendarReport::Failed { .. }) | EventKind::Failed { .. } => {
                let failed: Vec<CalendarEventId> = self.out.iter().filter(|(_, out)| awaiting(out)).map(|(id, _)| *id).collect();
                for id in failed {
                    self.free(id, &mut settled);
                }
            }
            _ => {}
        }
        settled
    }

    /// Whether a write to `event` waits here or is `held`.
    fn follows(&self, held: &[(Timestamp, Job)], event: CalendarEventId) -> bool {
        self.waiting.iter().chain(held.iter().map(|(_, job)| job)).any(|job| writes(job, event))
    }

    /// `tag` given to every write to `event` that waits here or is `held`, and to its copy in the store.
    fn retag(&mut self, held: &mut [(Timestamp, Job)], event: CalendarEventId, tag: &str, settled: &mut Settled) {
        for job in self.waiting.iter_mut().chain(held.iter_mut().map(|(_, job)| job)).filter(|job| writes(job, event)) {
            if let JobKind::Calendar(CalendarJob::Put { etag, .. } | CalendarJob::Delete { etag, .. }) = &mut job.kind {
                *etag = Some(tag.to_owned());
            }
            settled.changes.push(Change::QueueJob(job.clone()));
        }
    }

    /// The write out to `event` is over: the next one waiting goes out.
    fn free(&mut self, event: CalendarEventId, settled: &mut Settled) {
        self.out.remove(&event);
        let Some(next) = self.waiting.iter().position(|job| writes(job, event)) else { return };
        let job = self.waiting.remove(next);
        if let Some(job) = self.hand_out(job) {
            settled.jobs.push(job);
        }
    }
}
