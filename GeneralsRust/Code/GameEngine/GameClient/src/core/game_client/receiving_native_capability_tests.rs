//! Valid missing native services must not stop independent presentation work.
use super::*;
use crate::effects::particle_manager::{GameClientRandomVariable, ParticleShaderType};
use crate::effects::particle_system::{Particle, ParticleInfo};
use gamelogic::object::draw::w3d_model_draw::W3DAnimationInfo;

fn unsupported_template(name: &str, animation: bool, tracks: bool) -> ThingTemplate {
    let mut state = ModelConditionInfo::new();
    state.model_name = "NativeEmitterModel".into();
    state.particle_sys_bones.push(ParticleSysBoneInfo {
        bone_name: "FXBONE".into(),
        particle_system: "NativeBoneSmoke".into(),
    });
    if animation {
        state.animations.push(W3DAnimationInfo::new(
            "NativeEmitterModel.AuthoredAnimation".into(),
            false,
            0.,
        ));
    }
    let mut data = W3DModelDrawModuleData::new();
    data.condition_states.push(state);
    if tracks {
        data.track_file = "AuthoredTankTracks.tga".into();
    }
    let mut template = ThingTemplate::new();
    template.set_template_name(name.into());
    template.add_draw_module_info(
        "W3DModelDraw".into(),
        "UnsupportedNativeDraw".into(),
        Arc::new(data),
        ModuleInterfaceType::DRAW,
    );
    template
}

fn admit_mixed_drawables(client: &mut GameClient) -> [DrawableId; 3] {
    let supported = client
        .create_drawable_from_template(&authored_template())
        .unwrap();
    let animated = client
        .create_drawable_from_template(&unsupported_template("AnimatedEmitter", true, false))
        .unwrap();
    let tracks = client
        .create_drawable_from_template(&unsupported_template("TrackedEmitter", false, true))
        .unwrap();
    assert_eq!(
        client.drawables.native_drawable_order,
        [tracks, animated, supported],
        "unsupported modules precede the supported producer in the actual CPP roster"
    );
    [supported, animated, tracks]
}

fn admit_independent_particle(manager: &mut ParticleSystemManager, frame: u32) -> u32 {
    let mut template = ParticleSystemTemplate::new("IndependentFiniteEffect".into());
    template.info_mut().priority = ParticlePriorityType::AlwaysRender;
    template.info_mut().shader_type = ParticleShaderType::AlphaTest;
    template.info_mut().system_lifetime = 10;
    template.info_mut().burst_count = GameClientRandomVariable::new(0., 0.);
    let id = manager
        .create_particle_system_for_client_frame(&Arc::new(template), false, frame)
        .unwrap();
    manager
        .admit_particle(
            id,
            Particle::new(
                &ParticleInfo {
                    lifetime: 100,
                    ..Default::default()
                },
                1_000_000,
                frame,
            ),
        )
        .unwrap();
    id
}

fn independent_lifetime(manager: &ParticleSystemManager, id: u32) -> u32 {
    manager
        .find_particle_system(id)
        .expect("independent effect remains alive")
        .particles()
        .iter()
        .find(|particle| particle.personality == 1_000_000)
        .expect("actual admitted finite particle remains visible")
        .lifetime_left
}

fn capability_gaps(client: &GameClient) -> Vec<(DrawableId, usize, bool, bool, bool)> {
    client
        .native_particle_capability_gaps
        .iter()
        .map(|gap| {
            (
                gap.drawable_id,
                gap.module_index,
                gap.module_preparation,
                gap.animation_pose,
                gap.terrain_tracks,
            )
        })
        .collect()
}

#[test]
fn valid_missing_native_capabilities_preserve_supported_emitters_and_particle_time() {
    let assets = static_asset();
    let mut client = GameClient::new().unwrap();
    let [supported, animated, tracks] = admit_mixed_drawables(&mut client);
    let mut particles = emitter_manager();
    let independent = admit_independent_particle(&mut particles, 37);
    local_draw_phase(&mut client, &mut particles, &assets, 104, 37, false);
    assert_eq!(
        capability_gaps(&client),
        [
            (tracks, 0, false, false, true),
            (animated, 0, false, true, false)
        ],
        "valid missing services are reported with actual owner/module identity"
    );
    assert_eq!(particles.system_count(), 3);
    assert_eq!(
        particles
            .all_particle_systems()
            .filter(|system| system.attached_drawable_id() == supported)
            .count(),
        1,
        "later supported emitter still runs after both missing-capability modules"
    );
    assert_eq!(independent_lifetime(&particles, independent), 99);
    for id in [animated, tracks] {
        let drawable = client
            .find_drawable_by_id_mut(id)
            .unwrap()
            .as_any_mut()
            .downcast_mut::<BasicDrawable>()
            .unwrap();
        assert_eq!(drawable.native_state_particle_count(), 0);
    }
    local_draw_phase(&mut client, &mut particles, &assets, 104, 38, false);
    assert_eq!(
        independent_lifetime(&particles, independent),
        99,
        "a repeated Logic frame does not age existing FX despite a new Client offer"
    );
    assert_eq!(particles.system_count(), 3, "no duplicate supported birth");
    local_draw_phase(&mut client, &mut particles, &assets, 105, 39, true);
    assert_eq!(
        independent_lifetime(&particles, independent),
        98,
        "CPP Display1825 still advances independent particles when native Draw is frozen"
    );
    client.retire_native_drawable_particles_with_services(None, &mut particles);
    client.try_invalidate_presentation_drawable_world().unwrap();
}

#[test]
fn public_shell_missing_native_capabilities_reaches_actual_hud_and_existing_fx() {
    if crate::core::game_client::global_boundary_fixture::run_in_private_child(
        module_path!(),
        "public_shell_missing_native_capabilities_reaches_actual_hud_and_existing_fx",
    ) {
        return;
    }
    // The public operation still owns these existing application boundaries.
    // This test is run serially; no fixture mutex or alternate renderer is added.
    // A fresh standalone public shell routes to the real local player queue.
    gamelogic::commands::command_queue::get_command_queue_manager()
        .lock()
        .expect("standalone command manager")
        .initialize_player(0)
        .expect("fresh local player command queue");
    get_shell().init().unwrap();
    let mut bridge = crate::render_bridge::RenderBridge::new();
    *bridge.asset_manager_mut() = static_asset();
    let previous_bridge = crate::render_bridge::THE_RENDER_BRIDGE
        .lock()
        .unwrap()
        .replace(bridge);
    let mut manager = emitter_manager();
    let independent = admit_independent_particle(&mut manager, 37);
    let previous_particles = get_particle_system_manager_mut().unwrap().replace(manager);
    let observed = (|| -> Result<_, String> {
        let mut client = GameClient::new().map_err(|error| error.to_string())?;
        client.initialized = true;
        let [supported, animated, tracks] = admit_mixed_drawables(&mut client);
        client.set_frame(37);
        // Actual HUD delivery overwrites this existing last-delivery record.
        // An early particle error leaves it untouched; no UI implementation is faked.
        client.last_live_ingame_hud_draw.messages = 777;
        let result = client
            .update_presentation_shell_with_particle_frames(
                0.,
                NativeParticleFrames {
                    logic_frame: 104,
                    client_frame: 37,
                },
            )
            .map_err(|error| error.to_string());
        let gaps = capability_gaps(&client);
        let hud = client.last_live_ingame_hud_draw();
        let (lifetime, supported_emitters) = {
            let guard = get_particle_system_manager_mut().map_err(|error| error.to_string())?;
            let manager = guard.as_ref().expect("actual public receiving manager");
            (
                independent_lifetime(manager, independent),
                manager
                    .all_particle_systems()
                    .filter(|system| system.attached_drawable_id() == supported)
                    .count(),
            )
        };
        client
            .try_invalidate_presentation_drawable_world()
            .map_err(|error| error.to_string())?;
        drop(client);
        Ok((
            result,
            gaps,
            hud,
            lifetime,
            supported_emitters,
            animated,
            tracks,
        ))
    })();
    *get_particle_system_manager_mut().unwrap() = previous_particles;
    *crate::render_bridge::THE_RENDER_BRIDGE.lock().unwrap() = previous_bridge;
    let (result, gaps, hud, lifetime, supported_emitters, animated, tracks) = observed.unwrap();
    assert!(
        result.is_ok(),
        "valid missing services must not abort shell: {result:?}"
    );
    assert_eq!(
        gaps,
        [
            (tracks, 0, false, false, true),
            (animated, 0, false, true, false)
        ]
    );
    assert_eq!(supported_emitters, 1);
    assert_eq!(
        lifetime, 99,
        "existing independent FX advances in the public route"
    );
    assert_eq!(
        hud.messages, 0,
        "actual draw_live_ingame_hud ran after particle phase and replaced the prior record"
    );
}
