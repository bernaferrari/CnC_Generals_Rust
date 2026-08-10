// Team Snapshotable and thing-factory Team impls
//
// Split from `team.rs` for module-size parity.
// Observable behavior is unchanged.

/// Save/load support for Team.
/// Matches C++ Team::xfer (Team.cpp:2547, version 1).
impl Snapshotable for Team {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;

        let mut team_id = self.id;
        xfer.xfer_u32(&mut team_id).map_err(|e| e.to_string())?;

        let mut member_count = self.members.len() as u16;
        xfer.xfer_unsigned_short(&mut member_count)
            .map_err(|e| e.to_string())?;
        for &obj_id in &self.members {
            let mut id = obj_id;
            xfer.xfer_object_id(&mut id).map_err(|e| e.to_string())?;
        }

        let mut state = self.state.to_string();
        xfer.xfer_ascii_string(&mut state)
            .map_err(|e| e.to_string())?;

        let mut entered_or_exited = self.entered_or_exited;
        xfer.xfer_bool(&mut entered_or_exited)
            .map_err(|e| e.to_string())?;
        let mut active = self.active;
        xfer.xfer_bool(&mut active).map_err(|e| e.to_string())?;
        let mut created = self.created;
        xfer.xfer_bool(&mut created).map_err(|e| e.to_string())?;
        let mut check_enemy_sighted = self.check_enemy_sighted;
        xfer.xfer_bool(&mut check_enemy_sighted)
            .map_err(|e| e.to_string())?;
        let mut see_enemy = self.see_enemy;
        xfer.xfer_bool(&mut see_enemy).map_err(|e| e.to_string())?;
        let mut prev_see_enemy = self.prev_see_enemy;
        xfer.xfer_bool(&mut prev_see_enemy)
            .map_err(|e| e.to_string())?;
        let mut was_idle = self.was_idle;
        xfer.xfer_bool(&mut was_idle).map_err(|e| e.to_string())?;
        let mut destroy_threshold = self.destroy_threshold;
        xfer.xfer_int(&mut destroy_threshold)
            .map_err(|e| e.to_string())?;
        let mut cur_units = self.cur_units;
        xfer.xfer_int(&mut cur_units).map_err(|e| e.to_string())?;

        let mut waypoint_id = self.current_waypoint_id.unwrap_or(0);
        xfer.xfer_u32(&mut waypoint_id).map_err(|e| e.to_string())?;

        let mut generic_count = MAX_GENERIC_SCRIPTS as u16;
        xfer.xfer_unsigned_short(&mut generic_count)
            .map_err(|e| e.to_string())?;
        for i in 0..MAX_GENERIC_SCRIPTS {
            let mut val = self.should_attempt_generic_script[i];
            xfer.xfer_bool(&mut val).map_err(|e| e.to_string())?;
        }

        let mut recruitability_set = self.recruitability_set;
        xfer.xfer_bool(&mut recruitability_set)
            .map_err(|e| e.to_string())?;
        let mut recruitable = self.recruitable;
        xfer.xfer_bool(&mut recruitable)
            .map_err(|e| e.to_string())?;

        let mut target = self.common_attack_target;
        xfer.xfer_object_id(&mut target)
            .map_err(|e| e.to_string())?;

        // team_relations (inline, matching C++ TeamRelationMap::xfer pattern)
        let team_rel_count = self
            .team_relations
            .as_ref()
            .map(|r| r.map.len() as u16)
            .unwrap_or(0);
        let mut team_rel_count_xfer = team_rel_count;
        xfer.xfer_unsigned_short(&mut team_rel_count_xfer)
            .map_err(|e| e.to_string())?;
        if let Some(ref relations) = self.team_relations {
            for (&tid, &rel) in &relations.map {
                let mut team_id_val = tid;
                let mut rel_raw = rel as i32;
                xfer.xfer_u32(&mut team_id_val).map_err(|e| e.to_string())?;
                xfer.xfer_int(&mut rel_raw).map_err(|e| e.to_string())?;
            }
        }

        // player_relations (inline, matching C++ PlayerRelationMap::xfer pattern)
        let player_rel_count = self
            .player_relations
            .as_ref()
            .map(|r| r.len() as u16)
            .unwrap_or(0);
        let mut player_rel_count_xfer = player_rel_count;
        xfer.xfer_unsigned_short(&mut player_rel_count_xfer)
            .map_err(|e| e.to_string())?;
        if let Some(ref relations) = self.player_relations {
            for (&pidx, &rel) in relations {
                let mut player_idx = pidx;
                let mut rel_raw = rel as i32;
                xfer.xfer_int(&mut player_idx).map_err(|e| e.to_string())?;
                xfer.xfer_int(&mut rel_raw).map_err(|e| e.to_string())?;
            }
        }

        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // C++ Team::xfer version 1
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;

        // Team ID sanity check
        let mut team_id = self.id;
        xfer.xfer_u32(&mut team_id).map_err(|e| e.to_string())?;
        if team_id != self.id {
            return Err(format!(
                "Team::xfer - TeamID mismatch. Xfered '{}' but should be '{}'",
                team_id, self.id
            ));
        }

        // Member list
        let mut member_count = self.members.len() as u16;
        xfer.xfer_unsigned_short(&mut member_count)
            .map_err(|e| e.to_string())?;

        if xfer.get_xfer_mode() == XferMode::Save {
            for &obj_id in &self.members {
                let mut id = obj_id;
                xfer.xfer_object_id(&mut id).map_err(|e| e.to_string())?;
            }
        } else {
            // Load: store member IDs for post-process reconnection
            self.members.clear();
            for _ in 0..member_count {
                let mut id = INVALID_ID;
                xfer.xfer_object_id(&mut id).map_err(|e| e.to_string())?;
                self.members.push(id);
            }
        }

        // State
        let mut state = self.state.to_string();
        xfer.xfer_ascii_string(&mut state)
            .map_err(|e| e.to_string())?;

        // Status flags
        xfer.xfer_bool(&mut self.entered_or_exited)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.active)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.created)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.check_enemy_sighted)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.see_enemy)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.prev_see_enemy)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.was_idle)
            .map_err(|e| e.to_string())?;

        // Destruction tracking
        xfer.xfer_int(&mut self.destroy_threshold)
            .map_err(|e| e.to_string())?;
        xfer.xfer_int(&mut self.cur_units)
            .map_err(|e| e.to_string())?;

        // Current waypoint ID
        let mut waypoint_id: UnsignedInt = self.current_waypoint_id.unwrap_or(0);
        xfer.xfer_u32(&mut waypoint_id).map_err(|e| e.to_string())?;
        if xfer.get_xfer_mode() == XferMode::Load {
            self.current_waypoint_id = if waypoint_id == 0 {
                None
            } else {
                Some(waypoint_id)
            };
        }

        // Generic script attempt flags
        let mut generic_count = MAX_GENERIC_SCRIPTS as u16;
        xfer.xfer_unsigned_short(&mut generic_count)
            .map_err(|e| e.to_string())?;
        if generic_count as usize != MAX_GENERIC_SCRIPTS {
            return Err(
                "Team::xfer - The number of allowable Generic scripts has changed, and this chunk needs to be versioned."
                    .to_string(),
            );
        }
        for i in 0..MAX_GENERIC_SCRIPTS {
            xfer.xfer_bool(&mut self.should_attempt_generic_script[i])
                .map_err(|e| e.to_string())?;
        }

        // Recruitability
        xfer.xfer_bool(&mut self.recruitability_set)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.recruitable)
            .map_err(|e| e.to_string())?;

        // Common attack target
        xfer.xfer_object_id(&mut self.common_attack_target)
            .map_err(|e| e.to_string())?;

        // Team relations (inline, matching C++ TeamRelationMap::xfer)
        {
            let mut rel_version: XferVersion = 1;
            xfer.xfer_version(&mut rel_version, 1)
                .map_err(|e| e.to_string())?;

            let mut rel_count = self
                .team_relations
                .as_ref()
                .map(|r| r.map.len() as u16)
                .unwrap_or(0);
            xfer.xfer_unsigned_short(&mut rel_count)
                .map_err(|e| e.to_string())?;

            if xfer.get_xfer_mode() == XferMode::Save {
                if let Some(ref relations) = self.team_relations {
                    for (&tid, &rel) in &relations.map {
                        let mut team_id_val = tid;
                        let mut rel_raw = rel as Int;
                        xfer.xfer_u32(&mut team_id_val).map_err(|e| e.to_string())?;
                        xfer.xfer_int(&mut rel_raw).map_err(|e| e.to_string())?;
                    }
                }
            } else {
                self.team_relations = None;
                if rel_count > 0 {
                    let mut map = TeamRelationMap::new();
                    for _ in 0..rel_count {
                        let mut team_id_val: UnsignedInt = 0;
                        let mut rel_raw: Int = 0;
                        xfer.xfer_u32(&mut team_id_val).map_err(|e| e.to_string())?;
                        xfer.xfer_int(&mut rel_raw).map_err(|e| e.to_string())?;
                        let rel = match rel_raw {
                            0 => Relationship::Enemies,
                            1 => Relationship::Neutral,
                            2 => Relationship::Allies,
                            _ => Relationship::Neutral,
                        };
                        map.map.insert(team_id_val, rel);
                    }
                    self.team_relations = Some(map);
                }
            }
        }

        // Player relations (inline, matching C++ PlayerRelationMap::xfer)
        {
            let mut rel_version: XferVersion = 1;
            xfer.xfer_version(&mut rel_version, 1)
                .map_err(|e| e.to_string())?;

            let mut rel_count = self
                .player_relations
                .as_ref()
                .map(|r| r.len() as u16)
                .unwrap_or(0);
            xfer.xfer_unsigned_short(&mut rel_count)
                .map_err(|e| e.to_string())?;

            if xfer.get_xfer_mode() == XferMode::Save {
                if let Some(ref relations) = self.player_relations {
                    for (&pidx, &rel) in relations {
                        let mut player_idx = pidx;
                        let mut rel_raw = rel as Int;
                        xfer.xfer_int(&mut player_idx).map_err(|e| e.to_string())?;
                        xfer.xfer_int(&mut rel_raw).map_err(|e| e.to_string())?;
                    }
                }
            } else {
                self.player_relations = None;
                if rel_count > 0 {
                    let mut map = HashMap::new();
                    for _ in 0..rel_count {
                        let mut player_idx: Int = 0;
                        let mut rel_raw: Int = 0;
                        xfer.xfer_int(&mut player_idx).map_err(|e| e.to_string())?;
                        xfer.xfer_int(&mut rel_raw).map_err(|e| e.to_string())?;
                        let rel = match rel_raw {
                            0 => Relationship::Enemies,
                            1 => Relationship::Neutral,
                            2 => Relationship::Allies,
                            _ => Relationship::Neutral,
                        };
                        map.insert(player_idx, rel);
                    }
                    self.player_relations = Some(map);
                }
            }
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl game_engine::common::thing::thing_factory::Team for Team {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn team_id(&self) -> Option<u32> {
        Some(self.get_id())
    }
}

