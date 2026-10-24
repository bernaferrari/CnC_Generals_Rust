//! Receiving native containment healing, not the empty Core object registry.
use super::super::super::*;
use crate::game_logic::combat::DamageType;
use crate::game_logic::host_bone_fx_damage::{
    HostBoneFxAuthored, HostBoneFxDamageData, HostBoneFxSlot,
};
use crate::game_logic::host_enum_table_residual::HostBodyDamageType;

fn world(kind: &str, attributes: &str) -> (GameLogic, ObjectId, ObjectId) {
    world_with_duration(kind, attributes, 1000)
}

fn world_with_duration(
    kind: &str,
    attributes: &str,
    milliseconds: u32,
) -> (GameLogic, ObjectId, ObjectId) {
    let ini = format!(
        r#"
Object ContainmentHealingOwner
 KindOf = STRUCTURE
 Body = ActiveBody BodyTag
  MaxHealth = 100
  InitialHealth = 100
 End
 Behavior = {kind} ContainTag
  ContainMax = 5
  AllowInsideKindOf = INFANTRY
  TimeForFullHeal = {milliseconds}
  {attributes}
 End
End
Object ContainmentHealingPassenger
 KindOf = INFANTRY
 TransportSlotCount = 1
 Locomotor = SET_NORMAL BasicHumanLocomotor
 Body = ActiveBody BodyTag
  MaxHealth = 100
  InitialHealth = 100
 End
 Behavior = AIUpdateInterface AiTag
 End
End
"#
    );
    let mut owner = GameLogic::new();
    owner.world_min = Vec3::ZERO;
    owner.world_max = Vec3::splat(1000.0);
    owner.add_player(Player::new(1, Team::USA, "HealingOwner", true));
    owner
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "containment_healing_owner.ini".into(),
                ini,
            )])
            .unwrap(),
        )
        .unwrap();
    let container = owner
        .create_object_for_player("ContainmentHealingOwner", 1, Vec3::new(50.0, 0.0, 50.0))
        .unwrap();
    let passenger = owner
        .create_object_for_player("ContainmentHealingPassenger", 1, Vec3::new(50.0, 0.0, 50.0))
        .unwrap();
    assert_eq!(
        owner
            .host_object(container)
            .unwrap()
            .thing()
            .template
            .contain_module
            .frames_for_full_heal,
        Some((milliseconds as f32 * 30.0 / 1000.0).ceil() as u32)
    );
    assert_eq!(
        owner
            .host_object(container)
            .unwrap()
            .thing()
            .template
            .contain_module
            .slots,
        Some(5),
        "native ContainMax must be parsed before passenger admission"
    );
    if kind != "TunnelContain" {
        assert!(
            owner.host_object(container).unwrap().can_contain(),
            "live authored container must admit passengers"
        );
        assert!(
            owner.host_object(container).unwrap().has_capacity_for(1),
            "authored ContainMax must retain available capacity"
        );
    }
    if kind == "TunnelContain" {
        let key = owner.host_object(container).unwrap().tunnel_system_key();
        assert!(owner.tunnel_network.record_enter(key, passenger, container));
    } else {
        assert!(
            owner
                .host_object_mut(container)
                .unwrap()
                .add_occupant(passenger)
        );
    }
    owner.set_fixture_object_contained_by(passenger, Some(container));
    owner
        .tunnel_network
        .stamp_contained_by_frame(passenger, 100);
    let object = owner.host_object_mut(passenger).unwrap();
    assert_eq!(
        (object.health.current, object.health.maximum),
        (100.0, 100.0)
    );
    object.internal_change_health(-80.0);
    object.indestructible = true;
    object.last_damage_source = Some(ObjectId(9999));
    object.last_damage_info_type = Some(DamageType::Bullet);
    let mut authored = HostBoneFxAuthored::default();
    authored.damage_fx_types = gamelogic::damage::DamageTypeFlags::HEALING.bits();
    authored.fx[HostBodyDamageType::Pristine.ordinal() as usize][0] = Some(HostBoneFxSlot {
        bone: "Body".into(),
        only_once: true,
        delay_min: 0.0,
        delay_max: 0.0,
        name: "ContainmentHealingState".into(),
    });
    object.bone_fx_damage = Some(HostBoneFxDamageData::from_authored(authored));
    owner.frame = 101;
    (owner, container, passenger)
}

fn assert_healing(owner: &GameLogic, passenger: ObjectId, source: Option<ObjectId>, health: f32) {
    let object = owner.host_object(passenger).unwrap();
    assert!(
        (object.health.current - health).abs() < 0.0001,
        "current health {} expected {health}",
        object.health.current
    );
    assert_eq!(object.last_damage_source, source);
    assert_eq!(object.last_damage_info_type, Some(DamageType::Healing));
    assert_eq!(object.last_damage_timestamp, Some(owner.frame));
}

#[test]
fn heal_contain_uses_container_source_and_receiving_body_callbacks() {
    let (mut owner, container, passenger) = world("HealContain", "");
    let (peer, peer_container, peer_passenger) = world("HealContain", "");
    assert_eq!((container, passenger), (peer_container, peer_passenger));
    owner.tick_heal_contain_and_tunnel();
    assert_healing(&owner, passenger, Some(container), 20.0 + 100.0 / 30.0);
    assert_eq!(
        owner.host_object(passenger).unwrap().contained_by,
        Some(container)
    );
    owner
        .host_object_mut(container)
        .unwrap()
        .door_close_countdown = 99;
    owner.frame = 130;
    owner.tick_heal_contain_and_tunnel();
    assert_healing(&owner, passenger, Some(container), 100.0);
    assert_eq!(owner.host_object(passenger).unwrap().contained_by, None);
    assert!(
        owner
            .host_object(container)
            .unwrap()
            .contained_units()
            .is_empty()
    );
    assert_eq!(owner.tunnel_network.contained_by_frame(passenger), None);
    assert_eq!(owner.tunnel_network.heal_auto_exits, 1);
    assert_eq!(
        owner
            .host_object(passenger)
            .unwrap()
            .bone_fx_damage
            .as_ref()
            .unwrap()
            .pending[0]
            .fx_list
            .as_deref(),
        Some("ContainmentHealingState")
    );
    assert_eq!(peer.host_object(passenger).unwrap().health.current, 20.0);
    assert_eq!(
        peer.host_object(passenger).unwrap().last_damage_info_type,
        Some(DamageType::Bullet)
    );
}

#[test]
fn garrison_heal_objects_true_has_no_source_or_auto_exit() {
    let (mut owner, container, passenger) = world("GarrisonContain", "HealObjects = Yes");
    owner.tick_heal_contain_and_tunnel();
    assert_healing(&owner, passenger, None, 20.0 + 100.0 / 30.0);
    owner.frame = 130;
    owner.tick_heal_contain_and_tunnel();
    assert_healing(&owner, passenger, None, 100.0);
    assert_eq!(
        owner.host_object(passenger).unwrap().contained_by,
        Some(container)
    );
    assert_eq!(
        owner.host_object(container).unwrap().contained_units(),
        vec![passenger]
    );
    assert_eq!(owner.tunnel_network.heal_auto_exits, 0);
    assert_eq!(
        owner
            .host_object(passenger)
            .unwrap()
            .bone_fx_damage
            .as_ref()
            .unwrap()
            .pending
            .len(),
        1
    );
}

#[test]
fn garrison_time_for_full_heal_without_heal_objects_does_not_heal() {
    let (mut owner, container, passenger) = world("GarrisonContain", "HealObjects = No");
    owner.frame = 130;
    owner.tick_heal_contain_and_tunnel();
    let object = owner.host_object(passenger).unwrap();
    assert_eq!(object.health.current, 20.0);
    assert_eq!(object.last_damage_info_type, Some(DamageType::Bullet));
    assert_eq!(object.last_damage_source, Some(ObjectId(9999)));
    assert_eq!(object.contained_by, Some(container));
    assert!(object.bone_fx_damage.as_ref().unwrap().pending.is_empty());
}

#[test]
fn each_tunnel_entrance_heals_shared_passenger_with_no_source() {
    let (mut owner, container, passenger) = world("TunnelContain", "");
    let (peer, peer_container, peer_passenger) = world("TunnelContain", "");
    assert_eq!((container, passenger), (peer_container, peer_passenger));
    let second = owner
        .create_object_for_player("ContainmentHealingOwner", 1, Vec3::new(100.0, 0.0, 50.0))
        .unwrap();
    assert_eq!(
        owner.host_object(second).unwrap().tunnel_system_key(),
        owner.host_object(container).unwrap().tunnel_system_key()
    );
    owner.tick_heal_contain_and_tunnel();
    assert_healing(&owner, passenger, None, 20.0 + 2.0 * (100.0 / 30.0));
    assert_eq!(owner.tunnel_network.heal_ticks, 2);
    assert_eq!(
        owner.host_object(passenger).unwrap().contained_by,
        Some(container)
    );
    assert_eq!(peer.host_object(passenger).unwrap().health.current, 20.0);
    assert_eq!(peer.tunnel_network.heal_ticks, 0);
    owner.frame = 130;
    owner.tick_heal_contain_and_tunnel();
    assert_healing(&owner, passenger, None, 100.0);
    assert_eq!(owner.tunnel_network.heal_ticks, 4);
    assert_eq!(owner.tunnel_network.heal_auto_exits, 0);
    assert_eq!(
        owner
            .host_object(passenger)
            .unwrap()
            .bone_fx_damage
            .as_ref()
            .unwrap()
            .pending
            .len(),
        1
    );
    assert_eq!(peer.host_object(passenger).unwrap().health.current, 20.0);
}

#[test]
fn contained_duration_wraps_at_logic_frame_boundary() {
    let (mut owner, container, passenger) = world("GarrisonContain", "HealObjects = Yes");
    owner
        .tunnel_network
        .stamp_contained_by_frame(passenger, u32::MAX - 4);
    owner.frame = 25; // elapsed = 30, not a saturating zero
    owner.tick_heal_contain_and_tunnel();
    assert_healing(&owner, passenger, None, 100.0);
    assert_eq!(
        owner.host_object(passenger).unwrap().contained_by,
        Some(container)
    );
}

#[test]
fn zero_duration_heal_contain_completes_on_first_pulse() {
    let (mut owner, container, passenger) = world_with_duration("HealContain", "", 0);
    owner.tick_heal_contain_and_tunnel();
    assert_healing(&owner, passenger, Some(container), 100.0);
    assert_eq!(owner.host_object(passenger).unwrap().contained_by, None);
    assert!(
        owner
            .host_object(container)
            .unwrap()
            .contained_units()
            .is_empty()
    );
    assert_eq!(owner.tunnel_network.contained_by_frame(passenger), None);
    assert_eq!(owner.tunnel_network.heal_auto_exits, 1);
}

#[test]
fn tunnel_healing_continues_after_real_world_snapshot_restore() {
    let (mut owner, first, passenger) = world("TunnelContain", "");
    let second = owner
        .create_object_for_player("ContainmentHealingOwner", 1, Vec3::new(100.0, 0.0, 50.0))
        .unwrap();
    owner.tick_heal_contain_and_tunnel();
    let builder = crate::save_load::SnapshotBuilder::new();
    let bytes = bincode_legacy::serialize(&builder.create_world_snapshot(&owner).unwrap()).unwrap();
    let saved = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
    let (mut restored, _, _) = world("TunnelContain", "");
    builder
        .restore_from_snapshot(&saved, &mut restored)
        .unwrap();
    let key = owner.host_object(first).unwrap().tunnel_system_key();
    assert_eq!(
        restored.tunnel_network.contained_for_player(key),
        vec![passenger]
    );
    assert_eq!(
        restored.tunnel_network.contained_by_frame(passenger),
        Some(100)
    );
    assert!(restored.host_object(second).is_some());
    for receiving in [&mut owner, &mut restored] {
        receiving.frame = 102;
        receiving.tick_heal_contain_and_tunnel();
        assert_healing(receiving, passenger, None, 20.0 + 4.0 * (100.0 / 30.0));
        assert_eq!(
            receiving.host_object(passenger).unwrap().contained_by,
            Some(first)
        );
        assert_eq!(
            receiving.tunnel_network.contained_for_player(key),
            vec![passenger]
        );
    }
}

#[test]
fn zero_duration_enabled_garrison_heals_fully_without_exit() {
    let (mut owner, container, passenger) =
        world_with_duration("GarrisonContain", "HealObjects = Yes", 0);
    owner.tick_heal_contain_and_tunnel();
    assert_healing(&owner, passenger, None, 100.0);
    assert_eq!(
        owner.host_object(passenger).unwrap().contained_by,
        Some(container)
    );
    assert_eq!(
        owner.host_object(container).unwrap().contained_units(),
        vec![passenger]
    );
    assert_eq!(
        owner.tunnel_network.contained_by_frame(passenger),
        Some(100)
    );
    assert_eq!(owner.tunnel_network.heal_auto_exits, 0);
    assert_eq!(
        owner
            .host_object(passenger)
            .unwrap()
            .bone_fx_damage
            .as_ref()
            .unwrap()
            .pending
            .len(),
        1
    );
}

#[test]
fn heal_contain_admits_healing_in_passenger_insertion_order_not_object_id_order() {
    let (mut owner, container, first) = world("HealContain", "");
    let second = owner
        .create_object_for_player("ContainmentHealingPassenger", 1, Vec3::new(50.0, 0.0, 50.0))
        .unwrap();
    assert!(second.0 > first.0);
    assert!(
        owner
            .objects
            .get_mut(&container)
            .unwrap()
            .remove_occupant_with_partition_owner(
                first,
                &mut owner.ownership_events,
                &mut owner.partition_manager
            )
    );
    assert!(
        owner
            .host_object_mut(container)
            .unwrap()
            .add_occupant(second)
    );
    assert!(
        owner
            .host_object_mut(container)
            .unwrap()
            .add_occupant(first)
    );
    assert_eq!(
        owner.host_object(container).unwrap().contained_units(),
        vec![second, first]
    );
    let object = owner.host_object_mut(second).unwrap();
    object.internal_change_health(-80.0);
    object.indestructible = true;
    owner.set_fixture_object_contained_by(second, Some(container));
    owner.tunnel_network.stamp_contained_by_frame(second, 100);
    owner.health_events.drain_ordered();
    owner.frame = 130;
    owner.tick_heal_contain_and_tunnel();
    let targets: Vec<_> = owner
        .health_events
        .snapshot_ordered()
        .into_iter()
        .filter_map(|event| match event {
            crate::game_logic::host_health_log::HostHealthEvent::Heal(event) => Some(event.target),
            crate::game_logic::host_health_log::HostHealthEvent::Damage(_) => None,
        })
        .collect();
    assert_eq!(targets, vec![second, first]);
    assert!(
        owner
            .host_object(container)
            .unwrap()
            .contained_units()
            .is_empty()
    );
    assert_eq!(owner.tunnel_network.heal_auto_exits, 2);
    for passenger in [second, first] {
        assert_healing(&owner, passenger, Some(container), 100.0);
        assert_eq!(owner.host_object(passenger).unwrap().contained_by, None);
        assert_eq!(owner.tunnel_network.contained_by_frame(passenger), None);
    }
}

#[test]
fn containment_amounts_preserve_authored_negative_maximum_before_armor() {
    use crate::game_logic::host_tunnel_network::tunnel_tracker_heal_amount;
    use gamelogic::object::contain::garrison_heal_single_amount;
    // The original methods submit these amounts to Body::attemptHealing;
    // armor adjustment happens there, not in the containment calculator.
    let mut armor = gamelogic::object::armor::ArmorTemplate::new();
    armor.set_coefficient(gamelogic::damage::DamageType::Healing, -1.0);
    for (elapsed, duration, expected) in [(0, 6, -20.0), (6, 6, -120.0), (0, 0, -120.0)] {
        assert_eq!(
            tunnel_tracker_heal_amount(-120.0, elapsed, duration),
            expected
        );
        assert_eq!(
            garrison_heal_single_amount(-120.0, elapsed, duration as f32),
            expected
        );
        assert_eq!(
            armor.adjust_damage(gamelogic::damage::DamageType::Healing, expected),
            -expected,
            "negative input must reach armor before its resulting amount is clamped"
        );
    }
}
