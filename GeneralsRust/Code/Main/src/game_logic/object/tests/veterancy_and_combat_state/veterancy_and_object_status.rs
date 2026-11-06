use super::*;

#[test]
fn veterancy_increases_weapon_damage() {
    let mut object = make_test_object();
    object.gain_experience(
        60.0,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        &mut crate::game_logic::VeterancyFeedback::standalone(),
    ); // Veteran → +10% dmg
    let veteran_damage = object.weapon.as_ref().map(|w| w.damage).unwrap_or_default();
    assert!((veteran_damage - 110.0).abs() < 0.01);

    object.gain_experience(
        90.0,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        &mut crate::game_logic::VeterancyFeedback::standalone(),
    ); // Elite → +20% dmg (total)
    let elite_damage = object.weapon.as_ref().map(|w| w.damage).unwrap_or_default();
    assert!((elite_damage - 120.0).abs() < 0.01);
}
#[test]
fn level_up_sets_weaponset_and_weaponbonus_flags() {
    let mut object = make_test_object();
    assert!(!object.weapon_set_veteran && !object.weapon_bonus_veteran);
    object.gain_experience(
        60.0,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        &mut crate::game_logic::VeterancyFeedback::standalone(),
    );
    assert!(object.weapon_set_veteran);
    assert!(object.weapon_bonus_veteran);
    assert!(!object.weapon_set_elite && !object.weapon_set_hero);
    assert!(!object.weapon_bonus_elite && !object.weapon_bonus_hero);

    object.gain_experience(
        90.0,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        &mut crate::game_logic::VeterancyFeedback::standalone(),
    ); // 150 total → Elite
    assert!(!object.weapon_set_veteran && !object.weapon_bonus_veteran);
    assert!(object.weapon_set_elite);
    assert!(object.weapon_bonus_elite);
    assert!(!object.weapon_set_hero && !object.weapon_bonus_hero);

    object.gain_experience(
        150.0,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        &mut crate::game_logic::VeterancyFeedback::standalone(),
    ); // 300 total → Heroic
    assert!(!object.weapon_set_veteran && !object.weapon_set_elite);
    assert!(object.weapon_set_hero);
    assert!(object.weapon_bonus_hero);
    assert!(!object.weapon_bonus_veteran && !object.weapon_bonus_elite);
}

#[test]
fn weaponset_flag_and_salvage_unlock_unless_shared_across_sets() {
    // C++ WeaponSet::updateWeaponSet: set-flag / crate swaps drop a permanent
    // lock unless the incoming set has WeaponLockSharedAcrossSets.
    let mut object = make_test_object();
    object.secondary_weapon = Some(Weapon {
        damage: 30.0,
        ..Weapon::default()
    });
    assert!(object.set_weapon_lock(1, WeaponLockType::LockedPermanently));
    assert!(object.set_weapon_set_flag(0, true));
    assert!(!object.is_weapon_locked());
    assert_eq!(object.active_weapon_slot, 0);

    object.thing.template.weapon_lock_shared_across_sets = true;
    assert!(object.set_weapon_lock(1, WeaponLockType::LockedPermanently));
    object.set_weapon_set_mine_clearing_detail_at_frame(true, 0);
    assert!(object.weapon_set_mine_clearing_detail);
    assert!(object.is_weapon_locked());
    assert_eq!(object.weapon_lock_slot, 1);

    object.thing.template.weapon_lock_shared_across_sets = false;
    assert!(object.set_weapon_lock(1, WeaponLockType::LockedPermanently));
    object.apply_salvage_weapon_upgrade();
    assert!(!object.is_weapon_locked());
    assert_eq!(object.active_weapon_slot, 0);
}

#[test]
fn veterancy_preserves_health_ratio_when_max_health_changes() {
    let mut object = make_test_object();
    object.health.current = 50.0;
    object.health.maximum = 100.0;

    object.gain_experience(
        60.0,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        &mut crate::game_logic::VeterancyFeedback::standalone(),
    ); // Veteran → +20% HP
    assert!((object.health.maximum - 120.0).abs() < 0.01);
    assert!((object.health.current - 60.0).abs() < 0.01);
}

#[test]
fn level_up_scales_current_max_health_not_template() {
    // C++ ActiveBody PRESERVE_RATIO: Composite Armor / difficulty max must survive.
    let mut object = make_test_object();
    object.thing.template.max_health = 100.0;
    object.health.maximum = 250.0;
    object.health.current = 250.0;
    object.max_health = 250.0;
    object.gain_experience(
        60.0,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        &mut crate::game_logic::VeterancyFeedback::standalone(),
    );
    assert!(
        (object.health.maximum - 300.0).abs() < 0.01,
        "expected 250 * 1.2, got {}",
        object.health.maximum
    );
    assert!(
        (object.health.current - 300.0).abs() < 0.01,
        "PRESERVE_RATIO current, got {}",
        object.health.current
    );
}

pub(super) fn promotion_world() -> crate::game_logic::GameLogic {
    use crate::game_logic::promotion_definitions::PromotionDefinitions;
    let mut world = crate::game_logic::GameLogic::new();
    let mut definitions = PromotionDefinitions::from_ordered_sources(
        &[],
        &[("fixture-misc.ini", "MiscAudio\nUnitPromoted = UnitPromoted\nEnd\n".to_string())],
        &[("fixture-anims.ini", "Animation LevelGain\nNumberImages = 1\nImage = LevelGainImage\nAnimationMode = LOOP\nEnd\n".to_string())],
        &|name| name == "LevelGainImage",
    ).unwrap();
    definitions.rules.animation_name = "LevelGain".into();
    definitions.rules.display_seconds = 2.0;
    definitions.rules.rise_per_second = 15.0;
    world.admit_promotion_definitions(definitions).unwrap();
    world.add_player(crate::game_logic::Player::new(
        0,
        Team::USA,
        "Receiving",
        true,
    ));
    world
}
pub(super) fn assert_promotion_present(world: &crate::game_logic::GameLogic) {
    assert!(
        world
            .queued_audio_events
            .iter()
            .any(|event| event.event_type == "UnitPromoted"),
        "receiving UnitPromoted must queue on rank-up"
    );
    assert!(
        world
            .session_view
            .world_animations
            .births()
            .next()
            .is_some(),
        "receiving LevelGain must queue on rank-up"
    );
}
#[test]
fn level_up_gives_veterancy_upgrade_and_promote_fx() {
    let mut owner = promotion_world();
    let object = make_test_object();
    let id = object.id;
    owner.objects.insert(id, object);
    owner.award_experience(id, 60.0);
    assert!(
        owner.objects[&id]
            .has_upgrade_tag(crate::game_logic::host_unit_training::UPGRADE_VETERANCY_VETERAN)
    );
    assert_promotion_present(&owner);
}
#[test]
fn queue_veterancy_promote_fx_plays_for_local_visible_rank_up() {
    let mut owner = promotion_world();
    let mut object = make_test_object();
    object.owner_player_id = Some(0);
    object.status.stealthed = false;
    let id = object.id;
    owner.objects.insert(id, object);
    owner.award_experience(id, 60.0);
    assert_promotion_present(&owner);
}
#[test]
fn queue_veterancy_promote_fx_hides_undetected_stealthed_enemy() {
    let mut owner = promotion_world();
    let mut object = make_test_object();
    object.owner_player_id = Some(1);
    object.status.stealthed = true;
    object.status.detected = false;
    object.status.disguised = false;
    let id = object.id;
    owner.objects.insert(id, object);
    owner.award_experience(id, 60.0);
    assert!(
        owner.queued_audio_events.is_empty(),
        "undetected enemy must not leak UnitPromoted"
    );
    assert!(
        owner
            .session_view
            .world_animations
            .births()
            .next()
            .is_none(),
        "undetected enemy must not leak LevelGain"
    );
}
#[test]
fn queue_veterancy_promote_fx_skips_rider_swap_and_hijack_without_feedback() {
    let mut owner = promotion_world();
    let mut bike = make_test_object();
    bike.set_rider_change_veterancy_level(VeterancyLevel::Veteran);
    assert_eq!(bike.experience.level, VeterancyLevel::Veteran);
    let mut vehicle = make_test_object();
    let mut hijacker = make_test_object();
    hijacker.experience.level = VeterancyLevel::Elite;
    owner.with_veterancy_feedback(&mut vehicle, |object, audio, _feedback| {
        object.apply_hijacked_from(Some(&hijacker), audio);
    });
    assert_eq!(vehicle.experience.level, VeterancyLevel::Elite);
    assert!(
        owner.queued_audio_events.is_empty(),
        "quiet transfer queues no audio"
    );
    assert!(
        owner
            .session_view
            .world_animations
            .births()
            .next()
            .is_none(),
        "quiet transfer queues no animation"
    );
}
#[test]
fn receiving_promotion_output_survives_peer_reset_with_same_object_id() {
    let mut owner = promotion_world();
    let mut peer = promotion_world();
    let object = make_test_object();
    let id = object.id;
    owner.objects.insert(id, object);
    peer.objects.insert(id, make_test_object());
    owner.frame = 17;
    owner.award_experience(id, 60.0);
    let before = owner.queued_audio_events.len();
    assert_promotion_present(&owner);
    peer.award_experience(id, 300.0);
    peer.reset();
    assert_eq!(owner.queued_audio_events.len(), before);
    let births = owner
        .session_view
        .world_animations
        .births()
        .collect::<Vec<_>>();
    assert_eq!(births.len(), 1);
    assert_eq!(births[0].spawn_frame, 17);
    assert_eq!(owner.objects[&id].experience.level, VeterancyLevel::Veteran);
}

#[test]
fn ally_or_own_kill_awards_zero_experience() {
    let mut object = make_test_object();
    object.thing.template.experience_value = 40.0;
    object.thing.template.experience_values = [40.0, 40.0, 80.0, 120.0];
    assert!((object.kill_experience_value() - 40.0).abs() < 0.01);
    assert_eq!(object.kill_experience_value_from_killer(true), 0.0);
    object.status.under_construction = true;
    assert_eq!(object.kill_experience_value(), 0.0);
}

#[test]
fn kill_skill_point_value_uses_experience_value() {
    let mut object = make_test_object();
    object.thing.template.experience_value = 20.0;
    object.thing.template.experience_values = [20.0, 20.0, 40.0, 60.0];
    assert_eq!(object.kill_skill_point_value(), 20);
    object.experience.level = VeterancyLevel::Elite;
    assert_eq!(object.kill_skill_point_value(), 40);
    object.status.under_construction = true;
    assert_eq!(object.kill_skill_point_value(), 0);
}

#[test]
fn pilot_recrew_adds_pilot_levels_not_max() {
    let mut _ownership_output = crate::game_logic::host_owner_log::OwnershipEvents::default();

    let mut vehicle = make_test_object();
    vehicle.status.disabled_unmanned = true;
    vehicle.experience.level = VeterancyLevel::Elite;
    assert!(vehicle.apply_pilot_recrew(
        Team::USA,
        Some(0),
        VeterancyLevel::Veteran,
        &mut _ownership_output,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        &mut crate::game_logic::VeterancyFeedback::standalone(),
    ));
    assert_eq!(vehicle.experience.level, VeterancyLevel::Heroic);
}

#[test]
fn script_enabled_powered_set_live_disable_mask() {
    let mut obj = make_test_object();
    // can_move() gates on C++ mobile mobility (Infantry/Vehicle/Aircraft/
    // Worker KindOf); the shared fixture is kind-less, so give this unit a
    // mobile KindOf the way a real scripted vehicle would be authored.
    obj.thing.template.add_kind_of(KindOf::Vehicle);
    assert!(obj.can_move());
    assert!(obj.can_attack());
    obj.apply_object_panel_flag("Enabled", false);
    assert!(obj.is_disabled());
    assert!(obj.is_script_disabled());
    assert!(!obj.can_move());
    assert!(!obj.can_attack());
    obj.apply_object_panel_flag("Enabled", true);
    assert!(!obj.is_disabled());
    obj.apply_object_panel_flag("Powered", false);
    assert!(obj.is_script_underpowered());
    assert!(obj.is_disabled());
    assert!(!obj.can_move());
}

#[test]
fn script_indestructible_panel_flag_sets_body() {
    let mut obj = make_test_object();
    assert!(!obj.is_indestructible());
    obj.apply_object_panel_flag("Indestructible", true);
    assert!(obj.is_indestructible());
    obj.apply_object_panel_flag("Indestructible", false);
    assert!(!obj.is_indestructible());
}

#[test]
fn leftover_sa_player_targetable_sets_script_targetable() {
    let mut obj = make_test_object();
    assert!(!obj.is_script_targetable());
    obj.apply_object_panel_flag("Player Targetable", true);
    assert!(obj.script_targetable);
    assert!(obj.is_script_targetable());
    obj.apply_object_panel_flag("PlayerTargetable", false);
    assert!(!obj.script_targetable);
}

#[test]
fn disabled_status_does_not_issue_ai_idle() {
    let mut obj = make_test_object();
    obj.set_ai_state(
        crate::game_logic::AIState::Moving,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );
    obj.apply_disabled_emp(
        30,
        &mut crate::game_logic::HostHealthEvents::default(),
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );
    assert_eq!(obj.ai_state(), crate::game_logic::AIState::Moving);
}

#[test]
fn prone_and_cheer_timers_do_not_rewrite_ai_machine() {
    let mut obj = make_test_object();
    obj.set_ai_state(
        crate::game_logic::AIState::Moving,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );
    obj.go_prone(
        0.5,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );
    assert_eq!(obj.ai_state(), crate::game_logic::AIState::Moving);

    obj.set_ai_state(
        crate::game_logic::AIState::SpecialAbility,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );
    obj.cheer_timer = 0.1;
    obj.tick_timers(
        0.2,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );
    assert_eq!(obj.ai_state(), crate::game_logic::AIState::SpecialAbility);
}

#[test]
fn paralyzed_rejects_move_orders() {
    let mut obj = make_test_object();
    obj.apply_disabled_paralyzed(
        30,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );
    assert!(obj.is_disabled());
    assert!(!obj.can_move(), "C++ isMobile is false while paralyzed");
    assert!(!obj.can_attack());
}

#[test]
fn sold_and_under_construction_cannot_attack_or_fire() {
    let mut _attack_output = crate::game_logic::host_attack_log::AttackEvents::default();

    let mut shot_combat = crate::game_logic::combat::CombatSystem::new();
    // C++ Object::isAbleToAttack (Object.cpp:3171-3176).
    let mut obj = make_test_object();
    obj.weapon.as_mut().unwrap().last_fire_time = -10.0;
    assert!(obj.can_attack());
    assert!(obj.can_fire(0.0));
    assert!(obj.fire_at(
        ObjectId(2),
        0.0,
        0,
        &mut shot_combat,
        &mut _attack_output,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new())
    ));

    obj.status.under_construction = true;
    assert!(!obj.can_attack(), "UC Patriots/Stingers cannot acquire");
    assert!(!obj.can_fire(1.0), "UC cannot discharge");
    assert!(
        !obj.fire_at(
            ObjectId(2),
            1.0,
            0,
            &mut shot_combat,
            &mut _attack_output,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new())
        ),
        "UC fire_at must fail"
    );
    obj.status.under_construction = false;
    assert!(obj.can_attack());

    obj.status.sold = true;
    assert!(!obj.can_attack(), "sold defenses cannot acquire");
    assert!(!obj.can_fire(2.0), "sold cannot discharge");
    assert!(
        !obj.fire_at(
            ObjectId(2),
            2.0,
            0,
            &mut shot_combat,
            &mut _attack_output,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new())
        ),
        "sold fire_at must fail"
    );
}

#[test]
fn unmanned_enter_wipes_veterancy_and_auto_heal() {
    let mut _body_output = crate::game_logic::HostHealthEvents::default();

    let mut _ownership_output = crate::game_logic::host_owner_log::OwnershipEvents::default();

    let mut vehicle = make_test_object();
    vehicle.experience.level = VeterancyLevel::Elite;
    vehicle.experience.current = 200.0;
    vehicle.default_auto_heal = Some(crate::game_logic::host_heal::HostDefaultAutoHealData::new());
    assert!(
        vehicle
            .default_auto_heal
            .as_ref()
            .is_some_and(|h| h.upgrade_active)
    );
    vehicle.apply_kill_pilot_unmanned(
        &mut _body_output,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );
    assert_eq!(vehicle.experience.level, VeterancyLevel::Rookie);
    assert_eq!(vehicle.experience.current, 0.0);
    assert!(
        vehicle
            .default_auto_heal
            .as_ref()
            .is_some_and(|h| !h.upgrade_active)
    );
    assert!(vehicle.apply_pilot_recrew(
        Team::USA,
        Some(0),
        VeterancyLevel::Veteran,
        &mut _ownership_output,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        &mut crate::game_logic::VeterancyFeedback::standalone(),
    ));
    assert_eq!(vehicle.experience.level, VeterancyLevel::Veteran);
}

#[test]
fn battle_bus_second_life_is_held() {
    let mut obj = make_test_object();
    obj.status.disabled_held = true;
    assert!(obj.is_disabled());
    assert!(obj.is_physics_held());
    assert!(!obj.can_move());
    assert!(!obj.can_attack());
}

#[test]
fn hacked_spawn_site_idles_residual_slaves() {
    let mut _body_output = crate::game_logic::HostHealthEvents::default();

    let mut site = make_test_object();
    site.template_name = "GLAStingerSite".into();
    site.hive_slaves = crate::game_logic::host_base_defense::init_stinger_hive_slave_roster();
    site.hive_slave_count = 3;
    let _ = crate::game_logic::host_base_defense::order_hive_slaves_to_attack_target(
        &mut site.hive_slaves,
        99,
    );
    assert!(site.hive_slaves.iter().any(|s| s.ai_attacking));
    site.apply_disabled_hacked(
        60,
        &mut _body_output,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );
    assert!(site.is_hacked_disabled());
    assert!(site.hive_slaves.iter().all(|s| !s.ai_attacking));
}
