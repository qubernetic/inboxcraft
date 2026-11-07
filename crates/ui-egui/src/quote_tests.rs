// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{MAX_SEGMENTS, MAX_TEXT, Segment, split};

fn text(s: &str) -> Segment {
    Segment::Text(s.to_owned())
}

fn quote(attribution: Option<&str>, s: &str, lines: usize) -> Segment {
    Segment::Quote { attribution: attribution.map(str::to_owned), text: s.to_owned(), lines }
}

#[test]
fn text_without_quotes_is_one_segment() {
    assert_eq!(split("Hello,\r\nsee you at seven.\r\n"), [text("Hello,\nsee you at seven.")]);
    assert_eq!(split(""), []);
    assert_eq!(split("\r\n\r\n"), []);
}

#[test]
fn a_reply_folds_the_quote_with_its_attribution() {
    let reply = "The second week works for me.\r\n\r\nOn Thursday, Odile Brannock wrote:\r\n> Hi all,\r\n>\r\n> could you take the second week?\r\n";
    assert_eq!(
        split(reply),
        [text("The second week works for me."), quote(Some("On Thursday, Odile Brannock wrote:"), "Hi all,\n\ncould you take the second week?", 3)]
    );
}

#[test]
fn quotes_in_between_answers_stay_in_place_and_nested_levels_keep_their_marks() {
    let reply = "> > the first\n> the second\nMy answer.\n\n>third\n\nThanks";
    assert_eq!(split(reply), [quote(None, "> the first\nthe second", 2), text("My answer."), quote(None, "third", 1), text("Thanks")]);
}

#[test]
fn a_blank_line_inside_a_quote_keeps_it_one_quote() {
    assert_eq!(split("> one\n\n> two\nend"), [quote(None, "one\n\ntwo", 3), text("end")]);
}

#[test]
fn an_original_message_marker_folds_everything_after_it() {
    let forwarded = "See below.\n\n-----Original Message-----\nFrom: Teodor\nSubject: Board games\n\nSaturday?";
    assert_eq!(
        split(forwarded),
        [text("See below."), quote(Some("-----Original Message-----"), "From: Teodor\nSubject: Board games\n\nSaturday?", 4)]
    );
}

#[test]
fn a_line_ending_in_a_colon_far_from_the_quote_is_no_attribution() {
    assert_eq!(split("Here is the list:\n\n\n> milk"), [text("Here is the list:"), quote(None, "milk", 1)]);
    let long = format!("{}:", "x".repeat(300));
    assert_eq!(split(&format!("{long}\n> milk")), [text(&long), quote(None, "milk", 1)]);
}

#[test]
fn control_and_bidi_characters_never_reach_the_screen() {
    assert_eq!(split("a\u{0}b\u{1b}[2J\tc\u{202e}d\u{2066}e\r"), [text("ab[2J\tcde")]);
}

#[test]
fn hostile_sizes_stay_within_the_caps() {
    let huge = "é".repeat(MAX_TEXT);
    let segments = split(&huge);
    let shown: usize = segments
        .iter()
        .map(|segment| match segment {
            Segment::Text(text) | Segment::Quote { text, .. } => text.len(),
        })
        .sum();
    assert!(shown <= MAX_TEXT, "{shown}");
    let alternating = "> q\nt\n".repeat(MAX_SEGMENTS * 2);
    let segments = split(&alternating);
    assert!(segments.len() <= MAX_SEGMENTS, "{}", segments.len());
    assert!(matches!(segments.last(), Some(Segment::Text(rest)) if rest.contains("> q")), "the rest is shown as text");
}
