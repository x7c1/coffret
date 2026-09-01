use crate::reported::Reported;

/// One Entry a fill did not bring over, and why.
///
/// About that one Entry rather than about the fill (spec: EP-11): a file this
/// device did not place standing where the Entry belongs, a Container the
/// Library records no key for, an Entry that has since gone. Some of them are
/// findings the fetch surfaced and some are refusals no finding stands behind;
/// what makes them one list is that the fill records each and goes on — every
/// other Entry of the folder is a separate question — and the browser marks the
/// row with what the file route would have said, without anyone having to click
/// it to find out.
#[derive(Clone, Debug)]
pub struct Declined {
    /// The Entry this is about.
    pub path: String,
    /// The refusal, in the shape every refusal on these routes takes.
    pub refusal: Reported,
}
