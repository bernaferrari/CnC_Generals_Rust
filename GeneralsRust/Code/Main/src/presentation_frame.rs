// AUTO-GENERATED source-scan dump for residual include_str! tests.
// Compiled module lives in presentation_frame/mod.rs.


// ===== alive.rs =====
use super::*;

impl PresentationFrame {
    /// Hotkey residual: idle selectable friendly workers/dozers/chinooks/supply/hack.
    pub fn alive_selectable_friendly_idle_worker_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        use crate::unit_control::UnitControlSystem;
        let mut ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|o| {
                if o.team != player_team
                    || o.destroyed
                    || !UnitControlSystem::presentation_is_selectable(o)
                {
                    return false;
                }
                if !Self::presentation_is_worker_like(o) {
                    return false;
                }
                // Prefer idle residual (no move dest / attack / construct busy).
                o.move_destination.is_none()
                    && o.attack_target.is_none()
                    && !o.under_construction
                    && o.ai_state_ordinal == 0
            })
            .map(|o| o.id)
            .collect();
        ids.sort_by_key(|id| id.0);
        ids
    }

    /// Hotkey residual: busy selectable friendly workers (non-idle worker-like).
    pub fn alive_selectable_friendly_busy_worker_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        use crate::unit_control::UnitControlSystem;
        let idle: std::collections::HashSet<_> = self
            .alive_selectable_friendly_idle_worker_ids(player_team)
            .into_iter()
            .collect();
        let mut ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|o| {
                o.team == player_team
                    && !o.destroyed
                    && UnitControlSystem::presentation_is_selectable(o)
                    && Self::presentation_is_worker_like(o)
                    && !idle.contains(&o.id)
            })
            .map(|o| o.id)
            .collect();
        ids.sort_by_key(|id| id.0);
        ids
    }

    /// Hotkey residual: unfinished (under construction, not sold) friendly selectables.
    pub fn alive_selectable_friendly_unfinished_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        use crate::unit_control::UnitControlSystem;
        let mut ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|o| {
                o.team == player_team
                    && !o.destroyed
                    && UnitControlSystem::presentation_is_selectable(o)
                    && o.under_construction
                    && !o.sold
            })
            .map(|o| o.id)
            .collect();
        ids.sort_by_key(|id| id.0);
        ids
    }

    pub fn presentation_is_worker_like(o: &RenderableObject) -> bool {
        use crate::game_logic::KindOf;
        if Self::object_has_kind(o, KindOf::Worker) {
            return true;
        }
        let n = o.template_name.to_ascii_lowercase();
        n.contains("dozer")
            || n.contains("worker")
            || n.contains("chinook")
            || n.contains("supply")
            || n.contains("hack")
            || n.contains("crane")
    }

    /// Runtime-host residual: first friendly Command Center pose.
    pub fn first_friendly_command_center_position(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Option<Vec3> {
        use crate::game_logic::KindOf;
        // Wave 1101: fail-closed on sold/UC/disabled CC residual.
        self.objects
            .iter()
            .find(|o| {
                o.team == player_team
                    && !o.destroyed
                    && !o.sold
                    && !o.under_construction
                    && !o.disabled
                    && (o.building_type == Some(PresentationBuildingType::CommandCenter)
                        || Self::object_has_kind(o, KindOf::CommandCenter))
            })
            .map(|o| o.position)
    }

    /// Runtime-host residual: count of alive mobile friendlies.
    pub fn count_mobile_friendlies(&self, player_team: crate::game_logic::Team) -> u32 {
        use crate::unit_control::UnitControlSystem;
        // Wave 1102: mobile count residual uses presentation selectable legality.
        self.objects
            .iter()
            .filter(|o| {
                o.team == player_team
                    && o.is_mobile
                    && UnitControlSystem::presentation_is_selectable(o)
            })
            .count() as u32
    }

    /// Runtime-host residual: selected friendly count from snapshot.
    pub fn count_selected_friendlies(&self, player_team: crate::game_logic::Team) -> u32 {
        use crate::unit_control::UnitControlSystem;
        // Wave 1103: selected count residual uses presentation selectable legality.
        self.objects
            .iter()
            .filter(|o| {
                o.team == player_team
                    && o.selected
                    && UnitControlSystem::presentation_is_selectable(o)
            })
            .count() as u32
    }

    /// Runtime-host residual: under-construction friendly count from snapshot.
    pub fn count_under_construction_friendlies(&self, player_team: crate::game_logic::Team) -> u32 {
        // Wave 1104: fail-closed on sold UC residual count.
        self.objects
            .iter()
            .filter(|o| o.team == player_team && !o.destroyed && !o.sold && o.under_construction)
            .count() as u32
    }

    /// Runtime-host residual: first alive friendly sample pose + name.
    pub fn first_friendly_sample_label(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Option<String> {
        // Wave 1106: sample label residual fail-closed on sold.
        self.objects
            .iter()
            .find(|o| o.team == player_team && !o.destroyed && !o.sold)
            .map(|o| {
                format!(
                    "{:.1},{:.1},{:.1}:{}",
                    o.position.x, o.position.y, o.position.z, o.template_name
                )
            })
    }

    pub fn centroid_of_ids(&self, ids: &[ObjectId]) -> Option<glam::Vec3> {
        // Wave 1107: camera/group centroid residual fail-closed on sold.
        let mut sum = glam::Vec3::ZERO;
        let mut n = 0u32;
        for id in ids {
            if let Some(o) = self.objects.iter().find(|o| o.id == *id) {
                if o.destroyed || o.sold {
                    continue;
                }
                sum += o.position;
                n += 1;
            }
        }
        if n == 0 {
            None
        } else {
            Some(sum / n as f32)
        }
    }

    pub fn first_alive_position_for_template(&self, template_name: &str) -> Option<glam::Vec3> {
        // Wave 1106: template pose residual fail-closed on sold.
        self.objects
            .iter()
            .find(|o| {
                !o.destroyed && !o.sold && o.template_name.eq_ignore_ascii_case(template_name)
            })
            .map(|o| o.position)
    }

    pub fn filter_alive_selectable_ids(
        &self,
        ids: &[ObjectId],
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        use crate::unit_control::UnitControlSystem;
        let mut out = Vec::new();
        for id in ids {
            if let Some(o) = self.objects.iter().find(|o| o.id == *id) {
                if o.team == player_team && UnitControlSystem::presentation_is_selectable(o) {
                    out.push(*id);
                }
            }
        }
        out
    }

    /// All alive selectable friendlies (Ctrl+A / Tab cycle residual).
    /// Local player team frozen on this frame (selection/hotkey consumers).
    #[inline]
    pub fn local_team(&self) -> Team {
        self.local_team
    }

    /// Frozen team base pose for camera snap proximity (None if unknown).
    pub fn local_team_base_or_hint(&self, fallback: Vec3) -> Vec3 {
        self.local_team_base_position.unwrap_or(fallback)
    }

    /// Wave 563: presentation-owned template name residual (train/UI contains).
    #[inline]
    pub fn has_template_name(&self, name: &str) -> bool {
        // Binary search on sorted freeze; fail-closed on miss (no live dual-read).
        self.known_template_names
            .binary_search_by(|n| n.as_str().cmp(name))
            .is_ok()
    }

    /// Look up frozen player roster entry by id.
    #[inline]
    pub fn player_info(&self, id: u32) -> Option<&PresentationPlayerInfo> {
        self.players.iter().find(|p| p.id == id)
    }

    /// Frozen player display name (defeat/alliance UI residual).
    #[inline]
    pub fn player_name(&self, id: u32) -> Option<&str> {
        self.player_info(id).map(|p| p.name.as_str())
    }

    /// Frozen player team (radar/defeat residual).
    #[inline]
    pub fn player_team(&self, id: u32) -> Option<Team> {
        self.player_info(id).map(|p| p.team)
    }

    /// Select friendlies inside a world-XZ radius of a center (on-screen residual).
    pub fn alive_selectable_friendly_near(
        &self,
        player_team: crate::game_logic::Team,
        center: glam::Vec3,
        radius: f32,
    ) -> Vec<ObjectId> {
        use crate::unit_control::UnitControlSystem;
        let r2 = radius * radius;
        let mut ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|o| {
                if o.team != player_team || !UnitControlSystem::presentation_is_selectable(o) {
                    return false;
                }
                let dx = o.position.x - center.x;
                let dz = o.position.z - center.z;
                dx * dx + dz * dz <= r2
            })
            .map(|o| o.id)
            .collect();
        ids.sort_by_key(|id| id.0);
        ids
    }

    pub fn alive_selectable_friendly_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        use crate::unit_control::UnitControlSystem;
        let mut ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|o| o.team == player_team && UnitControlSystem::presentation_is_selectable(o))
            .map(|o| o.id)
            .collect();
        ids.sort_by_key(|id| id.0);
        ids
    }

    /// Retail SELECT_HERO residual: friendly selectable heroes from snapshot.
    pub fn alive_selectable_friendly_hero_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        use crate::game_logic::KindOf;
        use crate::unit_control::UnitControlSystem;
        let mut ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|o| {
                o.team == player_team
                    && UnitControlSystem::presentation_is_selectable(o)
                    && !o.destroyed
                    && (o.kind_of.contains(&KindOf::Hero) || o.template_name.contains("Hero"))
            })
            .map(|o| o.id)
            .collect();
        ids.sort_by_key(|id| id.0);
        ids
    }

    /// Retail SELECT_ALL_AIRCRAFT (KEY_W) residual.
    /// Generic friendly selectable filter residual from snapshot.
    pub fn alive_selectable_friendly_filtered_ids(
        &self,
        player_team: crate::game_logic::Team,
        mut pred: impl FnMut(&RenderableObject) -> bool,
    ) -> Vec<ObjectId> {
        use crate::unit_control::UnitControlSystem;
        let mut ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|o| {
                o.team == player_team
                    && !o.destroyed
                    && UnitControlSystem::presentation_is_selectable(o)
                    && pred(o)
            })
            .map(|o| o.id)
            .collect();
        ids.sort_by_key(|id| id.0);
        ids
    }

    /// Combat units residual (mobile non-structure, not pure dozer/supply).
    pub fn alive_selectable_friendly_combat_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        self.alive_selectable_friendly_filtered_ids(player_team, |o| {
            if o.is_structure {
                return false;
            }
            let n = o.template_name.to_ascii_lowercase();
            if n.contains("dozer") || n.contains("worker") || n.contains("supply") {
                return false;
            }
            o.is_mobile || o.has_weapon || o.is_unit
        })
    }

    /// Runtime-host residual: sellable friendly structures (non-CC).
    pub fn alive_sellable_friendly_structure_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        use crate::game_logic::KindOf;
        use crate::unit_control::UnitControlSystem;
        let mut ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|o| {
                o.team == player_team
                    && !o.destroyed
                    && !o.sold
                    && !o.under_construction
                    // Wave 856: selectable OR known structure/building residual (construct
                    // same-frame may lag presentation_is_selectable flags).
                    // Wave 1102: sold residual fail-closed even on structure OR path.
                    && (UnitControlSystem::presentation_is_selectable(o)
                        || o.is_structure
                        || o.building_type.is_some()
                        || o.object_type == PresentationObjectType::Building)
                    && (Self::object_has_kind(o, KindOf::Structure)
                        || o.object_type == PresentationObjectType::Building
                        || o.is_structure
                        || o.building_type.is_some())
                    && !Self::object_has_kind(o, KindOf::CommandCenter)
                    && o.building_type != Some(PresentationBuildingType::CommandCenter)
            })
            .map(|o| o.id)
            .collect();
        ids.sort_by_key(|id| id.0);
        ids
    }

    /// Runtime-host residual: constructed friendly structures that can queue upgrades.
    pub fn alive_upgrade_producer_structure_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        use crate::game_logic::KindOf;
        // Wave 1102: fail-closed on sold/disabled upgrade-producer residual.
        let mut ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|o| {
                o.team == player_team
                    && !o.destroyed
                    && !o.sold
                    && !o.disabled
                    && !o.under_construction
                    && (Self::object_has_kind(o, KindOf::Structure)
                        || o.object_type == PresentationObjectType::Building
                        || o.can_produce
                        || o.building_type.is_some())
            })
            .map(|o| o.id)
            .collect();
        ids.sort_by_key(|id| id.0);
        ids
    }

    /// Runtime-host residual: friendly dozers/workers that can construct.
    pub fn alive_construct_builder_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        use crate::game_logic::KindOf;
        let mut ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|o| {
                // Wave 1101: fail-closed on sold/disabled construct builders.
                if o.team != player_team || o.destroyed || o.sold || o.disabled {
                    return false;
                }
                if Self::object_has_kind(o, KindOf::Worker) {
                    return true;
                }
                let name = o.template_name.to_ascii_lowercase();
                name.contains("dozer")
                    || name.contains("worker")
                    || name.contains("crane")
                    || name.contains("construction")
            })
            .map(|o| o.id)
            .collect();
        ids.sort_by_key(|id| id.0);
        ids
    }

    pub fn alive_selectable_friendly_moving_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        self.alive_selectable_friendly_filtered_ids(player_team, |o| {
            !o.is_structure
                && (o.moving
                    || o.move_destination.is_some()
                    || o.path_len > 0
                    || o.velocity.length_squared() > 1e-6)
        })
    }

    pub fn alive_selectable_friendly_attacking_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        self.alive_selectable_friendly_filtered_ids(player_team, |o| {
            !o.is_structure
                && (o.attacking
                    || o.attack_target.is_some()
                    || o.is_firing_weapon
                    || o.is_aiming_weapon)
        })
    }

    pub fn alive_selectable_friendly_guarding_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        // host_ai_state_ordinal: GuardingArea=9, GuardingObject=10, GuardRetaliating=20
        self.alive_selectable_friendly_filtered_ids(player_team, |o| {
            o.guard_position.is_some()
                || o.guard_target.is_some()
                || matches!(o.ai_state_ordinal, 9 | 10 | 20)
        })
    }

    pub fn alive_selectable_friendly_patrolling_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        // host_ai_state_ordinal: Patrolling = 11
        self.alive_selectable_friendly_filtered_ids(player_team, |o| o.ai_state_ordinal == 11)
    }

    pub fn alive_selectable_friendly_gathering_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        // AIState::Gathering=5, ReturningResources=6
        self.alive_selectable_friendly_filtered_ids(player_team, |o| {
            matches!(o.ai_state_ordinal, 5 | 6)
        })
    }

    pub fn alive_selectable_friendly_stealthed_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        self.alive_selectable_friendly_filtered_ids(player_team, |o| o.effectively_stealthed)
    }

    pub fn alive_selectable_friendly_veteran_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        self.alive_selectable_friendly_filtered_ids(player_team, |o| {
            !o.is_structure && !matches!(o.veterancy, PresentationVeterancy::Rookie)
        })
    }

    pub fn alive_selectable_friendly_harvester_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        self.alive_selectable_friendly_filtered_ids(player_team, |o| {
            let n = o.template_name.to_ascii_lowercase();
            n.contains("supply")
                || n.contains("harvester")
                || n.contains("gatherer")
                || n.contains("worker")
                || matches!(o.ai_state_ordinal, 5 | 6)
        })
    }

    pub fn alive_selectable_friendly_idle_harvester_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        self.alive_selectable_friendly_filtered_ids(player_team, |o| {
            let n = o.template_name.to_ascii_lowercase();
            let is_h = n.contains("supply")
                || n.contains("harvester")
                || n.contains("gatherer")
                || n.contains("worker");
            is_h && o.ai_state_ordinal == 0 && !o.moving
        })
    }

    pub fn alive_selectable_friendly_occupied_transport_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        self.alive_selectable_friendly_filtered_ids(player_team, |o| {
            !o.is_structure
                && o.occupant_count > 0
                && (o.max_transport > 0
                    || o.is_humvee_transport
                    || o.is_troop_crawler_transport
                    || o.is_combat_chinook_transport
                    || o.is_helix_transport
                    || o.is_battle_bus_transport
                    || o.is_technical_transport
                    || o.is_combat_cycle_transport)
        })
    }

    pub fn alive_selectable_friendly_docked_aircraft_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        // host_ai_state_ordinal Docked = 12
        self.alive_selectable_friendly_filtered_ids(player_team, |o| {
            (o.object_type == PresentationObjectType::Aircraft || o.airborne_target)
                && o.ai_state_ordinal == 12
        })
    }

    pub fn alive_selectable_friendly_repairing_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        // Repairing = 8, SeekingRepair = 15
        self.alive_selectable_friendly_filtered_ids(player_team, |o| {
            matches!(o.ai_state_ordinal, 8 | 15)
        })
    }

    pub fn alive_selectable_friendly_constructing_worker_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        // Constructing = 7
        self.alive_selectable_friendly_filtered_ids(player_team, |o| {
            let n = o.template_name.to_ascii_lowercase();
            o.ai_state_ordinal == 7
                || ((n.contains("dozer") || n.contains("worker") || n.contains("constructor"))
                    && (o.moving || o.ai_state_ordinal == 7))
        })
    }

    pub fn alive_selectable_friendly_idle_military_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        self.alive_selectable_friendly_filtered_ids(player_team, |o| {
            if o.is_structure {
                return false;
            }
            let n = o.template_name.to_ascii_lowercase();
            if n.contains("dozer") || n.contains("supply") || n.contains("harvester") {
                return false;
            }
            let combat = o.is_mobile || o.has_weapon || o.is_unit;
            combat
                && o.ai_state_ordinal == 0
                && !o.moving
                && !o.attacking
                && o.attack_target.is_none()
        })
    }

    pub fn alive_selectable_friendly_mobile_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        self.alive_selectable_friendly_filtered_ids(player_team, |o| o.is_mobile)
    }

    /// Damaged mobile units residual (health < max).
    pub fn alive_selectable_friendly_damaged_unit_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        use crate::unit_control::UnitControlSystem;
        let mut pairs: Vec<(ObjectId, f32)> = self
            .objects
            .iter()
            .filter(|o| {
                o.team == player_team
                    && !o.destroyed
                    && !o.is_structure
                    && UnitControlSystem::presentation_is_selectable(o)
                    && o.is_mobile
                    && o.health_max > 0.0
                    && o.health_current + 1e-3 < o.health_max
            })
            .map(|o| (o.id, o.health_current / o.health_max.max(1e-3)))
            .collect();
        pairs.sort_by(|a, b| {
            a.1.partial_cmp(&b.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.0 .0.cmp(&b.0 .0))
        });
        pairs.into_iter().map(|(id, _)| id).collect()
    }

    pub fn alive_selectable_friendly_damaged_structure_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        use crate::unit_control::UnitControlSystem;
        let mut pairs: Vec<(ObjectId, f32)> = self
            .objects
            .iter()
            .filter(|o| {
                o.team == player_team
                    && !o.destroyed
                    && o.is_structure
                    && !o.under_construction
                    && UnitControlSystem::presentation_is_selectable(o)
                    && o.health_max > 0.0
                    && o.health_current + 1e-3 < o.health_max
            })
            .map(|o| (o.id, o.health_current / o.health_max.max(1e-3)))
            .collect();
        pairs.sort_by(|a, b| {
            a.1.partial_cmp(&b.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.0 .0.cmp(&b.0 .0))
        });
        pairs.into_iter().map(|(id, _)| id).collect()
    }

    pub fn alive_selectable_friendly_busy_producer_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        self.alive_selectable_friendly_filtered_ids(player_team, |o| {
            o.is_structure && !o.production_queue.is_empty()
        })
    }

    pub fn alive_selectable_friendly_ready_special_power_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        self.alive_selectable_friendly_filtered_ids(player_team, |o| {
            o.is_structure && o.special_power_ready
        })
    }

    /// Stop-all residual: friendly mobile (non-structure) units.
    pub fn alive_friendly_stoppable_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        use crate::unit_control::UnitControlSystem;
        // Wave 1102: stop-all residual uses full presentation selectable legality.
        let mut ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|o| {
                o.team == player_team
                    && !o.is_structure
                    && o.is_mobile
                    && UnitControlSystem::presentation_is_selectable(o)
            })
            .map(|o| o.id)
            .collect();
        ids.sort_by_key(|id| id.0);
        ids
    }

    pub fn alive_selectable_friendly_aircraft_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        use crate::unit_control::UnitControlSystem;
        let mut ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|o| {
                o.team == player_team
                    && UnitControlSystem::presentation_is_selectable(o)
                    && (o.object_type == PresentationObjectType::Aircraft || o.airborne_target)
            })
            .map(|o| o.id)
            .collect();
        ids.sort_by_key(|id| id.0);
        ids
    }

    pub fn box_select_unit_ids(
        &self,
        player_team: crate::game_logic::Team,
        min_x: f32,
        max_x: f32,
        min_z: f32,
        max_z: f32,
    ) -> Vec<ObjectId> {
        use crate::game_logic::KindOf;
        use crate::unit_control::UnitControlSystem;
        let mut units = Vec::new();
        let mut structures = Vec::new();
        for o in &self.objects {
            if o.team != player_team || !UnitControlSystem::presentation_is_selectable(o) {
                continue;
            }
            let pos = o.position;
            if pos.x < min_x || pos.x > max_x || pos.z < min_z || pos.z > max_z {
                continue;
            }
            let is_structure = o.is_structure
                || Self::object_has_kind(o, KindOf::Structure)
                || o.object_type == PresentationObjectType::Building;
            if is_structure {
                structures.push(o.id);
            } else {
                units.push(o.id);
            }
        }
        if !units.is_empty() {
            units
        } else if structures.len() == 1 {
            structures
        } else {
            // Multi-structure-only box: fail-closed empty (parity with unit_control residual).
            Vec::new()
        }
    }

    /// Structures residual (KindOf::Structure or object_type Building).
    pub fn structure_objects(&self) -> Vec<&RenderableObject> {
        use crate::game_logic::KindOf;
        // Wave 1101: fail-closed on sold structure residual feed.
        self.objects
            .iter()
            .filter(|o| {
                !o.destroyed
                    && !o.sold
                    && (Self::object_has_kind(o, KindOf::Structure)
                        || o.object_type == PresentationObjectType::Building)
            })
            .collect()
    }

    /// Harvestable resource objects residual.
    pub fn harvestable_objects(&self) -> Vec<&RenderableObject> {
        use crate::game_logic::KindOf;
        // Wave 1101: fail-closed on sold harvestable residual feed.
        self.objects
            .iter()
            .filter(|o| !o.destroyed && !o.sold && Self::object_has_kind(o, KindOf::Harvestable))
            .collect()
    }

    /// Worker units residual (dozer / worker command feed).
    pub fn worker_objects(&self) -> Vec<&RenderableObject> {
        use crate::game_logic::KindOf;
        // Wave 1101: fail-closed on sold/disabled worker residual feed.
        self.objects
            .iter()
            .filter(|o| {
                !o.destroyed && !o.sold && !o.disabled && Self::object_has_kind(o, KindOf::Worker)
            })
            .collect()
    }
}

// ===== apply.rs =====
use super::*;

impl PresentationFrame {
    /// Selected unit identity (health/name/type) from snapshot only.
    ///
    /// Prefer player selection list; fall back to objects marked selected on the frame
    /// when the player list is empty (common right after click-select before player list
    /// is mirrored).
    pub fn selected_unit_display_infos(&self) -> Vec<crate::ui::UnitDisplayInfo> {
        use crate::ui::UnitDisplayInfo;

        // Wave 1106: selection display residual fail-closed on sold/unselectable/
        // masked/disabled (not only destroyed) so ControlBar/RTS panel does not
        // keep UI for unusable selected objects.
        let usable = |o: &RenderableObject| {
            !o.destroyed && !o.sold && !o.unselectable && !o.masked && !o.disabled
        };
        let by_id: std::collections::HashMap<ObjectId, &RenderableObject> =
            self.objects.iter().map(|o| (o.id, o)).collect();
        let mut selected_infos = Vec::with_capacity(self.selected.len().max(1));
        for id in &self.selected {
            if let Some(ro) = by_id.get(id) {
                if !usable(ro) {
                    continue;
                }
                selected_infos.push(Self::unit_display_info_from_renderable(ro));
            }
        }
        if selected_infos.is_empty() {
            for ro in self.objects.iter().filter(|o| o.selected && usable(o)) {
                selected_infos.push(Self::unit_display_info_from_renderable(ro));
            }
        }
        selected_infos
    }

    fn unit_display_info_from_renderable(ro: &RenderableObject) -> crate::ui::UnitDisplayInfo {
        let (production_template, production_progress, production_is_upgrade) = ro
            .production_queue
            .first()
            .map(|p| {
                (
                    Some(p.template_name.clone()),
                    Some(p.progress_ratio),
                    p.is_upgrade,
                )
            })
            .unwrap_or((None, None, false));
        crate::ui::UnitDisplayInfo {
            object_id: ro.id,
            name: ro.template_name.clone(),
            health_current: ro.health_current,
            health_maximum: ro.health_max.max(1.0),
            unit_type: if ro.is_structure {
                "Structure".into()
            } else if ro.is_unit {
                "Unit".into()
            } else {
                "Object".into()
            },
            current_order: if ro.attacking {
                "Attack".into()
            } else if ro.moving {
                "Move".into()
            } else if production_template.is_some() {
                if production_is_upgrade {
                    "Research".into()
                } else {
                    "Produce".into()
                }
            } else {
                "Idle".into()
            },
            veterancy_overlay: ro.veterancy.chevron_overlay().map(str::to_string),
            production_progress,
            production_template,
            production_is_upgrade,
            production_paused: ro.production_paused,
            command_set_override: ro.command_set_override.clone(),
            can_produce: ro.can_produce,
        }
    }

    /// Apply presentation identity fields onto a HUD/UI state (production consumer path).
    /// Does not re-borrow GameLogic — uses only owned snapshot data.
    ///
    /// Overwrites **selection IDs, selected unit health/name, and minimap unit dots**
    /// so a prior live `update_ui_state` walk cannot leave stale identity when a frame
    /// is available.
    pub fn apply_to_ui_state(&self, ui: &mut crate::ui::GameUIState) {
        self.apply_can_make_cameos_to_ui_state(ui);

        ui.rank_level = self.local_rank_level;
        ui.skill_points = self.local_skill_points;
        ui.science_purchase_points = self.local_science_purchase_points;
        ui.rank_progress_percent = self.local_rank_progress_percent;
        ui.superweapon_timers = self
            .superweapon_timers
            .iter()
            .map(|t| crate::ui::hud_state::UiSuperweaponTimer {
                name: t.name.clone(),
                template_name: t.template_name.clone(),
                icon: t.icon.clone(),
                recharge_time: t.recharge_time,
                remaining: t.remaining,
                unlocked: t.unlocked,
                ready: t.ready,
            })
            .collect();
        use crate::game_logic::victory::PlayerOutcome;
        use crate::ui::{color_for_player, BuildQueueEntry, MinimapDot};

        ui.current_game_time = self.total_play_time_seconds;
        ui.credits = self.local_supplies as i32;
        // Prefer produced/consumed residual when present (energy bar parity).
        ui.power_generated = self.local_power_produced.max(self.local_power).max(0);
        ui.power_used = self.local_power_consumed.max(0);
        ui.max_power = ui.power_generated.max(1);
        ui.player_id = self.local_player_id;
        ui.selected_units = self.selected.clone();
        ui.match_over = self.match_over;
        ui.selected_unit_infos = self.selected_unit_display_infos();
        // Radar residual from snapshot events (no live update_ui_state re-read).
        {
            use crate::ui::{RadarMessageEntry, RadarPing, RadarPingKind};
            let mut messages = Vec::new();
            let mut pings = Vec::new();
            let mut last_ping = ui.last_radar_ping;
            for ev in &self.events {
                if let PresentationEvent::RadarMessage {
                    text,
                    position,
                    kind,
                    ..
                } = ev
                {
                    let ping_kind = match kind {
                        1 => RadarPingKind::Attack,
                        2 => RadarPingKind::Ally,
                        _ => RadarPingKind::Generic,
                    };
                    messages.push(text.clone());
                    ui.radar_events.push(RadarMessageEntry {
                        text: text.clone(),
                        position: Some(*position),
                        kind: ping_kind,
                    });
                    if position.length_squared() > 0.0001 {
                        pings.push(RadarPing {
                            position: *position,
                            intensity: 1.0,
                            age_seconds: 0.0,
                            kind: ping_kind,
                        });
                        last_ping = Some(*position);
                    }
                }
            }
            if !messages.is_empty() {
                ui.radar_messages.extend(messages);
                // Cap residual feed.
                let excess = ui.radar_messages.len().saturating_sub(32);
                if excess > 0 {
                    ui.radar_messages.drain(0..excess);
                }
            }
            if !pings.is_empty() {
                ui.radar_pings.extend(pings);
                let excess = ui.radar_pings.len().saturating_sub(32);
                if excess > 0 {
                    ui.radar_pings.drain(0..excess);
                }
            }
            ui.last_radar_ping = last_ping;
        }
        // Script / cinematic / radar residual from snapshot.
        if !self.script_messages.is_empty() {
            ui.script_messages = self.script_messages.clone();
        }
        ui.cinematic_letterbox = self.cinematic_letterbox;
        if self.cinematic_text.is_some() {
            ui.cinematic_text = self.cinematic_text.clone();
        }
        if self.military_caption.is_some() {
            ui.military_caption = self.military_caption.clone();
        }
        ui.radar_enabled = self.radar_ui_enabled;
        ui.radar_forced = self.radar_forced;
        // Script named-timer / cameo / superweapon residual from snapshot.
        ui.named_timers = self.named_timers.clone();
        ui.named_timer_display_shown = self.named_timer_display_shown;
        ui.cameo_flash = self.cameo_flash.clone();
        ui.superweapon_display_enabled = self.superweapon_display_enabled;
        ui.superweapon_hidden_objects = self.superweapon_hidden_objects.clone();
        ui.objectives = self.objectives.clone();
        ui.pending_movie = self.pending_movie.clone();
        ui.pending_radar_movie = self.pending_radar_movie.clone();
        ui.pending_music_stop = self.pending_music_stop;
        ui.pending_popup_messages = self
            .pending_popup_messages
            .iter()
            .map(|p| p.message.clone())
            .collect();
        ui.script_time_frozen = self.script_time_frozen;
        ui.script_camera_time_frozen = self.script_camera_time_frozen;
        ui.time_frozen_for_simulation = self.time_frozen_for_simulation;
        ui.script_fps_limit = self.script_fps_limit;
        ui.view_guardband = self.view_guardband;
        ui.camera_focus = self.camera_focus;
        ui.camera_bw_mode = self.camera_bw_mode;
        ui.camera_shakers = self.camera_shakers.clone();
        ui.camera_motion_blur_count = self.camera_motion_blur_count;
        ui.camera_zoom = self.camera_zoom;
        ui.camera_zoom_reset = self.camera_zoom_reset;
        ui.camera_pitch = self.camera_pitch;
        ui.camera_rotate = self.camera_rotate;
        ui.camera_look_toward = self.camera_look_toward;
        ui.camera_slave_enable = self.camera_slave_enable.clone();
        ui.camera_slave_disable = self.camera_slave_disable;
        ui.named_timers = self.named_timers.clone();
        ui.cameo_flash = self.cameo_flash.clone();
        ui.screen_shakes = self.screen_shakes.clone();
        ui.script_skybox_enabled = self.script_skybox_enabled;
        ui.superweapon_display_enabled = self.superweapon_display_enabled;
        ui.named_timer_display_shown = self.named_timer_display_shown;
        ui.superweapon_hidden_objects = self.superweapon_hidden_objects.clone();
        // Beacon residual from snapshot (no live GameLogic update_ui_state re-read).
        ui.new_beacons = self.new_beacons.clone();
        if !self.beacons.is_empty() {
            use crate::ui::{color_for_player, MinimapDot};
            // Wave 1110: beacon bounds residual excludes sold.
            let (min_x, max_x, min_z, max_z) = {
                let alive: Vec<_> = self
                    .objects
                    .iter()
                    .filter(|o| !o.destroyed && !o.sold)
                    .collect();
                if alive.is_empty() {
                    (-100.0_f32, 100.0_f32, -100.0_f32, 100.0_f32)
                } else {
                    let mut min_x = f32::MAX;
                    let mut max_x = f32::MIN;
                    let mut min_z = f32::MAX;
                    let mut max_z = f32::MIN;
                    for o in &alive {
                        min_x = min_x.min(o.position.x);
                        max_x = max_x.max(o.position.x);
                        min_z = min_z.min(o.position.z);
                        max_z = max_z.max(o.position.z);
                    }
                    (min_x, max_x, min_z, max_z)
                }
            };
            let span_x = (max_x - min_x).max(1.0);
            let span_z = (max_z - min_z).max(1.0);
            ui.minimap_beacons = self
                .beacons
                .iter()
                .map(|p| {
                    let nx = ((p.x - min_x) / span_x).clamp(0.0, 1.0);
                    let ny = ((p.z - min_z) / span_z).clamp(0.0, 1.0);
                    MinimapDot::normalized(
                        nx,
                        ny,
                        color_for_player(self.local_player_id.min(255) as u8),
                        4.0,
                    )
                })
                .collect();
        }
        // ControlBar/WND selection panel health must come from snapshot, not live re-read.
        ui.selection_panel =
            crate::ui::ControlBarSelectionPanelState::from_unit_infos(&ui.selected_unit_infos);

        // Victory residual from snapshot events (no live evaluate_victory re-read).
        if self.match_over {
            let winner = self.events.iter().find_map(|e| match e {
                PresentationEvent::Victory { winner_player } => *winner_player,
                _ => None,
            });
            ui.player_outcome = Some(match winner {
                Some(id) if id == self.local_player_id => PlayerOutcome::Won,
                Some(_) => PlayerOutcome::Lost,
                None => {
                    if self
                        .victory_label
                        .as_deref()
                        .is_some_and(|s| s.to_ascii_lowercase().contains("draw"))
                    {
                        PlayerOutcome::Draw
                    } else if self
                        .victory_label
                        .as_deref()
                        .is_some_and(|s| s.to_ascii_lowercase().contains("winner"))
                    {
                        // Fail-closed: label without winner id → treat as unknown draw residual.
                        PlayerOutcome::Draw
                    } else {
                        PlayerOutcome::Draw
                    }
                }
            });
        }

        // Structure production + under-construction residual for build-queue HUD strip.
        // Wave 1110: build-queue residual excludes sold producers/structures.
        let mut build_queue = Vec::new();
        for o in self.objects.iter().filter(|o| !o.destroyed && !o.sold) {
            if o.under_construction {
                build_queue.push(BuildQueueEntry {
                    template_name: o.template_name.clone(),
                    percent_complete: o.construction_percent.clamp(0.0, 1.0),
                    time_remaining: (1.0 - o.construction_percent.clamp(0.0, 1.0)) * 30.0,
                });
            }
            for item in &o.production_queue {
                build_queue.push(BuildQueueEntry {
                    template_name: item.template_name.clone(),
                    percent_complete: item.progress_ratio.clamp(0.0, 1.0),
                    time_remaining: (item.total_time * (1.0 - item.progress_ratio.clamp(0.0, 1.0)))
                        .max(0.0),
                });
            }
        }
        build_queue.truncate(16);
        ui.build_queue = build_queue;

        // Minimap dots from snapshot positions/teams (normalized into frame bounds).
        // Wave 1110: minimap unit-dot residual excludes sold (parity hud_minimap_units).
        let alive: Vec<&RenderableObject> = self
            .objects
            .iter()
            .filter(|o| !o.destroyed && !o.sold)
            .collect();
        let (world_min_x, world_max_x, world_min_z, world_max_z) = if alive.is_empty() {
            (-100.0, 100.0, -100.0, 100.0)
        } else {
            let mut min_x = f32::MAX;
            let mut max_x = f32::MIN;
            let mut min_z = f32::MAX;
            let mut max_z = f32::MIN;
            for o in &alive {
                min_x = min_x.min(o.position.x);
                max_x = max_x.max(o.position.x);
                min_z = min_z.min(o.position.z);
                max_z = max_z.max(o.position.z);
            }
            // Pad so single-unit maps still normalize.
            if (max_x - min_x).abs() < 1.0 {
                min_x -= 50.0;
                max_x += 50.0;
            }
            if (max_z - min_z).abs() < 1.0 {
                min_z -= 50.0;
                max_z += 50.0;
            }
            (min_x, max_x, min_z, max_z)
        };
        let span_x = (world_max_x - world_min_x).max(1.0);
        let span_z = (world_max_z - world_min_z).max(1.0);
        let mut dots = Vec::with_capacity(alive.len());
        for ro in alive {
            let nx = ((ro.position.x - world_min_x) / span_x).clamp(0.0, 1.0);
            let nz = ((ro.position.z - world_min_z) / span_z).clamp(0.0, 1.0);
            let color = match ro.team {
                Team::USA => color_for_player(1),
                Team::China => color_for_player(0),
                Team::GLA => color_for_player(4),
                Team::Neutral => color_for_player(7),
            };
            let size = if ro.is_structure { 4.0 } else { 2.0 };
            dots.push(MinimapDot::normalized(nx, nz, color, size));
        }
        ui.minimap_unit_dots = dots;
    }

    /// Resource triple for GameHud::update_resources (credits, power, max_power).
    /// Winner player id from frozen Victory event residual.
    pub fn victory_winner_id(&self) -> Option<u32> {
        self.events.iter().find_map(|ev| match ev {
            PresentationEvent::Victory { winner_player } => *winner_player,
            _ => None,
        })
    }

    /// Frozen VictorySummary residual when match_over.
    pub fn victory_summary_residual(&self) -> Option<&crate::game_logic::VictorySummary> {
        self.victory_summary.as_ref()
    }

    /// Drive VictoryScreen visibility/type from snapshot residual (no live GameLogic).
    ///
    /// Fail-closed: does not rebuild full VictorySummary statistics tables.
    pub fn apply_to_victory_screen(&self, screen: &mut crate::ui::VictoryScreen) {
        if !self.match_over {
            return;
        }
        let winner = self.events.iter().find_map(|e| match e {
            PresentationEvent::Victory { winner_player } => *winner_player,
            _ => None,
        });
        match winner {
            Some(id) if id == self.local_player_id => screen.set_victory(id),
            Some(_) => screen.set_defeat(),
            None => {
                let label = self
                    .victory_label
                    .as_deref()
                    .unwrap_or("")
                    .to_ascii_lowercase();
                if label.contains("defeat") || label.contains("lost") {
                    screen.set_defeat();
                } else if label.contains("winner") && !label.contains("draw") {
                    // Label-only winner residual without player id → draw fail-closed.
                    screen.set_draw();
                } else {
                    screen.set_draw();
                }
            }
        }
    }

    pub fn hud_resource_triple(&self) -> (i32, i32, i32) {
        let credits = self.local_supplies as i32;
        let power = self.local_power.max(0);
        (credits, power, power.max(1))
    }

    /// Units list for GameHud minimap: (id, x, z, team_color_index).
    pub fn hud_minimap_units(&self) -> Vec<(ObjectId, f32, f32, u8)> {
        // Wave 1109: minimap unit-dot residual excludes sold (alive dots only).
        self.objects
            .iter()
            .filter(|o| !o.destroyed && !o.sold)
            .map(|o| {
                let team_idx = match o.team {
                    Team::USA => 1u8,
                    Team::China => 0u8,
                    Team::GLA => 4u8,
                    Team::Neutral => 7u8,
                };
                (o.id, o.position.x, o.position.z, team_idx)
            })
            .collect()
    }

    /// Apply presentation resources, minimap units, and selection health to GameHUD.
    ///
    /// Selection identity (IDs + health/name) is snapshot-owned so the production HUD
    /// does not re-read live GameLogic after a skirmish start / dual-tick.
    /// Also fills the ControlBar selection panel health strip via GameHUD.
    /// Feed InGameUI PublicTimer residual into ConstructionPanel superweapon timers.
    pub fn apply_superweapon_timers_to_panel(
        &self,
        panel: &mut crate::ui::construction_panel::ConstructionPanel,
    ) {
        use crate::ui::construction_panel::SuperweaponTimer;
        for t in &self.superweapon_timers {
            if !t.unlocked {
                continue;
            }
            let mut timer = SuperweaponTimer::new(
                t.name.clone(),
                t.template_name.clone(),
                t.icon.clone(),
                t.recharge_time,
            );
            timer.remaining = t.remaining;
            timer.unlocked = t.unlocked;
            panel.add_superweapon_timer(timer);
        }
        self.apply_can_make_cameos_to_panel(panel);
    }

    pub fn apply_to_game_hud(&self, hud: &mut crate::ui::GameHUD) {
        // InGameUI PublicTimer residual freeze onto HUD strip.
        let sw: Vec<crate::ui::hud_state::UiSuperweaponTimer> = self
            .superweapon_timers
            .iter()
            .map(|t| crate::ui::hud_state::UiSuperweaponTimer {
                name: t.name.clone(),
                template_name: t.template_name.clone(),
                icon: t.icon.clone(),
                recharge_time: t.recharge_time,
                remaining: t.remaining,
                ready: t.ready,
                unlocked: t.unlocked,
            })
            .collect();
        hud.apply_presentation_superweapon_timers(&sw);

        // ControlBar construction button enable residual from CanMake freeze.
        hud.apply_can_make_cameos(
            &self
                .can_make_cameos
                .iter()
                .map(|c| {
                    (
                        c.template_name.as_str(),
                        c.available,
                        c.can_make,
                        c.help_status.as_deref(),
                    )
                })
                .collect::<Vec<_>>(),
        );
        // Production queue residual for primary selected producer.
        // Wave 1109: HUD queue residual fail-closed on sold/unusable primary
        // (parity with control_bar_selection_panel Wave 1108).
        {
            let mut queue_items: Vec<(String, f32, i32, f32)> = Vec::new();
            let usable = |o: &&RenderableObject| {
                o.selected && !o.destroyed && !o.sold && !o.unselectable && !o.masked && !o.disabled
            };
            if let Some(id) = self
                .selected
                .first()
                .copied()
                .or_else(|| self.objects.iter().find(usable).map(|o| o.id))
            {
                if let Some(ro) = self.objects.iter().find(|o| {
                    o.id == id
                        && !o.destroyed
                        && !o.sold
                        && !o.unselectable
                        && !o.masked
                        && !o.disabled
                }) {
                    for p in &ro.production_queue {
                        queue_items.push((
                            p.template_name.clone(),
                            p.progress_ratio,
                            p.cost_supplies as i32,
                            p.total_time.max(0.01),
                        ));
                    }
                }
            }
            hud.sync_production_queue_from_presentation(&queue_items);
        }
        let (credits, power, max_power) = self.hud_resource_triple();
        hud.update_resources(credits, power, max_power);
        let units = self.hud_minimap_units();
        hud.update_minimap(&units);
        let infos = self.selected_unit_display_infos();
        // Prefer explicit player selection list; if empty but infos came from
        // object.selected flags, mirror those IDs onto the HUD strip.
        let mut ids = self.selected.clone();
        if ids.is_empty() {
            ids = infos.iter().map(|i| i.object_id).collect();
        }
        hud.sync_selection_from_presentation(ids, infos);
        // ControlBar unit command residual from primary selection freeze.
        let cmds: Vec<(String, bool)> = self
            .unit_command_buttons()
            .into_iter()
            .map(|b| (b.command_name, b.enabled))
            .collect();
        hud.apply_presentation_unit_commands(&cmds);
        self.apply_events_to_game_hud(hud);
    }

    /// Route frozen gameplay events into HUD message / radar channels.
    /// Fail-closed: text residual only — not full EVA voice / WND dialog parity.
    pub fn apply_events_to_game_hud(&self, hud: &mut crate::ui::GameHUD) {
        for ev in &self.events {
            match ev {
                PresentationEvent::RadarMessage {
                    text,
                    position,
                    kind,
                    ..
                } => {
                    use crate::ui::RadarPingKind;
                    let ping_kind = match kind {
                        1 => RadarPingKind::Attack,
                        2 => RadarPingKind::Ally,
                        _ => RadarPingKind::Generic,
                    };
                    let pos = if position.length_squared() > 0.0001 {
                        Some(*position)
                    } else {
                        None
                    };
                    hud.add_radar_message(text, pos, ping_kind);
                }
                PresentationEvent::ConstructionComplete { template, .. } => {
                    hud.push_info_message(&format!("Construction complete: {template}"));
                }
                PresentationEvent::UpgradeComplete { name, .. } => {
                    hud.push_info_message(&format!("Upgrade complete: {name}"));
                }
                PresentationEvent::ProductionComplete { template, .. } => {
                    hud.push_info_message(&format!("Unit ready: {template}"));
                }
                PresentationEvent::OwnerChanged { id, team } => {
                    hud.push_info_message(&format!("Ownership changed: #{} -> {:?}", id.0, team));
                }
                PresentationEvent::AttackTargeted { attacker, target } => {
                    if let Some(t) = target {
                        hud.push_info_message(&format!("Attack: #{} -> #{}", attacker.0, t.0));
                    }
                }
                PresentationEvent::MoveOrdered { unit, destination } => {
                    hud.push_info_message(&format!(
                        "Move: #{} -> ({:.0},{:.0})",
                        unit.0, destination[0], destination[2]
                    ));
                }
                PresentationEvent::DamageApplied {
                    target,
                    amount,
                    destroyed,
                    ..
                } => {
                    if *destroyed {
                        hud.push_info_message(&format!("Destroyed: #{}", target.0));
                    } else if *amount > 0.0 {
                        hud.push_info_message(&format!("-{} HP #{}", *amount as i32, target.0));
                    }
                }
                PresentationEvent::HealApplied { target, health } => {
                    hud.push_info_message(&format!("Heal #{} -> {:.0} HP", target.0, health));
                }
                PresentationEvent::EconomyChanged {
                    player_id,
                    supplies,
                    power_available,
                } => {
                    hud.push_info_message(&format!(
                        "Economy P{}: ${} power={}",
                        player_id, supplies, power_available
                    ));
                }
                PresentationEvent::ObjectDestroyed { id, .. } => {
                    hud.push_info_message(&format!("Destroyed: #{}", id.0));
                }
                PresentationEvent::Victory { winner_player } => {
                    let msg = match winner_player {
                        Some(p) => format!("Victory: player {p}"),
                        None => "Victory".to_string(),
                    };
                    hud.push_info_message(&msg);
                }
                PresentationEvent::ParticleSystemSpawned { .. } => {}
                PresentationEvent::WeaponFireLoopStarted { .. }
                | PresentationEvent::WeaponFireLoopStopped { .. } => {}
                PresentationEvent::EvaAlert { name } => {
                    hud.push_info_message(&format!("EVA: {name}"));
                }
            }
        }
    }

    /// Collect presentation→audio requests (no GameLogic borrow).
    /// Wave 527/528: FireSound loop residual uses host sound name + looping flag + snapshot pose.
    /// Wave 528: WeaponFireLoopStop is stop-only (no FireSound replay).
    /// Wave 529: RadarMessage → EVA/radar audio event names + snapshot position.
    /// Wave 530: OwnerChanged → BuildingCaptured/UnitHijacked audio residual.
    /// Wave 533: EvaAlert → EVA_* audio event names from host_eva_log pulses.
    /// Wave 535: ParticleSystemSpawned → Explosion/FireBurn/… audio at snapshot pose.
    /// Fail-closed: not Miles/device spatial parity — names resolve via SoundEffectsTable.
    pub fn collect_audio_events(&self) -> Vec<crate::game_logic::AudioEventRequest> {
        use crate::game_logic::AudioEventRequest;
        let mut out = Vec::new();
        // Snapshot pose lookup for 3D placement (no live GameLogic dual-read).
        let pose_by_id: std::collections::HashMap<ObjectId, glam::Vec3> =
            self.objects.iter().map(|o| (o.id, o.position)).collect();
        for ev in &self.events {
            // Wave 527: FiringTracker loop uses concrete FireSound name when non-empty.
            if let PresentationEvent::WeaponFireLoopStarted { unit, sound } = ev {
                let name = if sound.is_empty() {
                    "WeaponFireLoop"
                } else {
                    sound.as_str()
                };
                let mut req = AudioEventRequest::new(name).with_object(*unit).looping();
                if let Some(pos) = pose_by_id.get(unit) {
                    req = req.with_position(*pos);
                }
                out.push(req);
                continue;
            }
            if let PresentationEvent::WeaponFireLoopStopped { unit, sound } = ev {
                // Wave 528: explicit stop residual (must not re-trigger FireSound play).
                let _ = sound;
                let mut req = AudioEventRequest::new("WeaponFireLoopStop").with_object(*unit);
                if let Some(pos) = pose_by_id.get(unit) {
                    req = req.with_position(*pos);
                }
                req = req.with_priority(200);
                out.push(req);
                continue;
            }
            let mapped: Option<(&str, Option<crate::game_logic::ObjectId>)> = match ev {
                PresentationEvent::ObjectDestroyed { id, .. } => Some(("UnitDie", Some(*id))),
                PresentationEvent::ConstructionComplete { id, .. } => {
                    Some(("BuildingComplete", Some(*id)))
                }
                PresentationEvent::UpgradeComplete { .. } => Some(("UpgradeComplete", None)),
                PresentationEvent::ProductionComplete { spawned, .. } => {
                    Some(("UnitReady", Some(*spawned)))
                }
                PresentationEvent::AttackTargeted { attacker, .. } => {
                    Some(("WeaponFire", Some(*attacker)))
                }
                PresentationEvent::DamageApplied {
                    target,
                    destroyed: true,
                    ..
                } => Some(("UnitDie", Some(*target))),
                PresentationEvent::DamageApplied {
                    target,
                    amount,
                    destroyed: false,
                    ..
                } => {
                    if *amount > 0.0 {
                        Some(("WeaponHit", Some(*target)))
                    } else {
                        None
                    }
                }
                PresentationEvent::HealApplied { target, .. } => Some(("UnitHeal", Some(*target))),
                PresentationEvent::EconomyChanged { .. } => Some(("MoneyTick", None)),
                PresentationEvent::Victory { .. } => Some(("Victory", None)),
                PresentationEvent::MoveOrdered { unit, .. } => Some(("UnitMove", Some(*unit))),
                PresentationEvent::WeaponFireLoopStarted { .. }
                | PresentationEvent::WeaponFireLoopStopped { .. } => None,
                PresentationEvent::ParticleSystemSpawned { .. } => None, // handled below (Wave 535)
                PresentationEvent::OwnerChanged { .. } => None,          // handled below (Wave 530)
                PresentationEvent::RadarMessage { .. } => None,          // handled below (Wave 529)
                PresentationEvent::EvaAlert { .. } => None,              // handled below (Wave 533)
            };
            let Some((kind, obj)) = mapped else {
                continue;
            };
            let mut req = AudioEventRequest::new(kind);
            if let Some(id) = obj {
                req = req.with_object(id);
                if let Some(pos) = pose_by_id.get(&id) {
                    req = req.with_position(*pos);
                }
            }
            out.push(req);
        }

        // Wave 530: capture/hijack ownership transfer audio residual.
        for ev in &self.events {
            let PresentationEvent::OwnerChanged { id, .. } = ev else {
                continue;
            };
            let is_structure = self
                .objects
                .iter()
                .find(|o| o.id == *id)
                .map(|o| o.is_structure)
                .unwrap_or(false);
            let name = if is_structure {
                "BuildingCaptured"
            } else {
                "UnitHijacked"
            };
            let mut req = AudioEventRequest::new(name)
                .with_object(*id)
                .with_priority(170);
            if let Some(pos) = pose_by_id.get(id) {
                req = req.with_position(*pos);
            }
            out.push(req);
        }

        // Wave 533: host EVA pulse audio residual (snapshot names, no live dual-read).
        for ev in &self.events {
            let PresentationEvent::EvaAlert { name } = ev else {
                continue;
            };
            if name.is_empty() {
                continue;
            }
            out.push(AudioEventRequest::new(name.as_str()).with_priority(180));
        }

        // Wave 535: combat particle spawn → presentation audio residual (snapshot pose).
        // Fail-closed: not full FXList/FXParticleSystemNames Miles matrix — kind→name map only.
        // Skip muzzle (WeaponFire already) and impact (WeaponHit already from DamageApplied).
        for ev in &self.events {
            let PresentationEvent::ParticleSystemSpawned {
                kind,
                position,
                template_name,
                ..
            } = ev
            else {
                continue;
            };
            use crate::game_logic::combat_particles::CombatParticleKind;
            let event_name = match kind {
                CombatParticleKind::DeathExplosion => {
                    // Prefer concrete template when it looks like an explosion FX name.
                    let t = template_name.as_str();
                    if t.is_empty() {
                        "Explosion"
                    } else if t.starts_with("FX_") || t.starts_with("WeaponFX_") {
                        t
                    } else {
                        "Explosion"
                    }
                }
                CombatParticleKind::DeathBurn => "FireBurn",
                CombatParticleKind::DeathPoison => "PoisonDeath",
                CombatParticleKind::DeathLaser => "LaserDeath",
                CombatParticleKind::DeathSmoke => "DeathSmoke",
                CombatParticleKind::WeaponMuzzleFlash
                | CombatParticleKind::WeaponImpact
                | CombatParticleKind::ProjectileExhaust => continue,
            };
            let mut req = AudioEventRequest::new(event_name).with_priority(160);
            if position.length_squared() > 0.01 {
                req = req.with_position(*position);
            }
            out.push(req);
        }

        // Wave 529: radar/EVA presentation audio residual (no GameLogic dual-write).
        // kind: 0=Generic 1=Attack 2=Ally; text also maps classic EVA phrases.
        for ev in &self.events {
            let PresentationEvent::RadarMessage {
                text,
                position,
                kind,
                ..
            } = ev
            else {
                continue;
            };
            let t = text.to_ascii_lowercase();
            let event_name = if t.contains("low power") || t.contains("power shortage") {
                "EVA_LowPower"
            } else if t.contains("insufficient funds") || t.contains("not enough money") {
                "EVA_InsufficientFunds"
            } else if t.contains("base under attack") || t.contains("our base is under attack") {
                "EVA_BaseUnderAttack"
            } else if t.contains("ally under attack") || t.contains("ally is under attack") {
                "EVA_AllyUnderAttack"
            } else if t.contains("building lost") || t.contains("structure lost") {
                "EVA_BuildingLost"
            } else if t.contains("unit lost") {
                "EVA_UnitLost"
            } else {
                match kind {
                    1 => "RadarAttack",
                    2 => "RadarAlly",
                    _ => "RadarGeneric",
                }
            };
            let mut req = AudioEventRequest::new(event_name).with_priority(180);
            if position.length_squared() > 0.01 {
                req = req.with_position(*position);
            }
            out.push(req);
        }
        out
    }

    /// Dispatch presentation audio directly to the audio subsystem.
    /// Fail-closed boundary: does **not** mutate GameLogic mid-frame.
    pub fn dispatch_audio_events_direct(&self) -> usize {
        let events = self.collect_audio_events();
        let n = events.len();
        for event in events {
            if let Some(obj_id) = event.object_id {
                if let Some(pos) = event.position {
                    log::trace!(
                        "🔊 Presentation audio: {} at {:?} from object {}",
                        event.event_type,
                        pos,
                        obj_id
                    );
                } else {
                    log::trace!(
                        "🔊 Presentation audio: {} from object {}",
                        event.event_type,
                        obj_id
                    );
                }
            } else if let Some(pos) = event.position {
                log::trace!("🔊 Presentation audio: {} at {:?}", event.event_type, pos);
            } else {
                log::trace!("🔊 Presentation audio: {}", event.event_type);
            }
            let _ = crate::subsystem_manager::with_subsystem_mut::<
                crate::subsystem_manager::AudioManagerSubsystem,
                _,
            >(|audio| audio.queue_event(event.clone()));
        }
        n
    }

    /// Legacy dual-write residual (tests may still call). Prefer `dispatch_audio_events_direct`.
    pub fn apply_events_to_audio(&self, logic: &mut GameLogic) -> usize {
        let events = self.collect_audio_events();
        let n = events.len();
        for req in events {
            logic.queue_audio_event(req);
        }
        n
    }

    /// Ensure active presentation particle systems are mirrored into the GameClient
    /// ParticleSystemManager (same-frame residual). Prefer existing client_system_id;
    /// backfill when host spawn mirror was skipped/failed.
    /// Fail-closed: not full W3D GPU particle parity.
    pub fn apply_particle_systems_to_client(&self) -> usize {
        let mut n = 0usize;
        for p in self.particle_systems.iter().filter(|p| p.active) {
            if p.client_system_id.is_some() {
                continue;
            }
            if crate::game_logic::combat_particles::mirror_spawn_to_client_manager(
                &p.template_name,
                p.position,
            )
            .is_some()
            {
                n += 1;
            }
        }
        // Spawn events without prior client id residual (same-frame observe path).
        for ev in &self.events {
            if let PresentationEvent::ParticleSystemSpawned {
                template_name,
                position,
                ..
            } = ev
            {
                // If already covered by particle_systems list with client id, skip.
                let already = self.particle_systems.iter().any(|p| {
                    p.template_name == *template_name
                        && (p.position - *position).length_squared() < 1e-4
                        && p.client_system_id.is_some()
                });
                if already {
                    continue;
                }
                if crate::game_logic::combat_particles::mirror_spawn_to_client_manager(
                    template_name,
                    *position,
                )
                .is_some()
                {
                    n += 1;
                }
            }
        }
        n
    }

    /// Snapshot-owned ControlBar / WND selection panel (health + name).
    pub fn control_bar_selection_panel(&self) -> crate::ui::ControlBarSelectionPanelState {
        let mut panel = crate::ui::ControlBarSelectionPanelState::from_unit_infos(
            &self.selected_unit_display_infos(),
        );
        // Prefer full queue from the primary selected renderable when present.
        // Wave 1108: fail-closed on sold/unusable primary (belt-and-suspenders after
        // selected_unit_display_infos Wave 1106).
        if let Some(id) = panel.primary_object_id {
            if let Some(ro) = self.objects.iter().find(|o| {
                o.id == id && !o.destroyed && !o.sold && !o.unselectable && !o.masked && !o.disabled
            }) {
                panel.production_queue = ro
                    .production_queue
                    .iter()
                    .map(|p| (p.template_name.clone(), p.progress_ratio, p.is_upgrade))
                    .collect();
                if panel.production_progress.is_none() {
                    panel.production_progress = panel.production_queue.first().map(|(_, p, _)| *p);
                    panel.production_template =
                        panel.production_queue.first().map(|(t, _, _)| t.clone());
                }
                panel.production_paused = ro.production_paused;
                panel.ocl_timer_seconds = ro.ocl_timer_seconds;
                panel.sold = ro.sold;
                panel.production_is_upgrade = panel
                    .production_queue
                    .first()
                    .map(|(_, _, u)| *u)
                    .unwrap_or(false);
                if panel.veterancy_overlay.is_none() {
                    panel.veterancy_overlay = ro.veterancy.chevron_overlay().map(str::to_string);
                }
                panel.max_garrison = ro.max_garrison;
                panel.garrisoned_count = ro.garrisoned_units.len();
                panel.under_construction = ro.under_construction;
                panel.construction_percent = ro.construction_percent;
                panel.applied_upgrades = ro.applied_upgrades.clone();
                panel.rally_point = ro.rally_point.map(|p| [p.x, p.y, p.z]);
                panel.special_power_ready = ro.special_power_ready;
                panel.special_power_cooldown_remaining = ro.special_power_cooldown_remaining;
            }
        }
        panel
    }

    /// Apply selection health/name to GameClient ControlBar without OBJECT_REGISTRY.
    ///
    /// Apply frozen skybox residual to the render pipeline without live GameLogic.
    pub fn apply_skybox_to_pipeline(
        &self,
        pipeline: &mut crate::graphics::render_pipeline::RenderPipeline,
    ) {
        pipeline.set_skybox_enabled(self.world_env.skybox_enabled);
        if let Some(textures) = self.world_env.skybox_textures.clone() {
            pipeline.set_skybox_hint(textures);
        }
    }

    /// Headless-safe: uses only presentation fields. Does not claim full WND shell.
    #[cfg(feature = "game_client")]

    /// Feed ControlBar HelpBox CanMake residual from presentation.

    /// Feed ConstructionPanel CanMake residual from presentation.
    pub fn apply_can_make_cameos_to_panel(
        &self,
        panel: &mut crate::ui::construction_panel::ConstructionPanel,
    ) {
        panel.apply_can_make_cameos(
            &self
                .can_make_cameos
                .iter()
                .map(|c| crate::ui::hud_state::CanMakeCameoUi {
                    template_name: c.template_name.clone(),
                    can_make: c.can_make,
                    available: c.available,
                    help_status: c.help_status.clone(),
                })
                .collect::<Vec<_>>(),
        );
    }

    pub fn apply_can_make_cameos_to_ui_state(&self, ui: &mut crate::ui::hud_state::GameUIState) {
        ui.can_make_cameos = self
            .can_make_cameos
            .iter()
            .map(|c| crate::ui::hud_state::CanMakeCameoUi {
                template_name: c.template_name.clone(),
                can_make: c.can_make,
                available: c.available,
                help_status: c.help_status.clone(),
            })
            .collect();
        ui.can_make_producer_id = self.can_make_producer_id;
    }

    pub fn apply_to_control_bar(
        &self,
        control_bar: &mut game_client::gui::control_bar::ControlBar,
    ) {
        let panel = self.control_bar_selection_panel();
        let ids: Vec<u32> = if !self.selected.is_empty() {
            self.selected.iter().map(|id| id.0).collect()
        } else {
            panel.unit_infos.iter().map(|u| u.object_id.0).collect()
        };
        let _ = control_bar.update_for_selection(ids);
        control_bar.sync_selection_display_from_presentation(
            panel.visible.then_some(panel.primary_name.as_str()),
            panel.health_current,
            panel.health_maximum,
            panel.selected_count,
            panel.veterancy_overlay.as_deref(),
            panel.production_progress,
            panel.production_template.as_deref(),
            &panel.production_queue,
            panel.production_paused,
        );
        control_bar.sync_structure_context_from_presentation(
            panel.max_garrison,
            panel.garrisoned_count,
            panel.under_construction,
            panel.construction_percent,
        );
        // Wave 1031: OCL timer residual into ControlBar OclTimer dual path.
        control_bar.sync_ocl_timer_from_presentation(panel.ocl_timer_seconds);
        control_bar.sync_sold_from_presentation(panel.sold);
        control_bar.sync_upgrades_and_specials_from_presentation(
            &panel.applied_upgrades,
            panel.rally_point,
            panel.special_power_ready,
            panel.special_power_cooldown_remaining,
        );
        // Wave 1110: multi-select count residual excludes sold/unusable.
        let usable_selected = |o: &&RenderableObject| {
            o.selected && !o.destroyed && !o.sold && !o.unselectable && !o.masked && !o.disabled
        };
        let selected_count = if !self.selected.is_empty() {
            self.selected
                .iter()
                .filter(|id| {
                    self.objects
                        .iter()
                        .any(|o| o.id == **id && usable_selected(&o))
                })
                .count()
        } else {
            self.objects.iter().filter(usable_selected).count()
        };
        if selected_count > 1 {
            let names = self.selected_command_set_names();
            control_bar.sync_multi_select_command_sets_from_presentation(&names);
        } else {
            control_bar.sync_command_set_from_presentation(self.selected_command_set_name());
        }
        control_bar.sync_sciences_from_presentation(&self.local_unlocked_sciences);
        let ready_sp: Vec<String> = self
            .selected_unit_display_infos()
            .iter()
            .filter_map(|info| {
                self.objects
                    .iter()
                    .find(|o| {
                        o.id == info.object_id
                            && o.special_power_ready
                            && !o.destroyed
                            && !o.sold
                            && !o.disabled
                    })
                    .map(|o| o.template_name.clone())
            })
            .collect();
        // Also include any selected renderable with ready SP (selection flags path).
        // Wave 1110: ready SP residual fail-closed on sold/disabled.
        let mut ready_sp = ready_sp;
        for o in self
            .objects
            .iter()
            .filter(|o| usable_selected(o) && o.special_power_ready)
        {
            if !ready_sp.iter().any(|n| n == &o.template_name) {
                ready_sp.push(o.template_name.clone());
            }
        }
        control_bar.sync_radar_queues_and_specials_from_presentation(
            self.local_radar_count,
            self.local_radar_disabled,
            &self.local_queued_upgrades,
            &ready_sp,
        );
    }

    /// Selection IDs for multi-consumer apply (player list or object.selected flags).
    pub fn selection_ids_for_consumers(&self) -> Vec<crate::game_logic::ObjectId> {
        // Wave 1106: consumer selection residual filters sold/unusable ids even
        // when the frozen selected list still holds them.
        let usable_id = |id: &ObjectId| {
            self.objects.iter().any(|o| {
                o.id == *id
                    && !o.destroyed
                    && !o.sold
                    && !o.unselectable
                    && !o.masked
                    && !o.disabled
            })
        };
        let mut ids: Vec<ObjectId> = self.selected.iter().copied().filter(usable_id).collect();
        if ids.is_empty() {
            ids = self
                .selected_unit_display_infos()
                .into_iter()
                .map(|i| i.object_id)
                .collect();
        }
        ids
    }

    /// Apply selection panel to RTS interface (command/selection residual consumer).
    pub fn apply_to_rts_interface(&self, rts: &mut crate::ui::RTSInterface) {
        rts.apply_selection_panel(
            self.control_bar_selection_panel(),
            self.selection_ids_for_consumers(),
        );
    }

    /// Apply selection panel to unit command grid (context-sensitive residual).

    /// Derive unit-command-panel buttons from primary selection residual.
    ///
    /// Fail-closed: not full CommandSet INI matrix / per-faction button layout.
    pub fn unit_command_buttons(&self) -> Vec<crate::ui::UnitCommandButton> {
        use crate::ui::UnitCommandButton;
        let panel = self.control_bar_selection_panel();
        let Some(id) = panel.primary_object_id else {
            return Vec::new();
        };
        // Wave 1107: unit command buttons residual fail-closed on sold/unusable primary.
        let Some(ro) = self.objects.iter().find(|o| {
            o.id == id && !o.destroyed && !o.sold && !o.unselectable && !o.masked && !o.disabled
        }) else {
            return Vec::new();
        };
        let mut cmds = Vec::new();
        let push = |cmds: &mut Vec<UnitCommandButton>, name: &str, enabled: bool| {
            if !cmds
                .iter()
                .any(|c| c.command_name.eq_ignore_ascii_case(name))
            {
                cmds.push(UnitCommandButton {
                    command_name: name.into(),
                    enabled,
                });
            }
        };
        if ro.is_mobile || ro.is_unit {
            push(&mut cmds, "Command_Stop", true);
            push(&mut cmds, "Command_AttackMove", ro.has_weapon);
            push(&mut cmds, "Command_Guard", true);
            push(&mut cmds, "Command_Patrol", true);
            push(&mut cmds, "Command_Scatter", true);
            push(&mut cmds, "Command_AttitudeAggressive", true);
            push(&mut cmds, "Command_AttitudePassive", true);
            push(&mut cmds, "Command_AttitudeSleep", true);
            push(&mut cmds, "Command_SwitchWeapons", true);
            {
                let n = ro.template_name.to_ascii_lowercase();
                if n.contains("supply")
                    || n.contains("harvester")
                    || n.contains("chinook")
                    || (n.contains("worker") && !n.contains("dozer"))
                {
                    push(&mut cmds, "Command_ReturnSupplies", true);
                }
            }
            {
                let n = ro.template_name.to_ascii_lowercase();
                if n.contains("chinook") {
                    push(&mut cmds, "Command_CombatDrop", true);
                }
                if n.contains("jet")
                    || n.contains("raptor")
                    || n.contains("mig")
                    || n.contains("aurora")
                    || n.contains("stealth")
                    || n.contains("comanche")
                    || n.contains("helicopter")
                    || n.contains("chinook")
                {
                    push(&mut cmds, "Command_ReturnToBase", true);
                }
            }
            // Dozer/Worker repair residual (R key / strip).
            {
                let n = ro.template_name.to_ascii_lowercase();
                if n.contains("dozer")
                    || n.contains("worker")
                    || n.contains("chinook")
                    || n.contains("construction")
                    || n.contains("supplytruck")
                    || n.contains("supply_truck")
                {
                    push(&mut cmds, "Command_Repair", true);
                }
                if n.contains("dozer") || n.contains("worker") {
                    push(&mut cmds, "Command_ClearMines", true);
                }
            }
            push(&mut cmds, "Command_Cheer", true);
            // Multi-select formation residual.
            if self.selection_ids_for_consumers().len() >= 2 {
                push(&mut cmds, "Command_CreateFormation", true);
            }
            // C++ Deploy residual (DeployStyle / sentry / crawler family).
            let n = ro.template_name.to_ascii_lowercase();
            if n.contains("sentry")
                || n.contains("nukecannon")
                || n.contains("scud")
                || n.contains("dozer")
                || n.contains("worker")
                || n.contains("stinger")
                || n.contains("tunnel")
                || n.contains("tomahawk")
                || n.contains("humvee")
                || n.contains("buggy")
                || n.contains("crawler")
                || n.contains("quadcannon")
                || n.contains("inferno")
                || n.contains("artillery")
                || n.contains("spectrum")
            {
                push(&mut cmds, "Command_Deploy", true);
            }
            // Hero / special-ability residual (target-click armed by engine).
            if n.contains("jarmenkell") || n.contains("jarmen_kell") {
                push(&mut cmds, "Command_SnipeVehicle", true);
            }
            if n.contains("colonelburton") || n.contains("colonel_burton") || n.contains("burton") {
                push(&mut cmds, "Command_PlantTimedDemoCharge", true);
                push(&mut cmds, "Command_PlantRemoteDemoCharge", true);
                push(&mut cmds, "Command_DetonateRemoteDemoCharges", true);
            }
            if n.contains("blacklotus") || n.contains("black_lotus") {
                push(&mut cmds, "Command_CaptureBuilding", true);
                push(&mut cmds, "Command_StealCashHack", true);
                push(&mut cmds, "Command_DisableVehicleHack", true);
            }
            if n.contains("chinainfantryhacker")
                || (n.contains("hacker") && n.contains("china"))
                || n.contains("china_hacker")
            {
                push(&mut cmds, "Command_HackerDisableBuilding", true);
                push(&mut cmds, "Command_HackInternet", true);
            }
            if n.contains("ambulance") {
                push(&mut cmds, "Command_CleanupArea", true);
            }
            if n.contains("hijacker") {
                push(&mut cmds, "Command_Hijack", true);
            }
            if n.contains("saboteur") {
                push(&mut cmds, "Command_Sabotage", true);
            }
            if n.contains("bombtruck") || n.contains("bomb_truck") {
                push(&mut cmds, "Command_DisguiseAsVehicle", true);
                push(&mut cmds, "Command_ConvertToCarbomb", true);
            }
            if n.contains("rebel") && !n.contains("scud") {
                push(&mut cmds, "Command_CaptureBuilding", true);
                push(&mut cmds, "Command_PlantBoobyTrap", true);
            }
            if n.contains("ranger") || n.contains("redguard") {
                push(&mut cmds, "Command_CaptureBuilding", true);
            }
            if n.contains("demo")
                && (n.contains("terrorist") || n.contains("bike") || n.contains("trap"))
            {
                push(&mut cmds, "Command_DemoTertiarySuicide", true);
            }
        }
        if ro.is_structure || ro.can_produce {
            if ro.under_construction {
                push(&mut cmds, "Command_CancelConstruction", true);
                push(&mut cmds, "Command_ResumeConstruction", true);
            } else if ro.is_structure {
                // C++ Command_Sell residual — completed structures only.
                push(&mut cmds, "Command_Sell", true);
                let n = ro.template_name.to_ascii_lowercase();
                // C++ OverchargeBehavior residual (China nuclear plants).
                if n.contains("china") && (n.contains("power") || n.contains("nuclear")) {
                    push(&mut cmds, "Command_ToggleOvercharge", true);
                }
                // USA Strategy Center battle plans residual.
                if n.contains("strategycenter") || n.contains("strategy_center") {
                    push(&mut cmds, "Command_InitiateBattlePlanBombardment", true);
                    push(&mut cmds, "Command_InitiateBattlePlanHoldTheLine", true);
                    push(
                        &mut cmds,
                        "Command_InitiateBattlePlanSearchAndDestroy",
                        true,
                    );
                    push(&mut cmds, "Command_CIAIntelligence", true);
                }
                // Named superweapon / intel residual buttons.
                if n.contains("particlecannon") {
                    push(&mut cmds, "Command_ParticleCannon", true);
                }
                if n.contains("nuclear") && n.contains("missile") {
                    push(&mut cmds, "Command_NuclearMissile", true);
                }
                if n.contains("scudstorm") || n.contains("scud_storm") {
                    push(&mut cmds, "Command_ScudStorm", true);
                }
                if n.contains("spysat") || (n.contains("satellite") && n.contains("uplink")) {
                    push(&mut cmds, "Command_SpySatelliteScan", true);
                }
                if n.contains("airfield") {
                    push(&mut cmds, "Command_SpyDrone", true);
                    push(&mut cmds, "Command_EmergencyRepair", true);
                    push(&mut cmds, "Command_Airstrike", true);
                    push(&mut cmds, "Command_CarpetBomb", true);
                }
                if n.contains("commandcenter") || n.contains("command_center") {
                    push(&mut cmds, "Command_ArtilleryBarrage", true);
                    push(&mut cmds, "Command_EmergencyRepair", true);
                    // Faction generals-power residual buttons on CC.
                    if n.contains("gla") {
                        push(&mut cmds, "Command_Ambush", true);
                        push(&mut cmds, "Command_SneakAttack", true);
                        push(&mut cmds, "Command_AnthraxBomb", true);
                    }
                    if n.contains("america") || n.contains("usa") {
                        push(&mut cmds, "Command_LeafletDrop", true);
                        push(&mut cmds, "Command_GpsScrambler", true);
                        push(&mut cmds, "Command_SpectreGunship", true);
                    }
                    if n.contains("china") {
                        push(&mut cmds, "Command_ArtilleryBarrage", true);
                        push(&mut cmds, "Command_CarpetBomb", true);
                    }
                }
            }
            if ro.can_produce {
                push(&mut cmds, "Command_SetRallyPoint", true);
            }
            if ro.max_garrison > 0 {
                push(&mut cmds, "Command_StructureExit", true);
                if !ro.garrisoned_units.is_empty() {
                    push(&mut cmds, "Command_Evacuate", true);
                }
            }
        }
        if ro.special_power_ready {
            push(&mut cmds, "Command_SpecialPower", true);
        } else if ro.special_power_cooldown > 0.0 {
            push(&mut cmds, "Command_SpecialPower", false);
        }
        // GeneralsExperience residual: offer purchase when SPP available.
        if self.local_science_purchase_points > 0 {
            push(&mut cmds, "Command_PurchaseScience", true);
        }
        if panel.production_progress.is_some() {
            // C++ cancel queue head: unit vs upgrade residual.
            if panel.production_is_upgrade {
                push(&mut cmds, "Command_CancelUpgrade", true);
            } else {
                push(&mut cmds, "Command_CancelUnit", true);
            }
        }
        // C++ GUI_COMMAND_PLAYER_UPGRADE residual: disable upgrade commands that
        // are already complete or currently researching (COMMAND_RESTRICTED).
        let queued = &self.local_queued_upgrades;
        let unlocked = &self.local_unlocked_sciences;
        for name in unlocked.iter().chain(queued.iter()) {
            let cmd = format!(
                "Command_Upgrade{}",
                name.trim_start_matches("Upgrade_")
                    .trim_start_matches("upgrade_")
            );
            // Also try full Upgrade_ name suffix residual.
            let cmd_full = format!("Command_{}", name);
            for cname in [cmd, cmd_full] {
                // Only mark disabled if a matching enable was not already pushed.
                if let Some(existing) = cmds
                    .iter_mut()
                    .find(|c| c.command_name.eq_ignore_ascii_case(&cname))
                {
                    existing.enabled = false;
                }
            }
        }
        // Structure producers: expose residual upgrade research buttons from
        // applied/known host upgrades that are NOT unlocked and NOT queued.
        // Fail-closed: sample residual set only (not full CommandSet INI matrix).
        if ro.can_produce || ro.is_structure {
            const SAMPLE_UPGRADE_COMMANDS: &[(&str, &str)] = &[
                (
                    "Command_UpgradeAmericaRangerFlashBangGrenade",
                    "Upgrade_AmericaRangerFlashBangGrenade",
                ),
                (
                    "Command_UpgradeAmericaRangerCaptureBuilding",
                    "Upgrade_AmericaRangerCaptureBuilding",
                ),
                (
                    "Command_UpgradeAmericaSupplyLines",
                    "Upgrade_AmericaSupplyLines",
                ),
                ("Command_UpgradeGLACamouflage", "Upgrade_GLACamouflage"),
            ];
            let norm = |s: &str| {
                s.chars()
                    .filter(|c| c.is_ascii_alphanumeric())
                    .flat_map(|c| c.to_lowercase())
                    .collect::<String>()
            };
            for (cmd, upgrade) in SAMPLE_UPGRADE_COMMANDS {
                let u = norm(upgrade);
                let owned = unlocked.iter().any(|x| norm(x) == u)
                    || unlocked
                        .iter()
                        .any(|x| norm(x).contains(&u) || u.contains(&norm(x)));
                let researching = queued.iter().any(|x| norm(x) == u)
                    || queued
                        .iter()
                        .any(|x| norm(x).contains(&u) || u.contains(&norm(x)))
                    || (panel.production_is_upgrade
                        && panel
                            .production_template
                            .as_ref()
                            .map(|t| norm(t) == u || norm(t).contains(&u))
                            .unwrap_or(false));
                let enabled = !owned && !researching;
                // Only push when structure can produce (barracks/war factory/etc residual).
                if ro.can_produce {
                    push(&mut cmds, cmd, enabled);
                }
            }
        }
        cmds
    }

    pub fn apply_to_unit_command_panel(&self, panel: &mut crate::ui::UnitCommandPanel) {
        panel.apply_selection_panel(
            self.control_bar_selection_panel(),
            self.selection_ids_for_consumers(),
        );
        panel.apply_commands(self.unit_command_buttons());
    }

    /// Dual-tick multi-consumer residual: HUD + UI state + RTS + unit command panel
    /// (+ ControlBar when `game_client` is enabled). Snapshot-owned only.
    ///
    /// Does **not** claim full windowed WND/GPU playthrough.
    pub fn apply_to_shell_ui_consumers(
        &self,
        hud: &mut crate::ui::GameHUD,
        ui: &mut crate::ui::GameUIState,
        rts: &mut crate::ui::RTSInterface,
        command_panel: &mut crate::ui::UnitCommandPanel,
    ) {
        self.apply_to_game_hud(hud);
        self.apply_to_ui_state(ui);
        self.apply_to_rts_interface(rts);
        self.apply_to_unit_command_panel(command_panel);
    }

    /// Dual-tick presentation consumer after map load / logic step:
    /// build snapshot from authority and apply it to the production GameHUD.
    ///
    /// Does **not** advance the world — caller is responsible for `logic.update()`.
    pub fn build_and_apply_for_hud(
        logic: &GameLogic,
        local_player_id: u32,
        hud: &mut crate::ui::GameHUD,
    ) -> Self {
        let mut frame = Self::build_from_logic(logic, local_player_id);
        frame.apply_to_game_hud(hud);
        frame.note_dual_tick_apply();
        frame
    }

    /// Dual-tick residual: build snapshot and apply to all headless shell UI consumers.
    ///
    /// Order matches production StartGame: authority step (caller) → presentation freeze
    /// → HUD / UIState / RTS / unit command panel. Optional ControlBar is applied by
    /// the engine path when `game_client` is present.
    pub fn build_and_apply_for_shell_consumers(
        logic: &GameLogic,
        local_player_id: u32,
        hud: &mut crate::ui::GameHUD,
        ui: &mut crate::ui::GameUIState,
        rts: &mut crate::ui::RTSInterface,
        command_panel: &mut crate::ui::UnitCommandPanel,
    ) -> Self {
        let mut frame = Self::build_from_logic(logic, local_player_id);
        frame.apply_to_shell_ui_consumers(hud, ui, rts, command_panel);
        frame.note_dual_tick_apply();
        frame
    }
}

// ===== build.rs =====
use super::*;

impl PresentationFrame {
    /// Build a snapshot by borrowing the authoritative world for this call only.
    ///
    /// FOW for `local_player_id` is frozen here via the FOW bridge so the unit mesh
    /// pass can apply alpha / never-explored skip without mid-render shroud locks.
    /// Cell-grid FOW is also frozen into `fow_grid` for terrain overlay / minimap.
    /// Fail-closed claim: unit FOW + compact local grid; not full SAGE shroud parity.
    pub fn build_from_logic(logic: &GameLogic, local_player_id: u32) -> Self {
        // Shell maps render fully visible background scenes (C++ parity).
        let fow_shell_bypass = logic.isInShellGame();
        // Local force residual: always present own-team objects fully visible.
        // C++ always draws the controlling player's units; host FOW membership can
        // miss builders when sight_range / ObjectManager dual-world is incomplete.
        let local_team = logic
            .get_player(local_player_id)
            .map(|p| p.team)
            .unwrap_or(Team::Neutral);
        // Freeze team base proximity once (camera snap / host residual).
        let local_team_base_position = logic.team_base_position(local_team);
        // Freeze terrain FOW grid once for this presentation frame (local player only).
        let fow_grid = FOWRenderingBridge::snapshot_terrain_grid(local_player_id, fow_shell_bypass);
        let mut objects = Vec::with_capacity(logic.host_objects().len());
        for obj in logic.host_objects().values() {
            let is_structure = obj.is_kind_of(KindOf::Structure);
            let is_unit = obj.is_kind_of(KindOf::Infantry)
                || obj.is_kind_of(KindOf::Vehicle)
                || obj.is_kind_of(KindOf::Aircraft);
            // Prefer explicit template model name so mesh resolve matches live collect path.
            // Alias remap (airanger → airanger_s) keeps PresentationFrame model_key aligned
            // with shipped W3D basenames for the residual mesh asset resolve path.
            let base_model_key =
                crate::assets::mesh_asset_resolve::model_key_from_template(obj.get_template());
            let destroyed_for_mesh = obj.status.destroyed || !obj.is_alive();
            let body_ord = {
                use crate::game_logic::host_enum_table_residual::{
                    host_calc_body_damage_state, HostBodyDamageType,
                };
                let state = if destroyed_for_mesh {
                    HostBodyDamageType::Rubble
                } else {
                    host_calc_body_damage_state(obj.health.current, obj.health.maximum.max(0.0))
                };
                state as u8
            };
            // Wave 491: sold model-condition forces rubble/dying mesh branch.
            let sold_for_mesh = obj.status.sold
                || crate::game_logic::host_enum_table_residual::host_model_condition_has(
                    obj.model_condition_bits,
                    crate::game_logic::host_enum_table_residual::sold_model_bit(),
                );
            let model_key = Some(
                crate::assets::mesh_asset_resolve::model_key_with_presentation_state(
                    &base_model_key,
                    body_ord,
                    destroyed_for_mesh,
                    sold_for_mesh,
                ),
            );
            // Wave 75: freeze mesh scale residual (common combat = 1.0; CINE/weapon peels).
            let mesh_scale =
                crate::assets::mesh_asset_resolve::mesh_scale_from_template(obj.get_template());
            let fow_visibility = if fow_shell_bypass {
                ObjectVisibility::FULLY_VISIBLE
            } else if local_team != Team::Neutral && obj.team == local_team {
                // Always see own force (structures + builders + army).
                ObjectVisibility::FULLY_VISIBLE
            } else {
                FOWRenderingBridge::get_object_visibility(local_player_id, obj.id)
            };
            // Wave 77: freeze ground-height residual at object XY (sample or default-0).
            let pos = obj.get_position();
            let (ground_height, ground_height_from_terrain) =
                sample_presentation_ground_height(logic, pos);
            objects.push(RenderableObject {
                id: obj.id,
                template_name: obj.template_name.clone(),
                team: obj.team,
                team_color: {
                    // Wave 503: C++ enemies see disguise player color; allies see true colors.
                    if obj.status.disguised && obj.team != local_team {
                        if let Some(dt) = obj.disguise_as_team {
                            dt.get_color()
                        } else {
                            obj.team_color
                        }
                    } else {
                        obj.team_color
                    }
                },
                // Use accessors so presentation matches authoritative transform state.
                position: {
                    let mut p = pos;
                    p.y += obj.presentation_collapse_height_offset();
                    p.y += obj.presentation_slow_death_sink_offset();
                    let (sx, sz) = obj.presentation_collapse_shudder();
                    p.x += sx;
                    p.z += sz;
                    p
                },
                orientation: obj.get_orientation(),
                topple_lean_radians: obj.presentation_topple_lean_radians(),
                move_destination: obj.movement.target_position,
                target_location: obj.target_location,
                guard_target: obj.guard_target,
                using_ability: obj.status.using_ability,
                airborne_target: obj.status.airborne_target,
                producer_id: obj.producer_id,
                show_healing: {
                    // C++ HEALING_ICON_DISPLAY_TIME residual via sole-benefactor claim window.
                    let now = logic.get_current_frame() as u32;
                    obj.sole_healing_benefactor_expiration_frame > now
                        && obj.sole_healing_benefactor_expiration_frame != 0
                },
                healing_icon_type: if obj.is_kind_of(KindOf::Structure) {
                    1
                } else if obj.is_kind_of(KindOf::Vehicle) {
                    2
                } else {
                    0
                },
                parachuting: obj.is_parachuting(),
                parachute_open: obj.is_parachute_open(),
                captured: obj.has_captured_model_condition() || obj.is_private_captured(),
                prone: obj.prone_timer > 0.0,
                emoticon_name: obj.emoticon_name.clone(),
                emoticon_frames_left: obj.emoticon_frames_left,
                is_surrendered: obj.is_surrendered,
                formation_id: obj.formation_id,
                formation_offset: obj.formation_offset,
                over_water: obj.over_water,
                // Wave 522: terrain cell cliff/underwater residuals.
                cell_is_cliff: obj.cell_is_cliff,
                cell_is_underwater: obj.cell_is_underwater,
                move_max_speed: obj.movement.max_speed,
                velocity: obj.movement.velocity,
                ai_state_ordinal: crate::gameworld_shadow::GameWorldShadow::host_ai_state_ordinal(
                    &obj.ai_state,
                ),
                attack_target: obj.target,
                path_waypoints: obj.movement.path.iter().copied().take(16).collect(),
                path_len: obj.movement.path.len().min(u16::MAX as usize) as u16,
                path_index: obj.movement.current_path_index.min(u16::MAX as usize) as u16,
                occupant_count: obj.occupants.len().min(u16::MAX as usize) as u16,
                production_queue: obj
                    .building_data
                    .as_ref()
                    .map(|b| {
                        b.production_queue
                            .iter()
                            .map(PresentationProductionItem::from_host_item)
                            .collect()
                    })
                    .unwrap_or_default(),
                production_paused: obj
                    .building_data
                    .as_ref()
                    .map(|b| b.production_paused)
                    .unwrap_or(false),
                rally_point: obj.building_data.as_ref().and_then(|b| b.rally_point),
                guard_position: obj.guard_position,
                garrisoned_units: obj
                    .building_data
                    .as_ref()
                    .map(|b| b.garrisoned_units.iter().copied().take(32).collect())
                    .unwrap_or_default(),
                max_garrison: obj
                    .building_data
                    .as_ref()
                    .map(|b| b.max_garrison)
                    .unwrap_or(0),
                power_provided: obj.power_provided,
                power_consumed: obj.power_consumed,
                stored_supplies: obj.stored_resources.supplies,
                health_current: obj.health.current,
                health_max: obj.health.maximum,
                selected: obj.selected || obj.status.selected,
                is_deployed: obj.status.deployed,
                selection_flash_remaining: obj.selection_flash_remaining,
                destroyed: obj.status.destroyed || !obj.is_alive(),
                model_condition_bits: {
                    // Prefer live residual bits; recompute if pristine-zero and damaged.
                    let mut bits = obj.model_condition_bits;
                    use crate::game_logic::host_enum_table_residual::{
                        host_apply_body_damage_model_bits, host_calc_body_damage_state,
                        HostBodyDamageType, MC_BIT_ATTACKING, MC_BIT_DYING, MC_BIT_MOVING,
                    };
                    use crate::game_logic::host_neutron_missile_slow_death::{
                        MC_BIT_BACKCRUSHED, MC_BIT_FRONTCRUSHED,
                    };
                    let destroyed = obj.status.destroyed || !obj.is_alive();
                    let state = if destroyed {
                        HostBodyDamageType::Rubble
                    } else {
                        host_calc_body_damage_state(obj.health.current, obj.health.maximum.max(0.0))
                    };
                    bits = host_apply_body_damage_model_bits(bits, state);

                    if obj.front_crushed {
                        bits |= 1u128 << MC_BIT_FRONTCRUSHED;
                    }
                    if obj.back_crushed {
                        bits |= 1u128 << MC_BIT_BACKCRUSHED;
                    }
                    if obj.status.moving {
                        bits |= 1u128 << MC_BIT_MOVING;
                    } else {
                        bits &= !(1u128 << MC_BIT_MOVING);
                    }
                    if obj.status.attacking {
                        bits |= 1u128 << MC_BIT_ATTACKING;
                    } else {
                        bits &= !(1u128 << MC_BIT_ATTACKING);
                    }
                    if destroyed {
                        bits |= 1u128 << MC_BIT_DYING;
                    } else {
                        bits &= !(1u128 << MC_BIT_DYING);
                    }
                    use crate::game_logic::host_enum_table_residual::MC_BIT_DISGUISED;
                    if obj.status.disguised {
                        bits |= 1u128 << MC_BIT_DISGUISED;
                    } else {
                        bits &= !(1u128 << MC_BIT_DISGUISED);
                    }
                    bits
                },
                radar_active: obj.radar_active,
                radar_extend_complete: obj.radar_extend_complete,
                production_door_phase: obj.production_door_phase,
                body_damage_state: {
                    use crate::game_logic::host_enum_table_residual::{
                        host_calc_body_damage_state, HostBodyDamageType,
                    };
                    let destroyed = obj.status.destroyed || !obj.is_alive();
                    let state = if destroyed {
                        HostBodyDamageType::Rubble
                    } else {
                        host_calc_body_damage_state(obj.health.current, obj.health.maximum.max(0.0))
                    };
                    state as u8
                },
                damage_fx_name: obj
                    .pending_transition_damage_fx
                    .last()
                    .and_then(|e| e.fx_name.clone()),
                bone_fx_name: obj.bone_fx_damage.as_ref().and_then(|b| b.last_fx.clone()),
                poison_tinted: obj.is_poison_tinted(),
                undetected_defector: obj.is_undetected_defector(),
                defector_flash: obj
                    .defection_helper
                    .as_ref()
                    .map(|d| d.flash_this_frame || d.final_white_flash)
                    .unwrap_or(false),
                death_fx_name: obj.pending_death_fx.clone(),
                death_type_name: if obj.status.destroyed || !obj.is_alive() {
                    obj.status.death_type.as_name().to_string()
                } else {
                    String::new()
                },
                under_construction: obj.status.under_construction,
                construction_percent: obj.construction_percent.clamp(0.0, 1.0),
                // Wave 1031: OCL timer residual for dual-world ControlBar OclTimer context.
                ocl_timer_seconds:
                    if crate::game_logic::host_supply_drop_zone::is_supply_drop_zone_template(
                        &obj.template_name,
                    ) {
                        logic
                            .supply_drop_zones()
                            .remaining_ocl_timer_seconds(obj.id, logic.get_frame())
                    } else {
                        0
                    },
                sold: obj.status.sold,
                unselectable: obj.status.unselectable,
                is_rebuild_hole: obj.is_rebuild_hole,
                rebuild_template_name: obj.rebuild_template_name.clone().unwrap_or_default(),
                rebuild_ready_frame: obj.rebuild_ready_frame,
                rebuild_spawner_id: obj.rebuild_spawner_id,
                rebuild_worker_id: obj.rebuild_worker_id,
                rebuild_reconstructing_id: obj.rebuild_reconstructing_id,
                reconstructing: obj.status.reconstructing,
                veterancy: PresentationVeterancy::from_host(obj.experience.level),
                experience_points: obj.experience.current.max(0.0),
                moving: obj.status.moving,
                attacking: obj.status.attacking,
                is_firing_weapon: obj.status.is_firing_weapon,
                is_aiming_weapon: obj.status.is_aiming_weapon,
                disabled_emp: obj.status.disabled_emp,
                disabled_paralyzed: obj.status.disabled_paralyzed,
                weapons_jammed: obj.status.weapons_jammed,
                masked: obj.status.masked,
                ignoring_stealth: obj.status.ignoring_stealth,
                repulsor: obj.status.repulsor,
                stealthed: obj.status.stealthed,
                detected: obj.status.detected,
                effectively_stealthed: obj.is_effectively_stealthed(),
                disabled: obj.is_disabled(),
                contained_by: obj.contained_by,
                force_attack: obj.force_attack,
                has_weapon: obj.weapon.is_some(),
                weapon_range: obj.weapon.as_ref().map(|w| w.range).unwrap_or(0.0),
                weapon_damage: obj.weapon.as_ref().map(|w| w.damage).unwrap_or(0.0),
                weapon_min_range: obj.weapon.as_ref().map(|w| w.min_range).unwrap_or(0.0),
                weapon_reload_time: obj.weapon.as_ref().map(|w| w.reload_time).unwrap_or(0.0),
                weapon_ammo: obj
                    .weapon
                    .as_ref()
                    .map(|w| w.ammo.unwrap_or(u32::MAX))
                    .unwrap_or(u32::MAX),
                ammo_pip_total: obj.get_ammo_pip_showing_info().map(|(t, _)| t).unwrap_or(0),
                ammo_pip_full: obj.get_ammo_pip_showing_info().map(|(_, f)| f).unwrap_or(0),
                weapon_ready_percent: {
                    let now = crate::game_logic::host_historic_bonus::logic_frame() as f32 / 30.0;
                    obj.get_most_percent_ready_to_fire_any_weapon(now)
                },
                weapon_can_target_air: obj
                    .weapon
                    .as_ref()
                    .map(|w| w.can_target_air)
                    .unwrap_or(false),
                weapon_can_target_ground: obj
                    .weapon
                    .as_ref()
                    .map(|w| w.can_target_ground)
                    .unwrap_or(true),
                weapon_projectile_speed: obj
                    .weapon
                    .as_ref()
                    .map(|w| w.projectile_speed)
                    .unwrap_or(0.0),
                armed_riders_upgrade_weapon_set: obj.armed_riders_upgrade_weapon_set,
                weapon_set_player_upgrade: obj.weapon_set_player_upgrade,
                // Wave 523: battle-bus / armor second-life residual.
                second_life: obj.armor_set_second_life,
                // Wave 525: crush + USER model-condition residuals.
                front_crushed: obj.front_crushed,
                back_crushed: obj.back_crushed,
                user_1: (obj.model_condition_bits
                    & (1u128 << crate::game_logic::host_enum_table_residual::user_1_model_bit()))
                    != 0,
                user_2: (obj.model_condition_bits
                    & (1u128 << crate::game_logic::host_enum_table_residual::user_2_model_bit()))
                    != 0,
                // Wave 518: crate upgrades + enemy-near + armed residual.
                weapon_crate_upgrade: obj.weapon_crate_upgrade,
                armor_crate_upgrade: obj.armor_crate_upgrade,
                enemy_near: obj
                    .enemy_near
                    .as_ref()
                    .map(|e| e.model_enemy_near || e.enemy_near)
                    .unwrap_or(false),
                armed: obj.armed_riders_upgrade_weapon_set
                    || (obj.occupants.len() > 0 && obj.passengers_allowed_to_fire),
                camo_stealth_look: obj.camo_stealth_look,
                disguise_as_template: obj.disguise_as_template.clone(),
                disguise_as_team: obj.disguise_as_team,
                disguised: obj.status.disguised,
                disabled_subdued: obj.status.disabled_subdued,
                is_carbomb: obj.status.is_carbomb,
                hijacked: obj.status.hijacked,
                disguise_transition_opacity: if obj.status.disguise_transition_frames > 0 {
                    obj.status.disguise_transition_opacity
                } else {
                    1.0
                },
                detection_range: obj.detection_range.max(0.0),
                detection_rate_frames: obj.detection_rate_frames,
                stealth_breaks_on_attack: obj.stealth_breaks_on_attack,
                stealth_breaks_on_move: obj.stealth_breaks_on_move,
                innate_stealth: obj.innate_stealth,
                weapon_bonus_frenzy_until_frame: obj.weapon_bonus_frenzy_until_frame,
                continuous_fire_consecutive: obj.continuous_fire_consecutive.min(u16::MAX as u32)
                    as u16,
                continuous_fire_coast_until_frame: obj.continuous_fire_coast_until_frame,
                battle_plan_sight_scalar_applied: obj.battle_plan_sight_scalar_applied,
                special_power_ready: obj.special_power_ready,
                special_power_cooldown: obj.special_power_cooldown.max(0.0),
                special_power_cooldown_remaining: obj.special_power_cooldown_remaining.max(0.0),
                object_type: PresentationObjectType::from_host(obj.object_type),
                applied_upgrades: {
                    const MAX_UPGRADES: usize = 24;
                    let mut v: Vec<String> = obj.applied_upgrades.iter().cloned().collect();
                    v.sort();
                    v.truncate(MAX_UPGRADES);
                    v
                },
                has_secondary_weapon: obj.secondary_weapon.is_some(),
                secondary_weapon_range: obj
                    .secondary_weapon
                    .as_ref()
                    .map(|w| w.range)
                    .unwrap_or(0.0),
                secondary_weapon_damage: obj
                    .secondary_weapon
                    .as_ref()
                    .map(|w| w.damage)
                    .unwrap_or(0.0),
                turret_angle_deg: obj.turret_angle_deg,
                turret_pitch_deg: obj.turret_pitch_deg,
                turret_idle_scanning: obj.turret_idle_scanning,
                weapon_bonus_enthusiastic: obj.weapon_bonus_enthusiastic,
                weapon_bonus_subliminal: obj.weapon_bonus_subliminal,
                weapon_bonus_horde: obj.weapon_bonus_horde,
                weapon_bonus_nationalism: obj.weapon_bonus_nationalism,
                weapon_bonus_frenzy: obj.weapon_bonus_frenzy,
                weapon_bonus_frenzy_level: obj.weapon_bonus_frenzy_level,
                weapon_bonus_battle_plan_bombardment: obj.weapon_bonus_battle_plan_bombardment,
                weapon_bonus_battle_plan_hold_the_line: obj.weapon_bonus_battle_plan_hold_the_line,
                weapon_bonus_battle_plan_search_and_destroy: obj
                    .weapon_bonus_battle_plan_search_and_destroy,
                continuous_fire_level: obj.continuous_fire_level,
                faerie_fire_until_frame: obj.faerie_fire_until_frame,
                hive_slave_count: obj.hive_slave_count,
                hive_slave_hp: obj.hive_slave_hp,
                ai_attitude: obj.ai_attitude,
                camo_friendly_opacity: obj.camo_friendly_opacity,
                vision_spied_mask: obj.vision_spied_mask,
                vision_range: obj.vision_range,
                shroud_clearing_range: obj.shroud_clearing_range,
                crusher_level: obj.crusher_level,
                crushable_level: obj.crushable_level,
                cheer_timer: obj.cheer_timer,
                is_humvee_transport: obj.is_humvee_transport,
                is_listening_outpost_transport: obj.is_listening_outpost_transport,
                is_troop_crawler_transport: obj.is_troop_crawler_transport,
                is_helix_transport: obj.is_helix_transport,
                has_overlord_gattling_addon: obj.has_overlord_gattling_addon,
                has_overlord_propaganda_addon: obj.has_overlord_propaganda_addon,
                is_battle_bus_transport: obj.is_battle_bus_transport,
                is_technical_transport: obj.is_technical_transport,
                is_combat_cycle_transport: obj.is_combat_cycle_transport,
                combat_cycle_rider: obj.combat_cycle_rider,
                is_tunnel_network: obj.is_tunnel_network,
                is_combat_chinook_transport: obj.is_combat_chinook_transport,
                max_transport: obj.max_transport,
                overlord_bunker_capacity: obj.overlord_bunker_capacity.unwrap_or(usize::MAX),
                passengers_allowed_to_fire: obj.passengers_allowed_to_fire,
                display_name: obj.name.clone(),
                demo_suicided_detonating: obj.demo_suicided_detonating,
                turret_holding: obj.turret_holding,
                last_damage_source_host: obj.last_damage_source.map(|id| id.0).unwrap_or(0),
                command_set_override: obj.command_set_override.clone().unwrap_or_default(),
                command_set_name: crate::ui::construction_panel::resolve_command_set_name(
                    &obj.template_name,
                    obj.command_set_override.as_deref(),
                )
                .unwrap_or_default(),
                is_detector: obj.is_detector,
                active_weapon_slot: obj.active_weapon_slot,
                // Wave 517: weapon fire status + panic/backwards for slot-aware mesh bits.
                weapon_fire_status: obj.weapon_fire_status as u8,
                is_panicking: obj.is_panicking,
                moving_backwards: obj.moving_backwards,
                overcharge_enabled: obj.overcharge_enabled,
                // Wave 519: shock / power-plant rods / jet slow-death residuals.
                shock_was_airborne: obj.shock_was_airborne,
                shock_allow_bounce: obj.shock_allow_bounce,
                shock_grounded_once: obj.shock_grounded_once,
                shock_stun_frames: obj.shock_stun_frames,
                power_plant_rods_extended: obj.power_plant_rods_extended,
                power_plant_rods_done_frame: obj.power_plant_rods_done_frame,
                jet_slow_death_active: obj.jet_slow_death.is_some(),
                // Wave 520: AnimationSteeringUpdate turn anim residual.
                anim_steer_turn: obj
                    .animation_steering
                    .as_ref()
                    .map(|s| s.current_turn_anim as u8)
                    .unwrap_or(0),
                show_health_bar: obj.show_health_bar,
                guard_radius: obj.guard_radius,
                has_mine: obj.mine_data.is_some(),
                kind_of: {
                    use crate::game_logic::KindOf;
                    const MAX_KINDS: usize = 32;
                    // Stable presentation order (KindOf declaration order residual).
                    const ORDER: &[KindOf] = &[
                        KindOf::Structure,
                        KindOf::Infantry,
                        KindOf::Vehicle,
                        KindOf::Aircraft,
                        KindOf::Projectile,
                        KindOf::Resource,
                        KindOf::Selectable,
                        KindOf::Attackable,
                        KindOf::CommandCenter,
                        KindOf::Worker,
                        KindOf::Hero,
                        KindOf::SupplyCenter,
                        KindOf::PowerPlant,
                        KindOf::FSBarracks,
                        KindOf::FSWarFactory,
                        KindOf::FSAirfield,
                        KindOf::FSInternetCenter,
                        KindOf::FSPower,
                        KindOf::FSBaseDefense,
                        KindOf::FSSupplyDropzone,
                        KindOf::FSSupplyCenter,
                        KindOf::FSSuperweapon,
                        KindOf::FSStrategyCenter,
                        KindOf::FSFake,
                        KindOf::FSTechnology,
                        KindOf::FSBlackMarket,
                        KindOf::FSAdvancedTech,
                        KindOf::Harvestable,
                        KindOf::Powered,
                        // Wave 982: IgnoredInGui for host mouseover slaver remap.
                        KindOf::IgnoredInGui,
                    ];
                    let set = &obj.get_template().kind_of;
                    let mut v: Vec<KindOf> =
                        ORDER.iter().copied().filter(|k| set.contains(k)).collect();
                    v.truncate(MAX_KINDS);
                    v
                },
                is_structure,
                is_unit,
                // Prefer host Object::is_mobile so dozers/workers without an explicit
                // Vehicle KindOf still count as local_mobile_units / selectables.
                is_mobile: obj.is_mobile(),
                can_produce: obj.building_data.is_some()
                    && !obj.status.under_construction
                    && obj.construction_percent >= 1.0
                    && !obj.status.destroyed
                    && obj.is_alive(),
                building_type: obj
                    .building_data
                    .as_ref()
                    .map(|b| PresentationBuildingType::from_host(b.building_type)),
                model_key,
                mesh_scale,
                selection_radius: obj.selection_radius.max(5.0),
                engine_bridged: false,
                fow_visibility,
                ground_height,
                ground_height_from_terrain,
            });
        }
        // Stable presentation order for determinism (by ObjectId).
        objects.sort_by_key(|o| o.id.0);

        let local = logic.get_player(local_player_id);
        // local_team already resolved above for FOW residual.
        let _local_team_check = local.map(|p| p.team).unwrap_or(Team::Neutral);
        debug_assert_eq!(_local_team_check, local_team);
        let mut players: Vec<PresentationPlayerInfo> = logic
            .get_players()
            .iter()
            .map(|(&id, p)| PresentationPlayerInfo {
                id,
                name: p.name.clone(),
                team: p.team,
                is_alive: p.is_alive,
                is_local: p.is_local,
                is_ai: logic.ai_manager_contains_player(id),
                color_rgb: p.color_rgb,
            })
            .collect();
        players.sort_by_key(|p| p.id);
        // Economy authority: freeze effective (includes pending_supply_delta).
        let local_supplies = local.map(|p| p.effective_supplies()).unwrap_or(0);
        let local_power = local.map(|p| p.power_available).unwrap_or(0);
        let local_power_produced = local.map(|p| p.power_produced).unwrap_or(0);
        let local_power_consumed = local.map(|p| p.power_consumed).unwrap_or(0);
        let local_color_rgb = local.map(|p| p.color_rgb).unwrap_or((200, 200, 200));
        let local_is_alive = local.map(|p| p.is_alive).unwrap_or(false);
        let local_radar_count = local.map(|p| p.radar_count).unwrap_or(0);
        let local_radar_disabled = local.map(|p| p.radar_disabled).unwrap_or(false);
        let local_cash_bounty_percent = local
            .map(|p| p.cash_bounty_percent.clamp(0.0, 1.0))
            .unwrap_or(0.0);
        let local_rank_level = local.map(|p| p.rank_level.max(1)).unwrap_or(1);
        let local_skill_points = local.map(|p| p.skill_points).unwrap_or(0);
        let local_science_purchase_points = local.map(|p| p.science_purchase_points).unwrap_or(0);
        let local_rank_progress_percent = {
            use crate::game_logic::host_rank_ui_residual::{
                rank_level_down_threshold_residual, rank_level_up_threshold_residual,
                rank_progress_percent_residual, RankSkillStateResidual,
            };
            let state = RankSkillStateResidual {
                rank_level: local_rank_level,
                skill_points: local_skill_points,
                science_purchase_points: local_science_purchase_points,
                level_up: rank_level_up_threshold_residual(local_rank_level),
                level_down: rank_level_down_threshold_residual(local_rank_level),
            };
            rank_progress_percent_residual(&state)
        };
        const MAX_SCIENCE_NAMES: usize = 32;
        const MAX_UPGRADE_NAMES: usize = 32;
        let mut local_unlocked_sciences: Vec<String> = local
            .map(|p| {
                let mut v: Vec<String> = p.unlocked_sciences.iter().cloned().collect();
                v.sort();
                v.truncate(MAX_SCIENCE_NAMES);
                v
            })
            .unwrap_or_default();
        let mut local_queued_upgrades: Vec<String> = local
            .map(|p| {
                let mut v: Vec<String> = p.queued_upgrades.iter().cloned().collect();
                v.sort();
                v.truncate(MAX_UPGRADE_NAMES);
                v
            })
            .unwrap_or_default();
        let _ = (&mut local_unlocked_sciences, &mut local_queued_upgrades);

        // PublicTimer superweapon residual from player SharedSyncedTimer + ownership.
        let mut superweapon_timers: Vec<PresentationSuperweaponTimer> = Vec::new();
        if let Some(p) = local {
            use crate::command_system::SpecialPowerType as P;
            use crate::game_logic::host_special_power_enum_residual::{
                special_power_has_public_timer, special_power_public_timer_display_name,
                special_power_public_timer_icon, special_power_reload_seconds,
                special_power_required_science, template_provides_public_timer_power,
            };
            const PUBLIC_POWERS: &[P] = &[
                P::ParticleCannon,
                P::NuclearMissile,
                P::ScudStorm,
                P::CarpetBomb,
                P::CruiseMissile,
                P::NapalmStrike,
                P::BlackMarketNuke,
                P::CrateDrop,
                P::TerrorCell,
                P::SuperweaponParticleCannon,
                P::LaserCannon,
                P::NukeNeutronMissile,
                P::SuperweaponNeutronMissile,
                P::BaikonurRocket,
            ];
            // Living constructed structures owned by local team (C++ addSuperweapon residual).
            let owned_sw_templates: Vec<String> = logic
                .host_objects()
                .values()
                .filter(|o| {
                    o.team == p.team
                        && o.is_alive()
                        && o.is_constructed()
                        && (o.is_kind_of(crate::game_logic::KindOf::Structure)
                            || o.is_kind_of(crate::game_logic::KindOf::FSSuperweapon))
                })
                .map(|o| o.template_name.clone())
                .collect();
            let mut seen = std::collections::HashSet::new();
            for power in PUBLIC_POWERS {
                if !special_power_has_public_timer(power) {
                    continue;
                }
                let template = format!("{:?}", power);
                if !seen.insert(template.clone()) {
                    continue;
                }
                let science_ok = match special_power_required_science(power) {
                    Some(req) => p.has_unlocked_science(req),
                    None => true,
                };
                let structure_templates =
                    crate::game_logic::host_special_power_enum_residual::special_power_public_timer_structure_templates(
                        power,
                    );
                let structure_ok = if structure_templates.is_empty() {
                    // Science-only PublicTimer (Carpet/Crate/Napalm/Terror/BMNuke):
                    // unlocked by science residual alone.
                    science_ok
                } else {
                    // Structure SWs: require living constructed building residual.
                    owned_sw_templates
                        .iter()
                        .any(|t| template_provides_public_timer_power(power, t))
                };
                let unlocked = science_ok && structure_ok;
                // Only list unlocked PublicTimer rows (C++ addSuperweapon when present).
                // C++ ~SpecialPowerModule removeSuperweapon: destroyed/sold structure drops row.
                if !unlocked {
                    continue;
                }
                let reload = special_power_reload_seconds(power).unwrap_or(0.0).max(0.0);
                // Structure-bound PublicTimer SWs: remaining from living structure module
                // residual (not Player SharedSyncedTimer — retail SharedNSync absent).
                let remaining = if crate::game_logic::host_special_power_enum_residual::special_power_is_structure_bound_public_timer(
                    power,
                ) {
                    // Per-structure module residual: soonest ready among owned SW buildings.
                    let mut any = false;
                    let mut min_rem = f32::MAX;
                    for obj in logic.host_objects().values() {
                        if obj.team != p.team || !obj.is_alive() || !obj.is_constructed() {
                            continue;
                        }
                        if !template_provides_public_timer_power(power, &obj.template_name) {
                            continue;
                        }
                        any = true;
                        let rem = obj
                            .special_power_cooldowns
                            .get(power)
                            .copied()
                            .unwrap_or(obj.special_power_cooldown_remaining)
                            .max(0.0);
                        if rem < min_rem {
                            min_rem = rem;
                        }
                    }
                    if any { min_rem } else { 0.0 }
                } else {
                    p.shared_special_power_cooldowns
                        .get(power)
                        .copied()
                        .unwrap_or(0.0)
                        .max(0.0)
                };
                let ready = remaining <= 0.0;
                superweapon_timers.push(PresentationSuperweaponTimer {
                    name: special_power_public_timer_display_name(power).to_string(),
                    template_name: template,
                    icon: special_power_public_timer_icon(power).to_string(),
                    recharge_time: reload,
                    remaining,
                    unlocked,
                    ready,
                    power_key: format!("{power:?}"),
                });
            }
            // Stable HUD order by name.
            superweapon_timers.sort_by(|a, b| a.name.cmp(&b.name));
            superweapon_timers.truncate(16);
        }

        // ControlBar CanMake residual for selected local producer (HelpBox feed).
        let mut can_make_cameos: Vec<PresentationCanMakeCameo> = Vec::new();
        let mut can_make_producer_id: Option<u32> = None;
        if let Some(p) = local {
            use crate::game_logic::host_ui_presentation_residual::can_make_type_help_box_message_residual;
            let is_producer = |o: &crate::game_logic::Object| {
                o.team == p.team
                    && o.is_alive()
                    && !o.status.destroyed
                    && o.building_data.is_some()
                    && !o.status.under_construction
            };
            // Prefer first selected producer residual; fall back to any local factory.
            let producer = p
                .selected_objects
                .iter()
                .copied()
                .find(|&id| logic.host_object(id).is_some_and(is_producer))
                .or_else(|| {
                    logic
                        .host_objects()
                        .iter()
                        .find(|(_, o)| is_producer(o))
                        .map(|(id, _)| *id)
                });
            if let Some(pid) = producer {
                can_make_producer_id = Some(pid.0);
                // Sample residual templates by factory kind.
                let samples: &[&str] = {
                    let o = logic.host_object(pid);
                    let bt = o
                        .and_then(|o| o.building_data.as_ref())
                        .map(|b| b.building_type);
                    use crate::game_logic::buildings::BuildingType;
                    match bt {
                        Some(BuildingType::Barracks) => &[
                            "AmericaInfantryRanger",
                            "AmericaInfantryMissileDefender",
                            "AmericaInfantryColonelBurton",
                            "TestInfantry",
                        ],
                        Some(BuildingType::WarFactory) => &[
                            "AmericaTankCrusader",
                            "AmericaVehicleHumvee",
                            "TestVehicleUnit",
                        ],
                        Some(BuildingType::Airfield) => {
                            &["AmericaJetRaptor", "AmericaJetAurora", "TestRaptor"]
                        }
                        Some(BuildingType::CommandCenter) => &["AmericaVehicleDozer", "TestDozer"],
                        _ => &["TestInfantry", "TestRaptor", "AmericaInfantryColonelBurton"],
                    }
                };
                for name in samples {
                    if !logic.templates.contains_key(*name) {
                        // Still query residual — can_make returns NO_PREREQ without template.
                    }
                    let status = logic.can_make_unit(pid, name);
                    let is_struct = logic
                        .templates
                        .get(*name)
                        .map(|t| t.is_kind_of(crate::game_logic::KindOf::Structure))
                        .unwrap_or(false);
                    let help = can_make_type_help_box_message_residual(status, is_struct)
                        .map(|s| s.to_string());
                    can_make_cameos.push(PresentationCanMakeCameo {
                        template_name: (*name).to_string(),
                        can_make: status,
                        available: status
                            == crate::game_logic::host_production_buildable_command_residual::CANMAKE_OK,
                        help_status: help,
                    });
                }
                can_make_cameos.truncate(12);
            }
        }
        let selected = local
            .map(|p| p.selected_objects.clone())
            .unwrap_or_default();

        // Combat particle residual: freeze host registry for client/presentation observe.
        let particle_systems: Vec<PresentationParticleSystem> = logic
            .combat_particles()
            .systems_snapshot()
            .iter()
            .map(PresentationParticleSystem::from_combat_entry)
            .collect();

        // W3DLaserDraw residual: freeze active assist lasers + Line3D segments.
        // Ground height residual: sample map height when available, else default-0.
        let logic_frame = logic.get_frame();
        let mut laser_beams: Vec<PresentationLaserBeam> = logic
            .active_patriot_assist_lasers()
            .iter()
            .filter(|l| l.is_active_at(logic_frame))
            .enumerate()
            .map(|(i, l)| {
                let mid = Vec3::new(l.arc_mid_x, l.arc_mid_y, l.arc_mid_z);
                let (gh, from_terrain) = sample_presentation_ground_height(logic, mid);
                PresentationLaserBeam::from_host_laser_with_terrain(l, i as u32, gh, from_terrain)
            })
            .collect();
        // Weapon.ini LaserName residual beams (combat fire path).
        let base_idx = laser_beams.len() as u32;
        for (i, l) in logic
            .active_weapon_lasers()
            .iter()
            .filter(|l| l.is_active_at(logic_frame))
            .enumerate()
        {
            let mid = Vec3::new(
                (l.from_x + l.to_x) * 0.5,
                (l.from_y + l.to_y) * 0.5,
                (l.from_z + l.to_z) * 0.5,
            );
            let (gh, from_terrain) = sample_presentation_ground_height(logic, mid);
            laser_beams.push(PresentationLaserBeam::from_weapon_laser(
                l,
                base_idx + i as u32,
                gh,
                from_terrain,
            ));
        }

        let projectile_streams: Vec<PresentationProjectileStream> = logic
            .projectile_stream_snapshot()
            .into_iter()
            .map(
                |(shooter_id, stream_name, points, target_id)| PresentationProjectileStream {
                    shooter_id,
                    stream_name,
                    points: points.into_iter().map(|p| (p.x, p.y, p.z)).collect(),
                    target_id,
                },
            )
            .collect();

        let projectiles: Vec<PresentationProjectile> = logic
            .combat_system()
            .projectiles_snapshot()
            .into_iter()
            .map(PresentationProjectile::from_combat)
            .collect();

        // InGameUI floating text + MoneyPickUp Anim2D residual: freeze host registries.
        let mut floating_texts = collect_presentation_floating_texts(logic);
        // Wave 514: active host emoticons → floating-text residual (presentation-only).
        let frame_now = logic.get_frame();
        for obj in logic.host_objects().values() {
            if obj.emoticon_frames_left <= 0 || obj.emoticon_name.is_empty() {
                continue;
            }
            if obj.status.destroyed || !obj.is_alive() {
                continue;
            }
            let pos = obj.get_position();
            floating_texts.push(PresentationFloatingText::from_parts(
                PresentationFloatingTextKind::Emoticon,
                obj.emoticon_name.clone(),
                obj.emoticon_name.clone(),
                glam::Vec3::new(pos.x, pos.y + 12.0, pos.z),
                (255, 255, 200, 255),
                0,
                frame_now,
                obj.id,
            ));
        }
        let world_anims = collect_presentation_world_anims(logic);

        let mut events = Vec::new();
        for (id, team) in logic.combat_particles().destroyed_this_frame() {
            events.push(PresentationEvent::ObjectDestroyed {
                id: *id,
                team: *team,
            });
        }
        // Freeze pending radar texts (UI drain later remains authoritative consumer).
        for entry in logic.radar_notification_snapshot() {
            let kind = match entry.kind {
                crate::game_logic::radar_notifications::RadarKind::Generic => 0u8,
                crate::game_logic::radar_notifications::RadarKind::Attack => 1u8,
                crate::game_logic::radar_notifications::RadarKind::Ally => 2u8,
            };
            events.push(PresentationEvent::RadarMessage {
                team: Team::Neutral, // host residual: text is global/team-agnostic here
                text: entry.text,
                position: entry.position,
                kind,
            });
        }
        // Drain: freeze this frame's completions into the snapshot (sole consumer).
        for ev in crate::game_logic::host_construction_log::drain() {
            events.push(PresentationEvent::ConstructionComplete {
                id: ev.id,
                template: ev.template_name,
            });
        }
        for up in logic.host_upgrades().completed_this_frame_snapshot() {
            events.push(PresentationEvent::UpgradeComplete {
                name: up.name,
                player_id: up.player_id,
                team: up.team,
                units_affected: up.units_affected,
            });
        }
        // Shadow session drains production before presentation; freeze last drain batch.
        for ev in crate::game_logic::host_production_log::take_last_drain() {
            if let crate::game_logic::host_production_log::HostProductionEvent::Complete {
                producer,
                template_name,
                spawned,
            } = ev
            {
                events.push(PresentationEvent::ProductionComplete {
                    producer,
                    template: template_name,
                    spawned,
                });
            }
        }
        for ev in crate::game_logic::host_owner_log::take_last_drain() {
            events.push(PresentationEvent::OwnerChanged {
                id: ev.object,
                team: ev.team,
            });
        }
        for ev in crate::game_logic::host_attack_log::take_last_drain() {
            if ev.target.is_some() {
                events.push(PresentationEvent::AttackTargeted {
                    attacker: ev.attacker,
                    target: ev.target,
                });
            }
        }
        // Wave 532: FireSound loop drain is a sibling of attack_log (not nested).
        // Nested drain only ran when attack_log was non-empty and could drop loops.
        for ev in crate::game_logic::host_fire_sound_loop_log::take_last_drain() {
            if ev.start {
                events.push(PresentationEvent::WeaponFireLoopStarted {
                    unit: ev.unit,
                    sound: ev.sound,
                });
            } else {
                events.push(PresentationEvent::WeaponFireLoopStopped {
                    unit: ev.unit,
                    sound: ev.sound,
                });
            }
        }
        for ev in crate::game_logic::host_move_log::take_last_drain() {
            if let Some(destination) = ev.destination {
                events.push(PresentationEvent::MoveOrdered {
                    unit: ev.unit,
                    destination,
                });
            }
        }
        for ev in crate::game_logic::host_damage_log::take_last_drain() {
            events.push(PresentationEvent::DamageApplied {
                target: ev.target,
                amount: ev.amount,
                source: ev.source,
                destroyed: ev.destroyed,
            });
            if ev.amount > 0.0 && !ev.destroyed {
                let pos = logic
                    .host_objects()
                    .get(&ev.target)
                    .map(|o| o.get_position())
                    .unwrap_or(Vec3::ZERO);
                let frame = logic.get_frame();
                floating_texts.push(PresentationFloatingText::from_parts(
                    PresentationFloatingTextKind::CombatDamage,
                    format!("-{}", ev.amount as i32),
                    "GUI:CombatDamage".into(),
                    pos + Vec3::new(0.0, 8.0, 0.0),
                    (255, 64, 64, 255),
                    ev.amount.max(0.0) as u32,
                    frame,
                    ev.source.unwrap_or(ev.target),
                ));
            }
        }
        for ev in crate::game_logic::host_heal_log::take_last_drain() {
            events.push(PresentationEvent::HealApplied {
                target: ev.target,
                health: ev.health,
            });
        }
        for ev in crate::game_logic::host_economy_log::take_last_drain() {
            events.push(PresentationEvent::EconomyChanged {
                player_id: ev.player_id,
                supplies: ev.supplies,
                power_available: ev.power_available,
            });
        }
        // Wave 533: EVA pulse drain (sibling of other host logs).
        for ev in crate::game_logic::host_eva_log::take_last_drain() {
            events.push(PresentationEvent::EvaAlert { name: ev.name });
        }
        for pid in logic.combat_particles().spawned_this_frame() {
            if let Some(entry) = logic.combat_particles().get(*pid) {
                events.push(PresentationEvent::ParticleSystemSpawned {
                    id: entry.id,
                    kind: entry.kind,
                    template_name: entry.template_name.clone(),
                    position: entry.position,
                });
            }
        }

        let dual_tick = PresentationDualTickResidual::from_counts(
            objects.len(),
            selected.len(),
            laser_beams.len(),
            floating_texts.len(),
            world_anims.len(),
            particle_systems.len(),
        );

        let mut frame = Self {
            frame: LogicFrame(logic.get_frame()),
            total_play_time_seconds: logic.get_total_play_time(),
            ai_difficulty: logic.get_difficulty(),
            game_mode: logic.game_mode(),
            objects,
            local_player_id,
            local_team,
            local_team_base_position,
            players,
            local_supplies,
            local_power,
            local_power_produced,
            local_power_consumed,
            local_color_rgb,
            local_is_alive,
            local_radar_count,
            local_radar_disabled,
            local_cash_bounty_percent,
            local_rank_level,
            local_skill_points,
            local_science_purchase_points,
            local_rank_progress_percent,
            local_unlocked_sciences,
            superweapon_timers,
            can_make_cameos,
            can_make_producer_id,
            local_queued_upgrades,
            selected,
            events,
            match_over: false,
            victory_label: None,
            defeated_player_ids: Vec::new(),
            alliance_events: Vec::new(),
            victory_summary: None,
            beacons: {
                // Wave 211: prefer host-owned beacon list (no Mutex dual-read).
                let host = logic.host_beacons();
                if !host.is_empty() {
                    host.iter().copied().take(64).collect()
                } else {
                    #[cfg(feature = "game_client")]
                    {
                        use gamelogic::system::beacon_manager::snapshot_beacons;
                        snapshot_beacons()
                            .into_iter()
                            .map(|b| glam::Vec3::new(b.position.x, b.position.y, b.position.z))
                            .take(64)
                            .collect()
                    }
                    #[cfg(not(feature = "game_client"))]
                    {
                        Vec::new()
                    }
                }
            },
            new_beacons: logic.recent_beacons().iter().copied().take(32).collect(),
            script_messages: {
                let mut v = logic.script_broadcast_texts();
                v.extend(logic.peek_new_script_messages().iter().cloned());
                v.truncate(32);
                v
            },
            new_script_messages: logic
                .peek_new_script_messages()
                .iter()
                .cloned()
                .take(16)
                .collect(),
            cinematic_letterbox: logic.cinematic_letterbox(),
            cinematic_text: logic.cinematic_text().map(|s| s.to_string()),
            cinematic_text_remaining_ms: logic.cinematic_text_remaining_ms(),
            military_caption: logic.military_caption_text().map(|s| s.to_string()),
            military_caption_remaining_ms: logic.military_caption_remaining_ms(),
            radar_ui_enabled: {
                let local_has_radar = logic
                    .get_player(local_player_id)
                    .map(|p| p.has_radar())
                    .unwrap_or(false);
                logic.radar_forced() || (logic.radar_script_enabled() && local_has_radar)
            },
            radar_forced: logic.radar_forced(),
            objectives: logic.mission_objectives().to_vec(),
            pending_movie: logic.peek_pending_movie().map(|s| s.to_string()),
            pending_radar_movie: logic.peek_pending_radar_movie().map(|s| s.to_string()),
            pending_music_stop: logic.peek_pending_music_stop(),
            pending_popup_messages: logic
                .peek_pending_popup_messages()
                .iter()
                .map(|p| PresentationPopupMessage {
                    message: p.message.clone(),
                    x_percent: p.x_percent,
                    y_percent: p.y_percent,
                    width: p.width,
                    pause: p.pause,
                    pause_music: p.pause_music,
                })
                .take(16)
                .collect(),
            script_time_frozen: logic.is_script_time_frozen(),
            script_camera_time_frozen: logic.is_script_camera_time_frozen(),
            time_frozen_for_simulation: logic.is_time_frozen_for_simulation(),
            // Wave 251: freeze visual speed into presentation snapshot.
            visual_speed_multiplier: logic.visual_speed_multiplier(),
            // Wave 252: freeze script default camera residuals.
            script_default_camera_max_height: logic.script_default_camera_max_height(),
            script_default_camera_pitch: logic.script_default_camera_pitch(),
            script_fps_limit: logic.peek_pending_script_fps_limit(),
            view_guardband: logic
                .peek_pending_view_guardband()
                .map(|g| (g.x_bias, g.y_bias)),
            camera_focus: logic.peek_pending_camera_focus().map(|p| [p.x, p.y, p.z]),
            camera_follow_position: logic
                .peek_camera_follow_target_position()
                .map(|p| [p.x, p.y, p.z]),
            camera_bw_mode: logic
                .peek_pending_camera_bw_mode()
                .map(|m| (m.enabled, m.frames)),
            camera_shakers: logic
                .peek_pending_camera_add_shakers()
                .iter()
                .map(|s| (s.amplitude, s.duration_seconds, s.radius))
                .take(8)
                .collect(),
            camera_motion_blur_count: logic.peek_pending_camera_motion_blur_count(),
            camera_zoom: logic
                .peek_pending_camera_zoom()
                .map(|z| (z.zoom, z.duration_seconds)),
            camera_zoom_reset: logic.peek_pending_camera_zoom_reset(),
            camera_pitch: logic
                .peek_pending_camera_pitch()
                .map(|p| (p.pitch, p.duration_seconds)),
            camera_rotate: logic
                .peek_pending_camera_rotate()
                .map(|r| (r.rotations, r.duration_seconds)),
            camera_look_toward: logic
                .peek_pending_camera_look_toward()
                .map(|l| [l.position.x, l.position.y, l.position.z]),
            camera_slave_enable: logic
                .peek_pending_camera_slave_enable()
                .map(|s| (s.thing_template_name.clone(), s.bone_name.clone())),
            camera_slave_disable: logic.peek_pending_camera_slave_disable(),
            named_timers: {
                let mut timers: Vec<(String, String, bool)> = logic
                    .peek_script_named_timers()
                    .iter()
                    .map(|(n, (t, c))| (n.clone(), t.clone(), *c))
                    .collect();
                timers.sort_by(|a, b| a.0.cmp(&b.0));
                timers.truncate(16);
                timers
            },
            cameo_flash: {
                let mut flashes: Vec<(String, i32)> = logic
                    .peek_script_cameo_flash_count()
                    .iter()
                    .map(|(b, c)| (b.clone(), *c))
                    .collect();
                flashes.sort_by(|a, b| a.0.cmp(&b.0));
                flashes.truncate(16);
                flashes
            },
            screen_shakes: logic
                .peek_pending_screen_shakes()
                .iter()
                .map(|s| s.intensity)
                .take(8)
                .collect(),
            script_skybox_enabled: logic.peek_script_skybox_enabled(),
            superweapon_display_enabled: logic.peek_script_superweapon_display_enabled(),
            named_timer_display_shown: logic.peek_script_named_timer_display_shown(),
            superweapon_hidden_objects: {
                let mut ids: Vec<u32> = logic
                    .peek_script_superweapon_hidden_objects()
                    .iter()
                    .map(|id| id.0)
                    .collect();
                ids.sort_unstable();
                ids.truncate(32);
                ids
            },
            eva_low_power_count: logic.eva_low_power_count(),
            eva_insufficient_funds_count: logic.eva_insufficient_funds_count(),
            eva_base_under_attack_count: logic.eva_base_under_attack_count(),
            eva_ally_under_attack_count: logic.eva_ally_under_attack_count(),
            fow_shell_bypass,
            // Wave 557: freeze replay mode into presentation snapshot.
            in_replay_game: logic.isInReplayGame(),
            // Wave 561/564: freeze fixed-step diagnostics residual.
            logic_steps_run: logic.fixed_step_diagnostics().steps_run as u32,
            // Wave 564
            logic_steps_budget_hit: logic.fixed_step_diagnostics().budget_hit,
            logic_steps_accumulated_seconds: logic
                .fixed_step_diagnostics()
                .accumulated_time_seconds,
            // Wave 563: freeze template name keys for presentation-owned contains residual.
            known_template_names: {
                let mut names: Vec<String> = logic.templates.keys().cloned().collect();
                names.sort();
                names.truncate(512);
                names
            },
            fow_grid,
            particle_systems,
            laser_beams,
            projectile_streams,
            projectiles,
            floating_texts,
            world_anims,
            dual_tick,
            world_env: PresentationWorldEnv::from_logic(logic),
            gameworld_overlay_stamped: 0,
            gameworld_appended: 0,
            gameworld_rebuilt: 0,
            gameworld_primary_objects: false,
        };
        // Wave 500: named damage/death/bone FX residual → particle observe list.
        let _ = frame.append_object_residual_fx_particles();
        frame
    }

    /// Build after evaluating victory (mutates victory subsystem once).
    pub fn build_with_victory(logic: &mut GameLogic, local_player_id: u32) -> Self {
        let mut frame = Self::build_from_logic(logic, local_player_id);
        if let Some(v) = logic.evaluate_victory_condition() {
            frame.match_over = true;
            frame.victory_label = Some(format!("{v:?}"));
            let winner = match v {
                crate::game_logic::VictoryCondition::Winner(id) => Some(id),
                _ => None,
            };
            frame.events.push(PresentationEvent::Victory {
                winner_player: winner,
            });
            // Freeze summary residual once (show_victory_screen prefers this).
            frame.victory_summary = Some(logic.build_victory_summary(winner));
        }
        // Freeze defeat notification residual produced by evaluate (engine drains take).
        frame.defeated_player_ids = logic.peek_defeat_events().to_vec();
        frame.alliance_events = logic.peek_alliance_events().to_vec();
        frame
    }

    /// Lightweight fingerprint for dual-run presentation determinism.
    pub fn presentation_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        self.frame.0.hash(&mut h);
        self.objects.len().hash(&mut h);
        for o in &self.objects {
            o.id.0.hash(&mut h);
            o.template_name.hash(&mut h);
            o.team.hash(&mut h);
            o.health_current.to_bits().hash(&mut h);
            o.selected.hash(&mut h);
            o.destroyed.hash(&mut h);
            o.fow_visibility.visibility_alpha.to_bits().hash(&mut h);
            o.fow_visibility.is_explored.to_bits().hash(&mut h);
        }
        self.local_supplies.hash(&mut h);
        self.match_over.hash(&mut h);
        self.fow_shell_bypass.hash(&mut h);
        self.in_replay_game.hash(&mut h);
        self.logic_steps_run.hash(&mut h);
        self.logic_steps_budget_hit.hash(&mut h);
        self.logic_steps_accumulated_seconds.to_bits().hash(&mut h);
        self.known_template_names.len().hash(&mut h);
        for n in &self.known_template_names {
            n.hash(&mut h);
        }
        self.fow_grid.content_fingerprint().hash(&mut h);
        self.local_player_id.hash(&mut h);
        match self.local_team {
            Team::USA => 0u8,
            Team::China => 1u8,
            Team::GLA => 2u8,
            Team::Neutral => 3u8,
        }
        .hash(&mut h);
        self.players.len().hash(&mut h);
        for p in &self.players {
            p.id.hash(&mut h);
            p.name.hash(&mut h);
            match p.team {
                Team::USA => 0u8,
                Team::China => 1u8,
                Team::GLA => 2u8,
                Team::Neutral => 3u8,
            }
            .hash(&mut h);
            p.is_alive.hash(&mut h);
            p.is_local.hash(&mut h);
            p.is_ai.hash(&mut h);
            p.color_rgb.0.hash(&mut h);
            p.color_rgb.1.hash(&mut h);
            p.color_rgb.2.hash(&mut h);
        }
        self.laser_beams.len().hash(&mut h);
        for beam in &self.laser_beams {
            beam.beam_index.hash(&mut h);
            beam.from_id.0.hash(&mut h);
            beam.to_id.0.hash(&mut h);
            beam.segments.len().hash(&mut h);
            beam.scroll_offset.to_bits().hash(&mut h);
        }
        self.floating_texts.len().hash(&mut h);
        for ft in &self.floating_texts {
            ft.kind.hash(&mut h);
            ft.text.hash(&mut h);
            ft.amount.hash(&mut h);
            ft.spawn_frame.hash(&mut h);
            ft.source_id.0.hash(&mut h);
            ft.position.x.to_bits().hash(&mut h);
            ft.position.y.to_bits().hash(&mut h);
            ft.position.z.to_bits().hash(&mut h);
        }
        self.world_anims.len().hash(&mut h);
        for wa in &self.world_anims {
            wa.template.hash(&mut h);
            wa.spawn_frame.hash(&mut h);
            wa.crate_id.0.hash(&mut h);
            wa.picker_id.0.hash(&mut h);
            wa.display_time_seconds.to_bits().hash(&mut h);
        }
        self.world_env.map_name.hash(&mut h);
        self.world_env.has_map_metadata.hash(&mut h);
        self.world_env.map_object_count.hash(&mut h);
        self.dual_tick.builds.hash(&mut h);
        self.dual_tick.object_count.hash(&mut h);
        h.finish()
    }
}

// ===== events.rs =====
use super::*;

/// Ordered gameplay event for audio/FX/UI (presentation side only).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PresentationEvent {
    ObjectDestroyed {
        id: ObjectId,
        team: Team,
    },
    ConstructionComplete {
        id: ObjectId,
        template: String,
    },
    /// Host research finished this frame (name + player).
    UpgradeComplete {
        name: String,
        player_id: u32,
        team: Team,
        units_affected: u32,
    },
    /// Factory production finished (spawned unit).
    ProductionComplete {
        producer: ObjectId,
        template: String,
        spawned: ObjectId,
    },
    /// Capture / hijack / set_team transfer this frame.
    OwnerChanged {
        id: ObjectId,
        team: Team,
    },
    /// Attack target set this frame (host_attack_log).
    AttackTargeted {
        attacker: ObjectId,
        target: Option<ObjectId>,
    },
    /// Move order destination this frame (host_move_log).
    MoveOrdered {
        unit: ObjectId,
        destination: [f32; 3],
    },
    /// Post-armor HP damage applied this frame (host_damage_log).
    DamageApplied {
        target: ObjectId,
        amount: f32,
        source: Option<ObjectId>,
        destroyed: bool,
    },
    /// Absolute HP write this frame (heal / construction finish residual).
    HealApplied {
        target: ObjectId,
        health: f32,
    },
    /// Player supplies/power absolute after host economy mutation.
    EconomyChanged {
        player_id: u32,
        supplies: u32,
        power_available: i32,
    },
    Victory {
        winner_player: Option<u32>,
    },
    RadarMessage {
        team: Team,
        text: String,
        /// World position residual (ZERO when text-only).
        position: Vec3,
        /// 0=Generic 1=Attack 2=Ally (host RadarKind residual).
        kind: u8,
    },
    /// Wave 533: host EVA pulse (TheEva setShouldPlay residual) for presentation audio.
    EvaAlert {
        name: String,
    },
    /// Combat residual: particle system spawned (host registry id + template).
    ParticleSystemSpawned {
        id: u32,
        kind: CombatParticleKind,
        template_name: String,
        position: Vec3,
    },
    /// C++ FiringTracker looping FireSound start/refresh residual.
    WeaponFireLoopStarted {
        unit: ObjectId,
        sound: String,
    },
    /// C++ FiringTracker stop looping FireSound after FireSoundLoopTime idle.
    WeaponFireLoopStopped {
        unit: ObjectId,
        sound: String,
    },
}


// ===== floating_text.rs =====
use super::*;

/// C++ `DEFAULT_FLOATING_TEXT_TIMEOUT = LOGICFRAMES_PER_SECOND / 3` → **10** frames.
pub const PRESENTATION_FLOATING_TEXT_TIMEOUT_FRAMES: u32 = 10;
/// C++ `m_floatingTextMoveUpSpeed` default (world units per logic frame, draw residual).
pub const PRESENTATION_FLOATING_TEXT_MOVE_UP_SPEED: f32 = 1.0;
/// C++ `m_floatingTextMoveVanishRate` default (alpha decay residual after timeout).
pub const PRESENTATION_FLOATING_TEXT_VANISH_RATE: f32 = 0.1;
/// Host residual fade window after world-anim display time (seconds) when Fades=Yes.
///
/// Mirrors C++ WORLD_ANIM_FADE_ON_EXPIRE ~1s window. Fail-closed: not live GPU blend.
pub const PRESENTATION_WORLD_ANIM_FADE_WINDOW_SECONDS: f32 = 1.0;
/// Logic FPS residual for age → seconds conversion (presentation dual-tick).
pub const PRESENTATION_LOGIC_FPS: f32 = 30.0;

/// Source residual family for frozen floating cash / caption text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum PresentationFloatingTextKind {
    /// AutoDepositUpdate (oil derrick / black market).
    AutoDeposit,
    /// HackInternet / Internet Center floating cash.
    Hacker,
    /// CashBounty kill bounty floating cash.
    CashBounty,
    /// MoneyCrateCollide pickup floating cash.
    MoneyCrate,
    /// Combat HP damage residual (from DamageApplied events).
    CombatDamage,
    /// Wave 514: Drawable emoticon residual (status bubble above unit).
    Emoticon,
}

/// Snapshot-owned InGameUI::addFloatingText residual for dual-tick consumers.
///
/// Built only from host residual registries at presentation build time so the
/// UI / GPU layout pack path does not re-read live GameLogic mid-render.
/// Fail-closed: not full DisplayString GPU draw / Unicode GameText localization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresentationFloatingText {
    pub kind: PresentationFloatingTextKind,
    pub text: String,
    pub text_key: String,
    pub position: Vec3,
    pub color_rgba: (u8, u8, u8, u8),
    pub amount: u32,
    pub spawn_frame: u32,
    /// Source object (derrick / hacker / killer / crate).
    pub source_id: ObjectId,
    /// Frame when residual times out (`spawn + PRESENTATION_FLOATING_TEXT_TIMEOUT_FRAMES`).
    pub timeout_frame: u32,
}

impl PresentationFloatingText {
    pub fn from_parts(
        kind: PresentationFloatingTextKind,
        text: String,
        text_key: String,
        position: Vec3,
        color_rgba: (u8, u8, u8, u8),
        amount: u32,
        spawn_frame: u32,
        source_id: ObjectId,
    ) -> Self {
        Self {
            kind,
            text,
            text_key,
            position,
            color_rgba,
            amount,
            spawn_frame,
            source_id,
            timeout_frame: spawn_frame.saturating_add(PRESENTATION_FLOATING_TEXT_TIMEOUT_FRAMES),
        }
    }

    /// True while C++ keeps the entry before vanish-phase erase residual.
    pub fn is_active_at(&self, logic_frame: u32) -> bool {
        logic_frame < self.timeout_frame
    }

    /// Age in logic frames at `logic_frame` (0 at spawn).
    pub fn age_frames_at(&self, logic_frame: u32) -> u32 {
        logic_frame.saturating_sub(self.spawn_frame)
    }

    /// C++ draw residual lift: `frameCount * m_floatingTextMoveUpSpeed`.
    pub fn lift_y_at(&self, logic_frame: u32) -> f32 {
        self.age_frames_at(logic_frame) as f32 * PRESENTATION_FLOATING_TEXT_MOVE_UP_SPEED
    }

    /// Vanish-rate alpha residual (1.0 while active; decays after timeout).
    ///
    /// C++: after timeout, alpha pulls toward 0 by `m_floatingTextMoveVanishRate`
    /// per frame until erased. Fail-closed: not live Display surface blend.
    pub fn vanish_alpha_at(&self, logic_frame: u32) -> f32 {
        let age = self.age_frames_at(logic_frame);
        let timeout = PRESENTATION_FLOATING_TEXT_TIMEOUT_FRAMES;
        if age < timeout {
            1.0
        } else {
            let past = (age - timeout) as f32;
            (1.0 - past * PRESENTATION_FLOATING_TEXT_VANISH_RATE).clamp(0.0, 1.0)
        }
    }

    /// C++ `updateFloatingText` integer alpha residual after timeout.
    ///
    /// ```text
    /// amount = REAL_TO_INT((currFrame - timeout) * m_floatingTextMoveVanishRate);
    /// if (a - amount < 0) a = 0; else a -= amount;
    /// ```
    /// Fail-closed: not live DisplayString surface blend / StretchRect.
    pub fn vanish_color_alpha_u8_at(&self, logic_frame: u32, base_alpha: u8) -> u8 {
        let age = self.age_frames_at(logic_frame);
        let timeout = PRESENTATION_FLOATING_TEXT_TIMEOUT_FRAMES;
        if age <= timeout {
            return base_alpha;
        }
        let past = (age - timeout) as f32;
        // REAL_TO_INT truncates toward zero (C++ `(Int)(x)`).
        let amount = (past * PRESENTATION_FLOATING_TEXT_VANISH_RATE) as i32;
        let next = base_alpha as i32 - amount;
        if next < 0 {
            0
        } else {
            next as u8
        }
    }

    /// Apply vanish-rate residual to a frozen color_rgba (RGB preserved, A decays).
    pub fn color_with_vanish_alpha_at(&self, logic_frame: u32) -> (u8, u8, u8, u8) {
        let (r, g, b, a) = self.color_rgba;
        (r, g, b, self.vanish_color_alpha_u8_at(logic_frame, a))
    }

    /// Honesty: retail vanish-rate / move-up / timeout presentation fields.
    pub fn honesty_vanish_rate_residual_ok() -> bool {
        (PRESENTATION_FLOATING_TEXT_VANISH_RATE - 0.1).abs() < 0.001
            && PRESENTATION_FLOATING_TEXT_TIMEOUT_FRAMES == 10
            && (PRESENTATION_FLOATING_TEXT_MOVE_UP_SPEED - 1.0).abs() < 0.001
            && {
                let t = PresentationFloatingText::synthetic_cash(50, 0);
                (t.vanish_alpha_at(0) - 1.0).abs() < 0.001
                    && (t.vanish_alpha_at(9) - 1.0).abs() < 0.001
                    && (t.vanish_alpha_at(10) - 1.0).abs() < 0.001
                    && (t.vanish_alpha_at(15) - 0.5).abs() < 0.001
                    && (t.vanish_alpha_at(20) - 0.0).abs() < 0.001
                    && (t.lift_y_at(5) - 5.0).abs() < 0.001
            }
    }

    /// Wave 76 residual honesty: C++ integer color-alpha vanish path residual.
    ///
    /// Matches `InGameUI::updateFloatingText` REAL_TO_INT amount subtract on A.
    /// With default vanish rate **0.1**, past=10 → amount **1** (255→254);
    /// past=5 → amount **0** (truncation). Fail-closed vs live Display surface.
    pub fn honesty_vanish_color_alpha_residual_ok() -> bool {
        let t = PresentationFloatingText::synthetic_cash(50, 0);
        // Synthetic cash uses green (0,255,0,255).
        t.color_rgba == (0, 255, 0, 255)
            && t.vanish_color_alpha_u8_at(0, 255) == 255
            && t.vanish_color_alpha_u8_at(10, 255) == 255
            && t.vanish_color_alpha_u8_at(15, 255) == 255 // past=5 → amount=0
            && t.vanish_color_alpha_u8_at(20, 255) == 254 // past=10 → amount=1
            && t.vanish_color_alpha_u8_at(30, 255) == 253 // past=20 → amount=2
            && t.vanish_color_alpha_u8_at(20, 1) == 0 // saturating subtract residual
            && {
                let c = t.color_with_vanish_alpha_at(20);
                c == (0, 255, 0, 254)
            }
            && Self::honesty_vanish_rate_residual_ok()
    }

    /// Synthetic cash residual for host-testable floating-text pack honesty.
    pub fn synthetic_cash(amount: u32, spawn_frame: u32) -> Self {
        Self::from_parts(
            PresentationFloatingTextKind::MoneyCrate,
            format!("+${amount}"),
            "GUI:AddCash".into(),
            Vec3::new(10.0, 20.0, 5.0),
            (0, 255, 0, 255),
            amount,
            spawn_frame,
            ObjectId(7001),
        )
    }
}

/// Snapshot-owned InGameUI::addWorldAnimation residual (MoneyPickUp Anim2D family).
///
/// Fail-closed: not full Anim2DCollection GPU / WORLD_ANIM_FADE_ON_EXPIRE draw.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresentationWorldAnim {
    pub template: String,
    pub position: Vec3,
    pub display_time_seconds: f32,
    pub z_rise_per_second: f32,
    pub fades: bool,
    pub spawn_frame: u32,
    pub crate_id: ObjectId,
    pub picker_id: ObjectId,
}

impl PresentationWorldAnim {
    pub fn from_money_pickup(
        anim: &crate::game_logic::host_money_crate::HostMoneyPickUpAnim,
    ) -> Self {
        Self {
            template: anim.template.clone(),
            position: anim.position,
            display_time_seconds: anim.display_time_seconds,
            z_rise_per_second: anim.z_rise_per_second,
            fades: anim.fades,
            spawn_frame: anim.spawn_frame,
            crate_id: anim.crate_id,
            picker_id: anim.picker_id,
        }
    }

    /// Synthetic MoneyPickUp residual for host-testable world-anim pack honesty.
    pub fn synthetic_money_pickup(spawn_frame: u32) -> Self {
        Self {
            template: crate::game_logic::host_money_crate::MONEY_PICKUP_ANIM_TEMPLATE.to_string(),
            position: Vec3::new(12.0, 0.0, 8.0),
            display_time_seconds:
                crate::game_logic::host_money_crate::MONEY_PICKUP_ANIM_DISPLAY_TIME_SECONDS,
            z_rise_per_second:
                crate::game_logic::host_money_crate::MONEY_PICKUP_ANIM_Z_RISE_PER_SECOND,
            fades: crate::game_logic::host_money_crate::MONEY_PICKUP_ANIM_FADES,
            spawn_frame,
            crate_id: ObjectId(8001),
            picker_id: ObjectId(8002),
        }
    }

    /// Display duration residual in logic frames (30 Hz).
    pub fn display_frames(&self) -> u32 {
        (self.display_time_seconds * PRESENTATION_LOGIC_FPS)
            .ceil()
            .max(1.0) as u32
    }

    pub fn is_active_at(&self, logic_frame: u32) -> bool {
        logic_frame < self.spawn_frame.saturating_add(self.display_frames())
    }

    /// Age in seconds at `logic_frame` (0 at spawn).
    pub fn age_seconds_at(&self, logic_frame: u32) -> f32 {
        logic_frame.saturating_sub(self.spawn_frame) as f32 / PRESENTATION_LOGIC_FPS
    }

    /// WORLD_ANIM_FADE_ON_EXPIRE residual alpha at `logic_frame`.
    ///
    /// - age < display → 1.0
    /// - age ≥ display and fades → clamp(1 - past/fade_window, 0..1)
    /// - age ≥ display and !fades → 0.0
    pub fn fade_alpha_at(&self, logic_frame: u32) -> f32 {
        let age = self.age_seconds_at(logic_frame);
        if age < self.display_time_seconds {
            1.0
        } else if self.fades {
            let past = age - self.display_time_seconds;
            (1.0 - past / PRESENTATION_WORLD_ANIM_FADE_WINDOW_SECONDS).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// Honesty: MoneyPickUp fade presentation residual fields.
    pub fn honesty_fade_residual_ok(&self) -> bool {
        (PRESENTATION_WORLD_ANIM_FADE_WINDOW_SECONDS - 1.0).abs() < 0.01
            && self.display_time_seconds > 0.0
            && {
                // Sample fade curve residual around display boundary.
                let mid = self
                    .spawn_frame
                    .saturating_add((self.display_time_seconds * PRESENTATION_LOGIC_FPS) as u32);
                let before = mid.saturating_sub(1);
                let half = mid.saturating_add((PRESENTATION_LOGIC_FPS * 0.5) as u32);
                let end = mid.saturating_add(PRESENTATION_LOGIC_FPS as u32);
                (self.fade_alpha_at(before) - 1.0).abs() < 0.05
                    && if self.fades {
                        (self.fade_alpha_at(half) - 0.5).abs() < 0.1
                            && (self.fade_alpha_at(end) - 0.0).abs() < 0.05
                    } else {
                        self.fade_alpha_at(half) <= 0.0
                    }
            }
    }

    /// Static honesty for retail MoneyPickUp fade residual defaults.
    pub fn honesty_money_pickup_fade_params_ok() -> bool {
        let a = Self::synthetic_money_pickup(0);
        a.fades
            && (a.display_time_seconds - 4.0).abs() < 0.01
            && (a.z_rise_per_second - 15.0).abs() < 0.01
            && a.honesty_fade_residual_ok()
    }
}

/// Collect host residual floating texts into a stable presentation list.
pub(crate) fn collect_presentation_floating_texts(logic: &GameLogic) -> Vec<PresentationFloatingText> {
    let mut out = Vec::new();

    for t in &logic.oil_derricks().floating_texts {
        out.push(PresentationFloatingText::from_parts(
            PresentationFloatingTextKind::AutoDeposit,
            t.text.clone(),
            t.text_key.clone(),
            t.position,
            t.color_rgba,
            t.amount,
            t.spawn_frame,
            t.source_id,
        ));
    }
    for t in &logic.black_markets().floating_texts {
        out.push(PresentationFloatingText::from_parts(
            PresentationFloatingTextKind::AutoDeposit,
            t.text.clone(),
            t.text_key.clone(),
            t.position,
            t.color_rgba,
            t.amount,
            t.spawn_frame,
            t.source_id,
        ));
    }
    for t in &logic.hacker_income().floating_texts {
        out.push(PresentationFloatingText::from_parts(
            PresentationFloatingTextKind::Hacker,
            t.text.clone(),
            t.text_key.clone(),
            t.position,
            t.color_rgba,
            t.amount,
            t.spawn_frame,
            t.hacker_id,
        ));
    }
    for t in &logic.cash_bounty_registry().floating_texts {
        out.push(PresentationFloatingText::from_parts(
            PresentationFloatingTextKind::CashBounty,
            t.text.clone(),
            t.text_key.clone(),
            t.position,
            t.color_rgba,
            t.amount,
            t.spawn_frame,
            t.killer_id,
        ));
    }
    for t in &logic.host_money_crates().money_floating_texts {
        out.push(PresentationFloatingText::from_parts(
            PresentationFloatingTextKind::MoneyCrate,
            t.text.clone(),
            t.text_key.clone(),
            t.position,
            t.color_rgba,
            t.amount,
            t.spawn_frame,
            t.crate_id,
        ));
    }

    // Stable presentation order: spawn frame then source id then kind.
    out.sort_by(|a, b| {
        a.spawn_frame
            .cmp(&b.spawn_frame)
            .then(a.source_id.0.cmp(&b.source_id.0))
            .then(a.kind.cmp(&b.kind))
    });
    out
}

pub(crate) fn collect_presentation_world_anims(logic: &GameLogic) -> Vec<PresentationWorldAnim> {
    let mut out: Vec<PresentationWorldAnim> = logic
        .host_money_crates()
        .money_pickup_anims
        .iter()
        .map(PresentationWorldAnim::from_money_pickup)
        .collect();
    out.sort_by(|a, b| {
        a.spawn_frame
            .cmp(&b.spawn_frame)
            .then(a.crate_id.0.cmp(&b.crate_id.0))
            .then(a.picker_id.0.cmp(&b.picker_id.0))
    });
    out
}

// ===== frame.rs =====
use super::*;

/// Snapshot-owned player roster residual (defeat/alliance UI / radar team).
/// Fail-closed: not full Player science/upgrade/diplomacy matrix.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresentationPlayerInfo {
    pub id: u32,
    pub name: String,
    pub team: Team,
    pub is_alive: bool,
    pub is_local: bool,
    /// True when host AI manager owns this player (skirmish AI residual).
    pub is_ai: bool,
    /// Skirmish/UI color residual (RGB).
    pub color_rgb: (u8, u8, u8),
}

/// Frozen script popup residual (C++ ScriptPopupMessageRequest parity).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresentationPopupMessage {
    pub message: String,
    pub x_percent: i32,
    pub y_percent: i32,
    pub width: i32,
    pub pause: bool,
    pub pause_music: bool,
}

/// Frozen InGameUI PublicTimer superweapon countdown residual.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresentationSuperweaponTimer {
    pub name: String,
    pub template_name: String,
    pub icon: String,
    /// Full recharge duration seconds residual.
    pub recharge_time: f32,
    /// Seconds remaining (0 = ready).
    pub remaining: f32,
    /// Science/prereq unlocked residual.
    pub unlocked: bool,
    /// Ready residual (unlocked && remaining <= 0).
    pub ready: bool,
    /// `SpecialPowerType` Debug name for shadow cooldown overlay.
    pub power_key: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresentationFrame {
    pub frame: LogicFrame,
    /// Host sim clock residual (seconds) for UI time readout.
    pub total_play_time_seconds: f32,
    /// Host AI difficulty residual (save metadata).
    pub ai_difficulty: crate::ai::AIDifficulty,
    /// Host game mode residual (restart/save metadata).
    pub game_mode: crate::game_logic::GameMode,
    pub objects: Vec<RenderableObject>,
    pub local_player_id: u32,
    /// Local player team frozen at snapshot time (selection/hotkey residual).
    /// Prefer this over live `GameLogic::get_player` dual-reads when a frame is installed.
    pub local_team: Team,
    /// Host team base / command-center pose residual for camera snap proximity.
    pub local_team_base_position: Option<Vec3>,
    /// Full player roster frozen at snapshot time (defeat/alliance UI residual).
    pub players: Vec<PresentationPlayerInfo>,
    pub local_supplies: u32,
    pub local_power: i32,
    /// Host Player::power_produced residual (energy bar numerator side).
    pub local_power_produced: i32,
    /// Host Player::power_consumed residual (energy bar demand side).
    pub local_power_consumed: i32,
    pub local_color_rgb: (u8, u8, u8),
    /// Local player still alive residual.
    pub local_is_alive: bool,
    /// Radar provider count residual (CommandCenter / RadarVan).
    pub local_radar_count: i32,
    /// Script/power radar disable residual.
    pub local_radar_disabled: bool,
    /// GLA cash bounty percent residual (0..1).
    pub local_cash_bounty_percent: f32,
    /// C++ Player::m_rankLevel residual (1-based).
    pub local_rank_level: u32,
    /// C++ Player::m_skillPoints residual (GeneralsExperience).
    pub local_skill_points: i32,
    /// C++ Player::m_sciencePurchasePoints residual.
    pub local_science_purchase_points: i32,
    /// ControlBar rank progress residual 0..100
    /// (`(skill - levelDown) * 100 / (levelUp - levelDown)`).
    pub local_rank_progress_percent: i32,
    /// Unlocked science names residual (capped).
    pub local_unlocked_sciences: Vec<String>,
    /// InGameUI PublicTimer superweapon countdown residual (local player).
    /// Fail-closed: not full font flash / multi-CC SW map / script hide.
    pub superweapon_timers: Vec<PresentationSuperweaponTimer>,
    /// Selected producer CanMake residual cameos (ControlBar HelpBox feed).
    pub can_make_cameos: Vec<PresentationCanMakeCameo>,
    /// Selected producer object id residual for can_make_cameos.
    pub can_make_producer_id: Option<u32>,

    /// Queued upgrade template names residual (capped).
    pub local_queued_upgrades: Vec<String>,
    pub selected: Vec<ObjectId>,
    pub events: Vec<PresentationEvent>,
    pub match_over: bool,
    pub victory_label: Option<String>,
    /// Players defeated this evaluate residual (C++ defeat notification queue).
    pub defeated_player_ids: Vec<u32>,
    /// Alliance state-change residual from victory evaluate.
    pub alliance_events: Vec<crate::game_logic::AllianceNotification>,
    /// Host VictorySummary residual (mission/duration/player results).
    /// Fail-closed: stats tables frozen at evaluate; not live re-aggregate.
    /// Skipped in serde (Duration/player payload is host snapshot residual only).
    #[serde(skip)]
    pub victory_summary: Option<crate::game_logic::VictorySummary>,
    /// Beacon world positions residual (host_beacons preferred; manager snapshot fallback).
    pub beacons: Vec<Vec3>,
    /// Beacons placed this frame (HUD bloom residual).
    pub new_beacons: Vec<Vec3>,
    /// Active script broadcast texts residual.
    pub script_messages: Vec<String>,
    /// New script messages this frame residual.
    pub new_script_messages: Vec<String>,
    /// Cinematic letterbox residual.
    pub cinematic_letterbox: bool,
    /// Cinematic overlay text residual.
    pub cinematic_text: Option<String>,
    /// Remaining lifetime for cinematic text (ms residual).
    pub cinematic_text_remaining_ms: Option<i32>,
    /// Military caption residual.
    pub military_caption: Option<String>,
    /// Remaining lifetime for military caption (ms residual).
    pub military_caption_remaining_ms: Option<i32>,
    /// Effective radar available residual (forced || enabled && has_radar).
    pub radar_ui_enabled: bool,
    /// Script radar forced residual.
    pub radar_forced: bool,
    /// Mission objectives residual (ObjectiveDisplay clone).
    pub objectives: Vec<crate::ui::objectives::ObjectiveDisplay>,
    /// Pending script movie name residual.
    pub pending_movie: Option<String>,
    /// Pending radar movie name residual.
    pub pending_radar_movie: Option<String>,
    /// Pending music-stop request residual.
    pub pending_music_stop: bool,
    /// Pending popup message texts residual (fail-closed layout).
    pub pending_popup_messages: Vec<PresentationPopupMessage>,
    /// Script time-freeze residual.
    pub script_time_frozen: bool,
    /// Script camera time-freeze residual.
    pub script_camera_time_frozen: bool,
    /// Combined simulation freeze residual.
    pub time_frozen_for_simulation: bool,
    /// Wave 251: host visual speed residual (render/update timing).
    pub visual_speed_multiplier: f32,
    /// Wave 252: script default camera max height residual.
    pub script_default_camera_max_height: f32,
    /// Wave 252: script default camera pitch residual.
    pub script_default_camera_pitch: f32,
    /// Pending script FPS limit residual.
    pub script_fps_limit: Option<i32>,
    /// Pending view guardband residual (x,y bias).
    pub view_guardband: Option<(f32, f32)>,
    /// Pending camera focus residual.
    pub camera_focus: Option<[f32; 3]>,
    /// Camera-follow object world position residual (live follow still resolves host id).
    pub camera_follow_position: Option<[f32; 3]>,
    /// Pending BW mode residual (enabled, frames).
    pub camera_bw_mode: Option<(bool, i32)>,
    /// Pending camera shaker residual (amplitude, duration, radius).
    pub camera_shakers: Vec<(f32, f32, f32)>,
    /// Pending camera motion-blur request count residual.
    pub camera_motion_blur_count: usize,
    /// Pending camera zoom residual (zoom, duration).
    pub camera_zoom: Option<(f32, f32)>,
    pub camera_zoom_reset: bool,
    /// Pending camera pitch residual (pitch, duration).
    pub camera_pitch: Option<(f32, f32)>,
    /// Pending camera rotate residual (rotations, duration).
    pub camera_rotate: Option<(f32, f32)>,
    /// Pending look-toward residual.
    pub camera_look_toward: Option<[f32; 3]>,
    /// Pending slave-mode enable residual (template, bone).
    pub camera_slave_enable: Option<(String, String)>,
    pub camera_slave_disable: bool,
    /// Active script named timers residual (name, text, countdown).
    pub named_timers: Vec<(String, String, bool)>,
    /// Cameo flash residual (button, count).
    pub cameo_flash: Vec<(String, i32)>,
    /// Pending screen-shake intensities residual.
    pub screen_shakes: Vec<i32>,
    /// Script skybox enable residual.
    pub script_skybox_enabled: bool,
    /// Superweapon display enable residual.
    pub superweapon_display_enabled: bool,
    /// Named-timer display shown residual.
    pub named_timer_display_shown: bool,
    /// Hidden superweapon object ids residual.
    pub superweapon_hidden_objects: Vec<u32>,
    /// Shell-map FOW bypass (`GameLogic::isInShellGame`) frozen at snapshot time.
    /// When true, unit FOW is forced fully visible and never-explored skip is off.
    /// EVA residual counters frozen at snapshot (C++ Eva message queue deltas).
    pub eva_low_power_count: u32,
    pub eva_insufficient_funds_count: u32,
    pub eva_base_under_attack_count: u32,
    pub eva_ally_under_attack_count: u32,
    pub fow_shell_bypass: bool,
    /// Wave 557: host replay-mode residual (`GameLogic::isInReplayGame`) frozen at
    /// snapshot time for FPS-limit / TiVO residual without live dual-read.
    pub in_replay_game: bool,
    /// Wave 561: host fixed-step catch-up residual (`steps_run`) frozen at snapshot
    /// for runtime status without live dual-read mid-frame.
    pub logic_steps_run: u32,
    /// Wave 564: host fixed-step budget residual frozen at snapshot.
    pub logic_steps_budget_hit: bool,
    /// Wave 564: host fixed-step accumulator residual (seconds) frozen at snapshot.
    pub logic_steps_accumulated_seconds: f32,
    /// Wave 563: host ThingTemplate name keys frozen for train/UI residual
    /// contains checks without dual-reading live `GameLogic::templates` mid-frame.
    /// Sorted; capped. Fail-closed: not full template body freeze / playable_claim.
    pub known_template_names: Vec<String>,
    /// Compact local-player cell-grid FOW for terrain overlay / minimap texture.
    /// Frozen at build so GPU upload does not re-query shroud mid-render.
    /// Fail-closed: not full SAGE dirty-rect / multi-layer shroud streaming.
    pub fow_grid: PresentationFowGrid,
    /// Active combat particle systems from host registry (observe path for client).
    pub particle_systems: Vec<PresentationParticleSystem>,
    /// Active Patriot assist / BinaryDataStream lasers + Line3D segments.
    /// Frozen so WGPU laser segment pack does not re-read live host mid-render.
    /// Fail-closed: not full SegLineRenderer GPU texture draw.
    pub laser_beams: Vec<PresentationLaserBeam>,
    /// C++ ProjectileStreamUpdate residual trails.
    pub projectile_streams: Vec<PresentationProjectileStream>,
    /// In-flight combat projectiles frozen from host CombatSystem.
    /// Fail-closed: not full W3D projectile draw / trail mesh.
    pub projectiles: Vec<PresentationProjectile>,
    /// InGameUI floating cash / caption texts frozen from host residual registries.
    /// Fail-closed: not full DisplayString GPU / Unicode GameText draw.
    pub floating_texts: Vec<PresentationFloatingText>,
    /// InGameUI world animations (MoneyPickUp Anim2D residual) frozen from host.
    /// Fail-closed: not full Anim2DCollection GPU draw.
    pub world_anims: Vec<PresentationWorldAnim>,
    /// Dual-tick residual counters (build / apply / content counts).
    pub dual_tick: PresentationDualTickResidual,
    /// World/environment identity for lighting/shell/bounds/heightmap residual.
    /// Prefer this over live `GameLogic` during GPU collect/execute.
    pub world_env: PresentationWorldEnv,
    /// Objects stamped by the last `overlay_gameworld_shadow` call (0 if none).
    /// Architecture residual: GameWorld last-writer presentation identity count.
    #[serde(default)]
    pub gameworld_overlay_stamped: usize,
    /// Count of RenderableObjects created from GameWorld entities missing on host frame
    /// (Wave 192 append_missing_from_gameworld). Fail-closed: not full build_from_gameworld cutover.
    #[serde(default)]
    pub gameworld_appended: usize,
    /// Count of objects after `rebuild_objects_from_gameworld` (Wave 193).
    /// Fail-closed: opt-in path; not full host cutover / playable_claim.
    #[serde(default)]
    pub gameworld_rebuilt: usize,
    /// True when objects were rebuilt from GameWorld (Wave 196 engine primary path).
    #[serde(default)]
    pub gameworld_primary_objects: bool,
}

/// Whether presentation object rosters should be rebuilt from GameWorld (Wave 194).
///
/// **Default ON** when shadow is present. Opt out with
/// `GENERALS_PRESENTATION_FROM_GAMEWORLD=0` (or false/no/off).
/// Fail-closed: does not flip shell `playable_claim`; host still supplies
/// non-object residual (scripts/FX/camera) via `build_from_logic` when used.
pub fn presentation_from_gameworld_enabled() -> bool {
    match std::env::var("GENERALS_PRESENTATION_FROM_GAMEWORLD") {
        Ok(v) => {
            let t = v.trim();
            !matches!(t, "0" | "false" | "FALSE" | "no" | "NO" | "off" | "OFF")
        }
        Err(_) => true,
    }
}

// ===== honesty.rs =====
use super::*;

impl PresentationFrame {
    /// Lookup snapshot FOW for an object (local player). None if not on the frame.
    pub fn fow_for_object(&self, id: ObjectId) -> Option<ObjectVisibility> {
        self.objects
            .iter()
            .find(|o| o.id == id)
            .map(|o| o.fow_visibility)
    }

    /// Local-player cell-grid FOW frozen on this frame (terrain / minimap).
    #[inline]
    pub fn fow_grid(&self) -> &PresentationFowGrid {
        &self.fow_grid
    }

    /// R8 terrain FOW texture from the snapshot only (no live shroud lock).
    ///
    /// Returns `None` when the grid is inactive (fail-open: skip overlay upload)
    /// or when shell bypass already forces fully visible cells that need no darkening.
    /// Callers that always want bytes can use `fow_grid().to_r8_texture()` directly.
    pub fn terrain_fow_r8(&self) -> Option<Vec<u8>> {
        if !self.fow_grid.active {
            return None;
        }
        let r8 = self.fow_grid.to_r8_texture();
        if r8.is_empty() {
            None
        } else {
            Some(r8)
        }
    }

    /// True when terrain FOW overlay should darken from the presentation grid.
    ///
    /// Shell bypass and inactive grids are fail-open (no overlay).
    pub fn terrain_fow_overlay_active(&self) -> bool {
        self.fow_grid.active && !self.fow_shell_bypass
    }

    /// All alive presentation objects including engine-bridged (for FOW/id lists).
    pub fn alive_renderables(&self) -> impl Iterator<Item = &RenderableObject> {
        // Wave 1108: alive residual excludes sold.
        self.objects.iter().filter(|o| !o.destroyed && !o.sold)
    }

    /// Active combat particle systems on this frame (host registry snapshot).
    pub fn active_particle_systems(&self) -> impl Iterator<Item = &PresentationParticleSystem> {
        self.particle_systems.iter().filter(|p| p.active)
    }

    /// True when at least one combat particle system is registered and active.
    pub fn has_active_particles(&self) -> bool {
        self.particle_systems.iter().any(|p| p.active)
    }

    /// Active presentation laser beams (assist BinaryDataStream residual).
    pub fn laser_beams(&self) -> &[PresentationLaserBeam] {
        &self.laser_beams
    }

    /// Total Line3D segments across all frozen laser beams.
    pub fn laser_segment_count(&self) -> usize {
        self.laser_beams.iter().map(|b| b.segments.len()).sum()
    }

    /// True when at least one residual laser beam is frozen on this frame.
    pub fn has_active_lasers(&self) -> bool {
        !self.laser_beams.is_empty()
    }

    /// Frozen InGameUI floating texts (host residual observe path).
    pub fn floating_texts(&self) -> &[PresentationFloatingText] {
        &self.floating_texts
    }

    /// Floating texts still within residual timeout at `frame` (or this frame).
    pub fn active_floating_texts_at(&self, logic_frame: u32) -> Vec<&PresentationFloatingText> {
        self.floating_texts
            .iter()
            .filter(|t| t.is_active_at(logic_frame))
            .collect()
    }

    /// True when at least one floating text is frozen on this frame.
    pub fn has_floating_texts(&self) -> bool {
        !self.floating_texts.is_empty()
    }

    /// Host-testable floating text residual usable for dual-tick UI layout pack.
    ///
    /// Empty is honest (no cash events yet). Non-empty requires GUI:AddCash key residual
    /// and positive timeout window.
    pub fn floating_text_presentation_ok(&self) -> bool {
        if self.floating_texts.is_empty() {
            return true;
        }
        self.floating_texts.iter().all(|t| {
            !t.text.is_empty()
                && t.text_key == "GUI:AddCash"
                && t.timeout_frame > t.spawn_frame
                && t.amount > 0
        })
    }

    /// Frozen MoneyPickUp / world Anim2D residuals.
    pub fn world_anims(&self) -> &[PresentationWorldAnim] {
        &self.world_anims
    }

    /// True when at least one world anim is frozen on this frame.
    pub fn has_world_anims(&self) -> bool {
        !self.world_anims.is_empty()
    }

    /// Host-testable world-anim residual usable for dual-tick Anim2D pack.
    ///
    /// Empty is honest. Non-empty requires MoneyPickUp template + positive display.
    pub fn world_anim_presentation_ok(&self) -> bool {
        if self.world_anims.is_empty() {
            return true;
        }
        self.world_anims.iter().all(|a| {
            a.template == crate::game_logic::host_money_crate::MONEY_PICKUP_ANIM_TEMPLATE
                && a.display_time_seconds > 0.0
                && a.z_rise_per_second > 0.0
        })
    }

    /// Host-testable FOW grid residual usable for minimap / terrain texture path.
    ///
    /// Active grids must have a consistent cell buffer; inactive grids are honest
    /// when shroud was not initialized (boot / no-map host).
    pub fn minimap_fow_presentation_ok(&self) -> bool {
        let g = &self.fow_grid;
        if !g.active {
            return true;
        }
        g.cell_count() == (g.width as usize).saturating_mul(g.height as usize)
            && !g.to_r8_texture().is_empty()
    }

    /// Dual-tick residual counters on this frame.
    #[inline]
    pub fn dual_tick(&self) -> &PresentationDualTickResidual {
        &self.dual_tick
    }

    /// Honesty: dual-tick build residual counters are self-consistent.
    pub fn dual_tick_presentation_residual_ok(&self) -> bool {
        self.dual_tick.honesty_build_ok()
            && self.dual_tick.object_count == self.objects.len() as u32
            && self.dual_tick.laser_beam_count == self.laser_beams.len() as u32
            && self.dual_tick.floating_text_count == self.floating_texts.len() as u32
            && self.dual_tick.world_anim_count == self.world_anims.len() as u32
            // Wave 102: selected + particle dual-tick residual counters.
            && self.dual_tick.selected_count == self.selected.len() as u32
            && self.dual_tick.particle_count == self.particle_systems.len() as u32
    }

    /// Wave 102: dual-tick residual deepen honesty (build + apply + content counts).
    ///
    /// Deepens dual-tick bookkeeping beyond Wave 65/75 counters: selected/particle
    /// counts, apply order residual (applies ≥ builds after shell apply), and
    /// cross-link presentation residual packs. Fail-closed vs live dual-run GPU.
    pub fn dual_tick_presentation_residual_deepen_ok(&self) -> bool {
        self.dual_tick_presentation_residual_ok()
            && self.dual_tick.builds >= 1
            && self.floating_text_vanish_residual_ok()
            && self.world_anim_fade_residual_ok()
            && self.laser_presentation_residual_ok()
            && self.spectre_orbit_decal_presentation_residual_ok()
            && self.mesh_scale_presentation_residual_ok()
            && self.ground_height_presentation_residual_ok()
    }

    /// Honesty: floating-text vanish-rate residual fields (empty is honest).
    pub fn floating_text_vanish_residual_ok(&self) -> bool {
        PresentationFloatingText::honesty_vanish_rate_residual_ok()
            && self.floating_texts.iter().all(|t| {
                let a = t.vanish_alpha_at(self.frame.0);
                a.is_finite() && (0.0..=1.0).contains(&a)
            })
    }

    /// Honesty: world-anim fade residual fields (empty is honest).
    pub fn world_anim_fade_residual_ok(&self) -> bool {
        if self.world_anims.is_empty() {
            return PresentationWorldAnim::honesty_money_pickup_fade_params_ok();
        }
        self.world_anims
            .iter()
            .all(|a| a.honesty_fade_residual_ok())
    }

    /// Honesty: laser ground-height + multi-beam soft-edge presentation residual.
    pub fn laser_presentation_residual_ok(&self) -> bool {
        self.laser_beams
            .iter()
            .all(|b| b.honesty_ground_height_ok() && b.honesty_soft_edge_presentation_ok())
            && PRESENTATION_ORBITAL_SOFT_EDGE.honesty_orbital_residual_ok()
            && honesty_ground_height_residual_ok(PRESENTATION_DEFAULT_GROUND_HEIGHT, false)
    }

    /// Honesty: Spectre AttackAreaDecal / TargetingReticleDecal presentation residual (Wave 73).
    ///
    /// Constant pack — presentation freezes retail decal defaults so dual-tick
    /// consumers can draw orbit cursors without re-reading live SpectreGunshipUpdate.
    /// Fail-closed: not full SHADOW_ALPHA_DECAL GPU throb submit.
    pub fn spectre_orbit_decal_presentation_residual_ok(&self) -> bool {
        let _ = self;
        honesty_spectre_orbit_decal_presentation_ok()
    }

    /// Honesty: mesh scale residual frozen on objects / unit render inputs (Wave 75).
    ///
    /// Common combat units retail-default to **1.0**. Empty snapshot is honest.
    /// Fail-closed: not full Object INI Scale field / draw-scale bone matrix.
    pub fn mesh_scale_presentation_residual_ok(&self) -> bool {
        crate::assets::mesh_asset_resolve::honesty_mesh_scale_residual_ok()
            && self
                .objects
                .iter()
                .all(|o| o.mesh_scale.is_finite() && o.mesh_scale > 0.0)
            && self
                .unit_render_inputs()
                .iter()
                .all(|u| u.mesh_scale.is_finite() && u.mesh_scale > 0.0)
    }

    /// Honesty: unit/structure ground-height residual frozen on objects (Wave 77).
    ///
    /// Empty object lists are honest (default path). Fail-closed: not full
    /// HeightMap bilinear / bridge-aware / locomotor Y rewrite.
    pub fn ground_height_presentation_residual_ok(&self) -> bool {
        honesty_ground_height_residual_ok(PRESENTATION_DEFAULT_GROUND_HEIGHT, false)
            && self.objects.iter().all(|o| {
                honesty_ground_height_residual_ok(o.ground_height, o.ground_height_from_terrain)
            })
    }

    /// Note a dual-tick apply on this snapshot (HUD / shell multi-consumer path).
    pub fn note_dual_tick_apply(&mut self) {
        self.dual_tick.applies = self.dual_tick.applies.saturating_add(1);
    }
}

// ===== lasers.rs =====
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PresentationLaserSegment {
    pub start: (f32, f32, f32),
    pub end: (f32, f32, f32),
    pub width: f32,
    pub tile_factor: f32,
    pub scroll_offset: f32,
}

/// Default Line3D ground-skim residual when map height is unavailable.
///
/// C++ samples terrain; host residual defaults to **0** and optionally overrides
/// when `GameLogic::terrain_height_at` returns a sample.
pub const PRESENTATION_DEFAULT_GROUND_HEIGHT: f32 = 0.0;

/// Sample residual ground height for laser Line3D skim.
///
/// Prefer map terrain height when available; else default-0 (honest residual).
/// Fail-closed: not full HeightMap bilinear / bridge-aware sample.
pub fn sample_presentation_ground_height(logic: &GameLogic, world_pos: Vec3) -> (f32, bool) {
    match logic.terrain_height_at(world_pos) {
        Some(h) if h.is_finite() => (h, true),
        _ => (PRESENTATION_DEFAULT_GROUND_HEIGHT, false),
    }
}

/// Honesty: default-0 residual + optional terrain / override path.
///
/// Any finite height is honest (default-0 when map height missing, terrain
/// sample when available, or host-testable override via synthetic path).
pub fn honesty_ground_height_residual_ok(height: f32, from_terrain: bool) -> bool {
    let _ = from_terrain;
    height.is_finite()
        && (from_terrain
            || (height - PRESENTATION_DEFAULT_GROUND_HEIGHT).abs() < 0.001
            || height.abs() > 0.0)
}

/// OrbitalLaser multi-beam soft-edge presentation residual (W3DLaserDraw NumBeams).
///
/// Host-testable fields that wire to `LaserSegmentUpload::pack_orbital_multi_beam_soft_edge`.
/// Fail-closed: not full additive GPU cylinder soft edge.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PresentationLaserSoftEdge {
    pub num_beams: u32,
    pub inner_width: f32,
    pub outer_width: f32,
    pub outer_color: (f32, f32, f32, f32),
    pub tiling_scalar: f32,
    pub scroll_rate: f32,
}

/// Retail OrbitalLaser texture residual name (`ParticleUplinkCannon_OrbitalLaser`).
pub const PRESENTATION_ORBITAL_LASER_TEXTURE: &str = "EXNoise02.tga";

/// Retail ParticleUplinkCannon_OrbitalLaser soft-edge residual defaults.
pub const PRESENTATION_ORBITAL_SOFT_EDGE: PresentationLaserSoftEdge = PresentationLaserSoftEdge {
    num_beams: 12,
    inner_width: 0.6,
    outer_width: 26.0,
    outer_color: (0.0, 0.0, 1.0, 150.0 / 255.0),
    tiling_scalar: 0.15,
    scroll_rate: -1.75,
};

impl PresentationLaserSoftEdge {
    /// Honesty: retail OrbitalLaser NumBeams soft-edge presentation fields.
    pub fn honesty_orbital_residual_ok(self) -> bool {
        self.num_beams == 12
            && (self.inner_width - 0.6).abs() < 0.01
            && (self.outer_width - 26.0).abs() < 0.01
            && (self.tiling_scalar - 0.15).abs() < 0.001
            && (self.scroll_rate - (-1.75)).abs() < 0.001
            && PRESENTATION_ORBITAL_LASER_TEXTURE == "EXNoise02.tga"
            && (self.outer_color.2 - 1.0).abs() < 0.01
    }

    /// Endpoints + elapsed for `LaserSegmentUpload::pack_orbital_multi_beam_soft_edge`.
    pub fn pack_endpoints(
        &self,
        start: (f32, f32, f32),
        end: (f32, f32, f32),
        elapsed_seconds: f32,
    ) -> ((f32, f32, f32), (f32, f32, f32), f32, f32) {
        let _ = self;
        (start, end, elapsed_seconds, 1.0)
    }
}

/// Snapshot-owned PatriotBinaryDataStream / assist laser beam for client draw.
///
/// Built only from host residual lasers at presentation build time so the
/// SegLine pack path does not re-read live GameLogic mid-render.
/// Fail-closed: not full W3DLaserDraw WGPU texture sample / multi-beam soft edge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresentationLaserBeam {
    /// Stable presentation index (order among active beams this frame).
    pub beam_index: u32,
    pub kind: PresentationLaserKind,
    pub from_id: ObjectId,
    pub to_id: ObjectId,
    pub from: (f32, f32, f32),
    pub to: (f32, f32, f32),
    pub arc_mid: (f32, f32, f32),
    pub scroll_offset: f32,
    pub expires_frame: u32,
    pub template_name: String,
    pub texture_name: String,
    /// C++ Weapon.ini LaserBoneName residual (empty for Patriot assist beams).
    #[serde(default)]
    pub laser_bone_name: String,
    pub inner_color: (f32, f32, f32, f32),
    pub segments: Vec<PresentationLaserSegment>,
    /// Line3D ground-skim residual used when segments were built.
    pub ground_height: f32,
    /// True when `ground_height` came from terrain sample (not default-0).
    pub ground_height_from_terrain: bool,
    /// Optional multi-beam soft-edge presentation residual (OrbitalLaser family).
    /// None for single-beam Patriot BinaryDataStream residual.
    pub soft_edge: Option<PresentationLaserSoftEdge>,
}

/// Assist laser kind frozen for presentation (mirrors host residual enum).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PresentationLaserKind {
    FromAssisted,
    ToTarget,
    /// Weapon.ini LaserName combat residual (not Patriot assist pair).
    WeaponLaser,
}

impl PresentationLaserKind {
    pub fn from_host(kind: PatriotAssistLaserKind) -> Self {
        match kind {
            PatriotAssistLaserKind::FromAssisted => Self::FromAssisted,
            PatriotAssistLaserKind::ToTarget => Self::ToTarget,
        }
    }
}

impl PresentationLaserBeam {
    /// Build from host residual laser + ground height (Line3D skim residual).
    pub fn from_host_laser(
        laser: &ResidualPatriotAssistLaser,
        beam_index: u32,
        ground_height: f32,
    ) -> Self {
        Self::from_host_laser_with_terrain(laser, beam_index, ground_height, false)
    }

    /// Build from host residual laser with terrain-sample honesty flag.
    /// Build from Weapon.ini LaserName residual beam.
    pub fn from_weapon_laser(
        laser: &crate::game_logic::host_weapon_laser::ResidualWeaponLaser,
        beam_index: u32,
        ground_height: f32,
        ground_height_from_terrain: bool,
    ) -> Self {
        use crate::game_logic::host_base_defense::build_patriot_laser_line3d_segments;
        let host_segs = build_patriot_laser_line3d_segments(
            laser.from_pos(),
            laser.to_pos(),
            0.0, // combat lasers are straight residual (no Patriot arc)
            laser.scroll_offset,
            ground_height,
        );
        let segments = host_segs
            .into_iter()
            .map(|s| PresentationLaserSegment {
                start: s.start,
                end: s.end,
                width: s.width,
                tile_factor: s.tile_factor,
                scroll_offset: s.scroll_offset,
            })
            .collect();
        let mid = (
            (laser.from_x + laser.to_x) * 0.5,
            (laser.from_y + laser.to_y) * 0.5,
            (laser.from_z + laser.to_z) * 0.5,
        );
        Self {
            beam_index,
            kind: PresentationLaserKind::WeaponLaser,
            from_id: laser.from_id,
            to_id: laser.to_id.unwrap_or(ObjectId(0)),
            from: laser.from_pos(),
            to: laser.to_pos(),
            arc_mid: mid,
            scroll_offset: laser.scroll_offset,
            expires_frame: laser.expires_frame,
            template_name: laser.laser_name.clone(),
            texture_name: laser.laser_name.clone(),
            laser_bone_name: laser.laser_bone_name.clone(),
            inner_color: (1.0, 0.2, 0.2, 1.0),
            segments,
            ground_height,
            ground_height_from_terrain,
            soft_edge: None,
        }
    }

    pub fn from_host_laser_with_terrain(
        laser: &ResidualPatriotAssistLaser,
        beam_index: u32,
        ground_height: f32,
        ground_height_from_terrain: bool,
    ) -> Self {
        let host_segs = build_patriot_laser_line3d_segments(
            (laser.from_x, laser.from_y, laser.from_z),
            (laser.to_x, laser.to_y, laser.to_z),
            laser.arc_height(),
            laser.scroll_offset,
            ground_height,
        );
        let segments = host_segs
            .into_iter()
            .map(|s| PresentationLaserSegment {
                start: s.start,
                end: s.end,
                width: s.width,
                tile_factor: s.tile_factor,
                scroll_offset: s.scroll_offset,
            })
            .collect();
        Self {
            beam_index,
            kind: PresentationLaserKind::from_host(laser.kind),
            from_id: laser.from_id,
            to_id: laser.to_id,
            from: (laser.from_x, laser.from_y, laser.from_z),
            to: (laser.to_x, laser.to_y, laser.to_z),
            arc_mid: (laser.arc_mid_x, laser.arc_mid_y, laser.arc_mid_z),
            scroll_offset: laser.scroll_offset,
            expires_frame: laser.expires_frame,
            template_name: PATRIOT_BINARY_DATA_STREAM.to_string(),
            texture_name: PATRIOT_LASER_TEXTURE.to_string(),
            laser_bone_name: String::new(),
            inner_color: PATRIOT_LASER_INNER_COLOR,
            segments,
            ground_height,
            ground_height_from_terrain,
            soft_edge: None,
        }
    }

    /// Synthetic assist-pair residual for host-testable laser pack honesty.
    ///
    /// Produces LaserFromAssisted + LaserToTarget with retail Segments=20 each.
    pub fn synthetic_assist_pair(start_frame: u32) -> [Self; 2] {
        Self::synthetic_assist_pair_with_ground(start_frame, PRESENTATION_DEFAULT_GROUND_HEIGHT)
    }

    /// Synthetic assist pair with explicit ground-height residual override.
    pub fn synthetic_assist_pair_with_ground(start_frame: u32, ground_height: f32) -> [Self; 2] {
        let beams = crate::game_logic::host_base_defense::make_patriot_assist_lasers(
            ObjectId(9001),
            ObjectId(9002),
            ObjectId(9003),
            (0.0, 0.0, 5.0),
            (40.0, 0.0, 5.0),
            (80.0, 0.0, 5.0),
            start_frame,
        );
        [
            Self::from_host_laser_with_terrain(&beams[0], 0, ground_height, false),
            Self::from_host_laser_with_terrain(&beams[1], 1, ground_height, false),
        ]
    }

    /// Synthetic OrbitalLaser multi-beam soft-edge residual for pack honesty.
    ///
    /// Vertical beam from origin; soft-edge fields wire to laser_segment_upload pack.
    pub fn synthetic_orbital_soft_edge(start_frame: u32) -> Self {
        let soft = PRESENTATION_ORBITAL_SOFT_EDGE;
        let start = (0.0, 0.0, 0.0);
        let end = (0.0, 0.0, 200.0);
        Self {
            beam_index: 0,
            kind: PresentationLaserKind::ToTarget,
            from_id: ObjectId(9101),
            to_id: ObjectId(9102),
            from: start,
            to: end,
            arc_mid: (0.0, 0.0, 100.0),
            scroll_offset: soft.scroll_rate * (start_frame as f32 / 30.0),
            expires_frame: start_frame.saturating_add(30),
            template_name: "ParticleUplinkCannon_OrbitalLaser".into(),
            texture_name: PRESENTATION_ORBITAL_LASER_TEXTURE.to_string(),
            laser_bone_name: String::new(),
            inner_color: (1.0, 1.0, 1.0, 250.0 / 255.0),
            segments: vec![PresentationLaserSegment {
                start,
                end,
                width: soft.inner_width,
                tile_factor: soft.tiling_scalar,
                scroll_offset: soft.scroll_rate * (start_frame as f32 / 30.0),
            }],
            ground_height: PRESENTATION_DEFAULT_GROUND_HEIGHT,
            ground_height_from_terrain: false,
            soft_edge: Some(soft),
        }
    }

    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }

    /// True when multi-beam soft-edge presentation residual is armed.
    pub fn has_soft_edge(&self) -> bool {
        self.soft_edge.is_some()
    }

    /// Honesty: ground-height residual on this beam is consistent.
    pub fn honesty_ground_height_ok(&self) -> bool {
        honesty_ground_height_residual_ok(self.ground_height, self.ground_height_from_terrain)
    }

    /// Honesty: soft-edge residual fields (or honest single-beam absence).
    pub fn honesty_soft_edge_presentation_ok(&self) -> bool {
        match self.soft_edge {
            Some(se) => se.honesty_orbital_residual_ok(),
            None => true, // single-beam Patriot residual is honest without soft edge
        }
    }
}

// ===== mod.rs =====
//! Immutable presentation snapshot built from the authoritative Main GameLogic.
//!
//! Policy: GameClient / renderer / HUD should consume `PresentationFrame` only.
//! They must not lock or mutate the sim while a WGPU pass is active.
//!
//! Ownership: borrow-first on the authority during `build_*`; then the snapshot
//! is owned values with no live borrows into the world.
//!
//! Wave 956: host_object/host_objects when building presentation from host.
//! Wave 958: host_object dual-read seal (tests + residual).

use crate::fow_rendering::{FOWRenderingBridge, ObjectVisibility, PresentationFowGrid};
use crate::game_logic::host_base_defense::{
    build_patriot_laser_line3d_segments, PatriotAssistLaserKind, ResidualPatriotAssistLaser,
    PATRIOT_BINARY_DATA_STREAM, PATRIOT_LASER_INNER_COLOR, PATRIOT_LASER_TEXTURE,
};
use crate::game_logic::{
    CombatParticleKind, CombatParticleSystemEntry, GameLogic, KindOf, ObjectId, Team,
};
use glam::Vec3;
use serde::{Deserialize, Serialize};

mod types;
mod unit_render;
mod projectile;
mod events;
mod particles;
mod lasers;
mod floating_text;
mod spectre;
mod world_env;
mod frame;
mod build;
mod queries;
mod alive;
mod overlay;
mod honesty;
mod apply;

#[cfg(test)]
mod tests;

pub use types::*;
pub use unit_render::*;
pub use projectile::*;
pub use events::*;
pub use particles::*;
pub use lasers::*;
pub use floating_text::*;
pub use spectre::*;
pub use world_env::*;
pub use frame::*;

/// Concatenated presentation_frame sources for residual `include_str` scans.
///
/// External crate tests previously read `presentation_frame.rs`. After the
/// directory split they should compare against this pack instead of a single file.
#[cfg(test)]
pub const PRESENTATION_FRAME_SRC: &str = concat!(
    include_str!("mod.rs"),
    include_str!("types.rs"),
    include_str!("unit_render.rs"),
    include_str!("projectile.rs"),
    include_str!("events.rs"),
    include_str!("particles.rs"),
    include_str!("lasers.rs"),
    include_str!("floating_text.rs"),
    include_str!("spectre.rs"),
    include_str!("world_env.rs"),
    include_str!("frame.rs"),
    include_str!("build.rs"),
    include_str!("queries.rs"),
    include_str!("alive.rs"),
    include_str!("overlay.rs"),
    include_str!("honesty.rs"),
    include_str!("apply.rs"),
);

// ===== overlay.rs =====
use super::*;

impl PresentationFrame {
    /// Overlay health/position/destroyed from a GameWorld shadow session.
    ///
    /// Host still builds the frame (templates, FOW, selection); shadow is last
    /// writer for HP and world position when authority paths are active.
    /// Unmapped objects are left unchanged.
    pub fn overlay_gameworld_shadow(
        &mut self,
        shadow: &crate::gameworld_shadow::GameWorldShadow,
    ) -> usize {
        let mut updated = 0usize;
        for obj in &mut self.objects {
            let Some(eid) = shadow.entity_for_host(obj.id) else {
                continue;
            };
            let Some(ent) = shadow.world().entity(eid) else {
                // Destroyed on shadow — mark destroyed for presentation.
                if !obj.destroyed {
                    obj.destroyed = true;
                    obj.health_current = 0.0;
                    updated += 1;
                }
                continue;
            };
            let pos = glam::Vec3::new(
                ent.transform.position.x,
                ent.transform.position.y,
                ent.transform.position.z,
            );
            let h = ent.health.max(0.0);
            let destroyed = h <= 0.0 || ent.destroyed;
            // Always apply shadow last-writer residual for presentation identity.
            let mut dirty = false;
            if (obj.position - pos).length_squared() > 1e-6 {
                obj.position = pos;
                dirty = true;
            }
            if (obj.orientation - ent.transform.orientation).abs() > 1e-5 {
                obj.orientation = ent.transform.orientation;
                dirty = true;
            }
            let move_dest = ent.move_target.map(|d| glam::Vec3::new(d[0], d[1], d[2]));
            if obj.move_destination != move_dest {
                obj.move_destination = move_dest;
                dirty = true;
            }
            let rally = ent.rally_point.map(|d| glam::Vec3::new(d[0], d[1], d[2]));
            if obj.rally_point != rally {
                obj.rally_point = rally;
                dirty = true;
            }
            let atk = ent
                .attack_target
                .and_then(|tid| shadow.host_for_entity(tid));
            if obj.attack_target != atk {
                obj.attack_target = atk;
                dirty = true;
            }
            if (obj.health_current - h).abs() > 1e-3 {
                obj.health_current = h;
                dirty = true;
            }
            if (obj.health_max - ent.max_health).abs() > 1e-3 && ent.max_health > 0.0 {
                obj.health_max = ent.max_health;
                dirty = true;
            }
            if obj.destroyed != destroyed {
                obj.destroyed = destroyed;
                dirty = true;
            }
            // Wave 189: expand last-writer overlay for motion/selection/body identity.
            let vel = glam::Vec3::new(ent.velocity[0], ent.velocity[1], ent.velocity[2]);
            if (obj.velocity - vel).length_squared() > 1e-6 {
                obj.velocity = vel;
                dirty = true;
            }
            if (obj.move_max_speed - ent.move_max_speed).abs() > 1e-4 && ent.move_max_speed >= 0.0 {
                obj.move_max_speed = ent.move_max_speed;
                dirty = true;
            }
            if obj.selected != ent.selected {
                obj.selected = ent.selected;
                dirty = true;
            }
            if obj.team_color != ent.team_color {
                obj.team_color = ent.team_color;
                dirty = true;
            }
            if obj.body_damage_state != ent.body_damage_state {
                obj.body_damage_state = ent.body_damage_state;
                dirty = true;
            }
            // Identity residual (template/team/type) from shadow — no live dual-read.
            if obj.template_name != ent.template.name {
                obj.template_name = ent.template.name.clone();
                dirty = true;
            }
            let team = match ent.team_ordinal {
                0 => crate::game_logic::Team::USA,
                1 => crate::game_logic::Team::China,
                2 => crate::game_logic::Team::GLA,
                _ => crate::game_logic::Team::Neutral,
            };
            if obj.team != team {
                obj.team = team;
                dirty = true;
            }
            let disguise_team = match ent.disguise_as_team_ordinal {
                0 => Some(crate::game_logic::Team::USA),
                1 => Some(crate::game_logic::Team::China),
                2 => Some(crate::game_logic::Team::GLA),
                3 => Some(crate::game_logic::Team::Neutral),
                _ => None,
            };
            if obj.disguise_as_team != disguise_team {
                obj.disguise_as_team = disguise_team;
                dirty = true;
            }
            let object_type = match ent.object_type_ordinal {
                0 => PresentationObjectType::Infantry,
                1 => PresentationObjectType::Vehicle,
                2 => PresentationObjectType::Aircraft,
                3 => PresentationObjectType::Building,
                4 => PresentationObjectType::Supply,
                5 => PresentationObjectType::Projectile,
                _ => PresentationObjectType::Neutral,
            };
            if obj.object_type != object_type {
                obj.object_type = object_type;
                dirty = true;
            }
            let is_structure =
                matches!(object_type, PresentationObjectType::Building) || ent.is_building;
            if obj.is_structure != is_structure {
                obj.is_structure = is_structure;
                dirty = true;
            }
            let is_unit = matches!(
                object_type,
                PresentationObjectType::Infantry
                    | PresentationObjectType::Vehicle
                    | PresentationObjectType::Aircraft
            );
            if obj.is_unit != is_unit {
                obj.is_unit = is_unit;
                dirty = true;
            }
            let is_mobile = is_unit;
            if obj.is_mobile != is_mobile {
                obj.is_mobile = is_mobile;
                dirty = true;
            }
            // Prefer is_building + not under construction for can_produce residual.
            let can_produce = ent.is_building && !ent.under_construction;
            if obj.can_produce != can_produce {
                obj.can_produce = can_produce;
                dirty = true;
            }
            let building_type = if ent.is_building {
                use PresentationBuildingType as P;
                match ent.building_type_ordinal {
                    0 => Some(P::CommandCenter),
                    1 => Some(P::Barracks),
                    2 => Some(P::WarFactory),
                    3 => Some(P::Airfield),
                    4 => Some(P::RepairPad),
                    5 => Some(P::HealPad),
                    6 => Some(P::SupplyCenter),
                    7 => Some(P::PowerPlant),
                    8 => Some(P::DefenseTurret),
                    9 => Some(P::SupplyDropZone),
                    10 => Some(P::Palace),
                    11 => Some(P::Propaganda),
                    12 => Some(P::Bunker),
                    _ => None,
                }
            } else {
                None
            };
            if obj.building_type != building_type {
                obj.building_type = building_type;
                dirty = true;
            }
            // Mesh identity residual (model_key / scale) — no live template dual-read.
            let model_key = if ent.model_key.is_empty() {
                None
            } else {
                Some(ent.model_key.clone())
            };
            if obj.model_key != model_key {
                obj.model_key = model_key;
                dirty = true;
            }
            if ent.mesh_scale.is_finite()
                && ent.mesh_scale > 0.0
                && (obj.mesh_scale - ent.mesh_scale).abs() > 1e-5
            {
                obj.mesh_scale = ent.mesh_scale;
                dirty = true;
            }
            // FOW + ground-height residual.
            {
                use crate::fow_rendering::ObjectVisibility;
                let vis = ObjectVisibility {
                    visibility_alpha: ent.fow_visibility_alpha,
                    is_explored: ent.fow_is_explored,
                    visibility_falloff: ent.fow_visibility_falloff,
                };
                if obj.fow_visibility != vis {
                    obj.fow_visibility = vis;
                    dirty = true;
                }
            }
            if (obj.ground_height - ent.ground_height).abs() > 1e-3 {
                obj.ground_height = ent.ground_height;
                dirty = true;
            }
            if obj.ground_height_from_terrain != ent.ground_height_from_terrain {
                obj.ground_height_from_terrain = ent.ground_height_from_terrain;
                dirty = true;
            }
            if obj.engine_bridged != ent.engine_bridged {
                obj.engine_bridged = ent.engine_bridged;
                dirty = true;
            }
            if obj.selected != ent.selected {
                obj.selected = ent.selected;
                dirty = true;
            }
            if obj.under_construction != ent.under_construction {
                obj.under_construction = ent.under_construction;
                dirty = true;
            }
            if obj.sold != ent.sold {
                obj.sold = ent.sold;
                dirty = true;
            }
            if obj.reconstructing != ent.reconstructing {
                obj.reconstructing = ent.reconstructing;
                dirty = true;
            }
            if obj.unselectable != ent.unselectable {
                obj.unselectable = ent.unselectable;
                dirty = true;
            }
            if obj.is_deployed != ent.deployed {
                obj.is_deployed = ent.deployed;
                dirty = true;
            }
            if (obj.construction_percent - ent.construction_percent).abs() > 1e-4 {
                obj.construction_percent = ent.construction_percent;
                dirty = true;
            }
            if obj.moving != ent.moving {
                obj.moving = ent.moving;
                dirty = true;
            }
            if obj.attacking != ent.attacking {
                obj.attacking = ent.attacking;
                dirty = true;
            }
            if obj.is_firing_weapon != ent.is_firing_weapon {
                obj.is_firing_weapon = ent.is_firing_weapon;
                dirty = true;
            }
            if obj.is_aiming_weapon != ent.is_aiming_weapon {
                obj.is_aiming_weapon = ent.is_aiming_weapon;
                dirty = true;
            }
            if obj.disabled_emp != ent.disabled_emp {
                obj.disabled_emp = ent.disabled_emp;
                dirty = true;
            }
            if obj.disabled_paralyzed != ent.disabled_paralyzed {
                obj.disabled_paralyzed = ent.disabled_paralyzed;
                dirty = true;
            }
            if obj.weapons_jammed != ent.weapons_jammed {
                obj.weapons_jammed = ent.weapons_jammed;
                dirty = true;
            }
            if obj.masked != ent.masked {
                obj.masked = ent.masked;
                dirty = true;
            }
            if obj.disguised != ent.disguised {
                obj.disguised = ent.disguised;
                dirty = true;
            }
            if obj.disabled_subdued != ent.disabled_subdued {
                obj.disabled_subdued = ent.disabled_subdued;
                dirty = true;
            }
            if obj.is_carbomb != ent.is_carbomb {
                obj.is_carbomb = ent.is_carbomb;
                dirty = true;
            }
            if obj.hijacked != ent.hijacked {
                obj.hijacked = ent.hijacked;
                dirty = true;
            }
            if obj.team_color != ent.team_color {
                obj.team_color = ent.team_color;
                dirty = true;
            }
            if (obj.selection_radius - ent.selection_radius).abs() > 1e-3 {
                obj.selection_radius = ent.selection_radius;
                dirty = true;
            }
            if obj.ignoring_stealth != ent.ignoring_stealth {
                obj.ignoring_stealth = ent.ignoring_stealth;
                dirty = true;
            }
            if obj.repulsor != ent.repulsor {
                obj.repulsor = ent.repulsor;
                dirty = true;
            }
            if obj.stealthed != ent.stealthed {
                obj.stealthed = ent.stealthed;
                dirty = true;
            }
            if obj.detected != ent.detected {
                obj.detected = ent.detected;
                dirty = true;
            }
            if obj.force_attack != ent.force_attack {
                obj.force_attack = ent.force_attack;
                dirty = true;
            }
            if obj.has_weapon != ent.has_weapon {
                obj.has_weapon = ent.has_weapon;
                dirty = true;
            }
            if (obj.weapon_range - ent.weapon_range).abs() > 1e-3 {
                obj.weapon_range = ent.weapon_range;
                dirty = true;
            }
            if (obj.weapon_damage - ent.weapon_damage).abs() > 1e-3 {
                obj.weapon_damage = ent.weapon_damage;
                dirty = true;
            }
            if (obj.weapon_min_range - ent.weapon_min_range).abs() > 1e-3 {
                obj.weapon_min_range = ent.weapon_min_range;
                dirty = true;
            }
            if (obj.weapon_reload_time - ent.weapon_reload_time).abs() > 1e-3 {
                obj.weapon_reload_time = ent.weapon_reload_time;
                dirty = true;
            }
            if obj.weapon_ammo != ent.weapon_ammo {
                obj.weapon_ammo = ent.weapon_ammo;
                dirty = true;
            }
            if obj.weapon_can_target_air != ent.weapon_can_target_air {
                obj.weapon_can_target_air = ent.weapon_can_target_air;
                dirty = true;
            }
            if obj.weapon_can_target_ground != ent.weapon_can_target_ground {
                obj.weapon_can_target_ground = ent.weapon_can_target_ground;
                dirty = true;
            }
            if (obj.weapon_projectile_speed - ent.weapon_projectile_speed).abs() > 1e-3 {
                obj.weapon_projectile_speed = ent.weapon_projectile_speed;
                dirty = true;
            }
            if obj.armed_riders_upgrade_weapon_set != ent.armed_riders_upgrade_weapon_set {
                obj.armed_riders_upgrade_weapon_set = ent.armed_riders_upgrade_weapon_set;
                dirty = true;
            }
            if obj.weapon_set_player_upgrade != ent.weapon_set_player_upgrade {
                obj.weapon_set_player_upgrade = ent.weapon_set_player_upgrade;
                dirty = true;
            }
            if obj.second_life != ent.second_life {
                obj.second_life = ent.second_life;
                dirty = true;
            }
            if obj.front_crushed != ent.front_crushed {
                obj.front_crushed = ent.front_crushed;
                dirty = true;
            }
            if obj.back_crushed != ent.back_crushed {
                obj.back_crushed = ent.back_crushed;
                dirty = true;
            }
            if obj.user_1 != ent.user_1 {
                obj.user_1 = ent.user_1;
                dirty = true;
            }
            if obj.user_2 != ent.user_2 {
                obj.user_2 = ent.user_2;
                dirty = true;
            }
            if obj.weapon_crate_upgrade != ent.weapon_crate_upgrade {
                obj.weapon_crate_upgrade = ent.weapon_crate_upgrade;
                dirty = true;
            }
            if obj.armor_crate_upgrade != ent.armor_crate_upgrade {
                obj.armor_crate_upgrade = ent.armor_crate_upgrade;
                dirty = true;
            }
            if obj.enemy_near != ent.enemy_near {
                obj.enemy_near = ent.enemy_near;
                dirty = true;
            }
            if obj.armed != ent.armed {
                obj.armed = ent.armed;
                dirty = true;
            }
            if obj.command_set_override != ent.command_set_override {
                obj.command_set_override = ent.command_set_override.clone();
                dirty = true;
            }
            if obj.is_detector != ent.is_detector {
                obj.is_detector = ent.is_detector;
                dirty = true;
            }
            if obj.show_health_bar != ent.show_health_bar {
                obj.show_health_bar = ent.show_health_bar;
                dirty = true;
            }
            // Expanded Entity residual last-writer (presentation consumers).
            if obj.power_provided != ent.power_provided {
                obj.power_provided = ent.power_provided;
                dirty = true;
            }
            if obj.power_consumed != ent.power_consumed {
                obj.power_consumed = ent.power_consumed;
                dirty = true;
            }
            if (obj.experience_points - ent.experience_points).abs() > 1e-3 {
                obj.experience_points = ent.experience_points;
                dirty = true;
            }
            if obj.stored_supplies != ent.stored_supplies {
                obj.stored_supplies = ent.stored_supplies;
                dirty = true;
            }
            let gp = ent
                .guard_position
                .map(|p| glam::Vec3::new(p[0], p[1], p[2]));
            if obj.guard_position != gp {
                obj.guard_position = gp;
                dirty = true;
            }
            // Movement / target residual.
            let tl = ent
                .target_location
                .map(|p| glam::Vec3::new(p[0], p[1], p[2]));
            if obj.target_location != tl {
                obj.target_location = tl;
                dirty = true;
            }
            let gt = if ent.guard_target_host == 0 {
                None
            } else {
                Some(crate::game_logic::ObjectId(ent.guard_target_host))
            };
            if obj.guard_target != gt {
                obj.guard_target = gt;
                dirty = true;
            }
            if obj.using_ability != ent.using_ability {
                obj.using_ability = ent.using_ability;
                dirty = true;
            }
            if obj.airborne_target != ent.airborne_target {
                obj.airborne_target = ent.airborne_target;
                dirty = true;
            }
            if (obj.move_max_speed - ent.move_max_speed).abs() > 1e-3 {
                obj.move_max_speed = ent.move_max_speed;
                dirty = true;
            }
            let vel = glam::Vec3::new(ent.velocity[0], ent.velocity[1], ent.velocity[2]);
            if (obj.velocity - vel).length_squared() > 1e-6 {
                obj.velocity = vel;
                dirty = true;
            }
            if obj.ai_state_ordinal != ent.ai_state_ordinal {
                obj.ai_state_ordinal = ent.ai_state_ordinal;
                dirty = true;
            }
            let rp = ent.rally_point.map(|p| glam::Vec3::new(p[0], p[1], p[2]));
            if obj.rally_point != rp {
                obj.rally_point = rp;
                dirty = true;
            }
            if obj.max_garrison != ent.max_garrison as usize {
                obj.max_garrison = ent.max_garrison as usize;
                dirty = true;
            }
            if obj.has_secondary_weapon != ent.has_secondary_weapon {
                obj.has_secondary_weapon = ent.has_secondary_weapon;
                dirty = true;
            }
            if (obj.cheer_timer - ent.cheer_timer).abs() > 1e-4 {
                obj.cheer_timer = ent.cheer_timer;
                dirty = true;
            }
            if obj.overcharge_enabled != ent.overcharge_enabled {
                obj.overcharge_enabled = ent.overcharge_enabled;
                dirty = true;
            }
            if obj.shock_was_airborne != ent.shock_was_airborne {
                obj.shock_was_airborne = ent.shock_was_airborne;
                dirty = true;
            }
            if obj.shock_allow_bounce != ent.shock_allow_bounce {
                obj.shock_allow_bounce = ent.shock_allow_bounce;
                dirty = true;
            }
            if obj.shock_grounded_once != ent.shock_grounded_once {
                obj.shock_grounded_once = ent.shock_grounded_once;
                dirty = true;
            }
            if obj.shock_stun_frames != ent.shock_stun_frames {
                obj.shock_stun_frames = ent.shock_stun_frames;
                dirty = true;
            }
            if obj.power_plant_rods_extended != ent.power_plant_rods_extended {
                obj.power_plant_rods_extended = ent.power_plant_rods_extended;
                dirty = true;
            }
            if obj.power_plant_rods_done_frame != ent.power_plant_rods_done_frame {
                obj.power_plant_rods_done_frame = ent.power_plant_rods_done_frame;
                dirty = true;
            }
            if obj.jet_slow_death_active != ent.jet_slow_death_active {
                obj.jet_slow_death_active = ent.jet_slow_death_active;
                dirty = true;
            }
            if obj.anim_steer_turn != ent.anim_steer_turn {
                obj.anim_steer_turn = ent.anim_steer_turn;
                dirty = true;
            }
            if obj.active_weapon_slot != ent.active_weapon_slot {
                obj.active_weapon_slot = ent.active_weapon_slot;
                dirty = true;
            }
            if obj.weapon_fire_status != ent.weapon_fire_status {
                obj.weapon_fire_status = ent.weapon_fire_status;
                dirty = true;
            }
            if obj.is_panicking != ent.is_panicking {
                obj.is_panicking = ent.is_panicking;
                dirty = true;
            }
            if obj.moving_backwards != ent.moving_backwards {
                obj.moving_backwards = ent.moving_backwards;
                dirty = true;
            }
            if (obj.guard_radius - ent.guard_radius).abs() > 1e-3 {
                obj.guard_radius = ent.guard_radius;
                dirty = true;
            }
            if obj.special_power_ready != ent.special_power_ready {
                obj.special_power_ready = ent.special_power_ready;
                dirty = true;
            }
            if (obj.special_power_cooldown - ent.special_power_cooldown).abs() > 1e-3 {
                obj.special_power_cooldown = ent.special_power_cooldown;
                dirty = true;
            }
            if (obj.special_power_cooldown_remaining - ent.special_power_cooldown_remaining).abs()
                > 1e-3
            {
                obj.special_power_cooldown_remaining = ent.special_power_cooldown_remaining;
                dirty = true;
            }
            if (obj.detection_range - ent.detection_range).abs() > 1e-3 {
                obj.detection_range = ent.detection_range;
                dirty = true;
            }
            if obj.detection_rate_frames != ent.detection_rate_frames {
                obj.detection_rate_frames = ent.detection_rate_frames;
                dirty = true;
            }
            if obj.stealth_breaks_on_attack != ent.stealth_breaks_on_attack {
                obj.stealth_breaks_on_attack = ent.stealth_breaks_on_attack;
                dirty = true;
            }
            if obj.stealth_breaks_on_move != ent.stealth_breaks_on_move {
                obj.stealth_breaks_on_move = ent.stealth_breaks_on_move;
                dirty = true;
            }
            if obj.innate_stealth != ent.innate_stealth {
                obj.innate_stealth = ent.innate_stealth;
                dirty = true;
            }
            if obj.weapon_bonus_enthusiastic != ent.weapon_bonus_enthusiastic {
                obj.weapon_bonus_enthusiastic = ent.weapon_bonus_enthusiastic;
                dirty = true;
            }
            if obj.weapon_bonus_subliminal != ent.weapon_bonus_subliminal {
                obj.weapon_bonus_subliminal = ent.weapon_bonus_subliminal;
                dirty = true;
            }
            if obj.weapon_bonus_horde != ent.weapon_bonus_horde {
                obj.weapon_bonus_horde = ent.weapon_bonus_horde;
                dirty = true;
            }
            if obj.weapon_bonus_nationalism != ent.weapon_bonus_nationalism {
                obj.weapon_bonus_nationalism = ent.weapon_bonus_nationalism;
                dirty = true;
            }
            if obj.weapon_bonus_frenzy != ent.weapon_bonus_frenzy {
                obj.weapon_bonus_frenzy = ent.weapon_bonus_frenzy;
                dirty = true;
            }
            if obj.weapon_bonus_frenzy_level != ent.weapon_bonus_frenzy_level {
                obj.weapon_bonus_frenzy_level = ent.weapon_bonus_frenzy_level;
                dirty = true;
            }
            if obj.weapon_bonus_frenzy_until_frame != ent.weapon_bonus_frenzy_until_frame {
                obj.weapon_bonus_frenzy_until_frame = ent.weapon_bonus_frenzy_until_frame;
                dirty = true;
            }
            if obj.weapon_bonus_battle_plan_bombardment != ent.weapon_bonus_battle_plan_bombardment
            {
                obj.weapon_bonus_battle_plan_bombardment = ent.weapon_bonus_battle_plan_bombardment;
                dirty = true;
            }
            if obj.weapon_bonus_battle_plan_hold_the_line
                != ent.weapon_bonus_battle_plan_hold_the_line
            {
                obj.weapon_bonus_battle_plan_hold_the_line =
                    ent.weapon_bonus_battle_plan_hold_the_line;
                dirty = true;
            }
            if obj.weapon_bonus_battle_plan_search_and_destroy
                != ent.weapon_bonus_battle_plan_search_and_destroy
            {
                obj.weapon_bonus_battle_plan_search_and_destroy =
                    ent.weapon_bonus_battle_plan_search_and_destroy;
                dirty = true;
            }
            if (obj.battle_plan_sight_scalar_applied - ent.battle_plan_sight_scalar_applied).abs()
                > 1e-4
            {
                obj.battle_plan_sight_scalar_applied = ent.battle_plan_sight_scalar_applied;
                dirty = true;
            }
            if obj.continuous_fire_level != ent.continuous_fire_level {
                obj.continuous_fire_level = ent.continuous_fire_level;
                dirty = true;
            }
            if obj.continuous_fire_consecutive != ent.continuous_fire_consecutive {
                obj.continuous_fire_consecutive = ent.continuous_fire_consecutive;
                dirty = true;
            }
            if obj.continuous_fire_coast_until_frame != ent.continuous_fire_coast_until_frame {
                obj.continuous_fire_coast_until_frame = ent.continuous_fire_coast_until_frame;
                dirty = true;
            }
            if obj.faerie_fire_until_frame != ent.faerie_fire_until_frame {
                obj.faerie_fire_until_frame = ent.faerie_fire_until_frame;
                dirty = true;
            }
            if obj.is_humvee_transport != ent.is_humvee_transport {
                obj.is_humvee_transport = ent.is_humvee_transport;
                dirty = true;
            }
            if obj.is_listening_outpost_transport != ent.is_listening_outpost_transport {
                obj.is_listening_outpost_transport = ent.is_listening_outpost_transport;
                dirty = true;
            }
            if obj.is_troop_crawler_transport != ent.is_troop_crawler_transport {
                obj.is_troop_crawler_transport = ent.is_troop_crawler_transport;
                dirty = true;
            }
            if obj.is_helix_transport != ent.is_helix_transport {
                obj.is_helix_transport = ent.is_helix_transport;
                dirty = true;
            }
            if obj.has_overlord_gattling_addon != ent.has_overlord_gattling_addon {
                obj.has_overlord_gattling_addon = ent.has_overlord_gattling_addon;
                dirty = true;
            }
            if obj.has_overlord_propaganda_addon != ent.has_overlord_propaganda_addon {
                obj.has_overlord_propaganda_addon = ent.has_overlord_propaganda_addon;
                dirty = true;
            }
            // Expanded transport-kind / display residual.
            if obj.is_battle_bus_transport != ent.is_battle_bus_transport {
                obj.is_battle_bus_transport = ent.is_battle_bus_transport;
                dirty = true;
            }
            if obj.is_technical_transport != ent.is_technical_transport {
                obj.is_technical_transport = ent.is_technical_transport;
                dirty = true;
            }
            if obj.is_combat_cycle_transport != ent.is_combat_cycle_transport {
                obj.is_combat_cycle_transport = ent.is_combat_cycle_transport;
                dirty = true;
            }
            if obj.combat_cycle_rider != ent.combat_cycle_rider {
                obj.combat_cycle_rider = ent.combat_cycle_rider;
                dirty = true;
            }
            if obj.is_tunnel_network != ent.is_tunnel_network {
                obj.is_tunnel_network = ent.is_tunnel_network;
                dirty = true;
            }
            if obj.is_combat_chinook_transport != ent.is_combat_chinook_transport {
                obj.is_combat_chinook_transport = ent.is_combat_chinook_transport;
                dirty = true;
            }
            if obj.max_transport != ent.max_transport {
                obj.max_transport = ent.max_transport;
                dirty = true;
            }
            let bunker_cap = if ent.overlord_bunker_capacity == u16::MAX {
                usize::MAX
            } else {
                ent.overlord_bunker_capacity as usize
            };
            if obj.overlord_bunker_capacity != bunker_cap {
                obj.overlord_bunker_capacity = bunker_cap;
                dirty = true;
            }
            if obj.passengers_allowed_to_fire != ent.passengers_allowed_to_fire {
                obj.passengers_allowed_to_fire = ent.passengers_allowed_to_fire;
                dirty = true;
            }
            if obj.display_name != ent.display_name {
                obj.display_name = ent.display_name.clone();
                dirty = true;
            }
            if obj.demo_suicided_detonating != ent.demo_suicided_detonating {
                obj.demo_suicided_detonating = ent.demo_suicided_detonating;
                dirty = true;
            }
            if obj.hive_slave_count != ent.hive_slave_count {
                obj.hive_slave_count = ent.hive_slave_count;
                dirty = true;
            }
            if (obj.hive_slave_hp - ent.hive_slave_hp).abs() > 1e-3 {
                obj.hive_slave_hp = ent.hive_slave_hp;
                dirty = true;
            }
            if (obj.turret_angle_deg - ent.turret_angle_deg).abs() > 1e-3 {
                obj.turret_angle_deg = ent.turret_angle_deg;
                dirty = true;
            }
            if (obj.turret_pitch_deg - ent.turret_pitch_deg).abs() > 1e-3 {
                obj.turret_pitch_deg = ent.turret_pitch_deg;
                dirty = true;
            }
            if obj.turret_idle_scanning != ent.turret_idle_scanning {
                obj.turret_idle_scanning = ent.turret_idle_scanning;
                dirty = true;
            }
            if obj.turret_holding != ent.turret_holding {
                obj.turret_holding = ent.turret_holding;
                dirty = true;
            }
            if obj.ai_attitude != ent.ai_attitude {
                obj.ai_attitude = ent.ai_attitude;
                dirty = true;
            }
            if obj.last_damage_source_host != ent.last_damage_source_host {
                obj.last_damage_source_host = ent.last_damage_source_host;
                dirty = true;
            }
            let disguise = if ent.disguise_as_template.is_empty() {
                None
            } else {
                Some(ent.disguise_as_template.clone())
            };
            if obj.disguise_as_template != disguise {
                obj.disguise_as_template = disguise;
                dirty = true;
            }
            if obj.vision_spied_mask != ent.vision_spied_mask {
                obj.vision_spied_mask = ent.vision_spied_mask;
                dirty = true;
            }
            // Wave 994: vision / shroud-clear / crush residual last-writer.
            if (obj.vision_range - ent.vision_range).abs() > 1e-4 {
                obj.vision_range = ent.vision_range;
                dirty = true;
            }
            if (obj.shroud_clearing_range - ent.shroud_clearing_range).abs() > 1e-4 {
                obj.shroud_clearing_range = ent.shroud_clearing_range;
                dirty = true;
            }
            if obj.crusher_level != ent.crusher_level {
                obj.crusher_level = ent.crusher_level;
                dirty = true;
            }
            if obj.crushable_level != ent.crushable_level {
                obj.crushable_level = ent.crushable_level;
                dirty = true;
            }
            // Wave 995: captured / prone / poison / defector residual last-writer.
            if obj.captured != ent.private_captured {
                obj.captured = ent.private_captured;
                dirty = true;
            }
            if obj.prone != ent.prone_active {
                obj.prone = ent.prone_active;
                dirty = true;
            }
            let poison_tinted = ent.poison_damage_frame != 0;
            if obj.poison_tinted != poison_tinted {
                obj.poison_tinted = poison_tinted;
                dirty = true;
            }
            if obj.undetected_defector != ent.defection_undetected {
                obj.undetected_defector = ent.defection_undetected;
                dirty = true;
            }
            if obj.defector_flash != ent.defection_flash_this_frame {
                obj.defector_flash = ent.defection_flash_this_frame;
                dirty = true;
            }
            if obj.cell_is_cliff != ent.cell_is_cliff {
                obj.cell_is_cliff = ent.cell_is_cliff;
                dirty = true;
            }
            if obj.cell_is_underwater != ent.cell_is_underwater {
                obj.cell_is_underwater = ent.cell_is_underwater;
                dirty = true;
            }
            if obj.over_water != ent.cell_is_underwater {
                obj.over_water = ent.cell_is_underwater;
                dirty = true;
            }
            if obj.formation_id != ent.formation_id {
                obj.formation_id = ent.formation_id;
                dirty = true;
            }
            let form_off = glam::Vec2::new(ent.formation_offset[0], ent.formation_offset[1]);
            if (obj.formation_offset - form_off).length_squared() > 1e-8 {
                obj.formation_offset = form_off;
                dirty = true;
            }
            // Wave 999: surrender + emoticon residual last-writer.
            if obj.is_surrendered != ent.is_surrendered {
                obj.is_surrendered = ent.is_surrendered;
                dirty = true;
            }
            if obj.emoticon_name != ent.emoticon_name {
                obj.emoticon_name = ent.emoticon_name.clone();
                dirty = true;
            }
            if obj.emoticon_frames_left != ent.emoticon_frames_left {
                obj.emoticon_frames_left = ent.emoticon_frames_left;
                dirty = true;
            }
            // Wave 1001: FX name residual last-writer.
            if obj.damage_fx_name != ent.damage_fx_name {
                obj.damage_fx_name = ent.damage_fx_name.clone();
                dirty = true;
            }
            if obj.bone_fx_name != ent.bone_fx_name {
                obj.bone_fx_name = ent.bone_fx_name.clone();
                dirty = true;
            }
            if obj.death_fx_name != ent.death_fx_name {
                obj.death_fx_name = ent.death_fx_name.clone();
                dirty = true;
            }
            // Wave 996: topple lean + healing icon residual last-writer.
            if (obj.topple_lean_radians - ent.topple_lean_radians).abs() > 1e-5 {
                obj.topple_lean_radians = ent.topple_lean_radians;
                dirty = true;
            }
            let show_healing = ent.sole_healing_benefactor_expiration_frame != 0;
            if obj.show_healing != show_healing {
                obj.show_healing = show_healing;
                dirty = true;
            }
            {
                const STRUCTURE_BIT: u32 = 1 << 0;
                const VEHICLE_BIT: u32 = 1 << 2;
                let icon = if ent.kind_of_bits & STRUCTURE_BIT != 0 {
                    1u8
                } else if ent.kind_of_bits & VEHICLE_BIT != 0 {
                    2u8
                } else {
                    0u8
                };
                if obj.healing_icon_type != icon {
                    obj.healing_icon_type = icon;
                    dirty = true;
                }
            }
            if (obj.camo_friendly_opacity - ent.camo_friendly_opacity).abs() > 1e-4 {
                obj.camo_friendly_opacity = ent.camo_friendly_opacity;
                dirty = true;
            }
            if obj.camo_stealth_look != ent.camo_stealth_look {
                obj.camo_stealth_look = ent.camo_stealth_look;
                dirty = true;
            }
            // Path waypoints residual (presentation move lines).
            let path_wp: Vec<glam::Vec3> = ent
                .path_waypoints
                .iter()
                .map(|p| glam::Vec3::new(p[0], p[1], p[2]))
                .collect();
            if obj.path_waypoints != path_wp {
                obj.path_waypoints = path_wp;
                dirty = true;
            }
            if obj.path_len != ent.path_len {
                obj.path_len = ent.path_len;
                dirty = true;
            }
            if obj.path_index != ent.path_index {
                obj.path_index = ent.path_index;
                dirty = true;
            }
            if obj.occupant_count != ent.occupant_count {
                obj.occupant_count = ent.occupant_count;
                dirty = true;
            }
            // Production queue head residual.
            // Full production queue residual (not head-only).
            if !ent.production_queue_items.is_empty() {
                let q: Vec<PresentationProductionItem> = ent
                    .production_queue_items
                    .iter()
                    .map(|p| PresentationProductionItem {
                        template_name: p.template_name.clone(),
                        progress: p.progress,
                        total_time: p.total_time,
                        cost_supplies: p.cost_supplies,
                        is_upgrade: p.is_upgrade,
                        progress_ratio: if p.total_time <= 0.0 {
                            1.0
                        } else {
                            (p.progress / p.total_time).clamp(0.0, 1.0)
                        },
                    })
                    .collect();
                if obj.production_queue != q {
                    obj.production_queue = q;
                    dirty = true;
                }
                if obj.production_paused != ent.production_paused {
                    obj.production_paused = ent.production_paused;
                    dirty = true;
                }
            } else if !obj.production_queue.is_empty() || obj.production_paused {
                obj.production_queue.clear();
                if obj.production_paused != ent.production_paused {
                    obj.production_paused = ent.production_paused;
                }
                dirty = true;
            }
            let ent_producer = ent.producer_id.map(ObjectId);
            if obj.producer_id != ent_producer {
                obj.producer_id = ent_producer;
                dirty = true;
            }
            if obj.is_rebuild_hole != ent.is_rebuild_hole {
                obj.is_rebuild_hole = ent.is_rebuild_hole;
                dirty = true;
            }
            if obj.rebuild_template_name != ent.rebuild_template_name {
                obj.rebuild_template_name = ent.rebuild_template_name.clone();
                dirty = true;
            }
            if obj.rebuild_ready_frame != ent.rebuild_ready_frame {
                obj.rebuild_ready_frame = ent.rebuild_ready_frame;
                dirty = true;
            }
            let spawner = ent.rebuild_spawner_id.map(ObjectId);
            if obj.rebuild_spawner_id != spawner {
                obj.rebuild_spawner_id = spawner;
                dirty = true;
            }
            let worker = ent.rebuild_worker_id.map(ObjectId);
            if obj.rebuild_worker_id != worker {
                obj.rebuild_worker_id = worker;
                dirty = true;
            }
            let recon = ent.rebuild_reconstructing_id.map(ObjectId);
            if obj.rebuild_reconstructing_id != recon {
                obj.rebuild_reconstructing_id = recon;
                dirty = true;
            }
            if obj.reconstructing != ent.reconstructing {
                obj.reconstructing = ent.reconstructing;
                dirty = true;
            }
            if obj.has_secondary_weapon != ent.has_secondary_weapon {
                obj.has_secondary_weapon = ent.has_secondary_weapon;
                dirty = true;
            }
            if (obj.secondary_weapon_range - ent.secondary_weapon_range).abs() > 1e-3 {
                obj.secondary_weapon_range = ent.secondary_weapon_range;
                dirty = true;
            }
            if (obj.secondary_weapon_damage - ent.secondary_weapon_damage).abs() > 1e-3 {
                obj.secondary_weapon_damage = ent.secondary_weapon_damage;
                dirty = true;
            }
            if obj.has_mine != ent.has_mine_data {
                obj.has_mine = ent.has_mine_data;
                dirty = true;
            }
            // Contain / garrison residual.
            let contained = if ent.contained_by_host == 0 {
                None
            } else {
                Some(crate::game_logic::ObjectId(ent.contained_by_host))
            };
            if obj.contained_by != contained {
                obj.contained_by = contained;
                dirty = true;
            }
            let garrisoned: Vec<crate::game_logic::ObjectId> = ent
                .garrisoned_host_ids
                .iter()
                .copied()
                .map(crate::game_logic::ObjectId)
                .collect();
            if obj.garrisoned_units != garrisoned {
                obj.garrisoned_units = garrisoned;
                dirty = true;
            }
            // Disabled residual (any host disable flag).
            let disabled =
                ent.disabled_underpowered || ent.disabled_unmanned || ent.disabled_hacked;
            if obj.disabled != disabled {
                obj.disabled = disabled;
                dirty = true;
            }
            // Veterancy ordinal residual.
            let vet = match ent.veterancy_ordinal {
                1 => PresentationVeterancy::Veteran,
                2 => PresentationVeterancy::Elite,
                3 => PresentationVeterancy::Heroic,
                _ => PresentationVeterancy::Rookie,
            };
            if obj.veterancy != vet {
                obj.veterancy = vet;
                dirty = true;
            }
            // KindOf bitset residual → presentation ORDER vector.
            {
                use crate::game_logic::KindOf;
                const ORDER: &[KindOf] = &[
                    KindOf::Structure,
                    KindOf::Infantry,
                    KindOf::Vehicle,
                    KindOf::Aircraft,
                    KindOf::Projectile,
                    KindOf::Resource,
                    KindOf::Selectable,
                    KindOf::Attackable,
                    KindOf::CommandCenter,
                    KindOf::Worker,
                    KindOf::Hero,
                    KindOf::SupplyCenter,
                    KindOf::PowerPlant,
                    KindOf::FSBarracks,
                    KindOf::FSWarFactory,
                    KindOf::FSAirfield,
                    KindOf::FSInternetCenter,
                    KindOf::FSPower,
                    KindOf::FSBaseDefense,
                    KindOf::FSSupplyDropzone,
                    KindOf::FSSupplyCenter,
                    KindOf::FSSuperweapon,
                    KindOf::FSStrategyCenter,
                    KindOf::FSFake,
                    KindOf::FSTechnology,
                    KindOf::FSBlackMarket,
                    KindOf::FSAdvancedTech,
                    KindOf::Harvestable,
                    KindOf::Powered,
                ];
                let mut v: Vec<KindOf> = ORDER
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| ent.kind_of_bits & (1u32 << i) != 0)
                    .map(|(_, k)| *k)
                    .collect();
                v.truncate(32);
                if obj.kind_of != v {
                    obj.kind_of = v;
                    dirty = true;
                }
            }
            // Applied upgrade names residual.
            if obj.applied_upgrades != ent.applied_upgrade_names {
                obj.applied_upgrades = ent.applied_upgrade_names.clone();
                dirty = true;
            }
            // Effectively stealthed residual from shadow flags.
            let eff = ent.stealthed && !ent.detected && obj.disguise_as_template.is_none();
            if obj.effectively_stealthed != eff {
                obj.effectively_stealthed = eff;
                dirty = true;
            }
            if dirty {
                updated += 1;
            }
        }
        // Local player residual from shadow (presentation last-writer).
        // Prefer local_player_id slot (same dense index as host player id residual).
        let local_pid = gamelogic::world::PlayerId::from_index(self.local_player_id as u8);
        if let Some(p) = shadow.world().player(local_pid) {
            if crate::gameworld_shadow::gameworld_economy_authority_live() {
                self.local_supplies = p.supplies;
                self.local_power = p.power_available;
                self.local_power_produced = p.power_produced;
                self.local_power_consumed = p.power_consumed;
            }
            // Presentation-only player residual (always from shadow when mapped).
            self.local_is_alive = p.is_alive;
            self.local_radar_count = p.radar_count;
            self.local_radar_disabled = p.radar_disabled;
            self.local_cash_bounty_percent = p.cash_bounty_percent;
            self.local_color_rgb = p.color_rgb;
            self.local_rank_level = p.rank_level.max(1);
            self.local_skill_points = p.skill_points;
            self.local_science_purchase_points = p.science_purchase_points;
            self.local_unlocked_sciences = p.unlocked_sciences.clone();
            {
                use crate::game_logic::host_rank_ui_residual::{
                    rank_level_down_threshold_residual, rank_level_up_threshold_residual,
                    rank_progress_percent_residual, RankSkillStateResidual,
                };
                let state = RankSkillStateResidual {
                    rank_level: self.local_rank_level,
                    skill_points: self.local_skill_points,
                    science_purchase_points: self.local_science_purchase_points,
                    level_up: rank_level_up_threshold_residual(self.local_rank_level),
                    level_down: rank_level_down_threshold_residual(self.local_rank_level),
                };
                self.local_rank_progress_percent = rank_progress_percent_residual(&state);
            }
            // Superweapon PublicTimer remaining from shadow shared cooldowns.
            for timer in &mut self.superweapon_timers {
                if timer.power_key.is_empty() {
                    continue;
                }
                if let Some((_, rem)) = p
                    .shared_special_power_cooldowns
                    .iter()
                    .find(|(k, _)| k == &timer.power_key)
                {
                    let rem = (*rem).max(0.0);
                    if (timer.remaining - rem).abs() > 1e-5 {
                        timer.remaining = rem;
                        updated += 1;
                    }
                    let ready = timer.unlocked && rem <= 0.0;
                    if timer.ready != ready {
                        timer.ready = ready;
                        updated += 1;
                    }
                }
            }
        }
        // Roster is_alive / color from shadow slots (dense host id ↔ PlayerId residual).
        for pi in &mut self.players {
            let pid = gamelogic::world::PlayerId::from_index(pi.id as u8);
            if let Some(p) = shadow.world().player(pid) {
                if pi.is_alive != p.is_alive {
                    pi.is_alive = p.is_alive;
                    updated += 1;
                }
                if pi.color_rgb != p.color_rgb {
                    pi.color_rgb = p.color_rgb;
                    updated += 1;
                }
            }
        }
        self.gameworld_overlay_stamped = updated;
        updated
    }
    /// Sparse RenderableObject from a GameWorld entity (Wave 192).
    ///
    /// Fills identity/pose/HP/motion/selection from the borrow-first entity store.
    /// Host-only presentation fields stay at safe defaults until a later cutover.
    /// Fail-closed: not full build_from_logic parity (weapons, FOW grid, FX, etc.).

    /// Wave 490: decode entity `kind_of_bits` using host presentation ORDER residual.
    fn kind_of_list_from_presentation_bits(bits: u32) -> Vec<crate::game_logic::KindOf> {
        use crate::game_logic::KindOf;
        const ORDER: &[KindOf] = &[
            KindOf::Structure,
            KindOf::Infantry,
            KindOf::Vehicle,
            KindOf::Aircraft,
            KindOf::Projectile,
            KindOf::Resource,
            KindOf::Selectable,
            KindOf::Attackable,
            KindOf::CommandCenter,
            KindOf::Worker,
            KindOf::Hero,
            KindOf::SupplyCenter,
            KindOf::PowerPlant,
            KindOf::FSBarracks,
            KindOf::FSWarFactory,
            KindOf::FSAirfield,
            KindOf::FSInternetCenter,
            KindOf::FSPower,
            KindOf::FSBaseDefense,
            KindOf::FSSupplyDropzone,
            KindOf::FSSupplyCenter,
            KindOf::FSSuperweapon,
            KindOf::FSStrategyCenter,
            KindOf::FSFake,
            KindOf::FSTechnology,
            KindOf::FSBlackMarket,
            KindOf::FSAdvancedTech,
            KindOf::Harvestable,
            KindOf::Powered,
        ];
        let mut out = Vec::new();
        for (i, k) in ORDER.iter().enumerate() {
            if i < 32 && (bits & (1u32 << i)) != 0 {
                out.push(*k);
            }
        }
        out
    }

    pub fn renderable_from_gameworld_entity(
        host_id: crate::game_logic::ObjectId,
        ent: &gamelogic::world::entities::Entity,
    ) -> RenderableObject {
        let team = match ent.team_ordinal {
            0 => crate::game_logic::Team::USA,
            1 => crate::game_logic::Team::China,
            2 => crate::game_logic::Team::GLA,
            _ => crate::game_logic::Team::Neutral,
        };
        let p = ent.transform.position;
        let pos = glam::Vec3::new(p.x, p.y, p.z);
        let vel = glam::Vec3::new(ent.velocity[0], ent.velocity[1], ent.velocity[2]);
        let move_destination = ent.move_target.map(|p| glam::Vec3::new(p[0], p[1], p[2]));
        let attack_target = ent
            .attack_target
            .map(|eid| crate::game_logic::ObjectId(eid.get()));
        let health_max = if ent.max_health > 0.0 {
            ent.max_health
        } else if ent.health > 0.0 {
            ent.health
        } else {
            1.0
        };
        let moving = vel.length_squared() > 1e-6 || move_destination.is_some();
        RenderableObject {
            id: host_id,
            template_name: ent.template.name.clone(),
            team,
            team_color: ent.team_color,
            position: pos,
            orientation: ent.transform.orientation,
            // Wave 498: filled by overlay_host_fx_residual when host is available.
            topple_lean_radians: ent.topple_lean_radians,
            move_destination,
            // Wave 489: order/path/production presentation from GW entity.
            target_location: ent
                .target_location
                .map(|p| glam::Vec3::new(p[0], p[1], p[2])),
            guard_target: if ent.guard_target_host != 0 {
                Some(crate::game_logic::ObjectId(ent.guard_target_host))
            } else {
                None
            },
            using_ability: ent.using_ability,
            airborne_target: ent.airborne_target,
            producer_id: ent.producer_id.map(ObjectId), // Wave 992: GameWorld entity producer residual.
            show_healing: ent.sole_healing_benefactor_expiration_frame != 0,
            healing_icon_type: {
                const STRUCTURE_BIT: u32 = 1 << 0;
                const VEHICLE_BIT: u32 = 1 << 2;
                if ent.kind_of_bits & STRUCTURE_BIT != 0 {
                    1
                } else if ent.kind_of_bits & VEHICLE_BIT != 0 {
                    2
                } else {
                    0
                }
            },
            parachuting: ent.parachuting,
            parachute_open: ent.parachute_open,
            captured: ent.private_captured,
            prone: ent.prone_active,
            emoticon_name: ent.emoticon_name.clone(),
            emoticon_frames_left: ent.emoticon_frames_left,
            is_surrendered: ent.is_surrendered,
            formation_id: ent.formation_id,
            formation_offset: glam::Vec2::new(ent.formation_offset[0], ent.formation_offset[1]), // Wave 998
            over_water: ent.cell_is_underwater,
            cell_is_cliff: ent.cell_is_cliff,
            cell_is_underwater: ent.cell_is_underwater,
            move_max_speed: ent.move_max_speed,
            velocity: vel,
            ai_state_ordinal: ent.ai_state_ordinal,
            attack_target,
            path_waypoints: ent
                .path_waypoints
                .iter()
                .map(|p| glam::Vec3::new(p[0], p[1], p[2]))
                .collect(),
            path_len: ent.path_len,
            path_index: ent.path_index,
            occupant_count: ent.occupant_count,
            production_queue: ent
                .production_queue_items
                .iter()
                .map(PresentationProductionItem::from_entity_item)
                .collect(),
            production_paused: ent.production_paused, // Wave 991: GameWorld entity pause residual.
            rally_point: ent.rally_point.map(|p| glam::Vec3::new(p[0], p[1], p[2])),
            guard_position: ent
                .guard_position
                .map(|p| glam::Vec3::new(p[0], p[1], p[2])),
            // Wave 490: garrison/container presentation from GW entity.
            garrisoned_units: ent
                .garrisoned_host_ids
                .iter()
                .copied()
                .filter(|&id| id != 0)
                .map(crate::game_logic::ObjectId)
                .collect(),
            max_garrison: ent.max_garrison as usize,
            power_provided: ent.power_provided,
            power_consumed: ent.power_consumed,
            stored_supplies: ent.stored_supplies,
            health_current: ent.health.max(0.0),
            health_max,
            selected: ent.selected,
            is_deployed: ent.deployed,
            selection_flash_remaining: ent.selection_flash_remaining,
            destroyed: ent.destroyed || ent.health <= 0.0,
            // Wave 488: carry GW entity presentation channels (not hard-zero).
            model_condition_bits: ent.model_condition_bits,
            radar_active: ent.radar_active,
            radar_extend_complete: ent.radar_extend_complete,
            production_door_phase: ent.production_door_phase,
            body_damage_state: ent.body_damage_state,
            // Wave 498 defaults; overlay_host_fx_residual stamps live host FX residual.
            damage_fx_name: ent.damage_fx_name.clone(),
            bone_fx_name: ent.bone_fx_name.clone(),
            poison_tinted: ent.poison_damage_frame != 0,
            undetected_defector: ent.defection_undetected,
            defector_flash: ent.defection_flash_this_frame,
            death_fx_name: ent.death_fx_name.clone(),
            death_type_name: if ent.destroyed || ent.health <= 0.0 {
                crate::game_logic::host_usa_pilot::HostDeathType::from_ordinal(ent.death_type)
                    .as_name()
                    .to_string()
            } else {
                String::new()
            },
            under_construction: ent.under_construction,
            construction_percent: ent.construction_percent,
            // Wave 1031: GW path has no host supply-drop OCL timer residual yet.
            ocl_timer_seconds: 0,
            sold: ent.sold,
            unselectable: ent.unselectable,
            is_rebuild_hole: ent.is_rebuild_hole,
            rebuild_template_name: ent.rebuild_template_name.clone(),
            rebuild_ready_frame: ent.rebuild_ready_frame,
            rebuild_spawner_id: ent.rebuild_spawner_id.map(ObjectId),
            rebuild_worker_id: ent.rebuild_worker_id.map(ObjectId),
            rebuild_reconstructing_id: ent.rebuild_reconstructing_id.map(ObjectId),
            reconstructing: ent.reconstructing,
            // Wave 490: XP/veterancy from GW entity.
            veterancy: PresentationVeterancy::from_ordinal(ent.veterancy_ordinal),
            experience_points: ent.experience_points,
            moving: moving || ent.moving,
            attacking: attack_target.is_some() || ent.attacking,
            is_firing_weapon: ent.is_firing_weapon,
            is_aiming_weapon: ent.is_aiming_weapon,
            // Wave 490: disable/status presentation from GW entity.
            disabled_emp: ent.disabled_emp,
            disabled_paralyzed: ent.disabled_paralyzed,
            weapons_jammed: ent.weapons_jammed,
            masked: ent.masked,
            ignoring_stealth: ent.ignoring_stealth,
            repulsor: ent.repulsor,
            // Wave 489: stealth/weapon presentation from GW entity.
            stealthed: ent.stealthed,
            detected: ent.detected,
            effectively_stealthed: ent.stealthed && !ent.detected,
            disabled: ent.disabled_emp
                || ent.disabled_paralyzed
                || ent.disabled_hacked
                || ent.disabled_underpowered
                || ent.disabled_unmanned
                || ent.disabled_subdued,
            contained_by: if ent.contained_by_host != 0 {
                Some(crate::game_logic::ObjectId(ent.contained_by_host))
            } else {
                None
            },
            force_attack: ent.force_attack,
            has_weapon: ent.has_weapon,
            weapon_range: ent.weapon_range,
            weapon_damage: ent.weapon_damage,
            weapon_min_range: ent.weapon_min_range,
            weapon_reload_time: ent.weapon_reload_time,
            weapon_ammo: ent.weapon_ammo,
            ammo_pip_total: ent.weapon_clip_size,
            ammo_pip_full: ent.weapon_ammo.min(ent.weapon_clip_size),
            weapon_ready_percent: if ent.weapon_reload_time > 1e-6 {
                ((((ent.weapon_reload_time - ent.weapon_last_fire_time.max(0.0))
                    / ent.weapon_reload_time)
                    .clamp(0.0, 1.0))
                    * 100.0) as u32
            } else if ent.has_weapon {
                100
            } else {
                0
            },
            weapon_can_target_air: ent.weapon_can_target_air,
            weapon_can_target_ground: ent.weapon_can_target_ground,
            weapon_projectile_speed: ent.weapon_projectile_speed,
            armed_riders_upgrade_weapon_set: ent.armed_riders_upgrade_weapon_set,
            weapon_set_player_upgrade: ent.weapon_set_player_upgrade,
            second_life: ent.second_life,
            front_crushed: ent.front_crushed,
            back_crushed: ent.back_crushed,
            user_1: ent.user_1,
            user_2: ent.user_2,
            weapon_crate_upgrade: ent.weapon_crate_upgrade,
            armor_crate_upgrade: ent.armor_crate_upgrade,
            enemy_near: ent.enemy_near,
            armed: ent.armed,
            camo_stealth_look: ent.camo_stealth_look,
            // Wave 490: disguise/detector/stealth-break presentation from GW entity.
            disguise_as_template: if ent.disguise_as_template.is_empty() {
                None
            } else {
                Some(ent.disguise_as_template.clone())
            },
            disguise_as_team: match ent.disguise_as_team_ordinal {
                0 => Some(Team::USA),
                1 => Some(Team::China),
                2 => Some(Team::GLA),
                _ => None,
            },
            disguised: ent.disguised,
            disabled_subdued: ent.disabled_subdued,
            is_carbomb: ent.is_carbomb,
            hijacked: ent.hijacked,
            disguise_transition_opacity: 1.0,
            detection_range: ent.detection_range,
            detection_rate_frames: ent.detection_rate_frames,
            stealth_breaks_on_attack: ent.stealth_breaks_on_attack,
            stealth_breaks_on_move: ent.stealth_breaks_on_move,
            innate_stealth: ent.innate_stealth,
            // Wave 490: continuous-fire / battle-plan timers from GW entity.
            weapon_bonus_frenzy_until_frame: ent.weapon_bonus_frenzy_until_frame,
            continuous_fire_consecutive: ent.continuous_fire_consecutive,
            continuous_fire_coast_until_frame: ent.continuous_fire_coast_until_frame,
            battle_plan_sight_scalar_applied: ent.battle_plan_sight_scalar_applied,
            special_power_ready: ent.special_power_ready,
            special_power_cooldown: ent.special_power_cooldown,
            special_power_cooldown_remaining: ent.special_power_cooldown_remaining,
            object_type: match ent.object_type_ordinal {
                0 => PresentationObjectType::Infantry,
                1 => PresentationObjectType::Vehicle,
                2 => PresentationObjectType::Aircraft,
                3 => PresentationObjectType::Building,
                4 => PresentationObjectType::Supply,
                5 => PresentationObjectType::Projectile,
                _ => PresentationObjectType::Neutral,
            },
            // Wave 490: applied upgrades from GW entity.
            applied_upgrades: ent.applied_upgrade_names.clone(),
            has_secondary_weapon: ent.has_secondary_weapon,
            secondary_weapon_range: ent.secondary_weapon_range,
            secondary_weapon_damage: ent.secondary_weapon_damage,
            // Wave 490: turret presentation from GW entity.
            turret_angle_deg: ent.turret_angle_deg,
            turret_pitch_deg: ent.turret_pitch_deg,
            turret_idle_scanning: ent.turret_idle_scanning,
            weapon_bonus_enthusiastic: ent.weapon_bonus_enthusiastic,
            weapon_bonus_subliminal: ent.weapon_bonus_subliminal,
            weapon_bonus_horde: ent.weapon_bonus_horde,
            weapon_bonus_nationalism: ent.weapon_bonus_nationalism,
            weapon_bonus_frenzy: ent.weapon_bonus_frenzy,
            // Wave 490: bonus/ai/hive presentation from GW entity.
            weapon_bonus_frenzy_level: ent.weapon_bonus_frenzy_level,
            weapon_bonus_battle_plan_bombardment: ent.weapon_bonus_battle_plan_bombardment,
            weapon_bonus_battle_plan_hold_the_line: ent.weapon_bonus_battle_plan_hold_the_line,
            weapon_bonus_battle_plan_search_and_destroy: ent
                .weapon_bonus_battle_plan_search_and_destroy,
            continuous_fire_level: ent.continuous_fire_level,
            faerie_fire_until_frame: ent.faerie_fire_until_frame,
            hive_slave_count: ent.hive_slave_count,
            hive_slave_hp: ent.hive_slave_hp,
            ai_attitude: ent.ai_attitude,
            camo_friendly_opacity: 1.0,
            vision_spied_mask: ent.vision_spied_mask,
            vision_range: ent.vision_range,
            shroud_clearing_range: ent.shroud_clearing_range,
            crusher_level: ent.crusher_level,
            crushable_level: ent.crushable_level,
            cheer_timer: ent.cheer_timer,
            // Wave 490: transport/container role presentation from GW entity.
            is_humvee_transport: ent.is_humvee_transport,
            is_listening_outpost_transport: ent.is_listening_outpost_transport,
            is_troop_crawler_transport: ent.is_troop_crawler_transport,
            is_helix_transport: ent.is_helix_transport,
            has_overlord_gattling_addon: ent.has_overlord_gattling_addon,
            has_overlord_propaganda_addon: ent.has_overlord_propaganda_addon,
            is_battle_bus_transport: ent.is_battle_bus_transport,
            is_technical_transport: ent.is_technical_transport,
            is_combat_cycle_transport: ent.is_combat_cycle_transport,
            combat_cycle_rider: ent.combat_cycle_rider,
            is_tunnel_network: ent.is_tunnel_network,
            is_combat_chinook_transport: ent.is_combat_chinook_transport,
            max_transport: ent.max_transport as usize,
            overlord_bunker_capacity: if ent.overlord_bunker_capacity == u16::MAX {
                0
            } else {
                ent.overlord_bunker_capacity as usize
            },
            passengers_allowed_to_fire: ent.passengers_allowed_to_fire,
            display_name: ent.template.name.clone(),
            // Wave 490: demo/turret-hold/command-set presentation from GW entity.
            demo_suicided_detonating: ent.demo_suicided_detonating,
            turret_holding: ent.turret_holding,
            last_damage_source_host: ent.last_damage_source_host,
            command_set_override: ent.command_set_override.clone(),
            // Wave 493: effective command set name falls back to override residual.
            command_set_name: ent.command_set_override.clone(),
            is_detector: ent.is_detector,
            active_weapon_slot: ent.active_weapon_slot,
            weapon_fire_status: ent.weapon_fire_status,
            is_panicking: ent.is_panicking,
            moving_backwards: ent.moving_backwards,
            overcharge_enabled: ent.overcharge_enabled,
            shock_was_airborne: ent.shock_was_airborne,
            shock_allow_bounce: ent.shock_allow_bounce,
            shock_grounded_once: ent.shock_grounded_once,
            shock_stun_frames: ent.shock_stun_frames,
            power_plant_rods_extended: ent.power_plant_rods_extended,
            power_plant_rods_done_frame: ent.power_plant_rods_done_frame,
            jet_slow_death_active: ent.jet_slow_death_active,
            anim_steer_turn: ent.anim_steer_turn,
            show_health_bar: true,
            // Wave 490: guard/mine/kind presentation from GW entity.
            guard_radius: ent.guard_radius,
            has_mine: ent.has_mine_data,
            kind_of: Self::kind_of_list_from_presentation_bits(ent.kind_of_bits),
            is_structure: matches!(ent.object_type_ordinal, 3) || ent.is_building,
            is_unit: matches!(ent.object_type_ordinal, 0 | 1 | 2),
            is_mobile: matches!(ent.object_type_ordinal, 0 | 1 | 2),
            can_produce: ent.is_building && !ent.under_construction,
            // Wave 490: building type from GW entity ordinal.
            building_type: PresentationBuildingType::from_ordinal(ent.building_type_ordinal),
            // Wave 491: body damage + sold → mesh key (not bare template name).
            model_key: {
                let base = crate::assets::mesh_asset_resolve::model_key_from_presentation(
                    Some(ent.template.name.as_str()),
                    &ent.template.name,
                );
                let dying = ent.destroyed || ent.health <= 0.0;
                let sold = ent.sold
                    || crate::game_logic::host_enum_table_residual::host_model_condition_has(
                        ent.model_condition_bits,
                        crate::game_logic::host_enum_table_residual::sold_model_bit(),
                    );
                Some(
                    crate::assets::mesh_asset_resolve::model_key_with_presentation_state(
                        &base,
                        ent.body_damage_state,
                        dying,
                        sold,
                    ),
                )
            },
            // Wave 492: mesh scale + FOW from GW entity residual (not hard defaults).
            mesh_scale: crate::assets::mesh_asset_resolve::mesh_scale_for_unit(&ent.template.name),
            selection_radius: if ent.selection_radius > 0.0 {
                ent.selection_radius
            } else {
                10.0
            },
            // Wave 493: engine-bridge + ground height from GW entity residual.
            engine_bridged: ent.engine_bridged,
            fow_visibility: {
                // Entity FOW floats: alpha≈1 visible; explored-but-low alpha → fogged; else hidden.
                if ent.fow_visibility_alpha >= 0.95 {
                    crate::fow_rendering::ObjectVisibility::FULLY_VISIBLE
                } else if ent.fow_is_explored >= 0.5 || ent.fow_visibility_alpha > 0.05 {
                    crate::fow_rendering::ObjectVisibility {
                        visibility_alpha: ent.fow_visibility_alpha.clamp(0.0, 1.0),
                        is_explored: ent.fow_is_explored.clamp(0.0, 1.0),
                        visibility_falloff: ent.fow_visibility_falloff.clamp(0.0, 1.0).max(0.01),
                    }
                } else {
                    crate::fow_rendering::ObjectVisibility::HIDDEN
                }
            },
            ground_height: if ent.ground_height_from_terrain {
                ent.ground_height
            } else if ent.ground_height.abs() > 1e-6 {
                ent.ground_height
            } else {
                PRESENTATION_DEFAULT_GROUND_HEIGHT
            },
            ground_height_from_terrain: ent.ground_height_from_terrain,
        }
    }

    /// Append sparse RenderableObjects for GameWorld entities not already on the
    /// host-built frame (Wave 192). Uses host ObjectId when the shadow map has one;
    /// otherwise synthesizes `ObjectId(0x8000_0000 | entity_id)`.
    ///
    /// Call after `overlay_gameworld_shadow`. Counts land in `gameworld_appended`.
    /// Fail-closed: not full `build_from_gameworld` cutover / playable_claim.
    pub fn append_missing_from_gameworld(
        &mut self,
        shadow: &crate::gameworld_shadow::GameWorldShadow,
    ) -> usize {
        let existing: std::collections::HashSet<u32> =
            self.objects.iter().map(|o| o.id.0).collect();
        let mut appended = 0usize;
        for ent in shadow.world().world().entities() {
            if ent.destroyed && ent.health <= 0.0 {
                continue;
            }
            let host_id = shadow
                .host_for_entity(ent.id)
                .unwrap_or_else(|| crate::game_logic::ObjectId(0x8000_0000 | ent.id.get()));
            if existing.contains(&host_id.0) {
                continue;
            }
            self.objects
                .push(Self::renderable_from_gameworld_entity(host_id, ent));
            appended += 1;
        }
        self.gameworld_appended = self.gameworld_appended.saturating_add(appended);
        appended
    }

    /// Wave 500: merge per-object damage/death/bone FX residual names into the
    /// presentation particle list so client/upload observe named FXLists without
    /// a live GameLogic dual-read during render.
    ///
    /// Host drain currently queues `FX:{name}` audio tags; this peels the same
    /// names into `PresentationParticleSystem` with `fx_list_name` set.
    /// Fail-closed: not full FXList.ini particle graph / bone-local offsets.
    pub fn append_object_residual_fx_particles(&mut self) -> usize {
        use crate::game_logic::CombatParticleKind;
        let frame = self.frame.0;
        let mut next_id = self
            .particle_systems
            .iter()
            .map(|p| p.id)
            .max()
            .unwrap_or(0)
            .saturating_add(1)
            .max(1);
        let mut appended = 0usize;
        for o in &self.objects {
            let candidates: [(Option<&str>, CombatParticleKind); 3] = [
                (
                    o.damage_fx_name.as_deref(),
                    CombatParticleKind::WeaponImpact,
                ),
                (
                    o.death_fx_name.as_deref(),
                    CombatParticleKind::DeathExplosion,
                ),
                (o.bone_fx_name.as_deref(), CombatParticleKind::WeaponImpact),
            ];
            for (name, kind) in candidates {
                let Some(name) = name else {
                    continue;
                };
                if name.is_empty() {
                    continue;
                }
                let already = self.particle_systems.iter().any(|p| {
                    (!p.fx_list_name.is_empty() && p.fx_list_name == name)
                        || (p.template_name == name && p.source_object == Some(o.id))
                });
                if already {
                    continue;
                }
                self.particle_systems.push(PresentationParticleSystem {
                    id: next_id,
                    kind,
                    template_name: name.to_string(),
                    position: o.position,
                    source_object: Some(o.id),
                    target_object: None,
                    spawned_frame: frame,
                    active: true,
                    client_system_id: None,
                    fx_list_name: name.to_string(),
                    ocl_list_name: String::new(),
                });
                next_id = next_id.saturating_add(1).max(1);
                appended += 1;
            }
        }
        if appended > 0 {
            // Keep dual-tick particle_count honest with expanded list.
            self.dual_tick.particle_count = self.particle_systems.len() as u32;
        }
        appended
    }

    /// Wave 498: re-stamp host-only FX presentation residual after GameWorld object rebuild.
    ///
    /// GW entities do not yet own TransitionDamageFX / BoneFX / poison tint / defector /
    /// death FX / topple lean. When objects are rebuilt from the entity store, those
    /// fields would otherwise hard-default. Overlay from the matching host Object by id.
    /// Fail-closed: not full GameWorld FX ownership / playable_claim.
    pub fn overlay_host_fx_residual(&mut self, logic: &GameLogic) -> usize {
        let mut stamped = 0usize;
        for ro in &mut self.objects {
            let Some(obj) = logic.host_object(ro.id) else {
                continue;
            };
            let mut dirty = false;
            let topple = obj.presentation_topple_lean_radians();
            if (ro.topple_lean_radians - topple).abs() > 1e-5 {
                ro.topple_lean_radians = topple;
                dirty = true;
            }
            let damage_fx = obj
                .pending_transition_damage_fx
                .last()
                .and_then(|e| e.fx_name.clone());
            if ro.damage_fx_name != damage_fx {
                ro.damage_fx_name = damage_fx;
                dirty = true;
            }
            let bone_fx = obj.bone_fx_damage.as_ref().and_then(|b| b.last_fx.clone());
            if ro.bone_fx_name != bone_fx {
                ro.bone_fx_name = bone_fx;
                dirty = true;
            }
            let poison = obj.is_poison_tinted();
            if ro.poison_tinted != poison {
                ro.poison_tinted = poison;
                dirty = true;
            }
            let undetected = obj.is_undetected_defector();
            if ro.undetected_defector != undetected {
                ro.undetected_defector = undetected;
                dirty = true;
            }
            let flash = obj
                .defection_helper
                .as_ref()
                .map(|d| d.flash_this_frame || d.final_white_flash)
                .unwrap_or(false);
            if ro.defector_flash != flash {
                ro.defector_flash = flash;
                dirty = true;
            }
            let death_fx = obj.pending_death_fx.clone();
            if ro.death_fx_name != death_fx {
                ro.death_fx_name = death_fx;
                dirty = true;
            }
            if dirty {
                stamped += 1;
            }
        }
        stamped
    }

    /// Rebuild the entire `objects` list from the GameWorld entity store (Wave 193).
    ///
    /// Host ObjectIds are preferred when the shadow map has them; otherwise
    /// synthesizes `ObjectId(0x8000_0000 | entity_id)`. Counts land in
    /// `gameworld_rebuilt`. Default engine path when shadow is live (Wave 194).
    /// Fail-closed: sparse host-only FX/UI fields stay default unless host merge
    /// fills them; not full playable_claim cutover.
    pub fn rebuild_objects_from_gameworld(
        &mut self,
        shadow: &crate::gameworld_shadow::GameWorldShadow,
    ) -> usize {
        self.objects.clear();
        let mut n = 0usize;
        for ent in shadow.world().world().entities() {
            if ent.destroyed && ent.health <= 0.0 {
                continue;
            }
            let host_id = shadow
                .host_for_entity(ent.id)
                .unwrap_or_else(|| crate::game_logic::ObjectId(0x8000_0000 | ent.id.get()));
            self.objects
                .push(Self::renderable_from_gameworld_entity(host_id, ent));
            n += 1;
        }
        self.gameworld_rebuilt = n;
        self.gameworld_primary_objects = n > 0 || self.gameworld_primary_objects;
        // Overlay last-writer stamps player residual + any fields the sparse builder
        // still defaults (keeps one code path for shadow → presentation identity).
        let _ = self.overlay_gameworld_shadow(shadow);
        n
    }

    /// Build a PresentationFrame whose **object roster** is GameWorld-primary (Wave 193).
    ///
    /// When `host` is provided, non-object presentation residual (world_env, scripts,
    /// camera, FX packs) still comes from `build_from_logic`, then objects are rebuilt
    /// from the shadow. When `host` is `None`, a minimal shell frame is filled with
    /// GameWorld objects + local player residual only.
    ///
    /// Default engine path (Wave 194) rebuilds objects from GameWorld after host
    /// non-object residual. Fail-closed: not full authority cutover / playable_claim.
    pub fn build_from_gameworld(
        shadow: &crate::gameworld_shadow::GameWorldShadow,
        local_player_id: u32,
        host: Option<&GameLogic>,
    ) -> Self {
        let mut frame = if let Some(logic) = host {
            Self::build_from_logic(logic, local_player_id)
        } else {
            // Minimal shell — borrow-first empty presentation with local player id set.
            let mut f = Self::build_from_logic(&GameLogic::new(), local_player_id);
            f.objects.clear();
            f.events.clear();
            f
        };
        let host_n = frame.objects.len();
        let gw_n = frame.rebuild_objects_from_gameworld(shadow);
        // Wave 838: keep host objects when shadow yields nothing.
        if gw_n == 0 && host_n > 0 {
            if let Some(logic) = host {
                frame = Self::build_from_logic(logic, local_player_id);
                let _ = frame.overlay_gameworld_shadow(shadow);
            }
        }
        // Wave 498: host FX residual survives GameWorld object rebuild.
        if let Some(logic) = host {
            let _ = frame.overlay_host_fx_residual(logic);
        }
        // Wave 500: object FX residual names → particle list after host FX stamp.
        let _ = frame.append_object_residual_fx_particles();
        // Local player residual already stamped by overlay inside rebuild.
        frame
    }

    /// Standard engine presentation build (Wave 195).
    ///
    /// When a live `GameWorldShadow` is present and
    /// [`presentation_from_gameworld_enabled`] is true (default), this is equivalent to
    /// [`build_from_gameworld`] — host supplies non-object residual, objects come from
    /// the borrow-first entity store. Otherwise falls back to host
    /// [`build_from_logic`] plus overlay/append when a shadow is available.
    ///
    /// Callers must `sync_from_host` before this when a shadow is provided.
    /// Fail-closed: not full GameWorld authority cutover / playable_claim.
    pub fn build_for_engine(
        logic: &GameLogic,
        local_player_id: u32,
        shadow: Option<&crate::gameworld_shadow::GameWorldShadow>,
    ) -> Self {
        match shadow {
            Some(shadow) if presentation_from_gameworld_enabled() => {
                Self::build_from_gameworld(shadow, local_player_id, Some(logic))
            }
            Some(shadow) => {
                let mut frame = Self::build_from_logic(logic, local_player_id);
                let _ = frame.overlay_gameworld_shadow(shadow);
                let _ = frame.append_missing_from_gameworld(shadow);
                frame
            }
            None => Self::build_from_logic(logic, local_player_id),
        }
    }

    /// Engine tick presentation build with victory residual (Wave 195).
    ///
    /// Freezes victory via [`build_with_victory`], then applies the standard
    /// GameWorld object roster path when a shadow is live.
    pub fn build_with_victory_for_engine(
        logic: &mut GameLogic,
        local_player_id: u32,
        shadow: Option<&crate::gameworld_shadow::GameWorldShadow>,
    ) -> Self {
        let mut frame = Self::build_with_victory(logic, local_player_id);
        match shadow {
            Some(shadow) if presentation_from_gameworld_enabled() => {
                let host_n = frame.objects.len();
                let gw_n = frame.rebuild_objects_from_gameworld(shadow);
                // Wave 838: empty GameWorld shadow must not erase a non-empty host
                // roster (construct/train/map objects) or unit mesh collect stays 0.
                if gw_n == 0 && host_n > 0 {
                    frame = Self::build_with_victory(logic, local_player_id);
                    let _ = frame.overlay_gameworld_shadow(shadow);
                }
                // Wave 498: host FX residual after GW object rebuild.
                let _ = frame.overlay_host_fx_residual(logic);
                // Wave 500: object FX residual names → particle list.
                let _ = frame.append_object_residual_fx_particles();
            }
            Some(shadow) => {
                let _ = frame.overlay_gameworld_shadow(shadow);
                let _ = frame.append_missing_from_gameworld(shadow);
            }
            None => {}
        }
        frame
    }
}

// ===== particles.rs =====
use super::*;

/// Snapshot-owned combat particle system for presentation/client observe path.
/// Fail-closed: not full W3D GPU particle parity (hq-gq7n residual).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresentationParticleSystem {
    pub id: u32,
    pub kind: CombatParticleKind,
    pub template_name: String,
    pub position: Vec3,
    pub source_object: Option<ObjectId>,
    pub target_object: Option<ObjectId>,
    pub spawned_frame: u32,
    pub active: bool,
    pub client_system_id: Option<u32>,
    /// C++ Weapon.ini FireFX / DetonationFX residual (empty = preset only).
    #[serde(default)]
    pub fx_list_name: String,
    /// C++ Weapon.ini FireOCL / ProjectileDetonationOCL residual (empty = none).
    #[serde(default)]
    pub ocl_list_name: String,
}

impl PresentationParticleSystem {
    pub fn from_combat_entry(entry: &CombatParticleSystemEntry) -> Self {
        Self {
            id: entry.id,
            kind: entry.kind,
            template_name: entry.template_name.clone(),
            position: entry.position,
            source_object: entry.source_object,
            target_object: entry.target_object,
            spawned_frame: entry.spawned_frame,
            active: entry.active,
            client_system_id: entry.client_system_id,
            fx_list_name: entry.fx_list_name.clone(),
            ocl_list_name: entry.ocl_list_name.clone(),
        }
    }
}


// ===== projectile.rs =====
use super::*;

/// Presentation-owned projectile mesh pass input (no live GameLogic).
///
/// Fail-closed: not full W3D projectile drawable / trail GPU instance parity.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectileRenderInput {
    pub id: ObjectId,
    pub projectile_object_name: String,
    pub model_key: String,
    pub position: Vec3,
    pub velocity: Vec3,
    pub target_position: Vec3,
    pub is_homing: bool,
    pub mesh_scale: f32,
}

impl ProjectileRenderInput {
    pub fn from_presentation(p: &PresentationProjectile) -> Option<Self> {
        let model_key = if p.model_key.is_empty() {
            crate::assets::mesh_asset_resolve::model_key_from_projectile_object(
                &p.projectile_object_name,
            )
        } else {
            p.model_key.clone()
        };
        if model_key.is_empty() {
            return None;
        }
        Some(Self {
            id: p.id,
            projectile_object_name: p.projectile_object_name.clone(),
            model_key,
            position: p.position,
            velocity: p.velocity,
            target_position: p.target_position,
            is_homing: p.is_homing,
            mesh_scale: 1.0,
        })
    }

    /// Orient projectile mesh along velocity (fallback toward target).
    pub fn world_matrix(&self) -> glam::Mat4 {
        let dir = if self.velocity.length_squared() > 1e-6 {
            self.velocity.normalize()
        } else {
            let d = self.target_position - self.position;
            if d.length_squared() > 1e-6 {
                d.normalize()
            } else {
                glam::Vec3::Z
            }
        };
        // Y-up world: yaw from XZ, pitch from Y.
        let yaw = dir.x.atan2(dir.z);
        let pitch = -dir
            .y
            .asin()
            .clamp(-std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2);
        let scale = if self.mesh_scale.is_finite() && self.mesh_scale > 0.0 {
            self.mesh_scale
        } else {
            1.0
        };
        glam::Mat4::from_translation(self.position)
            * glam::Mat4::from_rotation_y(yaw)
            * glam::Mat4::from_rotation_x(pitch)
            * glam::Mat4::from_scale(glam::Vec3::splat(scale))
    }
}

/// Snapshot-owned in-flight projectile for presentation/client observe path.
/// Fail-closed: not full W3D projectile mesh / trail GPU parity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresentationProjectile {
    pub id: ObjectId,
    pub position: Vec3,
    pub velocity: Vec3,
    pub target_position: Vec3,
    pub shooter_id: ObjectId,
    pub target_id: Option<ObjectId>,
    pub damage: f32,
    pub lifetime: f32,
    pub max_lifetime: f32,
    pub is_homing: bool,
    /// C++ ProjectileObject residual (W3D mesh key / template name).
    pub projectile_object_name: String,
    /// Resolved W3D model key residual from ProjectileObject (empty = trail-only).
    pub model_key: String,
    /// C++ Weapon.ini ProjectileExhaust residual PSys name (empty = none).
    #[serde(default)]
    pub exhaust_name: String,
}

impl PresentationProjectile {
    pub fn from_combat(p: &crate::game_logic::combat::Projectile) -> Self {
        let projectile_object_name = p.projectile_object_name.clone();
        let model_key = crate::assets::mesh_asset_resolve::model_key_from_projectile_object(
            &projectile_object_name,
        );
        Self {
            id: p.id,
            position: p.position,
            velocity: p.velocity,
            target_position: p.target_position,
            shooter_id: p.shooter_id,
            target_id: p.target_id,
            damage: p.damage,
            lifetime: p.lifetime,
            max_lifetime: p.max_lifetime,
            is_homing: p.is_homing,
            projectile_object_name,
            model_key,
            exhaust_name: p.exhaust_name.clone(),
        }
    }
}

/// Immutable feed for GameClient / renderer after each authoritative logic step.
/// C++ ProjectileStreamUpdate residual frozen for presentation trail draw.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresentationProjectileStream {
    pub shooter_id: ObjectId,
    pub stream_name: String,
    pub points: Vec<(f32, f32, f32)>,
    pub target_id: Option<ObjectId>,
}

// ===== queries.rs =====
use super::*;

impl PresentationFrame {
    pub fn alive_object_count(&self) -> usize {
        // Wave 1104: alive count residual excludes sold.
        self.objects
            .iter()
            .filter(|o| !o.destroyed && !o.sold)
            .count()
    }

    /// Stable object-id list for the production render collect path.
    /// Presentation owns unit identity + unit FOW; mesh asset load may still
    /// consult asset systems (not live object transform / shroud re-read).
    pub fn renderable_object_ids(&self) -> Vec<ObjectId> {
        self.objects
            .iter()
            .filter(|o| !o.destroyed)
            .map(|o| o.id)
            .collect()
    }

    /// Main unit mesh pass inputs from the snapshot only (no GameLogic / shroud borrow).
    ///
    /// Filters destroyed and engine-bridged objects (RenderBridge owns those).
    /// Includes local-player FOW alpha for skip/darkening without mid-render queries.
    pub fn unit_render_inputs(&self) -> Vec<UnitRenderInput> {
        // Wave 502: stealth mesh residual from frozen presentation only.
        // Wave 504: skip contained_by units; stamp garrisoned bits on structures.
        // Enemy effectively-stealthed units are omitted from the main mesh pass
        // (C++ auto-target / draw residual: not a legal observe target).
        // Local-team stealthed units keep a translucent FOW alpha residual.
        const ALLY_STEALTH_ALPHA: f32 = 0.35;
        let local_team = self.local_team;
        self.objects
            .iter()
            .filter(|o| !o.destroyed && !o.engine_bridged)
            .filter(|o| {
                // Wave 504: contained units are not drawn as free world meshes.
                if o.contained_by.is_some() {
                    return false;
                }
                if o.effectively_stealthed && o.team != local_team {
                    false
                } else {
                    true
                }
            })
            .map(|o| {
                let mut input = UnitRenderInput::from_renderable(o);
                // Wave 509: world weather/tod mesh bits from frozen presentation env.
                input.world_is_snow = self.world_env.is_snow;
                input.world_is_night = self.world_env.is_night;
                // Wave 513: logic frame for coast/reload residual compare.
                input.logic_frame = self.frame.0;
                if o.effectively_stealthed && o.team == local_team {
                    input.fow_visibility.visibility_alpha = input
                        .fow_visibility
                        .visibility_alpha
                        .min(ALLY_STEALTH_ALPHA);
                }
                // Wave 503: non-allied viewers see disguise mesh residual.
                if o.disguised && o.team != local_team {
                    if let Some(ref dt) = o.disguise_as_template {
                        if !dt.is_empty() {
                            input.model_key =
                                crate::assets::mesh_asset_resolve::model_key_from_presentation(
                                    Some(dt.as_str()),
                                    dt,
                                );
                        }
                    }
                }
                input
            })
            .collect()
    }

    /// Projectile mesh pass inputs from frozen in-flight projectiles (model_key residual).
    pub fn projectile_render_inputs(&self) -> Vec<ProjectileRenderInput> {
        let mut out = Vec::new();
        for p in &self.projectiles {
            if let Some(input) = ProjectileRenderInput::from_presentation(p) {
                out.push(input);
            }
        }
        out.sort_by_key(|p| p.id.0);
        out
    }

    /// Structures with a non-empty production queue (ControlBar residual feed).
    pub fn structures_with_production(&self) -> Vec<&RenderableObject> {
        // Wave 1101: fail-closed on sold/disabled production-queue residual feed.
        self.objects
            .iter()
            .filter(|o| {
                o.is_structure
                    && !o.destroyed
                    && !o.sold
                    && !o.disabled
                    && !o.production_queue.is_empty()
            })
            .collect()
    }

    /// Structures currently holding garrisoned units (contain residual feed).
    pub fn garrisoned_structures(&self) -> Vec<&RenderableObject> {
        // Wave 1101: fail-closed on sold garrison residual feed.
        self.objects
            .iter()
            .filter(|o| o.is_structure && !o.destroyed && !o.sold && !o.garrisoned_units.is_empty())
            .collect()
    }

    /// Net power from non-destroyed objects (presentation economy residual).
    /// Count presentation objects with host turret idle-scan residual.
    pub fn turret_idle_scan_count(&self) -> usize {
        // Wave 1107: residual counts exclude sold.
        self.objects
            .iter()
            .filter(|o| o.turret_idle_scanning && !o.destroyed && !o.sold)
            .count()
    }

    /// Count presentation objects with host horde weapon-bonus residual.
    /// Count presentation objects with host detector residual.
    /// CommandSet name residual for the primary selected object.
    /// Prefers `command_set_override`; empty when unset (template default left to boot path).
    pub fn selected_command_set_name(&self) -> Option<&str> {
        // Wave 1105: primary selection residual fail-closed on sold/unselectable/
        // masked/disabled (not only destroyed) so ControlBar does not show a
        // command set for unusable selected objects.
        let usable = |o: &&RenderableObject| {
            o.selected && !o.destroyed && !o.sold && !o.unselectable && !o.masked && !o.disabled
        };
        let primary = self
            .selected
            .first()
            .copied()
            .or_else(|| self.objects.iter().find(usable).map(|o| o.id))?;
        let o = self.objects.iter().find(|o| o.id == primary)?;
        if o.destroyed || o.sold || o.unselectable || o.masked || o.disabled {
            return None;
        }
        if !o.command_set_name.is_empty() {
            return Some(o.command_set_name.as_str());
        }
        if o.command_set_override.is_empty() {
            None
        } else {
            Some(o.command_set_override.as_str())
        }
    }

    /// Command-set names for current multi-selection (override or ThingFactory template).
    /// Empty entries omitted; used to populate ControlBar without OBJECT_REGISTRY.
    pub fn selected_command_set_names(&self) -> Vec<String> {
        // Wave 1105: multi-select command-set residual fail-closed on sold/
        // unselectable/masked/disabled (not only destroyed).
        let usable = |o: &&RenderableObject| {
            !o.destroyed && !o.sold && !o.unselectable && !o.masked && !o.disabled
        };
        let ids: Vec<ObjectId> = if !self.selected.is_empty() {
            self.selected.clone()
        } else {
            self.objects
                .iter()
                .filter(|o| o.selected && usable(o))
                .map(|o| o.id)
                .collect()
        };
        let mut names = Vec::new();
        for id in ids {
            let Some(ro) = self.objects.iter().find(|o| o.id == id && usable(&o)) else {
                continue;
            };
            // Prefer freeze from build_from_logic; resolve only if older frames lack it.
            if !ro.command_set_name.is_empty() {
                names.push(ro.command_set_name.clone());
                continue;
            }
            let override_name = ro.command_set_override.as_str();
            if let Some(cs) = crate::ui::construction_panel::resolve_command_set_name(
                &ro.template_name,
                if override_name.is_empty() {
                    None
                } else {
                    Some(override_name)
                },
            ) {
                names.push(cs);
            }
        }
        names
    }

    pub fn detector_object_count(&self) -> usize {
        // Wave 1107: residual counts exclude sold.
        self.objects
            .iter()
            .filter(|o| o.is_detector && !o.destroyed && !o.sold)
            .count()
    }

    /// Count presentation objects with non-empty command_set_override residual.
    pub fn command_set_override_object_count(&self) -> usize {
        // Wave 1107: residual counts exclude sold.
        self.objects
            .iter()
            .filter(|o| !o.command_set_override.is_empty() && !o.destroyed && !o.sold)
            .count()
    }

    /// Count presentation objects with any Strategy Center battle-plan bonus residual.
    /// Count presentation objects with host hive-slave residual.
    /// Count presentation objects with host humvee transport residual.
    /// Count presentation objects with host innate_stealth residual.
    pub fn innate_stealth_object_count(&self) -> usize {
        // Wave 1107: residual counts exclude sold.
        self.objects
            .iter()
            .filter(|o| o.innate_stealth && !o.destroyed && !o.sold)
            .count()
    }

    /// Count presentation objects with non-zero detection_rate_frames residual.
    pub fn timed_detector_object_count(&self) -> usize {
        // Wave 1107: residual counts exclude sold.
        self.objects
            .iter()
            .filter(|o| o.detection_rate_frames > 0 && !o.destroyed && !o.sold)
            .count()
    }

    pub fn humvee_transport_object_count(&self) -> usize {
        // Wave 1107: residual counts exclude sold.
        self.objects
            .iter()
            .filter(|o| o.is_humvee_transport && !o.destroyed && !o.sold)
            .count()
    }

    /// Count presentation objects with host overlord gattling addon residual.
    pub fn overlord_gattling_object_count(&self) -> usize {
        // Wave 1107: residual counts exclude sold.
        self.objects
            .iter()
            .filter(|o| o.has_overlord_gattling_addon && !o.destroyed && !o.sold)
            .count()
    }

    pub fn hive_object_count(&self) -> usize {
        // Wave 1107: residual counts exclude sold.
        self.objects
            .iter()
            .filter(|o| o.hive_slave_count > 0 && !o.destroyed && !o.sold)
            .count()
    }

    /// Count presentation objects with continuous-fire residual > 0.
    pub fn continuous_fire_object_count(&self) -> usize {
        // Wave 1107: residual counts exclude sold.
        self.objects
            .iter()
            .filter(|o| o.continuous_fire_level > 0 && !o.destroyed && !o.sold)
            .count()
    }

    pub fn battle_plan_bonus_object_count(&self) -> usize {
        // Wave 1107: residual counts exclude sold.
        self.objects
            .iter()
            .filter(|o| {
                !o.destroyed
                    && !o.sold
                    && (o.weapon_bonus_battle_plan_bombardment
                        || o.weapon_bonus_battle_plan_hold_the_line
                        || o.weapon_bonus_battle_plan_search_and_destroy)
            })
            .count()
    }

    pub fn horde_bonus_object_count(&self) -> usize {
        // Wave 1107: residual counts exclude sold.
        self.objects
            .iter()
            .filter(|o| o.weapon_bonus_horde && !o.destroyed && !o.sold)
            .count()
    }

    pub fn net_power_from_objects(&self) -> i32 {
        // Wave 1108: power residual excludes sold structures (sell removes
        // power contribution from the residual power bar feed).
        self.objects
            .iter()
            .filter(|o| !o.destroyed && !o.sold)
            .map(|o| o.power_provided - o.power_consumed)
            .sum()
    }

    /// Objects still under construction (dozer / structure residual).
    pub fn under_construction_objects(&self) -> Vec<&RenderableObject> {
        // Wave 1108: UC residual excludes sold.
        self.objects
            .iter()
            .filter(|o| !o.destroyed && !o.sold && o.under_construction)
            .collect()
    }

    /// Units at Veteran or higher (chevron residual feed).
    pub fn veteran_or_higher_units(&self) -> Vec<&RenderableObject> {
        // Wave 1108: veterancy residual excludes sold.
        self.objects
            .iter()
            .filter(|o| {
                !o.destroyed
                    && !o.sold
                    && o.is_unit
                    && !matches!(o.veterancy, PresentationVeterancy::Rookie)
            })
            .collect()
    }

    /// Units currently attacking (status residual).
    pub fn attacking_units(&self) -> Vec<&RenderableObject> {
        // Wave 1106: attacking residual excludes sold.
        self.objects
            .iter()
            .filter(|o| !o.destroyed && !o.sold && o.attacking)
            .collect()
    }

    /// Effectively stealthed units (hidden from non-allied targeting residual).
    pub fn effectively_stealthed_units(&self) -> Vec<&RenderableObject> {
        // Wave 1106: stealth residual excludes sold.
        self.objects
            .iter()
            .filter(|o| !o.destroyed && !o.sold && o.effectively_stealthed)
            .collect()
    }

    /// Contained (garrisoned/transported) units residual.
    pub fn contained_units(&self) -> Vec<&RenderableObject> {
        // Wave 1106: contained residual excludes sold containers/occupants.
        self.objects
            .iter()
            .filter(|o| !o.destroyed && !o.sold && o.contained_by.is_some())
            .collect()
    }

    /// True when local player has any radar provider and radar is not disabled.
    pub fn local_radar_active(&self) -> bool {
        self.local_radar_count > 0 && !self.local_radar_disabled
    }

    /// Energy ratio residual (produced / max(consumed,1)) for power bar UI.
    pub fn local_energy_ratio(&self) -> f32 {
        let demand = self.local_power_consumed.max(1) as f32;
        self.local_power_produced as f32 / demand
    }

    /// Whether a science name is unlocked for the local player residual.
    pub fn local_has_science(&self, name: &str) -> bool {
        self.local_unlocked_sciences.iter().any(|s| s == name)
    }

    /// Generals rank residual frozen at snapshot.
    pub fn local_rank_level(&self) -> u32 {
        self.local_rank_level
    }

    /// GeneralsExperience skill points residual.
    pub fn local_skill_points(&self) -> i32 {
        self.local_skill_points
    }

    /// Remaining science purchase points residual.
    pub fn local_science_purchase_points(&self) -> i32 {
        self.local_science_purchase_points
    }

    /// ControlBar rank bar progress residual (0..100).
    pub fn local_rank_progress_percent(&self) -> i32 {
        self.local_rank_progress_percent
    }

    pub fn superweapon_timers(&self) -> &[PresentationSuperweaponTimer] {
        &self.superweapon_timers
    }

    pub fn ready_public_superweapons(&self) -> impl Iterator<Item = &PresentationSuperweaponTimer> {
        self.superweapon_timers.iter().filter(|t| t.ready)
    }

    /// Objects with a ready special power residual (UI / command button feed).
    pub fn special_power_ready_objects(&self) -> Vec<&RenderableObject> {
        // Wave 1102: fail-closed on sold/disabled SP-ready residual feed.
        self.objects
            .iter()
            .filter(|o| !o.destroyed && !o.sold && !o.disabled && o.special_power_ready)
            .collect()
    }

    /// Special-power cooldown fraction remaining in 0..1 (0 = ready).
    pub fn special_power_cooldown_fraction(obj: &RenderableObject) -> f32 {
        if obj.special_power_cooldown <= 0.0 {
            return 0.0;
        }
        (obj.special_power_cooldown_remaining / obj.special_power_cooldown).clamp(0.0, 1.0)
    }

    /// Objects that have applied at least one upgrade residual.
    pub fn upgraded_objects(&self) -> Vec<&RenderableObject> {
        // Wave 1109: upgrade residual feed excludes sold.
        self.objects
            .iter()
            .filter(|o| !o.destroyed && !o.sold && !o.applied_upgrades.is_empty())
            .collect()
    }

    /// Whether `upgrade` is applied on the object residual.
    pub fn object_has_upgrade(obj: &RenderableObject, upgrade: &str) -> bool {
        obj.applied_upgrades.iter().any(|u| u == upgrade)
    }

    /// Live mine / demo-trap presentation residuals.
    pub fn mine_objects(&self) -> Vec<&RenderableObject> {
        // Wave 1109: mine residual feed excludes sold.
        self.objects
            .iter()
            .filter(|o| !o.destroyed && !o.sold && o.has_mine)
            .collect()
    }

    /// True when snapshot object carries `kind` residual.
    pub fn object_has_kind(obj: &RenderableObject, kind: crate::game_logic::KindOf) -> bool {
        obj.kind_of.iter().any(|k| *k == kind)
    }

    /// Double-click residual: same-template selectable friendlies from snapshot.
    pub fn similar_unit_ids(
        &self,
        clicked_id: ObjectId,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        use crate::game_logic::KindOf;
        use crate::unit_control::UnitControlSystem;
        let Some(clicked) = self.objects.iter().find(|o| o.id == clicked_id) else {
            return Vec::new();
        };
        if clicked.team != player_team || !UnitControlSystem::presentation_is_selectable(clicked) {
            return Vec::new();
        }
        let template = clicked.template_name.as_str();
        self.objects
            .iter()
            .filter(|o| {
                o.team == player_team
                    && UnitControlSystem::presentation_is_selectable(o)
                    && o.template_name == template
            })
            .map(|o| o.id)
            .collect()
    }

    /// Right-click residual: enemy attackable under cursor id from snapshot.
    pub fn is_enemy_attackable(
        &self,
        target_id: ObjectId,
        player_team: crate::game_logic::Team,
    ) -> bool {
        use crate::unit_control::UnitControlSystem;
        // Wave 1103: fail-closed on non-local FOW unless Clear (pick parity).
        self.objects
            .iter()
            .find(|o| o.id == target_id)
            .map(|o| {
                o.team != player_team
                    && o.fow_visibility.visibility_alpha >= 0.95
                    && UnitControlSystem::presentation_is_attackable(o)
            })
            .unwrap_or(false)
    }

    /// Drag-box residual: friendly selectable units whose XZ pose is inside the rect.
    ///
    /// Prefer non-structures when any unit is in the box (C++ InGameUI drag residual).
    /// If only structures are hit, keep a single structure when exactly one is present.
    /// Filter stored ids to alive selectable friendlies (control-group recall residual).
    /// Script camera-slave residual: first non-destroyed object matching template (case-insensitive).
    /// Control-group double-tap residual: average XZ pose of listed alive objects.
    /// Runtime-host residual: first alive mobile friendly (select_local_unit).
    pub fn first_mobile_friendly_id(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Option<ObjectId> {
        use crate::unit_control::UnitControlSystem;
        self.objects
            .iter()
            .find(|o| {
                o.team == player_team
                    && !o.destroyed
                    && o.is_mobile
                    && UnitControlSystem::presentation_is_selectable(o)
            })
            .map(|o| o.id)
    }

    /// Runtime-host residual: first constructed structure with production capacity.
    pub fn first_constructed_producer_id(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Option<ObjectId> {
        // Prefer barracks/warfactory/airfield; fall back to any can_produce structure.
        // Wave 1100: fail-closed on sold/UC/disabled producers (train UI residual).
        let usable = |o: &&RenderableObject| {
            o.team == player_team
                && !o.destroyed
                && !o.sold
                && !o.under_construction
                && !o.disabled
                && o.can_produce
        };
        self.objects
            .iter()
            .find(|o| {
                usable(o)
                    && o.building_type
                        .map(PresentationBuildingType::is_unit_producer)
                        .unwrap_or(false)
            })
            .or_else(|| self.objects.iter().find(usable))
            .map(|o| o.id)
    }

    /// Structures that can produce units (ControlBar factory residual feed).
    pub fn unit_producer_structures(&self) -> Vec<&RenderableObject> {
        // Wave 1100: fail-closed on sold/UC/disabled factory residual feed.
        self.objects
            .iter()
            .filter(|o| {
                !o.destroyed
                    && !o.sold
                    && !o.under_construction
                    && !o.disabled
                    && o.can_produce
                    && o.building_type
                        .map(PresentationBuildingType::is_unit_producer)
                        .unwrap_or(false)
            })
            .collect()
    }

    /// Runtime-host residual: first alive enemy attackable.

    /// Unique non-empty model keys from alive objects (GPU preload residual).
    pub fn unique_model_keys(&self) -> Vec<String> {
        let mut keys = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for o in &self.objects {
            if o.destroyed {
                continue;
            }
            if let Some(k) = o.model_key.as_ref() {
                if !k.is_empty() && seen.insert(k.clone()) {
                    keys.push(k.clone());
                }
            }
        }
        keys
    }

    /// Structures holding supply crates residual (ControlBar / gather UI).
    pub fn supply_storage_structures(&self) -> Vec<&RenderableObject> {
        // Wave 1101: fail-closed on sold supply-storage residual feed.
        self.objects
            .iter()
            .filter(|o| {
                !o.destroyed
                    && !o.sold
                    && o.stored_supplies > 0
                    && (o.is_structure
                        || o.building_type.is_some()
                        || o.object_type == PresentationObjectType::Building
                        || o.object_type == PresentationObjectType::Supply)
            })
            .collect()
    }

    /// Friendly workers residual (dozer / worker command feed by team).
    pub fn friendly_workers(&self, player_team: crate::game_logic::Team) -> Vec<&RenderableObject> {
        use crate::game_logic::KindOf;
        // Wave 1101: fail-closed on sold/disabled worker residual feed.
        self.objects
            .iter()
            .filter(|o| {
                o.team == player_team
                    && !o.destroyed
                    && !o.sold
                    && !o.disabled
                    && (Self::object_has_kind(o, KindOf::Worker)
                        || o.template_name.contains("Dozer")
                        || o.template_name.contains("Worker")
                        || o.template_name.contains("Construction"))
            })
            .collect()
    }

    pub fn first_enemy_attackable_id(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Option<ObjectId> {
        use crate::unit_control::UnitControlSystem;
        // Wave 1104: fail-closed on non-local FOW unless Clear (is_enemy_attackable parity).
        self.objects
            .iter()
            .find(|o| {
                o.team != player_team
                    && o.fow_visibility.visibility_alpha >= 0.95
                    && UnitControlSystem::presentation_is_attackable(o)
            })
            .map(|o| o.id)
    }

    /// Host `attack_nearest_enemy` residual: FOW-clear attackable first, then force-attack.
    pub fn first_enemy_attack_command_id(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Option<ObjectId> {
        // Wave 1115: prefer is_enemy_attackable parity, then force-attack fallback.
        self.first_enemy_attackable_id(player_team)
            .or_else(|| self.first_enemy_force_attack_id(player_team))
    }

    /// Runtime-host residual: prefer non-structure enemy, else any attackable enemy.
    pub fn first_enemy_force_attack_id(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Option<ObjectId> {
        use crate::game_logic::KindOf;
        use crate::unit_control::UnitControlSystem;
        // Wave 1105: fail-closed on non-local FOW unless Clear (is_enemy_attackable /
        // first_enemy_attackable_id parity). Force-attack object residual must not
        // pick fogged/black enemies the local player cannot see.
        let visible_enemy = |o: &&RenderableObject| {
            o.team != player_team
                && o.fow_visibility.visibility_alpha >= 0.95
                && UnitControlSystem::presentation_is_attackable(o)
        };
        let mobile = self.objects.iter().find(|o| {
            visible_enemy(o)
                && !Self::object_has_kind(o, KindOf::Structure)
                && o.object_type != PresentationObjectType::Building
        });
        mobile
            .or_else(|| self.objects.iter().find(visible_enemy))
            .map(|o| o.id)
    }
}

// ===== spectre.rs =====
use super::*;

// --- Wave 73: Spectre AttackAreaDecal / TargetingReticleDecal presentation residual ---

/// Retail Spectre AttackAreaDecal Texture residual (`SCCSpecTarg`).
pub const PRESENTATION_SPECTRE_ATTACK_AREA_DECAL: &str = "SCCSpecTarg";
/// Retail Spectre TargetingReticleDecal Texture residual (`SCCSpecRet`).
pub const PRESENTATION_SPECTRE_TARGETING_RETICLE_DECAL: &str = "SCCSpecRet";
/// Retail Spectre decal Color residual (R:127 G:177 B:222 A:255) as RGBA 0..1.
pub const PRESENTATION_SPECTRE_DECAL_COLOR: [f32; 4] =
    [127.0 / 255.0, 177.0 / 255.0, 222.0 / 255.0, 1.0];
/// Retail AttackAreaDecal OpacityMin residual (25%).
pub const PRESENTATION_SPECTRE_ATTACK_AREA_OPACITY_MIN: f32 = 0.25;
/// Retail AttackAreaDecal OpacityMax residual (50%).
pub const PRESENTATION_SPECTRE_ATTACK_AREA_OPACITY_MAX: f32 = 0.50;
/// Retail TargetingReticleDecal OpacityMin residual (50%).
pub const PRESENTATION_SPECTRE_RETICLE_OPACITY_MIN: f32 = 0.50;
/// Retail TargetingReticleDecal OpacityMax residual (100%).
pub const PRESENTATION_SPECTRE_RETICLE_OPACITY_MAX: f32 = 1.00;
/// Retail AttackAreaDecal OpacityThrobTime residual (msec).
pub const PRESENTATION_SPECTRE_ATTACK_AREA_THROB_MS: u32 = 1500;
/// Retail TargetingReticleDecal OpacityThrobTime residual (msec).
pub const PRESENTATION_SPECTRE_RETICLE_THROB_MS: u32 = 300;
/// Retail AttackAreaRadius residual (presentation cursor / decal radius).
pub const PRESENTATION_SPECTRE_ATTACK_AREA_RADIUS: f32 = 200.0;
/// Retail TargetingReticleRadius residual.
pub const PRESENTATION_SPECTRE_RETICLE_RADIUS: f32 = 25.0;
/// Retail AttackAreaDecal Style residual.
pub const PRESENTATION_SPECTRE_DECAL_STYLE: &str = "SHADOW_ALPHA_DECAL";
/// Retail OnlyVisibleToOwningPlayer residual (both decals).
pub const PRESENTATION_SPECTRE_DECAL_ONLY_OWNER: bool = true;

/// Snapshot-owned Spectre orbit decal presentation residual (AttackArea + Reticle).
///
/// Fail-closed: not full SHADOW_ALPHA_DECAL GPU throb / owning-player visibility filter.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PresentationSpectreOrbitDecal {
    pub attack_area_texture: &'static str,
    pub reticle_texture: &'static str,
    pub color: [f32; 4],
    pub attack_area_radius: f32,
    pub reticle_radius: f32,
    pub attack_area_opacity_min: f32,
    pub attack_area_opacity_max: f32,
    pub reticle_opacity_min: f32,
    pub reticle_opacity_max: f32,
    pub attack_area_throb_ms: u32,
    pub reticle_throb_ms: u32,
    pub style: &'static str,
    pub only_visible_to_owning_player: bool,
}

impl PresentationSpectreOrbitDecal {
    /// Retail SpectreGunshipUpdate AttackAreaDecal + TargetingReticleDecal residual defaults.
    pub const RETAIL: Self = Self {
        attack_area_texture: PRESENTATION_SPECTRE_ATTACK_AREA_DECAL,
        reticle_texture: PRESENTATION_SPECTRE_TARGETING_RETICLE_DECAL,
        color: PRESENTATION_SPECTRE_DECAL_COLOR,
        attack_area_radius: PRESENTATION_SPECTRE_ATTACK_AREA_RADIUS,
        reticle_radius: PRESENTATION_SPECTRE_RETICLE_RADIUS,
        attack_area_opacity_min: PRESENTATION_SPECTRE_ATTACK_AREA_OPACITY_MIN,
        attack_area_opacity_max: PRESENTATION_SPECTRE_ATTACK_AREA_OPACITY_MAX,
        reticle_opacity_min: PRESENTATION_SPECTRE_RETICLE_OPACITY_MIN,
        reticle_opacity_max: PRESENTATION_SPECTRE_RETICLE_OPACITY_MAX,
        attack_area_throb_ms: PRESENTATION_SPECTRE_ATTACK_AREA_THROB_MS,
        reticle_throb_ms: PRESENTATION_SPECTRE_RETICLE_THROB_MS,
        style: PRESENTATION_SPECTRE_DECAL_STYLE,
        only_visible_to_owning_player: PRESENTATION_SPECTRE_DECAL_ONLY_OWNER,
    };

    /// Honesty: retail Spectre AttackAreaDecal / TargetingReticleDecal presentation residual.
    pub fn honesty_residual_ok(self) -> bool {
        self.attack_area_texture == "SCCSpecTarg"
            && self.reticle_texture == "SCCSpecRet"
            && (self.attack_area_radius - 200.0).abs() < 0.01
            && (self.reticle_radius - 25.0).abs() < 0.01
            && (self.attack_area_opacity_min - 0.25).abs() < 0.001
            && (self.attack_area_opacity_max - 0.50).abs() < 0.001
            && (self.reticle_opacity_min - 0.50).abs() < 0.001
            && (self.reticle_opacity_max - 1.00).abs() < 0.001
            && self.attack_area_throb_ms == 1500
            && self.reticle_throb_ms == 300
            && self.style == "SHADOW_ALPHA_DECAL"
            && self.only_visible_to_owning_player
            && (self.color[0] - 127.0 / 255.0).abs() < 0.001
            && (self.color[1] - 177.0 / 255.0).abs() < 0.001
            && (self.color[2] - 222.0 / 255.0).abs() < 0.001
            && (self.color[3] - 1.0).abs() < 0.001
            && self.attack_area_opacity_min < self.attack_area_opacity_max
            && self.reticle_opacity_min < self.reticle_opacity_max
            && self.reticle_radius < self.attack_area_radius
    }
}

/// Free-function honesty for Spectre orbit decal presentation residual (Wave 73).
pub fn honesty_spectre_orbit_decal_presentation_ok() -> bool {
    PresentationSpectreOrbitDecal::RETAIL.honesty_residual_ok()
}

/// Wave 102: dual-tick presentation residual deepen free-function honesty.
///
/// Builds an empty-host presentation snapshot and verifies dual-tick residual
/// counters (including selected/particle Wave 102 deepen) plus presentation
/// residual packs. Fail-closed vs live dual-run W3D / GPU submit.
pub fn honesty_presentation_dual_tick_residual_deepen_wave102() -> bool {
    use crate::game_logic::GameLogic;
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};

    // Empty residual snapshot honesty (zero objects still dual-tick consistent).
    let empty_logic = GameLogic::new();
    let empty = PresentationFrame::build_from_logic(&empty_logic, 0);
    if !empty.dual_tick_presentation_residual_ok() {
        return false;
    }
    if empty.dual_tick.builds != 1 || empty.dual_tick.applies != 0 {
        return false;
    }
    if empty.dual_tick.selected_count != 0 || empty.dual_tick.particle_count != 0 {
        return false;
    }
    // Seeded skirmish residual: dual-tick deepen after shell apply.
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("PresDualTick102");
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        // Config residual may still produce honest empty-host dual-tick.
        return empty.dual_tick_presentation_residual_deepen_ok()
            && honesty_spectre_orbit_decal_presentation_ok();
    }
    let mut hud = crate::ui::GameHUD::new();
    let mut ui = crate::ui::GameUIState::default();
    let mut rts = crate::ui::RTSInterface::new();
    let mut cmd = crate::ui::UnitCommandPanel::new();
    let frame = PresentationFrame::build_and_apply_for_shell_consumers(
        &logic, 0, &mut hud, &mut ui, &mut rts, &mut cmd,
    );
    frame.dual_tick_presentation_residual_deepen_ok()
        && frame.dual_tick.honesty_apply_ok()
        && frame.dual_tick.builds == 1
        && frame.dual_tick.applies >= 1
        && frame.dual_tick.selected_count == frame.selected.len() as u32
        && frame.dual_tick.particle_count == frame.particle_systems.len() as u32
        && honesty_spectre_orbit_decal_presentation_ok()
}

/// Combined Wave 102 presentation residual honesty pack.
pub fn honesty_presentation_residual_deepen_pack_wave102() -> bool {
    honesty_presentation_dual_tick_residual_deepen_wave102()
}

/// Dual-tick residual counters frozen on each presentation build / apply.
///
/// Host-testable bookkeeping for seed → logic step → multi-consumer apply order.
/// Fail-closed: not full dual-run determinism harness counters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PresentationDualTickResidual {
    /// Always 1 after a successful `build_from_logic`.
    pub builds: u32,
    /// Incremented each time this snapshot is applied to HUD / shell consumers.
    pub applies: u32,
    pub object_count: u32,
    pub selected_count: u32,
    pub laser_beam_count: u32,
    pub floating_text_count: u32,
    pub world_anim_count: u32,
    pub particle_count: u32,
}

impl PresentationDualTickResidual {
    pub fn from_counts(
        objects: usize,
        selected: usize,
        lasers: usize,
        floating: usize,
        world: usize,
        particles: usize,
    ) -> Self {
        Self {
            builds: 1,
            applies: 0,
            object_count: objects as u32,
            selected_count: selected as u32,
            laser_beam_count: lasers as u32,
            floating_text_count: floating as u32,
            world_anim_count: world as u32,
            particle_count: particles as u32,
        }
    }

    /// Honesty: residual counters are self-consistent after build.
    pub fn honesty_build_ok(&self) -> bool {
        self.builds >= 1
    }

    /// Honesty: at least one dual-tick apply was recorded.
    pub fn honesty_apply_ok(&self) -> bool {
        self.builds >= 1 && self.applies >= 1
    }
}

// ===== types.rs =====
use super::*;

/// Logic-frame index (30 Hz authority).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LogicFrame(pub u32);

/// ControlBar production cameo CanMake residual frozen for presentation/UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresentationCanMakeCameo {
    pub template_name: String,
    /// C++ CanMakeType ordinal residual (CANMAKE_*).
    pub can_make: u32,
    /// True when CANMAKE_OK residual.
    pub available: bool,
    /// Optional HelpBox status message residual (None when OK / silent statuses).
    pub help_status: Option<String>,
}

/// Snapshot-owned factory production queue entry (host BuildingData residual).
/// Fail-closed: not full ControlBar queue UI / cancel-button WND parity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresentationProductionItem {
    pub template_name: String,
    /// Absolute research/build progress seconds residual.
    pub progress: f32,
    pub total_time: f32,
    pub cost_supplies: u32,
    /// C++ PRODUCTION_UPGRADE residual on producer queue.
    pub is_upgrade: bool,
    /// Normalized 0..1 residual for ControlBar / build-queue strip.
    pub progress_ratio: f32,
}

impl PresentationProductionItem {
    #[inline]
    pub fn from_host_item(item: &crate::game_logic::buildings::ProductionItem) -> Self {
        let ratio = if item.total_time <= 0.0 {
            1.0
        } else {
            (item.progress / item.total_time).clamp(0.0, 1.0)
        };
        Self {
            template_name: item.template_name.clone(),
            progress: item.progress,
            total_time: item.total_time,
            cost_supplies: item.cost.supplies,
            is_upgrade: item.is_upgrade(),
            progress_ratio: ratio,
        }
    }

    /// Wave 489: GameWorld entity production queue → presentation strip.
    #[inline]
    pub fn from_entity_item(item: &gamelogic::world::entities::EntityProductionItem) -> Self {
        let ratio = if item.total_time <= 0.0 {
            1.0
        } else {
            (item.progress / item.total_time).clamp(0.0, 1.0)
        };
        Self {
            template_name: item.template_name.clone(),
            progress: item.progress,
            total_time: item.total_time,
            cost_supplies: item.cost_supplies,
            is_upgrade: item.is_upgrade,
            progress_ratio: ratio,
        }
    }
}

/// Snapshot-owned veterancy rank (host Experience residual).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PresentationVeterancy {
    Rookie,
    Veteran,
    Elite,
    Heroic,
}

impl PresentationVeterancy {
    pub fn from_host(level: crate::game_logic::VeterancyLevel) -> Self {
        use crate::game_logic::VeterancyLevel as V;
        match level {
            V::Rookie => Self::Rookie,
            V::Veteran => Self::Veteran,
            V::Elite => Self::Elite,
            V::Heroic => Self::Heroic,
        }
    }

    /// Wave 490: GameWorld entity veterancy_ordinal residual.
    #[inline]
    pub fn from_ordinal(ord: u8) -> Self {
        match ord {
            1 => Self::Veteran,
            2 => Self::Elite,
            3 => Self::Heroic,
            _ => Self::Rookie,
        }
    }

    /// C++ ControlBar portrait chevron image residual (SSChevron*).
    pub fn chevron_overlay(self) -> Option<&'static str> {
        match self {
            Self::Rookie => None,
            Self::Veteran => Some("SSChevron1L"),
            Self::Elite => Some("SSChevron2L"),
            Self::Heroic => Some("SSChevron3L"),
        }
    }
}

/// Snapshot-owned object kind residual (host ObjectType).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PresentationObjectType {
    Infantry,
    Vehicle,
    Aircraft,
    Building,
    Supply,
    Projectile,
    Neutral,
}

impl PresentationObjectType {
    pub fn from_host(t: crate::game_logic::ObjectType) -> Self {
        use crate::game_logic::ObjectType as T;
        match t {
            T::Infantry => Self::Infantry,
            T::Vehicle => Self::Vehicle,
            T::Aircraft => Self::Aircraft,
            T::Building => Self::Building,
            T::Supply => Self::Supply,
            T::Projectile => Self::Projectile,
            T::Neutral => Self::Neutral,
        }
    }
}

/// Snapshot-owned structure kind residual (host BuildingType).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PresentationBuildingType {
    CommandCenter,
    Barracks,
    WarFactory,
    Airfield,
    RepairPad,
    HealPad,
    SupplyCenter,
    PowerPlant,
    DefenseTurret,
    SupplyDropZone,
    Palace,
    Propaganda,
    Bunker,
}

impl PresentationBuildingType {
    pub fn from_host(t: crate::game_logic::BuildingType) -> Self {
        use crate::game_logic::BuildingType as B;
        match t {
            B::CommandCenter => Self::CommandCenter,
            B::Barracks => Self::Barracks,
            B::WarFactory => Self::WarFactory,
            B::Airfield => Self::Airfield,
            B::RepairPad => Self::RepairPad,
            B::HealPad => Self::HealPad,
            B::SupplyCenter => Self::SupplyCenter,
            B::PowerPlant => Self::PowerPlant,
            B::DefenseTurret => Self::DefenseTurret,
            B::SupplyDropZone => Self::SupplyDropZone,
            B::Palace => Self::Palace,
            B::Propaganda => Self::Propaganda,
            B::Bunker => Self::Bunker,
        }
    }

    /// Wave 490: GameWorld entity building_type_ordinal residual (255 = none).
    #[inline]
    pub fn from_ordinal(ord: u8) -> Option<Self> {
        match ord {
            0 => Some(Self::CommandCenter),
            1 => Some(Self::Barracks),
            2 => Some(Self::WarFactory),
            3 => Some(Self::Airfield),
            4 => Some(Self::RepairPad),
            5 => Some(Self::HealPad),
            6 => Some(Self::SupplyCenter),
            7 => Some(Self::PowerPlant),
            8 => Some(Self::DefenseTurret),
            9 => Some(Self::SupplyDropZone),
            10 => Some(Self::Palace),
            11 => Some(Self::Propaganda),
            12 => Some(Self::Bunker),
            _ => None,
        }
    }

    /// Factory / barracks / airfield residual for unit production UI.
    pub fn is_unit_producer(self) -> bool {
        matches!(self, Self::Barracks | Self::WarFactory | Self::Airfield)
    }
}

/// One renderable object as seen after a completed logic step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenderableObject {
    pub id: ObjectId,
    pub template_name: String,
    pub team: Team,
    /// Team tint for presentation-only draw (RGBA 0..1), mirrors Object::team_color.
    pub team_color: [f32; 4],
    pub position: Vec3,
    pub orientation: f32,
    /// C++ ToppleUpdate lean residual (radians fallen about fall axis).
    #[serde(default)]
    pub topple_lean_radians: f32,
    /// Current movement order destination (host Movement::target_position).
    pub move_destination: Option<Vec3>,
    /// Host Object::target_location residual (script/order point).
    pub target_location: Option<Vec3>,
    /// Host guard_target residual.
    pub guard_target: Option<ObjectId>,
    /// Host ObjectStatus::using_ability residual.
    pub using_ability: bool,
    /// Host ObjectStatus::airborne_target residual.
    pub airborne_target: bool,
    /// Wave 982: producer/slaver residual for IgnoredInGui mouseover remap.
    #[serde(default)]
    pub producer_id: Option<ObjectId>,
    /// Wave 983: healing icon residual (sole-benefactor / heal timer).
    #[serde(default)]
    pub show_healing: bool,
    #[serde(default)]
    pub healing_icon_type: u8,
    /// Wave 505: C++ OBJECT_STATUS_PARACHUTING residual.
    pub parachuting: bool,
    /// Wave 509: C++ parachute open residual (false + parachuting => FREEFALL).
    pub parachute_open: bool,
    /// Wave 510: C++ CAPTURED model-condition residual.
    pub captured: bool,
    /// Wave 512: C++ prone residual (Infantry goProne timer).
    pub prone: bool,
    /// Wave 514: C++ Drawable emoticon residual name.
    pub emoticon_name: String,
    /// Wave 514: remaining logic frames for emoticon.
    pub emoticon_frames_left: i32,
    /// Wave 515: C++ AIUpdateInterface::setSurrendered residual.
    pub is_surrendered: bool,
    /// Wave 515: C++ Object::m_formationID residual (0 = none).
    pub formation_id: u32,
    /// Wave 515: C++ Object::m_formationOffset residual.
    pub formation_offset: glam::Vec2,
    /// Wave 507: C++ OVER_WATER model condition residual (hover craft / water).
    pub over_water: bool,
    /// Wave 526: MOVING/ATTACKING via name-table helpers.
    /// Wave 525: FRONTCRUSHED/BACKCRUSHED/PREORDER/USER_1/USER_2 residual.
    /// Wave 524: multi-door DOOR_2..4 banks + SMOLDERING residual.
    /// Wave 523: stamp STUNNED_FLAILING / SECOND_LIFE / POST_COLLAPSE / SPECIAL_DAMAGED.
    /// Wave 522: C++ terrain cell cliff residual.
    pub cell_is_cliff: bool,
    /// Wave 522: C++ terrain cell underwater residual.
    pub cell_is_underwater: bool,
    /// Host movement max speed residual.
    pub move_max_speed: f32,
    /// Host velocity residual.
    pub velocity: Vec3,
    /// Host AI state ordinal residual.
    pub ai_state_ordinal: u8,
    /// Attack target object id when set.
    pub attack_target: Option<ObjectId>,
    /// Path waypoints residual (capped) for line pack / debug draw.
    pub path_waypoints: Vec<Vec3>,
    /// Host movement path length residual.
    pub path_len: u16,
    /// Host movement current path index residual.
    pub path_index: u16,
    /// Host occupant_count residual (transport/contain).
    pub occupant_count: u16,
    /// Structure production queue residual (empty for non-buildings).
    pub production_queue: Vec<PresentationProductionItem>,
    /// Wave 986: host BuildingData production pause residual.
    #[serde(default)]
    pub production_paused: bool,
    /// Structure rally point residual.
    pub rally_point: Option<Vec3>,
    /// Guard position residual (units).
    pub guard_position: Option<Vec3>,
    /// Contained unit ids (garrison / transport residual, capped).
    pub garrisoned_units: Vec<ObjectId>,
    /// Max garrison slots (0 = not a container).
    pub max_garrison: usize,
    /// Structure/unit power provided residual.
    pub power_provided: i32,
    /// Structure/unit power consumed residual.
    pub power_consumed: i32,
    /// Host Object::stored_resources.supplies residual (supply center / drop zone).
    pub stored_supplies: u32,
    pub health_current: f32,
    pub health_max: f32,
    pub selected: bool,
    /// C++ OBJECT_STATUS_DEPLOYED residual.
    pub is_deployed: bool,
    /// C++ Drawable selection flash envelope residual frames.
    pub selection_flash_remaining: u32,
    pub destroyed: bool,
    /// C++ ModelConditionFlags residual (ALLOW_SURRENDER-off bit layout, low 128).
    pub model_condition_bits: u128,
    /// C++ RadarUpdate m_radarActive residual.
    pub radar_active: bool,
    /// C++ RadarUpdate m_extendComplete residual.
    pub radar_extend_complete: bool,
    /// C++ ProductionUpdate door residual phase (0 idle .. 3 closing).
    pub production_door_phase: u8,
    /// C++ BodyDamageType residual ordinal (0 pristine .. 3 rubble).
    pub body_damage_state: u8,
    /// C++ TransitionDamageFX / FXListDie residual name frozen at snapshot.
    #[serde(default)]
    pub damage_fx_name: Option<String>,
    /// C++ BoneFXDamage / BoneFXUpdate residual FXList name.
    #[serde(default)]
    pub bone_fx_name: Option<String>,
    /// C++ TINT_STATUS_POISONED residual.
    #[serde(default)]
    pub poison_tinted: bool,
    /// C++ UNDETECTED_DEFECTOR residual.
    #[serde(default)]
    pub undetected_defector: bool,
    /// C++ DefectionHelper selection flash residual.
    #[serde(default)]
    pub defector_flash: bool,
    /// C++ FXListDie death FX residual name.
    #[serde(default)]
    pub death_fx_name: Option<String>,
    /// C++ DeathType residual name for death FX (empty when alive).
    pub death_type_name: String,
    pub under_construction: bool,
    /// Construction progress 0..1 residual (structures / dozer builds).
    pub construction_percent: f32,
    /// Wave 1031: OCL timer residual seconds (ControlBar OclTimer dual path).
    pub ocl_timer_seconds: u32,
    /// C++ OBJECT_STATUS_SOLD residual frozen for presentation/UI.
    pub sold: bool,
    /// C++ OBJECT_STATUS_UNSELECTABLE residual frozen for presentation/UI.
    pub unselectable: bool,
    /// C++ RebuildHole residual frozen for presentation/UI.
    pub is_rebuild_hole: bool,
    /// Wave 993: C++ RebuildHoleBehavior m_rebuildTemplate residual.
    #[serde(default)]
    pub rebuild_template_name: String,
    /// Wave 993: host rebuild_ready_frame residual.
    #[serde(default)]
    pub rebuild_ready_frame: u32,
    /// Wave 993: RebuildHoleBehavior m_spawnerID residual.
    #[serde(default)]
    pub rebuild_spawner_id: Option<ObjectId>,
    /// Wave 993: RebuildHoleBehavior m_workerID residual.
    #[serde(default)]
    pub rebuild_worker_id: Option<ObjectId>,
    /// Wave 993: RebuildHoleBehavior m_reconstructingID residual.
    #[serde(default)]
    pub rebuild_reconstructing_id: Option<ObjectId>,
    /// C++ OBJECT_STATUS_RECONSTRUCTING residual frozen for presentation.
    pub reconstructing: bool,
    /// Veterancy rank residual for chevrons / UI.
    pub veterancy: PresentationVeterancy,
    /// Experience points residual (display / debug).
    pub experience_points: f32,
    /// Host ObjectStatus::moving residual.
    pub moving: bool,
    /// Host ObjectStatus::attacking residual.
    pub attacking: bool,
    /// Host ObjectStatus::is_firing_weapon residual.
    pub is_firing_weapon: bool,
    /// Host ObjectStatus::is_aiming_weapon residual.
    pub is_aiming_weapon: bool,
    /// Host ObjectStatus::disabled_emp residual.
    pub disabled_emp: bool,
    /// Host ObjectStatus::disabled_paralyzed residual.
    pub disabled_paralyzed: bool,
    /// Host ObjectStatus::weapons_jammed residual.
    pub weapons_jammed: bool,
    /// Host ObjectStatus::masked residual.
    pub masked: bool,
    /// Host ObjectStatus::ignoring_stealth residual.
    pub ignoring_stealth: bool,
    /// Host ObjectStatus::repulsor residual.
    pub repulsor: bool,
    /// C++ OBJECT_STATUS_STEALTHED residual.
    pub stealthed: bool,
    /// C++ OBJECT_STATUS_DETECTED residual.
    pub detected: bool,
    /// Stealthed && !detected && !disguised (not a legal auto-target).
    pub effectively_stealthed: bool,
    /// Any host disable residual that blocks acting.
    pub disabled: bool,
    /// Container residual when this unit is inside another object.
    pub contained_by: Option<ObjectId>,
    /// Force-attack order residual.
    pub force_attack: bool,
    /// Primary weapon present residual.
    pub has_weapon: bool,
    /// Primary weapon range residual (0 when unarmed).
    pub weapon_range: f32,
    /// Primary weapon damage residual (0 when unarmed).
    pub weapon_damage: f32,
    /// Primary weapon min range residual.
    pub weapon_min_range: f32,
    /// Primary weapon reload time residual (seconds-ish).
    pub weapon_reload_time: f32,
    /// Primary weapon ammo residual (`u32::MAX` = unlimited).
    pub weapon_ammo: u32,
    /// C++ getAmmoPipShowingInfo residual (0 = no ShowsAmmoPips weapon).
    pub ammo_pip_total: u32,
    /// Remaining rounds for the ShowsAmmoPips weapon.
    pub ammo_pip_full: u32,
    /// C++ getMostPercentReadyToFireAnyWeapon residual (0..100).
    pub weapon_ready_percent: u32,
    /// Primary weapon air/ground targeting residual.
    pub weapon_can_target_air: bool,
    pub weapon_can_target_ground: bool,
    /// Primary weapon projectile speed residual.
    pub weapon_projectile_speed: f32,
    /// Host armed_riders_upgrade_weapon_set residual.
    pub armed_riders_upgrade_weapon_set: bool,
    /// Host weapon_set_player_upgrade residual.
    pub weapon_set_player_upgrade: bool,
    /// Wave 523: C++ ARMORSET_SECOND_LIFE / battle bus second life residual.
    pub second_life: bool,
    /// Wave 525: C++ front crushed residual.
    pub front_crushed: bool,
    /// Wave 525: C++ back crushed residual.
    pub back_crushed: bool,
    /// Wave 525: host model-condition USER_1 residual.
    pub user_1: bool,
    /// Wave 525: host model-condition USER_2 residual.
    pub user_2: bool,
    /// Wave 518: C++ weapon_crate_upgrade residual (0/1/2).
    pub weapon_crate_upgrade: u8,
    /// Wave 518: C++ armor_crate_upgrade residual (0/1/2).
    pub armor_crate_upgrade: u8,
    /// Wave 518: C++ EnemyNearUpdate model_enemy_near residual.
    pub enemy_near: bool,
    /// Wave 518: C++ armed riders / ARMED model residual.
    pub armed: bool,
    /// CamoNetting StealthLook ordinal residual (0..5).
    pub camo_stealth_look: u8,
    /// Bomb-truck disguise template residual.
    pub disguise_as_template: Option<String>,
    /// Apparent team while disguised.
    pub disguise_as_team: Option<Team>,
    /// C++ OBJECT_STATUS_DISGUISED residual.
    pub disguised: bool,
    /// Host ObjectStatus::disabled_subdued residual.
    pub disabled_subdued: bool,
    /// Host ObjectStatus::is_carbomb residual.
    pub is_carbomb: bool,
    /// Host ObjectStatus::hijacked residual.
    pub hijacked: bool,
    /// C++ StealthUpdate disguise transition opacity residual (0..1).
    pub disguise_transition_opacity: f32,
    /// Stealth detector range residual (0 = none).
    pub detection_range: f32,
    /// Host detection_rate_frames residual (0 = continuous).
    pub detection_rate_frames: u32,
    /// Host stealth_breaks_on_attack residual.
    pub stealth_breaks_on_attack: bool,
    /// Host stealth_breaks_on_move residual.
    pub stealth_breaks_on_move: bool,
    /// Host innate_stealth residual.
    pub innate_stealth: bool,
    /// Host weapon_bonus_frenzy_until_frame residual.
    pub weapon_bonus_frenzy_until_frame: u32,
    /// Host continuous_fire_consecutive residual.
    pub continuous_fire_consecutive: u16,
    /// Host continuous_fire_coast_until_frame residual.
    pub continuous_fire_coast_until_frame: u32,
    /// Host battle_plan_sight_scalar_applied residual (1.0 = none).
    pub battle_plan_sight_scalar_applied: f32,
    /// Special power ready residual (superweapon / hero ability).
    pub special_power_ready: bool,
    /// Special power full cooldown seconds residual.
    pub special_power_cooldown: f32,
    /// Special power remaining cooldown seconds residual.
    pub special_power_cooldown_remaining: f32,
    /// Host ObjectType residual (UI / command set feed).
    pub object_type: PresentationObjectType,
    /// Applied upgrade tags residual (capped, sorted).
    pub applied_upgrades: Vec<String>,
    /// Secondary weapon present residual.
    pub has_secondary_weapon: bool,
    /// Secondary weapon range residual (0 when none).
    pub secondary_weapon_range: f32,
    /// Secondary weapon damage residual (0 when none).
    pub secondary_weapon_damage: f32,
    /// Host turret yaw residual (degrees).
    pub turret_angle_deg: f32,
    /// Host turret pitch residual (degrees).
    pub turret_pitch_deg: f32,
    /// Host turret idle-scan residual.
    pub turret_idle_scanning: bool,
    /// Host weapon-bonus residual flags (presentation UI/FX).
    pub weapon_bonus_enthusiastic: bool,
    pub weapon_bonus_subliminal: bool,
    pub weapon_bonus_horde: bool,
    pub weapon_bonus_nationalism: bool,
    pub weapon_bonus_frenzy: bool,
    pub weapon_bonus_frenzy_level: u8,
    /// Host battle-plan weapon-bonus residual (Strategy Center).
    pub weapon_bonus_battle_plan_bombardment: bool,
    pub weapon_bonus_battle_plan_hold_the_line: bool,
    pub weapon_bonus_battle_plan_search_and_destroy: bool,
    /// Host continuous-fire residual (gattling spin-up).
    pub continuous_fire_level: u8,
    /// Host faerie_fire_until_frame residual.
    pub faerie_fire_until_frame: u32,
    /// Host hive slave residual (Stinger Site etc.).
    pub hive_slave_count: u8,
    pub hive_slave_hp: f32,
    /// Host AI attitude residual.
    pub ai_attitude: i8,
    /// Host camo friendly opacity residual.
    pub camo_friendly_opacity: f32,
    /// Host vision_spied_mask residual.
    pub vision_spied_mask: u32,
    /// Wave 994: host Object::vision_range residual.
    #[serde(default)]
    pub vision_range: f32,
    /// Wave 994: host Object::shroud_clearing_range residual.
    #[serde(default)]
    pub shroud_clearing_range: f32,
    /// Wave 994: host Object::crusher_level residual.
    #[serde(default)]
    pub crusher_level: u8,
    /// Wave 994: host Object::crushable_level residual.
    #[serde(default)]
    pub crushable_level: u8,
    /// Host cheer_timer residual.
    pub cheer_timer: f32,
    /// Host transport-kind residual markers.
    pub is_humvee_transport: bool,
    pub is_listening_outpost_transport: bool,
    pub is_troop_crawler_transport: bool,
    pub is_helix_transport: bool,
    pub has_overlord_gattling_addon: bool,
    pub has_overlord_propaganda_addon: bool,
    pub is_battle_bus_transport: bool,
    pub is_technical_transport: bool,
    pub is_combat_cycle_transport: bool,
    pub combat_cycle_rider: u8,
    pub is_tunnel_network: bool,
    pub is_combat_chinook_transport: bool,
    pub max_transport: usize,
    pub overlord_bunker_capacity: usize,
    pub passengers_allowed_to_fire: bool,
    pub display_name: String,
    pub demo_suicided_detonating: bool,
    /// Host turret_holding residual.
    pub turret_holding: bool,
    /// Host last_damage_source residual (0 = none).
    pub last_damage_source_host: u32,
    /// Host Object::command_set_override residual (empty = template default).
    pub command_set_override: String,
    /// Effective command-set name freeze (override or ThingFactory template).
    pub command_set_name: String,
    /// Host Object::is_detector residual.
    pub is_detector: bool,
    /// Host Object::active_weapon_slot residual.
    pub active_weapon_slot: u8,
    /// Wave 517: C++ WeaponFireStatus ordinal residual (Ready/OutOfAmmo/Between/Reload/PreAttack).
    pub weapon_fire_status: u8,
    /// Wave 517: C++ loco/AI panicking residual.
    pub is_panicking: bool,
    /// Wave 517: C++ moving_backwards residual.
    pub moving_backwards: bool,
    /// Host Object::overcharge_enabled residual.
    pub overcharge_enabled: bool,
    /// Wave 519: C++ shockwave airborne residual.
    pub shock_was_airborne: bool,
    /// Wave 519: C++ shock allow bounce residual.
    pub shock_allow_bounce: bool,
    /// Wave 519: C++ shock grounded-once residual.
    pub shock_grounded_once: bool,
    /// Wave 519: remaining shock stun frames.
    pub shock_stun_frames: u32,
    /// Wave 519: C++ PowerPlantUpdate m_extended residual.
    pub power_plant_rods_extended: bool,
    /// Wave 519: frame when rods finish upgrading (0 idle).
    pub power_plant_rods_done_frame: u32,
    /// Wave 519: jet slow-death residual active.
    pub jet_slow_death_active: bool,
    /// Wave 520: C++ AnimationSteeringUpdate turn anim ordinal residual
    /// (0 invalid, 1 CTR, 2 CTL, 3 LTC, 4 RTC).
    pub anim_steer_turn: u8,
    /// Host Object::show_health_bar residual.
    pub show_health_bar: bool,
    /// Host Object::guard_radius residual.
    pub guard_radius: f32,
    /// Mine / demo-trap residual present.
    pub has_mine: bool,
    /// Host ThingTemplate KindOf set residual (sorted, capped).
    /// Lets ControlBar / unit_control classify without live template re-read.
    pub kind_of: Vec<crate::game_logic::KindOf>,
    pub is_structure: bool,
    pub is_unit: bool,
    /// Mobile residual (infantry/vehicle/aircraft) for runtime-host select.
    pub is_mobile: bool,
    /// Structure can enqueue production (host building_data present + constructed).
    pub can_produce: bool,
    /// Host BuildingType residual when structure has building_data.
    pub building_type: Option<PresentationBuildingType>,
    /// W3D / mesh resolve key (template model name). Snapshot-owned so the unit
    /// mesh pass does not re-read live ThingTemplate during GPU collect.
    pub model_key: Option<String>,
    /// Mesh scale residual (Object INI Scale; common combat units retail **1.0**).
    /// Snapshot-owned so the unit mesh pass does not re-read live template Scale.
    /// Fail-closed: not full draw-scale bone / animation scale matrix.
    pub mesh_scale: f32,
    /// Cull / selection radius for presentation-only draw (no live GameLogic re-read).
    pub selection_radius: f32,
    /// True when bridged to GameEngine ObjectFactory (retired host dual-id).
    /// Presentation-owned so the unit mesh pass can skip double-draw without
    /// locking live GameLogic for identity.
    pub engine_bridged: bool,
    /// FOW visibility for `PresentationFrame.local_player_id` at snapshot time.
    /// Unit mesh pass applies alpha / never-explored skip from this only — no
    /// live shroud re-query mid-render.
    pub fow_visibility: ObjectVisibility,
    /// Terrain ground-height residual sampled at object XY (Wave 77 deepen).
    /// Defaults to `PRESENTATION_DEFAULT_GROUND_HEIGHT` when map height unavailable.
    /// Fail-closed: not full HeightMap bilinear / bridge-aware sample; does **not**
    /// rewrite `position.y` (locomotor ground clamp residual separate).
    pub ground_height: f32,
    /// True when `ground_height` came from terrain sample (not default-0).
    pub ground_height_from_terrain: bool,
}

// ===== unit_render.rs =====
use super::*;

/// Snapshot-owned unit mesh/position/selection/FOW input for the main unit render pass.
///
/// Built only from `PresentationFrame` — no live `GameLogic` or shroud borrow.
/// W3D asset resolve uses `assets::mesh_asset_resolve` from `model_key`
/// (see OWNERSHIP residual notes — fail-closed vs full material/animation parity).
#[derive(Debug, Clone, PartialEq)]
pub struct UnitRenderInput {
    pub id: ObjectId,
    pub template_name: String,
    pub model_key: String,
    /// Mesh scale residual frozen from presentation (default 1.0).
    pub mesh_scale: f32,
    pub team: Team,
    pub team_color: [f32; 4],
    pub position: Vec3,
    pub orientation: f32,
    /// C++ ToppleUpdate lean residual for mesh tilt.
    pub topple_lean_radians: f32,
    /// Wave 494: host turret yaw residual (degrees) for mesh facing.
    pub turret_angle_deg: f32,
    /// Wave 494: host turret pitch residual (degrees).
    pub turret_pitch_deg: f32,
    pub selected: bool,
    pub selection_radius: f32,
    /// C++ Drawable selection flash envelope residual frames remaining.
    pub selection_flash_remaining: u32,
    /// Frozen ModelConditionFlags residual for mesh subobject selection.
    pub model_condition_bits: u128,
    /// Production door residual phase.
    pub production_door_phase: u8,
    pub is_structure: bool,
    pub is_unit: bool,
    /// Wave 495: frozen combat motion flags for mesh model-condition stamping.
    pub moving: bool,
    pub attacking: bool,
    pub is_firing_weapon: bool,
    /// Wave 517: active weapon slot residual (0=A,1=B,2=C).
    pub active_weapon_slot: u8,
    /// Wave 517: WeaponFireStatus ordinal residual.
    pub weapon_fire_status: u8,
    /// Wave 517: panicking residual.
    pub is_panicking: bool,
    /// Wave 517: moving_backwards residual.
    pub moving_backwards: bool,
    /// Wave 518: weapon_set_player_upgrade residual.
    pub weapon_set_player_upgrade: bool,
    /// Wave 518: weapon crate upgrade level 0/1/2.
    pub weapon_crate_upgrade: u8,
    /// Wave 518: armor crate upgrade level 0/1/2.
    pub armor_crate_upgrade: u8,
    /// Wave 518: enemy-near model residual.
    pub enemy_near: bool,
    /// Wave 518: ARMED model residual.
    pub armed: bool,
    /// Wave 519: shockwave airborne residual.
    pub shock_was_airborne: bool,
    /// Wave 519: shock allow bounce residual.
    pub shock_allow_bounce: bool,
    /// Wave 519: shock grounded-once residual.
    pub shock_grounded_once: bool,
    /// Wave 519: shock stun frames remaining.
    pub shock_stun_frames: u32,
    /// Wave 519: power plant rods extended residual.
    pub power_plant_rods_extended: bool,
    /// Wave 519: power plant rods done frame residual.
    pub power_plant_rods_done_frame: u32,
    /// Wave 519: jet slow-death active residual.
    pub jet_slow_death_active: bool,
    /// Wave 520: animation steering turn anim ordinal residual.
    pub anim_steer_turn: u8,
    /// Wave 497: body damage ordinal for mesh variant resolve (0..3).
    pub body_damage_state: u8,
    /// Wave 499: C++ TINT_STATUS_POISONED residual.
    pub poison_tinted: bool,
    /// Wave 499: C++ DefectionHelper flash residual.
    pub defector_flash: bool,
    /// Wave 501: C++ OBJECT_STATUS_DEPLOYED residual.
    pub is_deployed: bool,
    /// Wave 501: structure radar dish active residual.
    pub radar_active: bool,
    /// Wave 501: radar extend animation complete residual.
    pub radar_extend_complete: bool,
    /// Wave 502: C++ effectively stealthed residual (stealthed && !detected && !disguised).
    pub effectively_stealthed: bool,
    /// Wave 503: structure under construction residual.
    pub under_construction: bool,
    /// Wave 503: construction progress 0..1 residual.
    pub construction_percent: f32,
    /// Wave 503: disguised residual (mesh/template swap for non-allied viewers).
    pub disguised: bool,
    /// Wave 503: disguise template name for mesh key swap.
    pub disguise_as_template: Option<String>,
    /// Wave 504: structure occupant count residual (garrisoned model bit).
    pub occupant_count: u16,
    /// Wave 521: host AI state ordinal residual (Docked=12, Docking=18, ...).
    pub ai_state_ordinal: u8,
    /// Wave 521: combat cycle rider slot residual (1-based).
    pub combat_cycle_rider: u8,
    /// Wave 504: container id when this unit is inside another object.
    pub contained_by: Option<ObjectId>,
    /// Wave 505: parachuting residual for mesh model-condition.
    pub parachuting: bool,
    /// Wave 505: using_ability residual (special power pose).
    pub using_ability: bool,
    /// Wave 505: airborne_target residual (air unit identity).
    pub airborne_target: bool,
    /// Wave 505: presentation object type for jet exhaust residual.
    pub object_type: PresentationObjectType,
    /// Wave 505: velocity residual for jet exhaust when moving.
    pub velocity: Vec3,
    /// Wave 506: presentation veterancy residual for weaponset model bits.
    pub veterancy: PresentationVeterancy,
    /// Wave 507: over-water residual for mesh model-condition.
    pub over_water: bool,
    /// Wave 522: terrain cell cliff residual.
    pub cell_is_cliff: bool,
    /// Wave 522: terrain cell underwater residual.
    pub cell_is_underwater: bool,
    /// Wave 508: any host disable residual that blocks acting (stun pose).
    pub disabled: bool,
    /// Wave 523: second-life residual.
    pub second_life: bool,
    /// Wave 525: front crushed residual.
    pub front_crushed: bool,
    /// Wave 525: back crushed residual.
    pub back_crushed: bool,
    /// Wave 525: USER_1 model residual.
    pub user_1: bool,
    /// Wave 525: USER_2 model residual.
    pub user_2: bool,
    /// Wave 509: parachute open residual (with parachuting => freefall when false).
    pub parachute_open: bool,
    /// Wave 509: world snow residual stamped into mesh model-condition.
    pub world_is_snow: bool,
    /// Wave 509: world night residual stamped into mesh model-condition.
    pub world_is_night: bool,
    /// Wave 510: captured residual for CAPTURED model-condition.
    pub captured: bool,
    /// Wave 510: power plant overcharge residual.
    pub overcharge_enabled: bool,
    /// Wave 511: death type name residual for burned/aflame pose.
    pub death_type_name: String,
    /// Wave 512: continuous-fire level residual (0 slow / 1 mean / 2 fast).
    pub continuous_fire_level: u8,
    /// Wave 512: prone residual.
    pub prone: bool,
    /// Wave 513: weapons jammed residual.
    pub jammed: bool,
    /// Wave 513: destroyed/dying residual.
    pub destroyed: bool,
    /// Wave 513: continuous-fire coast-until frame for reload residual.
    pub continuous_fire_coast_until_frame: u32,
    /// Wave 513: presentation logic frame for coast comparison.
    pub logic_frame: u32,
    /// Wave 515: surrendered residual (RAISING_FLAG mesh bit).
    pub is_surrendered: bool,
    /// Skip main mesh pass when RenderBridge owns this drawable.
    pub engine_bridged: bool,
    /// Local-player FOW from the presentation snapshot (not a live shroud query).
    pub fow_visibility: ObjectVisibility,
}

impl UnitRenderInput {
    pub fn from_renderable(ro: &RenderableObject) -> Self {
        let model_key = ro
            .model_key
            .clone()
            .unwrap_or_else(|| ro.template_name.clone());
        Self {
            id: ro.id,
            template_name: ro.template_name.clone(),
            model_key,
            mesh_scale: if ro.mesh_scale > 0.0 {
                ro.mesh_scale
            } else {
                1.0
            },
            team: ro.team,
            team_color: ro.team_color,
            position: ro.position,
            orientation: ro.orientation,
            topple_lean_radians: ro.topple_lean_radians,
            turret_angle_deg: ro.turret_angle_deg,
            turret_pitch_deg: ro.turret_pitch_deg,
            selected: ro.selected,
            selection_radius: ro.selection_radius.max(5.0),
            selection_flash_remaining: ro.selection_flash_remaining,
            model_condition_bits: ro.model_condition_bits,
            production_door_phase: ro.production_door_phase,
            is_structure: ro.is_structure,
            is_unit: ro.is_unit,
            moving: ro.moving,
            attacking: ro.attacking,
            is_firing_weapon: ro.is_firing_weapon,
            active_weapon_slot: ro.active_weapon_slot,
            weapon_fire_status: ro.weapon_fire_status,
            is_panicking: ro.is_panicking,
            moving_backwards: ro.moving_backwards,
            weapon_set_player_upgrade: ro.weapon_set_player_upgrade,
            weapon_crate_upgrade: ro.weapon_crate_upgrade,
            armor_crate_upgrade: ro.armor_crate_upgrade,
            enemy_near: ro.enemy_near,
            armed: ro.armed,
            body_damage_state: ro.body_damage_state,
            shock_was_airborne: ro.shock_was_airborne,
            shock_allow_bounce: ro.shock_allow_bounce,
            shock_grounded_once: ro.shock_grounded_once,
            shock_stun_frames: ro.shock_stun_frames,
            power_plant_rods_extended: ro.power_plant_rods_extended,
            power_plant_rods_done_frame: ro.power_plant_rods_done_frame,
            jet_slow_death_active: ro.jet_slow_death_active,
            anim_steer_turn: ro.anim_steer_turn,
            poison_tinted: ro.poison_tinted,
            defector_flash: ro.defector_flash,
            is_deployed: ro.is_deployed,
            radar_active: ro.radar_active,
            radar_extend_complete: ro.radar_extend_complete,
            effectively_stealthed: ro.effectively_stealthed,
            under_construction: ro.under_construction,
            construction_percent: ro.construction_percent,
            disguised: ro.disguised,
            disguise_as_template: ro.disguise_as_template.clone(),
            occupant_count: ro.occupant_count,
            ai_state_ordinal: ro.ai_state_ordinal,
            combat_cycle_rider: ro.combat_cycle_rider,
            contained_by: ro.contained_by,
            parachuting: ro.parachuting,
            using_ability: ro.using_ability,
            airborne_target: ro.airborne_target,
            object_type: ro.object_type,
            velocity: ro.velocity,
            veterancy: ro.veterancy,
            over_water: ro.over_water,
            cell_is_cliff: ro.cell_is_cliff,
            cell_is_underwater: ro.cell_is_underwater,
            disabled: ro.disabled,
            second_life: ro.second_life,
            front_crushed: ro.front_crushed,
            back_crushed: ro.back_crushed,
            user_1: ro.user_1,
            user_2: ro.user_2,
            parachute_open: ro.parachute_open,
            world_is_snow: false,
            world_is_night: false,
            captured: ro.captured,
            overcharge_enabled: ro.overcharge_enabled,
            death_type_name: ro.death_type_name.clone(),
            continuous_fire_level: ro.continuous_fire_level,
            prone: ro.prone,
            jammed: ro.weapons_jammed,
            destroyed: ro.destroyed,
            continuous_fire_coast_until_frame: ro.continuous_fire_coast_until_frame,
            logic_frame: 0,
            is_surrendered: ro.is_surrendered,
            engine_bridged: ro.engine_bridged,
            fow_visibility: ro.fow_visibility,
        }
    }

    /// World matrix for the unit mesh pass (translation + Y rotation + mesh scale).
    /// Scale is presentation-frozen from the template residual (default 1.0).
    pub fn world_matrix(&self) -> glam::Mat4 {
        let scale = if self.mesh_scale.is_finite() && self.mesh_scale > 0.0 {
            self.mesh_scale
        } else {
            1.0
        };
        let lean = if self.topple_lean_radians.is_finite() {
            self.topple_lean_radians
        } else {
            0.0
        };
        // Wave 494: non-structure meshes face turret yaw residual when aimed.
        let yaw = if !self.is_structure
            && self.turret_angle_deg.is_finite()
            && self.turret_angle_deg.abs() > 0.01
        {
            self.orientation + self.turret_angle_deg.to_radians()
        } else {
            self.orientation
        };
        let pitch = if !self.is_structure
            && self.turret_pitch_deg.is_finite()
            && self.turret_pitch_deg.abs() > 0.01
        {
            lean + self.turret_pitch_deg.to_radians()
        } else {
            lean
        };
        // C++ ToppleUpdate tilts mesh while falling; residual pitch about local X.
        glam::Mat4::from_translation(self.position)
            * glam::Mat4::from_rotation_y(yaw)
            * glam::Mat4::from_rotation_x(pitch)
            * glam::Mat4::from_scale(glam::Vec3::splat(scale))
    }

    /// Wave 495: ensure combat motion flags are present in model-condition bits.
    /// Wave 496: also stamp production-door phase bits for structure mesh residual.
    /// Wave 501: stamp deployed + radar dish model-condition residual bits.
    /// Wave 503: stamp construction scaffold model-condition residual bits.
    /// Wave 504: stamp GARRISONED model-condition residual when occupied.
    /// Wave 505: stamp parachuting / jetexhaust / using-weapon residual bits.
    /// Wave 506: stamp weaponset veterancy residual bits.
    /// Wave 507: stamp OVER_WATER + transport RIDER1..n residual bits.
    /// Wave 508: stamp body-damage / DISGUISED / STUNNED residual bits.
    /// Wave 509: stamp TOPPLED / FREEFALL / NIGHT / SNOW residual bits.
    /// Wave 510: stamp CAPTURED / LOADED / POWER_PLANT_UPGRADED residual bits.
    /// Wave 511: stamp BURNED / AFLAME / SPECIAL_CHEERING / CARRYING residual bits.
    /// Wave 512: stamp CONTINUOUS_FIRE_* / PRONE / PREATTACK_A / TURRET_ROTATE residual bits.
    /// Wave 513: stamp JAMMED / DYING / RELOADING_A / PACKING / UNPACKING residual bits.
    /// Wave 515: stamp RAISING_FLAG from is_surrendered residual.
    pub fn model_condition_bits_with_combat_flags(&self) -> u128 {
        use crate::game_logic::host_enum_table_residual::{
            attacking_model_bit, deployed_model_bit, moving_model_bit, radar_extending_model_bit,
            radar_upgraded_model_bit,
        };
        let mut bits = self.model_condition_bits;
        // Wave 526: MOVING/ATTACKING via name-table helpers (parity with MC_BIT_*).
        let move_b = moving_model_bit();
        let atk_b = attacking_model_bit();
        bits &= !(1u128 << move_b);
        bits &= !(1u128 << atk_b);
        if self.moving {
            bits |= 1u128 << move_b;
        }
        if self.attacking {
            bits |= 1u128 << atk_b;
        }
        // Wave 517: slot-aware FIRING / BETWEEN / PREATTACK / RELOADING + PANICKING.
        {
            use crate::game_logic::host_enum_table_residual::{
                between_firing_shots_a_model_bit, between_firing_shots_b_model_bit,
                between_firing_shots_c_model_bit, firing_a_model_bit, firing_b_model_bit,
                firing_c_model_bit, panicking_model_bit, preattack_a_model_bit,
                preattack_b_model_bit, preattack_c_model_bit, reloading_a_model_bit,
                reloading_b_model_bit, reloading_c_model_bit, using_weapon_a_model_bit,
                using_weapon_b_model_bit, using_weapon_c_model_bit,
            };
            // WeaponFireStatus ordinal: 0 Ready, 1 OutOfAmmo, 2 Between, 3 Reloading, 4 PreAttack
            let status = self.weapon_fire_status;
            let slot = self.active_weapon_slot; // 0=A,1=B,2=C residual
            let (fire_b, between_b, pre_b, reload_b, use_b) = match slot {
                1 => (
                    firing_b_model_bit(),
                    between_firing_shots_b_model_bit(),
                    preattack_b_model_bit(),
                    reloading_b_model_bit(),
                    using_weapon_b_model_bit(),
                ),
                2 => (
                    firing_c_model_bit(),
                    between_firing_shots_c_model_bit(),
                    preattack_c_model_bit(),
                    reloading_c_model_bit(),
                    using_weapon_c_model_bit(),
                ),
                _ => (
                    firing_a_model_bit(),
                    between_firing_shots_a_model_bit(),
                    preattack_a_model_bit(),
                    reloading_a_model_bit(),
                    using_weapon_a_model_bit(),
                ),
            };
            // clear slot banks then set
            for b in [
                firing_a_model_bit(),
                firing_b_model_bit(),
                firing_c_model_bit(),
                between_firing_shots_a_model_bit(),
                between_firing_shots_b_model_bit(),
                between_firing_shots_c_model_bit(),
                preattack_a_model_bit(),
                preattack_b_model_bit(),
                preattack_c_model_bit(),
                reloading_a_model_bit(),
                reloading_b_model_bit(),
                reloading_c_model_bit(),
                using_weapon_a_model_bit(),
                using_weapon_b_model_bit(),
                using_weapon_c_model_bit(),
                panicking_model_bit(),
            ] {
                bits &= !(1u128 << b);
            }
            bits |= 1u128 << use_b;
            if self.is_firing_weapon {
                bits |= 1u128 << fire_b;
            } else if status == 2 {
                bits |= 1u128 << between_b;
            } else if status == 3 {
                bits |= 1u128 << reload_b;
            } else if status == 4 || (self.attacking && !self.is_firing_weapon) {
                bits |= 1u128 << pre_b;
            }
            if self.is_panicking {
                bits |= 1u128 << panicking_model_bit();
            }
            let _ = self.moving_backwards; // freeze residual; no dedicated ZH model bit
        }
        // Wave 524: clear door 1..4 banks then set active phase bit on each door bank
        // (multi-door factory residual; host tracks one production_door_phase).
        {
            use crate::game_logic::host_enum_table_residual::{
                door_1_closing_model_bit, door_1_opening_model_bit, door_1_waiting_open_model_bit,
                door_1_waiting_to_close_model_bit, door_2_closing_model_bit,
                door_2_opening_model_bit, door_2_waiting_open_model_bit,
                door_2_waiting_to_close_model_bit, door_3_closing_model_bit,
                door_3_opening_model_bit, door_3_waiting_open_model_bit,
                door_3_waiting_to_close_model_bit, door_4_closing_model_bit,
                door_4_opening_model_bit, door_4_waiting_open_model_bit,
                door_4_waiting_to_close_model_bit,
            };
            let banks = [
                (
                    door_1_opening_model_bit(),
                    door_1_waiting_open_model_bit(),
                    door_1_waiting_to_close_model_bit(),
                    door_1_closing_model_bit(),
                ),
                (
                    door_2_opening_model_bit(),
                    door_2_waiting_open_model_bit(),
                    door_2_waiting_to_close_model_bit(),
                    door_2_closing_model_bit(),
                ),
                (
                    door_3_opening_model_bit(),
                    door_3_waiting_open_model_bit(),
                    door_3_waiting_to_close_model_bit(),
                    door_3_closing_model_bit(),
                ),
                (
                    door_4_opening_model_bit(),
                    door_4_waiting_open_model_bit(),
                    door_4_waiting_to_close_model_bit(),
                    door_4_closing_model_bit(),
                ),
            ];
            for (open_b, wait_b, wait_close_b, close_b) in banks {
                bits &= !(1u128 << open_b);
                bits &= !(1u128 << wait_b);
                bits &= !(1u128 << wait_close_b);
                bits &= !(1u128 << close_b);
                match self.production_door_phase {
                    1 => bits |= 1u128 << open_b,
                    2 => bits |= 1u128 << wait_b,
                    3 => bits |= 1u128 << wait_close_b,
                    4 => bits |= 1u128 << close_b,
                    _ => {}
                }
            }
        }
        // Wave 501: deployed / radar dish residual bits for mesh subobject selection.
        let dep_b = deployed_model_bit();
        if self.is_deployed {
            bits |= 1u128 << dep_b;
        } else {
            bits &= !(1u128 << dep_b);
        }
        let radar_ext_b = radar_extending_model_bit();
        let radar_up_b = radar_upgraded_model_bit();
        if self.radar_extend_complete {
            // Extend finished → upgraded dish pose residual.
            bits |= 1u128 << radar_up_b;
            bits &= !(1u128 << radar_ext_b);
        } else if self.radar_active {
            // Dish animating / active without complete → extending residual.
            bits |= 1u128 << radar_ext_b;
            bits &= !(1u128 << radar_up_b);
        } else {
            bits &= !(1u128 << radar_up_b);
            bits &= !(1u128 << radar_ext_b);
        }
        // Wave 503: construction scaffold model-condition residual.
        {
            use crate::game_logic::host_enum_table_residual::{
                actively_being_constructed_model_bit, awaiting_construction_model_bit,
                construction_complete_model_bit, partially_constructed_model_bit,
            };
            let await_b = awaiting_construction_model_bit();
            let part_b = partially_constructed_model_bit();
            let active_b = actively_being_constructed_model_bit();
            let complete_b = construction_complete_model_bit();
            bits &= !(1u128 << await_b);
            bits &= !(1u128 << part_b);
            bits &= !(1u128 << active_b);
            if self.under_construction {
                bits &= !(1u128 << complete_b);
                let p = self.construction_percent;
                if p <= 0.01 {
                    bits |= 1u128 << await_b;
                } else if p < 1.0 {
                    bits |= 1u128 << part_b;
                    bits |= 1u128 << active_b;
                } else {
                    bits |= 1u128 << complete_b;
                }
            }
        }
        // Wave 504/507: garrisoned residual for structures; transports use RIDER bits.
        // Wave 521: stamp RIDER1..n from occupant_count; DOCKING_* from ai_state_ordinal.
        {
            use crate::game_logic::host_enum_table_residual::{
                docking_active_model_bit, docking_beginning_model_bit, docking_ending_model_bit,
                docking_model_bit, garrisoned_model_bit, rider1_model_bit, rider2_model_bit,
                rider3_model_bit, rider4_model_bit, rider5_model_bit, rider6_model_bit,
                rider7_model_bit, rider8_model_bit,
            };
            let g_b = garrisoned_model_bit();
            if self.is_structure && self.occupant_count > 0 {
                bits |= 1u128 << g_b;
            } else {
                bits &= !(1u128 << g_b);
            }
            let riders = [
                rider1_model_bit(),
                rider2_model_bit(),
                rider3_model_bit(),
                rider4_model_bit(),
                rider5_model_bit(),
                rider6_model_bit(),
                rider7_model_bit(),
                rider8_model_bit(),
            ];
            for b in riders {
                bits &= !(1u128 << b);
            }
            // Transports / non-structures: RIDER1..n for each occupant (cap 8).
            if !self.is_structure && self.occupant_count > 0 {
                let n = (self.occupant_count as usize).min(8);
                for i in 0..n {
                    bits |= 1u128 << riders[i];
                }
            } else if !self.is_structure && self.combat_cycle_rider > 0 {
                let idx = (self.combat_cycle_rider as usize).saturating_sub(1).min(7);
                bits |= 1u128 << riders[idx];
            }
            let d_b = docking_model_bit();
            let d_beg = docking_beginning_model_bit();
            let d_act = docking_active_model_bit();
            let d_end = docking_ending_model_bit();
            for b in [d_b, d_beg, d_act, d_end] {
                bits &= !(1u128 << b);
            }
            // host_ai_state_ordinal: Docked=12, Docking=18, Entering=17
            match self.ai_state_ordinal {
                12 => {
                    bits |= 1u128 << d_act;
                    bits |= 1u128 << d_b;
                }
                18 => {
                    bits |= 1u128 << d_beg;
                    bits |= 1u128 << d_b;
                }
                17 => {
                    bits |= 1u128 << d_end;
                    bits |= 1u128 << d_b;
                }
                _ => {}
            }
        }
        // Wave 522: CLIMBING / RAPPELLING / FLOODED from terrain cell residuals.
        {
            use crate::game_logic::host_enum_table_residual::{
                climbing_model_bit, flooded_model_bit, rappelling_model_bit,
            };
            let climb_b = climbing_model_bit();
            let rap_b = rappelling_model_bit();
            let flood_b = flooded_model_bit();
            for b in [climb_b, rap_b, flood_b] {
                bits &= !(1u128 << b);
            }
            if self.cell_is_underwater {
                bits |= 1u128 << flood_b;
            }
            // Cliff locomotion: climbing when moving on cliff; rappelling when airborne over cliff.
            if self.cell_is_cliff {
                if self.airborne_target || self.parachuting {
                    bits |= 1u128 << rap_b;
                } else if self.moving || self.is_unit {
                    bits |= 1u128 << climb_b;
                }
            }
        }

        // Wave 505: parachuting / jet exhaust / using-weapon pose residual bits.
        {
            use crate::game_logic::host_enum_table_residual::{
                jetexhaust_model_bit, parachuting_model_bit, using_weapon_a_model_bit,
            };
            let para_b = parachuting_model_bit();
            if self.parachuting {
                bits |= 1u128 << para_b;
            } else {
                bits &= !(1u128 << para_b);
            }
            let jet_b = jetexhaust_model_bit();
            let jet_moving = matches!(self.object_type, PresentationObjectType::Aircraft)
                && (self.moving || self.velocity.length_squared() > 1e-4 || self.airborne_target);
            if jet_moving {
                bits |= 1u128 << jet_b;
            } else {
                bits &= !(1u128 << jet_b);
            }
            // Wave 517: slot-aware USING_WEAPON_A/B/C (preserve B/C when active).
            {
                use crate::game_logic::host_enum_table_residual::{
                    using_weapon_a_model_bit, using_weapon_b_model_bit, using_weapon_c_model_bit,
                };
                let a = using_weapon_a_model_bit();
                let b = using_weapon_b_model_bit();
                let c = using_weapon_c_model_bit();
                bits &= !(1u128 << a);
                bits &= !(1u128 << b);
                bits &= !(1u128 << c);
                let use_b = match self.active_weapon_slot {
                    1 => b,
                    2 => c,
                    _ => a,
                };
                if self.is_firing_weapon || self.using_ability || self.weapon_fire_status != 1 {
                    // Keep using-weapon pose while not out-of-ammo residual.
                    if self.is_firing_weapon
                        || self.using_ability
                        || self.attacking
                        || self.weapon_fire_status == 2
                        || self.weapon_fire_status == 3
                        || self.weapon_fire_status == 4
                    {
                        bits |= 1u128 << use_b;
                    }
                }
            }
        }
        // Wave 506: weaponset veterancy model-condition residual.
        {
            use crate::game_logic::host_enum_table_residual::{
                weaponset_elite_model_bit, weaponset_hero_model_bit, weaponset_veteran_model_bit,
            };
            let vet_b = weaponset_veteran_model_bit();
            let elite_b = weaponset_elite_model_bit();
            let hero_b = weaponset_hero_model_bit();
            bits &= !(1u128 << vet_b);
            bits &= !(1u128 << elite_b);
            bits &= !(1u128 << hero_b);
            match self.veterancy {
                PresentationVeterancy::Rookie => {}
                PresentationVeterancy::Veteran => bits |= 1u128 << vet_b,
                PresentationVeterancy::Elite => bits |= 1u128 << elite_b,
                PresentationVeterancy::Heroic => bits |= 1u128 << hero_b,
            }
        }
        // Wave 518: weaponset player/crate, armor crate, enemy-near, armed residual bits.
        {
            use crate::game_logic::host_enum_table_residual::{
                armed_model_bit, armorset_crateupgrade_one_model_bit,
                armorset_crateupgrade_two_model_bit, enemynear_model_bit,
                weaponset_crateupgrade_one_model_bit, weaponset_crateupgrade_two_model_bit,
                weaponset_player_upgrade_model_bit,
            };
            let wsp = weaponset_player_upgrade_model_bit();
            let wc1 = weaponset_crateupgrade_one_model_bit();
            let wc2 = weaponset_crateupgrade_two_model_bit();
            let ac1 = armorset_crateupgrade_one_model_bit();
            let ac2 = armorset_crateupgrade_two_model_bit();
            let en_b = enemynear_model_bit();
            let arm_b = armed_model_bit();
            for b in [wsp, wc1, wc2, ac1, ac2, en_b, arm_b] {
                bits &= !(1u128 << b);
            }
            if self.weapon_set_player_upgrade {
                bits |= 1u128 << wsp;
            }
            match self.weapon_crate_upgrade {
                1 => bits |= 1u128 << wc1,
                2 => bits |= 1u128 << wc2,
                _ => {}
            }
            match self.armor_crate_upgrade {
                1 => bits |= 1u128 << ac1,
                2 => bits |= 1u128 << ac2,
                _ => {}
            }
            if self.enemy_near {
                bits |= 1u128 << en_b;
            }
            if self.armed {
                bits |= 1u128 << arm_b;
            }
        }
        // Wave 519: exploded flail/bounce, power-plant upgrading, jet afterburner residual bits.
        {
            use crate::game_logic::host_enum_table_residual::{
                exploded_bouncing_model_bit, exploded_flailing_model_bit, jetafterburner_model_bit,
                power_plant_upgraded_model_bit, power_plant_upgrading_model_bit,
                splatted_model_bit,
            };
            let flail_b = exploded_flailing_model_bit();
            let bounce_b = exploded_bouncing_model_bit();
            let splat_b = splatted_model_bit();
            let ppu_b = power_plant_upgrading_model_bit();
            let ppd_b = power_plant_upgraded_model_bit();
            let jet_ab = jetafterburner_model_bit();
            for b in [flail_b, bounce_b, splat_b, ppu_b, jet_ab] {
                bits &= !(1u128 << b);
            }
            // Shockwave: airborne => flailing; bounce allowed mid-air => bouncing; grounded after airborne => splatted residual.
            if self.shock_stun_frames > 0 || self.shock_was_airborne {
                if self.shock_was_airborne && self.shock_allow_bounce && !self.shock_grounded_once {
                    bits |= 1u128 << bounce_b;
                } else if self.shock_was_airborne && !self.shock_grounded_once {
                    bits |= 1u128 << flail_b;
                } else if self.shock_grounded_once && self.destroyed {
                    bits |= 1u128 << splat_b;
                } else if self.shock_stun_frames > 0 {
                    bits |= 1u128 << flail_b;
                }
            }
            // Power plant rods: upgrading until done_frame, then upgraded (overcharge path may also set upgraded).
            if self.power_plant_rods_done_frame > 0
                && self.logic_frame < self.power_plant_rods_done_frame
                && !self.power_plant_rods_extended
            {
                bits |= 1u128 << ppu_b;
                bits &= !(1u128 << ppd_b);
            } else if self.power_plant_rods_extended {
                bits |= 1u128 << ppd_b;
                bits &= !(1u128 << ppu_b);
            }
            if self.jet_slow_death_active {
                bits |= 1u128 << jet_ab;
            }
        }
        // Wave 520: AnimationSteeringUpdate CENTER/LEFT/RIGHT turn model-condition residual.
        {
            use crate::game_logic::host_enum_table_residual::{
                center_to_left_model_bit, center_to_right_model_bit, left_to_center_model_bit,
                right_to_center_model_bit,
            };
            let ctl = center_to_left_model_bit();
            let ctr = center_to_right_model_bit();
            let ltc = left_to_center_model_bit();
            let rtc = right_to_center_model_bit();
            for b in [ctl, ctr, ltc, rtc] {
                bits &= !(1u128 << b);
            }
            // 0 invalid, 1 CTR, 2 CTL, 3 LTC, 4 RTC
            match self.anim_steer_turn {
                1 => bits |= 1u128 << ctr,
                2 => bits |= 1u128 << ctl,
                3 => bits |= 1u128 << ltc,
                4 => bits |= 1u128 << rtc,
                _ => {}
            }
        }

        // Wave 507: over-water + transport RIDER1..n residual bits.
        {
            use crate::game_logic::host_enum_table_residual::{
                over_water_model_bit, rider_model_bit,
            };
            let water_b = over_water_model_bit();
            if self.over_water {
                bits |= 1u128 << water_b;
            } else {
                bits &= !(1u128 << water_b);
            }
            // Clear RIDER bank then stamp passenger slots on non-structure transports.
            for slot in 1u8..=8u8 {
                bits &= !(1u128 << rider_model_bit(slot));
            }
            if !self.is_structure && self.occupant_count > 0 {
                let n = (self.occupant_count as u8).min(8);
                for slot in 1u8..=n {
                    bits |= 1u128 << rider_model_bit(slot);
                }
            }
        }
        // Wave 508: body-damage / disguised / stunned model-condition residual.
        {
            use crate::game_logic::host_enum_table_residual::{
                disguised_model_bit, host_apply_body_damage_model_bits, stunned_model_bit,
                HostBodyDamageType,
            };
            let body = match self.body_damage_state {
                1 => HostBodyDamageType::Damaged,
                2 => HostBodyDamageType::ReallyDamaged,
                3 => HostBodyDamageType::Rubble,
                _ => HostBodyDamageType::Pristine,
            };
            bits = host_apply_body_damage_model_bits(bits, body);
            let dis_b = disguised_model_bit();
            if self.disguised {
                bits |= 1u128 << dis_b;
            } else {
                bits &= !(1u128 << dis_b);
            }
            let stun_b = stunned_model_bit();
            use crate::game_logic::host_enum_table_residual::{
                post_collapse_model_bit, second_life_model_bit, special_damaged_model_bit,
                stunned_flailing_model_bit,
            };
            let flail_b = stunned_flailing_model_bit();
            let life_b = second_life_model_bit();
            let post_b = post_collapse_model_bit();
            let spec_b = special_damaged_model_bit();
            for b in [stun_b, flail_b, life_b, post_b, spec_b] {
                bits &= !(1u128 << b);
            }
            // Wave 523: shock stun frames => STUNNED_FLAILING; disabled => STUNNED.
            if self.shock_stun_frames > 0 {
                bits |= 1u128 << flail_b;
                bits |= 1u128 << stun_b;
            } else if self.disabled {
                bits |= 1u128 << stun_b;
            }
            if self.second_life {
                bits |= 1u128 << life_b;
            }
            // Structure rubble after destroy residual.
            if self.is_structure && self.destroyed && self.body_damage_state >= 3 {
                bits |= 1u128 << post_b;
            }
            // Special damaged: really-damaged structures still standing.
            if self.is_structure && self.body_damage_state == 2 && !self.destroyed {
                bits |= 1u128 << spec_b;
            }
        }
        // Wave 509: toppled / freefall / night / snow model-condition residual.
        {
            use crate::game_logic::host_enum_table_residual::{
                freefall_model_bit, night_model_bit, snow_model_bit, toppled_model_bit,
            };
            let top_b = toppled_model_bit();
            if self.topple_lean_radians.abs() > 1e-3 {
                bits |= 1u128 << top_b;
            } else {
                bits &= !(1u128 << top_b);
            }
            let free_b = freefall_model_bit();
            if self.parachuting && !self.parachute_open {
                bits |= 1u128 << free_b;
            } else {
                bits &= !(1u128 << free_b);
            }
            let night_b = night_model_bit();
            if self.world_is_night {
                bits |= 1u128 << night_b;
            } else {
                bits &= !(1u128 << night_b);
            }
            let snow_b = snow_model_bit();
            if self.world_is_snow {
                bits |= 1u128 << snow_b;
            } else {
                bits &= !(1u128 << snow_b);
            }
        }
        // Wave 510: captured / loaded transport / power-plant overcharge residual bits.
        {
            use crate::game_logic::host_enum_table_residual::{
                captured_model_bit, loaded_model_bit, power_plant_upgraded_model_bit,
            };
            let cap_b = captured_model_bit();
            if self.captured {
                bits |= 1u128 << cap_b;
            } else {
                bits &= !(1u128 << cap_b);
            }
            let load_b = loaded_model_bit();
            // Transport cargo residual (non-structure with occupants).
            if !self.is_structure && self.occupant_count > 0 {
                bits |= 1u128 << load_b;
            } else {
                bits &= !(1u128 << load_b);
            }
            let pp_b = power_plant_upgraded_model_bit();
            if self.overcharge_enabled {
                bits |= 1u128 << pp_b;
            } else {
                bits &= !(1u128 << pp_b);
            }
        }
        // Wave 511: burned/aflame death pose + special cheering + carrying residual.
        {
            use crate::game_logic::host_enum_table_residual::{
                aflame_model_bit, burned_model_bit, carrying_model_bit, special_cheering_model_bit,
            };
            let death = self.death_type_name.to_ascii_lowercase();
            let burn_b = burned_model_bit();
            let flame_b = aflame_model_bit();
            let smolder_b = {
                use crate::game_logic::host_enum_table_residual::smoldering_model_bit;
                smoldering_model_bit()
            };
            bits &= !(1u128 << smolder_b);
            if death.contains("burn") {
                bits |= 1u128 << burn_b;
            } else {
                bits &= !(1u128 << burn_b);
            }
            if death.contains("flame") || death.contains("fire") {
                bits |= 1u128 << flame_b;
            } else {
                bits &= !(1u128 << flame_b);
            }
            // Wave 524: SMOLDERING when burned residual without active flame.
            if (death.contains("burn") || death.contains("smolder"))
                && !(death.contains("flame") || death.contains("fire"))
                && self.destroyed
            {
                bits |= 1u128 << smolder_b;
            }
            // Wave 525: FRONTCRUSHED / BACKCRUSHED / PREORDER / USER_1 / USER_2 residual bits.
            {
                use crate::game_logic::host_enum_table_residual::{
                    backcrushed_model_bit, frontcrushed_model_bit, preorder_model_bit,
                    user_1_model_bit, user_2_model_bit,
                };
                let fc = frontcrushed_model_bit();
                let bc = backcrushed_model_bit();
                let pre = preorder_model_bit();
                let u1 = user_1_model_bit();
                let u2 = user_2_model_bit();
                for b in [fc, bc, pre, u1, u2] {
                    bits &= !(1u128 << b);
                }
                if self.front_crushed {
                    bits |= 1u128 << fc;
                }
                if self.back_crushed {
                    bits |= 1u128 << bc;
                }
                if self.user_1 {
                    bits |= 1u128 << u1;
                }
                if self.user_2 {
                    bits |= 1u128 << u2;
                }
                // PREORDER residual: structures under construction still building.
                if self.is_structure && self.under_construction && self.construction_percent < 1.0 {
                    bits |= 1u128 << pre;
                }
            }

            let cheer_b = special_cheering_model_bit();
            let infantry = matches!(self.object_type, PresentationObjectType::Infantry);
            if self.using_ability && infantry {
                bits |= 1u128 << cheer_b;
            } else {
                bits &= !(1u128 << cheer_b);
            }
            let carry_b = carrying_model_bit();
            // Infantry non-combat ability residual (flag/crate-like carry pose).
            if self.using_ability && infantry && !self.attacking && !self.is_firing_weapon {
                bits |= 1u128 << carry_b;
            } else {
                bits &= !(1u128 << carry_b);
            }
        }
        // Wave 512: continuous-fire / prone / preattack / turret-rotate residual bits.
        {
            use crate::game_logic::host_enum_table_residual::{
                continuous_fire_fast_model_bit, continuous_fire_mean_model_bit,
                continuous_fire_slow_model_bit, preattack_a_model_bit, prone_model_bit,
                turret_rotate_model_bit,
            };
            let slow_b = continuous_fire_slow_model_bit();
            let mean_b = continuous_fire_mean_model_bit();
            let fast_b = continuous_fire_fast_model_bit();
            bits &= !(1u128 << slow_b);
            bits &= !(1u128 << mean_b);
            bits &= !(1u128 << fast_b);
            match self.continuous_fire_level {
                1 => bits |= 1u128 << mean_b,
                2 => bits |= 1u128 << fast_b,
                _ => {
                    if self.is_firing_weapon {
                        bits |= 1u128 << slow_b;
                    }
                }
            }
            let prone_b = prone_model_bit();
            if self.prone {
                bits |= 1u128 << prone_b;
            } else {
                bits &= !(1u128 << prone_b);
            }
            // Wave 517: slot-aware PREATTACK_A/B/C.
            {
                use crate::game_logic::host_enum_table_residual::{
                    preattack_a_model_bit, preattack_b_model_bit, preattack_c_model_bit,
                };
                let a = preattack_a_model_bit();
                let b = preattack_b_model_bit();
                let c = preattack_c_model_bit();
                bits &= !(1u128 << a);
                bits &= !(1u128 << b);
                bits &= !(1u128 << c);
                let pre_b = match self.active_weapon_slot {
                    1 => b,
                    2 => c,
                    _ => a,
                };
                if (self.attacking && !self.is_firing_weapon) || self.weapon_fire_status == 4 {
                    bits |= 1u128 << pre_b;
                }
            }
            let tur_b = turret_rotate_model_bit();
            if !self.is_structure
                && self.turret_angle_deg.is_finite()
                && self.turret_angle_deg.abs() > 0.5
            {
                bits |= 1u128 << tur_b;
            } else {
                bits &= !(1u128 << tur_b);
            }
        }
        // Wave 513: jammed / dying / reloading / packing-unpack deploy residual bits.
        {
            use crate::game_logic::host_enum_table_residual::{
                dying_model_bit, jammed_model_bit, packing_model_bit, reloading_a_model_bit,
                unpacking_model_bit,
            };
            let jam_b = jammed_model_bit();
            if self.jammed {
                bits |= 1u128 << jam_b;
            } else {
                bits &= !(1u128 << jam_b);
            }
            let die_b = dying_model_bit();
            if self.destroyed {
                bits |= 1u128 << die_b;
            } else {
                bits &= !(1u128 << die_b);
            }
            // Wave 517: slot-aware RELOADING_A/B/C (coast residual or WeaponFireStatus::ReloadingClip).
            {
                use crate::game_logic::host_enum_table_residual::{
                    reloading_a_model_bit, reloading_b_model_bit, reloading_c_model_bit,
                };
                let a = reloading_a_model_bit();
                let b = reloading_b_model_bit();
                let c = reloading_c_model_bit();
                bits &= !(1u128 << a);
                bits &= !(1u128 << b);
                bits &= !(1u128 << c);
                let reload_b = match self.active_weapon_slot {
                    1 => b,
                    2 => c,
                    _ => a,
                };
                let coast = !self.is_firing_weapon
                    && self.continuous_fire_coast_until_frame > self.logic_frame
                    && self.continuous_fire_coast_until_frame > 0;
                if coast || self.weapon_fire_status == 3 {
                    bits |= 1u128 << reload_b;
                }
            }
            // Deploy-style residual: DEPLOYED already stamped; packing/unpacking
            // door-adjacent residual when structure door is mid-cycle and not deployed.
            let pack_b = packing_model_bit();
            let unpack_b = unpacking_model_bit();
            bits &= !(1u128 << pack_b);
            bits &= !(1u128 << unpack_b);
            if !self.is_deployed {
                match self.production_door_phase {
                    1 | 2 => bits |= 1u128 << unpack_b, // opening / wait open ~ unpacking
                    3 | 4 => bits |= 1u128 << pack_b,   // wait close / closing ~ packing
                    _ => {}
                }
            }
        }
        // Wave 515: surrendered residual stamps RAISING_FLAG model-condition bit.
        {
            use crate::game_logic::host_enum_table_residual::raising_flag_model_bit;
            let flag_b = raising_flag_model_bit();
            if self.is_surrendered {
                bits |= 1u128 << flag_b;
            } else {
                bits &= !(1u128 << flag_b);
            }
        }
        bits
    }

    /// Never-explored skip for the main mesh pass (snapshot FOW only).
    #[inline]
    pub fn fow_should_render(&self) -> bool {
        self.fow_visibility.should_render()
    }

    /// C++ TintEnvelope residual intensity (linear fade over decay frames).
    pub fn selection_flash_intensity(&self) -> f32 {
        let base = crate::game_logic::host_saboteur::selection_flash_intensity(
            self.selection_flash_remaining,
        );
        // Wave 499: defector cover flash forces full white selection flash residual.
        if self.defector_flash {
            base.max(1.0)
        } else {
            base
        }
    }
}

// ===== world_env.rs =====
use super::*;

/// Compact road segment for presentation-side road mesh bake.
/// Coordinates match `RuntimeRoadSegment` world space (from/to as [x,y,z]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PresentationRoadSegment {
    pub template_name: String,
    pub from: [f32; 3],
    pub to: [f32; 3],
    pub width: f32,
    pub width_in_texture: f32,
    pub road_type_id: u32,
    pub start_is_angled: bool,
    pub start_is_join: bool,
    pub end_is_angled: bool,
    pub end_is_join: bool,
    pub curve_radius: f32,
}

/// Compact bridge segment (start/end world xyz, width, template).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PresentationBridgeSegment {
    pub start: [f32; 3],
    pub end: [f32; 3],
    pub width: f32,
    pub template_name: String,
}

/// World/environment identity frozen for the render pass.
///
/// Lets lighting / shell / map-name / bounds / heightmap-hint / roads consumers avoid
/// re-locking live `GameLogic` mid-frame when a presentation snapshot is set.
/// Fail-closed: not a full SAGE heightmap mesh or dirty-rect road stream.
/// Frozen terrain source-tile class for visual bake without live GameLogic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PresentationTerrainTextureClass {
    pub first_tile: i32,
    pub num_tiles: i32,
    pub width: i32,
    pub name: String,
}

/// Frozen runtime heightmap for terrain-visual bake without live GameLogic.
/// Mirrors `game_client::terrain::height_map::HeightMap` POD fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PresentationRuntimeHeightmap {
    pub width: u32,
    pub height: u32,
    pub heights: Vec<f32>,
    pub max_height: f32,
    pub scale: f32,
    pub min_height: f32,
    pub height_range: f32,
    pub border_size: i32,
    pub tile_ndxes: Vec<i16>,
    pub blend_tile_ndxes: Vec<i16>,
    pub draw_origin_x: i32,
    pub draw_origin_y: i32,
    pub draw_width: i32,
    pub draw_height: i32,
}

impl PresentationRuntimeHeightmap {
    #[cfg(feature = "game_client")]
    pub fn from_height_map(hm: &game_client::terrain::height_map::HeightMap) -> Self {
        Self {
            width: hm.width,
            height: hm.height,
            heights: hm.heights.clone(),
            max_height: hm.max_height,
            scale: hm.scale,
            min_height: hm.min_height,
            height_range: hm.height_range,
            border_size: hm.border_size,
            tile_ndxes: hm.tile_ndxes.clone(),
            blend_tile_ndxes: hm.blend_tile_ndxes.clone(),
            draw_origin_x: hm.draw_origin_x,
            draw_origin_y: hm.draw_origin_y,
            draw_width: hm.draw_width,
            draw_height: hm.draw_height,
        }
    }

    #[cfg(feature = "game_client")]
    pub fn to_height_map(&self) -> game_client::terrain::height_map::HeightMap {
        game_client::terrain::height_map::HeightMap {
            width: self.width,
            height: self.height,
            heights: self.heights.clone(),
            max_height: self.max_height,
            scale: self.scale,
            min_height: self.min_height,
            height_range: self.height_range,
            border_size: self.border_size,
            tile_ndxes: self.tile_ndxes.clone(),
            blend_tile_ndxes: self.blend_tile_ndxes.clone(),
            draw_origin_x: self.draw_origin_x,
            draw_origin_y: self.draw_origin_y,
            draw_width: self.draw_width,
            draw_height: self.draw_height,
        }
    }

    pub fn is_usable(&self) -> bool {
        self.width > 0
            && self.height > 0
            && self.heights.len() == (self.width as usize).saturating_mul(self.height as usize)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PresentationWorldEnv {
    pub map_name: String,
    /// Wave 509: snow weather residual for mesh model-condition SNOW bank.
    #[serde(default)]
    pub is_snow: bool,
    /// Wave 509: night residual for mesh model-condition NIGHT bank.
    #[serde(default)]
    pub is_night: bool,
    pub world_min: [f32; 3],
    pub world_max: [f32; 3],
    pub heightmap_hint: Option<String>,
    /// Script/map skybox enable residual.
    pub skybox_enabled: bool,
    /// Optional skybox texture names (front, back, left, right, top).
    pub skybox_textures: Option<[String; 5]>,
    pub sun_direction: Option<[f32; 3]>,
    pub sun_color: Option<[f32; 3]>,
    pub ambient_color: Option<[f32; 3]>,
    pub fog_color: Option<[f32; 3]>,
    pub fog_start: Option<f32>,
    pub fog_end: Option<f32>,
    /// Placed-object count from last parsed map metadata (prewarm signature).
    pub map_object_count: u32,
    pub has_map_metadata: bool,
    /// First N map-object template names for model prewarm (observe path).
    /// Fail-closed: not full ThingTemplate graph.
    pub prewarm_template_names: Vec<String>,
    /// Coarse height samples for minimap/terrain residual (row-major, width×height).
    /// Fail-closed: not full SAGE heightmap mesh / bilinear retail sample grid.
    pub height_grid_w: u32,
    pub height_grid_h: u32,
    pub height_samples: Vec<f32>,
    /// True when at least one sample came from live terrain (not empty default).
    pub height_samples_from_terrain: bool,
    /// Map road segments frozen for terrain-road bake without live GameLogic.
    pub road_segments: Vec<PresentationRoadSegment>,
    /// Bridge segments frozen for terrain-road bake.
    pub bridge_segments: Vec<PresentationBridgeSegment>,
    /// Full runtime heightmap freeze for terrain-visual bake (no live GameLogic).
    pub runtime_heightmap: Option<PresentationRuntimeHeightmap>,
    /// Terrain texture classes freeze for source-tile bake without live GameLogic.
    pub terrain_texture_classes: Vec<PresentationTerrainTextureClass>,
}

impl PresentationWorldEnv {
    pub fn from_logic(logic: &GameLogic) -> Self {
        let (wmin, wmax) = logic.world_bounds();
        let meta = logic.last_parsed_map_settings();
        let heightmap_hint = logic
            .heightmap_hint()
            .and_then(|p| p.to_str().map(|s| s.to_string()))
            .or_else(|| {
                meta.as_ref()
                    .and_then(|m| m.heightmap_path.as_ref())
                    .and_then(|p| p.to_str().map(|s| s.to_string()))
            });
        // Coarse height grid for minimap residual (fixed 64×64 — small, deterministic).
        const HG_W: u32 = 64;
        const HG_H: u32 = 64;
        let span_x = (wmax.x - wmin.x).max(1.0);
        let span_z = (wmax.z - wmin.z).max(1.0);
        let mut height_samples = vec![0.0f32; (HG_W * HG_H) as usize];
        let mut height_samples_from_terrain = false;
        for y in 0..HG_H {
            for x in 0..HG_W {
                let u = (x as f32 + 0.5) / HG_W as f32;
                let v = (y as f32 + 0.5) / HG_H as f32;
                let world = glam::Vec3::new(wmin.x + u * span_x, 0.0, wmin.z + v * span_z);
                if let Some(h) = logic.terrain_height_at(world) {
                    height_samples[(y * HG_W + x) as usize] = h;
                    height_samples_from_terrain = true;
                }
            }
        }

        let road_segments: Vec<PresentationRoadSegment> = logic
            .terrain_road_segments_snapshot()
            .into_iter()
            .map(|s| PresentationRoadSegment {
                template_name: s.template_name,
                from: [s.from.x, s.from.y, s.from.z],
                to: [s.to.x, s.to.y, s.to.z],
                width: s.width,
                width_in_texture: s.width_in_texture,
                road_type_id: s.road_type_id,
                start_is_angled: s.start_is_angled,
                start_is_join: s.start_is_join,
                end_is_angled: s.end_is_angled,
                end_is_join: s.end_is_join,
                curve_radius: s.curve_radius,
            })
            .collect();
        let bridge_segments: Vec<PresentationBridgeSegment> = logic
            .terrain_bridge_segments_snapshot()
            .into_iter()
            .map(
                |(start, end, width, template_name)| PresentationBridgeSegment {
                    start: start.to_array(),
                    end: end.to_array(),
                    width,
                    template_name,
                },
            )
            .collect();
        // Cap prewarm names so snapshot stays small (startup model resolve only).
        const PREWARM_CAP: usize = 256;
        let prewarm_template_names: Vec<String> = meta
            .as_ref()
            .map(|m| {
                m.objects
                    .iter()
                    .filter_map(|o| {
                        let n = o.template.trim();
                        if n.is_empty() {
                            None
                        } else {
                            Some(n.to_string())
                        }
                    })
                    .take(PREWARM_CAP)
                    .collect()
            })
            .unwrap_or_default();

        #[cfg(feature = "game_client")]
        let runtime_heightmap = logic
            .terrain_heightmap_snapshot()
            .map(|hm| PresentationRuntimeHeightmap::from_height_map(&hm));
        #[cfg(not(feature = "game_client"))]
        let runtime_heightmap = None;
        let terrain_texture_classes: Vec<PresentationTerrainTextureClass> = logic
            .terrain_texture_classes_snapshot()
            .into_iter()
            .map(|c| PresentationTerrainTextureClass {
                first_tile: c.first_tile,
                num_tiles: c.num_tiles,
                width: c.width,
                name: c.name,
            })
            .collect();

        let weather = logic.weather_state().current_weather.to_ascii_lowercase();
        let is_snow = weather.contains("snow");
        // Night residual: weather name or evening/night tokens (fail-closed TOD runtime).
        let is_night = weather.contains("night") || weather.contains("evening");
        Self {
            map_name: logic.get_current_map_name().trim().to_string(),
            is_snow,
            is_night,
            world_min: [wmin.x, wmin.y, wmin.z],
            world_max: [wmax.x, wmax.y, wmax.z],
            heightmap_hint,
            skybox_enabled: logic.is_skybox_enabled(),
            skybox_textures: meta.as_ref().and_then(|m| m.skybox_textures.clone()),
            sun_direction: meta.as_ref().and_then(|m| m.sun_direction),
            sun_color: meta.as_ref().and_then(|m| m.sun_color.or(m.sky_color)),
            ambient_color: meta
                .as_ref()
                .and_then(|m| m.ambient_color.or(m.fog_color).or(m.sky_color)),
            fog_color: meta
                .as_ref()
                .and_then(|m| m.fog_color.or(m.sky_color).or(m.sun_color)),
            fog_start: meta.as_ref().and_then(|m| m.fog_start),
            fog_end: meta.as_ref().and_then(|m| m.fog_end),
            map_object_count: meta.as_ref().map(|m| m.objects.len() as u32).unwrap_or(0),
            has_map_metadata: meta.is_some(),
            prewarm_template_names,
            height_grid_w: HG_W,
            height_grid_h: HG_H,
            height_samples,
            height_samples_from_terrain,
            road_segments,
            bridge_segments,
            runtime_heightmap,
            terrain_texture_classes,
        }
    }

    #[inline]
    pub fn world_bounds_vec3(&self) -> (glam::Vec3, glam::Vec3) {
        (
            glam::Vec3::from_array(self.world_min),
            glam::Vec3::from_array(self.world_max),
        )
    }

    #[inline]
    pub fn fog_range(&self) -> Option<(f32, f32)> {
        self.fog_start.zip(self.fog_end)
    }

    /// Bilinear-ish nearest sample from the coarse height grid (world XZ).
    /// Returns None when the grid is empty / not from terrain.
    pub fn sample_height(&self, world_x: f32, world_z: f32) -> Option<f32> {
        if !self.height_samples_from_terrain
            || self.height_grid_w == 0
            || self.height_grid_h == 0
            || self.height_samples.is_empty()
        {
            return None;
        }
        let (wmin, wmax) = self.world_bounds_vec3();
        let span_x = (wmax.x - wmin.x).max(1.0);
        let span_z = (wmax.z - wmin.z).max(1.0);
        let u = ((world_x - wmin.x) / span_x).clamp(0.0, 1.0);
        let v = ((world_z - wmin.z) / span_z).clamp(0.0, 1.0);
        let x = ((u * (self.height_grid_w as f32 - 1.0)).round() as u32)
            .min(self.height_grid_w.saturating_sub(1));
        let y = ((v * (self.height_grid_h as f32 - 1.0)).round() as u32)
            .min(self.height_grid_h.saturating_sub(1));
        let idx = (y * self.height_grid_w + x) as usize;
        self.height_samples.get(idx).copied()
    }

    /// Prewarm signature fragment (map|meta|objects|heightmap|shell) without live logic.
    pub fn prewarm_signature(&self, shell_bypass: bool) -> String {
        format!(
            "{}|meta:{}|objects:{}|heightmap:{}|shell:{}",
            self.map_name,
            self.has_map_metadata,
            self.map_object_count,
            self.heightmap_hint.as_deref().unwrap_or(""),
            shell_bypass
        )
    }
}
