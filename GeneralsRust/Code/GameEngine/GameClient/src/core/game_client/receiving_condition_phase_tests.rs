//! C++ updateDrawable applies Draw conditions for decals; draw applies them later.
use super::*;
use crate::drawable::drawable::{DrawModule as ClientDrawModule, TerrainDecalType};
use crate::effects::particle_manager::{ParticleSystemManager, get_particle_system_manager_mut};
use game_engine::common::bit_flags::{ModelConditionBitFlags, ModelConditionFlags};
use game_engine::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
use game_engine::common::thing::module::{BaseModuleData, Module, ModuleInterfaceType};
use std::sync::atomic::{AtomicU32, Ordering};

#[derive(Debug)]
struct ConditionProbe(Arc<AtomicU32>);
impl ClientDrawModule for ConditionProbe {
    fn replace_model_condition_state(&mut self, _: &ModelConditionBitFlags) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
#[derive(Debug)]
struct NextCu {
    actual: Box<dyn ClientDrawModule>,
    conditions: Arc<AtomicU32>,
    at_first_cu: Arc<AtomicU32>,
}
impl ClientDrawModule for NextCu {
    fn drawable_module_type_index(&self) -> usize {
        self.actual.drawable_module_type_index()
    }
    fn logic_module_mut(&mut self) -> Option<&mut dyn Module> {
        let _ = self.at_first_cu.compare_exchange(
            u32::MAX,
            self.conditions.load(Ordering::SeqCst),
            Ordering::SeqCst,
            Ordering::SeqCst,
        );
        self.actual.logic_module_mut()
    }
}
fn template() -> ThingTemplate {
    let mut template = ThingTemplate::new();
    template.set_template_name("ReceivingConditionPhase".into());
    template.add_client_update_module_info(
        "AnimatedParticleSysBoneClientUpdate".into(),
        "ActualCU".into(),
        Arc::new(BaseModuleData::new()),
        ModuleInterfaceType::CLIENT_UPDATE,
    );
    template
}
fn basic(client: &mut GameClient, id: DrawableId) -> &mut BasicDrawable {
    client
        .find_drawable_by_id_mut(id)
        .unwrap()
        .as_any_mut()
        .downcast_mut()
        .unwrap()
}
struct Observation {
    at_next_cu: u32,
    after_draw: u32,
    after_skipped_draw: u32,
    after_next_draw: u32,
    peer: u32,
    present: bool,
    fade_elapsed: Option<u32>,
    cu_life: Option<u32>,
    saved_payload: Option<(Vec<u8>, Vec<u8>)>,
}
fn run_public_phase(decal: bool, expires: bool, restore: bool) -> Result<Observation, String> {
    // Existing application boundaries, restored before any test assertions.
    // No fixture serialization mutex or ambient frame mutation is introduced.
    get_shell().init().map_err(|error| error.to_string())?;
    let previous_bridge = crate::render_bridge::THE_RENDER_BRIDGE
        .lock()
        .unwrap()
        .replace(crate::render_bridge::RenderBridge::new());
    let previous_particles = get_particle_system_manager_mut()
        .unwrap()
        .replace(ParticleSystemManager::new());
    let result = (|| {
        let template = template();
        let mut client = GameClient::new().map_err(|error| error.to_string())?;
        client.initialized = true;
        let next = client
            .create_drawable_from_template(&template)
            .map_err(|e| e.to_string())?;
        let id = client
            .create_drawable_from_template(&template)
            .map_err(|e| e.to_string())?;
        let mut peer = GameClient::new().map_err(|error| error.to_string())?;
        let peer_next = peer
            .create_drawable_from_template(&template)
            .map_err(|e| e.to_string())?;
        let peer_id = peer
            .create_drawable_from_template(&template)
            .map_err(|e| e.to_string())?;
        assert_eq!((next, id), (peer_next, peer_id));
        basic(&mut client, id).fade_out(4);
        if expires {
            basic(&mut client, id).set_expiration_frame(4);
        }
        let saved_payload = if restore {
            let mut bytes = Vec::new();
            basic(&mut client, id).xfer(&mut XferSave::new(Cursor::new(&mut bytes), 7))?;
            basic(&mut client, id).xfer(&mut XferLoad::new(Cursor::new(&bytes), 7))?;
            let mut resaved = Vec::new();
            basic(&mut client, id).xfer(&mut XferSave::new(Cursor::new(&mut resaved), 7))?;
            Some((bytes, resaved))
        } else {
            None
        };
        if decal {
            basic(&mut client, id).set_terrain_decal(TerrainDecalType::ShadowTexture);
        }
        // Install after decal setup so CU enters with genuinely pending conditions,
        // independent of the setter's separately tracked callback timing.
        let conditions = Arc::new(AtomicU32::new(0));
        let peer_conditions = Arc::new(AtomicU32::new(0));
        basic(&mut client, id).add_draw_module(Box::new(ConditionProbe(conditions.clone())));
        basic(&mut peer, peer_id)
            .add_draw_module(Box::new(ConditionProbe(peer_conditions.clone())));
        let at_first_cu = Arc::new(AtomicU32::new(u32::MAX));
        let actual = basic(&mut client, next).get_draw_modules_mut().remove(0);
        basic(&mut client, next).add_draw_module(Box::new(NextCu {
            actual,
            conditions: conditions.clone(),
            at_first_cu: at_first_cu.clone(),
        }));
        client.set_frame(30);
        client
            .update_presentation_shell_with_particle_frames(
                0.,
                NativeParticleFrames {
                    logic_frame: 4,
                    client_frame: 30,
                },
            )
            .map_err(|error| error.to_string())?;
        let at_next_cu = at_first_cu.load(Ordering::SeqCst);
        let after_draw = conditions.load(Ordering::SeqCst);
        let present = client.find_drawable_by_id(id).is_some();
        let mut fade_elapsed = None;
        let mut cu_life = None;
        let mut after_skipped_draw = after_draw;
        let mut after_next_draw = after_draw;
        if present {
            fade_elapsed = Some(basic(&mut client, id).time_elapsed_fade());
            cu_life = basic(&mut client, id)
                .get_draw_modules_mut()
                .iter_mut()
                .find_map(|slot| {
                    slot.logic_module_mut()?
                        .as_any_mut()
                        .downcast_mut::<AnimatedParticleSysBoneClientUpdateModule>()
                        .map(|cu| cu.life())
                });
            basic(&mut client, id).set_model_condition_state(ModelConditionFlags::MOVING);
            let mut particles = get_particle_system_manager_mut().unwrap();
            let particles = particles.as_mut().unwrap();
            // The real admission operation rejects a repeated frame and frozen offer.
            assert!(!client.admit_native_draw_frame(false));
            client.set_frame(31);
            assert!(!client.admit_native_draw_frame(true));
            client
                .run_native_draw_particle_phase_with_services(
                    particles,
                    None,
                    &[],
                    NativeParticleFrames {
                        logic_frame: 4,
                        client_frame: 31,
                    },
                    false,
                    false,
                )
                .map_err(|error| error.to_string())?;
            after_skipped_draw = conditions.load(Ordering::SeqCst);
            client.set_frame(32);
            let admitted = client.admit_native_draw_frame(false);
            assert!(admitted);
            client
                .run_native_draw_particle_phase_with_services(
                    particles,
                    None,
                    &[],
                    NativeParticleFrames {
                        logic_frame: 4,
                        client_frame: 32,
                    },
                    admitted,
                    false,
                )
                .map_err(|error| error.to_string())?;
            after_next_draw = conditions.load(Ordering::SeqCst);
        }
        let peer_count = peer_conditions.load(Ordering::SeqCst);
        assert!(peer.find_drawable_by_id(peer_id).is_some());
        client
            .try_invalidate_presentation_drawable_world()
            .map_err(|e| e.to_string())?;
        peer.try_invalidate_presentation_drawable_world()
            .map_err(|e| e.to_string())?;
        Ok(Observation {
            at_next_cu,
            after_draw,
            after_skipped_draw,
            after_next_draw,
            peer: peer_count,
            present,
            fade_elapsed,
            cu_life,
            saved_payload,
        })
    })();
    *get_particle_system_manager_mut().unwrap() = previous_particles;
    *crate::render_bridge::THE_RENDER_BRIDGE.lock().unwrap() = previous_bridge;
    result
}
fn assert_live(observed: &Observation, decal: bool) {
    assert_eq!(
        observed.at_next_cu,
        u32::from(decal),
        "only the decal branch may apply Draw conditions before the next actual CU"
    );
    assert!(observed.present);
    assert_eq!(observed.fade_elapsed, Some(1));
    assert_eq!(observed.cu_life, Some(1));
    assert_eq!(
        observed.after_draw, 1,
        "visible native Draw applies pending conditions once"
    );
    assert_eq!(
        observed.after_skipped_draw, 1,
        "repeated/frozen Draw cannot apply new conditions"
    );
    assert_eq!(
        observed.after_next_draw, 2,
        "next admitted Draw applies the new condition"
    );
    assert_eq!(observed.peer, 0);
}
fn public_no_decal_cu_leaves_conditions_for_visible_draw() {
    assert_live(&run_public_phase(false, false, false).unwrap(), false);
}
fn public_decal_cu_applies_conditions_before_next_cu_not_again_at_draw() {
    assert_live(&run_public_phase(true, false, false).unwrap(), true);
}
fn public_no_decal_expiration_drops_without_draw_condition_callbacks() {
    let observed = run_public_phase(false, true, false).unwrap();
    assert!(!observed.present);
    assert_eq!(
        observed.at_next_cu, 0,
        "expiry must not apply conditions before next CU"
    );
    assert_eq!(
        observed.after_draw, 0,
        "expired owner never reaches Draw or flushes at teardown"
    );
    assert_eq!(observed.peer, 0);
}
fn native_saved_no_decal_fade_continues_at_original_condition_phase() {
    let observed = run_public_phase(false, false, true).unwrap();
    assert_live(&observed, false);
    let (saved, resaved) = observed.saved_payload.unwrap();
    assert_eq!(
        saved, resaved,
        "native CU and fade payload roundtrip before continuation"
    );
}

// One public-shell lifecycle fixture owns the application adapters throughout
// these scenarios. The service-only witness below needs neither publication.
#[test]
fn public_condition_phase_lifecycle() {
    public_no_decal_cu_leaves_conditions_for_visible_draw();
    public_decal_cu_applies_conditions_before_next_cu_not_again_at_draw();
    public_no_decal_expiration_drops_without_draw_condition_callbacks();
    native_saved_no_decal_fade_continues_at_original_condition_phase();
}

#[test]
fn receiving_native_draw_applies_conditions_without_an_earlier_cu() {
    use gamelogic::object::draw::w3d_model_draw::ModelConditionInfo;
    let mut template = template();
    let mut data = W3DModelDrawModuleData::new();
    let mut state = ModelConditionInfo::new();
    state.model_name = "NONE".into();
    data.condition_states.push(state);
    data.default_state = 0;
    template.add_draw_module_info(
        "W3DModelDraw".into(),
        "ActualDraw".into(),
        Arc::new(data),
        ModuleInterfaceType::DRAW,
    );
    let mut client = GameClient::new().unwrap();
    let mut peer = GameClient::new().unwrap();
    let id = client.create_drawable_from_template(&template).unwrap();
    let peer_id = peer.create_drawable_from_template(&template).unwrap();
    assert_eq!(id, peer_id);
    let conditions = Arc::new(AtomicU32::new(0));
    let peer_conditions = Arc::new(AtomicU32::new(0));
    basic(&mut client, id).add_draw_module(Box::new(ConditionProbe(conditions.clone())));
    basic(&mut peer, peer_id).add_draw_module(Box::new(ConditionProbe(peer_conditions.clone())));
    let mut particles = ParticleSystemManager::new();
    client.set_frame(30);
    client
        .run_native_draw_particle_phase_with_services(
            &mut particles,
            None,
            &[],
            NativeParticleFrames {
                logic_frame: 4,
                client_frame: 30,
            },
            true,
            false,
        )
        .unwrap();
    assert_eq!(
        conditions.load(Ordering::SeqCst),
        1,
        "native Draw itself must apply conditions without relying on an earlier CU"
    );
    basic(&mut client, id).set_model_condition_state(ModelConditionFlags::MOVING);
    basic(&mut client, id).set_visible(false);
    client
        .run_native_draw_particle_phase_with_services(
            &mut particles,
            None,
            &[],
            NativeParticleFrames {
                logic_frame: 4,
                client_frame: 30,
            },
            true,
            false,
        )
        .unwrap();
    assert_eq!(
        conditions.load(Ordering::SeqCst),
        1,
        "invisible Draw retains dirty state"
    );
    basic(&mut client, id).set_visible(true);
    client
        .run_native_draw_particle_phase_with_services(
            &mut particles,
            None,
            &[],
            NativeParticleFrames {
                logic_frame: 4,
                client_frame: 30,
            },
            true,
            false,
        )
        .unwrap();
    assert_eq!(conditions.load(Ordering::SeqCst), 2);
    assert_eq!(peer_conditions.load(Ordering::SeqCst), 0);
    assert_eq!(particles.system_count(), 0);
    client.try_invalidate_presentation_drawable_world().unwrap();
    peer.try_invalidate_presentation_drawable_world().unwrap();
}
