//! Original Drawable deadlines use Logic time; flashing uses Client time.
//! These public-shell witnesses also compile against the published old API.
use super::*;
use crate::effects::particle_manager::{ParticleSystemManager, get_particle_system_manager_mut};
use game_engine::common::system::{
    xfer_load::XferLoad as CommonLoad, xfer_save::XferSave as CommonSave,
};
use game_engine::common::thing::module::{BaseModuleData, ModuleInterfaceType};
use gamelogic::object::update::AnimatedParticleSysBoneClientUpdateModule;

fn expiration_template() -> ThingTemplate {
    let mut template = ThingTemplate::new();
    template.set_template_name("ReceivingLogicExpiration".into());
    for tag in ["ExpirationFirst", "ExpirationSecond"] {
        template.add_client_update_module_info(
            "AnimatedParticleSysBoneClientUpdate".into(),
            tag.into(),
            Arc::new(BaseModuleData::new()),
            ModuleInterfaceType::CLIENT_UPDATE,
        );
    }
    template
}

#[derive(Debug)]
struct DrawableObservation {
    lives: Vec<u32>,
    tint: Vector3,
    expiration: u32,
}

fn observe(client: &mut GameClient, id: DrawableId) -> Option<DrawableObservation> {
    let drawable = client
        .find_drawable_by_id_mut(id)?
        .as_any_mut()
        .downcast_mut::<BasicDrawable>()?;
    let tint = drawable.get_tint_color();
    let expiration = drawable.capture_xfer_visuals().expiration_date;
    let lives = drawable
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
        .collect();
    Some(DrawableObservation {
        lives,
        tint,
        expiration,
    })
}

fn set_deadline_and_flash(
    client: &mut GameClient,
    id: DrawableId,
    deadline: u32,
) -> Result<(), String> {
    let drawable = client
        .find_drawable_by_id_mut(id)
        .and_then(|drawable| drawable.as_any_mut().downcast_mut::<BasicDrawable>())
        .ok_or_else(|| "factory-created receiving Drawable is absent".to_string())?;
    drawable.set_expiration_frame(deadline);
    drawable.color_flash(Vector3::new(1., 0., 0.), 4);
    Ok(())
}

fn native_payload(client: &mut GameClient, id: DrawableId) -> Result<Vec<u8>, String> {
    let drawable = client
        .find_drawable_by_id_mut(id)
        .and_then(|drawable| drawable.as_any_mut().downcast_mut::<BasicDrawable>())
        .ok_or_else(|| "native payload source is absent".to_string())?;
    let mut bytes = Vec::new();
    drawable.xfer(&mut CommonSave::new(Cursor::new(&mut bytes), 7))?;
    Ok(bytes)
}

struct ExpirationObservations {
    stages: Vec<Option<DrawableObservation>>,
    peer: Option<DrawableObservation>,
    payload: Option<(Vec<u8>, Vec<u8>)>,
}

fn run_public_expiration(
    restore_native_payload: bool,
    deadline: u32,
    offers: &[NativeParticleFrames],
) -> Result<ExpirationObservations, String> {
    // These are the existing application presentation boundaries, exercised
    // serially. No fixture lock, ambient clock write or substitute UI is added.
    get_shell().init().map_err(|error| error.to_string())?;
    let previous_bridge = crate::render_bridge::THE_RENDER_BRIDGE
        .lock()
        .unwrap()
        .replace(crate::render_bridge::RenderBridge::new());
    let previous_particles = get_particle_system_manager_mut()
        .unwrap()
        .replace(ParticleSystemManager::new());
    let result = (|| {
        let template = expiration_template();
        let mut receiving = GameClient::new().map_err(|error| error.to_string())?;
        receiving.initialized = true;
        let id = receiving
            .create_drawable_from_template(&template)
            .map_err(|error| error.to_string())?;
        let mut peer = GameClient::new().map_err(|error| error.to_string())?;
        let peer_id = peer
            .create_drawable_from_template(&template)
            .map_err(|error| error.to_string())?;
        if id != peer_id {
            return Err("independent constructors did not reuse the same local ID".to_string());
        }
        let payload = if restore_native_payload {
            let mut source = GameClient::new().map_err(|error| error.to_string())?;
            let source_id = source
                .create_drawable_from_template(&template)
                .map_err(|error| error.to_string())?;
            if source_id != id {
                return Err("fresh native receiver identity differs from the saved ID".to_string());
            }
            set_deadline_and_flash(&mut source, source_id, deadline)?;
            let saved = native_payload(&mut source, source_id)?;
            let receiver = receiving
                .find_drawable_by_id_mut(id)
                .and_then(|drawable| drawable.as_any_mut().downcast_mut::<BasicDrawable>())
                .ok_or_else(|| "fresh receiving factory Drawable is absent".to_string())?;
            receiver.xfer(&mut CommonLoad::new(Cursor::new(&saved), 7))?;
            let resaved = native_payload(&mut receiving, id)?;
            source
                .try_invalidate_presentation_drawable_world()
                .map_err(|error| error.to_string())?;
            Some((saved, resaved))
        } else {
            set_deadline_and_flash(&mut receiving, id, deadline)?;
            None
        };
        let mut stages = Vec::new();
        for &frames in offers {
            receiving.set_frame(frames.client_frame);
            receiving
                .update_presentation_shell_with_particle_frames(0., frames)
                .map_err(|error| error.to_string())?;
            stages.push(observe(&mut receiving, id));
        }
        let peer_observation = observe(&mut peer, peer_id);
        receiving
            .try_invalidate_presentation_drawable_world()
            .map_err(|error| error.to_string())?;
        peer.try_invalidate_presentation_drawable_world()
            .map_err(|error| error.to_string())?;
        Ok(ExpirationObservations {
            stages,
            peer: peer_observation,
            payload,
        })
    })();
    *get_particle_system_manager_mut().unwrap() = previous_particles;
    *crate::render_bridge::THE_RENDER_BRIDGE.lock().unwrap() = previous_bridge;
    result
}

fn assert_logic_expiration(observed: &ExpirationObservations) {
    // Drawable.cpp1148/1209 uses Logic time. Client30 alone cannot expire4.
    assert!(
        observed.stages[0].is_some(),
        "deadline4 must survive Logic3 even when the actual Client clock is30"
    );
    let before = observed.stages[0].as_ref().unwrap();
    assert_eq!(before.expiration, 4);
    assert_eq!(before.lives, [1, 1], "actual installed CU modules ran");
    assert!(
        before.tint.x > 0.,
        "Client30 is a flash boundary even though Logic3 is not"
    );
    assert!(
        observed.stages[1].is_some(),
        "a second Client offer at the same Logic3 cannot expire deadline4"
    );
    assert_eq!(observed.stages[1].as_ref().unwrap().lives, [2, 2]);
    assert!(
        observed.stages[2].is_none(),
        "Logic4 retires deadline4 on the actual public shell path"
    );
    let peer = observed.peer.as_ref().expect("same-ID peer remains live");
    assert_eq!(peer.lives, [0, 0]);
    assert_eq!(peer.tint, Vector3::zero());
    assert_eq!(peer.expiration, 0);
}

fn deadline_offers() -> [NativeParticleFrames; 3] {
    [
        NativeParticleFrames {
            logic_frame: 3,
            client_frame: 30,
        },
        NativeParticleFrames {
            logic_frame: 3,
            client_frame: 31,
        },
        NativeParticleFrames {
            logic_frame: 4,
            client_frame: 45,
        },
    ]
}

#[test]
fn public_shell_expiration_uses_logic_clock_and_flash_uses_client_clock() {
    let observed = run_public_expiration(false, 4, &deadline_offers()).unwrap();
    assert_logic_expiration(&observed);
}

#[test]
fn native_saved_expiration_continues_on_receiving_logic_clock() {
    let observed = run_public_expiration(true, 4, &deadline_offers()).unwrap();
    assert_logic_expiration(&observed);
    let (saved, resaved) = observed.payload.unwrap();
    assert_eq!(
        resaved, saved,
        "native Drawable deadline and module payload retain their original bytes"
    );
}

#[test]
fn public_shell_initial_client_zero_uses_actual_logic_one_for_expiration() {
    let observed = run_public_expiration(
        false,
        1,
        &[NativeParticleFrames {
            logic_frame: 1,
            client_frame: 0,
        }],
    )
    .unwrap();
    assert!(
        observed.stages[0].is_none(),
        "the actual Main initial Client0/Logic1 boundary retires Logic deadline1"
    );
    let peer = observed.peer.as_ref().expect("same-ID peer remains live");
    assert_eq!(peer.lives, [0, 0]);
    assert_eq!(peer.expiration, 0);
    assert_eq!(peer.tint, Vector3::zero());
}
