use serde::Serialize;

use crate::routes::activity::RefusalDto;

/// One part that was not written, named the way the caller named it.
///
/// By the name off the part and not by an Entry Path, because the commonest
/// refusal here is that the name is not one: a part carrying `../etc/passwd` has
/// no Entry Path to be reported under, and answering with one this server had
/// repaired would be answering about a file nobody sent.
#[derive(Serialize)]
pub(super) struct RefusedDto {
    pub(super) name: String,
    #[serde(flatten)]
    pub(super) refusal: RefusalDto,
}
