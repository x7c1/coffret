use super::map_fields::MapFields;

/// One row of the field-number registry: a field of a map kind, and the field
/// number it travels under.
#[derive(Debug)]
pub(crate) struct RegisteredField {
    pub(super) name: &'static str,
    pub(super) number: u64,
    /// A retired field number is never read or written again, and never
    /// reassigned.
    pub(super) retired: bool,
    /// The map kind this field's value is — or the kind of every map in the
    /// array this field's value is.
    pub(super) holds: Option<&'static MapFields>,
}

pub(super) const fn field(name: &'static str, number: u64) -> RegisteredField {
    RegisteredField {
        name,
        number,
        retired: false,
        holds: None,
    }
}

pub(super) const fn holding(
    name: &'static str,
    number: u64,
    holds: &'static MapFields,
) -> RegisteredField {
    RegisteredField {
        name,
        number,
        retired: false,
        holds: Some(holds),
    }
}
