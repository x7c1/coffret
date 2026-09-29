//! Asking whether a Library's app folder holds any head or Index Snapshot,
//! before there is a store over it.
//!
//! The sibling of [`read_app_folder_name`](crate::read_app_folder_name), and
//! the second of the two questions a device joining a Library puts to Drive.
//! The first — what is this folder called — is about identity and is answered
//! by the name alone (spec: FM-18). This one is about contents: whether the
//! Library whose folder this is has ever committed anything, which is what
//! decides whether a `fetch` right after the join will find an empty Library
//! and be right about it.
//!
//! No one object answers that, because which heads survive depends on what has
//! been pruned; what a Library that has committed anything always holds is a
//! head or a Snapshot (see
//! [`ControlObjectName::names_a_head_or_index_snapshot`]). So this asks for
//! both by the prefixes their names start with.
//!
//! It is deliberately a `bool` rather than a refusal, and the S3 gateway's
//! check answers the same question the same way: a Library created and
//! never synced holds nothing, and nothing at this layer can tell that apart
//! from a place that is not the Library's — so absence is an answer and the
//! caller is the one that says what it means.
//!
//! Drive has no lookup by prefix, and this crate already knows how to ask about
//! the contents of a folder: the port's `list` is `files.list` over the app
//! folder, and this is that same call with a name clause on the query. Nothing
//! new reaches Drive, and the folder's contents are described in the one place
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
use coffret_model::ControlObjectName;
use coffret_usecase::Missing;

use crate::answer_ceiling::MAX_LISTING_PAGE_LEN;
use crate::api::{
    authorization, name_prefixes_query, DriveApi, Endpoints, FailedResponse, FileList,
};
use crate::error::{AppFolderDefect, Error, Result};
use crate::http::{HttpRequest, HttpTransport, Method};
use crate::oauth::AccessTokens;

/// What the call is recorded and reported as.
const OPERATION: &str = "check_any_head_or_snapshot";

/// How many files one page of this listing asks for.
///
/// The whole page Drive gives, which is the page the port's own listing asks
/// for (`DriveSettings` defaults to it). Not for the sake of the answer — one
/// head or Snapshot is enough — but for the sake of the pages that come before
/// it: Drive cuts a page and then applies the query to what it cut, so a page
/// of one can come back empty once for every file in the folder it had to look
/// past. A Library's app folder holds a Container for every file it has ever
/// packed, so a page of one would spend a round trip on each of them on the way
/// to a head or a Snapshot — which is the bound below being reached by a folder
/// for being large rather than by a provider for saying nothing.
const PAGE_SIZE: u32 = 1000;

/// How many pages this question may take before the folder is called
/// unanswerable.
///
/// Deliberately not the hundred thousand the usecase layer's control listing
/// bounds itself by. That walk reads a whole Library's control objects a
/// thousand to a page, so its number is a bound on how large a Library may
/// grow. This one asks whether any head or Snapshot is in one folder, and the
/// first one on a page ends it — so every page counted here is one with
/// neither on it, put in front of the answer by Drive filtering a page after it
/// had cut it. Each of them is a whole [`PAGE_SIZE`] page with nothing of the
/// kind left on it, so a thousand is already far past what the contents of a
/// folder explain, and it is also a thousand round trips: beyond it, waiting
/// longer is waiting on something that is not making progress.
const MAX_PAGES: usize = 1_000;

/// Whether the folder at `folder_id` holds any live head or ordinary Index
/// Snapshot.
///
/// One `files.list` narrowed to the names that start the way every head's or
/// every Snapshot's does — what those names start with is the format's
/// ([`ControlObjectName::HEAD_NAME_PREFIX`],
/// [`ControlObjectName::INDEX_SNAPSHOT_NAME_PREFIX`]) rather than spelled here —
/// and each name that comes back read as a whole
/// ([`ControlObjectName::names_a_head_or_index_snapshot`]), because Drive's
/// `contains` narrows the listing rather than deciding it (see
/// `name_prefixes_query`) and a file that only looks like one cannot answer
/// for a Library. The first head or Snapshot ends the walk with `true`. A page
/// carrying neither and a continuation is not an answer, so the continuation
/// is followed until a page holds one or the listing says it is over — or
/// until a bounded number of them have gone by saying neither, which is a
/// provider that is not making progress rather than a folder being looked
/// into.
///
/// Everything that is not an answer about the folder — a grant Drive refused, a
/// folder id that names nothing this application may read, an answer that never
/// arrived whole, a listing that never ends — is a failure rather than a
/// `false`, because a caller reading one of those as "the Library holds
/// nothing" would report the wrong thing entirely.
pub async fn check_any_head_or_snapshot(
    transport: Arc<dyn HttpTransport>,
    tokens: Arc<dyn AccessTokens>,
    folder_id: &str,
) -> Result<bool> {
    let unreadable = |cause| Error::LibraryObjectUnreadable {
        folder_id: folder_id.to_owned(),
        cause,
    };

    let api = DriveApi::new(transport, tokens, Endpoints::default());
    let query = name_prefixes_query(
        folder_id,
        &[
            ControlObjectName::HEAD_NAME_PREFIX,
            ControlObjectName::INDEX_SNAPSHOT_NAME_PREFIX,
        ],
    );
    let page_size = PAGE_SIZE.to_string();
    let mut page: Option<String> = None;
    let mut pages: usize = 0;
    loop {
        // The identifier and the name, beside the continuation: the name is
        // what says whether a file on the page is a head or a Snapshot, and
        // the token is what says whether a page without one is the end.
        let url = {
            let mut pairs = url::form_urlencoded::Serializer::new(String::new());
            pairs.extend_pairs([
                ("q", query.as_str()),
                ("fields", "nextPageToken,files(id,name)"),
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
            // and the prefixes are control objects', which the format composes —
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

        let found = listing
            .files
            .iter()
            .filter_map(|file| file.name.as_deref())
            .any(ControlObjectName::names_a_head_or_index_snapshot);
        if found {
            return Ok(true);
        }
        pages += 1;
        match listing.next_page_token {
            // Every page so far held neither and this one still says to carry
            // on. The continuation is what makes the answer right, so it is
            // followed — but only so far: a provider answering this way
            // forever would leave the join with no answer and no end to
            // waiting for one.
            Some(_) if pages >= MAX_PAGES => {
                return Err(unreadable(AppFolderDefect::UnendingListing { pages }))
            }
            // Neither on this page and somewhere to carry on: Drive filtered a
            // page down to nothing of the kind rather than reaching the end of
            // the listing.
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
    use coffret_model::Generation;

    use crate::http::{HttpResponse, StubAnswer, StubTransport, TransportError};
    use crate::test_support::CountingTokens;

    /// The name of the head at `generation`, as the format spells it.
    fn head(generation: u64) -> String {
        let generation = Generation::new(generation).expect("the case names a real generation");
        ControlObjectName::head(generation).to_string()
    }

    /// A page of a listing naming `names`, carrying `next` as the token to
    /// carry on with where there is one.
    fn page(names: &[&str], next: Option<&str>) -> String {
        let files: Vec<_> = names
            .iter()
            .enumerate()
            .map(|(index, name)| serde_json::json!({ "id": format!("o-{index}"), "name": name }))
            .collect();
        let mut page = serde_json::json!({ "files": files });
        if let Some(next) = next {
            page["nextPageToken"] = serde_json::json!(next);
        }
        page.to_string()
    }

    // The case this exists for: a Library pruned past its first checkpoint no
    // longer holds generation 0, and still holds its Journal.
    #[tokio::test]
    async fn a_folder_holding_only_a_later_head_holds_the_library() {
        let later = head(3);
        let transport = StubTransport::new([StubAnswer::json(200, &page(&[&later], None))]);
        let tokens = CountingTokens::new();

        let held = check_any_head_or_snapshot(transport.clone(), tokens, "folder-1")
            .await
            .expect("Drive answered about the folder");
        assert!(held, "a head that is not the first is still a head");

        // The same call path the port's `list` takes, narrowed by the prefix
        // every head's name starts with: one folder, and no second way of
        // asking Drive what a folder holds.
        let request = transport.request(0);
        assert!(matches!(request.method, Method::Get));
        let query: String = url::Url::parse(&request.url)
            .expect("a listing is addressed at a URL")
            .query_pairs()
            .find(|(key, _)| key == "q")
            .map(|(_, value)| value.into_owned())
            .expect("a listing carries a query");
        assert_eq!(
            query,
            format!(
                "'folder-1' in parents and trashed = false and \
                 (name contains '{}' or name contains '{}')",
                ControlObjectName::HEAD_NAME_PREFIX,
                ControlObjectName::INDEX_SNAPSHOT_NAME_PREFIX,
            ),
        );
        for expected in [
            // The continuation is asked for by name, which is what lets a page
            // without a head or a Snapshot be told apart from the end of the
            // listing — and the name, which is what says whether a file is one.
            "fields=nextPageToken%2Cfiles%28id%2Cname%29",
            // And the whole page Drive gives rather than a page of one. Drive
            // cuts a page before it applies the query, so the page size is what
            // decides how many empty pages a folder can put in front of the
            // answer.
            &format!("pageSize={PAGE_SIZE}"),
        ] {
            assert!(
                request.url.contains(expected),
                "the listing must carry {expected:?}: {}",
                request.url,
            );
        }
    }

    #[tokio::test]
    async fn a_folder_holding_the_first_head_holds_the_library() {
        let first = head(0);
        let transport = StubTransport::new([StubAnswer::json(200, &page(&[&first], None))]);
        let tokens = CountingTokens::new();

        let held = check_any_head_or_snapshot(transport, tokens, "folder-1")
            .await
            .expect("Drive answered about the folder");
        assert!(held);
    }

    // The case heads alone would miss: a Library pruned past its last head
    // still holds the Snapshot that applied it (spec: CK-2, CK-4).
    #[tokio::test]
    async fn a_folder_holding_a_snapshot_and_no_head_holds_the_library() {
        let generation = Generation::new(3).expect("3 is a generation");
        let snapshot = ControlObjectName::index_snapshot(generation).to_string();
        let transport = StubTransport::new([StubAnswer::json(200, &page(&[&snapshot], None))]);
        let tokens = CountingTokens::new();

        let held = check_any_head_or_snapshot(transport, tokens, "folder-1")
            .await
            .expect("Drive answered about the folder");
        assert!(
            held,
            "a Snapshot is what a Library pruned of every head holds"
        );
    }

    // The whole point of the `bool`: a folder holding nothing of the Library is
    // an answer and not a failure, because a Library created and never synced
    // holds nothing either and the two cannot be told apart from here.
    #[tokio::test]
    async fn a_folder_holding_no_head_is_an_answer_rather_than_a_refusal() {
        let transport = StubTransport::new([StubAnswer::json(200, r#"{"files":[]}"#)]);
        let tokens = CountingTokens::new();

        let held = check_any_head_or_snapshot(transport, tokens, "folder-1")
            .await
            .expect("an empty folder is something Drive can answer");
        assert!(
            !held,
            "nothing came back, so neither a head nor a Snapshot is there"
        );
    }

    // A page carrying no `files` at all is the same answer: Drive leaves the
    // field out where a listing matched nothing.
    #[tokio::test]
    async fn a_page_naming_no_files_holds_nothing_either() {
        let transport = StubTransport::new([StubAnswer::json(200, r#"{}"#)]);
        let tokens = CountingTokens::new();

        let held = check_any_head_or_snapshot(transport, tokens, "folder-1")
            .await
            .expect("a page with no files on it is an answer");
        assert!(!held);
    }

    // Drive's `contains` narrows rather than decides, so a file whose name
    // only starts like a head's can come back. It is not a head, and the
    // head the listing carries on to is.
    #[tokio::test]
    async fn a_file_that_only_starts_like_a_head_is_looked_past() {
        let later = head(5);
        let transport = StubTransport::new([
            StubAnswer::json(200, &page(&["head-notes.txt"], Some("page-2"))),
            StubAnswer::json(200, &page(&[&later], None)),
        ]);
        let tokens = CountingTokens::new();

        let held = check_any_head_or_snapshot(transport.clone(), tokens, "folder-1")
            .await
            .expect("Drive answered about the folder");
        assert!(held);
        assert_eq!(transport.call_count(), 2);
    }

    // And a folder holding nothing but such a file holds neither.
    #[tokio::test]
    async fn a_folder_holding_only_a_stray_file_holds_no_head() {
        let transport =
            StubTransport::new([StubAnswer::json(200, &page(&["head-notes.txt"], None))]);
        let tokens = CountingTokens::new();

        let held = check_any_head_or_snapshot(transport, tokens, "folder-1")
            .await
            .expect("Drive answered about the folder");
        assert!(!held);
    }

    // Drive filters a page after it has cut one, so a listing that matches
    // something can still hand back an empty page and somewhere to carry on.
    // Reading that first page as the answer would tell somebody joining a
    // Library that has been committed to that nothing has ever been synced into
    // it — the one wrong answer this call exists to prevent.
    #[tokio::test]
    async fn an_empty_page_with_somewhere_to_carry_on_is_not_the_end_of_the_listing() {
        let later = head(2);
        let transport = StubTransport::new([
            StubAnswer::json(200, r#"{"files":[],"nextPageToken":"page-2"}"#),
            StubAnswer::json(200, &page(&[&later], None)),
        ]);
        let tokens = CountingTokens::new();

        let held = check_any_head_or_snapshot(transport.clone(), tokens, "folder-1")
            .await
            .expect("Drive answered about the folder");
        assert!(held, "the head is on the page the continuation led to");

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

        let held = check_any_head_or_snapshot(transport.clone(), tokens, "folder-1")
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

        let error = check_any_head_or_snapshot(transport.clone(), tokens, "folder-1")
            .await
            .expect_err("a listing that never ends answers nothing about the folder");

        let Error::LibraryObjectUnreadable { folder_id, cause } = &error else {
            panic!("expected the folder to be reported as unreadable, got {error:?}");
        };
        assert_eq!(folder_id, "folder-1");
        assert!(
            matches!(
                cause,
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

        let error = check_any_head_or_snapshot(transport, tokens, "folder-1")
            .await
            .expect_err("a refused listing answers nothing about the folder");

        let Error::LibraryObjectUnreadable { folder_id, cause } = &error else {
            panic!("expected the folder to be reported as unreadable, got {error:?}");
        };
        assert_eq!(folder_id, "folder-1");
        assert!(
            matches!(
                cause,
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

        let error = check_any_head_or_snapshot(transport, tokens, "folder-1")
            .await
            .expect_err("an unreadable answer says nothing about the folder");
        let Error::LibraryObjectUnreadable { cause, .. } = &error else {
            panic!("expected the folder to be reported as unreadable, got {error:?}");
        };
        assert!(
            matches!(cause, AppFolderDefect::Answer(_)),
            "expected an unreadable answer, got {cause:?}"
        );
    }
}
