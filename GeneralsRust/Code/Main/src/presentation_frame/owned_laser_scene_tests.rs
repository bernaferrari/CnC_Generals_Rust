use super::*;
use game_client::core::game_client::{ClientLaserServices, GameClient};
use game_engine::common::thing::{ThingTemplate, module::ModuleInterfaceType};
use gamelogic::common::Coord3D;
use gamelogic::object::draw::w3d_laser_draw::W3DLaserDrawModuleData;
use gamelogic::object::update::laser_update::LaserUpdateModuleData;

#[test]
fn receiving_authored_laser_geometry_freezes_once_and_packs_gpu_vertices() {
    let mut template = ThingTemplate::new();
    template.add_draw_module_info(
        "W3DLaserDraw".into(),
        "Beam".into(),
        std::sync::Arc::new(W3DLaserDrawModuleData {
            inner_beam_width: 4.0,
            outer_beam_width: 8.0,
            ..Default::default()
        }),
        ModuleInterfaceType::DRAW,
    );
    template.add_client_update_module_info(
        "LaserUpdate".into(),
        "Laser".into(),
        std::sync::Arc::new(LaserUpdateModuleData::default()),
        ModuleInterfaceType::CLIENT_UPDATE,
    );
    let mut client = GameClient::new().unwrap();
    let mut peer = GameClient::new().unwrap();
    let start = Coord3D::new(13.0, 29.0, 47.0);
    let end = Coord3D::new(17.0, 31.0, 53.0);
    let id = client.create_owned_ray(&template, start, end).unwrap();
    assert_eq!(
        peer.create_owned_ray(&template, Coord3D::new(100.0, 200.0, 300.0), end)
            .unwrap(),
        id
    );
    let mut particles = game_client::effects::particle_manager::ParticleSystemManager::new();
    let mut random = game_engine::common::random_value::RandomState::default();
    client
        .initialize_owned_laser(
            id,
            None,
            None,
            Some(start),
            Some(end),
            "",
            -4,
            &mut ClientLaserServices {
                frame: 10,
                particles: &mut particles,
                random: &mut random,
                effectively_dead: &|_| Ok(false),
            },
        )
        .unwrap();
    let lines = freeze_owned_laser_scene_lines(
        &mut client,
        &|_| Err("unexpected textured beam".into()),
        &mut |_, _| None,
    )
    .unwrap();
    assert!(!lines.is_empty());
    assert_eq!(lines[0].start, (13.0, 47.0, 29.0));
    assert_eq!(lines[0].end, (17.0, 53.0, 31.0));
    let upload = crate::graphics::laser_segment_upload::LaserSegmentUpload::pack_beams_streams_and_scene_lines(&[], &[], &lines);
    let first_vertex: Vec<_> = (0..3)
        .map(|axis| {
            f32::from_le_bytes(
                upload.vertex_bytes[axis * 4..axis * 4 + 4]
                    .try_into()
                    .unwrap(),
            )
        })
        .collect();
    assert_eq!(first_vertex, [13.0, 47.0, 29.0]);
    assert_eq!(upload.honesty.beams_packed as usize, lines.len());
    assert_eq!(peer.find_drawable_by_id(id).unwrap().get_position().x, 58.5);
}
