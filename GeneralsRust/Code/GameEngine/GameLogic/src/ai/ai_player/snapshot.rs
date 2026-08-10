//! Split from `ai/ai_player.rs` for module-size parity.
//! Observable AIPlayer behavior is unchanged.

#![allow(unused_imports)]

use super::*;

impl Snapshot for AIPlayer {
    /// C++ `AIPlayer::crc` is empty (no fields hashed).
    fn crc(&self, _xfer: &mut dyn Xfer) {
        // Intentionally empty — matches GeneralsMD AIPlayer.cpp.
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) {
        let mut version: u8 = 1;
        let _ = xfer.xfer_version(&mut version, 1);

        let mut team_build_queue_count = self.team_build_queue.len() as u16;
        let _ = xfer.xfer_unsigned_short(&mut team_build_queue_count);
        if xfer.is_loading() {
            self.team_build_queue.clear();
            for _ in 0..team_build_queue_count {
                let mut team = TeamInQueue::new();
                team.xfer(xfer);
                self.team_build_queue.push_back(team);
            }
        } else {
            for team in &mut self.team_build_queue {
                team.xfer(xfer);
            }
        }

        let mut team_ready_queue_count = self.team_ready_queue.len() as u16;
        let _ = xfer.xfer_unsigned_short(&mut team_ready_queue_count);
        if xfer.is_loading() {
            self.team_ready_queue.clear();
            for _ in 0..team_ready_queue_count {
                let mut team = TeamInQueue::new();
                team.xfer(xfer);
                self.team_ready_queue.push_back(team);
            }
        } else {
            for team in &mut self.team_ready_queue {
                team.xfer(xfer);
            }
        }

        let mut player_index = self.player_id as i32;
        let _ = xfer.xfer_int(&mut player_index);

        let mut ready_to_build_team = self.ready_to_build_team;
        let _ = xfer.xfer_bool(&mut ready_to_build_team);
        if xfer.is_loading() {
            self.ready_to_build_team = ready_to_build_team;
        }

        let mut ready_to_build_structure = self.ready_to_build_structure;
        let _ = xfer.xfer_bool(&mut ready_to_build_structure);
        if xfer.is_loading() {
            self.ready_to_build_structure = ready_to_build_structure;
        }

        let mut team_timer = self.team_timer as i32;
        let _ = xfer.xfer_int(&mut team_timer);
        if xfer.is_loading() {
            self.team_timer = team_timer as u32;
        }

        let mut structure_timer = self.structure_timer as i32;
        let _ = xfer.xfer_int(&mut structure_timer);
        if xfer.is_loading() {
            self.structure_timer = structure_timer as u32;
        }

        let mut build_delay = self.build_delay as i32;
        let _ = xfer.xfer_int(&mut build_delay);
        if xfer.is_loading() {
            self.build_delay = build_delay as u32;
        }

        let mut team_delay = self.team_delay as i32;
        let _ = xfer.xfer_int(&mut team_delay);
        if xfer.is_loading() {
            self.team_delay = team_delay as u32;
        }

        let mut team_seconds = self.team_seconds.round() as i32;
        let _ = xfer.xfer_int(&mut team_seconds);
        if xfer.is_loading() {
            self.team_seconds = team_seconds as Real;
        }

        let mut cur_warehouse_id = self.current_warehouse_id.unwrap_or(INVALID_ID);
        let _ = xfer.xfer_object_id(&mut cur_warehouse_id);
        if xfer.is_loading() {
            self.current_warehouse_id = if cur_warehouse_id == INVALID_ID {
                None
            } else {
                Some(cur_warehouse_id)
            };
        }

        let mut frame_last_building_built = self.frame_last_building_built as i32;
        let _ = xfer.xfer_int(&mut frame_last_building_built);
        if xfer.is_loading() {
            self.frame_last_building_built = frame_last_building_built as u32;
        }

        let mut difficulty = self.difficulty as i32;
        let _ = xfer.xfer_int(&mut difficulty);
        if xfer.is_loading() {
            self.difficulty = match difficulty {
                0 => GameDifficulty::Easy,
                1 => GameDifficulty::Normal,
                2 => GameDifficulty::Hard,
                3 => GameDifficulty::Brutal,
                _ => GameDifficulty::Normal,
            };
        }

        let mut skillset_selector = self.skillset_selector;
        let _ = xfer.xfer_int(&mut skillset_selector);
        if xfer.is_loading() {
            self.skillset_selector = skillset_selector;
        }

        xfer.xfer_coord3d(&mut self.base_center);

        let mut base_center_set = self.base_center_set;
        let _ = xfer.xfer_bool(&mut base_center_set);
        if xfer.is_loading() {
            self.base_center_set = base_center_set;
        }

        let mut base_radius = self.base_radius;
        let _ = xfer.xfer_real(&mut base_radius);
        if xfer.is_loading() {
            self.base_radius = base_radius;
        }

        for i in 0..MAX_STRUCTURES_TO_REPAIR {
            let mut id = self.structures_to_repair[i].unwrap_or(INVALID_ID);
            let _ = xfer.xfer_object_id(&mut id);
            if xfer.is_loading() {
                self.structures_to_repair[i] = if id == INVALID_ID { None } else { Some(id) };
            }
        }

        let mut repair_dozer = self.repair_dozer.unwrap_or(INVALID_ID);
        let _ = xfer.xfer_object_id(&mut repair_dozer);
        if xfer.is_loading() {
            self.repair_dozer = if repair_dozer == INVALID_ID {
                None
            } else {
                Some(repair_dozer)
            };
        }

        let mut structures_in_queue = self.structures_in_queue;
        let _ = xfer.xfer_int(&mut structures_in_queue);
        if xfer.is_loading() {
            self.structures_in_queue = structures_in_queue;
        }

        let mut dozer_queued_for_repair = self.dozer_queued_for_repair;
        let _ = xfer.xfer_bool(&mut dozer_queued_for_repair);
        if xfer.is_loading() {
            self.dozer_queued_for_repair = dozer_queued_for_repair;
        }

        let mut dozer_is_repairing = self.dozer_is_repairing;
        let _ = xfer.xfer_bool(&mut dozer_is_repairing);
        if xfer.is_loading() {
            self.dozer_is_repairing = dozer_is_repairing;
        }

        let mut bridge_timer = self.bridge_timer as i32;
        let _ = xfer.xfer_int(&mut bridge_timer);
        if xfer.is_loading() {
            self.bridge_timer = bridge_timer as u32;
        }
    }

    fn load_post_process(&mut self) {}
}
