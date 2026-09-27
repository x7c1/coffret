use serde::Serialize;

use crate::refresh::Standing;
use crate::routes::RefusalDto;

/// How far this device has got with the Library, and what stopped it where
/// something did.
///
/// Two fields and not one word, because a person is owed the reason as well as
/// the state: "this device has not caught up" is something to wait through, and
/// "Storage did not answer" is something to press a button about.
#[derive(Serialize)]
pub(super) struct CatalogDto {
    /// `catching_up`, `caught_up` or `behind`.
    state: &'static str,
    /// What stopped the last catch-up, and `null` where nothing did.
    stopped: Option<RefusalDto>,
}

impl CatalogDto {
    pub(super) fn of(standing: &Standing) -> Self {
        match standing {
            Standing::CatchingUp => Self {
                state: "catching_up",
                stopped: None,
            },
            Standing::CaughtUp => Self {
                state: "caught_up",
                stopped: None,
            },
            Standing::Behind(stopped) => Self {
                state: "behind",
                stopped: Some(RefusalDto::of(stopped)),
            },
        }
    }
}
