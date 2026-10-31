//! TurretAI spawn facts must come from the Object INI definition admitted by
//! the receiving GameLogic. Same-name sessions deliberately disagree.
use generals_main::game_logic::{GameLogic, Team};
use generals_main::save_load::SnapshotBuilder;
use glam::Vec3;

const TEMPLATE: &str = "AmericaTankCrusader";

fn world(turret: Option<(u32, u32, i32, i32)>) -> GameLogic {
    let turret_behavior = turret.map_or_else(String::new, |(turn, pitch, angle, natural_pitch)| {
        format!(
            " Behavior = AIUpdateInterface TurretBrain\n  Turret\n   TurretTurnRate = {turn}\n   TurretPitchRate = {pitch}\n   NaturalTurretAngle = {angle}\n   NaturalTurretPitch = {natural_pitch}\n  End\n End\n"
        )
    });
    let source = format!(
        "Object {TEMPLATE}\n KindOf = VEHICLE SELECTABLE\n Body = ActiveBody BodyTag\n  MaxHealth = 100\n  InitialHealth = 100\n End\n{turret_behavior}End\n"
    );
    let mut world = GameLogic::new();
    world
        .admit_object_ini_sources([("Object.ini".into(), source)])
        .expect("receiving Object INI accepted");
    world
}

fn world_with_slots(slots: &str, disabled: bool) -> GameLogic {
    let source = format!(
        "Object {TEMPLATE}\n KindOf = VEHICLE SELECTABLE\n Body = ActiveBody BodyTag\n  MaxHealth = 100\n  InitialHealth = 100\n End\n Behavior = AIUpdateInterface Brain\n  Turret\n   TurretTurnRate = 30\n   ControlledWeaponSlots = {slots}\n   InitiallyDisabled = {}\n  End\n End\nEnd\n",
        if disabled { "Yes" } else { "No" }
    );
    let mut world = GameLogic::new();
    world
        .admit_object_ini_sources([("Object.ini".into(), source)])
        .expect("receiving Object INI accepted");
    world
}

#[test]
fn disabled_turret_retains_receiving_controlled_slots_independent_of_peer() {
    let mut first = world_with_slots("SECONDARY", true);
    let mut peer = world_with_slots("PRIMARY TERTIARY", false);
    let mut none = world(None);
    let a = first
        .create_object(TEMPLATE, Team::USA, Vec3::ZERO)
        .unwrap();
    let b = peer.create_object(TEMPLATE, Team::USA, Vec3::ZERO).unwrap();
    let c = none.create_object(TEMPLATE, Team::USA, Vec3::ZERO).unwrap();
    assert_eq!((a, a), (b, c));
    let object = first.host_object(a).unwrap();
    assert!(!object.turret_enabled);
    assert!(!object.is_weapon_slot_on_turret(0));
    assert!(object.is_weapon_slot_on_turret(1));
    assert!(!object.is_weapon_slot_on_turret(2));
    let object = peer.host_object(b).unwrap();
    assert!(object.is_weapon_slot_on_turret(0));
    assert!(!object.is_weapon_slot_on_turret(1));
    assert!(object.is_weapon_slot_on_turret(2));
    assert!(!none.host_object(c).unwrap().is_weapon_slot_on_turret(0));

    let snapshot = SnapshotBuilder::new()
        .create_world_snapshot(&first)
        .unwrap();
    let mut restored = world_with_slots("SECONDARY", true);
    SnapshotBuilder::new()
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    let object = restored.host_object(a).unwrap();
    assert!(!object.turret_enabled);
    assert!(object.is_weapon_slot_on_turret(1));
    assert!(!object.is_weapon_slot_on_turret(0));
}

fn assert_spawn(
    world: &GameLogic,
    id: generals_main::game_logic::ObjectId,
    expected: Option<(f32, f32, f32)>,
) {
    let object = world.host_object(id).expect("created object");
    if let Some((turn_rate, angle, pitch)) = expected {
        assert!(object.turret_enabled);
        assert!((object.turret_turn_rate_rad - turn_rate.to_radians() / 30.0).abs() < 1e-6);
        assert!((object.turret_natural_angle_deg - angle).abs() < 1e-5);
        assert!((object.turret_natural_pitch_deg - pitch).abs() < 1e-5);
        assert!((object.turret_angle_deg - angle).abs() < 1e-5);
        assert!((object.turret_pitch_deg - pitch).abs() < 1e-5);
    } else {
        // An admitted definition without AIUpdate/TurretAI is authoritative;
        // the Crusader name must not activate the compatibility name heuristic.
        assert!(!object.turret_enabled);
        assert_eq!(object.turret_natural_angle_deg, 0.0);
        assert_eq!(object.turret_natural_pitch_deg, 0.0);
    }
}

#[test]
fn admitted_turret_spawn_facts_are_isolated_and_rebuilt_after_snapshot_load() {
    let mut owner = world(Some((30, 45, -25, 8)));
    let mut peer = world(Some((120, 60, 75, -12)));
    let owner_id = owner
        .create_object(TEMPLATE, Team::USA, Vec3::ZERO)
        .expect("owner object");
    let peer_id = peer
        .create_object(TEMPLATE, Team::USA, Vec3::ZERO)
        .expect("peer object");
    assert_eq!(owner_id, peer_id);
    assert_spawn(&owner, owner_id, Some((30.0, -25.0, 8.0)));
    assert_spawn(&peer, peer_id, Some((120.0, 75.0, -12.0)));

    let mut no_turret = world(None);
    let no_turret_id = no_turret
        .create_object(TEMPLATE, Team::USA, Vec3::ZERO)
        .expect("object with no authored turret");
    assert_eq!(no_turret_id, owner_id);
    assert_spawn(&no_turret, no_turret_id, None);

    let builder = SnapshotBuilder::new();
    // A non-natural barrel pose is instance state; rates and natural pose are
    // definition state. Restore into the peer's same-name but different INI to
    // check both channels are sourced from the proper owner.
    owner.host_object_mut(owner_id).unwrap().turret_angle_deg = -5.0;
    let snapshot = builder.create_world_snapshot(&owner).expect("snapshot");
    let mut restored = world(Some((120, 60, 75, -12)));
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("restore against receiving Object INI");
    let loaded = restored.host_object(owner_id).expect("restored object");
    assert!((loaded.turret_turn_rate_rad - 120.0_f32.to_radians() / 30.0).abs() < 1e-6);
    assert!((loaded.turret_natural_angle_deg - 75.0).abs() < 1e-5);
    assert!((loaded.turret_natural_pitch_deg + 12.0).abs() < 1e-5);
    assert!((loaded.turret_angle_deg + 5.0).abs() < 1e-5);
    assert_spawn(&peer, peer_id, Some((120.0, 75.0, -12.0)));
}
