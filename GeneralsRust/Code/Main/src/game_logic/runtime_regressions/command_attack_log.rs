//! Executable command and lifecycle regressions.

#[test]
fn attack_command_publishes_the_receiving_worlds_target_id() {
    use crate::command_system::{CommandType, GameCommand, ModifierKeys};
    use crate::game_logic::Team;
    use crate::presentation_frame::PresentationFrame;
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;
    use std::time::SystemTime;

    // C++ loads Object and Weapon INI before NewGame; AttackObject reaches
    // only an Object that authors an AIUpdateInterface and a WeaponSet.
    use crate::fixture_content::{FixtureObject, FixtureWeapon};
    let mut logic =
        crate::skirmish_config::lobby_definition_tests::slot_world_with_objects_and_weapons(
            &[
                FixtureObject::new("AtkLogUnit197")
                    .kind_of("VEHICLE SELECTABLE")
                    .health(120.0)
                    .walker()
                    .armed("AtkLogGun197"),
                FixtureObject::new("AtkLogTarget197")
                    .kind_of("VEHICLE SELECTABLE")
                    .health(120.0),
            ],
            &[FixtureWeapon::new("AtkLogGun197").damage(10.0).range(100.0)],
        );
    let cfg = crate::skirmish_config::lobby_definition_tests::slot_config(
        &logic,
        &golden_skirmish_config("AtkLog197"),
    );
    apply_skirmish_config(&mut logic, &cfg).unwrap();
    let attacker = logic
        .create_object("AtkLogUnit197", Team::USA, Vec3::new(10.0, 0.0, 10.0))
        .unwrap();
    let target = logic
        .create_object("AtkLogTarget197", Team::China, Vec3::new(30.0, 0.0, 30.0))
        .unwrap();

    logic.queue_command(GameCommand {
        command_type: CommandType::AttackObject { target_id: target },
        player_id: 0,
        command_id: 1,
        timestamp: SystemTime::now(),
        selected_units: vec![attacker],
        modifier_keys: ModifierKeys::default(),
    });
    logic.process_commands();

    assert_eq!(logic.host_object(attacker).unwrap().target, Some(target));
    let presentation = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    let row = presentation
        .objects
        .iter()
        .find(|row| row.id == attacker)
        .unwrap();
    assert_eq!(row.attack_target, Some(target));
    assert!(presentation.objects.iter().any(|row| row.id == target));
}
