// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{Changes, MAX_MULTIGET, MAX_RESOURCES, MAX_ROUNDS, Resource, Sync};
use crate::http::{Method, Response};
use crate::xml::{self, CALDAV, DAV};
use crate::{DavError, Origin};

const CAL: &str = "/cal/work/";

fn origin() -> Origin {
    Origin::parse("https://dav.example.org/").unwrap().0
}

fn known(items: &[(&str, &str)]) -> Vec<(String, String)> {
    items.iter().map(|(href, etag)| (format!("{CAL}{href}"), format!("\"{etag}\""))).collect()
}

fn sync(token: Option<&str>, items: &[(&str, &str)]) -> Sync {
    Sync::new(&origin(), CAL, token, known(items)).unwrap()
}

fn multistatus(responses: &str, token: Option<&str>) -> Response {
    let token = token.map(|token| format!("<d:sync-token>{token}</d:sync-token>")).unwrap_or_default();
    let body = format!(r#"<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav">{responses}{token}</d:multistatus>"#);
    Response::new(207).with_body(body.into_bytes())
}

fn etag(href: &str, etag: &str) -> String {
    format!(
        "<d:response><d:href>{CAL}{href}</d:href><d:propstat><d:prop><d:getetag>\"{etag}\"</d:getetag></d:prop>\
         <d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>"
    )
}

fn status(href: &str, code: u16) -> String {
    format!("<d:response><d:href>{href}</d:href><d:status>HTTP/1.1 {code} X</d:status></d:response>")
}

fn data(href: &str, etag: &str) -> String {
    format!(
        "<d:response><d:href>{CAL}{href}</d:href><d:propstat><d:prop><d:getetag>\"{etag}\"</d:getetag>\
         <c:calendar-data>BEGIN:VCALENDAR&#13;\n{href}&#13;\nEND:VCALENDAR&#13;\n</c:calendar-data></d:prop>\
         <d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>"
    )
}

fn resource(href: &str, etag: &str) -> Resource {
    Resource { href: format!("{CAL}{href}"), etag: Some(format!("\"{etag}\"")), data: format!("BEGIN:VCALENDAR\r\n{href}\r\nEND:VCALENDAR\r\n") }
}

fn invalid_token(status: u16) -> Response {
    Response::new(status).with_body(br#"<d:error xmlns:d="DAV:"><d:valid-sync-token/></d:error>"#.to_vec())
}

/// The hrefs a `calendar-multiget` request asks for.
fn asked(sync: &Sync) -> Vec<String> {
    let request = sync.request().unwrap();
    assert_eq!((request.method(), request.header("Depth")), (Method::Report, Some("1")));
    let root = xml::parse(request.body()).unwrap();
    assert!(root.is(CALDAV, "calendar-multiget"));
    root.children_named(DAV, "href").map(xml::Element::text).collect()
}

/// The sync token a sync report request sends.
fn token_sent(sync: &Sync) -> String {
    let request = sync.request().unwrap();
    assert_eq!((request.method(), request.path(), request.header("Depth")), (Method::Report, CAL, Some("0")));
    let root = xml::parse(request.body()).unwrap();
    assert!(root.is(DAV, "sync-collection"));
    assert_eq!(root.child(DAV, "sync-level").map(xml::Element::text).as_deref(), Some("1"));
    root.child(DAV, "sync-token").unwrap().text()
}

#[test]
fn a_sync_from_a_token_fetches_what_changed_and_names_what_went() {
    let mut sync = sync(Some("http://example.org/sync/1"), &[("a.ics", "1"), ("b.ics", "1"), ("c.ics", "1")]);
    assert_eq!(token_sent(&sync), "http://example.org/sync/1");
    let report = format!("{}{}{}{}", status(CAL, 200), etag("a.ics", "2"), status(&format!("{CAL}b.ics"), 404), etag("d.ics", "1"));
    assert_eq!(sync.receive(&multistatus(&report, Some("http://example.org/sync/2"))), Ok(None));
    assert_eq!(asked(&sync), [format!("{CAL}a.ics"), format!("{CAL}d.ics")]);
    let changes = sync.receive(&multistatus(&format!("{}{}", data("a.ics", "2"), data("d.ics", "1")), None)).unwrap().unwrap();
    let expected = Changes {
        changed: vec![resource("a.ics", "2"), resource("d.ics", "1")],
        removed: vec![format!("{CAL}b.ics")],
        token: Some("http://example.org/sync/2".into()),
        full: false,
    };
    assert_eq!(changes, expected);
    assert_eq!(sync.request(), Err(DavError::Finished));
    assert_eq!(sync.receive(&Response::new(207)), Err(DavError::Finished));
}

#[test]
fn nothing_changed_needs_no_fetch() {
    let mut sync = sync(Some("t1"), &[("a.ics", "1")]);
    let changes = sync.receive(&multistatus("", Some("t1"))).unwrap().unwrap();
    assert_eq!(changes, Changes { token: Some("t1".into()), ..Changes::default() });
}

#[test]
fn a_first_sync_compares_everything() {
    let mut sync = sync(None, &[("a.ics", "1"), ("c.ics", "1")]);
    assert_eq!(token_sent(&sync), "");
    assert_eq!(sync.receive(&multistatus(&format!("{}{}", etag("a.ics", "1"), etag("b.ics", "2")), Some("t1"))), Ok(None));
    assert_eq!(asked(&sync), [format!("{CAL}b.ics")]);
    let changes = sync.receive(&multistatus(&data("b.ics", "2"), None)).unwrap().unwrap();
    assert_eq!(
        changes,
        Changes { changed: vec![resource("b.ics", "2")], removed: vec![format!("{CAL}c.ics")], token: Some("t1".into()), full: true }
    );
}

#[test]
fn a_token_the_server_forgot_starts_again_without_one() {
    for code in [403, 409] {
        let mut sync = sync(Some("old"), &[("a.ics", "1"), ("gone.ics", "1")]);
        assert_eq!(sync.receive(&invalid_token(code)), Ok(None));
        assert_eq!(token_sent(&sync), "");
        let changes = sync.receive(&multistatus(&etag("a.ics", "1"), Some("new"))).unwrap().unwrap();
        assert_eq!(changes, Changes { removed: vec![format!("{CAL}gone.ics")], token: Some("new".into()), full: true, ..Changes::default() });
    }
}

#[test]
fn a_forgotten_first_token_is_refused_rather_than_looped() {
    let mut sync = sync(None, &[]);
    assert_eq!(sync.receive(&invalid_token(403)), Err(DavError::Refused { status: 403 }));
}

#[test]
fn a_server_without_the_sync_report_is_listed() {
    for code in [400, 403, 405, 501] {
        let mut sync = sync(Some("t1"), &[("a.ics", "1"), ("b.ics", "1"), ("gone.ics", "1")]);
        assert_eq!(sync.receive(&Response::new(code)), Ok(None));
        let request = sync.request().unwrap();
        assert_eq!((request.method(), request.header("Depth")), (Method::Propfind, Some("1")));
        let sub = format!(
            "<d:response><d:href>{CAL}sub/</d:href><d:propstat><d:prop><d:resourcetype><d:collection/></d:resourcetype></d:prop>\
             <d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>"
        );
        let listing = format!("{}{sub}{}{}{}", etag("", "x"), status(&format!("{CAL}odd.ics"), 404), etag("a.ics", "1"), etag("b.ics", "2"));
        assert_eq!(sync.receive(&multistatus(&listing, Some("ignored"))), Ok(None));
        assert_eq!(asked(&sync), [format!("{CAL}b.ics")]);
        let changes = sync.receive(&multistatus(&data("b.ics", "2"), None)).unwrap().unwrap();
        assert_eq!(changes, Changes { changed: vec![resource("b.ics", "2")], removed: vec![format!("{CAL}gone.ics")], token: None, full: true });
    }
}

#[test]
fn a_report_cut_short_is_asked_again_from_its_token() {
    let mut sync = sync(Some("t1"), &[("a.ics", "1")]);
    let first = format!("{}{}", etag("b.ics", "1"), status(&format!("{CAL}a.ics"), 404));
    assert_eq!(sync.receive(&multistatus(&format!("{first}{}", status(CAL, 507)), Some("t2"))), Ok(None));
    assert_eq!(token_sent(&sync), "t2");
    let second = format!("{}{}", etag("a.ics", "3"), status(&format!("{CAL}b.ics"), 404));
    assert_eq!(sync.receive(&multistatus(&second, Some("t3"))), Ok(None));
    assert_eq!(asked(&sync), [format!("{CAL}a.ics")]);
    let changes = sync.receive(&multistatus(&data("a.ics", "3"), None)).unwrap().unwrap();
    assert_eq!(
        changes,
        Changes { changed: vec![resource("a.ics", "3")], removed: vec![format!("{CAL}b.ics")], token: Some("t3".into()), full: false }
    );
}

#[test]
fn a_server_that_always_cuts_short_is_asked_a_bounded_number_of_times() {
    let mut sync = sync(Some("t"), &[]);
    let cut = multistatus(&status(CAL, 507), Some("t"));
    for _ in 0..MAX_ROUNDS - 1 {
        assert_eq!(sync.receive(&cut), Ok(None));
    }
    assert_eq!(sync.receive(&cut).unwrap().map(|changes| changes.token), Some(Some("t".into())));
    let mut sync = super::Sync::new(&origin(), CAL, Some("t"), []).unwrap();
    let changes = sync.receive(&multistatus(&status(CAL, 507), None)).unwrap().unwrap();
    assert_eq!(changes.token, None);
}

#[test]
fn many_changes_are_fetched_in_batches() {
    let mut sync = sync(None, &[]);
    let listing: String = (0..120).map(|n| etag(&format!("{n:03}.ics"), "1")).collect();
    assert_eq!(sync.receive(&multistatus(&listing, Some("t"))), Ok(None));
    let mut sizes = Vec::new();
    let mut first = Vec::new();
    loop {
        let batch = asked(&sync);
        sizes.push(batch.len());
        first.push(batch[0].clone());
        let answer: String = batch.iter().map(|href| data(href.trim_start_matches(CAL), "1")).collect();
        if let Some(changes) = sync.receive(&multistatus(&answer, None)).unwrap() {
            assert_eq!(changes.changed.len(), 120);
            assert_eq!(changes.token.as_deref(), Some("t"));
            break;
        }
    }
    assert_eq!(sizes, [MAX_MULTIGET, MAX_MULTIGET, 20]);
    assert_eq!(first, [format!("{CAL}000.ics"), format!("{CAL}050.ics"), format!("{CAL}100.ics")]);
}

#[test]
fn a_multiget_answer_is_taken_for_what_was_asked_only() {
    let mut sync = sync(Some("t1"), &[("gone.ics", "1")]);
    let report = format!("{}{}{}{}", etag("a.ics", "1"), etag("gone.ics", "2"), etag("lost.ics", "1"), etag("noetag.ics", "1"));
    assert_eq!(sync.receive(&multistatus(&report, Some("t2"))), Ok(None));
    let plain = format!(
        "<d:response><d:href>{CAL}noetag.ics</d:href><d:propstat><d:prop><c:calendar-data>x</c:calendar-data></d:prop>\
         <d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>"
    );
    let answer = format!("{}{}{}{}{plain}", data("a.ics", "1"), data("a.ics", "9"), data("other.ics", "1"), status(&format!("{CAL}gone.ics"), 404));
    let changes = sync.receive(&multistatus(&answer, None)).unwrap().unwrap();
    let noetag = Resource { href: format!("{CAL}noetag.ics"), etag: Some("\"1\"".into()), data: "x".into() };
    assert_eq!(changes.changed, [resource("a.ics", "1"), noetag]);
    assert_eq!(changes.removed, [format!("{CAL}gone.ics")]);
    assert_eq!(changes.token, None, "lost.ics was not fetched, so the next sync compares everything");
}

#[test]
fn errors_end_the_sync() {
    let mut first = sync(Some("t"), &[]);
    assert_eq!(first.receive(&Response::new(401)), Err(DavError::Unauthorized));
    assert_eq!(first.request(), Err(DavError::Finished));
    assert_eq!(sync(Some("t"), &[]).receive(&Response::new(500)), Err(DavError::Refused { status: 500 }));
    assert_eq!(sync(Some("t"), &[]).receive(&invalid_token(500)), Err(DavError::Refused { status: 500 }));
    assert_eq!(sync(Some("t"), &[]).receive(&invalid_token(400)), Err(DavError::Refused { status: 400 }));
    let mut listing = sync(Some("t"), &[]);
    assert_eq!(listing.receive(&Response::new(501)), Ok(None));
    assert_eq!(listing.receive(&Response::new(500)), Err(DavError::Refused { status: 500 }));
    let mut fetch = sync(Some("t"), &[]);
    assert_eq!(fetch.receive(&multistatus(&etag("a.ics", "1"), Some("t2"))), Ok(None));
    assert_eq!(fetch.receive(&Response::new(404)), Err(DavError::Refused { status: 404 }));
    assert_eq!(sync(None, &[]).receive(&Response::new(207).with_body(b"<d:x xmlns:d='DAV:'/>".to_vec())), Err(DavError::Missing("multistatus")));
}

#[test]
fn hrefs_and_tokens_are_checked_on_the_way_in() {
    assert_eq!(
        Sync::new(&origin(), "https://other.example.org/cal/", None, []),
        Err(DavError::OtherOrigin { url: "https://other.example.org/cal/".into() })
    );
    let sync = Sync::new(
        &origin(),
        "https://dav.example.org/cal/work/",
        Some("bad\r\ntoken"),
        [("https://other.example.org/x".to_owned(), "\"1\"".to_owned())],
    )
    .unwrap();
    assert_eq!(token_sent(&sync), "");
    assert!(sync.known.is_empty());
    let escaped = Sync::new(&origin(), CAL, Some("a<b&c"), []).unwrap();
    assert_eq!(token_sent(&escaped), "a<b&c");
}

#[test]
fn a_calendar_larger_than_the_cap_is_refused() {
    let mut sync = sync(Some("t"), &[]);
    let round = |from: usize| -> Response {
        let entries: String = (from..from + 40_000).map(|n| format!("<d:response><d:href>{n}</d:href></d:response>")).collect();
        multistatus(&format!("{entries}{}", status(CAL, 507)), Some("t"))
    };
    assert_eq!(sync.receive(&round(0)), Ok(None));
    assert_eq!(sync.receive(&round(40_000)), Ok(None));
    assert_eq!(sync.receive(&round(80_000)), Err(DavError::TooLarge { what: "calendar", limit: MAX_RESOURCES }));
}
