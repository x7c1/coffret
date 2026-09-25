/// What a join could tell about the Library at the place it was given.
///
/// Joining is the one flow that takes somebody's word for where a Library is,
/// and both providers are asked the same question about that word: does the
/// place hold what a Library keeps at the top of its own place? It is not the
/// question of identity, which the two answer very differently — on Drive the
/// app folder's name *is* the Library ID (spec: FM-18), so a folder that is not
/// a Library's is refused before anything is written, while on S3 the same
/// identity is a prefix somebody typed and a prefix nothing has ever been
/// written under cannot be wrong, because keys come into being by being
/// written.
///
/// Contents are the second question, and it is asked of both: a Drive folder
/// whose name is beyond doubt still says nothing about whether the Library in
/// it has ever been committed to, and somebody who joined it reads the same
/// empty `fetch` as somebody who mistyped a prefix. The answer has to be said
/// rather than acted on. Refusing [`NothingYet`](Self::NothingYet) would refuse
/// every join of a Library that was created a minute ago and not yet synced,
/// which is exactly the join a second device makes first.
///
/// What decides nothing is the answer, and only the answer. Storage failing
/// to give one decides that the join does not stand — and on Drive, where the
/// question can only be put after a grant, the staging goes with it, and so
/// does a new account consented to for this join, so the next attempt is
/// another browser consent where the join brought one.
/// The two differ because the question is put with the very call the work is
/// done with. Either value of this is a Library somebody can go on using,
/// while a place that cannot say which it is cannot be read from either, and a
/// join that recorded it would only hand the same failure to the first
/// `fetch`, which has less to say about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoundOnStorage {
    /// The place holds the Library: what a Library keeps at the top of its own
    /// place is there.
    TheLibrary,
    /// The place is where the Library would be, and holds nothing of one yet.
    ///
    /// Two things look like this and neither is a failure: a Library that has
    /// been created and never synced, and — on S3, where the place is a prefix
    /// rather than a folder whose name was checked — a prefix that is not the
    /// Library's, from a mistyped Library ID or the wrong base prefix in front
    /// of a right one. A caller says so, so that whoever typed it wrong finds
    /// out now rather than after a `fetch` that reports nothing and exits
    /// successfully.
    NothingYet,
}

impl FoundOnStorage {
    /// What a provider answering whether the first head object is there means.
    ///
    /// One place to read the `bool` so that the two providers cannot come to
    /// read it differently: the question is the same one, and a device joining
    /// is owed the same answer whichever of them holds its Library.
    pub(super) const fn of(held: bool) -> Self {
        match held {
            true => Self::TheLibrary,
            false => Self::NothingYet,
        }
    }
}
