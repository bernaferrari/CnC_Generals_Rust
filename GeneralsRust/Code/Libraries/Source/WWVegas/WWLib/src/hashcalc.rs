//! Hash calculator trait (ported from WWLib hashcalc.h).

pub trait HashCalculator<T> {
    fn items_match(&self, a: &T, b: &T) -> bool;
    fn compute_hash(&mut self, item: &T);
    fn num_hash_bits(&self) -> i32;
    fn num_hash_values(&self) -> i32;
    fn get_hash_value(&self, index: i32) -> i32;
}
