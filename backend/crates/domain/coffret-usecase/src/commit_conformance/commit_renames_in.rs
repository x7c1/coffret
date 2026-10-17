use coffret_format::{
    encode_control_object, encode_journal_record, ControlEncodeRequest, PurposeKey,
};
use coffret_model::{ControlObjectKind, ControlObjectName, EntryRename, JournalRecord};

use crate::byte_stream::ByteStream;
use crate::control_head::ControlHead;
use crate::index::Index;
use crate::object_store::ObjectStore;

/// Commits a record renaming `renames` on top of the head `index` stands at,
/// the way another device's rename would land: into the slot that head
/// carries, with the successor's two slots reserved afresh (spec: CP-2, CK-10,
/// FM-15).
///
/// The suites whose cases need another device's rename write the record
/// themselves, sealed under `journal_key` (spec: KD-4), rather than run a
/// rename on a second catalog: what a case arranges is the record, not the
/// run that produced it.
/// The renames go in the order a record holds them, whatever order they come
/// in (spec: FM-15).
/// `index` is only asked where the head stands; it catches the record up the
/// way any device does, on its next run.
pub(crate) async fn commit_renames_in(
    store: &dyn ObjectStore,
    index: &dyn Index,
    journal_key: &PurposeKey,
    mut renames: Vec<EntryRename>,
) {
    renames.sort_by_key(EntryRename::order_key);
    let standing = index
        .checkpoint()
        .await
        .expect("reading the checkpoint must succeed")
        .expect("an earlier run left the catalog at a head");
    let commit_slot = ControlHead::at(standing.head_generation())
        .commit_slot(standing.next_commit_slot())
        .expect("the head has a successor");
    let generation = standing
        .head_generation()
        .next()
        .expect("the head has a successor");
    let head = ControlHead::at(generation);
    let next_commit_slot = head
        .reserve_commit_slot(store)
        .await
        .expect("reserving the successor's slot must succeed");
    let snapshot_slot = head
        .reserve_snapshot_slot(store)
        .await
        .expect("reserving the checkpoint's slot must succeed");
    let record = JournalRecord::new(
        generation,
        Some(standing.head_generation()),
        standing.master_key_epoch(),
        standing.keyring().clone(),
        next_commit_slot.as_provider_id().map(str::to_owned),
        snapshot_slot.as_provider_id().map(str::to_owned),
        Vec::new(),
        Vec::new(),
    )
    .and_then(|record| record.with_renames(renames))
    .expect("a record renaming Entries is one a commit could write");
    let name = ControlObjectName::head(generation);
    let object = encode_control_object(&ControlEncodeRequest::new(
        &name,
        ControlObjectKind::Journal,
        journal_key,
        &encode_journal_record(&record).expect("a renaming record encodes"),
    ))
    .expect("sealing a Journal record must succeed");
    store
        .put_if_absent(&commit_slot, ByteStream::from(object.bytes().to_vec()))
        .await
        .expect("the renaming record takes the head's slot");
}
