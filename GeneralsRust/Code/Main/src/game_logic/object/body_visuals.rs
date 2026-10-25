//! Main ActiveBody health-state and visual transitions.
//! CPP ActiveBody.cpp internalChangeHealth / setCorrectDamageState / evaluateVisualCondition.
use super::*;

/// CPP BodyModuleInterface::MaxHealthChangeType; gameplay callers select the
/// actual health operation instead of writing maximum and current independently.
#[derive(Clone, Copy)]
pub(crate) enum MaxHealthChange {
    PreserveRatio,
    AddToCurrent,
    KeepCurrent,
    FullyHeal,
}

impl Object {
    pub(crate) fn body_damage_policy(
        &self,
    ) -> crate::game_logic::body_damage_rules::BodyDamagePolicy {
        self.body_damage_policy
    }
    pub(crate) fn classify_body_damage(
        &self,
        health: f32,
        maximum: f32,
    ) -> crate::game_logic::host_enum_table_residual::HostBodyDamageType {
        self.body_damage_policy.classify(health, maximum)
    }
    /// A detached query clone is not a receiving lifetime. Admission rebinds
    /// immutable definitions before any runtime Body callback can run.
    pub(in crate::game_logic) fn bind_body_damage_policy(
        &mut self,
        policy: crate::game_logic::body_damage_rules::BodyDamagePolicy,
    ) {
        if self.body_damage_policy != policy {
            self.body_damage_policy = policy;
            self.restore_body_observation(None);
        }
    }

    /// Ordinary Body arithmetic (CPP ActiveBody.cpp:1188–1234), without
    /// damage/healing timestamps or onDamage/onHealing/onDie dispatch.
    pub(crate) fn internal_change_health(
        &mut self,
        delta: f32,
    ) -> crate::game_logic::host_enum_table_residual::HostBodyDamageType {
        self.internal_change_health_with_body_fx(delta, &mut Object::standalone_body_fx)
    }

    pub(in crate::game_logic) fn internal_change_health_with_body_fx(
        &mut self,
        delta: f32,
        body_fx: &mut crate::game_logic::object::BodyStateFxReceiver<'_>,
    ) -> crate::game_logic::host_enum_table_residual::HostBodyDamageType {
        let old_state = self.body_damage_state;
        if self.is_inactive_body() {
            return old_state;
        }
        self.previous_health = self.health.current;
        self.health.current += delta;
        if self.health.current > self.health.maximum {
            self.health.current = self.health.maximum;
        }
        if self.health.current < 0.0 {
            self.health.current = 0.0;
        }
        self.recalculate_body_damage_state();
        // CPP ActiveBody1213–1228: the native Drawable sees canonical HP/state
        // before effectivelyDead is overwritten, and before any onDamage walk.
        if self.body_damage_state != old_state && !self.status.under_construction {
            let state = self.body_damage_state;
            body_fx(
                self,
                crate::game_logic::object::BodyStateFx::NativeVisualState(state),
                None,
            );
        }
        self.rebuild_model_condition_bits();
        self.status.effectively_dead = self.health.current <= 0.0;
        old_state
    }

    /// Loading reconstructs observations, not health operations or callbacks.
    /// A saved AI-dead flag can coexist with positive HP and must stay set.
    pub(crate) fn restore_body_observation(
        &mut self,
        saved_state: Option<crate::game_logic::host_enum_table_residual::HostBodyDamageType>,
    ) {
        if self.is_inactive_body() {
            self.body_damage_state =
                crate::game_logic::host_enum_table_residual::HostBodyDamageType::Pristine;
            self.status.effectively_dead = true;
            self.rebuild_model_condition_bits();
            return;
        }
        self.body_damage_state = saved_state
            .unwrap_or_else(|| self.classify_body_damage(self.health.current, self.health.maximum));
        self.status.effectively_dead |= self.health.current <= 0.0;
        self.rebuild_model_condition_bits();
    }

    pub(crate) fn set_body_max_health_with_mode(&mut self, maximum: f32, mode: MaxHealthChange) {
        self.set_body_max_health_with_mode_and_body_fx(
            maximum,
            mode,
            &mut Object::standalone_body_fx,
        );
    }

    pub(in crate::game_logic) fn set_body_max_health_with_mode_and_body_fx(
        &mut self,
        maximum: f32,
        mode: MaxHealthChange,
        body_fx: &mut BodyStateFxReceiver<'_>,
    ) {
        if self.is_inactive_body() {
            return;
        }
        let old_maximum = self.health.maximum;
        self.health.maximum = maximum;
        self.max_health = maximum;
        self.initial_health = maximum;
        match mode {
            MaxHealthChange::PreserveRatio => {
                let target = maximum * (self.health.current / old_maximum);
                self.internal_change_health_with_body_fx(target - self.health.current, body_fx);
            }
            MaxHealthChange::AddToCurrent => {
                self.internal_change_health_with_body_fx(maximum - old_maximum, body_fx);
            }
            MaxHealthChange::KeepCurrent => {}
            MaxHealthChange::FullyHeal => {
                self.internal_change_health_with_body_fx(maximum - self.health.current, body_fx);
            }
        }
        if self.health.current > maximum {
            self.internal_change_health_with_body_fx(maximum - self.health.current, body_fx);
        }
    }

    fn recalculate_body_damage_state(
        &mut self,
    ) -> (
        crate::game_logic::host_enum_table_residual::HostBodyDamageType,
        crate::game_logic::host_enum_table_residual::HostBodyDamageType,
    ) {
        use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
        let health = self.health.current;
        let max_h = if self.health.maximum > 0.0 {
            self.health.maximum
        } else {
            self.max_health.max(0.0)
        };
        let old_state = self.body_damage_state;
        if self.is_inactive_body() {
            self.body_damage_state = HostBodyDamageType::Pristine;
            return (old_state, HostBodyDamageType::Pristine);
        }
        let state = if self.status.destroyed || health <= 0.0 {
            HostBodyDamageType::Rubble
        } else {
            self.classify_body_damage(health, max_h)
        };
        self.body_damage_state = state;
        // C++ ActiveBody::setCorrectDamageState rubble (ActiveBody.cpp:189-208).
        if matches!(state, HostBodyDamageType::Rubble)
            && !matches!(old_state, HostBodyDamageType::Rubble)
        {
            self.apply_structure_rubble_collision_effects();
        }

        (old_state, state)
    }

    pub(in crate::game_logic) fn refresh_model_condition_bits_with_source(
        &mut self,
        source: Option<&crate::game_logic::host_transition_damage_fx::HostDamageFxVictim>,
        frame: u32,
    ) {
        self.refresh_model_condition_bits_with_body_fx(
            source,
            frame,
            &mut Object::standalone_body_fx,
        );
    }

    pub(in crate::game_logic) fn refresh_model_condition_bits_with_body_fx(
        &mut self,
        source: Option<&crate::game_logic::host_transition_damage_fx::HostDamageFxVictim>,
        frame: u32,
        body_fx: &mut BodyStateFxReceiver<'_>,
    ) {
        if self.is_inactive_body() {
            self.status.effectively_dead = true;
            self.rebuild_model_condition_bits();
            return;
        }
        let (old_state, _) = self.recalculate_body_damage_state();
        self.status.effectively_dead = self.health.current <= 0.0;
        self.react_to_body_damage_state_change_with_body_fx(old_state, source, frame, body_fx);
    }

    /// Timed DamageModule callbacks observe the already committed Body. CPP
    /// ActiveBody603 rereads its current state against the pre-mutation state.
    pub(super) fn react_to_body_damage_state_change(
        &mut self,
        old_state: crate::game_logic::host_enum_table_residual::HostBodyDamageType,
        source: Option<&crate::game_logic::host_transition_damage_fx::HostDamageFxVictim>,
        frame: u32,
    ) {
        self.react_to_body_damage_state_change_with_transition_fx(
            old_state,
            source,
            frame,
            &mut |object, event, source| {
                object.queue_transition_damage_fx_event_with_source(event, source)
            },
        );
    }

    /// The driving session supplies synchronous FX playback while the Body is
    /// borrowed; standalone Object callers retain their existing adapter.
    pub(super) fn react_to_body_damage_state_change_with_transition_fx(
        &mut self,
        old_state: crate::game_logic::host_enum_table_residual::HostBodyDamageType,
        source: Option<&crate::game_logic::host_transition_damage_fx::HostDamageFxVictim>,
        frame: u32,
        transition_fx: &mut dyn FnMut(
            &mut Object,
            crate::game_logic::host_transition_damage_fx::HostTransitionDamageFxEvent,
            Option<&crate::game_logic::host_transition_damage_fx::HostDamageFxVictim>,
        ),
    ) {
        self.react_to_body_damage_state_change_with_body_fx(
            old_state,
            source,
            frame,
            &mut |object, effect, source| match effect {
                BodyStateFx::Transition(event) => transition_fx(object, event, source),
                effect => Object::standalone_body_fx(object, effect, source),
            },
        );
    }

    pub(super) fn react_to_body_damage_state_change_with_body_fx(
        &mut self,
        old_state: crate::game_logic::host_enum_table_residual::HostBodyDamageType,
        source: Option<&crate::game_logic::host_transition_damage_fx::HostDamageFxVictim>,
        frame: u32,
        body_fx: &mut BodyStateFxReceiver<'_>,
    ) {
        use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
        let state = self.body_damage_state;

        // C++ GarrisonContain::onBodyDamageStateChange (GarrisonContain.cpp:1724-1732).
        if old_state != state
            && matches!(state, HostBodyDamageType::ReallyDamaged)
            && self.is_garrison_contain()
            && !self.is_kind_of(crate::game_logic::KindOf::GarrisonableUntilDestroyed)
            && !self.contained_units().is_empty()
        {
            self.status.pending_garrison_really_damaged_eject = true;
        }
        // C++ ActiveBody.cpp:1219 — skip evaluateVisualCondition while
        // OBJECT_STATUS_UNDER_CONSTRUCTION (no damaged-art health visuals).
        let show_health_visuals = !self.status.under_construction || self.status.destroyed;
        // C++ BoneFXDamage::onBodyDamageStateChange residual.
        if old_state != state && show_health_visuals {
            if self.bone_fx_damage.is_none() {
                self.bone_fx_damage =
                    crate::game_logic::host_bone_fx_damage::HostBoneFxDamageData::from_template(
                        &self.template_name,
                    );
            }
            let retired = self
                .bone_fx_damage
                .as_mut()
                .map(|bfx| bfx.prepare_body_state_retirement(state))
                .unwrap_or_else(
                    crate::game_logic::host_bone_fx_damage::BoneParticleRetirement::empty,
                );
            if !retired.is_empty() {
                body_fx(self, BodyStateFx::BoneParticleRetirement(retired), source);
            }
            if let Some(bfx) = self.bone_fx_damage.as_mut() {
                bfx.stamp_last_damage_type(self.last_damage_info_type);
            }
            body_fx(
                self,
                BodyStateFx::BoneStateChange {
                    old_state,
                    new_state: state,
                    frame,
                },
                source,
            );
        }
        if old_state != state && show_health_visuals {
            self.ensure_transition_damage_fx();
            if let Some(cfg) = self.transition_damage_fx.as_ref() {
                if let Some(ev) =
                    crate::game_logic::host_transition_damage_fx::on_body_damage_state_change_for_damage(
                        cfg,
                        old_state,
                        state,
                        self.last_damage_info_type.map(|d| d.to_store()),
                    )
                {
                    body_fx(self, BodyStateFx::Transition(ev), source);
                }
            }
        }

        self.rebuild_model_condition_bits();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::host_bone_fx_damage::{
        HostBoneFxAuthored, HostBoneFxDamageData, HostBoneFxSlot,
    };
    use crate::game_logic::host_create_object_die::HostCreateObjectDieData;
    use crate::game_logic::host_enum_table_residual::HostBodyDamageType;

    fn body() -> Object {
        let mut template = ThingTemplate::new("OrdinaryBodyHealthOwner");
        template.set_health(100.0);
        let mut object = Object::new(template, ObjectId(7), Team::USA);
        let mut authored = HostBoneFxAuthored::default();
        authored.fx[HostBodyDamageType::Rubble.ordinal() as usize][0] = Some(HostBoneFxSlot {
            bone: "BodyBone".into(),
            only_once: true,
            delay_min: 0.0,
            delay_max: 0.0,
            name: "BodyCommitRubbleFx".into(),
        });
        object.bone_fx_damage = Some(HostBoneFxDamageData::from_authored(authored));
        object.create_object_die = Some(HostCreateObjectDieData {
            ocl_name: "NeverAnOrdinaryHealthDeath".into(),
            spawn_templates: vec!["NeverAnOrdinaryHealthDeath".into()],
            transfer_previous_health: true,
            fired: false,
        });
        object.last_damage_timestamp = Some(9);
        object.last_healing_timestamp = Some(11);
        object
    }

    #[test]
    fn ordinary_health_clamps_and_classifies_without_damage_or_death_callbacks() {
        let mut object = body();
        object.internal_change_health(-120.0);
        assert_eq!(
            (object.previous_health, object.health.current),
            (100.0, 0.0)
        );
        assert_eq!(object.body_damage_state, HostBodyDamageType::Rubble);
        assert!(object.status.effectively_dead);
        assert_eq!(object.bone_fx_damage.as_ref().unwrap().transitions, 0);
        assert!(!object.create_object_die.as_ref().unwrap().fired);
        assert!(object.pending_create_object_die_spawns.is_empty());
        assert_eq!(object.last_damage_timestamp, Some(9));
        assert_eq!(object.last_healing_timestamp, Some(11));
        object.internal_change_health(150.0);
        assert_eq!(
            (object.previous_health, object.health.current),
            (0.0, 100.0)
        );
        assert_eq!(object.body_damage_state, HostBodyDamageType::Pristine);
        assert!(!object.status.effectively_dead);
    }

    #[test]
    fn shared_body_mutation_commits_state_before_timed_reaction() {
        let mut object = body();
        let old_state = object.internal_change_health(-100.0);
        assert_eq!(old_state, HostBodyDamageType::Pristine);
        assert_eq!(object.body_damage_state, HostBodyDamageType::Rubble);
        assert!(object.status.effectively_dead);
        assert_eq!(object.previous_health, 100.0);
        assert_eq!(object.bone_fx_damage.as_ref().unwrap().transitions, 0);
        assert!(!object.create_object_die.as_ref().unwrap().fired);
        object.react_to_body_damage_state_change(old_state, None, 123);
        assert!(object.bone_fx_damage.as_ref().unwrap().transitions > 0);
        // ActiveBody585-614 dispatches DamageModule state callbacks only.
        // Object::onDie4548-4566 owns CreateObjectDie after kill credit.
        assert!(!object.create_object_die.as_ref().unwrap().fired);
        assert!(object.pending_create_object_die_spawns.is_empty());
        object.complete_standalone_die_with_body_fx(None, &mut Object::standalone_body_fx);
        assert!(object.create_object_die.as_ref().unwrap().fired);
        assert!(!object.pending_create_object_die_spawns.is_empty());
    }

    #[test]
    fn max_health_modes_preserve_the_original_distinct_operations() {
        for (mode, expected) in [
            (MaxHealthChange::PreserveRatio, 80.0),
            (MaxHealthChange::AddToCurrent, 140.0),
            (MaxHealthChange::KeepCurrent, 40.0),
            (MaxHealthChange::FullyHeal, 200.0),
        ] {
            let mut object = body();
            object.internal_change_health(-60.0);
            object.set_body_max_health_with_mode(200.0, mode);
            assert_eq!(object.health.current, expected);
            assert_eq!(
                (
                    object.health.maximum,
                    object.initial_health,
                    object.max_health
                ),
                (200.0, 200.0, 200.0)
            );
            assert_eq!(object.bone_fx_damage.as_ref().unwrap().transitions, 0);
            assert_eq!(object.last_healing_timestamp, Some(11));
        }
        let mut object = body();
        object.set_body_max_health_with_mode(30.0, MaxHealthChange::KeepCurrent);
        assert_eq!(
            (object.previous_health, object.health.current),
            (100.0, 30.0)
        );
    }

    #[test]
    fn construction_health_updates_canonical_body_before_art_admission() {
        let mut object = body();
        object.set_status_under_construction(true);
        object.internal_change_health(-99.0);
        assert_eq!(object.body_damage_state, HostBodyDamageType::ReallyDamaged);
        assert_eq!(object.bone_fx_damage.as_ref().unwrap().transitions, 0);
        object.internal_change_health(79.0);
        assert_eq!((object.previous_health, object.health.current), (1.0, 80.0));
        assert_eq!(object.body_damage_state, HostBodyDamageType::Pristine);
        object.set_status_under_construction(false);
        assert_eq!(object.bone_fx_damage.as_ref().unwrap().transitions, 0);
    }
    #[test]
    fn early_visual_observation_precedes_effectively_dead_and_is_not_damage_module_completion() {
        let mut object = body();
        let mut calls = Vec::new();
        let old = object.internal_change_health_with_body_fx(-100.0, &mut |object, effect, _| {
            if let crate::game_logic::object::BodyStateFx::NativeVisualState(state) = effect {
                assert_eq!(object.health.current, 0.0);
                assert!(
                    !object.status.effectively_dead,
                    "CPP visual callback precedes setEffectivelyDead"
                );
                assert!(
                    !object.status.destroyed,
                    "ordinary health mutation is not onDie"
                );
                calls.push(state);
            }
        });
        assert_eq!(old, HostBodyDamageType::Pristine);
        assert_eq!(calls, vec![HostBodyDamageType::Rubble]);
        assert!(object.status.effectively_dead);
        assert!(!object.create_object_die.as_ref().unwrap().fired);
        object.internal_change_health_with_body_fx(0.0, &mut |_, _, _| {
            panic!("unchanged state must not replay native visual")
        });
        object.restore_body_observation(None);
        assert_eq!(calls.len(), 1, "save observation is not a health callback");
    }
}

#[cfg(test)]
#[path = "body_visual_trace_tests.rs"]
mod body_visual_trace_tests;
