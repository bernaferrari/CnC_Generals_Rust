//! Scoped native weapon test inputs and exact admitted body lifetimes.

use super::*;
use crate::object::Object;
use crate::system::game_logic::GameLogic;
use crate::terrain::TerrainLogic;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

pub(super) struct ScopedWeaponFixture<'guard> {
    _isolation: &'guard std::sync::MutexGuard<'static, ()>,
    seed: [u32; 6],
    previous_terrain: Option<TerrainLogic>,
}

impl<'guard> ScopedWeaponFixture<'guard> {
    pub(super) fn new(isolation: &'guard std::sync::MutexGuard<'static, ()>) -> Self {
        Self {
            _isolation: isolation,
            seed: game_engine::common::random_value::get_game_logic_random_seed_state(),
            previous_terrain: None,
        }
    }

    pub(super) fn install_flat_terrain(&mut self, raw_height: u8) {
        let plane = loaded_los_terrain(raw_height);
        let previous = {
            let mut terrain = crate::terrain::get_terrain_logic().write().unwrap();
            std::mem::replace(&mut *terrain, plane)
        };
        // Retain the exact prior world, including bridge/water/waypoint state.
        // Repeated installation only replaces this fixture's temporary plane.
        if self.previous_terrain.is_none() {
            self.previous_terrain = Some(previous);
        }
    }
}

impl Drop for ScopedWeaponFixture<'_> {
    fn drop(&mut self) {
        if let Some(previous) = self.previous_terrain.take() {
            let mut terrain = crate::terrain::get_terrain_logic()
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            *terrain = previous;
        }
        crate::helpers::set_game_logic_random_seed(self.seed);
    }
}

/// These objects belong to one local GameLogic, not the canonical global
/// world's queue. Native weapon readers resolve their exact registered handles.
/// No factory callbacks, unrelated admissions or manager slots are installed.
pub(super) struct AdmittedWeaponBodies {
    owner: GameLogic,
    source: Arc<RwLock<Object>>,
    target: Arc<RwLock<Object>>,
    source_id: ObjectID,
    target_id: ObjectID,
}

impl AdmittedWeaponBodies {
    pub(super) fn new() -> Self {
        let source_id = 0x0BEA_0001;
        let target_id = 0x0BEA_0002;
        for id in [source_id, target_id] {
            assert!(
                crate::object::registry::OBJECT_REGISTRY
                    .get_object(id)
                    .is_none(),
                "weapon fixture must not replace another owner's ID {id}"
            );
        }
        let mut fixture = Self {
            owner: GameLogic::new(),
            source: Arc::new(RwLock::new(Object::new_test(source_id, 100.0))),
            target: Arc::new(RwLock::new(Object::new_test(target_id, 100.0))),
            source_id,
            target_id,
        };
        let position_result = fixture
            .target
            .write()
            .unwrap()
            .set_position(&Coord3D::new(20.0, 0.0, 0.0));
        position_result.expect("position laser victim");
        let source_admission = fixture.owner.register_object(Arc::clone(&fixture.source));
        source_admission.expect("admit actual laser source body");
        let target_admission = fixture.owner.register_object(Arc::clone(&fixture.target));
        target_admission.expect("admit actual laser target body");
        fixture
    }

    pub(super) fn source_id(&self) -> ObjectID {
        self.source_id
    }

    pub(super) fn target_id(&self) -> ObjectID {
        self.target_id
    }

    pub(super) fn source_health(&self) -> f32 {
        self.source.read().unwrap().get_health()
    }

    pub(super) fn target_health(&self) -> f32 {
        self.target.read().unwrap().get_health()
    }

    fn retire(&mut self) -> Result<(), String> {
        for (id, expected) in [
            (self.source_id, &self.source),
            (self.target_id, &self.target),
        ] {
            if let Some(admitted) = self.owner.find_object_by_id(id) {
                if !Arc::ptr_eq(&admitted, expected) {
                    return Err(format!("weapon fixture owner identity {id} changed"));
                }
                self.owner.destroy_object(id);
            }
        }
        // No object guard crosses canonical callbacks. The local owner contains
        // only this pair, so its destroy queue cannot consume a foreign world.
        self.owner
            .process_destroy_list()
            .map_err(|e| e.to_string())?;
        for (id, object) in [
            (self.source_id, &self.source),
            (self.target_id, &self.target),
        ] {
            if self.owner.find_object_by_id(id).is_some()
                || crate::object::registry::OBJECT_REGISTRY
                    .get_object(id)
                    .is_some()
            {
                return Err(format!("weapon fixture identity {id} survived retirement"));
            }
            let retired_id = object
                .read()
                .map_err(|_| "retired weapon body poisoned")?
                .get_id();
            if retired_id != INVALID_ID {
                return Err(format!("weapon fixture identity {id} was not finalized"));
            }
        }
        Ok(())
    }
}

impl Drop for AdmittedWeaponBodies {
    fn drop(&mut self) {
        let unwinding = std::thread::panicking();
        let result = catch_unwind(AssertUnwindSafe(|| self.retire()));
        if unwinding {
            if !matches!(result, Ok(Ok(()))) {
                eprintln!("exact weapon body retirement failed during assertion unwind");
            }
        } else {
            match result {
                Ok(result) => result.expect("exact weapon body retirement"),
                Err(error) => resume_unwind(error),
            }
        }
    }
}

#[test]
fn assertion_unwind_retires_exact_bodies_and_restores_rng_and_terrain() {
    // Keep the isolation guard outside catch_unwind so the deliberate panic
    // does not poison another test's synchronization boundary.
    let isolation = weapon_range_test_guard();
    let seed = game_engine::common::random_value::get_game_logic_random_seed_state();
    let terrain_height = crate::terrain::get_terrain_logic()
        .read()
        .unwrap()
        .get_ground_height(100.0, 100.0, None);
    let frame = TheGameLogic::get_frame();
    let mut pins = Vec::new();
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let mut fixture = ScopedWeaponFixture::new(&isolation);
        fixture.install_flat_terrain(40);
        let bodies = AdmittedWeaponBodies::new();
        pins.push((bodies.source_id, Arc::clone(&bodies.source)));
        pins.push((bodies.target_id, Arc::clone(&bodies.target)));
        let _ = crate::helpers::get_game_logic_random_value_real(0.0, 1.0);
        panic!("intentional weapon fixture assertion unwind");
    }));
    assert!(outcome.is_err());
    assert_eq!(pins.len(), 2);
    for (id, object) in pins {
        let retired_id = object.read().unwrap().get_id();
        assert_eq!(retired_id, INVALID_ID);
        assert!(
            crate::object::registry::OBJECT_REGISTRY
                .get_object(id)
                .is_none()
        );
    }
    assert_eq!(
        game_engine::common::random_value::get_game_logic_random_seed_state(),
        seed
    );
    let restored_height = crate::terrain::get_terrain_logic()
        .read()
        .unwrap()
        .get_ground_height(100.0, 100.0, None);
    assert_eq!(restored_height, terrain_height);
    assert_eq!(
        TheGameLogic::get_frame(),
        frame,
        "fixture never advances the foreign logic clock"
    );
}
