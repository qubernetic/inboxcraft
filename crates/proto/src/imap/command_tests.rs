// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{Command, MAX_QUOTED};

fn finished(command: Command) -> (String, Vec<String>) {
    let (first, rest) = command.finish("A0001");
    (String::from_utf8_lossy(&first).into_owned(), rest.iter().map(|chunk| String::from_utf8_lossy(chunk).into_owned()).collect())
}

#[test]
fn words_and_quoted_strings_go_in_one_line() {
    let command = Command::new("LOGIN").string(b"ada@example.org").string(br#"pa"ss\word"#);
    assert_eq!(finished(command), ("A0001 LOGIN \"ada@example.org\" \"pa\\\"ss\\\\word\"\r\n".to_owned(), vec![]));
    assert_eq!(finished(Command::new("LIST").arg("\"\"").arg("\"*\"")), ("A0001 LIST \"\" \"*\"\r\n".to_owned(), vec![]));
    assert_eq!(finished(Command::new("SELECT").string(b"")), ("A0001 SELECT \"\"\r\n".to_owned(), vec![]));
}

#[test]
fn what_quotes_cannot_carry_goes_as_a_literal() {
    let command = Command::new("LOGIN").string(b"ada").string("jelszó".as_bytes()).arg("X");
    assert_eq!(finished(command), ("A0001 LOGIN \"ada\" {7}\r\n".to_owned(), vec!["jelszó X\r\n".to_owned()]));
    let command = Command::new("SELECT").string(b"a\r\nb");
    assert_eq!(finished(command), ("A0001 SELECT {4}\r\n".to_owned(), vec!["a\r\nb\r\n".to_owned()]));
    let long = vec![b'x'; MAX_QUOTED + 1];
    let (first, rest) = finished(Command::new("SELECT").string(&long));
    assert_eq!(first, format!("A0001 SELECT {{{}}}\r\n", MAX_QUOTED + 1));
    assert_eq!(rest.len(), 1);
    let (first, rest) = finished(Command::new("SELECT").string(&long[..MAX_QUOTED]));
    assert_eq!(first.len(), "A0001 SELECT \"\"\r\n".len() + MAX_QUOTED);
    assert!(rest.is_empty());
}

#[test]
fn two_literals_are_two_go_aheads() {
    let command = Command::new("LOGIN").string("ádá".as_bytes()).string("jó".as_bytes());
    assert_eq!(finished(command), ("A0001 LOGIN {5}\r\n".to_owned(), vec!["ádá {3}\r\n".to_owned(), "jó\r\n".to_owned()]));
}
