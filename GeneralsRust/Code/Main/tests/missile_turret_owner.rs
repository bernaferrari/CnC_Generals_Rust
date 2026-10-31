//! MissileAIUpdate inherits AIUpdateModuleData in the original engine.
use generals_main::game_logic::{GameLogic, Team};
use generals_main::save_load::SnapshotBuilder;
use glam::Vec3;

const OBJECT: &str = "OwnedMissileTurret";

#[test]
fn missile_parser_preserves_flight_and_inherited_ai_fields() {
    use game_engine::common::ini::INI;
    use gamelogic::object::update::missile_ai_update::MissileAIUpdateModuleData;

    let mut data = MissileAIUpdateModuleData::default();
    INI::new()
        .with_inline_source(
            "IgnitionDelay = 1000\nTurret\n ControlledWeaponSlots = SECONDARY\nEnd\nEnd\n",
            |ini| data.parse_from_ini(ini),
        )
        .unwrap();
    assert_eq!(data.ignition_delay, 30);
    assert_ne!(
        data.ai_base.turret_primary().unwrap().turret_weapon_slots,
        0
    );
}

fn world(primary_slots: &str, alternate_slots: &str, angle: i32) -> GameLogic {
    let source = format!(
        "Object {OBJECT}\n KindOf = STRUCTURE SELECTABLE\n Body = ActiveBody Body\n  MaxHealth = 500\n  InitialHealth = 500\n End\n Behavior = MissileAIUpdate Missile\n  IgnitionDelay = 1000\n  Turret\n   TurretTurnRate = 30\n   NaturalTurretAngle = {angle}\n   {primary_slots}\n  End\n  AltTurret\n   TurretTurnRate = 90\n   {alternate_slots}\n  End\n End\nEnd\n"
    );
    let mut world = GameLogic::new();
    world
        .admit_object_ini_sources([("MissileTurret.ini".into(), source)])
        .expect("missile accepts inherited AI fields");
    world
}

#[test]
fn missile_inherited_turrets_belong_to_receiving_session() {
    let mut first = world(
        "ControlledWeaponSlots = PRIMARY",
        "ControlledWeaponSlots = SECONDARY",
        -30,
    );
    let mut peer = world(
        "ControlledWeaponSlots = SECONDARY",
        "ControlledWeaponSlots = PRIMARY",
        45,
    );
    let a = first.create_object(OBJECT, Team::USA, Vec3::ZERO).unwrap();
    let b = peer.create_object(OBJECT, Team::USA, Vec3::ZERO).unwrap();
    assert_eq!(a, b);
    let obj = first.host_object(a).unwrap();
    assert!((obj.turret_natural_angle_deg + 30.0).abs() < 1e-5);
    assert!(obj.is_weapon_slot_on_turret(0));
    let peer_obj = peer.host_object(b).unwrap();
    assert!((peer_obj.turret_natural_angle_deg - 45.0).abs() < 1e-5);
    assert!(peer_obj.is_weapon_slot_on_turret(1));
    let snapshot = SnapshotBuilder::new()
        .create_world_snapshot(&first)
        .unwrap();
    let mut restored = world(
        "ControlledWeaponSlots = PRIMARY",
        "ControlledWeaponSlots = SECONDARY",
        -30,
    );
    SnapshotBuilder::new()
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    let obj = restored.host_object(a).unwrap();
    assert!((obj.turret_natural_angle_deg + 30.0).abs() < 1e-5);
    assert!(obj.is_weapon_slot_on_turret(0));
}

#[test]
fn missile_rejects_invalid_inherited_primary_and_alternate_turrets() {
    for (primary, alternate) in [
        ("", "ControlledWeaponSlots = SECONDARY"),
        ("ControlledWeaponSlots = PRIMARY", ""),
    ] {
        let mut invalid = world(primary, alternate, 0);
        assert!(
            invalid
                .create_object(OBJECT, Team::USA, Vec3::ZERO)
                .is_none()
        );
        assert_eq!(invalid.host_objects().registered_len(), 0);
    }
}
