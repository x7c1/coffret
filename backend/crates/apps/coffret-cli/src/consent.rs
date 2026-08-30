use coffret_logging::redact;
use tracing::info;

/// Puts the consent URL in front of the person, and records that it was.
///
/// The URL goes to standard error whole, because it is the instruction rather
/// than the answer — standard output carries what a caller might pipe — and a
/// URL a browser cannot be given in full is no instruction at all.
///
/// The line under it is there because what happens next is nothing: the flow
/// blocks on a loopback redirect for as long as the gateway waits, and a
/// terminal that has printed a URL and then gone quiet looks the same whether
/// it is waiting for the person or hung. Saying which it is costs one line.
///
/// What is *recorded* is the endpoint alone. A query string is where a provider
/// puts values that grant access, and over-redaction is the deliberate
/// direction of error, so the log keeps which endpoint consent was asked at and
/// nothing that was asked with it.
pub fn ask(operation: &'static str, url: &str) {
    eprintln!("\nOpen this in a browser to allow access:\n\n{url}\n");
    eprintln!("Waiting here until you have answered; this gives up after a few minutes.");
    info!(
        operation,
        endpoint = redact::url(url),
        "asked for consent on Google Drive"
    );
}
