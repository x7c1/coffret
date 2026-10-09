use coffret_usecase::fetch::KeptParcels;

use crate::open_library::OpenLibrary;

impl OpenLibrary {
    /// Where this Library's fetches keep the parcels they read, and how long
    /// one is (spec: PK-19, PK-21).
    pub fn kept_parcels(&self) -> KeptParcels<'_> {
        KeptParcels::new(self.local_fs.as_ref(), &self.parcel_dir).with_len(self.parcel_len)
    }
}
