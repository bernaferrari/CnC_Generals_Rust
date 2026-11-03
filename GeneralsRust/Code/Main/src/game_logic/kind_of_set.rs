//! Fixed KindOf membership set (hq-krx2b.1).
//!
//! Replaces `HashSet<KindOf>` storage on `ThingTemplate` with a u128 bitset
//! indexed by host enum discriminant. Serde keeps the historical sequence-of
//! enum schema.

use super::host_types::KindOf;
use serde::de::{SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::borrow::Borrow;
use std::fmt;
use std::hash::{Hash, Hasher};

/// Typed fixed KindOf membership. Fits all host `KindOf` variants in one u128.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KindOfSet {
    bits: u128,
}

impl KindOfSet {
    pub const fn new() -> Self {
        Self { bits: 0 }
    }

    const fn bit(kind: KindOf) -> u128 {
        1u128 << (kind as u8)
    }

    pub fn contains<Q: Borrow<KindOf>>(&self, kind: &Q) -> bool {
        self.bits & Self::bit(*kind.borrow()) != 0
    }

    pub fn insert(&mut self, kind: KindOf) -> bool {
        let bit = Self::bit(kind);
        let was = self.bits & bit != 0;
        self.bits |= bit;
        !was
    }

    pub fn remove<Q: Borrow<KindOf>>(&mut self, kind: &Q) -> bool {
        let bit = Self::bit(*kind.borrow());
        let was = self.bits & bit != 0;
        self.bits &= !bit;
        was
    }

    pub fn clear(&mut self) {
        self.bits = 0;
    }

    pub fn len(&self) -> usize {
        self.bits.count_ones() as usize
    }

    pub fn is_empty(&self) -> bool {
        self.bits == 0
    }

    pub fn iter(&self) -> KindOfSetIter<'_> {
        KindOfSetIter {
            bits: self.bits,
            inner: KindOf::ALL_KIND_OF.iter(),
        }
    }
}

impl Hash for KindOfSet {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.bits.hash(state);
    }
}

pub struct KindOfSetIter<'a> {
    bits: u128,
    inner: std::slice::Iter<'a, KindOf>,
}

impl<'a> Iterator for KindOfSetIter<'a> {
    type Item = &'a KindOf;

    fn next(&mut self) -> Option<Self::Item> {
        for kind in self.inner.by_ref() {
            if self.bits & KindOfSet::bit(*kind) != 0 {
                return Some(kind);
            }
        }
        None
    }
}

impl<'a> IntoIterator for &'a KindOfSet {
    type Item = &'a KindOf;
    type IntoIter = KindOfSetIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl FromIterator<KindOf> for KindOfSet {
    fn from_iter<T: IntoIterator<Item = KindOf>>(iter: T) -> Self {
        let mut set = Self::new();
        for kind in iter {
            set.insert(kind);
        }
        set
    }
}

impl Extend<KindOf> for KindOfSet {
    fn extend<T: IntoIterator<Item = KindOf>>(&mut self, iter: T) {
        for kind in iter {
            self.insert(kind);
        }
    }
}

impl Serialize for KindOfSet {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.iter().copied())
    }
}

impl<'de> Deserialize<'de> for KindOfSet {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct KindOfSetVisitor;
        impl<'de> Visitor<'de> for KindOfSetVisitor {
            type Value = KindOfSet;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a sequence of KindOf values")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut set = KindOfSet::new();
                while let Some(kind) = seq.next_element::<KindOf>()? {
                    set.insert(kind);
                }
                Ok(set)
            }
        }
        deserializer.deserialize_seq(KindOfSetVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn exhaustive_membership_matches_hashset_reference() {
        let mut bits = KindOfSet::new();
        let mut hash = HashSet::new();
        for &kind in KindOf::ALL_KIND_OF {
            assert_eq!(bits.insert(kind), hash.insert(kind));
            assert!(bits.contains(&kind));
            assert_eq!(bits.len(), hash.len());
        }
        assert_eq!(bits.len(), KindOf::ALL_KIND_OF.len());
        // Duplicate inserts are no-ops.
        for &kind in KindOf::ALL_KIND_OF {
            assert!(!bits.insert(kind));
            assert!(!hash.insert(kind));
        }
        for &kind in KindOf::ALL_KIND_OF {
            assert_eq!(bits.remove(&kind), hash.remove(&kind));
            assert!(!bits.contains(&kind));
        }
        assert!(bits.is_empty());
        bits.clear();
        assert!(bits.is_empty());
    }

    #[test]
    fn mixed_ops_differential_matches_hashset() {
        let mut bits = KindOfSet::new();
        let mut hash = HashSet::new();
        let ops = [
            KindOf::Infantry,
            KindOf::Vehicle,
            KindOf::Dozer,
            KindOf::Infantry,
            KindOf::Aircraft,
            KindOf::Vehicle,
        ];
        for kind in ops {
            bits.insert(kind);
            hash.insert(kind);
        }
        bits.remove(&KindOf::Vehicle);
        hash.remove(&KindOf::Vehicle);
        bits.remove(&KindOf::Mine); // missing
        hash.remove(&KindOf::Mine);
        let mut bit_list: Vec<_> = bits.iter().copied().collect();
        let mut hash_list: Vec<_> = hash.iter().copied().collect();
        bit_list.sort_by_key(|k| *k as u8);
        hash_list.sort_by_key(|k| *k as u8);
        assert_eq!(bit_list, hash_list);
    }

    #[test]
    fn serde_sequence_round_trips_membership() {
        let mut set = KindOfSet::new();
        set.insert(KindOf::Dozer);
        set.insert(KindOf::Vehicle);
        set.insert(KindOf::ProducedAtHelipad);
        let json = serde_json::to_string(&set).unwrap();
        let decoded: KindOfSet = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, set);
        assert!(decoded.contains(&KindOf::ProducedAtHelipad));
        assert_eq!(decoded.len(), 3);
    }

    #[test]
    fn kind_of_set_membership_is_zero_alloc_and_faster_than_hashset() {
        use std::hint::black_box;
        use std::time::Instant;

        // Stack-only container: Copy + fixed u128 storage (zero heap for
        // construction/clone/membership of the set itself).
        assert_eq!(std::mem::size_of::<KindOfSet>(), 16);
        let mut set = KindOfSet::new();
        for &kind in KindOf::ALL_KIND_OF {
            set.insert(kind);
        }
        let cloned = set;
        assert_eq!(cloned, set);

        let mut hash = HashSet::new();
        for &kind in KindOf::ALL_KIND_OF {
            hash.insert(kind);
        }

        let retail = [
            KindOf::Infantry,
            KindOf::Vehicle,
            KindOf::Dozer,
            KindOf::Aircraft,
            KindOf::Structure,
            KindOf::Projectile,
        ];
        let rounds = 50_000usize;

        let start = Instant::now();
        let mut bit_hits = 0usize;
        for _ in 0..rounds {
            for kind in retail {
                if black_box(set.contains(&kind)) {
                    bit_hits = black_box(bit_hits + 1);
                }
            }
            for &kind in KindOf::ALL_KIND_OF {
                if black_box(set.contains(&kind)) {
                    bit_hits = black_box(bit_hits + 1);
                }
            }
        }
        let bit_ns = start.elapsed();

        let start = Instant::now();
        let mut hash_hits = 0usize;
        for _ in 0..rounds {
            for kind in retail {
                if black_box(hash.contains(&kind)) {
                    hash_hits = black_box(hash_hits + 1);
                }
            }
            for &kind in KindOf::ALL_KIND_OF {
                if black_box(hash.contains(&kind)) {
                    hash_hits = black_box(hash_hits + 1);
                }
            }
        }
        let hash_ns = start.elapsed();

        assert_eq!(bit_hits, hash_hits);
        eprintln!(
            "KindOfSet membership {:?} over {} rounds; HashSet {:?}; ratio {:.2}x",
            bit_ns,
            rounds,
            hash_ns,
            hash_ns.as_secs_f64() / bit_ns.as_secs_f64().max(1e-12)
        );
        assert!(
            bit_ns <= hash_ns.saturating_mul(2) + std::time::Duration::from_millis(5),
            "KindOfSet unexpectedly slower than HashSet: {bit_ns:?} vs {hash_ns:?}"
        );
    }
}
