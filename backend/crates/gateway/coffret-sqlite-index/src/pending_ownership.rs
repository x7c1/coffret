use std::fs::{OpenOptions, TryLockError};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use coffret_usecase::{IndexError, IndexResult, PendingRowsGuard};

/// A separate file coordinates all connections without holding a SQLite
/// transaction open across network IO. It stays in place after unlock so a
/// second opener cannot lock a different inode at the same pathname.
pub(crate) fn take(path: &Path) -> IndexResult<PendingRowsGuard> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(path)
        .map_err(|cause| IndexError::Backend {
            operation: "opening the pending-row ownership lock",
            cause: Box::new(cause),
        })?;
    file.try_lock().map_err(|cause| match cause {
        TryLockError::WouldBlock => IndexError::PendingRowsBusy {
            cause: Box::new(cause),
        },
        TryLockError::Error(cause) => IndexError::Backend {
            operation: "locking pending rows",
            cause: Box::new(cause),
        },
    })?;
    Ok(PendingRowsGuard::holding(file))
}

/// Resolve aliases before choosing the sidecar, and retain the whole filename
/// so distinct Index files with the same stem do not share ownership.
pub(crate) fn path_for(path: &Path) -> IndexResult<PathBuf> {
    let path = path.canonicalize().map_err(|cause| IndexError::Backend {
        operation: "resolving the pending-row ownership path",
        cause: Box::new(cause),
    })?;
    let mut name = path.into_os_string();
    name.push(".pending.lock");
    Ok(name.into())
}
