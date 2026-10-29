//! Construct, ID maps, ordinal helpers, and host sync.

use super::types::HordePlayerRel;
use super::*;
use crate::game_logic::{GameLogic, ObjectId, Team};
use gamelogic::world::entities::{EntityId, EntityProductionItem, TemplateRef, Transform};
use gamelogic::world::{GameWorld, PlayerId, WorldMutation, WorldSnapshot};
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

    pub fn new(max_entities: usize) -> Self {
        Self {
            world: GameWorld::new(8),
            drawable_tint_envelopes: crate::game_logic::DrawableTintEnvelopes::default(),
            host_to_entity: HashMap::new(),
            entity_to_host: HashMap::new(),
            max_entities: max_entities.max(1),
            host_player_to_gw: HashMap::new(),
            horde_player_rel: HashMap::new(),

            production_power_factor_by_host: HashMap::new(),
            special_power_frozen_by_host: HashMap::new(),
            map_min_x: 0.0,
            map_min_z: 0.0,
            map_max_x: 0.0,
            map_max_z: 0.0,
            artillery_pending_drops: Vec::new(),
            carpet_pending_drops: Vec::new(),
        }
    }

    /// Discard all shadow state at a host-world replacement boundary.
    ///
    /// Host `ObjectId`s are allocator-local and may be reused by a reset,
    /// map load, or staged save restore.  Keeping the old `GameWorld` and its
    /// per-host residual queues would therefore alias entities from the
    /// previous match into the new authoritative world.  Reconstructing the
    /// shadow also clears the GameWorld's pending mutations/projectile and AI
    /// residuals, while retaining the configured entity capacity.
    pub fn reset_for_world_boundary(&mut self) {
        let max_entities = self.max_entities;
        *self = Self::new(max_entities);
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

    pub fn host_for_entity(&self, entity: EntityId) -> Option<ObjectId> {
        self.entity_to_host
            .get(&entity.get())
            .copied()
            .map(ObjectId)
    }

    pub fn mapped_count(&self) -> usize {
        self.host_to_entity.len()
    }

    pub(super) fn invalidate_dead_entity_maps(&mut self) {
        let dead: Vec<u32> = self
            .entity_to_host
            .keys()
            .copied()
            .filter(|eid| self.world.entity(EntityId::from_raw(*eid)).is_none())
            .collect();
        for eid in dead {
            if let Some(hid) = self.entity_to_host.remove(&eid) {
                self.host_to_entity.remove(&hid);
            }
        }
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

    pub fn host_ai_state_ordinal(s: &crate::game_logic::AIState) -> u8 {
        use crate::game_logic::AIState as A;
        match s {
            A::Idle => 0,
            A::Moving => 1,
            A::Attacking => 2,
            A::AttackMoving => 3,
            A::AttackingGround => 4,
            A::Gathering => 5,
            A::ReturningResources => 6,
            A::Constructing => 7,
            A::Repairing => 8,
            A::GuardingArea => 9,
            A::GuardingObject => 10,
            A::GuardRetaliating => 20,
            A::Patrolling => 11,
            A::Docked => 12,
            A::Garrisoned => 13,
            A::SpecialAbility => 14,
            A::SeekingRepair => 15,
            A::SeekingHealing => 16,
            A::Entering => 17,
            A::Docking => 18,
            A::Capturing => 19,
            A::FacingObject => 21,
            A::FacingPosition => 22,
            A::Panic => 23,
            A::Exiting => 24,
            A::Uninitialized => 25,
            A::Hunting => 26,
            A::JetReturningForLanding => 27,
            A::JetReturnToDeadAirfield => 28,
            A::JetCirclingDeadAirfield => 29,
            A::Dead => 30,
        }
    }

    pub fn ai_state_from_ordinal(ordinal: u8) -> crate::game_logic::AIState {
        use crate::game_logic::AIState as A;
        match ordinal {
            1 => A::Moving,
            2 => A::Attacking,
            3 => A::AttackMoving,
            4 => A::AttackingGround,
            5 => A::Gathering,
            6 => A::ReturningResources,
            7 => A::Constructing,
            8 => A::Repairing,
            9 => A::GuardingArea,
            10 => A::GuardingObject,
            11 => A::Patrolling,
            12 => A::Docked,
            13 => A::Garrisoned,
            14 => A::SpecialAbility,
            15 => A::SeekingRepair,
            16 => A::SeekingHealing,
            17 => A::Entering,
            18 => A::Docking,
            19 => A::Capturing,
            20 => A::GuardRetaliating,
            21 => A::FacingObject,
            22 => A::FacingPosition,
            23 => A::Panic,
            24 => A::Exiting,
            25 => A::Uninitialized,
            26 => A::Hunting,
            27 => A::JetReturningForLanding,
            28 => A::JetReturnToDeadAirfield,
            29 => A::JetCirclingDeadAirfield,
            30 => A::Dead,
            _ => A::Idle,
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

    pub fn sync_from_host(&mut self, logic: &GameLogic) {
        self.sync_players(logic);

        let mut obj_ids: Vec<ObjectId> = logic.host_objects().registered_keys().copied().collect();
        obj_ids.sort_by_key(|id| id.0);
        // C++ GameLogic grows its ObjectID lookup vector when a newly
        // allocated ID exceeds the current size (GameLogic.cpp:3827-3841).
        // `max_entities` is only the initial/reset capacity hint; silently
        // truncating this host set would leave later objects without a
        // shadow lifecycle mapping and bypass every coupled authority channel.
        if obj_ids.len() > self.max_entities {
            self.max_entities = obj_ids.len();
        }
        let host_set: HashSet<u32> = obj_ids.iter().map(|id| id.0).collect();

        // Remove shadow entities whose host object is gone.
        let stale: Vec<(u32, EntityId)> = self
            .host_to_entity
            .iter()
            .filter(|(hid, _)| !host_set.contains(hid))
            .map(|(&hid, &eid)| (hid, eid))
            .collect();
        for (hid, eid) in stale {
            let _ = self.world.world_mut().remove_entity(eid);
            self.host_to_entity.remove(&hid);
            self.entity_to_host.remove(&eid.get());
        }

        // Bind identities before copying references so every target/container
        // resolves on the first observation, independent of object iteration order.
        for &oid in &obj_ids {
            let Some(obj) = logic.host_object(oid) else {
                continue;
            };
            if let Some(&eid) = self.host_to_entity.get(&oid.0) {
                if self.world.world().entity(eid).is_some() {
                    continue;
                }
                self.host_to_entity.remove(&oid.0);
                self.entity_to_host.remove(&eid.get());
            }
            let pos = obj.get_position();
            let transform = Transform::new([pos.x, pos.y, pos.z], obj.get_orientation());
            let owner = self.owner_for_host_object(logic, obj);
            self.spawn_mapped(oid, obj, owner, transform, obj.health.current.max(0.0));
        }
        for oid in obj_ids {
            self.refresh_mapped_from_host(logic, oid);
        }

        let mapped: Vec<ObjectId> = self.host_to_entity.keys().copied().map(ObjectId).collect();
        for oid in mapped {
            let Some(obj) = logic.host_objects().lookup(&oid) else {
                continue;
            };
            let Some(&eid) = self.host_to_entity.get(&oid.0) else {
                continue;
            };
            self.sync_weapon_slots_from_host(eid, obj);
        }

        let (map_min, map_max) = logic.world_bounds();
        self.map_min_x = map_min.x;
        self.map_min_z = map_min.z;
        self.map_max_x = map_max.x;
        self.map_max_z = map_max.z;
        // Wave 792: mirror A10 pending missile drops for GW sole-tick.
        self.artillery_pending_drops = logic.artillery_barrage_flight_reg.pending_drops.clone();
        self.carpet_pending_drops = logic.carpet_bomb_flight_reg.pending_drops.clone();

        // Containment is an observation of the receiving Main roster. Rebuild
        // its indexed view after all IDs exist; never admit it back to Main.
        self.world.contain_roster_mut().clear();
        for (&host, object) in logic.host_objects().registered() {
            let Some(&container) = self.host_to_entity.get(&host.0) else {
                continue;
            };
            for occupant in object.contained_units() {
                if let Some(&occupant) = self.host_to_entity.get(&occupant.0) {
                    let _ = self.world.contain_roster_mut().enter(container, occupant);
                }
            }
        }

        self.sync_projectile_observations(logic);

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
        self.entity_to_host.insert(eid.get(), host.0);
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
        pd.name = p.name.clone();
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

    pub(super) fn sync_players(&mut self, logic: &GameLogic) {
        // Rebuild player slots when count/identity changes; economy always refreshed.
        let mut host_ids: Vec<u32> = logic.get_players().keys().copied().collect();
        host_ids.sort_unstable();
        let need_rebuild = host_ids.len() != self.host_player_to_gw.len()
            || host_ids
                .iter()
                .any(|id| !self.host_player_to_gw.contains_key(id));

        if need_rebuild {
            // Fresh world would drop entities — only rebuild player table on the existing world
            // by allocating missing players. Simpler: rebuild world players via new GameWorld
            // only when empty map; otherwise update economy in place when possible.
            if self.host_player_to_gw.is_empty() && self.host_to_entity.is_empty() {
                let cap = host_ids.len().max(8).min(255);
                self.world = GameWorld::new(cap);
            }
            self.host_player_to_gw.clear();
            // If world already has players from prior allocate, we still allocate on a fresh world
            // when entity map empty. When entities exist, update economy only for known mapping.
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
                        if let Some(p) = logic.get_player(*pid) {
                            if let Some(pd) = self.world.player_mut(gw) {
                                Self::copy_host_player_residual(pd, p, logic.frame);
                                pd.unlocked_sciences = Self::host_player_sciences(logic, *pid);
                            }
                        }
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
        } else {
            // Economy + science absolute refresh.
            for (hid, gw) in self.host_player_to_gw.clone() {
                if let Some(p) = logic.get_player(hid) {
                    if let Some(pd) = self.world.player_mut(gw) {
                        Self::copy_host_player_residual(pd, p, logic.frame);
                        pd.unlocked_sciences = Self::host_player_sciences(logic, hid);
                    }
                }
            }
        }
        // Always refresh science/power-bar residual for mapped players.
        for (hid, gw) in self.host_player_to_gw.clone() {
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

    /// Snapshot host `Player::getRelationship` facts used by GW AlliesOnly horde.
    pub(super) fn sync_horde_player_rel(&mut self, logic: &GameLogic) {
        self.horde_player_rel.clear();
        for (&id, p) in logic.get_players() {
            self.horde_player_rel.insert(
                id,
                HordePlayerRel {
                    alliance_team: p.alliance_team,
                    is_alive: p.is_alive,
                    map_relations: p.map_side.relations.clone(),
                },
            );
        }
    }

    /// C++ `Player::getRelationship` / `GameLogic::player_relationship_from_map`.
    /// Same controlling player is always ALLIES (even before the roster snapshot).
    fn horde_player_relationship(
        &self,
        source_player_id: u32,
        target_player_id: u32,
    ) -> gamelogic::common::Relationship {
        use gamelogic::common::Relationship;
        if source_player_id == target_player_id {
            return Relationship::Allies;
        }
        let Some(source) = self.horde_player_rel.get(&source_player_id) else {
            return Relationship::Neutral;
        };
        let Some(target) = self.horde_player_rel.get(&target_player_id) else {
            return Relationship::Neutral;
        };
        if !source.is_alive || !target.is_alive {
            return Relationship::Neutral;
        }
        if let Some(rel) = source.map_relations.get(&target_player_id) {
            return *rel;
        }
        if source.alliance_team >= 0 && source.alliance_team == target.alliance_team {
            Relationship::Allies
        } else if source.alliance_team >= 0 && target.alliance_team >= 0 {
            Relationship::Enemies
        } else {
            Relationship::Neutral
        }
    }

    /// C++ `PartitionFilterHordeMember::allow` AlliesOnly (`HordeUpdate.cpp:77-79`).
    pub(super) fn horde_allies_only(
        &self,
        a_owner: Option<u32>,
        a_team: u8,
        b_owner: Option<u32>,
        b_team: u8,
    ) -> bool {
        use gamelogic::common::Relationship;
        match (a_owner, b_owner) {
            (Some(a), Some(b)) => self.horde_player_relationship(a, b) == Relationship::Allies,
            (None, None) => a_team == b_team,
            _ => false,
        }
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
