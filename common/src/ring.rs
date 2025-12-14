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
    baseline: u64,       // Tick of the last item applied.
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
            baseline: 0,
        }
    }

    // Do I want insert to overwrite unprocessed items?
    pub fn insert(&mut self, item: Item<T>) {
        let sequence_number = item.sequence_number;
        if let Some(_) = self.extend(sequence_number) {
            let index = sequence_number as usize & self.mask;
            self.array[index] = item;
        }
    }

    pub fn get(&self, sequence_number: u16) -> Option<&Item<T>> {
        let index = sequence_number as usize & self.mask;
        let item = &self.array[index];

        if item.sequence_number == sequence_number {
            Some(item)
        } else {
            None
        }
    }

    pub fn update_baseline(&mut self, sequence_number: u16) {
        if let Some(extended) = self.extend(sequence_number) {
            self.baseline = self.baseline.max(extended);
        }
    }

    pub fn extend(&self, sequence_number: u16) -> Option<u64> {
        let baseline = self.baseline;
        let baseline_u16 = baseline as u16;
        let modular_difference = sequence_number.wrapping_sub(baseline_u16); // mod (1 << 16).

        // Cast by 2's complement, so that `modular_difference > 0x8000`
        // is negative, allowing us to add `difference` to baseline, saving
        // ourselves some conditional branching.
        let difference = (modular_difference as i16) as i64;

        baseline.checked_add_signed(difference)
    }
}
