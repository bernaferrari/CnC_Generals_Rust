use super::Object;
use crate::game_logic::host_transition_damage_fx::HostTransitionDamageFxEvent;

/// Effects emitted at the original Body callback observation point. This is
/// synchronous operation data, never a stored world queue or saved mirror.
pub(in crate::game_logic) enum BodyStateFx {
    Transition(HostTransitionDamageFxEvent),
    VoiceFear(crate::game_logic::AudioEventRequest),
    FxListDie,
    FxListDieSlot(crate::game_logic::host_fx_list_die::HostFxListDieData),
    ArmorDamage {
        damage_type: crate::game_logic::combat::DamageType,
        actual_damage: f32,
        frame: u32,
    },
    ObjectPhase {
        name: String,
        at_object: bool,
    },
}

pub(in crate::game_logic) type BodyStateFxReceiver<'a> = dyn FnMut(
        &mut Object,
        BodyStateFx,
        Option<&crate::game_logic::host_transition_damage_fx::HostDamageFxVictim>,
    ) + 'a;

impl Object {
    /// The original fear gate executes after body callbacks. Only a qualifying
    /// live crossing consumes a chance draw, including a silent definition.
    pub(in crate::game_logic) fn voice_fear_request(
        &self,
        draw: impl FnOnce() -> i32,
    ) -> Option<crate::game_logic::AudioEventRequest> {
        use crate::game_logic::host_transition_damage_fx::voice_fear_should_play;
        if !voice_fear_should_play(
            self.previous_health,
            self.health.current,
            self.health.maximum,
            0,
        ) {
            return None;
        }
        if draw() >= 25 {
            return None;
        }
        let name = self
            .thing()
            .template
            .voice_fear
            .as_deref()
            .filter(|name| !name.is_empty())?;
        // ActiveBody sets position and controlling player, not ObjectID.
        let mut request = crate::game_logic::AudioEventRequest::new(name)
            .with_position(self.get_position())
            .with_priority(150);
        request.player_index = self.owner_player_id.map(|id| id as i32);
        Some(request)
    }

    pub(super) fn standalone_body_fx(
        object: &mut Object,
        effect: BodyStateFx,
        source: Option<&crate::game_logic::host_transition_damage_fx::HostDamageFxVictim>,
    ) {
        match effect {
            // Detached Object fixtures have no AudioManager owner. A caller
            // observing presentation supplies its BodyStateFxReceiver; admitted
            // objects use the receiving session operations below.
            BodyStateFx::VoiceFear(_) => {}
            BodyStateFx::Transition(event) => {
                object.queue_transition_damage_fx_event_with_source(event, source)
            }
            BodyStateFx::FxListDie => object.fire_fx_list_die(),
            BodyStateFx::FxListDieSlot(mut config) => object.fire_fx_list_die_config_with_fx(
                &mut config,
                &mut |name, object, at_object| {
                    if at_object {
                        crate::game_logic::dispatch_fx_list_at_object(
                            name,
                            object.id.0,
                            source.map(|source| source.id),
                        )
                    } else {
                        crate::game_logic::dispatch_fx_list_at_pos(name, object.get_position())
                    }
                },
            ),
            BodyStateFx::ArmorDamage {
                damage_type,
                actual_damage,
                frame,
            } => {
                crate::game_logic::host_transition_damage_fx::dispatch_armor_damage_fx(
                    object,
                    damage_type,
                    actual_damage,
                    frame,
                    source,
                );
            }
            BodyStateFx::ObjectPhase { name, at_object } => {
                Self::standalone_object_fx(&name, object, at_object)
            }
        }
    }
}

// Immediate Object update effects borrow the driving session's view.
#[cfg(feature = "game_client")]
impl crate::game_logic::GameLogic {
    pub(crate) fn dispatch_object_phase_fx_with_view(
        name: &str,
        object: &crate::game_logic::Object,
        at_object: bool,
        player: Option<u32>,
        shroud: &std::sync::Arc<std::sync::Mutex<gamelogic::system::shroud_manager::ShroudManager>>,
        view: &mut game_client::fx_list::FxViewServices<'_>,
    ) {
        if at_object {
            let pose = Self::observe_host_fx_object(object, player, shroud);
            Self::dispatch_fx_list_at_object_with_view(name, &pose, None, view);
        } else {
            Self::dispatch_fx_list_at_position_with_view(
                name,
                object.get_position(),
                None,
                0.0,
                0.0,
                None,
                view,
            );
        }
    }
}

impl crate::game_logic::GameLogic {
    /// Complete the actual object's authored modules at this onDie point.
    pub(crate) fn fire_owned_fx_list_die(&mut self, id: crate::game_logic::ObjectId) {
        self.fire_owned_fx_list_die_with_effects(
            id,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn fire_owned_fx_list_die_with_effects(
        &mut self,
        id: crate::game_logic::ObjectId,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let source = self
            .objects
            .get(&id)
            .and_then(|object| object.last_damage_source);
        self.fire_owned_fx_list_die_from_source_with_effects(id, source, presentation_effects);
    }

    /// Actual DamageInfo source, including explicit NULL, is distinct from
    /// prior Body history used by the standalone/direct-admission adapter.
    pub(in crate::game_logic) fn fire_owned_fx_list_die_from_source(
        &mut self,
        id: crate::game_logic::ObjectId,
        source: Option<crate::game_logic::ObjectId>,
    ) {
        self.fire_owned_fx_list_die_from_source_with_effects(
            id,
            source,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(in crate::game_logic) fn fire_owned_fx_list_die_from_source_with_effects(
        &mut self,
        id: crate::game_logic::ObjectId,
        source: Option<crate::game_logic::ObjectId>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        #[cfg(feature = "game_client")]
        {
            let player = self.local_player_id();
            let secondary = source
                .and_then(|source| self.objects.get(&source))
                .map(|object| {
                    Self::observe_host_fx_object(object, player, self.world_services.shroud())
                });
            Self::with_host_fx_view_parts_and_terrain(
                &mut self.session_view,
                player,
                self.world_services.shroud(),
                presentation_effects.terrain.as_mut().map(|terrain| {
                    &mut **terrain as &mut dyn crate::game_logic::NativeTerrainEffects
                }),
                |view| {
                    if let Some(object) = self.objects.get_mut(&id) {
                        object.fire_fx_list_die_with_fx(&mut |name, object, at_object| {
                            Self::dispatch_object_die_fx_with_view(
                                name,
                                object,
                                at_object,
                                secondary.as_ref(),
                                player,
                                self.world_services.shroud(),
                                view,
                            )
                        });
                    }
                },
            );
        }
        #[cfg(not(feature = "game_client"))]
        if let Some(object) = self.objects.get_mut(&id) {
            object.fire_fx_list_die_with_fx(&mut |name, object, at_object| {
                if at_object {
                    crate::game_logic::dispatch_fx_list_at_object(
                        name,
                        object.id.0,
                        source.map(|source| source.0),
                    )
                } else {
                    crate::game_logic::dispatch_fx_list_at_pos(name, object.get_position())
                }
            });
        }
    }

    pub(in crate::game_logic) fn execute_body_state_fx_without_view(
        object: &mut Object,
        effect: BodyStateFx,
        source: Option<&crate::game_logic::host_transition_damage_fx::HostDamageFxVictim>,
        audio: &mut Vec<crate::game_logic::AudioEventRequest>,
    ) {
        match effect {
            BodyStateFx::VoiceFear(request) => audio.push(request),
            effect => Object::standalone_body_fx(object, effect, source),
        }
    }

    #[cfg(feature = "game_client")]
    pub(crate) fn execute_body_state_fx_with_view(
        object: &mut Object,
        effect: BodyStateFx,
        source: Option<&crate::game_logic::host_transition_damage_fx::HostDamageFxVictim>,
        secondary: Option<&gamelogic::helpers::HostFxObjectPose>,
        player: Option<u32>,
        shroud: &std::sync::Arc<std::sync::Mutex<gamelogic::system::shroud_manager::ShroudManager>>,
        particles: &mut crate::game_logic::combat_particles::CombatParticleRegistry,
        audio: &mut Vec<crate::game_logic::AudioEventRequest>,
        frame: u32,
        view: &mut game_client::fx_list::FxViewServices<'_>,
    ) {
        match effect {
            BodyStateFx::VoiceFear(request) => audio.push(request),
            BodyStateFx::Transition(event) => Self::execute_transition_damage_fx_with_view(
                object, event, source, particles, frame, view,
            ),
            BodyStateFx::ArmorDamage {
                damage_type,
                actual_damage,
                frame,
            } => {
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
                    Self::dispatch_object_die_fx_with_view(
                        name, object, at_object, secondary, player, shroud, view,
                    )
                })
            }
            BodyStateFx::FxListDieSlot(mut config) => object.fire_fx_list_die_config_with_fx(
                &mut config,
                &mut |name, object, at_object| {
                    Self::dispatch_object_die_fx_with_view(
                        name, object, at_object, secondary, player, shroud, view,
                    )
                },
            ),
            BodyStateFx::ObjectPhase { name, at_object } => {
                Self::dispatch_object_phase_fx_with_view(
                    &name, object, at_object, player, shroud, view,
                )
            }
        }
    }

    #[cfg(feature = "game_client")]
    pub(crate) fn execute_armor_damage_fx_with_view(
        object: &mut Object,
        damage_type: crate::game_logic::combat::DamageType,
        actual_damage: f32,
        frame: u32,
        source: Option<&crate::game_logic::host_transition_damage_fx::HostDamageFxVictim>,
        secondary: Option<&gamelogic::helpers::HostFxObjectPose>,
        player: Option<u32>,
        shroud: &std::sync::Arc<std::sync::Mutex<gamelogic::system::shroud_manager::ShroudManager>>,
        view: &mut game_client::fx_list::FxViewServices<'_>,
    ) {
        crate::game_logic::host_transition_damage_fx::dispatch_armor_damage_fx_with_dispatch(
            object,
            damage_type,
            actual_damage,
            frame,
            source,
            &mut |object, name, source| {
                let primary = Self::observe_host_fx_object(object, player, shroud);
                let secondary = source.and_then(|source| {
                    if source.id == primary.id {
                        Some(&primary)
                    } else {
                        secondary.filter(|pose| pose.id == source.id)
                    }
                });
                Self::dispatch_fx_list_at_object_with_view(name, &primary, secondary, view);
            },
        );
    }

    #[cfg(feature = "game_client")]
    pub(crate) fn observe_fx_list_die_secondary(
        &self,
        id: crate::game_logic::ObjectId,
    ) -> Option<gamelogic::helpers::HostFxObjectPose> {
        let killer = self.objects.get(&id)?.last_damage_source?;
        self.objects.get(&killer).map(|object| {
            Self::observe_host_fx_object(
                object,
                self.local_player_id(),
                self.world_services.shroud(),
            )
        })
    }

    #[cfg(feature = "game_client")]
    pub(crate) fn dispatch_object_die_fx_with_view(
        name: &str,
        object: &Object,
        at_object: bool,
        secondary: Option<&gamelogic::helpers::HostFxObjectPose>,
        player: Option<u32>,
        shroud: &std::sync::Arc<std::sync::Mutex<gamelogic::system::shroud_manager::ShroudManager>>,
        view: &mut game_client::fx_list::FxViewServices<'_>,
    ) -> bool {
        if at_object {
            let primary = Self::observe_host_fx_object(object, player, shroud);
            // Self-damage observes this same current transform, including any
            // pose updates made during earlier Body callbacks.
            // The caller supplied the current operation's source. NULL must
            // remain NULL even when Body history retains a prior self-hit.
            let secondary = secondary.map(|pose| {
                if pose.id == object.id.0 {
                    &primary
                } else {
                    pose
                }
            });
            Self::dispatch_fx_list_at_object_with_view(name, &primary, secondary, view)
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
    }
}

#[cfg(all(test, feature = "game_client"))]
#[path = "fx_owner_tests.rs"]
mod owner_tests;
