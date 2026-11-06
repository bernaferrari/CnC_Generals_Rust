use super::*;

impl GameLogic {
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
        // Player::getRelationship reads the stored team/player relations even
        // after defeat; activity is a separate predicate at callers that need it.
        if let Some(rel) = source.map_relationship(target_player_id) {
            return rel;
        }
        if source_player_id == target_player_id {
            return Relationship::Allies;
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
        Self::object_relationship_from_identities(
            &self.team_factory,
            &self.players,
            source.owner_player_id,
            source.team_instance_id,
            &source.team_instance_name,
            target.owner_player_id,
            target.team_instance_id,
            &target.team_instance_name,
        )
    }

    /// CPP ScriptConditions1029 -> Object1548 -> Team1447 -> Player542.
    /// Sighting observes explicit relationships even for defeated players;
    /// it cannot use the combat helper's ambient Core player-list fallback.
    pub(crate) fn script_sighting_relationship(
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
        // CPP Player763 defaults to "team" + authored playerName. Display
        // labels are not team identities when map metadata supplies a name.
        let team_name = |object: &Object| {
            if !object.team_instance_name.is_empty() {
                return object.team_instance_name.clone();
            }
            object
                .owner_player_id
                .and_then(|id| self.players.get(&id))
                .map(|player| {
                    let name = if player.map_side.map_player_name.is_empty() {
                        &player.name
                    } else {
                        &player.map_side.map_player_name
                    };
                    format!("team{name}")
                })
                .unwrap_or_default()
        };
        let source_team = team_name(source);
        let target_team = team_name(target);
        if let Some(relationship) = owned_source_team_override(
            &self.team_factory,
            source.team_instance_id,
            &source_team,
            target.team_instance_id,
            &target_team,
            target.owner_player_id,
        ) {
            return relationship;
        }
        let Some(player) = source.owner_player_id.and_then(|id| self.players.get(&id)) else {
            return Relationship::Neutral;
        };
        Self::team_id_for_relationship(&self.team_factory, target.team_instance_id, &target_team)
            .and_then(|id| player.team_relationship_override(id))
            .or_else(|| {
                target
                    .owner_player_id
                    .and_then(|id| player.map_relationship(id))
            })
            .unwrap_or(Relationship::Neutral)
    }

    /// C++ `Object::getRelationship` from frozen owner ids (Weapon.cpp:1360).
    /// `source` is the viewer (`curVictim->getRelationship(source)` when the
    /// first pair is the victim).
    pub fn object_relationship_from_owners(
        team_factory: &gamelogic::team::TeamFactoryHandle,
        players: &std::collections::HashMap<u32, crate::game_logic::Player>,
        source_owner: Option<u32>,
        source_team_instance: &str,
        target_owner: Option<u32>,
        target_team_instance: &str,
    ) -> gamelogic::common::Relationship {
        Self::object_relationship_from_identities(
            team_factory,
            players,
            source_owner,
            None,
            source_team_instance,
            target_owner,
            None,
            target_team_instance,
        )
    }

    /// C++ `charge->getControllingPlayer()->getRelationship(victim->getTeam())`
    /// for a receiving Team identity. This deliberately omits both the source
    /// Object's Team overrides and the trigger Object's defector override.
    pub(crate) fn object_relationship_from_owner_to_team_identity(
        team_factory: &gamelogic::team::TeamFactoryHandle,
        players: &std::collections::HashMap<u32, crate::game_logic::Player>,
        source_owner: Option<u32>,
        target_owner: Option<u32>,
        target_team_id: Option<u32>,
        target_team_instance: &str,
    ) -> gamelogic::common::Relationship {
        Self::object_relationship_from_identities(
            team_factory,
            players,
            source_owner,
            None,
            "",
            target_owner,
            target_team_id,
            target_team_instance,
        )
    }

    pub(crate) fn object_relationship_from_identities(
        team_factory: &gamelogic::team::TeamFactoryHandle,
        players: &std::collections::HashMap<u32, crate::game_logic::Player>,
        source_owner: Option<u32>,
        source_team_id: Option<u32>,
        source_team_instance: &str,
        target_owner: Option<u32>,
        target_team_id: Option<u32>,
        target_team_instance: &str,
    ) -> gamelogic::common::Relationship {
        if source_team_id.is_some() || !source_team_instance.is_empty() {
            if let Some(rel) = owned_source_team_override(
                team_factory,
                source_team_id,
                source_team_instance,
                target_team_id,
                target_team_instance,
                target_owner,
            ) {
                return rel;
            }
        }

        use gamelogic::common::Relationship;

        if let Some(rel) = source_owner
            .and_then(|id| players.get(&id))
            .and_then(|player| {
                Self::team_id_for_relationship(team_factory, target_team_id, target_team_instance)
                    .and_then(|id| player.team_relationship_override(id))
            })
        {
            return rel;
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
                .thing()
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
        let is_airborne_locomotor = vehicle.selected_locomotor_is_airborne();
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
            && vehicle.is_trainable()
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
            && !infantry.is_kind_of(crate::game_logic::KindOf::RejectUnmanned)
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
        if target.is_cave_style_container() {
            // CaveContain stores the roster in its tracker; it never calls
            // OpenContain::addToContainList's stealth counter update.
            return (0, self.cave_system.contain_count(target.cave_index));
        }
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

    /// C++ Object::getTransportSlotCount resolves special zero-slot
    /// containers through their passengers. Resolve IDs in this session and
    /// reject broken or cyclic containment rather than recursing indefinitely.
    pub(in crate::game_logic) fn effective_transport_slot_count(
        &self,
        object_id: ObjectId,
    ) -> Option<usize> {
        let object = self.host_object(object_id)?;
        if object.thing().template.contain_module.kind
            != crate::game_logic::ContainModuleKind::Parachute
        {
            return Some(object.transport_slot_count());
        }

        let mut pending = vec![(object_id, false)];
        let mut active = std::collections::HashSet::new();
        let mut total = 0usize;
        while let Some((id, leaving)) = pending.pop() {
            if leaving {
                active.remove(&id);
                continue;
            }
            let object = self.host_object(id)?;
            if object.thing().template.contain_module.kind
                == crate::game_logic::ContainModuleKind::Parachute
            {
                if !active.insert(id) {
                    return None;
                }
                pending.push((id, true));
                pending.extend(
                    object
                        .contained_units()
                        .into_iter()
                        .map(|child| (child, false)),
                );
            } else {
                total = total.checked_add(object.transport_slot_count())?;
            }
        }
        Some(total)
    }

    /// Remaining normal-Enter capacity before adding `unit_id`.
    ///
    /// Transports consume rider slots; garrisons and tunnels count bodies.
    /// Missing or invalid passengers are not silently treated as one slot.
    pub fn normal_enter_available_capacity_for(
        &self,
        unit_id: ObjectId,
        target_id: ObjectId,
    ) -> Option<usize> {
        let raw_target = target_id;
        let target_id = self.redirected_containment_owner(target_id);
        if raw_target == target_id
            && self
                .host_object(raw_target)
                .is_some_and(|object| object.is_overlord_style_container())
        {
            return None;
        }
        if unit_id == target_id {
            return None;
        }
        let unit = self.host_object(unit_id)?;
        let target = self.host_object(target_id)?;
        if !target.can_contain() || !target.supports_normal_enter() {
            return None;
        }

        if target.is_cave_style_container() {
            return Some(usize::from(
                self.cave_system.has_capacity(target.cave_index),
            ));
        }

        if target.is_tunnel_network_style_container() {
            // TunnelTracker::isValidContainerFor checks the shared count even
            // when this unit is already in the pool or a local roster is stale.
            return Some(usize::from(
                self.tunnel_network.has_capacity(target.tunnel_system_key()),
            ));
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

        if target.normal_enter_uses_transport_slots() {
            let capacity = target.transport_capacity();
            if capacity == 0 {
                return None;
            }
            let mut slots_in_use = 0usize;
            for occupant_id in target.contained_units() {
                let slots = self.effective_transport_slot_count(occupant_id)?;
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
        let shroud_manager = self.world_services.shroud();
        let Ok(shroud) = shroud_manager.lock() else {
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
        self.can_unit_enter_target_with_capacity(unit_id, target_id, true, false)
    }

    /// SCRIPT privateEnter asks DONT_CHECK_CAPACITY and bypasses display shroud.
    pub(crate) fn can_unit_enter_script_target(
        &self,
        unit_id: ObjectId,
        target_id: ObjectId,
    ) -> bool {
        self.can_unit_enter_target_with_capacity(unit_id, target_id, false, true)
    }

    pub(super) fn can_unit_enter_target_with_capacity(
        &self,
        unit_id: ObjectId,
        target_id: ObjectId,
        check_capacity: bool,
        from_script: bool,
    ) -> bool {
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
        if !from_script && self.is_object_shrouded_for_action(unit, target) {
            return false;
        }

        // Admission still observes the requested outer object. Redirection
        // changes the contain interface, not the ActionManager target gates.
        if !target.is_alive()
            || target.status.under_construction
            || target.status.sold
            || target.is_subdued_disabled()
            || target.status.effectively_dead
        {
            return false;
        }
        let raw_target = target_id;
        let target_id = self.redirected_containment_owner(target_id);
        if raw_target == target_id
            && self
                .host_object(raw_target)
                .is_some_and(|object| object.is_overlord_style_container())
        {
            return false;
        }
        if unit_id == target_id {
            return false;
        }
        let Some(target) = self.host_object(target_id) else {
            return false;
        };

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
            || ((check_capacity || target.normal_enter_uses_transport_slots())
                && unit.transport_slot_count() == 0)
            || unit.is_kind_of(KindOf::MobNexus)
            || target.status.effectively_dead
            || !target.can_contain()
            || !target.supports_normal_enter()
        {
            return false;
        }

        // C++ ActionManager.cpp:636-644 — HealContain is not a transport;
        // a unit at max health cannot enter barracks/hospital.
        if target
            .thing()
            .template
            .contain_module
            .kind
            .is_heal_contain()
            && unit.health.current >= unit.health.maximum
        {
            return false;
        }

        let unit_in_tunnel = self.tunnel_network.player_holding_unit(unit_id).is_some();
        if !unit.can_move() && !unit_in_tunnel {
            return false;
        }

        let is_cave = target.is_cave_style_container();
        let is_tunnel = target.is_tunnel_network_style_container();
        let relationship = self.normal_enter_relationship(unit, target);
        if !is_cave && !is_tunnel && !target.allows_normal_enter_for_relationship(relationship) {
            return false;
        }
        let same_controller = self.normal_enter_controller_matches(unit, target);
        if target.normal_enter_requires_exact_controller() && !same_controller {
            return false;
        }
        let mut skip_capacity_for_stealth_garrison = false;

        if is_cave {
            if unit.is_kind_of(KindOf::Aircraft)
                || (!same_controller
                    && (self.cave_system.contain_count(target.cave_index) > 0
                        || target.is_faction_structure()))
            {
                return false;
            }
        } else if is_tunnel {
            if unit.is_kind_of(KindOf::Aircraft)
                || (!same_controller
                    && (self
                        .tunnel_network
                        .contain_count(target.tunnel_system_key())
                        > 0
                        || target.is_faction_structure()))
            {
                return false;
            }
        } else {
            match target.normal_enter_admission() {
                crate::game_logic::ContainAdmission::AnyMobile => {}
                crate::game_logic::ContainAdmission::InfantryOnly => {
                    if !unit.is_kind_of(KindOf::Infantry) && !self.host_object_is_hero(unit) {
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
                .thing()
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
        if !check_capacity || skip_capacity_for_stealth_garrison {
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
            .registered_values()
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
}
