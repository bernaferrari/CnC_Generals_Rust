//! hq-2qedt.27: mine runtime binds from admitted module declarations
//! (MinefieldBehavior.cpp:59-69, DemoTrapUpdate.cpp:46-55), not the name.
use super::*;
use crate::game_logic::host_mines::{DemoTrapMode, HostMineKind};
use crate::game_logic::{Player, Team};
use glam::Vec3;

fn spawn(source: &str, name: &str) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "mine-typed-admission.ini".into(),
                source.to_string(),
            )])
            .unwrap(),
        )
        .unwrap();
    world.add_player(Player::new(0, Team::China, "MineOwner", true));
    let id = world.create_object_for_player(name, 0, Vec3::ZERO).unwrap();
    (world, id)
}

#[test]
fn unnamed_minefield_module_binds_authored_fields() {
    let (world, id) = spawn(
        "Object AuthoredGroundHazard\n KindOf = IMMOBILE\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nBehavior = MinefieldBehavior MineTag\n NumVirtualMines = 3\n Regenerates = Yes\n WorkersDetonate = Yes\n StopsRegenAfterCreatorDies = No\n RepeatDetonateMoveThresh = 7\nEnd\nEnd\n",
        "AuthoredGroundHazard",
    );
    let data = world.host_object(id).unwrap().mine_data.as_ref().expect("admitted MinefieldBehavior");
    assert_eq!(data.kind, HostMineKind::LandMine);
    assert_eq!(data.num_virtual_mines, 3);
    assert_eq!(data.virtual_mines_remaining, 3);
    assert!(data.regenerates && data.workers_detonate);
    assert!(!data.stops_regen_after_creator_dies);
    assert!((data.repeat_detonate_move_thresh - 7.0).abs() < 1e-4);
}

#[test]
fn unnamed_demo_trap_module_binds_range_and_mode() {
    let (world, id) = spawn(
        "Object AuthoredTripCharge\n KindOf = IMMOBILE\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nBehavior = DemoTrapUpdate TrapTag\n DefaultProximityMode = No\n TriggerDetonationRange = 42\nEnd\nEnd\n",
        "AuthoredTripCharge",
    );
    let data = world.host_object(id).unwrap().mine_data.as_ref().expect("admitted DemoTrapUpdate");
    assert_eq!(data.kind, HostMineKind::DemoTrap);
    assert!((data.trigger_range - 42.0).abs() < 1e-4);
    assert_eq!(data.demo_trap_mode, DemoTrapMode::Manual);
}

#[test]
fn mine_name_without_mine_module_stays_inert() {
    let (world, id) = spawn(
        "Object ChinaStandardMine\n KindOf = IMMOBILE\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n",
        "ChinaStandardMine",
    );
    assert!(world.host_object(id).unwrap().mine_data.is_none());
}
