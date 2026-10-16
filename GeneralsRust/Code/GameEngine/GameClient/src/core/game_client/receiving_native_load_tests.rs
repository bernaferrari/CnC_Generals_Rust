use super::*;
use game_engine::common::system::{
    xfer_load::XferLoad as CommonLoad, xfer_save::XferSave as CommonSave,
};
use std::io::Cursor;

#[test]
fn receiving_native_failed_drawable_decode_leaves_no_live_roster_node() {
    let mut factory = game_engine::common::thing::ThingFactory::new();
    let template = factory.new_template("NativeRosterMalformedDecode");
    let mut source = GameClient::new().unwrap();
    source.set_frame(37);
    source.create_drawable_from_template(&template).unwrap();
    let mut bytes = Vec::new();
    source
        .xfer(&mut CommonSave::new(Cursor::new(&mut bytes), 3))
        .unwrap();
    // Locate the actual Drawable payload with the native envelope operations,
    // then retain only its version byte. Both real readers must reach the
    // extracted receiver and fail during its own payload, not earlier TOC I/O.
    let mut probe = CommonLoad::new(Cursor::new(&bytes), 3);
    let mut version = 3;
    probe.xfer_version(&mut version, 3).unwrap();
    let mut frame = 0;
    probe.xfer_unsigned_int(&mut frame).unwrap();
    let mut envelope = GameClient::new().unwrap();
    envelope.xfer_drawable_toc(&mut probe).unwrap();
    let mut count = 0;
    probe.xfer_unsigned_short(&mut count).unwrap();
    assert_eq!(count, 1);
    let mut toc = 0;
    probe.xfer_unsigned_short(&mut toc).unwrap();
    probe.begin_block().unwrap();
    let mut object_id = 0;
    probe.xfer_unsigned_int(&mut object_id).unwrap();
    bytes.truncate(probe.bytes_read() as usize + 1);
    let old_factory = game_engine::common::thing::get_thing_factory()
        .unwrap()
        .replace(factory);
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), &bytes).unwrap();
    let mut observations = Vec::new();
    for runtime_reader in [false, true] {
        let mut receiving = GameClient::new().unwrap();
        let failed = if runtime_reader {
            let mut reader = game_engine::System::xfer_load::XferLoad::new();
            reader
                .open(file.path().to_string_lossy().into_owned())
                .unwrap();
            let result = xfer_game_client_state(&mut receiving, &mut reader);
            reader.close().unwrap();
            result.is_err()
        } else {
            receiving
                .xfer(&mut CommonLoad::new(Cursor::new(&bytes), 3))
                .is_err()
        };
        let no_live_node = receiving.drawables.native_drawable_order.is_empty()
            && receiving.drawables.drawable_map.is_empty();
        let mut particles = crate::effects::particle_manager::ParticleSystemManager::new();
        let draw_models = receiving.admit_native_draw_frame(false);
        let receiving_frame = receiving.get_frame();
        let next_draw = receiving
            .run_native_draw_particle_phase_with_services(
                &mut particles,
                None,
                &[],
                NativeParticleFrames {
                    logic_frame: 1,
                    client_frame: receiving_frame,
                },
                draw_models,
                true,
            )
            .is_ok();
        let reset = receiving.reset().is_ok();
        let next = receiving.create_drawable_from_template(&template).unwrap();
        let reused = receiving.drawables.native_drawable_order == [next];
        observations.push((failed, no_live_node, next_draw, reset, reused));
    }
    *game_engine::common::thing::get_thing_factory().unwrap() = old_factory;
    assert_eq!(observations, [(true, true, true, true, true); 2]);
}

#[test]
fn receiving_native_explicit_driver_always_advances_its_particle_owner() {
    let source = include_str!("impl_update.rs");
    let entry = source
        .split("pub fn update_presentation_shell_with_particle_frames(")
        .nth(1)
        .unwrap()
        .split("fn update_presentation_shell_with_particle_policy(")
        .next()
        .unwrap();
    assert!(
        entry
            .split_whitespace()
            .collect::<String>()
            .contains("particle_frames,true,&[])")
    );
    assert!(!entry.contains("OBJECT_REGISTRY"));
    assert!(!include_str!("native_particle_draw.rs").contains("OBJECT_REGISTRY"));
    // Actual manager execution/dedup with explicit true is exercised by the
    // frozen, frame-zero and equal-ID receiving-native Draw kernel regressions.
}

#[test]
fn receiving_native_loaded_id_reuses_exact_detached_roster_position() {
    let mut template = ThingTemplate::new();
    template.set_template_name("NativeRosterPosition".into());
    let mut client = GameClient::new().unwrap();
    let first = client.create_drawable_from_template(&template).unwrap();
    let second = client.create_drawable_from_template(&template).unwrap();
    let (mut node, position) = client
        .drawables
        .take_native_drawable_for_load(first)
        .unwrap();
    assert_eq!(position, 1);
    assert_eq!(client.drawables.native_drawable_order, [second]);
    let decoded = DrawableId(177);
    node.set_id(decoded);
    client
        .drawables
        .restore_native_drawable_after_load(decoded, node, position);
    assert_eq!(client.drawables.native_drawable_order, [second, decoded]);
    assert!(!client.drawables.drawable_map.contains_key(&first));
    assert_eq!(
        client
            .drawables
            .drawable_map
            .get(&decoded)
            .unwrap()
            .get_id(),
        decoded
    );
}
