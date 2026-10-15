//! Real native Draw selection, material ordering and receiving particle lifetime.
use super::receiving_native_draw_tests::{
    authored_template, static_asset, visible_emitter_manager,
};
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
    get_shell().init().unwrap();
    let mut client = GameClient::new().unwrap();
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
    get_shell().init().unwrap();
    let mut client = GameClient::new().unwrap();
    let mut peer = GameClient::new().unwrap();
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
