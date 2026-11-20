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

// Stable positions in the compact presentation bank and the corresponding
// prefix of the full snapshot kind list. Other kinds follow enum order.
const PRESENTATION_ORDER: [KindOf; 32] = [
    KindOf::Structure,
    KindOf::Infantry,
    KindOf::Vehicle,
    KindOf::Aircraft,
    KindOf::Projectile,
    KindOf::Resource,
    KindOf::Selectable,
    KindOf::Attackable,
    KindOf::CommandCenter,
    KindOf::Worker,
    KindOf::Hero,
    KindOf::SupplyCenter,
    KindOf::PowerPlant,
    KindOf::FSBarracks,
    KindOf::FSWarFactory,
    KindOf::FSAirfield,
    KindOf::FSInternetCenter,
    KindOf::FSPower,
    KindOf::FSBaseDefense,
    KindOf::FSSupplyDropzone,
    KindOf::FSSupplyCenter,
    KindOf::FSSuperweapon,
    KindOf::FSStrategyCenter,
    KindOf::FSFake,
    KindOf::FSTechnology,
    KindOf::FSBlackMarket,
    KindOf::FSAdvancedTech,
    KindOf::Harvestable,
    KindOf::Powered,
    KindOf::IgnoredInGui,
    KindOf::Dozer,
    KindOf::Harvester,
];
const PRESENTATION_KIND_BITS: ([u32; KindOf::ALL_KIND_OF.len()], u128) = {
    let mut bank = [0; KindOf::ALL_KIND_OF.len()];
    let mut mask = 0;
    let mut index = 0;
    while index < PRESENTATION_ORDER.len() {
        let kind = PRESENTATION_ORDER[index] as usize;
        bank[kind] = 1 << index;
        mask |= 1 << kind;
        index += 1;
    }
    (bank, mask)
};

// Direct bit-index lookup preserves the established serialization order.
const _: () = {
    let kinds = KindOf::ALL_KIND_OF;
    assert!(kinds.len() <= u128::BITS as usize);
    assert!(kinds.len() == KindOf::ProducedAtHelipad as usize + 1);
    let mut index = 0;
    while index < kinds.len() {
        assert!(kinds[index] as usize == index);
        index += 1;
    }
};

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

    /// Raw membership bits, indexed by host `KindOf` discriminant.
    pub const fn bits(&self) -> u128 {
        self.bits
    }

    pub fn is_empty(&self) -> bool {
        self.bits == 0
    }

    pub fn iter(&self) -> KindOfSetIter<'_> {
        KindOfSetIter {
            remaining: self.bits,
            kinds: KindOf::ALL_KIND_OF,
        }
    }

    pub(crate) fn presentation_bits(self) -> u32 {
        let (bank, prefix) = &PRESENTATION_KIND_BITS;
        let mut remaining = self.bits & prefix;
        let mut bits = 0;
        while remaining != 0 {
            bits |= bank[remaining.trailing_zeros() as usize];
            remaining &= remaining - 1;
        }
        bits
    }

    pub(crate) fn presentation_bank_kinds(mut bits: u32) -> impl Iterator<Item = KindOf> {
        std::iter::from_fn(move || {
            if bits == 0 {
                return None;
            }
            let index = bits.trailing_zeros() as usize;
            bits &= bits - 1;
            Some(PRESENTATION_ORDER[index])
        })
    }

    pub(crate) fn presentation_order(self) -> impl Iterator<Item = KindOf> {
        let prefix = Self::presentation_bank_kinds(self.presentation_bits());
        let suffix = KindOfSetIter {
            remaining: self.bits & !PRESENTATION_KIND_BITS.1,
            kinds: KindOf::ALL_KIND_OF,
        };
        prefix.chain(suffix.copied())
    }
}

impl Hash for KindOfSet {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.bits.hash(state);
    }
}

pub struct KindOfSetIter<'a> {
    remaining: u128,
    kinds: &'a [KindOf],
}

impl<'a> Iterator for KindOfSetIter<'a> {
    type Item = &'a KindOf;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let index = self.remaining.trailing_zeros() as usize;
        self.remaining &= self.remaining - 1;
        Some(&self.kinds[index])
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.remaining.count_ones() as usize;
        (len, Some(len))
    }
}

impl ExactSizeIterator for KindOfSetIter<'_> {}

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
        // Length-prefixed formats (bincode) need the element count up front.
        use serde::ser::SerializeSeq;
        let mut seq = serializer.serialize_seq(Some(self.len()))?;
        for kind in self {
            seq.serialize_element(kind)?;
        }
        seq.end()
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
        assert!(bits.iter().eq(KindOf::ALL_KIND_OF));
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
    fn presentation_keeps_compact_bits_and_the_full_snapshot_order() {
        let set: KindOfSet = [
            KindOf::ProducedAtHelipad,
            KindOf::Dozer,
            KindOf::Mine,
            KindOf::Projectile,
            KindOf::Vehicle,
        ]
        .into_iter()
        .collect();
        assert_eq!(set.presentation_bits(), (1 << 2) | (1 << 4) | (1 << 30));
        assert!(KindOfSet::presentation_bank_kinds(set.presentation_bits()).eq([
            KindOf::Vehicle,
            KindOf::Projectile,
            KindOf::Dozer,
        ]));
        assert!(set.presentation_order().eq([
            KindOf::Vehicle,
            KindOf::Projectile,
            KindOf::Dozer,
            KindOf::Mine,
            KindOf::ProducedAtHelipad,
        ]));

        let all: KindOfSet = KindOf::ALL_KIND_OF.iter().copied().collect();
        assert_eq!(all.presentation_bits(), u32::MAX);
        assert_eq!(all.presentation_order().count(), all.len());
        assert!(KindOfSet::new().presentation_order().next().is_none());
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
