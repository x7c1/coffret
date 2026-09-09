use std::io;
use std::path::Path;

use coffret_usecase::device_state::RootIdentity;
use coffret_usecase::{LocalIoError, LocalOperation, RootProbe};
use tokio::fs;
use tracing::debug;

pub(super) async fn probe_root(root: &Path) -> Result<Option<RootProbe>, LocalIoError> {
    let metadata = match fs::metadata(root).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            debug!("a mapped root was not there when it was stated");
            return Ok(None);
        }
        Err(cause) => return Err(LocalIoError::new(LocalOperation::Stating, root, cause)),
    };
    Ok(Some(RootProbe {
        identity: identity_of(&metadata),
    }))
}

#[cfg(unix)]
fn identity_of(metadata: &std::fs::Metadata) -> Option<RootIdentity> {
    use std::os::unix::fs::MetadataExt;

    Some(RootIdentity::new(format!("unix-dev:{}", metadata.dev())))
}

#[cfg(not(unix))]
fn identity_of(_metadata: &std::fs::Metadata) -> Option<RootIdentity> {
    None
}
