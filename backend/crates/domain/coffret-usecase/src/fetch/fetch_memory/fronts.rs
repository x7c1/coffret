use std::collections::{BTreeSet, VecDeque};
use std::sync::Arc;

use coffret_model::ContainerId;

/// The most Container fronts one memory keeps.
///
/// A reader turns the pages of one book, a fill walks one folder, and a
/// folder's Entries sit in a handful of Containers: a few dozen fronts is past
/// what either reaches before it moves on, and it keeps the bound a count of
/// objects rather than a function of the Library.
const MAX_FRONTS: usize = 64;

/// The most front bytes it keeps over those (spec: FM-2).
///
/// The count above bounds the queue and this bounds what is in it, because a
/// meta section may be as long as the header's ceiling allows — 64 MiB — and a
/// count of those would be a bound in name only. A Pack of a few hundred pages
/// has a front of tens of kilobytes, so this is reached only by fronts that
/// would not have been worth keeping anyway. A single front past it on its own
/// is never kept, and is read again the next time it is wanted.
const MAX_FRONT_BYTES: usize = 64 * 1024 * 1024;

/// The fronts, oldest first.
#[derive(Debug, Default)]
pub(super) struct Fronts {
    queue: VecDeque<(ContainerId, Arc<[u8]>)>,
    bytes: usize,
}

impl Fronts {
    pub(super) fn get(&mut self, container_id: ContainerId) -> Option<Arc<[u8]>> {
        let at = self.queue.iter().position(|(id, _)| *id == container_id)?;
        // Moved to the back, so the front a reader keeps turning pages in is
        // the last one a new Container pushes out.
        let kept = self.queue.remove(at)?;
        let front = Arc::clone(&kept.1);
        self.queue.push_back(kept);
        Some(front)
    }

    pub(super) fn keep(&mut self, container_id: ContainerId, front: Arc<[u8]>) {
        if front.len() > MAX_FRONT_BYTES {
            return;
        }
        if let Some(at) = self.queue.iter().position(|(id, _)| *id == container_id) {
            if let Some((_, old)) = self.queue.remove(at) {
                self.bytes -= old.len();
            }
        }
        while self.queue.len() >= MAX_FRONTS || self.bytes + front.len() > MAX_FRONT_BYTES {
            match self.queue.pop_front() {
                Some((_, old)) => self.bytes -= old.len(),
                None => break,
            }
        }
        self.bytes += front.len();
        self.queue.push_back((container_id, front));
    }

    pub(super) fn retain(&mut self, current: &BTreeSet<ContainerId>) {
        let bytes = &mut self.bytes;
        self.queue.retain(|(id, front)| {
            let keep = current.contains(id);
            if !keep {
                *bytes -= front.len();
            }
            keep
        });
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use coffret_model::ContainerId;

    use super::super::FetchMemory;
    use super::MAX_FRONTS;

    fn container(byte: u8) -> ContainerId {
        ContainerId::from_bytes([byte; ContainerId::BYTE_LEN])
    }

    fn front(len: usize) -> Arc<[u8]> {
        vec![0u8; len].into()
    }

    // The bound is a count of fronts, and the one pushed out is the one least
    // recently asked for: a book a reader is still in keeps its front while
    // the fill walks past others.
    #[test]
    fn the_front_least_recently_asked_for_is_the_one_forgotten() {
        let memory = FetchMemory::default();
        for byte in 0..MAX_FRONTS as u8 {
            memory.remember_front(container(byte), front(32));
        }
        assert!(memory.front(container(0)).is_some());

        memory.remember_front(container(0xff), front(32));

        assert!(memory.front(container(0)).is_some(), "asked for last");
        assert!(memory.front(container(1)).is_none(), "the oldest went");
        assert!(memory.front(container(0xff)).is_some());
    }
}
