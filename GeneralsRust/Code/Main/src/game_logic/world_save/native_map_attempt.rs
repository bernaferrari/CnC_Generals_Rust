//! Candidate effects belong to one complete map attempt, including early failure.
use super::*;

impl GameLogic {
    pub(super) fn load_map_with_definition_source_and_progress<F>(
        &mut self,
        map_name: &str,
        definition_source: Option<&str>,
        map_script_difficulty: Option<gamelogic::player::GameDifficulty>,
        water_delivery: Option<&mut dyn FnMut(&gamelogic::terrain_water::WaterGridState)>,
        mut native: Option<&mut dyn crate::game_logic::NativeMapAdmission>,
        report_progress: F,
    ) -> bool
    where
        F: FnMut(f32, &str),
    {
        self.load_map_with_definition_source_and_progress_checked(
            map_name,
            definition_source,
            map_script_difficulty,
            water_delivery,
            native,
            report_progress,
        )
        .unwrap_or_else(|error| {
            log::error!("Map admission failed: {error}");
            false
        })
    }

    pub(super) fn load_map_with_definition_source_and_progress_checked<F>(
        &mut self,
        map_name: &str,
        definition_source: Option<&str>,
        map_script_difficulty: Option<gamelogic::player::GameDifficulty>,
        water_delivery: Option<&mut dyn FnMut(&gamelogic::terrain_water::WaterGridState)>,
        mut native: Option<&mut dyn crate::game_logic::NativeMapAdmission>,
        report_progress: F,
    ) -> Result<bool, String>
    where
        F: FnMut(f32, &str),
    {
        let kind = self.begin_map_admission_attempt()?;
        let result = (|| {
            if let Some(native) = native.as_mut() {
                native.discard_attempt()?;
            }
            let accepted = self.load_map_contents_with_definition_source_and_progress(
                map_name,
                definition_source,
                map_script_difficulty,
                water_delivery,
                native
                    .as_mut()
                    .map(|native| &mut **native as &mut dyn crate::game_logic::NativeMapAdmission),
                report_progress,
            );
            if accepted && kind.is_new() {
                if let Some(native) = native.as_mut() {
                    native.on_level_start()?;
                }
            }
            Ok(accepted)
        })();
        // Cleanup and the owned phase restoration both run on rejection/error.
        // In particular cleanup failure cannot strand this world in NewMap.
        let cleanup = if matches!(&result, Ok(true)) {
            Ok(())
        } else if let Some(native) = native.as_mut() {
            native.discard_attempt()
        } else {
            Ok(())
        };
        let finish = self.finish_map_admission_attempt(kind);
        match (result, cleanup, finish) {
            (Ok(accepted), Ok(()), Ok(())) => Ok(accepted),
            (Err(error), Ok(()), Ok(())) => Err(error),
            (result, cleanup, finish) => Err(format!(
                "map admission failed: result={result:?}; cleanup={cleanup:?}; phase={finish:?}"
            )),
        }
    }
}

#[cfg(test)]
mod loading_gate_failure_tests {
    use super::*;
    struct FailedCleanup {
        calls: usize,
    }
    impl crate::game_logic::NativeMapAdmission for FailedCleanup {
        fn discard_attempt(&mut self) -> Result<(), String> {
            self.calls += 1;
            Err("receiving retirement rejected".into())
        }
        fn on_level_start(&mut self) -> Result<(), String> {
            panic!("failed admission cannot start level")
        }
        fn prepare(
            &mut self,
            _: &GameLogic,
            _: Option<&crate::game_logic::script_loader::MapMetadata>,
        ) -> Result<(), String> {
            panic!("cleanup rejection precedes preparation")
        }
        fn admit_restored_object_drawables(&mut self, _: &mut GameLogic) -> Result<(), String> {
            panic!("cleanup rejection cannot reconstruct saved Drawables")
        }
        fn admit_restored_heights(&mut self, _: &GameLogic) -> Result<(), String> {
            panic!("not a restore")
        }
        fn effects(&mut self) -> crate::game_logic::ScriptPresentationEffects<'_> {
            panic!("cleanup rejection cannot lend effects")
        }
    }
    #[test]
    fn failed_native_cleanup_restores_driving_gate_and_preserves_save_outer_gate() {
        for saved in [false, true] {
            let mut world = GameLogic::new();
            if saved {
                world.begin_saved_snapshot_admission().unwrap();
            }
            let mut native = FailedCleanup { calls: 0 };
            let result = world.load_map_with_definition_source_and_progress_checked(
                "unused-after-retirement-rejection",
                None,
                None,
                None,
                Some(&mut native),
                |_, _| {},
            );
            assert!(
                result
                    .unwrap_err()
                    .contains("receiving retirement rejected")
            );
            assert_eq!(native.calls, 2);
            assert!(!world.is_loading_map());
            assert_eq!(world.is_loading_saved_snapshot(), saved);
            if saved {
                world.finish_saved_snapshot_admission().unwrap();
            }
        }
    }
}
