use super::*;
use crate::assets::models::BlendMode;

fn bridge_test_pivot(name: &str, parent_idx: u32) -> crate::assets::W3dPivot {
    crate::assets::W3dPivot {
        name: name.to_string(),
        parent_idx,
        translation: [0.0; 3],
        euler_angles: [0.0; 3],
        rotation: [0.0, 0.0, 0.0, 1.0],
    }
}

fn bridge_test_animation(name: &str, x: f32) -> crate::assets::W3dAnimation {
    crate::assets::W3dAnimation {
        name: name.to_string(),
        hierarchy_name: "RANGER_SKL".to_string(),
        num_frames: 2,
        frame_rate: 30,
        source_is_compressed: false,
        channels: vec![crate::assets::W3dAnimChannel {
            first_frame: 0,
            last_frame: 1,
            vector_len: 1,
            flags: 0,
            pivot: 1,
            data: vec![x, x],
        }],
        raw_visibility_channels: Vec::new(),
        unsupported_visibility_pivots: Vec::new(),
    }
}

#[test]
fn visual_world_state_reset_clears_object_timeline_components() {
    use std::collections::HashMap;

    let mut animation_states = HashMap::from([(
        (1_u32, 2_u32),
        ObjectVisualState {
            animation: Some(ObjectAnimationState {
                animation_binding_key: None,
                animation_identity: "test.bind".to_string(),
                current_frame: 7.5,
                frame_rate: 30.0,
                num_frames: 12,
                mode: crate::assets::AuthoredDrawAnimationMode::Loop,
                last_sync_ms: 0,
                previous_frame: 0.,
                frame_rate_multiplier: 1.,
                direction: 1.,
                playback_mode: crate::assets::AuthoredDrawAnimationMode::Loop,
            }),
            ..Default::default()
        },
    )]);
    let mut stale_render_items = vec!["old-world-item"];
    let mut current_pass = Some(RenderPass::ForwardOpaque);
    let mut last_frame_time = 17.25;

    clear_visual_world_state_components(
        &mut animation_states,
        &mut stale_render_items,
        &mut current_pass,
        &mut last_frame_time,
    );

    assert!(
        animation_states.is_empty(),
        "raw object-id timelines must reset"
    );
    assert!(
        stale_render_items.is_empty(),
        "old-world submissions must drop"
    );
    assert_eq!(current_pass, None);
    assert_eq!(last_frame_time, 0.0, "new world starts with a fresh delta");
}

#[test]
fn frozen_direct_shroud_sidecar_culls_only_a_matching_direct_host_candidate() {
    use crate::presentation_frame::{
        PresentationDrawableShroudFacts, PresentationObjectShroudStatus,
    };
    use std::collections::HashMap;

    let direct_id = ObjectID(41);
    let other_id = ObjectID(42);
    let host_epoch = 19;
    let sidecar = HashMap::from([(
        direct_id,
        FrozenDirectDrawableShroudState {
            host_epoch,
            object_id: direct_id,
            drawable_id: 71,
            binding_generation: 23,
            scene_effectively_hidden: false,
            fully_obscured: true,
        },
    )]);
    let direct_facts = PresentationDrawableShroudFacts::direct_host_object(
        PresentationObjectShroudStatus::Fogged,
        false,
    );

    assert!(frozen_direct_candidate_is_fully_obscured(
        &sidecar,
        Some(host_epoch),
        direct_id,
        direct_facts,
    ));
    assert!(
        !frozen_direct_candidate_is_fully_obscured(
            &sidecar,
            Some(host_epoch),
            other_id,
            direct_facts,
        ),
        "a sidecar record must not leak to another direct object"
    );
    assert!(
        !frozen_direct_candidate_is_fully_obscured(
            &sidecar,
            Some(host_epoch),
            direct_id,
            PresentationDrawableShroudFacts::default(),
        ),
        "GameWorld-only/objectless records must not inherit direct Drawable state"
    );
    assert!(
        !frozen_direct_candidate_is_fully_obscured(
            &sidecar,
            Some(host_epoch + 1),
            direct_id,
            direct_facts,
        ),
        "a same-ID sidecar from an earlier direct visual world must fail open"
    );

    let scene_hidden = HashMap::from([(
        direct_id,
        FrozenDirectDrawableShroudState {
            scene_effectively_hidden: true,
            fully_obscured: false,
            ..sidecar[&direct_id]
        },
    )]);
    assert!(
        frozen_direct_candidate_is_scene_culled(
            &scene_hidden,
            Some(host_epoch),
            direct_id,
            direct_facts,
        ),
        "C++ hidden/stealth scene state must cull before model load"
    );
    assert!(
        !frozen_direct_candidate_is_fully_obscured(
            &scene_hidden,
            Some(host_epoch),
            direct_id,
            direct_facts,
        ),
        "source-hidden and fully-obscured are distinct C++ Visibility_Check predicates"
    );
    assert!(
        frozen_direct_scene_candidate(&scene_hidden, Some(host_epoch), direct_id, direct_facts)
            .is_none(),
        "a source-hidden direct drawable cannot reach the clear-frame writer"
    );
}

#[test]
fn objectless_drawable_controller_status_is_exact_and_not_alpha_derived() {
    use gamelogic::common::types::ObjectShroudStatus;

    assert_eq!(
        objectless_drawable_scene_status(None),
        ObjectShroudStatus::Clear
    );
    assert_eq!(
        objectless_drawable_scene_status(Some(ObjectShroudStatus::Clear)),
        ObjectShroudStatus::Clear
    );
    assert_eq!(
        objectless_drawable_scene_status(Some(ObjectShroudStatus::PartialClear)),
        ObjectShroudStatus::Clear
    );
    assert_eq!(
        objectless_drawable_scene_status(Some(ObjectShroudStatus::Invalid)),
        ObjectShroudStatus::Clear
    );
    assert_eq!(
        objectless_drawable_scene_status(Some(ObjectShroudStatus::Fogged)),
        ObjectShroudStatus::Shrouded
    );
    assert_eq!(
        objectless_drawable_scene_status(Some(ObjectShroudStatus::Shrouded)),
        ObjectShroudStatus::Shrouded
    );
}

#[test]
fn frozen_direct_scene_candidate_preserves_raw_facts_for_current_visible_binding() {
    use crate::presentation_frame::{
        PresentationDrawableShroudFacts, PresentationObjectShroudStatus,
    };
    use std::collections::HashMap;

    let object_id = ObjectID(52);
    let host_epoch = 31;
    let facts = PresentationDrawableShroudFacts::direct_host_object(
        PresentationObjectShroudStatus::Shrouded,
        true,
    );
    let sidecar = HashMap::from([(
        object_id,
        FrozenDirectDrawableShroudState {
            host_epoch,
            object_id,
            drawable_id: 17,
            binding_generation: 8,
            scene_effectively_hidden: false,
            fully_obscured: false,
        },
    )]);

    let candidate = frozen_direct_scene_candidate(&sidecar, Some(host_epoch), object_id, facts)
        .expect("current visible direct binding must reach the scene ledger");
    assert_eq!(candidate.host_epoch, host_epoch);
    assert_eq!(candidate.object_id, object_id);
    assert_eq!(candidate.drawable_id, 17);
    assert_eq!(candidate.binding_generation, 8);
    assert_eq!(
        candidate.raw_status,
        gamelogic::common::types::ObjectShroudStatus::Shrouded,
        "the scene callback must receive the frozen C++ ordinal, not FOW alpha"
    );
    assert!(candidate.effectively_dead);
    assert!(
        frozen_direct_scene_candidate(&sidecar, Some(host_epoch + 1), object_id, facts).is_none(),
        "a same-ID binding from another host visual world must not refresh a Drawable"
    );

    let fully_obscured = HashMap::from([(
        object_id,
        FrozenDirectDrawableShroudState {
            fully_obscured: true,
            ..sidecar[&object_id]
        },
    )]);
    assert!(
        frozen_direct_scene_candidate(&fully_obscured, Some(host_epoch), object_id, facts)
            .is_none(),
        "the scene ledger starts only after the same frozen fully-obscured cull"
    );
}

#[test]
fn direct_scene_decisions_validate_full_binding_drop_hidden_and_retain_scene_status() {
    use crate::assets::W3DMaterial;
    use crate::graphics::render_item::{
        FrozenDirectSceneShroudRenderState, RenderItem, RenderItemOwner,
    };
    use glam::{Mat4, Vec3};
    use std::collections::HashMap;

    let rendered_object = ObjectID(61);
    let hidden_object = ObjectID(62);
    let host_epoch = 44;
    let sidecar = HashMap::from([
        (
            rendered_object,
            FrozenDirectDrawableShroudState {
                host_epoch,
                object_id: rendered_object,
                drawable_id: 17,
                binding_generation: 9,
                scene_effectively_hidden: false,
                fully_obscured: false,
            },
        ),
        (
            hidden_object,
            FrozenDirectDrawableShroudState {
                host_epoch,
                object_id: hidden_object,
                drawable_id: 18,
                binding_generation: 10,
                scene_effectively_hidden: false,
                fully_obscured: false,
            },
        ),
    ]);
    let candidates = vec![
        FrozenDirectDrawableSceneCandidate {
            host_epoch,
            object_id: rendered_object,
            drawable_id: 17,
            binding_generation: 9,
            raw_status: gamelogic::common::types::ObjectShroudStatus::Fogged,
            effectively_dead: false,
        },
        FrozenDirectDrawableSceneCandidate {
            host_epoch,
            object_id: hidden_object,
            drawable_id: 18,
            binding_generation: 10,
            raw_status: gamelogic::common::types::ObjectShroudStatus::Clear,
            effectively_dead: false,
        },
    ];
    let item = |object_id| {
        RenderItem::new(
            object_id,
            "direct-scene-test".to_string(),
            0,
            Vec3::ZERO,
            Mat4::IDENTITY,
            &W3DMaterial::default(),
            RenderPass::ForwardOpaque,
        )
    };
    let mut render_items = vec![
        item(rendered_object),
        item(hidden_object),
        RenderItem::new_presentation_projectile(
            rendered_object,
            "projectile-sharing-rendered-object-id".to_string(),
            0,
            Vec3::ZERO,
            Mat4::IDENTITY,
            &W3DMaterial::default(),
            RenderPass::ForwardOpaque,
        ),
        RenderItem::new_presentation_projectile(
            hidden_object,
            "projectile-sharing-hidden-object-id".to_string(),
            0,
            Vec3::ZERO,
            Mat4::IDENTITY,
            &W3DMaterial::default(),
            RenderPass::ForwardOpaque,
        ),
        RenderItem::new_unbound_client_drawable(
            77,
            "standalone".to_string(),
            0,
            Vec3::ZERO,
            Mat4::IDENTITY,
            &W3DMaterial::default(),
            RenderPass::ForwardOpaque,
        ),
    ];

    apply_frozen_direct_scene_decisions_to_render_items(
        &mut render_items,
        &sidecar,
        Some(host_epoch),
        &candidates,
        [
            FrozenDirectDrawableSceneDecision {
                host_epoch,
                object_id: rendered_object,
                drawable_id: 17,
                binding_generation: 9,
                outcome: FrozenDirectDrawableSceneOutcome::RenderDrawable {
                    final_status: gamelogic::common::types::ObjectShroudStatus::PartialClear,
                    pushes_projected_shroud_pass: true,
                },
            },
            FrozenDirectDrawableSceneDecision {
                host_epoch,
                object_id: hidden_object,
                drawable_id: 18,
                binding_generation: 10,
                outcome: FrozenDirectDrawableSceneOutcome::HiddenDirectDrawable,
            },
            // A same-object decision for a different binding cannot hide or
            // stamp a replacement drawable merely because ObjectID matches.
            FrozenDirectDrawableSceneDecision {
                host_epoch,
                object_id: rendered_object,
                drawable_id: 17,
                binding_generation: 99,
                outcome: FrozenDirectDrawableSceneOutcome::HiddenDirectDrawable,
            },
        ],
    );

    assert!(
        !render_items
            .iter()
            .any(|item| item.owner == RenderItemOwner::Object(hidden_object)),
        "the exact hidden direct binding must leave no object-owned render item"
    );
    let rendered = render_items
        .iter()
        .find(|item| item.owner == RenderItemOwner::Object(rendered_object))
        .expect("the stale replacement decision must not remove the current item");
    assert_eq!(
        rendered.frozen_direct_scene_shroud,
        Some(FrozenDirectSceneShroudRenderState {
            final_status: gamelogic::common::types::ObjectShroudStatus::PartialClear,
            pushes_projected_shroud_pass: true,
        }),
        "hq-1a1 receives the exact scene status/pass decision, not a FOW approximation"
    );
    assert!(
        render_items.iter().any(|item| {
            item.owner == RenderItemOwner::UnboundClientDrawable(77)
                && item.frozen_direct_scene_shroud.is_none()
        }),
        "objectless client drawables must not inherit a direct object decision"
    );
    assert!(
        render_items.iter().any(|item| {
            item.owner == RenderItemOwner::PresentationProjectile(rendered_object)
                && item.frozen_direct_scene_shroud.is_none()
        }),
        "a presentation projectile sharing an ObjectID must not inherit a direct projected-shroud decision"
    );
    assert!(
        render_items.iter().any(|item| {
            item.owner == RenderItemOwner::PresentationProjectile(hidden_object)
                && item.frozen_direct_scene_shroud.is_none()
        }),
        "a presentation projectile sharing an ObjectID must not be removed by a direct hidden decision"
    );
    assert!(
        !frozen_direct_scene_outcome_has_valid_pass_eligibility(
            FrozenDirectDrawableSceneOutcome::RenderDrawable {
                final_status: gamelogic::common::types::ObjectShroudStatus::PartialClear,
                pushes_projected_shroud_pass: false,
            }
        ),
        "the projected-pass flag must stay coupled to the frozen C++ status ordinal"
    );
}

#[test]
fn cull_radius_preserves_subunit_direct_visual_scale_for_bounds_and_fallback() {
    let world_matrix = Mat4::from_scale(Vec3::splat(0.7));
    let world_scale = RenderPipeline::world_cull_scale(world_matrix);
    assert!(
        (world_scale - 0.7).abs() < f32::EPSILON,
        "a valid direct disguise scale must not be inflated to 1.0"
    );

    assert!(
        (RenderPipeline::scaled_world_cull_radius(Some(20.0), 10.0, world_scale) - 14.0).abs()
            < f32::EPSILON,
        "known local W3D bounds must scale by the authored 0.7 transform"
    );
    assert!(
        (RenderPipeline::scaled_world_cull_radius(Some(4.0), 10.0, world_scale) - 7.0).abs()
            < f32::EPSILON,
        "the scaled fallback remains the lower bound when known local bounds are smaller"
    );
    assert!(
        (RenderPipeline::scaled_world_cull_radius(None, 10.0, world_scale) - 7.0).abs()
            < f32::EPSILON,
        "missing model bounds must retain the same scaled fallback radius"
    );

    let invalid_scale = RenderPipeline::world_cull_scale(Mat4::from_cols(
        glam::Vec4::new(f32::NAN, 0.0, 0.0, 0.0),
        glam::Vec4::Y,
        glam::Vec4::Z,
        glam::Vec4::W,
    ));
    assert_eq!(
        invalid_scale, 1.0,
        "non-finite world scale must fail safely to a finite cull radius"
    );
}

#[test]
fn w3d_companion_animation_forward_palette_uses_render_item_binding() {
    use crate::assets::{W3DModel, W3dAnimationBinding, W3dHierarchy};
    use std::sync::Arc;

    let mut geometry = W3DModel::new("ranger_geometry".to_string());
    geometry.hierarchy = Some(W3dHierarchy {
        name: "RANGER_SKL".to_string(),
        pivots: vec![
            bridge_test_pivot("ROOT", u32::MAX),
            bridge_test_pivot("ARM", 0),
        ],
        pivot_fixups: Vec::new(),
    });
    // The geometry happens to contain a local clip zero, but it is not the
    // frozen Draw selection. ForwardPass must not use this ordinal fallback.
    geometry
        .animations
        .push(bridge_test_animation("LOCAL", 1.0));
    let companion = W3dAnimationBinding::companion(
        "RANGER_SKL.RUN",
        Arc::new(bridge_test_animation("RUN", 9.0)),
    );
    assert!(geometry.animation_binding_is_compatible(&companion));

    let material = W3DMaterial::default();
    let mut item = RenderItem::new(
        ObjectID(77),
        "ranger_geometry".to_string(),
        0,
        Vec3::ZERO,
        Mat4::IDENTITY,
        &material,
        RenderPass::ForwardOpaque,
    );
    item.animation_frame = 1.0;
    item.animation_binding = Some(companion);

    let palette = ForwardPass::sample_bone_palette_for_item(&geometry, &item)
        .expect("the frozen companion binding must reach the final GPU palette");
    assert_eq!(palette[1].w_axis.x, 9.0);
    assert_ne!(palette[1].w_axis.x, 1.0, "must not sample local clip zero");

    item.animation_binding = None;
    assert!(
        ForwardPass::sample_bone_palette_for_item(&geometry, &item).is_none(),
        "an absent Draw animation is bind pose and must not upload clip zero"
    );
}

#[test]
fn bridge_capture_controls_apply_to_the_same_gpu_palette_as_rigid_hlod_children() {
    use crate::assets::{
        W3DMesh, W3DModel, W3dAnimationBinding, W3dHierarchy, W3dHlod, W3dHlodLod, W3dHlodSubObject,
    };

    let mut geometry = W3DModel::new("capture_controls".to_string());
    geometry.hierarchy = Some(W3dHierarchy {
        name: "RANGER_SKL".to_string(),
        pivots: vec![
            bridge_test_pivot("ROOT", u32::MAX),
            bridge_test_pivot("TURRET", 0),
        ],
        pivot_fixups: Vec::new(),
    });
    geometry.hierarchy.as_mut().expect("test HTree").pivots[0].translation = [41.0, 0.0, 0.0];
    geometry.hlods.push(W3dHlod {
        version: 0x0001_0000,
        name: "CAPTURE_HLOD".to_string(),
        hierarchy_name: "RANGER_SKL".to_string(),
        lods: vec![W3dHlodLod {
            max_screen_size: f32::MAX,
            subobjects: vec![W3dHlodSubObject {
                name: "CAPTURE_HLOD.TurretMesh".to_string(),
                bone_index: 1,
            }],
        }],
        aggregates: None,
        proxies: None,
        has_unrendered_aggregates: false,
        has_invalid_trailing_records: false,
    });
    let mut turret_mesh = W3DMesh::new("TurretMesh".to_string());
    turret_mesh.container_name = "CAPTURE_HLOD".to_string();
    geometry.meshes.push(turret_mesh);
    geometry.animations.push(bridge_test_animation("IDLE", 0.0));
    geometry.animations[0]
        .channels
        .push(crate::assets::W3dAnimChannel {
            first_frame: 0,
            last_frame: 1,
            vector_len: 1,
            flags: 0,
            pivot: 0,
            data: vec![0.0, 99.0],
        });

    let material = W3DMaterial::default();
    let mut item = RenderItem::new(
        ObjectID(78),
        "capture_controls".to_string(),
        0,
        Vec3::ZERO,
        Mat4::IDENTITY,
        &material,
        RenderPass::ForwardOpaque,
    );
    item.animation_binding = Some(W3dAnimationBinding::local(0));
    item.capture_bone_controls = vec![
        (1, Mat4::from_translation(Vec3::new(1.0, 0.0, 0.0))),
        // C++ Control_Bone overwrites the captured relative transform for a
        // duplicate pivot; the later translation must win.
        (1, Mat4::from_translation(Vec3::new(2.0, 0.0, 0.0))),
    ];

    let palette = ForwardPass::sample_bone_palette_for_item(&geometry, &item)
        .expect("valid frozen bridge controls should reach the GPU palette");
    assert_eq!(
        palette[0],
        Mat4::IDENTITY,
        "HTree pivot zero is the external object root, not source bind/HAnim data"
    );
    assert_eq!(palette[1].w_axis.x, 2.0);

    let (rigid_transform, _) = geometry
        .mesh_local_transform_and_visibility_for_binding_and_capture_controls(
            0,
            item.animation_binding.as_ref(),
            item.animation_frame,
            &item.capture_bone_controls,
        )
        .expect("the same valid control must reach the rigid HLOD child");
    assert_eq!(rigid_transform.w_axis.x, palette[1].w_axis.x);

    item.capture_bone_controls = vec![(0, Mat4::IDENTITY)];
    assert!(
        ForwardPass::sample_bone_palette_for_item(&geometry, &item).is_none(),
        "C++ pivot zero is the no-bone sentinel and must fail closed"
    );
}

#[test]
fn frozen_fow_fogged_render_item_preserves_all_fields_to_the_mesh_bind_contract() {
    use crate::fow_rendering::ObjectVisibility;
    use ww3d_renderer_3d::rendering::mesh_system::FrozenFowVisibility;

    let fields =
        crate::graphics::fow_uniform_integration::frozen_fow_model_fields(ObjectVisibility::FOGGED);
    let frozen = FrozenFowVisibility::new(
        fields.visibility_alpha,
        fields.visibility_falloff,
        fields.is_explored,
    );

    assert_eq!(
        frozen.model_uniform_values(),
        (
            ObjectVisibility::FOGGED.visibility_alpha,
            ObjectVisibility::FOGGED.visibility_falloff,
            ObjectVisibility::FOGGED.is_explored,
        )
    );

    // The active final bind consumes this exact tuple for every material pass.
}

#[test]
fn material_pass_classifies_transparent_blend_modes() {
    let mut material = W3DMaterial::default();
    assert_eq!(
        RenderPipeline::render_pass_for_material(&material),
        RenderPass::ForwardOpaque
    );

    material.blend_mode = BlendMode::Alpha;
    assert_eq!(
        RenderPipeline::render_pass_for_material(&material),
        RenderPass::ForwardTransparent
    );

    material.blend_mode = BlendMode::Additive;
    assert_eq!(
        RenderPipeline::render_pass_for_material(&material),
        RenderPass::ForwardTransparent
    );
}

#[test]
fn material_pass_classifies_partial_opacity_as_transparent() {
    let mut material = W3DMaterial::default();
    material.opacity = 0.75;
    assert_eq!(
        RenderPipeline::render_pass_for_material(&material),
        RenderPass::ForwardTransparent
    );
}

#[test]
fn missing_model_debug_cubes_are_opt_in_only() {
    assert!(!RenderPipeline::missing_model_debug_cubes_enabled_from(
        None
    ));
    assert!(!RenderPipeline::missing_model_debug_cubes_enabled_from(
        Some(std::ffi::OsStr::new("0"))
    ));
    assert!(!RenderPipeline::missing_model_debug_cubes_enabled_from(
        Some(std::ffi::OsStr::new("false"))
    ));
    assert!(RenderPipeline::missing_model_debug_cubes_enabled_from(
        Some(std::ffi::OsStr::new("1"))
    ));
    assert!(RenderPipeline::missing_model_debug_cubes_enabled_from(
        Some(std::ffi::OsStr::new("TRUE"))
    ));
    assert!(RenderPipeline::missing_model_debug_cubes_enabled_from(
        Some(std::ffi::OsStr::new("on"))
    ));
}

#[test]
fn transparent_items_sort_back_to_front() {
    let mut mat = W3DMaterial::default();
    mat.blend_mode = BlendMode::Alpha;

    let mut far = RenderItem::new(
        ObjectID(1),
        "Model".to_string(),
        0,
        Vec3::new(0.0, 0.0, 100.0),
        Mat4::IDENTITY,
        &mat,
        RenderPass::ForwardTransparent,
    );
    far.distance = 100.0;

    let mut near = RenderItem::new(
        ObjectID(2),
        "Model".to_string(),
        0,
        Vec3::new(0.0, 0.0, 10.0),
        Mat4::IDENTITY,
        &mat,
        RenderPass::ForwardTransparent,
    );
    near.distance = 10.0;

    assert_eq!(
        RenderPipeline::compare_render_items(&far, &near),
        std::cmp::Ordering::Less
    );
}

#[test]
fn compare_render_items_tiebreaks_by_object_id_for_determinism() {
    let mat = W3DMaterial::default();
    let mut a = RenderItem::new(
        ObjectID(7),
        "Model".to_string(),
        0,
        Vec3::ZERO,
        Mat4::IDENTITY,
        &mat,
        RenderPass::ForwardOpaque,
    );
    let mut b = RenderItem::new(
        ObjectID(2),
        "Model".to_string(),
        0,
        Vec3::ZERO,
        Mat4::IDENTITY,
        &mat,
        RenderPass::ForwardOpaque,
    );

    a.distance = 0.0;
    b.distance = 0.0;
    a.material_key = "same".to_string();
    b.material_key = "same".to_string();

    assert_eq!(
        RenderPipeline::compare_render_items(&a, &b),
        std::cmp::Ordering::Greater
    );
    assert_eq!(
        RenderPipeline::compare_render_items(&b, &a),
        std::cmp::Ordering::Less
    );
}

#[test]
fn unit_render_collection_uses_presentation_frame_without_logic() {
    // Criterion: main unit mesh identity comes from PresentationFrame only.
    // Not full W3D retail — proves collect path does not need GameLogic for
    // position/model/selected when a frame is available.
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::presentation_frame::PresentationFrame;
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("UnitMeshPres");
    apply_skirmish_config(&mut logic, &cfg).expect("config");
    let mut t = ThingTemplate::new("PresMeshUnit");
    t.set_health(55.0);
    t.set_model("avhummer");
    t.add_kind_of(KindOf::Vehicle);
    t.add_kind_of(KindOf::Selectable);
    logic.templates.insert("PresMeshUnit".into(), t);
    let id = logic
        .create_object("PresMeshUnit", Team::USA, Vec3::new(15.0, 0.0, -3.0))
        .expect("unit");
    if let Some(o) = logic./* Wave 950 */ host_object_mut(id) {
        o.selected = true;
        o.status.selected = true;
        o.selection_radius = 14.0;
    }

    let snap = PresentationFrame::build_from_logic(&mut logic, 0);
    // Poison live world — unit collect must ignore it.
    if let Some(o) = logic.host_object_mut(id) {
        o.set_position(Vec3::new(777.0, 0.0, 777.0));
        o.selected = false;
        o.status.selected = false;
    }

    let inputs = RenderPipeline::collect_unit_render_inputs_from_presentation(&snap);
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].id, id);
    assert!((inputs[0].position.x - 15.0).abs() < 0.01);
    assert!((inputs[0].position.z + 3.0).abs() < 0.01);
    assert_eq!(inputs[0].model_key, "avhummer");
    assert_eq!(inputs[0].template_name, "PresMeshUnit");
    assert!(inputs[0].selected);
    assert!((inputs[0].selection_radius - 14.0).abs() < 0.01);
    assert!(!inputs[0].engine_bridged);
    // FOW is snapshot-owned on unit inputs (matches frame object FOW).
    assert_eq!(
        inputs[0].fow_visibility,
        snap.fow_for_object(id).expect("fow on frame")
    );

    // Structural: production collect prefers presentation unit pass + snapshot FOW.
}

#[test]
fn presentation_fow_never_explored_skip_is_snapshot_owned() {
    use crate::fow_rendering::ObjectVisibility;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::presentation_frame::{PresentationFrame, UnitRenderInput};
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("FowSnapSkip");
    apply_skirmish_config(&mut logic, &cfg).expect("config");
    let mut t = ThingTemplate::new("FowSkipUnit");
    t.set_health(40.0);
    t.add_kind_of(KindOf::Infantry);
    logic.templates.insert("FowSkipUnit".into(), t);
    let id = logic
        .create_object("FowSkipUnit", Team::China, Vec3::new(1.0, 0.0, 1.0))
        .expect("unit");

    let mut snap = PresentationFrame::build_from_logic(&mut logic, 0);
    // Force never-explored FOW on the owned snapshot (simulates post-build shroud).
    if let Some(ro) = snap.objects.iter_mut().find(|o| o.id == id) {
        ro.fow_visibility = ObjectVisibility::HIDDEN;
    }
    let inputs = RenderPipeline::collect_unit_render_inputs_from_presentation(&snap);
    assert_eq!(inputs.len(), 1);
    assert!(!inputs[0].fow_should_render());
    assert!(inputs[0].fow_visibility.never_explored());

    // Fogged (explored-not-visible) still renders with darkened alpha.
    let fogged = UnitRenderInput {
        fow_visibility: ObjectVisibility::FOGGED,
        drawable_shroud: Default::default(),
        ..inputs[0].clone()
    };
    assert!(fogged.fow_should_render());
    assert!((fogged.fow_visibility.visibility_alpha - 0.3).abs() < 0.01);
}
