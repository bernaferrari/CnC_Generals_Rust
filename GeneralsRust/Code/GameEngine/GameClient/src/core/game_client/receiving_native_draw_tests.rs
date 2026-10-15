use super::*;
use crate::effects::particle_manager::{
    ParticlePriorityType, ParticleSystemManager, ParticleSystemTemplate,
    get_particle_system_manager_mut,
};
use game_engine::common::thing::module::{BaseModuleData, ModuleInterfaceType};
use gamelogic::object::draw::w3d_model_draw::{ModelConditionInfo, ParticleSysBoneInfo};
use ww3d_assets::prototypes::{
    HierarchyPrototype, HlodLodEntry, HlodPrototype, HlodSubObject, MeshPrototype,
};
use ww3d_core::{W3dPivotStruct, W3dTriangleStruct, W3dVectorStruct};

fn static_asset() -> ww3d_assets::AssetManager {
    static_asset_at([2., 3., 4.])
}

fn static_asset_at(offset: [f32; 3]) -> ww3d_assets::AssetManager {
    let mut assets = ww3d_assets::AssetManager::new();
    let mut tree = HierarchyPrototype::new("NativeEmitterTree".into());
    for (name, parent, position) in [("ROOT", -1, [0., 0., 0.]), ("FXBONE", 0, offset)] {
        let mut bytes = [0u8; 16];
        bytes[..name.len()].copy_from_slice(name.as_bytes());
        tree.pivots.push(W3dPivotStruct {
            name: bytes,
            parent_idx: parent,
            translation: W3dVectorStruct {
                x: position[0],
                y: position[1],
                z: position[2],
            },
            euler_angles: W3dVectorStruct {
                x: 0.,
                y: 0.,
                z: 0.,
            },
        });
    }
    tree.num_pivots = 2;
    tree.recompute_bind_transforms();
    assets.add_prototype("NativeEmitterTree".into(), Box::new(tree));
    let mut mesh = MeshPrototype::new("NativeEmitterMesh".into());
    mesh.vertices = [[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]]
        .map(|v| W3dVectorStruct {
            x: v[0],
            y: v[1],
            z: v[2],
        })
        .to_vec();
    mesh.normals = vec![
        W3dVectorStruct {
            x: 0.,
            y: 0.,
            z: 1.
        };
        3
    ];
    mesh.triangles.push(W3dTriangleStruct {
        vindex: [0, 1, 2],
        attributes: 0,
        normal: W3dVectorStruct {
            x: 0.,
            y: 0.,
            z: 1.,
        },
        distance: 0.,
    });
    assets.add_prototype("NativeEmitterMesh".into(), Box::new(mesh));
    assets.add_prototype(
        "NativeEmitterModel".into(),
        Box::new(HlodPrototype {
            name: "NativeEmitterModel".into(),
            hierarchy_name: "NativeEmitterTree".into(),
            version: 1,
            lods: vec![HlodLodEntry {
                max_screen_size: f32::MAX,
                models: vec![HlodSubObject {
                    name: "NativeEmitterMesh".into(),
                    bone_index: 0,
                }],
            }],
            aggregates: vec![],
            proxy_entries: vec![],
        }),
    );
    assert!(
        assets.create_instance("NativeEmitterModel").is_some(),
        "real admitted static model exists"
    );
    assets
}

fn authored_template() -> ThingTemplate {
    let mut state = ModelConditionInfo::new();
    state.model_name = "NativeEmitterModel".into();
    state.particle_sys_bones.push(ParticleSysBoneInfo {
        bone_name: "FXBONE".into(),
        particle_system: "NativeBoneSmoke".into(),
    });
    let mut data = W3DModelDrawModuleData::new();
    data.condition_states.push(state);
    data.default_state = 0;
    let mut template = ThingTemplate::new();
    template.set_template_name("NativeBoneOwner".into());
    template.add_draw_module_info(
        "W3DModelDraw".into(),
        "NativeDraw".into(),
        Arc::new(data),
        ModuleInterfaceType::DRAW,
    );
    template.add_client_update_module_info(
        "AnimatedParticleSysBoneClientUpdate".into(),
        "NativeCU".into(),
        Arc::new(BaseModuleData::new()),
        ModuleInterfaceType::CLIENT_UPDATE,
    );
    template
}

fn emitter_manager() -> ParticleSystemManager {
    let mut manager = ParticleSystemManager::new();
    let mut template = ParticleSystemTemplate::new("NativeBoneSmoke".into());
    template.info_mut().slave_system_name = "NativeBoneSlave".into();
    template.info_mut().priority = ParticlePriorityType::AlwaysRender;
    template.info_mut().lifetime =
        crate::effects::particle_manager::GameClientRandomVariable::new(100., 100.);
    manager.register_template(Arc::new(template));
    let mut child = ParticleSystemTemplate::new("NativeBoneSlave".into());
    child.info_mut().priority = ParticlePriorityType::AlwaysRender;
    child.info_mut().lifetime =
        crate::effects::particle_manager::GameClientRandomVariable::new(100., 100.);
    manager.register_template(Arc::new(child));
    manager
}

#[test]
fn receiving_native_draw_admits_authored_emitter_before_next_client_bone_update() {
    // This is the actual existing native parent boundary. The containing test
    // runs serially/in isolation; local receiver tests cover module ownership.
    get_shell().init().unwrap();
    let assets = static_asset();
    let mut bridge = crate::render_bridge::RenderBridge::new();
    *bridge.asset_manager_mut() = assets;
    let previous_bridge = crate::render_bridge::THE_RENDER_BRIDGE
        .lock()
        .unwrap()
        .replace(bridge);
    let previous_particles = get_particle_system_manager_mut()
        .unwrap()
        .replace(emitter_manager());
    let observed = (|| -> Result<_, String> {
        let mut client = GameClient::new().map_err(|e| e.to_string())?;
        client.initialized = true;
        let id = client
            .create_drawable_from_template(&authored_template())
            .map_err(|e| e.to_string())?;
        client
            .bind_drawable_to_object(id, 77)
            .map_err(|e| e.to_string())?;
        client
            .find_drawable_by_id_mut(id)
            .unwrap()
            .set_position(Vector3::new(10., 20., 30.));
        client.set_frame(37);
        client
            .update_presentation_shell(0.)
            .map_err(|e| e.to_string())?;
        let birth = {
            let manager = get_particle_system_manager_mut().map_err(|e| e.to_string())?;
            let manager = manager.as_ref().unwrap();
            manager
                .all_particle_systems()
                .find(|system| system.template().name() == "NativeBoneSmoke")
                .map(|system| {
                    (
                        system.system_id(),
                        system.start_frame(),
                        system.is_saveable(),
                        system.attached_drawable_id(),
                    )
                })
        };
        let chain_frames = {
            let manager = get_particle_system_manager_mut().map_err(|e| e.to_string())?;
            manager
                .as_ref()
                .unwrap()
                .all_particle_systems()
                .map(|system| (system.template().name().to_string(), system.start_frame()))
                .collect::<Vec<_>>()
        };
        client
            .find_drawable_by_id_mut(id)
            .unwrap()
            .set_position(Vector3::new(40., 50., 60.));
        client.set_frame(38);
        client
            .update_drawables_local(
                0.,
                NativeParticleFrames {
                    logic_frame: 105,
                    client_frame: 38,
                },
            )
            .map_err(|e| e.to_string())?;
        let moved = {
            let manager = get_particle_system_manager_mut().map_err(|e| e.to_string())?;
            manager
                .as_ref()
                .unwrap()
                .all_particle_systems()
                .find(|system| system.template().name() == "NativeBoneSmoke")
                .map(|system| system.position())
        };
        client.destroy_drawable(id).map_err(|e| e.to_string())?;
        let retired = {
            let manager = get_particle_system_manager_mut().map_err(|e| e.to_string())?;
            manager
                .as_ref()
                .unwrap()
                .all_particle_systems()
                .map(|system| system.is_destroyed())
                .collect::<Vec<_>>()
        };
        drop(client);
        Ok((id, birth, moved, chain_frames, retired))
    })();
    *get_particle_system_manager_mut().unwrap() = previous_particles;
    *crate::render_bridge::THE_RENDER_BRIDGE.lock().unwrap() = previous_bridge;
    let (id, birth, moved, chain_frames, retired) = observed.unwrap();
    let (_, frame, saveable, attached) =
        birth.expect("actual authored native Draw phase must create its state emitter");
    assert_eq!(frame, 37, "creation uses actual receiving Client frame");
    assert_eq!(
        chain_frames,
        [
            ("NativeBoneSlave".to_string(), 37),
            ("NativeBoneSmoke".to_string(), 37)
        ],
        "slave ctor observes same actual frame and child-before-parent admission"
    );
    assert!(!saveable);
    assert_eq!(attached, id);
    assert_eq!(
        moved,
        Some(glam::Vec3::new(42., 53., 64.)),
        "next CU updates real installed tracker against receiving world pose"
    );
    assert_eq!(
        retired,
        [true, true],
        "actual Drawable destruction marks installed emitter and slave before another frame/save"
    );
}

fn visible_emitter_manager() -> ParticleSystemManager {
    let mut manager = emitter_manager();
    // Lifetime tests need particles that survive the original visibility gate.
    // Alpha defaults to zero opacity; C++ AlphaTest never retires for opacity.
    for name in ["NativeBoneSmoke", "NativeBoneSlave"] {
        let mut template = (*manager.find_template(name).unwrap()).clone();
        template.info_mut().shader_type =
            crate::effects::particle_manager::ParticleShaderType::AlphaTest;
        manager.register_template(Arc::new(template));
    }
    manager
}

fn local_draw_phase(
    client: &mut GameClient,
    particles: &mut ParticleSystemManager,
    assets: &ww3d_assets::AssetManager,
    logic_frame: u32,
    frame: u32,
    frozen: bool,
) {
    client.set_frame(frame);
    assert!(
        client
            .tick_mapped_drawables_with_particles(
                NativeParticleFrames {
                    logic_frame,
                    client_frame: frame
                },
                0.,
                Some(particles),
                Some(assets)
            )
            .unwrap()
            .is_empty()
    );
    let draw_models = client.admit_native_draw_frame(frozen);
    client
        .run_native_draw_particle_phase_with_services(
            particles,
            Some(assets),
            &[],
            NativeParticleFrames {
                logic_frame,
                client_frame: frame,
            },
            draw_models,
            true,
        )
        .unwrap();
}

#[test]
fn receiving_native_draw_phase_and_retirement_use_equal_id_local_owners() {
    let assets = static_asset();
    let peer_assets = static_asset_at([7., 8., 9.]);
    let mut first = GameClient::new().unwrap();
    let mut peer = GameClient::new().unwrap();
    let id = first
        .create_drawable_from_template(&authored_template())
        .unwrap();
    let peer_id = peer
        .create_drawable_from_template(&authored_template())
        .unwrap();
    first.bind_drawable_to_object(id, 77).unwrap();
    peer.bind_drawable_to_object(peer_id, 77).unwrap();
    assert_eq!(id, peer_id);
    let mut particles = emitter_manager();
    let mut peer_particles = emitter_manager();
    local_draw_phase(&mut first, &mut particles, &assets, 104, 37, false);
    local_draw_phase(&mut peer, &mut peer_particles, &peer_assets, 104, 37, false);
    assert_eq!(particles.system_count(), 2);
    assert_eq!(peer_particles.system_count(), 2);
    assert_eq!(
        particles
            .all_particle_systems()
            .map(|s| s.start_frame())
            .collect::<Vec<_>>(),
        [37, 37]
    );
    assert_eq!(
        peer_particles
            .all_particle_systems()
            .map(|s| s.start_frame())
            .collect::<Vec<_>>(),
        [37, 37]
    );
    assert!(
        !first.admit_native_draw_frame(false),
        "same receiving frame does not run a second Draw phase"
    );
    assert!(
        !peer.admit_native_draw_frame(false),
        "equal peer frame is independently admitted once"
    );
    first
        .find_drawable_by_id_mut(id)
        .unwrap()
        .set_position(Vector3::new(40., 50., 60.));
    peer.find_drawable_by_id_mut(peer_id)
        .unwrap()
        .set_position(Vector3::new(100., 200., 300.));
    first.set_frame(38);
    first
        .tick_mapped_drawables_with_particles(
            NativeParticleFrames {
                logic_frame: 105,
                client_frame: 38,
            },
            0.,
            Some(&mut particles),
            Some(&assets),
        )
        .unwrap();
    peer.set_frame(38);
    peer.tick_mapped_drawables_with_particles(
        NativeParticleFrames {
            logic_frame: 105,
            client_frame: 38,
        },
        0.,
        Some(&mut peer_particles),
        Some(&peer_assets),
    )
    .unwrap();
    let root_position = |manager: &ParticleSystemManager| {
        manager
            .all_particle_systems()
            .find(|s| s.template().name() == "NativeBoneSmoke")
            .unwrap()
            .position()
    };
    assert_eq!(root_position(&particles), glam::Vec3::new(42., 53., 64.));
    assert_eq!(
        root_position(&peer_particles),
        glam::Vec3::new(107., 208., 309.)
    );
    first.set_frame(38);
    assert!(!first.admit_native_draw_frame(true));
    assert!(
        !first.admit_native_draw_frame(false),
        "frozen observation consumes same frame exactly like CPP lastFrame"
    );
    first.set_frame(39);
    assert!(first.admit_native_draw_frame(false));
    let before = particles.capture_xfer_bytes().unwrap();
    let mut failed_candidate = GameClient::new().unwrap();
    assert!(failed_candidate.restore_from_xfer_bytes(&[255]).is_err());
    drop(failed_candidate);
    assert_eq!(particles.capture_xfer_bytes().unwrap(), before);
    peer.retire_native_drawable_particles_with_services(None, &mut peer_particles);
    assert!(
        peer_particles
            .all_particle_systems()
            .all(|s| s.is_destroyed())
    );
    assert_eq!(
        peer_particles.system_count(),
        2,
        "CPP destroy retains registration until normal retirement"
    );
    peer.try_invalidate_presentation_drawable_world().unwrap();
    assert!(peer.find_drawable_by_id(peer_id).is_none());
    assert_eq!(
        particles.capture_xfer_bytes().unwrap(),
        before,
        "peer retirement/invalidation leaves receiving A unchanged"
    );
    assert!(particles.all_particle_systems().all(|s| !s.is_destroyed()));
    first.retire_native_drawable_particles_with_services(None, &mut particles);
    first.try_invalidate_presentation_drawable_world().unwrap();
}

#[test]
fn receiving_native_draw_missing_parent_and_wrap_do_not_select_a_peer() {
    let assets = static_asset();
    let mut client = GameClient::new().unwrap();
    let id = client
        .create_drawable_from_template(&authored_template())
        .unwrap();
    client.bind_drawable_to_object(id, 77).unwrap();
    let mut particles = visible_emitter_manager();
    local_draw_phase(
        &mut client,
        &mut particles,
        &assets,
        104,
        u32::MAX - 1,
        false,
    );
    assert_eq!(particles.system_count(), 2);
    // This missing-parent lifetime fixture explicitly admits live particles;
    // unrelated application FX gating cannot turn retained-node checks vacuous.
    let ids = particles
        .all_particle_systems()
        .map(|system| system.system_id())
        .collect::<Vec<_>>();
    for id in ids {
        let info = crate::effects::particle_system::ParticleInfo {
            lifetime: 100,
            size: 1.,
            ..Default::default()
        };
        particles
            .admit_particle(
                id,
                crate::effects::particle_system::Particle::new(&info, 99, u32::MAX - 1),
            )
            .unwrap();
    }
    client.set_frame(u32::MAX);
    assert!(client.admit_native_draw_frame(false));
    client.set_frame(0);
    assert!(
        client.admit_native_draw_frame(false),
        "equality-based phase admission crosses unsigned frame wrap"
    );
    particles.update_with_receiving_drawables(0, 105, 0, &|_| None);
    assert_eq!(
        particles.system_count(),
        2,
        "live particles keep destroyed systems registered until original retirement"
    );
    assert!(
        particles
            .all_particle_systems()
            .all(|system| system.is_destroyed()),
        "missing receiving Drawable destroys root and slave, without a foreign fallback"
    );
    client.retire_native_drawable_particles_with_services(None, &mut particles);
    client.try_invalidate_presentation_drawable_world().unwrap();
}

#[test]
fn receiving_native_frozen_draw_still_advances_particles_on_actual_logic_clock() {
    let assets = static_asset();
    let mut client = GameClient::new().unwrap();
    let id = client
        .create_drawable_from_template(&authored_template())
        .unwrap();
    client.bind_drawable_to_object(id, 77).unwrap();
    let mut particles = visible_emitter_manager();
    local_draw_phase(&mut client, &mut particles, &assets, 104, 37, false);
    let root = particles
        .all_particle_systems()
        .find(|s| s.template().name() == "NativeBoneSmoke")
        .unwrap()
        .system_id();
    let info = crate::effects::particle_system::ParticleInfo {
        lifetime: 100,
        size: 1.,
        ..Default::default()
    };
    particles
        .admit_particle(
            root,
            crate::effects::particle_system::Particle::new(&info, 1_000_000, 37),
        )
        .unwrap();
    local_draw_phase(&mut client, &mut particles, &assets, 105, 38, true);
    let lifetime = |manager: &ParticleSystemManager| {
        manager
            .find_particle_system(root)
            .unwrap()
            .particles()
            .iter()
            .find(|p| p.personality == 1_000_000)
            .unwrap()
            .lifetime_left
    };
    assert_eq!(
        lifetime(&particles),
        99,
        "CPP Display1825 updates manager even when Draw/sync time freezes"
    );
    assert_eq!(
        particles.system_count(),
        2,
        "frozen Draw cannot create another state-emitter chain"
    );
    local_draw_phase(&mut client, &mut particles, &assets, 105, 38, true);
    assert_eq!(
        lifetime(&particles),
        99,
        "manager deduplicates actual logic frame independently of Client Draw observation"
    );
    assert_eq!(
        particles.find_particle_system(root).unwrap().start_frame(),
        37
    );
    client.retire_native_drawable_particles_with_services(None, &mut particles);
    client.try_invalidate_presentation_drawable_world().unwrap();
}

#[test]
fn receiving_native_initial_client_zero_uses_actual_nonzero_logic_dedup() {
    let assets = static_asset();
    let mut client = GameClient::new().unwrap();
    let id = client
        .create_drawable_from_template(&authored_template())
        .unwrap();
    client.bind_drawable_to_object(id, 77).unwrap();
    let mut particles = visible_emitter_manager();
    local_draw_phase(&mut client, &mut particles, &assets, 1, 0, false);
    let root = particles
        .all_particle_systems()
        .find(|s| s.template().name() == "NativeBoneSmoke")
        .unwrap();
    assert_eq!(root.start_frame(), 0);
    assert!(
        !root.is_destroyed(),
        "initial native Drawable resolves through receiving Client even though Core registry has no object77"
    );
    let use_fx = game_engine::common::ini::ini_game_data::get_global_data()
        .map(|data| data.read().use_fx)
        .unwrap_or(true);
    assert_eq!(
        root.particle_count(),
        usize::from(use_fx),
        "initial manager dedup uses actual Logic1 while individual particles use Client0"
    );
    assert!(
        root.particles()
            .iter()
            .all(|particle| particle.create_timestamp == 0)
    );
    client.retire_native_drawable_particles_with_services(None, &mut particles);
    client.try_invalidate_presentation_drawable_world().unwrap();
}

#[test]
fn receiving_native_real_factory_failure_stops_before_later_authored_rows() {
    let assets = static_asset();
    let mut state = ModelConditionInfo::new();
    state.model_name = "NativeEmitterModel".into();
    for name in ["NativeBoneCycle", "NativeBoneLater"] {
        state.particle_sys_bones.push(ParticleSysBoneInfo {
            bone_name: "FXBONE".into(),
            particle_system: name.into(),
        });
    }
    let mut data = W3DModelDrawModuleData::new();
    data.condition_states.push(state);
    data.default_state = 0;
    let mut template = ThingTemplate::new();
    template.set_template_name("NativeCreationError".into());
    template.add_draw_module_info(
        "W3DModelDraw".into(),
        "ErrorDraw".into(),
        Arc::new(data),
        ModuleInterfaceType::DRAW,
    );
    let mut client = GameClient::new().unwrap();
    let id = client.create_drawable_from_template(&template).unwrap();
    client.bind_drawable_to_object(id, 77).unwrap();
    client.set_frame(37);
    let mut particles = ParticleSystemManager::new();
    let mut cycle = ParticleSystemTemplate::new("NativeBoneCycle".into());
    cycle.info_mut().slave_system_name = "NativeBoneCycle".into();
    particles.register_template(Arc::new(cycle));
    particles.register_template(Arc::new(ParticleSystemTemplate::new(
        "NativeBoneLater".into(),
    )));
    assert!(client.admit_native_draw_frame(false));
    let error = client
        .run_native_draw_particle_phase_with_services(
            &mut particles,
            Some(&assets),
            &[],
            NativeParticleFrames {
                logic_frame: 104,
                client_frame: 37,
            },
            true,
            true,
        )
        .unwrap_err();
    assert!(error.to_string().contains("cyclic"));
    assert_eq!(
        particles.system_count(),
        0,
        "later valid row cannot run after actual first factory error"
    );
    assert!(
        !client.admit_native_draw_frame(false),
        "Display observed frame before the failing callback, matching original lastFrame placement"
    );
    client.try_invalidate_presentation_drawable_world().unwrap();
}

#[test]
fn receiving_native_two_drawables_emit_in_cpp_prepend_order() {
    let assets = static_asset();
    let mut client = GameClient::new().unwrap();
    let first = client
        .create_drawable_from_template(&authored_template())
        .unwrap();
    let second = client
        .create_drawable_from_template(&authored_template())
        .unwrap();
    client.bind_drawable_to_object(first, 77).unwrap();
    client.bind_drawable_to_object(second, 3).unwrap();
    assert_eq!(
        client.drawables.native_drawable_order,
        [second, first],
        "GameClient.cpp480 prepends actual constructors, independent of object IDs"
    );
    let mut particles = emitter_manager();
    local_draw_phase(&mut client, &mut particles, &assets, 104, 37, false);
    let roots: Vec<_> = particles
        .all_particle_systems()
        .filter(|system| system.template().name() == "NativeBoneSmoke")
        .map(|system| (system.system_id(), system.attached_drawable_id()))
        .collect();
    assert_eq!(roots.len(), 2);
    assert_eq!(
        roots.iter().map(|entry| entry.1).collect::<Vec<_>>(),
        [second, first],
        "newest actual Drawable consumes the first authored factory IDs and RNG draws"
    );
    assert!(roots[0].0 < roots[1].0);
    client.retire_native_drawable_particles_with_services(Some(second), &mut particles);
    assert!(
        particles
            .find_particle_system(roots[0].0)
            .unwrap()
            .is_destroyed()
    );
    assert!(
        !particles
            .find_particle_system(roots[1].0)
            .unwrap()
            .is_destroyed()
    );
    client.retire_native_drawable_particles_with_services(None, &mut particles);
    client.try_invalidate_presentation_drawable_world().unwrap();
    assert!(client.drawables.native_drawable_order.is_empty());
}

#[test]
fn receiving_native_factory_prefix_failure_cannot_duplicate_on_next_draw() {
    let assets = static_asset();
    let mut state = ModelConditionInfo::new();
    state.model_name = "NativeEmitterModel".into();
    for name in ["NativePrefix", "NativeCycle", "NativeLater"] {
        state.particle_sys_bones.push(ParticleSysBoneInfo {
            bone_name: "FXBONE".into(),
            particle_system: name.into(),
        });
    }
    let mut data = W3DModelDrawModuleData::new();
    data.condition_states.push(state);
    data.default_state = 0;
    let mut template = ThingTemplate::new();
    template.set_template_name("NativePartialCreationError".into());
    template.add_draw_module_info(
        "W3DModelDraw".into(),
        "PrefixDraw".into(),
        Arc::new(data),
        ModuleInterfaceType::DRAW,
    );
    let mut client = GameClient::new().unwrap();
    let id = client.create_drawable_from_template(&template).unwrap();
    client.bind_drawable_to_object(id, 77).unwrap();
    let mut particles = ParticleSystemManager::new();
    for name in ["NativePrefix", "NativeLater"] {
        particles.register_template(Arc::new(ParticleSystemTemplate::new(name.into())));
    }
    let mut cycle = ParticleSystemTemplate::new("NativeCycle".into());
    cycle.info_mut().slave_system_name = "NativeCycle".into();
    particles.register_template(Arc::new(cycle));
    let mut errors = Vec::new();
    for frame in [37, 38] {
        client.set_frame(frame);
        assert!(client.admit_native_draw_frame(false));
        errors.push(
            client
                .run_native_draw_particle_phase_with_services(
                    &mut particles,
                    Some(&assets),
                    &[],
                    NativeParticleFrames {
                        logic_frame: 104 + (frame - 37),
                        client_frame: frame,
                    },
                    true,
                    true,
                )
                .unwrap_err()
                .to_string(),
        );
        let systems: Vec<_> = particles
            .all_particle_systems()
            .map(|system| {
                (
                    system.system_id(),
                    system.template().name().to_owned(),
                    system.attached_drawable_id(),
                    system.start_frame(),
                )
            })
            .collect();
        assert_eq!(
            systems,
            [(1, "NativePrefix".to_owned(), id, 37)],
            "failed later row retains one installed prefix; retry cannot allocate IDs/RNG or later rows"
        );
    }
    assert_eq!(errors[0], errors[1]);
    client.retire_native_drawable_particles_with_services(None, &mut particles);
    assert!(particles.find_particle_system(1).unwrap().is_destroyed());
    client.try_invalidate_presentation_drawable_world().unwrap();
}

#[test]
fn receiving_native_state_bone_scale_is_applied_once_at_birth_and_update() {
    // W3DModelDraw.cpp2592 scales the local bone once; ParticleSys.cpp1862
    // attaches through Drawable.cpp4278's unscaled Thing/Object transform.
    let assets = static_asset();
    let mut client = GameClient::new().unwrap();
    let id = client
        .create_drawable_from_template(&authored_template())
        .unwrap();
    client.bind_drawable_to_object(id, 77).unwrap();
    let drawable = client.find_drawable_by_id_mut(id).unwrap();
    drawable.set_position(Vector3::new(10., 20., 30.));
    drawable.set_instance_scale(2.);
    let mut particles = emitter_manager();
    local_draw_phase(&mut client, &mut particles, &assets, 104, 37, false);
    let position = |manager: &ParticleSystemManager| {
        manager
            .all_particle_systems()
            .find(|system| system.template().name() == "NativeBoneSmoke")
            .unwrap()
            .position()
    };
    let birth = position(&particles);
    local_draw_phase(&mut client, &mut particles, &assets, 105, 38, false);
    let updated = position(&particles);
    client.retire_native_drawable_particles_with_services(None, &mut particles);
    assert_eq!(birth, glam::Vec3::new(14., 26., 38.));
    assert_eq!(
        updated, birth,
        "CU must not apply instance scale a second time"
    );
}

#[path = "receiving_native_capability_tests.rs"]
mod receiving_native_capability_tests;

#[test]
fn receiving_expiry_retires_actual_factory_emitters_through_supplied_manager() {
    let assets = static_asset();
    let mut client = GameClient::new().unwrap();
    let mut peer = GameClient::new().unwrap();
    let next = client
        .create_drawable_from_template(&authored_template())
        .unwrap();
    let first = client
        .create_drawable_from_template(&authored_template())
        .unwrap();
    assert_eq!(
        peer.create_drawable_from_template(&authored_template())
            .unwrap(),
        next
    );
    assert_eq!(
        peer.create_drawable_from_template(&authored_template())
            .unwrap(),
        first
    );
    let mut particles = emitter_manager();
    let mut peer_particles = emitter_manager();
    local_draw_phase(&mut client, &mut particles, &assets, 3, 29, false);
    local_draw_phase(&mut peer, &mut peer_particles, &assets, 3, 29, false);
    let roots: Vec<_> = particles
        .all_particle_systems()
        .filter(|system| system.template().name() == "NativeBoneSmoke")
        .map(|system| {
            (
                system.system_id(),
                system.slave_system_id().unwrap(),
                system.attached_drawable_id(),
            )
        })
        .collect();
    assert_eq!(particles.system_count(), 4, "two actual root/slave chains");
    assert_eq!(roots.len(), 2);
    client
        .find_drawable_by_id_mut(first)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<BasicDrawable>()
        .unwrap()
        .set_expiration_frame(4);
    client.set_frame(30);
    assert_eq!(
        client
            .tick_mapped_drawables_with_particles(
                NativeParticleFrames {
                    logic_frame: 4,
                    client_frame: 30
                },
                0.,
                Some(&mut particles),
                Some(&assets),
            )
            .unwrap(),
        [first]
    );
    assert!(client.find_drawable_by_id(first).is_none());
    assert_eq!(client.drawables.native_drawable_order, [next]);
    for (root, slave, owner) in roots {
        for id in [root, slave] {
            assert_eq!(
                particles.find_particle_system(id).unwrap().is_destroyed(),
                owner == first
            );
            assert!(
                !peer_particles
                    .find_particle_system(id)
                    .unwrap()
                    .is_destroyed()
            );
        }
    }
    client.retire_native_drawable_particles_with_services(None, &mut particles);
    peer.retire_native_drawable_particles_with_services(None, &mut peer_particles);
}
