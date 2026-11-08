// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The MCP server on any input: every line gets one JSON answer or none, nothing panics, and no tool's command
//! panics into the session's net.

#![no_main]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use inboxcraft_engine::Session;
use inboxcraft_mcp::Server;
use inboxcraft_model::{
    Account, AccountId, EmailAddress, Envelope, Folder, FolderId, FolderListing, FolderPath, Host, Message, MessageId, Security, Server as Imap,
    Summary, Uid,
};
use inboxcraft_store::{Batch, Change, Memory, Storage, StoreConfig};
use inboxcraft_time::{FakeClock, Timestamp};
use libfuzzer_sys::fuzz_target;

/// One account with an inbox and one message, so ids 1 name something.
fn mailbox() -> Option<Memory> {
    let imap = Imap::new(Host::new("imap.example.org").ok()?, Security::Tls.imap_port(), Security::Tls);
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
    let Some(store) = mailbox() else { return };
    let Ok(session) = Session::new(Box::new(store), Box::new(FakeClock::new(Timestamp::UNIX_EPOCH))) else { return };
    let mut server = Server::new(session);
    let mut output = Vec::new();
    if server.serve(&mut &data[..], &mut output).is_err() {
        return;
    }
    for line in output.split(|byte| *byte == b'\n').filter(|line| !line.is_empty()) {
        let answer = serde_json::from_slice::<serde_json::Value>(line);
        assert!(answer.is_ok(), "an answer that is not JSON");
        assert!(!String::from_utf8_lossy(line).contains("\"error panicked"), "a tool panicked");
    }
});
