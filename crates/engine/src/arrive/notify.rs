// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The notify stage: new mail that tells the owner, decided by `crate::notify` once the screener has had its say.

use inboxcraft_model::Message;

use super::Arrival;
use crate::EngineError;
use crate::sync::Plan;

/// The stage.
pub(crate) fn stage(arrival: &Arrival<'_>, message: &Message, plan: &mut Plan) -> Result<(), EngineError> {
    crate::notify::arrived(arrival, message, plan)
}
