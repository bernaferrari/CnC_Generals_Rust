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

    let collect = include_str!("pipeline_collect.rs");
    assert!(collect.contains("shroud_status_object_id"));
    assert!(collect.contains("frozen_objectless_drawable_shroud"));
    assert!(collect.contains("raw_status.as_game_logic_status()"));
    let objectless_body = collect
        .split_once("fn frozen_objectless_drawable_shroud_for_submission")
        .and_then(|(_, tail)| tail.split_once("pub(super) fn mesh_uv_override_for_submission"))
        .map(|(body, _)| body)
        .unwrap_or_default();
    assert!(
        !objectless_body.contains("visibility_alpha"),
        "objectless shroud status must not be inferred from FOW alpha"
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
fn frozen_direct_shroud_sidecar_stays_at_the_render_boundary_and_before_model_load() {
    let engine = crate::cnc_game_engine::ENGINE_SRC;
    let render = &engine[engine
        .find("pub fn render(&mut self)")
        .expect("render entry")..];
    let ensure = render
        .find("host_ensure_presentation_frame_for_render()")
        .expect("presentation seed");
    let capture = render
        .find("let frozen_direct_shroud_states")
        .expect("direct shroud capture");
    let frame = render
        .find("set_presentation_frame(self.last_presentation_frame.clone())")
        .expect("pipeline frame handoff");
    let sidecar = render
        .find("set_presentation_direct_shroud_states(frozen_direct_shroud_states)")
        .expect("pipeline direct shroud handoff");
    assert!(ensure < capture && capture < frame && frame < sidecar);
    let capture_window = &render[capture..sidecar];
    assert!(
        capture_window.contains("pres.direct_host_drawables")
            && capture_window.contains("direct.resident")
            && capture_window.contains("presentation_direct_drawable_state(")
            && capture_window.contains("binding_generation"),
        "only frozen resident direct records with GameClient's full guarded binding query may enter"
    );
    assert!(
        !capture_window.contains("!object.destroyed"),
        "direct visual residency cannot be derived from gameplay destruction"
    );

    let collect = include_str!("pipeline_collect.rs");
    let candidate = collect
        .split_once("let (cull_center, cull_radius) =")
        .expect("presentation candidate cull sphere")
        .1;
    let frustum = candidate
        .find("world_sphere_in_expanded_frustum(")
        .expect("frozen frustum candidate test");
    let direct_cull = candidate
        .find("frozen_direct_candidate_is_scene_culled(")
        .expect("direct scene sidecar cull");
    let model_load = candidate
        .find("let render_model_load_result = Self::ensure_render_model_loaded(")
        .expect("model load");
    assert!(frustum < direct_cull && direct_cull < model_load);
    let cull_window = &candidate[direct_cull..model_load];
    assert!(
        !cull_window.contains("game_client") && !cull_window.contains("evaluate_scene_direct"),
        "collection must read its immutable sidecar only; scene timing remains outside this seam"
    );
    let direct_ledger_window = &candidate[direct_cull
        ..candidate
            .find("RenderModelLoadResult::SkippedByBudget")
            .expect("direct source-model collection boundary")];
    assert!(
        !direct_ledger_window.contains("presentation_direct_drawable_state(")
            && !direct_ledger_window.contains("evaluate_frozen_direct_scene_candidate(")
            && !direct_ledger_window.contains("evaluate_frozen_direct_scene_shroud_candidates("),
        "direct collection must expose only a frozen candidate ledger; Main owns the GameClient callback"
    );

    let lifecycle = include_str!("pipeline_lifecycle.rs");
    assert!(
        lifecycle.contains("self.presentation_direct_shroud_states.clear();"),
        "ordinary frame handoff and world invalidation must discard stale direct associations"
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
fn direct_scene_candidate_ledger_follows_real_item_production_before_forward_sort() {
    let collect = include_str!("pipeline_collect.rs");
    let ready = collect
        .split_once("RenderModelLoadResult::Ready(w3d_model) =>")
        .map(|(_, ready)| ready)
        .expect("ready-model branch");
    let ready = ready
        .split_once("RenderModelLoadResult::SkippedByBudget")
        .map(|(ready, _)| ready)
        .expect("first ready-model branch must end at the load-budget branch");
    let item_count_before = ready
        .find("let render_item_count_before_model")
        .expect("candidate timing must start from the item count");
    let item_push = ready
        .find("self.render_items.push(render_item)")
        .expect("source W3D item append");
    let real_item_gate = ready
        .find("self.render_items.len() > render_item_count_before_model")
        .expect("candidate must require a real source item");
    let candidate_push = ready
        .find("direct_scene_candidates.push(candidate)")
        .expect("candidate ledger append");
    let fallback = ready
        .find("self.debug_last_zero_mesh_models")
        .expect("fallback/debug path");
    assert!(
        item_count_before < item_push
            && item_push < real_item_gate
            && real_item_gate < candidate_push
            && candidate_push < fallback,
        "scene candidates must be created only after source W3D item production, never by fallback geometry"
    );
    assert!(
        ready.contains("direct_scene_candidate_bindings.insert(binding_key)"),
        "multiple Draw modules/meshes must collapse to one full Drawable binding"
    );

    let execute = include_str!("pipeline_execute.rs");
    let collect_call = execute
        .find("self.collect_render_items(")
        .expect("execute collection call");
    let resolver = execute[collect_call..]
        .find("resolver(&direct_scene_candidates)")
        .map(|at| collect_call + at)
        .expect("Main-owned candidate resolver");
    let apply = execute[resolver..]
        .find("apply_frozen_direct_scene_decisions_to_render_items(")
        .map(|at| resolver + at)
        .expect("validated Main-owned decisions must be applied");
    let sort = execute
        .find("self.sort_render_items()")
        .expect("forward sort");
    assert!(
        collect_call < resolver && resolver < apply && apply < sort,
        "Main resolves and applies the transient candidate ledger after collection and before sort/forward"
    );
    assert!(
        execute.contains("Main owns this callback")
            && execute.contains("Vec<FrozenDirectDrawableSceneDecision>")
            && !execute.contains("game_client::")
            && !execute.contains("evaluate_frozen_direct_scene_candidate("),
        "RenderPipeline must stay GameClient-free; input owns callback resolution and returns Main data"
    );

    let pipeline = include_str!("mod.rs");
    let apply_body = pipeline
        .split_once("fn apply_frozen_direct_scene_decisions_to_render_items")
        .map(|(_, body)| body)
        .and_then(|body| body.split_once("/// Main render pipeline"))
        .map(|(body, _)| body)
        .expect("direct decision application body");
    assert!(
        apply_body.contains("frozen_direct_scene_decision_matches_current_binding")
            && apply_body.contains("RenderItemOwner::Object")
            && apply_body.contains("HiddenDirectDrawable")
            && apply_body.contains("set_frozen_direct_scene_shroud"),
        "only full-keyed direct bindings may remove or stamp object-owned items"
    );
    assert!(
        !apply_body.contains("visibility_alpha"),
        "scene status/pass application must not infer anything from FOW alpha"
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
fn w3d_companion_animation_forward_path_has_no_clip_zero_fallback() {
    let forward = include_str!("forward_render.rs");
    assert!(forward.contains("sample_bone_palette_for_item"));
    assert!(forward.contains("item.animation_binding.as_ref()"));
    assert!(
        !forward.contains("sample_animation(0"),
        "the GPU palette path must consume RenderItem.animation_binding exclusively"
    );
}

#[test]
fn frozen_fow_forward_mesh_bind_is_from_render_item() {
    let forward = include_str!("forward_render.rs");
    let body = forward
        .split_once("fn prepare_mesh_instance")
        .expect("forward mesh construction must exist")
        .1;

    assert!(body.contains("frozen_fow_model_fields(item.fow_visibility)"));
    assert!(body.contains("mesh.set_frozen_fow_visibility(FrozenFowVisibility::new("));
    assert!(body.contains("frozen_fow.visibility_alpha"));
    assert!(body.contains("frozen_fow.visibility_falloff"));
    assert!(body.contains("frozen_fow.is_explored"));
    assert!(
        !body.contains("FOWRenderingBridge"),
        "the active draw handoff must not query FOW after collection"
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
    let mesh_renderer = include_str!(
        "../../../../Libraries/Source/WWVegas/WW3D2/crates/ww3d-renderer-3d/src/rendering/mesh_system_impl/render_manager.rs"
    );
    assert!(mesh_renderer.contains("mesh.frozen_fow_visibility().model_uniform_values()"));
    assert!(mesh_renderer.contains("Some(visibility_alpha)"));
    assert!(mesh_renderer.contains("Some(visibility_falloff)"));
    assert!(mesh_renderer.contains("Some(is_explored)"));
}

#[test]
fn frozen_fow_hidden_render_items_skip_before_mesh_construction_or_queueing() {
    let forward = include_str!("forward_render.rs");
    let render = forward
        .split_once("pub(super) fn render(")
        .expect("forward render loop must exist")
        .1
        .split_once("pub(super) fn prepare_mesh_instance")
        .expect("mesh construction helper must follow the render loop")
        .0;

    // Live guard (forward_render.rs render loop): ghosts are FOW-exempt by
    // design; hidden non-ghost items skip before mesh construction/queueing.
    let hidden_skip = render
        .find("if !is_ghost && item.fow_visibility.visibility_alpha <= 0.01 {")
        .expect("hidden items must be skipped in the render loop");
    let prepare = render
        .find("self.prepare_mesh_instance(graphics_system, item)")
        .expect("render loop must construct eligible mesh instances");
    let queue = render
        .find("renderer.queue_mesh(mesh)")
        .expect("render loop must queue eligible mesh instances");

    assert!(
        hidden_skip < prepare && prepare < queue,
        "hidden FOW must bypass both mesh construction and material binding"
    );
}

#[test]
fn presentation_live_fallback_reads_honesty_counter_present() {
    let src = crate::graphics::render_pipeline::RENDER_PIPELINE_SRC;
    assert!(
        src.contains("debug_last_presentation_live_fallback_reads")
            && src.contains("last_presentation_live_fallback_reads"),
        "presentation dual-read honesty counter must exist"
    );
    // Live FOW/shell/bounds fallbacks must be presentation-first if/else (no or_else dual-read).
    let forbidden_shell_dual = {
        // Split so include_str!(this file) does not match the production dual-read form.
        let a = ".unwrap_or_else(|| game_logic.map(|";
        let b = "g| g.isInShellGame()).unwrap_or(false))";
        format!("{a}{b}")
    };
    assert!(
        !src.contains(&forbidden_shell_dual),
        "shell FOW bypass must not dual-read via unwrap_or_else when presentation present"
    );
    assert!(
        src.contains("if let Some(p) = self.presentation_frame.as_ref()")
            || src.contains("if let Some(p) = presentation.as_ref()")
            || src.contains("self.presentation_frame.as_ref()"),
        "presentation-first branching required for dual-read residual sites"
    );
}

#[test]
fn unit_mesh_collect_is_presentation_only() {
    let src = crate::graphics::render_pipeline::RENDER_PIPELINE_SRC;
    let body = src
        .split("fn collect_render_items")
        .nth(1)
        .and_then(|s| {
            s.split(
                "
    fn ",
            )
            .next()
        })
        .expect("collect body");
    assert!(
        body.contains("unit_render_inputs()")
            && body.contains("presentation-owned inputs only")
            && !body.contains("UnitPassSource::Live")
            && !body.contains("get_objects().get"),
        "unit mesh collect must be presentation-only (no live object dual-read)"
    );
    // Counter remains for honesty gates even if always zero on presentation path.
    assert!(
        src.contains("debug_last_live_unit_identity_reads"),
        "live-identity honesty counter must remain"
    );
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

    let snap = PresentationFrame::build_from_logic(&logic, 0);
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
    let src = crate::graphics::render_pipeline::RENDER_PIPELINE_SRC;
    assert!(
        src.contains("unit_render_inputs()"),
        "collect_render_items must iterate presentation unit_render_inputs"
    );
    assert!(
        src.contains("presentation_unit_pass"),
        "collect_render_items must gate live identity behind presentation_unit_pass"
    );
    assert!(
        src.contains("fow_shell_bypass") && src.contains("snapshot_fow"),
        "collect_render_items must apply presentation FOW without live shroud re-query"
    );
}
#[test]
fn presentation_unit_pass_records_zero_live_identity_reads() {
    // Structural: Live branch is the only counter bump; presentation maps UnitPassSource::Presentation only.
    let src = crate::graphics::render_pipeline::RENDER_PIPELINE_SRC;
    assert!(
        src.contains("debug_last_live_unit_identity_reads"),
        "must track live unit identity residual"
    );
    assert!(
        src.contains("UnitPassSource::Presentation"),
        "presentation path required"
    );
    // When presentation_unit_pass, pass_sources come only from unit_inputs map to Presentation.
    let idx = src
        .find("let pass_sources: Vec<UnitPassSource>")
        .expect("pass_sources");
    let window = &src[idx..idx + 500];
    assert!(
        window.contains("UnitPassSource::Presentation")
            && window.contains("presentation_unit_pass"),
        "pass_sources must gate on presentation_unit_pass: {window}"
    );
}
#[test]
fn collect_shell_fow_is_presentation_only() {
    let src = crate::graphics::render_pipeline::RENDER_PIPELINE_SRC;
    let idx = src
        .find("let bypass_fow = presentation")
        .expect("bypass_fow presentation");
    let window = &src[idx..idx + 280];
    assert!(
        window.contains("fow_shell_bypass") && !window.contains("isInShellGame"),
        "shell FOW must come only from presentation: {window}"
    );
}

#[test]
fn roads_and_minimap_are_presentation_only() {
    let src = crate::graphics::render_pipeline::RENDER_PIPELINE_SRC;
    let prod = src.split("#[cfg(test)]").next().unwrap_or(src);
    let roads = prod
        .split("fn sync_runtime_map_roads")
        .nth(1)
        .and_then(|s| s.split("\n    fn ").next())
        .expect("roads body");
    assert!(
        roads.contains("presentation_frame")
            && roads.contains("world_env")
            && roads.contains("set_runtime_map_road_segments")
            && !roads.contains("if road_segments.is_empty() && bridge_segments.is_empty()")
            && !roads.contains("terrain_road_segments_snapshot"),
        "roads must be presentation-only and still bake scorches when empty"
    );
    let mm = prod
        .split("fn build_minimap_terrain_base_texture")
        .nth(1)
        .and_then(|s| s.split("\n    fn ").next())
        .expect("minimap body");
    // Comment may mention terrain_height_at; require no live sample call.
    assert!(
        mm.contains("height_env") && !mm.contains("g.terrain_height_at"),
        "minimap heights must not dual-read live GameLogic"
    );
}

#[test]
fn prewarm_is_presentation_only() {
    let src = crate::graphics::render_pipeline::RENDER_PIPELINE_SRC;
    let body = src
        .split("fn prewarm_startup_models")
        .nth(1)
        .and_then(|s| {
            s.split(
                "
    fn ",
            )
            .next()
        })
        .expect("prewarm body");
    assert!(
        body.contains("presentation_frame.as_ref()")
            && body.contains("prewarm_template_names")
            && !body.contains("last_parsed_map_settings")
            && !body.contains("game_logic"),
        "prewarm must use presentation world_env only"
    );
}

#[test]
fn execute_accepts_optional_game_logic_for_presentation_only_path() {
    let src = crate::graphics::render_pipeline::RENDER_PIPELINE_SRC;
    let exec_at = src.find("pub fn execute(").expect("execute must exist");
    assert!(
        !src[exec_at..exec_at + 500].contains("game_logic: Option<&GameLogic>"),
        "execute must not take live GameLogic (presentation-only boundary)"
    );
    let cnc = crate::cnc_game_engine::ENGINE_SRC;
    let call_at = cnc
        .find("self.render_pipeline.execute(")
        .expect("engine execute call must exist");
    assert!(
        cnc.contains("PresentationFrame::build_from_logic")
            && cnc.contains("last_presentation_frame.is_none()")
            && !cnc[call_at..call_at + 350].contains("Some(&self.game_logic)")
            && !cnc[call_at..call_at + 350].contains("game_logic"),
        "engine must seed presentation and never pass live GameLogic into execute"
    );
}
#[test]
fn minimap_roads_heightmap_are_presentation_only() {
    let src = crate::graphics::render_pipeline::RENDER_PIPELINE_SRC;
    let prod = src.split("#[cfg(test)]").next().unwrap_or(src);
    assert!(prod.contains("fn refresh_minimap_terrain_base(&mut self)"));
    assert!(!prod.contains("refresh_minimap_terrain_base(&mut self, game_logic"));
    assert!(
        prod.contains("fn sync_runtime_map_roads(&mut self)")
            || prod.contains("pub fn sync_runtime_map_roads(&mut self)")
    );
    assert!(!prod.contains("sync_runtime_map_roads(&mut self, game_logic"));
    assert!(prod.contains("fn load_heightmap_from_runtime_terrain("));
    assert!(
        prod.contains("self.refresh_minimap_terrain_base()")
            && !prod.contains("self.refresh_minimap_terrain_base(game_logic)"),
        "minimap base refresh must be presentation-only"
    );
}

#[test]
fn light_environment_consumes_scene_dynamic_lights() {
    let src = crate::graphics::render_pipeline::RENDER_PIPELINE_SRC;
    let lights = src
        .split("fn build_light_environment")
        .nth(1)
        .and_then(|s| s.split("\n    fn ").next())
        .expect("light env body");
    assert!(
        lights.contains("scene_dynamic_lights") && lights.contains("LightClass::point"),
        "GPU light env must consume FXList createLightPulse pool: {lights}"
    );
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

    let mut snap = PresentationFrame::build_from_logic(&logic, 0);
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

#[test]
fn projectile_mesh_pass_uses_presentation_inputs() {
    let src = crate::graphics::render_pipeline::RENDER_PIPELINE_SRC;
    assert!(src.contains("projectile_render_inputs"));
    assert!(src.contains("Presentation projectile mesh residual"));
    assert!(src.contains("new_presentation_projectile"));
}

#[test]
fn execute_packs_presentation_fx_segments_from_frame() {
    let src = crate::graphics::render_pipeline::RENDER_PIPELINE_SRC;
    let i = src.find("pub fn execute").expect("execute");
    let body = &src[i..src.len().min(i + 3500)];
    assert!(
        body.contains("pack_presentation_laser_segments")
            && body.contains("pack_presentation_projectiles")
            && body.contains("pack_presentation_move_lines")
            && body.contains("pack_presentation_attack_lines")
            && body.contains("pack_presentation_floating_texts")
            && body.contains("pack_presentation_world_anims")
            && body.contains("pack_presentation_particle_systems"),
        "execute must pack presentation FX/order/UI/particle layout lines without GameLogic dual-read"
    );
    assert!(
        body.contains("debug_last_laser_segments_packed")
            && body.contains("debug_last_projectile_segments_packed"),
        "execute must record pack honesty counters"
    );
    assert!(
        body.contains("enqueue_laser_additive_draw"),
        "execute must issue the live additive laser draw after upload"
    );
    assert!(
        !body.contains("game_logic: Option<&GameLogic>") && !body.contains("&GameLogic"),
        "execute must stay presentation-only (no live GameLogic param)"
    );
}

#[test]
fn terrain_pass_clears_black_not_fog_color() {
    let src = crate::graphics::render_pipeline::RENDER_PIPELINE_SRC;
    let clear = src
        .split("fn terrain_clear_color")
        .nth(1)
        .and_then(|s| s.split("fn set_environment_lighting").next())
        .expect("terrain_clear_color");
    assert!(
        clear.contains("wgpu::Color::BLACK"),
        "C++ W3DDisplay.cpp:1859 Begin_Render clears black: {clear}"
    );
    assert!(
        !clear.contains("fog_color") && !clear.contains("cached_lighting"),
        "terrain pass must not peach-wipe the backbuffer with fog/sun: {clear}"
    );
    assert!(
        clear.contains("GENERALS_DEBUG_CLEAR_COLOR"),
        "debug green remains opt-in only"
    );
    let prewarm = src
        .split("fn update_and_enqueue_terrain_pass")
        .nth(1)
        .expect("terrain pass");
    assert!(
        prewarm.contains("self.terrain_clear_color()"),
        "terrain pass must use terrain_clear_color for the first LoadOp::Clear"
    );
}

#[test]
fn execute_composites_leftover_view_filters_and_camera_fade() {
    let src = crate::graphics::render_pipeline::RENDER_PIPELINE_SRC;
    assert!(
        src.contains("composite_live_view_filter"),
        "live execute must composite leftover View filters"
    );
    assert!(
        src.contains("tick_filter_fade"),
        "live execute must advance leftover View filter fade frames"
    );
    assert!(
        src.contains("record_camera_fade"),
        "live execute must blit scripted CAMERA_FADE after the 3D scene"
    );
}
