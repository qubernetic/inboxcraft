// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::num::NonZeroU16;

use inboxcraft_model::{Account, AccountId, EmailAddress, ErrorKind, Host, Security, Server};
use inboxcraft_testkit::TestPki;
use inboxcraft_time::{CivilDateTime, Offset, Timestamp};

use super::{CheckTarget, clock_checked, is_certificate_fault};
use crate::certificate;

fn at(year: i32) -> Timestamp {
    CivilDateTime::new(year, 6, 1, 0, 0, 0).unwrap().to_timestamp(Offset::UTC).unwrap()
}

#[test]
fn a_saved_account_is_checked_with_its_own_settings_as_seen() {
    let imap = Server::new(Host::new("imap.example.org").unwrap(), NonZeroU16::new(993).unwrap(), Security::Tls);
    let account = Account::new(AccountId::new(1), "", EmailAddress::new("ada@example.org").unwrap(), "ada", imap.clone()).unwrap();
    let target = CheckTarget::of(&account);
    assert_eq!((target.imap(), target.username(), target.is_reviewed()), (&imap, "ada", true));
    assert!(!CheckTarget::detected(imap, "ada").is_reviewed());
}

#[test]
fn only_the_certificate_kinds_belong_to_the_certificate_step() {
    for kind in [
        ErrorKind::CertificateUnknownIssuer,
        ErrorKind::CertificateExpired,
        ErrorKind::CertificateNotYetValid,
        ErrorKind::CertificateWrongHost,
        ErrorKind::ClockFarOff,
    ] {
        assert!(is_certificate_fault(kind), "{kind:?}");
    }
    for kind in [ErrorKind::Tls, ErrorKind::StartTlsMissing, ErrorKind::ConnectionDropped, ErrorKind::Protocol] {
        assert!(!is_certificate_fault(kind), "{kind:?}");
    }
}

fn tlv(tag: u8, contents: &[u8]) -> Vec<u8> {
    [vec![tag, u8::try_from(contents.len()).unwrap()], contents.to_vec()].concat()
}

/// A certificate valid from 2026-03-01 to 2027-03-01.
fn certificate() -> certificate::Certificate {
    let name = tlv(0x30, &tlv(0x31, &tlv(0x30, &[tlv(0x06, &[0x55, 0x04, 0x03]), tlv(0x0c, b"CA")].concat())));
    let validity = tlv(0x30, &[tlv(0x17, b"260301000000Z"), tlv(0x17, b"270301000000Z")].concat());
    let tbs = tlv(0x30, &[tlv(0x02, &[1]), tlv(0x30, &tlv(0x06, &[0x2a])), name, validity].concat());
    certificate::read(&tlv(0x30, &tbs)).unwrap()
}

#[test]
fn the_test_ca_is_read() {
    let pki = TestPki::new().unwrap();
    assert!(certificate::read(pki.ca().as_ref()).is_some());
}

#[test]
fn the_clock_check_blames_a_clock_far_from_the_certificate() {
    let ca = certificate();
    // At a sensible clock, a certificate outside its validity is its own fault: three months after its end, or a
    // few hours before its start.
    assert_eq!(clock_checked(ErrorKind::CertificateExpired, Some(&ca), at(2027)), ErrorKind::CertificateExpired);
    let just_before = CivilDateTime::new(2026, 2, 28, 20, 0, 0).unwrap().to_timestamp(Offset::UTC).unwrap();
    assert_eq!(clock_checked(ErrorKind::CertificateNotYetValid, Some(&ca), just_before), ErrorKind::CertificateNotYetValid);
    // A clock before this program existed, years past the end, or weeks before the start: the clock.
    assert_eq!(clock_checked(ErrorKind::CertificateExpired, Some(&ca), at(2001)), ErrorKind::ClockFarOff);
    assert_eq!(clock_checked(ErrorKind::CertificateExpired, Some(&ca), at(2120)), ErrorKind::ClockFarOff);
    let weeks_before = CivilDateTime::new(2026, 1, 20, 0, 0, 0).unwrap().to_timestamp(Offset::UTC).unwrap();
    assert_eq!(clock_checked(ErrorKind::CertificateNotYetValid, Some(&ca), weeks_before), ErrorKind::ClockFarOff);
    // Without the certificate's dates, or for another fault, the kind stays.
    assert_eq!(clock_checked(ErrorKind::CertificateExpired, None, at(2120)), ErrorKind::CertificateExpired);
    assert_eq!(clock_checked(ErrorKind::CertificateWrongHost, Some(&ca), at(2001)), ErrorKind::CertificateWrongHost);
}
