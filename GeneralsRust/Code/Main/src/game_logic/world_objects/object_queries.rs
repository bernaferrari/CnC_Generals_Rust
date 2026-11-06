//! Host objects `impl GameLogic` — `object_queries`.
//! find_object, players, session queries. Child of `world_objects` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;

impl GameLogic {
    /// Resolve a missing transitional name through this world's TeamFactory.
    /// An existing identity always wins, even when its Team has since gone.
    pub(crate) fn team_id_for_relationship(
        factory: &gamelogic::team::TeamFactoryHandle,
        identity: Option<u32>,
        name: &str,
    ) -> Option<u32> {
        identity.or_else(|| {
            if name.is_empty() {
                return None;
            }
            factory
                .with_read_without_flushing(|factory| {
                    factory
                        .find_team_instances(name)
                        .into_iter()
                        .next()
                        .map(|team| {
                            team.read()
                                .unwrap_or_else(|poisoned| poisoned.into_inner())
                                .get_id()
                        })
                })
                .unwrap_or_else(|poisoned| poisoned.into_inner())
        })
    }

    /// Build the immutable translator pick view from this world's canonical
    /// Object-list order. The snapshot carries only facts; HostObjectStore
    /// remains the sole mutable gameplay-object owner.
    pub fn translator_pick_object_view(
        &self,
    ) -> game_engine::common::message_stream::TranslatorPickObjectView {
        use game_engine::common::message_stream::{
            TranslatorPickCamera, TranslatorPickObjectFact, TranslatorPickObjectView,
        };
        use gamelogic::common::ObjectShroudStatus;

        let local_player = self.local_player_id();
        let camera = &self.session_view.camera;
        let (viewport_width, viewport_height) = camera.viewport.size();
        let (viewport_x, viewport_y) = camera.viewport.origin();
        let mut facts = Vec::with_capacity(self.objects.registered_len());
        let shroud = self.world_services.shroud().lock().ok();
        for (id, object) in self.objects.registered() {
            let position = object.get_position();
            let hidden_for_local_player = !object.get_template().always_visible
                && object.contained_by.is_none()
                && local_player
                    .and_then(|player| shroud.as_ref()?.get_host_object_shroud_status(player, id.0))
                    .is_some_and(|status| {
                        matches!(
                            status,
                            ObjectShroudStatus::Fogged
                                | ObjectShroudStatus::Shrouded
                                | ObjectShroudStatus::InvalidButPreviousValid
                        )
                    });
            facts.push(TranslatorPickObjectFact {
                object_id: id.0,
                position: [position.x, position.y, position.z],
                locally_controlled: local_player
                    .is_some_and(|player| object.owner_player_id == Some(player)),
                selectable: object.is_selectable() && object.is_kind_of(KindOf::Selectable),
                force_attackable: object.is_kind_of(KindOf::ForceAttackable),
                mine: object.is_kind_of(KindOf::Mine),
                shrubbery: object.is_kind_of(KindOf::Shrubbery),
                destroyed: object.status.destroyed,
                effectively_dead: object.status.effectively_dead,
                unselectable: object.status.unselectable,
                masked: object.status.masked,
                hidden_for_local_player,
            });
        }
        TranslatorPickObjectView {
            local_player_index: local_player.map_or(-1, |player| player as i32),
            objects: facts,
            native_drawables: Vec::new(),
            camera: Some(TranslatorPickCamera {
                view_matrix: camera.view_matrix.to_cols_array(),
                projection_matrix: camera.projection_matrix.to_cols_array(),
                camera_position: [
                    camera.camera_position.x,
                    camera.camera_position.y,
                    camera.camera_position.z,
                ],
                viewport_origin: [viewport_x, viewport_y],
                viewport_size: [viewport_width, viewport_height],
            }),
        }
    }
}

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
        .thing()
        .template
        .special_power_module_for_command(power)
        .is_some()
    {
        return true;
    }
    if obj
        .thing()
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
    ) && obj.thing().template.hacker_disable_building.is_some()
        // Cooldown-tracked residual evidence (see doc above).
        || obj.special_power_cooldowns.contains_key(power)
}

/// C++ `Team::getRelationship` team/player override maps written by
/// `TEAM_SET_OVERRIDE_RELATION_TO_TEAM` / `_TO_PLAYER`.
fn owned_source_team_override(
    team_factory: &gamelogic::team::TeamFactoryHandle,
    source_team_id: Option<u32>,
    source_team_name: &str,
    target_team_id: Option<u32>,
    target_team_name: &str,
    target_owner: Option<u32>,
) -> Option<gamelogic::common::Relationship> {
    // Live objects carry their exact Team IDs. Only older callers without an
    // identity may resolve by name; a missing ID must not select a peer Team.
    let (source, target) = team_factory
        .with_read_without_flushing(|factory| {
            let resolve = |id: Option<u32>, name: &str| match id {
                Some(id) => factory.find_team_by_id(id),
                None if !name.trim().is_empty() => {
                    factory.find_team_instances(name).into_iter().next()
                }
                None => None,
            };
            (
                resolve(source_team_id, source_team_name),
                resolve(target_team_id, target_team_name),
            )
        })
        .unwrap_or_else(|error| error.into_inner());
    let source = source?;
    let source_guard = source.read().unwrap_or_else(|error| error.into_inner());
    if let Some(target_arc) = target {
        let relationship = if std::sync::Arc::ptr_eq(&source, &target_arc) {
            source_guard.override_relationship_with_team(&source_guard)
        } else {
            let target_guard = target_arc.read().unwrap_or_else(|error| error.into_inner());
            source_guard.override_relationship_with_team(&target_guard)
        };
        if let Some(rel) = relationship {
            return Some(rel);
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
        &mut self,
        team: Team,
        from: Vec3,
    ) -> Option<ObjectId> {
        self.find_nearest_harvestable_supply_within(team, from, None, ObjectId(u32::MAX))
    }

    /// C++ `ResourceGatheringManager::findBestSupplyWarehouse` scan cap.
    /// `max_scan` is already AI-doubled via `warehouse_scan_distance`.
    /// C++ `computeRelativeCost` returns FLT_MAX when `!isClearToApproach`.
    pub(in super::super) fn find_nearest_harvestable_supply_within(
        &mut self,
        team: Team,
        from: Vec3,
        max_scan: Option<f32>,
        query_id: ObjectId,
    ) -> Option<ObjectId> {
        let _ = team; // supplies are neutral/shared residual
        let preferred = self
            .objects
            .get(&query_id)
            .filter(|collector| collector.thing().template.supply_truck_metadata.is_some())
            .and_then(|collector| collector.preferred_dock_id);
        if let Some(preferred) = preferred
            && self.supply_warehouse_available_for(preferred, query_id)
        {
            return Some(preferred);
        }
        let supply_truck_query = self
            .objects
            .get(&query_id)
            .is_some_and(|collector| collector.thing().template.supply_truck_metadata.is_some());
        // Snapshot object facts first so cancellation reconciliation can
        // mutably borrow this world without retaining an ObjectStore borrow.
        let candidates: Vec<_> = self
            .objects
            .registered()
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
                if obj.stored_resources.supplies == 0
                    && (obj.is_kind_of(KindOf::SupplySource)
                        || obj.is_kind_of(KindOf::Harvestable)
                        || obj.object_type == ObjectType::Supply)
                    && obj.thing().template.dock_kind == DockKind::SupplyWarehouse
                {
                    return None;
                }
                Some((
                    id,
                    obj.team,
                    obj.get_position(),
                    obj.status.under_construction,
                    obj.thing().template.dock_kind,
                ))
            })
            .collect();
        let candidates: Vec<_> = candidates
            .into_iter()
            .filter_map(|(id, team, position, under_construction, dock_kind)| {
                if dock_kind == DockKind::SupplyWarehouse {
                    if supply_truck_query {
                        if let Some(scan_limit) = max_scan {
                            // Keep the C++ strict comparison verbatim: NaN
                            // distances are not admitted by `distance < cap`.
                            if !(position.distance_squared(from) < scan_limit * scan_limit) {
                                return None;
                            }
                        }
                        if !self.supply_warehouse_available_for(id, query_id) {
                            return None;
                        }
                    } else {
                        self.sync_host_dock_queue_cancellations_for_dock(id);
                        if !self
                            .host_dock_approach_queues
                            .is_clear_to_approach(id, query_id)
                        {
                            return None;
                        }
                    }
                }
                Some(
                    crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                        id,
                        team,
                        position,
                        is_alive: true,
                        is_neutral: team == Team::Neutral,
                        under_construction,
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
            .registered()
            .filter_map(|(&obj_id, obj)| {
                if obj.team != team
                    || !obj.is_alive()
                    || !obj.is_constructed()
                    || self.player_owner_for_host_object(obj) != owner_player_id
                {
                    return None;
                }
                let on_manager = manager_ids.contains(&obj_id);
                let kind_fallback = obj.thing().template.dock_kind
                    == crate::game_logic::DockKind::SupplyCenter
                    || obj.is_kind_of(KindOf::SupplyCenter)
                    || obj.thing().template.has_supply_center_create;
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
                    && (center.thing().template.dock_kind
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
    pub fn get_objects(&self) -> crate::game_logic::HostObjectRead<'_> {
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
    pub fn get_objects_mut(&mut self) -> &mut HostObjectStore {
        self.host_objects_mut()
    }

    /// Get all players (for snapshot/save system)
    pub fn get_players(&self) -> &HashMap<u32, Player> {
        &self.players
    }

    /// Get mutable players (for snapshot restoration)
    pub(crate) fn get_players_mut(&mut self) -> &mut HashMap<u32, Player> {
        &mut self.players
    }

    /// Transfer an object to the player's default Team. As in Object::setTeam,
    /// an inactive controller redirects the object to this session's neutral
    /// player. Command authority follows the resulting `owner_player_id`.
    pub fn transfer_object_to_player(&mut self, object_id: ObjectId, player_id: u32) -> bool {
        self.transfer_object_to_player_with_effects(
            object_id,
            player_id,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn transfer_object_to_player_with_effects(
        &mut self,
        object_id: ObjectId,
        player_id: u32,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        let Some(destination) = self.host_default_team_id(player_id) else {
            return false;
        };
        self.transfer_object_to_selected_team_with_effects(
            object_id,
            player_id,
            destination,
            presentation_effects,
        )
    }

    /// Transfer through the exact Team selected by an enclosing operation.
    /// This keeps later callbacks from changing a player-wide transfer's
    /// already chosen destination by replacing that player's default Team.
    pub(in crate::game_logic) fn transfer_object_to_selected_team_with_effects(
        &mut self,
        object_id: ObjectId,
        player_id: u32,
        destination: u32,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        let requested_owner = self
            .team_factory
            .with_read_without_flushing(|factory| {
                factory.find_team_by_id(destination).and_then(|team| {
                    team.read()
                        .ok()
                        .and_then(|team| team.get_controlling_player_id())
                })
            })
            .ok()
            .flatten();
        if requested_owner != Some(player_id) {
            return false;
        }
        let effective_player_id = match self.inactive_controller_neutral_team(player_id) {
            Ok(Some(neutral)) => neutral.owner,
            Ok(None) => player_id,
            Err(_) => return false,
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
                let retained_overcharge_bonus = (old_owner_player_id != Some(effective_player_id)
                    && object.is_alive()
                    && object.is_constructed()
                    && object.overcharge_enabled
                    && object.is_disabled()
                    && object.thing().template.supports_overcharge())
                .then(|| object.thing().template.energy_bonus.unwrap_or(0));
                (old_owner_player_id, retained_overcharge_bonus)
            })
            .unwrap_or((None, None));

        if !self.set_host_object_team_with_effects(object_id, destination, presentation_effects) {
            return false;
        }

        if let Some(bonus) = retained_overcharge_bonus {
            if let Some(old_player_id) = old_owner_player_id {
                if let Some(old_player) = self.players.get_mut(&old_player_id) {
                    old_player.captured_overcharge_power_delta = old_player
                        .captured_overcharge_power_delta
                        .saturating_add(bonus);
                }
            }
            if let Some(new_player) = self.players.get_mut(&effective_player_id) {
                new_player.captured_overcharge_power_delta = new_player
                    .captured_overcharge_power_delta
                    .saturating_sub(bonus);
            }
        }
        true
    }
}

mod player_registry_and_frames;
mod relationship_and_enter_queries;
mod special_power_and_capture_queries;

#[cfg(test)]
mod sides_relationship_tests;

#[cfg(test)]
mod can_use_special_power_module_gate_tests;

#[cfg(test)]
mod human_enter_fog_gate_tests;

#[cfg(test)]
mod dock_approach_owner_tests;

#[cfg(test)]
#[path = "object_queries/script_name_owner_tests.rs"]
mod script_name_owner_tests;

#[cfg(test)]
mod receiving_player_team_relationship_owner_tests;
