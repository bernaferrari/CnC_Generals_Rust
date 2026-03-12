// FILE: drawable_info.rs
// Simple structure used to bind W3D renderObjects to our own Drawables.
// Ported from C++ DrawableInfo.h
// Author: Mark Wilczynski, August 2002

use crate::Common::game_type::ObjectID;

/// Extra rendering flags for drawable rendering control
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtraRenderFlags(u32);

impl ExtraRenderFlags {
    pub const IS_NORMAL: Self = Self(0);
    pub const IS_OCCLUDED: Self = Self(0x00000001);
    pub const POTENTIAL_OCCLUDER: Self = Self(0x00000002);
    pub const POTENTIAL_OCCLUDEE: Self = Self(0x00000004);
    pub const IS_TRANSLUCENT: Self = Self(0x00000008);
    pub const IS_NON_OCCLUDER_OR_OCCLUDEE: Self = Self(0x00000010);
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

/// Structure binding W3D render objects to our Drawables
/// Contains information needed for rendering and shroud status
#[derive(Debug)]
pub struct DrawableInfo {
    /// Since we sometimes have drawables without objects, this points to a parent object
    /// from which we pull shroud status
    pub shroud_status_object_id: ObjectID,

    /// Pointer back to drawable containing this DrawableInfo
    /// Using raw pointer for FFI compatibility with C++ system
    pub drawable: *mut std::ffi::c_void,

    /// Pointer to ghostObject for this drawable used for fogged versions
    pub ghost_object: *mut std::ffi::c_void,

    /// Extra render settings flags that are tied to render objects with drawables
    pub flags: ExtraRenderFlags,
}

impl DrawableInfo {
    /// Create a new DrawableInfo with default values
    pub fn new() -> Self {
        Self {
            shroud_status_object_id: ObjectID::INVALID,
            drawable: std::ptr::null_mut(),
            ghost_object: std::ptr::null_mut(),
            flags: ExtraRenderFlags::default(),
        }
    }

    /// Check if this drawable is occluded
    #[inline]
    pub fn is_occluded(&self) -> bool {
        self.flags.is_set(ExtraRenderFlags::IS_OCCLUDED)
    }

    /// Check if this drawable is translucent
    #[inline]
    pub fn is_translucent(&self) -> bool {
        self.flags.is_set(ExtraRenderFlags::IS_TRANSLUCENT)
    }

    /// Check if this drawable is a potential occluder
    #[inline]
    pub fn is_potential_occluder(&self) -> bool {
        self.flags.is_set(ExtraRenderFlags::POTENTIAL_OCCLUDER)
    }

    /// Check if this drawable is a potential occludee
    #[inline]
    pub fn is_potential_occludee(&self) -> bool {
        self.flags.is_set(ExtraRenderFlags::POTENTIAL_OCCLUDEE)
    }

    /// Check if delayed rendering is needed
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
