use crate::common::{Bool, Color, Coord2D, Coord3D, Int, ObjectID, Real, UnsignedInt};
pub use game_engine::common::system::xfer::XferVersion;
pub use game_engine::system::{Xfer, XferBlockSize, XferMode, XferStatus};
use glam::Vec3;

// Re-export for legacy compatibility
/// Xfer version identifiers used by legacy modules (note: actual Xfer uses u8, this is for legacy compatibility)
#[allow(dead_code)]
pub type LegacyXferVersion = u32;

/// Helper methods that provide additional xfer functionality beyond the base trait.
/// These methods accept a name parameter for documentation/debugging (which is ignored).
pub trait XferExt: Xfer {
    /// Legacy compatibility: is_loading() maps to checking XferMode
    fn is_loading(&self) -> bool {
        self.get_xfer_mode() == XferMode::Load
    }

    /// Legacy compatibility: is_reading() maps to is_loading()
    fn is_reading(&self) -> bool {
        self.is_loading()
    }

    /// Legacy compatibility: xfer_u32 maps to xfer_unsigned_int
    fn xfer_u32(&mut self, value: &mut u32) -> std::io::Result<()> {
        game_engine::system::Xfer::xfer_unsigned_int(self, value)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{:?}", e)))
    }

    /// Legacy compatibility: xfer_i32 maps to xfer_int
    fn xfer_i32(&mut self, value: &mut i32) -> std::io::Result<()> {
        game_engine::system::Xfer::xfer_int(self, value)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{:?}", e)))
    }

    /// Legacy compatibility: xfer_f32 maps to xfer_real
    fn xfer_f32(&mut self, value: &mut f32) -> std::io::Result<()> {
        game_engine::system::Xfer::xfer_real(self, value)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{:?}", e)))
    }

    /// Legacy compatibility: xfer_string maps to xfer_ascii_string
    fn xfer_string(&mut self, value: &mut String) -> std::io::Result<()> {
        game_engine::system::Xfer::xfer_ascii_string(self, value)
    }

    /// Writes version during save operation. For load operations, use xfer_version_read().
    fn xfer_version_write(&mut self, version: LegacyXferVersion) {
        let version_u8 = version as u8;
        let mut temp = version_u8;
        let _ = game_engine::system::Xfer::xfer_version(self, &mut temp, version_u8);
    }

    /// Reads version during load operation.
    fn xfer_version_read(&mut self) -> LegacyXferVersion {
        let mut version = 0u8;
        let _ = game_engine::system::Xfer::xfer_version(self, &mut version, 0);
        version as u32
    }

    fn xfer_option_particle_system_id(
        &mut self,
        _name: &str,
        value: &mut Option<crate::common::types::ParticleSystemId>,
    ) {
        let mut has_value = value.is_some();
        let _ = game_engine::system::Xfer::xfer_bool(self, &mut has_value);

        if has_value {
            let mut id = value.unwrap_or(0);
            let _ = game_engine::system::Xfer::xfer_unsigned_int(self, &mut id);
            *value = Some(id);
        } else {
            *value = None;
        }
    }

    fn xfer_option_drawable_id(
        &mut self,
        _name: &str,
        value: &mut Option<crate::common::types::DrawableId>,
    ) {
        let mut has_value = value.is_some();
        let _ = game_engine::system::Xfer::xfer_bool(self, &mut has_value);

        if has_value {
            let mut id = value.unwrap_or(0);
            let _ = game_engine::system::Xfer::xfer_unsigned_int(self, &mut id);
            *value = Some(id);
        } else {
            *value = None;
        }
    }

    fn xfer_option_object_id(&mut self, _name: &str, value: &mut Option<ObjectID>) {
        let mut has_value = value.is_some();
        let _ = game_engine::system::Xfer::xfer_bool(self, &mut has_value);

        if has_value {
            let mut id = value.unwrap_or(0);
            let _ = game_engine::system::Xfer::xfer_unsigned_int(self, &mut id);
            *value = Some(id);
        } else {
            *value = None;
        }
    }

    fn xfer_u8(&mut self, value: &mut u8) {
        let mut temp = *value as u32;
        let _ = game_engine::system::Xfer::xfer_unsigned_int(self, &mut temp);
        *value = temp as u8;
    }

    fn xfer_u16(&mut self, value: &mut u16) {
        let mut temp = *value as u32;
        let _ = game_engine::system::Xfer::xfer_unsigned_int(self, &mut temp);
        *value = temp as u16;
    }

    fn xfer_u64(&mut self, value: &mut u64) {
        // Transfer u64 as two u32 values (low word first, then high word)
        let mut low = (*value & 0xFFFFFFFF) as u32;
        let mut high = (*value >> 32) as u32;
        let _ = game_engine::system::Xfer::xfer_unsigned_int(self, &mut low);
        let _ = game_engine::system::Xfer::xfer_unsigned_int(self, &mut high);
        *value = ((high as u64) << 32) | (low as u64);
    }

    fn xfer_color(&mut self, color: &mut Color) {
        // Color is stored as u32 RGBA value (R in lowest byte, A in highest byte)
        // Pack: (A << 24) | (B << 16) | (G << 8) | R
        let mut packed = ((color.a as u32) << 24)
            | ((color.b as u32) << 16)
            | ((color.g as u32) << 8)
            | (color.r as u32);
        let _ = game_engine::system::Xfer::xfer_unsigned_int(self, &mut packed);
        // Unpack after transfer (for load operations)
        color.r = (packed & 0xFF) as u8;
        color.g = ((packed >> 8) & 0xFF) as u8;
        color.b = ((packed >> 16) & 0xFF) as u8;
        color.a = ((packed >> 24) & 0xFF) as u8;
    }

    fn xfer_coord2d(&mut self, value: &mut Coord2D) {
        let _ = game_engine::system::Xfer::xfer_real(self, &mut value.x);
        let _ = game_engine::system::Xfer::xfer_real(self, &mut value.y);
    }

    fn xfer_coord3d(&mut self, value: &mut Coord3D) {
        let _ = game_engine::system::Xfer::xfer_real(self, &mut value.x);
        let _ = game_engine::system::Xfer::xfer_real(self, &mut value.y);
        let _ = game_engine::system::Xfer::xfer_real(self, &mut value.z);
    }

    fn xfer_particle_system_id(&mut self, value: &mut crate::common::types::ParticleSystemId) {
        let _ = game_engine::system::Xfer::xfer_unsigned_int(self, value);
    }

    fn xfer_object_id(&mut self, value: &mut ObjectID) {
        let _ = game_engine::system::Xfer::xfer_unsigned_int(self, value);
    }

    fn xfer_option_weapon_id(
        &mut self,
        _name: &str,
        value: &mut Option<crate::common::types::WeaponId>,
    ) {
        let mut has_value = value.is_some();
        let _ = game_engine::system::Xfer::xfer_bool(self, &mut has_value);

        if has_value {
            let mut id = value.unwrap_or(0);
            let _ = game_engine::system::Xfer::xfer_unsigned_int(self, &mut id);
            *value = Some(id);
        } else {
            *value = None;
        }
    }

    fn xfer_real(&mut self, value: &mut Real) {
        let _ = game_engine::system::Xfer::xfer_real(self, value);
    }

    fn xfer_unsigned_int(&mut self, value: &mut UnsignedInt) {
        let _ = game_engine::system::Xfer::xfer_unsigned_int(self, value);
    }

    fn xfer_radius_decal(&mut self, decal: &crate::common::types::RadiusDecal) {
        // Save-only helper when a mutable decal is not available.
        let mut position = decal.position;
        let mut radius = decal.radius;
        let mut opacity = decal.opacity;
        let mut color = decal.color;
        let mut min_opacity = decal.min_opacity;
        let mut max_opacity = decal.max_opacity;
        let mut opacity_throb_time = decal.opacity_throb_time;
        let mut has_template = decal.template.is_some();
        let mut template_id = decal.template.unwrap_or(0);

        self.xfer_coord3d(&mut position);
        let _ = game_engine::system::Xfer::xfer_real(self, &mut radius);
        let _ = game_engine::system::Xfer::xfer_real(self, &mut opacity);
        let _ = game_engine::system::Xfer::xfer_unsigned_int(self, &mut color);
        let _ = game_engine::system::Xfer::xfer_real(self, &mut min_opacity);
        let _ = game_engine::system::Xfer::xfer_real(self, &mut max_opacity);
        let _ = game_engine::system::Xfer::xfer_unsigned_int(self, &mut opacity_throb_time);
        let _ = game_engine::system::Xfer::xfer_bool(self, &mut has_template);
        if has_template {
            let _ = game_engine::system::Xfer::xfer_unsigned_int(self, &mut template_id);
        }
    }

    fn xfer_radius_decal_mut(&mut self, decal: &mut crate::common::types::RadiusDecal) {
        self.xfer_coord3d(&mut decal.position);
        let _ = game_engine::system::Xfer::xfer_real(self, &mut decal.radius);
        let _ = game_engine::system::Xfer::xfer_real(self, &mut decal.opacity);
        let _ = game_engine::system::Xfer::xfer_unsigned_int(self, &mut decal.color);
        let _ = game_engine::system::Xfer::xfer_real(self, &mut decal.min_opacity);
        let _ = game_engine::system::Xfer::xfer_real(self, &mut decal.max_opacity);
        let _ = game_engine::system::Xfer::xfer_unsigned_int(self, &mut decal.opacity_throb_time);

        let mut has_template = decal.template.is_some();
        let _ = game_engine::system::Xfer::xfer_bool(self, &mut has_template);
        if has_template {
            let mut template_id = decal.template.unwrap_or(0);
            let _ = game_engine::system::Xfer::xfer_unsigned_int(self, &mut template_id);
            decal.template = Some(template_id);
        } else {
            decal.template = None;
        }
    }
}

impl<T: Xfer + ?Sized> XferExt for T {}
