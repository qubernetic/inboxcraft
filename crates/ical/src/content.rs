// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Content lines and components (RFC 5545 section 3.1 and 3.4): the untyped tree every calendar object is.

use crate::IcalError;

/// The longest input [`parse`] reads, in bytes.
pub const MAX_BYTES: usize = 4 * 1024 * 1024;

/// The most properties and components [`parse`] reads, all levels together.
pub const MAX_ITEMS: usize = 50_000;

/// The deepest nesting of components [`parse`] reads: `VCALENDAR` is 1, a `VALARM` in a `VEVENT` 3.
pub const MAX_DEPTH: usize = 8;

/// The longest line [`Component::write`] writes, in octets without the line break (RFC 5545 section 3.1).
const FOLD_AT: usize = 75;

/// A property parameter: a name and one or more values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Param {
    name: String,
    values: Vec<String>,
}

impl Param {
    /// The parameter's name, in capitals.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The values, at least one, quotes and RFC 6868 escapes undone.
    #[must_use]
    pub fn values(&self) -> &[String] {
        &self.values
    }
}

/// A property: a name, its parameters and its value as written (text escapes not undone).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Property {
    name: String,
    params: Vec<Param>,
    value: String,
}

impl Property {
    /// A property `name` (put in capitals) with the raw `value`.
    ///
    /// The value is written as it is, so a text value goes through [`crate::escape_text`] first; CR, LF and NUL are
    /// left out when it is written.
    #[must_use]
    pub fn new(name: &str, value: impl Into<String>) -> Self {
        Self { name: name.to_ascii_uppercase(), params: Vec::new(), value: value.into() }
    }

    /// The property with the parameter `name` set to `value`, replacing a parameter of that name.
    #[must_use]
    pub fn with_param(self, name: &str, value: impl Into<String>) -> Self {
        self.with_param_values(name, vec![value.into()])
    }

    /// The property with the parameter `name` set to `values` (one empty value when there are none).
    #[must_use]
    pub fn with_param_values(mut self, name: &str, values: Vec<String>) -> Self {
        let name = name.to_ascii_uppercase();
        let values = if values.is_empty() { vec![String::new()] } else { values };
        self.params.retain(|param| param.name != name);
        self.params.push(Param { name, values });
        self
    }

    /// The name, in capitals.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The raw value.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }

    /// The parameters, in the order they came.
    #[must_use]
    pub fn params(&self) -> &[Param] {
        &self.params
    }

    /// The first value of the parameter `name` (any case).
    #[must_use]
    pub fn param(&self, name: &str) -> Option<&str> {
        let param = self.params.iter().find(|param| param.name.eq_ignore_ascii_case(name))?;
        param.values.first().map(String::as_str)
    }

    /// The content line, unfolded, without its line break.
    fn line(&self) -> Result<String, IcalError> {
        if self.name == "BEGIN" || self.name == "END" {
            return Err(IcalError::bad_name(&self.name));
        }
        let mut line = name_checked(&self.name)?.to_owned();
        for param in &self.params {
            line.push(';');
            line.push_str(name_checked(&param.name)?);
            line.push('=');
            for (index, value) in param.values.iter().enumerate() {
                if index > 0 {
                    line.push(',');
                }
                push_param_value(&mut line, value);
            }
        }
        line.push(':');
        line.extend(self.value.chars().filter(|c| !matches!(c, '\r' | '\n' | '\0')));
        Ok(line)
    }
}

/// A component: `BEGIN:<name>`, its properties and the components inside it, `END:<name>`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Component {
    name: String,
    properties: Vec<Property>,
    components: Vec<Component>,
}

impl Component {
    /// An empty component `name` (put in capitals).
    #[must_use]
    pub fn new(name: &str) -> Self {
        Self { name: name.to_ascii_uppercase(), properties: Vec::new(), components: Vec::new() }
    }

    /// The component with `property` added after the others.
    #[must_use]
    pub fn with_property(mut self, property: Property) -> Self {
        self.properties.push(property);
        self
    }

    /// The component with `component` added inside it, after the others.
    #[must_use]
    pub fn with_component(mut self, component: Component) -> Self {
        self.components.push(component);
        self
    }

    /// Adds `property` after the others.
    pub fn push_property(&mut self, property: Property) {
        self.properties.push(property);
    }

    /// Adds `component` inside, after the others.
    pub fn push_component(&mut self, component: Component) {
        self.components.push(component);
    }

    /// The name, in capitals.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The properties, in the order they came.
    #[must_use]
    pub fn properties(&self) -> &[Property] {
        &self.properties
    }

    /// The components inside, in the order they came.
    #[must_use]
    pub fn components(&self) -> &[Component] {
        &self.components
    }

    /// The first property `name` (any case).
    #[must_use]
    pub fn property(&self, name: &str) -> Option<&Property> {
        self.properties.iter().find(|property| property.name.eq_ignore_ascii_case(name))
    }

    /// Every property `name` (any case).
    pub fn properties_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Property> + 'a {
        self.properties.iter().filter(move |property| property.name.eq_ignore_ascii_case(name))
    }

    /// Every component inside named `name` (any case).
    pub fn components_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Component> + 'a {
        self.components.iter().filter(move |component| component.name.eq_ignore_ascii_case(name))
    }

    /// The component as iCalendar text: CRLF line breaks, lines folded at 75 octets.
    ///
    /// # Errors
    ///
    /// [`IcalError::BadName`] when a component, property or parameter name is not letters, digits and hyphens, or a
    /// property is named `BEGIN` or `END`.
    pub fn write(&self) -> Result<String, IcalError> {
        let mut out = String::new();
        self.write_into(&mut out)?;
        Ok(out)
    }

    fn write_into(&self, out: &mut String) -> Result<(), IcalError> {
        let name = name_checked(&self.name)?;
        push_folded(out, &format!("BEGIN:{name}"));
        for property in &self.properties {
            push_folded(out, &property.line()?);
        }
        for component in &self.components {
            component.write_into(out)?;
        }
        push_folded(out, &format!("END:{name}"));
        Ok(())
    }
}

/// Reads the one component at the top of `input` (a calendar object is one `VCALENDAR`).
///
/// Lines are unfolded first (a line break followed by a space or a tab), on the bytes, so a fold inside a UTF-8
/// character does no harm; bytes that are no UTF-8 become U+FFFD. Bare LF line breaks are read as CRLF, empty lines
/// are skipped, names are put in capitals.
///
/// # Errors
///
/// An [`IcalError`]: input over [`MAX_BYTES`], over [`MAX_ITEMS`] items or nested deeper than [`MAX_DEPTH`], a line
/// that is no content line, an `END` that does not match, a component left open, or not exactly one component at the
/// top.
pub fn parse(input: &[u8]) -> Result<Component, IcalError> {
    if input.len() > MAX_BYTES {
        return Err(IcalError::TooLarge { limit: MAX_BYTES });
    }
    let mut stack: Vec<Component> = Vec::new();
    let mut top: Option<Component> = None;
    let mut items = 0usize;
    for (index, raw) in unfold(input).iter().enumerate() {
        let number = index.saturating_add(1);
        let property = read_line(&String::from_utf8_lossy(raw)).ok_or(IcalError::BadLine { line: number })?;
        items = items.saturating_add(1);
        if items > MAX_ITEMS {
            return Err(IcalError::TooManyItems { limit: MAX_ITEMS });
        }
        if top.is_some() {
            return Err(IcalError::NotACalendar);
        }
        match property.name.as_str() {
            "BEGIN" => {
                if stack.len() >= MAX_DEPTH {
                    return Err(IcalError::TooDeep { limit: MAX_DEPTH });
                }
                if !is_name(&property.value) {
                    return Err(IcalError::BadLine { line: number });
                }
                stack.push(Component::new(&property.value));
            }
            "END" => {
                let open = stack.pop().ok_or(IcalError::Unbalanced { line: number })?;
                if !open.name.eq_ignore_ascii_case(&property.value) {
                    return Err(IcalError::Unbalanced { line: number });
                }
                match stack.last_mut() {
                    Some(parent) => parent.components.push(open),
                    None => top = Some(open),
                }
            }
            _ => stack.last_mut().ok_or(IcalError::Unbalanced { line: number })?.properties.push(property),
        }
    }
    if !stack.is_empty() {
        return Err(IcalError::Unterminated);
    }
    top.ok_or(IcalError::NotACalendar)
}

/// The logical lines of `input`: folds undone, line ends dropped, empty lines left out.
fn unfold(input: &[u8]) -> Vec<Vec<u8>> {
    let mut lines: Vec<Vec<u8>> = Vec::new();
    for physical in input.split(|byte| *byte == b'\n') {
        let physical = physical.strip_suffix(b"\r").unwrap_or(physical);
        match (physical.split_first(), lines.last_mut()) {
            (Some((b' ' | b'\t', rest)), Some(last)) => last.extend_from_slice(rest),
            _ if physical.is_empty() => {}
            _ => lines.push(physical.to_vec()),
        }
    }
    lines
}

/// One content line, or `None` when it is none.
fn read_line(line: &str) -> Option<Property> {
    let name_end = line.find([';', ':'])?;
    let name = line.get(..name_end)?;
    if !is_name(name) {
        return None;
    }
    let mut property = Property::new(name, String::new());
    let mut rest = line.get(name_end..)?;
    while let Some(after) = rest.strip_prefix(';') {
        let equals = after.find('=')?;
        let param_name = after.get(..equals)?;
        if !is_name(param_name) {
            return None;
        }
        let mut values = Vec::new();
        rest = after.get(equals..)?;
        while let Some(after_sep) = rest.strip_prefix(['=', ',']) {
            let (value, left) = read_param_value(after_sep)?;
            values.push(value);
            rest = left;
        }
        property.params.push(Param { name: param_name.to_ascii_uppercase(), values });
    }
    property.value = rest.strip_prefix(':')?.to_owned();
    Some(property)
}

/// One parameter value at the start of `text` and what follows it, or `None` for an unclosed quote.
fn read_param_value(text: &str) -> Option<(String, &str)> {
    if let Some(quoted) = text.strip_prefix('"') {
        let end = quoted.find('"')?;
        return Some((uncaret(quoted.get(..end)?), quoted.get(end.saturating_add(1)..)?));
    }
    let end = text.find([';', ':', ',']).unwrap_or(text.len());
    Some((uncaret(text.get(..end)?), text.get(end..)?))
}

/// `text` with the RFC 6868 escapes undone: `^n` a line break, `^'` a double quote, `^^` a caret.
fn uncaret(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '^' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('\'') => out.push('"'),
            Some('^') => out.push('^'),
            Some(other) => {
                out.push('^');
                out.push(other);
            }
            None => out.push('^'),
        }
    }
    out
}

/// Writes one parameter value: RFC 6868 escapes, quoted when it holds `;`, `:` or `,`, control characters other than
/// the line break (escaped) and the tab left out.
fn push_param_value(line: &mut String, value: &str) {
    let quote = value.contains([';', ':', ',']);
    if quote {
        line.push('"');
    }
    for c in value.chars() {
        match c {
            '^' => line.push_str("^^"),
            '"' => line.push_str("^'"),
            '\n' => line.push_str("^n"),
            '\t' => line.push('\t'),
            c if c.is_control() => {}
            c => line.push(c),
        }
    }
    if quote {
        line.push('"');
    }
}

/// Whether `name` is an `iana-token` or `x-name`: letters, digits and hyphens, at least one.
fn is_name(name: &str) -> bool {
    !name.is_empty() && name.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

fn name_checked(name: &str) -> Result<&str, IcalError> {
    if is_name(name) { Ok(name) } else { Err(IcalError::bad_name(name)) }
}

/// Appends `line` folded at [`FOLD_AT`] octets, never inside a character, with CRLF after each piece.
fn push_folded(out: &mut String, line: &str) {
    let mut room = FOLD_AT;
    for c in line.chars() {
        if c.len_utf8() > room {
            out.push_str("\r\n ");
            room = FOLD_AT.saturating_sub(1);
        }
        out.push(c);
        room = room.saturating_sub(c.len_utf8());
    }
    out.push_str("\r\n");
}

#[cfg(test)]
#[path = "content_tests.rs"]
mod tests;
