// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{MAX_TOKEN, clean, propfind, read, token};
use crate::xml::{self, CALDAV, DAV};
use crate::{DavError, Origin};

fn origin() -> Origin {
    Origin::parse("https://dav.example.org/").unwrap().0
}

const ANSWER: &str = r#"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav">
  <d:response>
    <d:href>work/</d:href>
    <d:propstat><d:prop><d:displayname> Work&#9;stuff </d:displayname><d:resourcetype><d:collection/><c:calendar/></d:resourcetype>
      <d:current-user-principal><d:href>/principals/ada/</d:href></d:current-user-principal><d:getetag>"1"</d:getetag></d:prop>
      <d:status>HTTP/1.1 200 OK</d:status></d:propstat>
    <d:propstat><d:prop><c:calendar-color/></d:prop><d:status>HTTP/1.1 404 Not Found</d:status></d:propstat>
    <d:propstat><d:prop><d:other/></d:prop><d:status>garbage</d:status></d:propstat>
    <d:propstat><d:status>HTTP/1.1 200 OK</d:status></d:propstat>
  </d:response>
  <d:response><d:href>/cal/gone.ics</d:href><d:href>/cal/gone2.ics</d:href><d:status>HTTP/1.1 404 Not Found</d:status></d:response>
  <d:response><d:href>https://other.example.org/x</d:href><d:status>HTTP/1.1 404 Not Found</d:status></d:response>
  <d:sync-token> http://example.org/sync/2 </d:sync-token>
</d:multistatus>"#;

#[test]
fn entries_their_properties_and_the_sync_token_are_read() {
    let answer = read(&origin(), "/cal/", ANSWER.as_bytes()).unwrap();
    assert_eq!(answer.sync_token.as_deref(), Some("http://example.org/sync/2"));
    let hrefs: Vec<&str> = answer.entries.iter().map(|entry| entry.href.as_str()).collect();
    assert_eq!(hrefs, ["/cal/work/", "/cal/gone.ics", "/cal/gone2.ics"]);
    let work = &answer.entries[0];
    assert_eq!(work.status, None);
    assert_eq!(work.propstats.len(), 2);
    assert_eq!(work.text(DAV, "displayname", 64).as_deref(), Some("Workstuff"));
    assert_eq!(work.text(DAV, "displayname", 2).as_deref(), Some("Wo"));
    assert!(work.prop(CALDAV, "calendar-color").is_none());
    assert_eq!(work.text(CALDAV, "calendar-color", 9), None);
    assert!(work.is_type(CALDAV, "calendar") && !work.is_type(CALDAV, "schedule-inbox"));
    assert_eq!(work.etag().as_deref(), Some("\"1\""));
    assert_eq!(work.href_in(DAV, "current-user-principal", &origin(), "/cal/").as_deref(), Some("/principals/ada/"));
    assert_eq!(work.href_in(DAV, "displayname", &origin(), "/cal/"), None);
    assert_eq!(answer.entries[1].status, Some(404));
    assert!(answer.entries[1].propstats.is_empty());
}

#[test]
fn a_root_other_than_multistatus_is_refused() {
    assert_eq!(read(&origin(), "/", br#"<d:error xmlns:d="DAV:"/>"#), Err(DavError::Missing("multistatus")));
    assert_eq!(read(&origin(), "/", b"<a"), Err(DavError::BadXml("an unclosed tag")));
}

#[test]
fn an_empty_text_property_counts_as_absent() {
    let body = r#"<d:multistatus xmlns:d="DAV:"><d:response><d:href>/a</d:href><d:propstat><d:prop><d:displayname> </d:displayname></d:prop>
        <d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>"#;
    let answer = read(&origin(), "/", body.as_bytes()).unwrap();
    assert_eq!(answer.entries[0].text(DAV, "displayname", 9), None);
    assert!(answer.entries[0].prop(DAV, "displayname").is_some());
}

#[test]
fn tokens_must_be_printable_and_short() {
    assert_eq!(token(" \"abc\" ").as_deref(), Some("\"abc\""));
    assert_eq!(token(""), None);
    assert_eq!(token("a\r\nb"), None);
    assert_eq!(token("é"), None);
    assert_eq!(token(&"a".repeat(MAX_TOKEN + 1)), None);
    assert!(token(&"a".repeat(MAX_TOKEN)).is_some());
}

#[test]
fn clean_drops_control_characters_and_caps() {
    assert_eq!(clean(" a\u{7}b\u{9b}c ", 10), "abc");
    assert_eq!(clean("ab cd", 3), "ab");
}

#[test]
fn propfind_bodies_are_well_formed() {
    let body = propfind("<d:displayname/><c:calendar-home-set/><cs:getctag/><a:calendar-color/>");
    let root = xml::parse(body.as_bytes()).unwrap();
    assert!(root.is(DAV, "propfind"));
    let prop = root.child(DAV, "prop").unwrap();
    assert_eq!(prop.elements().count(), 4);
    assert!(prop.child(CALDAV, "calendar-home-set").is_some());
}
