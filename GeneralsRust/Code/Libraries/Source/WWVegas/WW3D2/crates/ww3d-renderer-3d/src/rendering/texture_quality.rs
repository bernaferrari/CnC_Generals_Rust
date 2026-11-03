//! Global texture quality settings mirroring the original WW3D knobs.
//!
//! The legacy engine exposed a couple of runtime switches that controlled how
//! aggressively textures should be reduced at load time.  The Rust port keeps
//! the same semantics so higher level code can toggle them in the same way as
//! before.


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


/// Fetch a snapshot of the current texture quality settings.
pub fn settings(r3d: &crate::process_state::Renderer3dRuntime) -> TextureQualitySettings {
    r3d.texture_quality.lock().unwrap_or_else(|p| p.into_inner()).clone()
}

/// Replace the active texture quality settings.
pub fn set(r3d: &crate::process_state::Renderer3dRuntime, settings: TextureQualitySettings) {
    let mut cfg = settings;
    cfg.min_dimension = cfg.min_dimension.max(1);
    *r3d.texture_quality.lock().unwrap_or_else(|p| p.into_inner()) = cfg;
}

/// Set the global reduction and minimum dimension as per the legacy `WW3D::Set_Texture_Reduction`.
pub fn set_texture_reduction(
    r3d: &crate::process_state::Renderer3dRuntime,
    reduction: u32,
    min_dimension: u32,
) {
    let mut cell = r3d.texture_quality.lock().unwrap_or_else(|p| p.into_inner());
    let extra = cell.large_texture_extra_reduction;
    *cell = TextureQualitySettings {
        reduction,
        min_dimension: min_dimension.max(1),
        large_texture_extra_reduction: extra,
    };
}

/// Retrieve the currently requested global reduction level.
pub fn texture_reduction(r3d: &crate::process_state::Renderer3dRuntime) -> u32 {
    settings(r3d).reduction
}

/// Retrieve the minimum dimension clamp used when applying reductions.
pub fn texture_min_dimension(r3d: &crate::process_state::Renderer3dRuntime) -> u32 {
    settings(r3d).min_dimension
}

/// Toggle the "large texture extra reduction" flag from the legacy renderer.
pub fn enable_large_texture_extra_reduction(
    r3d: &crate::process_state::Renderer3dRuntime,
    enabled: bool,
) {
    let mut cell = r3d.texture_quality.lock().unwrap_or_else(|p| p.into_inner());
    let mut cfg = *cell;
    cfg.large_texture_extra_reduction = enabled;
    *cell = cfg;
}

/// Query whether the extra large texture reduction is active.
pub fn is_large_texture_extra_reduction_enabled(
    r3d: &crate::process_state::Renderer3dRuntime,
) -> bool {
    settings(r3d).large_texture_extra_reduction
}

/// Compute how many mip levels should be skipped for the given texture.
pub fn compute_effective_reduction(
    r3d: &crate::process_state::Renderer3dRuntime,
    width: u32,
    height: u32,
    mip_levels: u32,
) -> u32 {
    if mip_levels <= 1 || width <= 32 || height <= 32 {
        return 0;
    }

    let settings = settings(r3d);
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
    fn test_lock() {
        let r3d = crate::process_state::Renderer3dRuntime::new();}

    #[test]
    fn respects_min_dimension() {
        let r3d = crate::process_state::Renderer3dRuntime::new();
        let r3d = crate::process_state::Renderer3dRuntime::new();
        set(TextureQualitySettings::default());
        set_texture_reduction(3, 64);
        assert_eq!(compute_effective_reduction(&r3d, 128, 128, 5), 2);
        assert_eq!(compute_effective_reduction(&r3d, 32, 64, 5), 0);
    }

    #[test]
    fn honours_large_texture_extra_reduction() {
        let r3d = crate::process_state::Renderer3dRuntime::new();
        set(TextureQualitySettings::default());
        set_texture_reduction(1, 1);
        enable_large_texture_extra_reduction(true);
        assert_eq!(compute_effective_reduction(&r3d, 512, 512, 6), 2);
        enable_large_texture_extra_reduction(false);
    }

    #[test]
    fn never_drops_all_mips() {
        let r3d = crate::process_state::Renderer3dRuntime::new();
        set(TextureQualitySettings::default());
        set_texture_reduction(10, 1);
        assert_eq!(compute_effective_reduction(&r3d, 1024, 1024, 3), 2);
    }
}
