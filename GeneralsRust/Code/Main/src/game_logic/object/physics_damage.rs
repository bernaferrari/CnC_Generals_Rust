//! Immediate Physics Body effects with services borrowed from the receiving owner.
use super::*;

/// Read-only source observations captured while both objects remain in the receiving map.
#[derive(Default)]
pub(in crate::game_logic) struct PhysicsDamageSources {
    pub players: [Option<(ObjectId, u32)>; 2],
    pub observations: [Option<(ObjectId, LastDamageSourceObservation)>; 2],
    /// Bounded values captured from this receiving store before its body loan.
    pub effects: [Option<crate::game_logic::host_transition_damage_fx::HostDamageFxVictim>; 2],
    #[cfg(feature = "game_client")]
    pub poses: [Option<gamelogic::helpers::HostFxObjectPose>; 2],
}
impl PhysicsDamageSources {
    fn observe(&self, id: Option<ObjectId>) -> LastDamageSourceObservation {
        id.and_then(|id| {
            self.observations
                .iter()
                .flatten()
                .find(|entry| entry.0 == id)
        })
        .map(|entry| entry.1)
        .unwrap_or_default()
    }
}

pub(in crate::game_logic) struct PhysicsDamageServices<'a> {
    pub frame: u32,
    pub sources: &'a PhysicsDamageSources,
    pub health_events: &'a mut crate::game_logic::HostHealthEvents,
    pub repulsors: &'a bool,
    pub player_effects: &'a mut dyn FnMut(BodyPlayerEffect),
    pub body_fx: &'a mut BodyStateFxReceiver<'a>,
}
impl PhysicsDamageServices<'_> {
    pub(super) fn apply(
        &mut self,
        victim: &mut Object,
        amount: f32,
        source: Option<ObjectId>,
        damage_type: crate::game_logic::combat::DamageType,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
    ) -> bool {
        let context = DamageHitContext::default().with_last_damage_observations(
            self.sources.observe(source),
            self.sources.observe(victim.last_damage_source),
        );
        let supplied = source.and_then(|id| {
            self.sources
                .effects
                .iter()
                .flatten()
                .find(|value| value.id == id.0)
        });
        let receiving = &mut *self.body_fx;
        let mut body_fx = |object: &mut Object,
                           effect: BodyStateFx,
                           _: Option<
            &crate::game_logic::host_transition_damage_fx::HostDamageFxVictim,
        >| {
            // PhysicsUpdate supplies the actual request's source: self for
            // falling, crusher for overlap, NULL for resting/stunned kill.
            // Retained lastDamageSource serves different Player bookkeeping.
            if source == Some(object.id) {
                let current =
                    crate::game_logic::host_transition_damage_fx::snapshot_damage_fx_source(object);
                receiving(object, effect, Some(&current));
            } else {
                receiving(object, effect, supplied);
            }
        };
        let application = victim.begin_damage_with_context_and_effects(
            amount,
            source,
            damage_type,
            death_type,
            None,
            self.frame,
            &context,
            self.health_events,
            None, // Physics supplies only Crush/Falling/Unresistable; no ownership transfer.
            &mut *self.player_effects,
            &mut body_fx,
        );
        // The receiving callback now also executes ArmorDamage. The adapter
        // supplies this request's current/self source, never retained history.
        application.finish_standalone_with_body_fx(victim, None, self.repulsors, &mut body_fx)
    }
}

impl crate::game_logic::GameLogic {
    /// Borrow the presentation adjunct for one immediate physics operation.
    /// Objects remain installed; no registry selection or end-frame queue is
    /// needed to execute the original synchronous Body callback sequence.
    pub(in crate::game_logic) fn with_physics_body_fx_parts<R>(
        session_view: &mut crate::presentation_frame::SessionView,
        player: Option<u32>,
        shroud: &std::sync::Arc<std::sync::Mutex<gamelogic::system::shroud_manager::ShroudManager>>,
        particles: &mut crate::game_logic::combat_particles::CombatParticleRegistry,
        frame: u32,
        sources: &PhysicsDamageSources,
        run: impl FnOnce(&mut BodyStateFxReceiver<'_>) -> R,
    ) -> R {
        #[cfg(feature = "game_client")]
        {
            Self::with_host_fx_view_parts(session_view, player, shroud, |view| {
                run(&mut |object, effect, source| match effect {
                    BodyStateFx::Transition(event) => Self::execute_transition_damage_fx_with_view(
                        object, event, source, particles, frame, view,
                    ),
                    BodyStateFx::ArmorDamage {
                        damage_type,
                        actual_damage,
                        frame,
                    } => {
                        let secondary = source.and_then(|source| {
                            sources
                                .poses
                                .iter()
                                .flatten()
                                .find(|pose| pose.id == source.id)
                        });
                        Self::execute_armor_damage_fx_with_view(
                            object,
                            damage_type,
                            actual_damage,
                            frame,
                            source,
                            secondary,
                            player,
                            shroud,
                            view,
                        );
                    }
                    BodyStateFx::FxListDie => {
                        object.fire_fx_list_die_with_fx(&mut |name, object, at_object| {
                            if at_object {
                                let primary = Self::observe_host_fx_object(object, player, shroud);
                                let secondary = source.and_then(|value| {
                                    if value.id == primary.id {
                                        Some(&primary)
                                    } else {
                                        sources
                                            .poses
                                            .iter()
                                            .flatten()
                                            .find(|pose| pose.id == value.id)
                                    }
                                });
                                Self::dispatch_fx_list_at_object_with_view(
                                    name, &primary, secondary, view,
                                )
                            } else {
                                Self::dispatch_fx_list_at_position_with_view(
                                    name,
                                    object.get_position(),
                                    None,
                                    0.0,
                                    0.0,
                                    None,
                                    view,
                                )
                            }
                        });
                    }
                    BodyStateFx::FxListDieSlot(mut config) => {
                        object.fire_fx_list_die_config_with_fx(
                            &mut config,
                            &mut |name, object, at_object| {
                                if at_object {
                                    let primary =
                                        Self::observe_host_fx_object(object, player, shroud);
                                    let secondary = source.and_then(|value| {
                                        if value.id == primary.id {
                                            Some(&primary)
                                        } else {
                                            sources
                                                .poses
                                                .iter()
                                                .flatten()
                                                .find(|pose| pose.id == value.id)
                                        }
                                    });
                                    Self::dispatch_fx_list_at_object_with_view(
                                        name, &primary, secondary, view,
                                    )
                                } else {
                                    Self::dispatch_fx_list_at_position_with_view(
                                        name,
                                        object.get_position(),
                                        None,
                                        0.0,
                                        0.0,
                                        None,
                                        view,
                                    )
                                }
                            },
                        );
                    }
                    BodyStateFx::ObjectPhase { name, at_object } => {
                        Self::dispatch_object_phase_fx_with_view(
                            &name, object, at_object, player, shroud, view,
                        )
                    }
                })
            })
        }
        #[cfg(not(feature = "game_client"))]
        {
            let _ = (session_view, player, shroud, particles, frame, sources);
            run(&mut Object::standalone_body_fx)
        }
    }
}
