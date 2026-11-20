//! C++ Player.cpp1639-1720 construction notification phases.
//! The admitted structure's Core runtime owns its pathfinder callback.
use super::*;

impl Player {
    /// The synchronous energy phase of CPP Player.cpp:1681. The driving
    /// completion releases this player loan before applying brownout to objects.
    /// Rebuilds execute the same phase; ownership membership excludes unfinished
    /// structures and does not synthesize this completion notification.
    pub(crate) fn apply_construction_power_influence(
        &mut self,
        power: Int,
        disabled: bool,
    ) -> Option<(PlayerIndex, bool)> {
        if power == 0 || (power > 0 && disabled) {
            return None;
        }
        if power > 0 {
            self.energy.add_power_production(power);
        } else {
            self.energy.add_power_consumption(-power);
        }
        let brownout = !self.energy.has_sufficient_power();
        if brownout {
            self.disable_radar();
        } else {
            self.enable_radar();
        }
        Some((self.player_index, brownout))
    }

    /// Synchronously complete the player's notification phases.
    /// Call with handles after releasing player and object guards.
    /// Matches C++ Player::onStructureConstructionComplete.
    pub fn on_structure_construction_complete(
        player: &Arc<RwLock<Player>>,
        builder_id: Option<ObjectID>,
        structure: &Arc<RwLock<Object>>,
        is_rebuild: Bool,
    ) -> Result<(), String> {
        Self::on_structure_construction_complete_with_scoring(
            player, builder_id, structure, is_rebuild, None,
        )
    }

    /// The scheduled caller already borrows the GameLogic whose current
    /// scoring bit controls ScoreKeeper.cpp:101–103.
    pub(crate) fn on_structure_construction_complete_with_scoring(
        player: &Arc<RwLock<Player>>,
        builder_id: Option<ObjectID>,
        structure: &Arc<RwLock<Object>>,
        is_rebuild: Bool,
        scoring_enabled: Option<bool>,
    ) -> Result<(), String> {
        // No caller-held player/object guard may span these reentrant phases.
        let (structure_id, structure_world) = {
            let structure = structure
                .read()
                .map_err(|error| format!("construction object: {error}"))?;
            (structure.get_id(), structure.owning_world_services())
        };
        // Player.cpp:1640 invalidates the receiving interpreter before the
        // footprint and AI callbacks. ScriptEngine.cpp:7785 only stamps its
        // frame; the next script update owns the named-object cache rebuild.
        if let Some(world) = structure_world.as_ref() {
            let frame = world.logic_frame();
            if let Ok(slot) = world.scripts().engine().read()
                && let Some(engine) = slot.as_ref()
            {
                engine.set_frame_object_count_changed(frame);
            }
        } else {
            crate::helpers::TheScriptEngine::notify_of_object_creation_or_destruction();
        }

        // C++ Player.cpp:1644-1645: remove then insert the full footprint on
        // the runtime that admitted this structure, before reentrant phases.
        {
            let structure_guard = structure.read().map_err(|error| format!("construction object: {error}"))?;
            if let Some(owner) = structure_guard.owning_pathfinding_runtime() {
                owner.refresh_completed_structure_footprint(&structure_guard)?;
            }
        }

        if !is_rebuild {
            if let (Ok(mut player_guard), Ok(structure_guard)) = (player.write(), structure.read())
            {
                // C++ onStructureConstructionComplete → addObjectBuilt + addMoneySpent.
                if let Some(enabled) = scoring_enabled {
                    player_guard.score_keeper
                        .add_object_built_obj_with_policy(&*structure_guard, enabled);
                } else {
                    player_guard.score_keeper.add_object_built_obj(&*structure_guard);
                }
                let cost = structure_guard
                    .get_template()
                    .calc_cost_to_build(Some(&*player_guard))
                    .max(0) as u32;
                player_guard.score_keeper.add_money_spent(cost);
                player_guard
                    .academy_stats
                    .record_building_built(structure_guard.get_template().get_name().as_str());
            }
        }

        // Snapshot just the power influence. Brownout handling can write other
        // objects, so neither the structure nor its player is borrowed here.
        let influence = structure.read().ok().map(|guard| {
            (
                guard.get_controlling_player(),
                guard.get_template().get_energy_production(),
                guard.is_disabled(),
            )
        });
        if let Some((Some(owner), power, disabled)) = influence {
            if power < 0 || (power > 0 && !disabled) {
                let brownout = owner.write().ok().and_then(|mut owner| {
                    owner.apply_construction_power_influence(power, disabled)
                });
                if let Some((owner_index, brownout)) = brownout {
                    Self::apply_power_brownout_to_owned_objects(owner_index, brownout);
                }
            }
        }

        let player_id = player.read().ok().map(|guard| guard.player_index as u32);
        if let Some(player_id) = player_id {
            // C++ notifies AI even when the builder is null (rebuild/startup).
            let _ = crate::ai::integration::with_ai_integration_mut(|manager| {
                manager.with_ai_player_mut(player_id, |ai_player| {
                    let _ = ai_player
                        .on_structure_produced(builder_id.unwrap_or(INVALID_ID), structure_id);
                })
            });
        }

        // C++ TheControlBar/TheEva of the GameEngine that owns the structure.
        crate::control_bar::mark_ui_dirty_from(structure_world.as_ref());

        let local_player = structure_world.as_ref().and_then(|world| {
            world.players().read().ok()?.get_local_player().cloned()
        });
        // C++ queries current ownership and powers after the AI callback.
        use crate::object::special_power_types::SpecialPowerType as Power;
        let (
            team,
            controlling_player_id,
            is_superweapon_particle,
            is_superweapon_nuke,
            is_superweapon_scud,
        ) = {
            let guard = structure.read().map_err(|error| format!("construction object: {error}"))?;
            (
                guard.get_team(),
                guard.get_controlling_player_id(),
                [
                    Power::ParticleUplinkCannon,
                    Power::SupwParticleUplinkCannon,
                    Power::LazrParticleUplinkCannon,
                ]
                .into_iter()
                .any(|power| guard.has_special_power(power)),
                [
                    Power::NeutronMissile,
                    Power::NukeNeutronMissile,
                    Power::SupwNeutronMissile,
                ]
                .into_iter()
                .any(|power| guard.has_special_power(power)),
                guard.has_special_power(Power::ScudStorm),
            )
        };
        if let Some(local_player) = local_player {
            let (own, relation) = {
                let local = local_player.read().map_err(|error| format!("construction local player: {error}"))?;
                (
                    Some(local.get_player_index() as u32) == controlling_player_id,
                    team.as_ref()
                        .and_then(|team| {
                            team.read()
                                .ok()
                                .map(|team| local.get_relationship_with_team(&team))
                        })
                        .unwrap_or(Relationship::Neutral),
                )
            };

            if is_superweapon_particle {
                if own {
                    let _ = crate::helpers::TheEva::set_should_play_from(
                        structure_world.as_ref(),
                        crate::helpers::EvaEvent::SuperweaponDetectedOwnParticleCannon,
                    );
                } else if relation != Relationship::Enemies {
                    let _ = crate::helpers::TheEva::set_should_play_from(
                        structure_world.as_ref(),
                        crate::helpers::EvaEvent::SuperweaponDetectedAllyParticleCannon,
                    );
                } else {
                    let _ = crate::helpers::TheEva::set_should_play_from(
                        structure_world.as_ref(),
                        crate::helpers::EvaEvent::SuperweaponDetectedEnemyParticleCannon,
                    );
                }
            }

            if is_superweapon_nuke {
                if own {
                    let _ = crate::helpers::TheEva::set_should_play_from(
                        structure_world.as_ref(),
                        crate::helpers::EvaEvent::SuperweaponDetectedOwnNuke,
                    );
                } else if relation != Relationship::Enemies {
                    let _ = crate::helpers::TheEva::set_should_play_from(
                        structure_world.as_ref(),
                        crate::helpers::EvaEvent::SuperweaponDetectedAllyNuke,
                    );
                } else {
                    let _ = crate::helpers::TheEva::set_should_play_from(
                        structure_world.as_ref(),
                        crate::helpers::EvaEvent::SuperweaponDetectedEnemyNuke,
                    );
                }
            }

            if is_superweapon_scud {
                if own {
                    let _ = crate::helpers::TheEva::set_should_play_from(
                        structure_world.as_ref(),
                        crate::helpers::EvaEvent::SuperweaponDetectedOwnScudStorm,
                    );
                } else if relation != Relationship::Enemies {
                    let _ = crate::helpers::TheEva::set_should_play_from(
                        structure_world.as_ref(),
                        crate::helpers::EvaEvent::SuperweaponDetectedAllyScudStorm,
                    );
                } else {
                    let _ = crate::helpers::TheEva::set_should_play_from(
                        structure_world.as_ref(),
                        crate::helpers::EvaEvent::SuperweaponDetectedEnemyScudStorm,
                    );
                }
            }
        }
        Ok(())
    }
}
