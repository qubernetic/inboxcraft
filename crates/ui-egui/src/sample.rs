// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The invented sample mailbox behind `--sample`: two accounts of one invented person, their folders and a few
//! dozen messages, all on `example.org`.
//!
//! Every name, address, subject, text and picture here is made up for InboxCraft; the pictures are drawn in code.
//! The dates count back from the `now` given, so the sample always looks recent. Two replies quote what they answer
//! and share a thread with it; a newsletter and a junk message carry remote pictures and links on `example.org` and
//! `example.net`. Nothing in it connects anywhere: the accounts point at `imap.example.org`, the sample session never
//! starts their workers, and [`Pictures`] draws the remote pictures instead of fetching them.

use std::time::Duration;

use inboxcraft_engine::Session;
use inboxcraft_mime::{Attachment, Mailbox as Sender, MessageBuilder};
use inboxcraft_model::{
    Account, AccountId, Address, EmailAddress, Envelope, Flag, Flags, Folder, FolderId, FolderListing, FolderPath, Host, Message, MessageId,
    MessageRef, Security, Server, SpecialUse, Summary, Uid,
};
use inboxcraft_store::{Batch, Body, Change, Memory, Storage, StoreConfig};
use inboxcraft_time::{Clock, Timestamp};

use crate::UiError;
use crate::fetch::{Fetch, Fetched};

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

/// Everyone in the sample.
const PEOPLE: [Person; 15] =
    [MIRA, MIRA_STUDIO, ODILE, TEODOR, ANSEL, LIBRARY, CYCLES, LETTINGS, PAPER_PINE, PRIZE, FERRIES, PETRA, LUMEN, KASIMIR, NEWS];

/// Everyone in the sample, `(name, address)`: their avatars take their colours from the theme's avatar palette.
#[must_use]
pub fn people() -> Vec<(&'static str, &'static str)> {
    PEOPLE.iter().map(|person| (person.name, person.address)).collect()
}

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

/// The thread links: a subject, its `Message-ID` and the `Message-ID` it answers.
const LINKS: &[(&str, &str, Option<&str>)] = &[
    ("Allotment rota for November", "rota.1@example.org", None),
    ("Re: Allotment rota for November", "rota.2@example.org", Some("rota.1@example.org")),
    ("Draft logo for the bakery", "logo.1@example.org", None),
    ("Re: Draft logo for the bakery", "logo.2@example.org", Some("logo.1@example.org")),
];

/// What a message says beyond its preview: its subject, its plain text, its HTML and the pictures attached to it.
struct Text {
    subject: &'static str,
    plain: &'static str,
    html: Option<&'static str>,
    files: &'static [&'static str],
}

const TEXTS: &[Text] = &[
    Text {
        subject: "Allotment rota for November",
        plain: "Hi all,\n\nI have pinned the watering rota to the shed door. Could you take the second week? The tap by the gate is \
                fixed now, so no more buckets from the stream.\n\nSee you at the plot,\nOdile\n",
        html: None,
        files: &[],
    },
    Text {
        subject: "Re: Allotment rota for November",
        plain: "The second week works for me. I can also cover the Sunday after.\n\nMira\n\nOdile Brannock wrote:\n> Hi all,\n>\n\
                > I have pinned the watering rota to the shed door. Could you take the\n> second week? The tap by the gate is fixed \
                now, so no more buckets from\n> the stream.\n>\n> See you at the plot,\n> Odile\n",
        html: None,
        files: &[],
    },
    Text {
        subject: "Photos from the coast walk",
        plain: "Here are the ones from the cliff path. The fog lifted just in time near the lighthouse.\n\nThe second one is my \
                favourite.\n\nTeodor\n",
        html: None,
        files: &["cliff-path.png", "lighthouse.png"],
    },
    Text {
        subject: "Draft logo for the bakery",
        plain: "Hi Mira,\n\nAttached are three sketches. We like the second one, but the wheat looks a little heavy. Could you try a \
                lighter stroke?\n\nPetra\n",
        html: None,
        files: &["sketch-1.png", "sketch-2.png", "sketch-3.png"],
    },
    Text {
        subject: "Re: Draft logo for the bakery",
        plain: "Thanks, Petra. I will lighten the wheat and send a new version tomorrow.\n\nMira\n\nPetra Harbour wrote:\n> Hi Mira,\n>\n\
                > Attached are three sketches. We like the second one, but the wheat looks\n> a little heavy. Could you try a lighter \
                stroke?\n>\n> Petra\n",
        html: None,
        files: &[],
    },
    Text {
        subject: "Your bike is due for a service",
        plain: "It has been a year since the last check. Book a slot and we will look at the brakes, the chain and the gears.\n\n\
                Book a slot: https://cycles.example.org/book\n",
        html: Some(
            "<h1>Time for a check-up</h1><p><img src=\"https://pictures.example.org/cycles/workshop.png\" alt=\"The workshop bench\"></p>\
             <p>It has been a year since the last check. Book a slot and we will look at the <b>brakes</b>, the <b>chain</b> and the \
             gears.</p><ul><li>Brakes and pads</li><li>Chain and gears</li><li>Tyres and lights</li></ul>\
             <p><a href=\"https://cycles.example.org/book\">Book a slot</a> or call in at the shop.</p><p>Wheelwright Cycles</p>",
        ),
        files: &[],
    },
    Text {
        subject: "You have won a lighthouse",
        plain: "Claim your prize today by sending your bank details. This offer expires soon.\n",
        html: Some(
            "<p><img src=\"https://track.example.net/open.png?id=4417\" alt=\"\"></p><h2>Congratulations!</h2>\
             <p>Claim your prize today by sending your bank details. This offer expires soon.</p>\
             <p><a href=\"https://prize-desk.example.net/claim\">https://bank.example.org/login</a></p>",
        ),
        files: &[],
    },
];

/// A PNG of `width` by `height` pixels, each painted by `paint`.
fn drawn(width: u32, height: u32, paint: impl Fn(u32, u32) -> [u8; 3]) -> Result<Vec<u8>, UiError> {
    let mut data = Vec::new();
    for y in 0..height {
        for x in 0..width {
            data.extend(paint(x, y));
        }
    }
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, width, height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header().and_then(|mut writer| writer.write_image_data(&data)).map_err(UiError::sample)?;
    Ok(out)
}

/// A small landscape drawn in code, its colours and sun picked by `seed`: sky, a hill or the sea, a round sun.
pub(crate) fn scene(seed: u64, width: u32, height: u32) -> Result<Vec<u8>, UiError> {
    let palettes: [([u8; 3], [u8; 3], [u8; 3]); 4] = [
        ([186, 214, 226], [70, 122, 140], [242, 200, 120]),
        ([236, 214, 182], [148, 108, 72], [214, 120, 72]),
        ([206, 222, 196], [96, 136, 92], [250, 236, 170]),
        ([214, 206, 230], [92, 96, 140], [244, 214, 160]),
    ];
    let pick = usize::try_from(seed % 4).unwrap_or(0);
    let (sky, ground, sun) = palettes.get(pick).copied().unwrap_or(palettes[0]);
    let (w, h) = (f64::from(width.max(1)), f64::from(height.max(1)));
    let sun_x = w * (0.25 + 0.15 * f64::from(u32::try_from(seed % 3).unwrap_or(0)));
    drawn(width, height, |x, y| {
        let (x, y) = (f64::from(x), f64::from(y));
        let horizon = h * (0.62 + 0.08 * (x / w * std::f64::consts::PI * 2.0 + f64::from(u32::try_from(seed % 5).unwrap_or(0))).sin());
        if y > horizon {
            ground
        } else if (x - sun_x).hypot(y - h * 0.3) < h * 0.14 {
            sun
        } else {
            let fade = (y / h * 40.0).clamp(0.0, 40.0) as u8;
            [sky[0].saturating_add(fade / 2), sky[1].saturating_add(fade / 3), sky[2]]
        }
    })
}

/// The remote pictures of the sample, drawn instead of fetched: every `https` address on `example.org` or
/// `example.net` gives a picture, anything else fails. Nothing connects anywhere.
#[derive(Debug, Default)]
pub struct Pictures {
    done: Vec<Fetched>,
}

impl Fetch for Pictures {
    fn start(&mut self, url: &str) {
        let host = url.strip_prefix("https://").and_then(|rest| rest.split(['/', '?', '#']).next()).unwrap_or_default();
        let answer = if host.ends_with(".example.org") || host.ends_with(".example.net") {
            let seed = url.bytes().fold(0_u64, |sum, byte| sum.wrapping_mul(31).wrapping_add(u64::from(byte)));
            scene(seed, 480, 200).map_err(|error| error.to_string())
        } else {
            Err(crate::tr!("The sample draws only its own pictures.").to_owned())
        };
        self.done.push((url.to_owned(), answer));
    }

    fn finished(&mut self) -> Vec<Fetched> {
        std::mem::take(&mut self.done)
    }
}

/// The source of a sample message: its headers, its text (the preview when it has no text of its own), its HTML and
/// its pictures; and the plain text for the search.
fn source(sample: &Sample) -> Result<(Vec<u8>, String), UiError> {
    let text = TEXTS.iter().find(|text| text.subject == sample.subject);
    let mut builder = MessageBuilder::new()
        .from(Sender::named(sample.from.name, sample.from.address))
        .to(Sender::named(sample.to.name, sample.to.address))
        .subject(sample.subject);
    if let Some((_, id, answers)) = LINKS.iter().find(|(subject, ..)| *subject == sample.subject) {
        builder = builder.message_id(id);
        if let Some(answers) = answers {
            builder = builder.in_reply_to(answers).reference(answers);
        }
    }
    let plain = text.map_or_else(|| format!("{}\n", sample.preview), |text| text.plain.to_owned());
    builder = builder.text(&plain);
    if let Some(html) = text.and_then(|text| text.html) {
        builder = builder.html(html);
    }
    for (index, file) in text.map(|text| text.files).unwrap_or_default().iter().enumerate() {
        let picture = scene(u64::try_from(index).unwrap_or(0).saturating_add(u64::try_from(file.len()).unwrap_or(0)), 320, 200)?;
        builder = builder.attach(Attachment::new(file, "image/png", picture));
    }
    Ok((builder.build().map_err(UiError::sample)?, plain))
}

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
    let mut envelope = Envelope::new(sample.subject).with_date(received).with_from(vec![address(sample.from)?]).with_to(vec![address(sample.to)?]);
    if let Some((_, id, answers)) = LINKS.iter().find(|(subject, ..)| *subject == sample.subject) {
        let answers: Vec<MessageRef> = answers.and_then(MessageRef::new).into_iter().collect();
        envelope = envelope.with_message_id(MessageRef::new(id)).with_in_reply_to(answers.clone()).with_references(answers);
    }
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
        let (source, text) = source(sample)?;
        batch.push(Change::PutBody(MessageId::new(id), Body::new(source, &text).map_err(UiError::sample)?));
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
