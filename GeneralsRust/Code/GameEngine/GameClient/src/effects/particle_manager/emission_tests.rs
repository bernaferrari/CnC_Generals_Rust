use super::*;

fn owner() -> (ParticleSystemManager, ParticleSystemId, ParticleSystemId) {
    owner_with_slave(ParticlePriorityType::AlwaysRender, false)
}

fn owner_with_slave(
    priority: ParticlePriorityType,
    ground_aligned: bool,
) -> (ParticleSystemManager, ParticleSystemId, ParticleSystemId) {
    owner_with_priorities(ParticlePriorityType::AlwaysRender, priority, ground_aligned)
}

fn owner_with_priorities(
    master_priority: ParticlePriorityType,
    priority: ParticlePriorityType,
    ground_aligned: bool,
) -> (ParticleSystemManager, ParticleSystemId, ParticleSystemId) {
    owner_with_attachment(master_priority, priority, ground_aligned, false)
}

fn owner_with_attachment(
    master_priority: ParticlePriorityType,
    priority: ParticlePriorityType,
    ground_aligned: bool,
    attached: bool,
) -> (ParticleSystemManager, ParticleSystemId, ParticleSystemId) {
    owner_with_attached_slave(master_priority, priority, ground_aligned, attached, false)
}

fn owner_with_attached_slave(
    master_priority: ParticlePriorityType,
    priority: ParticlePriorityType,
    ground_aligned: bool,
    attached: bool,
    attached_slave: bool,
) -> (ParticleSystemManager, ParticleSystemId, ParticleSystemId) {
    let mut manager = ParticleSystemManager::new();
    for (name, size, slave) in [
        ("EmissionMaster", 2.0, "EmissionSlave"),
        ("EmissionSlave", 3.0, ""),
    ] {
        let mut template = ParticleSystemTemplate::new(name.into());
        let info = template.info_mut();
        info.slave_system_name = slave.into();
        if attached && !slave.is_empty() {
            info.attached_system_name = "ControlledEmitter".into();
            info.emission_velocity_type = EmissionVelocityType::Ortho;
            info.emission_velocity = EmissionVelocity::Ortho {
                x: GameClientRandomVariable::new(1.0, 1.0),
                y: GameClientRandomVariable::new(0.0, 0.0),
                z: GameClientRandomVariable::new(0.0, 0.0),
            };
        }
        info.priority = if slave.is_empty() {
            priority
        } else {
            master_priority
        };
        info.is_ground_aligned = ground_aligned;
        info.shader_type = ParticleShaderType::Alpha;
        info.particle_type = ParticleType::Particle;
        info.lifetime = GameClientRandomVariable::new(100.0, 100.0);
        info.start_size = GameClientRandomVariable::new(size, size);
        info.start_size_rate = GameClientRandomVariable::new(1.0, 1.0);
        info.burst_count = GameClientRandomVariable::new(2.0, 2.0);
        info.burst_delay = GameClientRandomVariable::new(0.0, 0.0);
        info.alpha_keys[0].min_value = 1.0;
        info.alpha_keys[0].max_value = 1.0;
        info.alpha_keys[1].min_value = 1.0;
        info.alpha_keys[1].max_value = 1.0;
        info.alpha_keys[1].frame = 100;
        manager.register_template(Arc::new(template));
    }
    if attached {
        for name in ["ControlledEmitter", "ControlledSlave"] {
            let mut template = ParticleSystemTemplate::new(name.into());
            let info = template.info_mut();
            info.priority = ParticlePriorityType::AlwaysRender;
            info.shader_type = ParticleShaderType::Alpha;
            info.particle_type = ParticleType::Particle;
            info.lifetime = GameClientRandomVariable::new(20.0, 20.0);
            info.start_size = GameClientRandomVariable::new(1.0, 1.0);
            info.burst_count = GameClientRandomVariable::new(1.0, 1.0);
            info.alpha_keys[0].min_value = 1.0;
            info.alpha_keys[0].max_value = 1.0;
            info.alpha_keys[1].min_value = 1.0;
            info.alpha_keys[1].max_value = 1.0;
            info.alpha_keys[1].frame = 100;
            if name == "ControlledEmitter" && attached_slave {
                info.slave_system_name = "ControlledSlave".into();
            }
            manager.register_template(Arc::new(template));
        }
    }
    let template = manager.find_template("EmissionMaster").unwrap();
    let master = manager
        .create_particle_system_with_random(&template, true, &mut |lo, _| lo)
        .unwrap();
    let slave = manager
        .find_particle_system(master)
        .unwrap()
        .slave_system_id()
        .unwrap();
    (manager, master, slave)
}

fn sizes(manager: &ParticleSystemManager, id: ParticleSystemId) -> Vec<f32> {
    manager
        .find_particle_system(id)
        .unwrap()
        .particles()
        .iter()
        .map(|p| p.size)
        .collect()
}

#[test]
fn immediate_slave_emission_interleaves_fresh_mutable_generations_and_continues_save() {
    let (mut manager, master, slave) = owner();
    manager.update(0, 1);
    assert_eq!(sizes(&manager, master), vec![2.0, 4.0]);
    assert_eq!(sizes(&manager, slave), vec![9.0, 20.0]);
    let bytes = manager.capture_xfer_bytes().unwrap();
    let (mut restored, _, _) = owner();
    restored.restore_xfer_bytes(&bytes).unwrap().unwrap();
    manager.update(0, 2);
    restored.update(0, 2);
    assert_eq!(sizes(&manager, master), vec![2.0, 4.0, 6.0, 8.0]);
    assert_eq!(sizes(&manager, slave), vec![9.0, 20.0, 35.0, 54.0]);
    assert_eq!(sizes(&restored, master), sizes(&manager, master));
    assert_eq!(sizes(&restored, slave), sizes(&manager, slave));
}

#[test]
fn immediate_slave_emission_uses_master_priority_at_actual_field_cap() {
    let (mut manager, master, slave) = owner_with_slave(ParticlePriorityType::AreaEffect, true);
    let mut particle_info = crate::effects::particle_system::ParticleInfo::default();
    particle_info.lifetime = 100;
    particle_info.alpha_keys[0].value = 1.0;
    manager
        .find_particle_system_mut(slave)
        .unwrap()
        .push_particle(crate::effects::particle_system::Particle::new(
            &particle_info,
            0,
            0,
        ));
    manager.field_particle_count = usize::MAX;
    manager.update(0, 1);
    assert_eq!(
        manager
            .find_particle_system(master)
            .unwrap()
            .particle_count(),
        2
    );
    assert_eq!(
        manager
            .find_particle_system(slave)
            .unwrap()
            .particle_count(),
        3,
        "ALWAYS_RENDER master priority bypasses slave AREA_EFFECT field cap"
    );
}

#[test]
fn rejected_master_particle_does_not_generate_or_admit_slave() {
    let (mut manager, master, slave) = owner_with_priorities(
        ParticlePriorityType::AreaEffect,
        ParticlePriorityType::AlwaysRender,
        true,
    );
    let mut seed = crate::effects::particle_system::ParticleInfo::default();
    seed.lifetime = 100;
    seed.alpha_keys[0].value = 1.0;
    manager
        .find_particle_system_mut(master)
        .unwrap()
        .push_particle(crate::effects::particle_system::Particle::new(&seed, 0, 0));
    manager.field_particle_count = usize::MAX;
    manager.update(0, 1);
    assert_eq!(
        manager
            .find_particle_system(master)
            .unwrap()
            .particle_count(),
        1
    );
    assert_eq!(
        manager
            .find_particle_system(slave)
            .unwrap()
            .particle_count(),
        0
    );
    assert_eq!(
        manager
            .find_particle_system_mut(slave)
            .unwrap()
            .generate_particle_info(1, 1)
            .unwrap()
            .size,
        3.0,
        "rejected parent must not consume slave generation state"
    );
    assert_eq!(
        manager
            .find_particle_system_mut(master)
            .unwrap()
            .generate_particle_info(1, 1)
            .unwrap()
            .size,
        4.0,
        "only the two primary attempts advance the master bonus"
    );
}

#[test]
fn receiving_factory_burst_count_truncates_before_nonunit_multiplier() {
    let mut manager = ParticleSystemManager::new();
    let mut template = ParticleSystemTemplate::new("FractionalFactoryBurst".into());
    template.info_mut().priority = ParticlePriorityType::AlwaysRender;
    template.info_mut().shader_type = ParticleShaderType::Alpha;
    template.info_mut().burst_count = GameClientRandomVariable::new(2.9, 2.9);
    template.info_mut().lifetime = GameClientRandomVariable::new(100.0, 100.0);
    template.info_mut().alpha_keys[0].min_value = 1.0;
    template.info_mut().alpha_keys[0].max_value = 1.0;
    template.info_mut().alpha_keys[1].min_value = 1.0;
    template.info_mut().alpha_keys[1].max_value = 1.0;
    template.info_mut().alpha_keys[1].frame = 100;
    let template = Arc::new(template);
    let id = manager
        .create_particle_system_with_random(&template, false, &mut |lo, _| lo)
        .unwrap();
    manager
        .find_particle_system_mut(id)
        .unwrap()
        .set_burst_count_multiplier(2.0);
    manager.update(0, 1);
    assert_eq!(
        manager.find_particle_system(id).unwrap().particle_count(),
        4,
        "C++ truncates 2.9 to two before multiplying by two"
    );
}

#[test]
fn immediate_emission_binds_each_attached_system_to_its_admitted_parent() {
    let (mut manager, master, slave) = owner_with_attachment(
        ParticlePriorityType::AlwaysRender,
        ParticlePriorityType::AlwaysRender,
        false,
        true,
    );
    manager
        .find_particle_system_mut(master)
        .unwrap()
        .set_position(Vec3::new(10.0, 20.0, 30.0));
    manager.update(0, 1);
    let parents = manager.find_particle_system(master).unwrap().particles();
    assert_eq!(parents.len(), 2);
    assert_eq!(parents[0].personality, 0);
    assert_eq!(parents[1].personality, 1);
    assert_eq!(parents[0].controlled_system, Some(3));
    assert_eq!(parents[1].controlled_system, Some(4));
    assert_eq!(
        manager.active_system_ids_in_order(),
        vec![slave, master, 3, 4]
    );
    assert_eq!(sizes(&manager, master), vec![2.0, 4.0]);
    assert_eq!(sizes(&manager, slave), vec![9.0, 20.0]);
    for id in [3, 4] {
        assert_eq!(
            manager.find_particle_system(id).unwrap().template().name(),
            "ControlledEmitter"
        );
    }
    for id in [3, 4] {
        let system = manager.find_particle_system(id).unwrap();
        assert_eq!(system.position(), Vec3::new(11.0, 20.0, 30.0));
        assert_eq!(
            system.particle_count(),
            1,
            "new attached emitter is visited this frame"
        );
        assert_eq!(system.particles()[0].position, Vec3::new(11.0, 20.0, 30.0));
        assert_eq!(system.particles()[0].lifetime_left, 19);
    }
    manager.update(0, 2);
    for id in [3, 4] {
        assert_eq!(
            manager.find_particle_system(id).unwrap().position(),
            Vec3::new(12.0, 20.0, 30.0)
        );
    }
}

#[test]
fn appended_attached_slave_chain_runs_in_constructor_admission_order() {
    let (mut manager, master, slave) = owner_with_attached_slave(
        ParticlePriorityType::AlwaysRender,
        ParticlePriorityType::AlwaysRender,
        false,
        true,
        true,
    );
    manager.update(0, 1);
    assert_eq!(
        manager.active_system_ids_in_order(),
        vec![slave, master, 4, 3, 6, 5]
    );
    for (attached, child) in [(3, 4), (5, 6)] {
        let parent = manager.find_particle_system(attached).unwrap();
        let slave = manager.find_particle_system(child).unwrap();
        assert_eq!(parent.slave_system_id(), Some(child));
        assert_eq!(slave.master_system_id(), Some(attached));
        assert_eq!(parent.particle_count(), 1);
        assert_eq!(slave.particle_count(), 1);
        assert_eq!(parent.particles()[0].lifetime_left, 19);
        assert_eq!(
            slave.particles()[0].lifetime_left,
            20,
            "appended child runs before its parent and keeps its newborn unaged"
        );
        assert_eq!(parent.position(), Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(parent.particles()[0].position, Vec3::new(1.0, 0.0, 0.0));
    }
}

fn lifetime_pair(
    master_lifetime: u32,
    slave_lifetime: u32,
    master_burst: f32,
    slave_burst: f32,
) -> (ParticleSystemManager, u32, u32) {
    let mut manager = ParticleSystemManager::new();
    for (name, lifetime, burst, slave) in [
        (
            "RetiringMaster",
            master_lifetime,
            master_burst,
            "SurvivingSlave",
        ),
        ("SurvivingSlave", slave_lifetime, slave_burst, ""),
    ] {
        let mut template = ParticleSystemTemplate::new(name.into());
        let info = template.info_mut();
        info.priority = ParticlePriorityType::AlwaysRender;
        info.shader_type = ParticleShaderType::Alpha;
        info.slave_system_name = slave.into();
        info.system_lifetime = lifetime;
        info.burst_count = GameClientRandomVariable::new(burst, burst);
        info.lifetime = GameClientRandomVariable::new(20.0, 20.0);
        info.alpha_keys[0].min_value = 1.0;
        info.alpha_keys[0].max_value = 1.0;
        info.alpha_keys[1].min_value = 1.0;
        info.alpha_keys[1].max_value = 1.0;
        info.alpha_keys[1].frame = 100;
        manager.register_template(Arc::new(template));
    }
    let template = manager.find_template("RetiringMaster").unwrap();
    let master = manager
        .create_particle_system_with_random(&template, true, &mut |lo, _| lo)
        .unwrap();
    let slave = manager
        .find_particle_system(master)
        .unwrap()
        .slave_system_id()
        .unwrap();
    (manager, master, slave)
}

#[test]
fn natural_master_retirement_detaches_surviving_slave_without_destroying_it() {
    let (mut manager, master, slave) = lifetime_pair(1, 0, 0.0, 1.0);
    manager.update(0, 1);
    assert!(manager.find_particle_system(master).is_none());
    let survivor = manager.find_particle_system(slave).unwrap();
    assert!(!survivor.is_destroyed());
    assert_eq!(survivor.master_system_id(), None);
    assert_eq!(survivor.particle_count(), 0);
    manager.update(0, 2);
    assert_eq!(
        manager
            .find_particle_system(slave)
            .unwrap()
            .particle_count(),
        1
    );
}

#[test]
fn natural_child_retirement_clears_parent_link_without_skipping_shifted_parent() {
    let (mut manager, master, slave) = lifetime_pair(0, 1, 1.0, 0.0);
    manager.update(0, 1);
    assert!(manager.find_particle_system(slave).is_none());
    let parent = manager.find_particle_system(master).unwrap();
    assert_eq!(parent.slave_system_id(), None);
    assert_eq!(
        parent.particle_count(),
        1,
        "same list index visits parent shifted after child removal"
    );
    assert_eq!(manager.active_system_ids_in_order(), vec![master]);
}

#[test]
fn destroyed_empty_system_finishes_initial_delay_before_natural_retirement() {
    let mut manager = ParticleSystemManager::new();
    let mut template = ParticleSystemTemplate::new("DelayedDestroyed".into());
    template.info_mut().system_lifetime = 1;
    template.info_mut().initial_delay = GameClientRandomVariable::new(2.0, 2.0);
    let id = manager
        .create_particle_system_with_random(&Arc::new(template), false, &mut |lo, _| lo)
        .unwrap();
    manager.destroy_particle_system(id);
    manager.update(0, 1);
    let delayed = manager.find_particle_system(id).unwrap();
    assert_eq!(delayed.initial_delay_left(), 1);
    assert_eq!(delayed.start_frame(), 0);
    manager.update(0, 2);
    let delayed = manager.find_particle_system(id).unwrap();
    assert_eq!(delayed.initial_delay_left(), 0);
    assert_eq!(delayed.start_frame(), 2);
    manager.update(0, 3);
    assert!(manager.find_particle_system(id).is_none());
}

#[test]
fn natural_controlled_emitter_retirement_detaches_actual_parent_particle() {
    let mut manager = ParticleSystemManager::new();
    let mut parent = ParticleSystemTemplate::new("ControlParent".into());
    parent.info_mut().priority = ParticlePriorityType::AlwaysRender;
    parent.info_mut().shader_type = ParticleShaderType::Alpha;
    parent.info_mut().burst_count = GameClientRandomVariable::new(1.0, 1.0);
    parent.info_mut().lifetime = GameClientRandomVariable::new(20.0, 20.0);
    parent.info_mut().alpha_keys[0].min_value = 1.0;
    parent.info_mut().alpha_keys[0].max_value = 1.0;
    parent.info_mut().attached_system_name = "ControlChild".into();
    let mut child = ParticleSystemTemplate::new("ControlChild".into());
    child.info_mut().system_lifetime = 1;
    child.info_mut().burst_count = GameClientRandomVariable::new(0.0, 0.0);
    manager.register_template(Arc::new(child));
    let id = manager
        .create_particle_system_with_random(&Arc::new(parent), false, &mut |lo, _| lo)
        .unwrap();
    manager.update(0, 1);
    assert_eq!(manager.active_system_count(), 1);
    let parent = manager.find_particle_system(id).unwrap();
    assert_eq!(parent.particle_count(), 1);
    assert_eq!(parent.particles()[0].controlled_system, None);
}

#[test]
fn expired_parent_particles_release_actual_budget_before_later_emitter() {
    let mut manager = ParticleSystemManager::new();
    let mut parent = ParticleSystemTemplate::new("ExpiringBudgetParent".into());
    parent.info_mut().priority = ParticlePriorityType::AlwaysRender;
    parent.info_mut().shader_type = ParticleShaderType::Alpha;
    let parent_id = manager
        .create_particle_system_with_random(&Arc::new(parent), false, &mut |lo, _| lo)
        .unwrap();
    manager.find_particle_system_mut(parent_id).unwrap().stop();
    let mut later = ParticleSystemTemplate::new("LaterBudgetEmitter".into());
    let info = later.info_mut();
    info.priority = ParticlePriorityType::Critical;
    info.shader_type = ParticleShaderType::Alpha;
    info.burst_count = GameClientRandomVariable::new(1.0, 1.0);
    info.lifetime = GameClientRandomVariable::new(20.0, 20.0);
    info.alpha_keys[0].min_value = 1.0;
    info.alpha_keys[0].max_value = 1.0;
    info.alpha_keys[1].min_value = 1.0;
    info.alpha_keys[1].max_value = 1.0;
    info.alpha_keys[1].frame = 100;
    let later_id = manager
        .create_particle_system_with_random(&Arc::new(later), false, &mut |lo, _| lo)
        .unwrap();
    let cap = manager.live_max_particle_count();
    let mut seed = crate::effects::particle_system::ParticleInfo::default();
    seed.lifetime = 1;
    seed.alpha_keys[0].value = 1.0;
    for personality in 0..=cap {
        manager
            .find_particle_system_mut(parent_id)
            .unwrap()
            .push_particle(crate::effects::particle_system::Particle::new(
                &seed,
                personality as u32,
                0,
            ));
    }
    manager.particle_count = cap + 1;
    manager.update(0, 1);
    assert_eq!(
        manager
            .find_particle_system(parent_id)
            .unwrap()
            .particle_count(),
        0
    );
    assert_eq!(
        manager
            .find_particle_system(later_id)
            .unwrap()
            .particle_count(),
        1,
        "later admission observes particles already expired by earlier integration"
    );
    assert_eq!(manager.particle_count(), 1);
}

#[test]
fn prepared_internal_destroy_flag_cascades_before_natural_master_detach() {
    let mut manager = ParticleSystemManager::new();
    for (name, slave) in [
        ("MarkedRoot", "MarkedMiddle"),
        ("MarkedMiddle", "MarkedLeaf"),
        ("MarkedLeaf", ""),
    ] {
        let mut template = ParticleSystemTemplate::new(name.into());
        template.info_mut().slave_system_name = slave.into();
        template.info_mut().burst_count = GameClientRandomVariable::new(0.0, 0.0);
        manager.register_template(Arc::new(template));
    }
    let template = manager.find_template("MarkedRoot").unwrap();
    let root = manager
        .create_particle_system_with_random(&template, true, &mut |lo, _| lo)
        .unwrap();
    let middle = manager
        .find_particle_system(root)
        .unwrap()
        .slave_system_id()
        .unwrap();
    let leaf = manager
        .find_particle_system(middle)
        .unwrap()
        .slave_system_id()
        .unwrap();
    // Model the direct per-system mark used by attachment/module preparation,
    // not the public manager destroy route which already traverses the chain.
    manager.find_particle_system_mut(root).unwrap().destroy();
    assert!(!manager.find_particle_system(middle).unwrap().is_destroyed());
    manager.update(0, 1);
    assert!(manager.find_particle_system(root).is_none());
    assert!(manager.find_particle_system(middle).unwrap().is_destroyed());
    assert!(manager.find_particle_system(leaf).unwrap().is_destroyed());
    manager.update(0, 2);
    assert_eq!(manager.active_system_count(), 0);
}
