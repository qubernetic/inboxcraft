// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde_json::{Value, json};

use super::{MAX_BODY, MAX_RESULT, cap, check, fold, hidden, list, quote, read, search, thread};

#[test]
fn a_quoted_value_keeps_to_its_line_and_its_quotes() {
    assert_eq!(quote("Lunch", 10), "\"Lunch\"");
    assert_eq!(quote("a\"b\\c\nd\re\tf", 20), r#""a\"b\\c\nd\re\tf""#);
    assert_eq!(quote("\u{1b}[2J\u{7}\u{85}", 20), r#""\u001b[2J\u0007\u0085""#);
    assert_eq!(quote("\u{202e}evil\u{2028}\u{feff}\u{200b}", 20), "\"\\u202eevil\\u2028\\ufeff\\u200b\"");
    // What `quote` writes reads back as JSON to the same text.
    let tricky = "x\"\n\u{1b}\u{202e}y";
    assert_eq!(serde_json::from_str::<String>(&quote(tricky, 100)).unwrap(), tricky);
}

#[test]
fn a_long_value_is_cut_on_a_character_and_says_so() {
    assert_eq!(quote("ééééé", 3), "\"ééé\" (cut: 3 of 5 characters)");
    assert_eq!(quote("abc", 3), "\"abc\"");
}

#[test]
fn quoted_lines_fold_into_one_count() {
    assert_eq!(fold("Yes.\n> On Monday\n> you wrote\n\nThanks\n  > again\n"), "Yes.\n[2 quoted lines folded]\n\nThanks\n[1 quoted line folded]\n");
    assert_eq!(fold("plain"), "plain\n");
    assert_eq!(fold(""), "");
}

#[test]
fn a_list_says_where_the_next_page_starts() {
    let message = json!({ "id": 3, "date": "2026-11-02T08:30:00Z", "from": [{ "name": null, "address": "a@example.org" }], "subject": "Hi",
        "seen": true, "flagged": false, "flags": ["\\Seen", "\\Answered", "\\Draft"] });
    let answer = json!({ "view": { "folder": 2 }, "account": 1, "total": 30, "offset": 5, "messages": [message] });
    let text = list(&json!({}), &answer);
    assert!(text.starts_with("mail_list: folder 2 of account 1, 6-6 of 30; next page: offset 6\n"), "{text}");
    assert!(text.contains("\n3 | 2026-11-02 08:30 | \"a@example.org\" | \"Hi\" | answered draft\n"), "{text}");
    let empty = list(&json!({}), &json!({ "view": { "role": "trash" }, "total": 0, "offset": 0, "messages": [] }));
    assert_eq!(empty, "mail_list: trash, none of 0 here\n");
    let odd = list(&json!({}), &json!({ "view": null, "messages": [{ "date": "2026", "subject": null, "from": [] }] }));
    assert!(odd.starts_with("mail_list:, 1-1 of 0\n"), "{odd}");
    assert!(odd.contains("\n0 | 2026 | - | - | -\n"), "{odd}");
}

#[test]
fn a_cut_thread_says_how_many_are_shown() {
    let answer = json!({ "total": 1200, "messages": [{ "id": 1 }] });
    assert!(thread(&json!({ "id": 1 }), &answer).starts_with("thread_read: message 1, a thread of 1200, oldest first; the newest 1 shown\n"));
}

fn read_text(body: Value, extra: &[(&str, Value)]) -> String {
    let mut answer = json!({ "message": { "id": 1, "folder": 1, "account": 1, "date": null, "from": [], "to": [], "cc": [],
        "subject": "s", "seen": true, "body": body } });
    for (key, value) in extra {
        answer[*key] = value.clone();
    }
    read(&json!({}), &answer)
}

#[test]
fn a_read_says_why_there_is_no_text() {
    assert!(read_text(Value::Null, &[]).contains("body: none: not kept on this computer yet"));
    assert!(read_text(Value::Null, &[("body_error", json!("too_large"))]).contains("body: none: the message is too large"));
    assert!(read_text(json!({ "readable": false }), &[]).contains("body: none: the message cannot be read"));
    assert!(read_text(json!({ "readable": true, "text": null, "has_html": true }), &[]).contains("body: none: the message is HTML only"));
    assert!(read_text(json!({ "readable": true, "text": null, "has_html": false }), &[]).contains("body: none: the message has no text"));
    let text = read_text(json!({ "readable": true, "text": "x".repeat(MAX_BODY + 10) }), &[]);
    assert!(text.contains(&format!("(cut: {MAX_BODY} of {} characters)", MAX_BODY + 11)), "{text}");
    assert!(!text.contains("\ncc:"), "an empty cc is left out: {text}");
}

#[test]
fn a_read_lists_the_attachments() {
    let body = json!({ "readable": true, "text": "see attached", "attachments": [
        { "index": 0, "name": "plan.pdf", "type": "application/pdf", "size": 1234 },
        { "index": 1, "name": null, "type": null, "size": null },
    ]});
    let text = read_text(body, &[]);
    assert!(text.contains("attachments: 0 \"plan.pdf\" \"application/pdf\" 1234 bytes; 1 - - size unknown\n"), "{text}");
}

#[test]
fn a_check_lists_its_steps_and_quotes_the_issuer() {
    let steps = json!([
        { "step": "dns", "status": "passed", "message": null, "host": "imap.example.org", "port": 993, "issuer": null },
        { "step": "certificate", "status": "failed", "message": "the certificate is not trusted", "host": null, "port": null,
          "issuer": "Evil\nCA" },
        { "step": "smtp", "status": "not_built" },
    ]);
    let started = check(&json!({}), &json!({ "account": 1, "started": true, "steps": steps }));
    assert!(started.starts_with("account_check: account 1: a check was asked for, but this server does no network I/O"), "{started}");
    assert!(started.contains("\ndns | passed | imap.example.org:993\n"), "{started}");
    assert!(started.contains("\ncertificate | failed | the certificate is not trusted; issuer \"Evil\\nCA\"\n"), "{started}");
    assert!(started.contains("\nsmtp | not_built | -\n"), "{started}");
    let running = check(&json!({}), &json!({ "account": 1, "started": false, "steps": [] }));
    assert_eq!(running, "account_check: account 1: a check is already running\nstep | status | detail\n");
}

/// The characters that turn or hide text and must never be written raw: one of each run the security review found.
const HIDDEN: [char; 30] = [
    '\u{61c}',
    '\u{ad}',
    '\u{34f}',
    '\u{115f}',
    '\u{180e}',
    '\u{206a}',
    '\u{206f}',
    '\u{3164}',
    '\u{fe00}',
    '\u{fe0f}',
    '\u{fff9}',
    '\u{fffa}',
    '\u{fffb}',
    '\u{1d173}',
    '\u{1d17a}',
    '\u{e0000}',
    '\u{e0001}',
    '\u{e0041}',
    '\u{e007f}',
    '\u{e0100}',
    '\u{e01ef}',
    '\u{200b}',
    '\u{200f}',
    '\u{202a}',
    '\u{202e}',
    '\u{2028}',
    '\u{2029}',
    '\u{2066}',
    '\u{2069}',
    '\u{feff}',
];

#[test]
fn a_character_that_turns_or_hides_text_becomes_an_escape() {
    for c in HIDDEN {
        let text = format!("a{c}b");
        let quoted = quote(&text, 10);
        assert!(!quoted.contains(c), "U+{:04X} written raw: {quoted}", u32::from(c));
        assert_eq!(serde_json::from_str::<String>(&quoted).unwrap(), text, "U+{:04X}", u32::from(c));
    }
}

#[test]
fn a_character_above_the_basic_plane_is_a_surrogate_pair() {
    assert_eq!(quote("\u{e0041}", 10), "\"\\udb40\\udc41\"");
    assert_eq!(quote("\u{1d173}", 10), "\"\\ud834\\udd73\"");
    // A visible one is written as it is.
    assert_eq!(quote("\u{1f600}", 10), "\"\u{1f600}\"");
}

proptest::proptest! {
    #[test]
    fn any_text_reads_back_and_hides_nothing(text in proptest::prelude::any::<String>()) {
        let quoted = quote(&text, usize::MAX);
        proptest::prop_assert!(!quoted.chars().any(|c| c.is_control() || hidden(c)), "{quoted:?}");
        proptest::prop_assert_eq!(serde_json::from_str::<String>(&quoted).unwrap(), text);
    }
}

fn person(n: usize) -> Value {
    json!({ "name": format!("Person {n}"), "address": format!("p{n}@example.org") })
}

#[test]
fn a_list_row_names_three_senders_and_counts_the_rest() {
    let from: Vec<Value> = (0..1000).map(person).collect();
    let messages: Vec<Value> = (0..25).map(|id| json!({ "id": id, "from": from, "subject": "s" })).collect();
    let text = list(&json!({}), &json!({ "total": 25, "offset": 0, "messages": messages }));
    assert!(text.len() <= MAX_RESULT, "{} bytes", text.len());
    assert!(text.contains("\"Person 2 <p2@example.org>\" (and 997 more) | \"s\""), "{text}");
    assert!(!text.contains("p3@example.org"), "{text}");
    assert_eq!(text.lines().count(), 28, "{text}");
}

#[test]
fn a_read_names_fifty_people_and_fifty_attachments() {
    let to: Vec<Value> = (0..1000).map(person).collect();
    let attachments: Vec<Value> = (0..1000).map(|index| json!({ "index": index, "name": "a.pdf", "type": "application/pdf", "size": 1 })).collect();
    let answer = json!({ "message": { "id": 1, "from": [], "to": to, "cc": to, "subject": "s",
        "body": { "readable": true, "text": "hi", "attachments": attachments } } });
    let text = read(&json!({}), &answer);
    assert!(text.len() <= MAX_RESULT, "{} bytes", text.len());
    assert!(text.contains("<p49@example.org>\" (and 950 more)\ncc: "), "{text}");
    assert!(text.contains("<p49@example.org>\" (and 950 more)\nsubject: "), "{text}");
    assert!(!text.contains("p50@example.org"), "{text}");
    assert!(text.contains("; 49 \"a.pdf\" \"application/pdf\" 1 bytes (and 950 more)\n"), "{text}");
}

#[test]
fn a_result_longer_than_the_cap_is_cut_on_a_line_and_says_so() {
    let line = "x".repeat(99);
    let long = format!("{line}\n").repeat(MAX_RESULT / 50);
    let cut = cap(long.clone());
    assert!(cut.len() <= MAX_RESULT, "{} bytes", cut.len());
    assert!(
        cut.ends_with(&format!("\n[cut: the result was {} bytes, the first {} shown]\n", long.len(), (MAX_RESULT - 100) / 100 * 100)),
        "{}",
        &cut[cut.len() - 200..]
    );
    // One line longer than the cap is cut on a character.
    let one = "é".repeat(MAX_RESULT);
    let cut = cap(one.clone());
    assert!(cut.len() <= MAX_RESULT && cut.starts_with("éé"), "{} bytes", cut.len());
    assert!(cut.ends_with(" shown]\n") && cut.contains(&format!("\n[cut: the result was {} bytes, the first ", one.len())), "{cut:.80}");
    assert_eq!(cap("short\n".to_owned()), "short\n");
}

/// `n` tag characters: each is written as a 12-byte surrogate pair escape.
fn tags(n: usize) -> String {
    "\u{e0041}".repeat(n)
}

/// A message whose 3 senders and subject are `n` tag characters each.
fn heavy(id: usize, n: usize) -> Value {
    let from: Vec<Value> = (0..3).map(|k| json!({ "name": tags(n), "address": format!("p{k}@example.org") })).collect();
    json!({ "id": id, "from": from, "subject": tags(n) })
}

fn plain(id: usize) -> Value {
    json!({ "id": id, "from": [person(id)], "subject": "s" })
}

/// The rows of a table result: the lines after the header.
fn rows(text: &str) -> Vec<&str> {
    text.lines().skip(3).collect()
}

#[test]
fn a_quoted_value_stops_at_its_bytes_too() {
    // 200 characters for a field, but tag characters take 12 bytes each: the value stops at 1200 bytes.
    let quoted = quote(&tags(200), 200);
    assert!(quoted.len() <= 1200 + 40, "{} bytes", quoted.len());
    assert!(quoted.ends_with("\" (cut: 100 of 200 characters)"), "{quoted:.40}");
    assert_eq!(serde_json::from_str::<String>(quoted.split(" (cut").next().unwrap()).unwrap(), tags(100));
}

#[test]
fn heavy_senders_leave_every_row_of_a_page_in_place() {
    let messages: Vec<Value> = (1..=12).map(|id| heavy(id, 150)).chain((13..=25).map(plain)).collect();
    let text = list(&json!({}), &json!({ "view": { "role": "inbox" }, "total": 40, "offset": 0, "messages": messages }));
    assert!(text.len() <= MAX_RESULT, "{} bytes", text.len());
    assert!(text.starts_with("mail_list: inbox, 1-25 of 40; next page: offset 25\n"), "{text:.80}");
    assert_eq!(rows(&text).len(), 25);
    assert!(rows(&text).last().unwrap().starts_with("25 | "), "{text}");
}

#[test]
fn a_page_too_big_for_the_cap_says_where_the_rows_shown_end() {
    let messages: Vec<Value> = (1..=25).map(|id| heavy(id, 200)).collect();
    let answer = json!({ "view": { "role": "inbox" }, "total": 40, "offset": 10, "messages": messages });
    let text = list(&json!({}), &answer);
    assert!(text.len() <= MAX_RESULT, "{} bytes", text.len());
    let shown = rows(&text).len();
    assert!(shown > 0 && shown < 25, "{shown} rows");
    let last = 10 + shown;
    assert!(text.starts_with(&format!("mail_list: inbox, 11-{last} of 40; next page: offset {last}\n")), "{text:.80}");
    assert!(rows(&text).last().unwrap().starts_with(&format!("{shown} | ")), "{text:.80}");

    let found = search(&json!({}), &json!({ "total": 25, "messages": answer["messages"] }));
    assert!(found.len() <= MAX_RESULT, "{} bytes", found.len());
    let shown = rows(&found).len();
    assert!(found.starts_with(&format!("mail_search: 25 found, newest first; the first {shown} shown, the rest did not fit\n")), "{found:.120}");

    let threaded = thread(&json!({ "id": 1 }), &json!({ "total": 25, "messages": answer["messages"] }));
    assert!(threaded.len() <= MAX_RESULT, "{} bytes", threaded.len());
    let shown = rows(&threaded).len();
    assert!(
        threaded.starts_with(&format!("thread_read: message 1, a thread of 25, oldest first; the first {shown} shown, the rest did not fit\n")),
        "{threaded:.120}"
    );
    let long = thread(&json!({ "id": 1 }), &json!({ "total": 1200, "messages": answer["messages"] }));
    let expected =
        format!("thread_read: message 1, a thread of 1200, oldest first; of the newest 25, the first {shown} shown, the rest did not fit\n");
    assert!(long.starts_with(&expected), "{long:.120}");
}

#[test]
fn a_body_of_escapes_keeps_its_line_and_says_it_was_cut() {
    let text = read_text(json!({ "readable": true, "text": tags(MAX_BODY) }), &[]);
    assert!(text.len() <= MAX_RESULT, "{} bytes", text.len());
    let body = text.lines().find(|line| line.starts_with("body: \"")).unwrap_or_else(|| panic!("no body line: {text:.200}"));
    assert!(body.contains(&format!(" of {} characters)", MAX_BODY + 1)), "{:.80}", &body[body.len() - 80..]);
}

#[test]
fn heavy_headers_leave_room_for_the_body() {
    let people: Vec<Value> = (0..50).map(|k| json!({ "name": tags(200), "address": format!("p{k}@example.org") })).collect();
    let attachments: Vec<Value> = (0..50).map(|index| json!({ "index": index, "name": tags(200), "type": tags(200), "size": 1 })).collect();
    let answer = json!({ "message": { "id": 1, "from": people, "to": people, "cc": people, "reply_to": people, "subject": tags(200),
        "body": { "readable": true, "text": tags(MAX_BODY), "attachments": attachments } } });
    let text = read(&json!({}), &answer);
    assert!(text.len() <= MAX_RESULT, "{} bytes", text.len());
    for key in ["from", "to", "cc", "reply_to"] {
        let line = text.lines().find(|line| line.starts_with(&format!("{key}: "))).unwrap_or_else(|| panic!("no {key}: {text:.200}"));
        assert!(line.contains(" more)"), "{key}: {:.80}", &line[line.len() - 80..]);
    }
    let attached = text.lines().find(|line| line.starts_with("attachments: ")).unwrap();
    assert!(attached.ends_with(" more)"), "{:.80}", &attached[attached.len() - 80..]);
    assert!(text.lines().any(|line| line.starts_with("body: \"") && line.contains(" characters)")), "{:.200}", &text[text.len() - 200..]);
}
