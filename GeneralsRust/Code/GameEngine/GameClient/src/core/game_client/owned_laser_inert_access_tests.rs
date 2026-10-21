// Original CU access is inert; only the actual Draw phase flushes conditions.
use super::*;
use crate::drawable::drawable::DrawModule as ClientDrawModule;
use crate::effects::particle_manager::ParticleSystemManager;
use game_engine::common::bit_flags::{
    ModelConditionBitFlags, ModelConditionFlags, create_model_condition_flags,
};
use game_engine::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
use std::io::Cursor;
use std::sync::atomic::{AtomicU32, Ordering};

#[derive(Debug)]
struct ConditionProbe(Arc<AtomicU32>);
impl ClientDrawModule for ConditionProbe {
    fn replace_model_condition_state(&mut self, _: &ModelConditionBitFlags) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
struct World {
    client: GameClient,
    particles: ParticleSystemManager,
    random: RandomState,
    id: DrawableId,
}
impl World {
    fn new() -> Self {
        let mut template = ThingTemplate::new();
        template.add_draw_module_info(
            "W3DLaserDraw".into(),
            "Beam".into(),
            Arc::new(W3DLaserDrawModuleData {
                inner_beam_width: 4.,
                outer_beam_width: 8.,
                texture_name: "OwnedBeam".into(),
                ..W3DLaserDrawModuleData::default()
            }),
            ModuleInterfaceType::DRAW,
        );
        template.add_client_update_module_info(
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
        let id = client
            .create_owned_ray(
                &template,
                Coord3D::new(4., 5., 6.),
                Coord3D::new(20., 5., 6.),
            )
            .unwrap();
        let mut particles = ParticleSystemManager::new();
        particles.new_template("Lens".into());
        particles.new_template("Target".into());
        Self {
            client,
            particles,
            random: RandomState::default(),
            id,
        }
    }
    fn basic(&mut self) -> &mut BasicDrawable {
        self.client
            .find_drawable_by_id_mut(self.id)
            .unwrap()
            .as_any_mut()
            .downcast_mut()
            .unwrap()
    }
    fn probe(&mut self) -> Arc<AtomicU32> {
        let count = Arc::new(AtomicU32::new(0));
        self.basic()
            .add_draw_module(Box::new(ConditionProbe(count.clone())));
        count
    }
    fn initialize(&mut self) {
        self.client
            .initialize_owned_laser(
                self.id,
                None,
                None,
                Some(Coord3D::new(4., 5., 6.)),
                Some(Coord3D::new(20., 5., 6.)),
                "",
                -4,
                &mut ClientLaserServices {
                    frame: 17,
                    particles: &mut self.particles,
                    random: &mut self.random,
                    effectively_dead: &|_| Ok(false),
                },
            )
            .unwrap();
    }
    fn update(&mut self, frame: u32) {
        self.client
            .update_owned_laser(
                self.id,
                &mut ClientLaserServices {
                    frame,
                    particles: &mut self.particles,
                    random: &mut self.random,
                    effectively_dead: &|_| Ok(false),
                },
            )
            .unwrap();
    }
    fn destroy(&mut self) {
        self.try_destroy().unwrap();
    }
    fn try_destroy(&mut self) -> GameClientResult<()> {
        self.client.destroy_owned_laser(
            self.id,
            &mut ClientLaserServices {
                frame: 19,
                particles: &mut self.particles,
                random: &mut self.random,
                effectively_dead: &|_| Ok(false),
            },
        )
    }
    fn payload(&mut self) -> Vec<u8> {
        let mut bytes = Vec::new();
        self.client
            .with_owned_laser(self.id, |module| {
                module.xfer(&mut XferSave::new(Cursor::new(&mut bytes), 3))
            })
            .unwrap();
        bytes
    }
    fn geometry(&mut self) -> Vec<game_engine::common::system::SceneLineDesc> {
        self.client
            .owned_laser_geometry(self.id, &|_| Ok(1.), &mut |_, _| None)
            .unwrap()
    }
}

#[test]
fn owned_laser_initialization_keeps_dirty_callbacks_outside_receiving_particle_services() {
    let mut first = World::new();
    let mut peer = World::new();
    assert_eq!(first.id, peer.id);
    let first_probe = first.probe();
    let peer_probe = peer.probe();
    first.initialize();
    assert_eq!(
        first_probe.load(Ordering::SeqCst),
        0,
        "initial CU observation/access must not flush Draw conditions"
    );
    assert_eq!(peer_probe.load(Ordering::SeqCst), 0);
    assert_eq!(first.particles.system_count(), 2);
    assert_eq!(peer.particles.system_count(), 0);
    first.destroy();
    assert_eq!(first_probe.load(Ordering::SeqCst), 0);
}

#[test]
fn owned_laser_update_observation_is_inert_and_does_not_advance_equal_id_peer() {
    let mut first = World::new();
    let mut peer = World::new();
    assert_eq!(first.id, peer.id);
    first.initialize();
    peer.initialize();
    let peer_before = peer.payload();
    let first_probe = first.probe();
    let peer_probe = peer.probe();
    first.update(19);
    assert_eq!(
        first_probe.load(Ordering::SeqCst),
        0,
        "CU update and tracked observation must not flush Draw conditions"
    );
    assert_eq!(peer_probe.load(Ordering::SeqCst), 0);
    assert_eq!(peer.payload(), peer_before);
    assert_eq!(first.particles.system_count(), 2);
    assert_eq!(peer.particles.system_count(), 2);
    first.destroy();
    peer.destroy();
}

#[test]
fn owned_laser_count_and_binding_preserve_dirty_phase_and_native_payload() {
    let mut first = World::new();
    first.initialize();
    let before = first.payload();
    let probe = first.probe();
    let id = first.id;
    let drawable = first.client.find_drawable_by_id_mut(id).unwrap();
    assert_eq!(GameClient::owned_laser_count(drawable.as_mut()), 1);
    GameClient::bind_owned_laser_module(drawable.as_mut(), id);
    assert_eq!(
        probe.load(Ordering::SeqCst),
        0,
        "count/bind must inspect installed CU fields without applying Draw state"
    );
    assert_eq!(first.payload(), before);
    first.destroy();
}

#[test]
fn owned_laser_explicit_destruction_is_inert_and_retires_only_receiving_ids() {
    let mut first = World::new();
    let mut peer = World::new();
    assert_eq!(first.id, peer.id);
    first.initialize();
    peer.initialize();
    let probe = first.probe();
    let peer_probe = peer.probe();
    first.destroy();
    assert_eq!(
        probe.load(Ordering::SeqCst),
        0,
        "explicit CU destruction must not introduce a Draw callback"
    );
    assert!(first.client.find_drawable_by_id(first.id).is_none());
    assert!(
        first
            .particles
            .all_particle_systems()
            .all(|system| system.is_destroyed())
    );
    assert!(peer.client.find_drawable_by_id(peer.id).is_some());
    assert!(
        peer.particles
            .all_particle_systems()
            .all(|system| !system.is_destroyed())
    );
    assert_eq!(peer_probe.load(Ordering::SeqCst), 0);
    peer.destroy();
}

#[test]
fn owned_laser_draw_flushes_once_and_texture_failure_does_not_consume_cu_dirty_state() {
    let mut first = World::new();
    let mut peer = World::new();
    first.initialize();
    peer.initialize();
    let before = first.payload();
    let probe = first.probe();
    let peer_probe = peer.probe();
    let error = first
        .client
        .owned_laser_geometry(first.id, &|_| Err("texture witness".into()), &mut |_, _| {
            None
        })
        .unwrap_err();
    assert!(error.to_string().contains("texture witness"));
    assert_eq!(
        probe.load(Ordering::SeqCst),
        1,
        "original Draw boundary applies pending conditions before texture lookup"
    );
    assert_eq!(
        first.payload(),
        before,
        "failed texture observation precedes CU dirty consumption"
    );
    let lines = first.geometry();
    assert!(!lines.is_empty());
    assert_eq!(lines[0].start.x, 4.);
    assert_eq!(lines[0].end.x, 20.);
    first.geometry();
    assert_eq!(probe.load(Ordering::SeqCst), 1);
    let mut flags = create_model_condition_flags();
    flags.set(ModelConditionFlags::MOVING, true);
    first.basic().replace_model_condition_flags(flags, false);
    first.update(19);
    assert_eq!(probe.load(Ordering::SeqCst), 1);
    first.geometry();
    assert_eq!(probe.load(Ordering::SeqCst), 2);
    assert_eq!(peer_probe.load(Ordering::SeqCst), 0);
    first.destroy();
    peer.destroy();
}

#[test]
fn owned_laser_native_cu_restore_keeps_identity_and_future_geometry() {
    let mut first = World::new();
    let mut restored = World::new();
    assert_eq!(first.id, restored.id);
    first.initialize();
    restored.initialize();
    let bytes = first.payload();
    restored
        .client
        .with_owned_laser(restored.id, |module| {
            module.xfer(&mut XferLoad::new(Cursor::new(&bytes), 3))
        })
        .unwrap();
    assert_eq!(restored.payload(), bytes);
    let first_probe = first.probe();
    let restored_probe = restored.probe();
    for frame in 18..=21 {
        first.update(frame);
        restored.update(frame);
        let a = first.geometry();
        let b = restored.geometry();
        assert_eq!(a.len(), b.len());
        for (a, b) in a.iter().zip(&b) {
            assert_eq!(a.start, b.start);
            assert_eq!(a.end, b.end);
            assert_eq!(a.width, b.width);
        }
        assert_eq!(first.payload(), restored.payload());
    }
    assert_eq!(first_probe.load(Ordering::SeqCst), 1);
    assert_eq!(restored_probe.load(Ordering::SeqCst), 1);
    first.destroy();
    restored.destroy();
}

#[test]
fn owned_laser_ui_failure_preserves_installed_particles_payload_and_receiver() {
    let mut first = World::new();
    let mut peer = World::new();
    first.initialize();
    peer.initialize();
    assert_eq!(first.id, peer.id);
    let before = first.payload();
    let peer_before = peer.payload();
    let probe = first.probe();
    let peer_probe = peer.probe();
    // This is the existing UI concurrency boundary, deliberately poisoned.
    let ui = Arc::new(Mutex::new(
        crate::core::subsystems::InGameUISubsystem::default(),
    ));
    let _ = std::panic::catch_unwind({
        let ui = ui.clone();
        move || {
            let _loan = ui.lock().unwrap();
            panic!("UI failure before owned Laser teardown");
        }
    });
    first.client.subsystem_manager.in_game_ui = Some(ui);
    let error = first.try_destroy().unwrap_err();
    assert!(error.to_string().contains("UI lock poisoned"));
    assert_eq!(
        first.payload(),
        before,
        "failed UI cleanup must not clear actual CU particle slots"
    );
    assert!(first.client.find_drawable_by_id(first.id).is_some());
    assert!(first.client.drawables.owned_laser_order.contains(&first.id));
    assert!(
        first
            .particles
            .all_particle_systems()
            .all(|system| !system.is_destroyed())
    );
    assert_eq!(peer.payload(), peer_before);
    assert!(
        peer.particles
            .all_particle_systems()
            .all(|system| !system.is_destroyed())
    );
    assert_eq!(probe.load(Ordering::SeqCst), 0);
    assert_eq!(peer_probe.load(Ordering::SeqCst), 0);
    first.client.subsystem_manager.in_game_ui = None;
    first.destroy();
    assert!(
        first
            .particles
            .all_particle_systems()
            .all(|system| system.is_destroyed())
    );
    peer.destroy();
}

#[test]
fn map_attempt_clear_retires_owned_laser_without_draw_flush_or_peer_mutation() {
    let mut first = World::new();
    let mut peer = World::new();
    assert_eq!(first.id, peer.id);
    first.initialize();
    peer.initialize();
    let first_probe = first.probe();
    let peer_before = peer.payload();
    assert!(
        first
            .client
            .drawables
            .borrow_native()
            .try_clear_for_map_attempt(None)
            .is_err()
    );
    assert!(first.client.find_drawable_by_id(first.id).is_some());
    assert!(
        first
            .particles
            .all_particle_systems()
            .all(|system| !system.is_destroyed())
    );
    first
        .client
        .drawables
        .borrow_native()
        .try_clear_for_map_attempt(Some(&mut first.particles))
        .unwrap();
    assert!(first.client.find_drawable_by_id(first.id).is_none());
    assert!(
        first
            .particles
            .all_particle_systems()
            .all(|system| system.is_destroyed())
    );
    assert_eq!(first_probe.load(Ordering::SeqCst), 0);
    assert!(peer.client.find_drawable_by_id(peer.id).is_some());
    assert!(
        peer.particles
            .all_particle_systems()
            .all(|system| !system.is_destroyed())
    );
    assert_eq!(peer.payload(), peer_before);
    peer.destroy();
}
