//! WorldSnapshot Snapshot trait (crc / xfer / load_post_process).

use super::xfer_helpers::{
    default_ai_economic_state, default_ai_strategic_state, default_ai_tactical_state,
    default_object_snapshot, default_player_snapshot, xfer_vec_default,
};
use super::*;
use crate::game_logic::*;
use crate::save_load::{SaveLoadError, SaveLoadResult, Xfer, XferData, XferMode};
use gamelogic::system::shroud_manager::ShroudSnapshot;
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
        // A direct Xfer stream is positional and marker labels write no bytes.
        // Reject an invalid writer before emitting a partial record; on load,
        // validate immediately after the raw u32 and before timestamp/object
        // payload bytes can be consumed under the wrong layout.
        if xfer.get_mode() != XferMode::Load {
            validate_direct_world_snapshot_version(self.version)?;
        }
        self.version.xfer(xfer)?;
        if xfer.get_mode() == XferMode::Load {
            validate_direct_world_snapshot_version(self.version)?;
        }

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
                obj.xfer_for_world_version(xfer, self.version)?;
                self.objects.insert(id, obj);
            }
        } else {
            for (id, obj) in &mut self.objects {
                let mut id_copy = *id;
                id_copy.xfer(xfer)?;
                obj.xfer_for_world_version(xfer, self.version)?;
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
                is_active: true,
                base_center: None,
                base_radius: 0.0,
                activity_count: 0,
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

        if self.version >= WORLD_SNAPSHOT_DIRECT_XFER_V4_TAIL_VERSION {
            xfer.xfer_marker_label("NextWeaponDischargeSequence")?;
            xfer.xfer_u64(&mut self.next_weapon_discharge_sequence)?;
            if xfer.get_mode() == XferMode::Load {
                // Sequence zero is reserved only for an Object's unseen
                // discharge marker; the world counter always denotes the next
                // usable sequence.
                self.next_weapon_discharge_sequence = self
                    .next_weapon_discharge_sequence
                    .max(default_next_weapon_discharge_sequence());
            }
            xfer.xfer_marker_label("ClientDrawables")?;
            self.client_drawables.xfer(xfer)?;
        } else if xfer.get_mode() == XferMode::Load {
            self.next_weapon_discharge_sequence = default_next_weapon_discharge_sequence();
            self.client_drawables = ClientDrawableWorldSnapshot::default();
        }

        if self.version >= WORLD_SNAPSHOT_DIRECT_XFER_V5_TAIL_VERSION {
            xfer.xfer_marker_label("PlayerTemplateBindings")?;
            xfer_vec_default(
                xfer,
                &mut self.player_template_bindings,
                PlayerTemplateBindingSnapshot {
                    player_id: 0,
                    template_name: String::new(),
                    template_index: 0,
                },
            )?;
        } else if xfer.get_mode() == XferMode::Load {
            self.player_template_bindings.clear();
        }

        if self.version >= WORLD_SNAPSHOT_DIRECT_XFER_V6_TAIL_VERSION {
            xfer.xfer_marker_label("Shroud")?;
            self.shroud.xfer(xfer)?;
        } else if xfer.get_mode() == XferMode::Load {
            self.shroud = ShroudSnapshot::default();
        }

        if self.version >= WORLD_SNAPSHOT_DIRECT_XFER_V9_TAIL_VERSION {
            xfer.xfer_marker_label("LifecycleTail")?;
            super::xfer_helpers::xfer_vec_u8(xfer, &mut self.lifecycle_tail)?;
        } else if xfer.get_mode() == XferMode::Load {
            self.lifecycle_tail.clear();
        }

        if self.version >= WORLD_SNAPSHOT_DIRECT_XFER_V10_TAIL_VERSION {
            xfer.xfer_marker_label("PlayerRanks")?;
            xfer_vec_default(
                xfer,
                &mut self.player_ranks,
                PlayerRankSnapshot {
                    player_id: 0,
                    rank_level: 1,
                    skill_points: 0,
                    science_purchase_points: 0,
                },
            )?;
        } else if xfer.get_mode() == XferMode::Load {
            self.player_ranks.clear();
        }

        if self.version >= WORLD_SNAPSHOT_DIRECT_XFER_V11_TAIL_VERSION {
            xfer.xfer_marker_label("ObjectInstanceGuards")?;
            xfer_vec_default(
                xfer,
                &mut self.object_instance_guards,
                ObjectInstanceGuardSnapshot {
                    object_id: ObjectId(0),
                    instance_name: String::new(),
                    guard_position: None,
                    guard_target: None,
                    guard_radius: 0.0,
                    guard_mode: GuardMode::Normal,
                },
            )?;
        } else if xfer.get_mode() == XferMode::Load {
            self.object_instance_guards.clear();
        }

        if self.version >= WORLD_SNAPSHOT_DIRECT_XFER_V12_TAIL_VERSION {
            xfer.xfer_marker_label("OverchargeActive")?;
            xfer_vec_default(
                xfer,
                &mut self.overcharge_active,
                ObjectOverchargeSnapshot {
                    object_id: ObjectId(0),
                    overcharge_enabled: false,
                },
            )?;
        } else if xfer.get_mode() == XferMode::Load {
            self.overcharge_active.clear();
        }

        if self.version >= WORLD_SNAPSHOT_DIRECT_XFER_V13_TAIL_VERSION {
            xfer.xfer_marker_label("CiaIntelligence")?;
            self.cia_intelligence.xfer(xfer)?;
            xfer.xfer_marker_label("VisionSpied")?;
            xfer_vec_default(
                xfer,
                &mut self.vision_spied,
                ObjectVisionSpiedSnapshot {
                    object_id: ObjectId(0),
                    vision_spied_mask: 0,
                },
            )?;
            xfer.xfer_marker_label("BuilderTasks")?;
            xfer_vec_default(
                xfer,
                &mut self.builder_tasks,
                ObjectBuilderTaskSnapshot {
                    object_id: ObjectId(0),
                    builder_id: None,
                    dozer_task_build_target: None,
                    dozer_task_build_order_frame: 0,
                },
            )?;
            xfer.xfer_marker_label("SellList")?;
            xfer_vec_default(
                xfer,
                &mut self.sell_list,
                SellListEntrySnapshot {
                    object_id: ObjectId(0),
                    sell_frame: 0,
                },
            )?;
        } else if xfer.get_mode() == XferMode::Load {
            self.cia_intelligence =
                crate::game_logic::host_cia_intelligence::HostCiaIntelligenceRegistry::new();
            self.vision_spied.clear();
            self.builder_tasks.clear();
            self.sell_list.clear();
        }

        if self.version >= WORLD_SNAPSHOT_DIRECT_XFER_V14_TAIL_VERSION {
            xfer.xfer_marker_label("ObjectPersist")?;
            xfer_vec_default(
                xfer,
                &mut self.object_persist,
                ObjectPersistTailSnapshot {
                    object_id: ObjectId(0),
                    sole_healing_benefactor: None,
                    sole_healing_benefactor_expiration_frame: 0,
                    contained_by_frame: None,
                    original_team: None,
                    formation_id: 0,
                    formation_offset: [0.0, 0.0],
                    stealth_opacity: 1.0,
                    terrain_decal_type: 8,
                    terrain_decal_size: 0.0,
                },
            )?;
            xfer.xfer_marker_label("ClientDrawableVisuals")?;
            xfer_vec_default(
                xfer,
                &mut self.client_drawable_visuals,
                ClientDrawableVisualSnapshot {
                    object_id: 0,
                    draw_module_index: 0,
                    hidden: false,
                    hidden_by_stealth: false,
                    stealth_opacity: 1.0,
                    effective_opacity: 1.0,
                    loco_pitch: 0.0,
                    loco_roll: 0.0,
                    expiration_date: 0,
                    terrain_decal: 8,
                },
            )?;
        } else if xfer.get_mode() == XferMode::Load {
            self.object_persist.clear();
            self.client_drawable_visuals.clear();
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> SaveLoadResult<()> {
        // Rebuild any transient state after loading
        // Reconnect object references, rebuild caches, etc.
        Ok(())
    }
}
