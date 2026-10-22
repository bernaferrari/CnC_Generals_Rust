//! AutoHealBehavior.cpp:205–219 and 275–298: radius-zero pulses use the
//! receiving object's direct healing entry with itself as source. These tests
//! exercise installed Main Bodies; no Core registry or published world exists.
use crate::game_logic::combat::DamageType;
use crate::game_logic::host_bone_fx_damage::{
    HostBoneFxAuthored, HostBoneFxDamageData, HostBoneFxSlot,
};
use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
use crate::game_logic::host_heal::{DEFAULT_AUTO_HEAL_DELAY_FRAMES, HostDefaultAutoHealData};
use crate::game_logic::{GameLogic, ObjectId, Player, Team};
use glam::Vec3;

fn world(maximum: f32, initial: f32) -> (GameLogic, ObjectId) {
    let source = format!(
        "Object ReceivingDefaultAutoHeal\n KindOf = INFANTRY\n IsTrainable = Yes\n Body = ActiveBody BodyTag\n MaxHealth = {maximum}\n InitialHealth = {initial}\n End\nEnd\n"
    );
    let content = crate::assets::ReceivingObjectContent::from_sources([(
        "receiving_default_autoheal.ini".to_string(),
        source,
    )])
    .unwrap();
    let mut owner = GameLogic::new();
    owner.add_player(Player::new(1, Team::USA, "HealingOwner", true));
    // Body and template must be admitted from the same receiving catalog.
    // Keep exact authored zero/small/max/initial values; do not synthesize HP.
    owner.admit_receiving_module_content(content).unwrap();
    let id = owner
        .create_object_for_player("ReceivingDefaultAutoHeal", 1, Vec3::ZERO)
        .unwrap();
    let object = owner.host_object_mut(id).unwrap();
    assert_eq!(
        (object.health.current, object.health.maximum),
        (initial, maximum)
    );
    assert!(
        object.default_auto_heal.is_some(),
        "creation installs trainable default"
    );
    // Bounded pulse admission, independent of the separate damage-delay path.
    object.default_auto_heal = Some(HostDefaultAutoHealData::new());
    (owner, id)
}

#[test]
fn receiving_default_autoheal_commits_history_with_self_source_and_isolates_peer() {
    let (mut owner, id) = world(100.0, 100.0);
    let (peer, peer_id) = world(100.0, 100.0);
    assert_eq!(id, peer_id);
    let foreign = peer.host_object(id).unwrap();
    let peer_history = (
        foreign.last_damage_info_type,
        foreign.last_damage_source,
        foreign.last_damage_timestamp,
        foreign.last_healing_timestamp,
        foreign.default_auto_heal.as_ref().unwrap().wake_frame,
    );
    owner
        .host_object_mut(id)
        .unwrap()
        .internal_change_health(-80.0);
    owner.frame = 17;
    owner.update_default_auto_heal();
    let object = owner.host_object(id).unwrap();
    assert_eq!(
        (object.previous_health, object.health.current),
        (20.0, 22.0)
    );
    assert_eq!(object.last_damage_info_type, Some(DamageType::Healing));
    assert_eq!(object.last_damage_source, Some(id));
    assert_eq!(object.last_healing_timestamp, Some(17));
    assert_eq!(object.last_damage_timestamp, Some(17));
    assert_eq!(
        object.default_auto_heal.as_ref().unwrap().wake_frame,
        17 + DEFAULT_AUTO_HEAL_DELAY_FRAMES
    );
    let foreign = peer.host_object(id).unwrap();
    assert_eq!(
        (foreign.previous_health, foreign.health.current),
        (100.0, 100.0)
    );
    assert_eq!(
        (
            foreign.last_damage_info_type,
            foreign.last_damage_source,
            foreign.last_damage_timestamp,
            foreign.last_healing_timestamp,
            foreign.default_auto_heal.as_ref().unwrap().wake_frame,
        ),
        peer_history
    );
}

#[test]
fn held_default_autoheal_keeps_processing_the_receiving_body() {
    let (mut owner, id) = world(100.0, 100.0);
    let object = owner.host_object_mut(id).unwrap();
    object.internal_change_health(-80.0);
    // Native Object status admission, without fabricating a container actor.
    object.set_status_disabled_held(true);
    assert!(object.is_held_disabled());
    owner.frame = 23;
    owner.update_default_auto_heal();
    let object = owner.host_object(id).unwrap();
    assert!(object.is_held_disabled());
    assert_eq!(
        (object.previous_health, object.health.current),
        (20.0, 22.0)
    );
    assert_eq!(object.last_damage_info_type, Some(DamageType::Healing));
    assert_eq!(object.last_damage_source, Some(id));
    assert_eq!(object.last_healing_timestamp, Some(23));
}

#[test]
fn default_autoheal_admission_uses_native_effectively_dead_flag_not_hp_predicate() {
    let (mut owner, id) = world(100.0, 0.0);
    let object = owner.host_object(id).unwrap();
    assert!(!object.status.effectively_dead);
    assert!(!object.is_alive());
    owner.frame = 29;
    owner.update_default_auto_heal();
    let object = owner.host_object(id).unwrap();
    assert_eq!((object.previous_health, object.health.current), (0.0, 2.0));
    assert!(!object.status.effectively_dead);
    assert_eq!(object.last_damage_source, Some(id));
    assert_eq!(object.last_healing_timestamp, Some(29));

    let (mut dead_owner, dead_id) = world(100.0, 100.0);
    let object = dead_owner.host_object_mut(dead_id).unwrap();
    object.internal_change_health(-80.0);
    // Explicit prior lifecycle state: this is not a claim that HP writes kill.
    object.status.effectively_dead = true;
    let before = (
        object.previous_health,
        object.last_damage_source,
        object.last_healing_timestamp,
        object.default_auto_heal.as_ref().unwrap().wake_frame,
    );
    dead_owner.frame = 29;
    dead_owner.update_default_auto_heal();
    let object = dead_owner.host_object(dead_id).unwrap();
    assert_eq!(object.health.current, 20.0);
    assert!(object.status.effectively_dead);
    assert_eq!(
        (
            object.previous_health,
            object.last_damage_source,
            object.last_healing_timestamp,
            object.default_auto_heal.as_ref().unwrap().wake_frame,
        ),
        before
    );
}

#[test]
fn default_autoheal_bypasses_indestructible_and_delivers_healing_filtered_state_effect() {
    let (mut owner, id) = world(100.0, 100.0);
    let object = owner.host_object_mut(id).unwrap();
    object.internal_change_health(-31.0);
    assert_eq!(object.body_damage_state, HostBodyDamageType::Damaged);
    object.set_indestructible(true);
    let mut authored = HostBoneFxAuthored::default();
    authored.damage_fx_types = gamelogic::damage::DamageTypeFlags::HEALING.bits();
    authored.fx[HostBodyDamageType::Pristine.ordinal() as usize][0] = Some(HostBoneFxSlot {
        bone: "Body".into(),
        only_once: true,
        delay_min: 0.0,
        delay_max: 0.0,
        name: "ReceivingAutoHealPristine".into(),
    });
    object.bone_fx_damage = Some(HostBoneFxDamageData::from_authored(authored));
    owner.frame = 19;
    owner.update_default_auto_heal();
    let object = owner.host_object(id).unwrap();
    assert_eq!(
        (object.previous_health, object.health.current),
        (69.0, 71.0)
    );
    assert_eq!(object.body_damage_state, HostBodyDamageType::Pristine);
    assert_eq!(object.last_damage_info_type, Some(DamageType::Healing));
    assert_eq!(object.last_damage_source, Some(id));
    let effects = object.bone_fx_damage.as_ref().unwrap();
    assert_eq!(effects.transitions, 1);
    assert_eq!(effects.pending.len(), 1);
    assert_eq!(
        effects.pending[0].fx_list.as_deref(),
        Some("ReceivingAutoHealPristine")
    );
}

#[test]
fn default_autoheal_respects_native_zero_maximum_without_inventing_a_floor() {
    let (mut owner, id) = world(0.0, 0.25);
    owner.frame = 31;
    owner.update_default_auto_heal();
    let object = owner.host_object(id).unwrap();
    assert_eq!((object.health.current, object.health.maximum), (0.25, 0.0));
    assert_eq!(object.previous_health, 0.25);
    assert_eq!(object.last_healing_timestamp, None);
    assert_eq!(object.default_auto_heal.as_ref().unwrap().wake_frame, 0);
}

#[test]
fn default_autoheal_uses_strict_body_comparison_below_small_native_maximum() {
    let (mut owner, id) = world(0.5, 0.5);
    let below = f32::from_bits(0.5f32.to_bits() - 1);
    // Independently expose the epsilon sleep error without the old world
    // driver's invented maximum-health floor masking it.
    assert_eq!(
        HostDefaultAutoHealData::new().tick_heal_amount(41, below, 0.5),
        crate::game_logic::host_heal::DEFAULT_AUTO_HEAL_AMOUNT
    );
    // Pure comparison controls; native authored Body admission rejects NaN.
    for (current, maximum) in [(f32::NAN, 0.5), (0.49, f32::NAN)] {
        let mut helper = HostDefaultAutoHealData::new();
        assert_eq!(helper.tick_heal_amount(41, current, maximum), 0.0);
        assert_eq!(helper.wake_frame, 0);
    }
    owner
        .host_object_mut(id)
        .unwrap()
        .internal_change_health(below - 0.5);
    assert_eq!(owner.host_object(id).unwrap().health.current, below);
    owner.frame = 41;
    owner.update_default_auto_heal();
    let object = owner.host_object(id).unwrap();
    assert_eq!(
        (object.previous_health, object.health.current),
        (below, 0.5)
    );
    assert_eq!(object.last_healing_timestamp, Some(41));
    assert_eq!(object.last_damage_source, Some(id));
}

#[test]
fn default_autoheal_owner_and_pulse_deadline_survive_real_world_snapshot() {
    let (mut owner, id) = world(100.0, 100.0);
    owner
        .host_object_mut(id)
        .unwrap()
        .internal_change_health(-80.0);
    owner.frame = 51;
    owner.update_default_auto_heal();
    let builder = crate::save_load::SnapshotBuilder::new();
    let bytes = bincode_legacy::serialize(&builder.create_world_snapshot(&owner).unwrap()).unwrap();
    let snapshot = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
    let (mut restored, restored_id) = world(100.0, 100.0);
    assert_eq!(id, restored_id);
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    for frame in [
        51 + DEFAULT_AUTO_HEAL_DELAY_FRAMES - 1,
        51 + DEFAULT_AUTO_HEAL_DELAY_FRAMES,
    ] {
        for receiving in [&mut owner, &mut restored] {
            receiving.frame = frame;
            receiving.update_default_auto_heal();
            let object = receiving.host_object(id).unwrap();
            assert_eq!(
                object.health.current,
                if frame == 51 + DEFAULT_AUTO_HEAL_DELAY_FRAMES {
                    24.0
                } else {
                    22.0
                }
            );
            assert_eq!(object.last_damage_source, Some(id));
            assert_eq!(
                object.last_healing_timestamp,
                Some(if frame == 51 + DEFAULT_AUTO_HEAL_DELAY_FRAMES {
                    frame
                } else {
                    51
                })
            );
        }
    }
}

#[test]
fn default_autoheal_pulse_clocks_commit_after_released_body_callback_phase() {
    let (mut owner, id) = world(100.0, 100.0);
    owner
        .host_object_mut(id)
        .unwrap()
        .internal_change_health(-80.0);
    owner.frame = 17;
    let amount = owner
        .host_object_mut(id)
        .unwrap()
        .default_auto_heal
        .as_mut()
        .unwrap()
        .prepare_heal_amount(17, 20.0, 100.0);
    assert_eq!(
        amount,
        crate::game_logic::host_heal::DEFAULT_AUTO_HEAL_AMOUNT
    );
    let helper = owner
        .host_object(id)
        .unwrap()
        .default_auto_heal
        .as_ref()
        .unwrap();
    assert_eq!((helper.soonest_heal_frame, helper.wake_frame), (0, 0));
    owner
        .apply_owned_healing(id, amount, Some(id))
        .expect("receiving native Body pulse");
    let object = owner.host_object(id).unwrap();
    assert_eq!(object.health.current, 22.0);
    assert_eq!(object.last_damage_info_type, Some(DamageType::Healing));
    assert_eq!(object.last_damage_source, Some(id));
    let helper = object.default_auto_heal.as_ref().unwrap();
    assert_eq!((helper.soonest_heal_frame, helper.wake_frame), (0, 0));
    // A bounded callback witness, not an assertion that healing emits damage:
    // original callbacks can change module wake state before pulse completion.
    let helper = owner
        .host_object_mut(id)
        .unwrap()
        .default_auto_heal
        .as_mut()
        .unwrap();
    helper.on_damage(17);
    assert_eq!(helper.soonest_heal_frame, 0);
    assert_eq!(
        helper.wake_frame,
        17 + crate::game_logic::host_heal::DEFAULT_AUTO_HEAL_START_DELAY_FRAMES
    );
    helper.complete_heal_pulse(17);
    assert_eq!(
        (helper.soonest_heal_frame, helper.wake_frame),
        (
            17 + DEFAULT_AUTO_HEAL_DELAY_FRAMES,
            17 + DEFAULT_AUTO_HEAL_DELAY_FRAMES
        )
    );
}
