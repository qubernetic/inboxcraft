// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-FileCopyrightText: MailCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::unescape;

#[test]
fn named_references() {
    assert_eq!(unescape("&lt;b&gt; &amp; &quot;x&quot; &apos;y&apos;"), "<b> & \"x\" 'y'");
    assert_eq!(unescape("a&nbsp;b"), "a\u{a0}b");
    assert_eq!(unescape("&copy; &reg; &trade; &hellip; &mdash; &ndash;"), "© ® ™ … — –");
    assert_eq!(unescape("&lsquo;a&rsquo; &ldquo;b&rdquo; &euro;5"), "‘a’ “b” €5");
    assert_eq!(unescape("caf&eacute; na&iuml;ve &Uuml;ber &szlig; &yuml;"), "café naïve Über ß ÿ");
    assert_eq!(unescape("&alpha;&beta;&Omega;&sigmaf; &rarr; &le; &ne;"), "αβΩς → ≤ ≠");
    assert_eq!(unescape("&zwnj;"), "\u{200c}");
}

#[test]
fn names_are_case_sensitive() {
    assert_eq!(unescape("&Eacute;&eacute;"), "Éé");
    assert_eq!(unescape("&EACUTE;"), "&EACUTE;");
    assert_eq!(unescape("&AMP;&LT;"), "&<");
}

#[test]
fn numeric_references() {
    assert_eq!(unescape("&#233;&#xE9;&#Xe9;"), "ééé");
    assert_eq!(unescape("&#128512;"), "😀");
    assert_eq!(unescape("&#x1F600;"), "😀");
    // Without the semicolon.
    assert_eq!(unescape("&#65 &#x42."), "A B.");
    // Windows-1252 for the C1 range.
    assert_eq!(unescape("&#147;hi&#148; &#128; &#x99;"), "“hi” € ™");
}

#[test]
fn bad_numbers_become_the_replacement_character() {
    assert_eq!(unescape("&#0;"), "\u{fffd}");
    assert_eq!(unescape("&#xD800;"), "\u{fffd}");
    assert_eq!(unescape("&#x110000;"), "\u{fffd}");
    assert_eq!(unescape("&#99999999999999999999999999;"), "\u{fffd}");
    assert_eq!(unescape("&#xFFFFFFFFFFFFFFFFFFFF;"), "\u{fffd}");
}

#[test]
fn invalid_references_are_left_alone() {
    for s in ["&", "&&", "& ", "&;", "&#;", "&#x;", "&#xZZ;", "&bogus;", "AT&T", "a & b", "&amp", "x&y=z", "&#", "&#x", "&eacute", "ünï&"] {
        let want = if s == "&amp" { "&" } else { s };
        assert_eq!(unescape(s), want, "{s}");
    }
}

#[test]
fn legacy_names_work_without_a_semicolon_except_in_queries() {
    assert_eq!(unescape("a &lt b &gt c"), "a < b > c");
    assert_eq!(unescape("x&nbsp"), "x\u{a0}");
    assert_eq!(unescape("?a=1&amp=2&lt=3"), "?a=1&amp=2&lt=3");
    assert_eq!(unescape("?a=1&amp;b=2"), "?a=1&b=2");
    // A longer word that starts like a legacy name is not one.
    assert_eq!(unescape("&amplify"), "&amplify");
}

#[test]
fn a_long_run_of_ampersands_is_quick() {
    let s = "&".repeat(200_000);
    assert_eq!(unescape(&s), s);
    let s = "&#".repeat(100_000);
    assert_eq!(unescape(&s), s);
}

proptest::proptest! {
    #[test]
    fn unescape_never_panics(s in "[&#xX0-9a-zA-Z;= é]{0,60}") {
        let _ = unescape(&s);
    }
}
