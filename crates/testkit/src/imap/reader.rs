// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The fake's own command reader: a tag, then atoms, quoted strings, literals and parenthesised lists.
//!
//! It shares nothing with `proto`, so a parser bug there cannot hide behind the same bug here. It reads a command
//! that the session has already split into lines, each but the last ending in a literal announcement, and the
//! literals that followed them.

/// The longest literal the fake reads.
pub(crate) const MAX_LITERAL: usize = 64 * 1024;
/// The most literals in one command.
pub(crate) const MAX_LITERALS: usize = 8;
/// How deep parenthesised lists may nest.
const MAX_DEPTH: usize = 8;

/// One argument of a command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Token {
    /// An atom, as written.
    Atom(String),
    /// A quoted string or a literal.
    Str(Vec<u8>),
    /// A parenthesised list.
    List(Vec<Token>),
}

impl Token {
    /// The atom, or `None` for a string or a list.
    pub(crate) fn atom(&self) -> Option<&str> {
        match self {
            Self::Atom(atom) => Some(atom),
            _ => None,
        }
    }

    /// An atom or a string as bytes (an `astring`), or `None` for a list.
    pub(crate) fn astring(&self) -> Option<&[u8]> {
        match self {
            Self::Atom(atom) => Some(atom.as_bytes()),
            Self::Str(bytes) => Some(bytes),
            Self::List(_) => None,
        }
    }
}

/// A command as read: its tag, its name in upper case (`UID FETCH` for the UID forms) and its arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Command {
    pub(crate) tag: String,
    pub(crate) name: String,
    pub(crate) args: Vec<Token>,
}

/// A command line the reader refuses: the tag when there was one, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Refused {
    pub(crate) tag: Option<String>,
    pub(crate) reason: &'static str,
}

/// The literal a line announces at its end: its length and whether it is non-synchronising (`{n+}`).
pub(crate) fn literal_at_end(line: &[u8]) -> Option<(usize, bool)> {
    let inner = line.strip_suffix(b"}")?;
    let open = inner.iter().rposition(|&byte| byte == b'{')?;
    let digits = inner.get(open + 1..)?;
    let (digits, plus) = digits.strip_suffix(b"+").map_or((digits, false), |digits| (digits, true));
    if digits.is_empty() || digits.len() > 10 || !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    Some((std::str::from_utf8(digits).ok()?.parse().ok()?, plus))
}

/// The tag of a command line, when it has a usable one.
pub(crate) fn tag_of(line: &[u8]) -> Option<String> {
    let end = line.iter().position(|&byte| byte == b' ')?;
    let tag = line.get(..end)?;
    (!tag.is_empty() && tag.iter().all(|&byte| atom_byte(byte) && byte != b'+')).then(|| String::from_utf8_lossy(tag).into_owned())
}

/// Reads one command from its lines and literals.
pub(crate) fn read(lines: &[Vec<u8>], literals: &[Vec<u8>]) -> Result<Command, Refused> {
    let first = lines.first().map_or(&[][..], Vec::as_slice);
    if first.is_empty() {
        return Err(Refused { tag: None, reason: "Empty line" });
    }
    let Some(tag) = tag_of(first) else {
        let reason = if first.contains(&b' ') { "Bad tag" } else { "Missing command" };
        return Err(Refused { tag: None, reason });
    };
    let refuse = |reason| Refused { tag: Some(tag.clone()), reason };
    let mut cursor = Cursor { lines, literals, line: 0, at: tag.len() };
    let mut args = cursor.list(0).map_err(refuse)?;
    let mut name = match args.first() {
        Some(Token::Atom(word)) => word.to_ascii_uppercase(),
        _ => return Err(refuse("Missing command")),
    };
    args.remove(0);
    if name == "UID" {
        let Some(Token::Atom(word)) = args.first() else { return Err(refuse("Missing command")) };
        name = format!("UID {}", word.to_ascii_uppercase());
        args.remove(0);
    }
    Ok(Command { tag, name, args })
}

/// Whether `byte` may stand in an atom: printable ASCII but the specials that end one.
fn atom_byte(byte: u8) -> bool {
    byte.is_ascii_graphic() && !matches!(byte, b'(' | b')' | b'{' | b'"')
}

struct Cursor<'a> {
    lines: &'a [Vec<u8>],
    literals: &'a [Vec<u8>],
    line: usize,
    at: usize,
}

impl Cursor<'_> {
    fn text(&self) -> &[u8] {
        self.lines.get(self.line).map_or(&[][..], Vec::as_slice)
    }

    fn peek(&self) -> Option<u8> {
        self.text().get(self.at).copied()
    }

    /// The tokens up to the end of the command (`depth` 0) or the `)` that closes the list being read.
    fn list(&mut self, depth: usize) -> Result<Vec<Token>, &'static str> {
        let mut tokens = Vec::new();
        loop {
            while self.peek() == Some(b' ') {
                self.at += 1;
            }
            match self.peek() {
                None if depth == 0 => return Ok(tokens),
                None => return Err("Unclosed list"),
                Some(b')') if depth == 0 => return Err("Unexpected )"),
                Some(b')') => {
                    self.at += 1;
                    return Ok(tokens);
                }
                Some(b'(') if depth >= MAX_DEPTH => return Err("Lists nested too deep"),
                Some(b'(') => {
                    self.at += 1;
                    tokens.push(Token::List(self.list(depth + 1)?));
                }
                Some(b'"') => tokens.push(Token::Str(self.quoted()?)),
                Some(b'{') => tokens.push(Token::Str(self.literal()?)),
                Some(_) => tokens.push(Token::Atom(self.atom()?)),
            }
        }
    }

    fn quoted(&mut self) -> Result<Vec<u8>, &'static str> {
        let mut out = Vec::new();
        self.at += 1;
        loop {
            let byte = self.peek().ok_or("Unterminated quoted string")?;
            self.at += 1;
            match byte {
                b'"' => return Ok(out),
                b'\\' => match self.peek() {
                    Some(escaped @ (b'"' | b'\\')) => {
                        self.at += 1;
                        out.push(escaped);
                    }
                    _ => return Err("Bad escape in a quoted string"),
                },
                0 => return Err("Bad quoted string"),
                _ => out.push(byte),
            }
        }
    }

    /// A literal: the announcement must end this line, and the literal that followed it comes next.
    fn literal(&mut self) -> Result<Vec<u8>, &'static str> {
        let rest = self.text().get(self.at..).unwrap_or_default();
        let literal = self.literals.get(self.line).filter(|_| literal_at_end(rest).is_some() && !rest.get(1..).unwrap_or_default().contains(&b'{'));
        let literal = literal.ok_or("Unexpected {")?.clone();
        self.line += 1;
        self.at = 0;
        Ok(literal)
    }

    fn atom(&mut self) -> Result<String, &'static str> {
        let start = self.at;
        while self.peek().is_some_and(atom_byte) {
            self.at += 1;
        }
        let atom = self.text().get(start..self.at).unwrap_or_default();
        if atom.is_empty() {
            return Err("Bad atom");
        }
        Ok(String::from_utf8_lossy(atom).into_owned())
    }
}
