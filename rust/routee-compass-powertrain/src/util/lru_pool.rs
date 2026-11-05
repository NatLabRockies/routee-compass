use std::collections::{HashMap, VecDeque};
use std::hash::Hash;

/// Outcome of checking a resource back into the [`LruResourcePool`].
#[derive(Debug, PartialEq, Eq)]
pub enum CheckinOutcome<K, V> {
    /// Resource was stored without needing to evict any existing resource.
    Stored,
    /// Resource was stored after evicting the least-recently-used `(key, value)`.
    Evicted(K, V),
    /// Pool has `max_capacity == 0`, so the resource was immediately dropped.
    Dropped(V),
}

/// Single-threaded LRU multi-value table supporting dynamically-allocated access to a
/// pool of some resource type `V`. Evicts a value `V` for the least-recently-used key to
/// maintain `max_capacity` at all times.
pub struct LruResourcePool<K, V> {
    count: usize,
    max_capacity: usize,
    idle_by_key: HashMap<K, VecDeque<V>>,
    lru_order: VecDeque<K>,
}

impl<K: Eq + Hash + Clone, V> LruResourcePool<K, V> {
    /// Create a new, empty pool with the given max size.
    pub fn new(max_capacity: usize) -> Self {
        Self {
            count: 0,
            max_capacity,
            idle_by_key: HashMap::new(),
            lru_order: VecDeque::new(),
        }
    }

    /// Current number of idle resources in the pool across all keys.
    pub fn len(&self) -> usize {
        self.count
    }

    /// Returns `true` if there are no idle resources in the pool.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Maximum number of idle resources the pool can hold.
    pub fn max_capacity(&self) -> usize {
        self.max_capacity
    }

    /// Get the most-recently checked-in resource matching the given key.
    /// If no resource is available for `key`, returns `None`.
    pub fn checkout(&mut self, key: &K) -> Option<V> {
        let queue = self.idle_by_key.get_mut(key)?;
        let item = queue.pop_back()?;
        if queue.is_empty() {
            self.idle_by_key.remove(key);
        }
        self.count -= 1;
        if let Some(pos) = self.lru_order.iter().rposition(|k| k == key) {
            self.lru_order.remove(pos);
        }
        Some(item)
    }

    /// Checks a resource back into the pool (or pushes a newly created one).
    /// Returns [`CheckinOutcome`] indicating whether an older item was evicted.
    pub fn checkin(&mut self, key: K, item: V) -> CheckinOutcome<K, V> {
        if self.max_capacity == 0 {
            return CheckinOutcome::Dropped(item);
        }

        let mut evicted = None;
        while self.count >= self.max_capacity {
            if let Some(oldest_key) = self.lru_order.pop_front() {
                if let Some(q) = self.idle_by_key.get_mut(&oldest_key) {
                    if let Some(oldest_val) = q.pop_front() {
                        self.count -= 1;
                        evicted = Some((oldest_key.clone(), oldest_val));
                    }
                    if q.is_empty() {
                        self.idle_by_key.remove(&oldest_key);
                    }
                }
            } else {
                break;
            }
        }

        self.idle_by_key
            .entry(key.clone())
            .or_default()
            .push_back(item);
        self.lru_order.push_back(key);
        self.count += 1;

        match evicted {
            Some((k, v)) => CheckinOutcome::Evicted(k, v),
            None => CheckinOutcome::Stored,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checkout_empty_and_miss() {
        let mut pool: LruResourcePool<&str, i32> = LruResourcePool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.max_capacity(), 4);
        assert_eq!(pool.checkout(&"missing"), None);

        assert_eq!(pool.checkin("a", 1), CheckinOutcome::Stored);
        assert_eq!(pool.checkout(&"missing"), None);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn test_checkin_and_checkout_lifo_per_key() {
        let mut pool = LruResourcePool::new(4);
        assert_eq!(pool.checkin("a", 10), CheckinOutcome::Stored);
        assert_eq!(pool.checkin("a", 20), CheckinOutcome::Stored);
        assert_eq!(pool.len(), 2);

        assert_eq!(pool.checkout(&"a"), Some(20));
        assert_eq!(pool.len(), 1);
        assert_eq!(pool.checkout(&"a"), Some(10));
        assert_eq!(pool.len(), 0);
        assert!(pool.is_empty());
        assert!(!pool.idle_by_key.contains_key(&"a"));
        assert_eq!(pool.checkout(&"a"), None);
    }

    #[test]
    fn test_lru_eviction_across_keys() {
        let mut pool = LruResourcePool::new(2);
        assert_eq!(pool.checkin("a", 1), CheckinOutcome::Stored);
        assert_eq!(pool.checkin("b", 2), CheckinOutcome::Stored);
        assert_eq!(pool.len(), 2);

        // Checking in "c" should evict the oldest ("a", 1)
        assert_eq!(pool.checkin("c", 3), CheckinOutcome::Evicted("a", 1));
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.checkout(&"a"), None);
        assert_eq!(pool.checkout(&"b"), Some(2));
        assert_eq!(pool.checkout(&"c"), Some(3));
    }

    #[test]
    fn test_checkout_refreshes_recency() {
        let mut pool = LruResourcePool::new(2);
        assert_eq!(pool.checkin("a", 1), CheckinOutcome::Stored);
        assert_eq!(pool.checkin("b", 2), CheckinOutcome::Stored);

        // Checkout "a" and check it back in; now "b" is the least recently used
        let a_val = pool.checkout(&"a").unwrap();
        assert_eq!(pool.checkin("a", a_val), CheckinOutcome::Stored);

        // Checking in "c" should evict ("b", 2) instead of ("a", 1)
        assert_eq!(pool.checkin("c", 3), CheckinOutcome::Evicted("b", 2));
        assert_eq!(pool.checkout(&"b"), None);
        assert_eq!(pool.checkout(&"a"), Some(1));
        assert_eq!(pool.checkout(&"c"), Some(3));
    }

    #[test]
    fn test_multi_instance_eviction_order() {
        let mut pool = LruResourcePool::new(3);
        assert_eq!(pool.checkin("a", 1), CheckinOutcome::Stored);
        assert_eq!(pool.checkin("b", 10), CheckinOutcome::Stored);
        assert_eq!(pool.checkin("a", 2), CheckinOutcome::Stored);

        // Oldest entry in lru_order is the first "a" (value 1)
        assert_eq!(pool.checkin("c", 100), CheckinOutcome::Evicted("a", 1));
        assert_eq!(pool.len(), 3);

        // Second "a" (value 2) and "b" (value 10) should still be present
        assert_eq!(pool.checkout(&"a"), Some(2));
        assert_eq!(pool.checkout(&"b"), Some(10));
        assert_eq!(pool.checkout(&"c"), Some(100));
    }

    #[test]
    fn test_zero_capacity_drops_immediately() {
        let mut pool = LruResourcePool::new(0);
        assert_eq!(pool.max_capacity(), 0);
        assert_eq!(pool.checkin("a", 42), CheckinOutcome::Dropped(42));
        assert!(pool.is_empty());
        assert_eq!(pool.checkout(&"a"), None);
    }
}
