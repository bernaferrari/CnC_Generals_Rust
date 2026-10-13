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
