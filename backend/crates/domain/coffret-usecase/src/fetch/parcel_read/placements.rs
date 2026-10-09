use std::ops::Range;

use coffret_model::Redacted;
use tracing::{debug, warn};

use crate::fetch::fetch_error::{FetchError, FetchResult};
use crate::fetch::parcel_read::{Member, ParcelRead};
use crate::fetch::placement::{discard_all, Created, Placement};

impl ParcelRead<'_> {
    /// The placements for the members lying in one stretch of parcels.
    ///
    /// The Entry asked for refuses the read if its mapped root will not vouch
    /// for itself (spec: EP-13); a companion whose place cannot be opened is
    /// left out and said, since nobody asked for it.
    pub(super) async fn placements<'m>(
        &self,
        members: &'m [Member],
        stretch: &Range<u64>,
    ) -> FetchResult<Vec<Placement<'m>>> {
        let mut placements = Vec::new();
        for (position, member) in members.iter().enumerate() {
            if !(stretch.start <= member.parcels.start && member.parcels.end <= stretch.end) {
                continue;
            }
            let asked = position == 0;
            match Placement::create(
                self.reading.destinations,
                &member.target,
                member.entry.clone(),
            )
            .await
            {
                Ok(Created::Ready(placement)) => placements.push(*placement),
                Ok(Created::RootRefused(root)) if asked => {
                    discard_all(placements);
                    return Err(FetchError::RefusedRoot(root));
                }
                Err(error) if asked => {
                    discard_all(placements);
                    return Err(error);
                }
                Ok(Created::RootRefused(_)) => debug!(
                    "an Entry that would have come with the one asked for is under a refused root",
                ),
                Err(error) => warn!(
                    error = %error.redacted(),
                    "an Entry that would have come with the one asked for has no place open",
                ),
            }
        }
        Ok(placements)
    }
}
