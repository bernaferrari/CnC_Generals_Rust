use game_engine::common::ascii_string::AsciiString;
use game_engine::common::ini::ini_command_button::{get_control_bar_mut, initialize_control_bar};
use game_engine::common::ini::ini_command_set::{
    get_command_set_manager, initialize_command_set_manager,
};
use game_engine::common::ini::ini_upgrade::IniUpgrade;
use gamelogic::system::load_map_ini_ui_overrides_from_contents;

#[test]
fn map_ini_create_overrides_applies_command_set_and_upgrade() {
    initialize_command_set_manager();
    initialize_control_bar();
    if let Some(mut bar) = get_control_bar_mut() {
        bar.new_command_button("Command_ConstructAmericaPowerPlant".to_string());
        bar.new_command_button("Command_ConstructAmericaBarracks".to_string());
    }

    let mixed = "\
Object SomeUnit
  KindOf = STRUCTURE
End

CommandSet MapIniDozerCommandSet
  1 = Command_ConstructAmericaPowerPlant
  2 = Command_ConstructAmericaBarracks
End

Upgrade MapIniRangerCapture
  DisplayName = MapOverrideCapture
End

Weather
  SnowEnabled = Yes
End
";
    let applied = load_map_ini_ui_overrides_from_contents(mixed)
        .expect("map.ini CREATE_OVERRIDES must dispatch CommandSet/Upgrade");
    assert!(applied >= 2, "expected CommandSet+Upgrade, got {applied}");

    let manager = get_command_set_manager().expect("CommandSet manager");
    let set = manager
        .find_command_set_resolved("MapIniDozerCommandSet")
        .expect("map.ini CommandSet override must apply");
    assert_eq!(
        set.get_button_at_position(0).map(String::as_str),
        Some("Command_ConstructAmericaPowerPlant")
    );
    assert_eq!(
        set.get_button_at_position(1).map(String::as_str),
        Some("Command_ConstructAmericaBarracks")
    );

    let upgrade = IniUpgrade::find_template_by_name(&AsciiString::from("MapIniRangerCapture"))
        .expect("map.ini Upgrade CREATE_OVERRIDES must apply");
    assert_eq!(upgrade.display_name.as_str(), "MapOverrideCapture");
}
