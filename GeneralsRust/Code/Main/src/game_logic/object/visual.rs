use super::*;
use serde::{Deserialize, Serialize};

/// Per-object C++ DebrisDrawInterface state. The receiving renderer resolves
/// these names against its own model assets.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DebrisDrawState {
    pub model_name: String,
    pub anim_initial: String,
    pub anim_flying: String,
    pub anim_final: String,
    pub fx_final: Option<String>,
    pub allow_model_color_change: bool,
    pub shadow_type: u32,
}

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
    /// Receiving-session identity of the exact Team instance that owns this
    /// object. Unlike `team`, this distinguishes same-faction Team instances.
    pub fn team_instance_identity(&self) -> Option<u32> {
        self.team_instance_id
    }

    /// Current model name for presentation, including a per-debris override.
    pub fn current_visual_model_name(&self) -> String {
        self.debris_draw
            .as_ref()
            .map(|draw| draw.model_name.clone())
            .or_else(|| self.thing.template.model_name.clone())
            .unwrap_or_default()
    }

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
            model_name: self
                .debris_draw
                .as_ref()
                .map(|draw| draw.model_name.clone())
                .or_else(|| self.thing.template.model_name.clone()),
            object_type: self.object_type,
            team: self.team,
            under_construction: self.status.under_construction,
            construction_percent: self.construction_percent,
        }
    }

    /// C++ `Object::setTeam` / leftover `apply_team_ai_profile`.
    /// Attitude and attack priority come from the named Team prototype
    /// (`AmericaTeamRangers`, `teamAmerica`), never the faction enum.
    pub fn apply_named_team_ai_profile(
        &mut self,
        team_factory: &gamelogic::team::TeamFactoryHandle,
        force_attitude: bool,
    ) {
        let name = self.team_instance_name.trim();
        if name.is_empty() {
            return;
        }
        let Ok(factory) = team_factory.lock() else {
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

    /// Update team color and allegiance. Prototype-backed AI state is applied by
    /// [`set_team_with_factory`] at the world boundary that owns this object.
    pub fn set_team(
        &mut self,
        team: Team,
        ownership_events: &mut crate::game_logic::host_owner_log::OwnershipEvents,
    ) {
        if let Some(event) = self.set_team_state(team) {
            ownership_events.record(event.object, event.team, event.owner_player_id);
        }
    }

    pub(crate) fn set_team_with_partition_owner(
        &mut self,
        team: Team,
        ownership_events: &mut crate::game_logic::host_owner_log::OwnershipEvents,
        partition: &mut crate::game_logic::partition_manager::PartitionManager,
    ) {
        if let Some(event) = self.set_team_state_with_partition_context(team, Some(partition)) {
            ownership_events.record(event.object, event.team, event.owner_player_id);
        }
    }

    /// Canonical allegiance mutation for a detached body operation; live callers
    /// immediately admit its immutable observation into their receiving output.
    pub(super) fn set_team_state(
        &mut self,
        team: Team,
    ) -> Option<crate::game_logic::host_owner_log::HostOwnerEvent> {
        self.set_team_state_with_partition_context(team, None)
    }

    pub(super) fn set_team_state_with_partition_context(
        &mut self,
        team: Team,
        partition: Option<&mut crate::game_logic::partition_manager::PartitionManager>,
    ) -> Option<crate::game_logic::host_owner_log::HostOwnerEvent> {
        self.require_partition_owner(partition.as_deref());
        let changed = self.team != team;
        if changed {
            self.team = team;
            self.team_color = team.get_color();
            // A team-only transfer has no controlling-player provenance. Do
            // not leave the prior player's identity attached to a captured or
            // neutralized object.
            self.owner_player_id = None;
        } else {
            self.team = team;
            self.team_color = team.get_color();
        }

        self.apply_fake_building_terrain_decal();
        if changed {
            // C++ Team::setControllingPlayer / Object::setTeam → handlePartitionCellMaintenance.
            if let Some(partition) = partition {
                self.handle_partition_cell_maintenance_with_partition_owner(partition);
            }
        }
        changed.then_some(crate::game_logic::host_owner_log::HostOwnerEvent {
            object: self.id,
            team,
            owner_player_id: None,
        })
    }

    /// C++ Object.cpp:857-872 `setTeam`: apply the owning world's named-team
    /// AI profile after updating the object's allegiance.
    pub fn set_team_with_factory(
        &mut self,
        team: Team,
        team_factory: &gamelogic::team::TeamFactoryHandle,
        ownership_events: &mut crate::game_logic::host_owner_log::OwnershipEvents,
    ) {
        self.set_team(team, ownership_events);
        self.apply_named_team_ai_profile(team_factory, true);
    }

    /// Set faction presentation and exact controlling-player identity together.
    /// This is used by capture/hijack paths where the actor is known.
    pub fn set_team_and_owner(
        &mut self,
        team: Team,
        owner_player_id: Option<u32>,
        ownership_events: &mut crate::game_logic::host_owner_log::OwnershipEvents,
    ) {
        self.set_team_and_owner_with_partition_context(
            team,
            owner_player_id,
            ownership_events,
            None,
        );
    }
    pub(crate) fn set_team_and_owner_with_partition_owner(
        &mut self,
        team: Team,
        owner_player_id: Option<u32>,
        ownership_events: &mut crate::game_logic::host_owner_log::OwnershipEvents,
        partition: &mut crate::game_logic::partition_manager::PartitionManager,
    ) {
        self.set_team_and_owner_with_partition_context(
            team,
            owner_player_id,
            ownership_events,
            Some(partition),
        );
    }
    pub(super) fn set_team_and_owner_with_partition_context(
        &mut self,
        team: Team,
        owner_player_id: Option<u32>,
        ownership_events: &mut crate::game_logic::host_owner_log::OwnershipEvents,
        partition: Option<&mut crate::game_logic::partition_manager::PartitionManager>,
    ) {
        self.require_partition_owner(partition.as_deref());
        let changed = self.team != team || self.owner_player_id != owner_player_id;
        self.team = team;
        self.team_color = team.get_color();
        self.owner_player_id = owner_player_id;
        if changed {
            ownership_events.record(self.id, team, owner_player_id);
        }

        self.apply_fake_building_terrain_decal();
        if changed {
            // C++ Object::setTeam then onCapture handlePartitionCellMaintenance.
            if let Some(partition) = partition {
                self.handle_partition_cell_maintenance_with_partition_owner(partition);
            }
        }
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
