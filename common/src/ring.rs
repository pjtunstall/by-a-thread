use std::array;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WireItem<T: Clone + Default> {
    pub id: u16,
    pub data: T,
}

#[derive(Clone, Debug, Default)]
pub struct StoredItem<T: Clone + Default> {
    pub tick: u64,
    pub data: T,
}

#[derive(Clone, Debug)]
pub struct Ring<T: Clone + Default, const N: usize> {
    array: [StoredItem<T>; N], // N must be a power of 2, as enforced by the constructor.
    mask: usize,               // N - 1.
}

impl<T, const N: usize> Ring<T, N>
where
    T: Clone + Default,
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
    fn get(&self, tick: u64) -> Option<&T> {
        let index = tick as usize & self.mask;
        let item = &self.array[index];
        if item.tick == tick {
            Some(&item.data)
        } else {
            None
        }
    }

    #[inline(always)]
    fn insert(&mut self, tick: u64, data: T) {
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
    head: u64, // The "write" cursor: most recent item inserted.
    tail: u64, // The "read" cursor: last input processed or last snapshot interpolated.
}

impl<T, const N: usize> NetworkBuffer<T, N>
where
    T: Clone + Default,
{
    pub fn new() -> Self {
        const {
            // Thanks to the `i16` cast in extend (2's complement),
            // we can only unambiguishly distinguish between items in
            // the window [-32_68, +32_767] (2 << 15).
            assert!(
                N <= 1 << 14, // Half the signed horizon: big safety margin.
                "N must be <= 16384 to avoid sequence wrapping ambiguity"
            );
        }

        Self {
            ring: Ring::<T, N>::new(),
            head: 0,
            tail: 0,
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

    pub fn get(&self, tick: u64) -> Option<&T> {
        self.ring.get(tick)
    }

    pub fn extend(&self, id: u16) -> Option<u64> {
        let head = self.head;
        let head_u16 = head as u16;
        let modular_difference = id.wrapping_sub(head_u16);

        // Cast by 2's complement, so that `modular_difference` is negative when
        // greater than (1 << 15), allowing us to cast it to i16 and add the
        // resulting signed `difference` to baseline. This saves us some
        // conditional branching.
        let difference = (modular_difference as i16) as i64;

        head.checked_add_signed(difference)
    }

    pub fn advance_tail(&mut self, new_tail: u64) {
        self.tail = self.tail.max(new_tail);
    }
}
