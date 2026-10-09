/// Why a kept parcel turned out not to be held.
///
/// No `PartialEq`: the decoder's refusal travels as the value it reported, and
/// a caller tells the two apart by the variant.
#[derive(Debug, Clone)]
pub enum UnheldReason {
    /// No file was at the path the record names.
    Missing,
    /// The file was there and its chunks did not authenticate as that parcel,
    /// or it was short or long (spec: FM-5).
    Unauthenticated {
        /// What the chunk decoder refused the file with — a chunk that failed
        /// authentication, or a run that overran or fell short of the parcel.
        cause: coffret_format::Error,
    },
}
