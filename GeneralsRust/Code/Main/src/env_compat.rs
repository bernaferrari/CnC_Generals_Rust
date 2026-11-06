//! Legacy environment mutation used by test and `host-residuals` cohorts.
//!
//! These wrappers are unsound on platforms where a concurrent environment
//! read can race mutation. A test lock or a single libtest thread does not
//! exclude native readers. Remaining callers require migration to owned
//! configuration or child-process environment before these APIs can be removed.

pub(crate) fn set_var<K: AsRef<std::ffi::OsStr>, V: AsRef<std::ffi::OsStr>>(key: K, value: V) {
    // This legacy call does not establish std::env::set_var's safety requirements.
    unsafe { std::env::set_var(key, value) }
}

pub(crate) fn remove_var<K: AsRef<std::ffi::OsStr>>(key: K) {
    // This legacy call does not establish std::env::remove_var's safety requirements.
    unsafe { std::env::remove_var(key) }
}
