// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The invented sample mailbox behind `--sample`: two accounts of one invented person, their folders and a few
//! dozen messages, all on `example.org`.
//!
//! Every name, address, subject and preview here is made up for InboxCraft. The dates count back from the `now`
//! given, so the sample always looks recent. Nothing in it connects anywhere: the accounts point at
//! `imap.example.org`, and the sample session never starts their workers.

use std::time::Duration;

use inboxcraft_engine::Session;
use inboxcraft_model::{
    Account, AccountId, Address, EmailAddress, Envelope, Flag, Flags, Folder, FolderId, FolderListing, FolderPath, Host, Message, MessageId,
    Security, Server, SpecialUse, Summary, Uid,
};
use inboxcraft_store::{Batch, Change, Memory, Storage, StoreConfig};
use inboxcraft_time::{Clock, Timestamp};

use crate::UiError;

/// A person: a name and an address on `example.org`.
#[derive(Debug, Clone, Copy)]
struct Person {
    name: &'static str,
    address: &'static str,
}

const fn person(name: &'static str, address: &'static str) -> Person {
    Person { name, address }
}

const MIRA: Person = person("Mira Holloway", "mira@example.org");
const MIRA_STUDIO: Person = person("Mira Holloway", "holloway.studio@example.org");
const ODILE: Person = person("Odile Brannock", "odile@example.org");
const TEODOR: Person = person("Teodor Lisk", "teodor@example.org");
const ANSEL: Person = person("Ansel Quarry", "ansel@example.org");
const LIBRARY: Person = person("Riverside Library", "loans@example.org");
const CYCLES: Person = person("Wheelwright Cycles", "service@example.org");
const LETTINGS: Person = person("Lettings Desk", "lettings@example.org");
const PAPER_PINE: Person = person("Paper & Pine", "orders@example.org");
const PRIZE: Person = person("Prize Desk", "prize-desk@example.org");
const FERRIES: Person = person("Grey Gull Ferries", "bookings@example.org");
const PETRA: Person = person("Petra Harbour", "petra@example.org");
const LUMEN: Person = person("Lumen Print accounts", "accounts@example.org");
const KASIMIR: Person = person("Kasimir Vell", "kasimir@example.org");
const NEWS: Person = person("Studio Collective news", "news@example.org");

/// The two accounts: `(id, label, owner)`.
const ACCOUNTS: [(u64, &str, Person); 2] = [(1, "Personal", MIRA), (2, "Studio", MIRA_STUDIO)];

/// The folders: `(id, account, path, special use)`.
const FOLDERS: &[(u64, u64, &str, &[SpecialUse])] = &[
    (1, 1, "INBOX", &[]),
    (2, 1, "Drafts", &[SpecialUse::Drafts]),
    (3, 1, "Sent", &[SpecialUse::Sent]),
    (4, 1, "Archive", &[SpecialUse::Archive]),
    (5, 1, "Junk", &[SpecialUse::Junk]),
    (6, 1, "Trash", &[SpecialUse::Trash]),
    (7, 1, "Receipts", &[]),
    (8, 1, "Travel", &[]),
    (9, 1, "Travel/2026", &[]),
    (10, 2, "INBOX", &[]),
    (11, 2, "Sent Messages", &[SpecialUse::Sent]),
    (12, 2, "Archive", &[SpecialUse::Archive]),
    (13, 2, "Trash", &[SpecialUse::Trash]),
    (14, 2, "Clients", &[]),
    (15, 2, "Clients/Harbour Bakery", &[]),
];

/// One message: where it is, who wrote it to whom, what it says and how long ago it came.
struct Sample {
    folder: u64,
    from: Person,
    to: Person,
    subject: &'static str,
    preview: &'static str,
    minutes_ago: u64,
    seen: bool,
    flagged: bool,
}

#[expect(clippy::too_many_arguments, reason = "a table row")]
const fn sample(
    folder: u64,
    from: Person,
    to: Person,
    subject: &'static str,
    preview: &'static str,
    minutes_ago: u64,
    seen: bool,
    flagged: bool,
) -> Sample {
    Sample { folder, from, to, subject, preview, minutes_ago, seen, flagged }
}

const HOUR: u64 = 60;
const DAY: u64 = 24 * HOUR;

const MESSAGES: &[Sample] = &[
    sample(
        1,
        ODILE,
        MIRA,
        "Allotment rota for November",
        "Hi all, I have pinned the watering rota to the shed door. Could you take the second week?",
        25,
        false,
        false,
    ),
    sample(
        1,
        LIBRARY,
        MIRA,
        "Your library loan is due on Friday",
        "A Short History of Tides is due back this Friday. You can renew it once at the desk or online.",
        2 * HOUR,
        false,
        false,
    ),
    sample(
        1,
        TEODOR,
        MIRA,
        "Photos from the coast walk",
        "Here are the ones from the cliff path. The fog lifted just in time near the lighthouse.",
        5 * HOUR,
        true,
        true,
    ),
    sample(
        1,
        ODILE,
        MIRA,
        "Re: Soup recipe",
        "Add the lentils once the onions are soft, then one bay leaf. It keeps for three days.",
        28 * HOUR,
        true,
        false,
    ),
    sample(
        1,
        ANSEL,
        MIRA,
        "Choir practice moves to Thursday",
        "The hall is booked on Wednesday, so we meet on Thursday at seven instead.",
        3 * DAY,
        true,
        false,
    ),
    sample(
        1,
        CYCLES,
        MIRA,
        "Your bike is due for a service",
        "It has been a year since the last check. Book a slot and we will look at the brakes and chain.",
        9 * DAY,
        true,
        false,
    ),
    sample(
        1,
        TEODOR,
        MIRA,
        "Board game night?",
        "We could try the new tile game on Saturday. I will bring snacks if you bring the kettle.",
        12 * DAY,
        true,
        false,
    ),
    sample(2, MIRA, ANSEL, "Thank-you note", "Dear Ansel, thank you for lending me the music stand for the concert.", 6 * HOUR, true, false),
    sample(3, MIRA, ODILE, "Re: Allotment rota for November", "The second week works for me. I can also cover the Sunday after.", 20, true, false),
    sample(3, MIRA, TEODOR, "Weekend plans", "Shall we take the early train to the coast? The forecast says dry until noon.", 2 * DAY, true, false),
    sample(
        4,
        LETTINGS,
        MIRA,
        "Tenancy renewal",
        "Your tenancy renews next month. Nothing changes unless you tell us otherwise.",
        40 * DAY,
        true,
        false,
    ),
    sample(
        5,
        PRIZE,
        MIRA,
        "You have won a lighthouse",
        "Claim your prize today by sending your bank details. This offer expires soon.",
        7 * HOUR,
        false,
        false,
    ),
    sample(
        6,
        NEWS,
        MIRA,
        "Last month's newsletter",
        "Highlights from October: the print fair, a new kiln and a call for workshop ideas.",
        31 * DAY,
        true,
        false,
    ),
    sample(
        7,
        PAPER_PINE,
        MIRA,
        "Receipt for your order 4417",
        "Thank you for your order: one oak notebook and a pack of pencils.",
        6 * DAY,
        true,
        false,
    ),
    sample(
        9,
        FERRIES,
        MIRA,
        "Your ferry booking is confirmed",
        "Booking GG-2291: one foot passenger, outward on the morning sailing.",
        15 * DAY,
        true,
        true,
    ),
    sample(
        10,
        PETRA,
        MIRA_STUDIO,
        "Draft logo for the bakery",
        "Attached are three sketches. We like the second one, but the wheat looks a little heavy.",
        50,
        false,
        true,
    ),
    sample(10, LUMEN, MIRA_STUDIO, "Invoice 2026-031 is paid", "We received your payment for the poster run. Thank you.", 4 * HOUR, true, false),
    sample(
        10,
        KASIMIR,
        MIRA_STUDIO,
        "Studio open day: volunteers?",
        "We need two people at the door and one for the tea table. Who can help on the Saturday?",
        26 * HOUR,
        false,
        false,
    ),
    sample(
        10,
        PETRA,
        MIRA_STUDIO,
        "A question about the font licence",
        "Can we use the menu typeface on the shop window as well, or only in print?",
        4 * DAY,
        true,
        false,
    ),
    sample(
        11,
        MIRA_STUDIO,
        PETRA,
        "Re: Draft logo for the bakery",
        "Thanks, Petra. I will lighten the wheat and send a new version tomorrow.",
        30,
        true,
        false,
    ),
    sample(
        12,
        KASIMIR,
        MIRA_STUDIO,
        "Minutes of the studio meeting",
        "We agreed to share the big press on Tuesdays and to repaint the hallway.",
        22 * DAY,
        true,
        false,
    ),
    sample(13, NEWS, MIRA_STUDIO, "Studio Collective news", "Spring classes open for booking next week.", 45 * DAY, true, false),
    sample(
        15,
        PETRA,
        MIRA_STUDIO,
        "Brief: spring menu cards",
        "Six cards, folded, on recycled board. The first proof by the end of the month would be lovely.",
        20 * DAY,
        true,
        false,
    ),
];

fn address(person: Person) -> Result<Address, UiError> {
    Ok(Address::new(Some(person.name), EmailAddress::new(person.address).map_err(UiError::sample)?))
}

fn account(id: u64, label: &str, owner: Person) -> Result<Account, UiError> {
    let imap = Server::new(Host::new("imap.example.org").map_err(UiError::sample)?, Security::Tls.imap_port(), Security::Tls);
    let email = EmailAddress::new(owner.address).map_err(UiError::sample)?;
    Ok(Account::new(AccountId::new(id), label, email, owner.address, imap).map_err(UiError::sample)?.with_display_name(Some(owner.name)))
}

fn message(id: u64, uid: u32, account: u64, sample: &Sample, now: Timestamp) -> Result<Message, UiError> {
    let received = now.checked_sub(Duration::from_secs(sample.minutes_ago.saturating_mul(60)));
    let envelope = Envelope::new(sample.subject).with_date(received).with_from(vec![address(sample.from)?]).with_to(vec![address(sample.to)?]);
    let mut flags = Flags::default();
    if sample.seen {
        flags.insert(Flag::Seen);
    }
    if sample.flagged {
        flags.insert(Flag::Flagged);
    }
    let uid = Uid::new(uid).ok_or_else(|| UiError::Sample("a UID of 0".to_owned()))?;
    let size = u32::try_from(sample.preview.len().saturating_mul(12)).unwrap_or(u32::MAX);
    let summary = Summary::new(uid, envelope).with_flags(flags).with_received(received).with_size(size).with_preview(Some(sample.preview));
    Ok(Message::new(MessageId::new(id), AccountId::new(account), FolderId::new(sample.folder), summary))
}

/// The sample mailbox, its newest message 20 minutes before `now`.
///
/// # Errors
///
/// None in practice: the sample is fixed. A value the model or the store refuses becomes [`UiError::Sample`].
pub fn mailbox(now: Timestamp) -> Result<Memory, UiError> {
    let mut batch = Batch::new();
    for (id, label, owner) in ACCOUNTS {
        batch.push(Change::PutAccount(account(id, label, owner)?));
    }
    for (id, account, path, special_use) in FOLDERS {
        let path = FolderPath::new(path, Some('/')).map_err(UiError::sample)?;
        let listing = FolderListing::new(path, special_use.to_vec(), true);
        batch.push(Change::PutFolder(Folder::new(FolderId::new(*id), AccountId::new(*account), listing)));
    }
    let mut uids: Vec<(u64, u32)> = Vec::new();
    for (index, sample) in MESSAGES.iter().enumerate() {
        let account = FOLDERS.iter().find(|(id, ..)| *id == sample.folder).map_or(1, |(_, account, ..)| *account);
        let uid = match uids.iter_mut().find(|(folder, _)| *folder == sample.folder) {
            Some((_, uid)) => {
                *uid = uid.saturating_add(1);
                *uid
            }
            None => {
                uids.push((sample.folder, 1));
                1
            }
        };
        let id = u64::try_from(index).unwrap_or(u64::MAX).saturating_add(1);
        batch.push(Change::PutMessage(message(id, uid, account, sample, now)?));
    }
    let mut store = Memory::new(StoreConfig::default());
    store.apply(&batch).map_err(UiError::sample)?;
    Ok(store)
}

/// A session over the sample mailbox, its dates counted back from `clock`'s time now.
///
/// # Errors
///
/// As [`mailbox`], and [`UiError::Engine`] when the session refuses the store.
pub fn session(clock: Box<dyn Clock>) -> Result<Session, UiError> {
    let store = mailbox(clock.now())?;
    Session::new(Box::new(store), clock).map_err(UiError::Engine)
}

#[cfg(test)]
#[path = "sample_tests.rs"]
mod tests;
