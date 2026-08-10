//! WorldSnapshot Snapshot trait (crc / xfer / load_post_process).

use super::xfer_helpers::{
    default_ai_economic_state, default_ai_strategic_state, default_ai_tactical_state,
    default_object_snapshot, default_player_snapshot, xfer_vec_default,
};
use super::*;
use crate::game_logic::*;
use crate::save_load::{SaveLoadError, SaveLoadResult, Xfer, XferData, XferMode};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::SystemTime;

// Implement Snapshot trait for WorldSnapshot
impl Snapshot for WorldSnapshot {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        // Light CRC - just check critical values
        self.version.xfer(xfer)?;
        self.frame_number.xfer(xfer)?;
        self.random_seed.xfer(xfer)?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("WorldSnapshot")?;

        xfer.xfer_marker_label("Version")?;
        self.version.xfer(xfer)?;

        xfer.xfer_marker_label("Timestamp")?;
        let duration = self
            .timestamp
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap_or_default();
        let mut secs = duration.as_secs();
        let mut nanos = duration.subsec_nanos();
        xfer.xfer_u64(&mut secs)?;
        xfer.xfer_u32(&mut nanos)?;
        self.timestamp = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::new(secs, nanos);

        xfer.xfer_marker_label("FrameNumber")?;
        self.frame_number.xfer(xfer)?;

        xfer.xfer_marker_label("RandomSeed")?;
        self.random_seed.xfer(xfer)?;

        xfer.xfer_marker_label("Objects")?;
        let mut len = self.objects.len() as u32;
        xfer.xfer_u32(&mut len)?;
        if xfer.get_mode() == XferMode::Load {
            self.objects.clear();
            for _ in 0..len {
                let mut id = ObjectId(0);
                id.xfer(xfer)?;
                let mut obj = default_object_snapshot();
                obj.xfer(xfer)?;
                self.objects.insert(id, obj);
            }
        } else {
            for (id, obj) in &mut self.objects {
                let mut id_copy = *id;
                id_copy.xfer(xfer)?;
                obj.xfer(xfer)?;
            }
        }

        xfer.xfer_marker_label("Players")?;
        xfer_vec_default(xfer, &mut self.players, default_player_snapshot())?;

        xfer.xfer_marker_label("Teams")?;
        xfer_vec_default(
            xfer,
            &mut self.teams,
            TeamSnapshot {
                team: Team::Neutral,
                players: Vec::new(),
                allied_teams: Vec::new(),
                is_defeated: false,
                shared_vision: false,
                shared_control: false,
            },
        )?;

        xfer.xfer_marker_label("Terrain")?;
        self.terrain.xfer(xfer)?;

        xfer.xfer_marker_label("Weather")?;
        self.weather.xfer(xfer)?;

        xfer.xfer_marker_label("ResourceManager")?;
        self.resource_manager.xfer(xfer)?;

        xfer.xfer_marker_label("CombatTracker")?;
        self.combat_tracker.xfer(xfer)?;

        xfer.xfer_marker_label("ExperienceTracker")?;
        self.experience_tracker.xfer(xfer)?;

        xfer.xfer_marker_label("PathfindingCache")?;
        self.pathfinding_cache.xfer(xfer)?;

        xfer.xfer_marker_label("AIPlayers")?;
        xfer_vec_default(
            xfer,
            &mut self.ai_players,
            AIPlayerSnapshot {
                player_id: 0,
                difficulty: String::new(),
                personality: String::new(),
                current_strategy: String::new(),
                strategic_state: default_ai_strategic_state(),
                tactical_state: default_ai_tactical_state(),
                economic_state: default_ai_economic_state(),
            },
        )?;

        xfer.xfer_marker_label("GlobalAIState")?;
        self.global_ai_state.xfer(xfer)?;

        // Residual: host superweapon strike queue + combat particle registry.
        // Appended after GlobalAIState so earlier Xfer layouts stay stable until
        // a save that writes these markers. Empty defaults on missing streams
        // are handled by callers using Default; binary Xfer always pairs them.
        xfer.xfer_marker_label("SpecialPowerStrikes")?;
        self.special_power_strikes.xfer(xfer)?;

        xfer.xfer_marker_label("CombatParticles")?;
        self.combat_particles.xfer(xfer)?;

        xfer.xfer_marker_label("HostUpgrades")?;
        self.host_upgrades.xfer(xfer)?;

        Ok(())
    }

    fn load_post_process(&mut self) -> SaveLoadResult<()> {
        // Rebuild any transient state after loading
        // Reconnect object references, rebuild caches, etc.
        Ok(())
    }
}

