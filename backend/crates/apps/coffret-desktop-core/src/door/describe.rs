use std::sync::Arc;

use axum::extract::{Request, State};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

use super::fences::presented_token;
use super::{Door, DoorMode, Refusal};
use crate::{libraries, ListedLibrary};

/// What a window is shown: which mode it was opened in, and the Libraries it
/// may choose between or the one it unlocks.
#[derive(Serialize)]
#[serde(tag = "mode", rename_all = "lowercase")]
enum Described {
    Open { libraries: Vec<ListedLibrary> },
    Unlock { library: Option<String> },
}

/// `GET /window`: what the window that presents its token is opened to do
/// (spec: LA-22).
///
/// Behind the token, so that what is on this device is told only to a window
/// the shell opened. Looking counts as no attempt.
pub(super) async fn describe(State(door): State<Arc<Door>>, request: Request) -> Response {
    let headers = request.headers();
    if let Err(fence) = door.admits_look(headers) {
        return Refusal::OriginRefused.answer(fence, false);
    }
    let Some(mode) = presented_token(headers).and_then(|token| door.shared.windows.admit(token))
    else {
        return Refusal::AuthenticationRequired.answer("token", true);
    };
    let described = match mode {
        DoorMode::Open => match libraries() {
            Ok(libraries) => Described::Open { libraries },
            Err(error) => {
                tracing::warn!(
                    error = %crate::refusal_evidence(&error),
                    "the Libraries on this device could not be listed for a Passphrase window",
                );
                return Refusal::LibrariesUnread.answer("libraries", false);
            }
        },
        DoorMode::Unlock => Described::Unlock {
            library: door.shared.served().map(|opened| opened.state.name.clone()),
        },
    };
    Json(described).into_response()
}
