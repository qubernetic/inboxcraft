// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{Account, Calendar, Discovery, MAX_REDIRECTS};
use crate::http::{Method, Response};
use crate::{DavError, Origin};

fn multistatus(responses: &str) -> Response {
    let body = format!(
        r#"<?xml version="1.0"?><d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav"
        xmlns:cs="http://calendarserver.org/ns/" xmlns:a="http://apple.com/ns/ical/">{responses}</d:multistatus>"#
    );
    Response::new(207).with_body(body.into_bytes())
}

fn found(href: &str, props: &str) -> String {
    format!("<d:response><d:href>{href}</d:href><d:propstat><d:prop>{props}</d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>")
}

fn principal_answer() -> Response {
    multistatus(&found("/dav/", "<d:current-user-principal><d:href>/principals/ada/</d:href></d:current-user-principal>"))
}

fn home_answer() -> Response {
    multistatus(&found("/principals/ada/", "<c:calendar-home-set><d:href>/calendars/ada/</d:href></c:calendar-home-set>"))
}

fn calendars_answer() -> Response {
    let home = found("/calendars/ada/", "<d:resourcetype><d:collection/></d:resourcetype>");
    let work = found(
        "/calendars/ada/work/",
        r##"<d:resourcetype><d:collection/><c:calendar/></d:resourcetype><d:displayname>Work</d:displayname>
        <cs:getctag>"c1"</cs:getctag><d:sync-token>http://example.org/sync/1</d:sync-token><a:calendar-color>#3366CCFF</a:calendar-color>
        <c:supported-calendar-component-set><c:comp name="vevent"/><c:comp name=""/><c:comp/></c:supported-calendar-component-set>"##,
    );
    let tasks = found(
        "tasks/",
        r#"<d:resourcetype><d:collection/><c:calendar/></d:resourcetype><c:supported-calendar-component-set><c:comp name="VTODO"/></c:supported-calendar-component-set>"#,
    );
    let plain = found("/calendars/ada/plain/", "<d:resourcetype><d:collection/><c:calendar/></d:resourcetype>");
    let inbox = found("/calendars/ada/inbox/", "<d:resourcetype><d:collection/><c:schedule-inbox/></d:resourcetype>");
    multistatus(&format!("{home}{work}{tasks}{plain}{inbox}"))
}

/// Runs `discovery` through `answers` and returns what the last one gave.
fn run(discovery: &mut Discovery, answers: &[Response]) -> Result<Option<Account>, DavError> {
    let mut last = Ok(None);
    for answer in answers {
        discovery.request()?;
        last = discovery.receive(answer);
        last.clone()?;
    }
    last
}

#[test]
fn the_well_known_path_leads_to_the_calendars() {
    let mut discovery = Discovery::new("https://dav.example.org").unwrap();
    assert_eq!(discovery.origin(), &Origin::parse("https://dav.example.org/").unwrap().0);
    let first = discovery.request().unwrap();
    assert_eq!((first.method(), first.path(), first.header("Depth")), (Method::Propfind, "/.well-known/caldav", Some("0")));
    assert!(String::from_utf8_lossy(first.body()).contains("<d:current-user-principal/>"));
    assert_eq!(discovery.receive(&Response::new(301).with_header("Location", "https://dav.example.org/dav/")), Ok(None));
    assert_eq!(discovery.request().unwrap().path(), "/dav/");
    assert_eq!(discovery.receive(&principal_answer()), Ok(None));
    assert_eq!(discovery.request().unwrap().path(), "/principals/ada/");
    assert_eq!(discovery.receive(&home_answer()), Ok(None));
    let listing = discovery.request().unwrap();
    assert_eq!((listing.path(), listing.header("Depth")), ("/calendars/ada/", Some("1")));
    let account = discovery.receive(&calendars_answer()).unwrap().unwrap();
    assert_eq!((account.principal.as_str(), account.home.as_str()), ("/principals/ada/", "/calendars/ada/"));
    assert_eq!(account.origin.to_string(), "https://dav.example.org");
    let work = Calendar {
        href: "/calendars/ada/work/".into(),
        name: Some("Work".into()),
        color: Some("#3366CCFF".into()),
        ctag: Some("\"c1\"".into()),
        sync_token: Some("http://example.org/sync/1".into()),
        components: vec!["VEVENT".into()],
    };
    assert_eq!(account.calendars.first(), Some(&work));
    let hrefs: Vec<&str> = account.calendars.iter().map(|calendar| calendar.href.as_str()).collect();
    assert_eq!(hrefs, ["/calendars/ada/work/", "/calendars/ada/tasks/", "/calendars/ada/plain/"]);
    let events: Vec<bool> = account.calendars.iter().map(Calendar::holds_events).collect();
    assert_eq!(events, [true, false, true]);
    assert_eq!(discovery.request(), Err(DavError::Finished));
    assert_eq!(discovery.receive(&calendars_answer()), Err(DavError::Finished));
}

#[test]
fn a_missing_well_known_path_falls_back_to_the_root() {
    for status in [403, 404, 405, 501] {
        let mut discovery = Discovery::new("https://dav.example.org/").unwrap();
        assert_eq!(discovery.receive(&Response::new(status)), Ok(None));
        assert_eq!(discovery.request().unwrap().path(), "/");
        assert_eq!(discovery.receive(&Response::new(status)), Err(DavError::Refused { status }));
    }
}

#[test]
fn a_given_path_is_asked_first_and_not_left() {
    let mut discovery = Discovery::new("https://dav.example.org/remote.php/dav").unwrap();
    assert_eq!(discovery.request().unwrap().path(), "/remote.php/dav");
    assert_eq!(discovery.receive(&Response::new(404)), Err(DavError::Refused { status: 404 }));
    assert_eq!(discovery.request(), Err(DavError::Finished));
}

#[test]
fn a_context_that_names_the_home_set_skips_the_principal() {
    let mut discovery = Discovery::new("https://dav.example.org/dav/").unwrap();
    let answer = multistatus(&found("/dav/", "<c:calendar-home-set><d:href>/calendars/ada/</d:href></c:calendar-home-set>"));
    let account = run(&mut discovery, &[answer, calendars_answer()]).unwrap().unwrap();
    assert_eq!((account.principal.as_str(), account.home.as_str()), ("/dav/", "/calendars/ada/"));
}

#[test]
fn redirects_are_followed_at_every_stage() {
    let mut discovery = Discovery::new("https://dav.example.org/dav/").unwrap();
    let moved = |to: &str| Response::new(308).with_header("Location", to);
    let answers = [principal_answer(), moved("/p/ada/"), home_answer(), moved("../../c/ada/"), calendars_answer()];
    let mut paths = Vec::new();
    for answer in &answers {
        paths.push(discovery.request().unwrap().path().to_owned());
        let _ = discovery.receive(answer).unwrap();
    }
    assert_eq!(paths, ["/dav/", "/principals/ada/", "/p/ada/", "/calendars/ada/", "/c/ada/"]);
}

#[test]
fn a_redirect_to_another_origin_is_not_followed() {
    let mut discovery = Discovery::new("https://dav.example.org").unwrap();
    let away = Response::new(302).with_header("Location", "https://caldav.other.example.org/dav/");
    assert_eq!(discovery.receive(&away), Err(DavError::OtherOrigin { url: "https://caldav.other.example.org/dav/".into() }));
    let mut discovery = Discovery::new("https://dav.example.org").unwrap();
    let clear = Response::new(301).with_header("Location", "http://dav.example.org/dav/");
    assert_eq!(discovery.receive(&clear), Err(DavError::OtherOrigin { url: "http://dav.example.org/dav/".into() }));
}

#[test]
fn redirects_are_capped_and_need_a_location() {
    let mut discovery = Discovery::new("https://dav.example.org").unwrap();
    let again = Response::new(307).with_header("Location", "/loop/");
    for _ in 0..MAX_REDIRECTS {
        assert_eq!(discovery.receive(&again), Ok(None));
    }
    assert_eq!(discovery.receive(&again), Err(DavError::TooManyRedirects));
    let mut discovery = Discovery::new("https://dav.example.org").unwrap();
    assert_eq!(discovery.receive(&Response::new(301)), Err(DavError::Missing("redirect location")));
}

#[test]
fn an_answer_between_redirects_starts_the_count_again() {
    let mut discovery = Discovery::new("https://dav.example.org/dav/").unwrap();
    let again = Response::new(303).with_header("Location", "/x/");
    for _ in 0..MAX_REDIRECTS {
        assert_eq!(discovery.receive(&again), Ok(None));
    }
    assert_eq!(discovery.receive(&principal_answer()), Ok(None));
    for _ in 0..MAX_REDIRECTS {
        assert_eq!(discovery.receive(&again), Ok(None));
    }
}

#[test]
fn a_refused_password_ends_discovery() {
    let mut discovery = Discovery::new("https://dav.example.org").unwrap();
    assert_eq!(discovery.receive(&Response::new(401)), Err(DavError::Unauthorized));
    assert_eq!(discovery.request(), Err(DavError::Finished));
    let mut discovery = Discovery::new("https://dav.example.org/dav/").unwrap();
    let anonymous = multistatus(&found("/dav/", "<d:current-user-principal><d:unauthenticated/></d:current-user-principal>"));
    assert_eq!(discovery.receive(&anonymous), Err(DavError::Unauthorized));
}

#[test]
fn what_the_server_does_not_name_is_missing() {
    let mut discovery = Discovery::new("https://dav.example.org/dav/").unwrap();
    assert_eq!(discovery.receive(&multistatus(&found("/dav/", "<d:resourcetype/>"))), Err(DavError::Missing("current user principal")));
    let mut discovery = Discovery::new("https://dav.example.org/dav/").unwrap();
    assert_eq!(run(&mut discovery, &[principal_answer(), multistatus("")]), Err(DavError::Missing("calendar home set")));
}

#[test]
fn other_statuses_and_bad_answers_end_discovery() {
    let mut discovery = Discovery::new("https://dav.example.org").unwrap();
    assert_eq!(discovery.receive(&Response::new(500)), Err(DavError::Refused { status: 500 }));
    let mut discovery = Discovery::new("https://dav.example.org/dav/").unwrap();
    assert_eq!(discovery.receive(&Response::new(207).with_body(b"<x".to_vec())), Err(DavError::BadXml("an unclosed tag")));
    assert_eq!(Discovery::new("ftp://dav.example.org/"), Err(DavError::BadUrl("a scheme other than https or http")));
}
