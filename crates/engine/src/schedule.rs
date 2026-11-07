// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The scheduler: the entries of the store's schedule fire at their time, and the time presets in the owner's
//! wall-clock time.
//!
//! An entry ([`Scheduled`]) is due at a moment and names what to do: send a draft, bring a snoozed message back,
//! remind of a reply that did not come, show a calendar alert. [`crate::Session::tick`] fires every entry whose
//! time has come, on the session's clock: the entry leaves the schedule, a returned snooze or a fired reminder
//! clears its time in the message's state, and the session hands the entry back as a [`Fired`] event, `missed` when
//! its time passed while InboxCraft was off or asleep. A snoozed message still in the snoozed folder comes back to the
//! inbox then, unread and marked as returned (`crate::arrive::snooze`). Firing an entry twice cannot happen: it leaves
//! the schedule in the same batch.

use std::collections::BTreeMap;
use std::time::Duration;

use inboxcraft_model::{Due, MessageId, MessageState, Scheduled};
use inboxcraft_store::{Batch, Change};
use inboxcraft_time::{CivilDateTime, Offset, Offsets, Timestamp};

use crate::act::{Effects, Mover};
use crate::arrive::snooze;
use crate::session::Pending;
use crate::{Context, EngineError};

/// An entry fired later than this after its time was missed: InboxCraft was off, or the computer was asleep.
pub const LATE: Duration = Duration::from_secs(60);

/// The hour of "tomorrow morning".
pub const MORNING: u8 = 8;

/// The hour of "this evening".
pub const EVENING: u8 = 20;

/// How far ahead "later today" is, before it is rounded.
const LATER: Duration = Duration::from_secs(3 * 3600);

/// "Later today" is rounded to the nearest half hour of the wall clock.
const HALF_HOUR: i64 = 1800;

const DAY: Duration = Duration::from_secs(86_400);

/// A time the owner can pick with one click.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Preset {
    /// Three hours from now, to the nearest half hour, while that is still today.
    LaterToday,
    /// Today at [`EVENING`].
    ThisEvening,
    /// Tomorrow at [`MORNING`].
    TomorrowMorning,
}

impl Preset {
    /// Every preset, in the order a picker shows them.
    pub const ALL: [Self; 3] = [Self::LaterToday, Self::ThisEvening, Self::TomorrowMorning];

    /// The preset's name in commands: `later_today`, `this_evening`, `tomorrow_morning`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::LaterToday => "later_today",
            Self::ThisEvening => "this_evening",
            Self::TomorrowMorning => "tomorrow_morning",
        }
    }

    /// The moment the preset stands for at `now`, when `today` is the wall clock's date and time at `now`.
    fn at(self, now: Timestamp, today: CivilDateTime, offsets: &dyn Offsets) -> Option<Timestamp> {
        match self {
            Self::LaterToday => {
                // Rounded on the wall clock at the offset of the moment three hours on, so the result is three real
                // hours away (give or take the rounding) even when the clock is put forward or back between.
                let target = now.checked_add(LATER)?;
                let offset = i64::from(offsets.offset_at(target).minutes()).checked_mul(60)?;
                let local = target.unix_seconds().checked_add(offset)?;
                let rounded = local.checked_add(HALF_HOUR / 2)?.div_euclid(HALF_HOUR).checked_mul(HALF_HOUR)?;
                let shown = Timestamp::from_unix(rounded, 0)?.to_civil(Offset::UTC)?;
                let same_day = (shown.year(), shown.month(), shown.day()) == (today.year(), today.month(), today.day());
                same_day.then(|| Timestamp::from_unix(rounded.checked_sub(offset)?, 0)).flatten()
            }
            Self::ThisEvening => offsets.resolve(CivilDateTime::new(today.year(), today.month(), today.day(), EVENING, 0, 0)?),
            Self::TomorrowMorning => {
                let noon = CivilDateTime::new(today.year(), today.month(), today.day(), 12, 0, 0)?.to_timestamp(Offset::UTC)?;
                let tomorrow = noon.checked_add(DAY)?.to_civil(Offset::UTC)?;
                offsets.resolve(CivilDateTime::new(tomorrow.year(), tomorrow.month(), tomorrow.day(), MORNING, 0, 0)?)
            }
        }
    }
}

/// The presets ahead of `now` in the wall-clock time of `offsets`, in [`Preset::ALL`] order; one already behind, or
/// "later today" when it falls tomorrow, is left out.
#[must_use]
pub fn presets(now: Timestamp, offsets: &dyn Offsets) -> Vec<(Preset, Timestamp)> {
    let Some(today) = offsets.civil(now) else { return Vec::new() };
    Preset::ALL.into_iter().filter_map(|preset| preset.at(now, today, offsets).filter(|at| *at > now).map(|at| (preset, at))).collect()
}

/// A schedule entry whose time came: the event the front end and the later stages act on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fired {
    /// The entry, now gone from the schedule.
    pub entry: Scheduled,
    /// Its time passed while InboxCraft was off (before [`crate::Session::start`]) or more than [`LATE`] ago: the owner
    /// is told it happened late.
    pub missed: bool,
}

/// What a tick does: the store changes, the kept jobs, and the entries it fires.
pub(crate) struct Plan {
    pub(crate) batch: Batch,
    pub(crate) jobs: Vec<Pending>,
    pub(crate) fired: Vec<Fired>,
}

/// Fires every entry due at the context's time, in time order; `started` is when the session started.
pub(crate) fn due(context: &Context<'_>, started: Option<Timestamp>) -> Result<Plan, EngineError> {
    let now = context.now();
    let store = context.store();
    let mut batch = Batch::new();
    let mut states: BTreeMap<MessageId, MessageState> = BTreeMap::new();
    let mut returning = Vec::new();
    let mut fired = Vec::new();
    for entry in store.schedule()?.into_iter().filter(|entry| entry.due <= now) {
        batch.push(Change::RemoveScheduled(entry.id));
        if let Due::Unsnooze(message) | Due::FollowUp(message) = entry.what {
            let held = match states.get(&message) {
                Some(state) => state.clone(),
                None => store.message_state(message)?,
            };
            let state = cleared(held.clone(), &entry);
            if state != held {
                if matches!(entry.what, Due::Unsnooze(_)) {
                    returning.push(message);
                }
                states.insert(message, state);
            }
        }
        let late = now.duration_since(entry.due).is_some_and(|late| late > LATE);
        let missed = late || started.is_some_and(|started| entry.due < started);
        fired.push(Fired { entry, missed });
    }
    let mut effects = Effects::default();
    let mut mover = Mover::default();
    for id in returning {
        if let Some(entry) = store.message(id)?
            && snooze::is_snoozed(context.state(), entry.message())
            && let Some(state) = states.get_mut(&id)
        {
            snooze::back(store, context.state(), entry.message(), &mut mover, &mut effects)?;
            state.returned = Some(now);
        }
    }
    for change in effects.changes {
        batch.push(change);
    }
    for (message, state) in states {
        batch.push(Change::SetMessageState(message, state));
    }
    let jobs = effects.jobs.into_iter().map(|(account, kind)| Pending { account, kind, message: None, keep: true }).collect();
    Ok(Plan { batch, jobs, fired })
}

/// `state` without the time `entry` stood for: a message snoozed or followed up again for another time keeps it.
fn cleared(mut state: MessageState, entry: &Scheduled) -> MessageState {
    match entry.what {
        Due::Unsnooze(_) if state.snoozed_until == Some(entry.due) => state.snoozed_until = None,
        Due::FollowUp(_) if state.follow_up == Some(entry.due) => state.follow_up = None,
        _ => {}
    }
    state
}

#[cfg(test)]
#[path = "schedule_tests.rs"]
mod tests;
