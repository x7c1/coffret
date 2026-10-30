use std::collections::BTreeMap;

use coffret_model::EntryPath;

use crate::departure::{self, MappedFile};
use crate::device_state::LocalEntryState;
use crate::index_error::IndexResult;

use super::look::{Leaving, Look, Looked};
use super::{agrees, disk, Pending, Relocation, RelocationRefused, Run};

/// What deciding each record leaves to do on disk.
pub(super) struct Decided {
    /// The files to relocate, by where each stands.
    pub(super) pending: BTreeMap<EntryPath, Pending>,
    /// The records whose file is gone, to follow their Entry or be forgotten.
    pub(super) gone: Vec<Looked>,
}

impl Run<'_> {
    /// Decides each record: refuses what cannot be relocated, discards what a
    /// copy already placed makes redundant, and leaves the rest to relocate.
    pub(super) async fn decide(&mut self, looked: Vec<Look>) -> IndexResult<Decided> {
        let mut pending: BTreeMap<EntryPath, Pending> = BTreeMap::new();
        let mut gone: Vec<Looked> = Vec::new();
        for look in looked {
            let one = match look {
                Look::Looked(one) => *one,
                Look::Leaving(one) => {
                    self.decide_leaving(*one).await?;
                    continue;
                }
                // A record with no file has nothing to report about.
                Look::Refused { local, .. } if local.state == LocalEntryState::Absent => continue,
                Look::Refused { local, reason } => {
                    self.refuse(&local, reason);
                    continue;
                }
            };
            let source = match &one.source {
                MappedFile::Fenced { stopped_at } => {
                    let reason = RelocationRefused::SymbolicLink {
                        stopped_at: stopped_at.clone(),
                    };
                    self.refuse(&one.local, reason);
                    continue;
                }
                MappedFile::Nothing | MappedFile::Folder => {
                    gone.push(one);
                    continue;
                }
                MappedFile::File(source) => source.clone(),
            };
            match departure::unedited(self.disk.roots, &source, &one.local).await {
                Ok(true) => {}
                Ok(false) => {
                    self.refuse(&one.local, RelocationRefused::Edited);
                    continue;
                }
                Err(refused_io) => {
                    self.refuse(&one.local, disk(refused_io.into()));
                    continue;
                }
            }
            match &one.target {
                MappedFile::Nothing => {
                    pending.insert(
                        one.from().clone(),
                        Pending {
                            looked: one,
                            source,
                        },
                    );
                }
                MappedFile::Fenced { stopped_at } => {
                    let reason = RelocationRefused::SymbolicLink {
                        stopped_at: stopped_at.clone(),
                    };
                    self.refuse(&one.local, reason);
                }
                MappedFile::Folder => self.refuse(&one.local, RelocationRefused::DestinationTaken),
                MappedFile::File(there) => {
                    let standing = self.index.local_entry_at(one.to()).await?;
                    match standing {
                        // This device's own copy of the same Entry, as placed:
                        // the waiting one is redundant (spec: EP-15).
                        Some(placed)
                            if placed.state == LocalEntryState::Present
                                && !placed.awaits_relocation()
                                && placed.observation.entry == one.local.observation.entry
                                && agrees(&placed, there) =>
                        {
                            let done = Relocation::Discarded {
                                from: one.from().clone(),
                                to: one.to().clone(),
                            };
                            self.discard(&one.local, &one.mapping, &source, done)
                                .await?;
                        }
                        // Another file of this device's, waiting to be relocated
                        // itself: this one is relocated once it has.
                        Some(waiting)
                            if waiting.state == LocalEntryState::Present
                                && waiting.awaits_relocation() =>
                        {
                            pending.insert(
                                one.from().clone(),
                                Pending {
                                    looked: one,
                                    source,
                                },
                            );
                        }
                        _ => self.refuse(&one.local, RelocationRefused::DestinationTaken),
                    }
                }
            }
        }
        Ok(Decided { pending, gone })
    }

    /// Decides a record whose file cannot follow its Entry on this device: an
    /// unedited file is discarded into the trash, an edited one is kept where it
    /// stands and refused for its edit, and a record with no file standing for
    /// it is forgotten, since the Entry's current path holds nothing of this
    /// device's (spec: EP-10, EP-15).
    async fn decide_leaving(&mut self, one: Leaving) -> IndexResult<()> {
        let Leaving {
            local,
            mapping,
            source,
        } = one;
        let source = match source {
            MappedFile::File(source) => source,
            MappedFile::Fenced { stopped_at } => {
                self.refuse(&local, RelocationRefused::SymbolicLink { stopped_at });
                return Ok(());
            }
            MappedFile::Nothing | MappedFile::Folder => {
                return self.index.forget_local_entry(&local.observation.path).await;
            }
        };
        match departure::unedited(self.disk.roots, &source, &local).await {
            Ok(true) => self.leave(&local, &mapping, &source).await,
            Ok(false) => {
                self.refuse(&local, RelocationRefused::Edited);
                Ok(())
            }
            Err(refused_io) => {
                self.refuse(&local, disk(refused_io.into()));
                Ok(())
            }
        }
    }
}
