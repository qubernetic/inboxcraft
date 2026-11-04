// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::io::Write;

use flate2::Compression;
use flate2::write::DeflateEncoder;
use inboxcraft_model::ErrorKind;

use super::Deflate;
use crate::NetError;

fn deflated(data: &[u8]) -> Vec<u8> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(data).unwrap();
    encoder.flush().unwrap();
    encoder.get_ref().clone()
}

fn inflate_all(deflate: &mut Deflate, data: &[u8]) -> Result<(Vec<u8>, usize), NetError> {
    let mut out = Vec::new();
    let mut largest = 0;
    deflate.inflate(data, &mut |piece| {
        largest = largest.max(piece.len());
        out.extend_from_slice(piece);
        Ok(())
    })?;
    Ok((out, largest))
}

#[test]
fn what_one_end_compresses_the_other_inflates() {
    let mut client = Deflate::new();
    let mut server = Deflate::new();
    let first = client.compress(b"a1 UID FETCH 1:* (FLAGS)\r\n").unwrap();
    let second = client.compress(b"a2 NOOP\r\n").unwrap();
    assert!(!first.is_empty() && !second.is_empty());
    assert_eq!(inflate_all(&mut server, &first).unwrap().0, b"a1 UID FETCH 1:* (FLAGS)\r\n");
    assert_eq!(inflate_all(&mut server, &second).unwrap().0, b"a2 NOOP\r\n");
}

#[test]
fn a_stream_split_anywhere_inflates_the_same() {
    let text = b"* 1 FETCH (UID 1 FLAGS (\\Seen))\r\n".repeat(50);
    let data = deflated(&text);
    let mut deflate = Deflate::new();
    let mut out = Vec::new();
    for piece in data.chunks(7) {
        out.extend(inflate_all(&mut deflate, piece).unwrap().0);
    }
    assert_eq!(out, text);
}

#[test]
fn a_bomb_is_handed_on_in_small_pieces() {
    let data = deflated(&vec![b'a'; 8 * 1024 * 1024]);
    let mut deflate = Deflate::new();
    let (out, largest) = inflate_all(&mut deflate, &data).unwrap();
    assert_eq!(out.len(), 8 * 1024 * 1024);
    assert!(largest <= 128 * 1024, "{largest}");
}

#[test]
fn the_taker_can_stop_the_inflating() {
    let data = deflated(&vec![b'a'; 1024 * 1024]);
    let mut deflate = Deflate::new();
    let mut calls = 0;
    let stopped = deflate.inflate(&data, &mut |_| {
        calls += 1;
        Err(NetError::new(ErrorKind::TooLarge, "enough"))
    });
    assert_eq!(stopped.map_err(|error| error.kind()), Err(ErrorKind::TooLarge));
    assert_eq!(calls, 1);
}

#[test]
fn garbage_is_a_protocol_error() {
    let mut deflate = Deflate::new();
    let error = inflate_all(&mut deflate, &[0xff; 64]).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Protocol);
    assert!(error.detail().starts_with("the compressed stream is broken"), "{error}");
}
