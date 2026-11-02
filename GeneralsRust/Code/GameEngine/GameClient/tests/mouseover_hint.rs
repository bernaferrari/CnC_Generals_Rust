//! Live-host hover tooltip: HintSpy → InGameUI createMouseoverHint.
//! State stays on the receiving GameClient (no process-wide TheInGameUI / with_mouse).

use std::collections::HashMap;

use game_client_rust::core::game_client::{ClientInputAuthority, GameClient};
use game_client_rust::core::subsystems::InGameUISubsystem;
use game_client_rust::gui::ingame_ui::{
    MouseoverObjectObservation, PresentationUnitCatalogEntry, ReceivingMouseoverFacts,
};
use gamelogic::common::ObjectShroudStatus;

fn client() -> GameClient {
    let mut client =
        GameClient::new_with_input_authority(ClientInputAuthority::NativeHosted).unwrap();
    client.init_input_subsystems().unwrap();
    client
}

fn catalog_unit(id: u32, template: &str) -> PresentationUnitCatalogEntry {
    PresentationUnitCatalogEntry {
        object_id: id,
        template_name: template.to_string(),
        team_name: String::new(),
        selectable: true,
        position: [0.0; 3],
        orientation: 0.0,
        disabled: false,
        under_construction: false,
        construction_percent: 0.0,
        max_garrison: 0,
        occupant_count: 0,
        ocl_timer_seconds: 0,
        sold: false,
        script_unsellable: false,
        unselectable: false,
        destroyed: false,
        masked: false,
        effectively_stealthed: false,
        disguised: false,
        disguise_as_template: None,
        disguise_as_team: None,
        kind_names: std::sync::Arc::new(Vec::new()),
        special_power_ready: false,
        airborne_target: false,
        shroud_status: ObjectShroudStatus::Clear,
        slaver_object_id: None,
        health_current: 100.0,
        health_maximum: 100.0,
        veterancy_overlay: None,
        production_progress: None,
        production_template: None,
        production_paused: false,
        command_set_name: String::new(),
        hotkey_group: -1,
        caption: String::new(),
        supply_boxes: None,
    }
}

fn facts_for(id: u32, template: &str, display: &str) -> ReceivingMouseoverFacts {
    let mut facts = ReceivingMouseoverFacts::default();
    let mut display_names = HashMap::new();
    display_names.insert(template.to_string(), display.to_string());
    facts.objects.insert(
        id,
        MouseoverObjectObservation {
            visible_friendly: true,
            player: None,
            display_names,
        },
    );
    facts
}

#[test]
fn create_mouseover_hint_sets_cursor_tooltip_only_on_receiving_client() {
    let mut owner = client();
    let mut peer = client();
    let mut ui = InGameUISubsystem::default();
    ui.set_presentation_unit_catalog(vec![catalog_unit(7, "AmericaRanger")]);
    owner.set_in_game_ui_subsystem(ui);
    peer.set_in_game_ui_subsystem(InGameUISubsystem::default());
    peer.input_mouse_mut()
        .unwrap()
        .set_cursor_tooltip("peer retained".into(), Some(0), None, None);

    let facts = facts_for(7, "AmericaRanger", "Ranger");
    owner.create_mouseover_hint(Some(7), false, 1, &facts);

    assert_eq!(
        owner
            .in_game_ui_subsystem()
            .unwrap()
            .get_moused_over_drawable_id(),
        7
    );
    assert_eq!(
        owner
            .input_mouse()
            .unwrap()
            .cursor_tooltip_state()
            .tooltip_text,
        "Ranger"
    );
    assert_eq!(
        peer.input_mouse()
            .unwrap()
            .cursor_tooltip_state()
            .tooltip_text,
        "peer retained"
    );

    owner.create_mouseover_hint(None, true, 1, &Default::default());
    assert_eq!(
        owner
            .in_game_ui_subsystem()
            .unwrap()
            .get_moused_over_drawable_id(),
        0
    );
    assert_eq!(
        owner
            .input_mouse()
            .unwrap()
            .cursor_tooltip_state()
            .tooltip_text,
        ""
    );
    assert_eq!(
        peer.input_mouse()
            .unwrap()
            .cursor_tooltip_state()
            .tooltip_text,
        "peer retained"
    );
}
