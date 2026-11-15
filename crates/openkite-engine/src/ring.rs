//! Fixed-capacity ring buffer: drop-oldest overflow with an observable drop count.
use std::collections::VecDeque;

/// Default item capacity, sized to bound a bursty stream without an unbounded buffer.
pub const DEFAULT_RING_CAPACITY: usize = 4096;

/// Fixed-capacity FIFO that evicts the oldest item on overflow and counts the evictions.
pub struct RingBuffer<T> {
    capacity: usize,
    items: VecDeque<T>,
    dropped: u64,
}

impl<T> RingBuffer<T> {
    /// A ring holding at most `capacity` items; panics if `capacity` is zero.
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "ring buffer capacity must be non-zero");
        Self {
            capacity,
            items: VecDeque::with_capacity(capacity),
            dropped: 0,
        }
    }

    /// Pushes an item, evicting the oldest and incrementing `dropped` if already full.
    pub fn push(&mut self, item: T) {
        if self.items.len() == self.capacity {
            self.items.pop_front();
            self.dropped += 1;
        }
        self.items.push_back(item);
    }

    /// Pops the oldest item, if any.
    pub fn pop(&mut self) -> Option<T> {
        self.items.pop_front()
    }

    /// Number of items currently held.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the buffer holds no items.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The configured maximum number of items.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Total items evicted over this buffer's lifetime.
    pub fn dropped(&self) -> u64 {
        self.dropped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_within_capacity_keeps_everything() {
        let mut ring = RingBuffer::new(4);
        for item in 0..4 {
            ring.push(item);
        }
        assert_eq!(ring.len(), 4);
        assert_eq!(ring.dropped(), 0);
    }

    #[test]
    fn overflow_drops_oldest_first() {
        let mut ring = RingBuffer::new(3);
        for item in 0..5 {
            ring.push(item);
        }
        assert_eq!(ring.len(), 3);
        assert_eq!(ring.dropped(), 2);
        assert_eq!(ring.pop(), Some(2));
        assert_eq!(ring.pop(), Some(3));
        assert_eq!(ring.pop(), Some(4));
        assert_eq!(ring.pop(), None);
    }

    #[test]
    fn drop_count_is_observable_and_monotonic() {
        let mut ring = RingBuffer::new(2);
        ring.push(1);
        ring.push(2);
        assert_eq!(ring.dropped(), 0);
        ring.push(3);
        assert_eq!(ring.dropped(), 1);
        ring.push(4);
        assert_eq!(ring.dropped(), 2);
    }

    #[test]
    fn a_5mb_per_second_burst_never_grows_past_capacity() {
        // ~1 KiB lines at 5 MB/s => ~5000 pushes/sec; a far slower consumer never grows memory.
        let capacity = 4096;
        let mut ring: RingBuffer<Vec<u8>> = RingBuffer::new(capacity);
        let line = vec![0u8; 1024];
        let produced = 5_000usize;
        for _ in 0..produced {
            ring.push(line.clone());
            assert!(ring.len() <= capacity);
        }
        assert_eq!(ring.len(), capacity);
        assert_eq!(ring.dropped(), (produced - capacity) as u64);
    }

    #[test]
    #[should_panic(expected = "non-zero")]
    fn zero_capacity_is_rejected() {
        let _: RingBuffer<u8> = RingBuffer::new(0);
    }
}
