use generals_main::game_logic::{GameLogic, Player, Team, host_slave_drones::SlaveDroneKind};
use glam::Vec3;

const MASTER: &str = "AmericaVehicleHumvee";
const SCOUT: &str = "AmericaVehicleScoutDrone";

fn world(team: Team, position: Vec3) -> (GameLogic, generals_main::game_logic::ObjectId) {
    let mut logic = GameLogic::new();
    let object_ini = format!(
        "Object {MASTER}\nKindOf = VEHICLE SELECTABLE\nIsTrainable = Yes\nBody = ActiveBody Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd\nObject {SCOUT}\nKindOf = VEHICLE SELECTABLE\nIsTrainable = Yes\nBody = ActiveBody Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\nLocomotor = SET_NORMAL ComancheLocomotor\nBehavior = SlavedUpdate Slave\nGuardMaxRange = 35\nEnd\nEnd\n"
    );
    logic
        .admit_object_ini_sources([("SlaveDroneOwner.ini".into(), object_ini)])
        .unwrap();
    logic.add_player(Player::new(7, team, "Owner", true));
    let master = logic.create_object_for_player(MASTER, 7, position).unwrap();
    (logic, master)
}

#[test]
fn public_drone_spawn_uses_each_receiving_world_for_same_ids() {
    let (mut usa, usa_master) = world(Team::USA, Vec3::new(80.0, 0.0, 90.0));
    let (mut china, china_master) = world(Team::China, Vec3::new(280.0, 0.0, 390.0));
    assert_eq!(usa_master, china_master);

    let usa_drone = usa
        .residual_attach_slave_drone(usa_master, SlaveDroneKind::Scout)
        .expect("receiving USA master should create and bind its scout");
    assert!(
        china
            .host_objects()
            .registered_values()
            .all(|object| { object.template_name != SCOUT })
    );
    let usa_child = usa.host_object(usa_drone).unwrap();
    assert_eq!(usa_child.producer_id, Some(usa_master));
    assert_eq!(usa_child.team, Team::USA);

    let china_drone = china
        .residual_attach_slave_drone(china_master, SlaveDroneKind::Scout)
        .expect("receiving China master should create and bind its scout");
    assert_eq!(usa_drone, china_drone);
    let china_child = china.host_object(china_drone).unwrap();
    assert_eq!(china_child.producer_id, Some(china_master));
    assert_eq!(china_child.team, Team::China);
    assert_eq!(
        china_child.get_position().x - china.host_object(china_master).unwrap().get_position().x,
        usa_child.get_position().x - usa.host_object(usa_master).unwrap().get_position().x
    );

    // Retiring and reusing B's identities must not redirect the still-live A
    // master/slave relationship through the shared ObjectId values.
    china.reset();
    drop(china);
    let (mut replacement, replacement_master) = world(Team::China, Vec3::new(480.0, 0.0, 690.0));
    assert_eq!(replacement_master, usa_master);
    let replacement_drone = replacement
        .residual_attach_slave_drone(replacement_master, SlaveDroneKind::Scout)
        .expect("replacement session creates its own slave after ID reuse");
    assert_eq!(replacement_drone, usa_drone);
    let replacement_child = replacement.host_object(replacement_drone).unwrap();
    assert_eq!(replacement_child.producer_id, Some(replacement_master));
    assert_eq!(replacement_child.team, Team::China);
    let retained_usa_child = usa.host_object(usa_drone).unwrap();
    assert_eq!(retained_usa_child.producer_id, Some(usa_master));
    assert_eq!(retained_usa_child.team, Team::USA);
}
