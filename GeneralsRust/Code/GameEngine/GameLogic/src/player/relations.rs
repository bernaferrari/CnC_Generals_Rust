use super::*;

impl Player {
    /// Get relationship between this player and another player
    /// Matches C++ Player.cpp:542 Player::getRelationship
    pub fn get_relationship(&self, that_player: &Player) -> Relationship {
        self.player_relations
            .map
            .get(&that_player.get_player_index())
            .copied()
            .unwrap_or(Relationship::Neutral)
    }

    /// Get relationship between this player and a team
    /// Checks team override first, then player override, then neutral
    /// Matches C++ Player.cpp:542-572 Player::getRelationship(const Team*)
    pub fn get_relationship_with_team(&self, that_team: &Team) -> Relationship {
        // Check for team-specific relationship override
        if let Some(ref team_relations) = self.team_relations {
            if let Some(&relationship) = team_relations.map.get(&that_team.get_id()) {
                return relationship;
            }
        }

        // Check for player relationship override
        if let Some(controlling_player_id) = that_team.get_controlling_player_id() {
            if controlling_player_id as PlayerIndex == self.player_index {
                return Relationship::Allies;
            }
            if let Some(&relationship) = self
                .player_relations
                .map
                .get(&(controlling_player_id as PlayerIndex))
            {
                return relationship;
            }
        }

        Relationship::Neutral
    }

    /// Set player-to-player relationship
    /// Matches C++ Player.cpp:575 Player::setPlayerRelationship
    pub fn set_player_relationship(&mut self, that_player: &Player, relationship: Relationship) {
        self.player_relations
            .map
            .insert(that_player.get_player_index(), relationship);
    }

    /// Set player-to-player relationship by player index.
    /// Thin helper for script actions that only carry resolved player IDs.
    pub fn set_player_relationship_by_index(
        &mut self,
        that_player_index: PlayerIndex,
        relationship: Relationship,
    ) {
        self.player_relations
            .map
            .insert(that_player_index, relationship);
    }

    /// Remove player-to-player relationship override
    /// Matches C++ Player.cpp:585 Player::removePlayerRelationship
    pub fn remove_player_relationship(&mut self, that_player: &Player) -> Bool {
        self.player_relations
            .map
            .remove(&that_player.get_player_index())
            .is_some()
    }

    /// Set player-to-team relationship override
    /// Matches C++ Player.cpp:608 Player::setTeamRelationship
    pub fn set_team_relationship(&mut self, that_team: &Team, relationship: Relationship) {
        if self.team_relations.is_none() {
            self.team_relations = Some(TeamRelationMap::new());
        }
        if let Some(ref mut team_relations) = self.team_relations {
            team_relations.map.insert(that_team.get_id(), relationship);
        }
    }

    /// Explicit `m_teamRelations` entry only (C++ Player.cpp:548-554).
    pub fn override_relationship_for_team(&self, that_team: &Team) -> Option<Relationship> {
        self.team_relations
            .as_ref()
            .and_then(|rels| rels.map.get(&that_team.get_id()).copied())
    }

    /// Remove player-to-team relationship override
    /// Matches C++ Player.cpp:618 Player::removeTeamRelationship
    pub fn remove_team_relationship(&mut self, that_team: &Team) -> Bool {
        if let Some(ref mut team_relations) = self.team_relations {
            team_relations.map.remove(&that_team.get_id()).is_some()
        } else {
            false
        }
    }

    pub fn sciences_disabled_types(&self) -> &[ScienceType] {
        &self.sciences_disabled
    }

    pub fn sciences_hidden_types(&self) -> &[ScienceType] {
        &self.sciences_hidden
    }

    pub fn team_relation_pairs(&self) -> Vec<(TeamID, i32)> {
        self.team_relations
            .as_ref()
            .map(|rels| {
                rels.map
                    .iter()
                    .map(|(&id, &rel)| (id, rel as i32))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn player_relation_pairs(&self) -> Vec<(PlayerIndex, i32)> {
        self.player_relations
            .map
            .iter()
            .map(|(&idx, &rel)| (idx, rel as i32))
            .collect()
    }

    pub fn set_team_relationship_by_id(&mut self, team_id: TeamID, relationship: Relationship) {
        if self.team_relations.is_none() {
            self.team_relations = Some(TeamRelationMap::new());
        }
        if let Some(ref mut team_relations) = self.team_relations {
            team_relations.map.insert(team_id, relationship);
        }
    }

    /// Check if this player is allied with another player
    pub fn is_allied_with_player(&self, that_player: &Player) -> Bool {
        matches!(self.get_relationship(that_player), Relationship::Allies)
    }

    /// Check if this player is allied with a team
    pub fn is_allied_with_team(&self, that_team: &Team) -> Bool {
        matches!(
            self.get_relationship_with_team(that_team),
            Relationship::Allies
        )
    }

    /// Check if this player is enemy with another player
    pub fn is_enemy_with_player(&self, that_player: &Player) -> Bool {
        matches!(self.get_relationship(that_player), Relationship::Enemies)
    }

    /// Check if this player is enemy with a team
    pub fn is_enemy_with_team(&self, that_team: &Team) -> Bool {
        matches!(
            self.get_relationship_with_team(that_team),
            Relationship::Enemies
        )
    }

    /// Get all allies of this player
    pub fn get_allied_players(&self) -> Vec<PlayerIndex> {
        self.player_relations
            .map
            .iter()
            .filter_map(|(&index, &rel)| {
                if rel == Relationship::Allies {
                    Some(index)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get all enemies of this player
    pub fn get_enemy_players(&self) -> Vec<PlayerIndex> {
        self.player_relations
            .map
            .iter()
            .filter_map(|(&index, &rel)| {
                if rel == Relationship::Enemies {
                    Some(index)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Check if this player shares vision with another player (due to alliance)
    pub fn shares_vision_with(&self, that_player: &Player) -> Bool {
        self.is_allied_with_player(that_player)
    }

    /// Check if this player shares radar with another player (due to alliance)
    pub fn shares_radar_with(&self, that_player: &Player) -> Bool {
        self.is_allied_with_player(that_player) && self.has_radar() && that_player.has_radar()
    }

    /// Get tunnel system for this player (GLA faction tunnels)
    /// Returns reference to the tunnel network for this player
    pub fn get_tunnel_system(&self) -> Option<&TunnelTracker> {
        self.tunnel_tracker.as_ref()
    }

    /// Get mutable reference to tunnel system for this player
    pub fn get_tunnel_system_mut(&mut self) -> Option<&mut TunnelTracker> {
        self.tunnel_tracker.as_mut()
    }

    pub fn get_resource_manager(&self) -> Option<&ResourceGatheringManager> {
        self.resource_manager.as_ref()
    }

    pub fn get_resource_manager_mut(&mut self) -> Option<&mut ResourceGatheringManager> {
        self.resource_manager.as_mut()
    }

    /// Initialize tunnel tracker for this player
    /// Should be called when a player builds their first tunnel entrance
    pub fn init_tunnel_tracker(&mut self) {
        if self.tunnel_tracker.is_none() {
            self.tunnel_tracker = Some(TunnelTracker::new());
        }
    }

    /// Change battle plan count for this player
    /// Battle plans are strategic bonuses that affect units
    pub fn change_battle_plan(
        &mut self,
        plan_type: BattlePlanType,
        delta: Int,
        bonus: &BattlePlanBonuses,
    ) {
        let mut add_bonus = false;
        let mut remove_bonus = false;

        match plan_type {
            BattlePlanType::Bombard => {
                self.bombard_battle_plans += delta;
                if self.bombard_battle_plans == 1 && delta == 1 {
                    add_bonus = true;
                } else if self.bombard_battle_plans == 0 && delta == -1 {
                    remove_bonus = true;
                }
            }
            BattlePlanType::HoldTheLine => {
                self.hold_the_line_battle_plans += delta;
                if self.hold_the_line_battle_plans == 1 && delta == 1 {
                    add_bonus = true;
                } else if self.hold_the_line_battle_plans == 0 && delta == -1 {
                    remove_bonus = true;
                }
            }
            BattlePlanType::SearchAndDestroy => {
                self.search_and_destroy_battle_plans += delta;
                if self.search_and_destroy_battle_plans == 1 && delta == 1 {
                    add_bonus = true;
                } else if self.search_and_destroy_battle_plans == 0 && delta == -1 {
                    remove_bonus = true;
                }
            }
        }

        if add_bonus {
            self.apply_battle_plan_bonuses_for_player_objects(bonus);
        } else if remove_bonus {
            let mut inverted = bonus.clone();
            inverted.armor_scalar = 1.0 / inverted.armor_scalar.max(0.01);
            inverted.sight_range_scalar = 1.0 / inverted.sight_range_scalar.max(0.01);
            if inverted.bombardment > 0 {
                inverted.bombardment = -1;
            }
            if inverted.hold_the_line > 0 {
                inverted.hold_the_line = -1;
            }
            if inverted.search_and_destroy > 0 {
                inverted.search_and_destroy = -1;
            }
            self.apply_battle_plan_bonuses_for_player_objects(&inverted);
        }
    }

    /// Get battle plan count
    pub fn get_battle_plan_count(&self, plan_type: BattlePlanType) -> Int {
        match plan_type {
            BattlePlanType::Bombard => self.bombard_battle_plans,
            BattlePlanType::HoldTheLine => self.hold_the_line_battle_plans,
            BattlePlanType::SearchAndDestroy => self.search_and_destroy_battle_plans,
        }
    }

    /// Total number of active battle plans (matching C++ getNumBattlePlansActive).
    pub fn get_num_battle_plans_active(&self) -> Int {
        self.bombard_battle_plans
            + self.hold_the_line_battle_plans
            + self.search_and_destroy_battle_plans
    }

    pub(super) fn local_apply_battle_plan_bonuses_to_object(
        &self,
        obj: &mut Object,
        bonus: &BattlePlanBonuses,
    ) {
        let mut object_to_validate_id = obj.get_object_id();
        let is_projectile = obj.is_kind_of(KindOf::Projectile);
        if is_projectile {
            let producer_id = obj.get_producer_id();
            if producer_id != INVALID_ID {
                object_to_validate_id = producer_id;
            }
        }

        let kind_mask = if object_to_validate_id == obj.get_object_id() {
            obj.get_kind_of()
        } else {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(object_to_validate_id) else {
                return;
            };
            let Ok(guard) = obj_arc.read() else {
                return;
            };
            guard.get_kind_of()
        };
        if (kind_mask & bonus.valid_kind_of) == 0 {
            return;
        }
        if (kind_mask & bonus.invalid_kind_of) != 0 {
            return;
        }

        if !is_projectile {
            if (bonus.armor_scalar - 1.0).abs() > f32::EPSILON {
                if let Some(body) = obj.get_body_module() {
                    if let Ok(mut body_guard) = body.lock() {
                        let _ = body_guard.apply_damage_scalar(bonus.armor_scalar);
                    }
                }
            }
            if (bonus.sight_range_scalar - 1.0).abs() > f32::EPSILON {
                let new_range = obj.get_vision_range() * bonus.sight_range_scalar;
                let new_shroud = obj.get_shroud_clearing_range() * bonus.sight_range_scalar;
                obj.set_vision_range(new_range);
                obj.set_shroud_clearing_range(new_shroud);
            }
        }

        if bonus.bombardment > 0 {
            obj.set_weapon_bonus_condition(
                crate::common::types::WeaponBonusConditionType::BattlePlanBombardment,
            );
        } else {
            obj.clear_weapon_bonus_condition(
                crate::common::types::WeaponBonusConditionType::BattlePlanBombardment,
            );
        }
        if bonus.hold_the_line > 0 {
            obj.set_weapon_bonus_condition(
                crate::common::types::WeaponBonusConditionType::BattlePlanHoldTheLine,
            );
        } else {
            obj.clear_weapon_bonus_condition(
                crate::common::types::WeaponBonusConditionType::BattlePlanHoldTheLine,
            );
        }
        if bonus.search_and_destroy > 0 {
            obj.set_weapon_bonus_condition(
                crate::common::types::WeaponBonusConditionType::BattlePlanSearchAndDestroy,
            );
        } else {
            obj.clear_weapon_bonus_condition(
                crate::common::types::WeaponBonusConditionType::BattlePlanSearchAndDestroy,
            );
        }
    }

    /// New object or converted object gaining our current battle plan bonuses.
    pub fn apply_battle_plan_bonuses_for_object(&self, obj: &mut Object) {
        if let Some(bonuses) = &self.battle_plan_bonuses {
            self.local_apply_battle_plan_bonuses_to_object(obj, bonuses);
        }
    }

    /// Object has just left our team, so remove its bonuses.
    pub fn remove_battle_plan_bonuses_for_object(&self, obj: &mut Object) {
        let Some(bonuses) = &self.battle_plan_bonuses else {
            return;
        };

        let mut inverted = bonuses.clone();
        inverted.armor_scalar = 1.0 / inverted.armor_scalar.max(0.01);
        inverted.sight_range_scalar = 1.0 / inverted.sight_range_scalar.max(0.01);
        inverted.bombardment = -1;
        inverted.search_and_destroy = -1;
        inverted.hold_the_line = -1;

        self.local_apply_battle_plan_bonuses_to_object(obj, &inverted);
    }

    /// Battle plan bonuses changing, so apply to all of our objects.
    pub fn apply_battle_plan_bonuses_for_player_objects(&mut self, bonus: &BattlePlanBonuses) {
        if let Some(existing) = &mut self.battle_plan_bonuses {
            existing.armor_scalar *= bonus.armor_scalar;
            existing.sight_range_scalar *= bonus.sight_range_scalar;
            existing.bombardment = (existing.bombardment + bonus.bombardment).max(0);
            existing.hold_the_line = (existing.hold_the_line + bonus.hold_the_line).max(0);
            existing.search_and_destroy =
                (existing.search_and_destroy + bonus.search_and_destroy).max(0);
        } else {
            self.battle_plan_bonuses = Some(bonus.clone());
        }

        let owned_objects = self.owned_objects.clone();
        for object_id in owned_objects {
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                if let Ok(mut guard) = obj_arc.write() {
                    self.local_apply_battle_plan_bonuses_to_object(&mut guard, bonus);
                }
            }
        }
    }
}
