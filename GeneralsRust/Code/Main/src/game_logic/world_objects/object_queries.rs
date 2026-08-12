//! Host objects `impl GameLogic` — `object_queries`.
//! find_object, players, session queries. Child of `world_objects` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;

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

    /// Nearest alive harvestable supply pile residual for gather re-target.
    pub(in super::super) fn find_nearest_harvestable_supply(
        &self,
        team: Team,
        from: Vec3,
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
                let name = obj.template_name.to_ascii_lowercase();
                let harvestable = obj.is_kind_of(KindOf::Harvestable)
                    || obj.is_kind_of(KindOf::Resource)
                    || obj.object_type == ObjectType::Supply
                    || (name.contains("supply")
                        && !name.contains("center")
                        && !name.contains("dock")
                        && !name.contains("dropzone"));
                if !harvestable {
                    return None;
                }
                // Prefer piles that still have stored supplies when tracked.
                if obj.stored_resources.supplies == 0
                    && (obj.is_kind_of(KindOf::Harvestable)
                        || obj.object_type == ObjectType::Supply)
                {
                    // Some piles use infinite residual; only skip if explicitly zero and
                    // Harvestable with supplies field used as stock. Fail-open if never depleted.
                    if name.contains("warehouse") || name.contains("dock") {
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
        crate::game_logic::host_residual_acquire::pick_nearest_residual_target(
            ObjectId(u32::MAX),
            Team::Neutral,
            from,
            candidates,
            |_| f32::MAX,
            |_| true,
        )
        .map(|(id, _, _)| id)
    }

    pub(in super::super) fn find_nearest_supply_center(
        &self,
        team: Team,
        from_position: Vec3,
    ) -> Option<ObjectId> {
        // Pure residual acquire: nearest friendly constructed SupplyCenter (3D).
        let candidates: Vec<_> = self
            .objects
            .iter()
            .filter_map(|(&obj_id, obj)| {
                if obj.team != team
                    || !obj.is_alive()
                    || !obj.is_constructed()
                    || (obj.thing.template.dock_kind != crate::game_logic::DockKind::SupplyCenter
                        && !obj.is_kind_of(KindOf::SupplyCenter))
                {
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
        from_position: Vec3,
    ) -> Option<ObjectId> {
        let preferred = self
            .objects
            .get(&collector_id)
            .and_then(|collector| collector.preferred_dock_id);
        if let Some(center_id) = preferred {
            let valid_preferred_center = self.objects.get(&center_id).is_some_and(|center| {
                center.team == team
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

        self.find_nearest_supply_center(team, from_position)
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
        let Some(object) = self.objects.get_mut(&object_id) else {
            return false;
        };
        object.set_team_and_owner(team, Some(player_id));
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
    }

    /// Add a player directly (for snapshot restoration)
    pub fn add_player(&mut self, player: Player) {
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
        // C++ SpecialPowerStore::canUseSpecialPower science residual.
        if let Some(required) =
            crate::game_logic::host_special_power_enum_residual::special_power_required_science(
                power,
            )
        {
            match self.get_player_by_team(obj.team) {
                Some(player) if player.has_unlocked_science(required) => {}
                Some(_) => return false,
                // Fail-closed: science-gated powers need a controlling player residual.
                None => return false,
            }
        }
        if crate::game_logic::host_special_power_enum_residual::special_power_uses_shared_synced_timer(
            power,
        ) {
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
        let team = match self.host_object(object_id) {
            Some(o) => o.team,
            None => return false,
        };
        let reload =
            crate::game_logic::host_special_power_enum_residual::special_power_reload_seconds(
                power,
            )
            .unwrap_or_else(|| {
                self.host_object(object_id)
                    .map(|o| o.special_power_cooldown)
                    .unwrap_or(10.0)
            });

        if crate::game_logic::host_special_power_enum_residual::special_power_uses_shared_synced_timer(
            power,
        ) {
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
            obj.consume_special_power_charge(power);
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
