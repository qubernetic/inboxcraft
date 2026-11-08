// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The bounce stage: delivery reports (RFC 3464 in an RFC 6522 `multipart/report`) about mail the owner sent.
//!
//! A report is told by its header ([`inboxcraft_model::Signs::report`]). It passes the screener: this stage runs
//! before it and names the report in the plan, so the screener neither holds it nor learns its sender. Its body is
//! fetched at once, unless it came with a folder's first sync (old mail) or into a folder whose mail teaches the
//! screener nothing, the Sent folder among them.
//!
//! Once the body is kept (`crate::sync`), [`linked`] reads it: the report names the message it is about by the
//! `Message-ID` in its returned header, and a message of that id in the Sent folder of the account the report came
//! to is marked as bounced, with the report and the reason (each recipient it failed for, with the remote server's
//! reply or the status code). Recipients only delayed, delivered or relayed mark nothing; nor does a report that
//! cannot be read or names no message we sent.

use inboxcraft_model::{Bounce, JobKind, Message, MessageRef, MessageState, Role};
use inboxcraft_store::{Change, Storage};

use super::Arrival;
use super::followup::planned;
use super::screener::SILENT;
use crate::EngineError;
use crate::sync::Plan;

/// The most characters of a bounce's reason kept.
pub(crate) const MAX_REASON: usize = 1000;

/// The stage: a delivery report passes the screener and its body is asked for.
pub(crate) fn stage(arrival: &Arrival<'_>, message: &Message, plan: &mut Plan) -> Result<(), EngineError> {
    if !message.summary().envelope().signs().report() {
        return Ok(());
    }
    plan.reports.insert(message.id());
    let ignored = arrival.role.is_some_and(|role| matches!(role, Role::Sent) || SILENT.contains(&role));
    if arrival.first_sync || ignored {
        return Ok(());
    }
    let Some(uid_validity) = plan.state.validity.get(&arrival.folder).copied() else { return Ok(()) };
    let Some(folder) = arrival.store.folders(arrival.account)?.into_iter().find(|folder| folder.id() == arrival.folder) else { return Ok(()) };
    let path = folder.listing().path().clone();
    plan.fetch(arrival.account, JobKind::FetchBody { folder: folder.id(), path, uid_validity, uid: message.summary().uid() }, message.id());
    Ok(())
}

/// The body `raw` of `report` is kept: the sent message it names is marked as bounced.
pub(crate) fn linked(store: &dyn Storage, plan: &mut Plan, report: &Message, raw: &[u8]) -> Result<(), EngineError> {
    if !report.summary().envelope().signs().report() {
        return Ok(());
    }
    let Ok(part) = inboxcraft_mime::parse(raw) else { return Ok(()) };
    let Ok(status) = part.delivery_status() else { return Ok(()) };
    let failed: Vec<String> = status
        .recipients
        .iter()
        .filter(|recipient| recipient.action == inboxcraft_mime::Action::Failed)
        .map(|recipient| {
            let why = recipient.diagnostic_code.as_ref().map_or_else(|| recipient.status.to_string(), |code| code.value.clone());
            format!("{}: {why}", recipient.final_recipient.value)
        })
        .collect();
    if failed.is_empty() {
        return Ok(());
    }
    let Some(about) = part.reported_message_id().as_deref().and_then(MessageRef::new) else { return Ok(()) };
    let Some(sent) = plan.state.accounts.get(&report.account()).and_then(|held| held.roles.folder(Role::Sent)) else { return Ok(()) };
    let reason: String = failed.join("; ").chars().take(MAX_REASON).collect();
    for entry in store.messages(sent)? {
        let message = entry.message();
        if message.summary().envelope().message_id() != Some(&about) {
            continue;
        }
        let state = planned(plan, message.id()).map_or_else(|| store.message_state(message.id()), Ok)?;
        let bounced = Some(Bounce { report: report.id(), reason: reason.clone() });
        plan.change(Change::SetMessageState(message.id(), MessageState { bounced, ..state }));
    }
    Ok(())
}

#[cfg(test)]
#[path = "bounce_tests.rs"]
mod tests;
