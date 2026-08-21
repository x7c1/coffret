use std::fmt;

use serde_json::Value;

/// One record, read back as the file holds it.
///
/// The sink writes JSONL, so a case can ask an event for a field by name
/// instead of searching a line for `field="value"`. That is the stronger
/// question of the two: a substring proves the characters are somewhere in the
/// line, and a field proves the event was emitted carrying it.
pub struct LoggedEvent {
    line: String,
    record: Value,
}

impl LoggedEvent {
    /// Reads one line of the log back.
    ///
    /// # Panics
    ///
    /// If the line is not one JSON object — which is the sink's contract, so a
    /// case discovering otherwise has found a real failure rather than a
    /// missing case of its own.
    pub(super) fn parse(line: &str) -> Self {
        let record = serde_json::from_str(line).unwrap_or_else(|error| {
            panic!("every event must be one JSON object, and this one is not: {error}\nin: {line}")
        });

        Self {
            line: line.to_owned(),
            record,
        }
    }

    /// The level the event was emitted at.
    pub fn level(&self) -> String {
        self.text_of(self.entry("level"))
    }

    /// What the event says, in words.
    pub fn message(&self) -> String {
        self.field("message")
    }

    /// One of the event's fields, as text.
    ///
    /// # Panics
    ///
    /// If the event carries no field by that name. A case naming a field that
    /// is not there is asking about instrumentation that no longer exists,
    /// which is a failure and not a `None`.
    pub fn field(&self, name: &str) -> String {
        self.text_of(self.value(name))
    }

    /// One of the event's fields, which has to have been recorded as a number.
    ///
    /// # Panics
    ///
    /// If the field is absent, or was recorded as anything but a number.
    pub fn number(&self, name: &str) -> i64 {
        self.value(name).as_i64().unwrap_or_else(|| {
            panic!(
                "the field {name:?} was not recorded as a number:\n{}",
                self.line
            )
        })
    }

    /// Everything the record says, with the JSON escaping undone.
    ///
    /// What a reader gets after `jq`, and so what a search for something that
    /// must never have been recorded has to cover: a value carrying a quote or
    /// a newline reaches the file escaped, and would hide from a search of the
    /// line itself.
    pub(super) fn plain(&self) -> String {
        let mut found = String::new();
        collect(&self.record, &mut found);
        found
    }

    /// One of the envelope's own keys.
    fn entry(&self, key: &str) -> &Value {
        self.record
            .get(key)
            .unwrap_or_else(|| panic!("every event must carry {key:?}:\n{}", self.line))
    }

    /// One of the event's fields.
    fn value(&self, name: &str) -> &Value {
        self.entry("fields")
            .get(name)
            .unwrap_or_else(|| panic!("no field {name:?} on this event:\n{}", self.line))
    }

    /// A value the way it was recorded: a string as its own text, anything else
    /// as the JSON it is.
    fn text_of(&self, value: &Value) -> String {
        match value {
            Value::String(text) => text.clone(),
            other => other.to_string(),
        }
    }
}

/// Appends every scalar in a record, however deeply nested, one per line.
fn collect(value: &Value, found: &mut String) {
    match value {
        Value::Object(entries) => {
            for (key, entry) in entries {
                found.push_str(key);
                found.push('\n');
                collect(entry, found);
            }
        }
        Value::Array(entries) => {
            for entry in entries {
                collect(entry, found);
            }
        }
        Value::String(text) => {
            found.push_str(text);
            found.push('\n');
        }
        other => {
            found.push_str(&other.to_string());
            found.push('\n');
        }
    }
}

impl fmt::Display for LoggedEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.line)
    }
}

impl fmt::Debug for LoggedEvent {
    /// The record itself, because a case that fails is asking what was emitted
    /// rather than how this type holds it.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.line)
    }
}
