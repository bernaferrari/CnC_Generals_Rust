use super::*;

fn definition(name: &str, slave: &str, base: f32) -> Arc<ParticleSystemTemplate> {
    let mut template = ParticleSystemTemplate::new(name.into());
    let info = template.info_mut();
    info.slave_system_name = slave.into();
    info.initial_delay = GameClientRandomVariable::new(base, base + 1.0);
    info.wind_motion_start_angle_min = base + 2.0;
    info.wind_motion_start_angle_max = base + 3.0;
    info.wind_motion_end_angle_min = base + 4.0;
    info.wind_motion_end_angle_max = base + 5.0;
    Arc::new(template)
}

fn chain_manager() -> (ParticleSystemManager, Arc<ParticleSystemTemplate>) {
    let mut manager = ParticleSystemManager::new();
    let root = definition("Root", "Middle", 10.0);
    manager.register_template(root.clone());
    manager.register_template(definition("Middle", "Leaf", 20.0));
    manager.register_template(definition("Leaf", "", 30.0));
    (manager, root)
}

#[test]
fn authored_slave_factory_preserves_ids_links_draws_and_recursive_admission() {
    let (mut manager, root) = chain_manager();
    let mut draws = Vec::new();
    let id = manager
        .create_particle_system_with_random(&root, true, &mut |lo, hi| {
            draws.push((lo, hi));
            lo
        })
        .unwrap();
    assert_eq!(id, 1);
    assert_eq!(manager.next_system_id, 4);
    assert_eq!(manager.active_system_ids_in_order(), vec![3, 2, 1]);
    let expected: Vec<_> = [10.0, 20.0, 30.0]
        .into_iter()
        .flat_map(|b| {
            [
                (b, b + 1.0),
                (b + 2.0, b + 3.0),
                (b + 4.0, b + 5.0),
                (b + 2.0, b + 4.0),
            ]
        })
        .collect();
    assert_eq!(draws, expected);
    for id in 1..=3 {
        let system = manager.find_particle_system(id).unwrap();
        assert_eq!(system.master_system_id(), (id > 1).then_some(id - 1));
        assert_eq!(system.slave_system_id(), (id < 3).then_some(id + 1));
        assert!(Arc::ptr_eq(
            system.template(),
            &manager.find_template(system.template().name()).unwrap()
        ));
    }
    manager.destroy_particle_system(1);
    for id in 1..=3 {
        assert!(manager.find_particle_system(id).unwrap().is_destroyed());
    }
}

#[test]
fn authored_slave_factory_missing_child_and_false_path_stay_parent_only() {
    let mut manager = ParticleSystemManager::new();
    let missing = definition("MissingParent", "Absent", 10.0);
    let mut draws = 0;
    manager
        .create_particle_system_with_random(&missing, true, &mut |lo, _| {
            draws += 1;
            lo
        })
        .unwrap();
    assert_eq!(draws, 4);
    assert_eq!(manager.active_system_ids_in_order(), vec![1]);
    assert_eq!(
        manager.find_particle_system(1).unwrap().slave_system_id(),
        None
    );
    let (mut manager, root) = chain_manager();
    manager
        .create_particle_system_with_id_and_random(&root, 77, false, &mut |lo, _| lo)
        .unwrap();
    assert_eq!(manager.active_system_ids_in_order(), vec![77]);
    assert_eq!(manager.next_system_id, 78);
    assert_eq!(
        manager.find_particle_system(77).unwrap().slave_system_id(),
        None
    );
}

#[test]
fn authored_slave_factory_cycle_rejects_before_draws_or_receiver_mutation() {
    let mut manager = ParticleSystemManager::new();
    let root = definition("CycleA", "CycleB", 10.0);
    manager.register_template(root.clone());
    manager.register_template(definition("CycleB", "CycleA", 20.0));
    let mut draws = 0;
    assert!(
        manager
            .create_particle_system_with_random(&root, true, &mut |lo, _| {
                draws += 1;
                lo
            })
            .is_err()
    );
    assert_eq!(draws, 0);
    assert_eq!(manager.next_system_id, 1);
    assert_eq!(manager.active_system_count(), 0);
    assert!(manager.active_system_ids_in_order().is_empty());
    // FALSE is the deliberate restore/nonrecursive policy even with bad authored edges.
    manager
        .create_particle_system_with_random(&root, false, &mut |lo, _| lo)
        .unwrap();
    assert_eq!(manager.active_system_count(), 1);
}

#[test]
fn authored_slave_factory_cached_definition_and_same_id_peers_are_independent() {
    let mut root = ParticleSystemTemplate::new("CachedRoot".into());
    root.info_mut().slave_system_name = "NotInCatalog".into();
    root.set_slave_template(Some(definition("CachedLeaf", "", 30.0)));
    let root = Arc::new(root);
    let mut first = ParticleSystemManager::new();
    let mut peer = ParticleSystemManager::new();
    first
        .create_particle_system_with_random(&root, true, &mut |lo, _| lo)
        .unwrap();
    peer.create_particle_system_with_random(&root, true, &mut |_, hi| hi)
        .unwrap();
    assert_eq!(first.active_system_ids_in_order(), vec![2, 1]);
    assert_eq!(peer.active_system_ids_in_order(), vec![2, 1]);
    assert_eq!(
        first.find_particle_system(2).unwrap().template().name(),
        "CachedLeaf"
    );
    first.destroy_particle_system(1);
    assert!(!peer.find_particle_system(1).unwrap().is_destroyed());
    assert!(!peer.find_particle_system(2).unwrap().is_destroyed());
    assert_eq!(
        peer.find_particle_system(2).unwrap().master_system_id(),
        Some(1)
    );
}

#[test]
fn authored_slave_factory_native_load_does_not_expand_saved_chain_again() {
    let (mut source, root) = chain_manager();
    source
        .create_particle_system_with_random(&root, true, &mut |lo, _| lo)
        .unwrap();
    let bytes = source.capture_xfer_bytes().unwrap();
    let (mut receiving, _) = chain_manager();
    receiving.restore_xfer_bytes(&bytes).unwrap().unwrap();
    assert_eq!(receiving.active_system_ids_in_order(), vec![3, 2, 1]);
    assert_eq!(receiving.active_system_count(), 3);
    assert_eq!(receiving.next_system_id, 4);
    for id in 1..=3 {
        let system = receiving.find_particle_system(id).unwrap();
        assert_eq!(system.master_system_id(), (id > 1).then_some(id - 1));
        assert_eq!(system.slave_system_id(), (id < 3).then_some(id + 1));
    }
    // Even a saved nonrecursive root must not auto-create newly admitted slaves.
    let (mut source, root) = chain_manager();
    source
        .create_particle_system_with_random(&root, false, &mut |lo, _| lo)
        .unwrap();
    let bytes = source.capture_xfer_bytes().unwrap();
    receiving.restore_xfer_bytes(&bytes).unwrap().unwrap();
    assert_eq!(receiving.active_system_ids_in_order(), vec![1]);
    assert_eq!(
        receiving.find_particle_system(1).unwrap().slave_system_id(),
        None
    );
}

#[test]
fn authored_slave_factory_parsed_pair_updates_and_native_save_continues() {
    fn parsed_owner() -> ParticleSystemManager {
        let mut manager = ParticleSystemManager::new();
        let common = "Priority = ALWAYS_RENDER\nShader = ALPHA\nType = PARTICLE\nLifetime = 20 20\nAlpha1 = 1 1 0\nAlpha2 = 1 1 100\nColor1 = R:255 G:127.5 B:0 0\nColor2 = R:255 G:127.5 B:0 100\nVelocityType = ORTHO\nVelOrthoX = 0 0\nVelOrthoY = 0 0\nVelOrthoZ = 0 0\nVolumeType = POINT\nWindMotion = Unused\nBurstCount = 1 1\nBurstDelay = 0 0\n";
        let source = format!(
            "ParticleSystem ParsedMaster\n{common}SlaveSystem = ParsedSlave\nSize = 2 2\nEnd\nParticleSystem ParsedSlave\n{common}SlavePosOffset = X:1 Y:2 Z:3\nSize = 3 3\nEnd\n"
        );
        assert_eq!(
            ParticleSystemINIParser::default()
                .overlay_mixed_source(&source, &mut manager)
                .unwrap(),
            2
        );
        manager
    }
    fn observed(manager: &ParticleSystemManager) -> Vec<(u32, Vec<(Vec3, u32, f32, f32)>)> {
        manager
            .active_system_ids_in_order()
            .into_iter()
            .map(|id| {
                let particles = manager
                    .find_particle_system(id)
                    .unwrap()
                    .particles()
                    .iter()
                    .map(|particle| {
                        (
                            particle.position,
                            particle.lifetime_left,
                            particle.size,
                            particle.alpha,
                        )
                    })
                    .collect();
                (id, particles)
            })
            .collect()
    }
    let mut source = parsed_owner();
    let template = source.find_template("ParsedMaster").unwrap();
    let master_id = source
        .create_particle_system_with_random(&template, true, &mut |lo, _| lo)
        .unwrap();
    let slave_id = source
        .find_particle_system(master_id)
        .unwrap()
        .slave_system_id()
        .unwrap();
    source
        .find_particle_system_mut(master_id)
        .unwrap()
        .set_position(Vec3::new(10.0, 20.0, 30.0));
    assert_eq!(
        source.active_system_ids_in_order(),
        vec![slave_id, master_id]
    );
    source.update(0, 1);
    let master = source.find_particle_system(master_id).unwrap();
    let slave = source.find_particle_system(slave_id).unwrap();
    assert_eq!(master.particle_count(), 1);
    assert_eq!(
        slave.particle_count(),
        1,
        "slave suppresses independent BurstCount emission"
    );
    assert_eq!(master.particles()[0].position, Vec3::new(10.0, 20.0, 30.0));
    assert_eq!(slave.particles()[0].position, Vec3::new(11.0, 22.0, 33.0));
    assert_eq!(master.particles()[0].size, 2.0);
    assert_eq!(
        slave.particles()[0].size,
        6.0,
        "slave size scales freshly generated master size"
    );
    assert_eq!(master.particles()[0].lifetime_left, 19);
    assert_eq!(
        slave.particles()[0].lifetime_left,
        20,
        "child is visited before master creates its particle, so newborn slave is unaged"
    );
    let bytes = source.capture_xfer_bytes().unwrap();
    let mut restored = parsed_owner();
    restored.restore_xfer_bytes(&bytes).unwrap().unwrap();
    assert_eq!(observed(&source), observed(&restored));
    for frame in 2..=4 {
        source.update(0, frame);
        restored.update(0, frame);
        assert_eq!(
            observed(&source),
            observed(&restored),
            "continuation frame {frame}"
        );
        assert_eq!(
            source
                .find_particle_system(master_id)
                .unwrap()
                .particle_count(),
            frame as usize
        );
        assert_eq!(
            source
                .find_particle_system(slave_id)
                .unwrap()
                .particle_count(),
            frame as usize
        );
    }
}

#[test]
fn authored_slave_factory_exhausted_child_ids_reject_without_mutation() {
    let (mut manager, root) = chain_manager();
    let mut draws = 0;
    assert!(
        manager
            .create_particle_system_with_id_and_random(&root, u32::MAX - 1, true, &mut |lo, _| {
                draws += 1;
                lo
            })
            .is_err()
    );
    assert_eq!(draws, 0);
    assert_eq!(manager.next_system_id, 1);
    assert_eq!(manager.active_system_count(), 0);
    assert!(manager.active_system_ids_in_order().is_empty());
    manager
        .create_particle_system_with_id_and_random(&root, u32::MAX, false, &mut |lo, _| lo)
        .unwrap();
    assert_eq!(manager.active_system_ids_in_order(), vec![u32::MAX]);
    assert_eq!(manager.next_system_id, u32::MAX);
}
