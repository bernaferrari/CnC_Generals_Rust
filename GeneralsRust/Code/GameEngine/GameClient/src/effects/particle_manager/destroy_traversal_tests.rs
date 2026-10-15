//! C++ destroy marks parent then slave, retaining particles until ordinary update.
use super::*;
use crate::effects::particle_system::{Particle, ParticleInfo};

fn admit(
    manager: &mut ParticleSystemManager,
    template: &Arc<ParticleSystemTemplate>,
) -> ParticleSystemId {
    manager
        .create_particle_system_with_random(template, false, &mut |lo, _| lo)
        .unwrap()
}

fn connect(manager: &mut ParticleSystemManager, parent: ParticleSystemId, child: ParticleSystemId) {
    manager
        .find_particle_system_mut(parent)
        .unwrap()
        .set_slave(Some(child));
    manager
        .find_particle_system_mut(child)
        .unwrap()
        .set_master(Some(parent));
}

fn definition() -> Arc<ParticleSystemTemplate> {
    Arc::new(ParticleSystemTemplate::new("DestroyTraversal".into()))
}

// OLD recursive traversal can abort the process rather than unwind. Keep that
// failure inside a child, with a deadline and captured diagnostics. The child
// alone receives the environment marker; no process-wide test state is changed.
fn isolated(name: &str, body: impl FnOnce()) {
    const MARKER: &str = "GENERALS_PARTICLE_DESTROY_TRAVERSAL_CHILD";
    if std::env::var(MARKER).as_deref() == Ok(name) {
        body();
        return;
    }
    let full_name = format!("effects::particle_manager::destroy_traversal_tests::{name}");
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &full_name, "--test-threads=1", "--nocapture"])
        .env(MARKER, name)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let start = Instant::now();
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if start.elapsed() > std::time::Duration::from_secs(20) {
            child.kill().unwrap();
            let output = child.wait_with_output().unwrap();
            panic!(
                "destroy traversal timed out: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "destroy traversal child failed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed; 0 failed"));
}

#[test]
fn particle_destroy_traversal_long_chain_preserves_registered_state_and_peer() {
    isolated(
        "particle_destroy_traversal_long_chain_preserves_registered_state_and_peer",
        || {
            // A small driving stack makes accidental input-sized recursion observable
            // with a moderate fixture. Constructing the systems is iterative too.
            std::thread::Builder::new()
                .stack_size(128 * 1024)
                .spawn(|| {
                    let template = definition();
                    let mut manager = ParticleSystemManager::new();
                    let ids: Vec<_> = (0..4096).map(|_| admit(&mut manager, &template)).collect();
                    for pair in ids.windows(2) {
                        connect(&mut manager, pair[0], pair[1]);
                    }
                    let unrelated = admit(&mut manager, &template);
                    let mut peer = ParticleSystemManager::new();
                    let peer_id = admit(&mut peer, &template);
                    assert_eq!(peer_id, ids[0]);
                    let order = manager.active_system_ids_in_order();
                    let next_id = manager.next_system_id;
                    manager.destroy_particle_system(ids[0]);
                    assert_eq!(manager.active_system_ids_in_order(), order);
                    assert_eq!(manager.next_system_id, next_id);
                    assert_eq!(manager.system_count(), 4097);
                    assert_eq!(manager.particle_count(), 0);
                    for (index, id) in ids.iter().copied().enumerate() {
                        let system = manager.find_particle_system(id).unwrap();
                        assert!(system.is_destroyed());
                        assert!(!system.is_stopped());
                        assert_eq!(system.slave_system_id(), ids.get(index + 1).copied());
                        assert_eq!(
                            system.master_system_id(),
                            index.checked_sub(1).map(|i| ids[i])
                        );
                    }
                    assert!(
                        !manager
                            .find_particle_system(unrelated)
                            .unwrap()
                            .is_destroyed()
                    );
                    assert!(!peer.find_particle_system(peer_id).unwrap().is_destroyed());
                })
                .unwrap()
                .join()
                .unwrap();
        },
    );
}

#[test]
fn particle_destroy_traversal_cycles_terminate_without_detaching_or_deleting() {
    isolated(
        "particle_destroy_traversal_cycles_terminate_without_detaching_or_deleting",
        || {
            let template = definition();
            let mut manager = ParticleSystemManager::new();
            let a = admit(&mut manager, &template);
            let b = admit(&mut manager, &template);
            let own = admit(&mut manager, &template);
            let unrelated = admit(&mut manager, &template);
            connect(&mut manager, a, b);
            connect(&mut manager, b, a);
            connect(&mut manager, own, own);
            let order = manager.active_system_ids_in_order();
            manager.destroy_particle_system(a);
            for id in [a, b] {
                assert!(manager.find_particle_system(id).unwrap().is_destroyed());
            }
            assert!(!manager.find_particle_system(own).unwrap().is_destroyed());
            manager.destroy_particle_system(own);
            assert!(manager.find_particle_system(own).unwrap().is_destroyed());
            assert!(
                !manager
                    .find_particle_system(unrelated)
                    .unwrap()
                    .is_destroyed()
            );
            assert_eq!(manager.active_system_ids_in_order(), order);
            assert_eq!(manager.system_count(), 4);
            assert_eq!(
                manager.find_particle_system(a).unwrap().slave_system_id(),
                Some(b)
            );
            assert_eq!(
                manager.find_particle_system(b).unwrap().master_system_id(),
                Some(a)
            );
            assert_eq!(
                manager.find_particle_system(own).unwrap().slave_system_id(),
                Some(own)
            );
        },
    );
}

#[test]
fn particle_destroy_traversal_premarked_root_and_middle_still_reach_descendants() {
    let template = definition();
    let mut manager = ParticleSystemManager::new();
    let ids: Vec<_> = (0..4).map(|_| admit(&mut manager, &template)).collect();
    for pair in ids.windows(2) {
        connect(&mut manager, pair[0], pair[1]);
    }
    for id in [ids[0], ids[2]] {
        manager.find_particle_system_mut(id).unwrap().destroy();
    }
    assert!(!manager.find_particle_system(ids[1]).unwrap().is_destroyed());
    manager.destroy_particle_system(ids[0]);
    for id in ids {
        assert!(manager.find_particle_system(id).unwrap().is_destroyed());
    }
    assert_eq!(manager.system_count(), 4);
}

#[test]
fn particle_destroy_traversal_retains_particles_until_original_update_cleanup() {
    let template = definition();
    let mut manager = ParticleSystemManager::new();
    let ids: Vec<_> = (0..3).map(|_| admit(&mut manager, &template)).collect();
    for pair in ids.windows(2) {
        connect(&mut manager, pair[0], pair[1]);
    }
    let mut info = ParticleInfo::default();
    info.lifetime = 2;
    info.alpha_keys[0].value = 1.0;
    for id in &ids {
        manager
            .admit_particle(*id, Particle::new(&info, 0, 0))
            .unwrap();
    }
    assert_eq!(manager.particle_count(), 3);
    let order = manager.active_system_ids_in_order();
    manager.destroy_particle_system(ids[0]);
    assert_eq!(manager.active_system_ids_in_order(), order);
    assert_eq!(manager.particle_count(), 3);
    manager.update(0, 1);
    for id in ids {
        let system = manager.find_particle_system(id).unwrap();
        assert!(system.is_destroyed());
        assert_eq!(system.particle_count(), 1);
        assert_eq!(system.particles()[0].lifetime_left, 1);
    }
    assert_eq!(manager.particle_count(), 3);
    manager.update(0, 2);
    assert_eq!(manager.active_system_count(), 0);
    assert_eq!(manager.system_count(), 0);
    assert_eq!(manager.particle_count(), 0);
}

#[test]
fn particle_destroy_traversal_missing_ids_stop_at_receiving_lookup() {
    let template = definition();
    let mut manager = ParticleSystemManager::new();
    let root = admit(&mut manager, &template);
    let unrelated = admit(&mut manager, &template);
    manager.destroy_particle_system(999);
    assert!(!manager.find_particle_system(root).unwrap().is_destroyed());
    manager
        .find_particle_system_mut(root)
        .unwrap()
        .set_slave(Some(999));
    manager.destroy_particle_system(root);
    assert!(manager.find_particle_system(root).unwrap().is_destroyed());
    assert!(
        !manager
            .find_particle_system(unrelated)
            .unwrap()
            .is_destroyed()
    );
    assert_eq!(manager.system_count(), 2);
    assert_eq!(
        manager
            .find_particle_system(root)
            .unwrap()
            .slave_system_id(),
        Some(999)
    );
}
