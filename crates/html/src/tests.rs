// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{
    Block, Cell, Document, Image, ImageSource, LinkTarget, MAX_BLOCKS, MAX_DEPTH, MAX_IMAGES, MAX_INPUT, MAX_LINKS, MAX_TABLE_COLUMNS,
    MAX_TABLE_DEPTH, MAX_TABLE_ROWS, MAX_URL, Marker, Options, RemoteContent, Span, TextKind, display, truncate,
};

fn doc(html: &str) -> Document {
    display(html, &Options::default())
}

fn allowed(html: &str) -> Document {
    display(html, &Options { remote_content: RemoteContent::Allowed })
}

/// The text of every block, tables and pictures as markers.
fn texts(blocks: &[Block]) -> Vec<String> {
    blocks
        .iter()
        .map(|b| match b {
            Block::Text { .. } => b.text(),
            Block::Rule { .. } => "---".to_owned(),
            Block::Image { image, .. } => format!("[img {}]", image.alt),
            Block::Table { rows, .. } => format!("[table {}x{}]", rows.len(), rows.iter().map(Vec::len).max().unwrap_or(0)),
        })
        .collect()
}

fn spans(block: &Block) -> &[Span] {
    match block {
        Block::Text { spans, .. } => spans,
        _ => &[],
    }
}

fn kind(block: &Block) -> Option<TextKind> {
    match block {
        Block::Text { kind, .. } => Some(*kind),
        _ => None,
    }
}

fn quote(block: &Block) -> u8 {
    match block {
        Block::Text { quote, .. } | Block::Rule { quote } | Block::Image { quote, .. } | Block::Table { quote, .. } => *quote,
    }
}

fn image(block: &Block) -> Option<&Image> {
    match block {
        Block::Image { image, .. } => Some(image),
        _ => None,
    }
}

fn rows(block: &Block) -> &[Vec<Cell>] {
    match block {
        Block::Table { rows, .. } => rows,
        _ => &[],
    }
}

// ---- text ----

#[test]
fn empty_input_gives_nothing() {
    assert_eq!(doc(""), Document::default());
    assert_eq!(doc("   \n\t "), Document::default());
    assert_eq!(doc("<p></p><div> </div>"), Document::default());
}

#[test]
fn paragraphs_split_at_block_elements() {
    let d = doc("<p>One</p><p>Two</p>loose<div>Three</div><h2>Head</h2>tail");
    assert_eq!(texts(&d.blocks), ["One", "Two", "loose", "Three", "Head", "tail"]);
    assert_eq!(kind(&d.blocks[0]), Some(TextKind::Paragraph));
    assert_eq!(kind(&d.blocks[4]), Some(TextKind::Heading(2)));
    assert_eq!(kind(&d.blocks[5]), Some(TextKind::Paragraph));
}

#[test]
fn white_space_collapses_outside_pre() {
    let d = doc("  Hello \n\t  <b> big </b>  world  ");
    assert_eq!(texts(&d.blocks), ["Hello big world"]);
}

#[test]
fn a_line_break_stays_inside_the_paragraph() {
    let d = doc("<p>one<br>two<br/> three</p>");
    assert_eq!(texts(&d.blocks), ["one\ntwo\nthree"]);
    // A break at the start or the end shows nothing.
    assert_eq!(texts(&doc("<p><br>x<br></p>").blocks), ["x"]);
}

#[test]
fn character_references_are_decoded() {
    let d = doc("caf&eacute; &lt;b&gt; &amp;&nbsp;x &#x1F600;");
    assert_eq!(texts(&d.blocks), ["café <b> &\u{a0}x 😀"]);
}

#[test]
fn control_and_bidi_override_characters_are_removed() {
    let d = doc("a\u{0}b\u{7}c\u{202e}d\u{2066}e\u{7f}f\u{85}g&#8238;h");
    assert_eq!(texts(&d.blocks), ["abcdefgh"]);
}

#[test]
fn inline_looks_become_span_flags() {
    let d = doc("plain <b>bold <i>both</i></b> <u>under</u> <s>gone</s> <code>x()</code>");
    let s = spans(&d.blocks[0]);
    let find = |t: &str| s.iter().find(|s| s.text.contains(t)).cloned().unwrap_or_default();
    assert!(!find("plain").bold);
    assert!(find("bold").bold && !find("bold").italic);
    assert!(find("both").bold && find("both").italic);
    assert!(find("under").underline);
    assert!(find("gone").strike);
    assert!(find("x()").code);
    // Runs of one look are merged.
    assert_eq!(
        doc("<b>a</b><strong>b</strong>").blocks[0],
        Block::Text { kind: TextKind::Paragraph, quote: 0, spans: vec![Span { text: "ab".into(), bold: true, ..Span::default() }] }
    );
}

#[test]
fn unbalanced_end_tags_are_ignored() {
    let d = doc("</b>a<b>b</i>c</b>d</p></div></td></table>e");
    assert_eq!(texts(&d.blocks), ["abcd", "e"]);
    let s = spans(&d.blocks[0]);
    assert_eq!(s.iter().filter(|s| s.bold).map(|s| s.text.as_str()).collect::<String>(), "bc");
}

#[test]
fn an_end_tag_closes_what_it_encloses() {
    // `</b>` closes the `i` inside it too.
    let d = doc("<b><i>x</b>y");
    let s = spans(&d.blocks[0]);
    assert_eq!(s.len(), 2);
    assert!(s[0].bold && s[0].italic);
    assert!(!s[1].bold && !s[1].italic);
}

#[test]
fn preformatted_text_keeps_its_white_space() {
    let d = doc("<pre>  a  b\n\tc\n</pre>after");
    assert_eq!(texts(&d.blocks), ["  a  b\n\tc", "after"]);
    assert_eq!(kind(&d.blocks[0]), Some(TextKind::Preformatted));
    assert!(spans(&d.blocks[0])[0].code);
    // A form feed and a carriage return are no control characters in the text.
    assert_eq!(texts(&doc("<pre>a\x0cb\r\nc</pre>a\x0cb").blocks), ["a b\nc", "a b"]);
    // A leading newline right after `<pre>` is not content.
    assert_eq!(texts(&doc("<pre>\nx</pre>").blocks), ["x"]);
}

#[test]
fn headings_have_their_level() {
    for level in 1..=6u8 {
        let d = doc(&format!("<h{level}>t</h{level}>"));
        assert_eq!(kind(&d.blocks[0]), Some(TextKind::Heading(level)));
    }
}

#[test]
fn rules_are_blocks() {
    let d = doc("a<hr>b");
    assert_eq!(texts(&d.blocks), ["a", "---", "b"]);
    assert_eq!(d.blocks[1].text(), "");
}

#[test]
fn an_end_tag_br_is_a_line_break_as_in_browsers() {
    assert_eq!(texts(&doc("a</br>b").blocks), ["a\nb"]);
}

#[test]
fn a_tag_with_an_overlong_name_is_ignored() {
    assert_eq!(texts(&doc(&format!("<{}>x</p>", "a".repeat(100))).blocks), ["x"]);
}

// ---- lists and quotes ----

#[test]
fn lists_give_items_with_markers_and_depth() {
    let d = doc("<ul><li>a<li>b<ol start=3><li>c</li><li>d</ol></ul><ol><li>e</ol>");
    let kinds: Vec<_> = d.blocks.iter().filter_map(kind).collect();
    assert_eq!(
        kinds,
        [
            TextKind::ListItem { depth: 1, marker: Marker::Bullet },
            TextKind::ListItem { depth: 1, marker: Marker::Bullet },
            TextKind::ListItem { depth: 2, marker: Marker::Number(3) },
            TextKind::ListItem { depth: 2, marker: Marker::Number(4) },
            TextKind::ListItem { depth: 1, marker: Marker::Number(1) },
        ]
    );
    assert_eq!(texts(&d.blocks), ["a", "b", "c", "d", "e"]);
}

#[test]
fn a_list_item_marker_goes_to_its_first_block_only() {
    let d = doc("<ul><li><p>first</p><p>second</p></li></ul>");
    assert_eq!(kind(&d.blocks[0]), Some(TextKind::ListItem { depth: 1, marker: Marker::Bullet }));
    assert_eq!(kind(&d.blocks[1]), Some(TextKind::Paragraph));
}

#[test]
fn hostile_list_numbers_are_bounded() {
    let d = doc("<ol start=4294967295><li>a<li>b</ol><ol start=-5><li>c</ol><ol start=zz><li>d</ol>");
    let kinds: Vec<_> = d.blocks.iter().filter_map(kind).collect();
    assert_eq!(
        kinds,
        [
            TextKind::ListItem { depth: 1, marker: Marker::Number(u32::MAX) },
            TextKind::ListItem { depth: 1, marker: Marker::Number(u32::MAX) },
            TextKind::ListItem { depth: 1, marker: Marker::Number(1) },
            TextKind::ListItem { depth: 1, marker: Marker::Number(1) },
        ]
    );
}

#[test]
fn a_list_item_outside_a_list_is_a_bullet() {
    let d = doc("<li>x");
    assert_eq!(kind(&d.blocks[0]), Some(TextKind::ListItem { depth: 1, marker: Marker::Bullet }));
}

#[test]
fn quotes_count_their_depth() {
    let d = doc("a<blockquote>b<blockquote>c<hr></blockquote>d</blockquote>e");
    assert_eq!(texts(&d.blocks), ["a", "b", "c", "---", "d", "e"]);
    assert_eq!(d.blocks.iter().map(quote).collect::<Vec<_>>(), [0, 1, 2, 2, 1, 0]);
}

// ---- nothing runs ----

#[test]
fn scripts_styles_and_embeds_vanish_with_their_content() {
    let html = "a<script>alert(1)</script>b<style>p{}</style>c<iframe src=https://x.example>f</iframe>d\
                <object data=x>o</object>e<embed src=x>f<svg><text>s</text><script>x</script></svg>g\
                <template>t</template>h<math>m</math>i<head><title>t</title><meta http-equiv=refresh content=0></head>j\
                <video src=v>vid</video>k<audio>aud</audio>l<canvas>c</canvas>m<applet>ap</applet>n";
    assert_eq!(texts(&doc(html).blocks), ["abcdefghijklmn"]);
}

#[test]
fn event_attributes_and_styles_are_never_read() {
    let d = doc(r#"<p onclick="steal()" style="background:url(https://t.example/x)">x</p><img src=cid:a onerror=alert(1)><body onload=x>y"#);
    let all = format!("{d:?}");
    for bad in ["steal", "onerror", "alert", "onload", "url(", "t.example"] {
        assert!(!all.contains(bad), "{bad} leaked");
    }
    assert_eq!(texts(&d.blocks), ["x", "[img ]", "y"]);
}

#[test]
fn forms_keep_their_text_and_lose_what_submits() {
    let d = doc(
        r#"<form action="https://evil.example/collect" method=post>Name <input name=n value="prefilled secret"><select><option>One<option>Two</select><textarea>typed <b>here</b></textarea><button type=submit formaction="https://evil.example/2">Send</button></form>after"#,
    );
    let all = format!("{d:?}");
    for bad in ["evil.example", "prefilled", "submit", "post"] {
        assert!(!all.contains(bad), "{bad} leaked");
    }
    assert_eq!(texts(&d.blocks), ["Name OneTwotyped <b>here</b>Send", "after"]);
}

#[test]
fn hidden_elements_are_not_shown() {
    let d = doc(
        r#"a<div hidden>h1</div>b<span style="display:none">h2</span>c<span style="DISPLAY : NONE !important">h3</span>d<p style="visibility: hidden">h4</p>e<span style="display:block">shown</span>"#,
    );
    assert_eq!(texts(&d.blocks), ["a", "bcd", "eshown"]);
    assert_eq!(texts(&doc("<div hidden>unclosed <p>x").blocks), Vec::<String>::new());
}

// ---- links ----

#[test]
fn links_expose_their_targets() {
    let d = doc(r#"see <a href=" https://Example.ORG:8443/a?b=1&amp;c=2#d ">the page</a> or <a href="mailto:Anna@Example.org?subject=hi">mail</a>"#);
    assert_eq!(d.links.len(), 2);
    assert_eq!(d.links[0].target, LinkTarget::Web { url: "https://Example.ORG:8443/a?b=1&c=2#d".into(), host: "example.org".into() });
    assert_eq!(d.links[0].text, "the page");
    assert!(!d.links[0].mismatch);
    assert_eq!(d.links[1].target, LinkTarget::Mail { url: "mailto:Anna@Example.org?subject=hi".into(), address: "Anna@Example.org".into() });
    let s = spans(&d.blocks[0]);
    assert_eq!(s.iter().filter(|s| s.link == Some(0)).map(|s| s.text.as_str()).collect::<String>(), "the page");
    assert_eq!(s.iter().filter(|s| s.link == Some(1)).map(|s| s.text.as_str()).collect::<String>(), "mail");
    assert_eq!(s.iter().filter(|s| s.link.is_none()).map(|s| s.text.as_str()).collect::<String>(), "see  or ");
}

#[test]
fn dangerous_and_relative_targets_are_refused() {
    for href in [
        "javascript:alert(1)",
        "JaVaScRiPt:alert(1)",
        "java\tscript:alert(1)",
        "&#106;avascript:alert(1)",
        "vbscript:x",
        "data:text/html,<script>x</script>",
        "file:///etc/passwd",
        "/relative",
        "#top",
        "//evil.example/x",
        "https:///nohost",
        "http://",
        "ftp://example.org/x",
        "",
        "mailto:",
        "https://exa mple.org/\u{1}",
    ] {
        let d = doc(&format!("<a href=\"{href}\">x</a>"));
        assert_eq!(d.links[0].target, LinkTarget::Refused, "{href:?}");
    }
    let long = format!("https://example.org/{}", "a".repeat(MAX_URL));
    assert_eq!(doc(&format!("<a href=\"{long}\">x</a>")).links[0].target, LinkTarget::Refused);
}

#[test]
fn tab_and_newline_inside_a_target_are_removed() {
    let d = doc("<a href=\"ht\ntp\ts://exam\rple.org/a b\">x</a>");
    assert_eq!(d.links[0].target, LinkTarget::Web { url: "https://example.org/a%20b".into(), host: "example.org".into() });
}

#[test]
fn an_anchor_without_href_is_no_link() {
    let d = doc("<a name=top>x</a>");
    assert!(d.links.is_empty());
    assert_eq!(spans(&d.blocks[0])[0].link, None);
}

#[test]
fn a_nested_anchor_takes_over_and_gives_back() {
    let d = doc("<a href=https://a.example>a<a href=https://b.example>b</a>c</a>d");
    let s = spans(&d.blocks[0]);
    let by_link = |i: Option<usize>| s.iter().filter(|s| s.link == i).map(|s| s.text.as_str()).collect::<String>();
    assert_eq!(by_link(Some(0)), "ac");
    assert_eq!(by_link(Some(1)), "b");
    assert_eq!(by_link(None), "d");
}

#[test]
fn text_naming_another_host_is_a_mismatch() {
    let cases = [
        ("https://bank.example/login", "https://evil.example/login", true),
        ("bank.example", "https://evil.example/", true),
        ("www.bank.example/login", "https://bank.example.evil.example/", true),
        ("https://bank.example", "https://bank.example@evil.example/", true),
        ("help@bank.example", "https://evil.example/", true),
        ("help@bank.example", "mailto:thief@evil.example", true),
        ("https://bank.example", "javascript:alert(1)", true),
        ("bank.example", "https://www.bank.example/x", false),
        ("www.bank.example", "https://bank.example/", false),
        ("https://Bank.Example/a", "https://login.bank.example/b", false),
        ("Help@Bank.Example", "mailto:help@bank.example?subject=x", false),
        ("mailto:help@bank.example", "mailto:help@bank.example", false),
        ("Click here", "https://evil.example/", false),
        ("Version 2.0", "https://evil.example/", false),
        ("", "https://evil.example/", false),
        ("bank.example", "mailto:a@bank.example", true),
        ("https://bank.example", "#top", true),
    ];
    for (text, href, mismatch) in cases {
        let d = doc(&format!("<a href=\"{href}\">{text}</a>"));
        assert_eq!(d.links[0].mismatch, mismatch, "{text} -> {href}");
    }
}

#[test]
fn a_bidi_override_cannot_disguise_the_text() {
    // U+202E would show "elpmaxe.knab" backwards as "bank.example".
    let d = doc("<a href=\"https://elpmaxe.knab/\">&#8238;elpmaxe.knab</a>");
    assert_eq!(d.links[0].text, "elpmaxe.knab");
    assert!(!d.links[0].mismatch);
}

#[test]
fn invisible_characters_cannot_hide_a_mismatch() {
    let texts = [
        "bank&#x200B;.example",
        "ba&#x200D;nk.example",
        "ba&#x200C;nk.example",
        "ba&shy;nk.example",
        "bank.exa&#x2060;mple",
        "&#xFEFF;bank.example",
        "bank.example&#x200E;",
        "ban&#x61C;k.example",
        "bank&#x3002;example",
        "bank&#xFF0E;example",
        "bank&#xFF61;example",
    ];
    for text in texts {
        let d = doc(&format!("<a href=\"https://evil.example/\">{text}</a>"));
        assert!(d.links[0].mismatch, "{text}");
        let same = doc(&format!("<a href=\"https://bank.example/\">{text}</a>"));
        assert!(!same.links[0].mismatch, "{text}");
    }
    let d = doc("<a href=\"mailto:help@bank.example\">help&#x200B;@bank&#xFF0E;example</a>");
    assert!(!d.links[0].mismatch);
    let d = doc("<a href=\"mailto:thief@evil.example\">help&#x200B;@bank.example</a>");
    assert!(d.links[0].mismatch);
}

#[test]
fn any_invisible_character_anywhere_is_ignored_in_the_comparison() {
    let invisible = ["&shy;", "&#x61C;", "&#x180E;", "&#x200B;", "&#x200C;", "&#x200D;", "&#x200E;", "&#x200F;", "&#x2060;", "&#x2064;", "&#xFEFF;"];
    let dots = [".", "&#x3002;", "&#xFF0E;", "&#xFF61;"];
    for at in 0..="bank.example".len() {
        for (hidden, dot) in invisible.iter().zip(dots.iter().cycle()) {
            let (head, tail) = "bank.example".split_at(at);
            let text = format!("{head}{hidden}{tail}").replace('.', dot);
            for (href, mismatch) in [("https://evil.example/", true), ("https://bank.example/", false)] {
                let d = doc(&format!("<a href=\"{href}\">{text}</a>"));
                assert_eq!(d.links[0].mismatch, mismatch, "{text} -> {href}");
            }
        }
    }
}

#[test]
fn invisible_characters_are_not_shown_but_joiners_stay() {
    let d = doc("<p>a&shy;b&#x200B;c&#xFEFF;d&#x2060;e&#x200E;f&#x61C;g&#x180E;h</p>");
    assert_eq!(texts(&d.blocks), ["abcdefgh"]);
    // The joiners hold emoji sequences and some scripts together.
    let d = doc("<p>&#x1F469;&#x200D;&#x1F4BB; &#x645;&#x200C;&#x627;</p>");
    assert_eq!(texts(&d.blocks), ["\u{1f469}\u{200d}\u{1f4bb} \u{645}\u{200c}\u{627}"]);
}

#[test]
fn user_info_in_the_target_or_the_text_is_a_mismatch() {
    let cases = [
        ("Click here", "https://bank.example@evil.example/"),
        ("evil.example", "https://bank.example@evil.example/"),
        ("https://bank.example@evil.example", "https://evil.example/"),
        ("https://bank.example@evil.example", "https://bank.example@evil.example/"),
        ("Click here", "https://u:p@bank.example/"),
    ];
    for (text, href) in cases {
        let d = doc(&format!("<a href=\"{href}\">{text}</a>"));
        assert!(d.links[0].mismatch, "{text} -> {href}");
    }
    // A backslash ends the authority the way a browser reads it, so this `@` is in the path.
    let d = doc("<a href=\"https://bank.example\\@evil.example/\">bank.example</a>");
    assert!(!d.links[0].mismatch);
}

#[test]
fn link_text_is_capped() {
    let d = doc(&format!("<a href=https://a.example>{}</a>", "x".repeat(5_000)));
    assert_eq!(d.links[0].text.chars().count(), 1_024);
}

// ---- pictures ----

#[test]
fn remote_pictures_are_blocked_by_default() {
    let d = doc(r#"<img src="https://t.example/pixel.gif?id=42" alt="Logo"><img src=http://t.example/b.png>"#);
    assert_eq!(d.remote_blocked, 2);
    assert_eq!(image(&d.blocks[0]), Some(&Image { alt: "Logo".into(), source: ImageSource::Blocked, link: None }));
    assert!(!format!("{d:?}").contains("t.example"));
}

#[test]
fn consent_shows_remote_pictures() {
    let d = allowed(r#"<img src="https://t.example/a.png?x=1&amp;y=2" alt=a>"#);
    assert_eq!(d.remote_blocked, 0);
    assert_eq!(image(&d.blocks[0]).map(|i| &i.source), Some(&ImageSource::Remote("https://t.example/a.png?x=1&y=2".into())));
}

#[test]
fn cid_pictures_point_at_message_parts() {
    let d = doc(r#"<img src="cid:logo@example.org"><img src="CID:%3Cpart.2%40example.org%3E"><img src="cid:logo@example.org">"#);
    assert_eq!(image(&d.blocks[0]).map(|i| &i.source), Some(&ImageSource::Inline("logo@example.org".into())));
    assert_eq!(image(&d.blocks[1]).map(|i| &i.source), Some(&ImageSource::Inline("part.2@example.org".into())));
    assert_eq!(d.inline_images, ["logo@example.org", "part.2@example.org"]);
    assert_eq!(d.remote_blocked, 0);
    // Even with consent a `cid:` picture stays inline.
    assert_eq!(image(&allowed("<img src=cid:x>").blocks[0]).map(|i| &i.source), Some(&ImageSource::Inline("x".into())));
}

#[test]
fn other_picture_sources_are_unsupported() {
    for src in ["data:image/png;base64,AAAA", "javascript:x", "file:///x.png", "x.png", "", "cid:", "//t.example/x.png"] {
        let d = allowed(&format!("<img src=\"{src}\" alt=x>"));
        assert_eq!(image(&d.blocks[0]).map(|i| &i.source), Some(&ImageSource::Unsupported), "{src:?}");
    }
    // A picture with neither source nor text shows nothing.
    assert!(doc("<img>").blocks.is_empty());
}

#[test]
fn a_linked_picture_keeps_its_link() {
    let d = doc("<a href=https://shop.example><img src=cid:x alt=Buy></a>");
    assert_eq!(image(&d.blocks[0]).and_then(|i| i.link), Some(0));
    assert_eq!(d.links[0].text, "");
}

#[test]
fn a_picture_splits_the_paragraph() {
    let d = doc("before<img src=cid:x alt=pic>after");
    assert_eq!(texts(&d.blocks), ["before", "[img pic]", "after"]);
}

// ---- tables ----

#[test]
fn a_table_gives_rows_and_cells() {
    let d = doc("<table><tr><th>Name<th>Qty<tr><td>Tea<td>2</table>after");
    assert_eq!(texts(&d.blocks), ["[table 2x2]", "after"]);
    let r = rows(&d.blocks[0]);
    assert!(r[0][0].header && !r[1][0].header);
    assert_eq!(texts(&r[0][0].blocks), ["Name"]);
    assert!(spans(&r[0][0].blocks[0])[0].bold);
    assert_eq!(texts(&r[1][1].blocks), ["2"]);
}

#[test]
fn a_one_column_table_is_layout() {
    let d = doc("<table><tr><td>a</td></tr><tr><td><p>b</p><p>c</p></td></tr></table>");
    assert_eq!(texts(&d.blocks), ["a", "b", "c"]);
}

#[test]
fn table_sections_and_captions_are_read() {
    let d = doc("<table><caption>Cap</caption><thead><tr><td>a<td>b</thead><tbody><tr><td>c<td>d</tbody><tfoot><tr><td>e<td>f</tfoot></table>");
    let r = rows(&d.blocks[d.blocks.len() - 1]);
    assert_eq!(r.len(), 3);
    assert_eq!(texts(&d.blocks[..1]), ["Cap"]);
}

#[test]
fn stray_text_in_a_table_gets_a_cell() {
    let d = doc("<table>loose<tr><td>a<td>b</td>between<td>c</table>");
    let r = rows(&d.blocks[0]);
    let cells: Vec<Vec<String>> = r.iter().map(|row| row.iter().map(|c| texts(&c.blocks).join("|")).collect()).collect();
    assert_eq!(cells, [vec!["loose".to_owned()], vec!["a".into(), "b".into(), "between".into(), "c".into()]]);
}

#[test]
fn cells_outside_a_table_are_paragraphs() {
    let d = doc("<tr><td>a<td>b</tr>");
    assert_eq!(texts(&d.blocks), ["a", "b"]);
}

#[test]
fn an_unclosed_table_is_closed_at_the_end() {
    let d = doc("<table><tr><td>a<td><table><tr><td>x<td>y");
    let r = rows(&d.blocks[0]);
    assert_eq!(r[0].len(), 2);
    assert_eq!(texts(&r[0][1].blocks), ["[table 1x2]"]);
}

#[test]
fn table_quote_and_cell_blocks_have_their_own_depth() {
    let d = doc("<blockquote><table><tr><td>a<td>b</table></blockquote>");
    assert_eq!(quote(&d.blocks[0]), 1);
}

#[test]
fn nested_tables_past_the_cap_flatten() {
    let mut html = String::new();
    for _ in 0..MAX_TABLE_DEPTH + 5 {
        html.push_str("<table><tr><td>a</td><td>");
    }
    html.push_str("deep");
    let d = doc(&html);
    let mut depth = 0;
    let mut blocks = d.blocks.clone();
    while let Some(table) = blocks.iter().find(|b| matches!(b, Block::Table { .. })).cloned() {
        depth += 1;
        blocks = rows(&table)[0].last().map(|c| c.blocks.clone()).unwrap_or_default();
    }
    assert_eq!(depth, MAX_TABLE_DEPTH);
    assert!(format!("{d:?}").contains("deep"));
}

#[test]
fn rows_and_columns_past_the_cap_go_after_the_table() {
    let wide = format!("<table><tr>{}</table>", "<td>c".repeat(MAX_TABLE_COLUMNS + 3));
    let d = doc(&wide);
    assert_eq!(rows(&d.blocks[0])[0].len(), MAX_TABLE_COLUMNS);
    assert_eq!(d.blocks.len(), 1 + 3);
    let long = format!("<table>{}</table>", "<tr><td>a<td>b".repeat(MAX_TABLE_ROWS + 2));
    let d = doc(&long);
    assert_eq!(rows(&d.blocks[0]).len(), MAX_TABLE_ROWS);
    assert_eq!(d.blocks.len(), 1 + 4);
}

// ---- caps ----

#[test]
fn input_past_the_cap_is_cut() {
    let html = format!("<p>{}</p>é", "a".repeat(MAX_INPUT));
    let d = doc(&html);
    assert!(d.truncated);
    assert_eq!(d.blocks[0].text().len(), MAX_INPUT - 3);
    assert!(!doc("<p>x</p>").truncated);
}

#[test]
fn truncate_cuts_at_a_character_boundary() {
    assert_eq!(truncate("héllo", 2), "h");
    assert_eq!(truncate("héllo", 3), "hé");
    assert_eq!(truncate("hi", 10), "hi");
    assert_eq!(truncate("é", 0), "");
}

#[test]
fn deep_nesting_is_flattened() {
    let html = format!("{}x{}", "<div><b>".repeat(MAX_DEPTH * 4), "</b></div>".repeat(MAX_DEPTH * 4));
    let d = doc(&html);
    assert_eq!(texts(&d.blocks), ["x"]);
    let html = format!("{}y", "<blockquote>".repeat(1_000));
    let d = doc(&html);
    assert_eq!(texts(&d.blocks), ["y"]);
    assert!(quote(&d.blocks[0]) >= 100);
    let html = format!("{}<li>z", "<ul>".repeat(200));
    let d = doc(&html);
    assert_eq!(kind(&d.blocks[0]), Some(TextKind::ListItem { depth: 200, marker: Marker::Bullet }));
    // Past the cap the start tag is ignored and its text stays.
    let html = format!("{}<li>z", "<ul>".repeat(1_000));
    assert_eq!(texts(&doc(&html).blocks), ["z"]);
}

#[test]
fn blocks_past_the_cap_are_dropped() {
    let d = doc(&"<p>x</p>".repeat(MAX_BLOCKS + 10));
    assert_eq!(d.blocks.len(), MAX_BLOCKS);
    assert!(d.truncated);
}

#[test]
fn links_past_the_cap_are_plain_text() {
    let d = doc(&"<a href=https://a.example>x</a>".repeat(MAX_LINKS + 5));
    assert_eq!(d.links.len(), MAX_LINKS);
    assert!(d.truncated);
    assert_eq!(d.blocks[0].text().len(), MAX_LINKS + 5);
}

#[test]
fn pictures_past_the_cap_are_dropped() {
    let d = doc(&"<img src=cid:x>".repeat(MAX_IMAGES + 5));
    assert_eq!(d.blocks.len(), MAX_IMAGES);
    assert!(d.truncated);
}

proptest::proptest! {
    #[test]
    fn any_markup_gives_blocks(s in "[<>/=\"' a-z&;#0-9é\n]{0,200}") {
        let _ = display(&s, &Options::default());
        let _ = display(&s, &Options { remote_content: RemoteContent::Allowed });
    }

    #[test]
    fn any_tag_soup_gives_blocks(tags in proptest::collection::vec(proptest::sample::select(vec![
        "<p>", "</p>", "<b>", "</b>", "<a href=https://a.example>", "</a>", "<table>", "</table>", "<tr>", "</tr>", "<td>", "</td>",
        "<th>", "<ul>", "</ul>", "<ol>", "<li>", "</li>", "<blockquote>", "</blockquote>", "<pre>", "</pre>", "<br>", "<hr>",
        "<img src=cid:a>", "<img src=https://t.example/x>", "<div hidden>", "</div>", "<h1>", "</h1>", "text", " ", "<caption>",
        "</caption>", "<svg>", "</svg>", "<form>", "</form>",
    ]), 0..120)) {
        let html: String = tags.concat();
        let d = display(&html, &Options::default());
        proptest::prop_assert!(d.blocks.len() <= MAX_BLOCKS);
    }
}
