//! Actual private temporary Weapons use the receiving session at birth/load.
use super::*;
use crate::game_logic::{GameLogic, ObjectId, Player, Team};
use glam::Vec3;
const OBJECT: &str = "OwnedTemporaryBirth541";
const WEAPON: &str = "OwnedSameTemporaryWeapon541";
fn world(clip: u32) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    let source = format!(
        "Object {OBJECT}\n KindOf = VEHICLE\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = FireWeaponWhenDamagedBehavior ReactionTag\n StartsActive = Yes\n DamageAmount = 1\n ReactionWeaponPristine = {WEAPON}\n End\nEnd\n"
    );
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "owned-temporary-birth.ini".into(),
                source,
            )])
            .unwrap(),
        )
        .unwrap();
    let mut definitions = std::collections::HashMap::new();
    assert_eq!(
        crate::assets::ini_template_loader::parse_weapon_definitions_ini_into(
            &mut definitions,
            &format!(
                "Weapon {WEAPON}\n PrimaryDamage = 1\n PrimaryDamageRadius = 1\n DamageType = EXPLOSION\n RadiusDamageAffects = ENEMIES\n ProjectileObject = NONE\n ClipSize = {clip}\n AutoReloadsClip = No\n ClipReloadTime = 0\n DelayBetweenShots = 0\nEnd\n"
            ),
            crate::assets::ini_template_loader::WeaponDefinitionLoadType::Normal,
        ),
        1
    );
    // Receiving immutable construction facts differ despite identical names.
    // Do not register either definition with a compatibility WeaponStore.
    for definition in definitions.values_mut() {
        definition.shots_per_barrel = clip as i32;
        definition.suspend_fx_delay = clip;
        definition.min_target_pitch = 0.25;
        definition.max_target_pitch = 0.5;
    }
    world
        .admit_weapon_content_snapshot(
            definitions
                .into_values()
                .map(|rule| (rule.name.clone(), rule))
                .collect(),
            &[],
        )
        .unwrap();
    world.add_player(Player::new(0, Team::USA, "OwnedTemporary", true));
    world.set_current_frame(11);
    let id = world
        .create_object_for_player(OBJECT, 0, Vec3::ZERO)
        .unwrap();
    (world, id)
}
fn weapon(world: &GameLogic, id: ObjectId) -> &TemporaryWeaponRuntimeState {
    let object = world.host_object(id).unwrap();
    let metadata = &object.thing().template.fire_weapon_when_damaged_behaviors[0];
    object
        .temporary_weapon_runtime
        .damaged
        .iter()
        .find_map(|runtime| {
            runtime.weapon(TemporaryWeaponRuntimeKey {
                module_source_index: metadata.module_source_index,
                role: FireWeaponWhenDamagedWeaponRole::ReactionPristine,
            })
        })
        .expect("private weapon exists without compatibility registration")
}
fn assert_birth(world: &GameLogic, id: ObjectId, clip: u32) {
    let weapon = weapon(world, id);
    assert_eq!(weapon.weapon_template_name, WEAPON);
    assert_eq!(weapon.ammo_in_clip, clip);
    assert_eq!(weapon.num_shots_for_current_barrel, clip as i32);
    assert_eq!(weapon.suspend_fx_frame, 11 + clip);
    assert!(weapon.pitch_limited);
    assert!(!weapon.leech_weapon_range_active);
    assert_eq!(weapon.when_last_reload_started, 11);
}
#[test]
fn same_name_private_weapons_use_owned_defaults_at_interleaved_birth_and_restore() {
    let (mut a, a_id) = world(2);
    let (b, b_id) = world(5);
    assert_eq!(a_id, b_id);
    assert_birth(&a, a_id, 2);
    assert_birth(&b, b_id, 5);
    let next = a.create_object_for_player(OBJECT, 0, Vec3::X).unwrap();
    assert_birth(&a, next, 2);
    assert_birth(&b, b_id, 5);
    a.apply_owned_damage(
        a_id,
        1.0,
        None,
        crate::game_logic::combat::DamageType::Explosive,
        crate::game_logic::host_usa_pilot::HostDeathType::Normal,
        None,
        &Default::default(),
    )
    .unwrap();
    assert_eq!(weapon(&a, a_id).ammo_in_clip, 1);
    assert_eq!(a.host_object(a_id).unwrap().health.current, 99.0);
    let saved_runtime = a
        .host_object(a_id)
        .unwrap()
        .temporary_weapon_runtime
        .clone();
    let builder = crate::save_load::SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&a).unwrap();
    let (mut receiving, receiving_id) = world(2);
    assert_eq!(receiving_id, a_id);
    builder
        .restore_from_snapshot(&saved, &mut receiving)
        .unwrap();
    assert_eq!(
        receiving
            .host_object(a_id)
            .unwrap()
            .temporary_weapon_runtime,
        saved_runtime
    );
    assert_birth(&receiving, next, 2);
    receiving
        .apply_owned_damage(
            a_id,
            1.0,
            None,
            crate::game_logic::combat::DamageType::Explosive,
            crate::game_logic::host_usa_pilot::HostDeathType::Normal,
            None,
            &Default::default(),
        )
        .unwrap();
    assert_eq!(weapon(&receiving, a_id).ammo_in_clip, 0);
    assert_eq!(receiving.host_object(a_id).unwrap().health.current, 98.0);
    assert_eq!(weapon(&a, a_id).ammo_in_clip, 1);
    assert_birth(&b, b_id, 5);
}

#[test]
fn captured_missing_catalog_stays_absent_without_changing_existing_private_weapons() {
    let (mut owner, original) = world(2);
    let (peer, peer_id) = world(5);
    // Simulate an unavailable receiving definition source before another birth.
    // An already constructed private Weapon retains its own state.
    owner.weapon_definitions = None;
    let id = owner.create_object_for_player(OBJECT, 0, Vec3::X).unwrap();
    let object = owner.host_object(id).unwrap();
    let metadata = &object.thing().template.fire_weapon_when_damaged_behaviors[0];
    assert_eq!(object.temporary_weapon_runtime.damaged.len(), 1);
    assert!(
        object.temporary_weapon_runtime.damaged[0]
            .weapon(TemporaryWeaponRuntimeKey {
                module_source_index: metadata.module_source_index,
                role: FireWeaponWhenDamagedWeaponRole::ReactionPristine,
            })
            .is_none()
    );
    assert_birth(&owner, original, 2);
    assert_birth(&peer, peer_id, 5);
}
