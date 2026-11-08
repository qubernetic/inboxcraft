// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The fake's own command reader: a verb, then its arguments, with the paths and parameters of MAIL and RCPT.
//!
//! It shares nothing with `proto`, so a parser bug there cannot hide behind the same bug here. It reads one command
//! line that the session has already cut at its line end.

/// A command as read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Command {
    Ehlo(String),
    Helo(String),
    StartTls,
    Auth {
        mechanism: String,
        initial: Option<String>,
    },
    /// MAIL FROM: the path without its angle brackets, and the parameters.
    Mail {
        path: String,
        params: Vec<String>,
    },
    /// RCPT TO: the path without its angle brackets, and the parameters.
    Rcpt {
        path: String,
        params: Vec<String>,
    },
    Data,
    Rset,
    Noop,
    Quit,
}

/// Why the reader refused a line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Refused {
    /// No such command (`500 5.5.2`).
    Unknown,
    /// The command's arguments cannot be read (`501 5.5.4`), and why.
    Syntax(&'static str),
}

/// Reads one command line without its line end: the verb in upper case (empty when the line has none) and the
/// command, or why it was refused.
pub(super) fn read(line: &[u8]) -> (String, Result<Command, Refused>) {
    let Ok(line) = std::str::from_utf8(line) else { return (String::new(), Err(Refused::Syntax("The line is not UTF-8"))) };
    let (verb, rest) = line.split_once(' ').unwrap_or((line, ""));
    let verb = verb.to_ascii_uppercase();
    let words: Vec<&str> = rest.split(' ').filter(|word| !word.is_empty()).collect();
    let command = match verb.as_str() {
        "EHLO" | "HELO" => match words.as_slice() {
            [domain] if verb == "EHLO" => Ok(Command::Ehlo((*domain).to_owned())),
            [domain] => Ok(Command::Helo((*domain).to_owned())),
            _ => Err(Refused::Syntax("EHLO and HELO need one domain")),
        },
        "STARTTLS" => bare(&words, Command::StartTls),
        "DATA" => bare(&words, Command::Data),
        "RSET" => bare(&words, Command::Rset),
        "QUIT" => bare(&words, Command::Quit),
        "NOOP" => Ok(Command::Noop),
        "AUTH" => match words.as_slice() {
            [mechanism] => Ok(Command::Auth { mechanism: mechanism.to_ascii_uppercase(), initial: None }),
            [mechanism, initial] => Ok(Command::Auth { mechanism: mechanism.to_ascii_uppercase(), initial: Some((*initial).to_owned()) }),
            _ => Err(Refused::Syntax("AUTH needs a mechanism")),
        },
        "MAIL" => path(rest, "FROM:").map(|(path, params)| Command::Mail { path, params }),
        "RCPT" => path(rest, "TO:").map(|(path, params)| Command::Rcpt { path, params }),
        _ => Err(Refused::Unknown),
    };
    (verb, command)
}

/// `command` when there are no arguments.
fn bare(words: &[&str], command: Command) -> Result<Command, Refused> {
    if words.is_empty() { Ok(command) } else { Err(Refused::Syntax("The command takes no arguments")) }
}

/// Reads `FROM:<path> params` or `TO:<path> params`: `prefix` in any case, then the path in angle brackets (a space
/// before it is let pass), then the parameters, each after a space.
fn path(rest: &str, prefix: &str) -> Result<(String, Vec<String>), Refused> {
    let syntax = Refused::Syntax("Expected FROM:<path> or TO:<path>");
    let head = rest.get(..prefix.len()).filter(|head| head.eq_ignore_ascii_case(prefix)).ok_or(syntax)?;
    let after = rest.get(head.len()..).unwrap_or_default().trim_start_matches(' ');
    let inner = after.strip_prefix('<').ok_or(syntax)?;
    let (path, tail) = inner.split_once('>').ok_or(syntax)?;
    if path.contains(['<', ' ']) || !(tail.is_empty() || tail.starts_with(' ')) {
        return Err(syntax);
    }
    let params = tail.split(' ').filter(|param| !param.is_empty()).map(str::to_owned).collect();
    Ok((path.to_owned(), params))
}
