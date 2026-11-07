// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde_json::{Value, json};

use super::{MAX_BODY, check, fold, list, quote, read, thread};

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
