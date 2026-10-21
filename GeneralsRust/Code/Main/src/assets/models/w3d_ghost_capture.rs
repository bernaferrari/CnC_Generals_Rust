//! Clone the native Main W3D draw pose at its actual ModelDraw boundary.
//! C++ W3DGhostObject::snapShot/update clone the live RenderObj after HAnim,
//! turret/recoil and hide/show have been applied. The same pose functions used
//! by ordinary collection supply these fields; no asset-name reconstruction.
use super::*;
use crate::assets::{AuthoredDrawPrimaryTurret, AuthoredDrawSubobjectVisibility};
use gamelogic::object::w3d_ghost_object::{
    Matrix3x4, RenderObjectClass, RenderObjectState, RenderSubObjectSnapshot,
};

impl W3DModel {
    pub(crate) fn capture_native_ghost_render_object(
        &self,
        world_transform: Mat4,
        scale: f32,
        color: u32,
        animation_binding: Option<&W3dAnimationBinding>,
        animation_frame: f32,
        primary_turret: &AuthoredDrawPrimaryTurret,
        turret_angle: f32,
        turret_pitch: f32,
        weapon_controls: &[W3dWeaponVisualControl],
        visibility: &[AuthoredDrawSubobjectVisibility],
        world_is_night: bool,
    ) -> Option<RenderObjectState> {
        if self.name.is_empty() || !world_transform.is_finite() || !scale.is_finite() {
            return None;
        }
        // Main currently has exact clone state for a standalone mesh and one
        // rigid HLOD. Skin palettes and external/other RenderObj children are
        // separate native producers; never serialize their bind pose as live.
        if self.hlod_parse_failed
            || !self.hmodels.is_empty()
            || !self.collections.is_empty()
            || !self.aggregates.is_empty()
            || !self.dist_lods.is_empty()
        {
            return None;
        }
        let (class_id, sub_objects) = if self.hlods.is_empty() {
            if self.meshes.len() != 1
                || animation_binding.is_some()
                || Self::mesh_declares_skin(&self.meshes[0])
            {
                return None;
            }
            (RenderObjectClass::Mesh, Vec::new())
        } else {
            let (hlod, lod, _) = self.rigid_hlod_static_lod_context()?;
            if hlod
                .aggregates
                .as_ref()
                .is_some_and(|children| !children.subobjects.is_empty())
            {
                return None;
            }
            let mut children = Vec::with_capacity(lod.subobjects.len());
            for child in &lod.subobjects {
                let mesh_index = self.meshes.iter().enumerate().find_map(|(index, _)| {
                    self.rigid_hlod_subobject_for_mesh(index)
                        .is_some_and(|(source, _)| std::ptr::eq(source, child))
                        .then_some(index)
                })?;
                let mesh = &self.meshes[mesh_index];
                if Self::mesh_declares_skin(mesh) {
                    return None;
                }
                let (transform, animation_visible) = self
                    .mesh_local_transform_and_visibility_for_primary_turret_and_weapon_controls(
                        mesh_index,
                        animation_binding,
                        animation_frame,
                        primary_turret,
                        turret_angle,
                        turret_pitch,
                        weapon_controls,
                    )?;
                if !transform.is_finite() {
                    return None;
                }
                let authored_visible =
                    self.mesh_visible_for_authored_subobject_directives(mesh_index, visibility);
                let visible = animation_visible
                    && self
                        .muzzle_flash_visibility_override_for_mesh(mesh_index, weapon_controls)
                        .unwrap_or(authored_visible)
                    && (world_is_night || !mesh.name.contains("HEADLIGHT"));
                // Main stores asset scale on its geometry component rather
                // than mutating an Arc prototype. C++ Scale scales HTree
                // translations and vertices, not bone rotations. Preserve
                // that actual pose in the world-space snapshot; the renderer
                // applies the geometry scale to vertices exactly once.
                let mut scaled_pose = transform;
                scaled_pose.w_axis.x *= scale;
                scaled_pose.w_axis.y *= scale;
                scaled_pose.w_axis.z *= scale;
                children.push(RenderSubObjectSnapshot {
                    // Native ghost rendering resolves exactly the same mesh
                    // identity retained by this renderer's W3D asset.
                    name: child.name.clone(),
                    visible,
                    transform: Matrix3x4::from_logic_matrix(world_transform * scaled_pose),
                });
            }
            (RenderObjectClass::HLod, children)
        };
        Some(RenderObjectState {
            name: self.name.clone(),
            scale: 1.0,
            geometry_scale: scale,
            color,
            transform: Matrix3x4::from_logic_matrix(world_transform),
            sub_objects,
            class_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gamelogic::object::w3d_ghost_object::W3DRenderObjectSnapshot;

    fn model() -> W3DModel {
        let mut model = W3DModel::new("BUILDING".into());
        model.retain_source_hierarchy(W3dHierarchy {
            name: "BuildingHierarchy".into(),
            pivots: vec![
                W3dPivot {
                    name: "ROOT".into(),
                    parent_idx: u32::MAX,
                    translation: [0.0; 3],
                    euler_angles: [0.0; 3],
                    rotation: [0.0, 0.0, 0.0, 1.0],
                },
                W3dPivot {
                    name: "TURRET".into(),
                    parent_idx: 0,
                    translation: [10.0, 20.0, 30.0],
                    euler_angles: [0.0; 3],
                    rotation: [0.0, 0.0, 0.0, 1.0],
                },
            ],
            pivot_fixups: Vec::new(),
        });
        model.hlods.push(W3dHlod {
            version: 0x0001_0000,
            name: "BUILDING".into(),
            hierarchy_name: "BuildingHierarchy".into(),
            lods: vec![W3dHlodLod {
                max_screen_size: f32::MAX,
                subobjects: vec![
                    W3dHlodSubObject {
                        name: "BUILDING.BODY".into(),
                        bone_index: 1,
                    },
                    W3dHlodSubObject {
                        name: "BUILDING.MUZZLEFX".into(),
                        bone_index: 1,
                    },
                ],
            }],
            aggregates: None,
            proxies: None,
            has_unrendered_aggregates: false,
            has_invalid_trailing_records: false,
        });
        for name in ["BODY", "MUZZLEFX"] {
            let mut mesh = W3DMesh::new(name.into());
            mesh.container_name = "BUILDING".into();
            model.meshes.push(mesh);
        }
        model
    }

    #[test]
    fn native_hlod_clone_keeps_cpp_world_children_scaled_displacement_and_muzzle_hide() {
        let model = model();
        let parent = Mat4::from_translation(Vec3::new(100.0, 200.0, 300.0));
        let captured = model
            .capture_native_ghost_render_object(
                parent,
                2.0,
                0xff12_3456,
                None,
                0.0,
                &AuthoredDrawPrimaryTurret::default(),
                0.0,
                0.0,
                &[],
                &[AuthoredDrawSubobjectVisibility {
                    name: "BODY".into(),
                    hidden: true,
                }],
                false,
            )
            .unwrap();
        assert_eq!(captured.class_id, RenderObjectClass::HLod);
        assert_eq!(captured.sub_objects.len(), 2);
        assert_eq!(captured.sub_objects[0].name, "BUILDING.BODY");
        assert!(!captured.sub_objects[0].visible);
        // Source W3D Z-up translation [10,20,30] becomes Main Y-up
        // [10,30,20]; asset Scale2 changes translation, not bone rotation.
        let child = captured.sub_objects[0].transform.to_logic_matrix();
        assert_eq!(child.w_axis.truncate(), Vec3::new(120.0, 260.0, 340.0));
        assert_eq!(child.x_axis.truncate(), Vec3::X);
        let snapshot = W3DRenderObjectSnapshot::new(captured);
        assert!(!snapshot.render_object.sub_objects[1].visible);
        assert!(snapshot.uv_animations_disabled);
        assert_eq!(snapshot.render_object.color, 0xff12_3456);
        assert_eq!(snapshot.render_object.scale, 1.0);
        assert_eq!(snapshot.render_object.geometry_scale, 2.0);
        let reflected = model
            .capture_native_ghost_render_object(
                parent,
                -2.0,
                0xff12_3456,
                None,
                0.0,
                &AuthoredDrawPrimaryTurret::default(),
                0.0,
                0.0,
                &[],
                &[],
                false,
            )
            .expect("finite negative Drawable scale retains reflected geometry");
        assert_eq!(reflected.geometry_scale, -2.0);
        assert!(reflected.transform.to_logic_matrix().is_finite());
        assert_eq!(
            reflected.sub_objects[0]
                .transform
                .to_logic_matrix()
                .w_axis
                .truncate(),
            Vec3::new(80.0, 140.0, 260.0)
        );
        let zero = model
            .capture_native_ghost_render_object(
                parent,
                0.0,
                0xff12_3456,
                None,
                0.0,
                &AuthoredDrawPrimaryTurret::default(),
                0.0,
                0.0,
                &[],
                &[],
                false,
            )
            .expect("zero Drawable scale remains a finite collapsed prototype");
        assert_eq!(zero.geometry_scale.to_bits(), 0.0_f32.to_bits());
        assert!(zero.transform.to_logic_matrix().is_finite());
        for child in &zero.sub_objects {
            let transform = child.transform.to_logic_matrix();
            assert!(transform.is_finite());
            assert_eq!(
                transform.w_axis.truncate(),
                parent.w_axis.truncate(),
                "zero asset scale collapses HTree displacement without dividing by zero"
            );
        }
    }

    #[test]
    fn native_capture_rejects_unsupported_skin_instead_of_recording_bind_pose() {
        let mut model = model();
        model.meshes[0].vertex_influences = Some(vec![super::super::prelude::W3dVertInfStruct {
            bone_idx: 1,
            pad: [0; 6],
        }]);
        assert!(
            model
                .capture_native_ghost_render_object(
                    Mat4::IDENTITY,
                    1.0,
                    0,
                    None,
                    0.0,
                    &AuthoredDrawPrimaryTurret::default(),
                    0.0,
                    0.0,
                    &[],
                    &[],
                    false,
                )
                .is_none()
        );
    }
}
