//! C++ Player.cpp1639-1720 construction notification phases.
//! Shared handles are a temporary adapter for the registry-backed module path;
//! they do not grant ownership of mutable match state to the callback.
use super::*;

impl Player {
    /// Synchronously complete the player's notification phases.
    /// Call with handles after releasing player and object guards.
    /// Matches C++ Player::onStructureConstructionComplete.
    pub fn on_structure_construction_complete(
        player: &Arc<RwLock<Player>>,
        builder_id: Option<ObjectID>,
        structure: &Arc<RwLock<Object>>,
        is_rebuild: Bool,
    ) {
        // No caller-held player/object guard may span these reentrant phases.
        crate::helpers::TheScriptEngine::notify_of_object_creation_or_destruction();

        let structure_id = match structure.read() {
            Ok(guard) => guard.get_id(),
            Err(_) => return,
        };

        let (structure_pos, structure_layer) = {
            let Ok(guard) = structure.read() else {
                return;
            };
            (*guard.get_position(), guard.get_layer())
        };

        let ai_store = crate::ai::the_ai();
        if let Ok(ai_guard) = ai_store.read() {
            if let Some(pathfinding) = ai_guard.pathfinding_system() {
                if let Ok(mut system) = pathfinding.write() {
                    let layer =
                        crate::ai::pathfinding_system::PathfindLayerEnum::from(structure_layer);
                    let positions = [structure_pos];
                    system.remove_obstacle(structure_id, &positions, layer);
                    system.add_obstacle(structure_id, &positions, layer);
                }
            }
        }

        if !is_rebuild {
            if let (Ok(mut player_guard), Ok(structure_guard)) = (player.write(), structure.read())
            {
                // C++ onStructureConstructionComplete → addObjectBuilt + addMoneySpent.
                player_guard
                    .score_keeper
                    .add_object_built_obj(&*structure_guard);
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
                let brownout = owner.write().ok().map(|mut owner| {
                    if power > 0 {
                        owner.energy.add_power_production(power);
                    } else {
                        owner.energy.add_power_consumption(-power);
                    }
                    let brownout = !owner.energy.has_sufficient_power();
                    if brownout {
                        owner.disable_radar();
                    } else {
                        owner.enable_radar();
                    }
                    (owner.player_index, brownout)
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

        crate::control_bar::mark_ui_dirty();

        let local_player = crate::player::player_list()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned());
        // C++ queries current ownership and powers after the AI callback.
        use crate::object::special_power_types::SpecialPowerType as Power;
        let (
            team,
            controlling_player_id,
            is_superweapon_particle,
            is_superweapon_nuke,
            is_superweapon_scud,
        ) = {
            let Ok(guard) = structure.read() else {
                return;
            };
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
                let Ok(local) = local_player.read() else {
                    return;
                };
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
                    let _ = crate::helpers::TheEva::set_should_play(
                        crate::helpers::EvaEvent::SuperweaponDetectedOwnParticleCannon,
                    );
                } else if relation != Relationship::Enemies {
                    let _ = crate::helpers::TheEva::set_should_play(
                        crate::helpers::EvaEvent::SuperweaponDetectedAllyParticleCannon,
                    );
                } else {
                    let _ = crate::helpers::TheEva::set_should_play(
                        crate::helpers::EvaEvent::SuperweaponDetectedEnemyParticleCannon,
                    );
                }
            }

            if is_superweapon_nuke {
                if own {
                    let _ = crate::helpers::TheEva::set_should_play(
                        crate::helpers::EvaEvent::SuperweaponDetectedOwnNuke,
                    );
                } else if relation != Relationship::Enemies {
                    let _ = crate::helpers::TheEva::set_should_play(
                        crate::helpers::EvaEvent::SuperweaponDetectedAllyNuke,
                    );
                } else {
                    let _ = crate::helpers::TheEva::set_should_play(
                        crate::helpers::EvaEvent::SuperweaponDetectedEnemyNuke,
                    );
                }
            }

            if is_superweapon_scud {
                if own {
                    let _ = crate::helpers::TheEva::set_should_play(
                        crate::helpers::EvaEvent::SuperweaponDetectedOwnScudStorm,
                    );
                } else if relation != Relationship::Enemies {
                    let _ = crate::helpers::TheEva::set_should_play(
                        crate::helpers::EvaEvent::SuperweaponDetectedAllyScudStorm,
                    );
                } else {
                    let _ = crate::helpers::TheEva::set_should_play(
                        crate::helpers::EvaEvent::SuperweaponDetectedEnemyScudStorm,
                    );
                }
            }
        }
    }
}
