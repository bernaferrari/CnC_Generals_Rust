//! Live-host hover tooltip: InGameUI createMouseoverHint catalog path.

use game_client_rust::gui::ingame_ui::{InGameUI, PresentationUnitCatalogEntry};
use game_client_rust::input::mouse::with_mouse;
use game_engine::common::language::Language;
use gamelogic::common::ObjectShroudStatus;

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
        kind_names: Vec::new(),
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

#[test]
fn create_mouseover_hint_sets_cursor_tooltip_for_named_object_under_cursor() {
    Language::clear_localized_strings();
    Language::register_localized_string("ThingTemplate:AmericaRanger", "Ranger");
    let catalog = [catalog_unit(7, "AmericaRanger")];

    let moused = InGameUI::apply_catalog_mouseover_tooltip(&catalog, Some(7), false);

    assert_eq!(moused, 7);
    let tip = with_mouse(|m| m.cursor_tooltip_state().tooltip_text.clone());
    assert_eq!(tip, "Ranger");
    Language::clear_localized_strings();
}
