// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The queue of owner requests, as plain state: eight wait at most, only the one in front is asked and runs out of
//! time, after two minutes from when it came to the front, and nothing leaves it but by a settlement.

use std::time::Duration;

use inboxcraft_time::Timestamp;
use serde_json::json;

use super::{MOST_NAME_CHARS, MOST_WAITING, OwnerRequests, PATIENCE, Settled, data};
use crate::component::hidden;
use crate::unicode_class::{Class, class};

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_unix(1_793_869_200 + seconds, 0).unwrap()
}

#[test]
fn eight_wait_and_the_ninth_is_turned_away() {
    let mut requests = OwnerRequests::default();
    let tickets: Vec<_> = (0..8).map(|_| requests.push(None, "event.create", json!({}))).collect();
    assert!(tickets.iter().all(Option::is_some));
    assert_eq!(MOST_WAITING, 8);
    assert_eq!(requests.push(None, "event.create", json!({})), None);
    let mut seen = tickets.clone();
    seen.dedup();
    assert_eq!(seen.len(), 8, "every ticket is its own");
    requests.settle_front(Settled::Denied);
    assert!(requests.push(None, "event.create", json!({})).is_some(), "a settled request makes room");
}

#[test]
fn only_the_one_in_front_runs_out_of_time_and_after_two_minutes() {
    let mut requests = OwnerRequests::default();
    let (first, second) = (requests.push(None, "event.create", json!({})).unwrap(), requests.push(None, "event.create", json!({})).unwrap());
    requests.expire(at(0));
    requests.expire(at(119));
    assert!(requests.take_settled().is_empty());
    requests.expire(at(120));
    assert!(matches!(requests.take_settled().as_slice(), [(ticket, Settled::TimedOut)] if *ticket == first));
    requests.expire(at(239)); // the second's two minutes began at 120
    assert!(requests.take_settled().is_empty());
    requests.expire(at(240));
    assert!(matches!(requests.take_settled().as_slice(), [(ticket, Settled::TimedOut)] if *ticket == second));
    assert!(!requests.waiting());
}

#[test]
fn the_front_is_the_oldest_and_keeps_what_was_asked() {
    let mut requests = OwnerRequests::default();
    assert!(requests.front().is_none());
    let first = requests.push(Some("inboxcraft-cli"), "calendar.import", json!({ "calendar": 1 })).unwrap();
    requests.push(None, "event.create", json!({})).unwrap();
    let front = requests.front().unwrap();
    assert_eq!(front.ticket, first);
    assert_eq!(front.client.as_deref(), Some("inboxcraft-cli"));
    assert_eq!(front.command, "calendar.import");
    assert_eq!(front.params, json!({ "calendar": 1 }));
    assert_eq!(front.since, None);
}

#[test]
fn settling_answers_the_one_in_front_only() {
    let mut requests = OwnerRequests::default();
    requests.settle_front(Settled::Denied);
    assert!(requests.take_settled().is_empty(), "nothing waits, nothing is settled");
    let first = requests.push(None, "event.create", json!({})).unwrap();
    let second = requests.push(None, "event.create", json!({})).unwrap();
    requests.settle_front(Settled::Ran(Ok(json!({ "added": 1 }))));
    assert_eq!(requests.front().unwrap().ticket, second);
    let settled = requests.take_settled();
    assert!(matches!(settled.as_slice(), [(ticket, Settled::Ran(Ok(answer)))] if *ticket == first && answer["added"] == 1));
    assert!(requests.take_settled().is_empty(), "taken once");
}

#[test]
fn due_in_counts_down_the_front_s_two_minutes() {
    let mut requests = OwnerRequests::default();
    assert_eq!(requests.due_in(at(0)), None);
    requests.push(None, "event.create", json!({})).unwrap();
    assert_eq!(requests.due_in(at(0)), Some(Duration::ZERO), "not dated yet: look at once");
    requests.expire(at(0));
    assert_eq!(requests.due_in(at(0)), Some(PATIENCE));
    assert_eq!(requests.due_in(at(100)), Some(Duration::from_secs(20)));
    assert_eq!(requests.due_in(at(500)), Some(Duration::ZERO));
}

#[test]
fn a_clock_that_goes_back_does_not_date_the_front_again_and_never_ends_it_early() {
    let mut requests = OwnerRequests::default();
    let ticket = requests.push(None, "event.create", json!({})).unwrap();
    requests.expire(at(100));
    requests.expire(at(-1000));
    assert!(requests.take_settled().is_empty());
    assert_eq!(requests.due_in(at(-1000)), Some(PATIENCE), "a clock behind the date has waited nothing");
    requests.expire(at(-880));
    assert!(requests.take_settled().is_empty(), "not dated again at the clock's step back");
    requests.expire(at(219));
    assert!(requests.take_settled().is_empty());
    requests.expire(at(220));
    assert!(matches!(requests.take_settled().as_slice(), [(got, Settled::TimedOut)] if *got == ticket));
}

#[test]
fn a_withdrawn_request_leaves_the_queue_unanswered() {
    let mut requests = OwnerRequests::default();
    let first = requests.push(None, "event.create", json!({})).unwrap();
    let second = requests.push(None, "event.create", json!({})).unwrap();
    requests.expire(at(0));
    assert!(requests.withdraw(first));
    assert!(!requests.withdraw(first), "only once");
    assert_eq!(requests.front().map(|front| front.ticket), Some(second));
    assert_eq!(requests.front().and_then(|front| front.since), None, "its two minutes start when it is next looked at");
    assert!(requests.withdraw(second));
    assert!(!requests.waiting());
    assert!(requests.take_settled().is_empty());
}

#[test]
fn what_came_from_outside_is_drawn_plain_and_cut() {
    // Bidi overrides and isolates, zero-width characters, separators and the byte order mark are left out; a control
    // character is a space.
    let name = "Inbox\u{202E}Craft\u{2066}\u{200B}\u{2028}\u{FEFF}\nAllow";
    assert_eq!(data(name, MOST_NAME_CHARS), "InboxCraft Allow");
    for c in ['\u{202A}', '\u{202E}', '\u{2066}', '\u{2069}', '\u{200B}', '\u{200F}', '\u{2028}', '\u{2029}', '\u{FEFF}'] {
        assert!(hidden(c), "{c:?}");
    }
    assert!(!hidden('a') && !hidden('é'));
    let cut = data(&"x".repeat(700), MOST_NAME_CHARS);
    assert_eq!(cut.chars().count(), MOST_NAME_CHARS + 1);
    assert!(cut.ends_with('…'));
    assert_eq!(data("  \u{202E}\n ", MOST_NAME_CHARS), "");
}

#[test]
fn hidden_characters_never_empty_a_name() {
    // The characters that are never drawn are left out before the cut: they cannot push the name out of it.
    assert_eq!(data(&format!("{}Studio", "\u{200B}".repeat(5_000)), MOST_NAME_CHARS), "Studio");
    // Every format character is one never drawn (Unicode 15.1, general category Cf), among them those egui draws
    // nothing for and the tags.
    for c in [
        '\u{00AD}',
        '\u{0600}',
        '\u{180E}',
        '\u{206A}',
        '\u{206F}',
        '\u{FFF9}',
        '\u{FFFB}',
        '\u{110BD}',
        '\u{1D173}',
        '\u{E0001}',
        '\u{E0041}',
        '\u{E007F}',
    ] {
        assert!(crate::component::hidden(c), "U+{:04X}", u32::from(c));
        assert_eq!(data(&format!("a{c}b"), MOST_NAME_CHARS), "ab", "U+{:04X}", u32::from(c));
    }
}

#[test]
fn data_cannot_close_its_own_quote_and_has_one_space_between_words() {
    // The quotes the question puts around data stand for an apostrophe inside it.
    assert_eq!(data("cli” that you started, “x", MOST_NAME_CHARS), "cli' that you started, 'x");
    // Every mark `quote` holds, read from the set itself.
    let marks: Vec<char> = (0..=0x10_FFFF_u32).filter_map(char::from_u32).filter(|c| super::quote(*c)).collect();
    for wanted in ['"', '“', '”', '〝', '〞', '〟', '″', '‶', '˝', 'ʺ', 'ˮ', '״', '〃', 'ʼ', '\u{0384}', '\u{02BB}', '\u{030B}', '\u{00A8}']
    {
        assert!(marks.contains(&wanted), "{wanted:?} is a quote mark");
    }
    for quote in marks {
        assert_eq!(data(&format!("a{quote}b"), MOST_NAME_CHARS), "a'b", "U+{:04X}", u32::from(quote));
    }
    // A run of apostrophe-like marks reads as a double quote: it is one apostrophe.
    for run in ["''", "’’", "‘‘", "ʼʼ", "'ʼ'", "″'", "' '"] {
        let said = data(&format!("Dentist{run} · safe"), MOST_NAME_CHARS);
        assert!(!said.contains("''"), "{run:?}: {said:?}");
    }
    assert_eq!(data("It’’s", MOST_NAME_CHARS), "It's");
    // A character never drawn or a combining mark between two of them makes no two apostrophes; a combining mark on an
    // apostrophe is left out, and the double acute is a quote mark wherever it stands.
    for title in [
        "Dentist'\u{206A}'",
        "Dentist'\u{0301}'",
        "Dentist\u{030B}",
        "Dentist\u{02BB}\u{02BB}",
        "Dentist'\u{034F}\u{0301}\u{E0041}’",
        "Dentist\u{00A8}",
    ] {
        assert_eq!(data(title, MOST_NAME_CHARS), "Dentist'", "{title:?}");
    }
    assert_eq!(data("Dentist'\u{0301}", MOST_NAME_CHARS), "Dentist'", "no mark on an apostrophe");
    assert_eq!(data("Cafe\u{0301}", MOST_NAME_CHARS), "Cafe\u{0301}", "a mark on a letter stays");
    // Every run of white space, no-break and wide spaces among it, is one space.
    assert_eq!(data("Wide \u{00A0}\u{2003}\u{3000}  \t\r\n gap", MOST_NAME_CHARS), "Wide gap");
    assert_eq!(data("\u{00A0} edges \u{2003}", MOST_NAME_CHARS), "edges");
}

#[test]
fn a_mark_stands_only_on_a_letter_or_a_number() {
    // A combining mark after a space, at the start, after punctuation or a symbol would draw as its spacing twin (two
    // acutes as a double acute, a diaeresis): it is left out, and the comma above is a quote mark wherever it stands.
    for (text, plain) in [
        ("cli \u{0301} \u{0301} that you started", "cli that you started"),
        ("Dentist \u{0301} \u{0301} · safe", "Dentist · safe"),
        ("Dentist \u{0308} x", "Dentist x"),
        ("Dentist\u{0315}' x", "Dentist' x"),
        ("\u{0301}Dentist", "Dentist"),
        ("\u{0308} Dentist", "Dentist"),
        ("Dentist-\u{0308}x", "Dentist-x"),
        ("Dentist €\u{0301}\u{0301}", "Dentist €"),
        ("Dentist\u{E000}\u{0378} \u{0670} x", "Dentist x"),
        ("Dentist \u{0301}", "Dentist"),
    ] {
        assert_eq!(data(text, MOST_NAME_CHARS), plain, "{text:?}");
    }
}

/// What is wrong with `said` as data inside a question's quotes, as [`data`] must leave it; `None` when nothing is.
pub(crate) fn fault(said: &str) -> Option<&'static str> {
    let mut before: Option<char> = None;
    for c in said.chars() {
        let kind = class(c);
        if c != '\'' && (super::quote(c) || kind == Class::QuoteLike) {
            return Some("a mark that could pass for a quote");
        }
        if c == '\'' && before == Some('\'') {
            return Some("two apostrophes in a row");
        }
        if c == ' ' && matches!(before, None | Some(' ')) {
            return Some("a space at the start or after a space");
        }
        if kind == Class::Other && c != ' ' {
            return Some("a character of no kind a question keeps");
        }
        if kind == Class::Mark && !before.is_some_and(|on| matches!(class(on), Class::Letter | Class::Number | Class::Mark)) {
            return Some("a mark with no letter or number to stand on");
        }
        before = Some(c);
    }
    (before == Some(' ')).then_some("a space at the end")
}

#[test]
fn the_fault_finder_finds_each_fault() {
    for wrong in
        ["a”b", "a\u{02BC}b", "a´b", "a''b", " a", "a  b", "a ", "a\u{E000}b", "a\u{00A0}b", "\u{0301}a", "a \u{0301}", "a'\u{0301}", "-\u{0308}"]
    {
        assert!(fault(wrong).is_some(), "{wrong:?}");
    }
    for right in ["", "a", "It's a café – no. 2 (ok) 🎉", "Cafe\u{0301}", "e\u{0301}\u{0302}", "1\u{20E3}", "cut…"] {
        assert_eq!(fault(right), None, "{right:?}");
    }
}

#[test]
fn no_character_makes_data_draw_a_quote_or_a_mark_of_its_own() {
    // Every scalar value, in each place it could stand: at the start, between letters, between spaces, between
    // apostrophes, after punctuation, twice in a row, on a number and at the end.
    for c in (0..=0x10_FFFF_u32).filter_map(char::from_u32) {
        let said = data(&format!("{c}b a{c}b a {c} b a'{c}'b -{c} {c}{c} 1{c} €{c}'{c}"), 200);
        assert_eq!(fault(&said), None, "U+{:04X}: {said:?}", u32::from(c));
    }
    // A dense sample of what the marks, the quotes and the characters left out make of each other, cut short too.
    let pool: Vec<char> = "a1 '”-€🎉é\n\u{0301}\u{0308}\u{030B}\u{0315}\u{20E3}\u{FE0F}\u{200B}\u{E000}\u{02BC}´\u{00A0}ou".chars().collect();
    let mut seed = 0x2545_F491_u32;
    for _ in 0..40_000 {
        let mut text = String::new();
        for _ in 0..14 {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let at = usize::try_from(seed >> 16).unwrap_or(0) % pool.len();
            text.extend(pool.get(at));
        }
        for most in [4, 9, 80] {
            let said = data(&text, most);
            assert_eq!(fault(said.strip_suffix('…').unwrap_or(&said).trim_end()), None, "{text:?} cut to {most}: {said:?}");
            assert!(said.chars().count() <= most + 1, "{text:?}: {said:?}");
        }
    }
}

#[test]
fn ordinary_titles_keep_their_letters_accents_digits_emoji_and_punctuation() {
    for title in [
        "Zoë and Renée at the café: crème brûlée, jalapeño & piñata",
        "Erdős és Győr, tűzoltó – Ő is jön",
        "Cafe\u{0301} man\u{0303}ana, Zoe\u{0308}",
        "Call 2 - room 3/4 (north) – or — south, east. Why: fun! Ok?",
        "Q3 review, 10:30 #7 @ 5% + 20 € = ½ × 3² · §4 [a] {b} *c* _d_ a\\b ~1 <2> |3| $4",
        "🎉 Party 👍 at 8, 1\u{FE0F}\u{20E3} cake 🍰",
        "歯医者の予約 午後3時",
        "Встреча в 5, Ελληνικά, עברית, العربية",
        "مُحَمَّد and हिन्दी and ไทย and Việt Nam",
    ] {
        assert_eq!(data(title, MOST_NAME_CHARS), title);
    }
    // The double acute stands only where it makes a letter of its own, ő and ű, which it then is.
    assert_eq!(
        data("Erdo\u{030B}s e\u{0301}s Gyo\u{030B}r, tu\u{030B}zolto\u{0301}, O\u{030B} U\u{030B}", MOST_NAME_CHARS),
        "Erdős e\u{0301}s Győr, tűzolto\u{0301}, Ő Ű"
    );
    assert_eq!(data("o \u{030B} a\u{030B} o\u{0301}\u{030B}", MOST_NAME_CHARS), "o ' a' o\u{0301}'");
    // What is of no kind to keep is left out; a quote, a modifier letter and a modifier symbol are an apostrophe.
    assert_eq!(data("a\u{E000}\u{0378}\u{F0000}b", MOST_NAME_CHARS), "ab");
    for mark in
        ['\u{02C8}', '\u{02CA}', '\u{02F5}', '\u{A78C}', '\u{07F4}', '\u{1FBF}', '\u{1FCD}', '\u{2E42}', '\u{0385}', '^', '\u{2E03}', '\u{1F3FD}']
    {
        assert_eq!(data(&format!("a{mark}b"), MOST_NAME_CHARS), "a'b", "U+{:04X}", u32::from(mark));
    }
}
