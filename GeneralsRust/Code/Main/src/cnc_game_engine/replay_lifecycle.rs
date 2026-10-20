//! Receiving Recorder application inputs and original Engine lifecycle boundaries.
use super::*;

impl CnCGameEngine {
    /// Driving application/worker-admission ingress only, never a frame query.
    pub(super) fn capture_recorder_application_inputs(
        map: String,
        seed: u32,
    ) -> game_engine::common::recorder::RecorderOwnerInputs {
        let data = game_engine::common::ini::ini_game_data::ensure_global_data();
        let data = data.read();
        let directory = if data.get_path_user_data().trim().is_empty() {
            let mut path = std::env::current_exe().unwrap_or_default();
            path.pop();
            path
        } else {
            std::path::PathBuf::from(data.get_path_user_data())
        };
        game_engine::common::recorder::RecorderOwnerInputs {
            map,
            seed,
            replay_directory: directory.join("Replays"),
            exe_crc: data.exe_crc,
            ini_crc: data.ini_crc,
            save_stats: data.save_stats,
            stats_directory: std::path::PathBuf::from(data.base_stats_dir.trim()),
        }
    }

    pub(super) fn reset_receiving_recorder_at_engine_boundary(&mut self) {
        let map = self.game_logic.get_current_map_name().to_owned();
        let seed = self.game_logic.logic_base_seed;
        self.game_logic
            .replay_pending
            .reset_recorder_for_match_owned(map, seed);
    }
}
