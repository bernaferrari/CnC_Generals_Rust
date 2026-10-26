//! Source ParachuteContain attachment state and local sway, owned by one Object.
use glam::{Mat4, Vec3, Vec4};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct ParachuteAttachments {
    pub rider_attach_bone: Vec3,
    pub rider_sway_bone: Vec3,
    pub para_attach_bone: Vec3,
    pub para_sway_bone: Vec3,
    pub rider_attach_offset: Vec3,
    pub rider_sway_offset: Vec3,
    pub para_attach_offset: Vec3,
    pub para_sway_offset: Vec3,
    pub need_rider_bones: bool,
    pub need_para_bones: bool,
    /// OpenContain source m_conditionState; observation, not Drawable authority.
    #[serde(default)]
    pub observed_condition_bits: u128,
}
impl Default for ParachuteAttachments {
    fn default() -> Self {
        Self {
            rider_attach_bone: Vec3::ZERO,
            rider_sway_bone: Vec3::ZERO,
            para_attach_bone: Vec3::ZERO,
            para_sway_bone: Vec3::ZERO,
            rider_attach_offset: Vec3::ZERO,
            rider_sway_offset: Vec3::ZERO,
            para_attach_offset: Vec3::ZERO,
            para_sway_offset: Vec3::ZERO,
            need_rider_bones: true,
            need_para_bones: true,
            observed_condition_bits: 0,
        }
    }
}
impl ParachuteAttachments {
    pub fn validate(&self) -> bool {
        [
            self.rider_attach_bone,
            self.rider_sway_bone,
            self.para_attach_bone,
            self.para_sway_bone,
            self.rider_attach_offset,
            self.rider_sway_offset,
            self.para_attach_offset,
            self.para_sway_offset,
        ]
        .iter()
        .all(|v| v.is_finite())
    }
}
/// ParachuteContain.cpp141: translation is preserved while X then Y
/// pre-multiply only the rotation block (matrix3d.h1198/1232).
pub(crate) fn sway_matrix(offset: Vec3, pitch: f32, roll: f32) -> Mat4 {
    let basis = Mat4::from_cols(Vec4::X, Vec4::Z, Vec4::Y, Vec4::W);
    let cpp = Vec3::new(offset.x, offset.z, offset.y);
    basis
        * (Mat4::from_translation(cpp)
            * Mat4::from_rotation_y(pitch)
            * Mat4::from_rotation_x(roll)
            * Mat4::from_translation(-cpp))
        * basis
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_sway_preserves_pivot_and_pre_rotation_order() {
        let offset = Vec3::new(2., 3., 4.);
        let m = sway_matrix(offset, 0.3, 0.5);
        assert!(m.transform_point3(offset).distance(offset) < 1e-5);
        let s = Mat4::from_cols(Vec4::X, Vec4::Z, Vec4::Y, Vec4::W);
        let expected = s * (Mat4::from_rotation_y(0.3) * Mat4::from_rotation_x(0.5)) * s;
        assert!(
            m.transform_vector3(Vec3::X)
                .distance(expected.transform_vector3(Vec3::X))
                < 1e-5
        );
    }
    #[test]
    fn actual_object_attachment_lifecycle_payload_roundtrips() {
        use crate::game_logic::{Object, ObjectId, Team, ThingTemplate};
        let mut object = Object::new(
            ThingTemplate::new("SourceParaCache"),
            ObjectId(44),
            Team::USA,
        );
        let state = ParachuteAttachments {
            need_rider_bones: false,
            need_para_bones: false,
            rider_attach_bone: Vec3::new(1., 2., 3.),
            rider_attach_offset: Vec3::new(4., 5., 6.),
            ..Default::default()
        };
        object.parachute_attachments = Some(state.clone());
        let modules =
            super::super::entity_lifecycle_inventory::collect_module_states(&object).unwrap();
        let mut loaded = Object::new(
            ThingTemplate::new("SourceParaCache"),
            ObjectId(44),
            Team::USA,
        );
        super::super::entity_lifecycle_apply::apply_module_states(&mut loaded, &modules).unwrap();
        assert_eq!(loaded.parachute_attachments, Some(state));
    }
}
