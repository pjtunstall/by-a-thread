use std::array;

// This is what will be sent between client and server.
#[derive(Default, Clone, Debug)]
pub struct Item<T: Default + Clone> {
    pub sequence_number: u16,
    pub data: T,
}

#[derive(Clone, Debug)]
pub struct RingBuffer<T: Default + Clone, const N: usize> {
    array: [Item<T>; N], // N must be a power of 2, as enforced by the constructor.
    mask: usize,         // N - 1.
    top: u64,            // Highest sequence number received so far.
}

impl<T, const N: usize> RingBuffer<T, N>
where
    T: Default + Clone,
{
    pub fn new() -> Self {
        const {
            assert!(N != 0, "N must not be zero");
            assert!(N.is_power_of_two(), "size must be a power of 2");
        }

        Self {
            array: array::from_fn(|_| Item::<T>::default()),
            mask: N - 1,
            top: 0,
        }
    }

    pub fn insert(&mut self, item: Item<T>) {
        let extended_id = self.extend(item.sequence_number);
        if extended_id > self.top {
            self.top = extended_id;
        }

        let index = item.sequence_number as usize & self.mask;
        self.array[index] = item;
    }

    pub fn get(&self, sequence_number: u16) -> Option<&Item<T>> {
        let index = sequence_number as usize & self.mask;
        let item = &self.array[index];

        // Don't return old data if the buffer has wrapped around,
        // i.e. if the ticks don't match mod (1 << 16).
        if item.sequence_number == sequence_number {
            Some(item)
        } else {
            None
        }
    }

    pub fn extend(&self, received_u16: u16) -> u64 {
        let top_u64 = self.top;
        let top_u16 = top_u64 as u16;

        let unsigned_difference = received_u16.wrapping_sub(top_u16);
        let difference = (unsigned_difference as i16) as i64; // Large unsigned -> small negative signed.

        top_u64.wrapping_add(difference as u64)
    }
}
