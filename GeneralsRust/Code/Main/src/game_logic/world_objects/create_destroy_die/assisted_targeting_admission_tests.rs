//! hq-2qedt.30: Patriot assist slot/clip bind from the admitted
//! AssistedTargetingUpdate module (AssistedTargetingUpdate.cpp:74-75,
//! AssistedTargetingUpdate.h:26-27 defaults), not hardcoded clip 4 / slot 0.
use super::*;
use crate::game_logic::host_base_defense::HostAssistedTargetingData;
use crate::game_logic::{Player, Team};
use glam::Vec3;

const BODY: &str = " KindOf = STRUCTURE SELECTABLE FS_BASE_DEFENSE\n Body = ActiveBody BodyTag\n MaxHealth = 600\n InitialHealth = 600\n End\n";

fn world(source: &str) -> GameLogic {
    let mut world = GameLogic::new();
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "assisted-targeting-admission.ini".into(),
                source.to_string(),
            )])
            .unwrap(),
        )
        .unwrap();
    world.add_player(Player::new(0, Team::USA, "PatriotOwner", true));
    world.add_player(Player::new(1, Team::GLA, "Victim", true));
    for (player, other) in [(0, 1), (1, 0)] {
        world
            .get_player_mut(player)
            .unwrap()
            .set_map_relationship(other, gamelogic::common::Relationship::Enemies);
    }
    world
}

fn patriot(module: &str) -> String {
    format!("Object AmericaPatriotBattery\n{BODY}{module}End\nObject AuthoredVictim\n KindOf = VEHICLE\n Body = ActiveBody BodyTag\n MaxHealth = 500\n InitialHealth = 500\n End\nEnd\n")
}

#[test]
fn authored_assist_module_binds_clip_and_slot() {
    let mut w = world(&patriot(
        "Behavior = AssistedTargetingUpdate AssistTag\n AssistingClipSize = 2\n AssistingWeaponSlot = SECONDARY\nEnd\n",
    ));
    let id = w.create_object_for_player("AmericaPatriotBattery", 0, Vec3::ZERO).unwrap();
    assert_eq!(
        w.host_object(id).unwrap().assisted_targeting,
        Some(HostAssistedTargetingData { clip_size: 2, weapon_slot: 1 })
    );
}

#[test]
fn authored_assist_module_defaults_match_cpp() {
    let mut w = world(&patriot("Behavior = AssistedTargetingUpdate AssistTag\nEnd\n"));
    let id = w.create_object_for_player("AmericaPatriotBattery", 0, Vec3::ZERO).unwrap();
    assert_eq!(
        w.host_object(id).unwrap().assisted_targeting,
        Some(HostAssistedTargetingData { clip_size: 1, weapon_slot: 0 })
    );
}

#[test]
fn patriot_name_without_assist_module_stays_unbound() {
    let mut w = world(&patriot(""));
    let id = w.create_object_for_player("AmericaPatriotBattery", 0, Vec3::ZERO).unwrap();
    assert!(w.host_object(id).unwrap().assisted_targeting.is_none());
}

#[test]
fn pending_assist_clip_comes_from_bound_module_not_retail_four() {
    let mut w = world(&patriot(
        "Behavior = AssistedTargetingUpdate AssistTag\n AssistingClipSize = 2\n AssistingWeaponSlot = SECONDARY\nEnd\n",
    ));
    let id = w.create_object_for_player("AmericaPatriotBattery", 0, Vec3::ZERO).unwrap();
    let assist = w.host_object(id).unwrap().assisted_targeting.expect("bound");
    let pending = crate::game_logic::host_base_defense::PendingPatriotAssist::new(
        id, id, id, 1, "AmericaPatriotBattery", assist, false,
    );
    assert_eq!(pending.shots_remaining, 2);
    assert_eq!(pending.weapon_slot, 1);
}

/// hq-2qedt.31: AssistedTargetingUpdate.cpp:74-75 — assistAttack locks the
/// authored slot LOCKED_TEMPORARILY then aiAttackObject(victim, clipSize).
/// No instant clip damage is dealt by the assist path itself.
#[test]
fn pending_assist_locks_authored_slot_and_attacks_with_clip() {
    use crate::game_logic::{Weapon, WeaponLockType};
    let mut w = world(&patriot(
        "Behavior = AssistedTargetingUpdate AssistTag\n AssistingClipSize = 3\n AssistingWeaponSlot = SECONDARY\nEnd\n",
    ));
    let id = w.create_object_for_player("AmericaPatriotBattery", 0, Vec3::ZERO).unwrap();
    let victim = w
        .create_object_for_player("AuthoredVictim", 1, Vec3::new(40.0, 0.0, 0.0))
        .unwrap();
    {
        let o = w.objects.get_mut(&id).unwrap();
        crate::fixture_content::arm_with_slot_stats(o, [Some(Weapon { damage: 10.0, range: 200.0, ..Weapon::default() }), Some(Weapon { damage: 30.0, range: 200.0, ..Weapon::default() }), None]);
    }
    let assist = w.host_object(id).unwrap().assisted_targeting.expect("bound");
    let hp_before = w.host_object(victim).unwrap().health.current;
    w.frame = 1;
    w.pending_patriot_assists.push(
        crate::game_logic::host_base_defense::PendingPatriotAssist::new(
            id, victim, id, 1, "AmericaPatriotBattery", assist, false,
        ),
    );
    w.update_pending_patriot_assists();
    let o = w.host_object(id).unwrap();
    assert_eq!(o.weapon_lock_type, WeaponLockType::LockedTemporarily);
    assert_eq!(o.weapon_lock_slot, 1);
    assert_eq!(o.max_shots_to_fire, 3);
    assert_eq!(o.target, Some(victim));
    assert_eq!(w.host_object(victim).unwrap().health.current, hp_before);
    assert!(w.pending_patriot_assists.is_empty(), "clip handed to AI attack");
}

/// hq-2qedt.32.2: retail FactionBuilding.ini AmericaPatriotBattery WeaponSet is
/// PRIMARY PatriotMissileWeapon / SECONDARY PatriotMissileAssistWeapon /
/// TERTIARY PatriotMissileWeaponAir (AssistingWeaponSlot = SECONDARY).
/// Weapon.ini PatriotMissileAssistWeapon: dmg 25, range 450, ClipSize 4, AntiGround.
#[test]
fn patriot_runtime_slots_match_retail_weapon_set() {
    use crate::game_logic::host_base_defense::{
        PATRIOT_AIR_DAMAGE, PATRIOT_AIR_RANGE, PATRIOT_ASSIST_DAMAGE, PATRIOT_ASSIST_RANGE,
    };
    crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
    let mut w = world(&patriot(
        "WeaponSet\n Conditions = None\n Weapon = PRIMARY PatriotMissileWeapon\n Weapon = SECONDARY PatriotMissileAssistWeapon\n Weapon = TERTIARY PatriotMissileWeaponAir\nEnd\nBehavior = AssistedTargetingUpdate AssistTag\n AssistingClipSize = 4\n AssistingWeaponSlot = SECONDARY\nEnd\n",
    ));
    let id = w.create_object_for_player("AmericaPatriotBattery", 0, Vec3::ZERO).unwrap();
    let p = w.host_object(id).unwrap();
    let assist = p.weapon_slot(1).expect("SECONDARY = PatriotMissileAssistWeapon");
    assert!((assist.damage - PATRIOT_ASSIST_DAMAGE).abs() < 0.01);
    assert!((assist.range - PATRIOT_ASSIST_RANGE).abs() < 0.01);
    assert_eq!(assist.clip_size, 4);
    assert!(assist.can_target_ground, "assist weapon is AntiGround");
    assert!(assist.can_target_air);
    let air = p.weapon_slot(2).expect("TERTIARY = PatriotMissileWeaponAir");
    assert!((air.damage - PATRIOT_AIR_DAMAGE).abs() < 0.01);
    assert!((air.range - PATRIOT_AIR_RANGE).abs() < 0.01);
    assert!(air.can_target_air && !air.can_target_ground);
}

/// hq-2qedt.24: AssistedTargetingUpdate::assistAttack (AssistedTargetingUpdate.cpp:66-75)
/// has no assistant-to-victim weapon-range gate: it always locks the slot and
/// issues aiAttackObject(victim, clipSize); range is the attack machine's job.
#[test]
fn pending_assist_issues_attack_even_when_victim_beyond_assist_weapon_range() {
    use crate::game_logic::{Weapon, WeaponLockType};
    let mut w = world(&patriot(
        "Behavior = AssistedTargetingUpdate AssistTag\n AssistingClipSize = 4\n AssistingWeaponSlot = SECONDARY\nEnd\n",
    ));
    let id = w.create_object_for_player("AmericaPatriotBattery", 0, Vec3::ZERO).unwrap();
    let victim = w
        .create_object_for_player("AuthoredVictim", 1, Vec3::new(500.0, 0.0, 0.0))
        .unwrap();
    {
        let o = w.objects.get_mut(&id).unwrap();
        crate::fixture_content::arm_with_slot_stats(o, [Some(Weapon { damage: 10.0, range: 200.0, ..Weapon::default() }), Some(Weapon { damage: 25.0, range: 450.0, ..Weapon::default() }), None]);
    }
    let assist = w.host_object(id).unwrap().assisted_targeting.expect("bound");
    w.frame = 1;
    w.pending_patriot_assists.push(
        crate::game_logic::host_base_defense::PendingPatriotAssist::new(
            id, victim, id, 1, "AmericaPatriotBattery", assist, false,
        ),
    );
    w.update_pending_patriot_assists();
    let o = w.host_object(id).unwrap();
    assert_eq!(o.weapon_lock_type, WeaponLockType::LockedTemporarily);
    assert_eq!(o.weapon_lock_slot, 1);
    assert_eq!(o.max_shots_to_fire, 4);
    assert_eq!(o.target, Some(victim));
}

/// hq-2qedt.136: C++ binds PRIMARY/SECONDARY/TERTIARY from the template
/// WeaponSet at create (WeaponSet::updateWeaponSet), not a template-name host
/// table. A Patriot-named template authoring different weapons keeps them.
#[test]
fn patriot_create_weapons_come_from_authored_weapon_set() {
    use crate::game_logic::host_base_defense::{
        PATRIOT_AIR_DAMAGE, PATRIOT_ASSIST_DAMAGE,
    };
    crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
    let mut w = world(&patriot(
        "WeaponSet\n Conditions = None\n Weapon = PRIMARY HumveeGun\n Weapon = SECONDARY HumveeGun\n Weapon = TERTIARY HumveeGun\nEnd\nBehavior = AssistedTargetingUpdate AssistTag\n AssistingClipSize = 4\n AssistingWeaponSlot = SECONDARY\nEnd\n",
    ));
    let id = w.create_object_for_player("AmericaPatriotBattery", 0, Vec3::ZERO).unwrap();
    let p = w.host_object(id).unwrap();
    let prim = p.weapon_slot(0).expect("authored PRIMARY").clone();
    let sec = p.weapon_slot(1).expect("authored SECONDARY").clone();
    let ter = p.weapon_slot(2).expect("authored TERTIARY").clone();
    assert!(
        (sec.damage - PATRIOT_ASSIST_DAMAGE).abs() > 0.01,
        "authored SECONDARY overwritten by patriot_assist_weapon_for_template peel: {sec:?}"
    );
    assert!(
        (ter.damage - PATRIOT_AIR_DAMAGE).abs() > 0.01,
        "authored TERTIARY overwritten by patriot_air_weapon_for_template peel: {ter:?}"
    );
    assert_eq!(prim.damage, sec.damage, "PRIMARY/SECONDARY both authored HumveeGun");
    assert_eq!(prim.damage, ter.damage, "PRIMARY/TERTIARY both authored HumveeGun");
}

/// hq-2qedt.167: GLA Rebel PRIMARY binds from the authored WeaponSet at
/// create (WeaponSet::updateWeaponSet), not the `rebel_weapon` name peel.
#[test]
fn rebel_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::weapon_bootstrap::{GLA_REBEL_PRIMARY_WEAPON, HUMVEE_PRIMARY_WEAPON};
    crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
    let mut w = world(&format!(
        "Object GLAInfantryRebel\n KindOf = INFANTRY SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 120\n InitialHealth = 120\n End\nWeaponSet\n Conditions = None\n Weapon = PRIMARY {HUMVEE_PRIMARY_WEAPON}\nEnd\nEnd\n"
    ));
    let id = w.create_object_for_player("GLAInfantryRebel", 1, Vec3::ZERO).unwrap();
    let prim = w.host_object(id).unwrap().weapon_slot(0).expect("authored PRIMARY").clone();
    let peel = crate::game_logic::host_gla_rebel::rebel_weapon(false);
    assert!(
        (prim.damage - peel.damage).abs() > 0.01 || prim.clip_size != peel.clip_size,
        "authored PRIMARY ({HUMVEE_PRIMARY_WEAPON}) overwritten by rebel_weapon peel ({GLA_REBEL_PRIMARY_WEAPON}): {prim:?}"
    );
}

/// hq-2qedt.168: USA Ranger PRIMARY comes only from the authored WeaponSet
/// (WeaponSet::updateWeaponSet); no `ranger_rifle_weapon` name peel at create.
#[test]
fn ranger_create_primary_has_no_name_peel_without_authored_weapon_set() {
    crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
    let mut w = world(
        "Object AmericaInfantryRanger\n KindOf = INFANTRY SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 180\n InitialHealth = 180\n End\nEnd\n",
    );
    let id = w.create_object_for_player("AmericaInfantryRanger", 1, Vec3::ZERO).unwrap();
    let prim = w.host_object(id).unwrap().weapon_slot(0).cloned();
    let peel = crate::game_logic::host_ranger::ranger_rifle_weapon();
    let is_peel = prim.as_ref().is_some_and(|p| {
        p.projectile_speed == peel.projectile_speed && p.clip_reload_time == peel.clip_reload_time
    });
    assert!(!is_peel, "Ranger PRIMARY seeded by ranger_rifle_weapon name peel: {prim:?}");
}

/// hq-2qedt.169: GLA Terrorist PRIMARY comes only from the authored WeaponSet
/// (WeaponSet::updateWeaponSet); no `terrorist_suicide_weapon_for_profile` name peel at create.
#[test]
fn terrorist_create_primary_has_no_suicide_name_peel_without_authored_weapon_set() {
    crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
    let mut w = world(
        "Object GLAInfantryTerrorist\n KindOf = INFANTRY SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n",
    );
    let id = w.create_object_for_player("GLAInfantryTerrorist", 1, Vec3::ZERO).unwrap();
    let prim = w.host_object(id).unwrap().weapon_slot(0).cloned();
    let is_peel = prim.as_ref().is_some_and(|p| p.projectile_speed == 999_999.0 && p.ammo == Some(1));
    assert!(!is_peel, "Terrorist PRIMARY seeded by terrorist_suicide_weapon_for_profile peel: {prim:?}");
}

/// hq-2qedt.192: an unauthored Ranger gets no PRIMARY at create; C++
/// WeaponSet::updateWeaponSet leaves the slot empty with no authored WeaponSet.
#[test]
fn unauthored_ranger_create_primary_is_empty_as_cpp() {
    crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
    let mut w = world(
        "Object AmericaInfantryRanger\n KindOf = INFANTRY SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 180\n InitialHealth = 180\n End\nEnd\n",
    );
    let id = w.create_object_for_player("AmericaInfantryRanger", 1, Vec3::ZERO).unwrap();
    let o = w.host_object(id).unwrap();
    assert!(
        o.weapon_slot(0).is_none(),
        "unauthored Ranger PRIMARY not empty: {:?}",
        o.weapon_slot(0)
    );
    assert!(
        o.weapon_slot(1).is_none(),
        "unauthored Ranger SECONDARY not empty: {:?}",
        o.weapon_slot(1)
    );
}
