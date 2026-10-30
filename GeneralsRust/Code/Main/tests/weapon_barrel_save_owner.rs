//! C++ Weapon::xfer saves the raw barrel cursor, and privateFireWeapon
//! checks it against the current Drawable barrel count before the next shot.
use generals_main::game_logic::{GameLogic, Team, ThingTemplate, Weapon};
use generals_main::save_load::SnapshotBuilder;
use glam::Vec3;

fn world() -> GameLogic {
    let mut world = GameLogic::new();
    world
        .templates
        .insert("BarrelOwner".into(), ThingTemplate::new("BarrelOwner"));
    world
}

#[test]
fn restored_cursor_survives_until_receiving_draw_topology_and_next_shot() {
    let mut source = world();
    let id = source
        .create_object("BarrelOwner", Team::USA, Vec3::ZERO)
        .unwrap();
    let mut peer = world();
    let peer_id = peer
        .create_object("BarrelOwner", Team::USA, Vec3::ZERO)
        .unwrap();
    assert_eq!(id, peer_id);
    for world in [&mut source, &mut peer] {
        let object = world.host_object_mut(id).unwrap();
        object.weapon = Some(Weapon::default());
        assert!(object.set_weapon_barrel_count_for_slot(0, 3));
    }
    let object = source.host_object_mut(id).unwrap();
    assert_eq!(object.fired_barrel_for_slot(0), Some(0));
    object.advance_weapon_barrel_after_shot(0);
    assert_eq!(object.fired_barrel_for_slot(0), Some(1));
    object.advance_weapon_barrel_after_shot(0);
    assert_eq!(object.weapon_barrel_cursor_for_snapshot(0), Some((2, 1)));

    let builder = SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&source).unwrap();
    let mut loaded = world();
    builder.restore_from_snapshot(&saved, &mut loaded).unwrap();
    assert_eq!(
        loaded
            .host_object(id)
            .unwrap()
            .weapon_barrel_cursor_for_snapshot(0),
        Some((2, 1))
    );
    let saved_again = builder.create_world_snapshot(&loaded).unwrap();
    assert_eq!(
        saved_again.objects[&id].weapon_barrel_states[0],
        saved.objects[&id].weapon_barrel_states[0]
    );
    let object = loaded.host_object_mut(id).unwrap();
    assert!(object.set_weapon_barrel_count_for_slot(0, 3));
    assert_eq!(object.fired_barrel_for_slot(0), Some(2));
    object.advance_weapon_barrel_after_shot(0);
    assert_eq!(object.fired_barrel_for_slot(0), Some(0));
    assert_eq!(
        peer.host_object(id)
            .unwrap()
            .weapon_barrel_cursor_for_snapshot(0),
        Some((0, 1))
    );
}
