use super::*;
use game_engine::common::thing::module::{BaseModuleData, ModuleInterfaceType};
use gamelogic::object::update::AnimatedParticleSysBoneClientUpdateModule;

fn template() -> ThingTemplate {
    let mut template = ThingTemplate::new();
    template.set_template_name("ReceivingAnimatedClient".into());
    for tag in ["AnimatedFirst", "AnimatedSecond"] {
        template.add_client_update_module_info(
            "AnimatedParticleSysBoneClientUpdate".into(),
            tag.into(),
            Arc::new(BaseModuleData::new()),
            ModuleInterfaceType::CLIENT_UPDATE,
        );
    }
    template
}

fn lives(client: &mut GameClient, id: DrawableId) -> Vec<u32> {
    let drawable = client
        .find_drawable_by_id_mut(id)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<BasicDrawable>()
        .unwrap();
    drawable
        .get_draw_modules_mut()
        .iter_mut()
        .filter_map(|slot| {
            slot.logic_module_mut()
                .and_then(|module| {
                    module
                        .as_any_mut()
                        .downcast_mut::<AnimatedParticleSysBoneClientUpdateModule>()
                })
                .map(|module| module.life())
        })
        .collect()
}

#[test]
fn receiving_client_update_uses_installed_modules_with_empty_core_registry() {
    // No foreign Object or Drawable is installed to make the callback appear live.
    let mut first = GameClient::new().unwrap();
    let mut peer = GameClient::new().unwrap();
    let template = template();
    let id = first.create_drawable_from_template(&template).unwrap();
    let peer_id = peer.create_drawable_from_template(&template).unwrap();
    assert_eq!(id, peer_id);
    assert_eq!(lives(&mut first, id), [0, 0]);
    assert_eq!(lives(&mut peer, peer_id), [0, 0]);
    first.set_frame(37);
    first
        .update_drawables_local(
            0.0,
            NativeParticleFrames {
                logic_frame: 37,
                client_frame: 37,
            },
        )
        .unwrap();
    assert_eq!(lives(&mut first, id), [1, 1]);
    assert_eq!(lives(&mut peer, peer_id), [0, 0]);
    peer.set_frame(900);
    peer.update_drawables_local(
        0.0,
        NativeParticleFrames {
            logic_frame: 900,
            client_frame: 900,
        },
    )
    .unwrap();
    first
        .update_drawables_local(
            0.0,
            NativeParticleFrames {
                logic_frame: 37,
                client_frame: 37,
            },
        )
        .unwrap();
    assert_eq!(lives(&mut first, id), [2, 2]);
    assert_eq!(lives(&mut peer, peer_id), [1, 1]);
}

#[test]
fn receiving_client_update_local_manager_kernel_preserves_native_life_semantics() {
    use crate::effects::particle_manager::ParticleSystemManager;
    use game_engine::common::system::{Snapshotable, xfer_load::XferLoad, xfer_save::XferSave};
    use std::io::Cursor;
    let mut first = GameClient::new().unwrap();
    let mut peer = GameClient::new().unwrap();
    let template = template();
    let id = first.create_drawable_from_template(&template).unwrap();
    let peer_id = peer.create_drawable_from_template(&template).unwrap();
    let mut particles = ParticleSystemManager::new();
    let mut peer_particles = ParticleSystemManager::new();
    first.set_frame(37);
    assert!(
        first
            .tick_mapped_drawables_with_particles(
                NativeParticleFrames {
                    logic_frame: 37,
                    client_frame: 37
                },
                0.0,
                Some(&mut particles),
                None
            )
            .unwrap()
            .is_empty()
    );
    peer.set_frame(900);
    assert!(
        peer.tick_mapped_drawables_with_particles(
            NativeParticleFrames {
                logic_frame: 900,
                client_frame: 900
            },
            0.0,
            Some(&mut peer_particles),
            None
        )
        .unwrap()
        .is_empty()
    );
    first.set_frame(38);
    assert!(
        first
            .tick_mapped_drawables_with_particles(
                NativeParticleFrames {
                    logic_frame: 38,
                    client_frame: 38
                },
                0.0,
                Some(&mut particles),
                None
            )
            .unwrap()
            .is_empty()
    );
    assert_eq!(lives(&mut first, id), [2, 2]);
    assert_eq!(lives(&mut peer, peer_id), [1, 1]);
    let mut saved = Vec::new();
    let drawable = first
        .find_drawable_by_id_mut(id)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<BasicDrawable>()
        .unwrap();
    let module = drawable.get_draw_modules_mut()[0]
        .logic_module_mut()
        .unwrap();
    module
        .xfer(&mut XferSave::new(Cursor::new(&mut saved), 1))
        .unwrap();
    assert_eq!(
        saved,
        [1, 1, 1],
        "C++ Animated/ClientUpdate/DrawableModule versions only; life is not serialized"
    );
    let drawable = peer
        .find_drawable_by_id_mut(peer_id)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<BasicDrawable>()
        .unwrap();
    drawable.get_draw_modules_mut()[0]
        .logic_module_mut()
        .unwrap()
        .xfer(&mut XferLoad::new(Cursor::new(saved), 1))
        .unwrap();
    assert_eq!(
        lives(&mut peer, peer_id),
        [1, 1],
        "native load does not overwrite receiving life"
    );
    peer.set_frame(901);
    assert!(
        peer.tick_mapped_drawables_with_particles(
            NativeParticleFrames {
                logic_frame: 901,
                client_frame: 901
            },
            0.0,
            Some(&mut peer_particles),
            None
        )
        .unwrap()
        .is_empty()
    );
    assert_eq!(lives(&mut peer, peer_id), [2, 2]);
    assert_eq!(lives(&mut first, id), [2, 2]);
}

#[test]
fn native_client_update_and_expiration_follow_cpp_creation_roster() {
    use crate::effects::particle_manager::ParticleSystemManager;
    let mut client = GameClient::new().unwrap();
    let mut peer = GameClient::new().unwrap();
    let mut admitted = Vec::new();
    let template = template();
    // A real factory roster with enough nodes to expose hash-table visitation,
    // rather than relying on the incidental order of a two-element table.
    for _ in 0..64 {
        let id = client.create_drawable_from_template(&template).unwrap();
        assert_eq!(peer.create_drawable_from_template(&template).unwrap(), id);
        let drawable = client
            .find_drawable_by_id_mut(id)
            .unwrap()
            .as_any_mut()
            .downcast_mut::<BasicDrawable>()
            .unwrap();
        drawable.set_expiration_frame(30);
        drawable.color_flash(Vector3::new(1., 0., 0.), 4);
        admitted.push(id);
    }
    let expected: Vec<_> = admitted.iter().rev().copied().collect();
    assert_eq!(client.drawables.native_drawable_order, expected);
    let mut particles = ParticleSystemManager::new();
    client.set_frame(30);
    let expired = client
        .tick_mapped_drawables_with_particles(
            NativeParticleFrames {
                logic_frame: 30,
                client_frame: 30,
            },
            0.,
            Some(&mut particles),
            None,
        )
        .unwrap();
    assert_eq!(
        expired, expected,
        "CPP GameClient.cpp walks the prepended roster; lookup buckets cannot reorder retirement"
    );
    for id in admitted {
        assert!(
            client.find_drawable_by_id(id).is_none(),
            "expired node is already retired"
        );
        assert_eq!(
            lives(&mut peer, id),
            [0, 0],
            "equal-ID peer is not advanced"
        );
    }
    // Installed CU-before-retirement is observed through actual module Drop
    // in receiving_retirement_tests; expired values are no longer queryable.
    assert!(client.drawables.native_drawable_order.is_empty());
    assert!(client.drawables.drawable_map.is_empty());
    assert_eq!(peer.drawables.native_drawable_order, expected);
    client.try_invalidate_presentation_drawable_world().unwrap();
    peer.try_invalidate_presentation_drawable_world().unwrap();
}

#[test]
fn receiving_drawable_phase_rejects_foreign_client_frame_before_mutation() {
    let mut client = GameClient::new().unwrap();
    let id = client.create_drawable_from_template(&template()).unwrap();
    client
        .find_drawable_by_id_mut(id)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<BasicDrawable>()
        .unwrap()
        .set_expiration_frame(4);
    client.set_frame(30);
    let foreign = NativeParticleFrames {
        logic_frame: 4,
        client_frame: 31,
    };
    let timing = FrameTiming {
        frame_number: 31,
        delta_time: std::time::Duration::ZERO,
        total_time: std::time::Duration::ZERO,
    };
    for result in [
        client.update_drawables_local(0., foreign),
        client.update_for_rendering(&timing, foreign),
        client.update_drawables(0., foreign),
        client.update_presentation_shell_with_particle_frames(0., foreign),
    ] {
        let error = result.expect_err("foreign Client frame must be rejected");
        assert!(
            error.to_string().contains("Client frame"),
            "wrong rejection: {error}"
        );
    }
    assert_eq!(client.get_frame(), 30);
    assert_eq!(lives(&mut client, id), [0, 0]);
    let drawable = client.find_drawable_by_id(id).unwrap();
    assert!(!drawable.is_expired(3));
    assert!(drawable.is_expired(4));
    assert_eq!(client.drawables.native_drawable_order, [id]);
    client.try_invalidate_presentation_drawable_world().unwrap();
}
