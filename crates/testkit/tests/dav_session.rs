// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The fake CalDAV server without a socket: discovery, PROPFIND, the reports, GET, PUT and DELETE, sign-in.

use inboxcraft_testkit::dav::{FakeDav, HOME, PRINCIPAL, WORK};

const AUTH: &str = "Authorization: Basic YWRhQGV4YW1wbGUub3JnOm5vdC1hLXJlYWwtcGFzc3dvcmQ=";
const EVENT: &str = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:1@example.org\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

/// One answer: status, header fields, body.
#[derive(Debug)]
struct Answer {
    status: u16,
    fields: Vec<(String, String)>,
    body: String,
}

impl Answer {
    fn field(&self, name: &str) -> Option<&str> {
        self.fields.iter().find(|(written, _)| written.eq_ignore_ascii_case(name)).map(|(_, value)| value.as_str())
    }
}

/// A request with Host, the fake's password, `fields` and `body`.
fn request(method: &str, path: &str, fields: &[&str], body: &str) -> String {
    let fields: String = fields.iter().map(|field| format!("{field}\r\n")).collect();
    format!("{method} {path} HTTP/1.1\r\nHost: dav.example.org\r\n{AUTH}\r\n{fields}Content-Length: {}\r\n\r\n{body}", body.len())
}

/// Reads the answers in `out`, each framed by its Content-Length.
fn answers(out: &[u8]) -> Vec<Answer> {
    let mut text = String::from_utf8_lossy(out).into_owned();
    let mut found = Vec::new();
    while let Some((head, rest)) = text.split_once("\r\n\r\n") {
        let mut lines = head.split("\r\n");
        let status = lines.next().unwrap().split(' ').nth(1).unwrap().parse().unwrap();
        let fields: Vec<(String, String)> =
            lines.map(|line| line.split_once(": ").unwrap()).map(|(name, value)| (name.to_owned(), value.to_owned())).collect();
        let len: usize = fields.iter().find(|(name, _)| name == "Content-Length").unwrap().1.parse().unwrap();
        found.push(Answer { status, fields, body: rest[..len].to_owned() });
        text = rest[len..].to_owned();
    }
    found
}

fn one(fake: &FakeDav, input: &str) -> Answer {
    let mut all = answers(&fake.answer(input.as_bytes()));
    assert_eq!(all.len(), 1, "{all:?}");
    all.remove(0)
}

fn propfind(path: &str, depth: &str, props: &str) -> String {
    let body = format!(
        r#"<?xml version="1.0"?><d:propfind xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav" xmlns:cs="http://calendarserver.org/ns/"
        xmlns:a="http://apple.com/ns/ical/"><d:prop>{props}</d:prop></d:propfind>"#
    );
    request("PROPFIND", path, &[&format!("Depth: {depth}")], &body)
}

fn report(path: &str, body: &str) -> String {
    request("REPORT", path, &["Depth: 0"], body)
}

fn sync_body(token: &str) -> String {
    format!(
        r#"<d:sync-collection xmlns:d="DAV:"><d:sync-token>{token}</d:sync-token><d:sync-level>1</d:sync-level><d:prop><d:getetag/></d:prop></d:sync-collection>"#
    )
}

/// The text between `<d:sync-token>` and its end tag.
fn token_in(body: &str) -> String {
    body.split("<d:sync-token>").nth(1).unwrap().split("</d:sync-token>").next().unwrap().to_owned()
}

#[test]
fn discovery_leads_from_the_well_known_path_to_the_calendars() {
    let fake = FakeDav::new();
    let moved = one(&fake, &request("PROPFIND", "/.well-known/caldav", &["Depth: 0"], ""));
    assert_eq!((moved.status, moved.field("Location")), (301, Some("/")));
    let root = one(&fake, &propfind("/", "0", "<d:current-user-principal/><c:calendar-home-set/>"));
    assert_eq!(root.status, 207);
    assert!(root.body.contains(&format!("<d:current-user-principal><d:href>{PRINCIPAL}</d:href></d:current-user-principal>")), "{}", root.body);
    assert!(root.body.contains("<d:propstat><d:prop><c:calendar-home-set/></d:prop><d:status>HTTP/1.1 404 Not Found</d:status></d:propstat>"));
    let principal = one(&fake, &propfind(PRINCIPAL, "1", "<c:calendar-home-set/><d:resourcetype/><d:displayname/>"));
    assert!(principal.body.contains(&format!("<c:calendar-home-set><d:href>{HOME}</d:href></c:calendar-home-set>")));
    assert!(principal.body.contains("<d:resourcetype><d:principal/></d:resourcetype><d:displayname>Ada</d:displayname>"));
    assert_eq!(principal.body.matches("<d:response>").count(), 1);
    let props = "<d:resourcetype/><d:displayname/><cs:getctag/><d:sync-token/><c:supported-calendar-component-set/><a:calendar-color/><x:odd xmlns:x=\"urn:x\"/>";
    let home = one(&fake, &propfind(HOME, "1", props));
    assert_eq!(home.field("Content-Type"), Some("application/xml; charset=utf-8"));
    assert!(home.body.contains("<d:displayname>Calendars</d:displayname>"));
    assert!(home.body.contains(&format!("<d:href>{WORK}</d:href>")));
    assert!(
        home.body.contains(
            "<d:resourcetype><d:collection/><c:calendar/></d:resourcetype><d:displayname>Work</d:displayname><cs:getctag>\"0\"</cs:getctag>"
        )
    );
    assert!(home.body.contains("<c:supported-calendar-component-set><c:comp name=\"VEVENT\"/></c:supported-calendar-component-set><a:calendar-color>#3366CCFF</a:calendar-color>"));
    assert!(home.body.contains("<x:odd xmlns:x=\"urn:x\"/>"));
    assert!(home.body.contains("<d:sync-token>http://example.org/ns/sync/0-0</d:sync-token>"));
}

#[test]
fn calendars_elsewhere_in_the_tree_are_not_in_the_home_set() {
    let fake = FakeDav::new();
    assert_eq!(fake.calendars().add("calendars/ada/private", "Private", &["VEVENT", "VTODO"]), "/calendars/ada/private/");
    fake.calendars().add("/shared/team/", "Team", &[]);
    fake.calendars().add("/calendars/ada/deep/er/", "Deep", &[]);
    let home = one(&fake, &propfind(HOME, "1", "<d:displayname/>"));
    assert!(home.body.contains("<d:displayname>Private</d:displayname>"));
    assert!(!home.body.contains("Team") && !home.body.contains("Deep"));
    let team = one(&fake, &propfind("/shared/team/", "0", "<c:supported-calendar-component-set/>"));
    assert!(team.body.contains("<d:propstat><d:prop><c:supported-calendar-component-set/></d:prop><d:status>HTTP/1.1 200"), "{}", team.body);
}

#[test]
fn propfind_without_a_finite_depth_or_on_nothing_is_refused() {
    let fake = FakeDav::new();
    let infinite = one(&fake, &request("PROPFIND", HOME, &[], ""));
    assert_eq!(infinite.status, 403);
    assert!(infinite.body.contains("<d:propfind-finite-depth/>"));
    assert_eq!(one(&fake, &request("PROPFIND", HOME, &["Depth: infinity"], "")).status, 403);
    assert_eq!(one(&fake, &propfind("/nothing/", "0", "<d:displayname/>")).status, 404);
    assert_eq!(one(&fake, &request("PROPFIND", HOME, &["Depth: 0"], "<d:prop xmlns:d='DAV:'/>")).status, 400);
    assert_eq!(one(&fake, &request("PROPFIND", HOME, &["Depth: 0"], "<a>")).status, 400);
}

#[test]
fn propfind_without_names_lists_the_usual_properties() {
    let fake = FakeDav::new();
    let event = format!("{WORK}e1.ics");
    let etag = fake.calendars().put(&event, EVENT).unwrap();
    for body in ["", "  ", "<d:propfind xmlns:d=\"DAV:\"><d:allprop/></d:propfind>"] {
        let answer = one(&fake, &request("PROPFIND", &event, &["Depth: 0"], body));
        assert!(answer.body.contains(&format!("<d:getetag>{}</d:getetag>", etag.replace('"', "&quot;"))), "{}", answer.body);
        assert!(answer.body.contains("<d:resourcetype/>"));
        assert!(answer.body.contains("<d:getcontenttype>text/calendar; charset=utf-8</d:getcontenttype>"));
        assert!(answer.body.contains("<d:current-user-principal/>"));
    }
    let calendar = one(&fake, &propfind(WORK, "1", "<d:getetag/><c:calendar-data/>"));
    assert!(calendar.body.contains("BEGIN:VCALENDAR&#13;\nBEGIN:VEVENT"), "{}", calendar.body);
}

#[test]
fn every_request_needs_the_password() {
    let fake = FakeDav::new();
    let bare = format!("PROPFIND {HOME} HTTP/1.1\r\nHost: dav.example.org\r\nDepth: 0\r\n\r\n");
    let refused = one(&fake, &bare);
    assert_eq!((refused.status, refused.field("WWW-Authenticate")), (401, Some("Basic realm=\"example.org\", charset=\"UTF-8\"")));
    for wrong in ["Basic YWRhOndyb25n", "Bearer abc", "Basic", "Basic !!!!"] {
        let input = format!("GET {WORK}x.ics HTTP/1.1\r\nAuthorization: {wrong}\r\n\r\n");
        assert_eq!(one(&fake, &input).status, 401, "{wrong}");
    }
    let other = FakeDav::new().login("bea@example.org", "also-not-real");
    assert_eq!(one(&other, &propfind(HOME, "0", "<d:displayname/>")).status, 401);
    let input = format!("PROPFIND {HOME} HTTP/1.1\r\nAuthorization: basic YmVhQGV4YW1wbGUub3JnOmFsc28tbm90LXJlYWw=\r\nDepth: 0\r\n\r\n");
    assert_eq!(one(&other, &input).status, 207);
}

#[test]
fn the_well_known_path_can_be_missing() {
    let fake = FakeDav::new().without_well_known();
    assert_eq!(one(&fake, &request("GET", "/.well-known/caldav", &[], "")).status, 404);
}

#[test]
fn put_creates_and_replaces_with_preconditions() {
    let fake = FakeDav::new();
    let calendars = fake.calendars();
    let event = format!("{WORK}e1.ics");
    let created = one(&fake, &request("PUT", &event, &["If-None-Match: *", "Content-Type: text/calendar"], EVENT));
    assert_eq!(created.status, 201);
    let etag = created.field("ETag").unwrap().to_owned();
    assert_eq!(calendars.get(&event), Some((etag.clone(), EVENT.to_owned())));
    assert_eq!(one(&fake, &request("PUT", &event, &["If-None-Match: *"], EVENT)).status, 412);
    assert_eq!(one(&fake, &request("PUT", &event, &["If-Match: \"999\""], EVENT)).status, 412);
    let changed = EVENT.replace("UID:1", "UID:2");
    let replaced = one(&fake, &request("PUT", &event, &[&format!("If-Match: \"999\", {etag}")], &changed));
    assert_eq!(replaced.status, 204);
    assert_ne!(replaced.field("ETag"), Some(etag.as_str()));
    assert_eq!(calendars.get(&event).map(|(_, data)| data), Some(changed));
    assert_eq!(one(&fake, &request("PUT", &format!("{WORK}new.ics"), &["If-Match: *"], EVENT)).status, 412);
    assert_eq!(one(&fake, &request("PUT", &event, &["If-Match: *"], EVENT)).status, 204);
    assert_eq!(one(&fake, &request("PUT", &event, &[], EVENT)).status, 204);
}

#[test]
fn put_refuses_what_is_no_calendar_resource() {
    let fake = FakeDav::new();
    let bad = one(&fake, &request("PUT", &format!("{WORK}bad.ics"), &[], "not a calendar"));
    assert_eq!(bad.status, 403);
    assert!(bad.body.contains("<c:valid-calendar-data/>"));
    assert_eq!(one(&fake, &request("PUT", &format!("{WORK}bin.ics"), &["Content-Type: x"], "\u{fffd}")).status, 403);
    assert_eq!(one(&fake, &request("PUT", "/nowhere/e.ics", &[], EVENT)).status, 409);
    assert_eq!(one(&fake, &request("PUT", &format!("{WORK}sub/"), &[], EVENT)).status, 409);
    for collection in [WORK, HOME, PRINCIPAL, "/"] {
        assert_eq!(one(&fake, &request("PUT", collection, &[], EVENT)).status, 405, "{collection}");
    }
    assert!(fake.calendars().hrefs(WORK).is_empty());
}

#[test]
fn get_and_delete() {
    let fake = FakeDav::new();
    let event = format!("{WORK}e1.ics");
    let etag = fake.calendars().put(&event, EVENT).unwrap();
    let got = one(&fake, &request("GET", &event, &[], ""));
    assert_eq!((got.status, got.field("ETag"), got.body.as_str()), (200, Some(etag.as_str()), EVENT));
    assert_eq!(one(&fake, &request("GET", WORK, &[], "")).status, 404);
    assert_eq!(one(&fake, &request("DELETE", &event, &["If-Match: \"999\""], "")).status, 412);
    assert_eq!(one(&fake, &request("DELETE", &event, &[&format!("If-Match: {etag}")], "")).status, 204);
    assert_eq!(fake.calendars().get(&event), None);
    assert_eq!(one(&fake, &request("DELETE", &event, &[], "")).status, 404);
    assert_eq!(one(&fake, &request("DELETE", WORK, &[], "")).status, 403);
    assert_eq!(one(&fake, &request("DELETE", HOME, &[], "")).status, 403);
}

#[test]
fn a_sync_report_lists_the_changes_since_its_token() {
    let fake = FakeDav::new();
    let calendars = fake.calendars();
    let (a, b) = (format!("{WORK}a.ics"), format!("{WORK}b.ics"));
    calendars.put(&a, EVENT).unwrap();
    calendars.put(&b, EVENT).unwrap();
    let first = one(&fake, &report(WORK, &sync_body("")));
    assert_eq!(first.status, 207);
    assert!(first.body.contains(&format!("<d:href>{a}</d:href>")) && first.body.contains(&format!("<d:href>{b}</d:href>")));
    let token = token_in(&first.body);
    assert_eq!(calendars.token(WORK), Some(token.clone()));
    let quiet = one(&fake, &report(WORK, &sync_body(&token)));
    assert!(!quiet.body.contains("<d:response>"));
    assert!(calendars.remove(&a));
    assert!(!calendars.remove(&a));
    calendars.put(&b, &EVENT.replace("UID:1", "UID:3")).unwrap();
    let changes = one(&fake, &report(WORK, &sync_body(&token)));
    assert!(
        changes.body.contains(&format!("<d:response><d:href>{a}</d:href><d:status>HTTP/1.1 404 Not Found</d:status></d:response>")),
        "{}",
        changes.body
    );
    assert!(changes.body.contains(&format!("<d:href>{b}</d:href><d:propstat>")));
    assert_ne!(token_in(&changes.body), token);
}

#[test]
fn a_forgotten_or_foreign_token_is_refused_with_its_precondition() {
    let fake = FakeDav::new();
    let first = one(&fake, &report(WORK, &sync_body("")));
    fake.calendars().forget_tokens();
    for token in [token_in(&first.body), "http://example.org/ns/sync/9-9".into(), "urn:other".into(), "http://example.org/ns/sync/x".into()] {
        let refused = one(&fake, &report(WORK, &sync_body(&token)));
        assert_eq!(refused.status, 403, "{token}");
        assert!(refused.body.contains("<d:valid-sync-token/>"));
    }
    assert_eq!(one(&fake, &request("REPORT", WORK, &["Depth: 1"], &sync_body(""))).status, 400);
    let no_token = one(&fake, &report(WORK, r#"<d:sync-collection xmlns:d="DAV:"/>"#));
    assert_eq!(no_token.status, 207);
}

#[test]
fn a_paged_sync_hands_out_the_rest_from_its_token() {
    let fake = FakeDav::new().page(2);
    let calendars = fake.calendars();
    for name in ["a", "b", "c"] {
        calendars.put(&format!("{WORK}{name}.ics"), EVENT).unwrap();
    }
    let first = one(&fake, &report(WORK, &sync_body("")));
    assert!(first.body.contains(&format!("<d:response><d:href>{WORK}</d:href><d:status>HTTP/1.1 507 Insufficient Storage</d:status></d:response>")));
    assert_eq!(first.body.matches("<d:propstat>").count(), 2);
    let token = token_in(&first.body);
    assert!(calendars.remove(&format!("{WORK}a.ics")));
    let second = one(&fake, &report(WORK, &sync_body(&token)));
    assert!(second.body.contains(&format!("{WORK}c.ics")));
    assert!(second.body.contains(&format!("<d:response><d:href>{WORK}a.ics</d:href><d:status>HTTP/1.1 404")));
    assert!(!second.body.contains("507"));
    let paged = FakeDav::new().page(1);
    paged.calendars().put(&format!("{WORK}x.ics"), EVENT);
    paged.calendars().put(&format!("{WORK}y.ics"), EVENT);
    assert!(paged.calendars().remove(&format!("{WORK}x.ics")));
    let start = one(&paged, &report(WORK, &sync_body("http://example.org/ns/sync/0-1")));
    assert!(start.body.contains("507") && start.body.contains(&format!("{WORK}y.ics")));
    assert_eq!(token_in(&start.body), "http://example.org/ns/sync/0-2");
    let rest = one(&paged, &report(WORK, &sync_body("http://example.org/ns/sync/0-2")));
    assert!(rest.body.contains(&format!("<d:href>{WORK}x.ics</d:href><d:status>HTTP/1.1 404")));
}

#[test]
fn a_server_without_sync_has_no_tokens() {
    let fake = FakeDav::new().without_sync();
    let refused = one(&fake, &report(WORK, &sync_body("")));
    assert_eq!(refused.status, 403);
    assert!(refused.body.contains("<d:supported-report/>"));
    let calendar = one(&fake, &propfind(WORK, "0", "<d:sync-token/>"));
    assert!(calendar.body.contains("<d:sync-token/></d:prop><d:status>HTTP/1.1 404"), "{}", calendar.body);
}

#[test]
fn multiget_and_query_return_the_calendar_data() {
    let fake = FakeDav::new();
    let calendars = fake.calendars();
    let other = calendars.add("/calendars/ada/other/", "Other", &["VEVENT"]);
    let (a, b, elsewhere) = (format!("{WORK}a.ics"), format!("{WORK}b%20c.ics"), format!("{other}o.ics"));
    calendars.put(&a, EVENT).unwrap();
    calendars.put(&format!("{WORK}b c.ics"), EVENT).unwrap();
    calendars.put(&elsewhere, EVENT).unwrap();
    let body = format!(
        r#"<c:calendar-multiget xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav"><d:prop><d:getetag/><c:calendar-data/></d:prop>
        <d:href>{a}</d:href><d:href>{b}</d:href><d:href>{WORK}gone.ics</d:href><d:href>{elsewhere}</d:href><d:href>%zz</d:href></c:calendar-multiget>"#
    );
    let answer = one(&fake, &request("REPORT", WORK, &["Depth: 1"], &body));
    assert_eq!(answer.body.matches("<c:calendar-data>").count(), 2, "{}", answer.body);
    assert!(answer.body.contains(&format!("<d:href>{WORK}b c.ics</d:href>")));
    assert!(answer.body.contains(&format!("<d:href>{WORK}gone.ics</d:href><d:status>HTTP/1.1 404")));
    assert!(answer.body.contains(&format!("<d:href>{elsewhere}</d:href><d:status>HTTP/1.1 404")));
    let query =
        r#"<c:calendar-query xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav"><d:prop><d:getetag/></d:prop><c:filter/></c:calendar-query>"#;
    let listed = one(&fake, &request("REPORT", WORK, &["Depth: 1"], query));
    assert_eq!(listed.body.matches("<d:getetag>").count(), 2);
    let without_prop = one(&fake, &request("REPORT", WORK, &["Depth: 1"], r#"<c:calendar-query xmlns:c="urn:ietf:params:xml:ns:caldav"/>"#));
    assert_eq!(without_prop.body.matches("<d:getetag>").count(), 2);
}

#[test]
fn other_reports_and_places_are_refused() {
    let fake = FakeDav::new();
    let unknown = one(&fake, &report(WORK, r#"<d:expand-property xmlns:d="DAV:"/>"#));
    assert_eq!(unknown.status, 403);
    assert!(unknown.body.contains("<d:supported-report/>"));
    assert_eq!(one(&fake, &report(HOME, &sync_body(""))).status, 403);
    assert_eq!(one(&fake, &report("/nothing/", &sync_body(""))).status, 404);
    assert_eq!(one(&fake, &report(WORK, "not xml")).status, 400);
}

#[test]
fn options_and_unknown_methods() {
    let fake = FakeDav::new();
    let options = one(&fake, &request("OPTIONS", "/", &[], ""));
    assert_eq!((options.status, options.field("DAV")), (200, Some("1, 3, calendar-access")));
    assert_eq!(one(&fake, &request("MKCOL", "/x/", &[], "")).status, 405);
}

#[test]
fn requests_follow_one_another_on_one_connection_until_close() {
    let fake = FakeDav::new();
    let input = format!(
        "\r\n{}{}{}",
        request("GET", "/missing.ics", &[], ""),
        request("GET", "/missing.ics?x=1", &["Connection: close"], ""),
        request("GET", "/after-close.ics", &[], "")
    );
    let all = answers(&fake.answer(input.as_bytes()));
    assert_eq!(all.iter().map(|answer| answer.status).collect::<Vec<_>>(), [404, 404]);
}

#[test]
fn the_calendars_handle_sees_what_the_server_holds() {
    let fake = FakeDav::new();
    let calendars = fake.calendars();
    assert_eq!(calendars.put("/nowhere/e.ics", EVENT), None);
    assert_eq!(calendars.put(WORK, EVENT), None);
    assert_eq!(calendars.put("no-slash", EVENT), None);
    assert!(!calendars.remove("/nowhere/e.ics"));
    assert!(!calendars.remove("no-slash"));
    assert_eq!(calendars.get("/nowhere/e.ics"), None);
    assert_eq!(calendars.hrefs("/nowhere/"), Vec::<String>::new());
    assert_eq!(calendars.token("/nowhere/"), None);
    let first = calendars.put(&format!("{WORK}b.ics"), EVENT).unwrap();
    let second = calendars.put(&format!("{WORK}a.ics"), EVENT).unwrap();
    assert_ne!(first, second);
    assert_eq!(calendars.hrefs(WORK), [format!("{WORK}a.ics"), format!("{WORK}b.ics")]);
    assert_eq!(calendars.add(WORK, "Renamed", &[]), WORK);
    let work = one(&fake, &propfind(WORK, "0", "<d:displayname/>"));
    assert!(work.body.contains("<d:displayname>Work</d:displayname>"));
    let default = FakeDav::default();
    assert_eq!(default.calendars().hrefs(WORK), Vec::<String>::new());
}
