use super::*;
use serde::{Deserialize, Serialize};

/// Visual information structure for rendering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectVisualInfo {
    pub position: Vec3,
    pub orientation: f32,
    pub team_color: [f32; 4],
    pub selection_radius: f32,
    /// Terrain ground height residual at object XY (presentation / FOW residual).
    #[serde(default)]
    pub ground_height: f32,
    /// True when ground_height came from terrain sample (not default 0).
    #[serde(default)]
    pub ground_height_from_terrain: bool,
    pub is_selected: bool,
    pub show_health_bar: bool,
    pub health_percentage: f32,
    pub model_name: Option<String>,
    pub object_type: ObjectType,
    pub team: Team,
    pub under_construction: bool,
    pub construction_percent: f32,
}

impl Object {
    /// Get visual information for rendering
    pub fn get_visual_info(&self) -> ObjectVisualInfo {
        ObjectVisualInfo {
            position: self.get_position(),
            orientation: self.get_orientation(),
            team_color: self.team_color,
            selection_radius: self.selection_radius,
            ground_height: self.ground_height,
            ground_height_from_terrain: self.ground_height_from_terrain,
            is_selected: self.selected,
            show_health_bar: self.show_health_bar && self.is_alive(),
            health_percentage: self.get_health_percentage(),
            model_name: self.thing.template.model_name.clone(),
            object_type: self.object_type,
            team: self.team,
            under_construction: self.status.under_construction,
            construction_percent: self.construction_percent,
        }
    }

    /// C++ `Object::setTeam` / leftover `apply_team_ai_profile`.
    /// Attitude and attack priority come from the named Team prototype
    /// (`AmericaTeamRangers`, `teamAmerica`), never the faction enum.
    pub fn apply_named_team_ai_profile(&mut self, force_attitude: bool) {
        let name = self.team_instance_name.trim();
        if name.is_empty() {
            return;
        }
        let Ok(factory) = gamelogic::team::get_team_factory().lock() else {
            return;
        };
        let Some(proto) = factory.find_team_prototype(name) else {
            return;
        };
        let prio_name = proto.get_attack_priority_name().as_str().to_string();
        let initial_att_i8 = match proto.get_initial_team_attitude() {
            gamelogic::team::AttitudeType::Sleep => -2i8,
            gamelogic::team::AttitudeType::Passive => -1,
            gamelogic::team::AttitudeType::Normal => 0,
            gamelogic::team::AttitudeType::Alert => 1,
            gamelogic::team::AttitudeType::Aggressive => 2,
            gamelogic::team::AttitudeType::Invalid => 0,
        };
        drop(factory);
        if self.attack_priority_set.is_none() && !prio_name.is_empty() {
            self.attack_priority_set = Some(prio_name);
        }
        if force_attitude || (self.ai_attitude == 0 && initial_att_i8 != 0) {
            self.set_ai_attitude_i8(initial_att_i8.clamp(-2, 2));
        }
    }

    /// Update team color (useful for changing allegiance)
    pub fn set_team(&mut self, team: Team) {
        let changed = self.team != team;
        if changed {
            self.team = team;
            self.team_color = team.get_color();
            // A team-only transfer has no controlling-player provenance. Do
            // not leave the prior player's identity attached to a captured or
            // neutralized object.
            self.owner_player_id = None;
            crate::game_logic::host_owner_log::record(self.id, team);
        } else {
            self.team = team;
            self.team_color = team.get_color();
        }
        self.record_host_identity();
        self.apply_fake_building_terrain_decal();
        if changed {
            // C++ Team::setControllingPlayer / Object::setTeam → handlePartitionCellMaintenance.
            self.handle_partition_cell_maintenance();
        }
        // C++ Object.cpp:857-872 setTeam: ai->setAttitude(named proto).
        self.apply_named_team_ai_profile(true);
    }

    /// Set faction presentation and exact controlling-player identity together.
    /// This is used by capture/hijack paths where the actor is known.
    pub fn set_team_and_owner(&mut self, team: Team, owner_player_id: Option<u32>) {
        let changed = self.team != team || self.owner_player_id != owner_player_id;
        self.team = team;
        self.team_color = team.get_color();
        self.owner_player_id = owner_player_id;
        if changed {
            crate::game_logic::host_owner_log::record_with_owner(self.id, team, owner_player_id);
        }
        self.record_host_identity();
        self.apply_fake_building_terrain_decal();
        if changed {
            // C++ Object::setTeam then onCapture handlePartitionCellMaintenance.
            self.handle_partition_cell_maintenance();
        }
        // C++ Object.cpp:857-872 setTeam: ai->setAttitude(named proto).
        self.apply_named_team_ai_profile(true);
    }

    /// Check if this object is visible to a team (for fog of war / targeting UI).
    /// C++ residual: stealthed-and-undetected units are hidden from non-allied teams.
    /// Detected stealthed units become visible (and targetable).
    pub fn is_visible_to_team(&self, team: Team) -> bool {
        // Team-local baseline visibility check. Global shroud/fog filtering is applied by
        // higher-level visibility queries in GameLogic that have object IDs and player context.
        if team == self.team {
            return true;
        }
        !self.is_effectively_stealthed()
    }

    /// Get a description string for UI display.
    /// C++ parity: prefers per-object name override, then template display
    /// name (from INI DisplayName), then template internal name.
    pub fn get_display_name(&self) -> String {
        if !self.name.is_empty() {
            return self.name.clone();
        }
        let tmpl_display = &self.thing.template.display_name;
        if !tmpl_display.is_empty() && tmpl_display != &self.template_name {
            return crate::assets::ini_parser::translate_object_display_name(tmpl_display);
        }
        self.template_name.clone()
    }
}
