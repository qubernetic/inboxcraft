// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The window's side of the owner requests ([`crate::owner_request`]): an agent's request for an `owner` command
//! waits in the window's queue, and runs, for the owner, only on the owner's answer.
//!
//! Nothing here runs a command but [`InboxApp::answer_owner_request`] with `allow`, which the window's Allow button
//! calls: no time out, no second request and no headless store is an approval.
//!
//! The request in front is asked in a dialog over the dimmed window ([`crate::component::dialog`]), in the app's
//! words ([`crate::owner_request::preview`]): it opens on Deny, `Esc` and a press on the dimmed window deny, and only
//! the owner's press of Allow once allows, and only from [`ARM_SECONDS`] after that question showed: a key or a press
//! on its way when it opened, or meant for the question before it, is no answer. It waits while the owner's own
//! dialog or a key's capture stands, and the keyboard goes back where it was when no question is left.

use std::time::Duration;

use egui::{Id, Ui};
use inboxcraft_engine::AgentClass;
use serde_json::Value;

use super::InboxApp;
use crate::component::dialog::{self, Answer, Dialog, Tone};
use crate::owner_request::{ARM_SECONDS, MOST_WAITING, MayRun, Settled, Ticket};
use crate::theme::Theme;
use crate::tr;

impl InboxApp {
    /// Queues an agent's request to run the `owner` command `command` with `params`, from the client that named
    /// itself `client`, for the owner to answer. `None` when [`MOST_WAITING`](crate::owner_request::MOST_WAITING)
    /// wait already, or when `command` is no `owner` command (a `safe` one needs no asking, a `never` one is never
    /// offered, an unknown one is none).
    pub fn ask_owner(&mut self, client: Option<&str>, command: &str, params: &Value) -> Option<Ticket> {
        self.ask_owner_if(client, command, params, Box::new(|| true))
    }

    /// As [`InboxApp::ask_owner`], for a request that runs on Allow only when `may_run` says so at that moment: the
    /// asker can withdraw it (it left, or gave up waiting) up to the last moment, and the Allow then runs nothing and
    /// answers nobody. `may_run` is asked once, right before the command would run.
    pub fn ask_owner_if(&mut self, client: Option<&str>, command: &str, params: &Value, may_run: Box<dyn Fn() -> bool + Send>) -> Option<Ticket> {
        let (command, class) = self.mailbox.class_of(command)?;
        if class != AgentClass::Owner {
            return None;
        }
        self.owner_requests.push_if(client, command, params.clone(), MayRun::new(may_run))
    }

    /// Takes the request `ticket` out of the queue unanswered and unrun, as when its asker left; nothing when it is
    /// not waiting.
    pub fn withdraw_owner_request(&mut self, ticket: Ticket) {
        self.owner_requests.withdraw(ticket);
    }

    /// The owner's answer to the request in front: with `allow`, its command runs for the owner, one step of the
    /// window's undo, and is settled with its answer or refusal; without, it is settled [`Settled::Denied`] and
    /// nothing runs. A request whose two minutes are over is settled [`Settled::TimedOut`] instead, and the answer
    /// goes to no other request: the one behind it was not what the owner saw.
    pub fn answer_owner_request(&mut self, allow: bool) {
        let Some(asked) = self.owner_requests.front().map(|front| front.ticket) else { return };
        self.expire_owner_requests();
        if self.owner_requests.front().map(|front| front.ticket) != Some(asked) {
            return;
        }
        if !allow {
            self.owner_requests.settle_front(Settled::Denied);
            return;
        }
        // The app's words for it, if it has none yet (it was never drawn): one that cannot be previewed is answered
        // with the engine's refusal, and the Allow goes to no other request.
        self.prepare_owner_question();
        if self.owner_requests.front().map(|front| front.ticket) != Some(asked) {
            return;
        }
        if !self.owner_requests.front().is_some_and(|front| front.may_run.check()) {
            // Withdrawn by its asker since it was drawn: the Allow runs nothing and answers nobody.
            self.owner_requests.withdraw(asked);
            return;
        }
        let Some((command, params, preview)) = self.owner_requests.front().map(|front| (front.command, front.params.clone(), front.preview.clone()))
        else {
            return;
        };
        let ran = self.mailbox.run_for_owner_request(command, &params, |answer| match &preview {
            Some(preview) => preview.done(answer),
            None => tr!("An agent ran {command}", command = command),
        });
        self.owner_requests.settle_front(Settled::Ran(ran));
    }

    /// Builds the app's words for the request in front when it has none yet ([`crate::owner_request::preview`]); one
    /// the engine refuses to preview is settled with that refusal, and the next one is tried. Whether one waits, with
    /// its words.
    pub(crate) fn prepare_owner_question(&mut self) -> bool {
        for _ in 0..=MOST_WAITING {
            let Some(front) = self.owner_requests.front() else { return false };
            if front.preview.is_some() {
                return true;
            }
            match crate::owner_request::preview(&mut self.mailbox, front) {
                Ok(preview) => {
                    if let Some(front) = self.owner_requests.front_mut() {
                        front.preview = Some(preview);
                    }
                }
                Err(error) => self.owner_requests.settle_front(Settled::Ran(Err(error))),
            }
        }
        self.owner_requests.front().is_some_and(|front| front.preview.is_some())
    }

    /// The request the owner is asked about and its question, in the app's words, for a notification while the
    /// window is not in front; `None` when none waits.
    pub fn owner_question_text(&mut self) -> Option<(Ticket, String)> {
        if !self.prepare_owner_question() {
            return None;
        }
        let front = self.owner_requests.front()?;
        Some((front.ticket, front.preview.as_ref()?.question.clone()))
    }

    /// Whether the question of the request in front shows this frame: one waits, and neither the owner's own dialog
    /// nor a key's capture stands.
    pub(super) fn owner_question_shows(&self) -> bool {
        self.owner_requests.waiting() && !self.settings.confirms(self.onboarding.asking_remove()) && self.settings.capturing().is_none()
    }

    /// Draws the question of the request in front, as the module says, and takes the owner's answer to it.
    pub(super) fn owner_question(&mut self, ui: &Ui, theme: &Theme) {
        if !self.owner_question_shows() {
            if !self.owner_requests.waiting() {
                self.give_back_keyboard(ui.ctx());
            }
            return;
        }
        if !self.prepare_owner_question() {
            self.give_back_keyboard(ui.ctx());
            return;
        }
        let now = ui.input(|input| input.time);
        if self.owner_keyboard.is_none() {
            self.owner_keyboard = Some(ui.ctx().memory(|memory| memory.focused()));
        }
        let Some(front) = self.owner_requests.front_mut() else { return };
        // A clock that went back (a window opened again) starts the arming second again; it never shortens it.
        let shown_at = *front.shown_at.insert(front.shown_at.filter(|at| *at <= now).unwrap_or(now));
        let Some(preview) = front.preview.as_ref() else { return };
        let lines: Vec<(&str, Tone)> = preview.lines.iter().map(|(text, tone)| (text.as_str(), *tone)).collect();
        let shown = Dialog {
            id: Id::unique("owner-request").with(front.ticket),
            question: &preview.question,
            lines: &lines,
            keep: tr!("Deny"),
            act: tr!("Allow once"),
            near: None,
        };
        let answer = dialog::show(ui, theme, &self.keymap, &shown);
        let shown_for = now - shown_at;
        let armed = shown_for >= ARM_SECONDS;
        // The arming second and the two minutes pass without input.
        if !armed {
            ui.ctx().request_repaint_after(Duration::from_secs_f64((ARM_SECONDS - shown_for).clamp(0.0, ARM_SECONDS)));
        }
        // (One not dated yet is dated by the desktop's next look, which asks for its own frame.)
        if let Some(left) = self.owner_request_due_in().filter(|left| !left.is_zero()) {
            ui.ctx().request_repaint_after(left);
        }
        match answer {
            Some(Answer::Keep) => self.answer_owner_request(false),
            Some(Answer::Act) if armed => self.answer_owner_request(true),
            Some(Answer::Act) | None => {}
        }
        // The keyboard goes back on the next frame, the dialog's modal layer gone.
        if answer.is_some() {
            ui.ctx().request_repaint();
        }
    }

    /// Gives the keyboard back to where it was when the first question showed, once none is left: once the press that
    /// answered it is let go, so the press on the dimmed window takes nothing away.
    fn give_back_keyboard(&mut self, ctx: &egui::Context) {
        let (down, released) = ctx.input(|input| (input.pointer.any_down(), input.pointer.any_released()));
        if self.owner_keyboard.is_some() && (down || released) {
            // A release draws a frame of its own, and the one after it gives the keyboard back.
            if released {
                ctx.request_repaint();
            }
            return;
        }
        if let Some(had) = self.owner_keyboard.take()
            && let Some(id) = had
        {
            ctx.memory_mut(|memory| memory.request_focus(id));
        }
    }

    /// Settles the request in front as [`Settled::TimedOut`] once the owner has not answered it for two minutes, by
    /// the mailbox's clock; it starts the two minutes of a request that has just come to the front.
    pub fn expire_owner_requests(&mut self) {
        let now = self.mailbox.now();
        self.owner_requests.expire(now);
    }

    /// How long until the request in front runs out of time, so the window looks again then; `None` when none
    /// waits.
    #[must_use]
    pub fn owner_request_due_in(&self) -> Option<Duration> {
        self.owner_requests.due_in(self.mailbox.now())
    }

    /// The requests settled since the last call, oldest first, for their askers' answers.
    pub fn take_settled(&mut self) -> Vec<(Ticket, Settled)> {
        self.owner_requests.take_settled()
    }

    /// Whether a request waits for the owner's answer.
    #[must_use]
    pub fn asks_owner(&self) -> bool {
        self.owner_requests.waiting()
    }
}

#[cfg(test)]
#[path = "owner_request_tests.rs"]
mod tests;
