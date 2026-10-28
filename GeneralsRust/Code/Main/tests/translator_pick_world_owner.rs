use generals_main::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate};
use glam::Vec3;

fn world_at(position: Vec3) -> (GameLogic, u32) {
    let mut world = GameLogic::new();
    world.add_player(Player::new(1, Team::USA, "PickOwner", true));
    let mut template = ThingTemplate::new("PickOwnerUnit");
    template
        .set_health(100.0)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Infantry);
    world.templates.insert(template.name.clone(), template);
    let id = world
        .create_object_for_player("PickOwnerUnit", 1, position)
        .unwrap();
    world
        .create_object_for_player("PickOwnerUnit", 1, position + Vec3::new(5.0, 0.0, 0.0))
        .unwrap();
    (world, id.0)
}

#[test]
fn receiving_pick_projection_keeps_same_id_objects_world_scoped_and_ordered() {
    let (owner, owner_id) = world_at(Vec3::new(10.0, 0.0, 0.0));
    let (peer, peer_id) = world_at(Vec3::new(80.0, 0.0, 0.0));
    assert_eq!(owner_id, peer_id, "independent worlds reuse object IDs");

    let owner_view = owner.translator_pick_object_view();
    let peer_view = peer.translator_pick_object_view();
    assert_eq!(owner_view.objects.len(), 2);
    assert_eq!(peer_view.objects.len(), 2);
    // C++ GameLogic::registerObject prepends; the later birth is visited first.
    assert_eq!(owner_view.objects[1].object_id, owner_id);
    assert_eq!(peer_view.objects[1].object_id, peer_id);
    assert_eq!(owner_view.objects[0].position[0], 15.0);
    assert_eq!(peer_view.objects[0].position[0], 85.0);
    assert_eq!(owner_view.objects[1].position[0], 10.0);
    assert_eq!(peer_view.objects[1].position[0], 80.0);
    assert!(owner_view.objects[0].locally_controlled);
    assert!(peer_view.objects[0].locally_controlled);

    let empty_view = GameLogic::new().translator_pick_object_view();
    assert!(
        empty_view.objects.is_empty(),
        "empty owner remains authoritative"
    );
}

#[test]
fn receiving_point_ray_uses_only_the_bound_native_drawable_candidates() {
    use game_client::display::view::{
        IPoint2, Point3, pick_native_drawable_from_facts, world_to_screen_from_pick_camera,
    };
    use game_engine::common::message_stream::{TranslatorPickCamera, TranslatorPickDrawableFact};

    let camera = TranslatorPickCamera {
        view_matrix: glam::Mat4::IDENTITY.to_cols_array(),
        projection_matrix: glam::Mat4::IDENTITY.to_cols_array(),
        camera_position: [0.0, 0.0, -1.0],
        viewport_origin: [0, 0],
        viewport_size: [100, 100],
    };
    let owner_drawables = [TranslatorPickDrawableFact {
        object_id: 7,
        center: [0.0, 0.0, 0.0],
        radius: 0.5,
    }];
    let peer_drawables = [TranslatorPickDrawableFact {
        object_id: 7,
        center: [20.0, 0.0, 0.0],
        radius: 0.5,
    }];
    let mut no_window = |_, _| false;
    let point = IPoint2::new(50, 50);
    assert_eq!(
        pick_native_drawable_from_facts(&point, &mut no_window, &camera, &owner_drawables),
        Some(7)
    );
    assert_eq!(
        pick_native_drawable_from_facts(&point, &mut no_window, &camera, &peer_drawables),
        None,
        "same-ID peer drawable is not present in this receiving owner's hit candidates"
    );
    assert_eq!(
        world_to_screen_from_pick_camera(&Point3::new(0.0, 0.0, 0.0), &camera),
        Some(point)
    );
    assert_eq!(
        world_to_screen_from_pick_camera(&Point3::new(2.0, 0.0, 0.0), &camera),
        None,
        "off-frustum objects must not be picked through a projected screen point"
    );
}
