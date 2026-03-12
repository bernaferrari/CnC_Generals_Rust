// FILE: drawable_info.rs
// Ported from C++ DrawableInfo.h

use gamelogic::common::types::{ObjectID, INVALID_ID};

/// Extra rendering flags for drawable rendering control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtraRenderFlags(u32);

impl ExtraRenderFlags {
    pub const IS_NORMAL: Self = Self(0);
    pub const IS_OCCLUDED: Self = Self(0x0000_0001);
    pub const POTENTIAL_OCCLUDER: Self = Self(0x0000_0002);
    pub const POTENTIAL_OCCLUDEE: Self = Self(0x0000_0004);
    pub const IS_TRANSLUCENT: Self = Self(0x0000_0008);
    pub const IS_NON_OCCLUDER_OR_OCCLUDEE: Self = Self(0x0000_0010);
    pub const DELAYED_RENDER: Self = Self(Self::IS_TRANSLUCENT.0 | Self::POTENTIAL_OCCLUDEE.0);

    #[inline]
    pub fn is_set(&self, flag: Self) -> bool {
        (self.0 & flag.0) != 0
    }

    #[inline]
    pub fn set(&mut self, flag: Self) {
        self.0 |= flag.0;
    }

    #[inline]
    pub fn clear(&mut self, flag: Self) {
        self.0 &= !flag.0;
    }
}

impl Default for ExtraRenderFlags {
    fn default() -> Self {
        Self::IS_NORMAL
    }
}

/// Structure binding W3D render objects to Drawables.
#[derive(Debug)]
pub struct DrawableInfo {
    /// Pointer to the object used for shroud status when no object is available.
    pub shroud_status_object_id: ObjectID,
    /// Pointer back to the drawable containing this info (FFI compatibility).
    pub drawable: *mut std::ffi::c_void,
    /// Pointer to ghost object used for fogged versions.
    pub ghost_object: *mut std::ffi::c_void,
    /// Extra render flags tied to render objects.
    pub flags: ExtraRenderFlags,
}

impl DrawableInfo {
    /// Create a new DrawableInfo with default values.
    pub fn new() -> Self {
        Self {
            shroud_status_object_id: INVALID_ID,
            drawable: std::ptr::null_mut(),
            ghost_object: std::ptr::null_mut(),
            flags: ExtraRenderFlags::default(),
        }
    }

    #[inline]
    pub fn is_occluded(&self) -> bool {
        self.flags.is_set(ExtraRenderFlags::IS_OCCLUDED)
    }

    #[inline]
    pub fn is_translucent(&self) -> bool {
        self.flags.is_set(ExtraRenderFlags::IS_TRANSLUCENT)
    }

    #[inline]
    pub fn is_potential_occluder(&self) -> bool {
        self.flags.is_set(ExtraRenderFlags::POTENTIAL_OCCLUDER)
    }

    #[inline]
    pub fn is_potential_occludee(&self) -> bool {
        self.flags.is_set(ExtraRenderFlags::POTENTIAL_OCCLUDEE)
    }

    #[inline]
    pub fn needs_delayed_render(&self) -> bool {
        self.flags.is_set(ExtraRenderFlags::DELAYED_RENDER)
    }
}

impl Default for DrawableInfo {
    fn default() -> Self {
        Self::new()
    }
}

unsafe impl Send for DrawableInfo {}
unsafe impl Sync for DrawableInfo {}
