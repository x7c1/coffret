use crate::state::ServerState;

/// Removes what the writers of a process that is gone left in the mapped
/// folders under coffret's scratch names, as the server opens the Library and
/// before it serves anything (spec: OC-8).
///
/// Above all the file an add kept aside before writing over another (spec:
/// LA-36), which nobody holds once the server that carried the add is gone.
/// Before the first request, so that no add of this server holds a name yet.
///
/// Never fatal: a walk that could not go on is reported, and the next sync
/// meets what is left. A locked Library — which a server that has just opened
/// one is not — has nothing walked.
pub async fn remove_left_behind(state: &ServerState) {
    if let Ok(library) = state.unlocked_unattended() {
        library.remove_left_behind().await;
    }
}
