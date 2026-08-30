use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, Mutex, MutexGuard};

use tokio::sync::Mutex as AsyncMutex;

/// One turn at a time, per key.
///
/// A plain map of locks would grow for the life of the process: every key ever
/// waited on would keep a lock nobody is holding. So the map keeps a
/// [`Weak`](std::sync::Weak) reference and the waiters keep the strong ones,
/// which makes the map an index of what is in flight rather than a record of
/// everything that ever was. The last caller to let go drops the lock, and the
/// dead reference is swept on the way out.
///
/// The map's own lock is a synchronous one and is never held across an await:
/// what is awaited is the per-key lock, which is taken after the map's has been
/// let go.
#[derive(Debug)]
pub(super) struct Gates<K> {
    in_flight: Mutex<HashMap<K, std::sync::Weak<AsyncMutex<()>>>>,
}

impl<K> Default for Gates<K> {
    fn default() -> Self {
        Self {
            in_flight: Mutex::new(HashMap::new()),
        }
    }
}

impl<K: Clone + Eq + Hash> Gates<K> {
    /// Waits for whoever holds `key` and takes it.
    ///
    /// The turn lasts as long as the returned value: dropping it lets the next
    /// caller in and takes the key out of the map once nobody is waiting for it.
    pub(super) async fn take(&self, key: K) -> Turn<'_, K> {
        let gate = {
            let mut in_flight = self.locked();
            match in_flight.get(&key).and_then(std::sync::Weak::upgrade) {
                Some(gate) => gate,
                None => {
                    let gate = Arc::new(AsyncMutex::new(()));
                    in_flight.insert(key.clone(), Arc::downgrade(&gate));
                    gate
                }
            }
        };

        // Awaited with the map's lock let go, which is the whole point of the
        // two of them: a caller waiting its turn on one key must not stop
        // callers on every other key from finding theirs.
        let held = gate.lock_owned().await;
        Turn {
            gates: self,
            key,
            held: Some(held),
        }
    }

    fn locked(&self) -> MutexGuard<'_, HashMap<K, std::sync::Weak<AsyncMutex<()>>>> {
        // A caller that panicked while holding it had done nothing to the map
        // but read or insert one entry, so what is behind it is still a map.
        self.in_flight
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Forgets a key nobody is waiting on any more.
    fn sweep(&self, key: &K) {
        let mut in_flight = self.locked();
        if in_flight
            .get(key)
            .is_some_and(|gate| gate.strong_count() == 0)
        {
            in_flight.remove(key);
        }
    }
}

/// One caller's turn at one key, given up when it is dropped.
#[derive(Debug)]
pub(super) struct Turn<'a, K: Clone + Eq + Hash> {
    gates: &'a Gates<K>,
    key: K,
    /// The lock, and the only strong reference this caller holds to it — an
    /// owned guard keeps the [`Arc`] alive by itself. In an [`Option`] so that
    /// [`drop`] can let go of it before `sweep` counts who is still interested.
    held: Option<tokio::sync::OwnedMutexGuard<()>>,
}

impl<K: Clone + Eq + Hash> Drop for Turn<'_, K> {
    fn drop(&mut self) {
        drop(self.held.take());
        self.gates.sweep(&self.key);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use super::Gates;

    /// Takes a key, stays inside for a moment, and records how many callers
    /// were inside at once while it was.
    ///
    /// The high-water mark is the whole of what these cases assert: it is one
    /// where the gate served the callers in turn and two where it let them
    /// overlap.
    async fn visit(gates: &Gates<String>, key: &str, inside: &AtomicUsize, most: &AtomicUsize) {
        let _turn = gates.take(key.to_owned()).await;
        let now = inside.fetch_add(1, Ordering::SeqCst) + 1;
        most.fetch_max(now, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(10)).await;
        inside.fetch_sub(1, Ordering::SeqCst);
    }

    // What single-flight is for: two callers naming one key do the work one
    // after another rather than both at once.
    #[tokio::test]
    async fn one_key_admits_one_caller_at_a_time() {
        let gates = Gates::<String>::default();
        let inside = AtomicUsize::new(0);
        let most = AtomicUsize::new(0);

        tokio::join!(
            visit(&gates, "albums/spring.jpg", &inside, &most),
            visit(&gates, "albums/spring.jpg", &inside, &most),
        );
        assert_eq!(most.load(Ordering::SeqCst), 1);
    }

    // And what it is not for: a reader opening two pages at once is two keys,
    // and neither may wait on the other. Both callers are inside before either
    // leaves, which a gate over one key at a time could not do.
    #[tokio::test]
    async fn two_keys_do_not_wait_on_each_other() {
        let gates = Gates::<String>::default();
        let inside = AtomicUsize::new(0);
        let most = AtomicUsize::new(0);

        tokio::join!(
            visit(&gates, "albums/spring.jpg", &inside, &most),
            visit(&gates, "albums/summer.jpg", &inside, &most),
        );
        assert_eq!(most.load(Ordering::SeqCst), 2);
    }

    // The map is what is in flight and never a history of it: a process serving
    // a reader that scrolls through ten thousand pages must not end up holding
    // ten thousand locks nobody is waiting on.
    #[tokio::test]
    async fn a_key_nobody_holds_is_forgotten() {
        let gates = Gates::<String>::default();
        {
            let _turn = gates.take("albums/spring.jpg".to_owned()).await;
            assert_eq!(gates.locked().len(), 1);
        }
        assert!(gates.locked().is_empty());
    }
}
