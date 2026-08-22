// Transfer, kill, membership, and vision sharing
//
// Split from `team.rs` for module-size parity.
// Observable behavior is unchanged.

impl Team {
    /// Transfer all units to another team
    pub fn transfer_units_to(&mut self, new_team: &mut Team) {
        if self.id == new_team.id {
            return;
        }

        let new_team_arc = get_team_factory()
            .lock()
            .ok()
            .and_then(|factory| factory.find_team_by_id(new_team.id));

        let members = self.members.clone();
        for object_id in members {
            // C++ obj->setTeam(newTeam) updates team pointer, player membership,
            // and becomingTeamMember. Member lists are still patched here because
            // set_or_restore_team uses try_write and this caller already holds
            // both team write locks.
            if let Some(team_arc) = &new_team_arc {
                let _ = OBJECT_REGISTRY.with_object_mut(object_id, |object| {
                    let _ = object.set_team(Some(Arc::clone(team_arc)));
                });
            }
            new_team.add_member(object_id);
            self.remove_member(object_id);
        }
    }

    /// Kill all team members
    pub fn kill_team(&mut self) {
        // Wave 256: empty dual-world → no factory member walks.
        if dual_world_registry_unavailable() {
            return;
        }

        self.evacuate_team_containers();

        let neutral_default_team = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_neutral_player())
            .and_then(|player_arc| {
                player_arc
                    .read()
                    .ok()
                    .and_then(|player| player.get_default_team())
            });

        // C++ parity (Team::killTeam): effectively-dead beacon objects are still processed.
        let beacon_template = self
            .controlling_player_id
            .and_then(|player_id| {
                player_list()
                    .read()
                    .ok()
                    .and_then(|list| list.get_player(player_id as Int).cloned())
            })
            .and_then(|player_arc| {
                player_arc.read().ok().and_then(|player| {
                    player
                        .get_player_template()
                        .map(|template| template.beacon_name.clone())
                })
            })
            .and_then(|beacon_name| {
                if beacon_name.is_empty() {
                    None
                } else {
                    TheThingFactory::find_template(beacon_name.as_str())
                }
            });

        let members = self.members.clone();
        let mut moved_to_neutral = Vec::new();
        for object_id in members {
            let Some((_is_beacon, is_tech_building)) = OBJECT_REGISTRY
                .with_object(object_id, |object_guard| {
                    let is_beacon = beacon_template.as_ref().is_some_and(|template| {
                        object_guard
                            .get_template()
                            .is_equivalent_to(template.as_ref())
                    });
                    let destroyed = object_guard.is_destroyed();
                    let effectively_dead = object_guard.is_effectively_dead();
                    let same_team = object_guard.get_team_id() == Some(self.id);
                    if destroyed || (effectively_dead && !is_beacon) || !same_team {
                        return None;
                    }
                    Some((is_beacon, object_guard.is_kind_of(KindOf::TechBuilding)))
                })
                .flatten()
            else {
                continue;
            };

            if is_tech_building {
                if let Some(neutral_team) = neutral_default_team.clone() {
                    let moved = OBJECT_REGISTRY
                        .with_object_mut(object_id, |object_guard| {
                            let _ = object_guard.set_team(Some(neutral_team));
                        })
                        .is_some();
                    if moved {
                        moved_to_neutral.push(object_id);
                    }
                } else {
                    let _ = OBJECT_REGISTRY.with_object_mut(object_id, |object_guard| {
                        object_guard.kill(Some(DamageType::Unresistable), Some(DeathType::Normal));
                    });
                }
            } else {
                let _ = OBJECT_REGISTRY.with_object_mut(object_id, |object_guard| {
                    object_guard.kill(Some(DamageType::Unresistable), Some(DeathType::Normal));
                });
            }
        }

        if !moved_to_neutral.is_empty() {
            let moved_set: HashSet<ObjectID> = moved_to_neutral.iter().copied().collect();
            self.members.retain(|id| !moved_set.contains(id));
            self.cur_units = self.members.len() as Int;

            if let Some(neutral_team) = neutral_default_team {
                if let Ok(mut neutral_guard) = neutral_team.write() {
                    for object_id in moved_to_neutral {
                        neutral_guard.add_member(object_id);
                    }
                }
            }
        }
    }

    fn is_default_team_for_controller(&self) -> bool {
        let Some(controller_id) = self.controlling_player_id else {
            return false;
        };
        let Ok(players) = player_list().read() else {
            return false;
        };
        let Some(player_arc) = players.get_player(controller_id as Int).cloned() else {
            return false;
        };
        let Ok(player) = player_arc.read() else {
            return false;
        };
        let Some(default_team) = player.get_default_team() else {
            return false;
        };
        let Ok(default_team_guard) = default_team.read() else {
            return false;
        };
        default_team_guard.get_id() == self.id
    }

    fn evacuate_team_containers(&self) {
        if OBJECT_REGISTRY.is_empty() {
            return;
        }
        let members = self.members.clone();
        for object_id in members {
            let Some(contain_arc) = OBJECT_REGISTRY
                .with_object(object_id, |object_guard| {
                    if object_guard.is_destroyed() || object_guard.is_effectively_dead() {
                        return None;
                    }
                    object_guard.get_contain()
                })
                .flatten()
            else {
                continue;
            };
            let Ok(mut contain_guard) = contain_arc.lock() else {
                continue;
            };
            if contain_guard.get_contain_count() > 0 {
                let _ = contain_guard.remove_all_contained(false);
            }
        }
    }

    // Member management
    pub fn add_member(&mut self, object_id: ObjectID) {
        if !self.members.contains(&object_id) {
            self.members.push(object_id);
            // C++ DLINK insert does not touch m_curUnits; Team::updateState recounts.
        }
    }

    pub fn remove_member(&mut self, object_id: ObjectID) {
        self.members.retain(|&id| id != object_id);
        // C++ DLINK unlink does not touch m_curUnits; Team::updateState recounts.
    }

    pub fn get_members(&self) -> &[ObjectID] {
        &self.members
    }

    pub fn get_member_count(&self) -> usize {
        self.members.len()
    }

    pub fn has_member(&self, object_id: ObjectID) -> bool {
        self.members.contains(&object_id)
    }

    /// Check if this team is allied with another team
    pub fn is_allied_with(&self, that_team: &Team) -> Bool {
        matches!(self.get_relationship(that_team), Relationship::Allies)
    }

    /// Check if this team is enemy with another team
    pub fn is_enemy_with(&self, that_team: &Team) -> Bool {
        matches!(self.get_relationship(that_team), Relationship::Enemies)
    }

    /// Check if this team is neutral with another team
    pub fn is_neutral_with(&self, that_team: &Team) -> Bool {
        matches!(self.get_relationship(that_team), Relationship::Neutral)
    }

    /// Get all team members (for iteration/AI)
    pub fn iterate_members<F>(&self, mut func: F)
    where
        F: FnMut(ObjectID),
    {
        for &member_id in &self.members {
            func(member_id);
        }
    }

    /// Check if team can target another team (enemy relationship)
    pub fn can_target_team(&self, that_team: &Team) -> Bool {
        self.is_enemy_with(that_team)
    }

    /// Get vision shared teams (all allied teams)
    pub fn get_vision_shared_teams(&self) -> Vec<TeamID> {
        let Ok(factory) = get_team_factory().lock() else {
            return Vec::new();
        };

        let mut shared = Vec::new();
        for team_arc in factory.get_all_teams() {
            let Ok(team_guard) = team_arc.read() else {
                continue;
            };
            if team_guard.get_id() == self.id {
                continue;
            }
            if self.is_allied_with(&team_guard) {
                shared.push(team_guard.get_id());
            }
        }
        shared
    }

    /// Check if this team shares vision with another team
    pub fn shares_vision_with(&self, that_team: &Team) -> Bool {
        self.is_allied_with(that_team)
    }

    /// Check if this team shares radar with another team
    pub fn shares_radar_with(&self, that_team: &Team) -> Bool {
        // Teams share radar if they are allied
        // Full implementation would also check if controlling players have radar
        self.is_allied_with(that_team)
    }

    /// Check if this team is a singleton (only one instance allowed)
    /// Reference: C++ Team::GetIsSingleton()
    pub fn is_singleton(&self) -> Bool {
        self.is_singleton
    }

    /// Set singleton flag on team instance
    /// Reference: C++ Team::SetIsSingleton()
    pub fn set_singleton(&mut self, singleton: Bool) {
        self.is_singleton = singleton;
    }
}
