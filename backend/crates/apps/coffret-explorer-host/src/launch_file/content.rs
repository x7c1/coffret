use crate::credentials::InstanceId;

/// The page a launch file holds: a refresh to `target`, and a link to it for a
/// browser that does not follow a refresh. No script, no handler, nothing that
/// runs (spec: LA-21).
///
/// `target` and `instance` are the host's own address, a ticket and a public
/// name, all of them drawn from characters that need no escaping in HTML:
/// digits, `.`, `:`, `/`, `#`, `=` and unpadded base64url.
///
/// The generation's public name is carried beside the ticket, so that whoever
/// opens the file can tell a file of the next generation from the last one's
/// without exchanging anything (spec: LA-18).
pub(super) fn page(target: &str, instance: &InstanceId) -> String {
    format!(
        "<!doctype html>\n\
         <html lang=\"en\">\n\
         <head>\n\
         <meta charset=\"utf-8\">\n\
         <meta name=\"referrer\" content=\"no-referrer\">\n\
         <meta name=\"coffret-instance-id\" content=\"{instance}\">\n\
         <meta http-equiv=\"refresh\" content=\"0; url={target}\">\n\
         <title>Opening the coffret explorer</title>\n\
         </head>\n\
         <body>\n\
         <p>Opening the coffret explorer. If it does not open, \
         <a href=\"{target}\" rel=\"noreferrer\">open it here</a>.</p>\n\
         </body>\n\
         </html>\n"
    )
}
