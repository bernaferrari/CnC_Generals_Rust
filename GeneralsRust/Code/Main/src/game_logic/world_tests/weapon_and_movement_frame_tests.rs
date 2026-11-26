//! Receiving weapon slots, movement and shroud changes in published frames.
use crate::fixture_content::{FixtureObject, admit_fixture_objects};
use crate::game_logic::{GameLogic, ObjectAudio, ObjectId, Player, Team, Weapon};
use crate::presentation_frame::{PresentationFrame, PresentationProjectileClipStatus};
use glam::Vec3;

fn clip(ammo: u32, size: u32) -> Option<PresentationProjectileClipStatus> {
    Some(PresentationProjectileClipStatus {
        shots_remaining: ammo,
        max_shots: size,
    })
}

fn clip_statuses(
    world: &mut GameLogic,
    id: ObjectId,
) -> [Option<PresentationProjectileClipStatus>; 3] {
    let frame = PresentationFrame::try_build_from_logic(world, 0).unwrap();
    frame
        .objects
        .iter()
        .find(|row| row.id == id)
        .unwrap()
        .projectile_clip_statuses
}

#[test]
fn publishing_weapon_clips_preserves_each_slot_and_clears_removed_slots() {
    let mut logic = GameLogic::new();
    admit_fixture_objects(
        &mut logic,
        &[FixtureObject::new("ClipTestInfantry")
            .kind_of("INFANTRY SELECTABLE")
            .health(100.0)],
    );
    let id = logic
        .create_object("ClipTestInfantry", Team::USA, Vec3::new(4.0, 0.0, 4.0))
        .unwrap();
    assert_eq!(clip_statuses(&mut logic, id), [None; 3]);

    let actor = logic.host_object_mut(id).unwrap();
    actor.weapon = Some(Weapon {
        clip_size: 10,
        ammo: Some(7),
        reload_time: 1.0,
        last_fire_time: 0.2,
        ..Weapon::default()
    });
    actor.tertiary_weapon = Some(Weapon {
        clip_size: 4,
        ammo: Some(3),
        reload_time: 2.5,
        last_fire_time: 9.0,
        ..Weapon::default()
    });
    assert_eq!(
        clip_statuses(&mut logic, id),
        [clip(7, 10), None, clip(3, 4)]
    );

    let actor = logic.host_object_mut(id).unwrap();
    actor.weapon.as_mut().unwrap().ammo = Some(6);
    assert_eq!(
        clip_statuses(&mut logic, id),
        [clip(6, 10), None, clip(3, 4)]
    );
    assert_eq!(
        logic.host_authoritative_projectile_clip_statuses(id),
        [Some((6, 10)), None, Some((3, 4))]
    );

    logic.host_object_mut(id).unwrap().tertiary_weapon = None;
    assert_eq!(clip_statuses(&mut logic, id), [clip(6, 10), None, None]);
    logic.host_object_mut(id).unwrap().weapon = None;
    assert_eq!(clip_statuses(&mut logic, id), [None; 3]);
}

fn approaching_unit(dest: Vec3) -> (GameLogic, ObjectId) {
    let mut logic = GameLogic::new();
    admit_fixture_objects(
        &mut logic,
        &[FixtureObject::new("FrameWalker")
            .kind_of("INFANTRY SELECTABLE")
            .health(100.0)
            .walker()],
    );
    let id = logic
        .create_object("FrameWalker", Team::USA, Vec3::ZERO)
        .unwrap();
    let (unit, mut events) = logic.host_object_and_movement_events_mut(id).unwrap();
    unit.movement.max_speed = 30.0;
    unit.move_to(
        dest,
        0,
        &mut events,
        &mut ObjectAudio::record(&mut Vec::new()),
    );
    (logic, id)
}

#[test]
fn published_movement_matches_an_independent_same_id_run() {
    let dest = Vec3::new(80.0, 0.0, 0.0);
    let (mut observed, id) = approaching_unit(dest);
    let (mut independent, other_id) = approaching_unit(dest);
    assert_eq!(id, other_id);
    for _ in 0..45 {
        observed.update_movement_for_test(&[id], 1.0 / 30.0);
        independent.update_movement_for_test(&[other_id], 1.0 / 30.0);
        let frame = PresentationFrame::try_build_from_logic(&mut observed, 0).unwrap();
        let row = frame.objects.iter().find(|row| row.id == id).unwrap();
        let pose = observed.host_object(id).unwrap().get_position();
        assert_eq!(row.position, pose);
        assert_eq!(
            pose,
            independent.host_object(other_id).unwrap().get_position()
        );
    }
    assert!(observed.host_object(id).unwrap().get_position().x > 0.0);
}

fn shrouded_actor(ammo: u32, subdual: f32) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    admit_fixture_objects(
        &mut world,
        &[FixtureObject::new("ObservedWeapon").kind_of("INFANTRY SELECTABLE")],
    );
    world.add_player(Player::new(0, Team::USA, "Viewer", true));
    world.add_player(Player::new(1, Team::China, "Enemy", false));
    let id = world
        .create_object("ObservedWeapon", Team::China, Vec3::ZERO)
        .unwrap();
    let actor = world.host_object_mut(id).unwrap();
    actor.weapon = Some(Weapon {
        clip_size: 10,
        ammo: Some(ammo),
        ..Weapon::default()
    });
    actor.subdual_damage = subdual;
    (world, id)
}

#[test]
fn same_frame_publication_uses_current_shroud_and_each_worlds_weapon() {
    use crate::fow_rendering::ObjectVisibility;
    use gamelogic::common::ObjectShroudStatus;

    let (mut world, id) = shrouded_actor(7, 7.0);
    let (mut peer, peer_id) = shrouded_actor(8, 0.0);
    assert_eq!(id, peer_id);
    let frame = world.get_frame();
    let before = PresentationFrame::try_build_from_logic(&mut world, 0).unwrap();
    let initial = before.objects.iter().find(|row| row.id == id).unwrap();
    assert_eq!(initial.projectile_clip_statuses[0], clip(7, 10));
    assert!(initial.gaining_subdual);
    {
        let mut shroud = world.world_services.shroud().lock().unwrap();
        shroud.mark_host_vision_ready();
        shroud.mark_host_object_explored(0, id.0);
        shroud.set_host_object_shroud_status(0, id.0, ObjectShroudStatus::Fogged);
    }
    let foggy = PresentationFrame::try_build_from_logic(&mut world, 0).unwrap();
    let fogged = foggy.objects.iter().find(|row| row.id == id).unwrap();
    assert_eq!(
        fogged.fow_visibility.visibility_alpha,
        ObjectVisibility::FOGGED.visibility_alpha
    );
    assert_ne!(initial.fow_visibility, fogged.fow_visibility);
    assert_eq!(fogged.projectile_clip_statuses[0], clip(7, 10));

    let actor = world.host_object_mut(id).unwrap();
    actor.weapon.as_mut().unwrap().ammo = Some(5);
    actor.subdual_damage = 11.0;
    let foreign = PresentationFrame::try_build_from_logic(&mut peer, 0).unwrap();
    let same_id = foreign.objects.iter().find(|row| row.id == id).unwrap();
    assert_eq!(same_id.projectile_clip_statuses[0], clip(8, 10));
    assert!(!same_id.gaining_subdual);
    let revised = PresentationFrame::try_build_from_logic(&mut world, 0).unwrap();
    let own = revised.objects.iter().find(|row| row.id == id).unwrap();
    assert_eq!(own.projectile_clip_statuses[0], clip(5, 10));
    assert!(own.gaining_subdual);
    assert_eq!(own.fow_visibility, fogged.fow_visibility);
    assert_eq!(initial.projectile_clip_statuses[0], clip(7, 10));
    assert_eq!(world.get_frame(), frame);
    assert_eq!(peer.get_frame(), frame);
}
