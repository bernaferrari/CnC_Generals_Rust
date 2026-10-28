//! C++ PartitionData fog transitions through real Main actors, owned shroud
//! cells, native W3D capture, and the receiving session's ghost manager.
use super::super::*;
use super::fog_ghosts::CommittedHostGhostDraw;
use crate::assets::{AuthoredDrawPrimaryTurret, W3DMesh, W3DModel};
use gamelogic::common::{Coord3D, ObjectShroudStatus};
use glam::{Mat4, Vec3};

const PARENT: ObjectId = ObjectId(42);
fn world() -> GameLogic {
    let mut world = GameLogic::new();
    world.add_player(Player::new(1, Team::USA, "Viewer", true));
    world.add_player(Player::new(2, Team::China, "Enemy", false));
    let mut template = ThingTemplate::new("FogBuilding");
    template.add_kind_of(KindOf::Immobile).set_health(100.0);
    world
        .templates
        .insert(template.name.clone(), template.clone());
    let mut object = Object::new(template, PARENT, Team::China);
    object.owner_player_id = Some(2);
    object.set_position(Vec3::new(80.0, 3.0, 80.0));
    object.shroud_clearing_range = 0.0;
    world.add_object(object);
    world
        .world_services
        .shroud()
        .lock()
        .unwrap()
        .init_shroud_grid(240.0, 240.0);
    world
}
fn native_draw(world: &GameLogic, name: &str, generation: u64) -> CommittedHostGhostDraw {
    let object = &world.objects[&PARENT];
    let mut model = W3DModel::new(name.into());
    model.meshes.push(W3DMesh::new(name.into()));
    let render_object = model
        .capture_native_ghost_render_object(
            Mat4::from_translation(object.get_position()),
            1.75,
            0xff12_3456,
            None,
            0.0,
            &AuthoredDrawPrimaryTurret::default(),
            0.0,
            0.0,
            &[],
            &[],
            false,
        )
        .expect("real standalone native Mesh pose");
    CommittedHostGhostDraw {
        identity: world.live_draw_playback_identity(object),
        generation,
        render_objects: vec![render_object],
    }
}
fn reveal(world: &mut GameLogic) {
    world
        .world_services
        .shroud()
        .lock()
        .unwrap()
        .do_shroud_reveal(&Coord3D::new(80.0, 80.0, 3.0), 80.0, 1 << 1);
    world.update_main_crate_vision();
    assert_eq!(status(world), Some(ObjectShroudStatus::Clear));
}
fn fog(world: &mut GameLogic) {
    world
        .world_services
        .shroud()
        .lock()
        .unwrap()
        .undo_shroud_reveal(&Coord3D::new(80.0, 80.0, 3.0), 80.0, 1 << 1);
    world.update_main_crate_vision();
    assert_eq!(status(world), Some(ObjectShroudStatus::Fogged));
}
fn status(world: &GameLogic) -> Option<ObjectShroudStatus> {
    world
        .world_services
        .shroud()
        .lock()
        .unwrap()
        .get_host_object_shroud_status(1, PARENT.0)
}
fn captured_name(world: &GameLogic) -> &str {
    &world.w3d_ghosts.used()[0].snapshots(1)[0]
        .render_object
        .name
}

#[test]
fn native_fog_capture_and_reveal_follow_actual_cells_without_rewriting_fog_memory() {
    let mut world = world();
    reveal(&mut world);
    world.admit_host_ghost_draws(vec![native_draw(&world, "BuildingBeforeFog", 7)]);
    fog(&mut world);
    let ghost = &world.w3d_ghosts.used()[0];
    assert_eq!(
        ghost.previous_shroudedness(1),
        Some(3),
        "C++ Fogged ordinal, not PartialClear"
    );
    assert_eq!(ghost.parent_object_id(), Some(42));
    assert_eq!(ghost.parent_geometry().unwrap().position, [80.0, 80.0, 3.0]);
    assert_eq!(captured_name(&world), "BuildingBeforeFog");
    assert_eq!(ghost.snapshots(1)[0].render_object.scale, 1.0);
    assert_eq!(ghost.snapshots(1)[0].render_object.geometry_scale, 1.75);
    assert_eq!(ghost.snapshots(1)[0].render_object.color, 0xff12_3456);
    world.admit_host_ghost_draws(vec![native_draw(&world, "UnobservedDamageModel", 8)]);
    world.update_main_crate_vision();
    assert_eq!(
        captured_name(&world),
        "BuildingBeforeFog",
        "C++ snapshots only on entering fog"
    );
    reveal(&mut world);
    assert!(!world.w3d_ghosts.used()[0].has_snapshot(1));
    assert!(!world.w3d_ghosts.used()[0].parent_fully_obscured());
}

#[test]
fn same_id_worlds_reject_foreign_native_captures_and_keep_reset_local() {
    let mut a = world();
    let mut b = world();
    reveal(&mut a);
    reveal(&mut b);
    let a_draw = native_draw(&a, "NativeA", 1);
    let b_draw = native_draw(&b, "NativeB", 1);
    a.admit_host_ghost_draws(vec![a_draw.clone(), b_draw.clone()]);
    b.admit_host_ghost_draws(vec![b_draw, a_draw]);
    fog(&mut a);
    fog(&mut b);
    assert_eq!(captured_name(&a), "NativeA");
    assert_eq!(captured_name(&b), "NativeB");
    let before_b = b.w3d_ghosts.clone();
    a.clear_all_objects();
    assert_eq!(a.w3d_ghosts.used()[0].parent_object_id(), None);
    assert_eq!(b.w3d_ghosts, before_b);
    drop(a);
    assert_eq!(captured_name(&b), "NativeB");
}

#[test]
fn physical_parent_removal_preserves_orphan_until_its_original_location_is_revealed() {
    let mut world = world();
    reveal(&mut world);
    world.admit_host_ghost_draws(vec![native_draw(&world, "OrphanBuilding", 1)]);
    fog(&mut world);
    world.detach_host_fog_ghost(PARENT);
    world.objects.remove(&PARENT);
    let old_geometry = world.w3d_ghosts.used()[0].parent_geometry();
    world.update_host_fog_ghosts().unwrap();
    assert_eq!(captured_name(&world), "OrphanBuilding");
    assert_eq!(world.w3d_ghosts.used()[0].parent_object_id(), None);
    // Reusing ObjectID42 at a different position cannot move the partition ghost.
    let mut replacement = Object::new(ThingTemplate::new("Replacement"), PARENT, Team::China);
    replacement.set_position(Vec3::new(200.0, 0.0, 200.0));
    world.objects.insert(PARENT, replacement);
    world.update_host_fog_ghosts().unwrap();
    assert_eq!(world.w3d_ghosts.used()[0].parent_geometry(), old_geometry);
    world
        .world_services
        .shroud()
        .lock()
        .unwrap()
        .do_shroud_reveal(&Coord3D::new(80.0, 80.0, 3.0), 80.0, 1 << 1);
    world.update_host_fog_ghosts().unwrap();
    assert_eq!(world.w3d_ghosts.used_count(), 0);
}

#[test]
fn restore_links_only_real_receiving_parents_and_retains_saved_fog_continuation() {
    let mut source = world();
    reveal(&mut source);
    source.admit_host_ghost_draws(vec![native_draw(&source, "SavedNative", 3)]);
    fog(&mut source);
    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let mut receiver = GameLogic::new();
    // C++ load reconstructs actors from the same effective content catalog.
    receiver.templates = source.templates.clone();
    receiver.add_player(Player::new(1, Team::USA, "Viewer", true));
    receiver.add_player(Player::new(2, Team::China, "Enemy", false));
    builder
        .restore_from_snapshot(&snapshot, &mut receiver)
        .unwrap();
    assert_eq!(captured_name(&receiver), "SavedNative");
    assert_eq!(
        source.w3d_ghosts.used()[0].snapshots(1)[0]
            .render_object
            .geometry_scale,
        1.75
    );
    assert_eq!(
        receiver.w3d_ghosts.used()[0].snapshots(1)[0]
            .render_object
            .geometry_scale,
        1.0,
        "C++ clones reset wire ObjectScale, load recreates with that value"
    );
    assert_eq!(receiver.w3d_ghosts.used()[0].parent_object_id(), Some(42));
    receiver.update_main_crate_vision();
    assert_eq!(captured_name(&receiver), "SavedNative");
    // No source-world identity or renderer observation survived the load.
    receiver.admit_host_ghost_draws(vec![native_draw(&source, "ForeignAfterLoad", 4)]);
    assert_eq!(captured_name(&receiver), "SavedNative");
    receiver.objects.remove(&PARENT);
    receiver.restore_host_fog_ghost_links();
    assert_eq!(receiver.w3d_ghosts.used()[0].parent_object_id(), None);
    assert_eq!(captured_name(&source), "SavedNative");
}

#[cfg(feature = "game_client")]
#[test]
fn native_frozen_scene_materializes_actual_asset_and_remains_immutable_after_reveal() {
    let mut world = world();
    reveal(&mut world);
    world.admit_host_ghost_draws(vec![native_draw(&world, "NativeMesh", 9)]);
    fog(&mut world);
    let frozen = crate::presentation_frame::PresentationFrame::build_from_logic(&mut world, 1);
    let scene = frozen.w3d_ghost_scene.as_ref().unwrap();
    assert_eq!(scene.hidden_parent_object_ids, vec![42]);
    let mut asset = W3DModel::new("NativeMesh".into());
    asset.meshes.push(W3DMesh::new("NativeMesh".into()));
    let asset = std::sync::Arc::new(asset);
    let materialized =
        crate::graphics::render_item::materialize_frozen_w3d_ghost_scene(scene, |name| {
            (name == "NativeMesh").then(|| asset.clone())
        })
        .unwrap();
    assert_eq!(materialized.items.len(), 1);
    assert_eq!(materialized.items[0].state.argb_color, 0xff12_3456);
    assert_eq!(materialized.items[0].state.object_scale, 1.0);
    assert_eq!(materialized.items[0].state.geometry_scale, 1.75);
    assert!(materialized.parent_suppression.suppresses(PARENT));
    reveal(&mut world);
    let next = crate::presentation_frame::PresentationFrame::build_from_logic(&mut world, 1);
    assert!(next.w3d_ghost_scene.as_ref().unwrap().snapshots.is_empty());
    assert_eq!(
        scene.snapshots.len(),
        1,
        "published fog memory is immutable"
    );
}

#[test]
fn cold_and_no_coi_states_keep_previous_fog_memory_until_actual_cells_are_shrouded() {
    let mut world = world();
    reveal(&mut world);
    world.admit_host_ghost_draws(vec![native_draw(&world, "RetainedOffMap", 1)]);
    fog(&mut world);
    let snapshot = world.w3d_ghosts.used()[0].snapshots(1)[0].clone();
    world.set_fixture_object_position(PARENT, Vec3::new(1000.0, 0.0, 1000.0));
    world.update_main_crate_vision();
    assert_eq!(status(&world), Some(ObjectShroudStatus::Shrouded));
    assert_eq!(world.w3d_ghosts.used()[0].previous_shroudedness(1), Some(3));
    assert_eq!(world.w3d_ghosts.used()[0].snapshots(1)[0], snapshot);
    world
        .world_services
        .shroud()
        .lock()
        .unwrap()
        .reset_for_new_game();
    world.update_host_fog_ghosts().unwrap();
    assert_eq!(world.w3d_ghosts.used()[0].previous_shroudedness(1), Some(3));
    assert_eq!(world.w3d_ghosts.used()[0].snapshots(1)[0], snapshot);
    world
        .world_services
        .shroud()
        .lock()
        .unwrap()
        .init_shroud_grid(240.0, 240.0);
    world.set_fixture_object_position(PARENT, Vec3::new(80.0, 3.0, 80.0));
    world.update_main_crate_vision();
    assert_eq!(status(&world), Some(ObjectShroudStatus::Shrouded));
    assert!(!world.w3d_ghosts.used()[0].has_snapshot(1));
    assert_eq!(world.w3d_ghosts.used()[0].previous_shroudedness(1), Some(4));
}

#[cfg(feature = "game_client")]
#[test]
fn actual_host_local_player_selection_switches_only_its_owned_ghost_scene() {
    let mut world = world();
    let mut peer = self::world();
    assert_eq!(
        world.w3d_ghosts.local_player_index(),
        0,
        "inert constructor default"
    );
    reveal(&mut world);
    reveal(&mut peer);
    assert_eq!(
        world.w3d_ghosts.local_player_index(),
        1,
        "actual human local selection"
    );
    world.admit_host_ghost_draws(vec![native_draw(&world, "ViewerOneMemory", 1)]);
    peer.admit_host_ghost_draws(vec![native_draw(&peer, "PeerMemory", 1)]);
    fog(&mut world);
    fog(&mut peer);
    let frozen_one = crate::presentation_frame::PresentationFrame::build_from_logic(&mut world, 1);
    let peer_before = peer.w3d_ghosts.clone();
    assert!(world.select_local_player(Some(2)));
    world.update_main_crate_vision();
    assert_eq!(world.local_player_id(), Some(2));
    assert_eq!(world.w3d_ghosts.local_player_index(), 2);
    let frozen_two = crate::presentation_frame::PresentationFrame::build_from_logic(&mut world, 2);
    assert!(
        frozen_two
            .w3d_ghost_scene
            .as_ref()
            .unwrap()
            .snapshots
            .is_empty()
    );
    assert!(
        frozen_two
            .w3d_ghost_scene
            .as_ref()
            .unwrap()
            .hidden_parent_object_ids
            .is_empty()
    );
    assert_eq!(
        frozen_one
            .w3d_ghost_scene
            .as_ref()
            .unwrap()
            .hidden_parent_object_ids,
        vec![42]
    );
    assert_eq!(
        world.w3d_ghosts.used()[0].snapshots(1)[0]
            .render_object
            .name,
        "ViewerOneMemory"
    );
    assert!(world.select_local_player(Some(1)));
    world.update_main_crate_vision();
    let returned = crate::presentation_frame::PresentationFrame::build_from_logic(&mut world, 1);
    assert_eq!(
        returned.w3d_ghost_scene.as_ref().unwrap().snapshots[0]
            .render_object
            .render_object
            .name,
        "ViewerOneMemory"
    );
    assert_eq!(peer.w3d_ghosts, peer_before);
}

#[test]
fn authored_default_draw_exclusion_and_actual_admission_list_order_match_cpp() {
    let mut parser = crate::assets::IniParser::new();
    parser
        .parse_ini_content(
            "Object NoMemory\n KindOf = IMMOBILE\n Draw = W3DDefaultDraw Tag\n End\nEnd\n",
            "ghost-owner.ini",
        )
        .unwrap();
    let definition = parser.get_definition("NoMemory").unwrap();
    let excluded = GameLogic::build_template_from_object_definition("NoMemory", definition, None);
    assert!(excluded.default_draw_without_fog_memory);
    let mut world = GameLogic::new();
    world.add_object(Object::new(excluded, ObjectId(20), Team::Neutral));
    assert_eq!(world.w3d_ghosts.used_count(), 0);
    let mut template = ThingTemplate::new("RealImmobile");
    template.add_kind_of(KindOf::Immobile);
    world.add_object(Object::new(template.clone(), ObjectId(99), Team::Neutral));
    world.add_object(Object::new(template, ObjectId(1), Team::Neutral));
    assert_eq!(
        world
            .w3d_ghosts
            .used()
            .iter()
            .map(|ghost| ghost.parent_object_id())
            .collect::<Vec<_>>(),
        vec![Some(1), Some(99)]
    );
    world.update_host_fog_ghosts().unwrap();
    assert_eq!(
        world
            .w3d_ghosts
            .used()
            .iter()
            .map(|ghost| ghost.parent_object_id())
            .collect::<Vec<_>>(),
        vec![Some(1), Some(99)],
        "C++ insertion order survives canonical roster reconciliation"
    );
}

#[test]
fn completed_empty_native_replacement_invalidates_the_old_capture() {
    let mut world = world();
    reveal(&mut world);
    world.admit_host_ghost_draws(vec![native_draw(&world, "PreviousActualModel", 1)]);
    // Model=None/suppression is an actual absent RenderObj, not permission
    // to clone the previous model under the unchanged actor identity.
    let suppressed = W3DModel::new("SuppressedModel".into());
    let actual = suppressed.capture_native_ghost_render_object(
        Mat4::IDENTITY,
        1.0,
        0,
        None,
        0.0,
        &AuthoredDrawPrimaryTurret::default(),
        0.0,
        0.0,
        &[],
        &[],
        false,
    );
    assert!(actual.is_none());
    let identity = world.live_draw_playback_identity(&world.objects[&PARENT]);
    world.admit_host_ghost_draws(vec![CommittedHostGhostDraw {
        identity,
        generation: 2,
        render_objects: actual.into_iter().collect(),
    }]);
    fog(&mut world);
    assert!(world.w3d_ghosts.used()[0].snapshots(1).is_empty());
    assert_eq!(
        world.w3d_ghosts.used()[0].previous_shroudedness(1),
        Some(ObjectShroudStatus::Clear as u8)
    );
}

#[cfg(feature = "game_client")]
#[test]
fn frozen_native_capture_sources_include_only_original_eligible_actors() {
    let mut world = world();
    world.add_object(Object::new(
        ThingTemplate::new("MobileActor"),
        ObjectId(99),
        Team::USA,
    ));
    let mut default_draw = ThingTemplate::new("DefaultDrawImmobile");
    default_draw.add_kind_of(KindOf::Immobile);
    default_draw.default_draw_without_fog_memory = true;
    world.add_object(Object::new(default_draw, ObjectId(100), Team::USA));
    let frame = crate::presentation_frame::PresentationFrame::build_from_logic(&mut world, 1);
    assert_eq!(frame.native_ghost_scale_sources.len(), 1);
    assert_eq!(
        frame.native_ghost_scale_sources[&PARENT].identity,
        world.live_draw_playback_identity(&world.objects[&PARENT])
    );
    assert!(!frame.native_ghost_scale_sources.contains_key(&ObjectId(99)));
    assert!(
        !frame
            .native_ghost_scale_sources
            .contains_key(&ObjectId(100))
    );
}
