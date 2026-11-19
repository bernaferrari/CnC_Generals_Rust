//! Executable command and lifecycle regressions.

use crate::game_logic::{GameLogic, Team};
use glam::Vec3;

#[test]
fn owned_dozer_phase_completes_structure_without_ready_writeback() {
    use crate::fixture_content::{admit_fixture_objects, FixtureObject};
    use crate::game_logic::Player;
    let mut logic = GameLogic::new();
    // A live session runs past frame 0 (the Dozer machine picks its request next update).
    logic.frame = 10;
    crate::fixture_content::admit_retail_production_energy_rules(&mut logic);
    logic.admit_default_locomotors_if_unadmitted();
    logic.add_player(Player::new(0, Team::USA, "OwnedBuilder", true));
    // C++ Energy consumption == 0 returns production; a powered grid builds at
    // full speed, an empty one at the low-energy rate.
    admit_fixture_objects(
        &mut logic,
        &[
            FixtureObject::new("SfBarracks")
                .kind_of("STRUCTURE SELECTABLE")
                .health(200.0)
                .build_time(10.0),
            FixtureObject::new("SfPlant")
                .kind_of("STRUCTURE SELECTABLE")
                .health(200.0)
                .energy(10),
            FixtureObject::new("SfDozer")
                .kind_of("VEHICLE DOZER SELECTABLE")
                .health(200.0)
                .field("Locomotor = SET_NORMAL AmericaVehicleDozerLocomotor")
                .dozer(),
        ],
    );
    let plant = logic
        .create_object_for_player("SfPlant", 0, Vec3::new(-300.0, 0.0, -300.0))
        .unwrap();
    {
        let plant = logic.host_object_mut(plant).unwrap();
        plant.set_status_under_construction(false);
        plant.construction_percent = 1.0;
    }
    let id = logic
        .create_object_for_player("SfBarracks", 0, Vec3::ZERO)
        .unwrap();
    let dozer = logic
        .create_object_for_player("SfDozer", 0, Vec3::new(5.0, 0.0, 0.0))
        .unwrap();
    {
        let object = logic.host_object_mut(id).unwrap();
        object.set_status_under_construction(true);
        object.construction_percent = 0.99;
    }
    assert!(logic.resume_construction(&[dozer], id).expect("resume"));
    // The dozer walks to its dock, builds the last percent and completes the
    // structure in its own construction phase.
    for _ in 0..600 {
        let _ = logic.update_with_dt(1.0 / 30.0);
        if !logic.host_object(id).unwrap().status.under_construction {
            break;
        }
    }
    assert_eq!(logic.host_object(id).unwrap().construction_percent, 1.0);
    assert!(!logic.host_object(id).unwrap().status.under_construction);
    assert_eq!(
        logic.host_object(dozer).unwrap().dozer_task_build_target(),
        None
    );
}
