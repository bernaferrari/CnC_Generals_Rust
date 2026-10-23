// Team Snapshotable and thing-factory Team impls
//
// Split from `team.rs` for module-size parity.
// Observable behavior is unchanged.

/// Save/load support for Team.
/// Matches C++ Team::xfer (Team.cpp:2547, version 1).
impl Snapshotable for Team {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.validate_creation_snapshot()?;
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

        let mut target = self.common_attack_target.load(Ordering::Relaxed);
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
        // Loading fields cannot overwrite a running or failed creation owner.
        self.validate_creation_snapshot()?;
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
        if xfer.get_xfer_mode() == XferMode::Load {
            self.state = state.into();
        }

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
        let mut target = self.common_attack_target.load(Ordering::Relaxed);
        xfer.xfer_object_id(&mut target)
            .map_err(|e| e.to_string())?;
        self.common_attack_target.store(target, Ordering::Relaxed);

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

#[cfg(test)]
mod creation_xfer_admission_tests {
    use super::*;
    use game_engine::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
    use std::io::Cursor;

    fn ready_bytes() -> Vec<u8> {
        let mut source = Team::new("WireReady".into(), 42);
        source.state = "Attacking".into();
        source.members = vec![7, 9];
        source.active = true;
        source.created = true;
        source.entered_or_exited = true;
        source.current_waypoint_id = Some(91);
        let mut bytes = Cursor::new(Vec::new());
        source.xfer(&mut XferSave::new(&mut bytes, 1)).unwrap();
        bytes.into_inner()
    }

    fn rejects_before_read(mut target: Team, expected: &str) {
        let wire = ready_bytes();
        target.members = vec![100, 200];
        target.active = false;
        target.created = false;
        target.entered_or_exited = false;
        target.current_waypoint_id = Some(3);
        let before = format!("{target:?}");
        let mut input = Cursor::new(wire);
        {
            let mut load = XferLoad::new(&mut input, 1);
            let error = target.xfer(&mut load).unwrap_err();
            assert!(error.contains(expected), "{error}");
            assert_eq!(
                load.bytes_read(),
                0,
                "no version, ID or roster bytes may be consumed"
            );
        }
        assert_eq!(
            input.position(),
            0,
            "underlying Common XferLoad cursor remains untouched"
        );
        assert_eq!(
            format!("{target:?}"),
            before,
            "roster, gameplay state and exact admission receipt remain unchanged"
        );
    }

    #[test]
    fn running_team_load_rejects_before_cursor_or_roster_mutation() {
        let mut target = Team::new("Running".into(), 42);
        target.begin_creation_actions();
        target.begin_creation_actions();
        rejects_before_read(target, "running creation actions");
    }

    #[test]
    fn failed_team_load_rejects_before_cursor_or_roster_mutation() {
        let mut target = Team::new("Failed".into(), 42);
        target.begin_creation_actions();
        target.finish_creation_actions(Err("original exact failure".into()));
        rejects_before_read(target, "original exact failure");
    }

    #[test]
    fn ready_team_uses_unchanged_actual_common_xfer_roundtrip() {
        let wire = ready_bytes();
        let mut input = Cursor::new(wire.clone());
        let mut target = Team::new("ReceivingLabel".into(), 42);
        target.state = "Receiving".into();
        target.members = vec![1234];
        {
            let mut load = XferLoad::new(&mut input, 1);
            target.xfer(&mut load).unwrap();
            assert_eq!(load.bytes_read(), wire.len() as u64);
        }
        assert_eq!(input.position(), wire.len() as u64);
        assert_eq!(
            target.state.as_str(),
            "Attacking",
            "CPP Team.cpp2613 restores m_state"
        );
        assert_eq!(target.members, [7, 9]);
        assert!(target.active && target.created && target.entered_or_exited);
        assert_eq!(target.current_waypoint_id, Some(91));
        assert!(!target.creation_is_running());
        assert!(target.creation_failure().is_none());
        target.validate_creation_snapshot().unwrap();
        let mut output = Cursor::new(Vec::new());
        target.xfer(&mut XferSave::new(&mut output, 1)).unwrap();
        assert_eq!(
            output.into_inner(),
            wire,
            "Ready wire has no added creation-phase fields"
        );
    }
}
