use tracing::Level;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::fmt::format::{Format, Json, JsonFields};
use tracing_subscriber::fmt::{MakeWriter, Subscriber};

/// A subscriber that writes every event as one JSON object on one line.
///
/// The file exists to be analysed rather than read down a terminal — every
/// catch-all `warn` grouped by the reason a provider gave, whether an
/// unfamiliar 403 has ever arrived, how often a retry gave up — and those are
/// questions about fields. An event already carries them named; the default
/// formatter flattens them into a `field=value` tail and leaves whoever comes
/// to read it re-extracting them with regular expressions. JSON keeps them, and
/// a provider's own body, which arrives full of quotes and braces and
/// newlines, is escaped by the format instead of having to be flattened first.
///
/// Both the installed sink and the capturing subscriber the `testing` feature
/// offers are built here, so that a case asserting on a field is asserting on
/// the same bytes the file would have been given. A test reading a
/// differently-shaped record would prove nothing about the file.
///
/// # What a record carries, and what it is not allowed to
///
/// `timestamp`, `level`, `target`, and the event's own `fields` — nothing more.
/// The formatter offers several further keys, and each is off for a reason:
///
/// - `filename` and `line_number` name a source file of coffret's, so they give
///   away nothing about anybody's disk. They stay off because they answer a
///   question this file is not kept for: it says what a provider answered, not
///   which line recorded it, and every record would pay for them out of a fixed
///   byte budget.
/// - `span` and `spans` are on by default and are turned off here. They carry
///   the fields of whatever span was open, which is a second surface the rule
///   about what may never be written would have to be audited over — including
///   a dependency's spans, once `COFFRET_LOG` names its target. Coffret opens
///   no spans of its own, so nothing is lost today, and turning them off is
///   what keeps that true tomorrow.
/// - `threadName` and `threadId` stay off: a thread's name says which test or
///   which worker happened to run the call, and answers nothing about a
///   provider.
///
/// Colour never arises: the JSON formatter writes no escape sequences, so there
/// is nothing to turn off to keep the file parseable.
pub(crate) fn subscriber<W>(
    writer: W,
    level: Level,
) -> Subscriber<JsonFields, Format<Json>, LevelFilter, W>
where
    // A subscriber is shared by every thread that emits, which is what the last
    // two ask for.
    W: for<'writer> MakeWriter<'writer> + Send + Sync + 'static,
{
    tracing_subscriber::fmt()
        .json()
        // Fields stay nested under `fields` rather than being flattened into
        // the record: an event is free to name a field `level` or `target`, and
        // flattening would put it next to the envelope's own key of that name,
        // leaving a duplicate that readers resolve differently.
        .flatten_event(false)
        .with_current_span(false)
        .with_span_list(false)
        .with_writer(writer)
        .with_max_level(level)
        .finish()
}
