use std::array;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct WireItem<T: Clone + Default + PartialEq> {
    pub id: u16,
    pub data: T,
}

#[derive(Clone, Debug, Default)]
pub struct StoredItem<T: Clone + Default> {
    pub tick: u64,
    pub data: T,
}

// `WireItem` ids are unambiguous within ±2^15 ticks (~9.1 min) (see `extend`);
// choose `N` well below that.
#[derive(Clone, Debug)]
pub struct Ring<T: Clone + Default, const N: usize> {
    array: [StoredItem<T>; N], // N must be a power of 2, as enforced by the constructor.
    mask: usize,               // N - 1.
}

impl<T, const N: usize> Ring<T, N>
where
    T: Clone + Default + PartialEq,
{
    pub fn new() -> Self {
        const {
            assert!(N != 0, "length must not be zero");
            assert!(N.is_power_of_two(), "length must be a power of 2");
        }

        Self {
            array: array::from_fn(|_| StoredItem::<T>::default()),
            mask: N - 1,
        }
    }

    #[inline(always)]
    pub fn get(&self, tick: u64) -> Option<&T> {
        let index = tick as usize & self.mask;
        let item = &self.array[index];
        if item.tick == tick {
            Some(&item.data)
        } else {
            None
        }
    }

    #[inline(always)]
    pub fn insert(&mut self, tick: u64, data: T) {
        let index = tick as usize & self.mask;
        self.array[index] = StoredItem { tick, data };
    }

    #[inline(always)]
    pub fn peek_tick(&self, tick: u64) -> u64 {
        let index = tick as usize & self.mask;
        self.array[index].tick
    }
}

#[derive(Clone, Debug)]
pub struct NetworkBuffer<T: Clone + Default, const N: usize> {
    ring: Ring<T, N>,
    pub head: u64, // The "write" cursor: most recent item inserted.
    pub tail: u64, // The "read" cursor: older items will be considered stale.
}

impl<T, const N: usize> NetworkBuffer<T, N>
where
    T: Clone + Default + PartialEq,
{
    pub fn new(head: u64, tail: u64) -> Self {
        Self {
            ring: Ring::<T, N>::new(),
            head,
            tail,
        }
    }

    pub fn insert(&mut self, wire_item: WireItem<T>) {
        let WireItem { id, data } = wire_item;
        if let Some(tick) = self.extend(id) {
            // No need to insert if we've already processed the data for that
            // tick. The server can extract the most-recently processed input
            // for each of the players and store them separately.
            if tick <= self.tail {
                return;
            }

            // Only overwrite if new data is from a more recent tick than what's
            // already stored here.
            if self.ring.peek_tick(tick) < tick {
                self.ring.insert(tick, data);

                // Update the head if the new item is more recent.
                self.head = self.head.max(tick);
            }
        }
    }

    pub fn insert_first_item(&mut self, wire_item: WireItem<T>) {
        let WireItem { id, data } = wire_item;
        if let Some(tick) = self.extend(id) {
            self.ring.insert(tick, data);
            self.head = tick;
        }
    }

    pub fn get(&self, tick: u64) -> Option<&T> {
        self.ring.get(tick)
    }

    // Map a 16-bit wire id to the closest plausible 64-bit tick near the
    // current head, handling wrap-around. In case of overflow, return None.
    fn extend(&self, id: u16) -> Option<u64> {
        let head = self.head;
        let head_u16 = head as u16;
        let modular_difference = id.wrapping_sub(head_u16);

        // Cast by 2's complement, so that `modular_difference` is negative when
        // greater than 2^15, allowing us to cast it to `i16` and add the
        // resulting signed `difference` to baseline. This saves us some
        // conditional branching. The trick assumes the id is within 2^15 ticks
        // (~9.1 minutes at 60Hz) of the head. If the item is an old one from
        // 9.1-18.2 minutes after the head, it will be interpreted as up to 9.1
        // minutes before the head.
        let difference = (modular_difference as i16) as i64;

        head.checked_add_signed(difference)
    }

    pub fn advance_tail(&mut self, new_tail: u64) {
        self.tail = self.tail.max(new_tail);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_returns_data_for_matching_tick() {
        let mut ring = Ring::<u32, 8>::new();

        ring.insert(3, 10);

        assert_eq!(ring.get(3), Some(&10));
        assert_eq!(ring.get(4), None);
    }

    #[test]
    fn ring_replaces_slot_with_more_recent_tick() {
        let mut ring = Ring::<u32, 8>::new();

        ring.insert(1, 7);
        ring.insert(9, 42);

        assert_eq!(ring.get(1), None);
        assert_eq!(ring.get(9), Some(&42));
    }

    #[test]
    fn network_buffer_does_not_overwrite_with_older_tick_at_same_index() {
        let mut buffer = NetworkBuffer::<u32, 8>::new(0, 0);

        buffer.insert(WireItem { id: 2, data: 1 });
        buffer.insert(WireItem { id: 10, data: 2 });

        buffer.insert(WireItem { id: 2, data: 3 });

        assert_eq!(buffer.get(10), Some(&2));
        assert_eq!(buffer.get(2), None);
        assert_eq!(buffer.head, 10);
    }

    #[test]
    fn network_buffer_replaces_slot_with_newer_tick() {
        let mut buffer = NetworkBuffer::<u32, 8>::new(0, 0);

        buffer.insert(WireItem { id: 1, data: 1 });
        buffer.insert(WireItem { id: 9, data: 2 });

        assert_eq!(buffer.get(1), None);
        assert_eq!(buffer.get(9), Some(&2));
        assert_eq!(buffer.head, 9);
    }

    #[test]
    fn network_buffer_drops_ticks_at_or_before_tail() {
        let mut buffer = NetworkBuffer::<u32, 8>::new(0, 0);

        buffer.insert(WireItem { id: 12, data: 99 });
        buffer.advance_tail(12);

        buffer.insert(WireItem { id: 4, data: 7 });

        assert_eq!(buffer.get(4), None);
        assert_eq!(buffer.get(12), Some(&99));
        assert_eq!(buffer.head, 12);
    }

    #[test]
    fn extend_handles_wraparound_and_overflow() {
        let mut buffer = NetworkBuffer::<u8, 8>::new(0, 0);

        buffer.head = 65_000;
        assert_eq!(buffer.extend(64_000), Some(64_000));

        buffer.head = u64::MAX - 1;
        assert_eq!(buffer.extend(4), None);
    }
}
