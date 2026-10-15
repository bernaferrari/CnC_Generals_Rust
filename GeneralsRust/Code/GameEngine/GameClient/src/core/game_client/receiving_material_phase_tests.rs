//! Explicit native material admission, Draw clocks, lifetime and Xfer continuation.
use super::*;
use crate::drawable::drawable::{StealthLook, TintStatus};
use crate::effects::particle_manager::{ParticleSystemManager, get_particle_system_manager_mut};
use game_engine::common::system::{Snapshotable, xfer_load::XferLoad, xfer_save::XferSave};

fn client(epoch: u64) -> (GameClient, NativeMaterialObservation) {
    let mut receiver = GameClient::new().unwrap();
    receiver
        .sync_presentation_drawables([super::tests::presentation_drawable_sync_for_test(
            42,
            epoch,
            "MaterialReceiver",
            true,
            false,
            [0.; 3],
            0.,
        )])
        .unwrap();
    let key = receiver
        .presentation_direct_drawable_state(epoch, 42)
        .unwrap()
        .binding_key;
    (
        receiver,
        NativeMaterialObservation {
            binding_key: key,
            effectively_dead: false,
            frenzy: false,
            look: StealthLook::None,
            is_disguised: false,
            is_mine: false,
            friendly_opacity: 0.5,
            pending_opacity: Some(1.0),
        },
    )
}
fn basic(client: &mut GameClient, fact: NativeMaterialObservation) -> &mut BasicDrawable {
    client
        .find_drawable_by_id_mut(fact.binding_key.drawable_id)
        .unwrap()
        .as_any_mut()
        .downcast_mut()
        .unwrap()
}
fn opacity(client: &GameClient) -> f32 {
    client.capture_presentation_material_opacities()[0].1
}

#[test]
fn receiving_material_draw_is_explicit_previsibility_and_instance_isolated() {
    let (mut receiver, mut fact) = client(11);
    let (mut peer, peer_fact) = client(12);
    assert_eq!(
        fact.binding_key.drawable_id,
        peer_fact.binding_key.drawable_id
    );
    receiver.prepare_presentation_materials(&[fact]).unwrap();
    peer.prepare_presentation_materials(&[peer_fact]).unwrap();
    basic(&mut receiver, fact).set_visible(false);
    basic(&mut receiver, fact).update_after_client_modules(1);
    assert_eq!(opacity(&receiver), 1.0);
    assert!(receiver.advance_native_material_draw(&[peer_fact]).is_err());
    assert!(receiver.advance_native_material_draw(&[]).is_err());
    assert_eq!(
        opacity(&receiver),
        1.0,
        "invalid batches leave owner untouched"
    );
    receiver.advance_native_material_draw(&[fact]).unwrap();
    assert_eq!(opacity(&receiver), 0.8, "hidden native Draw still fades");
    assert_eq!(
        opacity(&peer),
        1.0,
        "equal IDs do not discover a peer world"
    );
    // A Draw never replays an already accepted absolute command.
    receiver.advance_native_material_draw(&[fact]).unwrap();
    assert_eq!(opacity(&receiver), 0.8 * 0.8);
    fact.effectively_dead = true;
    fact.frenzy = true;
    receiver.advance_native_material_draw(&[fact]).unwrap();
    assert_eq!(
        opacity(&receiver),
        0.8 * 0.8,
        "FRENZY precedes effective death"
    );
    fact.frenzy = false;
    receiver.advance_native_material_draw(&[fact]).unwrap();
    assert_eq!(opacity(&receiver), 0.0);
}

#[test]
fn receiving_material_threshold_look_rules_and_native_xfer_continue() {
    let (mut receiver, mut fact) = client(11);
    fact.look = StealthLook::VisibleFriendlyDetected;
    fact.is_disguised = true;
    fact.pending_opacity = Some(0.625);
    receiver.prepare_presentation_materials(&[fact]).unwrap();
    fact.pending_opacity = None;
    fact.look = StealthLook::VisibleFriendly;
    receiver.prepare_presentation_materials(&[fact]).unwrap();
    assert_eq!(
        opacity(&receiver),
        0.625,
        "disguised friendly transition preserves residual"
    );
    receiver.advance_native_material_draw(&[fact]).unwrap();
    assert_eq!(opacity(&receiver), 0.5);
    let mut bytes = Vec::new();
    basic(&mut receiver, fact)
        .xfer(&mut XferSave::new(Cursor::new(&mut bytes), 7))
        .unwrap();
    let (mut restored, restored_fact) = client(12);
    basic(&mut restored, restored_fact)
        .xfer(&mut XferLoad::new(Cursor::new(&bytes), 7))
        .unwrap();
    for _ in 0..5 {
        receiver.advance_native_material_draw(&[fact]).unwrap();
        restored
            .advance_native_material_draw(&[restored_fact])
            .unwrap();
        assert_eq!(opacity(&receiver), opacity(&restored));
    }
    receiver
        .apply_presentation_material_command(fact.binding_key, 0.001)
        .unwrap();
    receiver.advance_native_material_draw(&[fact]).unwrap();
    assert_eq!(opacity(&receiver), 0.0);
    let above = f32::from_bits(0.001f32.to_bits() + 1);
    receiver
        .apply_presentation_material_command(fact.binding_key, above)
        .unwrap();
    receiver.advance_native_material_draw(&[fact]).unwrap();
    assert_eq!(opacity(&receiver), above * 0.8);
    fact.look = StealthLook::VisibleDetected;
    fact.is_mine = true;
    fact.pending_opacity = Some(0.0);
    receiver.prepare_presentation_materials(&[fact]).unwrap();
    assert_eq!(opacity(&receiver), 0.0);
    fact.look = StealthLook::Invisible;
    fact.pending_opacity = Some(0.25);
    receiver.prepare_presentation_materials(&[fact]).unwrap();
    assert_eq!(
        opacity(&receiver),
        0.25,
        "logic's final ordered write follows look metadata"
    );
    assert!(basic(&mut receiver, fact).get_stealth_look() == StealthLook::Invisible);
}

#[test]
fn actual_native_material_shell_obeys_repeat_and_freeze_admission() {
    get_shell().init().unwrap();
    let previous_bridge = crate::render_bridge::THE_RENDER_BRIDGE
        .lock()
        .unwrap()
        .replace(crate::render_bridge::RenderBridge::new());
    let previous_particles = get_particle_system_manager_mut()
        .unwrap()
        .replace(ParticleSystemManager::new());
    let result = (|| -> GameClientResult<Vec<f32>> {
        let (mut receiver, mut fact) = client(11);
        receiver.initialized = true;
        receiver.prepare_presentation_materials(&[fact])?;
        fact.pending_opacity = None;
        basic(&mut receiver, fact).set_visible(false);
        let mut observations = Vec::new();
        receiver.set_frame(30);
        let frames = NativeParticleFrames {
            logic_frame: 1,
            client_frame: 30,
        };
        receiver.update_presentation_shell_with_material_frames(0., frames, &[fact])?;
        observations.push(opacity(&receiver));
        receiver.update_presentation_shell_with_material_frames(0., frames, &[fact])?;
        observations.push(opacity(&receiver));
        receiver.set_frame(31);
        assert!(!receiver.admit_native_draw_frame(true));
        observations.push(opacity(&receiver));
        receiver.set_frame(32);
        receiver.update_presentation_shell_with_material_frames(
            0.,
            NativeParticleFrames {
                logic_frame: 7,
                client_frame: 32,
            },
            &[fact],
        )?;
        observations.push(opacity(&receiver));
        // Retained Core preparation does not own an actual Draw. Its later
        // render entry advances the same receiver once, even when hidden.
        receiver.set_frame(40);
        receiver.apply_presentation_material_command(fact.binding_key, 1.0)?;
        receiver.update_native_draw_particle_phase_with_materials(
            false,
            NativeParticleFrames {
                logic_frame: 8,
                client_frame: 40,
            },
            false,
            &[fact],
        )?;
        observations.push(opacity(&receiver));
        basic(&mut receiver, fact).render(&Matrix4::identity(), &Matrix4::identity());
        observations.push(opacity(&receiver));
        // CU can retire one member of an admitted batch before Draw. The
        // departed owner cannot block the surviving owner's material phase.
        let mut expiry = GameClient::new()?;
        expiry.initialized = true;
        expiry.sync_presentation_drawables([42, 43].map(|id| {
            super::tests::presentation_drawable_sync_for_test(
                id,
                55,
                "ExpiringMaterial",
                true,
                false,
                [0.; 3],
                0.,
            )
        }))?;
        let expiry_facts: Vec<_> = [42, 43]
            .into_iter()
            .map(|id| NativeMaterialObservation {
                binding_key: expiry
                    .presentation_direct_drawable_state(55, id)
                    .unwrap()
                    .binding_key,
                pending_opacity: Some(1.0),
                ..fact
            })
            .collect();
        expiry.prepare_presentation_materials(&expiry_facts)?;
        basic(&mut expiry, expiry_facts[0]).set_expiration_frame(1);
        expiry.set_frame(30);
        expiry.update_presentation_shell_with_material_frames(
            0.,
            NativeParticleFrames {
                logic_frame: 1,
                client_frame: 30,
            },
            &expiry_facts,
        )?;
        observations.push(
            if expiry.presentation_direct_drawable_state(55, 42).is_none() {
                0.0
            } else {
                1.0
            },
        );
        observations.push(opacity(&expiry));
        Ok(observations)
    })();
    *get_particle_system_manager_mut().unwrap() = previous_particles;
    *crate::render_bridge::THE_RENDER_BRIDGE.lock().unwrap() = previous_bridge;
    assert_eq!(
        result.unwrap(),
        [0.8, 0.8, 0.8, 0.8 * 0.8, 1.0, 0.8, 0.0, 0.8]
    );
}

#[test]
fn objectless_material_draw_keeps_core_frenzy_and_threshold_rules() {
    let mut drawable = BasicDrawable::new(DrawableId(7));
    drawable.set_second_material_pass_opacity(1.0);
    drawable.set_tint_status(TintStatus::FRENZY);
    drawable.advance_core_material_draw();
    assert_eq!(drawable.material_pass_opacity(), 1.0);
    drawable.clear_tint_status(TintStatus::FRENZY);
    drawable.advance_core_material_draw();
    assert_eq!(drawable.material_pass_opacity(), 0.8);
}
