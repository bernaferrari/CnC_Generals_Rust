//! Shroud, visibility, and victory-summary host projections.
use super::*;
use gamelogic::common::Relationship;

#[derive(Clone, Copy)]
struct ObjectVisionObservation {
    id: ObjectId,
    position: glam::Vec3,
    owner: Option<u32>,
    look_range: f32,
    cover_range: f32,
    reveal_all_range: f32,
    reveal_to_all: bool,
    under_construction: bool,
    blocked: bool,
    stealthed_hidden: bool,
    vision_spied_mask: u32,
}

#[derive(Default)]
struct ObjectVisionReceipts {
    lookers: HashSet<ObjectId>,
    reveal_all: HashSet<ObjectId>,
    covers: HashSet<ObjectId>,
}

impl GameLogic {
    /// Feed Main-crate object positions into ShroudManager.
    ///
    /// C++ Object::look/unlook: unlook previous looker then look at
    /// ShroudClearingRange on move/death. Do not add lookers every frame.
    pub(in super::super::super) fn update_main_crate_vision(&mut self) {
        let shroud = std::sync::Arc::clone(self.world_services.shroud());
        let mut shroud_mgr = match shroud.lock() {
            Ok(mgr) => mgr,
            Err(_) => return,
        };

        let persist = host_unlook_persist_frames();
        let frame = self.frame;
        shroud_mgr.process_pending_undo_shroud_reveals(frame);

        let mut player_ids: Vec<u32> = self.players.keys().copied().collect();
        player_ids.sort_unstable();
        for &pid in &player_ids {
            shroud_mgr.clear_host_object_visibility(pid);
        }

        let snaps: Vec<_> = self
            .objects
            .registered_values()
            .filter(|object| object.is_alive())
            .map(|object| self.observe_object_vision(object))
            .collect();
        let mut live = ObjectVisionReceipts::default();
        for observation in snaps {
            self.stamp_object_vision(observation, &player_ids, &mut live, &mut shroud_mgr);
        }

        unlook_stale_host_partition_looks(
            &mut self.vision_last_looks,
            &live.lookers,
            &mut shroud_mgr,
            persist,
            frame,
        );
        unlook_stale_host_partition_looks(
            &mut self.vision_last_reveal_all,
            &live.reveal_all,
            &mut shroud_mgr,
            persist,
            frame,
        );
        unshroud_stale_host_partition_covers(
            &mut self.vision_last_shroud,
            &live.covers,
            &mut shroud_mgr,
        );

        // Reveal/cover counters, delayed expiry and object COI mixing all read
        // this session's grid. No competing global partition stamp/writeback.
        use crate::game_logic::partition_coi::{
            cells_touched_for_footprint_on_grid, mix_object_shroud_from_cells,
        };
        use gamelogic::common::{Relationship, types::ObjectShroudStatus};

        let object_snaps: Vec<_> = self
            .objects
            .registered_values()
            .filter(|o| o.is_alive())
            .map(|o| {
                let pos = o.get_position();
                let fp = crate::game_logic::game_logic::host_object_footprint(o);
                (
                    o.id,
                    o.owner_player_id,
                    o.contained_by.is_some(),
                    o.is_kind_of(KindOf::Immobile) || o.is_kind_of(KindOf::Structure),
                    o.is_kind_of(KindOf::Mine),
                    o.get_template().always_visible,
                    pos.x,
                    pos.z,
                    fp,
                )
            })
            .collect();

        let origin = shroud_mgr.grid_world_origin().unwrap_or([0.0, 0.0]);
        let Some((width, height, cell_size)) = shroud_mgr.grid_dimensions() else {
            return;
        };
        let dimensions = [width, height];
        for (id, owner, contained, immobile, mine, always_visible, x, z, fp) in object_snaps {
            let cells = match cells_touched_for_footprint_on_grid(
                origin, dimensions, cell_size, x, z, fp,
            ) {
                Ok(cells) => cells,
                Err(reason) => {
                    log::error!(
                        "Rejecting host object {} partition footprint: {reason}",
                        id.0
                    );
                    continue;
                }
            };
            for &pid in &player_ids {
                if always_visible || contained {
                    shroud_mgr.set_host_object_shroud_status(pid, id.0, ObjectShroudStatus::Clear);
                    shroud_mgr.mark_host_object_seen(pid, id.0);
                    shroud_mgr.set_host_object_ever_seen(pid, id.0, true);
                    continue;
                }
                if owner == Some(pid) {
                    // C++ Object::getShroudedStatus (Object.cpp) resolves through
                    // PartitionData::getShroudedStatus (PartitionManager.cpp:1582)
                    // over the object's COI cells — and every object is a shroud
                    // looker for its controlling player, so its own footprint is
                    // always Visible and the status for the owner is CLEAR.  A
                    // looker-less host world must not fog a player's own units
                    // out of ActionManager shroud gates (ActionManager.cpp:76-102).
                    shroud_mgr.set_host_object_shroud_status(pid, id.0, ObjectShroudStatus::Clear);
                    shroud_mgr.mark_host_object_seen(pid, id.0);
                    shroud_mgr.set_host_object_ever_seen(pid, id.0, true);
                    continue;
                }
                // C++ PartitionData mixes live 40wu COIs within the map. Read
                // the same owned counters updated above, including queued
                // unlooks and immediate shroud-cover changes.
                let counts = shroud_mgr.count_footprint_cells(pid, &cells);
                let ever = shroud_mgr.host_object_ever_seen(pid, id.0);
                let relationship_neutral = match owner {
                    Some(oid) => self.player_relationship(pid, oid) == Relationship::Neutral,
                    None => true,
                };
                let (status, ever_now) = mix_object_shroud_from_cells(
                    counts.total,
                    counts.shrouded,
                    counts.fogged,
                    relationship_neutral,
                    immobile,
                    mine,
                    ever,
                );
                shroud_mgr.set_host_object_ever_seen(pid, id.0, ever_now);
                shroud_mgr.set_host_object_shroud_status(pid, id.0, status);
                match status {
                    ObjectShroudStatus::Clear | ObjectShroudStatus::PartialClear => {
                        shroud_mgr.mark_host_object_seen(pid, id.0);
                    }
                    ObjectShroudStatus::Fogged => {
                        shroud_mgr.mark_host_object_explored(pid, id.0);
                    }
                    _ => {}
                }
            }
        }
        shroud_mgr.mark_host_vision_ready();
        if let Err(error) = self.update_host_fog_ghosts_with_shroud(&shroud_mgr) {
            log::error!("Cannot update this session's fog ghosts: {error}");
        }
    }

    fn observe_object_vision(&self, object: &Object) -> ObjectVisionObservation {
        let template = object.get_template();
        ObjectVisionObservation {
            id: object.id,
            position: object.get_position(),
            owner: object
                .owner_player_id
                .or_else(|| self.player_id_for_team(object.team)),
            look_range: object.get_shroud_clearing_range(),
            cover_range: object.shroud_range,
            reveal_all_range: template.shroud_reveal_to_all_range,
            reveal_to_all: template.reveal_to_all,
            under_construction: object.status.under_construction,
            blocked: object.contained_by.is_some_and(|id| {
                self.objects
                    .get(&id)
                    .is_some_and(container_blocks_passenger_look)
            }),
            stealthed_hidden: object.status.stealthed
                && !object.status.detected
                && !object.status.disguised,
            vision_spied_mask: object.vision_spied_mask,
        }
    }

    fn stamp_object_vision(
        &mut self,
        object: ObjectVisionObservation,
        players: &[u32],
        live: &mut ObjectVisionReceipts,
        shroud: &mut gamelogic::system::shroud_manager::ShroudManager,
    ) {
        self.stamp_object_looks(object, players, live, shroud);
        self.stamp_object_cover(object, players, live, shroud);
    }

    fn stamp_object_cover(
        &mut self,
        object: ObjectVisionObservation,
        players: &[u32],
        live: &mut ObjectVisionReceipts,
        shroud: &mut gamelogic::system::shroud_manager::ShroudManager,
    ) {
        let center = gamelogic::common::Coord3D::new(
            object.position.x,
            object.position.z,
            object.position.y,
        );
        if object.cover_range > 0.0 && !object.under_construction {
            if let Some(owner) = object.owner {
                let mut mask = 0u32;
                for &player in players {
                    if self.player_relationship(owner, player) != Relationship::Allies {
                        mask |= 1u32 << player.min(31);
                    }
                }
                if mask != 0 {
                    restamp_host_partition_shroud(
                        &mut self.vision_last_shroud,
                        &mut live.covers,
                        shroud,
                        object.id,
                        center,
                        object.cover_range,
                        mask,
                    );
                }
            }
        }
    }

    fn stamp_object_looks(
        &mut self,
        object: ObjectVisionObservation,
        players: &[u32],
        live: &mut ObjectVisionReceipts,
        shroud: &mut gamelogic::system::shroud_manager::ShroudManager,
    ) {
        if object.blocked {
            return;
        }
        let center = gamelogic::common::Coord3D::new(
            object.position.x,
            object.position.z,
            object.position.y,
        );
        if object.look_range > 0.0 {
            let mut mask = if object.reveal_to_all {
                players
                    .iter()
                    .fold(0u32, |mask, &player| mask | (1u32 << player.min(31)))
            } else if let Some(owner) = object.owner {
                players.iter().fold(0u32, |mask, &player| {
                    if self.player_relationship(owner, player) == Relationship::Allies {
                        mask | (1u32 << player.min(31))
                    } else {
                        mask
                    }
                })
            } else {
                0
            };
            mask |= object.vision_spied_mask;
            if mask != 0 {
                restamp_host_partition_look(
                    &mut self.vision_last_looks,
                    &mut live.lookers,
                    shroud,
                    object.id,
                    center,
                    object.look_range,
                    mask,
                    host_unlook_persist_frames(),
                    self.frame,
                );
            }
        }
        if object.reveal_all_range > 0.0 && !object.under_construction && !object.stealthed_hidden {
            if let Some(owner) = object.owner {
                let mut mask = 0u32;
                for &player in players {
                    if matches!(
                        self.player_relationship(owner, player),
                        Relationship::Enemies | Relationship::Neutral
                    ) {
                        mask |= 1u32 << player.min(31);
                    }
                }
                if mask != 0 {
                    restamp_host_partition_look(
                        &mut self.vision_last_reveal_all,
                        &mut live.reveal_all,
                        shroud,
                        object.id,
                        center,
                        object.reveal_all_range,
                        mask,
                        host_unlook_persist_frames(),
                        self.frame,
                    );
                }
            }
        }
    }

    /// Re-establish this object's look and cover at its boundary callback.
    pub(in crate::game_logic) fn relook_boundary_object(
        &mut self,
        id: ObjectId,
    ) -> Result<(), String> {
        let Some(observation) = self
            .objects
            .get(&id)
            .filter(|object| object.is_alive())
            .map(|object| self.observe_object_vision(object))
        else {
            return Ok(());
        };
        let mut players: Vec<_> = self.players.keys().copied().collect();
        players.sort_unstable();
        let shroud = std::sync::Arc::clone(self.world_services.shroud());
        let mut shroud = shroud
            .lock()
            .map_err(|_| "receiving shroud owner is poisoned")?;
        let mut live = ObjectVisionReceipts::default();
        // Object::handleShroud restores cover before its own look.
        self.stamp_object_cover(observation, &players, &mut live, &mut shroud);
        self.stamp_object_looks(observation, &players, &mut live, &mut shroud);
        Ok(())
    }

    pub(in super::super::super) fn shroud_visibility_snapshot_for_player(
        &self,
        player_id: u32,
    ) -> Option<ShroudVisibilitySnapshot> {
        let viewer = self.players.get(&player_id)?;
        let shroud_manager = self.world_services.shroud();
        let shroud_mgr = shroud_manager.lock().ok()?;
        let raw_visible_objects = shroud_mgr.get_visible_objects(player_id);

        // Match existing fail-open behavior while shroud has not produced runtime visibility yet.
        let runtime_active =
            shroud_mgr.get_last_update_frame() > 0 || !raw_visible_objects.is_empty();
        if !runtime_active {
            return None;
        }

        // Apply the receiving Main world's canonical Object status. The Core
        // ShroudManager compatibility query consults process-global Core
        // StealthManager/DetectionManager/ObjectManager singletons, whose IDs
        // can alias this session's native ObjectIds.
        let mut visible_objects = HashSet::with_capacity(raw_visible_objects.len());
        for object_id in raw_visible_objects {
            let id = ObjectId(object_id);
            let visible_to_team = self
                .objects
                .get(&id)
                .is_some_and(|object| self.is_object_visible_to_player(object, viewer));
            if visible_to_team {
                visible_objects.insert(object_id);
            }
        }

        Some(ShroudVisibilitySnapshot {
            visible_objects,
            explored_objects: shroud_mgr
                .get_explored_objects(player_id)
                .into_iter()
                .collect(),
        })
    }

    /// C++ StealthUpdate::getStealthLook uses the receiving Player's actual
    /// relationship to the object's Team. The global DETECTED bit remains a
    /// target-wide Object status, matching ObjectStatus::DETECTED.
    pub(in super::super::super) fn is_object_visible_to_player(
        &self,
        object: &Object,
        viewer: &Player,
    ) -> bool {
        if !object.is_alive() {
            return false;
        }
        if !object.status.stealthed || object.status.detected || object.status.disguised {
            return true;
        }
        if viewer.is_observer || !viewer.is_alive {
            return true;
        }

        let relationship = if let (Some(object_team), Some(viewer_team)) =
            (object.team_instance_id, viewer.default_team_id)
        {
            self.host_team_relationship(object_team, viewer_team)
                .unwrap_or(Relationship::Neutral)
        } else if let Some(owner) = object.owner_player_id {
            self.player_relationship(owner, viewer.id)
        } else {
            Relationship::Neutral
        };
        relationship == Relationship::Allies
    }

    pub(in super::super::super) fn is_object_visible_for_player(
        &self,
        object_id: ObjectId,
        object: &Object,
        viewer_id: u32,
        shroud_snapshot: Option<&ShroudVisibilitySnapshot>,
    ) -> bool {
        let Some(viewer) = self.players.get(&viewer_id) else {
            return false;
        };
        if !self.is_object_visible_to_player(object, viewer) {
            return false;
        }
        if let Some(snapshot) = shroud_snapshot {
            let id = object_id.0;
            snapshot.visible_objects.contains(&id) || snapshot.explored_objects.contains(&id)
        } else {
            true
        }
    }

    pub(in super::super::super) fn is_object_visible_on_minimap_for_player(
        &self,
        object_id: ObjectId,
        object: &Object,
        viewer_id: u32,
        shroud_snapshot: Option<&ShroudVisibilitySnapshot>,
    ) -> bool {
        let Some(viewer) = self.players.get(&viewer_id) else {
            return false;
        };
        if !self.is_object_visible_to_player(object, viewer) {
            return false;
        }
        let friendly = viewer.is_observer
            || !viewer.is_alive
            || if let (Some(object_team), Some(viewer_team)) =
                (object.team_instance_id, viewer.default_team_id)
            {
                self.host_team_relationship(object_team, viewer_team)
                    .is_some_and(|relation| relation == Relationship::Allies)
            } else {
                object.owner_player_id == Some(viewer_id)
            };
        if friendly {
            return true;
        }
        if let Some(snapshot) = shroud_snapshot {
            let id = object_id.0;
            if snapshot.visible_objects.contains(&id) {
                return true;
            }
            return object.is_kind_of(KindOf::Structure) && snapshot.explored_objects.contains(&id);
        }
        true
    }

    pub(in super::super::super) fn is_object_visible_for_team(
        object_id: ObjectId,
        object: &Object,
        viewing_team: Team,
        shroud_snapshot: Option<&ShroudVisibilitySnapshot>,
    ) -> bool {
        if !object.is_alive() || !object.is_visible_to_team(viewing_team) {
            return false;
        }

        if let Some(snapshot) = shroud_snapshot {
            let id = object_id.0;
            snapshot.visible_objects.contains(&id) || snapshot.explored_objects.contains(&id)
        } else {
            true
        }
    }

    pub(in super::super::super) fn is_object_visible_on_minimap_for_team(
        object_id: ObjectId,
        object: &Object,
        viewing_team: Team,
        shroud_snapshot: Option<&ShroudVisibilitySnapshot>,
    ) -> bool {
        if !object.is_alive() || !object.is_visible_to_team(viewing_team) {
            return false;
        }

        if object.team == viewing_team {
            return true;
        }

        if let Some(snapshot) = shroud_snapshot {
            let id = object_id.0;
            if snapshot.visible_objects.contains(&id) {
                return true;
            }
            // Keep explored structures on minimap for strategic continuity.
            return object.is_kind_of(KindOf::Structure) && snapshot.explored_objects.contains(&id);
        }

        true
    }

    pub fn first_opponent_id(&self, player_id: u32) -> Option<u32> {
        self.players
            .values()
            .find(|player| player.id != player_id)
            .map(|player| player.id)
    }

    pub fn build_victory_summary(&self, winner_id: Option<u32>) -> VictorySummary {
        let mission_name = if self.map_loaded {
            Some(self.map_name.clone())
        } else {
            None
        };

        let duration = if self.sim_time_seconds > 0.0 {
            Some(Duration::from_secs_f32(self.sim_time_seconds))
        } else {
            None
        };

        let mut player_results = Vec::new();
        for player in self.players.values() {
            let outcome = match winner_id {
                Some(id) if id == player.id => PlayerOutcome::Won,
                Some(_) => PlayerOutcome::Lost,
                None => PlayerOutcome::Draw,
            };

            player_results.push(PlayerResult {
                player_id: player.id,
                player_name: player.name.clone(),
                faction: player.team,
                units_built: player.statistics.units_built(),
                units_destroyed: player.statistics.units_destroyed(),
                units_lost: player.statistics.units_lost(),
                structures_built: player.statistics.structures_built(),
                structures_destroyed: player.statistics.structures_destroyed(),
                structures_lost: player.statistics.structures_lost(),
                resources_collected: player.statistics.resources_collected,
                resources_spent: player.statistics.resources_spent(),
                score: player.calculate_score().max(0) as u32,
                outcome,
            });
        }

        VictorySummary {
            mission_name,
            duration,
            player_results,
        }
    }
}

#[cfg(test)]
mod boundary_relook_tests {
    use super::*;
    use gamelogic::common::Coord3D;
    use gamelogic::system::shroud_manager::ShroudState;

    #[test]
    fn each_boundary_relook_stamps_only_its_object_and_preserves_peer() {
        let mut world = GameLogic::new();
        let mut peer = GameLogic::new();
        for session in [&mut world, &mut peer] {
            session.add_player(Player::new(1, Team::USA, "Viewer", true));
            let mut template = ThingTemplate::new("BoundaryLooker");
            template.shroud_clearing_range = 20.0;
            session.templates.insert(template.name.clone(), template);
            session
                .world_services
                .shroud()
                .lock()
                .unwrap()
                .init_shroud_grid(512.0, 512.0);
        }
        let first = world
            .create_object_for_player("BoundaryLooker", 1, glam::Vec3::new(100.0, 0.0, 100.0))
            .unwrap();
        let second = world
            .create_object_for_player("BoundaryLooker", 1, glam::Vec3::new(300.0, 0.0, 300.0))
            .unwrap();
        assert_eq!(
            peer.create_object_for_player("BoundaryLooker", 1, glam::Vec3::new(100.0, 0.0, 100.0)),
            Some(first)
        );
        let peer_before = peer
            .world_services
            .shroud()
            .lock()
            .unwrap()
            .snapshot_state();
        // Boundary reset retires the previous receipts before reattaching objects.
        world.vision_last_looks.clear();
        world.vision_last_reveal_all.clear();
        world.vision_last_shroud.clear();
        world
            .world_services
            .shroud()
            .lock()
            .unwrap()
            .init_shroud_grid(512.0, 512.0);
        let a = Coord3D::new(100.0, 100.0, 0.0);
        let b = Coord3D::new(300.0, 300.0, 0.0);
        world.relook_boundary_object(first).unwrap();
        {
            let shroud = world.world_services.shroud().lock().unwrap();
            assert_eq!(shroud.get_shroud_state(1, &a), ShroudState::Visible);
            assert_ne!(shroud.get_shroud_state(1, &b), ShroudState::Visible);
        }
        world.relook_boundary_object(second).unwrap();
        assert_eq!(
            world
                .world_services
                .shroud()
                .lock()
                .unwrap()
                .get_shroud_state(1, &b),
            ShroudState::Visible
        );
        assert_eq!(
            peer.world_services
                .shroud()
                .lock()
                .unwrap()
                .snapshot_state(),
            peer_before
        );
    }
}
