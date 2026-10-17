use std::collections::{HashSet, VecDeque};

/// Bounded memory of recently seen event ids, used to drop at-least-once duplicates.
#[derive(Debug)]
pub struct RecentIds {
    capacity: usize,
    order: VecDeque<String>,
    seen: HashSet<String>,
}

impl RecentIds {
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        Self {
            capacity,
            order: VecDeque::with_capacity(capacity),
            seen: HashSet::with_capacity(capacity),
        }
    }

    pub fn contains(&self, id: &str) -> bool {
        self.seen.contains(id)
    }

    /// Returns `true` the first time an id is seen, `false` for a duplicate.
    pub fn first_sighting(&mut self, id: &str) -> bool {
        if self.seen.contains(id) {
            return false;
        }
        if self.order.len() == self.capacity
            && let Some(oldest) = self.order.pop_front()
        {
            self.seen.remove(&oldest);
        }
        self.order.push_back(id.to_owned());
        self.seen.insert(id.to_owned());
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_id_is_a_first_sighting() {
        let mut ids = RecentIds::new(3);
        assert!(ids.first_sighting("a"));
    }

    #[test]
    fn a_repeated_id_is_a_duplicate() {
        let mut ids = RecentIds::new(3);
        ids.first_sighting("a");
        assert!(!ids.first_sighting("a"));
    }

    #[test]
    fn the_oldest_id_is_forgotten_beyond_capacity() {
        let mut ids = RecentIds::new(2);
        ids.first_sighting("a");
        ids.first_sighting("b");
        ids.first_sighting("c");
        assert!(ids.first_sighting("a"));
        assert!(!ids.first_sighting("c"));
    }

    #[test]
    fn contains_does_not_record_the_id() {
        let mut ids = RecentIds::new(2);
        assert!(!ids.contains("a"));
        assert!(ids.first_sighting("a"));
        assert!(ids.contains("a"));
    }

    #[test]
    fn zero_capacity_is_raised_to_one() {
        let mut ids = RecentIds::new(0);
        assert!(ids.first_sighting("a"));
        assert!(!ids.first_sighting("a"));
    }
}
