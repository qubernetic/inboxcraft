// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_model::{EmailAddress, Security};

use super::{MAX_FILE, MAX_SERVERS, parse};

fn address() -> EmailAddress {
    EmailAddress::new("ada@example.org").unwrap()
}

fn servers(found: &[super::Offer]) -> Vec<String> {
    found.iter().map(|offer| format!("{} as {}", offer.server, offer.username)).collect()
}

const FULL: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE clientConfig>
<!-- An invented provider. -->
<clientConfig version="1.1">
  <emailProvider id="example.org">
    <domain>example.org</domain>
    <displayName>Example Mail</displayName>
    <incomingServer type="imap">
      <hostname>imap.example.org</hostname>
      <port>993</port>
      <socketType>SSL</socketType>
      <authentication>password-cleartext</authentication>
      <username>%EMAILADDRESS%</username>
    </incomingServer>
    <incomingServer type='imap'>
      <hostname> imap2.%EMAILDOMAIN% </hostname>
      <port>143</port>
      <socketType>STARTTLS</socketType>
      <username>%EMAILLOCALPART%</username>
    </incomingServer>
    <incomingServer type="pop3">
      <hostname>pop.example.org</hostname><port>995</port><socketType>SSL</socketType>
    </incomingServer>
    <incomingServer type="imap">
      <hostname>plain.example.org</hostname><port>143</port><socketType>plain</socketType>
    </incomingServer>
    <outgoingServer type="smtp">
      <hostname>smtp.example.org</hostname>
      <port>465</port>
      <socketType>TLS</socketType>
    </outgoingServer>
    <outgoingServer type="smtp">
      <hostname>smtp.example.org</hostname><port>587</port><socketType>STARTTLS</socketType>
      <username>&#x61;da&amp;co</username>
    </outgoingServer>
    <documentation url="https://example.org/help"><descr lang="en">Help</descr></documentation>
  </emailProvider>
</clientConfig>
"#;

#[test]
fn a_full_file_gives_its_encrypted_imap_and_submission_servers() {
    let found = parse(FULL.as_bytes(), &address()).unwrap();
    assert_eq!(servers(&found.imap), ["imap.example.org:993 (TLS) as ada@example.org", "imap2.example.org:143 (STARTTLS) as ada"]);
    assert_eq!(servers(&found.submission), ["smtp.example.org:465 (TLS) as ada@example.org", "smtp.example.org:587 (STARTTLS) as ada&co"]);
    assert_eq!(found.imap[1].server.security(), Security::StartTls);
}

#[test]
fn a_byte_order_mark_cdata_and_self_closing_elements_are_read() {
    let xml = "\u{feff}<clientConfig><emailProvider id=\"x\"><empty/><incomingServer type=\"imap\"><hostname><![CDATA[imap.example.org]]></hostname><port>993</port><socketType>SSL</socketType><?pi skipped?><!-- c --></incomingServer></emailProvider></clientConfig>";
    let found = parse(xml.as_bytes(), &address()).unwrap();
    assert_eq!(servers(&found.imap), ["imap.example.org:993 (TLS) as ada@example.org"]);
}

#[test]
fn incomplete_or_bad_servers_are_left_out() {
    let server =
        |body: &str| format!("<clientConfig><emailProvider><incomingServer type=\"imap\">{body}</incomingServer></emailProvider></clientConfig>");
    for body in [
        "<port>993</port><socketType>SSL</socketType>",
        "<hostname>imap.example.org</hostname><socketType>SSL</socketType>",
        "<hostname>imap.example.org</hostname><port>993</port>",
        "<hostname>imap.example.org</hostname><port>0</port><socketType>SSL</socketType>",
        "<hostname>imap.example.org</hostname><port>99999</port><socketType>SSL</socketType>",
        "<hostname>bad host!</hostname><port>993</port><socketType>SSL</socketType>",
        "<hostname>imap.example.org</hostname><port>993</port><socketType>SSL</socketType><username> </username>",
        "<hostname>imap.example.org</hostname><port>993</port><socketType>SSL</socketType><username>a&#10;b</username>",
    ] {
        let found = parse(server(body).as_bytes(), &address()).unwrap();
        assert!(found.imap.is_empty(), "{body}");
    }
    let long = format!("<hostname>imap.example.org</hostname><port>993</port><socketType>SSL</socketType><username>{}</username>", "u".repeat(400));
    assert!(parse(server(&long).as_bytes(), &address()).unwrap().imap.is_empty());
}

#[test]
fn the_same_server_twice_is_kept_once_and_the_list_is_capped() {
    let one = "<incomingServer type=\"imap\"><hostname>imap.example.org</hostname><port>993</port><socketType>SSL</socketType></incomingServer>";
    let xml = format!("<clientConfig><emailProvider>{}</emailProvider></clientConfig>", one.repeat(3));
    assert_eq!(parse(xml.as_bytes(), &address()).unwrap().imap.len(), 1);
    let many: String = (1..40)
        .map(|n| {
            format!(
                "<incomingServer type=\"imap\"><hostname>imap{n}.example.org</hostname><port>993</port><socketType>SSL</socketType></incomingServer>"
            )
        })
        .collect();
    let xml = format!("<clientConfig><emailProvider>{many}</emailProvider></clientConfig>");
    assert_eq!(parse(xml.as_bytes(), &address()).unwrap().imap.len(), MAX_SERVERS);
}

#[test]
fn a_file_that_is_not_autoconfig_gives_nothing() {
    let broken: Vec<String> = vec![
        String::new(),
        "not xml".into(),
        "<html><body>404</body></html>".into(),
        "<clientConfig>".into(),
        "<clientConfig></other>".into(),
        "<clientConfig></clientConfig>trailing".into(),
        "<clientConfig a=1></clientConfig>".into(),
        "<clientConfig a=\"1></clientConfig>".into(),
        "<clientConfig>&unknown;</clientConfig>".into(),
        "<clientConfig>&#xD800;</clientConfig>".into(),
        "<clientConfig>&amp</clientConfig>".into(),
        "<clientConfig><!-- open</clientConfig>".into(),
        "<clientConfig><![CDATA[open</clientConfig>".into(),
        "<!DOCTYPE x [<!ENTITY a \"b\">]><clientConfig></clientConfig>".into(),
        "<?xml open<clientConfig></clientConfig>".into(),
        "<1clientConfig></1clientConfig>".into(),
        "<clientConfig x></clientConfig>".into(),
        format!("{}{}", "<a>".repeat(20), "</a>".repeat(20)),
    ];
    for xml in broken {
        assert_eq!(parse(xml.as_bytes(), &address()), None, "{xml}");
    }
    assert_eq!(parse(&[0xff, 0xfe], &address()), None);
    let huge = format!("<clientConfig>{}</clientConfig>", " ".repeat(MAX_FILE));
    assert_eq!(parse(huge.as_bytes(), &address()), None);
    let wide = format!("<clientConfig>{}</clientConfig>", "<x/>".repeat(3_000));
    assert_eq!(parse(wide.as_bytes(), &address()), None);
}

#[test]
fn an_empty_provider_list_is_an_autoconfig_without_servers() {
    let found = parse(b"<clientConfig version='1.1'/>", &address()).unwrap();
    assert!(found.imap.is_empty() && found.submission.is_empty());
    let found = parse(b"<clientConfig><emailProvider><outgoingServer type=\"other\"/></emailProvider></clientConfig>", &address()).unwrap();
    assert!(found.submission.is_empty());
}
