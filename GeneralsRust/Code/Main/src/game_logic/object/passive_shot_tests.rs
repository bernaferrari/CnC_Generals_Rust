//! Actual installed Object weapon state; no fixture registry or shared clock.
use super::*;

fn passenger() -> Object {
    let mut object = Object::new(
        ThingTemplate::new("PassiveReceiptPassenger"),
        ObjectId(17),
        Team::USA,
    );
    object.ai_state = AIState::Idle;
    object.contained_by = Some(ObjectId(91));
    object.weapon = Some(Weapon {
        damage: 5.0,
        last_fire_time: -10.0,
        ..Weapon::default()
    });
    object.secondary_weapon = Some(Weapon {
        damage: 37.0,
        last_fire_time: -10.0,
        ammo: Some(2),
        clip_size: 2,
        projectile_speed: 9.0,
        ..Weapon::default()
    });
    object
}

#[test]
fn passive_exact_slot_returns_only_its_shot_and_preserves_contained_ai_and_peer() {
    let mut source = passenger();
    let peer = passenger();
    assert!(source.set_weapon_barrel_count_for_slot(1, 3));
    let barrel_before = source.weapon_barrel_cursor_for_snapshot(1);
    let mut combat = crate::game_logic::combat::CombatSystem::new();
    let mut attacks = crate::game_logic::host_attack_log::AttackEvents::default();
    let mut prior = passenger();
    assert!(prior.fire_at(
        ObjectId(3),
        1.0,
        30,
        &mut combat,
        &mut attacks,
        &mut crate::game_logic::MovementAudio::queue(&mut Vec::new())
    ));
    let count = crate::game_logic::combat::pending_projectile_queue_len_for_test(&combat);
    let origin = Vec3::new(13.0, 47.0, 29.0);
    let (slot, shot) = source
        .prepare_passive_weapon_shot(
            1,
            ObjectId(7),
            1.0,
            false,
            false,
            30,
            origin,
            &mut combat.fire_sound_loop_events,
            &mut attacks,
            None,
            &mut crate::game_logic::MovementAudio::queue(&mut Vec::new()),
        )
        .unwrap();
    assert_eq!(slot, 1);
    assert_eq!(shot.shooter_id, source.id);
    assert_eq!(shot.shooter_pos, origin);
    assert_eq!(shot.target_id, Some(ObjectId(7)));
    assert_eq!(shot.damage, 37.0);
    assert_eq!(shot.speed, 9.0);
    assert_eq!(source.secondary_weapon.as_ref().unwrap().ammo, Some(1));
    assert_eq!(source.weapon.as_ref().unwrap().last_fire_time, -10.0);
    assert_eq!(source.ai_state, AIState::Idle);
    assert_eq!(source.contained_by, Some(ObjectId(91)));
    assert_eq!(peer.secondary_weapon.as_ref().unwrap().ammo, Some(2));
    assert_eq!(
        crate::game_logic::combat::pending_projectile_queue_len_for_test(&combat),
        count
    );
    // Durable owner commit is deliberately not performed by the Object phase.
    assert_eq!(source.weapon_barrel_cursor_for_snapshot(1), barrel_before);
}

#[test]
fn passive_windup_and_rejection_keep_ammo_and_never_enter_attack_state() {
    let mut source = passenger();
    source.secondary_weapon.as_mut().unwrap().pre_attack_delay = 0.5;
    let mut sounds = crate::game_logic::host_fire_sound_loop_log::FireSoundLoopEvents::default();
    let mut attacks = crate::game_logic::host_attack_log::AttackEvents::default();
    let origin = Vec3::new(13.0, 47.0, 29.0);
    assert!(
        source
            .prepare_passive_weapon_shot(
                1,
                ObjectId(7),
                1.0,
                false,
                false,
                30,
                origin,
                &mut sounds,
                &mut attacks,
                None,
                &mut crate::game_logic::MovementAudio::queue(&mut Vec::new()),
            )
            .is_none()
    );
    assert_eq!(source.secondary_weapon.as_ref().unwrap().ammo, Some(2));
    assert_eq!(source.ai_state, AIState::Idle);
    assert!(
        source
            .prepare_passive_weapon_shot(
                1,
                ObjectId(7),
                1.5,
                false,
                false,
                45,
                origin,
                &mut sounds,
                &mut attacks,
                None,
                &mut crate::game_logic::MovementAudio::queue(&mut Vec::new()),
            )
            .is_some()
    );
    assert_eq!(source.secondary_weapon.as_ref().unwrap().ammo, Some(1));
    assert_eq!(source.ai_state, AIState::Idle);
    source.status.weapons_jammed = true;
    assert!(
        source
            .prepare_passive_weapon_shot(
                1,
                ObjectId(7),
                5.0,
                false,
                false,
                150,
                origin,
                &mut sounds,
                &mut attacks,
                None,
                &mut crate::game_logic::MovementAudio::queue(&mut Vec::new()),
            )
            .is_none()
    );
    assert_eq!(source.secondary_weapon.as_ref().unwrap().ammo, Some(1));
    source.status.weapons_jammed = false;
    assert!(
        source
            .prepare_passive_weapon_shot(
                2,
                ObjectId(7),
                5.0,
                false,
                false,
                150,
                origin,
                &mut sounds,
                &mut attacks,
                None,
                &mut crate::game_logic::MovementAudio::queue(&mut Vec::new()),
            )
            .is_none()
    );
}
