// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Events (`VEVENT`, RFC 5545 section 3.6.1), their people and their alarms, read into fields and written back.

use inboxcraft_time::Timestamp;

use crate::{Component, IcalError, Param, Property, Rule, Span, When, escape_text, unescape_text};

/// Whether an event takes place.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Status {
    /// Not sure yet.
    Tentative,
    /// It does.
    Confirmed,
    /// It does not.
    Cancelled,
    /// A value RFC 5545 does not name, as written.
    Other(String),
}

/// Who may see an event's details.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Class {
    /// Everyone.
    Public,
    /// Only the owner.
    Private,
    /// Only that it is busy.
    Confidential,
    /// A value RFC 5545 does not name, as written.
    Other(String),
}

/// An attendee's answer (`PARTSTAT`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum PartStat {
    /// No answer yet, also when the parameter is missing.
    #[default]
    NeedsAction,
    /// Coming.
    Accepted,
    /// Not coming.
    Declined,
    /// Maybe.
    Tentative,
    /// Sent someone else.
    Delegated,
    /// A value RFC 5545 does not name, as written.
    Other(String),
}

const STATUSES: [(&str, Status); 3] = [("TENTATIVE", Status::Tentative), ("CONFIRMED", Status::Confirmed), ("CANCELLED", Status::Cancelled)];
const CLASSES: [(&str, Class); 3] = [("PUBLIC", Class::Public), ("PRIVATE", Class::Private), ("CONFIDENTIAL", Class::Confidential)];
const PART_STATS: [(&str, PartStat); 5] = [
    ("NEEDS-ACTION", PartStat::NeedsAction),
    ("ACCEPTED", PartStat::Accepted),
    ("DECLINED", PartStat::Declined),
    ("TENTATIVE", PartStat::Tentative),
    ("DELEGATED", PartStat::Delegated),
];

/// The value `names` gives `value` (any case), or `other` of it.
fn named<T: Clone>(value: &str, names: &[(&str, T)], other: fn(String) -> T) -> T {
    names.iter().find(|(name, _)| name.eq_ignore_ascii_case(value.trim())).map_or_else(|| other(value.to_owned()), |(_, known)| known.clone())
}

impl Status {
    fn code(&self) -> &str {
        match self {
            Self::Other(value) => value,
            known => STATUSES.iter().find(|(_, value)| value == known).map_or("TENTATIVE", |(name, _)| name),
        }
    }
}

impl Class {
    fn code(&self) -> &str {
        match self {
            Self::Other(value) => value,
            known => CLASSES.iter().find(|(_, value)| value == known).map_or("PUBLIC", |(name, _)| name),
        }
    }
}

impl PartStat {
    fn code(&self) -> &str {
        match self {
            Self::Other(value) => value,
            known => PART_STATS.iter().find(|(_, value)| value == known).map_or("NEEDS-ACTION", |(name, _)| name),
        }
    }
}

/// The parameters of an organizer or attendee read into fields of their own.
const READ_PARAMS: [&str; 4] = ["CN", "ROLE", "PARTSTAT", "RSVP"];

/// An organizer or an attendee: an address (`mailto:` and the like, as written) and a name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Attendee {
    /// The calendar address, a URI.
    pub address: String,
    /// The name (`CN`).
    pub name: Option<String>,
    /// The role (`ROLE`), as written.
    pub role: Option<String>,
    /// The answer (`PARTSTAT`).
    pub status: PartStat,
    /// Whether an answer is asked for (`RSVP=TRUE`).
    pub rsvp: bool,
    /// The other parameters (`SCHEDULE-AGENT`, `CUTYPE` and the like), kept as written so they go back unchanged.
    pub params: Vec<Param>,
}

impl Attendee {
    /// `address`, nothing else known.
    #[must_use]
    pub fn new(address: &str) -> Self {
        Self { address: address.to_owned(), name: None, role: None, status: PartStat::NeedsAction, rsvp: false, params: Vec::new() }
    }

    /// The first value of the other parameter `name` (any case).
    #[must_use]
    pub fn param(&self, name: &str) -> Option<&str> {
        self.params.iter().find(|param| param.name().eq_ignore_ascii_case(name)).and_then(|param| param.values().first()).map(String::as_str)
    }

    /// Sets the other parameter `name` to `value`, in place of the one it had.
    pub fn set_param(&mut self, name: &str, value: &str) {
        let param = Param::new(name, vec![value.to_owned()]);
        match self.params.iter_mut().find(|held| held.name() == param.name()) {
            Some(held) => *held = param,
            None => self.params.push(param),
        }
    }

    fn read(property: &Property) -> Self {
        Self {
            address: property.value().to_owned(),
            name: property.param("CN").map(str::to_owned),
            role: property.param("ROLE").map(str::to_owned),
            status: property.param("PARTSTAT").map_or(PartStat::NeedsAction, |value| named(value, &PART_STATS, PartStat::Other)),
            rsvp: property.param("RSVP").is_some_and(|value| value.eq_ignore_ascii_case("TRUE")),
            params: property.params().iter().filter(|param| !READ_PARAMS.contains(&param.name())).cloned().collect(),
        }
    }

    fn write(&self, name: &str) -> Property {
        let mut property = Property::new(name, self.address.clone());
        if let Some(cn) = &self.name {
            property = property.with_param("CN", cn.clone());
        }
        if let Some(role) = &self.role {
            property = property.with_param("ROLE", role.clone());
        }
        if self.status != PartStat::NeedsAction {
            property = property.with_param("PARTSTAT", self.status.code());
        }
        if self.rsvp {
            property = property.with_param("RSVP", "TRUE");
        }
        for param in &self.params {
            property = property.with_param_values(param.name(), param.values().to_vec());
        }
        property
    }
}

/// When an alarm goes off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Trigger {
    /// This long after the event's start (before it when negative).
    Start(Span),
    /// This long after the event's end (before it when negative).
    End(Span),
    /// At this moment.
    At(Timestamp),
}

/// A `VALARM`: what it does and when.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alarm {
    /// `DISPLAY`, `AUDIO`, `EMAIL` or another action, as written.
    pub action: String,
    /// When.
    pub trigger: Trigger,
    /// The text it shows.
    pub description: Option<String>,
    /// The properties it does not read, kept to be written back.
    pub other: Vec<Property>,
}

impl Alarm {
    /// A `DISPLAY` alarm at `trigger`.
    #[must_use]
    pub fn display(trigger: Trigger, description: &str) -> Self {
        Self { action: "DISPLAY".into(), trigger, description: Some(description.to_owned()), other: Vec::new() }
    }

    /// The alarm a `VALARM` component describes.
    ///
    /// # Errors
    ///
    /// [`IcalError::Missing`] without `ACTION` or `TRIGGER`, [`IcalError::BadValue`] when the trigger cannot be read.
    pub fn from_component(component: &Component) -> Result<Self, IcalError> {
        let missing = |property| IcalError::Missing { component: "VALARM", property };
        let action = component.property("ACTION").ok_or(missing("ACTION"))?.value().to_owned();
        let trigger = component.property("TRIGGER").ok_or(missing("TRIGGER"))?;
        let at_end = trigger.param("RELATED").is_some_and(|related| related.eq_ignore_ascii_case("END"));
        let trigger = if trigger.param("VALUE").is_some_and(|kind| kind.eq_ignore_ascii_case("DATE-TIME")) {
            match When::from_property(trigger)? {
                When::Utc(at) => Trigger::At(at),
                _ => return Err(IcalError::bad_value("TRIGGER")),
            }
        } else {
            let span = Span::parse(trigger.value()).map_err(|_| IcalError::bad_value("TRIGGER"))?;
            if at_end { Trigger::End(span) } else { Trigger::Start(span) }
        };
        let description = component.property("DESCRIPTION").map(|property| unescape_text(property.value()));
        let other =
            component.properties().iter().filter(|property| !matches!(property.name(), "ACTION" | "TRIGGER" | "DESCRIPTION")).cloned().collect();
        Ok(Self { action, trigger, description, other })
    }

    /// The `VALARM` component.
    #[must_use]
    pub fn to_component(&self) -> Component {
        let trigger = match self.trigger {
            Trigger::Start(span) => Property::new("TRIGGER", span.to_string()),
            Trigger::End(span) => Property::new("TRIGGER", span.to_string()).with_param("RELATED", "END"),
            Trigger::At(at) => When::Utc(at).to_property("TRIGGER").with_param("VALUE", "DATE-TIME"),
        };
        let mut component = Component::new("VALARM").with_property(Property::new("ACTION", self.action.clone())).with_property(trigger);
        if let Some(description) = &self.description {
            component.push_property(Property::new("DESCRIPTION", escape_text(description)));
        }
        self.other.iter().cloned().fold(component, Component::with_property)
    }
}

/// An event: one `VEVENT`, a single one, the master of a recurring one, or one changed instance of it
/// ([`Event::recurrence_id`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    /// The `UID`, the same for the master and its changed instances.
    pub uid: String,
    /// The `SEQUENCE`: how often the organizer changed it.
    pub sequence: u32,
    /// The `DTSTAMP`.
    pub stamp: Option<Timestamp>,
    /// The `DTSTART`; a [`When::Date`] makes it an all-day event.
    pub start: When,
    /// The `DTEND`.
    pub end: Option<When>,
    /// The `DURATION`, when there is no end.
    pub duration: Option<Span>,
    /// The `SUMMARY`, its text escapes undone.
    pub summary: Option<String>,
    /// The `DESCRIPTION`.
    pub description: Option<String>,
    /// The `LOCATION`.
    pub location: Option<String>,
    /// The `STATUS`.
    pub status: Option<Status>,
    /// `TRANSP:TRANSPARENT`: the time shows as free.
    pub free: bool,
    /// The `CLASS`.
    pub class: Option<Class>,
    /// The `RRULE`s.
    pub rules: Vec<Rule>,
    /// The `RDATE`s, one value each.
    pub dates: Vec<When>,
    /// The `EXDATE`s, one value each.
    pub exceptions: Vec<When>,
    /// The `RECURRENCE-ID`: the instance of the master this event changes.
    pub recurrence_id: Option<When>,
    /// The `ORGANIZER`.
    pub organizer: Option<Attendee>,
    /// The `ATTENDEE`s.
    pub attendees: Vec<Attendee>,
    /// The `VALARM`s that can be read.
    pub alarms: Vec<Alarm>,
    /// The properties it does not read, kept to be written back.
    pub other: Vec<Property>,
    /// The components inside it other than readable alarms, kept to be written back.
    pub components: Vec<Component>,
}

/// The properties [`Event`] reads into fields.
const READ: [&str; 18] = [
    "UID",
    "SEQUENCE",
    "DTSTAMP",
    "DTSTART",
    "DTEND",
    "DURATION",
    "SUMMARY",
    "DESCRIPTION",
    "LOCATION",
    "STATUS",
    "TRANSP",
    "CLASS",
    "RRULE",
    "RDATE",
    "EXDATE",
    "RECURRENCE-ID",
    "ORGANIZER",
    "ATTENDEE",
];

impl Event {
    /// An event `uid` at `start`, nothing else set.
    #[must_use]
    pub fn new(uid: &str, start: When) -> Self {
        Self {
            uid: uid.to_owned(),
            sequence: 0,
            stamp: None,
            start,
            end: None,
            duration: None,
            summary: None,
            description: None,
            location: None,
            status: None,
            free: false,
            class: None,
            rules: Vec::new(),
            dates: Vec::new(),
            exceptions: Vec::new(),
            recurrence_id: None,
            organizer: None,
            attendees: Vec::new(),
            alarms: Vec::new(),
            other: Vec::new(),
            components: Vec::new(),
        }
    }

    /// Whether it lasts whole days.
    #[must_use]
    pub fn all_day(&self) -> bool {
        self.start.is_date()
    }

    /// The event a `VEVENT` component describes.
    ///
    /// # Errors
    ///
    /// [`IcalError::Missing`] without `UID` or `DTSTART`; [`IcalError::BadValue`] when a property it reads cannot be
    /// read.
    pub fn from_component(component: &Component) -> Result<Self, IcalError> {
        let missing = |property| IcalError::Missing { component: "VEVENT", property };
        let uid = component.property("UID").ok_or(missing("UID"))?.value().to_owned();
        let start = When::from_property(component.property("DTSTART").ok_or(missing("DTSTART"))?)?;
        let mut event = Self::new(&uid, start);
        let when = |name| component.property(name).map(When::from_property).transpose();
        let text = |name| component.property(name).map(|property| unescape_text(property.value()));
        if let Some(sequence) = component.property("SEQUENCE") {
            event.sequence = sequence.value().trim().parse().map_err(|_| IcalError::bad_value("SEQUENCE"))?;
        }
        event.stamp = match when("DTSTAMP")? {
            Some(When::Utc(at)) => Some(at),
            Some(_) => return Err(IcalError::bad_value("DTSTAMP")),
            None => None,
        };
        event.end = when("DTEND")?;
        event.duration = component.property("DURATION").map(|property| Span::parse(property.value())).transpose()?;
        event.summary = text("SUMMARY");
        event.description = text("DESCRIPTION");
        event.location = text("LOCATION");
        event.status = component.property("STATUS").map(|property| named(property.value(), &STATUSES, Status::Other));
        event.free = component.property("TRANSP").is_some_and(|property| property.value().trim().eq_ignore_ascii_case("TRANSPARENT"));
        event.class = component.property("CLASS").map(|property| named(property.value(), &CLASSES, Class::Other));
        event.rules = component.properties_named("RRULE").map(Rule::from_property).collect::<Result<_, _>>()?;
        for property in component.properties_named("RDATE") {
            event.dates.extend(When::list_from_property(property)?);
        }
        for property in component.properties_named("EXDATE") {
            event.exceptions.extend(When::list_from_property(property)?);
        }
        event.recurrence_id = when("RECURRENCE-ID")?;
        event.organizer = component.property("ORGANIZER").map(Attendee::read);
        event.attendees = component.properties_named("ATTENDEE").map(Attendee::read).collect();
        event.other = component.properties().iter().filter(|property| !READ.contains(&property.name())).cloned().collect();
        for inner in component.components() {
            match (inner.name(), Alarm::from_component(inner)) {
                ("VALARM", Ok(alarm)) => event.alarms.push(alarm),
                _ => event.components.push(inner.clone()),
            }
        }
        Ok(event)
    }

    /// The `VEVENT` component: the end if there is one, else the duration.
    #[must_use]
    pub fn to_component(&self) -> Component {
        let mut component = Component::new("VEVENT").with_property(Property::new("UID", self.uid.clone()));
        let mut push = |property| component.push_property(property);
        if let Some(stamp) = self.stamp {
            push(When::Utc(stamp).to_property("DTSTAMP"));
        }
        push(self.start.to_property("DTSTART"));
        match (&self.end, self.duration) {
            (Some(end), _) => push(end.to_property("DTEND")),
            (None, Some(duration)) => push(Property::new("DURATION", duration.to_string())),
            (None, None) => {}
        }
        if self.sequence != 0 {
            push(Property::new("SEQUENCE", self.sequence.to_string()));
        }
        for (name, value) in [("SUMMARY", &self.summary), ("DESCRIPTION", &self.description), ("LOCATION", &self.location)] {
            if let Some(value) = value {
                push(Property::new(name, escape_text(value)));
            }
        }
        if let Some(status) = &self.status {
            push(Property::new("STATUS", status.code()));
        }
        if self.free {
            push(Property::new("TRANSP", "TRANSPARENT"));
        }
        if let Some(class) = &self.class {
            push(Property::new("CLASS", class.code()));
        }
        for rule in &self.rules {
            push(Property::new("RRULE", rule.to_string()));
        }
        for date in &self.dates {
            push(date.to_property("RDATE"));
        }
        for date in &self.exceptions {
            push(date.to_property("EXDATE"));
        }
        if let Some(id) = &self.recurrence_id {
            push(id.to_property("RECURRENCE-ID"));
        }
        if let Some(organizer) = &self.organizer {
            push(organizer.write("ORGANIZER"));
        }
        for attendee in &self.attendees {
            push(attendee.write("ATTENDEE"));
        }
        for property in &self.other {
            push(property.clone());
        }
        let component = self.alarms.iter().map(Alarm::to_component).fold(component, Component::with_component);
        self.components.iter().cloned().fold(component, Component::with_component)
    }
}

#[cfg(test)]
#[path = "event_tests.rs"]
mod tests;
