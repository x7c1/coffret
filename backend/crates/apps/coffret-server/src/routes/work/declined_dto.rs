use serde::Serialize;

use crate::fill::Declined;
use crate::routes::RefusalDto;

#[derive(Serialize)]
pub(super) struct DeclinedDto {
    path: String,
    #[serde(flatten)]
    refusal: RefusalDto,
}

impl DeclinedDto {
    pub(super) fn of(declined: &Declined) -> Self {
        Self {
            path: declined.path.clone(),
            refusal: RefusalDto::of(&declined.refusal),
        }
    }
}
