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
        campaign_context: crate::save_load::CampaignMapContext,
        mut report_progress: F,
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
            campaign_context,
            |_, _audio_policy, value, phase| {
                report_progress(value, phase);
                Ok(())
            },
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
        campaign_context: crate::save_load::CampaignMapContext,
        mut report_progress: F,
    ) -> Result<bool, String>
    where
        F: FnMut(
            &mut game_engine::common::random_value::RandomState,
            &game_engine::common::audio::script_policy::ScriptAudioPolicy,
            f32,
            &str,
        ) -> Result<(), String>,
    {
        // A loaned interpreter or missing accepted baseline cannot authorize
        // destructive replacement of any outgoing attempt.
        self.validate_map_replacement_preflight()?;
        let kind = self.begin_map_admission_attempt()?;
        let result = (|| {
            self.retire_map_attempt_objects(&mut native)?;
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
                campaign_context,
                &mut report_progress,
            )?;
            if accepted && kind.is_new() {
                if let Some(native) = native.as_mut() {
                    native.on_level_start(self)?;
                }
            }
            if accepted {
                // The final presentation belongs inside the attempt: an error
                // must retire its Objects before discarding native resources.
                report_progress(
                    &mut self.session_view.audio_random,
                    self.queued_audio_events.policy(),
                    1.0,
                    "Map load complete",
                )?;
            }
            Ok(accepted)
        })();
        // Cleanup and the owned phase restoration both run on rejection/error.
        // In particular cleanup failure cannot strand this world in NewMap.
        let cleanup = if matches!(&result, Ok(true)) {
            Ok(())
        } else {
            // Failed content may already own Objects. Their onDelete/native
            // phases precede candidate resource discard, just as on retry.
            // Terminal failure returns before discard and never retries it.
            self.retire_map_attempt_objects(&mut native).and_then(|()| {
                if let Some(native) = native.as_mut() {
                    native.discard_attempt()
                } else {
                    Ok(())
                }
            })
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

    pub(crate) fn validate_map_replacement_preflight(&self) -> Result<(), String> {
        if self.has_unregistered_host_objects() {
            return Err("map admission cannot replace an unfinished Object constructor".into());
        }
        if self.receiving_module_content.is_some() && self.receiving_template_baseline.is_none() {
            return Err("Receiving map admission has no accepted host template baseline".into());
        }
        if self.owned_script_engine.is_none() {
            return Err("Map admission requires the receiving ScriptEngine to be returned".into());
        }
        if let Some(error) = self.object_retirement_error() {
            return Err(error.to_string());
        }
        if self.object_lifecycle_callback_depth != 0 {
            return Err("map admission cannot replace an active Object lifecycle callback".into());
        }
        Ok(())
    }

    fn retire_map_attempt_objects(
        &mut self,
        native: &mut Option<&mut dyn crate::game_logic::NativeMapAdmission>,
    ) -> Result<(), String> {
        if self.objects.registered_is_empty() && !self.has_pending_object_removals() {
            return Ok(());
        }
        if let Some(native) = native.as_mut() {
            self.destroy_all_objects_immediate_with_effects_checked(&mut native.effects())
                .map_err(|error| error.to_string())
        } else {
            self.destroy_all_objects_immediate_with_effects_checked(&mut Default::default())
                .map_err(|error| error.to_string())
        }
    }
}

#[cfg(test)]
mod loading_gate_failure_tests {
    use super::*;
    struct FailedCleanup {
        calls: usize,
        effects_calls: usize,
        effects_seen_at_discard: Vec<usize>,
    }
    impl crate::game_logic::NativeMapAdmission for FailedCleanup {
        fn discard_attempt(&mut self) -> Result<(), String> {
            self.calls += 1;
            self.effects_seen_at_discard.push(self.effects_calls);
            Err("receiving retirement rejected".into())
        }
        fn on_level_start(&mut self, _: &GameLogic) -> Result<(), String> {
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
            self.effects_calls += 1;
            Default::default()
        }
    }
    #[test]
    fn failed_native_cleanup_restores_driving_gate_and_preserves_save_outer_gate() {
        for saved in [false, true] {
            let mut world = GameLogic::new();
            if saved {
                world.begin_saved_snapshot_admission().unwrap();
            }
            let mut native = FailedCleanup {
                calls: 0,
                effects_calls: 0,
                effects_seen_at_discard: Vec::new(),
            };
            let result = world.load_map_with_definition_source_and_progress_checked(
                "unused-after-retirement-rejection",
                None,
                None,
                None,
                Some(&mut native),
                crate::save_load::CampaignMapContext::default(),
                |_, _, _, _| Ok(()),
            );
            assert!(
                result
                    .unwrap_err()
                    .contains("receiving retirement rejected")
            );
            assert_eq!(native.calls, 2);
            assert_eq!(
                native.effects_calls, 0,
                "empty attempt need not borrow native owners"
            );
            assert!(!world.is_loading_map());
            assert_eq!(world.is_loading_saved_snapshot(), saved);
            if saved {
                world.finish_saved_snapshot_admission().unwrap();
            }
        }
    }

    #[test]
    fn outgoing_attempt_retires_before_native_discard_and_keeps_peer() {
        use crate::game_logic::{Object, ObjectId, Team, ThingTemplate};
        let mut owner = GameLogic::new();
        let mut peer = GameLogic::new();
        for world in [&mut owner, &mut peer] {
            world.add_object(Object::new(
                ThingTemplate::new("AttemptOwner"),
                ObjectId(7),
                Team::USA,
            ));
        }
        owner.frame = 91;
        let next_id = owner.next_object_id_for_snapshot();
        let mut native = FailedCleanup {
            calls: 0,
            effects_calls: 0,
            effects_seen_at_discard: Vec::new(),
        };
        let error = owner
            .load_map_with_definition_source_and_progress_checked(
                "unused-after-discard-rejection",
                None,
                None,
                None,
                Some(&mut native),
                crate::save_load::CampaignMapContext::default(),
                |_, _, _, _| Ok(()),
            )
            .unwrap_err();
        assert!(error.contains("receiving retirement rejected"));
        assert_eq!(
            native.effects_calls, 1,
            "logical retirement borrows its receiving effect owner before discard"
        );
        assert_eq!(native.calls, 2);
        assert_eq!(
            native.effects_seen_at_discard,
            vec![1, 1],
            "both discard observations follow logical retirement"
        );
        assert!(owner.objects.registered_is_empty());
        assert!(!owner.has_pending_object_removals());
        assert_eq!(owner.frame, 91);
        assert_eq!(
            owner.next_object_id_for_snapshot(),
            next_id,
            "rejected discard does not reset allocation"
        );
        assert!(!peer.objects[&ObjectId(7)].status.destroyed);
        assert!(!owner.is_loading_map());
    }

    #[test]
    fn map_replacement_preflight_never_destroys_when_interpreter_is_loaned() {
        use crate::game_logic::{Object, ObjectId, Team, ThingTemplate};
        let mut owner = GameLogic::new();
        owner.add_object(Object::new(
            ThingTemplate::new("AttemptOwner"),
            ObjectId(7),
            Team::USA,
        ));
        let engine = owner.owned_script_engine.take();
        let mut native = FailedCleanup {
            calls: 0,
            effects_calls: 0,
            effects_seen_at_discard: Vec::new(),
        };
        let error = owner
            .load_map_with_definition_source_and_progress_checked(
                "unused-loaned-script",
                None,
                None,
                None,
                Some(&mut native),
                crate::save_load::CampaignMapContext::default(),
                |_, _, _, _| Ok(()),
            )
            .unwrap_err();
        assert!(error.contains("ScriptEngine"));
        assert!(!owner.objects[&ObjectId(7)].status.destroyed);
        assert_eq!((native.effects_calls, native.calls), (0, 0));
        assert!(!owner.has_pending_object_removals());
        assert!(!owner.is_loading_map());
        owner.owned_script_engine = engine;
    }
}
