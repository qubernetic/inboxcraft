// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::{Arc, Mutex};

use inboxcraft_html::{Link, LinkTarget};
use inboxcraft_time::{FakeClock, Timestamp};
use serde_json::json;

use super::{Content, PictureState, Reader, inline_key, link_status, size_text, unbase64};
use crate::fetch::{Fetch, Fetched};
use crate::mailbox::Mailbox;
use crate::quote::Segment;
use crate::sample;
use crate::save::Save;

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

/// A fetch that writes down every address it is asked for and answers with a drawn picture, or fails.
#[derive(Clone, Default)]
pub(crate) struct Recorder {
    pub(crate) asked: Arc<Mutex<Vec<String>>>,
    done: Arc<Mutex<Vec<Fetched>>>,
    fail: bool,
}

impl Fetch for Recorder {
    fn start(&mut self, url: &str) {
        self.asked.lock().unwrap().push(url.to_owned());
        let answer = if self.fail { Err("refused\u{202e}".to_owned()) } else { sample::scene(1, 40, 20).map_err(|e| e.to_string()) };
        self.done.lock().unwrap().push((url.to_owned(), answer));
    }

    fn finished(&mut self) -> Vec<Fetched> {
        std::mem::take(&mut *self.done.lock().unwrap())
    }
}

/// The files a [`Kept`] was given.
type Files = Arc<Mutex<Vec<(String, Vec<u8>)>>>;

/// A save that keeps what it was given.
#[derive(Clone, Default)]
struct Kept(Files);

impl Save for Kept {
    fn save(&mut self, name: &str, bytes: &[u8]) -> Result<String, String> {
        self.0.lock().unwrap().push((name.to_owned(), bytes.to_vec()));
        Ok(format!("/downloads/{name}"))
    }
}

fn mailbox() -> Mailbox {
    Mailbox::new(sample::session(clock()).unwrap(), clock())
}

fn id_of(mailbox: &mut Mailbox, subject: &str) -> u64 {
    let found = mailbox.run("mail.search", &json!({ "subject": subject })).unwrap();
    found["messages"].as_array().unwrap().iter().find(|m| m["subject"] == subject).unwrap()["id"].as_u64().unwrap()
}

fn reader(fetch: &Recorder, kept: &Kept) -> Reader {
    Reader::new(Box::new(fetch.clone()), Box::new(kept.clone()))
}

#[test]
fn a_reply_opens_with_its_thread_oldest_first_and_its_quote_folded() {
    let (fetch, kept) = (Recorder::default(), Kept::default());
    let mut mailbox = mailbox();
    let reply = id_of(&mut mailbox, "Re: Allotment rota for November");
    let mut reader = reader(&fetch, &kept);
    reader.open(&mut mailbox, Some(reply));
    assert_eq!(reader.open_id(), Some(reply));
    let senders: Vec<&str> = reader.thread().iter().map(|row| row.sender.as_str()).collect();
    assert_eq!(senders, ["Odile Brannock", "Mira Holloway"]);
    let original = reader.thread()[0].id;
    assert!(!reader.is_expanded(original) && reader.opened(original).is_none(), "the older message is folded and not read");
    assert!(reader.is_expanded(reply));
    let opened = reader.opened(reply).unwrap();
    assert_eq!(opened.header.from[0].full(), "Mira Holloway <mira@example.org>");
    assert_eq!(opened.header.to[0].shown(), "Odile Brannock");
    let Content::Text(segments) = &opened.content else { panic!("{:?}", opened.content) };
    assert!(matches!(&segments[1], Segment::Quote { attribution: Some(a), lines: 8, .. } if a == "Odile Brannock wrote:"), "{segments:?}");
    assert!(!reader.shows_quotes(reply));
    reader.toggle_quotes(reply);
    assert!(reader.shows_quotes(reply));
    reader.toggle_quotes(reply);
    assert!(!reader.shows_quotes(reply));
    reader.expand(&mut mailbox, original);
    assert!(reader.is_expanded(original) && reader.opened(original).is_some());
    reader.expand(&mut mailbox, original);
    assert!(!reader.is_expanded(original), "folded again");
    reader.expand(&mut mailbox, reply);
    assert!(reader.is_expanded(reply), "the open message stays unfolded");
    reader.expand(&mut mailbox, 9999);
    assert!(!reader.is_expanded(9999));
    reader.toggle_details(reply);
    assert!(reader.shows_details(reply));
    reader.toggle_details(reply);
    assert!(!reader.shows_details(reply));
}

#[test]
fn no_remote_request_before_consent_and_once_loads_this_message_only() {
    let (fetch, kept) = (Recorder::default(), Kept::default());
    let mut mailbox = mailbox();
    let newsletter = id_of(&mut mailbox, "Your bike is due for a service");
    let mut reader = reader(&fetch, &kept);
    reader.open(&mut mailbox, Some(newsletter));
    assert!(!reader.poll());
    assert!(fetch.asked.lock().unwrap().is_empty(), "nothing asked before consent");
    let Content::Html(document) = &reader.opened(newsletter).unwrap().content else { panic!() };
    assert_eq!(document.remote_blocked, 1);
    assert!(!reader.remote_allowed(newsletter));
    reader.allow_once(&mut mailbox, newsletter);
    assert_eq!(*fetch.asked.lock().unwrap(), ["https://pictures.example.org/cycles/workshop.png"]);
    assert!(matches!(reader.picture("https://pictures.example.org/cycles/workshop.png"), Some(PictureState::Waiting)));
    assert!(reader.poll());
    assert!(matches!(reader.picture("https://pictures.example.org/cycles/workshop.png"), Some(PictureState::Ready(p)) if p.width == 40));
    // Once is for this message only: opening it again asks again, and nothing is fetched twice meanwhile.
    reader.open(&mut mailbox, Some(newsletter));
    assert!(!reader.remote_allowed(newsletter));
    assert_eq!(fetch.asked.lock().unwrap().len(), 1);
    assert_eq!(mailbox.run("remote.list", &json!({})).unwrap()["senders"], json!([]), "once changes no setting");
}

#[test]
fn always_for_this_sender_loads_now_and_next_time_without_asking() {
    let fetch = Recorder { fail: true, ..Recorder::default() };
    let kept = Kept::default();
    let mut mailbox = mailbox();
    let junk = id_of(&mut mailbox, "You have won a lighthouse");
    let mut reader = reader(&fetch, &kept);
    reader.open(&mut mailbox, Some(junk));
    assert!(fetch.asked.lock().unwrap().is_empty());
    reader.allow_always(&mut mailbox, junk);
    assert_eq!(*fetch.asked.lock().unwrap(), ["https://track.example.net/open.png?id=4417"]);
    assert_eq!(mailbox.run("remote.list", &json!({})).unwrap()["senders"], json!(["prize-desk@example.org"]));
    reader.poll();
    let Some(PictureState::Failed(reason)) = reader.picture("https://track.example.net/open.png?id=4417") else { panic!() };
    assert_eq!(reason, "refused", "the reason is shown without its bidi control");
    reader.open(&mut mailbox, None);
    reader.open(&mut mailbox, Some(junk));
    assert!(reader.remote_allowed(junk), "allowed without asking");
    let Content::Html(document) = &reader.opened(junk).unwrap().content else { panic!() };
    assert_eq!(document.remote_blocked, 0);
    assert_eq!(fetch.asked.lock().unwrap().len(), 1, "a picture already tried is not asked again");
    // An answer for an address nobody asked for is dropped.
    fetch.done.lock().unwrap().push(("https://other.example.org/x.png".to_owned(), Ok(Vec::new())));
    assert!(reader.poll());
    assert!(reader.picture("https://other.example.org/x.png").is_none());
}

#[test]
fn a_mismatched_link_is_called_out_on_hover() {
    let (fetch, kept) = (Recorder::default(), Kept::default());
    let mut mailbox = mailbox();
    let junk = id_of(&mut mailbox, "You have won a lighthouse");
    let mut reader = reader(&fetch, &kept);
    reader.open(&mut mailbox, Some(junk));
    let Content::Html(document) = &reader.opened(junk).unwrap().content else { panic!() };
    assert_eq!(
        link_status(&document.links[0]),
        "Careful: the text says “https://bank.example.org/login”, but the link goes to prize-desk.example.net: https://prize-desk.example.net/claim"
    );
    let web = Link {
        target: LinkTarget::Web { url: "https://cycles.example.org/book".to_owned(), host: "cycles.example.org".to_owned() },
        text: "Book".to_owned(),
        mismatch: false,
    };
    assert_eq!(link_status(&web), "https://cycles.example.org/book");
    let mail = Link {
        target: LinkTarget::Mail { url: "mailto:a@example.org".to_owned(), address: "a@example.org".to_owned() },
        text: String::new(),
        mismatch: false,
    };
    assert_eq!(link_status(&mail), "Write to a@example.org");
    let refused = Link { target: LinkTarget::Refused, text: "x".to_owned(), mismatch: false };
    assert!(link_status(&refused).starts_with("This link is not followed"));
}

#[test]
fn an_attachment_is_saved_through_the_save_the_app_handed_in() {
    let (fetch, kept) = (Recorder::default(), Kept::default());
    let mut mailbox = mailbox();
    let logo = id_of(&mut mailbox, "Draft logo for the bakery");
    let mut reader = reader(&fetch, &kept);
    reader.open(&mut mailbox, Some(logo));
    let files = &reader.opened(logo).unwrap().files;
    assert_eq!(files.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["sketch-1.png", "sketch-2.png", "sketch-3.png"]);
    let size = files[1].size;
    reader.save(&mut mailbox, logo, 1);
    assert_eq!(reader.notice(), Some("Saved sketch-2.png to /downloads/sketch-2.png"));
    let saved = kept.0.lock().unwrap().clone();
    assert_eq!(saved[0].0, "sketch-2.png");
    assert_eq!(Some(saved[0].1.len() as u64), size);
    assert!(crate::picture::decode(&saved[0].1).is_ok());
    reader.save(&mut mailbox, logo, 7);
    assert!(reader.notice().unwrap().contains("there is no attachment 7"), "{:?}", reader.notice());
    reader.dismiss_notice();
    assert_eq!(reader.notice(), None);
    reader.set_notice("Link copied".to_owned());
    assert_eq!(reader.notice(), Some("Link copied"));
}

#[test]
fn full_width_only_while_a_message_is_open() {
    let (fetch, kept) = (Recorder::default(), Kept::default());
    let mut mailbox = mailbox();
    let mut reader = reader(&fetch, &kept);
    reader.set_full_width(true);
    assert!(!reader.full_width());
    let id = id_of(&mut mailbox, "Photos from the coast walk");
    reader.open(&mut mailbox, Some(id));
    reader.set_full_width(true);
    assert!(reader.full_width());
    reader.open(&mut mailbox, None);
    assert!(!reader.full_width() && reader.thread().is_empty());
    assert!(format!("{reader:?}").starts_with("Reader { open: None"));
}

#[test]
fn a_message_that_is_not_there_says_so() {
    let (fetch, kept) = (Recorder::default(), Kept::default());
    let mut mailbox = mailbox();
    let mut reader = reader(&fetch, &kept);
    reader.open(&mut mailbox, Some(4242));
    assert!(reader.notice().unwrap().contains("there is no message 4242"));
    assert!(reader.opened(4242).is_none());
    reader.allow_once(&mut mailbox, 4242);
    reader.allow_always(&mut mailbox, 4242);
    assert!(fetch.asked.lock().unwrap().is_empty());
}

#[test]
fn inline_pictures_come_from_the_message_and_never_from_the_network() {
    use inboxcraft_engine::Session;
    use inboxcraft_store::{Batch, Body, Change, Storage};
    let png = sample::scene(2, 8, 8).unwrap();
    let encoded: String = png.chunks(57).map(|chunk| format!("{}\r\n", b64(chunk))).collect();
    let raw = format!(
        "From: Anna <anna@example.org>\r\nSubject: Inline\r\nContent-Type: multipart/related; boundary=b\r\n\r\n--b\r\n\
         Content-Type: text/html; charset=utf-8\r\n\r\n<p><img src=\"cid:pic@example.org\" alt=\"inline\"><img src=\"cid:missing@example.org\"></p>\r\n\
         --b\r\nContent-Type: image/png\r\nContent-ID: <pic@example.org>\r\nContent-Transfer-Encoding: base64\r\n\r\n{encoded}--b--\r\n"
    );
    let mut store = sample::mailbox(Timestamp::from_unix(1_793_869_200, 0).unwrap()).unwrap();
    let id = 1;
    store.apply(&Batch::new().with(Change::PutBody(inboxcraft_model::MessageId::new(id), Body::new(raw.into_bytes(), "").unwrap()))).unwrap();
    let mut mailbox = Mailbox::new(Session::new(Box::new(store), clock()).unwrap(), clock());
    let (fetch, kept) = (Recorder::default(), Kept::default());
    let mut reader = reader(&fetch, &kept);
    reader.open(&mut mailbox, Some(id));
    assert!(matches!(reader.picture(&inline_key(id, "pic@example.org")), Some(PictureState::Ready(p)) if p.width == 8));
    assert!(matches!(reader.picture(&inline_key(id, "missing@example.org")), Some(PictureState::Failed(_))));
    assert!(fetch.asked.lock().unwrap().is_empty());
}

/// Base64 for the hand-made test message.
pub(crate) fn b64(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b = |i: usize| u32::from(*chunk.get(i).unwrap_or(&0));
        let g = (b(0) << 16) | (b(1) << 8) | b(2);
        out.push(A[(g >> 18) as usize & 63] as char);
        out.push(A[(g >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { A[(g >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { A[g as usize & 63] as char } else { '=' });
    }
    out
}

#[test]
fn base64_from_the_engine_is_read_and_anything_else_refused() {
    assert_eq!(unbase64(""), Some(Vec::new()));
    assert_eq!(unbase64("Zm9vYmFy"), Some(b"foobar".to_vec()));
    assert_eq!(unbase64("Zm9vYg=="), Some(b"foob".to_vec()));
    assert_eq!(unbase64("Zm9vYmE="), Some(b"fooba".to_vec()));
    assert_eq!(unbase64("+//+"), Some(vec![0xfb, 0xff, 0xfe]));
    for bad in ["Zm9", "Zm9v!mFy", "Zg==Zg==", "Z===", "Zm 9v"] {
        assert_eq!(unbase64(bad), None, "{bad}");
    }
    for n in 0..20_u8 {
        let bytes: Vec<u8> = (0..n).map(|i| i.wrapping_mul(37)).collect();
        assert_eq!(unbase64(&b64(&bytes)), Some(bytes));
    }
}

#[test]
fn sizes_read_as_people_say_them() {
    assert_eq!(size_text(None), "size unknown");
    assert_eq!(size_text(Some(0)), "0 bytes");
    assert_eq!(size_text(Some(1)), "1 byte");
    assert_eq!(size_text(Some(900)), "900 bytes");
    assert_eq!(size_text(Some(13_000)), "13 KB");
    assert_eq!(size_text(Some(3_565_158)), "3.4 MB");
}
