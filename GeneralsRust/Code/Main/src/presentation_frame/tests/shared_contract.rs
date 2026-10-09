//! Production consumers for the extracted presentation contract.

use super::*;

#[test]
fn shared_hud_contract_borrows_frozen_values_and_updates_live_ui() {
    // C++ authority: InGameUI.cpp PublicTimer / radar residuals and
    // ControlBarCommand.cpp resource/objective-facing HUD updates.
    let mut logic = GameLogic::new();
    let config = golden_skirmish_config("PresentationContract");
    apply_skirmish_config(&mut logic, &config).expect("skirmish config");
    let frame = PresentationFrame::build_from_logic(&mut logic, 0);

    let hud = frame.hud_read_model();
    assert!(std::ptr::eq(hud.players.as_ptr(), frame.players.as_ptr()));
    assert!(std::ptr::eq(
        hud.superweapon_timers.as_ptr(),
        frame.superweapon_timers.as_ptr()
    ));
    assert!(std::ptr::eq(
        hud.objectives.as_ptr(),
        frame.objectives.as_ptr()
    ));
    assert_eq!(hud.local_player_id, frame.local_player_id);
    assert_eq!(hud.local_team, frame.local_team);

    let event_contract: Option<&generals_presentation::PresentationEvent> = frame.events.first();
    let _ = event_contract;
    let selected_id_contract: &[generals_game_domain::ObjectId] = &frame.selected;
    let _ = selected_id_contract;

    let mut ui = crate::ui::GameUIState::default();
    frame.apply_to_ui_state(&mut ui);
    assert_eq!(ui.player_id, frame.local_player_id);
    assert_eq!(ui.credits, frame.local_supplies as i32);
    assert_eq!(
        ui.power_generated,
        frame.local_power_produced.max(frame.local_power).max(0)
    );
    assert_eq!(ui.power_used, frame.local_power_consumed.max(0));
    assert_eq!(ui.radar_enabled, frame.radar_ui_enabled);
    assert_eq!(ui.objectives, frame.objectives);
}

#[test]
fn shared_command_contract_is_consumed_by_the_existing_unit_command_panel() {
    let button = generals_presentation::UnitCommandButton {
        command_name: "Command_Stop".into(),
        enabled: true,
        ..Default::default()
    };
    let mut panel = crate::ui::UnitCommandPanel::new();
    panel.apply_commands(vec![button]);
    assert_eq!(panel.commands()[0].command_name, "Command_Stop");
    assert!(panel.commands()[0].enabled);
}
