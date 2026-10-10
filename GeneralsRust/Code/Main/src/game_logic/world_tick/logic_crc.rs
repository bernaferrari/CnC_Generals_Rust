//! Recorder checksum of the authoritative Main simulation, never Core runtime.
//! C++ GameLogic.cpp:3988-4137 names Objects/RNG/Partition/Players/AI components.
//! This is a deterministic Rust checksum, not a retail checksum implementation:
//! object insertion order/full module fields, partition shroud cells, Player
//! battle-plan bonuses and Pathfinder ring/wall/AIGroup fields are still missing.
//! The approved Rust RNG also intentionally has a different state representation.
use super::super::*;
use game_engine::common::crc::Crc;

impl GameLogic {
    /// Read only this match. No callbacks, draws, or ambient simulation lookup.
    pub(crate) fn logic_crc(&self) -> u32 {
        let mut crc = Crc::new();
        crc.compute_crc(&self.frame.to_le_bytes());
        crc.compute_crc(b"MARKER:Objects");
        // Main currently has no original m_objList: stable ID order is the
        // explicit Rust encoding until original object visitation is transferred.
        let mut ids: Vec<_> = self.objects.keys().copied().collect();
        ids.sort_unstable_by_key(|id| id.0);
        for id in ids {
            let object = &self.objects[&id];
            crc.compute_crc(&id.0.to_le_bytes());
            for value in [
                object.get_position().x,
                object.get_position().y,
                object.get_position().z,
                object.health.current,
                object.health.maximum,
            ] {
                crc.compute_crc(&value.to_bits().to_le_bytes());
            }
        }
        for word in self.logic_random.seed_words() {
            crc.compute_crc(&word.to_le_bytes());
        }

        crc.compute_crc(b"MARKER:ThePartitionManager");
        let (width, height, passability) = self.snapshot_pathfinding_passability();
        crc.compute_crc(&width.to_le_bytes());
        crc.compute_crc(&height.to_le_bytes());
        for passable in passability {
            crc.compute_crc(&[u8::from(passable)]);
        }

        crc.compute_crc(b"MARKER:ThePlayerList");
        crc.compute_crc(&(self.players.len() as u32).to_le_bytes());
        let mut players: Vec<_> = self.players.values().collect();
        players.sort_unstable_by_key(|player| player.id);
        for player in players {
            crc.compute_crc(&player.id.to_le_bytes());
            crc.compute_crc(&player.skill_points.to_le_bytes());
            crc.compute_crc(&player.science_purchase_points.to_le_bytes());
            // Canonical Rust economy/controller observations; these additional
            // fields are deliberate Rust encoding, not Player::crc wire parity.
            crc.compute_crc(&player.resources.supplies.to_le_bytes());
            crc.compute_crc(&player.power_available.to_le_bytes());
            crc.compute_crc(&[u8::from(player.is_human), u8::from(player.is_alive)]);
        }

        crc.compute_crc(b"MARKER:TheAI");
        crc.compute_crc(&(self.pathfinding_system.pending_path_count() as u32).to_le_bytes());
        for request in self.pathfinding_system.pending_paths() {
            crc.compute_crc(&request.unit_id.0.to_le_bytes());
            for value in [
                request.destination.x,
                request.destination.y,
                request.destination.z,
            ] {
                crc.compute_crc(&value.to_bits().to_le_bytes());
            }
        }
        self.ai_definitions.fold_crc(&mut crc);
        crc.get()
    }
}
