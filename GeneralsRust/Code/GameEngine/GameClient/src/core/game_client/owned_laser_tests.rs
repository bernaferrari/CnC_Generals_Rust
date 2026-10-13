use super::*;
use game_engine::common::thing::module::ModuleInterfaceType;
use gamelogic::common::Coord3D;

fn template() -> ThingTemplate {
    let mut template = ThingTemplate::new();
    template.add_draw_module_info(
        "W3DLaserDraw".into(),
        "Beam".into(),
        Arc::new(W3DLaserDrawModuleData {
            inner_beam_width: 4.0,
            outer_beam_width: 8.0,
            ..W3DLaserDrawModuleData::default()
        }),
        ModuleInterfaceType::DRAW,
    );
    template.add_client_update_module_info(
        "LaserUpdate".into(),
        "Laser".into(),
        Arc::new(LaserUpdateModuleData::default()),
        ModuleInterfaceType::CLIENT_UPDATE,
    );
    template
}

#[test]
fn actual_ray_factory_and_geometry_use_the_receiving_installed_modules() {
    let mut first = GameClient::new().unwrap();
    let mut peer = GameClient::new().unwrap();
    let template = template();
    let start = Coord3D::new(10.0, 20.0, 30.0);
    let end = Coord3D::new(40.0, 20.0, 30.0);
    let ray = first.create_owned_ray(&template, start, end).unwrap();
    let peer_ray = peer.create_owned_ray(&template, start, end).unwrap();
    // Match constructor does not import another client's registry.
    assert_eq!(first.ray_effects.iter().count(), 1);
    assert_eq!(peer.ray_effects.iter().count(), 1);
    assert_eq!(
        first.find_drawable_by_id(ray).unwrap().get_position().x,
        25.0
    );
    let mut particles = crate::effects::particle_manager::ParticleSystemManager::new();
    let mut random = RandomState::default();
    let mut services = ClientLaserServices {
        frame: 10,
        particles: &mut particles,
        random: &mut random,
        effectively_dead: &|_| Ok(false),
    };
    first
        .initialize_owned_laser(
            ray,
            None,
            None,
            Some(start),
            Some(end),
            "",
            -4,
            &mut services,
        )
        .unwrap();
    let lines = first
        .owned_laser_geometry(ray, &|_| Err("unexpected texture".into()), &mut |_, _| None)
        .unwrap();
    assert!(!lines.is_empty());
    assert_eq!(lines[0].start.x, 10.0);
    assert_eq!(lines[0].end.x, 40.0);
    let width = lines[0].width;
    services.frame = 12;
    first.update_owned_laser(ray, &mut services).unwrap();
    let halfway = first
        .owned_laser_geometry(ray, &|_| Err("unexpected texture".into()), &mut |_, _| None)
        .unwrap();
    assert_eq!(halfway[0].width, width * 0.5);
    // C++ decay reaches zero, but never independently destroys the Drawable.
    services.frame = 14;
    first.update_owned_laser(ray, &mut services).unwrap();
    let finished = first
        .owned_laser_geometry(ray, &|_| Err("unexpected texture".into()), &mut |_, _| None)
        .unwrap();
    assert_eq!(finished[0].width, 0.0);
    assert!(first.find_drawable_by_id(ray).is_some());
    assert!(peer.find_drawable_by_id(peer_ray).is_some());
    first.destroy_drawable(ray).unwrap();
    assert_eq!(first.ray_effects.iter().count(), 0);
    assert_eq!(peer.ray_effects.iter().count(), 1);
}

#[test]
fn invalid_ray_template_never_creates_a_synthetic_drawable() {
    let mut client = GameClient::new().unwrap();
    assert!(
        client
            .create_owned_ray(&ThingTemplate::new(), Coord3D::ZERO, Coord3D::ZERO)
            .is_err()
    );
    assert!(client.drawable_map.is_empty());
    assert_eq!(client.ray_effects.iter().count(), 0);
}

#[test]
fn full_cpp_ray_registry_keeps_the_real_factory_drawable_alive() {
    let mut client = GameClient::new().unwrap();
    let template = template();
    let mut last = DrawableId::INVALID;
    for _ in 0..=crate::effects::ray_effect_registry::MAX_RAY_EFFECTS {
        last = client
            .create_owned_ray(&template, Coord3D::ZERO, Coord3D::new(1.0, 0.0, 0.0))
            .unwrap();
    }
    assert_eq!(client.ray_effects.iter().count(), 128);
    assert_eq!(client.drawable_map.len(), 129);
    assert!(client.find_drawable_by_id(last).is_some());
}

#[test]
fn same_id_actual_laser_destructor_retires_only_its_receiving_particle_manager() {
    let mut template = template();
    // A separate actual template carries the original Lens/Target definitions.
    let mut particle_template = ThingTemplate::new();
    for entry in template.get_draw_module_info().iter() {
        particle_template.add_draw_module_info(
            entry.name.clone(),
            entry.module_tag.clone(),
            Arc::clone(entry.data),
            ModuleInterfaceType::DRAW,
        );
    }
    particle_template.add_client_update_module_info(
        "LaserUpdate".into(),
        "Laser".into(),
        Arc::new(LaserUpdateModuleData {
            particle_system_name: "Lens".into(),
            target_particle_system_name: "Target".into(),
            ..LaserUpdateModuleData::default()
        }),
        ModuleInterfaceType::CLIENT_UPDATE,
    );
    let mut first = GameClient::new().unwrap();
    let mut peer = GameClient::new().unwrap();
    let start = Coord3D::new(4.0, 5.0, 6.0);
    let end = Coord3D::new(20.0, 5.0, 6.0);
    let id = first
        .create_owned_ray(&particle_template, start, end)
        .unwrap();
    let peer_id = peer
        .create_owned_ray(&particle_template, start, end)
        .unwrap();
    // Match-load-style exact identity installation. No process counter/registry
    // publication participates in the actual runtime or destructor assertions.
    let mut peer_drawable = peer.drawable_map.remove(&peer_id).unwrap();
    peer_drawable.set_id(id);
    peer.drawable_map.insert(id, peer_drawable);
    peer.with_owned_laser(id, |module| {
        module.bind_owned_drawable(id.0);
        Ok(())
    })
    .unwrap();
    peer.owned_laser_order = vec![id];
    peer.ray_effects.delete(peer_id);
    peer.ray_effects.add(id, start, end);
    let mut first_particles = crate::effects::particle_manager::ParticleSystemManager::new();
    let mut peer_particles = crate::effects::particle_manager::ParticleSystemManager::new();
    for manager in [&mut first_particles, &mut peer_particles] {
        manager.new_template("Lens".into());
        manager.new_template("Target".into());
    }
    let mut first_random = RandomState::default();
    let mut peer_random = RandomState::default();
    let mut first_services = ClientLaserServices {
        frame: 17,
        particles: &mut first_particles,
        random: &mut first_random,
        effectively_dead: &|_| Ok(false),
    };
    let mut peer_services = ClientLaserServices {
        frame: 17,
        particles: &mut peer_particles,
        random: &mut peer_random,
        effectively_dead: &|_| Ok(false),
    };
    first
        .initialize_owned_laser(
            id,
            None,
            None,
            Some(start),
            Some(end),
            "",
            0,
            &mut first_services,
        )
        .unwrap();
    peer.initialize_owned_laser(
        id,
        None,
        None,
        Some(start),
        Some(end),
        "",
        0,
        &mut peer_services,
    )
    .unwrap();
    assert_eq!(first_services.particles.active_system_count(), 2);
    assert_eq!(peer_services.particles.active_system_count(), 2);
    assert!(
        first.destroy_drawable(id).is_err(),
        "unsupplied destruction cannot silently abandon actual particles"
    );
    assert!(first.find_drawable_by_id(id).is_some());
    first.destroy_owned_laser(id, &mut first_services).unwrap();
    // ParticleSys.cpp1255 destroy marks systems; 2059/2923 retires empty
    // systems only during the receiving manager's next update phase.
    assert_eq!(first_services.particles.active_system_count(), 2);
    assert!(
        first_services
            .particles
            .all_particle_systems()
            .all(|system| system.is_destroyed())
    );
    assert!(
        peer_services
            .particles
            .all_particle_systems()
            .all(|system| !system.is_destroyed())
    );
    first_services.particles.update(0, 18);
    assert_eq!(first_services.particles.active_system_count(), 0);
    assert_eq!(peer_services.particles.active_system_count(), 2);
    assert!(first.find_drawable_by_id(id).is_none());
    assert!(peer.find_drawable_by_id(id).is_some());
    peer.destroy_owned_laser(id, &mut peer_services).unwrap();
    assert!(
        peer_services
            .particles
            .all_particle_systems()
            .all(|system| system.is_destroyed())
    );
    peer_services.particles.update(0, 18);
    assert_eq!(peer_services.particles.active_system_count(), 0);
}

#[test]
fn actual_multibeam_arc_preserves_cpp_draw_order_and_receiving_terrain_clamp() {
    let mut template = ThingTemplate::new();
    template.add_draw_module_info(
        "W3DLaserDraw".into(),
        "Beam".into(),
        Arc::new(W3DLaserDrawModuleData {
            inner_beam_width: 2.0,
            outer_beam_width: 8.0,
            num_beams: 4,
            segments: 3,
            arc_height: 10.0,
            ..W3DLaserDrawModuleData::default()
        }),
        ModuleInterfaceType::DRAW,
    );
    template.add_client_update_module_info(
        "LaserUpdate".into(),
        "Laser".into(),
        Arc::new(LaserUpdateModuleData::default()),
        ModuleInterfaceType::CLIENT_UPDATE,
    );
    let mut client = GameClient::new().unwrap();
    let start = Coord3D::new(0.0, 0.0, 0.0);
    let end = Coord3D::new(30.0, 0.0, 0.0);
    let id = client.create_owned_ray(&template, start, end).unwrap();
    let mut particles = crate::effects::particle_manager::ParticleSystemManager::new();
    let mut random = RandomState::default();
    let mut services = ClientLaserServices {
        frame: 10,
        particles: &mut particles,
        random: &mut random,
        effectively_dead: &|_| Ok(false),
    };
    client
        .initialize_owned_laser(id, None, None, Some(start), Some(end), "", 0, &mut services)
        .unwrap();
    let lines = client
        .owned_laser_geometry(id, &|_| Err("no texture authored".into()), &mut |_, _| {
            Some(100.0)
        })
        .unwrap();
    assert_eq!(lines.len(), 12);
    for segment in lines.chunks(4) {
        assert_eq!(
            segment.iter().map(|line| line.width).collect::<Vec<_>>(),
            vec![8.0, 6.0, 4.0, 2.0]
        );
        assert!(
            segment
                .iter()
                .all(|line| line.start.z == 102.0 && line.end.z == 102.0)
        );
    }
    // A second read retains actual draw-cache state; it does not reread another
    // session's terrain when LaserUpdate and the draw's own flags are clean.
    let unchanged = client
        .owned_laser_geometry(id, &|_| Err("no texture authored".into()), &mut |_, _| {
            Some(-100.0)
        })
        .unwrap();
    assert!(
        unchanged
            .iter()
            .all(|line| line.start.z == 102.0 && line.end.z == 102.0)
    );
}

#[test]
fn actual_host_direct_drawable_is_converted_once_into_cpp_laser_coordinates() {
    let mut client = GameClient::new().unwrap();
    let source = presentation_drawable_sync_for_test(
        77,
        4,
        "HostSource",
        true,
        false,
        [13.0, 47.0, 29.0],
        0.0,
    );
    client.sync_presentation_drawables([source]);
    let source_id = client.get_drawable_for_object(77).unwrap();
    assert!(
        client
            .presentation_direct_drawable_bindings
            .contains_key(&source_id)
    );
    let parent = LaserObjectObservation {
        position: Coord3D::new(13.0, 29.0, 47.0),
        drawable: Some(source_id.0),
        visible_to_local_player: true,
    };
    let id = client
        .create_owned_ray(&template(), parent.position, Coord3D::new(50.0, 29.0, 47.0))
        .unwrap();
    let mut particles = crate::effects::particle_manager::ParticleSystemManager::new();
    let mut random = RandomState::default();
    let mut services = ClientLaserServices {
        frame: 20,
        particles: &mut particles,
        random: &mut random,
        effectively_dead: &|_| Ok(false),
    };
    client
        .initialize_owned_laser(
            id,
            Some(parent),
            None,
            None,
            Some(Coord3D::new(50.0, 29.0, 47.0)),
            "",
            0,
            &mut services,
        )
        .unwrap();
    let geometry = client
        .owned_laser_geometry(id, &|_| Err("no texture".into()), &mut |_, _| None)
        .unwrap();
    assert_eq!(
        (
            geometry[0].start.x,
            geometry[0].start.y,
            geometry[0].start.z
        ),
        (13.0, 29.0, 47.0)
    );
    let actual_source = client
        .find_drawable_by_id(source_id)
        .unwrap()
        .get_position();
    assert_eq!(
        (actual_source.x, actual_source.y, actual_source.z),
        (13.0, 47.0, 29.0)
    );
    // Named worldspace bone belongs to that same host projection and receives
    // the identical one-time input conversion before the C++ kernel.
    let mut bones = crate::drawable::BoneData::default();
    bones.worldspace_bones.insert(
        "Muzzle".into(),
        crate::drawable::Matrix4::translation(crate::drawable::Vector3::new(17.0, 53.0, 31.0)),
    );
    client
        .drawable_map
        .get_mut(&source_id)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<BasicDrawable>()
        .unwrap()
        .set_bone_data(bones);
    client
        .initialize_owned_laser(
            id,
            Some(parent),
            None,
            None,
            Some(Coord3D::new(50.0, 29.0, 47.0)),
            "Muzzle",
            0,
            &mut services,
        )
        .unwrap();
    let geometry = client
        .owned_laser_geometry(id, &|_| Err("no texture".into()), &mut |_, _| None)
        .unwrap();
    assert_eq!(
        (
            geometry[0].start.x,
            geometry[0].start.y,
            geometry[0].start.z
        ),
        (17.0, 31.0, 53.0)
    );
}
