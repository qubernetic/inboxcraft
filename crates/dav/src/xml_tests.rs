// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{CALDAV, DAV, Element, MAX_DEPTH, MAX_NODES, Node, escape, parse};
use crate::DavError;

fn read(xml: &str) -> Element {
    parse(xml.as_bytes()).unwrap()
}

#[test]
fn names_are_read_with_their_namespace() {
    let root = read(
        r#"<?xml version="1.0" encoding="utf-8"?>
        <d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav">
          <d:response><d:href>/a/</d:href><c:calendar-data>x</c:calendar-data></d:response>
        </d:multistatus>"#,
    );
    assert!(root.is(DAV, "multistatus"));
    let response = root.child(DAV, "response").unwrap();
    assert_eq!(response.child(DAV, "href").unwrap().text(), "/a/");
    assert_eq!(response.child(CALDAV, "calendar-data").unwrap().text(), "x");
    assert!(response.child(CALDAV, "href").is_none());
    assert_eq!(root.children_named(DAV, "response").count(), 1);
}

#[test]
fn a_default_namespace_holds_for_unprefixed_names_and_can_be_undone() {
    let root = read(r#"<multistatus xmlns="DAV:"><response xmlns=""><href/></response><x:a xmlns:x="urn:x"/></multistatus>"#);
    assert!(root.is(DAV, "multistatus"));
    let response = root.elements().next().unwrap();
    assert_eq!((response.namespace.as_str(), response.name.as_str()), ("", "response"));
    assert!(response.child("", "href").is_some());
    assert!(root.child("urn:x", "a").is_some());
}

#[test]
fn a_prefix_holds_only_inside_the_element_that_declares_it() {
    let err = parse(br#"<a xmlns="urn:a"><b xmlns:p="urn:p"/><p:c/></a>"#);
    assert_eq!(err, Err(DavError::BadXml("a prefix without a namespace")));
    let root = read(r#"<p:a xmlns:p="urn:one"><p:b xmlns:p="urn:two"/><p:c/></p:a>"#);
    assert!(root.child("urn:two", "b").is_some());
    assert!(root.child("urn:one", "c").is_some());
    let root = read(r#"<a xml:lang="en"/>"#);
    assert_eq!(root.attribute("xml:lang"), Some("en"));
}

#[test]
fn text_entities_cdata_and_comments() {
    let root = read("<a>1 &lt; 2 &amp;&amp; &quot;x&apos; &gt; &#65;&#x42;<!-- no --><![CDATA[<b>&amp;]]>\u{e9}</a>");
    assert_eq!(root.text(), "1 < 2 && \"x' > AB<b>&amp;\u{e9}");
    assert_eq!(root.children.len(), 1);
    assert_eq!(read("<a><?pi ok?>t</a>").text(), "t");
    assert_eq!(read("<a>x<b>no</b>y<![CDATA[]]></a>").text(), "xy");
}

#[test]
fn attributes_are_read_in_both_quotes() {
    let root = read("<c:comp xmlns:c='urn:ietf:params:xml:ns:caldav' name=\"VEVENT\" other = 'a&amp;b' />");
    assert_eq!(root.attribute("name"), Some("VEVENT"));
    assert_eq!(root.attribute("other"), Some("a&b"));
    assert_eq!(root.attribute("missing"), None);
}

#[test]
fn a_byte_order_mark_and_space_around_the_root_are_fine() {
    let root = read("\u{feff}<?xml version='1.0'?>\n<!-- c -->\n<a>\n</a>\n<!-- after -->\n<?pi?>\n");
    assert!(root.is("", "a"));
    assert_eq!(root.text(), "\n");
    assert!(matches!(root.children.first(), Some(Node::Text(_))));
}

#[test]
fn a_document_type_declaration_is_refused_so_no_entity_can_grow() {
    let laughs = r#"<?xml version="1.0"?><!DOCTYPE l [<!ENTITY a "aaaa">]><l>&a;</l>"#;
    assert_eq!(parse(laughs.as_bytes()), Err(DavError::BadXml("a document type declaration")));
    assert_eq!(parse(b"<a>&a;</a>"), Err(DavError::BadXml("an unknown entity")));
}

#[test]
fn what_is_not_well_formed_is_refused() {
    let cases: &[(&[u8], &str)] = &[
        (b"", "no root element"),
        (b"  ", "no root element"),
        (b"text", "text outside the root element"),
        (b"<a>", "an unclosed element"),
        (b"<a></b>", "an end tag that does not match"),
        (b"</a>", "an end tag that does not match"),
        (b"<a/><b/>", "a second root element"),
        (b"<a/>x", "text outside the root element"),
        (b"<![CDATA[x]]><a/>", "text outside the root element"),
        (b"<a><![CDATA[x</a>", "an unclosed section"),
        (b"<a><!-- x</a>", "an unclosed section"),
        (b"<?xml", "an unclosed section"),
        (b"<a", "an unclosed tag"),
        (b"<a b>", "an attribute without a value"),
        (b"<a b=c>", "an attribute without quotes"),
        (b"<a b='c>", "an unclosed tag"),
        (b"<a b='<'>", "a < in an attribute"),
        (b"<a b='1' b='2'/>", "an attribute given twice"),
        (b"<a xmlns:p=''/>", "an empty namespace for a prefix"),
        (b"<p:a/>", "a prefix without a namespace"),
        (b"<a p:b='1'/>", "a prefix without a namespace"),
        (b"<:a/>", "a bad name"),
        (b"<a:/>", "a bad name"),
        (b"<a:b:c xmlns:a='x'/>", "a bad name"),
        (b"<1a/>", "a bad name"),
        (b"< a/>", "a bad name"),
        (b"<a>&amp</a>", "an unclosed entity"),
        (b"<a>&#0;</a>", "a character reference to no character"),
        (b"<a>&#xD800;</a>", "a character reference to no character"),
        (b"<a>&#x;</a>", "a character reference to no character"),
        (b"<a>&#99999999999;</a>", "a character reference to no character"),
        (b"<a>\xff</a>", "not UTF-8"),
        (b"<a>]]></a>", "a ]]> in text"),
        (b"<a><!ELEMENT a></a>", "a document type declaration"),
        (b"<a></a x>", "an end tag that does not match"),
        (b"<a/ >", "an unclosed tag"),
        (b"<a>\0</a>", "a character XML does not allow"),
    ];
    for &(xml, why) in cases {
        assert_eq!(parse(xml), Err(DavError::BadXml(why)), "{:?}", String::from_utf8_lossy(xml));
    }
}

#[test]
fn an_end_tag_may_have_space_before_its_close() {
    assert!(read("<a></a\n>").is("", "a"));
}

#[test]
fn nesting_is_capped() {
    let deep = format!("{}{}", "<a>".repeat(MAX_DEPTH + 1), "</a>".repeat(MAX_DEPTH + 1));
    assert_eq!(parse(deep.as_bytes()), Err(DavError::TooLarge { what: "XML nesting", limit: MAX_DEPTH }));
    let fits = format!("{}{}", "<a>".repeat(MAX_DEPTH), "</a>".repeat(MAX_DEPTH));
    assert!(parse(fits.as_bytes()).is_ok());
}

#[test]
fn the_number_of_nodes_is_capped() {
    let many = format!("<a>{}</a>", "<b/>".repeat(MAX_NODES));
    assert_eq!(parse(many.as_bytes()), Err(DavError::TooLarge { what: "XML node count", limit: MAX_NODES }));
}

#[test]
fn attributes_and_names_are_capped() {
    let attributes: String = (0..65).map(|n| format!(" a{n}='1'")).collect();
    let many = format!("<a{attributes}/>");
    assert_eq!(parse(many.as_bytes()), Err(DavError::TooLarge { what: "XML attribute count", limit: 64 }));
    let long = format!("<{}/>", "a".repeat(300));
    assert_eq!(parse(long.as_bytes()), Err(DavError::BadXml("a bad name")));
}

#[test]
fn escape_writes_text_that_reads_back() {
    let text = "a<b>&\"c'";
    assert_eq!(escape(text), "a&lt;b&gt;&amp;&quot;c&apos;");
    let xml = format!("<a x=\"{0}\">{0}</a>", escape(text));
    let root = read(&xml);
    assert_eq!(root.text(), text);
    assert_eq!(root.attribute("x"), Some(text));
}
