// Auto-generated C++ compatibility shim for sampler
use rand::seq::SliceRandom;
use rand::thread_rng;

pub fn sample_one<T: Clone>(items: &[T]) -> Option<T> {
    let mut rng = thread_rng();
    items.choose(&mut rng).cloned()
}
