//! Executable command and lifecycle regressions.

#[test]
fn evacuation_clears_occupancy_and_publishes_the_released_passenger() {
    use crate::game_logic::Team;
    use crate::presentation_frame::PresentationFrame;
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    // C++ loads Object INI before NewGame: the bunker authors its garrison
    // contain module and the rider is an infantry Object.
    let mut logic = crate::skirmish_config::lobby_definition_tests::slot_world_with_objects(&[
        crate::fixture_content::FixtureObject::new("EvacBunker201")
            .kind_of("STRUCTURE")
            .health(500.0)
            .module("Behavior = GarrisonContain FixtureContain\n ContainMax = 5\n EnterSound = None\nEnd"),
        crate::fixture_content::FixtureObject::new("EvacInf201")
            .kind_of("INFANTRY SELECTABLE")
            .health(100.0)
            .walker(),
    ]);
    let cfg = crate::skirmish_config::lobby_definition_tests::slot_config(
        &logic,
        &golden_skirmish_config("EvacContain201"),
    );
    apply_skirmish_config(&mut logic, &cfg).unwrap();
    let bunker = logic
        .create_object("EvacBunker201", Team::USA, Vec3::new(10.0, 0.0, 10.0))
        .unwrap();
    let inf = logic
        .create_object("EvacInf201", Team::USA, Vec3::new(12.0, 0.0, 12.0))
        .unwrap();
    let (passenger, partition) = logic.host_object_and_partition_mut(inf).unwrap();
    passenger.set_contained_by_with_partition_owner(Some(bunker), true, partition);
    let building = logic
        .host_object_mut(bunker)
        .unwrap()
        .building_data
        .as_mut()
        .unwrap();
    building.garrisoned_units.push(inf);
    let occupied = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    let inside = occupied
        .objects
        .iter()
        .find(|row| row.id == bunker)
        .unwrap();
    assert_eq!(inside.garrisoned_units, [inf]);

    assert!(logic.evacuate_container_now(bunker, false).unwrap());
    assert_eq!(logic.host_object(inf).unwrap().contained_by, None);
    let building = logic
        .host_object(bunker)
        .unwrap()
        .building_data
        .as_ref()
        .unwrap();
    assert!(building.garrisoned_units.is_empty());

    let presentation = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    let row = presentation
        .objects
        .iter()
        .find(|row| row.id == bunker)
        .unwrap();
    assert!(row.garrisoned_units.is_empty());
    let released = presentation
        .objects
        .iter()
        .find(|row| row.id == inf)
        .unwrap();
    assert!(!released.drawable_hidden);
}
