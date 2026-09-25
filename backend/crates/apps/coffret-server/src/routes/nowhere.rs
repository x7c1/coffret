use crate::api_error::ApiError;

/// What a path none of the routes is registered at is answered with.
///
/// [`ApiError::no_such_route`] says why it is one kind with the method's answer
/// below, and why neither names what was asked.
pub async fn no_such_route() -> ApiError {
    ApiError::no_such_route()
}

/// What a registered path asked by a method it does not take is answered with.
pub async fn no_such_method() -> ApiError {
    ApiError::no_such_method()
}
