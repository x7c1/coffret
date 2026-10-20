//! The Node-API addon a desktop shell written for Node loads to run the
//! server inside its own process: a thin binding over
//! [`coffret_desktop_core`], which every decision here is.
//!
//! Every export is `#[napi(catch_unwind)]`, so that a panic becomes a
//! JavaScript exception in the shell rather than an abort of the process it
//! runs in: a panic never crosses the boundary. None takes a Passphrase. The
//! Passphrase reaches the core through the door alone, from the window the
//! shell loads [`open_door`]'s URL into, so it never passes through the
//! shell's own process (spec: DK-10, LA-22).
//!
//! No export returns a future, so napi-rs runs no runtime of its own: the one
//! runtime is the core's, started by [`start`]. The core calls back into the
//! shell through a threadsafe function ([`listen`]).

mod desktop_event;
pub use desktop_event::DesktopEvent;

mod door_mode;
pub use door_mode::DoorMode;

mod served_library;
pub use served_library::ServedLibrary;

use std::ffi::OsStr;

use coffret_desktop_core::{self as core, Desktop};
use napi::bindgen_prelude::*;
use napi::threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode};
use napi::JsString;
use napi_derive::napi;

/// The callback the shell registers for [`DesktopEvent`]s: called with the
/// event alone, and weak, so that it holds the Node process open no longer
/// than the shell's own handles do.
type EventCallback = ThreadsafeFunction<DesktopEvent, (), DesktopEvent, Status, false, true>;

/// Starts the core: the log, the one runtime, and the Passphrase door.
///
/// `identifier` is the app's, which says whether this is the installed app or
/// the development one; `idle_minutes` is what `COFFRET_IDLE_MINUTES` was set
/// to, if anything, which is refused where it is not a whole number of at
/// least one. Refused once the core has started.
#[napi(catch_unwind)]
pub fn start(identifier: String, idle_minutes: Option<String>) -> Result<()> {
    core::start(&identifier, idle_minutes.as_deref().map(OsStr::new))
        .map(|_| ())
        .map_err(failed)
}

/// The names of the Libraries on this device.
#[napi(catch_unwind)]
pub fn libraries() -> Result<Vec<String>> {
    core::libraries().map_err(failed)
}

/// A fresh token for one Passphrase window opened in `mode`, as the URL to
/// load that window from. The token is in the URL's fragment, which the page
/// takes out of the address as it loads; the URL is for the window and for
/// nothing else, a log included.
#[napi(catch_unwind)]
pub fn open_door(env: &Env, mode: DoorMode) -> Result<JsString<'_>> {
    let url = desktop()?.open_door(mode.into()).map_err(failed)?;
    env.create_string(url.expose())
}

/// The Library served, if one is open and its server has not stopped.
#[napi(catch_unwind)]
pub fn served() -> Result<Option<ServedLibrary>> {
    Ok(desktop()?.served().map(|served| ServedLibrary {
        name: served.name,
        unlocked: served.unlocked,
    }))
}

/// A one-time launch ticket for the explorer, to hand the explorer's page in
/// its fragment as `#launch=<ticket>`. Good for one exchange within a minute.
#[napi(catch_unwind)]
pub fn issue_ticket(env: &Env) -> Result<JsString<'_>> {
    let ticket = desktop()?.issue_ticket().map_err(failed)?;
    env.create_string(&ticket)
}

/// Where the explorer is served, as `127.0.0.1:<port>`, if a Library is open.
#[napi(catch_unwind)]
pub fn explorer_address() -> Result<Option<String>> {
    Ok(desktop()?
        .explorer_address()
        .map(|address| address.to_string()))
}

/// Hands every [`DesktopEvent`] from now on to `callback`, in place of any
/// callback before it.
#[napi(catch_unwind)]
pub fn listen(callback: EventCallback) -> Result<()> {
    desktop()?.listen(move |event| {
        let status = callback.call(event.into(), ThreadsafeFunctionCallMode::NonBlocking);
        if status != Status::Ok {
            tracing::warn!(
                kind = "desktop_event_undelivered",
                status = %status,
                "an event could not be handed to the shell",
            );
        }
    });
    Ok(())
}

/// Discards every browser session, launch ticket and door token: what the
/// shell does as it quits.
#[napi(catch_unwind)]
pub fn retire() -> Result<()> {
    desktop()?.retire();
    Ok(())
}

/// This process's core, once [`start`] has started it.
fn desktop() -> Result<&'static Desktop> {
    core::started().ok_or_else(|| Error::from_reason("the desktop has not started"))
}

/// A refusal, as the exception the shell catches: the whole chain, which is
/// the sentence a person is shown where the shell shows one.
fn failed(error: anyhow::Error) -> Error {
    Error::from_reason(format!("{error:#}"))
}
