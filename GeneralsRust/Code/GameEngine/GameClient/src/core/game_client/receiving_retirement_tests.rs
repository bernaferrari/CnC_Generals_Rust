//! Actual installed CU lifetime, per-node destruction, and fallible UI teardown.
use super::*;
use crate::drawable::drawable::{DrawModule as ClientDrawModule, TerrainDecalType};
use crate::effects::particle_manager::ParticleSystemManager;
use game_engine::common::thing::module::{BaseModuleData, Module, ModuleInterfaceType};
use std::sync::atomic::{AtomicU32, Ordering};

fn template() -> ThingTemplate {
    let mut template = ThingTemplate::new();
    template.set_template_name("ReceivingRetirement".into());
    template.add_client_update_module_info(
        "AnimatedParticleSysBoneClientUpdate".into(),
        "RetirementCU".into(),
        Arc::new(BaseModuleData::new()),
        ModuleInterfaceType::CLIENT_UPDATE,
    );
    template
}

#[derive(Debug)]
struct ObservedCu {
    actual: Box<dyn ClientDrawModule>,
    retired_life: Arc<AtomicU32>,
}
impl ClientDrawModule for ObservedCu {
    fn drawable_module_type_index(&self) -> usize {
        self.actual.drawable_module_type_index()
    }
    fn logic_module_mut(&mut self) -> Option<&mut dyn Module> {
        self.actual.logic_module_mut()
    }
}
impl Drop for ObservedCu {
    fn drop(&mut self) {
        let life = self
            .actual
            .logic_module_mut()
            .unwrap()
            .as_any_mut()
            .downcast_mut::<AnimatedParticleSysBoneClientUpdateModule>()
            .unwrap()
            .life();
        self.retired_life.store(life, Ordering::SeqCst);
    }
}
#[derive(Debug)]
struct NextCu {
    actual: Box<dyn ClientDrawModule>,
    retired_life: Arc<AtomicU32>,
}
impl ClientDrawModule for NextCu {
    fn drawable_module_type_index(&self) -> usize {
        self.actual.drawable_module_type_index()
    }
    fn logic_module_mut(&mut self) -> Option<&mut dyn Module> {
        assert_eq!(
            self.retired_life.load(Ordering::SeqCst),
            1,
            "previous installed CU must run once and be destroyed before next installed CU callback"
        );
        self.actual.logic_module_mut()
    }
}

#[derive(Debug)]
struct NextVisual {
    retired_life: Arc<AtomicU32>,
    observations: Arc<AtomicU32>,
}
impl ClientDrawModule for NextVisual {
    fn set_terrain_decal_opacity(&mut self, _: f32) {
        assert_eq!(
            self.retired_life.load(Ordering::SeqCst),
            1,
            "previous installed CU must run once and be destroyed before next visual callback"
        );
        self.observations.fetch_add(1, Ordering::SeqCst);
    }
}
fn basic(client: &mut GameClient, id: DrawableId) -> &mut BasicDrawable {
    client
        .find_drawable_by_id_mut(id)
        .unwrap()
        .as_any_mut()
        .downcast_mut()
        .unwrap()
}

fn lives(client: &mut GameClient, id: DrawableId) -> Vec<u32> {
    basic(client, id)
        .get_draw_modules_mut()
        .iter_mut()
        .filter_map(|slot| {
            slot.logic_module_mut()?
                .as_any_mut()
                .downcast_mut::<AnimatedParticleSysBoneClientUpdateModule>()
                .map(|module| module.life())
        })
        .collect()
}

#[test]
fn receiving_expiration_retires_actual_cu_before_next_installed_cu_callback() {
    let mut client = GameClient::new().unwrap();
    let mut peer = GameClient::new().unwrap();
    let template = template();
    let next = client.create_drawable_from_template(&template).unwrap();
    let first = client.create_drawable_from_template(&template).unwrap();
    assert_eq!(peer.create_drawable_from_template(&template).unwrap(), next);
    assert_eq!(
        peer.create_drawable_from_template(&template).unwrap(),
        first
    );
    let retired_life = Arc::new(AtomicU32::new(0));
    let observations = Arc::new(AtomicU32::new(0));
    let actual = basic(&mut client, first).get_draw_modules_mut().remove(0);
    basic(&mut client, first).add_draw_module(Box::new(ObservedCu {
        actual,
        retired_life: retired_life.clone(),
    }));
    basic(&mut client, first).set_expiration_frame(4);
    let actual = basic(&mut client, next).get_draw_modules_mut().remove(0);
    basic(&mut client, next).add_draw_module(Box::new(NextCu {
        actual,
        retired_life: retired_life.clone(),
    }));
    basic(&mut client, next).get_draw_modules_mut().insert(
        0,
        Box::new(NextVisual {
            retired_life: retired_life.clone(),
            observations: observations.clone(),
        }),
    );
    basic(&mut client, next).set_terrain_decal(TerrainDecalType::ShadowTexture);
    basic(&mut client, next).set_terrain_decal_fade_target(1., 0.1);
    client.set_frame(30);
    let mut particles = ParticleSystemManager::new();
    let retired = client
        .tick_mapped_drawables_with_particles(
            NativeParticleFrames {
                logic_frame: 4,
                client_frame: 30,
            },
            0.,
            Some(&mut particles),
            None,
        )
        .unwrap();
    assert_eq!(retired, [first]);
    assert_eq!(retired_life.load(Ordering::SeqCst), 1);
    assert_eq!(observations.load(Ordering::SeqCst), 1);
    assert!(client.find_drawable_by_id(first).is_none());
    assert_eq!(client.drawables.native_drawable_order, [next]);
    assert!(peer.find_drawable_by_id(first).is_some());
    assert_eq!(lives(&mut peer, first), [0]);
    assert_eq!(lives(&mut client, next), [1]);
}

#[test]
fn receiving_expiration_propagates_ui_failure_without_advancing_next_drawable() {
    let mut client = GameClient::new().unwrap();
    let template = template();
    let next = client.create_drawable_from_template(&template).unwrap();
    let first = client.create_drawable_from_template(&template).unwrap();
    basic(&mut client, first).set_expiration_frame(4);
    // Exercise the real concurrent UI adapter's poison error, not a synthetic Result.
    let ui = Arc::new(Mutex::new(
        crate::core::subsystems::InGameUISubsystem::default(),
    ));
    let _ = std::panic::catch_unwind({
        let ui = ui.clone();
        move || {
            let _loan = ui.lock().unwrap();
            panic!("poison receiving UI for error-path witness");
        }
    });
    client.subsystem_manager.in_game_ui = Some(ui.clone());
    client.set_frame(30);
    let mut particles = ParticleSystemManager::new();
    let error = client
        .tick_mapped_drawables_with_particles(
            NativeParticleFrames {
                logic_frame: 4,
                client_frame: 30,
            },
            0.,
            Some(&mut particles),
            None,
        )
        .unwrap_err();
    assert!(error.to_string().contains("UI lock poisoned"));
    assert_eq!(lives(&mut client, first), [1]);
    assert_eq!(lives(&mut client, next), [0]);
    assert_eq!(client.drawables.native_drawable_order, [first, next]);
    assert!(client.find_drawable_by_id(first).is_some());
    client.subsystem_manager.in_game_ui = None;
    client
        .tick_mapped_drawables_with_particles(
            NativeParticleFrames {
                logic_frame: 4,
                client_frame: 30,
            },
            0.,
            Some(&mut particles),
            None,
        )
        .unwrap();
    assert!(client.find_drawable_by_id(first).is_none());
    assert_eq!(lives(&mut client, next), [1]);
}

#[derive(Debug)]
struct ConditionProbe(Arc<AtomicU32>);
impl ClientDrawModule for ConditionProbe {
    fn replace_model_condition_state(
        &mut self,
        _: &game_engine::common::bit_flags::ModelConditionBitFlags,
    ) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn receiving_retirement_rejects_classic_cu_without_flushing_dirty_model() {
    use crate::drawable::drawable::LogicDrawModuleSnapshotAdapter;
    use gamelogic::object::update::laser_update::{LaserUpdateModule, LaserUpdateModuleData};
    let mut client = GameClient::new().unwrap();
    let id = client.create_drawable_from_template(&template()).unwrap();
    let observations = Arc::new(AtomicU32::new(0));
    basic(&mut client, id).add_draw_module(Box::new(ConditionProbe(observations.clone())));
    basic(&mut client, id).add_draw_module(Box::new(
        LogicDrawModuleSnapshotAdapter::client_update_module(
            "ClassicUnsupported",
            Box::new(LaserUpdateModule::new(
                0,
                Arc::new(LaserUpdateModuleData::default()),
                Some(id.0),
            )),
        ),
    ));
    for claimed_membership in [false, true] {
        if claimed_membership {
            client.drawables.owned_laser_order.push(id);
        }
        let error = client.destroy_drawable(id).unwrap_err();
        assert!(error.to_string().contains("classic LaserUpdate"));
        assert_eq!(
            observations.load(Ordering::SeqCst),
            0,
            "retirement preflight must not flush dirty model conditions or discover their services"
        );
        assert!(client.find_drawable_by_id(id).is_some());
        assert_eq!(client.drawables.native_drawable_order, [id]);
    }
}
