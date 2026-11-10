//! Dual-tick presentation HUD selection residual tests.

pub use super::*;

#[test]
fn dual_tick_after_map_load_seeds_hud_selection_health() {
    // Residual closed by this change: after skirmish config + (optional) map load,
    // dual-tick presentation must put selection health on GameHUD.
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("ShellHudSel");
    // Retail skirmish runs VICTORY_NOBUILDINGS (C++ GameLogic.cpp:1606); a
    // structure-less player is defeated on the first logic frame and
    // Player::killPlayer destroys its army (VictoryConditions.cpp). Like a
    // real starting base, author a victory-counting HQ so the selection under
    // test survives the dual tick. Content is admitted before NewGame
    // (GameEngine.cpp:428-458).
    crate::fixture_content::admit_fixture_objects(
        &mut logic,
        &[
            crate::fixture_content::FixtureObject::new("ShellSelUnit")
                .kind_of("INFANTRY SELECTABLE")
                .health(64.0),
            crate::fixture_content::FixtureObject::new("ShellSelHQ")
                .kind_of("STRUCTURE MP_COUNT_FOR_VICTORY")
                .health(100.0),
        ],
    );
    assert!(apply_skirmish_config(&mut logic, &cfg).is_ok());
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
    let seed = PresentationFrame::build_and_apply_for_hud(&mut logic, 0, &mut hud);
    assert!(
        seed.alive_object_count() >= 1,
        "seed presentation must see map/host units"
    );
    assert!(
        hud.selected_unit_ids().contains(&id),
        "seed apply must set HUD selection"
    );

    logic.update();
    let post = PresentationFrame::build_and_apply_for_hud(&mut logic, 0, &mut hud);
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
        let owner = game_client::gui::window_manager::WindowManagerOwner::new();
        game_client::gui::game_window::WindowEventEffects::with_standalone(|effects| {
            owner.with_manager(|manager| {
                let mut bar = game_client::gui::control_bar::ControlBar::new();
                post.apply_to_control_bar(&mut bar, manager, effects);
                let (hp, _) = bar
                    .selection_panel_health()
                    .expect("ControlBar health from dual-tick presentation");
                assert!((hp - 64.0).abs() < 0.01, "ControlBar HP {hp}");
            });
        });
    }
}
