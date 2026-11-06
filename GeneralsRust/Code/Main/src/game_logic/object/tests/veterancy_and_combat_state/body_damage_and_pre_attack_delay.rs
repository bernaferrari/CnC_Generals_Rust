use super::*;

#[test]
fn effective_max_lift_uses_damaged_locomotor() {
    use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("LIFT_V");
    tpl.add_kind_of(KindOf::Aircraft);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(100.0);
    logic.templates.insert("LIFT_V".to_string(), tpl);
    let id = logic
        .create_object("LIFT_V", Team::USA, Vec3::new(0.0, 50.0, 0.0))
        .unwrap();
    {
        let o = logic.host_object_mut(id).unwrap();
        let mut definition = game_engine::common::ini::ini_locomotor::LocomotorTemplate::new(
            "TypedDamageLift".into(),
        );
        definition
            .surfaces
            .add_surface(game_engine::common::ini::ini_locomotor::LocomotorSurfaceTypeMask::AIR);
        definition.appearance = game_engine::common::ini::ini_locomotor::LocomotorAppearance::Hover;
        definition.lift = 8.;
        definition.lift_damaged = 3.;
        let binding = crate::game_logic::locomotor_bootstrap::host_locomotor_binding_from_template(
            &definition,
        )
        .unwrap();
        crate::game_logic::locomotor_bootstrap::apply_host_locomotor_binding(o, &binding);
        o.health.current = 100.0;
        o.health.maximum = 100.0;
        o.refresh_model_condition_bits();
        assert_eq!(o.body_damage_state, HostBodyDamageType::Pristine);
        assert!((o.effective_max_lift() - 8.0).abs() < 0.01);
        o.internal_change_health(10.0 - o.health.current);
        assert_eq!(o.body_damage_state, HostBodyDamageType::ReallyDamaged);
        assert!(
            (o.effective_max_lift() - 3.0).abs() < 0.01,
            "really damaged uses max_lift_damaged"
        );
    }
}

#[test]
fn body_damage_sets_model_condition_bits() {
    let mut health_events = crate::game_logic::HostHealthEvents::default();

    use crate::game_logic::host_enum_table_residual::{
        HostBodyDamageType, MC_BIT_DAMAGED, MC_BIT_DYING, MC_BIT_REALLYDAMAGED, MC_BIT_RUBBLE,
        host_model_condition_has,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut tmpl = ThingTemplate::new("McBits");
    tmpl.set_health(100.0);
    tmpl.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(tmpl, ObjectId(90), Team::USA);
    o.refresh_model_condition_bits();
    assert_eq!(o.body_damage_state, HostBodyDamageType::Pristine);
    assert!(!host_model_condition_has(
        o.model_condition_bits,
        MC_BIT_DAMAGED
    ));

    o.internal_change_health(40.0 - o.health.current); // between 0.25 and 0.5
    assert_eq!(o.body_damage_state, HostBodyDamageType::Damaged);
    assert!(host_model_condition_has(
        o.model_condition_bits,
        MC_BIT_DAMAGED
    ));
    assert!(!host_model_condition_has(
        o.model_condition_bits,
        MC_BIT_REALLYDAMAGED
    ));

    o.internal_change_health(10.0 - o.health.current);
    assert_eq!(o.body_damage_state, HostBodyDamageType::ReallyDamaged);
    assert!(host_model_condition_has(
        o.model_condition_bits,
        MC_BIT_REALLYDAMAGED
    ));

    o.take_damage(9999.0, 0, &mut health_events);
    assert!(o.status.destroyed);
    assert_eq!(o.body_damage_state, HostBodyDamageType::Rubble);
    assert!(host_model_condition_has(
        o.model_condition_bits,
        MC_BIT_RUBBLE
    ));
    assert!(host_model_condition_has(
        o.model_condition_bits,
        MC_BIT_DYING
    ));
}

#[test]
fn body_damage_threshold_cpp_surface() {
    use crate::game_logic::host_enum_table_residual::{
        HOST_UNIT_DAMAGED_THRESH, HOST_UNIT_REALLY_DAMAGED_THRESH, HostBodyDamageType,
        host_calc_body_damage_state,
    };
    // C++ ActiveBody.cpp:88 + GameData.ini UnitDamagedThreshold 0.7 / 0.35.
    assert!((HOST_UNIT_DAMAGED_THRESH - 0.7).abs() < 1e-6);
    assert!((HOST_UNIT_REALLY_DAMAGED_THRESH - 0.35).abs() < 1e-6);
    assert_eq!(
        host_calc_body_damage_state(100.0, 100.0),
        HostBodyDamageType::Pristine
    );
    assert_eq!(
        host_calc_body_damage_state(50.0, 100.0),
        HostBodyDamageType::Damaged
    );
    assert_eq!(
        host_calc_body_damage_state(25.0, 100.0),
        HostBodyDamageType::ReallyDamaged
    );
    assert_eq!(
        host_calc_body_damage_state(0.0, 100.0),
        HostBodyDamageType::Rubble
    );
}

#[test]
fn leech_range_waives_max_range_after_activate() {
    let mut tmpl = ThingTemplate::new("GLAInfantryTerrorist");
    tmpl.primary_weapon_name = Some("GLAInfantryTerrorist".into());
    let mut atk = Object::new(tmpl, ObjectId(1), Team::GLA);
    atk.set_position(glam::Vec3::ZERO);
    atk.weapon = Some(Weapon {
        damage: 100.0,
        range: 20.0,
        min_range: 0.0,
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: 0.0,
        ..Weapon::default()
    });

    let mut tgt = Object::new(
        ThingTemplate::new("AmericaTankCrusader"),
        ObjectId(2),
        Team::USA,
    );
    tgt.set_position(glam::Vec3::new(100.0, 0.0, 0.0)); // out of 20 range
    tgt.thing.template.add_kind_of(KindOf::Vehicle);
    tgt.thing.template.add_kind_of(KindOf::Attackable);

    // Before leech: out of range.
    assert!(!atk.can_target_with_slot(&tgt, atk.weapon.as_ref().unwrap(), Some(0)));

    // Activate leech (as if pre-fire / fire occurred in range).
    atk.activate_leech_range_for_slot(0);
    assert!(atk.leech_range_active_primary);
    assert!(atk.can_target_with_slot(&tgt, atk.weapon.as_ref().unwrap(), Some(0)));

    // stop_attack clears.
    atk.stop_attack(&mut crate::game_logic::ObjectAudio::record(&mut Vec::new()));
    assert!(!atk.leech_range_active_primary);
    assert!(!atk.can_target_with_slot(&tgt, atk.weapon.as_ref().unwrap(), Some(0)));
}

#[test]
fn acceptable_aim_delta_blocks_then_allows_after_turn() {
    let mut tmpl = ThingTemplate::new("AmericaTankCrusader");
    tmpl.primary_weapon_name = Some("AmericaTankCrusaderGun".into());
    let mut atk = Object::new(tmpl, ObjectId(1), Team::USA);
    atk.set_position(glam::Vec3::ZERO);
    atk.set_orientation(0.0); // face +X residual (movement convention)
    atk.weapon = Some(Weapon {
        damage: 10.0,
        range: 200.0,
        ..Weapon::default()
    });
    let target = glam::Vec3::new(0.0, 0.0, 50.0); // off to +Z (~90°)
    let aim = atk.aim_delta_for_slot(0);
    let rel = atk.relative_angle_2d_to(target);
    // 20° aim residual should NOT be aimed at 90° offset.
    assert!(
        !atk.is_aimed_at_position(target, 0),
        "unexpectedly aimed: aim_delta={aim} rel={rel} ori={}",
        atk.get_orientation()
    );
    // Turn in steps until aimed.
    let mut aimed = false;
    for _ in 0..20 {
        if atk.turn_toward_position(target, 0, 0.2) {
            aimed = true;
            break;
        }
    }
    assert!(
        aimed,
        "should aim after turns, ori={}",
        atk.get_orientation()
    );
    assert!(atk.is_aimed_at_position(target, 0));
}

#[test]
fn omni_aim_delta_always_aimed() {
    let mut tmpl = ThingTemplate::new("AmericaSentryDrone");
    tmpl.primary_weapon_name = Some("AmericaSentryDroneGun".into());
    let mut atk = Object::new(tmpl, ObjectId(3), Team::USA);
    atk.set_position(glam::Vec3::ZERO);
    atk.set_orientation(0.0);
    let target = glam::Vec3::new(-40.0, 0.0, 10.0);
    assert!(atk.is_aimed_at_position(target, 0));
}

#[test]
fn pre_attack_type_per_shot_delays_every_discharge() {
    let mut _attack_output = crate::game_logic::host_attack_log::AttackEvents::default();

    let mut shot_combat = crate::game_logic::combat::CombatSystem::new();
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    use glam::Vec3;
    let mut tmpl = ThingTemplate::new("Gattling");
    tmpl.primary_weapon_name = Some("AmericaGattlingTankGun".into());
    tmpl.add_kind_of(KindOf::Vehicle);
    tmpl.add_kind_of(KindOf::Attackable);
    let mut atk = Object::new(tmpl, ObjectId(1), Team::USA);
    atk.set_position(Vec3::ZERO);
    atk.weapon = Some(Weapon {
        damage: 5.0,
        range: 100.0,
        reload_time: 0.0,
        last_fire_time: -100.0,
        pre_attack_delay: 0.5,
        ..Weapon::default()
    });
    let tgt = ObjectId(9);
    // First wind-up
    assert!(!atk.fire_at(
        tgt,
        10.0,
        0,
        &mut shot_combat,
        &mut _attack_output,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new())
    ));
    assert!((atk.pre_attack_ready_at - 10.5).abs() < 1e-4);
    // Still winding
    assert!(!atk.fire_at(
        tgt,
        10.2,
        0,
        &mut shot_combat,
        &mut _attack_output,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new())
    ));
    // Fire after delay
    assert!(atk.fire_at(
        tgt,
        10.5,
        0,
        &mut shot_combat,
        &mut _attack_output,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new())
    ));
    assert_eq!(atk.consecutive_shots_at_target, 1);
    // PER_SHOT: next shot needs a new delay even vs same target
    assert!(!atk.fire_at(
        tgt,
        10.5,
        0,
        &mut shot_combat,
        &mut _attack_output,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new())
    ));
    assert!(atk.pre_attack_ready_at > 10.5);
    assert!(!atk.fire_at(
        tgt,
        10.7,
        0,
        &mut shot_combat,
        &mut _attack_output,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new())
    ));
    assert!(atk.fire_at(
        tgt,
        11.0,
        0,
        &mut shot_combat,
        &mut _attack_output,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new())
    ));
    assert_eq!(atk.consecutive_shots_at_target, 2);
}

#[test]
fn pre_attack_type_per_attack_delays_once_per_target() {
    let mut _attack_output = crate::game_logic::host_attack_log::AttackEvents::default();

    let mut shot_combat = crate::game_logic::combat::CombatSystem::new();
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    use glam::Vec3;
    let mut tmpl = ThingTemplate::new("Ranger");
    tmpl.primary_weapon_name = Some("AmericaRangerMachineGun".into());
    tmpl.add_kind_of(KindOf::Infantry);
    tmpl.add_kind_of(KindOf::Attackable);
    let mut atk = Object::new(tmpl, ObjectId(1), Team::USA);
    atk.set_position(Vec3::ZERO);
    atk.weapon = Some(Weapon {
        damage: 5.0,
        range: 100.0,
        reload_time: 0.0,
        last_fire_time: -100.0,
        pre_attack_delay: 1.0,
        ammo: Some(5),
        clip_size: 5,
        ..Weapon::default()
    });
    let tgt = ObjectId(9);
    assert!(!atk.fire_at(
        tgt,
        5.0,
        0,
        &mut shot_combat,
        &mut _attack_output,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new())
    )); // wind-up
    assert!(atk.fire_at(
        tgt,
        6.0,
        0,
        &mut shot_combat,
        &mut _attack_output,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new())
    )); // fire
    // Same target: no second wind-up
    assert!(atk.fire_at(
        tgt,
        6.0,
        0,
        &mut shot_combat,
        &mut _attack_output,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new())
    ));
    assert_eq!(atk.consecutive_shots_at_target, 2);
    // New target: delay again
    let tgt2 = ObjectId(10);
    assert!(!atk.fire_at(
        tgt2,
        6.0,
        0,
        &mut shot_combat,
        &mut _attack_output,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new())
    ));
    assert!(atk.fire_at(
        tgt2,
        7.0,
        0,
        &mut shot_combat,
        &mut _attack_output,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new())
    ));
}

#[test]
fn pre_attack_type_per_clip_delays_on_full_clip_only() {
    let mut _attack_output = crate::game_logic::host_attack_log::AttackEvents::default();

    let mut shot_combat = crate::game_logic::combat::CombatSystem::new();
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    use glam::Vec3;
    let mut tmpl = ThingTemplate::new("Scud");
    // Seed-only name (not in WeaponStore) so PreAttackType peels to PER_CLIP.
    tmpl.primary_weapon_name = Some("HostTestScudStormClipWeapon".into());
    tmpl.add_kind_of(KindOf::Structure);
    tmpl.add_kind_of(KindOf::Attackable);
    let mut atk = Object::new(tmpl, ObjectId(1), Team::GLA);
    atk.set_position(Vec3::ZERO);
    atk.weapon = Some(Weapon {
        damage: 50.0,
        range: 300.0,
        reload_time: 0.0,
        last_fire_time: -100.0,
        pre_attack_delay: 2.0,
        ammo: Some(3),
        clip_size: 3,
        // Retail ScudStormWeapon ClipReloadTime = 10000 (Weapon.ini:130370):
        // an emptied AutoReloadsClip clip holds fire (C++ Weapon.cpp:1893
        // m_whenWeCanFireAgain = start + ClipReloadTime) and the host refill
        // completes only after it elapses, so ammo reads 0 on the same tick.
        clip_reload_time: 10.0,
        ..Weapon::default()
    });
    let tgt = ObjectId(9);
    // Full clip → delay
    assert!(!atk.fire_at(
        tgt,
        1.0,
        0,
        &mut shot_combat,
        &mut _attack_output,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new())
    ));
    assert!(atk.fire_at(
        tgt,
        3.0,
        0,
        &mut shot_combat,
        &mut _attack_output,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new())
    ));
    assert_eq!(atk.weapon.as_ref().unwrap().ammo, Some(2));
    // Mid-clip → no delay
    assert!(atk.fire_at(
        tgt,
        3.0,
        0,
        &mut shot_combat,
        &mut _attack_output,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new())
    ));
    assert_eq!(atk.weapon.as_ref().unwrap().ammo, Some(1));
    assert!(atk.fire_at(
        tgt,
        3.0,
        0,
        &mut shot_combat,
        &mut _attack_output,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new())
    ));
    assert_eq!(atk.weapon.as_ref().unwrap().ammo, Some(0));
}

#[test]
fn successive_experience_awards_are_owned_before_promotion_callbacks() {
    let mut first = make_test_object();
    let mut peer = make_test_object();
    assert_eq!(first.id, peer.id);
    first.gain_experience(
        60.0,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        &mut crate::game_logic::VeterancyFeedback::standalone(),
    );
    assert_eq!(first.experience.current, 60.0);
    assert_eq!(first.experience.level, VeterancyLevel::Veteran);
    let first_health = first.health.clone();
    peer.gain_experience(
        300.0,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        &mut crate::game_logic::VeterancyFeedback::standalone(),
    );
    assert_eq!(peer.experience.current, 300.0);
    assert_eq!(peer.experience.level, VeterancyLevel::Heroic);
    assert_eq!(first.experience.current, 60.0);
    assert_eq!(first.health.current, first_health.current);
    first.gain_experience(
        90.0,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        &mut crate::game_logic::VeterancyFeedback::standalone(),
    );
    assert_eq!(first.experience.current, 150.0);
    assert_eq!(first.experience.level, VeterancyLevel::Elite);
    assert_eq!(peer.experience.current, 300.0);
    assert_eq!(first.weapon.as_ref().unwrap().damage, 120.0);
}

// Append to the proposed veterancy_and_combat_state.rs; uses promotion_world().
#[test]
fn authored_body_promotion_precedes_bonus_updates_and_ignores_ui_hide() {
    let mut owner = promotion_world();
    owner.set_draw_icon_ui(false);
    let mut object = make_test_object();
    let id = object.id;
    object.health.maximum = 250.0;
    object.health.current = 125.0;
    object.max_health = 250.0;
    object.thing.template.sound_promoted[0] = Some("AuthoredVeteran".into());
    owner.objects.insert(id, object);
    let mut observed = Vec::new();
    owner
        .with_object_veterancy_feedback(id, |object, _queue, feedback| {
            let mut emit = |object: &mut Object, request: crate::game_logic::AudioEventRequest| {
                observed.push((
                    request.event_type,
                    object.health.maximum,
                    object.health.current,
                    object.armor_set_veteran,
                    object.experience.level,
                ));
            };
            object.gain_experience(
                60.0,
                &mut crate::game_logic::ObjectAudio::callback(&mut emit),
                feedback,
            );
        })
        .unwrap();
    assert_eq!(
        observed.len(),
        1,
        "hidden UI still emits authored Body cue, not MiscAudio"
    );
    assert_eq!(observed[0].0, "AuthoredVeteran");
    assert_eq!((observed[0].1, observed[0].2), (250.0, 125.0));
    assert!(
        !observed[0].3,
        "Body sound callback observes old armor bonus"
    );
    assert_eq!(
        observed[0].4,
        VeterancyLevel::Veteran,
        "XP level changes before callback"
    );
    assert!((owner.objects[&id].health.maximum - 300.0).abs() < 0.01);
    assert!(owner.objects[&id].armor_set_veteran);
    assert!(
        owner
            .session_view
            .world_animations
            .births()
            .next()
            .is_none()
    );
}
#[test]
fn local_player_two_stealth_visibility_does_not_assume_player_zero() {
    let mut owner = promotion_world();
    assert!(owner.select_local_player(None));
    owner.add_player(crate::game_logic::Player::new(
        2,
        Team::USA,
        "ActualLocal",
        true,
    ));
    let mut object = make_test_object();
    let id = object.id;
    object.owner_player_id = Some(2);
    object.status.stealthed = true;
    object.status.detected = false;
    object.status.disguised = false;
    owner.objects.insert(id, object);
    owner.award_experience(id, 60.0);
    assert_promotion_present(&owner);
    let mut enemy = make_test_object();
    enemy.id = ObjectId(2);
    enemy.owner_player_id = Some(0);
    enemy.status.stealthed = true;
    enemy.status.detected = false;
    enemy.status.disguised = false;
    owner.objects.insert(enemy.id, enemy);
    let before = owner.queued_audio_events.len();
    owner.award_experience(ObjectId(2), 60.0);
    assert_eq!(owner.queued_audio_events.len(), before);
    assert_eq!(owner.session_view.world_animations.births().count(), 1);
}
#[test]
fn missing_animation_does_not_suppress_receiving_misc_audio() {
    let mut owner = promotion_world();
    owner.promotion_definitions.rules.animation_name = "NotAdmitted".into();
    let object = make_test_object();
    let id = object.id;
    owner.objects.insert(id, object);
    owner.award_experience(id, 60.0);
    assert_eq!(
        owner
            .queued_audio_events
            .iter()
            .filter(|event| event.event_type == "UnitPromoted")
            .count(),
        1
    );
    assert!(
        owner
            .session_view
            .world_animations
            .births()
            .next()
            .is_none()
    );
}
#[test]
fn zero_frame_animation_expires_only_in_unpaused_receiving_ui_phase() {
    let mut owner = promotion_world();
    owner.promotion_definitions.rules.display_seconds = 0.01;
    let object = make_test_object();
    let id = object.id;
    owner.objects.insert(id, object);
    owner.award_experience(id, 60.0);
    assert_eq!(owner.frame, 0);
    owner.set_paused(true);
    owner.advance_world_animations_for_presentation();
    let first = owner.session_view.world_animations.births().next().unwrap();
    assert_eq!(
        first.spawn_frame, 0,
        "birth zero is an actual captured frame"
    );
    assert_eq!(owner.session_view.world_animations.births().count(), 1);
    let mut peer = promotion_world();
    peer.frame = 500;
    peer.advance_world_animations_for_presentation();
    peer.reset();
    assert_eq!(owner.session_view.world_animations.births().count(), 1);
    owner.set_paused(false);
    owner.advance_world_animations_for_presentation();
    assert_eq!(owner.session_view.world_animations.births().count(), 0);
    owner.award_experience(id, 90.0);
    assert_eq!(owner.session_view.world_animations.births().count(), 1);
    owner.reset();
    assert_eq!(owner.session_view.world_animations.births().count(), 0);
}

// Append to object/tests/veterancy_and_combat_state.rs; uses promotion_world().
#[test]
fn repeated_real_presentation_freezes_preserve_zero_birth_identity_and_audio_once() {
    use crate::presentation_frame::PresentationFrame;
    let mut owner = promotion_world();
    let mut peer = promotion_world();
    let mut object = make_test_object();
    let id = object.id;
    object.owner_player_id = Some(0);
    object.thing.template.sound_promoted[1] = Some("DetachedEliteBodyCue".into());
    owner.objects.insert(id, object);
    peer.objects.insert(id, make_test_object());
    owner.award_experience(id, 60.0);
    assert_eq!(owner.queued_audio_events.len(), 1);
    assert_eq!(owner.queued_audio_events[0].event_type, "UnitPromoted");
    let first = PresentationFrame::build_from_logic(&mut owner, 0);
    assert_eq!(first.world_anims.len(), 1);
    let identity = first.world_anims[0].identity;
    let initial_frame = first.world_anims[0].initial_frame;
    assert_ne!(
        identity, 0,
        "actual owned animation admission assigns identity"
    );
    assert_eq!(first.world_anims[0].spawn_frame, 0);
    // Snapshot reads cannot age an actual frame-zero birth to the query frame.
    // Deliberately do not invoke the separate UI expiration phase here.
    owner.frame = 10;
    for _ in 0..3 {
        let repeated = PresentationFrame::build_from_logic(&mut owner, 0);
        assert_eq!(repeated.world_anims.len(), 1);
        assert_eq!(repeated.world_anims[0].spawn_frame, 0);
        assert_eq!(repeated.world_anims[0].identity, identity);
        assert_eq!(repeated.world_anims[0].initial_frame, initial_frame);
        assert_eq!(
            owner.queued_audio_events.len(),
            1,
            "snapshot emits no duplicate audio"
        );
    }
    let mut detached = owner.objects[&id].clone();
    let mut detached_output = Vec::new();
    detached.gain_experience(
        90.0,
        &mut crate::game_logic::ObjectAudio::record(&mut detached_output),
        &mut crate::game_logic::VeterancyFeedback::standalone(),
    );
    assert_eq!(
        detached_output.len(),
        1,
        "explicit local collector receives authored Body cue"
    );
    assert_eq!(detached_output[0].event_type, "DetachedEliteBodyCue");
    assert_eq!(
        owner.queued_audio_events.len(),
        1,
        "clone has no implicit owner writer"
    );
    peer.award_experience(id, 300.0);
    peer.reset();
    let after_peer = PresentationFrame::build_from_logic(&mut owner, 0);
    assert_eq!(after_peer.world_anims.len(), 1);
    assert_eq!(after_peer.world_anims[0].spawn_frame, 0);
    assert_eq!(after_peer.world_anims[0].identity, identity);
    assert_eq!(after_peer.world_anims[0].initial_frame, initial_frame);
    assert_eq!(owner.queued_audio_events.len(), 1);
    assert_eq!(owner.objects[&id].experience.level, VeterancyLevel::Veteran);
    assert_eq!(
        first.world_anims[0].spawn_frame, 0,
        "old snapshot remains frozen"
    );
}
