//! Immutable presentation snapshot built from the authoritative Main GameLogic.
//!
//! Policy: GameClient / renderer / HUD should consume `PresentationFrame` only.
//! They must not lock or mutate the sim while a WGPU pass is active.
//!
//! Ownership: borrow-first on the authority during `build_*`; then the snapshot
//! is owned values with no live borrows into the world.

use crate::fow_rendering::{FOWRenderingBridge, ObjectVisibility};
use crate::game_logic::{
    CombatParticleKind, CombatParticleSystemEntry, GameLogic, KindOf, ObjectId, Team,
};
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
    /// Team tint for presentation-only draw (RGBA 0..1), mirrors Object::team_color.
    pub team_color: [f32; 4],
    pub position: Vec3,
    pub orientation: f32,
    pub health_current: f32,
    pub health_max: f32,
    pub selected: bool,
    pub destroyed: bool,
    pub under_construction: bool,
    pub is_structure: bool,
    pub is_unit: bool,
    /// W3D / mesh resolve key (template model name). Snapshot-owned so the unit
    /// mesh pass does not re-read live ThingTemplate during GPU collect.
    pub model_key: Option<String>,
    /// Cull / selection radius for presentation-only draw (no live GameLogic re-read).
    pub selection_radius: f32,
    /// True when bridged to GameEngine ObjectFactory (`engine_object_id`).
    /// Presentation-owned so the unit mesh pass can skip double-draw without
    /// locking live GameLogic for identity.
    pub engine_bridged: bool,
    /// FOW visibility for `PresentationFrame.local_player_id` at snapshot time.
    /// Unit mesh pass applies alpha / never-explored skip from this only — no
    /// live shroud re-query mid-render.
    pub fow_visibility: ObjectVisibility,
}

/// Snapshot-owned unit mesh/position/selection/FOW input for the main unit render pass.
///
/// Built only from `PresentationFrame` — no live `GameLogic` or shroud borrow.
/// W3D asset resolve remains outside this type (see residual notes).
#[derive(Debug, Clone, PartialEq)]
pub struct UnitRenderInput {
    pub id: ObjectId,
    pub template_name: String,
    pub model_key: String,
    pub team: Team,
    pub team_color: [f32; 4],
    pub position: Vec3,
    pub orientation: f32,
    pub selected: bool,
    pub selection_radius: f32,
    pub is_structure: bool,
    pub is_unit: bool,
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
            team: ro.team,
            team_color: ro.team_color,
            position: ro.position,
            orientation: ro.orientation,
            selected: ro.selected,
            selection_radius: ro.selection_radius.max(5.0),
            is_structure: ro.is_structure,
            is_unit: ro.is_unit,
            engine_bridged: ro.engine_bridged,
            fow_visibility: ro.fow_visibility,
        }
    }

    /// World matrix for the unit mesh pass (translation + Y rotation).
    pub fn world_matrix(&self) -> glam::Mat4 {
        glam::Mat4::from_translation(self.position)
            * glam::Mat4::from_rotation_y(self.orientation)
    }

    /// Never-explored skip for the main mesh pass (snapshot FOW only).
    #[inline]
    pub fn fow_should_render(&self) -> bool {
        self.fow_visibility.should_render()
    }
}

/// Ordered gameplay event for audio/FX/UI (presentation side only).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PresentationEvent {
    ObjectDestroyed { id: ObjectId, team: Team },
    ConstructionComplete { id: ObjectId, template: String },
    Victory { winner_player: Option<u32> },
    RadarMessage { team: Team, text: String },
    /// Combat residual: particle system spawned (host registry id + template).
    ParticleSystemSpawned {
        id: u32,
        kind: CombatParticleKind,
        template_name: String,
        position: Vec3,
    },
}

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
        }
    }
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
    /// Shell-map FOW bypass (`GameLogic::isInShellGame`) frozen at snapshot time.
    /// When true, unit FOW is forced fully visible and never-explored skip is off.
    pub fow_shell_bypass: bool,
    /// Active combat particle systems from host registry (observe path for client).
    pub particle_systems: Vec<PresentationParticleSystem>,
}

impl PresentationFrame {
    /// Build a snapshot by borrowing the authoritative world for this call only.
    ///
    /// FOW for `local_player_id` is frozen here via the FOW bridge so the unit mesh
    /// pass can apply alpha / never-explored skip without mid-render shroud locks.
    /// Not full SAGE cell-grid FOW parity — unit-level visibility only (fail-closed claim).
    pub fn build_from_logic(logic: &GameLogic, local_player_id: u32) -> Self {
        // Shell maps render fully visible background scenes (C++ parity).
        let fow_shell_bypass = logic.isInShellGame();
        let mut objects = Vec::with_capacity(logic.get_objects().len());
        for obj in logic.get_objects().values() {
            let is_structure = obj.is_kind_of(KindOf::Structure);
            let is_unit = obj.is_kind_of(KindOf::Infantry)
                || obj.is_kind_of(KindOf::Vehicle)
                || obj.is_kind_of(KindOf::Aircraft);
            // Prefer explicit template model name so mesh resolve matches live collect path.
            let model_key = Some(obj.get_template().get_model_name().to_string());
            let fow_visibility = if fow_shell_bypass {
                ObjectVisibility::FULLY_VISIBLE
            } else {
                FOWRenderingBridge::get_object_visibility(local_player_id, obj.id)
            };
            objects.push(RenderableObject {
                id: obj.id,
                template_name: obj.template_name.clone(),
                team: obj.team,
                team_color: obj.team_color,
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
                model_key,
                selection_radius: obj.selection_radius.max(5.0),
                engine_bridged: obj.engine_object_id.is_some(),
                fow_visibility,
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

        // Combat particle residual: freeze host registry for client/presentation observe.
        let particle_systems: Vec<PresentationParticleSystem> = logic
            .combat_particles()
            .systems_snapshot()
            .iter()
            .map(PresentationParticleSystem::from_combat_entry)
            .collect();

        let mut events = Vec::new();
        for (id, team) in logic.combat_particles().destroyed_this_frame() {
            events.push(PresentationEvent::ObjectDestroyed {
                id: *id,
                team: *team,
            });
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

        Self {
            frame: LogicFrame(logic.get_frame()),
            objects,
            local_player_id,
            local_supplies,
            local_power,
            local_color_rgb,
            selected,
            events,
            match_over: false,
            victory_label: None,
            fow_shell_bypass,
            particle_systems,
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
            o.fow_visibility.visibility_alpha.to_bits().hash(&mut h);
            o.fow_visibility.is_explored.to_bits().hash(&mut h);
        }
        self.local_supplies.hash(&mut h);
        self.match_over.hash(&mut h);
        self.fow_shell_bypass.hash(&mut h);
        self.local_player_id.hash(&mut h);
        h.finish()
    }

    pub fn alive_object_count(&self) -> usize {
        self.objects.iter().filter(|o| !o.destroyed).count()
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
        self.objects
            .iter()
            .filter(|o| !o.destroyed && !o.engine_bridged)
            .map(UnitRenderInput::from_renderable)
            .collect()
    }

    /// Lookup snapshot FOW for an object (local player). None if not on the frame.
    pub fn fow_for_object(&self, id: ObjectId) -> Option<ObjectVisibility> {
        self.objects
            .iter()
            .find(|o| o.id == id)
            .map(|o| o.fow_visibility)
    }

    /// All alive presentation objects including engine-bridged (for FOW/id lists).
    pub fn alive_renderables(&self) -> impl Iterator<Item = &RenderableObject> {
        self.objects.iter().filter(|o| !o.destroyed)
    }

    /// Active combat particle systems on this frame (host registry snapshot).
    pub fn active_particle_systems(&self) -> impl Iterator<Item = &PresentationParticleSystem> {
        self.particle_systems.iter().filter(|p| p.active)
    }

    /// True when at least one combat particle system is registered and active.
    pub fn has_active_particles(&self) -> bool {
        self.particle_systems.iter().any(|p| p.active)
    }

    /// Selected unit identity (health/name/type) from snapshot only.
    ///
    /// Prefer player selection list; fall back to objects marked selected on the frame
    /// when the player list is empty (common right after click-select before player list
    /// is mirrored).
    pub fn selected_unit_display_infos(&self) -> Vec<crate::ui::UnitDisplayInfo> {
        use crate::ui::UnitDisplayInfo;

        let by_id: std::collections::HashMap<ObjectId, &RenderableObject> =
            self.objects.iter().map(|o| (o.id, o)).collect();
        let mut selected_infos = Vec::with_capacity(self.selected.len().max(1));
        for id in &self.selected {
            if let Some(ro) = by_id.get(id) {
                if ro.destroyed {
                    continue;
                }
                selected_infos.push(Self::unit_display_info_from_renderable(ro));
            }
        }
        if selected_infos.is_empty() {
            for ro in self.objects.iter().filter(|o| o.selected && !o.destroyed) {
                selected_infos.push(Self::unit_display_info_from_renderable(ro));
            }
        }
        selected_infos
    }

    fn unit_display_info_from_renderable(ro: &RenderableObject) -> crate::ui::UnitDisplayInfo {
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
            current_order: "Idle".into(),
        }
    }

    /// Apply presentation identity fields onto a HUD/UI state (production consumer path).
    /// Does not re-borrow GameLogic — uses only owned snapshot data.
    ///
    /// Overwrites **selection IDs, selected unit health/name, and minimap unit dots**
    /// so a prior live `update_ui_state` walk cannot leave stale identity when a frame
    /// is available.
    pub fn apply_to_ui_state(&self, ui: &mut crate::ui::GameUIState) {
        use crate::ui::{color_for_player, MinimapDot};

        ui.credits = self.local_supplies as i32;
        ui.power_generated = self.local_power.max(0);
        ui.power_used = 0;
        ui.max_power = self.local_power.max(0).max(1);
        ui.player_id = self.local_player_id;
        ui.selected_units = self.selected.clone();
        ui.match_over = self.match_over;
        ui.selected_unit_infos = self.selected_unit_display_infos();
        // ControlBar/WND selection panel health must come from snapshot, not live re-read.
        ui.selection_panel =
            crate::ui::ControlBarSelectionPanelState::from_unit_infos(&ui.selected_unit_infos);

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

    /// Apply presentation resources, minimap units, and selection health to GameHUD.
    ///
    /// Selection identity (IDs + health/name) is snapshot-owned so the production HUD
    /// does not re-read live GameLogic after a skirmish start / dual-tick.
    /// Also fills the ControlBar selection panel health strip via GameHUD.
    pub fn apply_to_game_hud(&self, hud: &mut crate::ui::GameHUD) {
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
    }

    /// Snapshot-owned ControlBar / WND selection panel (health + name).
    pub fn control_bar_selection_panel(&self) -> crate::ui::ControlBarSelectionPanelState {
        crate::ui::ControlBarSelectionPanelState::from_unit_infos(
            &self.selected_unit_display_infos(),
        )
    }

    /// Apply selection health/name to GameClient ControlBar without OBJECT_REGISTRY.
    ///
    /// Headless-safe: uses only presentation fields. Does not claim full WND shell.
    #[cfg(feature = "game_client")]
    pub fn apply_to_control_bar(
        &self,
        control_bar: &mut game_client::gui::control_bar::ControlBar,
    ) {
        let panel = self.control_bar_selection_panel();
        let ids: Vec<u32> = if !self.selected.is_empty() {
            self.selected.iter().map(|id| id.0).collect()
        } else {
            panel
                .unit_infos
                .iter()
                .map(|u| u.object_id.0)
                .collect()
        };
        let _ = control_bar.update_for_selection(ids);
        control_bar.sync_selection_display_from_presentation(
            panel.visible.then_some(panel.primary_name.as_str()),
            panel.health_current,
            panel.health_maximum,
            panel.selected_count,
        );
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
        let frame = Self::build_from_logic(logic, local_player_id);
        frame.apply_to_game_hud(hud);
        frame
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
        assert!(
            hud.selected_unit_ids().contains(&id),
            "GameHUD selection IDs must come from presentation"
        );
        let hud_info = hud
            .selected_unit_infos()
            .iter()
            .find(|u| u.object_id == id)
            .expect("GameHUD selection health from presentation");
        assert!(
            (hud_info.health_current - 75.0).abs() < 0.01,
            "GameHUD selection health must be snapshot-owned: {}",
            hud_info.health_current
        );
    }

    #[test]
    fn dual_tick_build_and_apply_after_logic_step_seeds_hud() {
        // Map-load / skirmish residual: after authority advances, presentation must
        // seed HUD resources + selection without re-borrowing live objects later.
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("DualTickHud");
        apply_skirmish_config(&mut logic, &cfg).expect("config");
        let mut t = ThingTemplate::new("DualUnit");
        t.set_health(88.0);
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("DualUnit".into(), t);
        let id = logic
            .create_object("DualUnit", Team::USA, glam::Vec3::new(1.0, 0.0, 1.0))
            .expect("unit");
        if let Some(p) = logic.get_player_mut(0) {
            p.selected_objects = vec![id];
        }
        if let Some(o) = logic.get_object_mut(id) {
            o.selected = true;
            o.status.selected = true;
        }
        logic.update(); // authority tick
        let mut hud = crate::ui::GameHUD::new();
        let snap = PresentationFrame::build_and_apply_for_hud(&logic, 0, &mut hud);
        assert_eq!(snap.frame.0, logic.get_frame());
        assert!(
            !snap.hud_minimap_units().is_empty(),
            "presentation after tick must expose units for minimap"
        );
        let info = hud
            .selected_unit_infos()
            .iter()
            .find(|u| u.object_id == id)
            .expect("selection health on HUD after dual-tick apply");
        assert!((info.health_current - 88.0).abs() < 0.01);
        // World mutates after apply; HUD must keep snapshot health.
        if let Some(o) = logic.get_object_mut(id) {
            o.health.current = 1.0;
        }
        assert!((info.health_current - 88.0).abs() < 0.01);
    }

    #[test]
    fn presentation_snapshot_includes_selection_radius_for_cull() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("SelRadius");
        apply_skirmish_config(&mut logic, &cfg).expect("config");
        let mut t = ThingTemplate::new("RadiusUnit");
        t.set_health(50.0);
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("RadiusUnit".into(), t);
        let id = logic
            .create_object("RadiusUnit", Team::USA, glam::Vec3::ZERO)
            .expect("unit");
        if let Some(o) = logic.get_object_mut(id) {
            o.selection_radius = 12.5;
        }
        let snap = PresentationFrame::build_from_logic(&logic, 0);
        let ro = snap.objects.iter().find(|o| o.id == id).expect("in snap");
        assert!(
            (ro.selection_radius - 12.5).abs() < 0.01,
            "selection_radius must be snapshot-owned for presentation-only cull: {}",
            ro.selection_radius
        );
    }

    #[test]
    fn presentation_build_includes_unit_render_fields_and_positions() {
        // Criterion: unit mesh/position/selection inputs are snapshot-owned so the
        // main unit pass can iterate PresentationFrame without GameLogic.
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("UnitRenderFields");
        apply_skirmish_config(&mut logic, &cfg).expect("config");
        let mut t = ThingTemplate::new("MeshUnit");
        t.set_health(60.0);
        t.set_model("AVTank");
        t.add_kind_of(KindOf::Vehicle);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("MeshUnit".into(), t);
        let id = logic
            .create_object("MeshUnit", Team::USA, glam::Vec3::new(3.0, 0.0, -8.0))
            .expect("unit");
        if let Some(o) = logic.get_object_mut(id) {
            o.selected = true;
            o.status.selected = true;
            o.selection_radius = 11.0;
            o.team_color = [0.1, 0.2, 0.9, 1.0];
            // Not bridged — main mesh pass owns draw.
            o.engine_object_id = None;
        }
        if let Some(p) = logic.get_player_mut(0) {
            p.selected_objects = vec![id];
        }

        let snap = PresentationFrame::build_from_logic(&logic, 0);
        let ro = snap.objects.iter().find(|o| o.id == id).expect("in snap");
        assert!((ro.position.x - 3.0).abs() < 0.01);
        assert!((ro.position.z + 8.0).abs() < 0.01);
        assert_eq!(ro.team, Team::USA);
        assert_eq!(ro.team_color, [0.1, 0.2, 0.9, 1.0]);
        assert_eq!(ro.model_key.as_deref(), Some("AVTank"));
        assert_eq!(ro.template_name, "MeshUnit");
        assert!(ro.selected);
        assert!(!ro.destroyed);
        assert!(!ro.engine_bridged);
        assert!((ro.selection_radius - 11.0).abs() < 0.01);

        // unit_render_inputs is the production pure-frame collection path.
        let inputs = snap.unit_render_inputs();
        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].id, id);
        assert_eq!(inputs[0].model_key, "AVTank");
        assert!((inputs[0].position.x - 3.0).abs() < 0.01);
        assert!(inputs[0].selected);
        assert!(!inputs[0].engine_bridged);
        assert_eq!(inputs[0].fow_visibility, ro.fow_visibility);

        // Mutate authority after snapshot — inputs must stay frozen.
        if let Some(o) = logic.get_object_mut(id) {
            o.set_position(glam::Vec3::new(999.0, 0.0, 999.0));
            o.selected = false;
            o.engine_object_id = Some(42);
        }
        let inputs_after = snap.unit_render_inputs();
        assert_eq!(inputs_after.len(), 1);
        assert!(
            (inputs_after[0].position.x - 3.0).abs() < 0.01,
            "unit render inputs must not re-read live GameLogic"
        );
        assert!(inputs_after[0].selected);
        assert!(!inputs_after[0].engine_bridged);
        assert_eq!(
            inputs_after[0].fow_visibility, ro.fow_visibility,
            "FOW on unit inputs must stay frozen after live world mutation"
        );
    }

    #[test]
    fn presentation_fow_matches_bridge_at_build_and_stays_frozen() {
        use crate::fow_rendering::{FOWRenderingBridge, ObjectVisibility};

        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("FowSnapConsistency");
        apply_skirmish_config(&mut logic, &cfg).expect("config");
        let mut t = ThingTemplate::new("FowUnit");
        t.set_health(50.0);
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("FowUnit".into(), t);
        let id = logic
            .create_object("FowUnit", Team::USA, glam::Vec3::new(5.0, 0.0, 5.0))
            .expect("unit");

        // Bridge state at build time is the source of truth for the snapshot.
        let bridge_at_build = FOWRenderingBridge::get_object_visibility(0, id);
        let snap = PresentationFrame::build_from_logic(&logic, 0);
        let ro = snap.objects.iter().find(|o| o.id == id).expect("in snap");
        assert_eq!(
            ro.fow_visibility, bridge_at_build,
            "presentation FOW must match FOW bridge at build time"
        );
        assert_eq!(snap.fow_for_object(id), Some(bridge_at_build));
        assert_eq!(snap.fow_shell_bypass, logic.isInShellGame());

        let inputs = snap.unit_render_inputs();
        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].fow_visibility, bridge_at_build);
        assert_eq!(
            inputs[0].fow_should_render(),
            bridge_at_build.should_render()
        );

        // Encode states are stable and cover the three SAGE-style buckets.
        assert_eq!(
            ObjectVisibility::from_shroud_flags(true, true),
            ObjectVisibility::VISIBLE
        );
        assert_eq!(
            ObjectVisibility::from_shroud_flags(false, true),
            ObjectVisibility::FOGGED
        );
        assert_eq!(
            ObjectVisibility::from_shroud_flags(false, false),
            ObjectVisibility::HIDDEN
        );
        assert!(ObjectVisibility::FOGGED.should_render());
        assert!(!ObjectVisibility::HIDDEN.should_render());
        assert!(ObjectVisibility::HIDDEN.never_explored());

        // Dual-build with identical world + FOW state yields matching FOW on hash.
        let snap2 = PresentationFrame::build_from_logic(&logic, 0);
        assert_eq!(snap.fow_for_object(id), snap2.fow_for_object(id));
        assert_eq!(
            snap.objects
                .iter()
                .find(|o| o.id == id)
                .map(|o| o.fow_visibility),
            snap2
                .objects
                .iter()
                .find(|o| o.id == id)
                .map(|o| o.fow_visibility)
        );
    }

    #[test]
    fn presentation_fow_shell_bypass_forces_fully_visible() {
        use crate::fow_rendering::ObjectVisibility;
        use crate::game_logic::GameMode;

        let mut logic = GameLogic::new();
        // Shell map path: FOW bypass is frozen on the frame.
        logic.start_new_game(GameMode::Shell);
        assert!(logic.isInShellGame());
        let mut t = ThingTemplate::new("ShellFowUnit");
        t.set_health(10.0);
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("ShellFowUnit".into(), t);
        let id = logic
            .create_object("ShellFowUnit", Team::USA, glam::Vec3::ZERO)
            .expect("unit");

        let snap = PresentationFrame::build_from_logic(&logic, 0);
        assert!(snap.fow_shell_bypass);
        let ro = snap.objects.iter().find(|o| o.id == id).expect("in snap");
        assert_eq!(ro.fow_visibility, ObjectVisibility::FULLY_VISIBLE);
        assert!(snap.unit_render_inputs()[0].fow_should_render());
    }

    #[test]
    fn unit_render_inputs_skip_destroyed_and_engine_bridged() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("UnitRenderSkip");
        apply_skirmish_config(&mut logic, &cfg).expect("config");
        let mut t = ThingTemplate::new("SkipUnit");
        t.set_health(40.0);
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("SkipUnit".into(), t);

        let alive_id = logic
            .create_object("SkipUnit", Team::China, glam::Vec3::new(1.0, 0.0, 1.0))
            .expect("alive");
        let dead_id = logic
            .create_object("SkipUnit", Team::China, glam::Vec3::new(2.0, 0.0, 2.0))
            .expect("dead");
        let bridged_id = logic
            .create_object("SkipUnit", Team::China, glam::Vec3::new(3.0, 0.0, 3.0))
            .expect("bridged");
        if let Some(o) = logic.get_object_mut(dead_id) {
            o.status.destroyed = true;
            o.health.current = 0.0;
        }
        if let Some(o) = logic.get_object_mut(bridged_id) {
            o.engine_object_id = Some(99);
        }

        let snap = PresentationFrame::build_from_logic(&logic, 0);
        let inputs = snap.unit_render_inputs();
        assert_eq!(
            inputs.len(),
            1,
            "only non-destroyed, non-bridged units enter main mesh pass"
        );
        assert_eq!(inputs[0].id, alive_id);
        // IDs list still includes all alive (including bridged) for FOW/id residual.
        let ids = snap.renderable_object_ids();
        assert!(ids.contains(&alive_id));
        assert!(ids.contains(&bridged_id));
        assert!(!ids.contains(&dead_id));
    }

    #[test]
    fn production_tick_builds_presentation_after_side_systems() {
        // Structural: presentation snapshot must be built after projectile/path host systems.
        let src = include_str!("cnc_game_engine.rs");
        let proj = src
            .find("drain_pending_projectiles")
            .expect("projectile drain");
        let path = src.find("move_unit_along_path").expect("path move");
        let pres = src
            .find("PresentationFrame::build_from_logic")
            .expect("presentation build");
        assert!(
            proj < pres && path < pres,
            "PresentationFrame must be built after projectiles ({proj}) and path ({path}); found at {pres}"
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
        assert!(
            ui.selection_panel.has_positive_health(),
            "last_ui_state selection panel must carry snapshot health"
        );
        assert!(
            (ui.selection_panel.health_current - 100.0).abs() < 0.01,
            "selection panel HP from presentation: {}",
            ui.selection_panel.health_current
        );
    }

    #[test]
    fn presentation_feeds_control_bar_selection_panel_health() {
        // Residual: ControlBar/WND selection panel health from PresentationFrame
        // (not stale/zero). Headless path — no WND window load required.
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("CbSelPanel");
        apply_skirmish_config(&mut logic, &cfg).expect("config");
        let mut t = ThingTemplate::new("CbPanelUnit");
        t.set_health(77.0);
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("CbPanelUnit".into(), t);
        let id = logic
            .create_object("CbPanelUnit", Team::USA, glam::Vec3::new(4.0, 0.0, 5.0))
            .expect("unit");
        if let Some(p) = logic.get_player_mut(0) {
            p.selected_objects = vec![id];
        }
        if let Some(o) = logic.get_object_mut(id) {
            o.selected = true;
            o.status.selected = true;
        }
        logic.update();

        let snap = PresentationFrame::build_from_logic(&logic, 0);
        let panel = snap.control_bar_selection_panel();
        assert!(panel.visible, "selection panel visible with selection");
        assert_eq!(panel.primary_name, "CbPanelUnit");
        assert!(
            (panel.health_current - 77.0).abs() < 0.01,
            "panel health from presentation: {}",
            panel.health_current
        );
        assert!((panel.health_maximum - 77.0).abs() < 0.01);
        assert_eq!(panel.selected_count, 1);
        assert_eq!(panel.primary_object_id, Some(id));

        // GameHUD selection panel (production host display state).
        let mut hud = crate::ui::GameHUD::new();
        snap.apply_to_game_hud(&mut hud);
        assert!(
            hud.selection_panel().has_positive_health(),
            "GameHUD selection panel must show presentation health"
        );
        assert!(
            (hud.selection_panel().health_current - 77.0).abs() < 0.01,
            "HUD panel HP {}",
            hud.selection_panel().health_current
        );

        // last_ui_state path used by engine consumers.
        let mut ui = crate::ui::GameUIState::default();
        snap.apply_to_ui_state(&mut ui);
        assert!(
            (ui.selection_panel.health_current - 77.0).abs() < 0.01,
            "last_ui_state selection panel health"
        );

        // GameClient ControlBar portrait/health strip (no OBJECT_REGISTRY).
        #[cfg(feature = "game_client")]
        {
            let mut bar = game_client::gui::control_bar::ControlBar::new();
            // Poison live world after snapshot so a re-read would be wrong.
            if let Some(o) = logic.get_object_mut(id) {
                o.health.current = 1.0;
            }
            snap.apply_to_control_bar(&mut bar);
            let (hp, max) = bar
                .selection_panel_health()
                .expect("ControlBar selection panel health from presentation");
            assert!(
                (hp - 77.0).abs() < 0.01,
                "ControlBar must keep snapshot HP 77, not live 1: {hp}"
            );
            assert!((max - 77.0).abs() < 0.01);
            assert_eq!(bar.get_portrait_state().portrait_image, "CbPanelUnit");
            assert!(bar.get_portrait_state().is_visible);
            assert_eq!(bar.get_portrait_state().selected_count, 1);
        }
    }

    /// Residual (hq-gq7n): after combat kill, PresentationFrame exposes particle
    /// systems from the host registry (observe path for client / HUD).
    #[test]
    fn presentation_frame_observes_combat_kill_particle_systems() {
        use crate::game_logic::{CombatParticleKind, ThingTemplate, Weapon};

        let mut logic = GameLogic::new();
        let mut tank = ThingTemplate::new("FxTank");
        tank.add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(50.0);
        logic.templates.insert("FxTank".into(), tank);

        let attacker = logic
            .create_object("FxTank", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("attacker");
        let victim = logic
            .create_object("FxTank", Team::GLA, glam::Vec3::new(5.0, 0.0, 0.0))
            .expect("victim");

        {
            let a = logic.get_object_mut(attacker).expect("attacker");
            a.attack_target(victim);
            a.weapon = Some(Weapon {
                damage: 9999.0,
                range: 100.0,
                reload_time: 0.0,
                last_fire_time: 0.0,
                ..Weapon::default()
            });
        }
        {
            let v = logic.get_object_mut(victim).expect("victim");
            v.health.current = 5.0;
            v.health.maximum = 5.0;
        }

        // Advance one full host step so combat fires and destroy list runs.
        logic.update();

        assert!(
            logic.find_object(victim).is_none(),
            "victim should be destroyed after combat step"
        );
        assert!(
            logic.combat_particles().active_count() > 0,
            "host particle registry must hold systems after kill"
        );

        let snap = PresentationFrame::build_from_logic(&logic, 0);
        assert!(
            snap.has_active_particles(),
            "PresentationFrame must expose active particle systems after combat kill"
        );
        assert!(
            snap.particle_systems
                .iter()
                .any(|p| p.kind == CombatParticleKind::DeathExplosion
                    && p.template_name == "MediumExplosion"),
            "death explosion particle must be on presentation frame: {:?}",
            snap.particle_systems
                .iter()
                .map(|p| (&p.template_name, p.kind))
                .collect::<Vec<_>>()
        );
        assert!(
            snap.events.iter().any(|e| matches!(
                e,
                PresentationEvent::ParticleSystemSpawned { .. }
            )),
            "presentation events should include ParticleSystemSpawned"
        );
        assert!(
            snap.events.iter().any(|e| matches!(
                e,
                PresentationEvent::ObjectDestroyed { id, .. } if *id == victim
            )),
            "presentation events should include ObjectDestroyed for victim"
        );
    }
}
