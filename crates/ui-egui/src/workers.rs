// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! How the window reaches the `net` workers: through [`Workers`] the app hands in.
//!
//! The front end opens no connection of its own. Each frame, [`crate::mailbox::Mailbox::pump`] hands the engine's
//! queued jobs to the worker of their account and applies the events that came back. The desktop app gives workers
//! over `net`, which read each password from the OS credential store; the sample and the tests give [`NoWorkers`],
//! which runs nothing.

use inboxcraft_model::{Account, AccountId, CheckStep, Event, EventKind, Job, JobKind, StepReport, StepStatus};

/// The `net` workers, one per account.
pub trait Workers: Send {
    /// Hands `job` to the worker of `account`, starting the worker first when there is none.
    fn send(&mut self, account: &Account, job: Job);

    /// Stops every worker whose account is not in `accounts`.
    fn keep(&mut self, accounts: &[AccountId]);

    /// The events that came since the last call, in order.
    fn events(&mut self) -> Vec<Event>;
}

/// Workers that connect nowhere and answer each Check job from a script: the steps given, with the account's server
/// and `issuer` on the certificate step; the other built steps skipped (or left pending with
/// [`Scripted::unfinished`]), the steps not built yet as such. Every other job is only written down. For the
/// `ui_shot` pictures and the tests.
#[derive(Debug, Default, Clone)]
pub struct Scripted {
    steps: Vec<(CheckStep, StepStatus)>,
    issuer: Option<String>,
    unfinished: bool,
    /// Every job handed over, by account, in order.
    pub sent: Vec<(AccountId, JobKind)>,
    /// What [`Workers::keep`] was last told.
    pub kept: Vec<AccountId>,
    events: Vec<Event>,
}

impl Scripted {
    /// Workers that answer every check with `steps`.
    #[must_use]
    pub fn answering(steps: &[(CheckStep, StepStatus)]) -> Self {
        Self { steps: steps.to_vec(), issuer: Some("Example Test CA".to_owned()), ..Self::default() }
    }

    /// Workers whose checks pass every built step.
    #[must_use]
    pub fn passing() -> Self {
        Self::answering(&CheckStep::ALL.into_iter().filter(|step| step.is_built()).map(|step| (step, StepStatus::Passed)).collect::<Vec<_>>())
            .without(CheckStep::SmtpLogin)
    }

    /// The same workers, answering `step` as a worker would on its own: an account without a submission server has no
    /// SMTP sign-in to pass.
    #[must_use]
    pub fn without(mut self, step: CheckStep) -> Self {
        self.steps.retain(|(scripted, _)| *scripted != step);
        self
    }

    /// The same workers, reporting only the scripted steps: the check stays running.
    #[must_use]
    pub fn unfinished(mut self) -> Self {
        self.unfinished = true;
        self
    }
}

impl Workers for Scripted {
    fn send(&mut self, account: &Account, job: Job) {
        self.sent.push((account.id(), job.kind.clone()));
        if !matches!(job.kind, JobKind::Check { .. }) {
            return;
        }
        let server = account.imap();
        for step in CheckStep::ALL {
            let status = match self.steps.iter().find(|(scripted, _)| *scripted == step) {
                Some((_, status)) => *status,
                None if self.unfinished => continue,
                None if step == CheckStep::SmtpLogin && account.submission().is_none() => StepStatus::NotSet,
                None if step.is_built() => StepStatus::Skipped,
                None => StepStatus::NotBuilt,
            };
            let issuer = self.issuer.as_deref().filter(|_| step == CheckStep::Certificate && status != StepStatus::Skipped);
            let report = StepReport::new(step, status).with_server(server.host().clone(), server.port().get()).with_issuer(issuer);
            self.events.push(Event::new(account.id(), Some(job.id), EventKind::CheckStep(report)));
        }
    }

    fn keep(&mut self, accounts: &[AccountId]) {
        accounts.clone_into(&mut self.kept);
    }

    fn events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }
}

/// The workers of a window that connects nowhere: jobs are dropped and no event comes.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoWorkers;

impl Workers for NoWorkers {
    fn send(&mut self, _account: &Account, _job: Job) {}

    fn keep(&mut self, _accounts: &[AccountId]) {}

    fn events(&mut self) -> Vec<Event> {
        Vec::new()
    }
}
