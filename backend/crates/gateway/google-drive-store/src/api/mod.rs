//! The Drive REST API, as the gateway uses it.
//!
//! [`DriveApi`] is what every call goes through — the bearer header and the
//! one-refresh answer to a 401 — [`FileResource`], [`FileList`], and
//! [`GeneratedIds`] are the shapes Drive answers in, [`FailedResponse`] is
//! where a refusal becomes one of the port's errors, and [`UploadSession`] is
//! the one place an upload's bytes are sent to.

mod drive_api;
pub use drive_api::{authorization, DriveApi};

mod endpoints;
pub use endpoints::{Endpoints, DRIVE_API};

mod failed_response;
pub use failed_response::FailedResponse;

mod file_list;
pub use file_list::FileList;

mod file_resource;
pub use file_resource::{FileResource, FILE_FIELDS, LIST_FIELDS};

mod generated_ids;
pub use generated_ids::GeneratedIds;

mod live_files_query;
pub use live_files_query::{live_files_query, name_prefixes_query};

mod upload_session;
pub use upload_session::UploadSession;
