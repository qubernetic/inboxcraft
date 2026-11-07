// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Every engine command on any JSON parameters (the CLI, the control channel and MCP hand them in): none panics, so
//! the session's panic net is never used, and every refusal is a plain error.

#![no_main]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use inboxcraft_engine::{Caller, EngineError, Session};
use inboxcraft_model::{
    Account, AccountId, EmailAddress, Envelope, Folder, FolderId, FolderListing, FolderPath, Host, Message, MessageId, Security, Server, Summary,
    Uid,
};
use inboxcraft_store::{Batch, Change, Memory, Storage, StoreConfig};
use inboxcraft_time::{FakeClock, Timestamp};
use libfuzzer_sys::fuzz_target;

/// One account with an inbox and one message, so ids 1 name something.
fn mailbox() -> Option<Memory> {
    let imap = Server::new(Host::new("imap.example.org").ok()?, Security::Tls.imap_port(), Security::Tls);
    let account = Account::new(AccountId::new(1), "Home", EmailAddress::new("anna@example.org").ok()?, "anna", imap).ok()?;
    let inbox = Folder::new(FolderId::new(1), AccountId::new(1), FolderListing::new(FolderPath::new("INBOX", Some('/')).ok()?, vec![], true));
    let summary = Summary::new(Uid::new(1)?, Envelope::new("Hello")).with_received(Timestamp::from_unix(1_000_000, 0));
    let message = Message::new(MessageId::new(1), AccountId::new(1), FolderId::new(1), summary);
    let mut store = Memory::new(StoreConfig::default());
    let batch = Batch::new().with(Change::PutAccount(account)).with(Change::PutFolder(inbox)).with(Change::PutMessage(message));
    store.apply(&batch).ok()?;
    Some(store)
}

fuzz_target!(|data: &[u8]| {
    let Ok(params) = serde_json::from_slice::<serde_json::Value>(data) else { return };
    let Some(store) = mailbox() else { return };
    let Ok(mut session) = Session::new(Box::new(store), Box::new(FakeClock::new(Timestamp::UNIX_EPOCH))) else { return };
    let ids: Vec<&'static str> = session.registry().specs().map(|spec| spec.id).collect();
    for id in ids {
        for caller in [Caller::Owner, Caller::Agent] {
            let result = session.run_as(caller, id, &params);
            assert!(!matches!(result, Err(EngineError::Panicked(_))), "{id} panicked");
        }
    }
});
