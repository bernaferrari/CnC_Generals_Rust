//! Immutable presentation snapshot built from the authoritative Main GameLogic.
//!
//! Policy: GameClient / renderer / HUD should consume `PresentationFrame` only.
//! They must not lock or mutate the sim while a WGPU pass is active.
//!
//! Ownership: borrow-first on the authority during `build_*`; then the snapshot
//! is owned values with no live borrows into the world.

use crate::game_logic::{GameLogic, KindOf, ObjectId, Team};
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Logic-frame index (30 Hz authority).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LogicFrame(pub u32);

/// One renderable object as seen after a completed logic step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenderableObject {
    pub id: ObjectId,
    pub template_name: String,
    pub team: Team,
    pub position: Vec3,
    pub orientation: f32,
    pub health_current: f32,
    pub health_max: f32,
    pub selected: bool,
    pub destroyed: bool,
    pub under_construction: bool,
    pub is_structure: bool,
    pub is_unit: bool,
    pub model_key: Option<String>,
}

/// Ordered gameplay event for audio/FX/UI (presentation side only).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PresentationEvent {
    ObjectDestroyed { id: ObjectId, team: Team },
    ConstructionComplete { id: ObjectId, template: String },
    Victory { winner_player: Option<u32> },
    RadarMessage { team: Team, text: String },
}

/// Immutable feed for GameClient / renderer after each authoritative logic step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresentationFrame {
    pub frame: LogicFrame,
    pub objects: Vec<RenderableObject>,
    pub local_player_id: u32,
    pub local_supplies: u32,
    pub local_power: i32,
    pub local_color_rgb: (u8, u8, u8),
    pub selected: Vec<ObjectId>,
    pub events: Vec<PresentationEvent>,
    pub match_over: bool,
    pub victory_label: Option<String>,
}

impl PresentationFrame {
    /// Build a snapshot by borrowing the authoritative world for this call only.
    pub fn build_from_logic(logic: &GameLogic, local_player_id: u32) -> Self {
        let mut objects = Vec::with_capacity(logic.get_objects().len());
        for obj in logic.get_objects().values() {
            let is_structure = obj.is_kind_of(KindOf::Structure);
            let is_unit = obj.is_kind_of(KindOf::Infantry)
                || obj.is_kind_of(KindOf::Vehicle)
                || obj.is_kind_of(KindOf::Aircraft);
            objects.push(RenderableObject {
                id: obj.id,
                template_name: obj.template_name.clone(),
                team: obj.team,
                // Use accessors so presentation matches authoritative transform state.
                position: obj.get_position(),
                orientation: obj.get_orientation(),
                health_current: obj.health.current,
                health_max: obj.health.maximum,
                selected: obj.selected || obj.status.selected,
                destroyed: obj.status.destroyed || !obj.is_alive(),
                under_construction: obj.status.under_construction,
                is_structure,
                is_unit,
                model_key: Some(obj.template_name.clone()),
            });
        }
        // Stable presentation order for determinism (by ObjectId).
        objects.sort_by_key(|o| o.id.0);

        let local = logic.get_player(local_player_id);
        let local_supplies = local.map(|p| p.resources.supplies).unwrap_or(0);
        let local_power = local.map(|p| p.power_available).unwrap_or(0);
        let local_color_rgb = local.map(|p| p.color_rgb).unwrap_or((200, 200, 200));
        let selected = local
            .map(|p| p.selected_objects.clone())
            .unwrap_or_default();

        Self {
            frame: LogicFrame(logic.get_frame()),
            objects,
            local_player_id,
            local_supplies,
            local_power,
            local_color_rgb,
            selected,
            events: Vec::new(),
            match_over: false,
            victory_label: None,
        }
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
        }
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
        }
        self.local_supplies.hash(&mut h);
        self.match_over.hash(&mut h);
        h.finish()
    }

    pub fn alive_object_count(&self) -> usize {
        self.objects.iter().filter(|o| !o.destroyed).count()
    }

    /// Stable object-id list for the production render collect path.
    /// Presentation is the frame identity; GameLogic is only used to resolve
    /// mesh/transform for those IDs when still present.
    pub fn renderable_object_ids(&self) -> Vec<ObjectId> {
        self.objects
            .iter()
            .filter(|o| !o.destroyed)
            .map(|o| o.id)
            .collect()
    }

    /// Apply presentation identity fields onto a HUD/UI state (production consumer path).
    /// Does not re-borrow GameLogic — uses only owned snapshot data.
    ///
    /// Overwrites **selection IDs, selected unit health/name, and minimap unit dots**
    /// so a prior live `update_ui_state` walk cannot leave stale identity when a frame
    /// is available.
    pub fn apply_to_ui_state(&self, ui: &mut crate::ui::GameUIState) {
        use crate::ui::{color_for_player, MinimapDot, UnitDisplayInfo};

        ui.credits = self.local_supplies as i32;
        ui.power_generated = self.local_power.max(0);
        ui.power_used = 0;
        ui.max_power = self.local_power.max(0).max(1);
        ui.player_id = self.local_player_id;
        ui.selected_units = self.selected.clone();
        ui.match_over = self.match_over;

        // Selected unit identity (health/name/type) from snapshot only.
        let by_id: std::collections::HashMap<ObjectId, &RenderableObject> =
            self.objects.iter().map(|o| (o.id, o)).collect();
        let mut selected_infos = Vec::with_capacity(self.selected.len());
        for id in &self.selected {
            if let Some(ro) = by_id.get(id) {
                if ro.destroyed {
                    continue;
                }
                selected_infos.push(UnitDisplayInfo {
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
                    current_order: "Idle".into(),
                });
            }
        }
        // Also include objects marked selected on the snapshot when player list is empty.
        if selected_infos.is_empty() {
            for ro in self.objects.iter().filter(|o| o.selected && !o.destroyed) {
                selected_infos.push(UnitDisplayInfo {
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
                    current_order: "Idle".into(),
                });
            }
        }
        ui.selected_unit_infos = selected_infos;

        // Minimap dots from snapshot positions/teams (normalized into frame bounds).
        let alive: Vec<&RenderableObject> = self.objects.iter().filter(|o| !o.destroyed).collect();
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
    pub fn hud_resource_triple(&self) -> (i32, i32, i32) {
        let credits = self.local_supplies as i32;
        let power = self.local_power.max(0);
        (credits, power, power.max(1))
    }

    /// Units list for GameHud minimap: (id, x, z, team_color_index).
    pub fn hud_minimap_units(&self) -> Vec<(ObjectId, f32, f32, u8)> {
        self.objects
            .iter()
            .filter(|o| !o.destroyed)
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

    /// Apply presentation resources + minimap units to the production GameHUD.
    pub fn apply_to_game_hud(&self, hud: &mut crate::ui::GameHUD) {
        let (credits, power, max_power) = self.hud_resource_triple();
        hud.update_resources(credits, power, max_power);
        let units = self.hud_minimap_units();
        hud.update_minimap(&units);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{GameMode, KindOf, Player, ThingTemplate};
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};

    #[test]
    fn presentation_frame_is_built_from_authority_without_arc() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("PresMap");
        apply_skirmish_config(&mut logic, &cfg).expect("config");
        let mut t = ThingTemplate::new("PresUnit");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("PresUnit".into(), t);
        let id = logic
            .create_object("PresUnit", Team::USA, glam::Vec3::new(1.0, 0.0, 2.0))
            .expect("unit");

        let snap = PresentationFrame::build_from_logic(&logic, 0);
        assert_eq!(snap.frame.0, logic.get_frame());
        assert!(snap.objects.iter().any(|o| o.id == id));
        assert_eq!(snap.local_supplies, 10_000);
        // Snapshot is owned — mutating world after build must not require re-borrow of snap.
        logic.update();
        assert_eq!(snap.objects.len(), 1);
        let h1 = snap.presentation_hash();
        let snap2 = PresentationFrame::build_from_logic(&logic, 0);
        // Frame advanced; hash may change.
        assert!(snap2.frame.0 >= snap.frame.0);
        let _ = h1;
    }

    #[test]
    fn dual_presentation_hashes_match_for_identical_worlds() {
        let mk = || {
            let mut logic = GameLogic::new();
            logic.start_new_game(GameMode::Skirmish);
            logic.clear_all_players();
            logic.add_player(Player::new(0, Team::USA, "P", true));
            let mut t = ThingTemplate::new("HashUnit");
            t.set_health(50.0);
            t.add_kind_of(KindOf::Vehicle);
            logic.templates.insert("HashUnit".into(), t);
            let _ = logic.create_object("HashUnit", Team::USA, glam::Vec3::ZERO);
            PresentationFrame::build_from_logic(&logic, 0).presentation_hash()
        };
        assert_eq!(mk(), mk());
    }

    #[test]
    fn client_reads_snapshot_not_live_world() {
        // Simulate: authority builds snapshot, then world mutates; client still holds old frame.
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("ClientSnap");
        apply_skirmish_config(&mut logic, &cfg).expect("config");
        let mut t = ThingTemplate::new("SnapUnit");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("SnapUnit".into(), t);
        let id = logic
            .create_object("SnapUnit", Team::USA, glam::Vec3::ZERO)
            .expect("unit");
        let client_view = PresentationFrame::build_from_logic(&logic, 0);
        assert_eq!(client_view.alive_object_count(), 1);
        // Authority continues without client re-borrowing world during "render".
        if let Some(o) = logic.get_object_mut(id) {
            o.status.destroyed = true;
            o.health.current = 0.0;
        }
        // Stale presentation still has the pre-destroy object; proves client feed is owned data.
        assert_eq!(client_view.objects.len(), 1);
        assert!(!client_view.objects[0].destroyed);
        // Fresh presentation reflects authority.
        let next = PresentationFrame::build_from_logic(&logic, 0);
        assert!(
            next.objects.iter().all(|o| o.destroyed || o.id != id)
                || next.alive_object_count() == 0
                || next.objects.iter().any(|o| o.id == id && o.destroyed)
        );
    }

    #[test]
    fn shipped_hud_consumer_uses_snapshot_owned_fields() {
        // Criterion: after logic update, HUD/minimap consumers use snapshot-owned
        // id/transform/health/team/selection/model — not a live re-borrow.
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("HudFields");
        apply_skirmish_config(&mut logic, &cfg).expect("config");
        let mut t = ThingTemplate::new("HudUnit");
        t.set_health(75.0);
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("HudUnit".into(), t);
        let id = logic
            .create_object("HudUnit", Team::USA, glam::Vec3::new(9.0, 0.0, -4.0))
            .expect("unit");
        if let Some(o) = logic.get_object_mut(id) {
            o.selected = true;
            o.status.selected = true;
        }
        if let Some(p) = logic.get_player_mut(0) {
            p.selected_objects = vec![id];
        }
        logic.update();
        let snap = PresentationFrame::build_from_logic(&logic, 0);
        let obj = snap
            .objects
            .iter()
            .find(|o| o.id == id)
            .expect("object in snapshot");
        assert!((obj.position.x - 9.0).abs() < 0.01);
        assert!((obj.position.z + 4.0).abs() < 0.01);
        assert_eq!(obj.health_current, 75.0);
        assert_eq!(obj.health_max, 75.0);
        assert_eq!(obj.team, Team::USA);
        assert!(obj.selected);
        assert_eq!(obj.model_key.as_deref(), Some("HudUnit"));

        let mut ui = crate::ui::GameUIState::default();
        snap.apply_to_ui_state(&mut ui);
        assert_eq!(ui.credits, snap.local_supplies as i32);
        assert!(ui.selected_units.contains(&id));

        let mut hud = crate::ui::GameHUD::new();
        snap.apply_to_game_hud(&mut hud);
        let mini = snap.hud_minimap_units();
        assert!(
            mini.iter().any(|(oid, x, z, _)| {
                *oid == id && (*x - 9.0).abs() < 0.01 && (*z + 4.0).abs() < 0.01
            }),
            "minimap units must come from snapshot positions"
        );
    }

    #[test]
    fn apply_to_ui_state_overwrites_live_identity_after_mutation() {
        // Production path: live update_ui_state may run first; apply_to_ui_state must
        // replace selection health + minimap dots with snapshot-owned values.
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("HudIdentity");
        apply_skirmish_config(&mut logic, &cfg).expect("config");
        let mut t = ThingTemplate::new("HudIdUnit");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("HudIdUnit".into(), t);
        let id = logic
            .create_object("HudIdUnit", Team::USA, glam::Vec3::new(10.0, 0.0, 20.0))
            .expect("unit");
        if let Some(p) = logic.get_player_mut(0) {
            p.selected_objects = vec![id];
        }
        if let Some(o) = logic.get_object_mut(id) {
            o.selected = true;
            o.status.selected = true;
        }

        let snap = PresentationFrame::build_from_logic(&logic, 0);
        // Live world mutates after snapshot (would poison a re-read).
        if let Some(o) = logic.get_object_mut(id) {
            o.set_position(glam::Vec3::new(999.0, 0.0, 999.0));
            o.health.current = 3.0;
        }

        // Simulate production: live walk first, then presentation overlay.
        let mut ui = logic.update_ui_state(0);
        snap.apply_to_ui_state(&mut ui);

        assert!(
            ui.selected_units.contains(&id),
            "selection ids from snapshot"
        );
        let info = ui
            .selected_unit_infos
            .iter()
            .find(|u| u.object_id == id)
            .expect("selected_unit_infos from snapshot");
        assert!(
            (info.health_current - 100.0).abs() < 0.01,
            "health must be snapshot 100, not live 3: {}",
            info.health_current
        );
        assert!(
            !ui.minimap_unit_dots.is_empty(),
            "minimap dots filled from presentation objects"
        );
        assert_eq!(
            ui.minimap_unit_dots.len(),
            snap.objects.iter().filter(|o| !o.destroyed).count()
        );
    }
}
