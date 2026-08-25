//! Shroud, visibility, and victory-summary host projections.
use super::*;

impl GameLogic {
    /// Feed Main-crate object positions into ShroudManager.
    ///
    /// C++ Object::look/unlook: unlook previous looker then look at
    /// ShroudClearingRange on move/death. Do not add lookers every frame.
    pub(in super::super::super) fn update_main_crate_vision(&mut self) {
        use gamelogic::common::Coord3D;

        let shroud = get_shroud_manager();
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

        let mut live_lookers = std::collections::HashSet::new();
        let mut live_reveal_all = std::collections::HashSet::new();
        let mut live_covers = std::collections::HashSet::new();
        let mut cell_ops: Vec<(Coord3D, f32, u32, bool)> = Vec::new();

        let snaps: Vec<_> = self
            .objects
            .values()
            .filter(|obj| obj.is_alive())
            .map(|obj| {
                let pos = obj.get_position();
                let tpl = obj.get_template();
                let vision_range = if obj.vision_range > 0.0 {
                    obj.vision_range
                } else {
                    tpl.sight_range
                };
                // C++ Object::look calls getShroudClearingRange() (Object.cpp:4938).
                // The getter, not look() itself, clamps UNDER_CONSTRUCTION to
                // the bounding-circle radius (Object.cpp:5128-5140).
                let mut shroud_range = obj.get_shroud_clearing_range();
                if !obj.status.under_construction {
                    if shroud_range <= 0.0 {
                        shroud_range = tpl.resolved_shroud_clearing_range();
                    }
                    if shroud_range < 0.0 {
                        shroud_range = vision_range;
                    }
                }
                let owner_pid = obj
                    .owner_player_id
                    .or_else(|| self.player_id_for_team(obj.team));
                let blocked = obj.contained_by.is_some_and(|cid| {
                    self.objects
                        .get(&cid)
                        .is_some_and(container_blocks_passenger_look)
                });
                let stealthed_hidden =
                    obj.status.stealthed && !obj.status.detected && !obj.status.disguised;
                (
                    obj.id,
                    pos,
                    owner_pid,
                    shroud_range,
                    obj.shroud_range,
                    tpl.shroud_reveal_to_all_range,
                    tpl.reveal_to_all,
                    obj.status.under_construction,
                    blocked,
                    stealthed_hidden,
                    obj.vision_spied_mask,
                )
            })
            .collect();

        for (
            id,
            pos,
            owner_pid,
            shroud_range,
            cover_range,
            reveal_all_range,
            reveal_to_all_kind,
            under_construction,
            blocked,
            stealthed_hidden,
            vision_spied_mask,
        ) in snaps
        {
            let center = Coord3D::new(pos.x, pos.z, pos.y);

            if !blocked {
                if shroud_range > 0.0 {
                    let mut player_mask = if reveal_to_all_kind {
                        player_ids
                            .iter()
                            .fold(0u32, |mask, &pid| mask | (1u32 << pid.min(31)))
                    } else if let Some(owner_pid) = owner_pid {
                        let mut mask = 0u32;
                        for &pid in &player_ids {
                            if self.player_relationship(owner_pid, pid)
                                == gamelogic::common::Relationship::Allies
                            {
                                mask |= 1u32 << pid.min(31);
                            }
                        }
                        mask
                    } else {
                        0u32
                    };
                    // C++ Object::look: lookingMask |= m_visionSpiedMask.
                    player_mask |= vision_spied_mask;
                    if player_mask != 0 {
                        restamp_host_partition_look(
                            &mut self.vision_last_looks,
                            &mut live_lookers,
                            &mut shroud_mgr,
                            &mut cell_ops,
                            id,
                            center,
                            shroud_range,
                            player_mask,
                            persist,
                            frame,
                        );
                    }
                }

                if reveal_all_range > 0.0 && !under_construction && !stealthed_hidden {
                    if let Some(owner_pid) = owner_pid {
                        let mut reveal_mask = 0u32;
                        for &pid in &player_ids {
                            let rel = self.player_relationship(owner_pid, pid);
                            if matches!(
                                rel,
                                gamelogic::common::Relationship::Enemies
                                    | gamelogic::common::Relationship::Neutral
                            ) {
                                reveal_mask |= 1u32 << pid.min(31);
                            }
                        }
                        if reveal_mask != 0 {
                            restamp_host_partition_look(
                                &mut self.vision_last_reveal_all,
                                &mut live_reveal_all,
                                &mut shroud_mgr,
                                &mut cell_ops,
                                id,
                                center,
                                reveal_all_range,
                                reveal_mask,
                                persist,
                                frame,
                            );
                        }
                    }
                }
            }

            // C++ Object::shroud: !UNDER_CONSTRUCTION && !dead && getShroudRange() > 0.
            // Cover is independent of passenger-look blocking.
            if cover_range > 0.0 && !under_construction {
                if let Some(owner_pid) = owner_pid {
                    let mut shrouding_mask = 0u32;
                    for &pid in &player_ids {
                        if self.player_relationship(owner_pid, pid)
                            != gamelogic::common::Relationship::Allies
                        {
                            shrouding_mask |= 1u32 << pid.min(31);
                        }
                    }
                    if shrouding_mask != 0 {
                        restamp_host_partition_shroud(
                            &mut self.vision_last_shroud,
                            &mut live_covers,
                            &mut shroud_mgr,
                            id,
                            center,
                            cover_range,
                            shrouding_mask,
                        );
                    }
                }
            }
        }

        unlook_stale_host_partition_looks(
            &mut self.vision_last_looks,
            &live_lookers,
            &mut shroud_mgr,
            &mut cell_ops,
            persist,
            frame,
        );
        unlook_stale_host_partition_looks(
            &mut self.vision_last_reveal_all,
            &live_reveal_all,
            &mut shroud_mgr,
            &mut cell_ops,
            persist,
            frame,
        );
        unshroud_stale_host_partition_covers(
            &mut self.vision_last_shroud,
            &live_covers,
            &mut shroud_mgr,
        );

        drop(shroud_mgr);
        for (center, radius, mask, add) in cell_ops {
            gamelogic::object::stamp_partition_cell_lookers(&center, radius, mask, add);
        }

        // C++ PartitionData::getShroudedStatus — object FOW is the footprint
        // COI mix, not a VisionRange circle (hq-mvlin).
        let Ok(mut shroud_mgr) = shroud.lock() else {
            return;
        };
        use crate::game_logic::partition_coi::{
            HostPartitionFootprint, cells_touched_for_footprint, mix_object_shroud_from_cells,
        };
        use gamelogic::common::{Relationship, types::ObjectShroudStatus};
        use gamelogic::system::shroud_manager::ShroudState;

        let leftover_cell_size = shroud_mgr
            .grid_dimensions()
            .map(|(_, _, s)| s)
            .unwrap_or(40.0);

        let object_snaps: Vec<_> = self
            .objects
            .values()
            .filter(|o| o.is_alive())
            .map(|o| {
                let pos = o.get_position();
                let geom = &o.thing.template.geometry_info;
                let fp = if geom.authored {
                    HostPartitionFootprint {
                        major_radius: geom.major_radius,
                        minor_radius: geom.minor_radius,
                        angle: o.get_orientation(),
                        is_small: geom.is_small,
                        is_box: matches!(geom.geom_type, crate::game_logic::HostGeometryType::Box),
                    }
                } else {
                    HostPartitionFootprint::small_circle(o.selection_radius.max(1.0))
                };
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

        for (id, owner, contained, immobile, mine, always_visible, x, z, fp) in object_snaps {
            let cells = cells_touched_for_footprint(x, z, fp);
            for &pid in &player_ids {
                if always_visible || contained {
                    shroud_mgr.set_host_object_shroud_status(pid, id.0, ObjectShroudStatus::Clear);
                    shroud_mgr.mark_host_object_seen(pid, id.0);
                    shroud_mgr.set_host_object_ever_seen(pid, id.0, true);
                    continue;
                }
                let mut shrouded_cells = 0usize;
                let mut fogged_cells = 0usize;
                for &(cx, cz) in &cells {
                    // Leftover DiscreteCircle lookers, not PARTITION_MANAGER square disk.
                    match leftover_discrete_circle_looker_cell(
                        &shroud_mgr,
                        pid,
                        cx,
                        cz,
                        leftover_cell_size,
                    ) {
                        ShroudState::Hidden => shrouded_cells += 1,
                        ShroudState::Explored => fogged_cells += 1,
                        ShroudState::Visible => {}
                    }
                }
                let ever = shroud_mgr.host_object_ever_seen(pid, id.0);
                let relationship_neutral = match owner {
                    Some(oid) => self.player_relationship(pid, oid) == Relationship::Neutral,
                    None => true,
                };
                let (status, ever_now) = mix_object_shroud_from_cells(
                    cells.len(),
                    shrouded_cells,
                    fogged_cells,
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
    }

    pub(in super::super::super) fn shroud_visibility_snapshot_for_team(
        &self,
        viewing_team: Team,
    ) -> Option<ShroudVisibilitySnapshot> {
        let player_id = self.player_id_for_team(viewing_team)?;
        let shroud_mgr = get_shroud_manager().lock().ok()?;
        let raw_visible_objects = shroud_mgr.get_visible_objects(player_id);

        // Match existing fail-open behavior while shroud has not produced runtime visibility yet.
        let runtime_active =
            shroud_mgr.get_last_update_frame() > 0 || !raw_visible_objects.is_empty();
        if !runtime_active {
            return None;
        }

        // Apply stealth-aware visibility to currently visible objects.
        let mut visible_objects = HashSet::with_capacity(raw_visible_objects.len());
        for object_id in raw_visible_objects {
            if shroud_mgr
                .can_see_object_with_stealth(player_id, object_id)
                .unwrap_or(true)
            {
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
                units_built: player.statistics.units_built,
                units_destroyed: player.statistics.units_destroyed,
                units_lost: player.statistics.units_lost,
                structures_built: player.statistics.structures_built,
                structures_destroyed: player.statistics.structures_destroyed,
                structures_lost: player.statistics.structures_lost,
                resources_collected: player.statistics.resources_collected,
                resources_spent: player.statistics.resources_spent,
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
