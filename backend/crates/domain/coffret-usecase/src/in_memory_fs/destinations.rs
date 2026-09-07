use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;

use crate::descent_error::DescentError;
use crate::destination::Destination;
use crate::destinations::Destinations;
use crate::in_memory_fs::in_memory_destination::InMemoryDestination;
use crate::in_memory_fs::InMemoryFs;
use crate::standing::Standing;

#[async_trait]
impl Destinations for InMemoryFs {
    async fn reach(
        &self,
        root: &Path,
        components: &[String],
    ) -> Result<Box<dyn Destination>, DescentError> {
        let (name, folders) = split(components);
        let mut state = self.state();
        let folder = state.reach(root, folders)?;
        drop(state);
        Ok(Box::new(InMemoryDestination::new(
            Arc::clone(&self.state),
            folder,
            name.clone(),
        )))
    }

    async fn look_up(
        &self,
        root: &Path,
        components: &[String],
    ) -> Result<Option<Standing>, DescentError> {
        let (name, folders) = split(components);
        let state = self.state();
        let Some(folder) = state.walk(root, folders)? else {
            return Ok(None);
        };
        Ok(state.standing(&folder.join(name)))
    }
}

/// The file's own name and the folders above it.
///
/// A translated place always has at least one component — it is a mapping's
/// local root with the Entry Path's components below the prefix pushed onto it,
/// and an Entry standing at exactly the prefix is refused before a place is made
/// at all (spec: EP-9). So the split is an assertion rather than a question, and
/// the real filesystem's descent makes the same one.
fn split(components: &[String]) -> (&String, &[String]) {
    components
        .split_last()
        .expect("a place under a mapped root names at least the file itself")
}
