// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{ContentType, Disposition, MAX_PARAMS, Params};

#[test]
fn a_media_type_is_lower_case_with_its_parameters() {
    let content_type = ContentType::parse("Text/HTML; Charset=\"UTF-8\"; format=flowed").unwrap();
    assert_eq!((content_type.kind(), content_type.subtype()), ("text", "html"));
    assert_eq!(content_type.essence(), "text/html");
    assert!(content_type.is("TEXT", "Html"));
    assert!(!content_type.is("text", "plain"));
    assert_eq!(content_type.param("charset"), Some("UTF-8"));
    assert_eq!(content_type.param("CHARSET"), Some("UTF-8"));
    assert_eq!(content_type.params().get("format"), Some("flowed"));
    assert_eq!(content_type.param("boundary"), None);
}

#[test]
fn a_comment_after_the_media_type_is_ignored() {
    let content_type = ContentType::parse("  text/plain (plain text) ; charset=us-ascii").unwrap();
    assert_eq!(content_type.essence(), "text/plain");
    assert_eq!(content_type.param("charset"), Some("us-ascii"));
}

#[test]
fn something_that_is_no_media_type_is_none() {
    for bad in ["", "text", "text/", "/plain", "text/plain/x", "te xt/plain", "text/pl\"ain", "tëxt/plain", "; charset=utf-8"] {
        assert_eq!(ContentType::parse(bad), None, "{bad:?}");
    }
}

#[test]
fn the_default_is_plain_us_ascii_text() {
    let content_type = ContentType::default();
    assert_eq!(content_type.essence(), "text/plain");
    assert_eq!(content_type.param("charset"), Some("us-ascii"));
}

#[test]
fn quoted_values_keep_their_semicolons_and_escapes() {
    let params = Params::parse("; name=\"a;b \\\"c\\\" \\\\ d\\e\" junk ; next = plain value ;");
    assert_eq!(params.get("name"), Some("a;b \"c\" \\ d\\e"));
    assert_eq!(params.get("next"), Some("plain value"));
}

#[test]
fn a_parameter_without_a_name_or_a_value() {
    let params = Params::parse("; =orphan; flag; x=1");
    assert_eq!(params.get(""), None);
    assert_eq!(params.get("flag"), Some(""));
    assert_eq!(params.get("x"), Some("1"));
}

#[test]
fn the_first_of_two_plain_parameters_wins() {
    assert_eq!(Params::parse("; a=1; A=2").get("a"), Some("1"));
}

#[test]
fn rfc_2231_extended_values_are_decoded() {
    let params = Params::parse("; filename*=utf-8''na%C3%AFve%20file.txt; title*=iso-8859-1'en'caf%E9");
    assert_eq!(params.text("filename").as_deref(), Some("naïve file.txt"));
    assert_eq!(params.text("title").as_deref(), Some("café"));
    let params = Params::parse("; a*=''100%25%zz%4; b*=x-unknown''%C3%A9; c*=no-quotes%41");
    assert_eq!(params.text("a").as_deref(), Some("100%%zz%4"));
    assert_eq!(params.text("b").as_deref(), Some("é"));
    assert_eq!(params.text("c").as_deref(), Some("no-quotesA"));
}

#[test]
fn rfc_2231_continuations_are_joined_in_order() {
    let params = Params::parse("; name*1=\" part\"; name*0*=utf-8''first%C3%A9; name*2*=%21; name=plain");
    assert_eq!(params.text("name").as_deref(), Some("firsté part!"));
    assert_eq!(params.get("name"), Some("firsté part!"));
    let params = Params::parse("; url*0=\"http://example.org/\"; url*1=\"a\"; bad*x=1");
    assert_eq!(params.get("url"), Some("http://example.org/a"));
    assert_eq!(params.get("bad"), None);
}

#[test]
fn encoded_words_in_a_plain_parameter_are_decoded_as_text() {
    let params = Params::parse("; name=\"=?UTF-8?B?w6lk?=.pdf\"; evil=\"a=?UTF-8?Q?=0A?=b\"");
    assert_eq!(params.get("name"), Some("=?UTF-8?B?w6lk?=.pdf"));
    assert_eq!(params.text("name").as_deref(), Some("éd.pdf"));
    assert_eq!(params.text("evil").as_deref(), Some("a b"));
    assert_eq!(params.text("none"), None);
}

#[test]
fn parameters_are_capped() {
    let many: String = (0..MAX_PARAMS + 10).map(|i| format!("; p{i}={i}")).collect();
    let params = Params::parse(&many);
    assert_eq!(params.get(&format!("p{}", MAX_PARAMS - 1)), Some(&*(MAX_PARAMS - 1).to_string()));
    assert_eq!(params.get(&format!("p{MAX_PARAMS}")), None);
}

#[test]
fn a_disposition_says_attachment_or_inline_and_names_the_file() {
    let disposition = Disposition::parse("Attachment; filename=\"report.pdf\"");
    assert!(disposition.is_attachment());
    assert!(!disposition.is_inline());
    assert_eq!(disposition.kind(), "attachment");
    assert_eq!(disposition.file_name().as_deref(), Some("report.pdf"));
    assert_eq!(disposition.params().get("filename"), Some("report.pdf"));

    let disposition = Disposition::parse("inline");
    assert!(disposition.is_inline());
    assert_eq!(disposition.file_name(), None);

    let disposition = Disposition::parse("  ;filename=x");
    assert_eq!(disposition.kind(), "");
    assert!(!disposition.is_attachment() && !disposition.is_inline());
}
