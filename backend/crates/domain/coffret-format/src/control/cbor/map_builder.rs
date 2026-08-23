use ciborium::Value;

/// One CBOR map under construction, in the order its rule states the fields.
///
/// The order is the encoder's own choice — a reader looks fields up by name —
/// but writing them in the rule's order keeps a payload dumped for inspection
/// readable against the rule it follows.
pub(in crate::control) struct MapBuilder(Vec<(Value, Value)>);

impl MapBuilder {
    pub(in crate::control) fn new() -> Self {
        Self(Vec::new())
    }

    pub(in crate::control) fn uint(&mut self, key: &str, value: u64) -> &mut Self {
        self.value(key, Value::from(value))
    }

    pub(in crate::control) fn optional_uint(&mut self, key: &str, value: Option<u64>) -> &mut Self {
        match value {
            Some(value) => self.uint(key, value),
            None => self,
        }
    }

    pub(in crate::control) fn text(&mut self, key: &str, value: &str) -> &mut Self {
        self.value(key, Value::Text(value.to_owned()))
    }

    pub(in crate::control) fn optional_text(
        &mut self,
        key: &str,
        value: Option<&str>,
    ) -> &mut Self {
        match value {
            Some(value) => self.text(key, value),
            None => self,
        }
    }

    pub(in crate::control) fn bytes(&mut self, key: &str, value: &[u8]) -> &mut Self {
        self.value(key, Value::Bytes(value.to_vec()))
    }

    pub(in crate::control) fn value(&mut self, key: &str, value: Value) -> &mut Self {
        self.0.push((Value::Text(key.to_owned()), value));
        self
    }

    pub(in crate::control) fn build(&mut self) -> Value {
        Value::Map(std::mem::take(&mut self.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optional_fields_are_left_out_rather_than_written_empty() {
        let value = MapBuilder::new()
            .optional_uint("prev", None)
            .optional_text("next_commit_slot", None)
            .uint("schema", 1)
            .build();
        let Value::Map(entries) = &value else {
            panic!("the builder makes a map");
        };
        assert_eq!(entries.len(), 1);
    }
}
