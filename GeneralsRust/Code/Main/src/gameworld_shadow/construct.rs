//! Construct, ID maps, ordinal helpers, and host sync.

use super::*;
use crate::game_logic::{GameLogic, ObjectId, Team};
use gamelogic::world::entities::{EntityId, EntityProductionItem, TemplateRef, Transform};
use gamelogic::world::{GameWorld, PlayerId};
use std::collections::{HashMap, HashSet};

impl GameWorldShadow {
    fn host_ready_structure_special_power_template(
        logic: &GameLogic,
        obj: &crate::game_logic::Object,
    ) -> Option<(String, u32)> {
        use crate::command_system::SpecialPowerType as P;

        obj.thing()
            .template
            .special_power_modules
            .iter()
            .find(|module| {
                module.command_power.as_ref().is_some_and(|power| {
                    matches!(
                        power,
                        &P::ParticleCannon
                            | &P::SuperweaponParticleCannon
                            | &P::LaserCannon
                            | &P::ScudStorm
                            | &P::NuclearMissile
                            | &P::NukeNeutronMissile
                            | &P::SuperweaponNeutronMissile
                    ) && logic.is_special_power_ready_for(obj.id, power)
                })
            })
            .map(|module| {
                (
                    module.special_power_template.clone(),
                    module.special_power_template_id,
                )
            })
    }

    pub fn new() -> Self {
        Self {
            world: GameWorld::new(8),
            host_to_entity: Default::default(),
            host_player_to_gw: HashMap::new(),
        }
    }

    pub fn world(&self) -> &GameWorld {
        &self.world
    }

    #[cfg(test)]
    pub fn world_mut(&mut self) -> &mut GameWorld {
        &mut self.world
    }

    pub fn entity_for_host(&self, host: ObjectId) -> Option<EntityId> {
        self.host_to_entity.get(&host.0).copied()
    }

    /// Resolve this projection's player identity without consulting another session.
    #[cfg(test)]
    pub(crate) fn player_for_host(&self, host: u32) -> Option<PlayerId> {
        self.host_player_to_gw.get(&host).copied()
    }

    /// Full/delta sync from host: create, update health/transform/owner, destroy missing.
    /// Preserves EntityId for host objects that still exist.
    pub(super) fn host_building_type_ordinal(t: crate::game_logic::BuildingType) -> u8 {
        use crate::game_logic::BuildingType as B;
        match t {
            B::CommandCenter => 0,
            B::Barracks => 1,
            B::WarFactory => 2,
            B::Airfield => 3,
            B::RepairPad => 4,
            B::HealPad => 5,
            B::SupplyCenter => 6,
            B::PowerPlant => 7,
            B::DefenseTurret => 8,
            B::SupplyDropZone => 9,
            B::Palace => 10,
            B::Propaganda => 11,
            B::Bunker => 12,
        }
    }

    pub(super) fn host_veterancy_ordinal(level: crate::game_logic::VeterancyLevel) -> u8 {
        use crate::game_logic::VeterancyLevel as V;
        match level {
            V::Rookie => 0,
            V::Veteran => 1,
            V::Elite => 2,
            V::Heroic => 3,
        }
    }

    /// Presentation KindOf ORDER residual (must match PresentationFrame freeze ORDER).
    pub(super) fn host_kind_of_bits(obj: &crate::game_logic::Object) -> u32 {
        obj.presentation_kind_of_bits()
    }

    pub(super) fn host_object_type_ordinal(t: crate::game_logic::ObjectType) -> u8 {
        use crate::game_logic::ObjectType as T;
        match t {
            T::Infantry => 0,
            T::Vehicle => 1,
            T::Aircraft => 2,
            T::Building => 3,
            T::Supply => 4,
            T::Projectile => 5,
            T::Neutral => 6,
        }
    }

    pub(super) fn host_team_ordinal(team: Team) -> u8 {
        match team {
            Team::USA => 0,
            Team::China => 1,
            Team::GLA => 2,
            Team::Neutral => 255,
        }
    }

    pub(super) fn entity_team_from_ordinal(ord: u8) -> Team {
        match ord {
            0 => Team::USA,
            1 => Team::China,
            2 => Team::GLA,
            _ => Team::Neutral,
        }
    }

    fn sync_object_identities(&mut self, logic: &GameLogic, ids: &[ObjectId]) {
        self.host_to_entity.retain(|host, entity| {
            if ids.binary_search_by_key(host, |id| id.0).is_ok() {
                return self.world.entity(*entity).is_some();
            }
            self.world.world_mut().remove_entity(*entity);
            false
        });

        // Bind every ObjectID before publishing any cross-object references.
        for &id in ids {
            if self.host_to_entity.contains_key(&id.0) {
                continue;
            }
            let Some(object) = logic.host_object(id) else {
                continue;
            };
            let position = object.get_position();
            let pose = Transform::new(position.to_array(), object.get_orientation());
            let owner = self.owner_for_host_object(logic, object);
            self.spawn_mapped(id, object, owner, pose, object.health.current.max(0.0));
        }
    }

    pub fn sync_from_host(&mut self, logic: &GameLogic) {
        use crate::fow_rendering::{ObjectShroudFrame, ObjectVisibility};

        self.sync_players(logic);
        let host_objects = logic.host_objects();
        let mut ids = host_objects.registered_ids().to_vec();
        ids.sort_unstable_by_key(|id| id.0);
        self.sync_object_identities(logic, &ids);
        let assets = logic.application_assets();
        let catalog = assets.as_ref().and_then(|assets| {
            assets.lock().ok().map(|manager| manager.object_catalog())
        });
        let local_player = logic.local_player_id().unwrap_or(0);
        let shell = logic.isInShellGame();
        let shroud = if shell {
            None
        } else {
            logic.world_services.shroud().lock().ok()
        };
        let shroud_frame = ObjectShroudFrame::new(shroud.as_deref(), local_player);
        for oid in ids {
            let Some(obj) = logic.host_object(oid) else {
                continue;
            };
            let Some(&eid) = self.host_to_entity.get(&oid.0) else {
                continue;
            };
            // Registered objects missing their controller retain the vision pass's
            // faction fallback. GameWorld ownership itself uses the explicit ID.
            let visibility = if shell
                || obj.owner_player_id.or_else(|| logic.player_id_for_team(obj.team))
                    == Some(local_player)
            {
                ObjectVisibility::FULLY_VISIBLE
            } else {
                shroud_frame.visibility(obj.id)
            };
            self.refresh_mapped_from_host(
                logic,
                oid,
                obj,
                eid,
                catalog.as_deref(),
                visibility,
            );
        }
        drop(shroud);

        // Align frame.
        let target = logic.get_frame() as u64;
        self.world.set_frame(target);
    }

    pub(super) fn spawn_mapped(
        &mut self,
        host: ObjectId,
        obj: &crate::game_logic::Object,
        owner: Option<PlayerId>,
        transform: Transform,
        health: f32,
    ) {
        let eid = self.world.spawn_entity(
            TemplateRef::new(obj.template_name.clone()),
            owner,
            transform,
            health,
        );
        self.host_to_entity.insert(host.0, eid);
    }

    pub(super) fn copy_host_player_residual(
        pd: &mut gamelogic::world::PlayerData,
        p: &crate::game_logic::Player,
        frame: u32,
    ) {
        // C++ Player::init binds the faction team at player creation
        // (Player.cpp:287 radar state is per-owning-player). A reused GW slot
        // must track ITS OWN host player's faction, or a player-set mutation
        // (skirmish enemy/observer spawn) leaves the slot keyed to a stale
        // faction and the radar recompute/writeback zero the wrong player.
        pd.team = match p.team {
            Team::USA => Some(0),
            Team::China => Some(1),
            Team::GLA => Some(2),
            Team::Neutral => None,
        };
        pd.supplies = p.resources.supplies;
        pd.power_available = p.energy_available_at(frame);
        pd.power_produced = p.energy_production_at(frame);
        pd.power_consumed = p.power_consumed;
        pd.radar_count = p.radar_count;
        pd.radar_disabled = p.radar_disabled;
        pd.is_alive = p.is_alive;
        pd.cash_bounty_percent = p.cash_bounty_percent.clamp(0.0, 1.0);
        pd.color_rgb = p.color_rgb;
        pd.rank_level = p.rank_level.max(1);
        pd.skill_points = p.skill_points;
        pd.science_purchase_points = p.science_purchase_points;
        let mut cds: Vec<(String, f32)> = p
            .shared_special_power_cooldowns
            .iter()
            .map(|(k, v)| (format!("{k:?}"), (*v).max(0.0)))
            .collect();
        cds.sort_by(|a, b| a.0.cmp(&b.0));
        pd.shared_special_power_cooldowns = cds;
        pd.is_human = p.is_human;
        pd.name.clone_from(&p.name);
    }

    pub(super) fn host_player_sciences(logic: &GameLogic, host_pid: u32) -> Vec<String> {
        logic
            .get_player(host_pid)
            .map(|p| {
                let mut sciences: Vec<String> = p.unlocked_sciences.iter().cloned().collect();
                sciences.sort();
                sciences
            })
            .unwrap_or_default()
    }

    fn sync_player_identities(&mut self, logic: &GameLogic) {
        let players = logic.get_players();
        if players.len() == self.host_player_to_gw.len()
            && players.keys().all(|id| self.host_player_to_gw.contains_key(id))
        {
            return;
        }
        let mut host_ids: Vec<u32> = players.keys().copied().collect();
        host_ids.sort_unstable();

        self.host_player_to_gw.clear();
        // Existing entities retain the receiving world and its player slots.
        if self.host_to_entity.is_empty() {
            let cap = host_ids.len().max(8).min(255);
            self.world = GameWorld::new(cap);
            for pid in &host_ids {
                let Some(p) = logic.get_player(*pid) else {
                    continue;
                };
                let team = match p.team {
                    Team::USA => Some(0),
                    Team::China => Some(1),
                    Team::GLA => Some(2),
                    Team::Neutral => None,
                };
                if let Some(gw_id) = self.world.allocate_player_with_economy(
                    Some(p.name.clone()),
                    team,
                    p.is_human,
                    p.resources.supplies,
                    p.energy_available_at(logic.frame),
                ) {
                    self.host_player_to_gw.insert(*pid, gw_id);
                }
            }
        } else {
            // Entities live: keep existing GW players; rebuild host map by sorted order
            // matching prior allocation order (dense 0..n).
            for (idx, pid) in host_ids.iter().enumerate() {
                let gw = PlayerId::from_index(idx as u8);
                if self.world.player(gw).is_some() {
                    self.host_player_to_gw.insert(*pid, gw);
                } else if let Some(p) = logic.get_player(*pid) {
                    let team = match p.team {
                        Team::USA => Some(0),
                        Team::China => Some(1),
                        Team::GLA => Some(2),
                        Team::Neutral => None,
                    };
                    if let Some(gw_id) = self.world.allocate_player_with_economy(
                        Some(p.name.clone()),
                        team,
                        p.is_human,
                        p.resources.supplies,
                        p.energy_available_at(logic.frame),
                    ) {
                        self.host_player_to_gw.insert(*pid, gw_id);
                    }
                }
            }
        }
    }

    fn sync_players(&mut self, logic: &GameLogic) {
        self.sync_player_identities(logic);
        // Always refresh science/power-bar residual for mapped players.
        for (&hid, &gw) in &self.host_player_to_gw {
            if let Some(p) = logic.get_player(hid) {
                if let Some(pd) = self.world.player_mut(gw) {
                    Self::copy_host_player_residual(pd, p, logic.frame);
                    pd.unlocked_sciences = Self::host_player_sciences(logic, hid);
                }
            }
        }
    }

    /// Reverse map GameWorld owner → host Team (for TransferOwner writeback).
    pub(super) fn host_team_for_gw_owner(
        &self,
        logic: &GameLogic,
        owner: Option<PlayerId>,
    ) -> Option<Team> {
        let Some(pid) = owner else {
            return Some(Team::Neutral);
        };
        for (&hid, &gpid) in &self.host_player_to_gw {
            if gpid == pid {
                if let Some(p) = logic.get_player(hid) {
                    return Some(p.team);
                }
            }
        }
        None
    }

    /// Reverse-map a GameWorld owner to its host player identity.
    pub(super) fn host_player_for_gw_owner(&self, owner: Option<PlayerId>) -> Option<u32> {
        let owner = owner?;
        self.host_player_to_gw
            .iter()
            .find_map(|(&host_player_id, &gw_player_id)| {
                (gw_player_id == owner).then_some(host_player_id)
            })
    }

    /// Resolve a host object through its explicit controlling player. Objects
    /// without a player remain neutral in GameWorld; choosing the first player
    /// with the same faction breaks USA-vs-USA skirmishes.
    pub(super) fn owner_for_host_object(
        &self,
        _logic: &GameLogic,
        object: &crate::game_logic::Object,
    ) -> Option<PlayerId> {
        object
            .owner_player_id
            .and_then(|player_id| self.host_player_to_gw.get(&player_id).copied())
    }
}

#[path = "object_observation.rs"]
mod object_observation;
