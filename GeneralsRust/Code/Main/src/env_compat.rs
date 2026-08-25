//! Centralized environment mutation for the Rust 2024 edition.
//!
//! `std::env::set_var`/`remove_var` are `unsafe` in edition 2024 because env
//! mutation is process-global and not synchronized. In this port env vars are
//! `GENERALS_*` configuration toggles read only through the env caches at
//! defined boundaries; test access is serialized by `authority_env_lock`
//! (gameworld_shadow tests harness) and the repo-standard `--test-threads=1`.
//! All mutation funnels through these wrappers so the safety argument lives in
//! exactly one place.

pub(crate) fn set_var<K: AsRef<std::ffi::OsStr>, V: AsRef<std::ffi::OsStr>>(key: K, value: V) {
    // SAFETY: env mutation is serialized per the module docs above.
    unsafe { std::env::set_var(key, value) }
}

pub(crate) fn remove_var<K: AsRef<std::ffi::OsStr>>(key: K) {
    // SAFETY: env mutation is serialized per the module docs above.
    unsafe { std::env::remove_var(key) }
}
