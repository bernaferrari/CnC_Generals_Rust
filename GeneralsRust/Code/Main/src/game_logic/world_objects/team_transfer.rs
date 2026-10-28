//! Receiving Object::setTeam and Object::defect. Team identity is session-local;
//! release the roster borrow before running capture and containment callbacks.
use super::super::*;

/// Owned continuation data: no Team guard survives into callbacks.
pub(super) struct HostNeutralTeam {
    pub(super) team: Team,
    pub(super) owner: u32,
    pub(super) name: String,
    pub(super) identity: u32,
}

impl GameLogic {
    /// Validate the receiving-world team identities used by GarrisonContain's
    /// owner recalculation before OpenContain commits the occupant relationship.
    /// The actual transfer runs later, after GarrisonContain::onContaining has
    /// completed its station/pose work, preserving the C++ callback order.
    pub(in crate::game_logic) fn validate_garrison_owner_team_transfer(
        &self,
        container_id: ObjectId,
        arriving_id: ObjectId,
    ) -> gamelogic::GameLogicResult<()> {
        let Some(container) = self.objects.get(&container_id) else {
            return Ok(());
        };
        if !container.is_garrison_contain() {
            return Ok(());
        }
        let controlling_unit = container
            .contained_units()
            .first()
            .copied()
            .unwrap_or(arriving_id);
        let Some(owner) = self
            .objects
            .get(&controlling_unit)
            .and_then(|object| object.owner_player_id)
        else {
            // C++ leaves the source Team unchanged when the first rider has no
            // controlling Player or default Team.
            return Ok(());
        };
        let invalid = |reason: &str| gamelogic::GameLogicError::Configuration(reason.into());
        // GarrisonContain::recalcApparentControllingPlayer only changes the
        // source Team when the controlling Player exposes a default Team.
        // A missing default is an authored/runtime no-switch case; a present
        // but unresolvable identity remains an invalid receiving-world state.
        if self
            .players
            .get(&owner)
            .ok_or_else(|| invalid("Garrison rider controller is missing"))?
            .default_team_id
            .is_none()
        {
            return Ok(());
        }
        let default_team = self
            .host_default_team_id(owner)
            .ok_or_else(|| invalid("Garrison rider has no receiving default Team"))?;
        let destination = self
            .inactive_controller_neutral_team(owner)?
            .map(|neutral| neutral.identity)
            .unwrap_or(default_team);
        let factory = self
            .team_factory
            .lock()
            .map_err(|error| gamelogic::GameLogicError::Threading(error.to_string()))?;
        let team = factory
            .find_team_by_id(destination)
            .ok_or_else(|| invalid("Garrison destination Team is absent from receiving factory"))?;
        let selected = team
            .read()
            .map_err(|error| gamelogic::GameLogicError::Threading(error.to_string()))?;
        let name = selected.get_name().as_str();
        if selected.get_controlling_player_id().is_none()
            || factory.find_team_prototype(name).is_none()
            || !self
                .players
                .contains_key(&selected.get_controlling_player_id().unwrap())
        {
            return Err(invalid(
                "Garrison destination Team lacks receiving controller or prototype",
            ));
        }
        Ok(())
    }

    pub(super) fn inactive_controller_neutral_team(
        &self,
        owner: u32,
    ) -> gamelogic::GameLogicResult<Option<HostNeutralTeam>> {
        let invalid = |reason: &str| gamelogic::GameLogicError::Configuration(reason.into());
        let player = self
            .players
            .get(&owner)
            .ok_or_else(|| invalid("requested Team controller is missing"))?;
        // Player.cpp:3923, shared by semantic birth and subsequent Team changes.
        if player.is_alive && !player.is_observer {
            return Ok(None);
        }
        let mut neutrals = self
            .players
            .values()
            .filter(|player| player.is_reserved_neutral());
        let neutral = neutrals.next().ok_or_else(|| {
            invalid("inactive controller requires the receiving reserved neutral Player")
        })?;
        if neutrals.next().is_some() {
            return Err(invalid(
                "inactive controller has ambiguous reserved neutral Players",
            ));
        }
        let name = self.default_host_team_instance_name(Some(neutral.id), neutral.team);
        let factory = self
            .team_factory
            .lock()
            .map_err(|error| gamelogic::GameLogicError::Threading(error.to_string()))?;
        let instance = factory
            .find_team_instances(&name)
            .into_iter()
            .next()
            .ok_or_else(|| {
                invalid("inactive controller requires an admitted neutral default Team")
            })?;
        let instance = instance
            .read()
            .map_err(|error| gamelogic::GameLogicError::Threading(error.to_string()))?;
        if instance.get_name().as_str() != name
            || instance.get_controlling_player_id() != Some(neutral.id)
        {
            return Err(invalid(
                "neutral Team does not belong to the receiving neutral Player",
            ));
        }
        Ok(Some(HostNeutralTeam {
            team: neutral.team,
            owner: neutral.id,
            name,
            identity: instance.get_id(),
        }))
    }
    pub(crate) fn host_default_team_id(&self, player_id: u32) -> Option<u32> {
        let player = self.players.get(&player_id)?;
        let identity = player.default_team_id?;
        let expected_name = self.default_host_team_instance_name(Some(player_id), player.team);
        let expected_owner = player.script_identity()?;
        let factory = self.team_factory.lock().unwrap_or_else(|e| e.into_inner());
        let instance = factory.find_team_by_id(identity)?;
        let instance = instance.read().unwrap_or_else(|e| e.into_inner());
        if instance.get_name().as_str() != expected_name
            || instance.get_controlling_player_id() != Some(player_id)
        {
            return None;
        }
        let prototype = factory.find_team_prototype(&expected_name)?;
        let belongs_to_player = prototype.get_owner_name().as_str() == expected_owner
            && factory
                .owned_team_prototypes_for_controller(player_id)
                .iter()
                .any(|owned| std::sync::Arc::ptr_eq(owned, &prototype));
        belongs_to_player.then_some(identity)
    }

    /// Team.cpp:1447: exact self, team override, player override, receiving
    /// player's relationship. Never call Core's ambient PlayerList fallback.
    pub(in crate::game_logic) fn host_team_relationship(
        &self,
        source: u32,
        target: u32,
    ) -> Option<gamelogic::common::Relationship> {
        let factory = self.team_factory.lock().unwrap_or_else(|e| e.into_inner());
        let source_team = factory.find_team_by_id(source)?;
        let target_team = factory.find_team_by_id(target)?;
        drop(factory);
        let source_team = source_team.read().unwrap_or_else(|e| e.into_inner());
        let (relation, target_owner) = if source == target {
            (
                source_team.override_relationship_with_team(&source_team),
                source_team.get_controlling_player_id()?,
            )
        } else {
            let target_team = target_team.read().unwrap_or_else(|e| e.into_inner());
            (
                source_team.override_relationship_with_team(&target_team),
                target_team.get_controlling_player_id()?,
            )
        };
        if let Some(relation) = relation {
            return Some(relation);
        }
        let source_owner = source_team.get_controlling_player_id()?;
        drop(source_team);
        let player = self.players.get(&source_owner)?;
        if let Some(relation) = player
            .team_relationship_override(target)
            .or_else(|| player.map_relationship(target_owner))
        {
            return Some(relation);
        }
        Some(self.player_relationship(source_owner, target_owner))
    }

    /// Object.cpp:806–903. Membership is observable before onCapture. The
    /// numeric no-op keeps sibling instances distinct even with one controller.
    pub(in crate::game_logic) fn set_host_object_team(
        &mut self,
        id: ObjectId,
        requested: u32,
    ) -> bool {
        self.set_host_object_team_with_effects(id, requested, &mut Default::default())
    }

    pub(in crate::game_logic) fn set_host_object_team_with_effects(
        &mut self,
        id: ObjectId,
        requested: u32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        let Some(old) = self.objects.get(&id) else {
            return false;
        };
        let old_identity = old.team_instance_id;
        let old_owner = old.owner_player_id;
        let old_faction = old.team;
        let old_birth = std::sync::Arc::downgrade(&old.visual_draw_identity_token);
        let destination_owner = {
            let factory = self.team_factory.lock().unwrap_or_else(|e| e.into_inner());
            let Some(team) = factory.find_team_by_id(requested) else {
                return false;
            };
            let owner = team
                .read()
                .unwrap_or_else(|e| e.into_inner())
                .get_controlling_player_id();
            owner
        };
        let Some(owner) = destination_owner else {
            return false;
        };
        let destination = match self.inactive_controller_neutral_team(owner) {
            Ok(Some(neutral)) => neutral.identity,
            Ok(None) => requested,
            Err(error) => {
                log::error!("Cannot resolve receiving Team transfer: {error}");
                return false;
            }
        };
        if old_identity == Some(destination) {
            return true;
        }
        let (name, owner, faction, attitude, priority, destination_team, previous_teams) = {
            let factory = self.team_factory.lock().unwrap_or_else(|e| e.into_inner());
            let Some(team) = factory.find_team_by_id(destination) else {
                return false;
            };
            let selected = team.read().unwrap_or_else(|e| e.into_inner());
            let name = selected.get_name().as_str().to_owned();
            let Some(owner) = selected.get_controlling_player_id() else {
                return false;
            };
            let Some(player) = self.players.get(&owner) else {
                return false;
            };
            let Some(prototype) = factory.find_team_prototype(&name) else {
                return false;
            };
            let attitude = match prototype.get_initial_team_attitude() {
                gamelogic::team::AttitudeType::Sleep => -2,
                gamelogic::team::AttitudeType::Passive => -1,
                gamelogic::team::AttitudeType::Normal | gamelogic::team::AttitudeType::Invalid => 0,
                gamelogic::team::AttitudeType::Alert => 1,
                gamelogic::team::AttitudeType::Aggressive => 2,
            };
            let priority = prototype.get_attack_priority_name().as_str().to_owned();
            let previous_teams = if let Some(previous) =
                old_identity.and_then(|identity| factory.find_team_by_id(identity))
            {
                vec![previous]
            } else {
                // Preserve cleanup of unbound older fixtures without inventing
                // an old controlling Team or charging an unadmitted object.
                factory
                    .get_all_teams()
                    .into_iter()
                    .filter(|previous| !std::sync::Arc::ptr_eq(previous, &team))
                    .collect()
            };
            drop(selected);
            (
                name,
                owner,
                player.team,
                attitude,
                priority,
                team,
                previous_teams,
            )
        };
        let mut left_previous = false;
        for previous in previous_teams {
            let mut previous = previous.write().unwrap_or_else(|error| error.into_inner());
            left_previous |= previous.has_member(id.0);
            previous.remove_member(id.0);
        }
        if left_previous && old_identity.is_some() {
            self.adjust_object_team_energy(id, false);
        }
        let object = self
            .objects
            .get_mut(&id)
            .expect("resolved receiving object");
        object.team_instance_id = Some(destination);
        object.team_instance_name = name;
        object.team = faction;
        object.team_color = faction.get_color();
        object.owner_player_id = Some(owner);
        let joined = {
            let mut team = destination_team
                .write()
                .unwrap_or_else(|error| error.into_inner());
            let joined = !team.has_member(id.0);
            team.add_member(id.0);
            joined
        };
        if joined {
            self.adjust_object_team_energy(id, true);
        }
        let object = self.objects.get_mut(&id).expect("receiving Team member");
        if object.has_ai_update_interface() {
            object.set_ai_attitude_i8(attitude);
            if !priority.is_empty() && self.attack_priority_sets.contains_key(&priority) {
                object.attack_priority_set = Some(priority);
            }
        }
        object.apply_fake_building_terrain_decal();
        if old_identity.is_some() {
            self.on_capture_object_owned_with_effects(
                id,
                old_faction,
                faction,
                old_owner != Some(owner),
                effects,
                Some((old_birth, old_owner, Some(owner))),
            );
        }
        if self.radar.contains_object(id.0) {
            self.host_radar_remove_object(id);
            self.host_radar_add_object_with_effects(id, effects);
        }
        if old_owner != Some(owner) || old_faction != faction {
            self.ownership_events.record(id, faction, Some(owner));
        }
        true
    }

    /// Object.cpp:6111–6220. Defect has stronger admission gates and before/
    /// after-switch effects than a direct team assignment.
    #[cfg(test)]
    pub(in crate::game_logic) fn defect_host_object(
        &mut self,
        id: ObjectId,
        destination: u32,
        frames: u32,
    ) -> bool {
        self.defect_host_object_with_effects(id, destination, frames, &mut Default::default())
            .expect("valid defect test fixture")
    }

    pub(in crate::game_logic) fn defect_host_object_with_effects(
        &mut self,
        id: ObjectId,
        destination: u32,
        frames: u32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        let Some(object) = self.objects.get(&id) else {
            return Ok(false);
        };
        if object.contained_by.is_some() || object.status.under_construction || object.status.sold {
            return Ok(false);
        }
        let Some(old_owner) = object.owner_player_id else {
            return Ok(false);
        };
        let Some(default) = self.host_default_team_id(old_owner) else {
            return Ok(false);
        };
        if default == destination {
            return Ok(false);
        }
        let target_owner = {
            let factory = self.team_factory.lock().unwrap_or_else(|e| e.into_inner());
            let Some(team) = factory.find_team_by_id(destination) else {
                return Ok(false);
            };
            let owner = team
                .read()
                .unwrap_or_else(|e| e.into_inner())
                .get_controlling_player_id();
            owner
        };
        let Some(new_owner) = target_owner.filter(|owner| self.players.contains_key(owner)) else {
            return Ok(false);
        };
        let position = object.get_position();
        let voice_defect = object.thing().template.voice_defect.clone();
        self.cancel_all_production(id);
        if self.player_is_playable_side(old_owner) && self.player_is_playable_side(new_owner) {
            self.try_infiltration_event(id);
        }
        self.objects
            .get_mut(&id)
            .expect("admitted defector")
            .begin_undetected_defection(self.frame, frames, true);
        if !self.set_host_object_team_with_effects(id, destination, effects) {
            return Ok(false);
        }
        let has_ai = if let Some(object) = self.objects.get_mut(&id) {
            object.handle_partition_cell_maintenance_with_partition_owner(
                &mut self.partition_manager,
            );
            let has_ai = object.has_ai_update_interface();
            if has_ai {
                object.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                    &mut self.queued_audio_events,
                ));
                object.set_status_moving(false);
                object.set_status_attacking(false);
            }
            has_ai
        } else {
            false
        };
        if has_ai {
            self.set_ai_state_decision_aware_with_effects(id, AIState::Idle, effects)?;
        }
        if has_ai {
            self.stop_attack_decision_aware(id);
            self.clear_target_decision_aware(id);
        }
        if let Some(event) = voice_defect {
            self.queue_audio_event(
                AudioEventRequest::new(&event)
                    .with_object(id)
                    .with_position(position)
                    .with_priority(180),
            );
        }
        if let Some(object) = self.objects.get_mut(&id) {
            object.flash_as_selected();
        }
        self.queue_audio_event(
            AudioEventRequest::new(
                crate::game_logic::host_defector_special_power::DEFECTOR_TIMER_TICK_AUDIO,
            )
            .with_object(id)
            .with_position(position)
            .with_priority(160),
        );
        let faction = self.objects.get(&id).expect("receiving defector").team;
        self.on_defect_kick_passengers_owned_with_effects(id, faction, effects);
        self.defect_parked_units_owned_with_effects(id, destination, frames, effects)?;
        // Object.cpp:6203 visits produced mines in the canonical Object list.
        let mines: Vec<_> = self
            .objects
            .ordered_ids()
            .iter()
            .copied()
            .filter(|mine_id| {
                self.objects.get(mine_id).is_some_and(|mine| {
                    mine.is_kind_of(KindOf::Mine) && mine.producer_id == Some(id)
                })
            })
            .collect();
        for mine in mines {
            self.set_host_object_team_with_effects(mine, destination, effects);
        }
        Ok(true)
    }
}
