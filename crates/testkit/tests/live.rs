// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The opt-in live tests against the owner's two throwaway test accounts (`just it-live`, profile `live`).
//!
//! Every test here is named `live_*` and starts with [`live::start`]: without the `INBOXCRAFT_LIVE_*` credentials,
//! and always in the agent sandbox and in CI, it prints the skip reason and the test passes. Only the owner runs
//! them with credentials. The IMAP tests live with `net` (`crates/net/tests/live.rs`): both accounts sign in and list
//! their folders, and the round trip on `live-imap`; SMTP and CalDAV join them there.

use inboxcraft_testkit::live;

#[test]
fn live_configuration_is_complete() -> Result<(), Box<dyn std::error::Error>> {
    let Some(config) = live::start("live_configuration_is_complete")? else {
        return Ok(());
    };
    let [imap, imap2] = config.accounts();
    config.check_recipient(imap.address())?;
    config.check_recipient(imap2.address())?;
    println!("live: accounts {} and {}, run folder {}", imap.name(), imap2.name(), config.run_folder('/')?);
    Ok(())
}
