//! Dual-tick presentation HUD selection residual tests.

pub use super::*;

#[test]
fn dual_tick_after_map_load_seeds_hud_selection_health() {
    // Residual closed by this change: after skirmish config + (optional) map load,
    // dual-tick presentation must put selection health on GameHUD.
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("ShellHudSel");
    assert!(apply_skirmish_config(&mut logic, &cfg).is_ok());
    // Retail skirmish runs short-game rules (C++ GameLogic.cpp:1606
    // setVictoryConditions(VICTORY_NOBUILDINGS)) and a structure-less playable
    // player is defeated on the first tick (VictoryConditions.cpp:262-276,
    // Player::killPlayer destroys the army). Seed the established
    // MpCountForVictory keep-alive (continue_attack.rs / sell_heal.rs
    // precedent) so the selection-health residual is what's under test.
    if !logic.templates.contains_key("VictoryKeepAlive") {
        let mut t = ThingTemplate::new("VictoryKeepAlive");
        t.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::MpCountForVictory);
        logic.templates.insert("VictoryKeepAlive".into(), t);
    }
    let _keep_alive = logic
        .create_object("VictoryKeepAlive", Team::USA, Vec3::new(6.0, 0.0, 6.0))
        .expect("keep-alive");
    let mut t = ThingTemplate::new("ShellSelUnit");
    t.set_health(64.0);
    t.add_kind_of(KindOf::Infantry);
    t.add_kind_of(KindOf::Selectable);
    logic.templates.insert("ShellSelUnit".into(), t);
    // Retail skirmish runs VICTORY_NOBUILDINGS (C++ GameLogic.cpp:1606);
    // a structure-less player is defeated on the first logic frame and
    // Player::killPlayer destroys its army (VictoryConditions.cpp). Seed a
    // victory-counting HQ like a real starting base so the selection under
    // test survives the dual tick.
    let mut hq = ThingTemplate::new("ShellSelHQ");
    hq.set_health(100.0);
    hq.add_kind_of(KindOf::Structure);
    hq.add_kind_of(KindOf::MpCountForVictory);
    logic.templates.insert("ShellSelHQ".into(), hq);
    let _hq_id = logic
        .create_object("ShellSelHQ", Team::USA, Vec3::new(20.0, 0.0, 20.0))
        .expect("hq");
    let id = logic
        .create_object("ShellSelUnit", Team::USA, Vec3::new(2.0, 0.0, 2.0))
        .expect("unit");
    if let Some(p) = logic.get_player_mut(0) {
        p.selected_objects = vec![id];
    }
    if let Some(o) = logic.host_object_mut(id) {
        o.selected = true;
        o.status.selected = true;
    }

    // Seed like start_game_from_ui before first logic frame.
    let mut hud = GameHUD::new();
    let seed = PresentationFrame::build_and_apply_for_hud(&logic, 0, &mut hud);
    assert!(
        seed.alive_object_count() >= 1,
        "seed presentation must see map/host units"
    );
    assert!(
        hud.selected_unit_ids().contains(&id),
        "seed apply must set HUD selection"
    );

    logic.update();
    let post = PresentationFrame::build_and_apply_for_hud(&logic, 0, &mut hud);
    let info = hud
        .selected_unit_infos()
        .iter()
        .find(|u| u.object_id == id)
        .expect("dual-tick HUD selection health");
    assert!(
        (info.health_current - 64.0).abs() < 0.01,
        "health from presentation after dual-tick: {}",
        info.health_current
    );
    assert!(
        hud.selection_panel().has_positive_health(),
        "ControlBar selection panel health after dual-tick"
    );
    assert!(
        (hud.selection_panel().health_current - 64.0).abs() < 0.01,
        "selection panel HP from presentation: {}",
        hud.selection_panel().health_current
    );
    assert_eq!(post.frame.0, logic.get_frame());
    assert!(!post.hud_minimap_units().is_empty());

    #[cfg(feature = "game_client")]
    {
        let mut bar = game_client::gui::control_bar::ControlBar::new();
        post.apply_to_control_bar(&mut bar);
        let (hp, _) = bar
            .selection_panel_health()
            .expect("ControlBar health from dual-tick presentation");
        assert!((hp - 64.0).abs() < 0.01, "ControlBar HP {hp}");
    }
}
