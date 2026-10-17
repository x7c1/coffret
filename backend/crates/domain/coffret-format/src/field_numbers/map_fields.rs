use super::registered_field::RegisteredField;

/// The fields of one map kind, with the field numbers the registry assigns
/// them.
#[derive(Debug)]
pub struct MapFields {
    pub(super) map: &'static str,
    pub(super) fields: &'static [RegisteredField],
}

impl MapFields {
    /// The map kind's name, as the registry's `map` column spells it.
    pub fn map(&self) -> &'static str {
        self.map
    }

    pub(super) fn live_by_name(&self, name: &str) -> Option<&RegisteredField> {
        self.fields
            .iter()
            .find(|field| !field.retired && field.name == name)
    }

    pub(super) fn live_by_number(&self, number: u64) -> Option<&RegisteredField> {
        self.fields
            .iter()
            .find(|field| !field.retired && field.number == number)
    }

    /// The field number `name` travels under in this map, if it is a live
    /// field of it.
    #[cfg(test)]
    pub(crate) fn number_of(&self, name: &str) -> Option<u64> {
        self.live_by_name(name).map(|field| field.number)
    }
}
