use crate::device_state::RootIdentity;

/// What stating one mapped root turned up (spec: EP-12).
///
/// A value rather than a bare [`RootIdentity`] option, because the two answers
/// it stands between are not the same question. Whether the root is *there* at
/// all is the [`Option`] the probe itself comes back in; this is what the
/// platform could say about the root it found, and `None` inside it means the
/// platform can say nothing rather than that the root is missing. A mapping on
/// such a platform is guarded by the root's existence alone.
///
/// Nothing about what the root *holds* is here. That question is asked only
/// where the identity has already moved, and it is asked by listing the folder —
/// so the answer is the listing's rather than a third field nothing usually
/// fills in.
#[derive(Debug, Clone)]
pub struct RootProbe {
    /// The filesystem the root stands on, as this device spells it, or `None`
    /// where the platform reports nothing it can be spelled from.
    pub identity: Option<RootIdentity>,
}
