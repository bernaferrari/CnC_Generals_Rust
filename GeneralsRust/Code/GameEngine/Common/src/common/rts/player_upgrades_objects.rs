use super::*;

impl Player {
    // =========================================================
    // Upgrade List Management (C++ Player.h line 336)
    // =========================================================

    /// Add an upgrade to the player's list
    /// C++ Reference: Player::addUpgrade() (Player.cpp)
    pub fn add_upgrade(&mut self, upgrade_name: String, status: UpgradeStatus) {
        // Check if already exists
        if let Some(existing) = self
            .upgrade_list
            .iter_mut()
            .find(|u| u.get_name() == upgrade_name)
        {
            existing.set_status(status);
        } else {
            let mut upgrade = UpgradeInfo::new(upgrade_name);
            upgrade.set_status(status);
            self.upgrade_list.push(upgrade);
        }
    }

    /// Remove an upgrade from the player's list
    /// C++ Reference: Player::removeUpgrade() (Player.cpp)
    pub fn remove_upgrade(&mut self, upgrade_name: &str) {
        self.upgrade_list.retain(|u| u.get_name() != upgrade_name);
    }

    /// Find an upgrade by name
    /// C++ Reference: Player::findUpgrade() (Player.h line 163)
    pub fn find_upgrade(&self, upgrade_name: &str) -> Option<&UpgradeInfo> {
        self.upgrade_list
            .iter()
            .find(|u| u.get_name() == upgrade_name)
    }

    /// Find mutable upgrade by name
    pub fn find_upgrade_mut(&mut self, upgrade_name: &str) -> Option<&mut UpgradeInfo> {
        self.upgrade_list
            .iter_mut()
            .find(|u| u.get_name() == upgrade_name)
    }

    /// Check if player has an upgrade complete
    /// C++ Reference: Player::hasUpgradeComplete() (Player.h line 157)
    pub fn has_upgrade_complete(&self, upgrade_name: &str) -> bool {
        self.upgrade_list
            .iter()
            .any(|u| u.get_name() == upgrade_name && u.is_complete())
    }

    /// Check if player has an upgrade in production
    /// C++ Reference: Player::hasUpgradeInProduction() (Player.h line 160)
    pub fn has_upgrade_in_production(&self, upgrade_name: &str) -> bool {
        self.upgrade_list
            .iter()
            .any(|u| u.get_name() == upgrade_name && u.is_in_production())
    }

    /// Get completed upgrade mask
    /// C++ Reference: Player::getCompletedUpgradeMask() (Player.h line 159)
    pub fn get_completed_upgrade_mask(&self) -> u128 {
        self.upgrades_completed
    }

    /// Set upgrade in progress bit
    pub fn set_upgrade_in_progress(&mut self, bit: u32) {
        if bit < 128 {
            self.upgrades_in_progress |= 1u128 << bit;
        }
    }

    /// Clear upgrade in progress bit
    pub fn clear_upgrade_in_progress(&mut self, bit: u32) {
        if bit < 128 {
            self.upgrades_in_progress &= !(1u128 << bit);
        }
    }

    /// Set upgrade completed bit
    pub fn set_upgrade_completed(&mut self, bit: u32) {
        if bit < 128 {
            self.upgrades_completed |= 1u128 << bit;
            // Clear from in-progress when completed
            self.upgrades_in_progress &= !(1u128 << bit);
        }
    }

    /// Clear upgrade completed bit
    pub fn clear_upgrade_completed(&mut self, bit: u32) {
        if bit < 128 {
            self.upgrades_completed &= !(1u128 << bit);
        }
    }

    // =========================================================
    // Team Prototype List (C++ Player.h line 375)
    // =========================================================

    /// Add a team prototype to the player's list
    /// Add a team prototype to the player's list
    /// C++ Reference: Player::addTeamToList() (Player.cpp lines 974-982)
    pub fn add_team_prototype(&mut self, team_name: String) {
        if !self.team_prototypes.contains(&team_name) {
            self.team_prototypes.push(team_name.clone());
        }
        if let Ok(mut factory) = crate::common::rts::team::get_team_factory().lock() {
            if let Some(proto) = factory.find_team_prototype_mut(&team_name) {
                proto.set_controlling_player(Some(self.index as usize));
            }
        }
    }

    /// Remove a team prototype from the player's list
    /// C++ Reference: Player::removeTeamFromList() (Player.cpp lines 985-995)
    pub fn remove_team_prototype(&mut self, team_name: &str) {
        self.team_prototypes.retain(|name| name != team_name);
    }

    /// Get all team prototypes
    pub fn get_team_prototypes(&self) -> &[String] {
        &self.team_prototypes
    }

    pub fn add_owned_object(&mut self, id: ObjectID) {
        if id != INVALID_OBJECT_ID && !self.owned_objects.contains(&id) {
            self.owned_objects.push(id);
        }
    }

    pub fn remove_owned_object(&mut self, id: ObjectID) {
        self.owned_objects.retain(|existing| *existing != id);
    }

    pub fn owned_object_ids(&self) -> &[ObjectID] {
        &self.owned_objects
    }

    pub(super) fn collect_owned_object_ids(
        &self,
        world: &Arc<dyn PlayerObjectWorld>,
    ) -> Vec<ObjectID> {
        let mut ids = world.object_ids_for_player(self.index);
        ids.extend(self.owned_objects.iter().copied());
        if let Ok(factory) = crate::common::rts::team::get_team_factory().lock() {
            for name in &self.team_prototypes {
                if let Some(proto) = factory.find_team_prototype(name) {
                    for team in proto.iter_team_instances() {
                        ids.extend(team.iter_members().copied());
                    }
                }
            }
        }
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    /// C++ Player::updateTeamStates
    pub fn update_team_states(&mut self) {
        if let Ok(mut factory) = crate::common::rts::team::get_team_factory().lock() {
            for name in &self.team_prototypes {
                if let Some(proto) = factory.find_team_prototype_mut(name) {
                    proto.update_state();
                }
            }
        }
    }

    pub fn is_local_player(&self) -> bool {
        self.is_local
    }

    /// C++ Player::becomingLocalPlayer
    pub fn becoming_local_player(&mut self, yes: bool) {
        self.is_local = yes;
        if !yes {
            return;
        }
        if let Some(world) = get_player_object_world() {
            if let Some(color) = self.current_player_template().map(|t| t.preferred_color) {
                let r = ((color >> 16) & 0xFF) as i32;
                let g = ((color >> 8) & 0xFF) as i32;
                let b = (color & 0xFF) as i32;
                world.set_team_color(r, g, b);
            }

            let mut ids = world.all_object_ids();
            if ids.is_empty() {
                ids = self.collect_owned_object_ids(&world);
            }
            for id in ids {
                if let Some(snap) = world.snapshot(id) {
                    if snap.has_contain {
                        world.recalc_contain_and_radar(id);
                    }
                    if snap.is_disguiser {
                        world.refresh_disguise_for_local(id, self.index);
                    }
                } else {
                    world.recalc_contain_and_radar(id);
                }
            }
            world.mark_ui_dirty();
        }
    }

    // =========================================================
    // Tunnel System (C++ Player.h line 341)
    // =========================================================

    /// Add a tunnel entrance
    pub fn add_tunnel_entrance(&mut self, entrance_id: ObjectID) {
        if !self.tunnel_entrances.contains(&entrance_id) {
            self.tunnel_entrances.push(entrance_id);
        }
    }

    /// Remove a tunnel entrance
    pub fn remove_tunnel_entrance(&mut self, entrance_id: ObjectID) {
        self.tunnel_entrances.retain(|&id| id != entrance_id);
    }

    /// Get all tunnel entrances
    pub fn get_tunnel_entrances(&self) -> &[ObjectID] {
        &self.tunnel_entrances
    }
}
