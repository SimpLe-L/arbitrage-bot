use std::{
    cmp::Ordering,
    collections::{BinaryHeap, HashMap},
    time::{Duration, Instant},
};

use simulator::SimulateCtx;
use ethers::types::{Address, H256};

use crate::types::Source;

pub struct ArbItem {
    pub token: String,
    pub pool_address: Option<Address>,
    pub tx_hash: H256,
    pub sim_ctx: SimulateCtx,
    pub source: Source,
}

impl ArbItem {
    pub fn new(token: String, pool_address: Option<Address>, entry: ArbEntry) -> Self {
        Self {
            token: token.to_string(),
            pool_address,
            tx_hash: entry.hash,
            sim_ctx: entry.sim_ctx,
            source: entry.source,
        }
    }
}

/// The value stored in the HashMap for each token.
pub struct ArbEntry {
    hash: H256,
    sim_ctx: SimulateCtx,
    generation: u64,
    expires_at: Instant,
    source: Source,
}

#[derive(Eq, PartialEq)]
struct HeapItem {
    expires_at: Instant,
    generation: u64,
    token: String,
    pool_address: Option<Address>,
}

impl Ord for HeapItem {
    fn cmp(&self, other: &Self) -> Ordering {
        // Default BinaryHeap is a max-heap, so we invert ordering:
        // We want the earliest expiration at the front, so we compare timestamps reversed.
        self.expires_at
            .cmp(&other.expires_at)
            .then(self.generation.cmp(&other.generation))
            .reverse()
    }
}

impl PartialOrd for HeapItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// A structure to manage ArbItems with uniqueness, reordering, and timed expiration.
pub struct ArbCache {
    map: HashMap<String, ArbEntry>,
    heap: BinaryHeap<HeapItem>,
    generation_counter: u64,
    expiration_duration: Duration,
}

impl ArbCache {
    pub fn new(expiration_duration: Duration) -> Self {
        Self {
            map: HashMap::new(),
            heap: BinaryHeap::new(),
            generation_counter: 0,
            expiration_duration,
        }
    }

    /// Insert or update an ArbItem.
    /// If the token already exists, this updates it with a new generation and expiration time.
    pub fn insert(
        &mut self,
        token: String,
        pool_address: Option<Address>,
        hash: H256,
        sim_ctx: SimulateCtx,
        source: Source,
    ) {
        let now = Instant::now();
        self.generation_counter += 1;
        let generation = self.generation_counter;
        let expires_at = now + self.expiration_duration;

        // Insert into the map
        self.map.insert(
            token.clone(),
            ArbEntry {
                hash,
                sim_ctx,
                generation,
                expires_at,
                source,
            },
        );

        // Insert into the heap
        self.heap.push(HeapItem {
            expires_at,
            generation,
            token,
            pool_address,
        });
    }

    /// Attempt to get an ArbItem by token.
    #[allow(dead_code)]
    pub fn get(&self, token: &str) -> Option<(H256, SimulateCtx)> {
        self.map.get(token).map(|entry| (entry.hash, entry.sim_ctx.clone()))
    }

    /// Periodically call this to remove expired entries.
    /// This will pop from the heap until it finds an entry that is not stale and not expired.
    pub fn remove_expired(&mut self) -> Vec<String> {
        let mut expired_tokens = Vec::new();
        let now = Instant::now();
        while let Some(top) = self.heap.peek() {
            // If top is outdated (stale) or expired, pop it and remove from map if needed
            if let Some(entry) = self.map.get(&top.token) {
                if entry.generation != top.generation {
                    // Stale entry, just discard from heap
                    self.heap.pop();
                    continue;
                }
                // Matching generation
                if entry.expires_at <= now {
                    // It's actually expired
                    expired_tokens.push(top.token.clone());
                    self.map.remove(&top.token);
                    self.heap.pop();
                } else {
                    // The top is not expired and not stale. We can break now.
                    break;
                }
            } else {
                // Token not in map means stale in heap
                self.heap.pop();
            }
        }
        expired_tokens
    }

    pub fn pop_one(&mut self) -> Option<ArbItem> {
        let now = Instant::now();
        // Keep popping until we find a valid, current entry that's not expired.
        while let Some(top) = self.heap.pop() {
            if let Some(entry) = self.map.get(&top.token) {
                if entry.generation == top.generation {
                    // It's the current entry for this token
                    if entry.expires_at > now {
                        // It's valid and not expired. We can remove it and return.
                        let entry = self.map.remove(&top.token).unwrap();
                        return Some(ArbItem::new(top.token, top.pool_address, entry));
                    } else {
                        // It's current but expired, remove it from map and continue.
                        self.map.remove(&top.token);
                    }
                } else {
                    // Stale entry, just continue without touching the map.
                    // Because a newer entry for this token exists.
                }
            } else {
                // The map no longer has this token, meaning it's stale.
                continue;
            }
        }
        // No valid entries were found
        None
    }
}
