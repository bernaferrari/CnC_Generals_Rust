//! FXList ParticleSystemFXNugget apply + ParticleSystemManager LOD spawn.
//!
//! Drives shipped `FXNugget::do_fx_pos` and `ParticleSystemManager::create_particle_system`.

use game_client_rust::effects::fxlist_integration::{FXContext, FXNugget, ParticleSystemFXNugget};
use game_client_rust::effects::particle_manager::{
    GameClientRandomVariable, ParticlePriorityType, ParticleSystemManager,
};
use game_client_rust::effects::particle_presets;
use nalgebra::{Matrix3, Point3};

#[test]
fn fxlist_particle_nugget_trait_apply_spawns_and_honors_delay_and_orient() {
    let mut manager = ParticleSystemManager::new();
    let preset = particle_presets::get_preset_by_name("Fire").expect("Fire preset");
    manager.register_template(preset);

    let mut nugget = ParticleSystemFXNugget::new("Fire".to_string());
    nugget.delay = GameClientRandomVariable::new(1000.0, 1000.0);
    nugget.orient_to_object = true;
    let mtx = Matrix3::new(0.0, -1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0);

    let mut ctx = FXContext {
        particle_manager: &mut manager,
        ray_effect_manager: None,
        decal_manager: None,
        bone_query: None,
        current_frame: 0,
        local_player_index: 0,
    };
    FXNugget::do_fx_pos(
        &nugget,
        Point3::new(10.0, 20.0, 30.0),
        Some(&mtx),
        0.0,
        None,
        0.0,
        &mut ctx,
    );

    assert_eq!(
        manager.active_system_count(),
        1,
        "FXList particle nugget must spawn via trait do_fx_pos"
    );
    let system = manager.all_particle_systems().next().expect("spawned");
    assert_eq!(
        system.initial_delay_left(),
        30,
        "1000ms delay → 30 logic frames (ceil(msec*30/1000))"
    );
    assert_eq!(
        system.local_transform(),
        mtx,
        "orient_to_object copies caller matrix"
    );
}

#[test]
fn fxlist_light_pulse_nugget_creates_display_pulse() {
    use game_client_rust::effects::fxlist_integration::LightPulseFXNugget;
    use game_client_rust::fx_list::drain_display_light_pulses;

    let _ = drain_display_light_pulses();
    let nugget = LightPulseFXNugget {
        color: [0.8, 0.1, 0.1],
        radius: 60.0,
        bounding_circle_pct: 0.0,
        increase_frames: 4,
        decrease_frames: 12,
    };
    let mut manager = ParticleSystemManager::new();
    let mut ctx = FXContext {
        particle_manager: &mut manager,
        ray_effect_manager: None,
        decal_manager: None,
        bone_query: None,
        current_frame: 0,
        local_player_index: 0,
    };
    FXNugget::do_fx_pos(
        &nugget,
        Point3::new(1.0, 2.0, 3.0),
        None,
        0.0,
        None,
        0.0,
        &mut ctx,
    );
    let pulses = drain_display_light_pulses();
    assert_eq!(pulses.len(), 1);
    assert_eq!(pulses[0].inner_radius, 1.0);
    assert_eq!(pulses[0].outer_radius, 60.0);
    assert_eq!(pulses[0].pos, [1.0, 2.0, 3.0]);
}

#[test]
fn particle_manager_create_respects_lod_priority_floor() {
    let mut manager = ParticleSystemManager::new();
    let preset = particle_presets::get_preset_by_name("Fire").expect("Fire preset");
    assert_eq!(preset.info().priority, ParticlePriorityType::Constant);
    manager.register_template(preset);

    manager.set_lod_params(
        2500,
        500,
        ParticlePriorityType::Critical,
        ParticlePriorityType::AlwaysRender,
        0,
    );

    let fire = manager.find_template("Fire").expect("Fire template");
    // C++ createParticleSystem always instantiates; LOD skips createParticle.
    let id = manager
        .create_particle_system(&fire, false)
        .expect("system must exist even when LOD floor is Critical");
    assert_eq!(manager.active_system_count(), 1);

    {
        let system = manager.find_particle_system_mut(id).expect("active");
        system.trigger();
    }
    manager.update(0, 1);
    assert_eq!(
        manager.find_particle_system(id).unwrap().particle_count(),
        0,
        "Constant < Critical must skip individual particles"
    );

    manager.set_lod_params(
        2500,
        500,
        ParticlePriorityType::WeaponExplosion,
        ParticlePriorityType::Critical,
        0,
    );
    {
        let system = manager.find_particle_system_mut(id).expect("active");
        system.trigger();
    }
    manager.update(0, 2);
    assert!(
        manager.find_particle_system(id).unwrap().particle_count() > 0,
        "Fire particles spawn when LOD floor is WeaponExplosion"
    );
}

#[test]
fn finite_lifetime_last_burst_survives_same_tick_create_particle_commit() {
    use game_client_rust::effects::particle_manager::{
        GameClientRandomVariable, ParticleSystemTemplate,
    };
    use std::sync::Arc;

    let mut manager = ParticleSystemManager::new();
    manager.set_lod_params(
        2500,
        500,
        ParticlePriorityType::WeaponExplosion,
        ParticlePriorityType::Critical,
        0,
    );

    let mut template = ParticleSystemTemplate::new("FiniteBurst".to_string());
    {
        let info = template.info_mut();
        info.priority = ParticlePriorityType::Constant;
        info.system_lifetime = 1;
        info.lifetime = GameClientRandomVariable::new(5.0, 5.0);
        info.burst_count = GameClientRandomVariable::new(3.0, 3.0);
        info.burst_delay = GameClientRandomVariable::new(0.0, 0.0);
        info.initial_delay = GameClientRandomVariable::new(0.0, 0.0);
        info.is_one_shot = false;
    }
    let template = Arc::new(template);
    manager.register_template(template.clone());

    let id = manager
        .create_particle_system(&template, false)
        .expect("finite-lifetime system must instantiate");
    {
        let system = manager.find_particle_system_mut(id).expect("active");
        system.trigger();
    }

    manager.update(0, 1);
    let first = manager
        .find_particle_system(id)
        .expect("C++ keeps system until last burst particles die");
    assert!(
        first.particle_count() >= 3,
        "lifetime=1 last burst must commit before remove; got {}",
        first.particle_count()
    );

    manager.update(0, 2);
    assert!(
        manager.find_particle_system(id).is_some(),
        "particles with lifetime 5 still keep the system on the next tick"
    );
}

#[test]
fn particle_gpu_mesh_bake_matches_live_particle_positions() {
    use game_client_rust::effects::bake_particle_system_gpu_mesh;

    let mut manager = ParticleSystemManager::new();
    let preset = particle_presets::get_preset_by_name("Fire").expect("Fire preset");
    manager.register_template(preset);
    manager.set_lod_params(
        2500,
        500,
        ParticlePriorityType::WeaponExplosion,
        ParticlePriorityType::Critical,
        0,
    );
    let fire = manager.find_template("Fire").expect("Fire template");
    let id = manager.create_particle_system(&fire, false).unwrap();
    {
        let system = manager.find_particle_system_mut(id).unwrap();
        system.set_position(Point3::new(12.0, 4.0, -3.0));
        system.trigger();
    }
    manager.update(0, 1);
    let system = manager.find_particle_system(id).unwrap();
    let mesh = bake_particle_system_gpu_mesh(system);
    assert_eq!(mesh.len(), system.particle_count());
    assert!(!mesh.is_empty());
    let live: Vec<_> = system.particles().iter().collect();
    for (vertex, particle) in mesh.iter().zip(live.iter()) {
        assert_eq!(
            vertex.position,
            [
                particle.position.x,
                particle.position.y,
                particle.position.z
            ]
        );
        assert_eq!(vertex.size, [particle.size, particle.size]);
        assert_eq!(vertex.rotation, particle.angle_z);
        assert_eq!(vertex.alpha, particle.alpha);
    }
}

#[test]
fn terrain_scorch_add_dedups_and_drops_oldest_like_cpp() {
    use game_client_rust::terrain::scorch_mesh::{MAX_SCORCH_MARKS, TerrainScorchBuffer};

    let mut buf = TerrainScorchBuffer::new();
    assert!(buf.add_scorch([100.0, 200.0, 0.0], 20.0, 1));
    assert!(
        !buf.add_scorch([101.0, 201.0, 0.0], 20.0, 1),
        "abs(dx)<radius/4 and same type/radius is a C++ duplicate"
    );
    assert!(
        buf.add_scorch([100.0, 200.0, 0.0], 20.0, 2),
        "different type is not a duplicate"
    );
    assert_eq!(buf.len(), 2);

    buf.clear();
    for i in 0..MAX_SCORCH_MARKS {
        assert!(buf.add_scorch([i as f32 * 1000.0, 0.0, 0.0], 10.0, 0));
    }
    assert_eq!(buf.len(), MAX_SCORCH_MARKS);
    assert!(buf.add_scorch([999_000.0, 0.0, 0.0], 10.0, 0));
    assert_eq!(buf.len(), MAX_SCORCH_MARKS);
    assert_eq!(buf.marks()[0].location[0], 1000.0);
    assert_eq!(buf.marks()[MAX_SCORCH_MARKS - 1].location[0], 999_000.0);
}

#[test]
fn terrain_scorch_gpu_bake_drapes_height_and_type_uv() {
    use game_client_rust::terrain::height_map::HeightMap;
    use game_client_rust::terrain::scorch_mesh::{
        SCORCH_FLOAT_AMOUNT, SCORCH_PER_ROW, add_terrain_scorch, bake_terrain_scorch_gpu_mesh,
        clear_terrain_scorches,
    };
    use gamelogic::common::types::MAP_XY_FACTOR;

    clear_terrain_scorches();
    let mut height = HeightMap::new(16, 16, 80.0, MAP_XY_FACTOR);
    height.set_height_at_index(5, 5, 1.0);
    let loc = [5.0 * MAP_XY_FACTOR, 5.0 * MAP_XY_FACTOR, 0.0];
    assert!(add_terrain_scorch(loc, 20.0, 4));
    let mesh = bake_terrain_scorch_gpu_mesh(&height, 0xFF112233);
    assert!(!mesh.vertices.is_empty());
    assert_eq!(mesh.indices.len() % 6, 0);
    assert!(!mesh.indices.is_empty());

    let center = mesh
        .vertices
        .iter()
        .find(|v| (v.x - loc[0]).abs() < 0.01 && (v.y - loc[1]).abs() < 0.01)
        .expect("grid sample at scorch center");
    let expected_z = SCORCH_FLOAT_AMOUNT + height.world_height_at_index(5, 5);
    assert!(
        (center.z - expected_z).abs() < 0.001,
        "z {} vs expected {}",
        center.z,
        expected_z
    );
    assert_eq!(center.diffuse, 0xFF112233);

    let u_offset = (4 % SCORCH_PER_ROW) as f32 * 1.5;
    let v_offset = (4 / SCORCH_PER_ROW) as f32 * 1.5;
    let expected_u = (u_offset + 0.5) / (SCORCH_PER_ROW as f32 + 1.0);
    let expected_v = (v_offset + 0.5) / (SCORCH_PER_ROW as f32 + 1.0);
    assert!((center.u1 - expected_u).abs() < 0.001);
    assert!((center.v1 - expected_v).abs() < 0.001);
    clear_terrain_scorches();
}

#[test]
fn fxlist_terrain_scorch_nugget_calls_add_scorch_with_type_and_radius() {
    use game_client_rust::effects::fxlist_integration::{
        FXContext, FXNugget, ScorchType, TerrainScorchFXNugget,
    };
    use game_client_rust::terrain::scorch_mesh::{clear_terrain_scorches, terrain_scorch_marks};

    clear_terrain_scorches();
    let nugget = TerrainScorchFXNugget {
        scorch_type: ScorchType::Scorch3,
        radius: 25.0,
    };
    let mut manager = ParticleSystemManager::new();
    let mut ctx = FXContext {
        particle_manager: &mut manager,
        ray_effect_manager: None,
        decal_manager: None,
        bone_query: None,
        current_frame: 12,
        local_player_index: 0,
    };
    FXNugget::do_fx_pos(
        &nugget,
        Point3::new(40.0, 50.0, 6.0),
        None,
        0.0,
        None,
        0.0,
        &mut ctx,
    );
    let marks = terrain_scorch_marks();
    assert_eq!(marks.len(), 1);
    assert_eq!(marks[0].location, [40.0, 50.0, 6.0]);
    assert_eq!(marks[0].radius, 25.0);
    assert_eq!(marks[0].scorch_type, 2);
    clear_terrain_scorches();
}

#[test]
fn fxlist_terrain_scorch_nugget_does_not_invent_decal_quad() {
    use game_client_rust::effects::decals::DecalManager;
    use game_client_rust::effects::fxlist_integration::{
        FXContext, FXNugget, ScorchType, TerrainScorchFXNugget,
    };
    use game_client_rust::terrain::scorch_mesh::{clear_terrain_scorches, terrain_scorch_marks};

    clear_terrain_scorches();
    let nugget = TerrainScorchFXNugget {
        scorch_type: ScorchType::Scorch1,
        radius: 12.0,
    };
    let mut manager = ParticleSystemManager::new();
    let mut decals = DecalManager::new();
    let mut ctx = FXContext {
        particle_manager: &mut manager,
        ray_effect_manager: None,
        decal_manager: Some(&mut decals),
        bone_query: None,
        current_frame: 1,
        local_player_index: 0,
    };
    FXNugget::do_fx_pos(
        &nugget,
        Point3::new(8.0, 9.0, 1.0),
        None,
        0.0,
        None,
        0.0,
        &mut ctx,
    );
    assert_eq!(terrain_scorch_marks().len(), 1);
    assert_eq!(
        decals.active_decal_count(),
        0,
        "C++ TerrainScorchFXNugget never creates a timed decal quad"
    );
    clear_terrain_scorches();
}

#[test]
fn fxlist_tracer_nugget_sets_parms_transform_and_ceil_expiration() {
    use game_client_rust::effects::fxlist_integration::{FXContext, FXNugget, TracerFXNugget};
    use game_client_rust::effects::tracer_fx::{
        bake_tracer_gpu_mesh, build_tracer_transform, clear_tracer_fx, live_tracer_fx,
        tracer_distance, tracer_expiration_frames, update_tracer_fx,
    };
    use glam::Vec3;

    clear_tracer_fx();
    let mut nugget = TracerFXNugget::new("GenericTracer".to_string());
    nugget.speed = 10.0;
    nugget.length = 10.0;
    nugget.width = 2.0;
    nugget.color = [0.9, 0.2, 0.1];
    nugget.decay_at = 0.5;
    nugget.probability = 1.0;

    let primary = Point3::new(0.0, 0.0, 0.0);
    let secondary = Point3::new(100.0, 0.0, 0.0);
    let mut manager = ParticleSystemManager::new();
    let mut ctx = FXContext {
        particle_manager: &mut manager,
        ray_effect_manager: None,
        decal_manager: None,
        bone_query: None,
        current_frame: 30,
        local_player_index: 0,
    };
    FXNugget::do_fx_pos(&nugget, primary, None, 0.0, Some(secondary), 0.0, &mut ctx);

    let tracers = live_tracer_fx();
    assert_eq!(tracers.len(), 1);
    let tracer = &tracers[0];
    assert_eq!(tracer.tracer_name, "GenericTracer");
    assert_eq!(tracer.speed, 10.0);
    assert_eq!(tracer.length, 10.0);
    assert_eq!(tracer.width, 2.0);
    assert_eq!(tracer.color, [0.9, 0.2, 0.1]);
    assert_eq!(tracer.opacity, 1.0);
    assert!((tracer.dir[0] - 1.0).abs() < 1e-5);
    assert!(tracer.dir[1].abs() < 1e-5);
    assert!(tracer.dir[2].abs() < 1e-5);

    let dist = tracer_distance([0.0, 0.0, 0.0], [100.0, 0.0, 0.0]);
    let frames = tracer_expiration_frames(dist - 10.0, 10.0, 0.5);
    assert_eq!(frames, 5, "ceil((90/10)*0.5) = ceil(4.5) = 5");
    assert_eq!(tracer.expire_frame, 30 + frames);

    let xform = build_tracer_transform(tracer.pos, tracer.dir);
    let x_axis = xform.transform_vector3(Vec3::X);
    assert!((x_axis - Vec3::new(1.0, 0.0, 0.0)).length() < 1e-4);
    let origin = xform.transform_point3(Vec3::ZERO);
    assert!((origin - Vec3::from(tracer.pos)).length() < 1e-4);

    let mesh = bake_tracer_gpu_mesh(tracer, 0);
    assert_eq!(mesh.vertices.len(), 4);
    assert_eq!(mesh.indices, vec![0, 1, 2, 0, 2, 3]);
    assert_eq!(mesh.vertices[0].color[3], 1.0);

    update_tracer_fx(30);
    let after = live_tracer_fx();
    assert_eq!(after.len(), 1);
    assert!((after[0].pos[0] - 10.0).abs() < 1e-4);
    assert!((after[0].opacity - 0.8).abs() < 1e-4, "1.0 - 1.0/5 = 0.8");

    update_tracer_fx(35);
    assert!(
        live_tracer_fx().is_empty(),
        "drawable expires at spawn+ceil frames"
    );
    clear_tracer_fx();
}

#[test]
fn fxlist_view_shake_nugget_uses_tactical_view_cpp_falloff() {
    use game_client_rust::display::view::{
        Point3 as ViewPoint3, with_tactical_view, with_tactical_view_ref,
    };
    use game_client_rust::effects::fxlist_integration::{
        FXContext, FXNugget, ShakeType, ViewShakeFXNugget,
    };

    with_tactical_view(|view| {
        view.set_position(&ViewPoint3::new(0.0, 0.0, 0.0));
        view.reset_camera_shake();
    });

    let nugget = ViewShakeFXNugget {
        shake_type: ShakeType::Subtle,
    };
    let mut manager = ParticleSystemManager::new();
    let mut ctx = FXContext {
        particle_manager: &mut manager,
        ray_effect_manager: None,
        decal_manager: None,
        bone_query: None,
        current_frame: 0,
        local_player_index: 0,
    };
    FXNugget::do_fx_pos(
        &nugget,
        Point3::new(75.0, 0.0, 0.0),
        None,
        0.0,
        None,
        0.0,
        &mut ctx,
    );

    let intensity = with_tactical_view_ref(|view| view.camera_shake_intensity());
    let data = game_engine::common::global_data::read();
    let expected = data.shake_subtle_intensity * (1.0 - 75.0 / data.max_shake_range);
    assert!(
        (intensity - expected).abs() < 1e-4,
        "got {intensity} expected {expected}"
    );

    with_tactical_view(|view| view.reset_camera_shake());
    FXNugget::do_fx_pos(
        &nugget,
        Point3::new(data.max_shake_range + 10.0, 0.0, 0.0),
        None,
        0.0,
        None,
        0.0,
        &mut ctx,
    );
    let after = with_tactical_view_ref(|view| view.camera_shake_intensity());
    assert_eq!(after, 0.0, "beyond maxShakeRange is a no-op");
}

#[test]
fn fxlist_ray_effect_nugget_creates_midpoint_template_entry() {
    use game_client_rust::effects::fxlist_integration::{FXContext, FXNugget, RayEffectFXNugget};
    use game_client_rust::effects::ray_effect_system::{
        MAX_RAY_EFFECTS, bake_ray_effect_gpu_endpoints, create_ray_effect_by_template,
        live_ray_effects, ray_effect_midpoint, reset_ray_effects,
    };

    reset_ray_effects();
    let mut nugget = RayEffectFXNugget::new("GenericLaser".to_string());
    nugget.primary_offset = nalgebra::Vector3::new(1.0, 2.0, 3.0);
    nugget.secondary_offset = nalgebra::Vector3::new(-1.0, 0.0, 1.0);

    let mut manager = ParticleSystemManager::new();
    let mut ctx = FXContext {
        particle_manager: &mut manager,
        ray_effect_manager: None,
        decal_manager: None,
        bone_query: None,
        current_frame: 0,
        local_player_index: 0,
    };
    FXNugget::do_fx_pos(
        &nugget,
        Point3::new(10.0, 20.0, 30.0),
        None,
        0.0,
        Some(Point3::new(50.0, 60.0, 70.0)),
        0.0,
        &mut ctx,
    );

    let rays = live_ray_effects();
    assert_eq!(rays.len(), 1);
    let start = [11.0, 22.0, 33.0];
    let end = [49.0, 60.0, 71.0];
    assert_eq!(rays[0].template_name, "GenericLaser");
    assert_eq!(rays[0].start, start);
    assert_eq!(rays[0].end, end);
    assert_eq!(rays[0].midpoint, ray_effect_midpoint(start, end));
    let (b_start, b_mid, b_end) = bake_ray_effect_gpu_endpoints(&rays[0]);
    assert_eq!(b_start, start);
    assert_eq!(b_end, end);
    assert_eq!(b_mid, rays[0].midpoint);

    assert!(create_ray_effect_by_template(start, end, "").is_none());

    reset_ray_effects();
    for i in 0..MAX_RAY_EFFECTS {
        assert!(
            create_ray_effect_by_template([i as f32, 0.0, 0.0], [i as f32, 1.0, 0.0], "Laser")
                .is_some()
        );
    }
    assert!(
        create_ray_effect_by_template([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], "Laser").is_none(),
        "C++ MAX_RAY_EFFECTS=128 rejects overflow"
    );
    reset_ray_effects();
}

#[test]
fn fxlist_ray_effect_expires_after_max_intensity_plus_fade() {
    use game_client_rust::effects::ray_effect_system::{
        create_ray_effect_by_template, live_ray_effects, reset_ray_effects, update_ray_effects,
    };

    reset_ray_effects();
    update_ray_effects(10);
    let ray = create_ray_effect_by_template([0.0, 0.0, 0.0], [10.0, 0.0, 0.0], "GenericLaser")
        .expect("template present");
    assert_eq!(live_ray_effects().len(), 1);
    assert!((ray.width_scalar - 1.0).abs() < 1e-5);
    assert!(ray.expire_frame > ray.created_frame);

    update_ray_effects(ray.fade_start_frame);
    assert_eq!(live_ray_effects().len(), 1);

    update_ray_effects(ray.expire_frame);
    assert!(
        live_ray_effects().is_empty(),
        "FXList RayEffect must expire after MaxIntensityLifetime+FadeLifetime"
    );
    reset_ray_effects();
}
