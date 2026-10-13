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
            .create_owned_ray(&ThingTemplate::new(), Coord3D::origin(), Coord3D::origin())
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
            .create_owned_ray(&template, Coord3D::origin(), Coord3D::new(1.0, 0.0, 0.0))
            .unwrap();
    }
    assert_eq!(client.ray_effects.iter().count(), 128);
    assert_eq!(client.drawable_map.len(), 129);
    assert!(client.find_drawable_by_id(last).is_some());
}
