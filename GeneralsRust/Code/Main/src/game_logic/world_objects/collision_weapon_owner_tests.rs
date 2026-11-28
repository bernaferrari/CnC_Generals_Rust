use crate::game_logic::{GameLogic, ObjectId, Team};
use crate::save_load::{SnapshotBuilder, snapshot::decode_bincode_world_snapshot};
use generals_sim::damage::DamageType;
use generals_sim::weapon::WeaponDefinition;
use glam::Vec3;

const WEAPON: &str = "ReceivingContactWeapon";
const SHARED_SOURCE: &str = "FirestormReceivingContact";

fn world(damage: Option<u32>, with_module: bool) -> (GameLogic, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    let module = if with_module {
        "Behavior = FireWeaponCollide ContactTag\nCollideWeapon = ReceivingContactWeapon\n\
         RequiredStatus = +DETECTED\nForbiddenStatus = +STEALTHED\nFireOnce = Yes\nEnd\n"
    } else {
        ""
    };
    world
        .admit_object_ini_sources([(
            "collide-owner.ini".into(),
            format!(
                "Object {SHARED_SOURCE}\nKindOf = IMMOBILE\n{module}End\n\
                 Object ReceivingContactVictim\nKindOf = INFANTRY\n\
                 Body = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd\n"
            ),
        )])
        .unwrap();
    let catalog = damage
        .map(|amount| {
            (
                WEAPON.to_owned(),
                WeaponDefinition {
                    primary_damage: amount as f32,
                    damage_type: DamageType::Unresistable,
                    ..WeaponDefinition::new(WEAPON.to_owned())
                },
            )
        })
        .into_iter()
        .collect();
    world.admit_receiving_weapon_snapshot(catalog).unwrap();
    let source = world
        .create_object(SHARED_SOURCE, Team::China, Vec3::ZERO)
        .unwrap();
    world
        .host_object_mut(source)
        .unwrap()
        .set_status_detected(true);
    let victim = world
        .create_object("ReceivingContactVictim", Team::USA, Vec3::ZERO)
        .unwrap();
    (world, source, victim)
}

#[test]
fn contact_weapon_reads_the_receiving_module_and_weapon_after_save() {
    let (mut owner, source, victim) = world(Some(13), true);
    let (mut peer, peer_source, peer_victim) = world(Some(31), true);
    assert_eq!((source, victim), (peer_source, peer_victim));

    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&owner).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = decode_bincode_world_snapshot(&bytes).unwrap();
    let (mut restored, _, _) = world(Some(13), true);
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();

    let contact = |world: &mut GameLogic| {
        world.dispatch_host_collide_modules(source, victim).unwrap();
        let object = world.host_object(victim).unwrap();
        (object.health.current, object.last_damage_source)
    };
    assert_eq!(contact(&mut peer), (69., Some(source)));
    assert_eq!(contact(&mut owner), (87., Some(source)));
    assert_eq!(contact(&mut restored), (87., Some(source)));
    owner
        .host_object_mut(source)
        .unwrap()
        .set_status_detected(false);
    assert_eq!(contact(&mut owner).0, 87.);
    owner
        .host_object_mut(source)
        .unwrap()
        .set_status_detected(true);
    owner
        .host_object_mut(source)
        .unwrap()
        .set_status_stealthed(true);
    assert_eq!(contact(&mut owner).0, 87.);
    owner
        .host_object_mut(source)
        .unwrap()
        .set_status_stealthed(false);
    // FireWeaponCollide does not set m_everFired after dispatch.
    assert_eq!(contact(&mut owner), (74., Some(source)));
    assert_eq!(peer.host_object(victim).unwrap().health.current, 69.);

    let (mut missing_weapon, _, _) = world(None, true);
    let (mut missing_module, _, _) = world(Some(31), false);
    assert_eq!(contact(&mut missing_weapon), (100., None));
    assert_eq!(contact(&mut missing_module), (100., None));
}
