//! Encoded continuation uses the receiving world's immutable Armor.ini rows.
use super::*;

fn receiving_world(coefficient: f32) -> GameLogic {
    let mut catalog = gamelogic::object::armor::ArmorDefinitions::new();
    let mut row = gamelogic::object::armor::ArmorTemplate::new();
    row.set_default(coefficient);
    catalog.register_template("SharedArmor", row);
    let mut world = GameLogic::new();
    world.armor_definitions = Some(std::sync::Arc::new(catalog));
    let mut template = ThingTemplate::new("ReceivingArmor");
    template.set_health(100.0);
    template.armor_sets.push(HostArmorSet {
        conditions: 0,
        armor: Some("SharedArmor".into()),
        damage_fx: None,
    });
    world.templates.insert("ReceivingArmor".into(), template);
    world
}

#[test]
fn encoded_save_armor_continuation_uses_receiving_catalog_and_preserves_peer() {
    let mut source = receiving_world(0.25);
    let id = source
        .create_object("ReceivingArmor", Team::USA, Vec3::ZERO)
        .unwrap();
    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded: WorldSnapshot = bincode_legacy::deserialize(&bytes).unwrap();

    // The same authored name resolves independently in each receiving session.
    let mut peer = receiving_world(0.5);
    let peer_id = peer
        .create_object("ReceivingArmor", Team::USA, Vec3::ZERO)
        .unwrap();
    assert_eq!(id, peer_id);
    let mut restored = receiving_world(0.25);
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    let damage = crate::game_logic::combat::DamageType::Explosive;
    let mut events = crate::game_logic::HostHealthEvents::default();
    source
        .host_object_mut(id)
        .unwrap()
        .take_damage_from_typed(20.0, None, damage, 0, &mut events);
    restored
        .host_object_mut(id)
        .unwrap()
        .take_damage_from_typed(20.0, None, damage, 0, &mut events);
    peer.host_object_mut(peer_id)
        .unwrap()
        .take_damage_from_typed(20.0, None, damage, 0, &mut events);
    assert_eq!(source.host_object(id).unwrap().health.current, 95.0);
    assert_eq!(restored.host_object(id).unwrap().health.current, 95.0);
    assert_eq!(peer.host_object(peer_id).unwrap().health.current, 90.0);

    // Historical templates retain their wire: the receiving catalog is not saved.
    let mut other = source.templates["ReceivingArmor"].clone();
    let before = bincode_legacy::serialize(&other).unwrap();
    other.armor_definitions = peer.armor_definitions.clone();
    assert_eq!(bincode_legacy::serialize(&other).unwrap(), before);
}
