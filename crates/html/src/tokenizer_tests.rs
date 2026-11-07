// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-FileCopyrightText: MailCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{MAX_ATTRS, Token, Tokenizer};

fn tokens(html: &str) -> Vec<String> {
    Tokenizer::new(html)
        .map(|t| match t {
            Token::Text(t) => format!("T:{t}"),
            Token::Open(tag) => {
                let attrs: Vec<String> = tag.attrs.iter().map(|(n, v)| format!("{n}={v}")).collect();
                if attrs.is_empty() { format!("<{}>", tag.name) } else { format!("<{} {}>", tag.name, attrs.join(" ")) }
            }
            Token::Close(n) => format!("</{n}>"),
        })
        .collect()
}

#[test]
fn text_and_tags() {
    assert_eq!(tokens("Hi <B>there</b>!"), ["T:Hi ", "<b>", "T:there", "</b>", "T:!"]);
    assert_eq!(tokens(""), Vec::<String>::new());
    assert_eq!(tokens("plain"), ["T:plain"]);
    assert_eq!(tokens("<br/><hr />"), ["<br>", "<hr>"]);
}

#[test]
fn attributes_in_every_quoting_style() {
    assert_eq!(tokens(r#"<a href="x y" title='it"s' id=z data-k>"#), [r#"<a href=x y title=it"s id=z data-k=>"#]);
    assert_eq!(tokens("<a href = \"u\"\n\tclass = c >t"), ["<a href=u class=c>", "T:t"]);
    assert_eq!(tokens("<img src=a.png/>"), ["<img src=a.png/>"]);
    assert_eq!(tokens("<p =x=y>"), ["<p =x=y>"]);
    // A `>` inside a quoted value does not end the tag.
    assert_eq!(tokens(r#"<a title="a>b">t</a>"#), ["<a title=a>b>", "T:t", "</a>"]);
}

#[test]
fn attribute_values_are_decoded_on_demand() {
    let mut t = Tokenizer::new(r#"<a HREF="?a=1&amp;b=2" href="second" hidden>"#);
    let Some(Token::Open(tag)) = t.next() else { panic!("no tag") };
    assert_eq!(tag.attr("href").as_deref(), Some("?a=1&b=2"));
    assert_eq!(tag.attr("missing"), None);
    assert!(tag.has("HIDDEN"));
    assert!(!tag.has("title"));
}

#[test]
fn a_lone_angle_bracket_is_text() {
    assert_eq!(tokens("a < b and c > d"), ["T:a < b and c > d"]);
    assert_eq!(tokens("1<2<3"), ["T:1<2<3"]);
    assert_eq!(tokens("<"), ["T:<"]);
    assert_eq!(tokens("x<"), ["T:x<"]);
    assert_eq!(tokens("<3 <- </ x"), ["T:<3 <- </ x"]);
    assert_eq!(tokens("a</>b"), ["T:a", "T:b"]);
}

#[test]
fn comments_and_declarations_vanish() {
    assert_eq!(tokens("a<!-- x -- > y -->b"), ["T:a", "T:b"]);
    assert_eq!(tokens("<!DOCTYPE html><p>x"), ["<p>", "T:x"]);
    assert_eq!(tokens("<?xml version=\"1.0\"?>x"), ["T:x"]);
    assert_eq!(tokens("a<!-->b<!--->c"), ["T:a", "T:b", "T:c"]);
    assert_eq!(tokens("<!--[if mso]><p>only word</p><![endif]-->shown"), ["T:shown"]);
    assert_eq!(tokens("<![if !mso]>shown<![endif]>"), ["T:shown"]);
    // An unfinished comment or declaration swallows the rest.
    assert_eq!(tokens("a<!-- b <p>c"), ["T:a"]);
    assert_eq!(tokens("a<!DOCTYPE"), ["T:a"]);
}

#[test]
fn script_and_style_content_is_dropped() {
    assert_eq!(tokens("a<script>if (a<b) alert('</p>')</script>b"), ["T:a", "T:b"]);
    assert_eq!(tokens("a<STYLE type=text/css>p > b { color: red }</Style >b"), ["T:a", "T:b"]);
    assert_eq!(tokens("<title>Hello</title>x"), ["T:x"]);
    assert_eq!(tokens("<iframe src=x>inside</iframe>x"), ["T:x"]);
    assert_eq!(tokens("a<script>never closed <p>x"), ["T:a"]);
    // `</scripty>` is not the end tag.
    assert_eq!(tokens("<script>x</scripty>y</script>z"), ["T:z"]);
}

#[test]
fn textarea_content_is_text() {
    assert_eq!(tokens("<textarea><b>x</b></textarea>y"), ["<textarea>", "T:<b>x</b>", "</textarea>", "T:y"]);
    assert_eq!(tokens("<textarea></textarea>"), ["<textarea>", "</textarea>"]);
    assert_eq!(tokens("<xmp>a<p>b"), ["<xmp>", "T:a<p>b"]);
}

#[test]
fn unfinished_tags_run_to_the_end() {
    assert_eq!(tokens("a<b"), ["T:a", "<b>"]);
    assert_eq!(tokens("a<a href=\"x"), ["T:a", "<a href=x>"]);
    assert_eq!(tokens("a</b"), ["T:a", "</b>"]);
    assert_eq!(tokens("a<b c"), ["T:a", "<b c=>"]);
    assert_eq!(tokens("a<b c="), ["T:a", "<b c=>"]);
}

#[test]
fn non_ascii_everywhere() {
    assert_eq!(tokens("é<b é=ü ñ>ß</b>ø"), ["T:é", "<b é=ü ñ=>", "T:ß", "</b>", "T:ø"]);
    assert_eq!(tokens("<é>"), ["T:<é>"]);
    assert_eq!(tokens("<bé>x"), ["<bé>", "T:x"]);
}

#[test]
fn long_names_and_many_attributes_are_capped() {
    let long = format!("<{}>x", "a".repeat(100));
    assert_eq!(tokens(&long), ["<>", "T:x"]);
    let many = format!("<p {}>", (0..100).map(|i| format!("a{i}=1")).collect::<Vec<_>>().join(" "));
    let Some(Token::Open(tag)) = Tokenizer::new(&many).next() else { panic!("no tag") };
    assert_eq!(tag.attrs.len(), MAX_ATTRS);
}

#[test]
fn hostile_runs_are_linear() {
    for s in [
        "<".repeat(300_000),
        "<a ".repeat(100_000),
        "<!--".repeat(100_000),
        "</".repeat(150_000),
        "<script>".repeat(50_000),
        "<b x=\"".repeat(50_000),
    ] {
        let n = Tokenizer::new(&s).count();
        assert!(n <= s.len());
    }
}

proptest::proptest! {
    #[test]
    fn any_input_tokenizes_and_keeps_the_text(s in "[<>/!a-c =\"'&;é-]{0,80}") {
        let total: usize = Tokenizer::new(&s).map(|t| match t { Token::Text(t) => t.len(), _ => 0 }).sum();
        proptest::prop_assert!(total <= s.len());
    }
}
