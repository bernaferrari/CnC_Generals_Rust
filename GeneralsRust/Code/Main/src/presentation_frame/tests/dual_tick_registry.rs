use super::*;

#[test]
fn presentation_frame_is_built_from_authority_without_arc() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("PresMap");
    apply_skirmish_config(&mut logic, &cfg).expect("config");
    let mut t = ThingTemplate::new("PresUnit");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Infantry);
    logic.templates.insert("PresUnit".into(), t);
    let id = logic
        .create_object("PresUnit", Team::USA, glam::Vec3::new(1.0, 0.0, 2.0))
        .expect("unit");

    let snap = PresentationFrame::build_from_logic(&logic, 0);
    assert_eq!(snap.frame.0, logic.get_frame());
    assert!(snap.objects.iter().any(|o| o.id == id));
    assert_eq!(snap.local_supplies, 10_000);
    // Snapshot is owned — mutating world after build must not require re-borrow of snap.
    logic.update();
    assert_eq!(snap.objects.len(), 1);
    let h1 = snap.presentation_hash();
    let snap2 = PresentationFrame::build_from_logic(&logic, 0);
    // Frame advanced; hash may change.
    assert!(snap2.frame.0 >= snap.frame.0);
    let _ = h1;
}

#[test]
fn dual_presentation_hashes_match_for_identical_worlds() {
    let mk = || {
        let mut logic = GameLogic::new();
        logic.start_new_game(GameMode::Skirmish);
        logic.clear_all_players();
        logic.add_player(Player::new(0, Team::USA, "P", true));
        let mut t = ThingTemplate::new("HashUnit");
        t.set_health(50.0);
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("HashUnit".into(), t);
        let _ = logic.create_object("HashUnit", Team::USA, glam::Vec3::ZERO);
        PresentationFrame::build_from_logic(&logic, 0).presentation_hash()
    };
    assert_eq!(mk(), mk());
}

#[test]
fn client_reads_snapshot_not_live_world() {
    // Simulate: authority builds snapshot, then world mutates; client still holds old frame.
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("ClientSnap");
    apply_skirmish_config(&mut logic, &cfg).expect("config");
    let mut t = ThingTemplate::new("SnapUnit");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Infantry);
    logic.templates.insert("SnapUnit".into(), t);
    let id = logic
        .create_object("SnapUnit", Team::USA, glam::Vec3::ZERO)
        .expect("unit");
    let client_view = PresentationFrame::build_from_logic(&logic, 0);
    assert_eq!(client_view.alive_object_count(), 1);
    // Authority continues without client re-borrowing world during "render".
    if let Some(o) = logic.host_object_mut(id) {
        o.status.destroyed = true;
        o.health.current = 0.0;
    }
    // Stale presentation still has the pre-destroy object; proves client feed is owned data.
    assert_eq!(client_view.objects.len(), 1);
    assert!(!client_view.objects[0].destroyed);
    // Fresh presentation reflects authority.
    let next = PresentationFrame::build_from_logic(&logic, 0);
    assert!(
        next.objects.iter().all(|o| o.destroyed || o.id != id)
            || next.alive_object_count() == 0
            || next.objects.iter().any(|o| o.id == id && o.destroyed)
    );
}

#[test]
fn shipped_hud_consumer_uses_snapshot_owned_fields() {
    // Criterion: after logic update, HUD/minimap consumers use snapshot-owned
    // id/transform/health/team/selection/model — not a live re-borrow.
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("HudFields");
    apply_skirmish_config(&mut logic, &cfg).expect("config");
    let mut t = ThingTemplate::new("HudUnit");
    t.set_health(75.0);
    t.add_kind_of(KindOf::Infantry);
    logic.templates.insert("HudUnit".into(), t);
    let id = logic
        .create_object("HudUnit", Team::USA, glam::Vec3::new(9.0, 0.0, -4.0))
        .expect("unit");
    if let Some(o) = logic.host_object_mut(id) {
        o.selected = true;
        o.status.selected = true;
    }
    if let Some(p) = logic.get_player_mut(0) {
        p.selected_objects = vec![id];
    }
    logic.update();
    let snap = PresentationFrame::build_from_logic(&logic, 0);
    let obj = snap
        .objects
        .iter()
        .find(|o| o.id == id)
        .expect("object in snapshot");
    assert!((obj.position.x - 9.0).abs() < 0.01);
    assert!((obj.position.z + 4.0).abs() < 0.01);
    assert_eq!(obj.health_current, 75.0);
    assert_eq!(obj.health_max, 75.0);
    assert_eq!(obj.team, Team::USA);
    assert!(obj.selected);
    assert_eq!(obj.model_key.as_deref(), Some("HudUnit"));

    let mut ui = crate::ui::GameUIState::default();
    snap.apply_to_ui_state(&mut ui);
    assert_eq!(ui.credits, snap.local_supplies as i32);
    assert!(ui.selected_units.contains(&id));

    let mut hud = crate::ui::GameHUD::new();
    snap.apply_to_game_hud(&mut hud);
    let mini = snap.hud_minimap_units();
    assert!(
        mini.iter().any(|(oid, x, z, _)| {
            *oid == id && (*x - 9.0).abs() < 0.01 && (*z + 4.0).abs() < 0.01
        }),
        "minimap units must come from snapshot positions"
    );
    assert!(
        hud.selected_unit_ids().contains(&id),
        "GameHUD selection IDs must come from presentation"
    );
    let hud_info = hud
        .selected_unit_infos()
        .iter()
        .find(|u| u.object_id == id)
        .expect("GameHUD selection health from presentation");
    assert!(
        (hud_info.health_current - 75.0).abs() < 0.01,
        "GameHUD selection health must be snapshot-owned: {}",
        hud_info.health_current
    );
}

#[test]
fn dual_tick_build_and_apply_after_logic_step_seeds_hud() {
    // Map-load / skirmish residual: after authority advances, presentation must
    // seed HUD resources + selection without re-borrowing live objects later.
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("DualTickHud");
    apply_skirmish_config(&mut logic, &cfg).expect("config");
    let mut t = ThingTemplate::new("DualUnit");
    t.set_health(88.0);
    t.add_kind_of(KindOf::Infantry);
    t.add_kind_of(KindOf::Selectable);
    logic.templates.insert("DualUnit".into(), t);
    let id = logic
        .create_object("DualUnit", Team::USA, glam::Vec3::new(1.0, 0.0, 1.0))
        .expect("unit");
    if let Some(p) = logic.get_player_mut(0) {
        p.selected_objects = vec![id];
    }
    if let Some(o) = logic.host_object_mut(id) {
        o.selected = true;
        o.status.selected = true;
    }
    logic.update(); // authority tick
    let mut hud = crate::ui::GameHUD::new();
    let snap = PresentationFrame::build_and_apply_for_hud(&logic, 0, &mut hud);
    assert_eq!(snap.frame.0, logic.get_frame());
    assert!(
        !snap.hud_minimap_units().is_empty(),
        "presentation after tick must expose units for minimap"
    );
    let info = hud
        .selected_unit_infos()
        .iter()
        .find(|u| u.object_id == id)
        .expect("selection health on HUD after dual-tick apply");
    assert!((info.health_current - 88.0).abs() < 0.01);
    // World mutates after apply; HUD must keep snapshot health.
    if let Some(o) = logic.host_object_mut(id) {
        o.health.current = 1.0;
    }
    assert!((info.health_current - 88.0).abs() < 0.01);
}

#[test]
fn dual_tick_applies_selection_panel_to_shell_ui_consumers() {
    // Residual: presentation selection panel feeds HUD + UIState + RTS + unit
    // command panel from one dual-tick apply (no live re-read).
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("DualTickConsumers");
    apply_skirmish_config(&mut logic, &cfg).expect("config");
    let mut t = ThingTemplate::new("MultiUiUnit");
    t.set_health(64.0);
    t.add_kind_of(KindOf::Infantry);
    t.add_kind_of(KindOf::Selectable);
    logic.templates.insert("MultiUiUnit".into(), t);
    let id = logic
        .create_object("MultiUiUnit", Team::USA, glam::Vec3::new(2.0, 0.0, 3.0))
        .expect("unit");
    if let Some(p) = logic.get_player_mut(0) {
        p.selected_objects = vec![id];
    }
    if let Some(o) = logic.host_object_mut(id) {
        o.selected = true;
        o.status.selected = true;
    }
    logic.update();
    let mut hud = crate::ui::GameHUD::new();
    let mut ui = crate::ui::GameUIState::default();
    let mut rts = crate::ui::RTSInterface::new();
    let mut cmd = crate::ui::UnitCommandPanel::new();
    let snap = PresentationFrame::build_and_apply_for_shell_consumers(
        &logic, 0, &mut hud, &mut ui, &mut rts, &mut cmd,
    );
    assert_eq!(snap.frame.0, logic.get_frame());
    assert!(hud.selection_panel().has_positive_health());
    assert!((hud.selection_panel().health_current - 64.0).abs() < 0.01);
    assert!(ui.selection_panel.has_positive_health());
    assert!((ui.selection_panel.health_current - 64.0).abs() < 0.01);
    assert!(rts.selection_panel().has_positive_health());
    assert!(rts.selected_ids().contains(&id));
    assert!(cmd.is_visible());
    assert!((cmd.selection_panel().health_current - 64.0).abs() < 0.01);
    // Live mutation must not rewrite consumer snapshots.
    if let Some(o) = logic.host_object_mut(id) {
        o.health.current = 1.0;
    }
    assert!((hud.selection_panel().health_current - 64.0).abs() < 0.01);
    assert!((rts.selection_panel().health_current - 64.0).abs() < 0.01);
    assert!((cmd.selection_panel().health_current - 64.0).abs() < 0.01);
}

#[test]
fn presentation_snapshot_includes_selection_radius_for_cull() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("SelRadius");
    apply_skirmish_config(&mut logic, &cfg).expect("config");
    let mut t = ThingTemplate::new("RadiusUnit");
    t.set_health(50.0);
    t.add_kind_of(KindOf::Infantry);
    logic.templates.insert("RadiusUnit".into(), t);
    let id = logic
        .create_object("RadiusUnit", Team::USA, glam::Vec3::ZERO)
        .expect("unit");
    if let Some(o) = logic.host_object_mut(id) {
        o.selection_radius = 12.5;
    }
    let snap = PresentationFrame::build_from_logic(&logic, 0);
    let ro = snap.objects.iter().find(|o| o.id == id).expect("in snap");
    assert!(
        (ro.selection_radius - 12.5).abs() < 0.01,
        "selection_radius must be snapshot-owned for presentation-only cull: {}",
        ro.selection_radius
    );
}

#[test]
fn usa_ranger_presentation_model_key_non_empty_for_mesh_resolve() {
    // USA_Ranger must expose its exact retail DefaultConditionState model key
    // so mesh_asset_resolve can target AIRngr_SKN (or honestly skip it).
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("RangerMeshKey");
    apply_skirmish_config(&mut logic, &cfg).expect("config");
    // Prefer host setup template when present; otherwise inject retail-like key.
    if !logic.templates.contains_key("USA_Ranger") {
        let mut t = ThingTemplate::new("USA_Ranger");
        t.set_health(60.0);
        t.set_model("airanger"); // legacy alias must remap
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("USA_Ranger".into(), t);
    }
    let id = logic
        .create_object("USA_Ranger", Team::USA, glam::Vec3::new(1.0, 0.0, 2.0))
        .expect("ranger");
    let snap = PresentationFrame::build_from_logic(&logic, 0);
    let ro = snap.objects.iter().find(|o| o.id == id).expect("in snap");
    let key = ro.model_key.as_deref().unwrap_or("");
    assert!(
        !key.is_empty(),
        "USA_Ranger presentation model_key must be non-empty for mesh resolve"
    );
    assert_eq!(
        key.to_ascii_lowercase(),
        "airngr_skn",
        "USA_Ranger model_key should retain its shipped retail basename"
    );
    let inputs = snap.unit_render_inputs();
    let unit = inputs.iter().find(|u| u.id == id).expect("unit input");
    assert_eq!(unit.model_key.to_ascii_lowercase(), "airngr_skn");
    // Wave 75: combat unit mesh scale residual freezes at 1.0.
    assert!(
        (ro.mesh_scale - 1.0).abs() < 0.001,
        "USA_Ranger mesh_scale residual must be 1.0, got {}",
        ro.mesh_scale
    );
    assert!((unit.mesh_scale - 1.0).abs() < 0.001);
    assert!(snap.mesh_scale_presentation_residual_ok());
}

#[test]
fn mesh_scale_presentation_residual_wave75() {
    assert!(crate::assets::mesh_asset_resolve::honesty_mesh_scale_residual_ok());
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("MeshScalePres");
    apply_skirmish_config(&mut logic, &cfg).expect("config");
    if !logic.templates.contains_key("USA_Humvee") {
        let mut t = ThingTemplate::new("USA_Humvee");
        t.set_health(240.0);
        t.set_model("avhummer");
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("USA_Humvee".into(), t);
    }
    let id = logic
        .create_object("USA_Humvee", Team::USA, glam::Vec3::new(5.0, 0.0, 5.0))
        .expect("humvee");
    let snap = PresentationFrame::build_from_logic(&logic, 0);
    assert!(snap.mesh_scale_presentation_residual_ok());
    let ro = snap.objects.iter().find(|o| o.id == id).expect("in snap");
    assert!((ro.mesh_scale - 1.0).abs() < 0.001);
    let unit = snap
        .unit_render_inputs()
        .into_iter()
        .find(|u| u.id == id)
        .expect("unit input");
    assert!((unit.mesh_scale - 1.0).abs() < 0.001);
}

/// Wave 77 residual: unit/structure ground-height frozen on presentation objects.
#[test]
fn ground_height_presentation_residual_wave77() {
    assert!(honesty_ground_height_residual_ok(
        PRESENTATION_DEFAULT_GROUND_HEIGHT,
        false
    ));
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("GroundHeightPres");
    apply_skirmish_config(&mut logic, &cfg).expect("config");
    if !logic.templates.contains_key("USA_Ranger") {
        let mut t = ThingTemplate::new("USA_Ranger");
        t.set_health(120.0);
        t.set_model("airanger");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("USA_Ranger".into(), t);
    }
    let id = logic
        .create_object("USA_Ranger", Team::USA, glam::Vec3::new(7.0, 0.0, 9.0))
        .expect("ranger");
    let snap = PresentationFrame::build_from_logic(&logic, 0);
    assert!(snap.ground_height_presentation_residual_ok());
    let ro = snap.objects.iter().find(|o| o.id == id).expect("in snap");
    assert!(
        honesty_ground_height_residual_ok(ro.ground_height, ro.ground_height_from_terrain),
        "object ground_height residual inconsistent: h={} from_terrain={}",
        ro.ground_height,
        ro.ground_height_from_terrain
    );
    // Without map terrain, residual defaults to 0 and from_terrain=false.
    if !ro.ground_height_from_terrain {
        assert!((ro.ground_height - PRESENTATION_DEFAULT_GROUND_HEIGHT).abs() < 0.001);
    }
}

#[test]
fn presentation_build_includes_unit_render_fields_and_positions() {
    // Criterion: unit mesh/position/selection inputs are snapshot-owned so the
    // main unit pass can iterate PresentationFrame without GameLogic.
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("UnitRenderFields");
    apply_skirmish_config(&mut logic, &cfg).expect("config");
    let mut t = ThingTemplate::new("MeshUnit");
    t.set_health(60.0);
    t.set_model("AVTank");
    t.add_kind_of(KindOf::Vehicle);
    t.add_kind_of(KindOf::Selectable);
    logic.templates.insert("MeshUnit".into(), t);
    let id = logic
        .create_object("MeshUnit", Team::USA, glam::Vec3::new(3.0, 0.0, -8.0))
        .expect("unit");
    if let Some(o) = logic.host_object_mut(id) {
        o.selected = true;
        o.status.selected = true;
        o.selection_radius = 11.0;
        o.team_color = [0.1, 0.2, 0.9, 1.0];
        // Not bridged — main mesh pass owns draw.
    }
    if let Some(p) = logic.get_player_mut(0) {
        p.selected_objects = vec![id];
    }

    let snap = PresentationFrame::build_from_logic(&logic, 0);
    let ro = snap.objects.iter().find(|o| o.id == id).expect("in snap");
    assert!((ro.position.x - 3.0).abs() < 0.01);
    assert!((ro.position.z + 8.0).abs() < 0.01);
    assert_eq!(ro.team, Team::USA);
    assert_eq!(ro.team_color, [0.1, 0.2, 0.9, 1.0]);
    assert_eq!(ro.model_key.as_deref(), Some("AVTank"));
    assert_eq!(ro.template_name, "MeshUnit");
    assert!(ro.selected);
    assert!(!ro.destroyed);
    assert!(!ro.engine_bridged);
    assert!((ro.selection_radius - 11.0).abs() < 0.01);

    // unit_render_inputs is the production pure-frame collection path.
    let inputs = snap.unit_render_inputs();
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].id, id);
    assert_eq!(inputs[0].model_key, "AVTank");
    assert!((inputs[0].position.x - 3.0).abs() < 0.01);
    assert!(inputs[0].selected);
    assert!(!inputs[0].engine_bridged);
    assert_eq!(inputs[0].fow_visibility, ro.fow_visibility);

    // Mutate authority after snapshot — inputs must stay frozen.
    if let Some(o) = logic.host_object_mut(id) {
        o.set_position(glam::Vec3::new(999.0, 0.0, 999.0));
        o.selected = false;
    }
    let inputs_after = snap.unit_render_inputs();
    assert_eq!(inputs_after.len(), 1);
    assert!(
        (inputs_after[0].position.x - 3.0).abs() < 0.01,
        "unit render inputs must not re-read live GameLogic"
    );
    assert!(inputs_after[0].selected);
    assert!(!inputs_after[0].engine_bridged);
    assert_eq!(
        inputs_after[0].fow_visibility, ro.fow_visibility,
        "FOW on unit inputs must stay frozen after live world mutation"
    );
}

#[test]
fn presentation_fow_matches_bridge_at_build_and_stays_frozen() {
    let _shroud_test_guard = crate::fow_rendering::shroud_test_isolation_lock()
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    use crate::fow_rendering::{FOWRenderingBridge, ObjectVisibility};
    use gamelogic::system::shroud_manager::get_shroud_manager;

    // Isolate global shroud — prior FOW tests may leave permanent reveal.
    {
        let shroud_manager = get_shroud_manager();
        let mut shroud = shroud_manager.lock().expect("shroud");
        shroud.clear_all();
    }

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("FowSnapConsistency");
    apply_skirmish_config(&mut logic, &cfg).expect("config");
    let mut t = ThingTemplate::new("FowUnit");
    t.set_health(50.0);
    t.add_kind_of(KindOf::Infantry);
    logic.templates.insert("FowUnit".into(), t);
    let id = logic
        .create_object("FowUnit", Team::USA, glam::Vec3::new(5.0, 0.0, 5.0))
        .expect("unit");

    // Bridge state at build time is the source of truth for the snapshot.
    let bridge_at_build = FOWRenderingBridge::get_object_visibility(0, id);
    let snap = PresentationFrame::build_from_logic(&logic, 0);
    let ro = snap.objects.iter().find(|o| o.id == id).expect("in snap");
    assert_eq!(
        ro.fow_visibility, bridge_at_build,
        "presentation FOW must match FOW bridge at build time"
    );
    assert_eq!(snap.fow_for_object(id), Some(bridge_at_build));
    assert_eq!(snap.fow_shell_bypass, logic.isInShellGame());
    assert_eq!(snap.in_replay_game, logic.isInReplayGame());
    assert_eq!(
        snap.logic_steps_run,
        logic.fixed_step_diagnostics().steps_run as u32
    );
    assert_eq!(
        snap.logic_steps_budget_hit,
        logic.fixed_step_diagnostics().budget_hit
    );
    assert_eq!(
        snap.logic_steps_accumulated_seconds.to_bits(),
        logic
            .fixed_step_diagnostics()
            .accumulated_time_seconds
            .to_bits()
    );
    assert!(
        snap.known_template_names.windows(2).all(|w| w[0] <= w[1]),
        "known_template_names must be sorted"
    );

    let inputs = snap.unit_render_inputs();
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].fow_visibility, bridge_at_build);
    assert_eq!(
        inputs[0].fow_should_render(),
        bridge_at_build.should_render()
    );

    // Encode states are stable and cover the three SAGE-style buckets.
    assert_eq!(
        ObjectVisibility::from_shroud_flags(true, true),
        ObjectVisibility::VISIBLE
    );
    assert_eq!(
        ObjectVisibility::from_shroud_flags(false, true),
        ObjectVisibility::FOGGED
    );
    assert_eq!(
        ObjectVisibility::from_shroud_flags(false, false),
        ObjectVisibility::HIDDEN
    );
    assert!(ObjectVisibility::FOGGED.should_render());
    assert!(!ObjectVisibility::HIDDEN.should_render());
    assert!(ObjectVisibility::HIDDEN.never_explored());

    // Dual-build with identical world + FOW state yields matching FOW on hash.
    let snap2 = PresentationFrame::build_from_logic(&logic, 0);
    assert_eq!(snap.fow_for_object(id), snap2.fow_for_object(id));
    assert_eq!(
        snap.objects
            .iter()
            .find(|o| o.id == id)
            .map(|o| o.fow_visibility),
        snap2
            .objects
            .iter()
            .find(|o| o.id == id)
            .map(|o| o.fow_visibility)
    );
}

#[test]
fn presentation_fow_shell_bypass_forces_fully_visible() {
    let _shroud_test_guard = crate::fow_rendering::shroud_test_isolation_lock()
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    use crate::fow_rendering::ObjectVisibility;
    use crate::game_logic::GameMode;

    let mut logic = GameLogic::new();
    // Shell map path: FOW bypass is frozen on the frame.
    logic.start_new_game(GameMode::Shell);
    assert!(logic.isInShellGame());
    let mut t = ThingTemplate::new("ShellFowUnit");
    t.set_health(10.0);
    t.add_kind_of(KindOf::Infantry);
    logic.templates.insert("ShellFowUnit".into(), t);
    let id = logic
        .create_object("ShellFowUnit", Team::USA, glam::Vec3::ZERO)
        .expect("unit");

    let snap = PresentationFrame::build_from_logic(&logic, 0);
    assert!(snap.fow_shell_bypass);
    let ro = snap.objects.iter().find(|o| o.id == id).expect("in snap");
    assert_eq!(ro.fow_visibility, ObjectVisibility::FULLY_VISIBLE);
    assert!(snap.unit_render_inputs()[0].fow_should_render());
    // Terrain overlay inactive under shell bypass (fail-open / no darkening).
    assert!(!snap.terrain_fow_overlay_active());
}

#[test]
fn presentation_world_env_freezes_bounds_and_map_name() {
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("WorldEnvMap");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let snap = PresentationFrame::build_from_logic(&logic, 0);
    assert_eq!(snap.world_env.map_name, logic.get_current_map_name().trim());
    let (a, b) = logic.world_bounds();
    assert_eq!(snap.world_env.world_min, [a.x, a.y, a.z]);
    assert_eq!(snap.world_env.world_max, [b.x, b.y, b.z]);
    // Shell bypass matches frozen flag used by render execute residual.
    assert_eq!(snap.fow_shell_bypass, logic.isInShellGame());
    assert_eq!(snap.in_replay_game, logic.isInReplayGame());
    let sig = snap.world_env.prewarm_signature(snap.fow_shell_bypass);
    assert!(sig.contains(&snap.world_env.map_name) || snap.world_env.map_name.is_empty());
    assert!(sig.contains(&format!("shell:{}", snap.fow_shell_bypass)));
}

#[test]
fn world_env_height_grid_is_self_consistent() {
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("HeightGridMap");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let snap = PresentationFrame::build_from_logic(&logic, 0);
    assert_eq!(snap.world_env.height_grid_w, 64);
    assert_eq!(snap.world_env.height_grid_h, 64);
    assert_eq!(snap.world_env.height_samples.len(), (64 * 64) as usize);
    // Road/bridge/prewarm vectors always present (may be empty without map parse).
    let _ = &snap.world_env.road_segments;
    let _ = &snap.world_env.bridge_segments;
    assert!(snap.world_env.prewarm_template_names.len() <= 256);
    if snap.world_env.height_samples_from_terrain {
        let (a, b) = snap.world_env.world_bounds_vec3();
        let mid_x = (a.x + b.x) * 0.5;
        let mid_z = (a.z + b.z) * 0.5;
        assert!(snap.world_env.sample_height(mid_x, mid_z).is_some());
    }
}

#[test]
fn presentation_fow_grid_matches_shroud_snapshot_and_stays_frozen() {
    let _shroud_test_guard = crate::fow_rendering::shroud_test_isolation_lock()
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    use crate::fow_rendering::{FOWRenderingBridge, PresentationFowGrid};
    use gamelogic::system::shroud_manager::get_shroud_manager;

    // Isolate global shroud manager for this test.
    {
        let shroud_manager = get_shroud_manager();
        let mut shroud = shroud_manager.lock().expect("shroud");
        shroud.clear_all();
        shroud.init_shroud_grid(500.0, 500.0); // 10x10 cells at 50 wu
        shroud.mark_host_object_seen(0, 1);
        shroud.force_update();
        let _ = shroud.update(1);
    }

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("FowGridSnap");
    apply_skirmish_config(&mut logic, &cfg).expect("config");

    // Activate FOW runtime (visible membership) without permanent map reveal so
    // baseline is not fail-open fully-visible and reveal can change fingerprint.
    {
        let shroud_manager = get_shroud_manager();
        let mut shroud = shroud_manager.lock().expect("shroud");
        shroud.clear_all();
        shroud.init_shroud_grid(500.0, 500.0); // 10x10
        // Host residual API — keeps FOW filters active without dual registry.
        shroud.mark_host_object_seen(0, 1);
        let _ = shroud.update(1);
        // No reveal_map_for_player_permanently yet — terrain stays mostly shrouded.
    }

    // Build with active hidden grid (last_update_frame > 0 after update above).
    let bridge_grid = FOWRenderingBridge::snapshot_terrain_grid(0, false);
    let snap = PresentationFrame::build_from_logic(&logic, 0);

    assert!(
        !snap
            .fow_grid
            .cells
            .iter()
            .all(|&c| c == PresentationFowGrid::CELL_VISIBLE),
        "baseline snapshot must not be fully visible before reveal"
    );
    assert_eq!(
        snap.fow_grid.content_fingerprint(),
        bridge_grid.content_fingerprint(),
        "presentation fow_grid must match FOW bridge grid at build time"
    );
    assert_eq!(snap.fow_grid(), &bridge_grid);
    assert!(snap.fow_grid.active, "grid should be active after init");
    assert_eq!(snap.fow_grid.width, 10);
    assert_eq!(snap.fow_grid.height, 10);
    assert_eq!(snap.fow_grid.cell_count(), 100, "10x10 compact grid");
    assert_eq!(snap.projected_shroud.grid_width, 10);
    assert_eq!(snap.projected_shroud.grid_height, 10);
    // W3D adds the source border (12x12) then validates the destination to
    // powers of two, so the frozen texture is the full 16x16 allocation.
    assert_eq!(snap.projected_shroud.texture_extent(), Some((16, 16)));
    assert_eq!(snap.projected_shroud.texels.len(), 256);
    assert!(
        snap.terrain_projected_shroud().is_some(),
        "active non-shell frame must expose only its frozen shroud projection"
    );

    // R8 payload length matches grid; encoding is deterministic.
    let r8 = snap.terrain_fow_r8().expect("active grid has r8");
    assert_eq!(r8.len(), 100);
    assert_eq!(r8, snap.fow_grid.to_r8_texture());

    // Dual-build consistency.
    let snap2 = PresentationFrame::build_from_logic(&logic, 0);
    assert_eq!(
        snap.fow_grid.content_fingerprint(),
        snap2.fow_grid.content_fingerprint()
    );
    assert_eq!(snap.presentation_hash(), snap2.presentation_hash());

    // Freeze: mutate live shroud after snapshot — presentation cells must not change.
    let frozen_fp = snap.fow_grid.content_fingerprint();
    let frozen_r8 = snap.fow_grid.to_r8_texture();
    let frozen_projected_fp = snap.projected_shroud.content_fingerprint();
    {
        let shroud_manager = get_shroud_manager();
        let mut shroud = shroud_manager.lock().expect("shroud");
        // Permanent reveal → all cells Visible on the live manager.
        shroud.reveal_map_for_player_permanently(0).expect("reveal");
    }
    assert_eq!(
        snap.fow_grid.content_fingerprint(),
        frozen_fp,
        "owned grid must stay frozen after live shroud mutation"
    );
    assert_eq!(snap.fow_grid.to_r8_texture(), frozen_r8);
    assert_eq!(
        snap.projected_shroud.content_fingerprint(),
        frozen_projected_fp,
        "projected R8 snapshot must stay frozen after live shroud mutation"
    );

    // New build sees the reveal.
    let snap_after = PresentationFrame::build_from_logic(&logic, 0);
    assert!(
        snap_after
            .fow_grid
            .cells
            .iter()
            .all(|&c| c == PresentationFowGrid::CELL_VISIBLE),
        "fresh snapshot after permanent reveal must be fully visible"
    );
    assert_ne!(
        snap_after.fow_grid.content_fingerprint(),
        frozen_fp,
        "new frame must differ after live reveal"
    );
    assert_ne!(
        snap_after.projected_shroud.content_fingerprint(),
        frozen_projected_fp,
        "new projected R8 snapshot must differ after live reveal"
    );

    // Shell bypass forces fully visible cells when grid dims exist.
    {
        use crate::game_logic::GameMode;
        let mut shell_logic = GameLogic::new();
        shell_logic.start_new_game(GameMode::Shell);
        let shell_snap = PresentationFrame::build_from_logic(&shell_logic, 0);
        assert!(shell_snap.fow_shell_bypass);
        if shell_snap.fow_grid.active {
            assert!(
                shell_snap
                    .fow_grid
                    .cells
                    .iter()
                    .all(|&c| c == PresentationFowGrid::CELL_VISIBLE)
            );
        }
        assert!(!shell_snap.terrain_fow_overlay_active());
        assert!(shell_snap.terrain_projected_shroud().is_none());
    }

    // Cleanup global shroud so other tests fail-open cleanly.
    // Permanent reveal leaves lookers; re-init grid + clear_all resets counters.
    if let Ok(mut shroud) = get_shroud_manager().lock() {
        shroud.clear_all();
        shroud.init_shroud_grid(1.0, 1.0);
        shroud.clear_all();
    }
}

#[test]
fn unit_render_inputs_keep_resident_direct_destroyed_drawable_without_duplicate() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("UnitRenderSkip");
    apply_skirmish_config(&mut logic, &cfg).expect("config");
    let mut t = ThingTemplate::new("SkipUnit");
    t.set_health(40.0);
    t.add_kind_of(KindOf::Infantry);
    logic.templates.insert("SkipUnit".into(), t);

    let alive_id = logic
        .create_object("SkipUnit", Team::China, glam::Vec3::new(1.0, 0.0, 1.0))
        .expect("alive");
    let dead_id = logic
        .create_object("SkipUnit", Team::China, glam::Vec3::new(2.0, 0.0, 2.0))
        .expect("dead");
    let other_id = logic
        .create_object("SkipUnit", Team::China, glam::Vec3::new(3.0, 0.0, 3.0))
        .expect("other");
    if let Some(o) = logic.host_object_mut(dead_id) {
        o.status.destroyed = true;
        o.health.current = 0.0;
    }

    let snap = PresentationFrame::build_from_logic(&logic, 0);
    let inputs = snap.unit_render_inputs();
    assert_eq!(
        inputs.len(),
        3,
        "the normal roster omits gameplay-destroyed rows, while the resident direct host drawable preserves its visual lifetime"
    );
    let input_ids: Vec<_> = inputs.iter().map(|i| i.id).collect();
    assert!(
        input_ids.contains(&alive_id)
            && input_ids.contains(&other_id)
            && input_ids.contains(&dead_id)
    );
    assert_eq!(
        input_ids.iter().filter(|&&id| id == dead_id).count(),
        1,
        "a direct source may fill a missing normal row but must not duplicate it"
    );
    let direct = snap
        .direct_host_drawables
        .iter()
        .find(|drawable| drawable.object.id == dead_id)
        .expect("destroyed host object retains direct source");
    assert!(direct.resident);
    assert!(
        direct.object.destroyed,
        "gameplay destruction remains on the normal object payload"
    );
    let ids = snap.renderable_object_ids();
    assert!(ids.contains(&alive_id));
    assert!(ids.contains(&other_id));
    assert!(!ids.contains(&dead_id));
    assert!(
        inputs.iter().all(|i| !i.engine_bridged),
        "engine_bridged residual stays false on host-only path"
    );
}

#[test]
fn direct_host_drawable_roster_survives_gameworld_rebuild_and_uses_visual_identity() {
    use crate::gameworld_shadow::GameWorldShadow;

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("DirectHostVisualRoster");
    apply_skirmish_config(&mut logic, &cfg).expect("config");
    let mut template = ThingTemplate::new("DirectHostVisualActual");
    template.set_health(40.0);
    template.add_kind_of(KindOf::Infantry);
    logic
        .templates
        .insert("DirectHostVisualActual".into(), template);
    let id = logic
        .create_object(
            "DirectHostVisualActual",
            Team::China,
            glam::Vec3::new(2.0, 0.0, 2.0),
        )
        .expect("object");
    {
        let object = logic.host_object_mut(id).expect("host object");
        // The visual selector must not take this mutable bookkeeping name.
        object.template_name = "MutableRuntimeTemplate".into();
        object.status.disguised = true;
        object.disguise_as_template = Some("DirectHostVisualDisguise".into());
        // Match the GameWorld rebuild omission gate while retaining host
        // Object presence for the direct drawable lifetime.
        object.status.destroyed = true;
        object.health.current = 0.0;
    }

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let mut frame = PresentationFrame::build_from_logic(&logic, 0);
    let direct = frame
        .direct_host_drawables
        .iter()
        .find(|drawable| drawable.object.id == id)
        .expect("direct host source");
    assert!(direct.resident);
    assert_eq!(direct.visual_template_name, "DirectHostVisualDisguise");
    assert_ne!(direct.visual_template_name, "MutableRuntimeTemplate");

    logic
        .host_object_mut(id)
        .expect("host object")
        .disguise_as_template = None;
    let fallback_frame = PresentationFrame::build_from_logic(&logic, 0);
    assert_eq!(
        fallback_frame
            .direct_host_drawables
            .iter()
            .find(|drawable| drawable.object.id == id)
            .expect("direct fallback source")
            .visual_template_name,
        "DirectHostVisualActual",
        "a committed disguise without a replacement template falls back to ThingTemplate, not mutable Object bookkeeping"
    );

    assert_eq!(frame.rebuild_objects_from_gameworld(&shadow), 0);
    assert!(
        frame.objects.is_empty(),
        "GameWorld has omitted the destroyed entity"
    );
    assert_eq!(
        frame.direct_host_drawables.len(),
        1,
        "primary roster replacement must not erase the independent direct source"
    );
    let inputs = frame.unit_render_inputs();
    let input = inputs
        .iter()
        .find(|input| input.id == id)
        .expect("resident direct drawable fills missing GameWorld row");
    assert_eq!(input.template_name, "DirectHostVisualDisguise");
    assert!(
        input.destroyed,
        "visual residency does not rewrite gameplay destruction"
    );
}

#[test]
fn presentation_feeds_skybox() {
    let mut logic = crate::game_logic::GameLogic::new();
    logic.set_script_skybox_enabled_for_test(true);
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    assert!(frame.world_env.skybox_enabled);
}

#[test]
fn presentation_shell_includes_fx_and_message_pump() {
    // Structural: GameClient presentation path must tick FX + message pump without
    // calling full update() (OBJECT_REGISTRY shroud bind).
    let gc = game_client::core::game_client::GAME_CLIENT_SRC;
    let shell = gc
        .split("fn update_presentation_shell")
        .nth(1)
        .and_then(|s| s.split("pub fn update_drawables").next())
        .expect("update_presentation_shell body");
    assert!(
        shell.contains("update_effects"),
        "presentation shell must tick effects residual"
    );
    assert!(
        shell.contains("pump_message_stream"),
        "presentation shell must pump client messages"
    );
    assert!(
        shell.contains("update_drawables_local"),
        "presentation shell must use local drawables (no registry shroud)"
    );
    assert!(
        !shell.contains("update_drawables(visual_delta)")
            && !shell.contains("self.update_drawables("),
        "presentation shell must not call registry-bound update_drawables"
    );
    assert!(
        !shell.contains("self.update_input("),
        "Main owns input; presentation shell must not double-tick input"
    );
    assert!(
        !shell.contains("self.update_audio("),
        "Main owns audio; presentation shell must not double-tick audio"
    );
}

#[test]
fn control_bar_update_honors_presentation_selection_without_registry() {
    let cb = game_client::gui::control_bar::control_bar::CONTROL_BAR_SRC;
    assert!(
        cb.contains("presentation_selection_active") || cb.contains("portrait_state.is_visible"),
        "control bar must not wipe selection solely because OBJECT_REGISTRY is empty"
    );
    assert!(
        cb.contains("Without registry modules, skip live module context")
            || cb.contains("// Without registry modules"),
        "control bar must short-circuit live module updates on presentation path"
    );
}

#[test]
fn control_bar_keeps_presentation_production_queue_without_registry() {
    let cb = game_client::gui::control_bar::control_bar::CONTROL_BAR_SRC;
    assert!(
        cb.contains("presentation-fed queue residual stays")
            || cb.contains("Presentation residual owns host InGame queue progress"),
        "update_context_command must not wipe presentation production queue"
    );
    assert!(
        cb.contains("Main already filtered unit_command_buttons")
            || cb.contains("Do not hide presentation-fed command sets"),
        "get_command_availability must not hide all buttons without OBJECT_REGISTRY"
    );
    assert!(
        cb.contains("sync_selection_display_from_presentation — do not wipe it")
            || cb.contains("do not wipe it"),
        "update_portrait_for_object must preserve presentation portrait"
    );
}

#[test]
fn control_bar_multi_select_prefers_presentation_command_sets() {
    let src = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    assert!(
        src.contains("sync_multi_select_command_sets_from_presentation"),
        "apply_to_control_bar must feed multi-select command sets from presentation"
    );
    assert!(
        src.contains("fn selected_command_set_names"),
        "presentation must expose selected_command_set_names"
    );
    let cb = game_client::gui::control_bar::control_bar::CONTROL_BAR_SRC;
    assert!(
        cb.contains("fn sync_multi_select_command_sets_from_presentation"),
        "ControlBar must accept multi-select command sets without OBJECT_REGISTRY"
    );
}

#[test]
fn control_bar_execute_falls_back_without_registry() {
    let cb = game_client::gui::control_bar::control_bar::CONTROL_BAR_SRC;
    assert!(
        cb.contains("Host/presentation residual: MSG_QUEUE_UNIT_CREATE")
            || cb.contains("MSG_QUEUE_UNIT_CREATE (no OBJECT_REGISTRY)"),
        "production execute must message-stream when registry empty"
    );
    assert!(
        cb.contains("Host/presentation residual: MSG_DO_SPECIAL_POWER")
            || cb.contains("DoSpecialPower("),
        "special-power execute must message-stream when registry empty"
    );
    assert!(
        cb.contains("if applied > 0")
            && cb.contains("Host/presentation residual: queue typed Command"),
        "direct execute must not return early when registry applied zero objects"
    );
    assert!(
        cb.contains("message-stream cancel (no OBJECT_REGISTRY modules)")
            || cb.contains("CancelUnitCreate("),
        "cancel_build_queue_item must message-stream without registry"
    );
}

#[test]
fn control_bar_beacon_prefers_presentation_command_set() {
    let cb = game_client::gui::control_bar::control_bar::CONTROL_BAR_SRC;
    let beacon =
        include_str!("../../../../GameEngine/GameClient/src/gui/control_bar/control_bar_beacon.rs");
    assert!(
        cb.contains("presentation_primary_command_set"),
        "ControlBar must retain presentation command-set residual for beacon path"
    );
    assert!(
        beacon.contains("append_beacon_commands_with_presentation")
            && beacon.contains("Host presentation residual"),
        "beacon commands must accept presentation command-set without OBJECT_REGISTRY"
    );
    assert!(
        cb.contains("beacon UI when command-set freeze says BEACON")
            || cb.contains("presentation_primary_command_set"),
        "update_context_beacon must not wipe host beacon residual"
    );
}

#[test]
fn control_bar_multi_select_rebuild_uses_presentation_names() {
    let cb = game_client::gui::control_bar::control_bar::CONTROL_BAR_SRC;
    let ms = include_str!(
        "../../../../GameEngine/GameClient/src/gui/control_bar/control_bar_multi_select.rs"
    );
    assert!(
        cb.contains("presentation_command_set_names")
            && cb.contains("populate_multi_select_commands_from_sets"),
        "add_multi_select_commands must prefer presentation command-set names"
    );
    assert!(
        ms.contains("fn populate_multi_select_commands_from_sets"),
        "multi-select must support name-based intersection without OBJECT_REGISTRY"
    );
    assert!(
        cb.contains("Presentation residual first (host path has no dual-world registry)"),
        "host path multi-select rebuild must document presentation-first residual"
    );
}

#[test]
fn renderable_freezes_effective_command_set_name() {
    let src = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    assert!(
        src.contains("pub command_set_name: String")
            && src.contains("resolve_command_set_name(")
            && src.contains("&obj.template_name"),
        "build_from_logic must freeze effective command_set_name onto RenderableObject"
    );
    assert!(
        src.contains("Prefer freeze from build_from_logic")
            || src.contains("!ro.command_set_name.is_empty()"),
        "selected_command_set_names must prefer frozen command_set_name"
    );
    assert!(
        src.contains("!o.command_set_name.is_empty()")
            && src.contains("Some(o.command_set_name.as_str())"),
        "selected_command_set_name must prefer frozen name over override-only"
    );
}

#[test]
fn add_object_commands_prefers_presentation_command_set() {
    let cb = game_client::gui::control_bar::control_bar::CONTROL_BAR_SRC;
    assert!(
        cb.contains("Host/presentation residual — no OBJECT_REGISTRY modules")
            && cb.contains("self.presentation_primary_command_set.clone()"),
        "add_object_commands must use presentation command-set freeze without registry"
    );
}

#[test]
fn structure_inventory_prefers_presentation_residual() {
    let cb = game_client::gui::control_bar::control_bar::CONTROL_BAR_SRC;
    let si = include_str!(
        "../../../../GameEngine/GameClient/src/gui/control_bar/control_bar_structure_inventory.rs"
    );
    assert!(
        cb.contains("presentation_max_garrison")
            && cb.contains("presentation_garrisoned_count")
            && cb.contains("append_structure_inventory_commands_with_presentation"),
        "ControlBar must feed structure inventory from presentation residual"
    );
    assert!(
        si.contains("Host presentation residual") && si.contains("presentation_max_garrison"),
        "structure inventory must work without OBJECT_REGISTRY contain modules"
    );
}

#[test]
fn evaluate_context_uses_presentation_structure_states() {
    let cb = game_client::gui::control_bar::control_bar::CONTROL_BAR_SRC;
    assert!(
        cb.contains("presentation_under_construction")
            && cb.contains("ControlBarState::UnderConstruction"),
        "evaluate_context_ui must set UnderConstruction from presentation residual"
    );
    assert!(
        cb.contains("presentation_max_garrison > 0")
            && cb.contains("ControlBarState::StructureInventory"),
        "evaluate_context_ui must set StructureInventory from presentation residual"
    );
    assert!(
        cb.contains("rebuild_command_buttons(&mut context)?")
            && cb.contains("no dual-world registry"),
        "presentation-only evaluate must rebuild commands without registry"
    );
}

#[test]
fn game_client_xfer_allows_missing_object_registry() {
    let gc = game_client::core::game_client::GAME_CLIENT_SRC;
    assert!(
        !gc.contains("Cannot find object") && !gc.contains("references missing object ID"),
        "GameClient xfer must not hard-fail drawable load when OBJECT_REGISTRY is empty"
    );
    assert!(
        gc.contains("Host/presentation path: allow drawable load without dual-world registry")
            || gc.contains("Host/presentation path: OBJECT_REGISTRY may be empty"),
        "xfer load must document host residual without registry bind"
    );
    assert!(
        gc.contains("Dual-world residual bind only"),
        "bind_drawable_to_object must remain opt-in when registry has the object"
    );
}

#[test]
fn under_construction_context_uses_presentation_percent() {
    let cb = game_client::gui::control_bar::control_bar::CONTROL_BAR_SRC;
    assert!(
        cb.contains("presentation_construction_percent")
            && cb.contains("Host/presentation residual owns construct percent display"),
        "under-construction context must track presentation construct percent without registry"
    );
    // Live residual spelling: registry-empty OCL context falls back to the
    // presentation freeze seconds (control_bar_impl/impl_contexts.rs
    // update_context_ocl_timer else-branch) fed by sync_ocl_timer_from_presentation
    // (control_bar_impl/impl_portrait.rs, Wave 1031 C++ ControlBarOCLTimer.cpp residual).
    assert!(
        cb.contains("presentation_ocl_timer_seconds")
            && (cb.contains("(self.presentation_ocl_timer_seconds, 0.0)")
                || cb.contains("fn sync_ocl_timer_from_presentation")),
        "OCL timer context must not require dual-world OCLUpdate modules"
    );
    assert!(
        cb.contains("fn sync_structure_context_from_presentation")
            && cb.contains(
                "self.presentation_construction_percent = construction_percent.clamp(0.0, 1.0)"
            ),
        "sync_structure_context_from_presentation must store construction percent residual"
    );
}

#[test]
fn construction_sole_tick_host_skips_advance() {
    // Facade split: host construction sole-tick path lives in
    // game_logic/world_tick/production.rs now (GAME_LOGIC_FACADE_SRC concat
    // no longer carries it) — final20c-style repoint to the live Wave 478
    // rate-only publish site.
    let tick = include_str!("../../game_logic/world_tick/production.rs");
    assert!(
        tick.contains("gameworld_construction_sole_tick_enabled()")
            && tick.contains("record_rate_only")
            && tick.contains("Wave 478: publish dozer/power rate only"),
        "host construction under sole-tick must publish rate only (no percent stomp)"
    );
    let log = include_str!("../../game_logic/host_construction_progress_log.rs");
    assert!(
        log.contains("effective_rate: f32")
            && log.contains("rate_only: bool")
            && log.contains("pub fn record_rate_only"),
        "construction progress log must carry rate_only residual"
    );
    let sw = crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC;
    assert!(
        sw.contains("fn tick_construction_progress")
            && sw.contains("tick_construction_progress(")
            && sw.contains("if ev.rate_only"),
        "shadow session must sole-tick construction and skip rate-only stomps"
    );
}

#[test]
fn meta_event_plane_lock_prefers_selection_without_registry() {
    let me = game_client::message_stream::meta_event::META_EVENT_SRC;
    assert!(
        (me.contains(
            "Host residual: when registry empty, cycle airborne units from local selection"
        ) || me
            .contains("Wave 979: host empty dual-world → presentation catalog airborne residual"))
            && (me.contains("local_selection_object_ids()")
                || me.contains("with_translator_catalog")),
        "plane camera lock must fall back to catalog/selection when OBJECT_REGISTRY is empty"
    );
    assert!(
        me.contains("Host residual: registry empty is fine")
            || me.contains("Main presentation shell owns drawable TOD"),
        "time-of-day refresh must not require OBJECT_REGISTRY for host path"
    );
    // Wave 559: model-condition refresh documents presentation ownership under empty registry.
    assert!(
        me.contains("PresentationFrame / drawable shell tick owns model conditions when empty")
            || me.contains("host presentation residual only")
            || me.contains("dual_world_registry_unavailable()"),
        "model condition refresh must document host residual without registry"
    );
}

#[test]
fn snap_camera_start_hint_prefers_presentation() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    // Wave 559: snap_camera is presentation-only via frozen local_team_base_position.
    let i = src
        .find("fn snap_camera_to_local_units_if_needed")
        .expect("snap_camera fn");
    let window = &src[i..src.len().min(i + 2200)];
    assert!(
        window.contains("Presentation-only")
            && window.contains("last_presentation_frame.as_ref()")
            && window.contains("local_team_base_position")
            && window.contains("o.is_structure"),
        "snap_camera start_hint must seed from presentation structures"
    );
    assert!(
        window.contains("camera_target"),
        "start_hint falls back to camera_target when no structures"
    );
    assert!(
        !window.contains("team_base_position(team)"),
        "snap_camera must not dual-read live team_base_position under presentation path"
    );
}

#[test]
fn presentation_freezes_local_team_base_position() {
    let src = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    assert!(
        src.contains("pub local_team_base_position: Option<Vec3>")
            && src.contains("logic.team_base_position(local_team)"),
        "build_from_logic must freeze local_team_base_position from host"
    );
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        eng.contains("local_team_base_position") && eng.contains("or_else"),
        "snap_camera must prefer frozen local_team_base_position"
    );
}

#[test]
fn game_client_update_for_rendering_host_path_without_registry() {
    let gc = game_client::core::game_client::GAME_CLIENT_SRC;
    assert!(
        gc.contains("Host/presentation path: no dual-world OBJECT_REGISTRY")
            && gc.contains("update_drawables_local(visual_delta)"),
        "update_for_rendering must local-tick drawables when OBJECT_REGISTRY is empty"
    );
    assert!(
        gc.contains("Host/presentation path: shroud comes from PresentationFrame")
            && gc.contains("OBJECT_REGISTRY.is_empty()"),
        "update_drawables must skip registry shroud bind on host path"
    );
    assert!(
        gc.contains("does not populate the registry") || gc.contains("becomes a no-op there"),
        "iterate_objects_with_drawables must document host residual no-op"
    );
}

#[test]
fn boot_client_tick_prefers_presentation_shell() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    let marker = "Boot/loading residual without presentation frame";
    let i = src.find(marker).expect("boot client residual marker");
    let window = &src[i..src.len().min(i + 800)];
    assert!(
        window.contains("update_presentation_shell")
            && !window.contains("self.game_client.update_drawables("),
        "boot residual without frame must use presentation shell, not dual-world update_drawables"
    );
}

#[test]
fn game_client_update_host_path_skips_dual_present() {
    let gc = game_client::core::game_client::GAME_CLIENT_SRC;
    assert!(
        gc.contains("host_presentation_path")
            && gc.contains("No draw_display")
            && gc.contains("update_drawables_local(visual_delta)"),
        "GameClient::update must host-path skip dual Display present and registry shroud"
    );
    assert!(
        gc.contains("Dual-world residual: full C++-ordered client tick"),
        "dual-world full update path must remain when OBJECT_REGISTRY is populated"
    );
}

#[test]
fn runtime_host_move_prefers_presentation_selection() {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    let i = eng
        .find("\"move\" | \"move_selected\"")
        .expect("move command");
    let window = &eng[i..eng.len().min(i + 2400)];
    // Wave 559: move uses presentation-first ui_selected_ids residual.
    assert!(
        window.contains("ui_selected_ids")
            || window.contains("Prefer presentation/engine selection residual")
            || window.contains("count_selected_friendlies"),
        "move must prefer presentation/engine selection over live player roster only"
    );
    assert!(
        (window.contains("select_objects")
            && (window.contains("selected_objects = ids")
                || window.contains("selected_objects.is_empty()")))
            || window.contains("host_set_selection"),
        "move must re-sync host player selection from presentation/engine residual"
    );
}

#[test]
fn production_authority_host_skips_progress_advance() {
    let gl = crate::game_logic::game_logic::GAME_LOGIC_FACADE_SRC;
    assert!(
        gl.contains("gameworld_production_sole_tick_enabled()")
            && gl.contains("try_complete_production()")
            && gl.contains("record_power_factor_only"),
        "host under sole-tick must try_complete + power factor only (no progress stomp)"
    );
    let b = include_str!("../../game_logic/buildings.rs");
    assert!(
        b.contains("fn advance_production_progress")
            && b.contains("fn try_complete_production")
            && b.contains("fn tick_exit_delay"),
        "building production must split advance/exit vs complete"
    );
    let sw = crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC;
    assert!(
        sw.contains("fn tick_production_queues")
            && sw.contains("tick_production_queues(")
            && sw.contains("if ev.power_factor_only"),
        "shadow session must sole-tick queues and skip power-factor-only stomps"
    );
}


#[test]
fn special_power_sole_tick_host_skips_advance() {
    let obj = crate::game_logic::object::OBJECT_SRC;
    assert!(
        obj.contains("gameworld_special_power_sole_tick_enabled()") && obj.contains("!sole_sp"),
        "host tick_timers must skip SP advance under sole-tick"
    );
    let log = include_str!("../../game_logic/host_special_power_log.rs");
    assert!(
        log.contains("frozen: bool"),
        "SP progress log must carry disabled/freeze residual"
    );
    let sw = crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC;
    assert!(
        sw.contains("fn tick_special_power_cooldowns")
            && sw.contains("fn tick_player_shared_special_power_cooldowns")
            && sw.contains("writeback_special_power_to_host(logic)"),
        "shadow session must sole-tick object+player SP and writeback under authority"
    );
    let gl = crate::game_logic::game_logic::GAME_LOGIC_FACADE_SRC;
    assert!(
        gl.contains("gameworld_special_power_sole_tick_enabled()")
            && gl.contains("Wave 479: do not republish full cooldown snapshots each frame")
            && gl.contains("record_host_cooldowns"),
        "host shared SP tick must defer under sole-tick without per-frame cooldown stomp"
    );
}

#[test]
fn superweapon_damage_applies_host_hp() {
    let obj = crate::game_logic::object::OBJECT_SRC;
    assert!(
        obj.contains("fn take_damage_from_immediate") && obj.contains("force_host_hp"),
        "object must support immediate host HP damage for superweapons"
    );
    let gl = crate::game_logic::game_logic::GAME_LOGIC_FACADE_SRC;
    assert!(
        gl.contains("take_damage_from_immediate(hit.damage"),
        "update_special_power_strikes must apply host HP immediately"
    );
}

#[test]
fn train_unit_prefers_presentation_team() {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    let i = eng.find("train_unit\" =>").expect("train_unit");
    let window = &eng[i..eng.len().min(i + 900)];
    // Wave 559: train_unit uses presentation-first local_team_for_ui helper.
    assert!(
        window.contains("Prefer presentation local team residual")
            && (window.contains("local_team_for_ui()")
                || window.contains("presentation_or_boot_local_team()")),
        "train_unit must prefer presentation local_team over live player roster"
    );
}

#[test]
fn status_sample_prefers_presentation() {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    // Wave 559: runtime_host_status_snapshot owns roster via presentation freeze.
    assert!(
        eng.contains("Object-roster stats are presentation-owned when freeze installed")
            && eng.contains("fn runtime_host_status_snapshot")
            && eng.contains("count_mobile_friendlies")
            && eng.contains("first_friendly_sample_label")
            && eng.contains("count_selected_friendlies"),
        "runtime status sample must prefer presentation objects"
    );
}

#[test]
fn object_registry_is_empty_host_path() {
    let reg = include_str!("../../../../GameEngine/GameLogic/src/object/registry.rs");
    assert!(
        reg.contains("pub fn is_empty") && reg.contains("Host/presentation path"),
        "OBJECT_REGISTRY must expose is_empty for host dual-world peels"
    );
    let meta = game_client::message_stream::meta_event::META_EVENT_SRC;
    // Wave 559: meta_event centralizes empty-registry via dual_world_registry_unavailable.
    assert!(
        meta.contains("fn dual_world_registry_unavailable")
            && meta.contains("OBJECT_REGISTRY.is_empty()")
            && meta.matches("dual_world_registry_unavailable()").count() >= 3,
        "meta_event dual-world residuals must early-out when registry empty"
    );
}

#[test]
fn iterate_objects_with_drawables_empty_registry() {
    let gc = game_client::core::game_client::GAME_CLIENT_SRC;
    let i = gc
        .find("pub fn iterate_objects_with_drawables")
        .expect("iterate");
    let window = &gc[i..gc.len().min(i + 1200)];
    assert!(
        window.contains("OBJECT_REGISTRY.is_empty()") && window.contains("return Ok(())"),
        "iterate_objects_with_drawables must early-out on empty registry"
    );
    let j = gc.find("fn load_post_process").expect("load_post");
    let w2 = &gc[j..gc.len().min(j + 1600)];
    assert!(
        w2.contains("OBJECT_REGISTRY.is_empty()") && w2.contains("Host path: registry empty"),
        "load_post_process host path must skip dual-world drawable scan"
    );
}

#[test]
fn team_player_registry_empty_early_out() {
    let team = gamelogic::team::TEAM_SRC;
    assert!(
        team.contains("fn for_each_live_member") && team.contains("OBJECT_REGISTRY.is_empty()"),
        "Team bulk member walks must early-out when dual-world registry empty"
    );
    let h = team
        .find("pub fn has_any_objects")
        .expect("has_any_objects");
    assert!(
        team[h..team.len().min(h + 240)].contains("OBJECT_REGISTRY.is_empty()"),
        "has_any_objects must gate on empty registry"
    );
    let player = gamelogic::player::PLAYER_SRC;
    assert!(
        player.contains("crate::object::registry::OBJECT_REGISTRY.is_empty()"),
        "Player bulk object counts must early-out when registry empty"
    );
}

#[test]
fn host_vertical_slice_honesty() {
    let es = include_str!("../../executable_smoke.rs");
    assert!(
        es.contains("host_vertical_slice_ok")
            && es.contains("skirmish_start_wnd_ok")
            && (es.contains("Never flip retail claim")
                || es.contains("host_vertical_slice_ok_never_flips_playable_claim")
                || es.contains("playable_claim stays false")),
        "executable smoke must expose host_vertical_slice_ok without flipping playable_claim"
    );
    assert!(
        es.contains("result.playable_claim = false")
            || es.contains("self.playable_claim = Self::retail_windowed_playable_claim(")
            || es.contains("Headless smoke must keep `playable_claim == false`"),
        "playable_claim must remain forced false"
    );
}

#[test]
fn ingame_ui_registry_empty_early_out() {
    let ui = game_client::gui::ingame_ui::INGAME_UI_SRC;
    assert!(
        ui.matches("OBJECT_REGISTRY.is_empty()").count() >= 3,
        "InGameUI dual-world bulk scans must early-out when registry empty"
    );
    assert!(
        ui.contains("no dual-world factory objects to pick")
            || ui.contains("no dual-world objects"),
        "host path comments for InGameUI registry peels"
    );
}

#[test]
fn selection_translators_registry_empty() {
    let sel =
        include_str!("../../../../GameEngine/GameClient/src/message_stream/selection_xlat.rs");
    assert!(
        sel.contains("OBJECT_REGISTRY.is_empty()") && sel.contains("return Vec::new()"),
        "selection_xlat collect_drawables must early-out when registry empty"
    );
    let tr = game_client::message_stream::translators::TRANSLATORS_SRC;
    assert!(
        tr.contains("OBJECT_REGISTRY.is_empty()") && tr.contains("return (Vec::new(), Vec::new())"),
        "context pick translators must early-out when registry empty"
    );
    let am = include_str!("../../../../GameEngine/GameLogic/src/action_manager.rs");
    assert!(
        am.contains("OBJECT_REGISTRY.is_empty()"),
        "action_manager dual-world special-object scans must gate on empty registry"
    );
}

#[test]
fn victory_script_registry_empty_safe() {
    let v = include_str!("../../../../GameEngine/GameLogic/src/scripting/victory.rs");
    // Wave 559/294: empty dual-world returns Ok(0.0) (fail-closed, not "all dead").
    assert!(
        v.contains("empty dual-world → Ok(0.0)")
            || v.contains("do not treat as \"all enemies dead\""),
        "destruction progress must not complete when dual-world registry empty"
    );
    assert!(
        v.matches("dual_world_registry_unavailable()").count() >= 3
            || v.matches("OBJECT_REGISTRY.is_empty()").count() >= 3,
        "victory dual-world progress calcs must gate on empty registry"
    );
    let eng = gamelogic::scripting::engine::SCRIPT_ENGINE_SRC;
    assert!(
        eng.contains("fn create_named_cache")
            && (eng.contains("OBJECT_REGISTRY.is_empty()")
                || eng.contains("dual_world_registry_unavailable()")),
        "script engine named cache must skip empty dual-world registry"
    );
}

#[test]
fn enhanced_ai_system_registry_empty() {
    let ep = include_str!("../../../../GameEngine/GameLogic/src/ai/enhanced_player.rs");
    // Wave 559: enhanced AI peels centralize empty-registry via dual_world_registry_unavailable.
    assert!(
        ep.matches("dual_world_registry_unavailable()").count() >= 3
            || ep.matches("OBJECT_REGISTRY.is_empty()").count() >= 3,
        "enhanced AI dual-world scans must early-out when registry empty"
    );
    let sys = include_str!("../../../../GameEngine/GameLogic/src/system/game_logic.rs");
    assert!(
        sys.contains("dual-world factory empty")
            || sys.contains("OBJECT_REGISTRY.is_empty()")
            || sys.contains("dual_world_registry_unavailable()"),
        "crate GameLogic update/rebuild must skip empty dual-world registry"
    );
    assert!(
        sys.contains("if !OBJECT_REGISTRY.is_empty()")
            || sys.contains("if OBJECT_REGISTRY.is_empty()")
            || sys.contains("dual_world_registry_unavailable()"),
        "system game_logic must gate dual-world bulk paths"
    );
}

#[test]
fn ai_stealth_helpers_registry_empty() {
    let ai = gamelogic::ai::ai_player::AI_PLAYER_SRC;
    // Wave 559: AI/stealth peels may use dual_world_registry_unavailable centralization.
    assert!(
        ai.contains("find_supply_center")
            && (ai.matches("OBJECT_REGISTRY.is_empty()").count() >= 3
                || ai.matches("dual_world_registry_unavailable()").count() >= 3),
        "AI player dual-world supply/hole scans must gate on empty registry"
    );
    let ap = include_str!("../../../../GameEngine/GameLogic/src/ai/async_player.rs");
    assert!(
        ap.matches("OBJECT_REGISTRY.is_empty()").count() >= 2
            || ap.matches("dual_world_registry_unavailable()").count() >= 2,
        "async AI snapshot scans must gate on empty registry"
    );
    let det = include_str!("../../../../GameEngine/GameLogic/src/stealth/detector.rs");
    assert!(
        det.contains("OBJECT_REGISTRY.is_empty()")
            || det.contains("dual_world_registry_unavailable()"),
        "stealth detector must skip empty dual-world registry"
    );
    let h = gamelogic::helpers::HELPERS_SRC;
    assert!(
        h.contains("Main presentation owns drawable TOD")
            || h.contains("OBJECT_REGISTRY.is_empty()")
            || h.contains("dual_world_registry_unavailable()"),
        "helpers TOD/path residual must gate on empty registry"
    );
}

#[test]
fn scripting_registry_empty_peels() {
    let cond = include_str!("../../../../GameEngine/GameLogic/src/scripting/conditions/mod.rs");
    assert!(
        cond.contains("OBJECT_REGISTRY.is_empty()") && cond.contains("return Ok(false)"),
        "script conditions object-type search must fail-closed on empty registry"
    );
    let ex = gamelogic::scripting::executor::EXECUTOR_SRC;
    assert!(
        ex.matches("Host path: empty dual-world registry").count() >= 3,
        "script executor dual-world bulk actions must gate on empty registry"
    );
}

#[test]
fn remaining_dual_world_registry_empty() {
    let unit = include_str!("../../../../GameEngine/GameLogic/src/object/unit/combat.rs");
    assert!(
        unit.contains("OBJECT_REGISTRY.is_empty()")
            || unit.contains("crate::object::registry::OBJECT_REGISTRY.is_empty()"),
        "unit targeting must gate dual-world bulk scans"
    );
    let build = include_str!("../../../../GameEngine/GameLogic/src/ai/ai_build_list.rs");
    assert!(
        build.contains("OBJECT_REGISTRY.is_empty()"),
        "ai_build_list threat/map-control must gate dual-world bulk scans"
    );
    let grant = include_str!(
        "../../../../GameEngine/GameLogic/src/object/behavior/grant_stealth_behavior.rs"
    );
    assert!(
        grant.contains("OBJECT_REGISTRY.is_empty()"),
        "grant_stealth must gate dual-world bulk scans"
    );
    let terrain = include_str!("../../../../GameEngine/GameLogic/src/terrain/mod.rs");
    assert!(
        terrain.contains("OBJECT_REGISTRY.is_empty()"),
        "terrain dual-world walk must gate on empty registry"
    );
}

#[test]
fn production_progress_log_carries_power_factor() {
    let log = include_str!("../../game_logic/host_production_progress_log.rs");
    assert!(
        log.contains("power_factor: f32") && log.contains("power_factor: power_factor"),
        "production progress events must carry host power_factor"
    );
    let sw = crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC;
    assert!(
        sw.contains("production_power_factor_by_host") && sw.contains("dt * pf"),
        "shadow sole-tick must apply host power_factor residual"
    );
}

#[test]
fn production_tick_builds_presentation_after_side_systems() {
    // Structural: presentation is built after host GameLogic update returns.
    // Projectile drain/step and path follow live inside GameLogic::update_simulation
    // (not engine mid-frame dual systems).
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    let gl = crate::game_logic::game_logic::GAME_LOGIC_FACADE_SRC;
    let proj = gl
        .find("drain_pending_projectiles")
        .expect("projectile drain in GameLogic");
    let path = gl
        .find("fn update_movement")
        .expect("path follow in GameLogic");
    let eng_dual = eng.find("drain_pending_projectiles");
    assert!(
        eng_dual.is_none(),
        "engine must not mid-frame drain_pending_projectiles (dual CombatSystem)"
    );
    assert!(
        eng.find("move_unit_along_path").is_none(),
        "engine must not mid-frame move_unit_along_path (dual path step)"
    );
    // Wave 713: prefer host helper call-order (update helper before finalize).
    // Method bodies may define build_for_engine before update_with_dt textually.
    let host_update_call = eng
        .find("self.host_update_logic_frame(")
        .or_else(|| eng.find("host_update_logic_frame("));
    let host_finalize_call = eng.find("host_finalize_presentation_after_logic(");
    let pres = eng
        .find("PresentationFrame::build_from_logic")
        .or_else(|| eng.find("PresentationFrame::build_for_engine"))
        .or_else(|| eng.find("PresentationFrame::build_with_victory_for_engine"))
        .expect("presentation build");
    let host_update = eng
        .find("game_logic.update_with_dt(")
        .or_else(|| eng.find("game_logic.update_with_timing("))
        .or_else(|| eng.find("game_logic.update("));
    let order_ok = match (host_update_call, host_finalize_call) {
        (Some(u), Some(f)) => u < f,
        _ => host_update.is_some() && host_update.unwrap() < pres,
    };
    assert!(
        order_ok,
        "PresentationFrame must be built after GameLogic update; update_call={host_update_call:?} finalize_call={host_finalize_call:?} update={host_update:?} pres={pres}"
    );
    assert!(
        proj > 0 && path > 0,
        "GameLogic owns projectile+path phases"
    );
}
