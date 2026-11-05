// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_testkit::TestPki;
use inboxcraft_time::{CivilDateTime, Offset, Timestamp};

use super::read;

fn tlv(tag: u8, contents: &[u8]) -> Vec<u8> {
    let mut out = vec![tag];
    let length = contents.len();
    if length < 0x80 {
        out.push(u8::try_from(length).unwrap());
    } else if length < 0x100 {
        out.extend([0x81, u8::try_from(length).unwrap()]);
    } else {
        out.extend([0x82, u8::try_from(length >> 8).unwrap(), u8::try_from(length & 0xff).unwrap()]);
    }
    out.extend_from_slice(contents);
    out
}

fn attribute(oid: &[u8], tag: u8, value: &[u8]) -> Vec<u8> {
    tlv(0x31, &tlv(0x30, &[tlv(0x06, oid), tlv(tag, value)].concat()))
}

const CN: &[u8] = &[0x55, 0x04, 0x03];
const O: &[u8] = &[0x55, 0x04, 0x0a];

/// A certificate with `issuer` (the contents of the Name) and `validity` (the contents of the Validity).
fn certificate(version: bool, issuer: &[u8], validity: &[u8]) -> Vec<u8> {
    let mut tbs = Vec::new();
    if version {
        tbs.extend(tlv(0xa0, &tlv(0x02, &[2])));
    }
    tbs.extend(tlv(0x02, &[1, 2, 3]));
    tbs.extend(tlv(0x30, &tlv(0x06, &[0x2a, 0x86, 0x48])));
    tbs.extend(tlv(0x30, issuer));
    tbs.extend(tlv(0x30, validity));
    tbs.extend(tlv(0x30, &attribute(CN, 0x0c, b"mail.example.org")));
    tlv(0x30, &[tlv(0x30, &tbs), tlv(0x30, &tlv(0x06, &[0x2a])), tlv(0x03, &[0])].concat())
}

fn validity() -> Vec<u8> {
    [tlv(0x17, b"990102030405Z"), tlv(0x18, b"20510607080910Z")].concat()
}

fn at(year: i32, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> Timestamp {
    CivilDateTime::new(year, month, day, hour, minute, second).unwrap().to_timestamp(Offset::UTC).unwrap()
}

#[test]
fn the_test_ca_names_itself_and_is_valid_now() {
    let pki = TestPki::new().unwrap();
    let ca = read(pki.ca().as_ref()).unwrap();
    assert!(ca.issuer().contains("CN=InboxCraft test CA"), "{}", ca.issuer());
    let now = inboxcraft_time::Clock::now(&inboxcraft_time::SystemClock::new());
    assert!(ca.not_before() <= now && now < ca.not_after(), "{ca:?}");
}

#[test]
fn the_issuer_keeps_the_known_attributes_in_order() {
    let issuer = [
        attribute(&[0x55, 0x04, 0x06], 0x13, b"HU"),
        attribute(O, 0x13, b"Example"),
        attribute(&[0x55, 0x04, 0x0b], 0x1e, &[0x00, b'I', 0x00, b'T']),
        attribute(&[0x55, 0x04, 0x07], 0x0c, b"Budapest"),
        attribute(CN, 0x0c, "Példa CA".as_bytes()),
    ]
    .concat();
    let read = read(&certificate(true, &issuer, &validity())).unwrap();
    assert_eq!(read.issuer(), "C=HU, O=Example, OU=IT, CN=Példa CA");
    assert_eq!(read.not_before(), at(1999, 1, 2, 3, 4, 5));
    assert_eq!(read.not_after(), at(2051, 6, 7, 8, 9, 10));
}

#[test]
fn a_certificate_without_a_version_field_is_read_too() {
    let read = read(&certificate(false, &attribute(CN, 0x0c, b"Old CA"), &validity())).unwrap();
    assert_eq!(read.issuer(), "CN=Old CA");
}

#[test]
fn every_string_type_is_decoded_and_control_characters_go() {
    let cases: [(u8, &[u8], &str); 7] = [
        (0x0c, b"UTF8\r\nX", "UTF8  X"),
        (0x13, b"Printable", "Printable"),
        (0x16, b"ia5", "ia5"),
        (0x14, &[b'T', 0xe9, b'l'], "Tél"),
        (0x14, "Tél".as_bytes(), "Tél"),
        (0x1e, &[0xd8, 0x00, 0x00], "\u{fffd}\u{fffd}"),
        (0x1c, &[0, 0, 0, b'U', 0xff, 0xff, 0xff, 0xff, 1], "U\u{fffd}\u{fffd}"),
    ];
    for (tag, value, expected) in cases {
        let read = read(&certificate(true, &attribute(CN, tag, value), &validity())).unwrap();
        assert_eq!(read.issuer(), format!("CN={expected}"), "{tag:#x}");
    }
}

#[test]
fn long_values_and_many_attributes_are_capped() {
    let long = "A".repeat(500);
    let read_long = read(&certificate(true, &attribute(CN, 0x0c, long.as_bytes()), &validity())).unwrap();
    assert_eq!(read_long.issuer().len(), "CN=".len() + 128);
    let many: Vec<u8> = (0..40).flat_map(|_| attribute(O, 0x13, b"x")).collect();
    let read_many = read(&certificate(true, &many, &validity())).unwrap();
    assert_eq!(read_many.issuer().matches("O=x").count(), 16);
}

#[test]
fn an_issuer_with_only_other_attributes_is_empty() {
    let read = read(&certificate(true, &attribute(&[0x55, 0x04, 0x07], 0x0c, b"Town"), &validity())).unwrap();
    assert_eq!(read.issuer(), "");
}

#[test]
fn broken_encodings_are_refused() {
    let good = certificate(true, &attribute(CN, 0x0c, b"CA"), &validity());
    assert!(read(&good).is_some());
    for cut in 0..good.len() {
        assert_eq!(read(&good[..cut]), None, "cut at {cut}");
    }
    let broken: [Vec<u8>; 11] = [
        Vec::new(),
        vec![0x31, 0x00],
        vec![0x30, 0x80, 0x00, 0x00],
        vec![0x30, 0x85, 0, 0, 0, 0, 1],
        vec![0x30, 0x84, 0xff, 0xff, 0xff, 0xff],
        vec![0x3f, 0x00],
        certificate(true, &attribute(CN, 0x05, b"CA"), &validity()),
        certificate(true, &tlv(0x30, b""), &validity()),
        certificate(true, &tlv(0x31, &tlv(0x02, &[1])), &validity()),
        certificate(true, &tlv(0x31, &tlv(0x30, &tlv(0x02, &[1]))), &validity()),
        certificate(true, &attribute(CN, 0x0c, b"CA"), &tlv(0x17, b"990102030405Z")),
    ];
    for der in broken {
        assert_eq!(read(&der), None, "{der:02x?}");
    }
}

#[test]
fn broken_times_are_refused() {
    let issuer = attribute(CN, 0x0c, b"CA");
    for time in [
        tlv(0x17, b"9901020304Z"),
        tlv(0x17, b"990102030405"),
        tlv(0x17, b"99010203040aZ"),
        tlv(0x17, b"991302030405Z"),
        tlv(0x18, b"990102030405Z"),
        tlv(0x02, b"990102030405Z"),
    ] {
        let validity = [time.clone(), tlv(0x17, b"990102030405Z")].concat();
        assert_eq!(read(&certificate(true, &issuer, &validity)), None, "{time:02x?}");
    }
}

#[test]
fn utc_time_years_before_fifty_are_this_century() {
    let validity = [tlv(0x17, b"490102030405Z"), tlv(0x17, b"500102030405Z")].concat();
    let read = read(&certificate(true, &attribute(CN, 0x0c, b"CA"), &validity)).unwrap();
    assert_eq!(read.not_before(), at(2049, 1, 2, 3, 4, 5));
    assert_eq!(read.not_after(), at(1950, 1, 2, 3, 4, 5));
}

#[test]
fn long_form_lengths_are_read() {
    let issuer: Vec<u8> = (0..30).flat_map(|_| attribute(O, 0x13, b"long-form")).collect();
    assert!(issuer.len() > 255);
    let read = read(&certificate(true, &issuer, &validity())).unwrap();
    assert!(read.issuer().starts_with("O=long-form"));
}
