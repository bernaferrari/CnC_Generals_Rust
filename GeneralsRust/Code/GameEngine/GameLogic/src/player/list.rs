use super::*;

/// Global player management
pub(super) static PLAYER_LIST: OnceLock<RwLock<PlayerList>> = OnceLock::new();

/// Player list management (matching C++ PlayerList functionality)
#[derive(Debug)]
pub struct PlayerList {
    pub(super) players: Vec<Arc<RwLock<Player>>>,
    pub(super) local_player_index: PlayerIndex,
}

impl PlayerList {
    pub fn new() -> Self {
        Self {
            players: Vec::new(),
            local_player_index: PLAYER_INDEX_INVALID,
        }
    }

    pub fn add_player(&mut self, player: Arc<RwLock<Player>>) {
        self.players.push(player);
    }

    pub fn get_player(&self, index: PlayerIndex) -> Option<&Arc<RwLock<Player>>> {
        // C++ PlayerList::getNthPlayer (PlayerList.cpp:66-75) returns the fixed
        // slot m_players[i]; the ctor allocates one Player(i) per index
        // (PlayerList.cpp:43-47). The addressed slot is the player's own index,
        // never the count of live players before it, so a sparse list (e.g.
        // only the ReplayObserver side registered at index N) must not hand
        // back its first live player for index 0.
        if index < 0 {
            return None;
        }
        self.players
            .iter()
            .find(|player| {
                player
                    .read()
                    .ok()
                    .is_some_and(|guard| guard.get_player_index() == index)
            })
    }

    pub fn get_player_count(&self) -> usize {
        self.players.len()
    }

    pub fn set_local_player_index(&mut self, index: PlayerIndex) {
        self.local_player_index = index;
    }

    pub fn get_local_player_index(&self) -> PlayerIndex {
        self.local_player_index
    }

    pub fn get_local_player(&self) -> Option<&Arc<RwLock<Player>>> {
        if self.local_player_index != PLAYER_INDEX_INVALID {
            self.get_player(self.local_player_index)
        } else {
            None
        }
    }

    pub fn clear(&mut self) {
        self.players.clear();
        self.local_player_index = PLAYER_INDEX_INVALID;
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Arc<RwLock<Player>>> {
        self.players.iter()
    }

    pub fn get_neutral_player(&self) -> Option<Arc<RwLock<Player>>> {
        self.players.iter().find_map(|player| {
            let guard = player.read().ok()?;
            if guard.get_player_type() == PlayerType::Neutral {
                Some(Arc::clone(player))
            } else {
                None
            }
        })
    }

    /// Find a player by name key (from player name)
    /// Matches C++ PlayerList::findPlayerWithNameKey()
    pub fn find_player_by_name(&self, name: &str) -> Option<Arc<RwLock<Player>>> {
        let key = NameKeyGenerator::name_to_key(name);
        self.players.iter().find_map(|player| {
            let guard = player.read().ok()?;
            if guard.get_player_name_key() == key {
                Some(Arc::clone(player))
            } else {
                None
            }
        })
    }
}

// Provide PlayerManager operations directly on PlayerList for systems that hold the list lock.
impl crate::commands::command_processor::PlayerManager for PlayerList {
    fn get_player_resources(
        &self,
        player_id: Int,
    ) -> Option<crate::commands::command_processor::PlayerResources> {
        let player_arc = self.get_player(player_id).cloned()?;
        let player = player_arc.read().ok()?;
        Some(crate::commands::command_processor::PlayerResources {
            supplies: player.get_money().get_money(),
            power_available: player.get_energy().production(),
            power_used: player.get_energy().consumption(),
        })
    }

    fn modify_player_resources(&mut self, player_id: Int, supplies: Int, power: Int) {
        if let Some(player_arc) = self.get_player(player_id).cloned() {
            if let Ok(mut player) = player_arc.write() {
                player.get_money_mut().add_money(supplies);
                if power > 0 {
                    player.add_power_production(power);
                } else if power < 0 {
                    player.add_power_consumption(-power);
                }
            }
        }
    }

    fn can_player_afford(
        &self,
        player_id: Int,
        cost: &crate::commands::command_processor::ResourceCost,
    ) -> bool {
        if let Some(player_arc) = self.get_player(player_id).cloned() {
            if let Ok(player) = player_arc.read() {
                return player.get_money().can_afford(cost.supplies);
            }
        }
        false
    }
}

/// Global access to player list (matching C++ ThePlayerList)
pub fn player_list() -> &'static RwLock<PlayerList> {
    PLAYER_LIST.get_or_init(|| RwLock::new(PlayerList::new()))
}

/// Convenience alias for C++ compatibility
pub use player_list as ThePlayerList;
/// Extension trait for Arc<RwLock<Player>> to provide helper methods
pub trait PlayerArcExt {
    fn change_battle_plan(&self, plan_type: BattlePlanType, delta: Int, bonus: &BattlePlanBonuses);
    fn has_upgrade_complete(&self, upgrade_template: &UpgradeTemplate) -> Bool;
    fn has_upgrade_in_production(&self, upgrade_template: &UpgradeTemplate) -> Bool;
    fn add_upgrade(
        &self,
        upgrade_template: &UpgradeTemplate,
        status: crate::upgrade::UpgradeStatus,
    );
    fn remove_upgrade(&self, upgrade_template: &UpgradeTemplate);
    fn iterate_objects<F>(&self, func: F) -> Result<(), GameError>
    where
        F: FnMut(Arc<RwLock<Object>>) -> Result<(), GameError>;
    fn iterate_object_ids<F>(&self, func: F) -> Result<(), GameError>
    where
        F: FnMut(ObjectID) -> Result<(), GameError>;
    fn get_player_template(&self) -> Option<Arc<PlayerTemplate>>;
    fn allowed_to_build(&self, template: &dyn crate::common::ThingTemplate) -> Bool;
}

impl PlayerArcExt for Arc<RwLock<Player>> {
    /// Change battle plan count for this player
    fn change_battle_plan(&self, plan_type: BattlePlanType, delta: Int, bonus: &BattlePlanBonuses) {
        if let Ok(mut guard) = self.write() {
            guard.change_battle_plan(plan_type, delta, bonus);
        }
    }

    /// Check if player has upgrade complete
    fn has_upgrade_complete(&self, upgrade_template: &UpgradeTemplate) -> Bool {
        if let Ok(guard) = self.read() {
            guard.has_upgrade_complete(upgrade_template)
        } else {
            false
        }
    }

    /// Check if upgrade is in production
    fn has_upgrade_in_production(&self, upgrade_template: &UpgradeTemplate) -> Bool {
        if let Ok(guard) = self.read() {
            guard.has_upgrade_in_production(upgrade_template)
        } else {
            false
        }
    }

    /// Add upgrade to player
    /// Matches C++ Player::addUpgrade
    fn add_upgrade(
        &self,
        upgrade_template: &UpgradeTemplate,
        status: crate::upgrade::UpgradeStatus,
    ) {
        // Owned-object roster snapshot taken on completion, for the
        // C++ onUpgradeCompleted fan-out after the lock is released.
        let mut completed_roster: Vec<ObjectID> = Vec::new();
        if let Ok(mut guard) = self.write() {
            // Create new upgrade instance
            let upgrade = Upgrade::new(Arc::new(upgrade_template.clone()));

            // Set the status
            let mut upgrade_mut = upgrade;
            upgrade_mut.set_status(status);

            // Get the upgrade mask bit for this upgrade
            let upgrade_name = upgrade_template.get_name();
            let upgrade_mask = crate::upgrade::upgrade_mask_for_name(upgrade_name.as_str());
            let mask_bit = UpgradeMaskType::from_bits_retain(upgrade_mask.bits());
            // Update the appropriate mask based on status
            match status {
                crate::upgrade::UpgradeStatus::InProduction => {
                    guard.upgrades_in_progress = guard.upgrades_in_progress | mask_bit;
                }
                crate::upgrade::UpgradeStatus::Complete => {
                    guard.upgrades_completed = guard.upgrades_completed | mask_bit;
                    // Remove from in-progress if it was there
                    guard.upgrades_in_progress = guard.upgrades_in_progress & !mask_bit;
                    // Live leftover add_upgrade notify (host upgrade complete).
                    guard.academy_stats.record_upgrade(upgrade_template, false);
                    // Keep PlayerUpgradeManager.active_upgrades in sync: the
                    // per-object re-check reads that mask (C++ reads the same
                    // completed mask via Object::updateUpgradeModules).
                    if let Some(manager) = guard.get_upgrade_manager_mut() {
                        manager.add_completed_upgrade(upgrade_template.get_name_key(), upgrade_mask);
                    }
                    completed_roster = guard.get_all_objects();
                }
                crate::upgrade::UpgradeStatus::Invalid => {
                    // Do nothing for invalid status
                }
            }

            // Add to upgrade list if not already present
            if !guard
                .upgrade_list
                .iter()
                .any(|u| u.get_template().get_name() == upgrade_template.get_name())
            {
                guard.upgrade_list.push(upgrade_mut);
            }
        }

        // C++ Player.cpp:3038 — addUpgrade(COMPLETE) ends with
        // onUpgradeCompleted, re-checking UpgradeModules on every object the
        // player owns. Runs after the player write guard is dropped because
        // the per-object re-check reads this player again.
        if !completed_roster.is_empty() {
            on_upgrade_completed_fanout(completed_roster);
        }
    }

    /// Remove upgrade from player
    /// Matches C++ Player::removeUpgrade
    fn remove_upgrade(&self, upgrade_template: &UpgradeTemplate) {
        if let Ok(mut guard) = self.write() {
            // Remove from upgrade list
            let upgrade_name = upgrade_template.get_name();
            guard
                .upgrade_list
                .retain(|u| u.get_template().get_name() != upgrade_name);

            // Clear from masks
            let mask_bit = UpgradeMaskType::from_bits_retain(
                crate::upgrade::upgrade_mask_for_name(upgrade_name.as_str()).bits(),
            );
            guard.upgrades_in_progress = guard.upgrades_in_progress & !mask_bit;
            guard.upgrades_completed = guard.upgrades_completed & !mask_bit;
        }
    }

    /// Iterate over the objects owned by this player
    fn iterate_objects<F>(&self, mut func: F) -> Result<(), GameError>
    where
        F: FnMut(Arc<RwLock<Object>>) -> Result<(), GameError>,
    {
        if let Ok(guard) = self.read() {
            // Get all objects owned by this player from the object manager
            let obj_manager = get_object_manager();
            if let Ok(manager) = obj_manager.read() {
                let object_ids =
                    manager.get_objects_owned_by_player(guard.player_index as UnsignedInt);

                // Iterate through each object and call the function
                for obj_id in object_ids {
                    if let Some(obj_arc) = manager.get_object(obj_id) {
                        // Call the function with the object
                        if let Ok(obj_instance) = obj_arc.read() {
                            let base_obj = obj_instance.base();
                            func(base_obj)?;
                        }
                    }
                }
            }
            Ok(())
        } else {
            Err(GameLogicError::LockError)
        }
    }

    fn iterate_object_ids<F>(&self, mut func: F) -> Result<(), GameError>
    where
        F: FnMut(ObjectID) -> Result<(), GameError>,
    {
        if let Ok(guard) = self.read() {
            guard.iterate_object_ids(func)
        } else {
            Ok(())
        }
    }

    /// Get the player template
    fn get_player_template(&self) -> Option<Arc<PlayerTemplate>> {
        if let Ok(guard) = self.read() {
            guard.get_player_template().cloned()
        } else {
            None
        }
    }

    /// Check if player is allowed to build the given template
    fn allowed_to_build(&self, template: &dyn crate::common::ThingTemplate) -> Bool {
        if let Ok(guard) = self.read() {
            guard.can_build_template(template)
        } else {
            false
        }
    }
}

/// C++ Player::onUpgradeCompleted (Player.cpp:3054-3081): an upgrade just
/// finished, tell all of the player's objects to re-check their
/// UpgradeModules (StatusBits/ReplaceObject/WeaponSet and friends).
fn on_upgrade_completed_fanout(object_ids: Vec<ObjectID>) {
    // The create-hook owner already holds its object's write lock; its init
    // tail re-checks modules (C++ Object::initObject → updateUpgradeModules).
    let skip_id = crate::object::create::create_owner_id();
    for object_id in object_ids {
        if Some(object_id) == skip_id {
            continue;
        }
        let _ = crate::object::registry::OBJECT_REGISTRY
            .with_object_mut(object_id, |object_guard| {
                object_guard.update_upgrade_modules_from_player();
            });
    }
}
