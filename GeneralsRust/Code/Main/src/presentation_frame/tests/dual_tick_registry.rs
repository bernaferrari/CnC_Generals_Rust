use super::*;

fn complete_impl_method<'a>(source: &'a str, signature: &str) -> &'a str {
    let start = source.find(signature).expect("actual method declaration");
    let remaining = &source[start..];
    let end = remaining
        .find("\n    }\n")
        .expect("complete impl method closing boundary");
    &remaining[..end + "\n    }\n".len()]
}

/// Retail skirmish runs short-game rules (C++ GameLogic.cpp:1606
/// VICTORY_NOBUILDINGS): a structure-less playable player is defeated on the
/// first logic frame and Player::killPlayer destroys its army
/// (VictoryConditions.cpp). Real players start with a victory-counting
/// structure, so tests that run logic.update() seed one to keep the pinned
/// units alive (shell_smoke dual_tick.rs / transform_health.rs precedent).
fn seed_victory_keep_alive(logic: &mut GameLogic) {
    if !logic.templates.contains_key("VictoryKeepAlive") {
        let mut t = ThingTemplate::new("VictoryKeepAlive");
        t.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::MpCountForVictory);
        logic.templates.insert("VictoryKeepAlive".into(), t);
    }
    logic
        .create_object(
            "VictoryKeepAlive",
            Team::USA,
            glam::Vec3::new(60.0, 0.0, 60.0),
        )
        .expect("keep-alive structure");
}

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

    let snap = PresentationFrame::build_from_logic(&mut logic, 0);
    assert_eq!(snap.frame.0, logic.get_frame());
    assert!(snap.objects.iter().any(|o| o.id == id));
    assert_eq!(snap.local_supplies, 10_000);
    // Snapshot is owned — mutating world after build must not require re-borrow of snap.
    logic.update();
    assert_eq!(snap.objects.len(), 1);
    let h1 = snap.presentation_hash();
    let snap2 = PresentationFrame::build_from_logic(&mut logic, 0);
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
        PresentationFrame::build_from_logic(&mut logic, 0).presentation_hash()
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
    let client_view = PresentationFrame::build_from_logic(&mut logic, 0);
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
    let next = PresentationFrame::build_from_logic(&mut logic, 0);
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
    seed_victory_keep_alive(&mut logic);
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
    let snap = PresentationFrame::build_from_logic(&mut logic, 0);
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
            *oid == id && (0.0..=1.0).contains(x) && (0.0..=1.0).contains(z)
        }),
        "minimap units must be normalized into world bounds (0..1)"
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
    seed_victory_keep_alive(&mut logic);
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
    let snap = PresentationFrame::build_and_apply_for_hud(&mut logic, 0, &mut hud);
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
    seed_victory_keep_alive(&mut logic);
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
        &mut logic, 0, &mut hud, &mut ui, &mut rts, &mut cmd,
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
    let snap = PresentationFrame::build_from_logic(&mut logic, 0);
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
    let snap = PresentationFrame::build_from_logic(&mut logic, 0);
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
    let snap = PresentationFrame::build_from_logic(&mut logic, 0);
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
    let snap = PresentationFrame::build_from_logic(&mut logic, 0);
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

    let snap = PresentationFrame::build_from_logic(&mut logic, 0);
    let ro = snap.objects.iter().find(|o| o.id == id).expect("in snap");
    assert!((ro.position.x - 3.0).abs() < 0.01);
    assert!((ro.position.z + 8.0).abs() < 0.01);
    assert_eq!(ro.team, Team::USA);
    // C++ getIndicatorColor: build resolves team_color to the controlling
    // player's house color (Wave 503 parity), not the object's raw tint.
    let expected_team_color = logic
        .player_house_color_rgba(ro.owner_player_id)
        .unwrap_or([0.1, 0.2, 0.9, 1.0]);
    assert_eq!(ro.team_color, expected_team_color);
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
    // A normal, unchanged visual template retains the resident authored
    // model. The gameplay template name is not a replacement model key.
    assert_eq!(inputs[0].model_key, "AVTank");
    assert_eq!(inputs[0].template_name, "MeshUnit");
    assert!((inputs[0].position.x - 3.0).abs() < 0.01);
    assert!(inputs[0].selected);
    assert!(!inputs[0].engine_bridged);
    assert_eq!(inputs[0].fow_visibility, ro.fow_visibility);

    // Mutate authority after snapshot — inputs must stay frozen.
    if let Some((o, fixture_partition)) = logic.host_object_and_partition_mut(id) {
        o.set_position_with_partition_owner(
            glam::Vec3::new(999.0, 0.0, 999.0),
            0,
            &mut Default::default(),
            fixture_partition,
        );
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
    let bridge_at_build = FOWRenderingBridge::get_object_visibility(
        logic.world_services.shroud().lock().ok().as_deref(),
        0,
        id,
    );
    let snap = PresentationFrame::build_from_logic(&mut logic, 0);
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
    let snap2 = PresentationFrame::build_from_logic(&mut logic, 0);
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

    let snap = PresentationFrame::build_from_logic(&mut logic, 0);
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
    let snap = PresentationFrame::build_from_logic(&mut logic, 0);
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
    let snap = PresentationFrame::build_from_logic(&mut logic, 0);
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

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("FowGridSnap");
    apply_skirmish_config(&mut logic, &cfg).expect("config");

    // Activate FOW runtime (visible membership) without permanent map reveal so
    // baseline is not fail-open fully-visible and reveal can change fingerprint.
    {
        let shroud_manager = logic.world_services.shroud();
        let mut shroud = shroud_manager.lock().expect("shroud");
        shroud.clear_all();
        shroud.init_shroud_grid(500.0, 500.0); // manager-owned 40-wu cells
        // Host residual API — keeps FOW filters active without dual registry.
        shroud.mark_host_vision_ready();
        shroud.mark_host_object_seen(0, 1);
        let _ = shroud.update(1);
        // No reveal_map_for_player_permanently yet — terrain stays mostly shrouded.
    }

    // Build with active hidden grid (last_update_frame > 0 after update above).
    let bridge_grid = FOWRenderingBridge::snapshot_terrain_grid(
        logic.world_services.shroud().lock().ok().as_deref(),
        0,
        false,
    );
    let snap = PresentationFrame::build_from_logic(&mut logic, 0);

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
    // Cell size is the shroud manager's constant (SHROUD_GRID_CELL_SIZE = 40 wu,
    // C++ parity), so derive the frozen dims from the bridge grid instead of
    // pinning the stale 50-wu 10x10 arithmetic.
    assert_eq!(snap.fow_grid.width, bridge_grid.width);
    assert_eq!(snap.fow_grid.height, bridge_grid.height);
    assert_eq!(
        snap.fow_grid.cell_count(),
        (snap.fow_grid.width * snap.fow_grid.height) as usize,
        "compact grid"
    );
    // Projected shroud is 1:1 with the frozen FOW grid (no extra source
    // border at the current C++-parity cell size).
    assert_eq!(snap.projected_shroud.grid_width, snap.fow_grid.width);
    assert_eq!(snap.projected_shroud.grid_height, snap.fow_grid.height);
    assert_eq!(snap.projected_shroud.texture_extent(), Some((16, 16)));
    assert_eq!(snap.projected_shroud.texels.len(), 256);
    assert!(
        snap.terrain_projected_shroud().is_some(),
        "active non-shell frame must expose only its frozen shroud projection"
    );

    // R8 payload length matches grid; encoding is deterministic.
    let r8 = snap.terrain_fow_r8().expect("active grid has r8");
    assert_eq!(r8.len(), snap.fow_grid.cell_count());
    assert_eq!(r8, snap.fow_grid.to_r8_texture());

    // Dual-build consistency.
    let snap2 = PresentationFrame::build_from_logic(&mut logic, 0);
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
        let shroud_manager = logic.world_services.shroud();
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
    let snap_after = PresentationFrame::build_from_logic(&mut logic, 0);
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
        let shell_snap = PresentationFrame::build_from_logic(&mut shell_logic, 0);
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

    let snap = PresentationFrame::build_from_logic(&mut logic, 0);
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
    let mut frame = PresentationFrame::build_from_logic(&mut logic, 0);
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
    let fallback_frame = PresentationFrame::build_from_logic(&mut logic, 0);
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
    let frame = PresentationFrame::build_from_logic(&mut logic, 0);
    assert!(frame.world_env.skybox_enabled);
}
