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
/// are for. One is about this file, and the rest of the drop carries on without
/// it. In the order this function meets them: its name is not an Entry Path; the
/// Library holds it inside a Pack; its name carries a component coffret keeps
/// for itself, refused by name before any disk is reached (spec: EP-11, EP-14);
/// or the way down from its mapped root passes through something that is not a
/// real folder of it, refused where the descent meets it (spec: EP-4, EP-11).
/// The whole set is enumerated once, in [the module's own](super) account of
/// what is refused before anything lands.
///
/// A failure is none of those — nothing about this file was decided — and it
/// reaches the caller the same way: a folder above it that could not be made, a
/// scratch name that could not be created, a catalog that would not answer.
/// [`Refusal`]'s own conversion makes each of them about this one file, so the
/// drop carries on without it rather than stopping at it.
///
/// The other kind is about the request: it has outrun a budget, this device has
/// not the room for what is still coming, or the part's mapped root is not the
/// root the mapping was recorded against, refused as that root is opened
/// (spec: EP-13). What the three have in common is the whole of why they reach
/// that far: none of them is truer of the next part than of this one — the
/// budget has already been passed, the disk is no roomier for the part behind
/// this one, and the root is the one every part of a drop onto a folder goes
/// through. Only a drop onto the Library root carries parts under mappings of
/// their own (spec: EP-9), and it stops at the first of those roots that is
/// refused: the request fails as a whole the way a declined placement fails a
/// single writer's (spec: EP-11).
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
