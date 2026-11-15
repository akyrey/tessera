//! Short-lived server-side state for WebAuthn ceremonies.
//!
//! The app runs as a single replica, so an in-memory map is enough; a pod
//! restart mid-ceremony only means the user taps the button again. The store
//! is bounded overall and per client, so one client (or a few) cannot fill it
//! and lock everyone else out of signing in.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use uuid::Uuid;

pub const CEREMONY_TTL: Duration = Duration::from_secs(5 * 60);
pub const MAX_PENDING: usize = 512;
/// A real user has one or two prompts open at a time.
pub const MAX_PENDING_PER_CLIENT: usize = 8;

struct Entry<T> {
    created: Instant,
    client: IpAddr,
    state: T,
}

pub struct CeremonyStore<T> {
    ttl: Duration,
    capacity: usize,
    per_client: usize,
    entries: Mutex<HashMap<Uuid, Entry<T>>>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct StoreFull;

impl<T> CeremonyStore<T> {
    pub fn new(ttl: Duration, capacity: usize, per_client: usize) -> Self {
        Self {
            ttl,
            capacity,
            per_client,
            entries: Mutex::new(HashMap::new()),
        }
    }

    /// Stores `state` for `client` and returns the id the client must send back.
    pub fn insert(&self, client: IpAddr, state: T) -> Result<Uuid, StoreFull> {
        let now = Instant::now();
        let mut entries = self.lock();
        let ttl = self.ttl;
        entries.retain(|_, entry| now.duration_since(entry.created) < ttl);
        let pending_for_client = entries.values().filter(|e| e.client == client).count();
        if entries.len() >= self.capacity || pending_for_client >= self.per_client {
            return Err(StoreFull);
        }
        let id = Uuid::new_v4();
        entries.insert(
            id,
            Entry {
                created: now,
                client,
                state,
            },
        );
        Ok(id)
    }

    /// Removes and returns the state; each ceremony can be finished at most once.
    pub fn take(&self, id: Uuid) -> Option<T> {
        let entry = self.lock().remove(&id)?;
        (entry.created.elapsed() < self.ttl).then_some(entry.state)
    }

    pub fn purge_expired(&self) {
        let now = Instant::now();
        let ttl = self.ttl;
        self.lock()
            .retain(|_, entry| now.duration_since(entry.created) < ttl);
    }

    pub fn len(&self) -> usize {
        self.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<Uuid, Entry<T>>> {
        // A panic while holding the lock cannot leave the map inconsistent.
        self.entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::*;

    const ALICE: IpAddr = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 1));
    const MALLORY: IpAddr = IpAddr::V4(Ipv4Addr::new(198, 51, 100, 7));

    fn store<T>(ttl: Duration, capacity: usize) -> CeremonyStore<T> {
        CeremonyStore::new(ttl, capacity, MAX_PENDING_PER_CLIENT)
    }

    #[test]
    fn take_returns_state_once() {
        let store = store(CEREMONY_TTL, 4);
        let id = store.insert(ALICE, "state").unwrap();

        assert_eq!(store.take(id), Some("state"));
        assert_eq!(store.take(id), None);
    }

    #[test]
    fn unknown_id_returns_nothing() {
        let store: CeremonyStore<()> = store(CEREMONY_TTL, 4);
        assert_eq!(store.take(Uuid::new_v4()), None);
    }

    #[test]
    fn expired_state_is_not_returned() {
        let store = store(Duration::ZERO, 4);
        let id = store.insert(ALICE, 1).unwrap();
        assert_eq!(store.take(id), None);
    }

    #[test]
    fn full_store_rejects_new_ceremonies() {
        let store = store(CEREMONY_TTL, 2);
        store.insert(ALICE, 1).unwrap();
        store.insert(MALLORY, 2).unwrap();
        assert_eq!(store.insert(ALICE, 3), Err(StoreFull));
    }

    #[test]
    fn full_store_evicts_expired_entries_first() {
        let store = store(Duration::ZERO, 1);
        store.insert(ALICE, 1).unwrap();
        assert!(store.insert(ALICE, 2).is_ok());
    }

    #[test]
    fn one_client_cannot_crowd_out_others() {
        let store = store(CEREMONY_TTL, MAX_PENDING);
        for n in 0..MAX_PENDING_PER_CLIENT {
            store.insert(MALLORY, n).unwrap();
        }

        assert_eq!(store.insert(MALLORY, 99), Err(StoreFull));
        assert!(store.insert(ALICE, 1).is_ok());
    }

    #[test]
    fn finishing_a_ceremony_frees_the_client_slot() {
        let store = store(CEREMONY_TTL, MAX_PENDING);
        let ids: Vec<_> = (0..MAX_PENDING_PER_CLIENT)
            .map(|n| store.insert(ALICE, n).unwrap())
            .collect();

        store.take(ids[0]);

        assert!(store.insert(ALICE, 99).is_ok());
    }

    #[test]
    fn purge_drops_expired_entries() {
        let store = store(Duration::ZERO, 4);
        store.insert(ALICE, 1).unwrap();
        store.purge_expired();
        assert!(store.is_empty());
    }
}
