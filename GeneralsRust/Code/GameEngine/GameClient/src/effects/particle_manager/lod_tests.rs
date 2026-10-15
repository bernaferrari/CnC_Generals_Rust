//! The real Common LOD owner's counter drives the actual admission loop.
use super::*;
use game_engine::common::ini::{
    DynamicGameLODLevel, GameLODManager, ParticlePriorityType as IniPriority,
};

fn lod(minimum: IniPriority, skip_minimum: IniPriority, mask: u32) -> GameLODManager {
    let mut lod = GameLODManager::new();
    let info = &mut lod.dynamic_game_lod_info[DynamicGameLODLevel::Low.to_index().unwrap()];
    info.min_dynamic_particle_priority = minimum;
    info.min_dynamic_particle_skip_priority = skip_minimum;
    info.dynamic_particle_skip_mask = mask;
    assert!(lod.set_dynamic_lod_level(DynamicGameLODLevel::Low));
    lod
}

fn owner(slave: bool) -> (ParticleSystemManager, ParticleSystemId) {
    let mut manager = ParticleSystemManager::new();
    for (name, priority) in [
        ("OwnedLodMaster", ParticlePriorityType::Constant),
        ("OwnedLodSlave", ParticlePriorityType::AlwaysRender),
    ] {
        let mut definition = ParticleSystemTemplate::new(name.into());
        let info = definition.info_mut();
        info.priority = priority;
        info.burst_count = GameClientRandomVariable::new(3.0, 3.0);
        info.burst_delay = GameClientRandomVariable::new(0.0, 0.0);
        info.lifetime = GameClientRandomVariable::new(100.0, 100.0);
        for key in &mut info.alpha_keys[..2] {
            key.min_value = 1.0;
            key.max_value = 1.0;
        }
        info.alpha_keys[1].frame = 100;
        if slave && name == "OwnedLodMaster" {
            info.slave_system_name = "OwnedLodSlave".into();
        }
        manager.register_template(Arc::new(definition));
    }
    let definition = manager.find_template("OwnedLodMaster").unwrap();
    let id = manager
        .create_particle_system_with_random(&definition, slave, &mut |lo, _| lo)
        .unwrap();
    (manager, id)
}

#[test]
fn owned_particle_lod_reject_and_always_render_do_not_advance_skip_counter() {
    let mut owner = lod(IniPriority::Critical, IniPriority::AlwaysRender, 3);
    assert!(owner.should_skip(ParticlePriorityType::Constant));
    assert!(!owner.should_skip(ParticlePriorityType::AlwaysRender));
    // Neither branch consumes (++generation & mask): these are draws1,2,3.
    assert!(owner.is_particle_skipped());
    assert!(owner.is_particle_skipped());
    assert!(!owner.is_particle_skipped());
}

#[test]
fn owned_particle_lod_actual_admission_keeps_same_id_peers_independent() {
    let (mut first, id) = owner(false);
    let (mut peer, peer_id) = owner(false);
    assert_eq!(id, peer_id);
    let mut first_lod = lod(IniPriority::WeaponExplosion, IniPriority::Critical, 3);
    let mut peer_lod = lod(IniPriority::Critical, IniPriority::AlwaysRender, 0);
    first.update_with_lod(0, 1, &mut first_lod);
    assert_eq!(first.find_particle_system(id).unwrap().particle_count(), 1);
    peer.update_with_lod(0, 1, &mut peer_lod);
    assert_eq!(peer.find_particle_system(id).unwrap().particle_count(), 0);
    assert_eq!(first.find_particle_system(id).unwrap().particle_count(), 1);
    // The duplicate-frame guard precedes admission; it cannot draw another mask.
    first.update_with_lod(0, 1, &mut first_lod);
    assert!(first_lod.is_particle_skipped(), "next real draw is4");
    assert!(first_lod.is_particle_skipped(), "next real draw is5");
    assert!(first_lod.is_particle_skipped(), "next real draw is6");
    assert!(!first_lod.is_particle_skipped(), "next real draw is7");
}

#[test]
fn owned_particle_lod_slave_gate_uses_same_owner_and_master_priority() {
    let (mut manager, id) = owner(true);
    let slave = manager
        .find_particle_system(id)
        .unwrap()
        .slave_system_id()
        .unwrap();
    let mut lod = lod(IniPriority::WeaponExplosion, IniPriority::Critical, 1);
    manager.update_with_lod(0, 1, &mut lod);
    assert_eq!(
        manager.find_particle_system(id).unwrap().particle_count(),
        3
    );
    assert_eq!(
        manager
            .find_particle_system(slave)
            .unwrap()
            .particle_count(),
        0,
        "each master passes odd draws1/3/5; each slave rejects even2/4/6 using master priority"
    );
    assert!(
        !lod.is_particle_skipped(),
        "next draw is7 after all six gates"
    );
}

#[test]
fn owned_particle_admission_field_rejection_cannot_evict_existing_particles() {
    let (mut manager, id) = owner(false);
    manager.set_fallback_particle_limits(2, 1);
    let maximum = manager.live_max_particle_count();
    assert!(maximum < 100_000, "bounded fixture setup");
    for personality in 0..=maximum {
        let info = crate::effects::particle_system::ParticleInfo::default();
        manager
            .admit_particle(
                id,
                crate::effects::particle_system::Particle::new(&info, personality as u32, 0),
            )
            .unwrap();
    }
    // A completed presentation observation, not a mutation of living totals.
    manager.field_particle_count = manager.live_max_field_particle_count() + 1;
    let before = manager.particle_count();
    let mut lod = lod(IniPriority::WeaponExplosion, IniPriority::Critical, 0);
    assert!(!manager.can_create_particle_for_system_with_lod(
        ParticlePriorityType::AreaEffect,
        true,
        maximum + 1,
        false,
        &mut lod
    ));
    assert_eq!(
        manager.particle_count(),
        before,
        "C++ field rejection occurs before the global-cap eviction path"
    );
    assert_eq!(
        manager.find_particle_system(id).unwrap().particles()[0].personality,
        0
    );
}

#[test]
fn owned_particle_admission_zero_cap_evicts_before_rejecting_new_particle() {
    let (mut manager, id) = owner(false);
    let info = crate::effects::particle_system::ParticleInfo::default();
    manager
        .admit_particle(
            id,
            crate::effects::particle_system::Particle::new(&info, 0, 0),
        )
        .unwrap();
    assert!(!manager.enforce_particle_limit(ParticlePriorityType::Critical, 0));
    assert_eq!(
        manager.particle_count(),
        0,
        "C++ evicts the existing lower-priority particle before its zero-cap rejection"
    );
    assert_eq!(
        manager.find_particle_system(id).unwrap().particle_count(),
        0
    );
}
