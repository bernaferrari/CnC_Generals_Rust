//! Engine-object bridge + process-stable GENERALS_* env flag caches.

/// Whether the optional engine shadow path is enabled.
/// True when Main create_object may attach gamelogic OBJECT_REGISTRY ids (opt-in only).
/// Host Object pose/HP/alive never dual-read the registry — stamp is metadata only.
/// Cached flag (env is process-stable; per-call getenv was a Lone Eagle hotspot).
/// 0 = unset, 1 = off, 2 = on.
static ENGINE_OBJECT_BRIDGE_CACHE: std::sync::atomic::AtomicU8 =
    std::sync::atomic::AtomicU8::new(0);

/// Invalidate bridge cache after test env mutation (production never needs this).
pub fn refresh_engine_object_bridge_cache() {
    ENGINE_OBJECT_BRIDGE_CACHE.store(0, std::sync::atomic::Ordering::Relaxed);
}

#[inline]
pub fn engine_object_bridge_enabled() -> bool {
    #[cfg(test)]
    {
        return std::env::var_os("GENERALS_ALLOW_DUAL_TICK").is_some()
            || std::env::var_os("GENERALS_BRIDGE_ENGINE_OBJECTS").is_some();
    }
    #[cfg(not(test))]
    {
        use std::sync::atomic::Ordering::Relaxed;
        match ENGINE_OBJECT_BRIDGE_CACHE.load(Relaxed) {
            1 => false,
            2 => true,
            _ => {
                let on = std::env::var_os("GENERALS_ALLOW_DUAL_TICK").is_some()
                    || std::env::var_os("GENERALS_BRIDGE_ENGINE_OBJECTS").is_some();
                ENGINE_OBJECT_BRIDGE_CACHE.store(if on { 2 } else { 1 }, Relaxed);
                on
            }
        }
    }
}

/// Process-stable env bool cache: 0=unset, 1=false, 2=true.
#[inline]
pub fn env_flag_raw(name: &str, default_on: bool) -> bool {
    match std::env::var(name) {
        Ok(v) => {
            let v = v.trim();
            !(v == "0"
                || v.eq_ignore_ascii_case("false")
                || v.eq_ignore_ascii_case("off")
                || v.eq_ignore_ascii_case("no"))
        }
        Err(_) => default_on,
    }
}

#[inline]
pub fn env_flag_cached(cache: &std::sync::atomic::AtomicU8, name: &str, default_on: bool) -> bool {
    // Unit tests mutate GENERALS_* mid-process; always re-read under cfg(test).
    #[cfg(test)]
    {
        let _ = cache;
        return env_flag_raw(name, default_on);
    }
    #[cfg(not(test))]
    {
        use std::sync::atomic::Ordering::Relaxed;
        match cache.load(Relaxed) {
            1 => false,
            2 => true,
            _ => {
                let on = env_flag_raw(name, default_on);
                cache.store(if on { 2 } else { 1 }, Relaxed);
                on
            }
        }
    }
}

/// Invalidate authority env caches after test env mutation.
pub fn refresh_gameworld_authority_env_caches() {
    refresh_engine_object_bridge_cache();
    super::authority::reset_authority_env_caches();
}

