//! Real native Draw selection, material ordering and receiving particle lifetime.
use super::receiving_native_draw_tests::{authored_template, static_asset, visible_emitter_manager};
use super::*;
use crate::drawable::drawable::StealthLook;
use crate::effects::particle_manager::ParticleSystemManager;
use crate::effects::particle_system::{Particle, ParticleInfo};

fn region(lo: [f32; 3], hi: [f32; 3]) -> NativeDrawView {
    NativeDrawView {
        region: crate::display::client_draw_schedule::ViewAabb { lo, hi },
    }
}
fn receiver(client: &mut GameClient, id: DrawableId) -> &mut BasicDrawable {
    client
        .find_drawable_by_id_mut(id)
        .unwrap()
        .as_any_mut()
        .downcast_mut()
        .unwrap()
}
fn opacity(client: &GameClient, id: DrawableId) -> f32 {
    client
        .find_drawable_by_id(id)
        .unwrap()
        .as_any()
        .downcast_ref::<BasicDrawable>()
        .unwrap()
        .material_pass_opacity()
}
fn frames(client: &mut GameClient, logic_frame: u32, client_frame: u32) -> NativeParticleFrames {
    client.set_frame(client_frame);
    NativeParticleFrames {
        logic_frame,
        client_frame,
    }
}
fn authored(client: &mut GameClient, position: Vector3, hidden: bool) -> DrawableId {
    let id = client
        .create_drawable_from_template(&authored_template())
        .unwrap();
    let draw = receiver(client, id);
    draw.set_position(position);
    draw.set_hidden_by_stealth(hidden);
    draw.set_second_material_pass_opacity(1.);
    id
}
fn direct(client: &mut GameClient, epoch: u64, position: [f32; 3]) -> NativeMaterialObservation {
    client
        .sync_presentation_drawables([super::tests::presentation_drawable_sync_for_test(
            41,
            epoch,
            "DirectViewMaterial",
            true,
            false,
            position,
            0.,
        )])
        .unwrap();
    let binding_key = client
        .presentation_direct_drawable_state(epoch, 41)
        .unwrap()
        .binding_key;
    let fact = NativeMaterialObservation {
        binding_key,
        effectively_dead: false,
        frenzy: false,
        look: StealthLook::None,
        is_disguised: false,
        is_mine: false,
        friendly_opacity: 0.5,
        pending_opacity: Some(1.),
    };
    client.prepare_presentation_materials(&[fact]).unwrap();
    fact
}

#[test]
fn native_views_filter_actual_factory_draws_and_repeat_overlap_without_touching_peer() {
    let assets = static_asset();
    let mut client = GameClient::new().unwrap();
    let mut peer = GameClient::new().unwrap();
    let inside = authored(&mut client, Vector3::new(0., 0., 0.), false);
    let outside = authored(&mut client, Vector3::new(50., 0., 0.), false);
    let hidden = authored(&mut client, Vector3::new(2., 0., 0.), true);
    let later_inside = authored(&mut client, Vector3::new(0.5, 0., 0.), false);
    assert_eq!(authored(&mut peer, Vector3::new(0., 0., 0.), false), inside);
    assert_eq!(
        authored(&mut peer, Vector3::new(50., 0., 0.), false),
        outside
    );
    assert_eq!(authored(&mut peer, Vector3::new(2., 0., 0.), true), hidden);
    assert_eq!(
        authored(&mut peer, Vector3::new(0.5, 0., 0.), false),
        later_inside
    );
    // Direct raw Y-up [0,90,0] belongs to original Z-up [0,0,90].
    let fact = direct(&mut client, 71, [0., 90., 0.]);
    let peer_fact = direct(&mut peer, 72, [90.; 3]);
    assert_eq!(
        fact.binding_key.drawable_id,
        peer_fact.binding_key.drawable_id
    );
    let mut particles = visible_emitter_manager();
    let peer_particles = visible_emitter_manager();
    let views = [
        region([-5., -5., -1.], [5., 5., 100.]),
        region([-1., -1., -1.], [1., 1., 100.]),
    ];
    let clock = frames(&mut client, 3, 29);
    client
        .run_native_views_with_services(
            &mut particles,
            Some(&assets),
            &[],
            clock,
            &[fact],
            &views,
            NativeDrawPhase {
                draw_drawables: true,
                update_particles: false,
            },
        )
        .unwrap();
    assert_eq!(opacity(&client, inside), 0.8 * 0.8);
    assert_eq!(
        opacity(&client, hidden),
        0.8,
        "in-region hidden Draw still fades before return"
    );
    assert_eq!(
        opacity(&client, outside),
        1.,
        "outside-region Draw does not run"
    );
    assert_eq!(opacity(&client, fact.binding_key.drawable_id), 0.8 * 0.8);
    assert_eq!(opacity(&peer, inside), 1.);
    assert_eq!(opacity(&peer, peer_fact.binding_key.drawable_id), 1.);
    assert_eq!(peer_particles.system_count(), 0);
    assert_eq!(
        particles.system_count(),
        4,
        "only two visible in-region actual factory models install root/slave chains"
    );
    assert_eq!(
        receiver(&mut client, inside).native_state_particle_count(),
        1
    );
    assert_eq!(
        receiver(&mut client, hidden).native_state_particle_count(),
        0
    );
    assert_eq!(
        receiver(&mut client, outside).native_state_particle_count(),
        0
    );
    let roots: Vec<_> = particles
        .all_particle_systems()
        .filter(|system| system.template().name() == "NativeBoneSmoke")
        .map(|system| system.attached_drawable_id())
        .collect();
    assert_eq!(
        roots,
        [later_inside, inside],
        "actual emitter creation follows native constructor roster"
    );
    let root = particles
        .all_particle_systems()
        .find(|system| {
            system.template().name() == "NativeBoneSmoke" && system.attached_drawable_id() == inside
        })
        .unwrap()
        .system_id();
    let info = ParticleInfo {
        lifetime: 100,
        size: 1.,
        ..Default::default()
    };
    particles
        .admit_particle(root, Particle::new(&info, 424_242, 29))
        .unwrap();
    let clock = frames(&mut client, 4, 30);
    client
        .run_native_views_with_services(
            &mut particles,
            Some(&assets),
            &[],
            clock,
            &[fact],
            &views,
            NativeDrawPhase {
                draw_drawables: true,
                update_particles: true,
            },
        )
        .unwrap();
    let lifetime = |manager: &ParticleSystemManager| {
        manager
            .find_particle_system(root)
            .unwrap()
            .particles()
            .iter()
            .find(|particle| particle.personality == 424_242)
            .unwrap()
            .lifetime_left
    };
    assert_eq!(
        lifetime(&particles),
        99,
        "manager integrates actual particle once after both views"
    );
    assert_eq!(
        particles.find_particle_system(root).unwrap().position(),
        glam::Vec3::new(2., 3., 4.)
    );
    assert_eq!(opacity(&client, inside), 0.8 * 0.8 * 0.8 * 0.8);
    let before = opacity(&client, inside);
    let clock = frames(&mut client, 5, 31);
    client
        .run_native_views_with_services(
            &mut particles,
            Some(&assets),
            &[],
            clock,
            &[],
            &[],
            NativeDrawPhase {
                draw_drawables: false,
                update_particles: true,
            },
        )
        .unwrap();
    assert_eq!(
        opacity(&client, inside),
        before,
        "no Draw means no material advance"
    );
    assert_eq!(
        lifetime(&particles),
        98,
        "frozen Draw still allows separately admitted manager update"
    );
    let clock = frames(&mut client, 6, 32);
    client
        .run_native_views_with_services(
            &mut particles,
            Some(&assets),
            &[],
            clock,
            &[],
            &views,
            NativeDrawPhase {
                draw_drawables: false,
                update_particles: false,
            },
        )
        .unwrap();
    assert_eq!(opacity(&client, inside), before);
    assert_eq!(lifetime(&particles), 98);
    client.retire_native_drawable_particles_with_services(None, &mut particles);
}

#[test]
fn native_views_empty_view_list_does_not_advance_draw_or_install_emitters() {
    let assets = static_asset();
    let mut client = GameClient::new().unwrap();
    let id = authored(&mut client, Vector3::new(0., 0., 0.), false);
    let mut particles = visible_emitter_manager();
    let clock = frames(&mut client, 1, 0);
    client
        .run_native_views_with_services(
            &mut particles,
            Some(&assets),
            &[],
            clock,
            &[],
            &[],
            NativeDrawPhase {
                draw_drawables: true,
                update_particles: true,
            },
        )
        .unwrap();
    assert_eq!(opacity(&client, id), 1.);
    assert_eq!(receiver(&mut client, id).native_state_particle_count(), 0);
    assert_eq!(particles.system_count(), 0);
}

#[test]
fn public_split_shell_defers_draw_and_rejects_changed_client_frame() {
    let mut client = GameClient::new().unwrap();
    client.window_manager_owner().shell_handle().init().unwrap();
    client.initialized = true;
    client.set_frame(37);
    let id = client
        .create_drawable_from_template(&ThingTemplate::new())
        .unwrap();
    receiver(&mut client, id).set_second_material_pass_opacity(1.);
    let frames = NativeParticleFrames {
        logic_frame: 37,
        client_frame: 37,
    };
    let prepared = client
        .begin_presentation_shell_with_material_frames(0., frames, &[])
        .unwrap();
    assert!(prepared.display_requested());
    assert_eq!(
        opacity(&client, id),
        1.,
        "Client update cannot perform deferred Draw"
    );
    client
        .finish_presentation_shell_with_native_views(
            prepared,
            &[],
            &[],
            NativeDrawPhase {
                draw_drawables: false,
                update_particles: false,
            },
        )
        .unwrap();
    assert_eq!(
        opacity(&client, id),
        1.,
        "a skipped Display phase cannot fade material"
    );
    assert_eq!(client.get_frame(), 37);
    let prepared = client
        .begin_presentation_shell_with_material_frames(0., frames, &[])
        .unwrap();
    client.set_frame(38);
    assert!(
        client
            .finish_presentation_shell_with_native_views(
                prepared,
                &[],
                &[],
                NativeDrawPhase {
                    draw_drawables: false,
                    update_particles: false,
                }
            )
            .is_err()
    );
    assert_eq!(opacity(&client, id), 1.);
}

#[test]
fn split_shell_rejects_equal_frame_peer_reset_and_superseded_operation() {
    let mut client = GameClient::new().unwrap();
    let mut peer = GameClient::new().unwrap();
    client.window_manager_owner().shell_handle().init().unwrap();
    peer.window_manager_owner().shell_handle().init().unwrap();
    client.initialized = true;
    peer.initialized = true;
    client.set_frame(37);
    peer.set_frame(37);
    let frames = NativeParticleFrames {
        logic_frame: 37,
        client_frame: 37,
    };
    let phase = NativeDrawPhase {
        draw_drawables: false,
        update_particles: false,
    };
    let foreign = client
        .begin_presentation_shell_with_material_frames(0., frames, &[])
        .unwrap();
    assert!(
        peer.finish_presentation_shell_with_native_views(foreign, &[], &[], phase)
            .is_err(),
        "equal frame is not receiving Client identity"
    );
    let stale = client
        .begin_presentation_shell_with_material_frames(0., frames, &[])
        .unwrap();
    let current = client
        .begin_presentation_shell_with_material_frames(0., frames, &[])
        .unwrap();
    assert!(
        client
            .finish_presentation_shell_with_native_views(stale, &[], &[], phase)
            .is_err()
    );
    client
        .finish_presentation_shell_with_native_views(current, &[], &[], phase)
        .unwrap();
    let before_reset = client
        .begin_presentation_shell_with_material_frames(0., frames, &[])
        .unwrap();
    client.invalidate_presentation_drawable_world();
    assert!(
        client
            .finish_presentation_shell_with_native_views(before_reset, &[], &[], phase)
            .is_err(),
        "reset invalidates even an equal-frame continuation"
    );
    let fresh = client
        .begin_presentation_shell_with_material_frames(0., frames, &[])
        .unwrap();
    client
        .finish_presentation_shell_with_native_views(fresh, &[], &[], phase)
        .unwrap();
}

#[test]
fn failed_map_discard_requires_particles_before_unlink_and_preserves_peer() {
    let assets = static_asset();
    let mut client = GameClient::new().unwrap();
    let mut peer = GameClient::new().unwrap();
    let id = authored(&mut client, Vector3::new(0., 0., 0.), false);
    assert_eq!(authored(&mut peer, Vector3::new(0., 0., 0.), false), id);
    let mut particles = visible_emitter_manager();
    let clock = frames(&mut client, 3, 29);
    client
        .run_native_views_with_services(
            &mut particles,
            Some(&assets),
            &[],
            clock,
            &[],
            &[region([-5., -5., -5.], [5., 5., 5.])],
            NativeDrawPhase {
                draw_drawables: true,
                update_particles: false,
            },
        )
        .unwrap();
    assert_eq!(receiver(&mut client, id).native_state_particle_count(), 1);
    let count = particles.system_count();
    assert!(count > 0);
    let original_order = client.drawables.native_drawable_order.clone();
    let result = client
        .drawables
        .borrow_native()
        .try_clear_for_map_attempt(None);
    assert!(
        result.is_err(),
        "missing real service must fail before identity unlink"
    );
    assert_eq!(client.drawables.native_drawable_order, original_order);
    assert!(client.find_drawable_by_id(id).is_some());
    assert_eq!(receiver(&mut client, id).native_state_particle_count(), 1);
    assert_eq!(particles.system_count(), count);
    client
        .drawables
        .borrow_native()
        .try_clear_for_map_attempt(Some(&mut particles))
        .unwrap();
    assert!(client.drawables.native_drawable_order.is_empty());
    assert!(client.find_drawable_by_id(id).is_none());
    assert!(client.drawables.drawable_object_map.is_empty());
    assert!(
        client
            .drawables
            .presentation_direct_drawable_bindings
            .is_empty()
    );
    assert!(client.drawables.owned_laser_order.is_empty());
    assert!(client.drawables.text_bearing_drawables.is_empty());
    assert_eq!(
        particles.system_count(),
        count,
        "C++ destroy keeps registered particles to finish their lifetime"
    );
    assert!(
        particles
            .all_particle_systems()
            .all(|system| system.is_destroyed())
    );
    assert!(peer.find_drawable_by_id(id).is_some());
    assert_eq!(peer.drawables.native_drawable_order, vec![id]);
    // A reused attempt is empty; cleanup never needs a fabricated manager.
    client
        .drawables
        .borrow_native()
        .try_clear_for_map_attempt(None)
        .unwrap();
}

#[test]
fn failed_constructor_retirement_selects_only_its_actual_drawable() {
    let assets = static_asset();
    let mut client = GameClient::new().unwrap();
    let failed = authored(&mut client, Vector3::new(0., 0., 0.), false);
    let peer = authored(&mut client, Vector3::new(1., 0., 0.), false);
    let mut particles = visible_emitter_manager();
    let clock = frames(&mut client, 3, 29);
    client
        .run_native_views_with_services(
            &mut particles,
            Some(&assets),
            &[],
            clock,
            &[],
            &[region([-5., -5., -5.], [5., 5., 5.])],
            NativeDrawPhase {
                draw_drawables: true,
                update_particles: false,
            },
        )
        .unwrap();
    assert_eq!(
        receiver(&mut client, failed).native_state_particle_count(),
        1
    );
    assert_eq!(receiver(&mut client, peer).native_state_particle_count(), 1);
    assert!(
        client
            .drawables
            .borrow_native()
            .try_remove_for_map_attempt(failed, None)
            .is_err()
    );
    assert_eq!(client.drawables.native_drawable_order, vec![peer, failed]);
    client
        .drawables
        .borrow_native()
        .try_remove_for_map_attempt(failed, Some(&mut particles))
        .unwrap();
    assert!(client.find_drawable_by_id(failed).is_none());
    assert!(client.find_drawable_by_id(peer).is_some());
    assert_eq!(client.drawables.native_drawable_order, vec![peer]);
    assert_eq!(receiver(&mut client, peer).native_state_particle_count(), 1);
    let selected: Vec<_> = particles
        .all_particle_systems()
        .filter(|system| system.attached_drawable_id() == failed)
        .collect();
    assert!(!selected.is_empty());
    assert!(selected.iter().all(|system| system.is_destroyed()));
    let surviving: Vec<_> = particles
        .all_particle_systems()
        .filter(|system| system.attached_drawable_id() == peer)
        .collect();
    assert!(!surviving.is_empty());
    assert!(surviving.iter().all(|system| !system.is_destroyed()));
    client
        .drawables
        .borrow_native()
        .try_clear_for_map_attempt(Some(&mut particles))
        .unwrap();
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn client_chunk_replacement_retires_actual_manager_before_roster_clear() {
    if let crate::test_process::TestProcess::ParentVerified = crate::test_process::run_bounded(
        module_path!(),
        "client_chunk_replacement_retires_actual_manager_before_roster_clear",
    ) {
        return;
    }
    use crate::effects::particle_manager::get_particle_system_manager_mut;
    struct Restore(Option<ParticleSystemManager>);
    impl Drop for Restore {
        fn drop(&mut self) {
            *get_particle_system_manager_mut().unwrap() = self.0.take();
        }
    }
    let prior = get_particle_system_manager_mut().unwrap().take();
    let _restore = Restore(prior);
    let bytes = GameClient::new().unwrap().capture_xfer_bytes().unwrap();
    assert!(
        !bytes.is_empty(),
        "exercise real nonempty empty-roster Client chunk"
    );
    let assets = static_asset();
    let mut client = GameClient::new().unwrap();
    let id = authored(&mut client, Vector3::new(0., 0., 0.), false);
    let mut manager = visible_emitter_manager();
    let actual_owner = manager.runtime_owner();
    let mut peer = visible_emitter_manager();
    let definition = std::sync::Arc::new(
        crate::effects::particle_manager::ParticleSystemTemplate::new("PeerSavedRetirement".into()),
    );
    let peer_id = peer.create_particle_system(&definition, false).unwrap();
    let peer_owner = peer.runtime_owner();
    let clock = frames(&mut client, 3, 29);
    client
        .run_native_views_with_services(
            &mut manager,
            Some(&assets),
            &[],
            clock,
            &[],
            &[region([-5., -5., -5.], [5., 5., 5.])],
            NativeDrawPhase {
                draw_drawables: true,
                update_particles: false,
            },
        )
        .unwrap();
    assert_eq!(receiver(&mut client, id).native_state_particle_count(), 1);
    assert!(
        client.restore_from_xfer_bytes(&bytes).is_err(),
        "missing actual receiving manager rejects before unlink"
    );
    assert!(client.find_drawable_by_id(id).is_some());
    assert_eq!(receiver(&mut client, id).native_state_particle_count(), 1);
    assert!(
        manager
            .all_particle_systems()
            .all(|system| !system.is_destroyed())
    );
    *get_particle_system_manager_mut().unwrap() = Some(manager);
    client.restore_from_xfer_bytes(&bytes).unwrap();
    assert!(client.find_drawable_by_id(id).is_none());
    let installed = get_particle_system_manager_mut().unwrap();
    let installed = installed.as_ref().unwrap();
    assert_eq!(installed.runtime_owner(), actual_owner);
    assert!(
        installed.system_count() > 0,
        "C++ destroy preserves registered systems until particles finish"
    );
    assert!(
        installed
            .all_particle_systems()
            .all(|system| system.is_destroyed())
    );
    assert_eq!(peer.runtime_owner(), peer_owner);
    assert!(!peer.find_particle_system(peer_id).unwrap().is_destroyed());
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn native_retirement_releases_particle_guard_before_actual_module_drop() {
    if let crate::test_process::TestProcess::ParentVerified = crate::test_process::run_bounded(
        module_path!(),
        "native_retirement_releases_particle_guard_before_actual_module_drop",
    ) {
        return;
    }
    use crate::effects::particle_manager::{PARTICLE_SYSTEM_MANAGER, get_particle_system_manager_mut};
    use std::sync::atomic::{AtomicBool, Ordering};
    #[derive(Debug)]
    struct DropProbe(Arc<AtomicBool>);
    impl crate::drawable::drawable::DrawModule for DropProbe {
        fn drawable_module_type_index(&self) -> usize {
            1
        }
    }
    impl Drop for DropProbe {
        fn drop(&mut self) {
            // Nonblocking: OLD records false instead of hanging the test.
            self.0.store(
                PARTICLE_SYSTEM_MANAGER.try_write().is_ok(),
                Ordering::SeqCst,
            );
        }
    }
    struct Restore(Option<ParticleSystemManager>);
    impl Drop for Restore {
        fn drop(&mut self) {
            *get_particle_system_manager_mut().unwrap() = self.0.take();
        }
    }
    let prior = get_particle_system_manager_mut().unwrap().take();
    let _restore = Restore(prior);
    for selected_only in [true, false] {
        let assets = static_asset();
        let mut client = GameClient::new().unwrap();
        let id = authored(&mut client, Vector3::new(0., 0., 0.), false);
        let mut manager = visible_emitter_manager();
        let actual_owner = manager.runtime_owner();
        let clock = frames(&mut client, 3, 29);
        client
            .run_native_views_with_services(
                &mut manager,
                Some(&assets),
                &[],
                clock,
                &[],
                &[region([-5., -5., -5.], [5., 5., 5.])],
                NativeDrawPhase {
                    draw_drawables: true,
                    update_particles: false,
                },
            )
            .unwrap();
        assert_eq!(
            receiver(&mut client, id).native_state_particle_count(),
            1,
            "actual native resource forces the guarded retirement phase"
        );
        assert!(manager.system_count() > 0);
        let unlocked = Arc::new(AtomicBool::new(false));
        receiver(&mut client, id)
            .get_draw_modules_mut()
            .push(Box::new(DropProbe(Arc::clone(&unlocked))));
        *get_particle_system_manager_mut().unwrap() = Some(manager);
        NativeParticleRetirementService
            .retire(
                &mut client.drawables.borrow_native(),
                selected_only.then_some(id),
            )
            .unwrap();
        assert!(client.find_drawable_by_id(id).is_none());
        assert!(
            unlocked.load(Ordering::SeqCst),
            "native module Drop must reacquire the actual manager lock"
        );
        let manager = get_particle_system_manager_mut().unwrap();
        let manager = manager.as_ref().unwrap();
        assert_eq!(manager.runtime_owner(), actual_owner);
        assert!(manager.system_count() > 0);
        assert!(
            manager
                .all_particle_systems()
                .all(|system| system.is_destroyed())
        );
    }
}
