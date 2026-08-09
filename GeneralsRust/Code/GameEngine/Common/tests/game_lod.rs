#![cfg(feature = "internal")]
//! GameLODManager debris/particle skip parity with C++ GameLOD.h.

use game_engine::common::ini::ini_game_lod::{
    DynamicGameLODLevel, GameLODManager, ParticlePriorityType,
};

#[test]
fn skip_mask_zero_never_skips() {
    let mut manager = GameLODManager::new();
    for _ in 0..32 {
        assert!(!manager.is_debris_skipped());
        assert!(!manager.is_particle_skipped());
    }
}

#[test]
fn skip_mask_one_skips_every_other_call_like_cpp() {
    let mut manager = GameLODManager::new();
    let low = DynamicGameLODLevel::Low.to_index().unwrap();
    manager.dynamic_game_lod_info[low].dynamic_debris_skip_mask = 1;
    manager.dynamic_game_lod_info[low].dynamic_particle_skip_mask = 1;
    manager.dynamic_game_lod_info[low].slow_death_scale = 0.25;
    manager.dynamic_game_lod_info[low].min_dynamic_particle_priority =
        ParticlePriorityType::ScorchMark;
    manager.dynamic_game_lod_info[low].min_dynamic_particle_skip_priority =
        ParticlePriorityType::AreaEffect;

    assert!(manager.set_dynamic_lod_level(DynamicGameLODLevel::Low));
    assert_eq!(manager.get_slow_death_scale(), 0.25);
    assert_eq!(
        manager.get_min_dynamic_particle_priority(),
        ParticlePriorityType::ScorchMark
    );
    assert_eq!(
        manager.get_min_dynamic_particle_skip_priority(),
        ParticlePriorityType::AreaEffect
    );

    // C++: (++n & 1) != 1 → keep, skip, keep, skip...
    assert!(!manager.is_debris_skipped());
    assert!(manager.is_debris_skipped());
    assert!(!manager.is_debris_skipped());
    assert!(manager.is_debris_skipped());

    assert!(!manager.is_particle_skipped());
    assert!(manager.is_particle_skipped());
    assert!(!manager.is_particle_skipped());
    assert!(manager.is_particle_skipped());
}

#[test]
fn set_dynamic_lod_level_copies_masks_from_info() {
    let mut manager = GameLODManager::new();
    let low = DynamicGameLODLevel::Low.to_index().unwrap();
    manager.dynamic_game_lod_info[low].dynamic_debris_skip_mask = 7;
    manager.dynamic_game_lod_info[low].dynamic_particle_skip_mask = 3;

    assert_eq!(manager.get_dynamic_lod_level(), DynamicGameLODLevel::High);
    assert!(manager.set_dynamic_lod_level(DynamicGameLODLevel::Low));
    assert_eq!(manager.get_dynamic_lod_level(), DynamicGameLODLevel::Low);

    // mask=7 (0b111) keeps when (++n & 7) == 7, i.e. every 8th generation.
    let debris: Vec<bool> = (0..8).map(|_| manager.is_debris_skipped()).collect();
    assert_eq!(
        debris,
        vec![true, true, true, true, true, true, false, true]
    );

    let particles: Vec<bool> = (0..4).map(|_| manager.is_particle_skipped()).collect();
    // mask=3 keeps when (++n & 3) == 3.
    assert_eq!(particles, vec![true, true, false, true]);
}
