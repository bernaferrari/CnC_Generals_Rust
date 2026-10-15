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
    first.update_drawables_local(0.0).unwrap();
    assert_eq!(lives(&mut first, id), [1, 1]);
    assert_eq!(lives(&mut peer, peer_id), [0, 0]);
    peer.set_frame(900);
    peer.update_drawables_local(0.0).unwrap();
    first.update_drawables_local(0.0).unwrap();
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
    assert!(
        first
            .tick_mapped_drawables_with_particles(37, 0.0, Some(&mut particles), None)
            .unwrap()
            .is_empty()
    );
    assert!(
        peer.tick_mapped_drawables_with_particles(900, 0.0, Some(&mut peer_particles), None)
            .unwrap()
            .is_empty()
    );
    assert!(
        first
            .tick_mapped_drawables_with_particles(38, 0.0, Some(&mut particles), None)
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
    assert!(
        peer.tick_mapped_drawables_with_particles(901, 0.0, Some(&mut peer_particles), None)
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
    let expired = client
        .tick_mapped_drawables_with_particles(30, 0., Some(&mut particles), None)
        .unwrap();
    assert_eq!(
        expired, expected,
        "CPP GameClient.cpp walks the prepended roster; lookup buckets cannot reorder retirement"
    );
    for id in admitted {
        assert_eq!(
            lives(&mut client, id),
            [1, 1],
            "installed CU modules run before expiration"
        );
        assert_eq!(
            lives(&mut peer, id),
            [0, 0],
            "an equal-ID peer is not advanced"
        );
        assert_eq!(
            client.find_drawable_by_id(id).unwrap().get_tint_color(),
            Vector3::zero(),
            "expiration returns before flashing, as in Drawable.cpp1210"
        );
    }
    // The receiving phase reports actual retirement requests; the outer owner
    // still releases its application service loans before running destruction.
    // Immediate per-node destruction is a separate lifecycle obligation.
    for id in expired {
        client.destroy_drawable(id).unwrap();
    }
    assert!(client.drawables.native_drawable_order.is_empty());
    assert!(client.drawables.drawable_map.is_empty());
    assert_eq!(peer.drawables.native_drawable_order, expected);
    client.try_invalidate_presentation_drawable_world().unwrap();
    peer.try_invalidate_presentation_drawable_world().unwrap();
}
