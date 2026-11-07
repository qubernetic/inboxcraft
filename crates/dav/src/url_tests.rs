// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{MAX_URL, Origin};
use crate::DavError;

fn origin(url: &str) -> (Origin, String) {
    Origin::parse(url).unwrap()
}

#[test]
fn an_https_url_is_split_into_origin_and_path() {
    let (origin, path) = origin("HTTPS://Dav.Example.ORG/calendars/ada/");
    assert!(origin.is_encrypted());
    assert_eq!((origin.host(), origin.port()), ("dav.example.org", 443));
    assert_eq!(path, "/calendars/ada/");
    assert_eq!(origin.to_string(), "https://dav.example.org");
    assert_eq!(origin.authority(), "dav.example.org");
    assert_eq!(origin.url("/a/"), "https://dav.example.org/a/");
}

#[test]
fn ports_other_than_the_default_are_kept() {
    let (origin, path) = origin("https://dav.example.org:8443");
    assert_eq!((origin.port(), path.as_str()), (8443, "/"));
    assert_eq!(origin.authority(), "dav.example.org:8443");
    assert_eq!(origin.to_string(), "https://dav.example.org:8443");
    let (plain, _) = Origin::parse("http://127.0.0.1:8080/x").unwrap();
    assert!(!plain.is_encrypted());
    assert_eq!(plain.authority(), "127.0.0.1:8080");
    let (plain, _) = Origin::parse("http://dav.example.org:80/").unwrap();
    assert_eq!(plain.authority(), "dav.example.org");
    let (v6, _) = Origin::parse("https://[::1]:8443/").unwrap();
    assert_eq!(v6.authority(), "[::1]:8443");
}

#[test]
fn urls_that_cannot_be_used_are_refused() {
    let cases = [
        ("ftp://dav.example.org/", "a scheme other than https or http"),
        ("dav.example.org/", "a scheme other than https or http"),
        ("https://ada:secret@dav.example.org/", "a user name in the URL"),
        ("https:///path", "a bad host"),
        ("https://dav example.org/", "a bad host"),
        ("https://[::1/", "a bad host"),
        ("https://[zz]/", "a bad host"),
        ("https://[::1]x/", "a bad port"),
        ("https://dav.example.org:/", "a bad port"),
        ("https://dav.example.org:0/", "a bad port"),
        ("https://dav.example.org:65536/", "a bad port"),
        ("https://dav.example.org:4x/", "a bad port"),
        ("https://dav.example.org/a\r\nHost: evil", "a control character"),
    ];
    for (url, why) in cases {
        assert_eq!(Origin::parse(url), Err(DavError::BadUrl(why)), "{url}");
    }
    let long = format!("https://dav.example.org/{}", "a".repeat(MAX_URL));
    assert_eq!(Origin::parse(&long), Err(DavError::BadUrl("a URL too long")));
}

#[test]
fn paths_are_encoded_and_dot_segments_removed() {
    let (_, path) = origin("https://dav.example.org/a b/./c/../d%2f%41%7e%40é?x=1#frag");
    assert_eq!(path, "/a%20b/d%2FA~@%C3%A9?x=1");
    let (_, path) = origin("https://dav.example.org/../../a/..");
    assert_eq!(path, "/");
    let (_, path) = origin("https://dav.example.org/100%/<x>\"");
    assert_eq!(path, "/100%25/%3Cx%3E%22");
}

#[test]
fn references_resolve_against_the_base() {
    let (origin, _) = origin("https://dav.example.org/");
    let base = "/calendars/ada/work/";
    let cases = [
        ("event.ics", "/calendars/ada/work/event.ics"),
        ("./event.ics", "/calendars/ada/work/event.ics"),
        ("../home/", "/calendars/ada/home/"),
        ("/principals/ada/", "/principals/ada/"),
        ("https://DAV.example.org:443/x/", "/x/"),
        ("//dav.example.org/y/", "/y/"),
        ("", "/calendars/ada/work/"),
        ("#top", "/calendars/ada/work/"),
        ("?q=1", "/calendars/ada/work/?q=1"),
    ];
    for (reference, resolved) in cases {
        assert_eq!(origin.resolve(base, reference).as_deref(), Ok(resolved), "{reference}");
    }
    assert_eq!(origin.resolve("/calendars/ada/work?x=1", "y").as_deref(), Ok("/calendars/ada/y"));
    assert_eq!(origin.resolve("relative", "").as_deref(), Ok("/relative"));
}

#[test]
fn a_reference_to_another_origin_is_named_not_followed() {
    let (origin, _) = origin("https://dav.example.org/");
    let err = origin.resolve("/", "https://other.example.org/dav/?a#b");
    assert_eq!(err, Err(DavError::OtherOrigin { url: "https://other.example.org/dav/?a".into() }));
    let err = origin.resolve("/", "http://dav.example.org/dav/");
    assert_eq!(err, Err(DavError::OtherOrigin { url: "http://dav.example.org/dav/".into() }));
    let err = origin.resolve("/", "https://dav.example.org:8443/");
    assert_eq!(err, Err(DavError::OtherOrigin { url: "https://dav.example.org:8443/".into() }));
    assert_eq!(origin.resolve("/", "mailto:ada@example.org"), Err(DavError::BadUrl("a scheme other than https or http")));
    assert_eq!(origin.resolve("/", "https://ada@dav.example.org/"), Err(DavError::BadUrl("a user name in the URL")));
    assert_eq!(origin.resolve("/", "a\0b"), Err(DavError::BadUrl("a control character")));
}

#[test]
fn a_colon_in_a_relative_path_is_no_scheme() {
    let (origin, _) = origin("https://dav.example.org/");
    assert_eq!(origin.resolve("/c/", "a/b:c").as_deref(), Ok("/c/a/b:c"));
    assert_eq!(origin.resolve("/c/", "1x:y").as_deref(), Ok("/c/1x:y"));
}

#[test]
fn equal_paths_written_differently_come_out_the_same() {
    let (origin, _) = origin("https://dav.example.org/");
    let one = origin.resolve("/", "/cal/ada%40example.org/%7Eev%2a.ics").unwrap();
    let two = origin.resolve("/", "/cal/ada@example.org/~ev*.ics").unwrap();
    assert_eq!(one, two);
    assert_eq!(one, "/cal/ada@example.org/~ev*.ics");
}
