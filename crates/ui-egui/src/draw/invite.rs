// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The invitation card over a message's text (`crate::invite`), drawn by [`invite_card`]: the day, the title, WHEN,
//! WHERE, WHO and CAL, then by what the invitation does:
//!
//! - an invitation: Going (the card's one primary button), Maybe and No, and to whom the answer goes at once, under
//!   what changed since the version before; once answered, the answer and Change answer, which offers the three again
//!   with Keep my answer; one that cannot be answered says why;
//! - a cancellation: that the event is off, and Remove from calendar while the event is in it, or a line under the
//!   card that it is not in the calendar;
//! - a guest's answer: what each guest said, the first 20 and how many more;
//! - anything else: that it asks for no answer.
//!
//! A version a later one replaced is one line; the line about the version before stands under the card. Every word
//! of the event is the organiser's data, shown flat and never acted on; the buttons are the owner's own commands
//! (`invite.answer` sends mail and is the owner's to press).

use egui::{Frame, RichText, Shape, Stroke, StrokeKind, Ui};

use super::margin;
use super::reader::{Asked, Drawn};
use crate::component::invite_card::{CardAsked, CardEvent, CardState, Said, invite_card, invite_card_with};
use crate::invite::{Invite, Method, Rsvp};
use crate::keymap::Keymap;
use crate::reader::Reader;
use crate::theme::Theme;
use crate::tr;

/// What the owner asked on a card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ask {
    /// Send this answer.
    Answer(Rsvp),
    /// Offer the answers again, or no longer.
    Change,
    /// Take the cancelled event out of the calendar.
    Remove,
}

/// A card across `ui`'s width with `add` drawn inside, on the `surface` in a 1 px `cyan_edge`: it asks the owner
/// something.
pub(super) fn asking<R>(ui: &mut Ui, theme: &Theme, add: impl FnOnce(&mut Ui) -> R) -> R {
    let p = &theme.palette;
    let slot = ui.painter().add(Shape::Noop);
    let inner = Frame::new().inner_margin(margin(theme.spacing.s4)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        add(ui)
    });
    let rect = inner.response.rect;
    let shapes = vec![Shape::rect_filled(rect, 0.0, p.surface), Shape::rect_stroke(rect, 0.0, Stroke::new(1.0, p.cyan_edge), StrokeKind::Inside)];
    ui.painter().set(slot, Shape::Vec(shapes));
    inner.inner
}

/// The most guests' answers a card of replies lists one by one.
const MOST_REPLIES: usize = 20;

/// The words of an event the card shows, owned.
struct Words {
    weekday: String,
    day: String,
    month: String,
    title: String,
    when: String,
    place: String,
    who: String,
}

impl Words {
    fn of(invite: &Invite) -> Self {
        let title = if invite.event.summary.is_empty() { tr!("(no title)").to_owned() } else { invite.event.summary.clone() };
        let who = invite.organizer.as_ref().map(|organizer| tr!("{organizer} (organiser)", organizer = organizer)).unwrap_or_default();
        Self {
            weekday: invite.weekday(),
            day: invite.day(),
            month: invite.month(),
            title,
            when: invite.card_when(),
            place: invite.event.location.clone(),
            who,
        }
    }

    fn event(&self) -> CardEvent<'_> {
        CardEvent {
            weekday: &self.weekday,
            day: &self.day,
            month: &self.month,
            title: &self.title,
            when: &self.when,
            place: &self.place,
            who: &self.who,
            cal: "",
        }
    }
}

/// What a guest said, as the card marks it.
fn said(answer: Option<Rsvp>) -> Said {
    match answer {
        Some(Rsvp::Going) => Said::Going,
        Some(Rsvp::Maybe) => Said::Maybe,
        Some(Rsvp::No) => Said::No,
        None => Said::Nothing,
    }
}

/// Whether the card of `invite` asks the owner for an answer: its Going is then the pane's one primary button.
pub(super) fn asks(reader: &Reader, invite: &Invite) -> bool {
    !invite.outdated && invite.method == Method::Request && invite.can_answer && (invite.answer.is_none() || reader.changing(invite.message))
}

/// The card of `invite`, with the line about the version before it.
pub(super) fn card(ui: &mut Ui, theme: &Theme, keymap: &Keymap, reader: &Reader, invite: &Invite, drawn: &mut Drawn) {
    let words = Words::of(invite);
    let event = words.event();
    if invite.outdated {
        let line = match invite.answered() {
            Some(answered) => tr!("Out of date: a later invitation replaces this one. {answered}", answered = answered),
            None => tr!("Out of date: a later invitation replaces this one.").to_owned(),
        };
        invite_card(ui, theme, keymap, &event, &CardState::OutOfDate { line: &line });
        return;
    }
    let goes_to = invite.organizer_address.clone().unwrap_or_default();
    let changing = reader.changing(invite.message);
    // The engine's reason, as a sentence.
    let why = invite.why_not.as_deref().map(|why| if why.ends_with('.') { why.to_owned() } else { format!("{why}.") }).unwrap_or_default();
    let answered = invite.answered().unwrap_or_default();
    // The box's label says "Changed": its words are what changed.
    let changed = invite.changed().map(|changed| changed.strip_prefix(tr!("Changed: ")).map_or_else(|| changed.clone(), str::to_owned));
    let cancelled = match &invite.organizer {
        Some(organizer) => tr!("{organizer} cancelled this event.", organizer = organizer),
        None => tr!("This event is cancelled.").to_owned(),
    };
    let lines: Vec<String> = invite.replies.iter().take(MOST_REPLIES).map(Invite::reply_line).collect();
    let more = invite.replies.len().saturating_sub(MOST_REPLIES);
    let more_line = tr!("{count} more guests answered.", count = more);
    let mut replies: Vec<(Said, &str)> = invite.replies.iter().zip(&lines).map(|(reply, line)| (said(reply.answer), line.as_str())).collect();
    if more > 0 {
        replies.push((Said::Nothing, &more_line));
    }
    let state = match invite.method {
        Method::Request if !invite.can_answer => CardState::CannotAnswer { why: &why },
        Method::Request if invite.answer.is_some() && !changing => CardState::Answered { answer: &answered },
        Method::Request => match &changed {
            Some(changed) => CardState::Changed { changed, goes_to: &goes_to },
            None => CardState::Request { goes_to: &goes_to },
        },
        Method::Cancel => CardState::Cancelled { line: &cancelled, remove: invite.can_remove },
        Method::Reply => CardState::Replies { replies: &replies },
        Method::Other => CardState::NoAnswerAsked,
    };
    let ask = |drawn: &mut Drawn, ask: Ask| drawn.asked.push(Asked::Invite(invite.message, ask));
    match invite_card_with(ui, theme, keymap, &event, &state, changing) {
        Some(CardAsked::Answer(index)) => {
            if let Some(answer) = Rsvp::ALL.get(index) {
                ask(drawn, Ask::Answer(*answer));
            }
        }
        Some(CardAsked::Change | CardAsked::Keep) => ask(drawn, Ask::Change),
        Some(CardAsked::Remove) => ask(drawn, Ask::Remove),
        None => {}
    }
    if invite.method == Method::Cancel && !invite.can_remove && invite.calendar_event.is_none() {
        ui.add_space(theme.spacing.s2);
        let note = RichText::new(tr!("It is not in your calendar.")).font(theme.fonts.caption.id()).color(theme.palette.muted);
        ui.add(egui::Label::new(note).truncate());
    }
    if let Some(line) = invite.earlier_line() {
        ui.add_space(theme.spacing.s2);
        invite_card(ui, theme, keymap, &event, &CardState::OutOfDate { line: &line });
    }
}
