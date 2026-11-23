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
        frame: u32,
    ) -> Option<(PlayerIndex, bool)> {
        if power == 0 || (power > 0 && disabled) {
            return None;
        }
        self.energy.adjust_power(power, true);
        Some((self.player_index, !self.energy.has_sufficient_power_at(frame)))
    }

    pub(crate) fn score_structure_construction(
        &mut self,
        structure: &Object,
        is_rebuild: Bool,
        scoring_enabled: Option<bool>,
    ) {
        if is_rebuild {
            return;
        }
        // Player.cpp:1648–1651 records the build before applying power/AI effects.
        if let Some(enabled) = scoring_enabled {
            self.score_keeper
                .add_object_built_obj_with_policy(structure, enabled);
        } else {
            self.score_keeper.add_object_built_obj(structure);
        }
        let template = structure.get_template();
        let cost = template.calc_cost_to_build(Some(self)).max(0) as u32;
        self.score_keeper.add_money_spent(cost);
        self.academy_stats
            .record_building_built(template.get_name().as_str());
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
            let structure_guard = structure
                .read()
                .map_err(|error| format!("construction object: {error}"))?;
            if let Some(owner) = structure_guard.owning_pathfinding_runtime() {
                owner.refresh_completed_structure_footprint(&structure_guard)?;
            }
        }

        if !is_rebuild {
            if let (Ok(mut player), Ok(structure)) = (player.write(), structure.read()) {
                player.score_structure_construction(&structure, false, scoring_enabled);
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
                let frame = structure_world.as_ref().map_or_else(
                    crate::helpers::TheGameLogic::get_frame,
                    |world| world.logic_frame(),
                );
                let brownout = owner.write().ok().and_then(|mut owner| {
                    let effect = owner.apply_construction_power_influence(power, disabled, frame)?;
                    owner.set_brownout_radar(effect.1);
                    Some(effect)
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

        let local_player = structure_world
            .as_ref()
            .and_then(|world| world.players().read().ok()?.get_local_player().cloned());
        if let Some(local_player) = local_player {
            let events = {
                let structure = structure
                    .read()
                    .map_err(|error| format!("construction object: {error}"))?;
                let local = local_player
                    .read()
                    .map_err(|error| format!("construction local player: {error}"))?;
                Self::completed_structure_eva(&structure, &local)
            };
            for event in events {
                let _ =
                    crate::helpers::TheEva::set_should_play_from(structure_world.as_ref(), event);
            }
        }
        Ok(())
    }

    pub(crate) fn completed_structure_eva(
        structure: &Object,
        local: &Player,
    ) -> impl Iterator<Item = crate::helpers::EvaEvent> + use<> {
        use crate::helpers::EvaEvent::*;
        use crate::object::special_power_types::SpecialPowerType as Power;
        // Player.cpp:1664–1720 treats neutrals as allies for this notification.
        let own = Some(local.get_player_index() as u32) == structure.get_controlling_player_id();
        let enemy = !own
            && structure.get_team().is_some_and(|team| {
                team.read().ok().is_some_and(|team| {
                    local.get_relationship_with_team(&team) == Relationship::Enemies
                })
            });
        let select = |owned, allied, hostile| {
            if own {
                owned
            } else if enemy {
                hostile
            } else {
                allied
            }
        };
        let particle = [
            Power::ParticleUplinkCannon,
            Power::SupwParticleUplinkCannon,
            Power::LazrParticleUplinkCannon,
        ]
        .into_iter()
        .any(|power| structure.has_special_power(power));
        let nuke = [
            Power::NeutronMissile,
            Power::NukeNeutronMissile,
            Power::SupwNeutronMissile,
        ]
        .into_iter()
        .any(|power| structure.has_special_power(power));
        let scud = structure.has_special_power(Power::ScudStorm);
        [
            particle.then(|| {
                select(
                    SuperweaponDetectedOwnParticleCannon,
                    SuperweaponDetectedAllyParticleCannon,
                    SuperweaponDetectedEnemyParticleCannon,
                )
            }),
            nuke.then(|| {
                select(
                    SuperweaponDetectedOwnNuke,
                    SuperweaponDetectedAllyNuke,
                    SuperweaponDetectedEnemyNuke,
                )
            }),
            scud.then(|| {
                select(
                    SuperweaponDetectedOwnScudStorm,
                    SuperweaponDetectedAllyScudStorm,
                    SuperweaponDetectedEnemyScudStorm,
                )
            }),
        ]
        .into_iter()
        .flatten()
    }
}
