use std::array;

// This is what will be sent between client and server.
#[derive(Default, Clone, Debug)]
pub struct WireItem<T: Default + Clone> {
    pub id: u16,
    pub data: T,
}

#[derive(Default, Clone, Debug)]
pub struct Item<T: Default + Clone> {
    pub tick: u64,
    pub data: T,
}

#[derive(Clone, Debug)]
pub struct RingBuffer<T: Default + Clone, const N: usize> {
    array: [Item<T>; N], // N must be a power of 2, as enforced by the constructor.
    mask: usize,         // N - 1.
    head: u64,           // Write cursor: most recent item inserted.
    tail: u64,           // Read cursor: last input processed or last snapshot interpolated.
}

impl<T, const N: usize> RingBuffer<T, N>
where
    T: Default + Clone,
{
    pub fn new() -> Self {
        const {
            assert!(N != 0, "N must not be zero");
            assert!(N.is_power_of_two(), "size must be a power of 2");

            // Thanks to the `i16` cast in extend` (2's complement),
            // we can only unambiguishly distinguish between items in
            // the window [-32_68, +32_767] (2 << 15).
            assert!(
                N <= 1 << 14, // Half the signed horizon: big safety margin.
                "N must be <= 16384 to avoid sequence wrapping ambiguity"
            );
        }

        Self {
            array: array::from_fn(|_| Item::<T>::default()),
            mask: N - 1,
            head: 0,
            tail: 0,
        }
    }

    pub fn insert(&mut self, wire_item: WireItem<T>) {
        let WireItem { id, data } = wire_item;
        if let Some(tick) = self.extend(id) {
            self.head = self.head.max(tick);
            let index = id as usize & self.mask;
            self.array[index] = Item { tick, data };
        }
    }

    pub fn get(&mut self, tick: u64) -> Option<&Item<T>> {
        let index = tick as usize & self.mask;
        let item = &self.array[index];

        if item.tick == tick {
            self.tail = tick;
            Some(item)
        } else {
            None
        }
    }

    pub fn extend(&self, id: u16) -> Option<u64> {
        let head = self.head;
        let head_u16 = head as u16;
        let modular_difference = id.wrapping_sub(head_u16);

        // Cast by 2's complement, so that `modular_difference > (1 << 15)`
        // is negative, allowing us to add `difference` to baseline, to
        // save branching.
        let difference = (modular_difference as i16) as i64;

        head.checked_add_signed(difference)
    }
}
