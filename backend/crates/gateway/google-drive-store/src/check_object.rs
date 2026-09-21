//! Asking whether a Library's app folder holds one named object, before there
//! is a store over it.
//!
//! The sibling of [`read_app_folder_name`](crate::read_app_folder_name), and
//! the second of the two questions a device joining a Library puts to Drive.
//! The first — what is this folder called — is about identity and is answered
//! by the name alone (spec: FM-18). This one is about contents: whether the
//! Library whose folder this is has ever committed anything, which is what
//! decides whether a `fetch` right after the join will find an empty Library
//! and be right about it.
//!
//! It is deliberately a `bool` rather than a refusal, and the S3 gateway's
//! object check answers the same question the same way: a Library created and
//! never synced holds nothing, and nothing at this layer can tell that apart
//! from a place that is not the Library's — so absence is an answer and the
//! caller is the one that says what it means.
//!
//! Drive has no lookup by name, and this crate already knows how to ask about
//! the contents of a folder: the port's `list` is `files.list` over the app
//! folder, and this is that same call with a name on the query. Nothing new
//! reaches Drive, and the folder's contents are described in the one place
//! they are described.
//!
//! Which is also why it reads the continuation the way `list` does. Drive
//! applies a query per page, so a listing that matches something can still hand
//! back a page with nothing on it and a `nextPageToken` for where to carry on;
//! only the absence of that token says the listing is over. Stopping at the
//! first empty page would report a Library that has been committed to as one
//! nobody has synced yet — the one wrong answer this exists to prevent.

use std::sync::Arc;

use coffret_logging::redact::PrivateValues;
use coffret_usecase::Missing;

use crate::answer_ceiling::MAX_LISTING_PAGE_LEN;
use crate::api::{authorization, named_file_query, DriveApi, Endpoints, FailedResponse, FileList};
use crate::error::{AppFolderDefect, Error, Result};
use crate::http::{HttpRequest, HttpTransport, Method};
use crate::oauth::AccessTokens;

/// What the call is recorded and reported as.
const OPERATION: &str = "check_object";

/// How many files one page of this listing asks for.
///
/// The whole page Drive gives, which is the page the port's own listing asks
/// for (`DriveSettings` defaults to it). Not for the sake of the answer — the
/// query names one object and the fields name one field of it — but for the
/// sake of the pages that come before it: Drive cuts a page and then applies
/// the query to what it cut, so a page of one can come back empty once for
/// every file in the folder it had to look past. A Library's app folder holds
/// a Container for every file it has ever packed, so a page of one would spend
/// a round trip on each of them on the way to a `head-` object — which is the
/// bound below being reached by a folder for being large rather than by a
/// provider for saying nothing.
const PAGE_SIZE: u32 = 1000;

/// How many pages this question may take before the folder is called
/// unanswerable.
///
/// Deliberately not the hundred thousand the usecase layer's control listing
/// bounds itself by. That walk reads a whole Library's control objects a
/// thousand to a page, so its number is a bound on how large a Library may
/// grow. This one asks whether one name is in one folder, and the first page
/// carrying anything at all ends it — so every page counted here is an empty
/// one, put in front of the answer by Drive filtering a page after it had cut
/// it. Each of them is a whole [`PAGE_SIZE`] page with nothing left on it, so a
/// thousand is already far past what the contents of a folder explain, and it
/// is also a thousand round trips: beyond it, waiting longer is waiting on
/// something that is not making progress.
const MAX_PAGES: usize = 1_000;

/// Whether the folder at `folder_id` holds a live object called `name`.
///
/// `files.list` narrowed to that name: what is wanted is whether anything came
/// back, and Drive reports nothing about a folder's contents more cheaply than
/// a listing of it. An empty page carrying
/// a continuation is not an answer, so the continuation is followed until a
/// page holds something or the listing says it is over — or until a bounded
/// number of them have gone by saying neither, which is a provider that is not
/// making progress rather than a folder being looked into.
///
/// Everything that is not an answer about the folder — a grant Drive refused, a
/// folder id that names nothing this application may read, an answer that never
/// arrived whole — is a failure rather than a `false`, because a caller reading
/// one of those as "the Library holds nothing" would report the wrong thing
/// entirely.
pub async fn check_object(
    transport: Arc<dyn HttpTransport>,
    tokens: Arc<dyn AccessTokens>,
    folder_id: &str,
    name: &str,
) -> Result<bool> {
    let unreadable = |cause| Error::LibraryObjectUnreadable {
        folder_id: folder_id.to_owned(),
        name: name.to_owned(),
        cause: Box::new(cause),
    };

    let api = DriveApi::new(transport, tokens, Endpoints::default());
    let query = named_file_query(folder_id, name);
    let page_size = PAGE_SIZE.to_string();
    let mut page: Option<String> = None;
    let mut pages: usize = 0;
    loop {
        // Only the identifiers are asked for, beside the continuation: the
        // answer this makes of a page is whether it is empty, and the token is
        // what says whether an empty one is the end.
        let url = {
            let mut pairs = url::form_urlencoded::Serializer::new(String::new());
            pairs.extend_pairs([
                ("q", query.as_str()),
                ("fields", "nextPageToken,files(id)"),
                ("pageSize", page_size.as_str()),
            ]);
            if let Some(token) = &page {
                pairs.append_pair("pageToken", token);
            }
            format!("{}?{}", api.endpoints().files(), pairs.finish())
        };

        let response = api
            .send(|token| {
                let (header, value) = authorization(token);
                HttpRequest::new(Method::Get, &url)
                    .with_header(header, value)
                    // A page of a listing rather than one of the documents the
                    // ordinary ceiling is for, and held against the same number
                    // the port's own listing is.
                    .within(MAX_LISTING_PAGE_LEN)
            })
            .await
            .map_err(|cause| unreadable(AppFolderDefect::Call(cause)))?;

        if !response.is_success() {
            // Nothing private here: the folder is the Library's own app folder
            // and the name is a control object's, which the format composes —
            // EL-5 leaves both as permitted evidence. The folder somebody chose
            // is the app folder's parent, which this call never names.
            let cause = FailedResponse::read(response, OPERATION, &PrivateValues::none())
                .await
                .into_error(Missing::Listing);
            return Err(unreadable(AppFolderDefect::Call(cause)));
        }

        let body = response
            .into_body()
            .into_bytes_within(MAX_LISTING_PAGE_LEN)
            .await
            .map_err(|cause| unreadable(AppFolderDefect::Call(cause)))?;
        let listing: FileList = serde_json::from_slice(&body)
            .map_err(|cause| unreadable(AppFolderDefect::Answer(cause)))?;

        if !listing.files.is_empty() {
            return Ok(true);
        }
        pages += 1;
        match listing.next_page_token {
            // Every page so far was empty and this one still says to carry on.
            // The continuation is what makes the answer right, so it is
            // followed — but only so far: a provider answering this way
            // forever would leave the join with no answer and no end to
            // waiting for one.
            Some(_) if pages >= MAX_PAGES => {
                return Err(unreadable(AppFolderDefect::UnendingListing { pages }))
            }
            // Nothing on this page and somewhere to carry on: Drive filtered a
            // page down to nothing rather than reaching the end of the listing.
            Some(token) => page = Some(token),
            None => return Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::{AtomicUsize, Ordering};

    use async_trait::async_trait;

    use crate::http::{HttpResponse, StubAnswer, StubTransport, TransportError};
    use crate::test_support::CountingTokens;

    /// The object a Library keeps at the top of its own place once it has
    /// committed anything at all: the first link of its head chain
    /// (spec: FM-12, CP-1).
    const NAME: &str = "head-0.cfrt";

    #[tokio::test]
    async fn an_object_the_folder_holds_is_there() {
        let transport = StubTransport::new([StubAnswer::json(200, r#"{"files":[{"id":"o-1"}]}"#)]);
        let tokens = CountingTokens::new();

        let held = check_object(transport.clone(), tokens, "folder-1", NAME)
            .await
            .expect("Drive answered about the folder");
        assert!(held, "a page with a file on it says the object is there");

        // The same call path the port's `list` takes, narrowed to one name:
        // one folder, and no second way of asking Drive what a folder holds.
        let request = transport.request(0);
        assert!(matches!(request.method, Method::Get));
        for expected in [
            "folder-1",
            "head-0.cfrt",
            "trashed",
            // The continuation is asked for by name, which is what lets an
            // empty page be told apart from the end of the listing.
            "fields=nextPageToken",
        ] {
            assert!(
                request.url.contains(expected),
                "the listing must carry {expected:?}: {}",
                request.url,
            );
        }
        // And the whole page Drive gives rather than a page of one. Drive cuts
        // a page before it applies the query, so the page size is what decides
        // how many empty pages a folder can put in front of the answer — and
        // the walk below is bounded at a number that only means "a provider
        // that is not making progress" while they stay rare.
        assert!(
            request.url.contains(&format!("pageSize={PAGE_SIZE}")),
            "the listing must ask for a whole page: {}",
            request.url,
        );
    }

    // The whole point of the `bool`: a folder holding nothing of the Library is
    // an answer and not a failure, because a Library created and never synced
    // holds nothing either and the two cannot be told apart from here.
    #[tokio::test]
    async fn a_folder_holding_nothing_of_the_library_is_an_answer_rather_than_a_refusal() {
        let transport = StubTransport::new([StubAnswer::json(200, r#"{"files":[]}"#)]);
        let tokens = CountingTokens::new();

        let held = check_object(transport, tokens, "folder-1", NAME)
            .await
            .expect("an empty folder is something Drive can answer");
        assert!(
            !held,
            "nothing came back, so nothing of the Library is there"
        );
    }

    // A page carrying no `files` at all is the same answer: Drive leaves the
    // field out where a listing matched nothing.
    #[tokio::test]
    async fn a_page_naming_no_files_holds_nothing_either() {
        let transport = StubTransport::new([StubAnswer::json(200, r#"{}"#)]);
        let tokens = CountingTokens::new();

        let held = check_object(transport, tokens, "folder-1", NAME)
            .await
            .expect("a page with no files on it is an answer");
        assert!(!held);
    }

    // Drive filters a page after it has cut one, so a listing that matches
    // something can still hand back an empty page and somewhere to carry on.
    // Reading that first page as the answer would tell somebody joining a
    // Library that has been committed to that nothing has ever been synced into
    // it — the one wrong answer this call exists to prevent.
    #[tokio::test]
    async fn an_empty_page_with_somewhere_to_carry_on_is_not_the_end_of_the_listing() {
        let transport = StubTransport::new([
            StubAnswer::json(200, r#"{"files":[],"nextPageToken":"page-2"}"#),
            StubAnswer::json(200, r#"{"files":[{"id":"o-1"}]}"#),
        ]);
        let tokens = CountingTokens::new();

        let held = check_object(transport.clone(), tokens, "folder-1", NAME)
            .await
            .expect("Drive answered about the folder");
        assert!(held, "the object is on the page the continuation led to");

        assert_eq!(transport.call_count(), 2, "the continuation was followed");
        assert!(
            transport.request(1).url.contains("pageToken=page-2"),
            "the second call carries what the first said to carry on with: {}",
            transport.request(1).url,
        );
    }

    // And the same listing run out: every page was empty and the last one said
    // so by carrying no continuation, which is the only thing that makes the
    // `false` an answer rather than a page boundary.
    #[tokio::test]
    async fn a_listing_that_runs_out_empty_holds_nothing() {
        let transport = StubTransport::new([
            StubAnswer::json(200, r#"{"files":[],"nextPageToken":"page-2"}"#),
            StubAnswer::json(200, r#"{"files":[]}"#),
        ]);
        let tokens = CountingTokens::new();

        let held = check_object(transport.clone(), tokens, "folder-1", NAME)
            .await
            .expect("a listing that ends is something Drive can answer");
        assert!(!held, "nothing came back on any page of the listing");
        assert_eq!(transport.call_count(), 2);
    }

    /// A Drive that never ends a listing: every page is empty and every one of
    /// them names somewhere else to carry on.
    ///
    /// Scripted answers cannot say "forever", and a transport is the only place
    /// the difference between a bounded walk and an unbounded one shows.
    struct EndlessListing {
        pages: AtomicUsize,
    }

    #[async_trait]
    impl HttpTransport for EndlessListing {
        async fn execute(
            &self,
            _request: HttpRequest,
        ) -> std::result::Result<HttpResponse, TransportError> {
            let page = self.pages.fetch_add(1, Ordering::SeqCst) + 1;
            Ok(HttpResponse::json(
                200,
                &format!(r#"{{"files":[],"nextPageToken":"page-{page}"}}"#),
            ))
        }
    }

    // The continuation is followed because an empty page is not an answer, and
    // that is exactly what a provider needs to answer to keep this call going
    // forever. Somebody who asked a device to join a Library would be left
    // watching a command that never returns, so the walk gives up and says so.
    #[tokio::test]
    async fn a_listing_that_never_ends_is_refused_rather_than_walked_forever() {
        let transport = Arc::new(EndlessListing {
            pages: AtomicUsize::new(0),
        });
        let tokens = CountingTokens::new();

        let error = check_object(transport.clone(), tokens, "folder-1", NAME)
            .await
            .expect_err("a listing that never ends answers nothing about the folder");

        let Error::LibraryObjectUnreadable {
            folder_id,
            name,
            cause,
        } = &error
        else {
            panic!("expected the folder to be reported as unreadable, got {error:?}");
        };
        assert_eq!(folder_id, "folder-1");
        assert_eq!(name, NAME);
        assert!(
            matches!(
                cause.as_ref(),
                AppFolderDefect::UnendingListing { pages } if *pages == MAX_PAGES,
            ),
            "expected the listing to be reported as unending, got {cause:?}",
        );
        assert_eq!(
            transport.pages.load(Ordering::SeqCst),
            MAX_PAGES,
            "the walk stopped at the cap rather than carrying on",
        );
        // What the person reads says how far it got, which is what tells this
        // apart from a folder that answered nothing at all.
        assert_eq!(
            cause.to_string(),
            format!("the listing did not end within {MAX_PAGES} pages"),
        );
    }

    // A grant Drive refused says nothing about what the folder holds, so it
    // must never come back as the `false` a caller reads as an empty Library.
    #[tokio::test]
    async fn a_refusal_is_not_an_empty_folder() {
        let transport = StubTransport::new([StubAnswer::json(
            403,
            r#"{"error":{"message":"No access.","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
        )]);
        let tokens = CountingTokens::new();

        let error = check_object(transport, tokens, "folder-1", NAME)
            .await
            .expect_err("a refused listing answers nothing about the folder");

        let Error::LibraryObjectUnreadable {
            folder_id,
            name,
            cause,
        } = &error
        else {
            panic!("expected the folder to be reported as unreadable, got {error:?}");
        };
        assert_eq!(folder_id, "folder-1");
        assert_eq!(name, NAME, "the object that was asked for travels with it");
        assert!(
            matches!(
                cause.as_ref(),
                AppFolderDefect::Call(coffret_usecase::Error::PermissionDenied { .. })
            ),
            "expected the refusal to arrive classified, got {cause:?}",
        );
    }

    // An answer this build cannot read is not an empty folder either.
    #[tokio::test]
    async fn an_answer_that_is_not_a_listing_is_not_an_empty_folder() {
        let transport = StubTransport::new([StubAnswer::json(200, r#"{"files":"none"}"#)]);
        let tokens = CountingTokens::new();

        let error = check_object(transport, tokens, "folder-1", NAME)
            .await
            .expect_err("an unreadable answer says nothing about the folder");
        let Error::LibraryObjectUnreadable { cause, .. } = &error else {
            panic!("expected the folder to be reported as unreadable, got {error:?}");
        };
        assert!(
            matches!(cause.as_ref(), AppFolderDefect::Answer(_)),
            "expected an unreadable answer, got {cause:?}"
        );
    }
}
