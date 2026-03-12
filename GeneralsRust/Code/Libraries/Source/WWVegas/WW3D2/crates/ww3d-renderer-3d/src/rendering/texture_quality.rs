//! Global texture quality settings mirroring the original WW3D knobs.
//!
//! The legacy engine exposed a couple of runtime switches that controlled how
//! aggressively textures should be reduced at load time.  The Rust port keeps
//! the same semantics so higher level code can toggle them in the same way as
//! before.

use std::sync::{OnceLock, RwLock};

/// Texture quality knobs shared across the renderer.
#[derive(Clone, Copy, Debug)]
pub struct TextureQualitySettings {
    pub reduction: u32,
    pub min_dimension: u32,
    pub large_texture_extra_reduction: bool,
}

impl Default for TextureQualitySettings {
    fn default() -> Self {
        Self {
            reduction: 0,
            min_dimension: 1,
            large_texture_extra_reduction: false,
        }
    }
}

static SETTINGS: OnceLock<RwLock<TextureQualitySettings>> = OnceLock::new();

fn storage() -> &'static RwLock<TextureQualitySettings> {
    SETTINGS.get_or_init(|| RwLock::new(TextureQualitySettings::default()))
}

/// Fetch a snapshot of the current texture quality settings.
pub fn settings() -> TextureQualitySettings {
    *storage().read().expect("texture quality settings poisoned")
}

/// Replace the active texture quality settings.
pub fn set(settings: TextureQualitySettings) {
    *storage()
        .write()
        .expect("texture quality settings poisoned") = TextureQualitySettings {
        reduction: settings.reduction,
        min_dimension: settings.min_dimension.max(1),
        large_texture_extra_reduction: settings.large_texture_extra_reduction,
    };
}

/// Set the global reduction and minimum dimension as per the legacy `WW3D::Set_Texture_Reduction`.
pub fn set_texture_reduction(reduction: u32, min_dimension: u32) {
    let mut guard = storage()
        .write()
        .expect("texture quality settings poisoned");
    guard.reduction = reduction;
    guard.min_dimension = min_dimension.max(1);
}

/// Retrieve the currently requested global reduction level.
pub fn texture_reduction() -> u32 {
    settings().reduction
}

/// Retrieve the minimum dimension clamp used when applying reductions.
pub fn texture_min_dimension() -> u32 {
    settings().min_dimension
}

/// Toggle the "large texture extra reduction" flag from the legacy renderer.
pub fn enable_large_texture_extra_reduction(enabled: bool) {
    storage()
        .write()
        .expect("texture quality settings poisoned")
        .large_texture_extra_reduction = enabled;
}

/// Query whether the extra large texture reduction is active.
pub fn is_large_texture_extra_reduction_enabled() -> bool {
    settings().large_texture_extra_reduction
}

/// Compute how many mip levels should be skipped for the given texture.
pub fn compute_effective_reduction(width: u32, height: u32, mip_levels: u32) -> u32 {
    if mip_levels <= 1 || width <= 32 || height <= 32 {
        return 0;
    }

    let settings = settings();
    let mut desired = settings.reduction;

    if settings.large_texture_extra_reduction && (width > 256 || height > 256) {
        desired = desired.saturating_add(1);
    }

    let min_dimension = settings.min_dimension.max(1);
    let max_droppable = mip_levels.saturating_sub(1);

    let mut effective = 0u32;
    let mut current_width = width;
    let mut current_height = height;

    while effective < desired
        && effective < max_droppable
        && current_width >= min_dimension
        && current_height >= min_dimension
    {
        current_width = (current_width / 2).max(1);
        current_height = (current_height / 2).max(1);
        effective += 1;
    }

    effective.min(max_droppable)
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;
    use std::sync::{Mutex, MutexGuard, OnceLock};

    fn test_lock() -> MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    #[test]
    fn respects_min_dimension() {
        let _guard = test_lock();
        set(TextureQualitySettings::default());
        set_texture_reduction(3, 64);
        assert_eq!(compute_effective_reduction(128, 128, 5), 2);
        assert_eq!(compute_effective_reduction(32, 64, 5), 0);
    }

    #[test]
    fn honours_large_texture_extra_reduction() {
        let _guard = test_lock();
        set(TextureQualitySettings::default());
        set_texture_reduction(1, 1);
        enable_large_texture_extra_reduction(true);
        assert_eq!(compute_effective_reduction(512, 512, 6), 2);
        enable_large_texture_extra_reduction(false);
    }

    #[test]
    fn never_drops_all_mips() {
        let _guard = test_lock();
        set(TextureQualitySettings::default());
        set_texture_reduction(10, 1);
        assert_eq!(compute_effective_reduction(1024, 1024, 3), 2);
    }
}
