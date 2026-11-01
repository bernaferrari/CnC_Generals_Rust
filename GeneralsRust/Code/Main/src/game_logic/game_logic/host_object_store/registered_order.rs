//! Place registered map entries in the owning world's object-list order.
use std::{collections::HashMap, hash::Hash};

pub(super) fn registered_mut<'a, K: Copy + Eq + Hash, V>(
    map: &'a mut HashMap<K, V>,
    registration_order: &[K],
) -> impl Iterator<Item = (&'a K, &'a mut V)> {
    let order: HashMap<_, _> = registration_order
        .iter()
        .enumerate()
        .map(|(index, id)| (*id, index))
        .collect();
    // An object may exist in the lookup map before its constructor registers
    // it in the C++ object list. Such entries are excluded from traversal.
    let mut rows: Vec<_> = std::iter::repeat_with(|| None)
        .take(registration_order.len())
        .collect();
    for (id, object) in map.iter_mut() {
        if let Some(&index) = order.get(id) {
            rows[index] = Some((id, object));
        }
    }
    rows.into_iter().flatten()
}
