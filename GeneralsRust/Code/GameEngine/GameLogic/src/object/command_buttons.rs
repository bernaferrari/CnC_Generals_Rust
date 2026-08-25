//! Object command-button special-power / dozer / hijack C++ parity hooks.
//!
//! Child of `object` so private `Object` fields remain visible.

#![allow(unused_imports)]

use super::object_impl_imports::*;
use super::*;

impl Object {
    pub(super) fn command_source_is_forced(source: CommandSource) -> bool {
        source == CommandSourceType::FromScript
    }

    /// No-target `GUI_COMMAND_SPECIAL_POWER` → `doSpecialPower(..., cmdSource == CMD_FROM_SCRIPT)`.
    pub(super) fn command_button_special_power_no_target(
        &self,
        template_name: &str,
        options: crate::object::special_power_module::SpecialPowerCommandOptions,
        source: CommandSource,
    ) {
        self.do_special_power(
            template_name,
            options,
            Self::command_source_is_forced(source),
        );
    }

    /// At-object special power with `forced = (source == FromScript)`.
    pub(super) fn command_button_special_power_at_object(
        &self,
        template_name: &str,
        target_id: ObjectID,
        options: crate::object::special_power_module::SpecialPowerCommandOptions,
        source: CommandSource,
    ) {
        self.do_special_power_at_object(
            template_name,
            target_id,
            options,
            Self::command_source_is_forced(source),
        );
    }

    /// At-location special power; forwards `angle` like C++ `doSpecialPowerAtLocation`.
    pub(super) fn command_button_special_power_at_location(
        &self,
        template_name: &str,
        location: &Coord3D,
        angle: f32,
        options: crate::object::special_power_module::SpecialPowerCommandOptions,
        source: CommandSource,
    ) {
        self.do_special_power_at_location(
            template_name,
            location,
            angle,
            options,
            Self::command_source_is_forced(source),
        );
    }

    /// Waypoint special power with `forced = (source == FromScript)`.
    pub(super) fn command_button_special_power_using_waypoints(
        &self,
        template_name: &str,
        waypoint: &crate::object::special_power_module::Waypoint,
        options: crate::object::special_power_module::SpecialPowerCommandOptions,
        source: CommandSource,
    ) {
        let _ = self.do_special_power_using_waypoints_forced(
            template_name,
            waypoint,
            options,
            Self::command_source_is_forced(source),
        );
    }

    /// No-target `GUI_COMMAND_DOZER_CONSTRUCT` / `UNIT_BUILD` → `queueCreateUnit`.
    pub(super) fn command_button_dozer_construct_no_target(
        &self,
        template: &Arc<dyn crate::common::ThingTemplate>,
    ) -> bool {
        self.queue_unit_via_production(template)
    }

    /// At-position `GUI_COMMAND_DOZER_CONSTRUCT` → `TheBuildAssistant->buildObjectNow`.
    pub(super) fn command_button_dozer_construct_at_position(
        &self,
        template: &Arc<dyn crate::common::ThingTemplate>,
        pos: &Coord3D,
    ) {
        use game_engine::common::system::build_assistant;

        let Some(assistant) = build_assistant::get_build_assistant() else {
            return;
        };
        let builder_snapshot = build_assistant::Object {
            id: self.get_id(),
            position: build_assistant::Coord3D {
                x: self.get_position().x,
                y: self.get_position().y,
                z: self.get_position().z,
            },
            orientation: self.get_orientation(),
            command_set: None,
        };
        let player_index = self
            .get_controlling_player()
            .and_then(|p| p.read().ok().map(|g| g.get_player_index() as u32))
            .unwrap_or(0);
        let owning_player = build_assistant::Player { player_index };
        let mut assistant_template =
            build_assistant::ThingTemplate::new(template.get_name().as_str());
        let template_geometry = template.get_template_geometry_info();
        assistant_template.geometry_info.major_radius =
            template_geometry.get_major_radius().max(1.0);
        assistant_template.geometry_info.minor_radius =
            template_geometry.get_minor_radius().max(1.0);
        assistant_template.geometry_info.height =
            template_geometry.get_max_height_above_position().max(1.0);
        let _ = assistant.build_object_now(
            Some(&builder_snapshot),
            &assistant_template,
            &build_assistant::Coord3D {
                x: pos.x,
                y: pos.y,
                z: pos.z,
            },
            0.0,
            &owning_player,
        );
    }
}
