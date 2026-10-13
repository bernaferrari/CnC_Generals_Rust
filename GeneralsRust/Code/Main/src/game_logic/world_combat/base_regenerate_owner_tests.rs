//! CPP BaseRenerateUpdate.cpp:89–122 through the real receiving Healing driver.
use super::*;
use crate::game_logic::host_base_regenerate::base_regen_heal_amount;

fn world(frame: u32) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "RegenOwner", true));
    world.frame = frame;
    let mut template = ThingTemplate::new("AmericaCommandCenter");
    template.add_kind_of(KindOf::Structure).set_health(1000.0);
    world.templates.insert(template.name.clone(), template);
    let id = world
        .create_object_for_player("AmericaCommandCenter", 0, Vec3::ZERO)
        .unwrap();
    let object = world.host_object_mut(id).unwrap();
    object.internal_change_health(500.0 - object.health.current);
    let regen = object
        .base_regenerate
        .as_mut()
        .expect("actual CommandCenter BaseRegenerate");
    regen.wake_frame = frame;
    regen.pending_damage = false;
    // Install the real poison module's already-active continuation. Healing's
    // callback must clear it; numeric-only HP writes do not.
    object.notify_poisoned_on_damage(
        frame,
        crate::game_logic::combat::DamageType::Toxin,
        1.0,
        crate::game_logic::host_usa_pilot::HostDeathType::Poisoned,
    );
    (world, id)
}

#[test]
fn actual_regen_healing_is_immediate_self_sourced_and_peer_local() {
    let (mut owner, id) = world(100);
    let (peer, peer_id) = world(700);
    assert_eq!(id, peer_id);
    assert!(owner.host_object(id).unwrap().is_poison_tinted());
    owner.update_base_regenerate();
    let body = owner.host_object(id).unwrap();
    assert!((body.health.current - (500.0 + base_regen_heal_amount(1000.0))).abs() < 1e-4);
    assert_eq!(body.previous_health, 500.0);
    assert_eq!(body.last_healing_timestamp, Some(100));
    assert_eq!(body.last_damage_timestamp, Some(100));
    assert_eq!(
        body.last_damage_info_type,
        Some(crate::game_logic::combat::DamageType::Healing)
    );
    assert_eq!(body.last_damage_source, Some(id));
    assert!(!body.is_poison_tinted());
    assert_eq!(owner.base_regenerate_reg.heal_ticks, 1);
    let peer_body = peer.host_object(peer_id).unwrap();
    assert_eq!(peer_body.health.current, 500.0);
    assert_eq!(peer_body.last_healing_timestamp, None);
    assert!(peer_body.is_poison_tinted());
}

#[test]
fn healing_keeps_three_frame_timer_and_sold_construction_gates() {
    let (mut owner, id) = world(100);
    owner.update_base_regenerate();
    let first = owner.host_object(id).unwrap().health.current;
    for frame in [101, 102] {
        owner.frame = frame;
        owner.update_base_regenerate();
        assert_eq!(owner.host_object(id).unwrap().health.current, first);
    }
    owner.frame = 103;
    owner
        .host_object_mut(id)
        .unwrap()
        .set_status_under_construction(true);
    owner.update_base_regenerate();
    assert_eq!(owner.host_object(id).unwrap().health.current, first);
    owner
        .host_object_mut(id)
        .unwrap()
        .set_status_under_construction(false);
    owner.update_base_regenerate();
    assert_eq!(owner.base_regenerate_reg.heal_ticks, 2);
    assert_eq!(
        owner.host_object(id).unwrap().last_healing_timestamp,
        Some(103)
    );
    owner.frame = 106;
    owner.host_object_mut(id).unwrap().status.sold = true;
    let before_sold = owner.host_object(id).unwrap().health.current;
    owner.update_base_regenerate();
    assert_eq!(owner.host_object(id).unwrap().health.current, before_sold);
    assert!(
        owner
            .host_object(id)
            .unwrap()
            .base_regenerate
            .as_ref()
            .unwrap()
            .done_sold
    );
}

#[test]
fn normal_saved_regen_continues_same_body_and_callback_future() {
    let (mut owner, id) = world(100);
    owner.update_base_regenerate();
    let builder = crate::save_load::SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&owner).unwrap();
    let (mut restored, peer_id) = world(900);
    assert_eq!(id, peer_id);
    builder
        .restore_from_snapshot(&saved, &mut restored)
        .unwrap();
    for frame in [101, 102, 103] {
        owner.frame = frame;
        restored.frame = frame;
        owner.update_base_regenerate();
        restored.update_base_regenerate();
        let expected = owner.host_object(id).unwrap();
        let actual = restored.host_object(id).unwrap();
        assert_eq!(actual.health.current, expected.health.current);
        assert_eq!(actual.previous_health, expected.previous_health);
        assert_eq!(
            actual.last_healing_timestamp,
            expected.last_healing_timestamp
        );
        assert_eq!(actual.last_damage_source, expected.last_damage_source);
        assert_eq!(
            actual.base_regenerate.as_ref().unwrap().wake_frame,
            expected.base_regenerate.as_ref().unwrap().wake_frame
        );
    }
}
