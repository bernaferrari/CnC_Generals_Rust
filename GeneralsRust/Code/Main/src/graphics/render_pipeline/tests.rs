use super::*;
use crate::assets::models::BlendMode;

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
            || src.contains("if let Some(p) = presentation.as_ref()"),
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
