// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The opt-in live tests against the owner's two throwaway test accounts (`just it-live`, profile `live`).
//!
//! Every test here is named `live_*` and starts with [`live::start`]: without the `INBOXCRAFT_LIVE_*` credentials,
//! and always in the agent sandbox and in CI, it prints the skip reason and the test passes. Only the owner runs
//! them with credentials. The IMAP round trip lives with `net` (`crates/net/tests/live.rs`); SMTP and CalDAV join it
//! there.

use inboxcraft_testkit::live;

#[test]
fn live_configuration_is_complete() -> Result<(), Box<dyn std::error::Error>> {
    let Some(config) = live::start("live_configuration_is_complete")? else {
        return Ok(());
    };
    let [imap, google] = config.accounts();
    config.check_recipient(imap.address())?;
    config.check_recipient(google.address())?;
    println!("live: accounts {} and {}, run folder {}", imap.name(), google.name(), config.run_folder('/')?);
    Ok(())
}
