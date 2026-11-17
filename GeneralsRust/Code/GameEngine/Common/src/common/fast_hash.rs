//! Deterministic fast hashing for hot, id-keyed lookup tables.
//!
//! `std::collections::HashMap` defaults to SipHash with a per-process random
//! key: DoS-resistant, which nothing here needs, and several times slower than
//! a multiply-rotate hash on the `u32` ids and short names the simulation
//! looks up every frame. Tables using these aliases must never feed their
//! iteration order into simulation decisions (the same rule the random
//! default already imposes): keep a separate ordered list, as
//! `HostObjectStore::registration_order` does.

pub use rustc_hash::{FxBuildHasher, FxHashMap, FxHashSet};
