//! Receiving Object::setTeam and Object::defect. Team identity is session-local;
//! release the roster borrow before running capture and containment callbacks.
use super::super::*;

impl GameLogic {
    pub(in crate::game_logic) fn host_default_team_id(&self, player_id: u32) -> Option<u32> {
        let player = self.players.get(&player_id)?;
        self.ensure_host_default_team(player_id);
        let name = self.default_host_team_instance_name(Some(player_id), player.team);
        let factory = self.team_factory.lock().unwrap_or_else(|e| e.into_inner());
        let instance = factory.find_team_instances(&name).into_iter().next()?;
        let identity = instance.read().unwrap_or_else(|e| e.into_inner()).get_id();
        Some(identity)
    }

    /// Team.cpp:1447: exact self, team override, player override, receiving
    /// player's relationship. Never call Core's ambient PlayerList fallback.
    pub(in crate::game_logic) fn host_team_relationship(
        &self,
        source: u32,
        target: u32,
    ) -> Option<gamelogic::common::Relationship> {
        use gamelogic::common::Relationship;
        let factory = self.team_factory.lock().unwrap_or_else(|e| e.into_inner());
        let source_team = factory.find_team_by_id(source)?;
        let target_team = factory.find_team_by_id(target)?;
        drop(factory);
        if source == target {
            return Some(Relationship::Allies);
        }
        let source_team = source_team.read().unwrap_or_else(|e| e.into_inner());
        let target_team = target_team.read().unwrap_or_else(|e| e.into_inner());
        if let Some(relation) = source_team.override_relationship_with_team(&target_team) {
            return Some(relation);
        }
        let source_owner = source_team.get_controlling_player_id()?;
        let target_owner = target_team.get_controlling_player_id()?;
        let target_name = target_team.get_name().as_str().to_owned();
        drop(target_team);
        drop(source_team);
        if let Some(relation) = self
            .players
            .get(&source_owner)?
            .team_relationship_override(&target_name)
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
        let Some(old) = self.objects.get(&id) else {
            return false;
        };
        let old_identity = old.team_instance_id;
        let old_owner = old.owner_player_id;
        let old_faction = old.team;
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
        let Some(destination_player) = destination_owner.and_then(|owner| self.players.get(&owner))
        else {
            return false;
        };
        let destination = if destination_player.is_alive {
            requested
        } else {
            // Object::setTeam redirects an inactive controller to neutral's default.
            let neutral = self
                .players
                .iter()
                .filter_map(|(id, p)| p.is_reserved_neutral().then_some(*id))
                .min();
            let Some(identity) = neutral.and_then(|id| self.host_default_team_id(id)) else {
                return false;
            };
            identity
        };
        if old_identity == Some(destination) {
            return true;
        }
        let (name, owner, faction, attitude, priority) = {
            let factory = self.team_factory.lock().unwrap_or_else(|e| e.into_inner());
            let Some(team) = factory.find_team_by_id(destination) else {
                return false;
            };
            let mut team = team.write().unwrap_or_else(|e| e.into_inner());
            let name = team.get_name().as_str().to_owned();
            let Some(owner) = team.get_controlling_player_id() else {
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
            if let Some(previous) =
                old_identity.and_then(|identity| factory.find_team_by_id(identity))
            {
                previous
                    .write()
                    .unwrap_or_else(|e| e.into_inner())
                    .remove_member(id.0);
            } else {
                // Unbound fixtures/older admission routes carry no exact identity.
                // Remove existing membership only; never create a lookup instance.
                for previous in factory.get_all_teams() {
                    if !std::sync::Arc::ptr_eq(
                        &previous,
                        &factory
                            .find_team_by_id(destination)
                            .expect("resolved destination"),
                    ) {
                        previous
                            .write()
                            .unwrap_or_else(|e| e.into_inner())
                            .remove_member(id.0);
                    }
                }
            }
            team.add_member(id.0);
            (name, owner, player.team, attitude, priority)
        };
        let object = self
            .objects
            .get_mut(&id)
            .expect("resolved receiving object");
        object.team_instance_id = Some(destination);
        object.team_instance_name = name;
        object.team = faction;
        object.team_color = faction.get_color();
        object.owner_player_id = Some(owner);
        if object.has_ai_update_interface() {
            object.set_ai_attitude_i8(attitude);
            if !priority.is_empty() && self.attack_priority_sets.contains_key(&priority) {
                object.attack_priority_set = Some(priority);
            }
        }
        object.apply_fake_building_terrain_decal();
        if old_identity.is_some() {
            self.on_capture_object_owned(id, old_faction, faction, old_owner != Some(owner));
        }
        self.host_radar_remove_object(id);
        self.host_radar_add_object(id);
        if old_owner != Some(owner) || old_faction != faction {
            self.ownership_events.record(id, faction, Some(owner));
        }
        true
    }

    /// Object.cpp:6111–6220. Defect has stronger admission gates and before/
    /// after-switch effects than a direct team assignment.
    pub(in crate::game_logic) fn defect_host_object(
        &mut self,
        id: ObjectId,
        destination: u32,
        frames: u32,
    ) -> bool {
        let Some(object) = self.objects.get(&id) else {
            return false;
        };
        if object.contained_by.is_some() || object.status.under_construction || object.status.sold {
            return false;
        }
        let Some(old_owner) = object.owner_player_id else {
            return false;
        };
        let Some(default) = self.host_default_team_id(old_owner) else {
            return false;
        };
        if default == destination {
            return false;
        }
        let target_owner = {
            let factory = self.team_factory.lock().unwrap_or_else(|e| e.into_inner());
            let Some(team) = factory.find_team_by_id(destination) else {
                return false;
            };
            let owner = team
                .read()
                .unwrap_or_else(|e| e.into_inner())
                .get_controlling_player_id();
            owner
        };
        let Some(new_owner) = target_owner.filter(|owner| self.players.contains_key(owner)) else {
            return false;
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
        if !self.set_host_object_team(id, destination) {
            return false;
        }
        if let Some(object) = self.objects.get_mut(&id) {
            object.handle_partition_cell_maintenance();
            if object.has_ai_update_interface() {
                object.stop_moving();
                object.set_status_moving(false);
                object.set_status_attacking(false);
                object.set_ai_state(AIState::Idle);
            }
        }
        self.stop_attack_decision_aware(id);
        self.clear_target_decision_aware(id);
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
        self.on_defect_kick_passengers_owned(id, faction);
        self.defect_parked_units_owned(id, destination, frames);
        // The host's canonical world-list ordering remains a separate migration.
        let mines: Vec<_> = self
            .objects
            .iter()
            .filter_map(|(mine_id, mine)| {
                (mine.is_kind_of(KindOf::Mine) && mine.producer_id == Some(id)).then_some(*mine_id)
            })
            .collect();
        for mine in mines {
            self.set_host_object_team(mine, destination);
        }
        true
    }
}
