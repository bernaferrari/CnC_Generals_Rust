//! UniqueArrayClass implementation (ported from WWLib uarray.h).

use crate::hashcalc::HashCalculator;
use crate::vector_class::DynamicVectorClass;

const NO_ITEM: i32 = -1;

#[derive(Clone, Debug)]
struct HashItem<T: Clone> {
    item: T,
    next_hash_index: i32,
}

impl<T: Clone + Default> Default for HashItem<T> {
    fn default() -> Self {
        Self {
            item: T::default(),
            next_hash_index: NO_ITEM,
        }
    }
}

/// Dynamic array of unique items backed by a hash table.
pub struct UniqueArrayClass<T, H>
where
    T: Clone + Default,
    H: HashCalculator<T>,
{
    unique_items: DynamicVectorClass<HashItem<T>>,
    hash_table_size: usize,
    hash_table: Vec<i32>,
    hash_calculator: H,
}

impl<T, H> UniqueArrayClass<T, H>
where
    T: Clone + Default,
    H: HashCalculator<T>,
{
    pub fn new(initial_size: i32, growth_rate: i32, mut hasher: H) -> Self {
        let bits = hasher.num_hash_bits();
        assert!(bits > 0);
        assert!(bits < 24);

        let hash_table_size = 1usize << (bits as usize);
        let mut hash_table = Vec::with_capacity(hash_table_size);
        hash_table.resize(hash_table_size, NO_ITEM);

        let mut unique_items = DynamicVectorClass::new(initial_size.max(0) as usize, None);
        unique_items.set_growth_step(growth_rate.max(0) as usize);

        UniqueArrayClass {
            unique_items,
            hash_table_size,
            hash_table,
            hash_calculator: hasher,
        }
    }

    pub fn add(&mut self, new_item: T) -> i32 {
        self.hash_calculator.compute_hash(&new_item);
        let num_hash_vals = self.hash_calculator.num_hash_values();

        let mut last_hash = NO_ITEM;
        for hidx in 0..num_hash_vals {
            let hash = self.hash_calculator.get_hash_value(hidx);
            if hash != last_hash {
                let mut test_index = self.hash_table[hash as usize];
                while test_index != NO_ITEM {
                    let entry = &self.unique_items[test_index as usize];
                    if self.hash_calculator.items_match(&entry.item, &new_item) {
                        return test_index;
                    }
                    test_index = entry.next_hash_index;
                }
            }
            last_hash = hash;
        }

        let index = self.unique_items.count() as i32;
        let hash_index = self.hash_calculator.get_hash_value(0) as usize;
        let entry = HashItem {
            item: new_item,
            next_hash_index: self.hash_table[hash_index],
        };
        self.hash_table[hash_index] = index;
        self.unique_items.add(entry);
        index
    }

    pub fn count(&self) -> i32 {
        self.get_unique_count()
    }

    pub fn get_unique_count(&self) -> i32 {
        self.unique_items.count() as i32
    }

    pub fn get(&self, index: i32) -> Option<&T> {
        if index < 0 {
            return None;
        }
        let idx = index as usize;
        if idx >= self.unique_items.count() {
            return None;
        }
        Some(&self.unique_items[idx].item)
    }

    pub fn hash_table_size(&self) -> usize {
        self.hash_table_size
    }
}
