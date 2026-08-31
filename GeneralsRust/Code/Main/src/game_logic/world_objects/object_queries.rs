//! Host objects `impl GameLogic` — `object_queries`.
//! find_object, players, session queries. Child of `world_objects` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;

/// C++ `SpecialPowerStore::canUseSpecialPower` (`SpecialPower.cpp:308`)
/// requires `Object::getSpecialPowerModule`. Frenzy/CashHack and every other
/// command fail closed without an authored module on the firing object.
/// Capture / Hacker-disable live residuals store that module outside
/// `special_power_modules` and still count as present.
///
/// A per-object cooldown entry for this power is equivalent C++
/// `SpecialPowerModule` evidence: the countdown only exists because a live
/// module (or its host residual bind) was armed. `SpecialPowerModule::isReady`
/// reads the timer, not the parsed INI record, so cooldown-tracked objects
/// without a parsed module record still carry the module.
fn object_has_special_power_module(
    obj: &Object,
    power: &crate::command_system::SpecialPowerType,
) -> bool {
    if obj
        .thing
        .template
        .special_power_module_for_command(power)
        .is_some()
    {
        return true;
    }
    if obj
        .thing
        .template
        .capture_power
        .special_power_type()
        .as_ref()
        == Some(power)
    {
        return true;
    }
    matches!(
        power,
        crate::command_system::SpecialPowerType::HackerDisableBuilding
    ) && obj.thing.template.hacker_disable_building.is_some()
        // Cooldown-tracked residual evidence (see doc above).
        || obj.special_power_cooldowns.contains_key(power)
}

/// C++ `Player::getRelationship(const Team*)` leftover map written by
/// `PLAYER_SET_OVERRIDE_RELATION_TO_TEAM`.
fn leftover_team_relationship_override(
    source_player_id: u32,
    team_name: &str,
) -> Option<gamelogic::common::Relationship> {
    if team_name.trim().is_empty() {
        return None;
    }
    let team = {
        let Ok(factory) = gamelogic::team::get_team_factory().lock() else {
            return None;
        };
        factory.find_team_instances(team_name).into_iter().next()?
    };
    let Ok(list) = gamelogic::player::ThePlayerList().read() else {
        return None;
    };
    let player_arc = list
        .get_player(source_player_id as gamelogic::player::PlayerIndex)
        .cloned()?;
    drop(list);
    let Ok(player) = player_arc.read() else {
        return None;
    };
    let Ok(team_guard) = team.read() else {
        return None;
    };
    player.override_relationship_for_team(&team_guard)
}

/// C++ `Team::getRelationship` team/player override maps written by
/// `TEAM_SET_OVERRIDE_RELATION_TO_TEAM` / `_TO_PLAYER`.
fn leftover_source_team_override(
    source_team_name: &str,
    target_team_name: &str,
    target_owner: Option<u32>,
) -> Option<gamelogic::common::Relationship> {
    if source_team_name.trim().is_empty() {
        return None;
    }
    let source = {
        let Ok(factory) = gamelogic::team::get_team_factory().lock() else {
            return None;
        };
        factory
            .find_team_instances(source_team_name)
            .into_iter()
            .next()?
    };
    let target = if target_team_name.trim().is_empty() {
        None
    } else {
        match gamelogic::team::get_team_factory().lock() {
            Ok(factory) => factory
                .find_team_instances(target_team_name)
                .into_iter()
                .next(),
            Err(_) => None,
        }
    };
    let Ok(source_guard) = source.read() else {
        return None;
    };
    if let Some(target_arc) = target {
        if let Ok(target_guard) = target_arc.read() {
            if let Some(rel) = source_guard.override_relationship_with_team(&target_guard) {
                return Some(rel);
            }
        }
    }
    if let Some(pid) = target_owner {
        if let Some(rel) = source_guard.override_relationship_with_player(pid as i32) {
            return Some(rel);
        }
    }
    None
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
        self.find_nearest_harvestable_supply_within(team, from, None, ObjectId(u32::MAX))
    }

    /// C++ `ResourceGatheringManager::findBestSupplyWarehouse` scan cap.
    /// `max_scan` is already AI-doubled via `warehouse_scan_distance`.
    /// C++ `computeRelativeCost` returns FLT_MAX when `!isClearToApproach`.
    pub(in super::super) fn find_nearest_harvestable_supply_within(
        &self,
        team: Team,
        from: Vec3,
        max_scan: Option<f32>,
        query_id: ObjectId,
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
                // C++ `computeRelativeCost`: occupied approach-queues score FLT_MAX.
                if obj.thing.template.dock_kind == DockKind::SupplyWarehouse
                    && !crate::game_logic::host_supply_gather::live_dock_is_clear_to_approach(
                        id, query_id,
                    )
                {
                    return None;
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

    /// C++ `Player::getRelationship` / `PlayerList` relationship pass.
    /// Distinct players default Neutral (not Enemies). Map playerAllies /
    /// playerEnemies win. Skirmish lobby `alliance_team` still allies a slot
    /// team and treats two assigned-but-different teams as enemies.
    pub fn player_relationship(
        &self,
        source_player_id: u32,
        target_player_id: u32,
    ) -> gamelogic::common::Relationship {
        Self::player_relationship_from_map(&self.players, source_player_id, target_player_id)
    }

    /// C++ `Player::getRelationship` / `PlayerList` using an explicit roster.
    /// CombatSystem splash applies while `objects` is mutably borrowed, so the
    /// live host passes `&self.players` instead of `&self`.
    pub fn player_relationship_from_map(
        players: &std::collections::HashMap<u32, crate::game_logic::Player>,
        source_player_id: u32,
        target_player_id: u32,
    ) -> gamelogic::common::Relationship {
        use gamelogic::common::Relationship;

        let Some(source) = players.get(&source_player_id) else {
            return Relationship::Neutral;
        };
        let Some(target) = players.get(&target_player_id) else {
            return Relationship::Neutral;
        };
        if source_player_id == target_player_id {
            return Relationship::Allies;
        }
        // Inactive/map-placeholder slots are not combat enemies.
        if !source.is_alive || !target.is_alive {
            return Relationship::Neutral;
        }
        if let Some(rel) = source.map_relationship(target_player_id) {
            return rel;
        }
        if source.alliance_team >= 0 && source.alliance_team == target.alliance_team {
            Relationship::Allies
        } else if source.alliance_team >= 0 && target.alliance_team >= 0 {
            Relationship::Enemies
        } else {
            Relationship::Neutral
        }
    }

    /// Relationship inferred from persistent object ownership. Team-id
    /// overrides (C++ `Player::getRelationship(const Team*)`) win first so a
    /// named team can be allied while the rest of that player stays enemy.
    ///
    /// C++ `Object::getRelationship` (`Object.cpp:1548-1568`) applies
    /// undetected-defector overrides before the team map: self → Neutral
    /// (do not auto-acquire), that → Allies (treat flashing defector as own).
    pub fn object_relationship(
        &self,
        source: &Object,
        target: &Object,
    ) -> gamelogic::common::Relationship {
        use gamelogic::common::Relationship;
        if source.is_undetected_defector() {
            return Relationship::Neutral;
        }
        if target.is_undetected_defector() {
            return Relationship::Allies;
        }
        Self::object_relationship_from_owners(
            &self.players,
            source.owner_player_id,
            &source.team_instance_name,
            target.owner_player_id,
            &target.team_instance_name,
        )
    }

    /// C++ `Object::getRelationship` from frozen owner ids (Weapon.cpp:1360).
    /// `source` is the viewer (`curVictim->getRelationship(source)` when the
    /// first pair is the victim).
    pub fn object_relationship_from_owners(
        players: &std::collections::HashMap<u32, crate::game_logic::Player>,
        source_owner: Option<u32>,
        source_team_instance: &str,
        target_owner: Option<u32>,
        target_team_instance: &str,
    ) -> gamelogic::common::Relationship {
        if !source_team_instance.is_empty() {
            if let Some(rel) = leftover_source_team_override(
                source_team_instance,
                target_team_instance,
                target_owner,
            ) {
                return rel;
            }
            if let Some(source_player_id) = source_owner {
                if let Some(source_player) = players.get(&source_player_id) {
                    if !target_team_instance.is_empty() {
                        if let Some(rel) = source_player
                            .team_instance_team_override(source_team_instance, target_team_instance)
                        {
                            return rel;
                        }
                    }
                    if let Some(target_player_id) = target_owner {
                        if let Some(rel) = source_player
                            .team_instance_player_override(source_team_instance, target_player_id)
                        {
                            return rel;
                        }
                    }
                }
            }
        }

        use gamelogic::common::Relationship;

        if let Some(source_player_id) = source_owner {
            if !target_team_instance.is_empty() {
                if let Some(rel) =
                    leftover_team_relationship_override(source_player_id, target_team_instance)
                {
                    return rel;
                }
                if let Some(source_player) = players.get(&source_player_id) {
                    if let Some(rel) =
                        source_player.team_relationship_override(target_team_instance)
                    {
                        return rel;
                    }
                }
            }
        }

        match (source_owner, target_owner) {
            (Some(source_player_id), Some(target_player_id)) => {
                Self::player_relationship_from_map(players, source_player_id, target_player_id)
            }
            _ => Relationship::Neutral,
        }
    }

    /// Stamp pathfinder occupancy ALLIES bits from Player relationships.
    /// C++ checkForMovement uses getRelationship == ALLIES (AIPathfind.cpp:5037).
    pub fn refresh_pathfind_ally_masks(&mut self) {
        use gamelogic::common::Relationship;
        let mut masks = [0u16; 16];
        for a in 0..16u32 {
            for b in 0..16u32 {
                if self.player_relationship(a, b) == Relationship::Allies {
                    masks[a as usize] |= 1u16 << b;
                }
            }
        }
        self.pathfinding_system.set_player_ally_masks(masks);
        let mut human = 0u16;
        for (id, player) in &self.players {
            if player.is_local && *id < 16 {
                human |= 1u16 << *id;
            }
        }
        self.pathfinding_system.set_human_player_mask(human);
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

    /// C++ `ActionManager::canEnterObject` unmanned branch (`:552-560`):
    /// any infantry that is not `KINDOF_REJECT_UNMANNED` may Enter a
    /// `DISABLED_UNMANNED` husk even when it has no transport capacity.
    /// Distinct from USA Pilot `VeterancyCrateCollide IsPilot` recrew.
    #[inline]
    pub fn can_execute_infantry_unmanned_recrew(
        &self,
        infantry_id: ObjectId,
        vehicle_id: ObjectId,
    ) -> bool {
        if infantry_id == vehicle_id {
            return false;
        }
        let Some(infantry) = self.host_object(infantry_id) else {
            return false;
        };
        let Some(vehicle) = self.host_object(vehicle_id) else {
            return false;
        };
        infantry.is_alive()
            && infantry.is_kind_of(KindOf::Infantry)
            && infantry.can_move()
            && !infantry.status.under_construction
            && !crate::game_logic::host_car_bomb::object_definition_has_kind(
                &infantry.template_name,
                "REJECT_UNMANNED",
            )
            && vehicle.is_alive()
            && vehicle.is_unmanned()
            && !vehicle.status.under_construction
            && !vehicle.status.sold
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

    /// C++ `ContainModuleInterface::getStealthUnitsContained` recount:
    /// occupants with `KINDOF_STEALTH_GARRISON` vs everyone else.

    pub fn stealth_garrison_occupant_counts(&self, target: &Object) -> (usize, usize) {
        let occupants = target.contained_units();
        let stealth = occupants
            .iter()
            .filter(|id| {
                self.host_object(**id)
                    .is_some_and(|occupant| occupant.is_kind_of(KindOf::StealthGarrison))
            })
            .count();
        (stealth, occupants.len().saturating_sub(stealth))
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
            let player_id = target.tunnel_system_key();
            return Some(
                if self.tunnel_network.is_in_network(player_id, unit_id)
                    || self.tunnel_network.has_capacity(player_id)
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

    /// C++ `isObjectShroudedForAction` (`ActionManager.cpp:76-102`).
    /// Human controlling player, impetus not `CMD_FROM_SCRIPT`, target
    /// `getShroudedStatus >= OBJECTSHROUD_FOGGED`. Live host is the player
    /// path, so command source is FromPlayer.
    pub(crate) fn is_object_shrouded_for_action(&self, source: &Object, target: &Object) -> bool {
        let Some(player_id) = self.player_owner_for_host_object(source) else {
            return false;
        };
        if !self.player_is_human(player_id) {
            return false;
        }
        // Leftover `Object::getShroudedStatus` (Object.cpp:1778-1788).
        if target.get_template().always_visible || target.contained_by.is_some() {
            return false;
        }
        let Ok(shroud) = gamelogic::system::shroud_manager::get_shroud_manager().lock() else {
            return false;
        };
        match shroud.get_host_object_shroud_status(player_id, target.id.0) {
            Some(status) => (status as u8) >= (gamelogic::common::ObjectShroudStatus::Fogged as u8),
            None => false,
        }
    }

    /// ID wrapper so command-executor recrew can apply the same C++ gate.
    pub(crate) fn is_enter_target_shrouded_for_action(
        &self,
        source_id: ObjectId,
        target_id: ObjectId,
    ) -> bool {
        let Some(source) = self.host_object(source_id) else {
            return false;
        };
        let Some(target) = self.host_object(target_id) else {
            return false;
        };
        self.is_object_shrouded_for_action(source, target)
    }

    /// C++ `ActionManager::canHijackVehicle` (`ActionManager.cpp:829-887`).
    /// Guard HijackGuard inner scan installs `PartitionFilterPossibleToHijack`
    /// which calls this. Relationship `ENEMIES` is applied by the scan
    /// (`ALLOW_ENEMIES` / leftover `relationship_to`); this gate is live,
    /// unshrouded, KINDOF_VEHICLE, not AIRCRAFT, not DRONE, plus
    /// `HijackedVehicleCrateCollide::wouldLikeToCollideWith`.
    pub(crate) fn can_hijack_vehicle(&self, hijacker_id: ObjectId, target: &Object) -> bool {
        if !target.is_alive() {
            return false;
        }
        let Some(hijacker) = self.objects.get(&hijacker_id) else {
            return false;
        };
        if self.is_object_shrouded_for_action(hijacker, target) {
            return false;
        }
        if !target.is_kind_of(KindOf::Vehicle) {
            return false;
        }
        if target.is_kind_of(KindOf::Aircraft) {
            return false;
        }
        if target.is_kind_of(KindOf::Drone) {
            return false;
        }
        // C++ wouldLikeToCollideWith → isValidToExecute extra gates.
        !crate::game_logic::host_car_bomb::hijack_target_rejected(target)
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

        // C++ ActionManager.cpp:519-521 — leftover can_enter_object.
        if self.is_object_shrouded_for_action(unit, target) {
            return false;
        }

        // C++ GarrisonContain::isValidContainerFor — health <= 0 or
        // BODY_REALLYDAMAGED unless KINDOF_GARRISONABLE_UNTIL_DESTROYED.
        if target.is_garrison_contain() && !target.garrison_container_accepts_entry() {
            return false;
        }

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

        // C++ ActionManager.cpp:636-644 — HealContain is not a transport;
        // a unit at max health cannot enter barracks/hospital.
        if target.thing.template.contain_module.kind.is_heal_contain()
            && unit.health.current >= unit.health.maximum
        {
            return false;
        }

        let unit_in_tunnel = self.tunnel_network.player_holding_unit(unit_id).is_some();
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
        let mut skip_capacity_for_stealth_garrison = false;

        if target.is_tunnel_network_style_container() {
            if unit.is_kind_of(KindOf::Aircraft) {
                return false;
            }
            // C++ ActionManager.cpp:662-670 — faction structure rejects a
            // different controlling player. Same-faction allies/enemies cannot
            // board another player's Tunnel Network.
            if !same_controller {
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

            // Leftover OpenContain::is_valid_container_for KindOf algebra.
            // Forbid HUGE_VEHICLE rejects Overlord/Helix without fail-closing
            // infantry / ordinary vehicle Enter.
            if !target
                .thing
                .template
                .contain_module
                .leftover_kind_masks_admit(unit.kind_of_cpp_mask())
            {
                return false;
            }
            if target.is_combat_chinook_style_container()
                && !crate::game_logic::host_combat_chinook::combat_chinook_allows_rider(
                    unit.is_kind_of(KindOf::Infantry),
                    unit.is_kind_of(KindOf::Vehicle),
                    unit.is_kind_of(KindOf::Aircraft),
                    unit.is_kind_of(KindOf::HugeVehicle),
                )
            {
                return false;
            }

            // C++ ActionManager.cpp:656-675: a different player may Enter a
            // non-faction container when every occupant is KINDOF_STEALTH_GARRISON.
            // Mixed / regular occupants and faction structures still reject.
            // Stealth-only also skips CHECK_CAPACITY (kick-out happens on arrive).
            skip_capacity_for_stealth_garrison = false;

            if !same_controller {
                let (stealth, non_stealth) = self.stealth_garrison_occupant_counts(target);
                if non_stealth > 0 || target.is_faction_structure() {
                    return false;
                }
                if stealth > 0 && non_stealth == 0 {
                    skip_capacity_for_stealth_garrison = true;
                }
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
        if skip_capacity_for_stealth_garrison {
            return true;
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

    /// C++ `PartitionFilterHordeMember::allow` AlliesOnly (`HordeUpdate.cpp:77-79`).
    /// Owned objects use `Player::getRelationship == ALLIES`. Ownerless synthetic
    /// worlds keep same-faction equality so existing residual tests still horde.
    pub fn horde_allies_only(
        &self,
        a_owner: Option<u32>,
        a_team: Team,
        b_owner: Option<u32>,
        b_team: Team,
    ) -> bool {
        use gamelogic::common::Relationship;
        match (a_owner, b_owner) {
            (Some(a), Some(b)) => self.player_relationship(a, b) == Relationship::Allies,
            (None, None) if self.uses_legacy_team_ownership_fallback() => {
                a_team != Team::Neutral && a_team == b_team
            }
            _ => false,
        }
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

    /// C++ `TheAI` next formation id after restoring stamped formations.
    pub fn set_next_formation_id_for_restore(&mut self, next_formation_id: u32) {
        self.next_formation_id = next_formation_id.max(1);
    }

    /// C++ `Object::m_containedByFrame` map for HealContain auto-exit.
    pub fn contained_by_frame_for_snapshot(&self, unit_id: ObjectId) -> Option<u32> {
        self.tunnel_network.contained_by_frame(unit_id)
    }

    pub fn stamp_contained_by_frame(&mut self, unit_id: ObjectId, frame: u32) {
        self.tunnel_network.stamp_contained_by_frame(unit_id, frame);
    }

    pub fn restore_contained_by_frames(&mut self, frames: &[(ObjectId, u32)]) {
        self.tunnel_network.restore_contained_by_frames(frames);
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

    /// Wave 239: command-center world pose for a specific player.
    /// C++ `viewCommandCenter` iterates `localPlayer` objects only
    /// (`CommandXlat.cpp:780-793`), then most expensive structure via
    /// `calcCostToBuild(controllingPlayer)`.
    pub fn player_command_center_position(&self, id: u32) -> Option<glam::Vec3> {
        let team = self.player_team(id)?;
        let mut fallback = None;
        let mut highest_cost = i32::MIN;
        let sole_team_owner = self
            .players
            .values()
            .filter(|p| p.team == team && p.is_alive)
            .count()
            <= 1;

        for obj in self.objects.values() {
            if !obj.is_alive() {
                continue;
            }
            let ours = match obj.owner_player_id {
                Some(owner) => owner == id,
                None => sole_team_owner && obj.team == team,
            };
            if !ours {
                continue;
            }
            if obj.is_kind_of(KindOf::CommandCenter) {
                return Some(obj.get_position());
            }
            if obj.is_kind_of(KindOf::Structure) {
                let cost = self.modified_build_cost_supplies(
                    id,
                    &obj.template_name,
                    obj.thing.template.build_cost.supplies,
                ) as i32;
                if cost > highest_cost {
                    highest_cost = cost;
                    fallback = Some(obj.get_position());
                }
            }
        }
        fallback
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
        self.players.get(&id).map(|p| p.house_color_rgb())
    }

    /// C++ Object::getIndicatorColor → controlling Player::getPlayerColor.
    /// Black / missing owner is not a house color (Create_Render_Obj reallycolor).
    #[inline]
    pub fn player_house_color_rgba(&self, owner_player_id: Option<u32>) -> Option<[f32; 4]> {
        let (r, g, b) = self.player_color_rgb(owner_player_id?)?;
        if r == 0 && g == 0 && b == 0 {
            return None;
        }
        Some([r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0])
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
        // C++ Player.cpp:1240-1304 `doFindSpecialPowerSourceObject` /
        // `doCountSpecialPowersReady` refuse UNDER_CONSTRUCTION and SOLD.
        // `is_disabled()` already covers UC, but sell clears UC and keeps
        // the object alive — sold must be an explicit fire skip.
        if obj.status.under_construction || obj.status.sold {
            return false;
        }
        // C++ SpecialPowerModule::doSpecialPower / isReady: disabled objects cannot fire.
        // Covers underpowered POWERED SWs (PUC/Nuke), EMP, hacked, unmanned, etc.
        if obj.is_disabled() {
            return false;
        }
        // C++ SpecialPower.cpp:308 — no any-unit fallback when the object
        // does not carry this SpecialPowerModule (Frenzy/CashHack included).
        if !object_has_special_power_module(obj, power) {
            return false;
        }
        let parsed_module = obj.thing.template.special_power_module_for_command(power);
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
            // C++ SpecialPowerModule.cpp:278/323 getControllingPlayer, not first
            // same-faction slot. Mirror 1v1 USA/USA must not share science.
            match self.controlling_player_for_special_power(obj) {
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
            // C++ doSpecialPower refuses a paused module independently of
            // SharedNSync; isReady only compares the player timer.
            if obj.is_special_power_countdown_paused(power) {
                return false;
            }
            // C++ getReadyFrame via Player::getOrStartSpecialPowerReadyFrame.
            if let Some(player) = self.controlling_player_for_special_power(obj) {
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
        let (owner_id, team, parsed_module) = match self.host_object(object_id) {
            Some(o) => (
                self.player_owner_for_host_object(o),
                o.team,
                o.thing
                    .template
                    .special_power_module_for_command(power)
                    .cloned(),
            ),
            None => return false,
        };
        // Cooldown-tracked objects without a parsed module record consume via
        // the residual reload table below (C++ SpecialPowerModule::isReady /
        // startPowerRecharge read the live timer, not a parsed INI record).
        // Only objects with no module evidence at all fail closed — and that
        // case already returned false inside is_special_power_ready_for.
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
            if let Some(pid) = owner_id {
                if let Some(player) = self.get_player_mut(pid) {
                    player.reset_shared_special_power_timer(power, reload);
                }
            }
            // Mirror onto living objects owned by the same controlling player.
            // C++ SharedNSync is per-Player, not per-faction.
            for obj in self.objects.values_mut() {
                let same_controller = match owner_id {
                    Some(want) => {
                        obj.owner_player_id == Some(want)
                            || (obj.owner_player_id.is_none() && obj.team == team)
                    }
                    None => obj.team == team && obj.owner_player_id.is_none(),
                };
                if !same_controller || !obj.is_alive() {
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

    /// C++ `Object::getControllingPlayer` residual for science / SharedNSync.
    fn controlling_player_for_special_power(&self, obj: &Object) -> Option<&Player> {
        let owner_id = self.player_owner_for_host_object(obj)?;
        self.players.get(&owner_id)
    }

    /// C++ `SpecialPowerModule::aboutToDoSpecialPower` /
    /// `SpecialPowerCompletionDie::onDie` ScriptEngine notify residual.
    pub fn notify_script_engine_special_power_event(
        &self,
        source_object: ObjectId,
        power: &crate::command_system::SpecialPowerType,
        triggered: bool,
        completed: bool,
    ) {
        if !triggered && !completed {
            return;
        }
        let player_index = self
            .host_object(source_object)
            .and_then(|obj| self.player_owner_for_host_object(obj))
            .unwrap_or(0) as usize;
        let power_name =
            crate::game_logic::host_special_power_enum_residual::special_power_ini_template_name(
                power,
            );
        let _ = gamelogic::scripting::engine::with_script_engine_mut(|engine| {
            if triggered {
                engine.notify_of_triggered_special_power(player_index, power_name, source_object.0);
            }
            if completed {
                engine.notify_of_completed_special_power(player_index, power_name, source_object.0);
            }
        });
    }

    /// C++ `ScriptActions::doNamedStopSpecialPowerCountdown` residual.
    pub fn script_pause_special_power_countdown(
        &mut self,
        object_id: ObjectId,
        power: &crate::command_system::SpecialPowerType,
        pause: bool,
    ) -> bool {
        let Some(obj) = self.host_object_mut(object_id) else {
            return false;
        };
        obj.pause_special_power_countdown(power, pause);
        true
    }

    /// C++ `ScriptActions::doNamedSetSpecialPowerCountdown` residual.
    pub fn script_set_special_power_countdown(
        &mut self,
        object_id: ObjectId,
        power: &crate::command_system::SpecialPowerType,
        seconds: i32,
    ) -> bool {
        let Some(obj) = self.host_object_mut(object_id) else {
            return false;
        };
        obj.set_special_power_ready_seconds(power, seconds.max(0) as f32);
        true
    }

    /// C++ `ScriptActions::doNamedAddSpecialPowerCountdown` residual.
    pub fn script_add_special_power_countdown(
        &mut self,
        object_id: ObjectId,
        power: &crate::command_system::SpecialPowerType,
        seconds: i32,
    ) -> bool {
        let Some(obj) = self.host_object_mut(object_id) else {
            return false;
        };
        let next = obj.special_power_countdown_seconds(power) + seconds as f32;
        obj.set_special_power_ready_seconds(power, next.max(0.0));
        true
    }

    /// Resolve a host object by script unit name (`Object::name` or tracker).
    pub fn host_object_id_by_script_name(&self, unit_name: &str) -> Option<ObjectId> {
        if unit_name.is_empty() {
            return None;
        }
        if let Some((id, _)) = self
            .objects
            .iter()
            .find(|(_, obj)| !obj.name.is_empty() && obj.name.eq_ignore_ascii_case(unit_name))
        {
            return Some(*id);
        }
        let tracker = gamelogic::scripting::engine::get_named_object_tracker();
        tracker
            .get_object_id(unit_name)
            .ok()
            .flatten()
            .map(ObjectId)
    }

    /// Apply NAMED_STOP/START/SET/ADD_SPECIAL_POWER_COUNTDOWN to a named host object.
    pub fn script_named_special_power_countdown(
        &mut self,
        unit_name: &str,
        power_name: &str,
        op: crate::game_logic::NamedSpecialPowerCountdownOp,
        seconds: i32,
    ) -> bool {
        use crate::command_system::special_power_type_from_template_name;
        use crate::game_logic::NamedSpecialPowerCountdownOp;
        let Some(object_id) = self.host_object_id_by_script_name(unit_name) else {
            return false;
        };
        let Some(power) = special_power_type_from_template_name(power_name) else {
            return false;
        };
        match op {
            NamedSpecialPowerCountdownOp::Stop => {
                self.script_pause_special_power_countdown(object_id, &power, true)
            }
            NamedSpecialPowerCountdownOp::Start => {
                self.script_pause_special_power_countdown(object_id, &power, false)
            }
            NamedSpecialPowerCountdownOp::Set => {
                self.script_set_special_power_countdown(object_id, &power, seconds)
            }
            NamedSpecialPowerCountdownOp::Add => {
                self.script_add_special_power_countdown(object_id, &power, seconds)
            }
        }
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
        let mut ready_events: Vec<(u32, String)> = Vec::new();
        for player in self.players.values_mut() {
            let player_id = player.id;
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
                ready_events.push((player_id, template.to_string()));
            }
        }
        for (player_id, name) in ready_events {
            self.try_eva_superweapon_ready_for_player(player_id, &name);
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
        {
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
        // C++ Player::addScience → SpecialPowerModule::onSpecialPowerCreation.
        // CashBountyPower is the only setter; no palace module ⇒ no bounty.
        let _ = self.apply_cash_bounty_from_palace_modules(player_id, Some(science_name));
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
        let Some(source) = self.objects.get(&source_id) else {
            return false;
        };
        let Some(metadata) = source.thing.template.hacker_disable_building.as_ref() else {
            return false;
        };
        let power = metadata.command_power();
        if !metadata.update_module_starts_attack
            || metadata.scripted_special_power_only
            || !source.is_alive()
            || source.is_disabled()
            || source.is_special_power_countdown_paused(&power)
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
            return self
                .players
                .get(&owner_id)
                .is_some_and(|player| player.is_shared_special_power_ready(&power));
        }
        source.is_special_power_ready(&power)
    }

    /// Start the exact parsed HDB reload at C++
    /// `SpecialAbilityUpdate::startPreparation`, not at click time.
    pub fn consume_hacker_disable_building_charge(&mut self, source_id: ObjectId) -> bool {
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
        let power = metadata.command_power();
        if metadata.shared_n_sync {
            let Some(owner_id) = owner_id else {
                return false;
            };
            let Some(player) = self.players.get_mut(&owner_id) else {
                return false;
            };
            player.reset_shared_special_power_timer(
                &power,
                metadata.reload_time_frames as f32 / 30.0,
            );
        } else if let Some(source) = self.objects.get_mut(&source_id) {
            source.start_power_recharge_with_frames(&power, metadata.reload_time_frames);
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

        // C++ ActionManager SPECIAL_HACKER_DISABLE_BUILDING reads
        // `obj->getRelationship(target)` (Object.cpp:1548-1568), which falls
        // through to the controlling players' team relationship: two living
        // players on different non-neutral teams are ENEMIES even when the
        // lobby carries no explicit playerEnemies row. A strict player-map
        // NEUTRAL must not hard-reject that default hostility.
        let mut relation = match (source.owner_player_id, target.owner_player_id) {
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
        if relation == Relationship::Neutral
            && source.team != Team::Neutral
            && target.team != Team::Neutral
            && source.team != target.team
        {
            relation = Relationship::Enemies;
        }
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
        let mut satellite_hack_activate = None;
        let mut attach_slave_drone = None;

        if let Some(obj) = self.objects.get_mut(&object_id) {
            // C++ UpgradeMux::isAlreadyUpgraded — first give only.
            // C++ wouldUpgrade ConflictsWith: leftover ObjectCreationUpgrade already
            // matches; host objects skip OCL when another drone tag is complete.
            let first_give = !obj.has_upgrade_tag(upgrade);
            let drone_conflicts =
                crate::game_logic::host_slave_drones::slave_drone_conflicts_with_owned(
                    upgrade,
                    obj.applied_upgrades.iter().map(String::as_str),
                );
            obj.apply_upgrade_tag(upgrade);
            if first_give {
                if !drone_conflicts {
                    attach_slave_drone =
                        crate::game_logic::host_slave_drones::SlaveDroneKind::from_upgrade_name(
                            upgrade,
                        );
                }
                if let Some(spec) =
                    crate::game_logic::host_satellite_hack::satellite_hack_spy_spec(upgrade)
                {
                    if crate::game_logic::host_satellite_hack::object_authors_spy_vision_update(
                        &obj.template_name,
                    ) || obj.is_kind_of(KindOf::FSInternetCenter)
                    {
                        satellite_hack_activate = Some(spec);
                    }
                }
            }
            // C++ StatusBitsUpgrade::upgradeImplementation — INI StatusToSet/Clear.
            {
                let pairs =
                    crate::game_logic::host_status_bits_upgrade::collect_status_bits_for_upgrade(
                        upgrade,
                        &obj.template_name,
                    );
                for (set, clear) in &pairs {
                    let set_refs: Vec<&str> = set.iter().map(String::as_str).collect();
                    let clear_refs: Vec<&str> = clear.iter().map(String::as_str).collect();
                    let (set_c, clear_c) =
                        obj.apply_status_bits_upgrade_masks(&set_refs, &clear_refs);
                    self.status_bits_upgrade_reg.record_apply(set_c, clear_c);
                }
            }

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
            // C++ LocomotorSetUpgrade::upgradeImplementation only — not a
            // name-sniff of Heroic rank or AutoLoader research.
            if crate::game_logic::host_upgrade_module_residuals::apply_locomotor_set_upgrade(
                obj, upgrade,
            ) {
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

        // C++ SpyVisionUpdate::upgradeImplementation — activate on upgrade.
        if let Some(spec) = satellite_hack_activate {
            let _ = self.activate_satellite_hack_spy_vision(object_id, spec);
        }
        // C++ ObjectCreationUpgrade::upgradeImplementation — OCL on this vehicle
        // (Battle/Scout/Hellfire). Leftover module already matches C++; host
        // objects have no leftover modules, so spawn via leftover OCL store
        // (or residual attach). Object-scoped complete muxes only the producer.
        if let Some(kind) = attach_slave_drone {
            self.apply_object_creation_upgrade_ocl(object_id, kind);
        }
        if installed_gattling {
            self.overlord_addons.record_gattling_install();
        }
        if installed_propaganda {
            self.overlord_addons.record_propaganda_install();
        }
        if installed_gattling || installed_propaganda || installed_bunker {
            self.ensure_overlord_portable_addon_occupant(object_id);
        }

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
        // C++ RadarUpgrade::upgradeImplementation → RadarUpdate::extendRadar.
        if crate::game_logic::host_upgrades::HostUpgradeKind::from_name(upgrade)
            == crate::game_logic::host_upgrades::HostUpgradeKind::Radar
        {
            self.maybe_start_radar_extend(object_id);
        }
    }

    /// C++ `ObjectCreationUpgrade::upgradeImplementation` on this vehicle.
    /// Leftover OCL store first; residual attach if the list is not loaded.
    fn apply_object_creation_upgrade_ocl(
        &mut self,
        object_id: ObjectId,
        kind: crate::game_logic::host_slave_drones::SlaveDroneKind,
    ) {
        use crate::game_logic::host_slave_drones::{SlaveDroneKind, drone_ocl_name};

        let ctx = self.objects.get(&object_id).map(|obj| {
            (
                obj.team,
                obj.get_position(),
                obj.get_orientation(),
                obj.experience.level,
                obj.movement.velocity,
            )
        });
        let Some((team, pos, orient, vet, vel)) = ctx else {
            return;
        };
        let spawned = self.execute_parsed_weapon_ocl_at(
            drone_ocl_name(kind),
            Some(object_id),
            team,
            vet,
            orient,
            vel,
            pos,
        );
        if spawned.is_empty() {
            let _ = self.residual_attach_slave_drone(object_id, kind);
            return;
        }
        // C++ SlavedUpdate startSlavedEffects + UpgradeDie on the OCL spawn.
        for drone_id in spawned {
            if let Some(drone) = self.objects.get_mut(&drone_id) {
                drone.producer_id = Some(object_id);
                drone.set_status_unselectable(true);
                if drone.upgrade_die.is_none() {
                    drone.install_upgrade_die(kind.upgrade_name());
                }
            }
        }
        match kind {
            SlaveDroneKind::Scout => {
                self.scout_drone_residual_attaches =
                    self.scout_drone_residual_attaches.saturating_add(1);
            }
            SlaveDroneKind::Hellfire => {
                self.hellfire_drone_residual_attaches =
                    self.hellfire_drone_residual_attaches.saturating_add(1);
            }
            SlaveDroneKind::Battle => {
                self.battle_drone_residual_attaches =
                    self.battle_drone_residual_attaches.saturating_add(1);
            }
        }
    }
}

#[cfg(test)]
mod sides_relationship_tests {
    use super::*;
    use gamelogic::common::Relationship;

    #[test]
    fn distinct_players_default_neutral_not_enemies() {
        let mut logic = GameLogic::new();
        logic.add_player(Player::new(0, Team::USA, "A", true));
        logic.add_player(Player::new(1, Team::GLA, "B", false));
        assert_eq!(logic.player_relationship(0, 1), Relationship::Neutral);
        assert_eq!(logic.player_relationship(1, 0), Relationship::Neutral);
        assert_eq!(logic.player_relationship(0, 0), Relationship::Allies);
    }

    #[test]
    fn map_allies_and_enemies_override_default() {
        let mut logic = GameLogic::new();
        let mut a = Player::new(0, Team::USA, "PlyrAmerica", true);
        let mut b = Player::new(1, Team::China, "PlyrChina", false);
        let mut c = Player::new(2, Team::GLA, "PlyrGLA", false);
        a.set_map_relationship(1, Relationship::Allies);
        a.set_map_relationship(2, Relationship::Enemies);
        logic.add_player(a);
        logic.add_player(b);
        logic.add_player(c);
        assert_eq!(logic.player_relationship(0, 1), Relationship::Allies);
        assert_eq!(logic.player_relationship(0, 2), Relationship::Enemies);
    }

    #[test]
    fn skirmish_alliance_teams_still_enemies_when_assigned() {
        let mut logic = GameLogic::new();
        let mut a = Player::new(0, Team::USA, "A", true);
        let mut b = Player::new(1, Team::USA, "B", false);
        a.alliance_team = 1;
        b.alliance_team = 2;
        logic.add_player(a);
        logic.add_player(b);
        assert_eq!(logic.player_relationship(0, 1), Relationship::Enemies);
    }

    #[test]
    fn team_override_allies_one_named_team() {
        // C++ Player.cpp:542-554 team-id override before player relations.
        let mut logic = GameLogic::new();
        let mut usa = Player::new(0, Team::USA, "PlyrAmerica", true);
        let gla = Player::new(1, Team::GLA, "PlyrGLA", false);
        usa.set_map_relationship(1, Relationship::Enemies);
        usa.set_team_relationship_override("CivilianConvoy", Relationship::Allies);
        logic.add_player(usa);
        logic.add_player(gla);

        let mut tmpl = ThingTemplate::new("Ranger");
        tmpl.add_kind_of(KindOf::Infantry);
        let mut src = Object::new(tmpl.clone(), ObjectId(1), Team::USA);
        src.owner_player_id = Some(0);
        let mut convoy = Object::new(tmpl.clone(), ObjectId(2), Team::GLA);
        convoy.owner_player_id = Some(1);
        convoy.team_instance_name = "CivilianConvoy".into();
        let mut other = Object::new(tmpl, ObjectId(3), Team::GLA);
        other.owner_player_id = Some(1);

        assert_eq!(
            logic.object_relationship(&src, &convoy),
            Relationship::Allies
        );
        assert_eq!(
            logic.object_relationship(&src, &other),
            Relationship::Enemies
        );
    }

    #[test]
    fn team_set_override_relation_to_team_beats_player_map() {
        // C++ Team::getRelationship team-id override before player relations.
        let mut logic = GameLogic::new();
        let mut usa = Player::new(0, Team::USA, "PlyrAmerica", true);
        let gla = Player::new(1, Team::GLA, "PlyrGLA", false);
        usa.set_map_relationship(1, Relationship::Enemies);
        usa.set_team_instance_team_override("RangerTeam", "CivilianConvoy", Relationship::Allies);
        logic.add_player(usa);
        logic.add_player(gla);

        let mut tmpl = ThingTemplate::new("Ranger");
        tmpl.add_kind_of(KindOf::Infantry);
        let mut src = Object::new(tmpl.clone(), ObjectId(1), Team::USA);
        src.owner_player_id = Some(0);
        src.team_instance_name = "RangerTeam".into();
        let mut convoy = Object::new(tmpl.clone(), ObjectId(2), Team::GLA);
        convoy.owner_player_id = Some(1);
        convoy.team_instance_name = "CivilianConvoy".into();
        let mut other = Object::new(tmpl, ObjectId(3), Team::GLA);
        other.owner_player_id = Some(1);

        assert_eq!(
            logic.object_relationship(&src, &convoy),
            Relationship::Allies
        );
        assert_eq!(
            logic.object_relationship(&src, &other),
            Relationship::Enemies
        );
    }

    #[test]
    fn undetected_defector_overrides_owner_map() {
        // C++ Object::getRelationship: self undetected → Neutral, that → Allies.
        let mut logic = GameLogic::new();
        let mut usa = Player::new(0, Team::USA, "PlyrAmerica", true);
        let gla = Player::new(1, Team::GLA, "PlyrGLA", false);
        usa.set_map_relationship(1, Relationship::Enemies);
        logic.add_player(usa);
        logic.add_player(gla);

        let mut tmpl = ThingTemplate::new("Ranger");
        tmpl.add_kind_of(KindOf::Infantry);
        let mut src = Object::new(tmpl.clone(), ObjectId(10), Team::USA);
        src.owner_player_id = Some(0);
        let mut tgt = Object::new(tmpl, ObjectId(11), Team::GLA);
        tgt.owner_player_id = Some(1);

        assert_eq!(logic.object_relationship(&src, &tgt), Relationship::Enemies);

        src.begin_undetected_defection(0, 30, false);
        assert_eq!(
            logic.object_relationship(&src, &tgt),
            Relationship::Neutral,
            "undetected defector must not auto-acquire old enemies"
        );

        src.blow_defector_cover();
        tgt.begin_undetected_defection(0, 30, false);
        assert_eq!(
            logic.object_relationship(&src, &tgt),
            Relationship::Allies,
            "teammates must treat flashing defector as own"
        );
    }
}

#[cfg(test)]
mod can_use_special_power_module_gate_tests {
    use super::*;
    use crate::command_system::SpecialPowerType;

    fn test_module(
        power: SpecialPowerType,
        template: &str,
        kind: SpecialPowerModuleKind,
    ) -> SpecialPowerModuleMetadata {
        SpecialPowerModuleMetadata {
            source_index: 0,
            module_tag: Some("ModuleTag_SpecialPower".into()),
            module_kind: kind,
            special_power_template: template.into(),
            special_power_template_id: 1,
            command_power: Some(power),
            reload_time_frames: 0,
            required_science: None,
            public_timer: false,
            shared_n_sync: false,
            shortcut_power: false,
            update_module_starts_attack: false,
            starts_paused: false,
            scripted_special_power_only: false,
        }
    }

    /// C++ `SpecialPowerStore::canUseSpecialPower` (`SpecialPower.cpp:308`)
    /// requires `getSpecialPowerModule`. A tank with no Frenzy/CashHack
    /// module must not be ready; a command center that carries the modules is.
    #[test]
    fn frenzy_and_cash_hack_require_special_power_module() {
        let mut logic = GameLogic::new();
        logic.add_player(Player::new(0, Team::China, "China", true));

        let mut tank = ThingTemplate::new("Hq4vpduTank");
        tank.set_health(100.0);
        logic.templates.insert("Hq4vpduTank".into(), tank);
        let tank_id = logic
            .create_object("Hq4vpduTank", Team::China, glam::Vec3::ZERO)
            .expect("tank");

        assert!(
            !logic.is_special_power_ready_for(tank_id, &SpecialPowerType::Frenzy),
            "any-unit Frenzy without SpecialPowerModule must fail closed"
        );
        assert!(
            !logic.is_special_power_ready_for(tank_id, &SpecialPowerType::CashHack),
            "any-unit CashHack without SpecialPowerModule must fail closed"
        );
        assert!(
            !logic.is_special_power_ready_for(tank_id, &SpecialPowerType::EarlyFrenzy),
            "EarlyFrenzy without module must fail closed"
        );
        assert!(!logic.consume_special_power_charge_for(tank_id, &SpecialPowerType::Frenzy));
        assert!(!logic.consume_special_power_charge_for(tank_id, &SpecialPowerType::CashHack));

        let mut cc = ThingTemplate::new("Hq4vpduCC");
        cc.set_health(5000.0);
        cc.special_power_modules.push(test_module(
            SpecialPowerType::Frenzy,
            "SuperweaponFrenzy",
            SpecialPowerModuleKind::OclSpecialPower,
        ));
        cc.special_power_modules.push(test_module(
            SpecialPowerType::CashHack,
            "SuperweaponCashHack",
            SpecialPowerModuleKind::CashHackSpecialPower,
        ));
        logic.templates.insert("Hq4vpduCC".into(), cc);
        let cc_id = logic
            .create_object("Hq4vpduCC", Team::China, glam::Vec3::new(10.0, 0.0, 0.0))
            .expect("cc");

        assert!(
            logic.is_special_power_ready_for(cc_id, &SpecialPowerType::Frenzy),
            "object that carries Frenzy SpecialPowerModule must be ready"
        );
        assert!(
            logic.is_special_power_ready_for(cc_id, &SpecialPowerType::CashHack),
            "object that carries CashHack SpecialPowerModule must be ready"
        );
        assert!(
            !logic.is_special_power_ready_for(cc_id, &SpecialPowerType::EarlyFrenzy),
            "CC without EarlyFrenzy module must not inherit Frenzy"
        );
    }

    /// C++ Player.cpp:1240-1304 sold / under-construction sources cannot fire.
    #[test]
    fn sold_and_under_construction_cannot_fire_special_power() {
        let mut logic = GameLogic::new();
        logic.add_player(Player::new(0, Team::USA, "USA", true));

        let mut puc = ThingTemplate::new("Hq6mi3iPuc");
        puc.set_health(4000.0);
        puc.special_power_modules.push(test_module(
            SpecialPowerType::ParticleCannon,
            "SuperweaponParticleUplinkCannon",
            SpecialPowerModuleKind::OclSpecialPower,
        ));
        logic.templates.insert("Hq6mi3iPuc".into(), puc);
        let id = logic
            .create_object("Hq6mi3iPuc", Team::USA, glam::Vec3::ZERO)
            .expect("puc");
        if let Some(o) = logic.host_object_mut(id) {
            o.status.under_construction = false;
            o.status.sold = false;
            o.construction_percent = 1.0;
        }
        assert!(
            logic.is_special_power_ready_for(id, &SpecialPowerType::ParticleCannon),
            "constructed unsold PUC must be ready"
        );

        if let Some(o) = logic.host_object_mut(id) {
            o.status.sold = true;
            o.construction_percent = 0.999;
            o.status.under_construction = false;
        }
        assert!(
            !logic.is_special_power_ready_for(id, &SpecialPowerType::ParticleCannon),
            "C++ Player iterators skip OBJECT_STATUS_SOLD"
        );
        assert!(
            !logic.consume_special_power_charge_for(id, &SpecialPowerType::ParticleCannon),
            "sold PUC must not consume/recharge"
        );

        if let Some(o) = logic.host_object_mut(id) {
            o.status.sold = false;
            o.status.under_construction = true;
            o.construction_percent = 0.4;
        }
        assert!(
            !logic.is_special_power_ready_for(id, &SpecialPowerType::ParticleCannon),
            "C++ Player iterators skip OBJECT_STATUS_UNDER_CONSTRUCTION"
        );
    }
}

#[cfg(test)]
mod human_enter_fog_gate_tests {
    use super::*;
    use crate::game_logic::{ContainAdmission, ContainModuleKind, ContainModuleMetadata};
    use gamelogic::common::ObjectShroudStatus;
    use gamelogic::system::shroud_manager::get_shroud_manager;

    fn garrison_template(name: &str) -> ThingTemplate {
        let mut t = ThingTemplate::new(name);
        t.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .set_health(1000.0);
        t.contain_module = ContainModuleMetadata {
            kind: ContainModuleKind::Garrison,
            slots: Some(5),
            admission: ContainAdmission::InfantryOnly,
            is_enclosing_container: true,
            ..ContainModuleMetadata::default()
        };
        t
    }

    fn infantry_template(name: &str) -> ThingTemplate {
        let mut t = ThingTemplate::new(name);
        t.add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .set_health(120.0);
        t.transport_slot_count = Some(1);
        t
    }

    fn set_target_shroud(player_id: u32, object_id: ObjectId, status: ObjectShroudStatus) {
        let mut mgr = get_shroud_manager().lock().expect("shroud");
        mgr.set_host_object_shroud_status(player_id, object_id.0, status);
    }

    /// C++ `isObjectShroudedForAction`: human + not FromScript + shroud >= Fogged.
    #[test]
    fn human_enter_rejects_fogged_or_shrouded_container() {
        let _lock = crate::fow_rendering::shroud_test_isolation_lock()
            .lock()
            .unwrap_or_else(|e| e.into_inner());

        let mut logic = GameLogic::new();
        logic.add_player(Player::new(1, Team::USA, "HqSb42vHuman", true));
        logic.add_player(Player::new(2, Team::China, "HqSb42vAi", false));
        logic
            .templates
            .insert("HqSb42vBunker".into(), garrison_template("HqSb42vBunker"));
        logic
            .templates
            .insert("HqSb42vRanger".into(), infantry_template("HqSb42vRanger"));

        let bunker = logic
            .create_object_for_player("HqSb42vBunker", 1, glam::Vec3::ZERO)
            .expect("bunker");
        let ranger = logic
            .create_object_for_player("HqSb42vRanger", 1, glam::Vec3::new(4.0, 0.0, 0.0))
            .expect("ranger");
        let ai_bunker = logic
            .create_object_for_player("HqSb42vBunker", 2, glam::Vec3::new(40.0, 0.0, 0.0))
            .expect("ai bunker");
        let ai_ranger = logic
            .create_object_for_player("HqSb42vRanger", 2, glam::Vec3::new(44.0, 0.0, 0.0))
            .expect("ai ranger");

        assert!(
            logic.can_unit_enter_normal_target(ranger, bunker),
            "uninitialized shroud must fail-open like missing PartitionData"
        );

        set_target_shroud(1, bunker, ObjectShroudStatus::Clear);
        assert!(
            logic.can_unit_enter_normal_target(ranger, bunker),
            "CLEAR container stays enterable"
        );
        set_target_shroud(1, bunker, ObjectShroudStatus::PartialClear);
        assert!(
            logic.can_unit_enter_normal_target(ranger, bunker),
            "PARTIAL_CLEAR is below Fogged"
        );

        set_target_shroud(1, bunker, ObjectShroudStatus::Fogged);
        assert!(
            !logic.can_unit_enter_normal_target(ranger, bunker),
            "human Enter must reject FOGGED garrison"
        );
        set_target_shroud(1, bunker, ObjectShroudStatus::Shrouded);
        assert!(
            !logic.can_unit_enter_normal_target(ranger, bunker),
            "human Enter must reject SHROUDED garrison"
        );

        set_target_shroud(2, ai_bunker, ObjectShroudStatus::Fogged);
        assert!(
            logic.can_unit_enter_normal_target(ai_ranger, ai_bunker),
            "computer player skips the human fog gate"
        );

        if let Some(b) = logic.host_object_mut(bunker) {
            b.thing.template.always_visible = true;
        }
        set_target_shroud(1, bunker, ObjectShroudStatus::Fogged);
        assert!(
            logic.can_unit_enter_normal_target(ranger, bunker),
            "AlwaysVisible getShroudedStatus is CLEAR"
        );

        set_target_shroud(1, bunker, ObjectShroudStatus::Clear);
        set_target_shroud(2, ai_bunker, ObjectShroudStatus::Clear);
    }
}
