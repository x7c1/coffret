use axum::extract::multipart::Field;
use coffret_device::{ContainerKind, EntryPath, OpenLibrary};

use crate::api_error::ApiError;
use crate::entry_query::shaped;
use crate::envelope::Envelope;

use super::landed::Landed;
use super::outran::outran;
use super::refusal::Refusal;
use super::room_for::room_for;
use super::under::under;

/// Takes one part into the folder, or says why it was not taken.
///
/// The order is the point: the name is shaped, the catalog is asked what stands
/// at the path, and only then is anything opened. Everything that can refuse this
/// file has refused it before the first byte is written, so a refusal never
/// leaves a partial file behind — and a failure part way through does not either,
/// because the bytes are going to a temporary name that is removed when the
/// incoming file is dropped (spec: EP-11).
///
/// Two kinds of refusal come out of it, which is what [`Refusal`]'s two variants
/// are for. One is about this file — its name is not an Entry Path, the Library
/// holds it inside a Pack, this device could not write it — and the rest of the
/// drop carries on without it. The other is about the request: it has outrun a
/// budget, or this device has not the room for what is still coming, and neither
/// of those is truer of the next part than of this one.
///
/// `coming` is how much room the caller is to be asked to have. It is what the
/// request said is left of it, so a book being dropped asks for the rest of the
/// book and not for one page at a time.
pub(super) async fn receive(
    library: &OpenLibrary,
    envelope: &Envelope,
    coming: u64,
    folder: Option<&EntryPath>,
    name: &str,
    mut part: Field<'_>,
) -> Result<Landed, Refusal> {
    let path = under(folder, &shaped(name)?);
    if library.container_of(&path).await? == Some(ContainerKind::Pack) {
        return Err(ApiError::pack_resident().into());
    }

    let mut incoming = library.receive_file(&path).await?;
    // Asked once the destination is open and before a byte of the part is
    // written, which is the one moment both halves of the question are settled:
    // the descent has arrived at the folder these bytes are going into, so what
    // is asked about is the volume they will land on rather than whatever a name
    // would have resolved to. A refusal here drops the incoming file, and
    // dropping it takes the empty scratch name with it (spec: EP-11).
    room_for(envelope, &incoming.scratch_path(), coming)?;

    while let Some(chunk) = part
        .chunk()
        .await
        .map_err(|cause| Refusal::Request(ApiError::multipart(cause)))?
    {
        // Met before the bytes are written rather than after, so the file that is
        // refused is one this device never finished taking.
        if incoming.written().saturating_add(chunk.len() as u64) > envelope.part_bytes {
            return Err(Refusal::Request(outran(
                "one file in it is over that on its own, so dropping fewer beside it \
                 changes nothing",
            )));
        }
        incoming.write(&chunk).await?;
    }
    let bytes = incoming.written();
    incoming.keep().await?;
    Ok(Landed { path, bytes })
}
