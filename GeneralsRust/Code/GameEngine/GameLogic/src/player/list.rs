use super::*;
use std::sync::{LazyLock, RwLockWriteGuard};

/// Global player management.
/// Initializer is fixed, so the list lives in a LazyLock rather than OnceLock.
pub(super) static PLAYER_LIST: LazyLock<RwLock<PlayerList>> =
    LazyLock::new(|| RwLock::new(PlayerList::new()));

/// Player list management (matching C++ PlayerList functionality).
///
/// The list is the single owner of each `Player`. Callers that need a player
/// outside this lock use [`with_player`] / [`with_player_mut`], which check the
/// player out of the map and drop the list lock before the closure.
#[derive(Debug)]
pub struct PlayerList {
    pub(super) players: Vec<Player>,
    pub(super) local_player_index: PlayerIndex,
}

impl PlayerList {
    pub fn new() -> Self {
        Self {
            players: Vec::new(),
            local_player_index: PLAYER_INDEX_INVALID,
        }
    }

    pub fn add_player(&mut self, player: Player) {
        self.players.push(player);
    }

    pub fn get_player(&self, index: PlayerIndex) -> Option<&Player> {
        self.player(index)
    }

    pub fn get_player_mut(&mut self, index: PlayerIndex) -> Option<&mut Player> {
        self.player_mut(index)
    }

    /// Borrow a resident player. Do not call [`with_player`] while holding this borrow.
    pub fn player(&self, index: PlayerIndex) -> Option<&Player> {
        // C++ PlayerList::getNthPlayer returns the slot whose own index is `i`,
        // not the i-th live entry. A sparse list must not hand back player N for 0.
        if index < 0 {
            return None;
        }
        self.players
            .iter()
            .find(|player| player.get_player_index() == index)
    }

    pub fn player_mut(&mut self, index: PlayerIndex) -> Option<&mut Player> {
        if index < 0 {
            return None;
        }
        self.players
            .iter_mut()
            .find(|player| player.get_player_index() == index)
    }

    pub fn player_indices(&self) -> Vec<PlayerIndex> {
        self.players
            .iter()
            .map(|player| player.get_player_index())
            .collect()
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

    pub fn get_local_player(&self) -> Option<&Player> {
        if self.local_player_index != PLAYER_INDEX_INVALID {
            self.player(self.local_player_index)
        } else {
            None
        }
    }

    pub fn get_local_player_mut(&mut self) -> Option<&mut Player> {
        if self.local_player_index != PLAYER_INDEX_INVALID {
            self.player_mut(self.local_player_index)
        } else {
            None
        }
    }

    pub fn clear(&mut self) {
        self.players.clear();
        self.local_player_index = PLAYER_INDEX_INVALID;
    }

    /// Move every player out. Callers must put them back or they are dropped.
    pub fn take_players(&mut self) -> Vec<Player> {
        std::mem::take(&mut self.players)
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Player> {
        self.players.iter()
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, Player> {
        self.players.iter_mut()
    }

    pub fn neutral_player_index(&self) -> Option<PlayerIndex> {
        self.players.iter().find_map(|player| {
            (player.get_player_type() == PlayerType::Neutral).then_some(player.get_player_index())
        })
    }

    pub fn get_neutral_player(&self) -> Option<&Player> {
        self.neutral_player_index()
            .and_then(|index| self.player(index))
    }

    /// Find a player by name key (from player name).
    /// Matches C++ PlayerList::findPlayerWithNameKey().
    pub fn find_player_index_by_name(&self, name: &str) -> Option<PlayerIndex> {
        let key = NameKeyGenerator::name_to_key(name);
        self.players.iter().find_map(|player| {
            (player.get_player_name_key() == key).then_some(player.get_player_index())
        })
    }

    pub fn find_player_by_name(&self, name: &str) -> Option<&Player> {
        self.find_player_index_by_name(name)
            .and_then(|index| self.player(index))
    }

    fn take_player(&mut self, index: PlayerIndex) -> Option<Player> {
        let pos = self
            .players
            .iter()
            .position(|player| player.get_player_index() == index)?;
        Some(self.players.swap_remove(pos))
    }
}

// Provide PlayerManager operations directly on PlayerList for systems that hold the list lock.
impl crate::commands::command_processor::PlayerManager for PlayerList {
    fn get_player_resources(
        &self,
        player_id: Int,
    ) -> Option<crate::commands::command_processor::PlayerResources> {
        let player = self.get_player(player_id)?;
        Some(crate::commands::command_processor::PlayerResources {
            supplies: player.get_money().get_money(),
            power_available: player.get_energy().production(),
            power_used: player.get_energy().consumption(),
        })
    }

    fn modify_player_resources(&mut self, player_id: Int, supplies: Int, power: Int) {
        if let Some(player) = self.get_player_mut(player_id) {
            player.get_money_mut().add_money(supplies);
            if power > 0 {
                player.add_power_production(power);
            } else if power < 0 {
                player.add_power_consumption(-power);
            }
        }
    }

    fn can_player_afford(
        &self,
        player_id: Int,
        cost: &crate::commands::command_processor::ResourceCost,
    ) -> bool {
        self.get_player(player_id)
            .is_some_and(|player| player.get_money().can_afford(cost.supplies))
    }
}

/// Global access to player list (matching C++ ThePlayerList)
pub fn player_list() -> &'static RwLock<PlayerList> {
    &PLAYER_LIST
}

/// Convenience alias for C++ compatibility
pub use player_list as ThePlayerList;

fn player_list_write() -> RwLockWriteGuard<'static, PlayerList> {
    match player_list().write() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

struct PlayerCheckout {
    player: Option<Player>,
}

impl Drop for PlayerCheckout {
    fn drop(&mut self) {
        if let Some(player) = self.player.take() {
            player_list_write().players.push(player);
        }
    }
}

fn checkout_player(index: PlayerIndex) -> Option<PlayerCheckout> {
    if index < 0 {
        return None;
    }
    let player = player_list_write().take_player(index)?;
    Some(PlayerCheckout {
        player: Some(player),
    })
}

/// Check `index` out of the player list, drop the list lock, then call `f`.
///
/// Same-index re-entry inside `f` returns `None`. Use the borrowed player.
pub fn with_player<R>(index: PlayerIndex, f: impl FnOnce(&Player) -> R) -> Option<R> {
    let mut checkout = checkout_player(index)?;
    let player = checkout.player.as_mut()?;
    player.flush_money_side_effects();
    Some(f(player))
}

/// Mutable checkout. See [`with_player`].
pub fn with_player_mut<R>(index: PlayerIndex, f: impl FnOnce(&mut Player) -> R) -> Option<R> {
    let mut checkout = checkout_player(index)?;
    let player = checkout.player.as_mut()?;
    player.flush_money_side_effects();
    let result = f(player);
    if let Some(player) = checkout.player.as_mut() {
        player.flush_money_side_effects();
    }
    Some(result)
}

pub fn with_local_player<R>(f: impl FnOnce(&Player) -> R) -> Option<R> {
    let index = player_list().read().ok()?.get_local_player_index();
    with_player(index, f)
}

pub fn with_local_player_mut<R>(f: impl FnOnce(&mut Player) -> R) -> Option<R> {
    let index = player_list().read().ok()?.get_local_player_index();
    with_player_mut(index, f)
}

pub fn with_neutral_player<R>(f: impl FnOnce(&Player) -> R) -> Option<R> {
    let index = player_list().read().ok()?.neutral_player_index()?;
    with_player(index, f)
}

pub fn with_neutral_player_mut<R>(f: impl FnOnce(&mut Player) -> R) -> Option<R> {
    let index = player_list().read().ok()?.neutral_player_index()?;
    with_player_mut(index, f)
}

pub fn with_player_named<R>(name: &str, f: impl FnOnce(&Player) -> R) -> Option<R> {
    let index = player_list()
        .read()
        .ok()?
        .find_player_index_by_name(name)?;
    with_player(index, f)
}

pub fn with_player_named_mut<R>(name: &str, f: impl FnOnce(&mut Player) -> R) -> Option<R> {
    let index = player_list()
        .read()
        .ok()?
        .find_player_index_by_name(name)?;
    with_player_mut(index, f)
}

pub fn with_each_player(mut f: impl FnMut(&Player)) {
    let indices = player_list()
        .read()
        .ok()
        .map(|list| list.player_indices())
        .unwrap_or_default();
    for index in indices {
        let _ = with_player(index, |player| f(player));
    }
}

pub fn with_each_player_mut(mut f: impl FnMut(&mut Player)) {
    let indices = player_list()
        .read()
        .ok()
        .map(|list| list.player_indices())
        .unwrap_or_default();
    for index in indices {
        let _ = with_player_mut(index, |player| f(player));
    }
}

impl Player {
    /// Add upgrade to player. Matches C++ Player::addUpgrade.
    ///
    /// `skip_object_id` is the object already mutably borrowed by a create hook.
    pub fn add_upgrade(
        &mut self,
        upgrade_template: &UpgradeTemplate,
        status: crate::upgrade::UpgradeStatus,
        skip_object_id: Option<ObjectID>,
    ) {
        let upgrade = Upgrade::new(Arc::new(upgrade_template.clone()));
        let mut upgrade_mut = upgrade;
        upgrade_mut.set_status(status);

        let upgrade_name = upgrade_template.get_name();
        let upgrade_mask = crate::upgrade::upgrade_mask_for_name(upgrade_name.as_str());
        let mask_bit = UpgradeMaskType::from_bits_retain(upgrade_mask.bits());
        let mut completed_roster: Vec<ObjectID> = Vec::new();
        match status {
            crate::upgrade::UpgradeStatus::InProduction => {
                self.upgrades_in_progress = self.upgrades_in_progress | mask_bit;
            }
            crate::upgrade::UpgradeStatus::Complete => {
                self.upgrades_completed = self.upgrades_completed | mask_bit;
                self.upgrades_in_progress = self.upgrades_in_progress & !mask_bit;
                self.academy_stats.record_upgrade(upgrade_template, false);
                if let Some(manager) = self.get_upgrade_manager_mut() {
                    manager.add_completed_upgrade(upgrade_template.get_name_key(), upgrade_mask);
                }
                completed_roster = self.get_all_objects();
            }
            crate::upgrade::UpgradeStatus::Invalid => {}
        }

        if !self
            .upgrade_list
            .iter()
            .any(|u| u.get_template().get_name() == upgrade_template.get_name())
        {
            self.upgrade_list.push(upgrade_mut);
        }

        // C++ Player.cpp:3038 — onUpgradeCompleted after the player is not borrowed
        // by this method's caller only when they drop us first. The fan-out reads
        // objects, not this player, so it is safe while `self` is still borrowed
        // as long as object callbacks do not re-enter this same player mutably.
        if !completed_roster.is_empty() {
            on_upgrade_completed_fanout(completed_roster, skip_object_id);
        }
    }

    /// Remove upgrade from player. Matches C++ Player::removeUpgrade.
    pub fn remove_upgrade(&mut self, upgrade_template: &UpgradeTemplate) {
        let upgrade_name = upgrade_template.get_name();
        self.upgrade_list
            .retain(|u| u.get_template().get_name() != upgrade_name);
        let mask_bit = UpgradeMaskType::from_bits_retain(
            crate::upgrade::upgrade_mask_for_name(upgrade_name.as_str()).bits(),
        );
        self.upgrades_in_progress = self.upgrades_in_progress & !mask_bit;
        self.upgrades_completed = self.upgrades_completed & !mask_bit;
    }
}

/// C++ Player::onUpgradeCompleted (Player.cpp:3054-3081).
///
/// `skip_object_id` is the object already mutably borrowed by a create hook.
fn on_upgrade_completed_fanout(object_ids: Vec<ObjectID>, skip_object_id: Option<ObjectID>) {
    for object_id in object_ids {
        if Some(object_id) == skip_object_id {
            continue;
        }
        let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(object_id, |object_guard| {
            object_guard.update_upgrade_modules_from_player();
        });
    }
}

