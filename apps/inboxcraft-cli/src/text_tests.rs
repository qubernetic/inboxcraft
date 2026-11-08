// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde_json::json;

use super::render;

#[test]
fn an_object_is_one_field_a_line_nested_by_two_spaces() {
    let value = json!({ "id": 1, "name": "INBOX", "seen": true, "thread": null, "view": { "role": "inbox" } });
    assert_eq!(render(&value), "id: 1\nname: INBOX\nseen: true\nthread: null\nview:\n  role: inbox\n");
}

#[test]
fn a_list_is_one_item_a_line() {
    let value = json!({ "flags": ["\\Seen", "\\Flagged"], "messages": [{ "id": 1 }, { "id": 2, "to": [] }], "empty": {} });
    assert_eq!(render(&value), "empty: {}\nflags:\n  - \\Seen\n  - \\Flagged\nmessages:\n  -\n    id: 1\n  -\n    id: 2\n    to: []\n");
}

#[test]
fn a_bare_value_is_one_line() {
    assert_eq!(render(&json!("plain")), "plain\n");
    assert_eq!(render(&json!([1, [2]])), "- 1\n-\n  - 2\n");
}

#[test]
fn text_of_several_lines_is_a_block() {
    let value = json!({ "text": "Hello,\r\nsee you.\n" });
    assert_eq!(render(&value), "text: |\n  Hello,\n  see you.\n");
}

#[test]
fn control_and_direction_characters_never_reach_the_terminal() {
    let value = json!({ "subject": "Paid\u{1b}[2J\u{7}\u{9b}31m \u{202e}fdp.exe\u{2066}", "text": "one\n\u{1b}]0;title\u{7}two" });
    let shown = render(&value);
    assert!(
        !shown.chars().any(|c| c != '\n' && (c.is_control() || ('\u{202a}'..='\u{202e}').contains(&c) || ('\u{2066}'..='\u{2069}').contains(&c))),
        "{shown:?}"
    );
    assert!(shown.contains("Paid\\u{1b}[2J\\u{7}\\u{9b}31m \\u{202e}fdp.exe\\u{2066}"), "{shown}");
    assert!(shown.contains("  \\u{1b}]0;title\\u{7}two\n"), "{shown}");
}

#[test]
fn a_key_is_cleaned_too() {
    assert_eq!(render(&json!({ "a\u{1b}b": 1 })), "a\\u{1b}b: 1\n");
}

#[test]
fn a_tab_stays() {
    assert_eq!(render(&json!({ "text": "a\tb" })), "text: a\tb\n");
}
