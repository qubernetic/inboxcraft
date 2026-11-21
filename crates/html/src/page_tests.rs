// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{MAX_TITLE, page};
use crate::{Block, Cell, Document, Look, Options, RemoteContent, display};

fn allowed() -> Options {
    Options { remote_content: RemoteContent::Allowed }
}

/// What a browser could load or follow: an address with a scheme, a path on the tracker, or an element or attribute that
/// names one. (None of the test messages has an address as plain words.)
fn loads_anything(page: &str) -> Vec<&'static str> {
    let lower = page.to_ascii_lowercase();
    [
        "http:",
        "https:",
        "mailto:",
        "javascript:",
        "data:",
        "file:",
        "cid:",
        "t.example/",
        "src=",
        "href=",
        "url(",
        "srcset",
        "action=",
        "<base",
        "<link",
        "<img",
        "@import",
    ]
    .into_iter()
    .filter(|needle| lower.contains(needle))
    .collect()
}

#[test]
fn nothing_in_the_page_runs() {
    let html = r#"<p onclick="steal()">Hi<script>steal()</script></p><img src=x onerror=steal()><a href="javascript:steal()">go</a>
        <button onclick="steal()" formaction="https://evil.example/">Buy</button><svg onload=steal()></svg><iframe src=https://evil.example/></iframe>"#;
    let page = page(&display(html, &allowed()), "Hi");
    let lower = page.to_ascii_lowercase();
    for needle in ["<script", "onclick", "onerror", "onload", "steal(", "<iframe", "<svg", "<button", "<form", "<object", "<embed"] {
        assert!(!lower.contains(needle), "{needle} in {page}");
    }
    assert!(page.contains("Hi"), "{page}");
    assert!(page.contains("Buy"), "{page}");
}

#[test]
fn no_remote_address_is_written_even_with_pictures_allowed() {
    let html = r#"<p style="background:url(https://t.example/bg.gif)">Hello <a href="https://t.example/track?id=1">the shop</a>
        and <a href="mailto:a@example.org">mail</a></p><img src="https://t.example/p.gif" alt="Logo" width="120" height="40">
        <img src="cid:part1"><table background="https://t.example/t.gif"><tr><td><img src=//t.example/x.gif>a</td><td>b</td></tr></table>"#;
    for options in [Options::default(), allowed()] {
        let document = display(html, &options);
        let page = page(&document, "Subject");
        assert_eq!(loads_anything(&page), Vec::<&str>::new(), "{page}");
        // The links' words and the picture's alternative text stay.
        assert!(page.contains("the shop") && page.contains("Logo"), "{page}");
    }
}

#[test]
fn a_link_s_tooltip_holds_no_address_even_one_the_message_hid_in_a_mail_address() {
    let html = r#"<a href="mailto:&quot;x://y(z&quot;@example.org">q</a> <a href="https://example.org:8443/a">w</a>"#;
    let page = page(&display(html, &allowed()), "Subject");
    assert!(!page.contains("://") && !page.contains("url("), "{page}");
    assert!(page.contains("title=\"&quot;x&#58;//y&#40;z&quot;@example.org\""), "the words stay, as references: {page}");
}

#[test]
fn the_page_forbids_every_load_itself() {
    let page = page(&Document::default(), "");
    assert!(page.contains("<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'"), "{page}");
    assert!(page.contains("<meta name=\"referrer\" content=\"no-referrer\">"), "{page}");
    assert!(page.starts_with("<!DOCTYPE html>"), "{page}");
}

#[test]
fn text_and_title_are_escaped_and_control_characters_go() {
    let document = display("<p>&lt;script&gt;steal()&lt;/script&gt; &amp; \"quoted\" 'single'</p>", &Options::default());
    let page = page(&document, "</title><script>steal()</script>\0\u{202e}\r\nend");
    assert!(!page.to_ascii_lowercase().contains("<script"), "{page}");
    assert!(page.contains("&lt;script&gt;steal()&lt;/script&gt; &amp; &quot;quoted&quot; &#39;single&#39;"), "{page}");
    assert!(page.contains("<title>&lt;/title&gt;&lt;script&gt;steal()&lt;/script&gt;end</title>"), "{page}");
    assert!(!page.contains('\0') && !page.contains('\u{202e}') && !page.contains('\r'), "{page:?}");
}

#[test]
fn a_long_title_is_cut() {
    let page = page(&Document::default(), &"x".repeat(10_000));
    let title = page.split("<title>").nth(1).and_then(|rest| rest.split("</title>").next()).unwrap_or_default();
    assert_eq!(title.chars().count(), MAX_TITLE);
}

#[test]
fn the_blocks_keep_their_shape() {
    let html = r#"<h2>Head</h2><ol start="3"><li>three</li></ol><ul><li>dot</li></ul><pre>a  b</pre><hr>
        <p align="center"><b>bold</b> <i>it</i></p><table width="50%"><tr><th>x</th><td>y</td></tr></table>
        <div style="background:#d33a2c;text-align:right">red</div>"#;
    let page = page(&display(html, &Options::default()), "");
    for needle in [
        "<h2>Head</h2>",
        "3. three",
        "\u{2022} dot",
        "<pre>a  b</pre>",
        "<hr>",
        "text-align:center",
        "<b>bold</b>",
        "<i>it</i>",
        "<table style=\"width:50%\">",
        "<th><p><b>x</b></p>\n</th>",
        "<td><p>y</p>\n</td>",
        "text-align:right",
        "background:#d33a2c",
    ] {
        assert!(page.contains(needle), "{needle} not in {page}");
    }
}

#[test]
fn a_document_nested_past_any_cap_is_written_flat_and_bounded() {
    // A document built by hand, deeper than `display` ever makes one.
    let mut block = Block::Text {
        kind: crate::TextKind::Paragraph,
        quote: 0,
        look: Look::default(),
        spans: vec![crate::Span { text: "core".into(), ..Default::default() }],
    };
    for _ in 0..10_000 {
        block = Block::Table {
            quote: 0,
            look: Look::default(),
            width: None,
            rows: vec![vec![Cell { blocks: vec![block], ..Default::default() }, Cell::default()]],
        };
    }
    let document = Document { blocks: vec![block], ..Default::default() };
    let page = page(&document, "");
    assert!(page.matches("<table").count() <= super::MAX_NESTING + 1, "{}", page.matches("<table").count());
    assert!(page.contains("core"));
    // Dropping a nested block by hand would overflow the stack the same way; leak it.
    std::mem::forget(document);
}

#[test]
fn a_link_s_tooltip_reads_its_right_to_left_labels_in_the_written_order() {
    let html =
        r#"<a href="https://שלום.קום.example.org/a?q=1">w</a> <a href="mailto:rota@שלום.example.org">m</a> <a href="https://example.org/b">p</a>"#;
    let page = page(&display(html, &allowed()), "Subject");
    // A left-to-right mark leads the tooltip and stands at each edge of a right-to-left run: a browser lays it out left
    // to right and each label stays where it is written, as a click would copy it.
    let mark = '\u{200E}';
    assert!(page.contains(&format!("title=\"{mark}שלום{mark}.{mark}קום{mark}.example.org\"")), "{page}");
    assert!(page.contains(&format!("title=\"{mark}rota@{mark}שלום{mark}.example.org\"")), "{page}");
    // What it says is the same: a host of no right-to-left letter is as it was.
    assert!(page.contains("title=\"example.org\""), "{page}");
}
