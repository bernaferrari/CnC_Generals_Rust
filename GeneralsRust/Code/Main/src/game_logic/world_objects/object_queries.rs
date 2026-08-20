//! Host objects `impl GameLogic` — `object_queries`.
//! find_object, players, session queries. Child of `world_objects` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;

/// The structure-bound powers whose authority historically came from a
/// superweapon-looking template name.  They now require a parsed source
/// `SpecialPowerModule`; other legacy command paths retain their own staged
/// module ports.
fn is_structure_superweapon_power(power: &crate::command_system::SpecialPowerType) -> bool {
    use crate::command_system::SpecialPowerType as P;
    matches!(
        power,
        P::ParticleCannon
            | P::SuperweaponParticleCannon
            | P::LaserCannon
            | P::ScudStorm
            | P::NuclearMissile
            | P::NukeNeutronMissile
            | P::SuperweaponNeutronMissile
    )
}

impl GameLogic {
    /// Wave 958: legacy alias — prefer [`Self::host_object`].
    #[inline]
    pub fn find_object(&self, id: ObjectId) -> Option<&Object> {
        self.host_object(id)
    }

    /// Wave 958: legacy alias — prefer [`Self::host_object_mut`].
    #[inline]
    pub fn find_object_mut(&mut self, id: ObjectId) -> Option<&mut Object> {
        self.host_object_mut(id)
    }

    /// Find the nearest supply center (refinery/supply dropzone) for a team.

    pub(in super::super) fn find_nearest_harvestable_supply(
        &self,
        team: Team,
        from: Vec3,
    ) -> Option<ObjectId> {
        self.find_nearest_harvestable_supply_within(team, from, None)
    }

    /// C++ `ResourceGatheringManager::findBestSupplyWarehouse` scan cap.
    /// `max_scan` is already AI-doubled via `warehouse_scan_distance`.
    pub(in super::super) fn find_nearest_harvestable_supply_within(
        &self,
        team: Team,
        from: Vec3,
        max_scan: Option<f32>,
    ) -> Option<ObjectId> {
        let _ = team; // supplies are neutral/shared residual
                      // Pure residual acquire: nearest harvestable supply pile (3D distance).
        let candidates: Vec<_> = self
            .objects
            .iter()
            .filter_map(|(&id, obj)| {
                if !obj.is_alive() || obj.status.destroyed {
                    return None;
                }
                let harvestable = obj.is_kind_of(KindOf::SupplySource)
                    || obj.is_kind_of(KindOf::Harvestable)
                    || obj.is_kind_of(KindOf::Resource)
                    || obj.object_type == ObjectType::Supply;
                if !harvestable {
                    return None;
                }
                // Prefer piles that still have stored supplies when tracked.
                if obj.stored_resources.supplies == 0
                    && (obj.is_kind_of(KindOf::SupplySource)
                        || obj.is_kind_of(KindOf::Harvestable)
                        || obj.object_type == ObjectType::Supply)
                {
                    if obj.thing.template.dock_kind == DockKind::SupplyWarehouse {
                        return None;
                    }
                }
                Some(
                    crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                        id,
                        team: obj.team,
                        position: obj.get_position(),
                        is_alive: true,
                        is_neutral: obj.team == Team::Neutral,
                        under_construction: obj.status.under_construction,
                        combat_kind: true,
                        effectively_stealthed: false,
                        is_air: false,
                        eject_invulnerable: false,
                    },
                )
            })
            .collect();
        let scan = max_scan.filter(|d| *d > 0.0).unwrap_or(f32::MAX);
        crate::game_logic::host_residual_acquire::pick_nearest_residual_target(
            ObjectId(u32::MAX),
            Team::Neutral,
            from,
            candidates,
            |_| scan,
            |_| true,
        )
        .map(|(id, _, _)| id)
    }

    pub(in super::super) fn find_nearest_supply_center(
        &self,
        team: Team,
        owner_player_id: Option<u32>,
        from_position: Vec3,
    ) -> Option<ObjectId> {
        let manager_ids: Vec<ObjectId> = owner_player_id
            .and_then(|pid| self.players.get(&pid))
            .map(|player| player.resource_supply_centers.clone())
            .unwrap_or_default();
        let candidates: Vec<_> = self
            .objects
            .iter()
            .filter_map(|(&obj_id, obj)| {
                if obj.team != team
                    || !obj.is_alive()
                    || !obj.is_constructed()
                    || self.player_owner_for_host_object(obj) != owner_player_id
                {
                    return None;
                }
                let on_manager = manager_ids.contains(&obj_id);
                let kind_fallback = obj.thing.template.dock_kind
                    == crate::game_logic::DockKind::SupplyCenter
                    || obj.is_kind_of(KindOf::SupplyCenter)
                    || obj.thing.template.has_supply_center_create;
                if !manager_ids.is_empty() {
                    if !on_manager {
                        return None;
                    }
                } else if !kind_fallback {
                    return None;
                }
                Some(
                    crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                        id: obj_id,
                        team: obj.team,
                        position: obj.get_position(),
                        is_alive: true,
                        is_neutral: false,
                        under_construction: false,
                        combat_kind: true,
                        effectively_stealthed: false,
                        is_air: false,
                        eject_invulnerable: false,
                    },
                )
            })
            .collect();
        crate::game_logic::host_residual_acquire::pick_nearest_residual_target(
            ObjectId(u32::MAX),
            team,
            from_position,
            candidates,
            |_| f32::MAX,
            |_| true,
        )
        .map(|(id, _, _)| id)
    }

    /// C++ `SupplyTruckAIUpdate::m_preferredDock` overrides the ordinary
    /// ResourceManager center search after `AIPlayer::queueSupplyTruck` issues
    /// a `CMD_FROM_PLAYER` dock order.  Keep that preference only while it is
    /// still a live, constructed friendly SupplyCenter; a destroyed or
    /// repurposed dock falls back to the normal closest-center behavior.
    pub(in super::super) fn preferred_supply_center_or_nearest(
        &self,
        collector_id: ObjectId,
        team: Team,
        owner_player_id: Option<u32>,
        from_position: Vec3,
    ) -> Option<ObjectId> {
        let preferred = self
            .objects
            .get(&collector_id)
            .and_then(|collector| collector.preferred_dock_id);
        if let Some(center_id) = preferred {
            let valid_preferred_center = self.objects.get(&center_id).is_some_and(|center| {
                center.team == team
                    && self.player_owner_for_host_object(center) == owner_player_id
                    && center.is_alive()
                    && center.is_constructed()
                    && (center.thing.template.dock_kind
                        == crate::game_logic::DockKind::SupplyCenter
                        || center.is_kind_of(KindOf::SupplyCenter))
            });
            if valid_preferred_center {
                return Some(center_id);
            }
        }

        self.find_nearest_supply_center(team, owner_player_id, from_position)
    }

    /// Wave 958: legacy alias — prefer [`Self::host_objects`].
    #[inline]
    pub fn get_objects(&self) -> &HashMap<ObjectId, Object> {
        self.host_objects()
    }

    /// Partition-backed candidate ids near a world position (empty if partition cold).
    /// Callers must still apply team/alive/stealth filters — this is broadphase only.
    #[inline]
    pub fn object_ids_near(&self, position: glam::Vec3, radius: f32) -> Vec<ObjectId> {
        self.partition_manager
            .ids_in_radius(position.x, position.z, radius)
            .into_iter()
            .map(ObjectId)
            .collect()
    }

    /// Wave 958: legacy alias — prefer [`Self::host_objects_mut`].
    #[inline]
    pub fn get_objects_mut(&mut self) -> &mut HashMap<ObjectId, Object> {
        self.host_objects_mut()
    }

    /// Get all players (for snapshot/save system)
    pub fn get_players(&self) -> &HashMap<u32, Player> {
        &self.players
    }

    /// Get mutable players (for snapshot restoration)
    pub fn get_players_mut(&mut self) -> &mut HashMap<u32, Player> {
        &mut self.players
    }

    /// Transfer an object to an exact active player. The player's faction is
    /// copied only for template/art identity; command authority follows the
    /// persistent `owner_player_id`.
    pub fn transfer_object_to_player(&mut self, object_id: ObjectId, player_id: u32) -> bool {
        let Some(team) = self
            .players
            .get(&player_id)
            .filter(|player| player.is_alive && player.team != Team::Neutral)
            .map(|player| player.team)
        else {
            return false;
        };

        // C++ Object::setTeam changes membership before it dispatches
        // BehaviorModule::onCapture.  Base power follows that membership, but
        // `OverchargeBehavior::onCapture` deliberately leaves its
        // ThingTemplate EnergyBonus with the old Energy pool when the active
        // plant is disabled.  Capture the typed module state before changing
        // the owner so the ownership-derived power scan can retain that one
        // fire-and-forget delta afterwards.  This is not a name/KindOf
        // fallback: only the parsed behavior authorizes it.
        let (old_owner_player_id, retained_overcharge_bonus) = self
            .objects
            .get(&object_id)
            .map(|object| {
                let old_owner_player_id = self.player_owner_for_host_object(object);
                let retained_overcharge_bonus = (old_owner_player_id != Some(player_id)
                    && object.is_alive()
                    && object.is_constructed()
                    && object.overcharge_enabled
                    && object.is_disabled()
                    && object.thing.template.supports_overcharge())
                .then(|| object.thing.template.energy_bonus.unwrap_or(0));
                (old_owner_player_id, retained_overcharge_bonus)
            })
            .unwrap_or((None, None));

        let Some(object) = self.objects.get_mut(&object_id) else {
            return false;
        };
        object.set_team_and_owner(team, Some(player_id));

        if let Some(bonus) = retained_overcharge_bonus {
            if let Some(old_player_id) = old_owner_player_id {
                if let Some(old_player) = self.players.get_mut(&old_player_id) {
                    old_player.captured_overcharge_power_delta = old_player
                        .captured_overcharge_power_delta
                        .saturating_add(bonus);
                }
            }
            if let Some(new_player) = self.players.get_mut(&player_id) {
                new_player.captured_overcharge_power_delta = new_player
                    .captured_overcharge_power_delta
                    .saturating_sub(bonus);
            }
        }
        true
    }

    /// C++ `Player::getRelationship` foundation for host objects. Faction is
    /// intentionally absent from this calculation: two USA slots can be
    /// enemies, while differently skinned allies can share an alliance team.
    pub fn player_relationship(
        &self,
        source_player_id: u32,
        target_player_id: u32,
    ) -> gamelogic::common::Relationship {
        use gamelogic::common::Relationship;

        let Some(source) = self.players.get(&source_player_id) else {
            return Relationship::Neutral;
        };
        let Some(target) = self.players.get(&target_player_id) else {
            return Relationship::Neutral;
        };
        if source_player_id == target_player_id {
            return Relationship::Allies;
        }
        // Inactive/map-placeholder slots are not combat enemies.
        if !source.is_alive || !target.is_alive {
            return Relationship::Neutral;
        }
        if source.alliance_team >= 0 && source.alliance_team == target.alliance_team {
            Relationship::Allies
        } else {
            Relationship::Enemies
        }
    }

    /// Relationship inferred from persistent object ownership. Any unowned
    /// object is neutral rather than being assigned to a player by faction.
    pub fn object_relationship(
        &self,
        source: &Object,
        target: &Object,
    ) -> gamelogic::common::Relationship {
        use gamelogic::common::Relationship;

        match (source.owner_player_id, target.owner_player_id) {
            (Some(source_player_id), Some(target_player_id)) => {
                self.player_relationship(source_player_id, target_player_id)
            }
            _ => Relationship::Neutral,
        }
    }

    /// C++ `ActionManager::canGetRepairedAt` / `canGetHealedAt` service
    /// relationship gate.  A repair bay or heal pad is usable only by
    /// `ALLIES`; faction equality is not ownership when two human slots chose
    /// the same faction.
    ///
    /// Live object ownership is authoritative and must resolve to an active
    /// player with the object's recorded faction.  The narrow compatibility
    /// fallback exists only for a wholly ownerless legacy/synthetic world,
    /// only for one unambiguous non-neutral faction (or a world with no
    /// player roster at all).  A mixed or ambiguous record is deliberately
    /// not upgraded into an alliance by its `Team` value.
    #[inline]
    pub fn service_relationship_is_allies(&self, source_id: ObjectId, target_id: ObjectId) -> bool {
        use gamelogic::common::Relationship;

        if source_id == target_id {
            return false;
        }
        let Some(source) = self.host_object(source_id) else {
            return false;
        };
        let Some(target) = self.host_object(target_id) else {
            return false;
        };

        match (source.owner_player_id, target.owner_player_id) {
            (Some(source_player_id), Some(target_player_id))
                if self.player_owner_for_host_object(source) == Some(source_player_id)
                    && self.player_owner_for_host_object(target) == Some(target_player_id) =>
            {
                self.player_relationship(source_player_id, target_player_id) == Relationship::Allies
            }
            (None, None) if self.uses_legacy_team_ownership_fallback() => {
                source.team != Team::Neutral
                    && source.team == target.team
                    && (self.players.is_empty()
                        || self.unique_player_id_for_team(source.team).is_some())
            }
            _ => false,
        }
    }

    /// C++ `ActionManager::canRepairObject` relationship gate.  Manual dozer
    /// repair is wider than service-dock use: it rejects enemies, while an
    /// ownerless neutral civilian structure remains a legal repair target.
    /// Exact player provenance still takes precedence over faction identity.
    #[inline]
    pub fn repair_relationship_is_not_enemy(
        &self,
        source_id: ObjectId,
        target_id: ObjectId,
    ) -> bool {
        use gamelogic::common::Relationship;

        if source_id == target_id {
            return false;
        }
        let Some(source) = self.host_object(source_id) else {
            return false;
        };
        let Some(target) = self.host_object(target_id) else {
            return false;
        };

        match (source.owner_player_id, target.owner_player_id) {
            (Some(source_player_id), Some(target_player_id))
                if self.player_owner_for_host_object(source) == Some(source_player_id)
                    && self.player_owner_for_host_object(target) == Some(target_player_id) =>
            {
                self.player_relationship(source_player_id, target_player_id)
                    != Relationship::Enemies
            }
            // C++ explicitly allows a dozer to repair neutral civilian
            // buildings. A real player-owned source facing an ownerless
            // neutral map target is therefore a proven neutral relationship,
            // not an ambiguous faction fallback.
            (Some(source_player_id), None)
                if self.player_owner_for_host_object(source) == Some(source_player_id)
                    && target.team == Team::Neutral =>
            {
                true
            }
            (None, None) if self.uses_legacy_team_ownership_fallback() => {
                target.team == Team::Neutral
                    || (source.team != Team::Neutral
                        && source.team == target.team
                        && (self.players.is_empty()
                            || self.unique_player_id_for_team(source.team).is_some()))
            }
            _ => false,
        }
    }

    /// Resolve the normal-Enter controlling-player check without ever treating
    /// a faction as a player.  Explicit owners must match exactly.  The old
    /// team fallback is retained only when *both* objects are genuinely
    /// ownerless and that faction identifies one live player; a half-proven
    /// or ambiguous same-faction pair fails closed.
    #[inline]
    pub fn normal_enter_controller_matches(&self, source: &Object, target: &Object) -> bool {
        match (source.owner_player_id, target.owner_player_id) {
            (Some(source_player_id), Some(target_player_id)) => {
                self.player_owner_for_host_object(source) == Some(source_player_id)
                    && self.player_owner_for_host_object(target) == Some(target_player_id)
                    && source_player_id == target_player_id
            }
            (None, None) if source.team == target.team => {
                self.unique_player_id_for_team(source.team).is_some()
            }
            _ => false,
        }
    }

    /// C++ `VeterancyCrateCollide::isValidToExecute` pilot ownership gate.
    ///
    /// A pilot is a crate-like collide source, not an allied transport rider:
    /// the destination must have the **same controlling player**, not merely
    /// an Allied relationship.  `DISABLED_UNMANNED` commonly neutralizes a
    /// vehicle, so `apply_kill_pilot_unmanned` snapshots its exact owner ID
    /// before the visible team transfer.  A stale ID, a mixed owner-aware
    /// world, or a generic neutral target all fail closed.
    #[inline]
    pub fn pilot_recrew_controller_matches(
        &self,
        source_id: ObjectId,
        target_id: ObjectId,
    ) -> bool {
        if source_id == target_id {
            return false;
        }
        let Some(source) = self.host_object(source_id) else {
            return false;
        };
        let Some(target) = self.host_object(target_id) else {
            return false;
        };

        let source_owner = self.player_owner_for_host_object(source);
        let target_owner = match target.owner_player_id {
            Some(_) => self.player_owner_for_host_object(target),
            None if target.is_unmanned() => {
                target.status.unmanned_owner_player_id.filter(|player_id| {
                    target
                        .status
                        .unmanned_owner_team
                        .and_then(|owner_team| {
                            self.players
                                .get(player_id)
                                .map(|player| (player, owner_team))
                        })
                        .is_some_and(|(player, owner_team)| {
                            player.is_alive && player.team == owner_team
                        })
                })
            }
            None => None,
        };

        if let (Some(source_owner), Some(target_owner)) = (source_owner, target_owner) {
            return source_owner == target_owner;
        }

        // Compatibility only for an entirely ownerless legacy fixture or
        // snapshot.  A neutral target needs a pre-neutralization team record;
        // neutral itself must never become an implicit ally.
        source.owner_player_id.is_none()
            && target.owner_player_id.is_none()
            && target.status.unmanned_owner_player_id.is_none()
            && self.uses_legacy_team_ownership_fallback()
            && source.team != Team::Neutral
            && (target.team == source.team
                || (target.is_unmanned()
                    && target.team == Team::Neutral
                    && target.status.unmanned_owner_team == Some(source.team)))
            && (self.players.is_empty() || self.unique_player_id_for_team(source.team).is_some())
    }

    /// One live authority predicate for C++ `VeterancyCrateCollide IsPilot`
    /// re-crew execution.  It is deliberately narrower than the generic
    /// crate/experience system: only the retail `RequiredKindOf = VEHICLE`,
    /// `ForbiddenKindOf = DOZER`, `EffectRange = 0`, and
    /// `AddsOwnerVeterancy = Yes` pilot record is currently representable.
    ///
    /// The executor uses this before it queues Enter, and the arrival/physics
    /// paths repeat it immediately before mutating the target.  This prevents
    /// a frozen RMB decision, an owner change, or an airborne transition from
    /// becoming a permanent authorization.
    #[inline]
    pub fn can_execute_pilot_recrew(&self, pilot_id: ObjectId, vehicle_id: ObjectId) -> bool {
        use crate::game_logic::host_usa_pilot::{
            is_recrewable_unmanned_vehicle, is_significantly_above_terrain, pilot_levels_to_gain,
            vehicle_can_gain_exp_for_levels,
        };

        if pilot_id == vehicle_id {
            return false;
        }
        let Some(pilot) = self.host_object(pilot_id) else {
            return false;
        };
        let Some(vehicle) = self.host_object(vehicle_id) else {
            return false;
        };
        if !pilot.is_alive()
            || !pilot
                .thing
                .template
                .veterancy_crate_collide
                .as_ref()
                .is_some_and(|metadata| metadata.supports_pilot_recrew())
            || !self.pilot_recrew_controller_matches(pilot_id, vehicle_id)
        {
            return false;
        }

        // `RequiredKindOf`/`ForbiddenKindOf` are exact authored KindOf masks;
        // do not fall back to ObjectType, Worker, or a template basename.
        let is_airborne_locomotor =
            vehicle.is_kind_of(KindOf::Aircraft) || vehicle.status.airborne_target;
        let terrain_y = self
            .terrain_height_at(vehicle.get_position())
            .unwrap_or(0.0);
        let significantly_above_terrain =
            is_significantly_above_terrain(vehicle.get_position().y - terrain_y);
        let recrewable = is_recrewable_unmanned_vehicle(
            vehicle.is_alive(),
            vehicle.is_kind_of(KindOf::Vehicle),
            is_airborne_locomotor,
            vehicle.is_unmanned(),
            vehicle.status.under_construction,
            vehicle.is_kind_of(KindOf::Dozer),
        );
        recrewable
            && !significantly_above_terrain
            // The compact host has no separate parsed IsTrainable record;
            // its existing physical vehicle path is the bounded stand-in.
            && vehicle_can_gain_exp_for_levels(
                vehicle.experience.level,
                pilot_levels_to_gain(pilot.experience.level),
            )
    }

    /// C++ `OpenContain` relationship for normal Enter.  Ownership provenance
    /// is authoritative.  Team behavior survives solely for an unambiguous
    /// pair of ownerless legacy objects.  A one-sided/stale owner stays
    /// Neutral instead of inheriting a same-faction relationship.
    #[inline]
    pub fn normal_enter_relationship(
        &self,
        source: &Object,
        target: &Object,
    ) -> gamelogic::common::Relationship {
        use gamelogic::common::Relationship;

        match (source.owner_player_id, target.owner_player_id) {
            (Some(source_player_id), Some(target_player_id))
                if self.player_owner_for_host_object(source) == Some(source_player_id)
                    && self.player_owner_for_host_object(target) == Some(target_player_id) =>
            {
                self.player_relationship(source_player_id, target_player_id)
            }
            (None, None)
                if self.unique_player_id_for_team(source.team).is_some()
                    && self.unique_player_id_for_team(target.team).is_some() =>
            {
                if source.team == target.team {
                    Relationship::Allies
                } else if source.team == Team::Neutral || target.team == Team::Neutral {
                    Relationship::Neutral
                } else {
                    Relationship::Enemies
                }
            }
            _ => Relationship::Neutral,
        }
    }

    /// Remaining normal-Enter capacity before adding `unit_id`.
    ///
    /// `TransportContain`/`RailedTransportContain` consume each rider's
    /// authored `TransportSlotCount`; garrisons and the shared tunnel network
    /// intentionally remain body-count containers.  A missing contained
    /// object or missing slot metadata is not silently treated as one slot.
    pub fn normal_enter_available_capacity_for(
        &self,
        unit_id: ObjectId,
        target_id: ObjectId,
    ) -> Option<usize> {
        if unit_id == target_id {
            return None;
        }
        let unit = self.host_object(unit_id)?;
        let target = self.host_object(target_id)?;
        if !target.can_contain() || !target.supports_normal_enter() {
            return None;
        }

        if target.contained_units().contains(&unit_id) {
            // Retain the historical idempotent Enter behavior.  The caller
            // will not try to add the rider a second time.
            return Some(usize::MAX);
        }

        // RiderChangeContain deliberately does not price a replacement
        // against its occupied TransportContain slot.  C++ validates the
        // authored RiderN equivalence with CHECK_CAPACITY=false, ejects the
        // old rider, then boards the new one in one transaction.
        if target.supports_authored_rider_change_normal_enter() {
            return target
                .authored_rider_change_rider_for_template(&unit.template_name)
                .map(|_| usize::MAX)
                .or(Some(0));
        }

        if target.is_tunnel_network_style_container() {
            let team = unit.team;
            return Some(
                if self.tunnel_network.is_in_network(team, unit_id)
                    || self.tunnel_network.has_capacity(team)
                {
                    1
                } else {
                    0
                },
            );
        }

        if target.normal_enter_uses_transport_slots() {
            let capacity = target.transport_capacity();
            if capacity == 0 {
                return None;
            }
            let mut slots_in_use = 0usize;
            for occupant_id in target.contained_units() {
                let occupant = self.host_object(occupant_id)?;
                let slots = occupant.transport_slot_count();
                if slots == 0 {
                    return None;
                }
                slots_in_use = slots_in_use.checked_add(slots)?;
            }
            return capacity.checked_sub(slots_in_use);
        }

        // GarrisonContain (including Overlord's retained bunker role) uses
        // body count, not passenger slot cost.
        let capacity = target
            .garrison_capacity()
            .max(target.overlord_bunker_slot_capacity());
        (capacity > 0)
            .then(|| capacity.checked_sub(target.contained_units().len()))
            .flatten()
    }

    /// Authoritative C++ `ActionManager::canEnterObject(..., CHECK_CAPACITY)`
    /// subset used by normal player Enter.  Pilot recrew is intentionally
    /// handled by the specialized executor path before this generic container
    /// check.  Dock uses a different command path entirely.
    pub fn can_unit_enter_normal_target(&self, unit_id: ObjectId, target_id: ObjectId) -> bool {
        if unit_id == target_id {
            return false;
        }
        let Some(unit) = self.host_object(unit_id) else {
            return false;
        };
        let Some(target) = self.host_object(target_id) else {
            return false;
        };

        if !unit.is_alive()
            || !target.is_alive()
            || unit.status.under_construction
            || target.status.under_construction
            || unit.status.sold
            || target.status.sold
            || target.is_subdued_disabled()
            || unit.is_kind_of(KindOf::IgnoredInGui)
            || target.is_kind_of(KindOf::IgnoredInGui)
            || unit.is_kind_of(KindOf::Structure)
            || unit.is_kind_of(KindOf::Immobile)
            || unit.transport_slot_count() == 0
            || !target.can_contain()
            || !target.supports_normal_enter()
        {
            return false;
        }

        let unit_in_tunnel = self.tunnel_network.team_holding_unit(unit_id).is_some();
        if !unit.can_move() && !unit_in_tunnel {
            return false;
        }

        let relationship = self.normal_enter_relationship(unit, target);
        if !target.allows_normal_enter_for_relationship(relationship) {
            return false;
        }
        let same_controller = self.normal_enter_controller_matches(unit, target);
        if target.normal_enter_requires_exact_controller() && !same_controller {
            return false;
        }

        if target.is_tunnel_network_style_container() {
            if unit.is_kind_of(KindOf::Aircraft)
                || (target.team != unit.team && target.team != Team::Neutral)
            {
                return false;
            }
        } else {
            match target.normal_enter_admission() {
                crate::game_logic::ContainAdmission::AnyMobile => {}
                crate::game_logic::ContainAdmission::InfantryOnly => {
                    if !unit.is_kind_of(KindOf::Infantry) && !unit.is_hero() {
                        return false;
                    }
                }
                crate::game_logic::ContainAdmission::InfantryOrVehicle => {
                    if unit.is_kind_of(KindOf::Aircraft) {
                        return false;
                    }
                }
                crate::game_logic::ContainAdmission::MoneyHackerOnly => {
                    if !unit.is_kind_of(KindOf::MoneyHacker) {
                        return false;
                    }
                }
                crate::game_logic::ContainAdmission::Unsupported => return false,
            }

            // C++ lets a non-owner Enter an empty civilian/non-faction
            // garrison, but not an occupied target or a faction structure.
            // Use exact controllers here: same-faction skirmish slots are not
            // implicitly the same owner.
            if !same_controller
                && (target.is_faction_structure() || !target.contained_units().is_empty())
            {
                return false;
            }

            // Repeat the exact parsed roster lookup in authority even when
            // presentation already filtered it.  Do not turn a Combat Cycle
            // template-name residual into a rider admission heuristic.
            if target.supports_authored_rider_change_normal_enter()
                && target
                    .authored_rider_change_rider_for_template(&unit.template_name)
                    .is_none()
            {
                return false;
            }
        }

        let Some(available) = self.normal_enter_available_capacity_for(unit_id, target_id) else {
            return false;
        };
        if available == usize::MAX {
            return true;
        }
        if target.normal_enter_uses_transport_slots() {
            available >= unit.transport_slot_count()
        } else {
            available > 0
        }
    }

    /// Whether both objects have exact player provenance. Only this condition
    /// may override a legacy faction relationship: a player-owned object facing
    /// a neutral/map object must retain its existing neutral handling.
    #[inline]
    pub fn has_object_ownership_provenance(&self, source: &Object, target: &Object) -> bool {
        source.owner_player_id.is_some() && target.owner_player_id.is_some()
    }

    /// Whether this host world is an old, wholly ownerless snapshot.  Faction
    /// may stand in for a controlling player only in this compatibility case;
    /// a mixed owner-aware world must never turn two same-faction slots into
    /// the same controller.
    #[inline]
    pub fn uses_legacy_team_ownership_fallback(&self) -> bool {
        self.objects
            .values()
            .all(|object| object.owner_player_id.is_none())
    }

    /// Get current frame number
    pub fn get_current_frame(&self) -> u64 {
        self.frame as u64
    }

    /// Set current frame number (for snapshot restoration)
    pub fn set_current_frame(&mut self, frame: u64) {
        self.frame = frame as u32;
    }

    /// Clear all objects (for snapshot restoration)
    pub fn clear_all_objects(&mut self) {
        self.objects.clear();
        self.host_view_dirty.clear();
        self.next_object_id = ObjectId(1);
        self.next_formation_id = 1;
    }

    /// Set the next object ID counter (for snapshot restoration).
    pub fn set_next_object_id_for_restore(&mut self, next_object_id: ObjectId) {
        self.next_object_id = next_object_id;
    }

    /// C++ TheAI::getNextFormationID residual.
    pub fn alloc_formation_id(&mut self) -> u32 {
        let id = self.next_formation_id;
        self.next_formation_id = self.next_formation_id.saturating_add(1).max(1);
        id
    }

    /// Clear all players (for snapshot restoration)
    pub fn clear_all_players(&mut self) {
        self.players.clear();
        self.player_template_bindings.clear();
    }

    /// Add a player directly (for snapshot restoration)
    pub fn add_player(&mut self, player: Player) {
        self.player_template_bindings.remove(&player.id);
        self.players.insert(player.id, player);
    }

    pub fn command_center_position(&self, team: Team) -> Option<Vec3> {
        let mut fallback = None;
        let mut highest_cost = i32::MIN;

        for obj in self.objects.values() {
            if obj.team != team || !obj.is_alive() {
                continue;
            }

            if obj.is_kind_of(KindOf::CommandCenter) {
                return Some(obj.get_position());
            }

            if obj.is_kind_of(KindOf::Structure) {
                let cost = obj.thing.template.build_cost.supplies as i32;
                if cost > highest_cost {
                    highest_cost = cost;
                    fallback = Some(obj.get_position());
                }
            }
        }

        fallback
    }

    /// Get player by ID
    pub fn get_player(&self, player_id: u32) -> Option<&Player> {
        self.players.get(&player_id)
    }

    /// Exact C++ PlayerTemplate selection retained for this host session.
    pub fn player_template_identity(&self, player_id: u32) -> Option<&PlayerTemplateIdentity> {
        self.player_template_bindings.get(&player_id)
    }

    /// Snapshot-only copy of exact offline identities.  The map remains
    /// session-private so ordinary gameplay cannot replace a selected General
    /// without going through the validated bind path.
    pub(crate) fn player_template_identities_for_snapshot(
        &self,
    ) -> Vec<(u32, PlayerTemplateIdentity)> {
        self.player_template_bindings
            .iter()
            .map(|(&player_id, identity)| (player_id, identity.clone()))
            .collect()
    }

    /// Install a snapshot-owned exact identity after `restore_all_players`.
    ///
    /// This deliberately does *not* call `bind_player_template_identity`:
    /// `PlayerSnapshot` has already restored the saved resources, sciences,
    /// and GameInfo overrides, and re-applying `Player::init(pt)` would
    /// overwrite them.  The indexed lookup makes a stale store ordering or
    /// name fail closed, and prevents a saved Random slot from being resolved
    /// a second time.
    pub(crate) fn install_restored_player_template_identity(
        &mut self,
        player_id: u32,
        player_template: PlayerTemplateIdentity,
    ) -> bool {
        let Some(template) = player_template.resolve() else {
            return false;
        };
        let Some(template_team) = PlayerTemplateIdentity::team_for_template(&template) else {
            return false;
        };
        let Some(player) = self.players.get(&player_id) else {
            return false;
        };
        if player.team != template_team {
            return false;
        }

        self.player_template_bindings
            .insert(player_id, player_template);
        true
    }

    /// Resolve the immutable Common PlayerTemplate for a concrete host player.
    /// Callers must use this before Team residual tables so a selected General
    /// never silently degrades to a base faction.
    pub(crate) fn resolved_player_template(
        &self,
        player_id: u32,
    ) -> Option<game_engine::common::rts::player_template::PlayerTemplate> {
        self.player_template_identity(player_id)
            .and_then(PlayerTemplateIdentity::resolve)
    }

    /// Exact `PlayerTemplate::ProductionCostChange` factor for one player and
    /// ThingTemplate.  A host player without a bound Campaign/Challenge
    /// template retains the ordinary `1.0` factor; a bound General never falls
    /// back to a base-side table.
    pub(crate) fn player_template_production_cost_factor(
        &self,
        player_id: u32,
        build_template_name: &str,
    ) -> f32 {
        self.resolved_player_template(player_id)
            .map(|template| {
                PlayerTemplateIdentity::production_cost_factor_for_template(
                    &template,
                    build_template_name,
                )
            })
            .unwrap_or(1.0)
    }

    /// Exact `PlayerTemplate::ProductionTimeChange` factor for one player and
    /// ThingTemplate.  Low-power timing remains a separate production/construction
    /// concern, as it is in C++ `ThingTemplate::calcTimeToBuild`.
    pub(crate) fn player_template_production_time_factor(
        &self,
        player_id: u32,
        build_template_name: &str,
    ) -> f32 {
        self.resolved_player_template(player_id)
            .map(|template| {
                PlayerTemplateIdentity::production_time_factor_for_template(
                    &template,
                    build_template_name,
                )
            })
            .unwrap_or(1.0)
    }

    /// C++ Object construction sets the controller's exact
    /// `ProductionVeterancyLevel` for every spawned ThingTemplate.  `None`
    /// means this player is not backed by a selected PlayerTemplate; a bound
    /// template with no entry explicitly maps to Main's Rookie default.
    pub(crate) fn player_template_production_veterancy(
        &self,
        player_id: u32,
        build_template_name: &str,
    ) -> Option<VeterancyLevel> {
        self.resolved_player_template(player_id).map(|template| {
            PlayerTemplateIdentity::production_veterancy_for_template(
                &template,
                build_template_name,
            )
        })
    }

    /// Wave 238: economy probe without exposing `&Player` to engine dual-read paths.
    #[inline]
    pub fn player_economy(&self, id: u32) -> Option<(u32, i32, i32, i32)> {
        self.players.get(&id).map(|p| {
            (
                p.effective_supplies(),
                p.power_available,
                p.power_produced,
                p.power_consumed,
            )
        })
    }

    /// Wave 238: unlocked sciences without exposing `&Player`.
    #[inline]
    pub fn player_unlocked_sciences(&self, id: u32) -> Vec<String> {
        self.players
            .get(&id)
            .map(|p| p.unlocked_sciences.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Wave 238: science purchase points without exposing `&Player`.
    #[inline]
    pub fn player_science_purchase_points(&self, id: u32) -> i32 {
        self.players
            .get(&id)
            .map(|p| p.science_purchase_points)
            .unwrap_or(0)
    }

    /// Wave 238: science purchase capability without exposing `&Player`.
    #[inline]
    pub fn player_can_purchase_science(&self, id: u32, science_name: &str) -> bool {
        self.players
            .get(&id)
            .map(|p| p.is_capable_of_purchasing_science(science_name))
            .unwrap_or(false)
    }

    /// Wave 239: team probe without exposing `&Player`.
    #[inline]
    pub fn player_team(&self, id: u32) -> Option<Team> {
        self.players.get(&id).map(|p| p.team)
    }

    /// Wave 239: command-center world pose for a player's team (camera boot residual).
    #[inline]
    pub fn player_command_center_position(&self, id: u32) -> Option<glam::Vec3> {
        let team = self.player_team(id)?;
        self.command_center_position(team)
    }

    /// Wave 240: existence probe without exposing `&Player`.
    #[inline]
    pub fn player_exists(&self, id: u32) -> bool {
        self.players.contains_key(&id)
    }

    /// Wave 240: lowest player id (boot local residual).
    #[inline]
    pub fn min_player_id(&self) -> Option<u32> {
        self.players.keys().copied().min()
    }

    /// Wave 240: display name without exposing `&Player`.
    #[inline]
    pub fn player_name(&self, id: u32) -> Option<String> {
        self.players.get(&id).map(|p| p.name.clone())
    }

    /// Wave 240: alive flag without exposing `&Player`.
    #[inline]
    pub fn player_is_alive(&self, id: u32) -> bool {
        self.players.get(&id).map(|p| p.is_alive).unwrap_or(false)
    }

    /// Wave 240: local flag without exposing `&Player`.
    #[inline]
    pub fn player_is_local(&self, id: u32) -> bool {
        self.players.get(&id).map(|p| p.is_local).unwrap_or(false)
    }

    /// Wave 240: UI color without exposing `&Player`.
    #[inline]
    pub fn player_color_rgb(&self, id: u32) -> Option<(u8, u8, u8)> {
        self.players.get(&id).map(|p| p.color_rgb)
    }

    /// Wave 240: selected object ids without exposing `&Player`.
    #[inline]
    pub fn player_selected_objects(&self, id: u32) -> Vec<ObjectId> {
        self.players
            .get(&id)
            .map(|p| p.selected_objects.clone())
            .unwrap_or_default()
    }

    /// Wave 240: ordered player id roster without exposing `&Player`.
    #[inline]
    pub fn player_ids(&self) -> Vec<u32> {
        let mut ids: Vec<u32> = self.players.keys().copied().collect();
        ids.sort_unstable();
        ids
    }

    /// Wave 240: raise supplies floor without exposing `&mut Player`.
    #[inline]
    pub fn ensure_player_min_supplies(&mut self, id: u32, min_supplies: u32) {
        if let Some(p) = self.players.get_mut(&id) {
            p.resources.supplies = p.resources.supplies.max(min_supplies);
        }
    }

    /// Wave 242: extend selection without exposing `&mut Player`.
    #[inline]
    pub fn player_extend_selection(&mut self, id: u32, units: &[ObjectId]) {
        let Some(p) = self.players.get_mut(&id) else {
            return;
        };
        for unit in units {
            if !p.selected_objects.contains(unit) {
                p.selected_objects.push(*unit);
            }
        }
    }

    /// Wave 242: selection count without exposing `&Player`.
    #[inline]
    pub fn player_selected_count(&self, id: u32) -> usize {
        self.players
            .get(&id)
            .map(|p| p.selected_objects.len())
            .unwrap_or(0)
    }

    /// Wave 243: spend build cost without exposing `&mut Player`.
    #[inline]
    pub fn try_spend_player_resources(&mut self, id: u32, cost: &Resources) -> bool {
        let Some(p) = self.players.get_mut(&id) else {
            return false;
        };
        p.spend_resources(cost)
    }

    /// Wave 243: refund supplies without exposing `&mut Player`.
    #[inline]
    pub fn player_refund_supplies(&mut self, id: u32, supplies: u32) {
        if let Some(p) = self.players.get_mut(&id) {
            p.resources.supplies = p.resources.supplies.saturating_add(supplies);
        }
    }

    /// Wave 243: constructor team probe without exposing `&Object`.
    #[inline]
    pub fn unit_team_if_can_construct(&self, id: ObjectId) -> Option<Team> {
        let obj = self.objects.get(&id)?;
        if obj.can_construct() {
            Some(obj.team)
        } else {
            None
        }
    }

    /// Exact constructor provenance for player-issued construction. Returning
    /// both values preserves the faction data the template path needs while
    /// keeping billing and ownership with the actual player slot.
    #[inline]
    pub fn unit_owner_if_can_construct(&self, id: ObjectId) -> Option<(u32, Team)> {
        let obj = self.objects.get(&id)?;
        let player_id = obj.owner_player_id?;
        if !obj.can_construct() {
            return None;
        }
        self.players
            .get(&player_id)
            .filter(|player| player.is_alive && player.team == obj.team)
            .map(|player| (player.id, player.team))
    }

    /// Get mutable player by ID
    pub fn get_player_mut(&mut self, player_id: u32) -> Option<&mut Player> {
        self.players.get_mut(&player_id)
    }

    pub fn get_player_mut_by_team(&mut self, team: Team) -> Option<&mut Player> {
        let key = self
            .players
            .iter()
            .find_map(|(id, p)| if p.team == team { Some(*id) } else { None })?;
        self.players.get_mut(&key)
    }

    pub fn get_player_by_team(&self, team: Team) -> Option<&Player> {
        self.players.values().find(|p| p.team == team)
    }

    /// Combined object + SharedSyncedTimer + RequiredScience residual ready gate.
    ///
    /// C++ order residual: object alive → science (`canUseSpecialPower`) →
    /// sharedNSync / per-object cooldown.
    pub fn is_special_power_ready_for(
        &self,
        object_id: ObjectId,
        power: &crate::command_system::SpecialPowerType,
    ) -> bool {
        let Some(obj) = self.host_object(object_id) else {
            return false;
        };
        if !obj.is_alive() {
            return false;
        }
        // C++ SpecialPowerModule::doSpecialPower / isReady: disabled objects cannot fire.
        // Covers underpowered POWERED SWs (PUC/Nuke), EMP, hacked, unmanned, etc.
        if obj.is_disabled() {
            return false;
        }
        let parsed_module = obj.thing.template.special_power_module_for_command(power);
        if is_structure_superweapon_power(power) && parsed_module.is_none() {
            return false;
        }
        // A parsed module's loaded SpecialPowerTemplate owns science and
        // SharedSyncedTimer policy.  Fall back only for command families not
        // yet represented by a source module record.
        let required_science = parsed_module
            .and_then(|module| module.required_science.as_deref())
            .or_else(|| {
                (parsed_module.is_none()).then(|| {
                    crate::game_logic::host_special_power_enum_residual::special_power_required_science(
                        power,
                    )
                })
                .flatten()
            });
        if let Some(required) = required_science {
            match self.get_player_by_team(obj.team) {
                Some(player) if player.has_unlocked_science(required) => {}
                Some(_) => return false,
                // Fail-closed: science-gated powers need a controlling player residual.
                None => return false,
            }
        }
        let shared_n_sync = parsed_module
            .map(|module| module.shared_n_sync)
            .unwrap_or_else(|| {
                crate::game_logic::host_special_power_enum_residual::special_power_uses_shared_synced_timer(
                    power,
                )
            });
        if shared_n_sync {
            // C++ getReadyFrame via Player::getOrStartSpecialPowerReadyFrame.
            if let Some(player) = self.get_player_by_team(obj.team) {
                if !player.is_shared_special_power_ready(power) {
                    return false;
                }
            }
            return true;
        }
        obj.is_special_power_ready(power)
    }

    /// Consume charge with SharedSyncedTimer residual when applicable.
    pub fn consume_special_power_charge_for(
        &mut self,
        object_id: ObjectId,
        power: &crate::command_system::SpecialPowerType,
    ) -> bool {
        if !self.is_special_power_ready_for(object_id, power) {
            return false;
        }
        let (team, parsed_module) = match self.host_object(object_id) {
            Some(o) => (
                o.team,
                o.thing
                    .template
                    .special_power_module_for_command(power)
                    .cloned(),
            ),
            None => return false,
        };
        if is_structure_superweapon_power(power) && parsed_module.is_none() {
            return false;
        }
        let reload = parsed_module
            .as_ref()
            .map(|module| module.reload_time_frames as f32 / 30.0)
            .unwrap_or_else(|| {
                crate::game_logic::host_special_power_enum_residual::special_power_reload_seconds(
                    power,
                )
                .unwrap_or_else(|| {
                    self.host_object(object_id)
                        .map(|o| o.special_power_cooldown)
                        .unwrap_or(10.0)
                })
            });

        let shared_n_sync = parsed_module
            .as_ref()
            .map(|module| module.shared_n_sync)
            .unwrap_or_else(|| {
                crate::game_logic::host_special_power_enum_residual::special_power_uses_shared_synced_timer(
                    power,
                )
            });
        if shared_n_sync {
            if let Some(player) = self.get_player_mut_by_team(team) {
                player.reset_shared_special_power_timer(power, reload);
            }
            // Mirror onto all living same-team objects for HUD/presentation residual.
            for obj in self.objects.values_mut() {
                if obj.team != team || !obj.is_alive() {
                    continue;
                }
                if reload > 0.0 {
                    obj.special_power_cooldowns.insert(power.clone(), reload);
                } else {
                    obj.special_power_cooldowns.remove(power);
                }
                obj.refresh_special_power_aggregate_cooldown();
            }
        } else if let Some(obj) = self.host_object_mut(object_id) {
            if let Some(module) = parsed_module.as_ref() {
                obj.start_power_recharge_with_frames(power, module.reload_time_frames);
                obj.set_ai_state(AIState::Idle);
            } else {
                obj.consume_special_power_charge(power);
            }
        }
        true
    }

    /// Tick all players' SharedSyncedTimer residual cooldowns.
    ///
    /// Fires EVA SuperweaponReady residual when a PublicTimer power finishes
    /// recharging (own/ally/enemy classification via try_eva_superweapon_ready).
    pub fn tick_shared_special_power_timers(&mut self, dt: f32) {
        if dt <= 0.0 {
            return;
        }
        // Under SPECIAL_POWER_AUTHORITY+shadow, GameWorld sole-ticks shared SP cds.
        if crate::gameworld_shadow::gameworld_special_power_sole_tick_enabled() {
            // Wave 479: do not republish full cooldown snapshots each frame —
            // that stomped GW sole-tick progress. Fire/reset still records via
            // reset_shared_special_power_timer → record_host_cooldowns.
            return;
        }
        let mut ready_events: Vec<(Team, String)> = Vec::new();
        for player in self.players.values_mut() {
            let team = player.team;
            for power in player.tick_shared_special_power_timers(dt) {
                use crate::game_logic::host_special_power_enum_residual::special_power_has_public_timer;
                if !special_power_has_public_timer(&power) {
                    continue;
                }
                // Map power → structure template name residual for EVA classifier.
                let template = match power {
                    crate::command_system::SpecialPowerType::ParticleCannon
                    | crate::command_system::SpecialPowerType::SuperweaponParticleCannon
                    | crate::command_system::SpecialPowerType::LaserCannon => {
                        "AmericaParticleCannonUplink"
                    }
                    crate::command_system::SpecialPowerType::NuclearMissile
                    | crate::command_system::SpecialPowerType::NukeNeutronMissile
                    | crate::command_system::SpecialPowerType::SuperweaponNeutronMissile
                    | crate::command_system::SpecialPowerType::BaikonurRocket => {
                        "ChinaNuclearMissileLauncher"
                    }
                    crate::command_system::SpecialPowerType::ScudStorm => "GLAScudStorm",
                    _ => continue, // EVA only for PUC/Nuke/Scud residual family
                };
                ready_events.push((team, template.to_string()));
            }
        }
        for (team, name) in ready_events {
            // source id unused by try_eva_superweapon_ready residual.
            self.try_eva_superweapon_ready(crate::game_logic::ObjectId(0), team, &name);
        }
    }

    /// C++ SpecialPowerModule::onSpecialPowerCreation residual.
    ///
    /// When a science is first acquired, sharedNSync powers that require it are
    /// expressed ready-now on the player timer (Dustin residual: start ready to fire).
    /// Fail-closed: not full StartsPaused upgrade gate / InGameUI addSuperweapon font.
    pub fn on_special_power_science_creation(&mut self, player_id: u32, science_name: &str) {
        use crate::command_system::SpecialPowerType as P;
        use crate::game_logic::host_sp_science_upgrade_player_team_residual_wave109::normalize_science_name_residual;
        use crate::game_logic::host_special_power_enum_residual::{
            special_power_required_science, special_power_uses_shared_synced_timer,
        };
        let sci = normalize_science_name_residual(science_name);
        if sci.is_empty() {
            return;
        }
        let Some(player) = self.players.get_mut(&player_id) else {
            return;
        };
        // Sample of host powers that may require this science residual.
        const CANDIDATES: &[P] = &[
            P::Airstrike,
            P::AirForceAirstrike,
            P::DaisyCutter,
            P::AirForceDaisyCutter,
            P::FuelAirBomb,
            P::SpyDrone,
            P::Paradrop,
            P::InfantryParadrop,
            P::TankParadrop,
            P::CarpetBomb,
            P::AirForceCarpetBomb,
            P::EarlyChinaCarpetBomb,
            P::ClusterMines,
            P::EmpPulse,
            P::LeafletDrop,
            P::Ambush,
            P::TerrorCell,
            P::Frenzy,
            P::EmergencyRepair,
            P::GpsScrambler,
            P::SneakAttack,
            P::SpectreGunship,
            P::AirForceSpectreGunship,
            P::NapalmStrike,
            P::BlackMarketNuke,
            P::Artillery,
            P::CrateDrop,
            P::CashHack,
            P::SpySatellite,
        ];
        for power in CANDIDATES {
            let Some(req) = special_power_required_science(power) else {
                continue;
            };
            // Match science residual (canonical or alias).
            let req_n = req.to_ascii_lowercase();
            let sci_n = sci.to_ascii_lowercase();
            if req_n != sci_n && !sci_n.ends_with(&req_n) && !req_n.ends_with(&sci_n) {
                continue;
            }
            if !special_power_uses_shared_synced_timer(power) {
                continue;
            }
            // C++: startPowerRecharge then express ready-now for sharedNSync.
            player.express_shared_special_power_ready_now(power);
        }
    }

    pub fn team_has_completed_capture_upgrade(&self, team: Team) -> bool {
        let Some(player) = self.players.values().find(|player| player.team == team) else {
            return true;
        };
        capture_upgrade_names_for_team(team)
            .iter()
            .any(|upgrade| player.has_unlocked_upgrade(upgrade))
    }

    /// C++ `ActionManager::canCaptureBuilding` authority slice.
    ///
    /// This is intentionally shared by physical RMB classification, command
    /// execution, and the in-progress capture state.  It retains the exact
    /// Object INI semantic inputs (SpecialPower, CAPTURABLE,
    /// IMMUNE_TO_CAPTURE, GarrisonContain) rather than deriving permission
    /// from Infantry/Hero/template spelling.
    pub fn can_unit_capture_building(
        &self,
        source_id: ObjectId,
        target_id: ObjectId,
        require_power_ready: bool,
    ) -> bool {
        use gamelogic::common::Relationship;

        if source_id == target_id {
            return false;
        }
        let Some(source) = self.objects.get(&source_id) else {
            return false;
        };
        let Some(target) = self.objects.get(&target_id) else {
            return false;
        };

        let Some(power) = source.thing.template.capture_power.special_power_type() else {
            return false;
        };
        if !source.is_alive()
            || !source.can_move()
            || !target.is_alive()
            || !target.is_kind_of(KindOf::Structure)
            || target.status.under_construction
            || target.status.sold
            || target.thing.template.immune_to_capture
            // C++ ActionManager rejects a pure-stealth target before the
            // relationship/capturable tests.  A disguise is intentionally
            // not pure stealth for this purpose.
            || (target.status.stealthed && !target.status.detected && !target.status.disguised)
        {
            return false;
        }
        if require_power_ready && !self.is_special_power_ready_for(source_id, &power) {
            return false;
        }

        // Player ownership is authoritative whenever both objects have it.
        // Map/unowned objects keep the old faction/Neutral fallback rather
        // than being fabricated into a player relation.
        let relation = if self.has_object_ownership_provenance(source, target) {
            self.object_relationship(source, target)
        } else if source.team == target.team {
            Relationship::Allies
        } else if source.team == Team::Neutral || target.team == Team::Neutral {
            Relationship::Neutral
        } else {
            Relationship::Enemies
        };
        if !(relation == Relationship::Enemies
            || (target.thing.template.capturable && relation != Relationship::Allies))
        {
            return false;
        }

        // C++ rejects a garrisonable target containing any non-stealthed
        // occupant, and *separately* rejects an apparent friendly occupant.
        // The latter is not conditional on a GarrisonContain module, so do
        // not accidentally authorize a structure whose runtime occupant list
        // still contains a friendly child. Missing objects are conservatively
        // rejected so stale links cannot make a defended building capturable.
        let target_is_garrisonable = target.thing.template.garrison_contain_max.is_some();
        for contained_id in target.contained_units() {
            let Some(contained) = self.objects.get(&contained_id) else {
                return false;
            };
            if target_is_garrisonable && !contained.status.stealthed {
                return false;
            }
            let contained_relation = if self.has_object_ownership_provenance(source, contained) {
                self.object_relationship(source, contained)
            } else if source.team == contained.team {
                Relationship::Allies
            } else if source.team == Team::Neutral || contained.team == Team::Neutral {
                Relationship::Neutral
            } else {
                Relationship::Enemies
            };
            if contained_relation == Relationship::Allies {
                return false;
            }
        }
        true
    }

    /// Exact readiness gate for a parsed Hacker Disable Building
    /// `SpecialAbility` module.  This deliberately bypasses the old global
    /// command-enum residual table: C++ owns HDB reload/science/shared state
    /// in the loaded SpecialPowerTemplate paired with this object.
    pub fn is_hacker_disable_building_ready(&self, source_id: ObjectId) -> bool {
        use crate::command_system::SpecialPowerType;

        let Some(source) = self.objects.get(&source_id) else {
            return false;
        };
        let Some(metadata) = source.thing.template.hacker_disable_building.as_ref() else {
            return false;
        };
        if !metadata.update_module_starts_attack
            || metadata.scripted_special_power_only
            || !source.is_alive()
            || source.is_disabled()
            || source
                .special_power_paused
                .contains(&SpecialPowerType::HackerDisableBuilding)
        {
            return false;
        }

        let owner_id = self.player_owner_for_host_object(source);
        if let Some(required_science) = metadata.required_science.as_deref() {
            let Some(owner_id) = owner_id else {
                return false;
            };
            if !self
                .players
                .get(&owner_id)
                .is_some_and(|player| player.has_unlocked_science(required_science))
            {
                return false;
            }
        }
        if metadata.shared_n_sync {
            let Some(owner_id) = owner_id else {
                return false;
            };
            return self.players.get(&owner_id).is_some_and(|player| {
                player.is_shared_special_power_ready(&SpecialPowerType::HackerDisableBuilding)
            });
        }
        source.is_special_power_ready(&SpecialPowerType::HackerDisableBuilding)
    }

    /// Start the exact parsed HDB reload at C++
    /// `SpecialAbilityUpdate::startPreparation`, not at click time.
    pub fn consume_hacker_disable_building_charge(&mut self, source_id: ObjectId) -> bool {
        use crate::command_system::SpecialPowerType;

        if !self.is_hacker_disable_building_ready(source_id) {
            return false;
        }
        let Some((metadata, owner_id)) = self.objects.get(&source_id).and_then(|source| {
            source
                .thing
                .template
                .hacker_disable_building
                .clone()
                .map(|metadata| (metadata, self.player_owner_for_host_object(source)))
        }) else {
            return false;
        };
        if metadata.shared_n_sync {
            let Some(owner_id) = owner_id else {
                return false;
            };
            let Some(player) = self.players.get_mut(&owner_id) else {
                return false;
            };
            player.reset_shared_special_power_timer(
                &SpecialPowerType::HackerDisableBuilding,
                metadata.reload_time_frames as f32 / 30.0,
            );
        } else if let Some(source) = self.objects.get_mut(&source_id) {
            source.start_power_recharge_with_frames(
                &SpecialPowerType::HackerDisableBuilding,
                metadata.reload_time_frames,
            );
        } else {
            return false;
        }
        true
    }

    /// C++ `ActionManager::canDisableBuildingViaHacking` authority slice.
    ///
    /// Both executor and the in-progress HDB channel call this typed path.
    /// `require_power_ready=false` preserves a channel that has already begun
    /// its authored recharge; it still revalidates all live source/target
    /// semantics without treating an expired cooldown as a cancellation.
    pub fn can_unit_hacker_disable_building(
        &self,
        source_id: ObjectId,
        target_id: ObjectId,
        require_power_ready: bool,
    ) -> bool {
        use gamelogic::common::Relationship;

        if source_id == target_id {
            return false;
        }
        let Some(source) = self.objects.get(&source_id) else {
            return false;
        };
        let Some(target) = self.objects.get(&target_id) else {
            return false;
        };
        let Some(metadata) = source.thing.template.hacker_disable_building.as_ref() else {
            return false;
        };
        if !metadata.update_module_starts_attack
            || metadata.scripted_special_power_only
            || !source.is_alive()
            || source.is_disabled()
            || !target.is_alive()
            || !target.is_kind_of(KindOf::Structure)
            || target.is_rebuild_hole
            || target.status.under_construction
            || target.is_effectively_stealthed()
        {
            return false;
        }
        if require_power_ready && !self.is_hacker_disable_building_ready(source_id) {
            return false;
        }

        // `isObjectShroudedForAction` applies to human click authority, not
        // an already running update.  Main's local player is the exact host
        // representation of C++ PLAYER_HUMAN; stale/poisoned visibility fails
        // closed rather than turning an unseen enemy into a valid target.
        if require_power_ready {
            if let Some(owner_id) = self.player_owner_for_host_object(source) {
                if self
                    .players
                    .get(&owner_id)
                    .is_some_and(|player| player.is_local)
                {
                    let visible = gamelogic::system::shroud_manager::get_shroud_manager()
                        .lock()
                        .map(|shroud| shroud.can_see_object(owner_id, target_id.0))
                        .unwrap_or(false);
                    if !visible {
                        return false;
                    }
                }
            }
        }

        let relation = match (source.owner_player_id, target.owner_player_id) {
            (Some(source_owner), Some(target_owner))
                if self.player_owner_for_host_object(source) == Some(source_owner)
                    && self.player_owner_for_host_object(target) == Some(target_owner) =>
            {
                self.player_relationship(source_owner, target_owner)
            }
            (None, None) if self.uses_legacy_team_ownership_fallback() => {
                if source.team == target.team {
                    Relationship::Allies
                } else if source.team == Team::Neutral || target.team == Team::Neutral {
                    Relationship::Neutral
                } else {
                    Relationship::Enemies
                }
            }
            _ => Relationship::Neutral,
        };
        if relation != Relationship::Enemies {
            return false;
        }

        // C++ permits either a normal capturable building or its FSTechnology
        // exception.  The exception has to remain separate: a technology
        // structure is legal even if it lacks CAPTURABLE, but not if immune.
        let capturable = target.thing.template.capturable && !target.is_rebuild_hole;
        let technology_exception =
            target.is_kind_of(KindOf::FSTechnology) && !target.thing.template.immune_to_capture;
        if !(capturable || technology_exception) {
            return false;
        }

        // `appearsToContainFriendlies` is independent of GarrisonContain.
        // A stale contained id is unknown to C++; fail closed rather than
        // making malformed containment look empty and hackable.
        for contained_id in target.contained_units() {
            let Some(contained) = self.objects.get(&contained_id) else {
                return false;
            };
            let contained_relation = match (source.owner_player_id, contained.owner_player_id) {
                (Some(source_owner), Some(contained_owner))
                    if self.player_owner_for_host_object(source) == Some(source_owner)
                        && self.player_owner_for_host_object(contained)
                            == Some(contained_owner) =>
                {
                    self.player_relationship(source_owner, contained_owner)
                }
                (None, None) if self.uses_legacy_team_ownership_fallback() => {
                    if source.team == contained.team {
                        Relationship::Allies
                    } else if source.team == Team::Neutral || contained.team == Team::Neutral {
                        Relationship::Neutral
                    } else {
                        Relationship::Enemies
                    }
                }
                _ => Relationship::Neutral,
            };
            if contained_relation == Relationship::Allies {
                return false;
            }
        }
        true
    }

    /// Exact Object INI capture range for an already-issued capture order.
    /// A missing `SpecialAbilityUpdate` fails closed rather than silently
    /// granting the old generic infantry/hero radius.
    pub fn unit_capture_start_ability_range(&self, object_id: ObjectId) -> Option<f32> {
        self.objects
            .get(&object_id)
            .and_then(|obj| obj.thing.template.capture_start_ability_range)
    }

    pub fn local_player_id(&self) -> Option<u32> {
        self.players
            .values()
            .find(|player| player.is_local)
            .map(|player| player.id)
    }

    pub fn is_local_player(&self, player_id: u32) -> bool {
        self.players
            .get(&player_id)
            .map(|player| player.is_local)
            .unwrap_or(false)
    }

    /// Override a player's display name (used by CLI / networking parity).
    pub fn set_player_name(&mut self, player_id: u32, name: &str) -> bool {
        if let Some(player) = self.players.get_mut(&player_id) {
            player.name = name.to_string();
            true
        } else {
            false
        }
    }

    /// Override a player's team/faction at runtime (used by menu selection).
    pub fn set_player_team(&mut self, player_id: u32, team: Team) -> bool {
        if let Some(player) = self.players.get_mut(&player_id) {
            player.team = team;
            true
        } else {
            false
        }
    }

    /// Wave 921: single authority boundary for match start + local faction (+ optional AI).
    #[inline]
    pub fn start_new_game_with_faction(
        &mut self,
        mode: GameMode,
        player_id: u32,
        faction_team: Team,
        setup_skirmish_ai: bool,
    ) {
        self.start_new_game(mode);
        let _ = self.set_player_team(player_id, faction_team);
        if setup_skirmish_ai {
            self.setup_skirmish_ai(player_id);
        }
    }

    /// C++ GameInfo/SidesList start path for a selected Campaign or Challenge
    /// PlayerTemplate.  The exact identity is resolved before any base-team
    /// behavior is chosen; an invalid late resolution removes the local player
    /// instead of starting the wrong General.
    #[inline]
    pub fn start_new_game_with_player_template(
        &mut self,
        mode: GameMode,
        player_id: u32,
        player_template: PlayerTemplateIdentity,
    ) -> bool {
        self.start_new_game(mode);
        if !self.bind_player_template_identity(player_id, player_template) {
            log::error!(
                "Rejecting selected PlayerTemplate after session reset; removing local player {} rather than falling back to a base Team",
                player_id
            );
            self.players.remove(&player_id);
            self.player_template_bindings.remove(&player_id);
            return false;
        }
        true
    }

    /// Bind an already validated offline GameInfo slot selection to an
    /// existing bootstrap player. Campaign/Challenge and offline Skirmish
    /// share this exact C++ `Player::init(PlayerTemplate)` seam; callers
    /// remain responsible for restoring any GameInfo dict overrides (such as
    /// the per-slot color) after this template-owned state is applied.
    pub(crate) fn bind_player_template_identity(
        &mut self,
        player_id: u32,
        player_template: PlayerTemplateIdentity,
    ) -> bool {
        let Some(template) = player_template.resolve() else {
            return false;
        };
        let Some(team) = PlayerTemplateIdentity::team_for_template(&template) else {
            return false;
        };
        let Some(player) = self.players.get_mut(&player_id) else {
            return false;
        };

        // C++ Player::init(pt): the exact template owns base side, preferred
        // color, starting money fallback, and rank-one sciences.  The generic
        // player seeded by start_new_game is only a bootstrap shell.
        player.team = team;
        player.apply_player_template_start_state(&template);
        self.player_template_bindings
            .insert(player_id, player_template);
        true
    }

    /// Apply an upgrade tag to an object.
    /// Mirrors C++ behavior where upgrades are persistent object state, not display-name edits.
    pub(crate) fn apply_upgrade_to_object(&mut self, object_id: ObjectId, upgrade: &str) {
        use crate::game_logic::host_overlord_addons::{
            is_bunker_addon_upgrade, is_gattling_addon_upgrade, is_overlord_family_host,
            is_propaganda_addon_upgrade,
        };

        let mut installed_gattling = false;
        let mut installed_propaganda = false;
        let mut installed_bunker = false;

        if let Some(obj) = self.objects.get_mut(&object_id) {
            obj.apply_upgrade_tag(upgrade);
            // C++ SubObjectsUpgrade residual (BombTruck loads / Helix BombWing).
            {
                let applied =
                    crate::game_logic::host_sub_objects_upgrade::sub_objects_for_upgrade_tags(
                        &obj.applied_upgrades,
                        &obj.template_name,
                    );
                if applied.matched {
                    obj.sub_object_visibility
                        .apply_show_hide(&applied.show, &applied.hide);
                    self.sub_objects_upgrades.record(&applied.show);
                }
            }
            // C++ ModelConditionUpgrade residual.
            let _ = crate::game_logic::host_model_condition_upgrade::apply_model_condition_upgrade(
                &mut obj.model_condition_bits,
                upgrade,
            );
            // C++ WeaponBonusUpgrade residual.
            if crate::game_logic::host_upgrade_module_residuals::is_weapon_bonus_upgrade(upgrade) {
                obj.set_weapon_bonus_player_upgrade(true);
                self.upgrade_module_residuals.record_weapon_bonus(upgrade);
            }
            // C++ WeaponSetUpgrade residual → WEAPONSET_PLAYER_UPGRADE.
            if crate::game_logic::host_upgrade_module_residuals::is_weapon_set_upgrade(upgrade) {
                obj.set_weapon_set_flag(0, true);
                self.upgrade_module_residuals.record_weapon_set(upgrade);
            }
            // C++ ArmorUpgrade residual → ARMORSET_PLAYER_UPGRADE (+ ChemSuit decal).
            if crate::game_logic::host_upgrade_module_residuals::is_armor_upgrade(upgrade) {
                obj.set_armor_set_player_upgrade(true);
                if crate::game_logic::host_upgrade_module_residuals::is_chemical_suits_upgrade(
                    upgrade,
                ) {
                    obj.set_terrain_decal_chemsuit(true);
                }
                self.upgrade_module_residuals.record_armor_set(upgrade);
            }
            // C++ LocomotorSetUpgrade residual → setLocomotorUpgrade(true) + speed peels.
            if crate::game_logic::host_upgrade_module_residuals::is_locomotor_set_upgrade(upgrade) {
                obj.set_locomotor_upgrade(true);
                if let Some(speed) =
                    crate::game_logic::host_upgrade_module_residuals::locomotor_upgrade_speed(
                        upgrade,
                        &obj.template_name,
                    )
                {
                    // Host residual: raise movement max speed when peel known.
                    obj.movement.max_speed = obj.movement.max_speed.max(speed);
                    obj.movement.max_speed_damaged =
                        obj.movement.max_speed_damaged.max(speed * 0.5);
                }
                self.upgrade_module_residuals.record_locomotor_set(upgrade);
            }
            // C++ UnpauseSpecialPowerUpgrade residual.  Capture uses its
            // exact module-local `TriggeredBy` field; do not turn an upgrade
            // spelling that merely contains "capture" into an ability on a
            // Ranger/RedGuard/Rebel-named object.
            let capture_unpause = obj
                .thing
                .template
                .capture_power
                .special_power_type()
                .filter(|_| {
                    obj.thing
                        .template
                        .capture_upgrade_trigger
                        .as_deref()
                        .is_some_and(|trigger| trigger.trim().eq_ignore_ascii_case(upgrade.trim()))
                });
            if let Some(power) = capture_unpause {
                obj.pause_special_power_countdown(&power, false);
                self.upgrade_module_residuals.record_unpause(upgrade);
            } else if let Some(power) =
                crate::game_logic::host_upgrade_module_residuals::unpause_power_for_upgrade(upgrade)
            {
                // Capture-family values are handled only above by exact INI
                // metadata.  The remaining non-capture residual remains
                // compatible with its existing module peel.
                if crate::game_logic::CapturePowerKind::from_special_power_type(&power)
                    == crate::game_logic::CapturePowerKind::None
                {
                    for p in crate::game_logic::host_upgrade_module_residuals::unpause_power_family(
                        power,
                    ) {
                        obj.pause_special_power_countdown(&p, false);
                    }
                    self.upgrade_module_residuals.record_unpause(upgrade);
                }
            }
            // C++ CommandSetUpgrade residual.
            if let Some(cs) =
                crate::game_logic::host_replace_object_upgrade::command_set_override_for_upgrade(
                    upgrade,
                    &obj.template_name,
                )
            {
                obj.set_command_set_override(Some(cs.to_string()));
                self.replace_grant_command_upgrades.record_command_set(cs);
            }
            if is_overlord_family_host(&obj.template_name) {
                if is_gattling_addon_upgrade(upgrade) {
                    obj.install_overlord_gattling_addon();
                    installed_gattling = true;
                } else if is_propaganda_addon_upgrade(upgrade) {
                    obj.install_overlord_propaganda_addon();
                    installed_propaganda = true;
                } else if is_bunker_addon_upgrade(upgrade) {
                    // C++ ChinaTankOverlordBattleBunker TransportContain.Slots = 5.
                    // Helix bunker also uses Slots residual 5 (ChinaHelixBattleBunker).
                    obj.install_overlord_battle_bunker(5);
                    // C++ PassengersFireUpgrade TriggeredBy Upgrade_ChinaHelixBattleBunker.
                    use crate::game_logic::host_passengers_fire_upgrade::should_enable_passengers_fire;
                    if should_enable_passengers_fire(upgrade, &obj.template_name)
                        || obj.is_helix_transport
                    {
                        obj.passengers_allowed_to_fire = true;
                        obj.record_host_stealth_flags();
                        self.passengers_fire_upgrade_reg.record_apply(1);
                    }
                    installed_bunker = true;
                }
            }
        }

        if installed_gattling {
            self.overlord_addons.record_gattling_install();
        }
        if installed_propaganda {
            self.overlord_addons.record_propaganda_install();
        }
        let _ = installed_bunker;

        // C++ CostModifierUpgrade residual — player KindOf production cost change.
        if let Some((kind, percent)) =
            crate::game_logic::host_upgrade_module_residuals::cost_modifier_for_upgrade(upgrade)
        {
            let team = self.objects.get(&object_id).map(|o| o.team);
            if let Some(team) = team {
                if let Some(player) = self.players.values_mut().find(|p| p.team == team) {
                    player.add_kind_of_production_cost_change(kind, percent);
                    self.upgrade_module_residuals.record_cost(upgrade);
                }
            }
        }

        // C++ GenerateMinefieldBehavior::upgradeImplementation residual.
        let _mines = self.place_structure_minefield_for_upgrade(object_id, upgrade);

        // C++ GrantScienceUpgrade residual.
        if let Some(science) =
            crate::game_logic::host_replace_object_upgrade::grant_science_for_upgrade(upgrade)
        {
            let team = self.objects.get(&object_id).map(|o| o.team);
            if let Some(team) = team {
                if let Some(player) = self.players.values_mut().find(|p| p.team == team) {
                    if player.unlock_science(science) {
                        self.replace_grant_command_upgrades.record_science(science);
                    }
                }
            }
        }

        // C++ ReplaceObjectUpgrade residual (FakeGLA* → real building).
        // C++ destroys immediately (pathfinder unmark + destroyObject) before spawn.
        // Host residual: remove from world map now (skip topple/deferred die list).
        if crate::game_logic::host_replace_object_upgrade::is_replace_object_upgrade(upgrade) {
            let info = self
                .objects
                .get(&object_id)
                .map(|o| (o.template_name.clone(), o.team, o.get_position()));
            if let Some((template_name, team, pos)) = info {
                if let Some(replacement) =
                    crate::game_logic::host_replace_object_upgrade::replacement_template_for_fake(
                        &template_name,
                    )
                {
                    if !self.templates.contains_key(&replacement) {
                        if let Some(src) = self.templates.get(&template_name).cloned() {
                            let mut dst = src;
                            dst.name = replacement.clone();
                            self.templates.insert(replacement.clone(), dst);
                        }
                    }
                    // Immediate remove — same spirit as C++ destroy-before-create.
                    self.host_radar_remove_object(object_id);
                    let _removed = self.objects.remove(&object_id);
                    if let Some(new_id) = self.create_object(&replacement, team, pos) {
                        if let Some(obj) = self.objects.get_mut(&new_id) {
                            obj.status.under_construction = false;
                        }
                        self.replace_grant_command_upgrades
                            .record_replace(&template_name, &replacement);
                        let _ = new_id;
                    }
                }
            }
        }
    }
}
