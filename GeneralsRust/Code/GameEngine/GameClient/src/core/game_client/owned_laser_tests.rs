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
    assert_eq!(first.drawables.ray_effects.iter().count(), 1);
    assert_eq!(peer.drawables.ray_effects.iter().count(), 1);
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
    assert_eq!(first.drawables.ray_effects.iter().count(), 0);
    assert_eq!(peer.drawables.ray_effects.iter().count(), 1);
}

#[test]
fn invalid_ray_template_never_creates_a_synthetic_drawable() {
    let mut client = GameClient::new().unwrap();
    assert!(
        client
            .create_owned_ray(&ThingTemplate::new(), Coord3D::ZERO, Coord3D::ZERO)
            .is_err()
    );
    assert!(client.drawables.drawable_map.is_empty());
    assert_eq!(client.drawables.ray_effects.iter().count(), 0);
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
    assert_eq!(client.drawables.ray_effects.iter().count(), 128);
    assert_eq!(client.drawables.drawable_map.len(), 129);
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
    let (mut peer_drawable, roster_position) = peer
        .drawables
        .take_native_drawable_for_load(peer_id)
        .unwrap();
    peer_drawable.set_id(id);
    peer.drawables
        .restore_native_drawable_after_load(id, peer_drawable, roster_position);
    peer.with_owned_laser(id, |module| {
        module.bind_owned_drawable(id.0);
        Ok(())
    })
    .unwrap();
    peer.drawables.owned_laser_order = vec![id];
    peer.drawables.ray_effects.delete(peer_id);
    peer.drawables.ray_effects.add(id, start, end);
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
            .drawables
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
        .drawables
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

#[test]
fn actual_classic_module_rejects_owned_services_before_any_mutation() {
    let mut client = GameClient::new().unwrap();
    let mut drawable = BasicDrawable::new(DrawableId::INVALID);
    drawable.set_position(crate::drawable::Vector3::new(13.0, 29.0, 47.0));
    drawable.add_draw_module(Box::new(
        LogicDrawModuleSnapshotAdapter::client_update_module(
            "ClassicLaser",
            Box::new(LaserUpdateModule::new(
                NameKeyGenerator::name_to_key("LaserUpdate"),
                Arc::new(LaserUpdateModuleData::default()),
                None,
            )),
        ),
    ));
    let id = client
        .register_drawable_with_template(Box::new(drawable), Some("ClassicLaser".into()))
        .unwrap();
    let mut particles = crate::effects::particle_manager::ParticleSystemManager::new();
    let mut random = RandomState::default();
    let mut services = ClientLaserServices {
        frame: 20,
        particles: &mut particles,
        random: &mut random,
        effectively_dead: &|_| Ok(false),
    };
    let result = client.initialize_owned_laser(
        id,
        None,
        None,
        Some(Coord3D::ZERO),
        Some(Coord3D::new(50.0, 0.0, 0.0)),
        "",
        -4,
        &mut services,
    );
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("classic LaserUpdate")
    );
    assert_eq!(services.particles.active_system_count(), 0);
    assert!(
        client
            .owned_laser_geometry(id, &|_| Err("unexpected texture".into()), &mut |_, _| None)
            .unwrap_err()
            .to_string()
            .contains("classic LaserUpdate")
    );
    let drawable = client.drawables.drawable_map.get_mut(&id).unwrap();
    let position = drawable.get_position();
    assert_eq!((position.x, position.y, position.z), (13.0, 29.0, 47.0));
    let basic = drawable
        .as_any_mut()
        .downcast_mut::<BasicDrawable>()
        .unwrap();
    let module = basic.get_draw_modules_mut()[0]
        .logic_module_mut()
        .unwrap()
        .as_any_mut()
        .downcast_mut::<LaserUpdateModule>()
        .unwrap();
    assert!(!module.uses_owned_services());
    assert_eq!(module.update_mut().get_width_scale(), 1.0);
    assert!(!module.update_mut().is_dirty());
    assert_eq!(module.tracked_drawables().0, None);
    assert_eq!(module.tracked_drawables().1, None);
    assert!(client.drawables.owned_laser_order.is_empty());
    assert_eq!(client.drawables.ray_effects.iter().count(), 0);
}

#[test]
fn normal_template_factory_binds_one_owned_callback_without_ray_enrollment() {
    let mut client = GameClient::new().unwrap();
    let template = template();
    let first = client.create_drawable_from_template(&template).unwrap();
    let second = client.create_drawable_from_template(&template).unwrap();
    assert_eq!(client.drawables.owned_laser_order, vec![second, first]);
    assert_eq!(client.drawables.ray_effects.iter().count(), 0);
    for id in [first, second] {
        let drawable = client.drawables.drawable_map.get_mut(&id).unwrap();
        assert_eq!(GameClient::owned_laser_count(drawable.as_mut()), 1);
        client
            .with_owned_laser(id, |module| {
                assert!(module.uses_owned_services());
                Ok(())
            })
            .unwrap();
    }
    let mut particles = crate::effects::particle_manager::ParticleSystemManager::new();
    let mut random = RandomState::default();
    let mut services = ClientLaserServices {
        frame: 10,
        particles: &mut particles,
        random: &mut random,
        effectively_dead: &|_| Ok(false),
    };
    client
        .initialize_owned_laser(
            first,
            None,
            None,
            Some(Coord3D::ZERO),
            Some(Coord3D::new(30.0, 0.0, 0.0)),
            "",
            -4,
            &mut services,
        )
        .unwrap();
    services.frame = 12;
    client.tick_owned_lasers(&mut services).unwrap();
    let first_geometry = client
        .owned_laser_geometry(first, &|_| Err("no texture".into()), &mut |_, _| None)
        .unwrap();
    assert_eq!(first_geometry[0].width, 2.0);
    assert_eq!(client.drawables.owned_laser_order, vec![second, first]);
    assert_eq!(client.drawables.ray_effects.iter().count(), 0);
}

#[test]
fn invalid_normal_laser_data_is_rejected_before_native_id_or_slot_admission() {
    let mut client = GameClient::new().unwrap();
    let mut malformed = ThingTemplate::new();
    malformed.add_client_update_module_info(
        "LaserUpdate".into(),
        "BadLaser".into(),
        Arc::new(W3DLaserDrawModuleData::default()),
        ModuleInterfaceType::CLIENT_UPDATE,
    );
    let counter = client.get_drawable_id_counter();
    assert!(
        client
            .create_drawable_from_template(&malformed)
            .unwrap_err()
            .to_string()
            .contains("LaserUpdate module data type mismatch")
    );
    assert_eq!(client.get_drawable_id_counter(), counter);
    assert!(client.drawables.drawable_map.is_empty());
    assert!(client.drawables.owned_laser_order.is_empty());
    assert_eq!(client.drawables.ray_effects.iter().count(), 0);
}

#[test]
fn supplied_native_reset_marks_real_laser_particles_and_preserves_other_drawables() {
    let mut template = template();
    // Replace the template's single client bucket with an authored particle-bearing definition.
    let mut authored = ThingTemplate::new();
    for entry in template.get_draw_module_info().iter() {
        authored.add_draw_module_info(
            entry.name.clone(),
            entry.module_tag.clone(),
            Arc::clone(entry.data),
            ModuleInterfaceType::DRAW,
        );
    }
    authored.add_client_update_module_info(
        "LaserUpdate".into(),
        "Laser".into(),
        Arc::new(LaserUpdateModuleData {
            particle_system_name: "Lens".into(),
            target_particle_system_name: "Target".into(),
            ..LaserUpdateModuleData::default()
        }),
        ModuleInterfaceType::CLIENT_UPDATE,
    );
    let mut client = GameClient::new().unwrap();
    let first = client.create_drawable_from_template(&authored).unwrap();
    let second = client.create_drawable_from_template(&authored).unwrap();
    let other = client
        .register_drawable_with_template(
            Box::new(BasicDrawable::new(DrawableId::INVALID)),
            Some("Other".into()),
        )
        .unwrap();
    let mut particles = crate::effects::particle_manager::ParticleSystemManager::new();
    particles.new_template("Lens".into());
    particles.new_template("Target".into());
    let mut random = RandomState::default();
    let mut services = ClientLaserServices {
        frame: 17,
        particles: &mut particles,
        random: &mut random,
        effectively_dead: &|_| Ok(false),
    };
    for id in [first, second] {
        client
            .initialize_owned_laser(
                id,
                None,
                None,
                Some(Coord3D::ZERO),
                Some(Coord3D::new(30.0, 0.0, 0.0)),
                "",
                0,
                &mut services,
            )
            .unwrap();
    }
    assert_eq!(services.particles.active_system_count(), 4);
    client.reset_owned_lasers(&mut services).unwrap();
    assert!(client.find_drawable_by_id(first).is_none());
    assert!(client.find_drawable_by_id(second).is_none());
    assert!(client.find_drawable_by_id(other).is_some());
    assert!(client.drawables.owned_laser_order.is_empty());
    assert_eq!(client.drawables.ray_effects.iter().count(), 0);
    assert!(
        services
            .particles
            .all_particle_systems()
            .all(|system| system.is_destroyed())
    );
    services.particles.update(0, 18);
    assert_eq!(services.particles.active_system_count(), 0);
}
