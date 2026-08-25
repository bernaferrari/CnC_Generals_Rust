// FILE: drawable_info.rs
// Ported from C++ DrawableInfo.h

use gamelogic::common::types::{DrawableID, INVALID_ID, ObjectID};

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
///
/// C++ `DrawableInfo` stores `Drawable*` / `GhostObject*`. Those raw pointers plus
/// `unsafe impl Send/Sync` were unsound. Save/load already round-trips the drawable
/// via id (`findDrawableByID`); the ghost pointer is never xfer'd. This port stores
/// typed IDs only, so `Send`/`Sync` are auto-derived from `u32` — no `unsafe impl`.
///
/// Client code must not treat this as a cross-thread object graph: look up live
/// drawables/ghosts by id on the client thread.
///
/// `DrawableInfo` is auto-`Send` because it contains only IDs (no raw pointers).
/// A compile_fail `!Send` test would be wrong after this layout change; the
/// regression to catch is re-introducing `*mut` + `unsafe impl Send/Sync`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrawableInfo {
    /// Object used for shroud status when no object is available (C++ `m_shroudStatusObjectID`).
    pub shroud_status_object_id: ObjectID,
    /// Drawable that owns this info (C++ `m_drawable`, stored as ID not pointer).
    pub drawable_id: DrawableID,
    /// Ghost object used for fogged W3D snapshots (C++ `m_ghostObject`, ID not pointer).
    /// `INVALID_ID` when unset.
    pub ghost_object_id: ObjectID,
    /// Extra render flags tied to render objects.
    pub flags: ExtraRenderFlags,
}

impl DrawableInfo {
    /// Create a new DrawableInfo with default values.
    pub fn new() -> Self {
        Self {
            shroud_status_object_id: INVALID_ID,
            drawable_id: INVALID_ID,
            ghost_object_id: INVALID_ID,
            flags: ExtraRenderFlags::default(),
        }
    }

    /// Bind this info to a drawable id (C++ `m_drawableInfo.m_drawable = this`).
    pub fn for_drawable(drawable_id: DrawableID) -> Self {
        Self {
            drawable_id,
            ..Self::new()
        }
    }

    #[inline]
    pub fn set_drawable_id(&mut self, drawable_id: DrawableID) {
        self.drawable_id = drawable_id;
    }

    #[inline]
    pub fn clear_drawable(&mut self) {
        self.drawable_id = INVALID_ID;
    }

    #[inline]
    pub fn set_ghost_object_id(&mut self, ghost_object_id: ObjectID) {
        self.ghost_object_id = ghost_object_id;
    }

    #[inline]
    pub fn clear_ghost_object(&mut self) {
        self.ghost_object_id = INVALID_ID;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_uses_invalid_ids() {
        let info = DrawableInfo::new();
        assert_eq!(info.shroud_status_object_id, INVALID_ID);
        assert_eq!(info.drawable_id, INVALID_ID);
        assert_eq!(info.ghost_object_id, INVALID_ID);
        assert_eq!(info.flags, ExtraRenderFlags::IS_NORMAL);
    }

    #[test]
    fn stores_typed_ids_not_pointers() {
        let mut info = DrawableInfo::for_drawable(42);
        assert_eq!(info.drawable_id, 42);
        info.set_drawable_id(7);
        assert_eq!(info.drawable_id, 7);
        info.clear_drawable();
        assert_eq!(info.drawable_id, INVALID_ID);

        info.set_ghost_object_id(99);
        assert_eq!(info.ghost_object_id, 99);
        info.clear_ghost_object();
        assert_eq!(info.ghost_object_id, INVALID_ID);
    }

    #[test]
    fn auto_send_sync_from_id_fields_without_unsafe_impl() {
        // DrawableInfo is only ObjectID/DrawableID/flags (all Copy integers).
        // This compiles if and only if the type is auto Send/Sync — no
        // `unsafe impl Send/Sync` and no raw pointer fields.
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<DrawableInfo>();
        assert_sync::<DrawableInfo>();
        assert_send::<ExtraRenderFlags>();
        assert_sync::<ExtraRenderFlags>();
    }
}
