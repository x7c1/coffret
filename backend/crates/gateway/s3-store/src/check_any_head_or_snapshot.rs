//! Asking a bucket whether any head or Index Snapshot of a Library is under a
//! prefix, before there is a store over it.
//!
//! The sibling of [`check_bucket`](crate::check_bucket) and here for the same
//! reason: taking up a Library that already exists has one question for Storage
//! that it has to ask before a store is built, and reading the answer means
//! reading a status and an S3 error code — which this crate holds the one table
//! for.
//!
//! Where a Library lives on S3 is a prefix somebody typed, and a prefix that is
//! not a Library's looks exactly like one: keys come into being by being
//! written, so nothing about the shape of a prefix says whether any Library is
//! there. No one object answers it either, because which heads survive depends
//! on what has been pruned; what a Library that has committed anything always
//! holds is a head or a Snapshot (see
//! [`ControlObjectName::names_a_head_or_index_snapshot`]). So this asks by
//! prefix whether either is there, and it is deliberately a `bool` rather than
//! a refusal: a Library that has been created and never synced holds neither,
//! so absence is two different things and neither of them is an error.

use aws_sdk_s3::Client;
use coffret_logging::redact::PrivateValues;
use coffret_model::ControlObjectName;
use coffret_usecase::{Error, Missing, Result};

use crate::error::classify;
use crate::key_layout::{KeyLayout, DELIMITER};

/// What the call is recorded and reported as.
const OPERATION: &str = "check_any_head_or_snapshot";

/// How many pages one of this question's listings may take before the prefix
/// is called unanswerable.
///
/// Each page is one key, and the first key that is a head or a Snapshot ends
/// it. What puts pages in front of the answer is keys that start like one and
/// are not — or a nested key the listing collapses into a common prefix, which
/// takes a page's one slot and names no object — and a Library writes neither.
/// A thousand of them is far past anything a Library's own prefix explains,
/// and beyond it waiting longer is waiting on a listing that is not making
/// progress.
const MAX_PAGES: usize = 1_000;

/// Whether `bucket` holds any head or ordinary Index Snapshot under `prefix`.
///
/// Heads and Snapshots are spelled with different prefixes and sit among the
/// Containers directly under the Library's, so no one listing narrower than
/// the whole Library covers both. Two narrow ones do: a `ListObjectsV2` of the
/// prefix every head's key starts with, and — only when that one ends with no
/// head — one of the prefix every Snapshot's key starts with, each asking for a
/// single key per page. The first head or Snapshot answers `true`, and `false`
/// is only both listings ending with neither. A Library that has committed
/// anything and never pruned holds heads, so it is answered by the first page.
///
/// The prefix goes through the same key layout every other call in this crate
/// uses, so the keys asked about are the keys the store writes, and what a
/// head's or a Snapshot's name starts with is the format's
/// ([`ControlObjectName::HEAD_NAME_PREFIX`],
/// [`ControlObjectName::INDEX_SNAPSHOT_NAME_PREFIX`]) rather than spelled here.
/// A key under either prefix still has to name one as a whole
/// ([`ControlObjectName::names_a_head_or_index_snapshot`]) to count, so a stray
/// object that only starts like one cannot answer for a Library.
///
/// Everything that is not an answer about the prefix — credentials that were
/// refused, an endpoint nothing is listening at, a bucket that is not there, a
/// listing that never ends — travels as the port's own error, because those say
/// nothing about the prefix and a caller reading them as "no Library here"
/// would be reporting the wrong thing entirely. A bucket that is not there is
/// among them: unlike a key asked about by name, a listing answers an empty
/// prefix with an empty page, so a `404` to it is only ever about the bucket.
pub async fn check_any_head_or_snapshot(
    client: &Client,
    bucket: &str,
    prefix: &str,
) -> Result<bool> {
    let layout = KeyLayout::new(prefix);
    // S3 answers a refusal by quoting back what it was asked about — in prose,
    // in the URI, or both — so the bucket and the prefix are taken out of
    // whatever provider text the failure carries (spec: EL-5). The prefix is
    // the Library's own, which is somebody's configuration, and it goes in
    // without its trailing separator for the reason the store's own does: that
    // is the one spelling every key under it contains.
    let private = PrivateValues::none()
        .with(bucket)
        .with(prefix.trim_end_matches(DELIMITER));

    // Heads first: every Library that has committed anything and not been
    // pruned of all of them is answered here, in one call.
    for name_prefix in [
        ControlObjectName::HEAD_NAME_PREFIX,
        ControlObjectName::INDEX_SNAPSHOT_NAME_PREFIX,
    ] {
        if any_under(client, bucket, &layout, name_prefix, &private).await? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Whether a listing of the live keys whose names start with `name_prefix`
/// turns up a head or a Snapshot before it ends.
async fn any_under(
    client: &Client,
    bucket: &str,
    layout: &KeyLayout,
    name_prefix: &str,
    private: &PrivateValues,
) -> Result<bool> {
    let keys = layout.live_key(name_prefix);
    let mut page: Option<String> = None;
    let mut pages: usize = 0;
    loop {
        let mut request = client
            .list_objects_v2()
            .bucket(bucket)
            .prefix(&keys)
            // Collapse everything below a separator, as the store's own listing
            // does: a nested key is no object this Library stores.
            .delimiter(DELIMITER)
            .max_keys(1);
        if let Some(token) = &page {
            request = request.continuation_token(token);
        }
        let response = request
            .send()
            .await
            .map_err(|error| classify(OPERATION, Missing::Listing, error, private))?;

        let found = response
            .contents()
            .iter()
            .filter_map(|object| object.key())
            .filter_map(|key| layout.name_of(key))
            .any(ControlObjectName::names_a_head_or_index_snapshot);
        if found {
            return Ok(true);
        }
        pages += 1;
        match response.next_continuation_token() {
            // Nothing of the kind on this page and a listing that says to carry
            // on: followed, because stopping here would call a Library a prefix
            // holding nothing — but only so far, because a listing answering
            // this way forever would leave the join with no answer.
            Some(_) if pages >= MAX_PAGES => {
                return Err(Error::ListingPastCap {
                    pages,
                    source: None,
                })
            }
            Some(token) if response.is_truncated() == Some(true) => page = Some(token.to_owned()),
            _ => return Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use aws_sdk_s3::config::retry::RetryConfig;
    use aws_sdk_s3::config::{BehaviorVersion, Credentials, Region};
    use aws_smithy_http_client::test_util::{ReplayEvent, StaticReplayClient};
    use aws_smithy_runtime_api::client::http::{
        HttpClient, HttpConnector, HttpConnectorFuture, HttpConnectorSettings, SharedHttpConnector,
    };
    use aws_smithy_runtime_api::client::orchestrator::{HttpRequest, HttpResponse};
    use aws_smithy_runtime_api::client::runtime_components::RuntimeComponents;
    use aws_smithy_types::body::SdkBody;
    use coffret_model::Generation;

    use super::*;

    /// The bucket the cases ask about.
    const BUCKET: &str = "someones-holiday-photos";

    /// The prefix a Library of its own would sit under.
    const PREFIX: &str = "archive/coffret-0123456789abcdef/";

    /// The key of the head at `generation` under [`PREFIX`].
    fn head_key(generation: u64) -> String {
        let generation = Generation::new(generation).expect("the case names a real generation");
        format!("{PREFIX}{}", ControlObjectName::head(generation))
    }

    /// The key of the ordinary Index Snapshot at `generation` under [`PREFIX`].
    fn snapshot_key(generation: u64) -> String {
        let generation = Generation::new(generation).expect("the case names a real generation");
        format!("{PREFIX}{}", ControlObjectName::index_snapshot(generation))
    }

    /// A page of a listing holding `keys`, carrying `next` as the token to
    /// carry on with where there is one.
    fn page(keys: &[&str], next: Option<&str>) -> String {
        let contents: String = keys
            .iter()
            .map(|key| format!("<Contents><Key>{key}</Key><Size>1</Size></Contents>"))
            .collect();
        let continuation = next
            .map(|token| format!("<NextContinuationToken>{token}</NextContinuationToken>"))
            .unwrap_or_default();
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><ListBucketResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/"><Name>{BUCKET}</Name><KeyCount>{}</KeyCount><MaxKeys>1</MaxKeys><IsTruncated>{}</IsTruncated>{contents}{continuation}</ListBucketResult>"#,
            keys.len(),
            next.is_some(),
        )
    }

    /// A client over `http_client`, pointed at nowhere and never retrying.
    fn client_over(http_client: impl HttpClient + 'static) -> Client {
        let config = aws_sdk_s3::Config::builder()
            .behavior_version(BehaviorVersion::latest())
            .region(Region::new("us-east-1"))
            .endpoint_url("http://storage.invalid")
            .credentials_provider(Credentials::new("key", "secret", None, None, "test"))
            .retry_config(RetryConfig::disabled())
            .force_path_style(true)
            .http_client(http_client)
            .build();
        Client::from_conf(config)
    }

    /// A client whose calls are answered in order, each with its status and
    /// body, and the replay that recorded what was asked.
    fn answering(answers: &[(u16, &str)]) -> (Client, StaticReplayClient) {
        let http_client = StaticReplayClient::new(
            answers
                .iter()
                .map(|(status, body)| {
                    ReplayEvent::new(
                        HttpRequest::empty(),
                        HttpResponse::new(
                            (*status).try_into().expect("the case names a real status"),
                            SdkBody::from(*body),
                        ),
                    )
                })
                .collect(),
        );
        (client_over(http_client.clone()), http_client)
    }

    /// The query of the `index`th request the replay recorded.
    fn query_of(replay: &StaticReplayClient, index: usize) -> String {
        let requests: Vec<_> = replay.actual_requests().collect();
        requests[index]
            .uri()
            .split_once('?')
            .map(|(_, query)| query)
            .unwrap_or("")
            .to_owned()
    }

    // The case this exists for: a Library pruned past its first checkpoint no
    // longer holds generation 0, and still holds its Journal.
    #[tokio::test]
    async fn a_prefix_holding_only_a_later_head_holds_the_library() {
        let later = head_key(1);
        let (client, replay) = answering(&[(200, &page(&[&later], None))]);

        let result = check_any_head_or_snapshot(&client, BUCKET, PREFIX).await;
        assert!(
            matches!(result, Ok(true)),
            "expected a head, got {result:?}"
        );

        // One key under the prefix every head's key starts with — through the
        // key layout, so what is asked about is what the store writes.
        let query = query_of(&replay, 0);
        for expected in [
            "list-type=2",
            "prefix=archive%2Fcoffret-0123456789abcdef%2Fhead-",
            "max-keys=1",
        ] {
            assert!(query.contains(expected), "expected {expected:?} in {query}");
        }
    }

    #[tokio::test]
    async fn a_prefix_holding_the_first_head_holds_the_library() {
        let first = head_key(0);
        let (client, _) = answering(&[(200, &page(&[&first], None))]);

        let result = check_any_head_or_snapshot(&client, BUCKET, PREFIX).await;
        assert!(
            matches!(result, Ok(true)),
            "expected a head, got {result:?}"
        );
    }

    // The whole point of the `bool`: a prefix holding nothing is an answer and
    // not a failure, because a Library that has never been synced holds nothing
    // either and the two cannot be told apart from here.
    #[tokio::test]
    async fn an_empty_prefix_is_an_answer_rather_than_a_refusal() {
        let (client, replay) = answering(&[(200, &page(&[], None)), (200, &page(&[], None))]);

        let result = check_any_head_or_snapshot(&client, BUCKET, PREFIX).await;
        assert!(
            matches!(result, Ok(false)),
            "expected an empty prefix to be an answer, got {result:?}"
        );
        // Heads and Snapshots were both asked about before the answer was `false`.
        let query = query_of(&replay, 1);
        assert!(
            query.contains("prefix=archive%2Fcoffret-0123456789abcdef%2Fidx-"),
            "expected the Snapshots to be asked about in {query}"
        );
    }

    // The case heads alone would miss: a Library pruned past its last head
    // still holds the Snapshot that applied it (spec: CK-2, CK-4).
    #[tokio::test]
    async fn a_prefix_holding_a_snapshot_and_no_head_holds_the_library() {
        let snapshot = snapshot_key(3);
        let (client, replay) =
            answering(&[(200, &page(&[], None)), (200, &page(&[&snapshot], None))]);

        let result = check_any_head_or_snapshot(&client, BUCKET, PREFIX).await;
        assert!(
            matches!(result, Ok(true)),
            "expected a Snapshot, got {result:?}"
        );
        let query = query_of(&replay, 1);
        for expected in [
            "prefix=archive%2Fcoffret-0123456789abcdef%2Fidx-",
            "max-keys=1",
        ] {
            assert!(query.contains(expected), "expected {expected:?} in {query}");
        }
    }

    // A key that only starts like a head's is not one, and a listing that says
    // to carry on past it is followed to the head behind it.
    #[tokio::test]
    async fn a_key_that_only_starts_like_a_head_s_is_looked_past() {
        let stray = format!("{PREFIX}head-notes.txt");
        let later = head_key(7);
        let (client, replay) = answering(&[
            (200, &page(&[&stray], Some("after-the-stray"))),
            (200, &page(&[&later], None)),
        ]);

        let result = check_any_head_or_snapshot(&client, BUCKET, PREFIX).await;
        assert!(
            matches!(result, Ok(true)),
            "expected a head, got {result:?}"
        );
        assert!(
            query_of(&replay, 1).contains("continuation-token=after-the-stray"),
            "{}",
            query_of(&replay, 1)
        );
    }

    // And a prefix holding nothing but such a key holds no head.
    #[tokio::test]
    async fn a_prefix_holding_only_a_stray_key_holds_no_head() {
        let stray = format!("{PREFIX}head-notes.txt");
        let (client, _) = answering(&[(200, &page(&[&stray], None)), (200, &page(&[], None))]);

        let result = check_any_head_or_snapshot(&client, BUCKET, PREFIX).await;
        assert!(matches!(result, Ok(false)), "{result:?}");
    }

    // Credentials S3 refused say nothing about whether a head is there, so
    // they must never come back as the `false` that a caller reads as an empty
    // Library.
    #[tokio::test]
    async fn credentials_s3_refused_are_not_an_empty_prefix() {
        let (client, _) = answering(&[(403, "")]);

        let result = check_any_head_or_snapshot(&client, BUCKET, PREFIX).await;
        assert!(
            matches!(&result, Err(Error::PermissionDenied { .. })),
            "expected a refusal of the credentials, got {result:?}"
        );
    }

    // A listing answers an empty prefix with an empty page, so a `404` is about
    // the bucket and never an empty Library.
    #[tokio::test]
    async fn a_bucket_that_is_not_there_is_not_an_empty_prefix() {
        let (client, _) = answering(&[(404, "")]);

        let result = check_any_head_or_snapshot(&client, BUCKET, PREFIX).await;
        assert!(
            matches!(
                &result,
                Err(Error::NotFound {
                    missing: Missing::Listing
                })
            ),
            "expected the listing's location to be missing, got {result:?}"
        );
    }

    /// An S3 that never ends a listing: every page is empty and every one of
    /// them names somewhere else to carry on.
    #[derive(Debug, Clone, Default)]
    struct EndlessListing {
        pages: Arc<AtomicUsize>,
    }

    impl HttpConnector for EndlessListing {
        fn call(&self, _request: HttpRequest) -> HttpConnectorFuture {
            let page_number = self.pages.fetch_add(1, Ordering::SeqCst) + 1;
            let body = page(&[], Some(&format!("page-{page_number}")));
            HttpConnectorFuture::ready(Ok(HttpResponse::new(
                200.try_into().expect("200 is a status"),
                SdkBody::from(body),
            )))
        }
    }

    impl HttpClient for EndlessListing {
        fn http_connector(
            &self,
            _settings: &HttpConnectorSettings,
            _components: &RuntimeComponents,
        ) -> SharedHttpConnector {
            SharedHttpConnector::new(self.clone())
        }
    }

    // Following the continuation is what makes a `false` right, and it is
    // exactly what a provider needs to answer to keep this call going forever.
    #[tokio::test]
    async fn a_listing_that_never_ends_is_refused_rather_than_walked_forever() {
        let endless = EndlessListing::default();
        let client = client_over(endless.clone());

        let result = check_any_head_or_snapshot(&client, BUCKET, PREFIX).await;
        assert!(
            matches!(&result, Err(Error::ListingPastCap { pages, .. }) if *pages == MAX_PAGES),
            "expected the listing to be reported as past its cap, got {result:?}"
        );
        assert_eq!(endless.pages.load(Ordering::SeqCst), MAX_PAGES);
    }

    // Nothing a person typed reaches what is reported: the prefix carries the
    // Library's own name on Storage, and the bucket is their configuration.
    #[tokio::test]
    async fn no_answer_carries_what_it_was_asked_about() {
        for status in [403, 401, 404, 500] {
            let echo = format!(
                "<Error><Code>Refused</Code><Message>{BUCKET}/{PREFIX}head-</Message></Error>"
            );
            let (client, _) = answering(&[(status, &echo)]);
            let error = check_any_head_or_snapshot(&client, BUCKET, PREFIX)
                .await
                .expect_err("the case names a status S3 refuses with");
            let rendered = format!("{error} {error:?}");
            let detail = match &error {
                Error::PermissionDenied { detail, .. }
                | Error::Unauthenticated { detail, .. }
                | Error::ServiceUnavailable { detail, .. } => detail.clone(),
                _ => String::new(),
            };
            assert!(!error.to_string().contains(BUCKET), "{status}: {rendered}");
            assert!(!error.to_string().contains(PREFIX), "{status}: {rendered}");
            assert!(!detail.contains(BUCKET), "{status}: {detail}");
            assert!(
                !detail.contains(PREFIX.trim_end_matches('/')),
                "{status}: {detail}"
            );
        }
    }
}
