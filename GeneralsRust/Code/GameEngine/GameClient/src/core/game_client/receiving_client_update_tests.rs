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
