use super::*;
fn actor(id: u32, name: &str, kind: KindOf, position: glam::Vec3) -> Object {
    let mut template = ThingTemplate::new(name);
    template.add_kind_of(kind).set_health(100.0);
    let mut object = Object::new(template, ObjectId(id), Team::USA);
    object.set_position(position);
    object
}
fn world(position: glam::Vec3) -> GameLogic {
    let mut world = GameLogic::new();
    world.add_player(Player::new(1, Team::USA, "Local", true));
    let mut rider = actor(
        51,
        "HijackerOwner",
        KindOf::Infantry,
        glam::Vec3::new(20.0, 0.0, 20.0),
    );
    rider.owner_player_id = Some(1);
    rider.selection_radius = 4.0;
    rider.crusher_level = 2;
    rider.begin_hijacker_in_vehicle(ObjectId(52), &mut ObjectAudio::queue(&mut Vec::new()));
    world.objects.insert(ObjectId(51), rider);
    world.objects.insert(
        ObjectId(52),
        actor(52, "VehicleOwner", KindOf::Vehicle, position),
    );
    world
}
#[test]
fn missing_target_unhides_in_place_instead_of_restoring_stale_eject_position() {
    let mut world = world(glam::Vec3::ZERO);
    world.objects.remove(&ObjectId(52));
    let rider = world.objects.get_mut(&ObjectId(51)).unwrap();
    let position = rider.get_position();
    rider.hijacker_eject_pos = Some(glam::Vec3::new(1.0, 0.0, 1.0));
    world.tick_hijacker_updates();
    let rider = world.objects.get(&ObjectId(51)).unwrap();
    assert_eq!(rider.get_position(), position);
    assert!(!rider.drawable_hidden);
    assert!(!rider.hijacker_in_vehicle);
    assert!(!rider.hijacker_update_active);
    assert!(!rider.hijacker_was_airborne);
    assert!(world.partition_manager.is_registered(51));
}
#[test]
fn ride_preserves_retained_dead_target_and_uses_each_experience_threshold() {
    let mut world = world(glam::Vec3::new(4.0, 0.0, 5.0));
    let rider = world.objects.get_mut(&ObjectId(51)).unwrap();
    rider.template_mut().veterancy_xp_thresholds = [100.0, 200.0, 300.0];
    rider.experience.current = 15.0;
    let vehicle = world.objects.get_mut(&ObjectId(52)).unwrap();
    vehicle.experience.level = VeterancyLevel::Elite;
    vehicle.experience.current = 999.0;
    vehicle.status.effectively_dead = true;
    world.tick_hijacker_updates();
    let rider = world.objects.get(&ObjectId(51)).unwrap();
    assert!(rider.hijacker_in_vehicle);
    assert_eq!(rider.get_position(), glam::Vec3::new(4.0, 0.0, 5.0));
    assert_eq!(rider.experience.level, VeterancyLevel::Elite);
    assert_eq!(
        rider.experience.current, 200.0,
        "source sets rider's own level threshold, not target XP"
    );
    assert_eq!(
        world.objects.get(&ObjectId(52)).unwrap().experience.current,
        999.0,
        "unchanged source level preserves earned XP"
    );
}
#[cfg(feature = "game_client")]
#[test]
fn same_id_peer_reset_does_not_redirect_receiving_ride_tree_fx() {
    use game_engine::common::{
        audio::{AudioEventRts, AudioSubmissionLocality},
        ini::ini_fx_list::{FXList, FXNugget},
    };
    struct Audio(Vec<String>);
    impl crate::game_logic::NativeAudioEffects for Audio {
        fn add_audio_event(&mut self, event: &AudioEventRts, _: &AudioSubmissionLocality) {
            self.0.push(event.get_event_name().to_owned());
        }
    }
    let mut a = world(glam::Vec3::new(2.0, 0.0, 2.0));
    let mut b = world(glam::Vec3::new(80.0, 0.0, 80.0));
    a.world_services
        .shroud()
        .lock()
        .unwrap()
        .init_shroud_grid(100.0, 100.0);
    a.world_services
        .shroud()
        .lock()
        .unwrap()
        .reveal_map_for_player_permanently(1)
        .unwrap();
    let mut definition = FXList::new("HijackerTreeFX".into());
    definition.add_nugget(FXNugget::Sound {
        name: "HijackerTreeSound".into(),
    });
    let mut catalog = game_client::fx_list::FXListStore::new();
    catalog.add_fx_list(
        "HijackerTreeFX".into(),
        game_client::fx_list::FXList::from_definition(&definition),
    );
    a.admit_fx_catalog(catalog);
    let mut terrain = game_client::terrain::terrain_visual::TerrainVisualImpl::new();
    terrain
        .tree_buffer_mut()
        .set_bounds(game_client::terrain::TreeRegion2D::new(
            glam::Vec2::ZERO,
            glam::Vec2::splat(100.0),
        ));
    terrain
        .tree_buffer_mut()
        .add_tree(
            900,
            glam::Vec3::new(2.0, 2.0, 0.0),
            1.0,
            0.0,
            1.0,
            game_client::terrain::TreeModuleData {
                model_name: "HijackerTree".into(),
                do_topple: true,
                topple_fx: Some("HijackerTreeFX".into()),
                ..Default::default()
            },
            game_client::terrain::TreeSphere {
                center: glam::Vec3::ZERO,
                radius: 2.0,
            },
        )
        .unwrap();
    b.tick_hijacker_updates();
    b.reset();
    drop(b);
    let mut audio = Audio(Vec::new());
    a.tick_hijacker_updates_with_effects(&mut crate::game_logic::ScriptPresentationEffects {
        terrain: Some(&mut terrain),
        audio: Some(&mut audio),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(audio.0, ["HijackerTreeSound"]);
    assert!(terrain.tree_buffer_mut().pending_fx_events().is_empty());
    assert_eq!(
        a.objects.get(&ObjectId(51)).unwrap().get_position(),
        glam::Vec3::new(2.0, 0.0, 2.0)
    );
}
