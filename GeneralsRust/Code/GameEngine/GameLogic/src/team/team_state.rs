// Sighted state, update_state, and relationship overrides
//
// Split from `team.rs` for module-size parity.
// Observable behavior is unchanged.

fn host_hook_member_ids(team_name: &str) -> Vec<u32> {
    crate::scripting::host_script_team_member_ids(team_name)
}

fn host_hook_member_count(team_name: &str) -> Int {
    host_hook_member_ids(team_name).len() as Int
}

fn host_hook_live_count(team_name: &str) -> Int {
    host_hook_member_ids(team_name)
        .into_iter()
        .filter_map(crate::scripting::host_script_query_object_by_id)
        .filter(|obj| obj.alive && !obj.effectively_dead)
        .count() as Int
}

fn host_object_counts_as_team_ai(obj: &crate::scripting::HostScriptQueryObject) -> bool {
    // C++ Team::updateState OnIdle only counts members with AIUpdateInterface.
    !obj.kind_structure && !obj.kind_projectile && !obj.kind_inert && !obj.kind_mine
}

fn host_hook_idle_state(team_name: &str) -> (bool, bool) {
    let mut is_idle = true;
    let mut any_alive = false;
    for id in host_hook_member_ids(team_name) {
        let Some(obj) = crate::scripting::host_script_query_object_by_id(id) else {
            continue;
        };
        if !obj.alive || obj.effectively_dead {
            continue;
        }
        if !host_object_counts_as_team_ai(&obj) {
            continue;
        }
        any_alive = true;
        if !obj.idle {
            is_idle = false;
        }
    }
    (is_idle, any_alive)
}

fn host_hook_is_enemy(
    looker: &crate::scripting::HostScriptQueryObject,
    cand: &crate::scripting::HostScriptQueryObject,
) -> bool {
    if !looker.owner_player.is_empty()
        && looker.owner_player.eq_ignore_ascii_case(&cand.owner_player)
    {
        return false;
    }
    const HOST_NEUTRAL_TEAM: u32 = 3;
    if looker.team == HOST_NEUTRAL_TEAM || cand.team == HOST_NEUTRAL_TEAM {
        return false;
    }
    looker.team != cand.team
}

fn host_hook_enemy_sighted(team_name: &str) -> (bool, bool) {
    let members = host_hook_member_ids(team_name);
    let mut candidates = members.clone();
    for faction in 0..4 {
        candidates.extend(crate::scripting::host_script_team_unit_ids(faction));
    }
    candidates.sort_unstable();
    candidates.dedup();

    let mut any_alive = false;
    for mid in &members {
        let Some(looker) = crate::scripting::host_script_query_object_by_id(*mid) else {
            continue;
        };
        if !looker.alive || looker.effectively_dead {
            continue;
        }
        any_alive = true;
        let range_sq = looker.vision_range * looker.vision_range;
        for cid in &candidates {
            if *cid == looker.id {
                continue;
            }
            let Some(cand) = crate::scripting::host_script_query_object_by_id(*cid) else {
                continue;
            };
            if !cand.alive || cand.effectively_dead {
                continue;
            }
            let dx = cand.x - looker.x;
            let dz = cand.z - looker.z;
            if dx * dx + dz * dz > range_sq {
                continue;
            }
            if host_hook_is_enemy(&looker, &cand) {
                return (true, true);
            }
        }
    }
    (false, any_alive)
}

impl Team {

    fn compute_enemy_sighted_state(&self) -> (Bool, Bool) {
        let Some(partition) = ThePartitionManager::get() else {
            return (false, false);
        };

        let mut any_alive_in_team = false;
        for &object_id in &self.members {
            let Some(found_enemy) = OBJECT_REGISTRY
                .with_object(object_id, |source| {
                    if source.is_effectively_dead() {
                        return None;
                    }
                    any_alive_in_team = true;

                    let source_pos = *source.get_position();
                    let vision_range = source.get_vision_range();
                    let source_off_map = source.is_off_map();

                    for candidate_id in partition.get_objects_in_range(&source_pos, vision_range) {
                        if candidate_id == object_id {
                            continue;
                        }
                        let is_enemy = OBJECT_REGISTRY
                            .with_object(candidate_id, |candidate| {
                                if candidate.is_effectively_dead() {
                                    return false;
                                }
                                if candidate.is_off_map() != source_off_map {
                                    return false;
                                }

                                // C++ Team.cpp:1823-1833: ALLOW_ENEMIES + Alive + SameMapStatus only.
                                // PartitionFilterStealthedAndUndetected exists in C++ and is unused here.
                                source.relationship_to(candidate) == Relationship::Enemies
                            })
                            .unwrap_or(false);
                        if is_enemy {
                            return Some(true);
                        }
                    }
                    Some(false)
                })
                .flatten()
            else {
                continue;
            };
            if found_enemy {
                return (true, true);
            }
        }

        (false, any_alive_in_team)
    }

    /// Check if team was just created
    pub fn is_created(&self) -> Bool {
        self.created
    }

    /// Note that a team member entered/exited trigger area
    pub fn set_entered_exited(&mut self) {
        self.entered_or_exited = true;
    }

    /// Check if member entered/exited trigger area
    pub fn did_enter_or_exit(&self) -> Bool {
        self.entered_or_exited
    }

    /// Update team state (called each frame)
    pub fn update_state(&mut self) {
        // C++ Team.cpp:1785 — no dual-world registry skip.
        self.entered_or_exited = false;
        if !self.active {
            return;
        }

        let host_census =
            OBJECT_REGISTRY.is_empty() && crate::scripting::host_script_query_has_any();

        if self.created {
            self.created = false;

            if !self.script_on_create.is_empty() {
                queue_team_script_event(self.name.as_str(), self.script_on_create.as_str());
            }

            if !self.script_on_destroyed.is_empty() {
                self.cur_units = if host_census {
                    host_hook_member_count(self.name.as_str())
                } else {
                    self.members.len() as Int
                };
                self.destroy_threshold = self.cur_units
                    - (self.cur_units as Real * self.destroyed_threshold_ratio) as Int;

                if self.destroy_threshold > self.cur_units - 1 {
                    self.destroy_threshold = self.cur_units - 1;
                }
                if self.destroy_threshold < 0 {
                    self.destroy_threshold = 0;
                }
            }
        }

        if self.check_enemy_sighted {
            self.prev_see_enemy = self.see_enemy;
            let (see_enemy_now, any_alive_in_team) = if host_census {
                host_hook_enemy_sighted(self.name.as_str())
            } else {
                self.compute_enemy_sighted_state()
            };
            self.see_enemy = see_enemy_now;

            if any_alive_in_team && self.prev_see_enemy != self.see_enemy {
                if self.see_enemy {
                    queue_team_script_event(
                        self.name.as_str(),
                        self.script_on_enemy_sighted.as_str(),
                    );
                } else {
                    queue_team_script_event(self.name.as_str(), self.script_on_all_clear.as_str());
                }
            }
        }

        if !self.script_on_destroyed.is_empty() {
            let prev_units = self.cur_units;
            self.cur_units = if host_census {
                host_hook_live_count(self.name.as_str())
            } else if OBJECT_REGISTRY.is_empty() {
                // No host snapshot: listed members stay counted so OnDestroyed
                // does not fire at activation (C++ only drops live units).
                self.members.len() as Int
            } else {
                let mut live = 0;
                for &object_id in &self.members {
                    let alive = OBJECT_REGISTRY
                        .with_object(object_id, |object_guard| {
                            !object_guard.is_effectively_dead()
                        })
                        .unwrap_or(false);
                    if alive {
                        live += 1;
                    }
                }
                live
            };

            if self.cur_units != prev_units && self.cur_units <= self.destroy_threshold {
                queue_team_script_event(self.name.as_str(), self.script_on_destroyed.as_str());
                self.destroy_threshold = -1;
            }
        }

        if !self.script_on_idle.is_empty() {
            let (is_idle, any_alive_in_team) = if host_census {
                host_hook_idle_state(self.name.as_str())
            } else {
                let mut is_idle = true;
                let mut any_alive_in_team = false;

                for &object_id in &self.members {
                    let Some(idle) = OBJECT_REGISTRY
                        .with_object(object_id, |object_guard| {
                            if object_guard.is_effectively_dead() {
                                return None;
                            }
                            if object_guard.get_ai_update_interface().is_none() {
                                return None;
                            }
                            Some(object_guard.is_idle())
                        })
                        .flatten()
                    else {
                        continue;
                    };

                    any_alive_in_team = true;
                    if !idle {
                        is_idle = false;
                    }
                }
                (is_idle, any_alive_in_team)
            };

            if any_alive_in_team && is_idle && self.was_idle {
                queue_team_script_event(self.name.as_str(), self.script_on_idle.as_str());
            }
            self.was_idle = is_idle;
        }
    }

    /// Notify team of object death
    pub fn notify_team_of_object_death(&mut self) {
        if self.script_on_unit_destroyed.is_empty() {
            return;
        }

        queue_team_script_event(self.name.as_str(), self.script_on_unit_destroyed.as_str());
    }

    /// Get relationship with another team
    /// Matches C++ Team.cpp:1447 Team::getRelationship
    pub fn get_relationship(&self, that_team: &Team) -> Relationship {
        if self.get_id() == that_team.get_id() {
            return Relationship::Allies;
        }

        // Check for team-specific relationship override first
        if let Some(ref relations) = self.team_relations {
            if let Some(&relationship) = relations.map.get(&that_team.get_id()) {
                return relationship;
            }
        }

        // Check for player-specific override
        if let Some(ref player_relations) = self.player_relations {
            if let Some(that_player_id) = that_team.get_controlling_player_id() {
                if let Some(&relationship) = player_relations.get(&(that_player_id as Int)) {
                    return relationship;
                }
            }
        }

        // Fall back to controlling player's relationship with that team.
        if let Some(my_player_id) = self.get_controlling_player_id() {
            if let Ok(players) = player_list().read() {
                if let Some(my_player_arc) = players.get_player(my_player_id as Int).cloned() {
                    if let Ok(my_player) = my_player_arc.read() {
                        return my_player.get_relationship_with_team(that_team);
                    }
                }
            }
        }

        Relationship::Neutral
    }

    /// Get relationship between this team and a player
    pub fn get_relationship_with_player(&self, player_index: Int) -> Relationship {
        // Check for player-specific override
        if let Some(ref player_relations) = self.player_relations {
            if let Some(&relationship) = player_relations.get(&player_index) {
                return relationship;
            }
        }

        if let Some(my_player_id) = self.get_controlling_player_id() {
            if let Ok(players) = player_list().read() {
                if let (Some(my_player_arc), Some(that_player_arc)) = (
                    players.get_player(my_player_id as Int).cloned(),
                    players.get_player(player_index).cloned(),
                ) {
                    if let (Ok(my_player), Ok(that_player)) =
                        (my_player_arc.read(), that_player_arc.read())
                    {
                        return my_player.get_relationship(&that_player);
                    }
                }
            }
        }

        Relationship::Neutral
    }

    /// Set override team relationship
    pub fn set_override_team_relationship(&mut self, team_id: TeamID, relationship: Relationship) {
        if team_id == TEAM_ID_INVALID {
            return;
        }
        if self.team_relations.is_none() {
            self.team_relations = Some(TeamRelationMap::new());
        }
        if let Some(ref mut relations) = self.team_relations {
            relations.map.insert(team_id, relationship);
        }
    }

    /// Remove override team relationship
    pub fn remove_override_team_relationship(&mut self, team_id: TeamID) -> Bool {
        if let Some(ref mut relations) = self.team_relations {
            if relations.map.is_empty() {
                return false;
            }
            if team_id == TEAM_ID_INVALID {
                relations.map.clear();
                return true;
            }
            relations.map.remove(&team_id).is_some()
        } else {
            false
        }
    }

    /// Clear all team-to-team relationship overrides.
    /// Matches C++ Team::removeOverrideTeamRelationship(NULL) behavior.
    pub fn clear_override_team_relationships(&mut self) {
        if let Some(ref mut relations) = self.team_relations {
            relations.map.clear();
        }
    }

    /// Set override player relationship
    pub fn set_override_player_relationship(
        &mut self,
        player_index: Int,
        relationship: Relationship,
    ) {
        if player_index == PLAYER_INDEX_INVALID {
            return;
        }
        if self.player_relations.is_none() {
            self.player_relations = Some(HashMap::new());
        }
        if let Some(ref mut relations) = self.player_relations {
            relations.insert(player_index, relationship);
        }
    }

    /// Remove override player relationship
    pub fn remove_override_player_relationship(&mut self, player_index: Int) -> Bool {
        if let Some(ref mut relations) = self.player_relations {
            if relations.is_empty() {
                return false;
            }
            if player_index == PLAYER_INDEX_INVALID {
                relations.clear();
                return true;
            }
            relations.remove(&player_index).is_some()
        } else {
            false
        }
    }

    /// Clear all team-to-player relationship overrides.
    /// Matches C++ Team::removeOverridePlayerRelationship(NULL) behavior.
    pub fn clear_override_player_relationships(&mut self) {
        if let Some(ref mut relations) = self.player_relations {
            relations.clear();
        }
    }

    /// Explicit `m_teamRelations` / `m_playerRelations` only (C++ Team.cpp:1449-1471).
    /// Does not fall back to the controlling player's relationship.
    pub fn override_relationship_with_team(&self, that_team: &Team) -> Option<Relationship> {
        if let Some(ref relations) = self.team_relations {
            if let Some(&relationship) = relations.map.get(&that_team.get_id()) {
                return Some(relationship);
            }
        }
        if let Some(ref player_relations) = self.player_relations {
            if let Some(that_player_id) = that_team.get_controlling_player_id() {
                if let Some(&relationship) = player_relations.get(&(that_player_id as Int)) {
                    return Some(relationship);
                }
            }
        }
        None
    }

    /// Explicit `m_playerRelations` entry only (TEAM_SET_OVERRIDE_RELATION_TO_PLAYER).
    pub fn override_relationship_with_player(&self, player_index: Int) -> Option<Relationship> {
        self.player_relations
            .as_ref()
            .and_then(|relations| relations.get(&player_index).copied())
    }

    /// Snapshot `m_teamRelations` for Team::xfer / CHUNK_TeamFactory.
    pub fn team_relation_override_pairs(&self) -> Vec<(TeamID, Relationship)> {
        self.team_relations
            .as_ref()
            .map(|relations| relations.map.iter().map(|(&id, &rel)| (id, rel)).collect())
            .unwrap_or_default()
    }

    /// Snapshot `m_playerRelations` for Team::xfer / CHUNK_TeamFactory.
    pub fn player_relation_override_pairs(&self) -> Vec<(Int, Relationship)> {
        self.player_relations
            .as_ref()
            .map(|relations| relations.iter().map(|(&idx, &rel)| (idx, rel)).collect())
            .unwrap_or_default()
    }
}
