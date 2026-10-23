mod explorer_address;
mod issue_ticket;
mod listen;
mod open_door;
mod retire;
mod served;
mod take_drop;

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use tokio::runtime::Runtime;

use crate::door::Door;
use crate::page_source::page_source;
use crate::shared::Shared;
use crate::{AppBuild, Error, IoStep, Result};

/// What a desktop shell holds for the whole of its process: the runtime the
/// server, the explorer's host and the door run on, and what they serve.
///
/// The shell drives it through a handful of calls, none of which takes a
/// Passphrase: the Passphrase reaches the core through the door alone, from
/// the window the shell loads the door's URL into ([`open_door`]).
///
/// [`open_door`]: Self::open_door
pub struct Desktop {
    shared: Arc<Shared>,
    /// Where the door is bound.
    door: SocketAddr,
    /// What every task runs on, and dropping it, last, ends them with what
    /// they serve. A call that has to wait for work of its own — the walk of a
    /// drop — waits on it here.
    runtime: Runtime,
}

impl Desktop {
    /// A desktop with nothing open yet, for the app called `identifier`,
    /// whose Library locks after `idle_minutes` in which nobody wants it
    /// (spec: DK-4). The door is bound and answering by the time this returns.
    ///
    /// A process holds one ([`crate::start`]); a case may hold several.
    pub fn new(identifier: &str, idle_minutes: u64) -> Result<Self> {
        let runtime = Runtime::new().map_err(|cause| Error::Io {
            step: IoStep::Runtime,
            cause,
        })?;
        Self::on(runtime, identifier, idle_minutes)
    }

    /// A desktop whose every task runs on the thread that drives its runtime,
    /// for a case that reads back what that thread logged.
    #[cfg(test)]
    pub(crate) fn on_current_thread(identifier: &str, idle_minutes: u64) -> Result<Self> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|cause| Error::Io {
                step: IoStep::Runtime,
                cause,
            })?;
        Self::on(runtime, identifier, idle_minutes)
    }

    /// A desktop running on `runtime`.
    fn on(runtime: Runtime, identifier: &str, idle_minutes: u64) -> Result<Self> {
        let shared = Arc::new(Shared::new(
            AppBuild::of(identifier).host_kind(),
            page_source(),
            idle_minutes,
        ));

        // Loopback, and any free port, as the explorer's host is: the shell
        // learns the number from the URL it is handed, and nothing else needs
        // it (spec: LA-13).
        let failed = |step| move |cause| Error::Io { step, cause };
        let listener = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .map_err(failed(IoStep::DoorBind))?;
        let door = listener.local_addr().map_err(failed(IoStep::DoorAddress))?;
        listener
            .set_nonblocking(true)
            .map_err(failed(IoStep::DoorNonBlocking))?;
        let listener = {
            let _on_runtime = runtime.enter();
            tokio::net::TcpListener::from_std(listener).map_err(failed(IoStep::DoorHandOver))?
        };
        let router = Door::new(door, Arc::clone(&shared))?.router();
        runtime.spawn(coffret_explorer_host::serve(listener, router));
        Ok(Self {
            shared,
            door,
            runtime,
        })
    }

    /// The runtime everything here runs on, for a case to drive what it
    /// serves on.
    #[cfg(test)]
    pub(crate) fn runtime(&self) -> &Runtime {
        &self.runtime
    }

    /// What the door's routes are given, for a case to build a router of its
    /// own over.
    #[cfg(test)]
    pub(crate) fn shared(&self) -> &Arc<Shared> {
        &self.shared
    }

    /// Where the door is bound.
    #[cfg(test)]
    pub(crate) fn door(&self) -> SocketAddr {
        self.door
    }
}
