//! Native Drawable pose on the receiving view, binding, and particle owners.
use super::*;

#[test]
fn installed_draw_bone_observes_completed_cpu_physics_before_particle_phase() {
    use super::receiving_native_draw_tests::{static_asset, visible_emitter_manager};
    use game_engine::common::thing::module::{ModuleDataHandle, ModuleInterfaceType};
    use gamelogic::object::draw::w3d_model_draw::{ModelConditionInfo, ParticleSysBoneInfo};

    crate::test_support::ensure_ambient_host();
    let assets = static_asset();
    let mut client = GameClient::new().unwrap();
    let mut state = ModelConditionInfo::new();
    state.model_name = "NativeEmitterModel".into();
    state.particle_sys_bones.push(ParticleSysBoneInfo {
        bone_name: "FXBONE".into(),
        particle_system: "NativeBoneSmoke".into(),
    });
    let mut data = W3DModelDrawModuleData::new();
    data.condition_states.push(state);
    data.default_state = 0;
    data.particles_attached_to_animated_bones = true;
    let mut template = ThingTemplate::new();
    template.set_template_name("CompletedNativePhysicsBone".into());
    template.add_draw_module_info(
        "W3DModelDraw".into(),
        "NativeDraw".into(),
        ModuleDataHandle::new(data),
        ModuleInterfaceType::DRAW,
    );
    let id = client.create_drawable_from_template(&template).unwrap();
    let mut manager = visible_emitter_manager();
    let draw = client
        .find_drawable_by_id_mut(id)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<BasicDrawable>()
        .unwrap();
    draw.set_position(Vector3::new(10., 0., 20.));
    draw.set_instance_transform(Matrix4::translation(Vector3::new(1., 2., 3.)));
    let local = glam::Mat4::from_translation(glam::Vec3::new(4., 5., 6.));
    draw.prepare_native_state_particles_for_draw(
        &mut manager,
        Some(&assets),
        &[],
        17,
        None,
        Some(local),
    )
    .unwrap();
    let system = manager
        .all_particle_systems()
        .find(|system| system.template().name() == "NativeBoneSmoke")
        .unwrap();
    // Main world parent (15, 7, 29) is converted to C++ Z-up before adding
    // the installed model's real (2, 3, 4) bone.
    assert_eq!(system.position(), glam::Vec3::new(17., 32., 11.));
    assert_eq!(system.attached_drawable_id(), id);
    assert_eq!(
        draw.native_particle_parent_transform()
            .transform_point3(glam::Vec3::ZERO),
        glam::Vec3::new(10., 0., 20.),
        "ordinary attachment queries still observe the Object transform",
    );
}

#[test]
fn native_cpu_draw_pose_follows_view_order_and_binding_lifetime() {
    use super::tests::presentation_drawable_sync_for_test;
    use crate::drawable::drawable::StealthLook;

    crate::test_support::ensure_ambient_host();
    let mut client = GameClient::new().unwrap();
    let ids = [41, 42];
    let sync = |id, epoch, name, x| {
        presentation_drawable_sync_for_test(id, epoch, name, true, false, [x, 0., 0.], 0.)
    };
    client
        .sync_presentation_drawables([
            sync(ids[0], 71, "PhysicsFirst", 0.),
            sync(ids[1], 71, "PhysicsSecond", 3.),
        ])
        .unwrap();
    let key = |client: &GameClient, id, epoch| {
        client
            .presentation_direct_drawable_state(epoch, id)
            .unwrap()
            .binding_key
    };
    let [first, second] = ids.map(|id| key(&client, id, 71));
    let observations = [first, second].map(|key| NativeMaterialObservation {
        binding_key: key,
        effectively_dead: false,
        frenzy: false,
        look: StealthLook::None,
        is_disguised: false,
        is_mine: false,
        friendly_opacity: 0.5,
        pending_opacity: None,
    });
    let view = |x| NativeDrawView {
        region: crate::display::client_draw_schedule::ViewAabb {
            lo: [-1., -1., -1.],
            hi: [x, 1., 1.],
        },
    };
    let mut draws = Vec::new();
    let mut advance = |binding| {
        draws.push(binding);
        Some(glam::Mat4::from_translation(
            glam::Vec3::X * draws.len() as f32,
        ))
    };
    client.set_frame(17);
    client
        .draw_native_views_with_receiving_state(
            NativeParticleFrames {
                logic_frame: 18,
                client_frame: 17,
            },
            &observations,
            &[view(4.), view(1.)],
            NativeDrawPhase {
                draw_drawables: true,
                update_particles: false,
            },
            None,
            None,
            Some(&mut advance),
        )
        .unwrap();
    assert_eq!(draws, [second, first, first]);
    assert_eq!(
        client
            .borrow_native_drawable_owner()
            .native_draw_mesh_pose(first)
            .unwrap()
            .1,
        Some(glam::Mat4::from_translation(glam::Vec3::X * 3.)),
    );
    client
        .sync_presentation_drawables([sync(ids[0], 72, "RecreatedPhysicsFirst", 0.)])
        .unwrap();
    let recreated = key(&client, ids[0], 72);
    assert_ne!(recreated, first);
    let owner = client.borrow_native_drawable_owner();
    assert!(owner.native_draw_mesh_pose(first).is_err());
    assert_eq!(owner.native_draw_mesh_pose(recreated).unwrap().1, None);
}
