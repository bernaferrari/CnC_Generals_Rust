//! C++ BuildAssistant::buildObjectLineNow: one constructor and two creation paths.
use super::*;
use std::collections::HashSet;

impl CommandExecutor<'_> {
    pub(super) fn execute_dozer_line(
        &mut self,
        units: &[ObjectId],
        template_name: &str,
        start: Vec3,
        end: Vec3,
        orientation: f32,
    ) -> CommandResult {
        let Some(&builder) = units.first() else {
            return CommandResult::InvalidCommand;
        };
        if !start.is_finite() || !end.is_finite() || !orientation.is_finite() {
            return CommandResult::InvalidLocation;
        }
        let Some(producer) = self.game_logic.host_object(builder) else {
            return CommandResult::InvalidCommand;
        };
        if !producer.is_alive() || producer.owner_player_id != Some(self.current_player_id) {
            return CommandResult::InvalidCommand;
        }
        let positions =
            self.line_build_tiled_positions(builder, template_name, start, end, orientation);
        // Original partition cells admit births at the later update phase.
        // Keep the live objects callback-visible while limiting these collision
        // queries to the cohort that existed before this synchronous line.
        // Full partition dirty-cell ownership remains tracked by hq-n692e.
        let collision_candidates = self
            .game_logic
            .host_objects()
            .registered_keys()
            .copied()
            .collect::<HashSet<_>>();
        let mut placed = false;
        // C++ computes positions once, then tries EVERY prepared tile, even after
        // one buildObjectNow fails. Each call rereads the same constructor.
        for location in positions {
            if !self
                .game_logic
                .constructor_can_build_template(builder, template_name)
            {
                continue;
            }
            let ordinary_dozer = self
                .game_logic
                .host_object(builder)
                .is_some_and(|producer| producer.is_kind_of(KindOf::Dozer))
                && self
                    .game_logic
                    .get_templates()
                    .get(template_name)
                    .is_some_and(|template| !template.is_kind_of(KindOf::LineBuild));
            if ordinary_dozer {
                placed |= self.execute_prepared_dozer_build(
                    builder,
                    template_name,
                    location,
                    orientation,
                    &collision_candidates,
                ) == CommandResult::Success;
                continue;
            }
            self.clear_removable_for_construction_with_candidates(
                location,
                orientation,
                template_name,
                Some(&collision_candidates),
            );
            if !self
                .game_logic
                .move_objects_for_construction_with_candidates_with_effects(
                    template_name,
                    location,
                    orientation,
                    Some(builder),
                    Some(&collision_candidates),
                    &mut self.ui,
                )
                && self.game_logic.player_is_human(self.current_player_id)
            {
                continue;
            }
            placed |= self
                .game_logic
                .create_instant_line_build_object_for_player_with_effects(
                    template_name,
                    self.current_player_id,
                    location,
                    orientation,
                    builder,
                    &mut self.ui,
                )
                .is_some();
        }
        // GameLogicDispatch emits this once after the whole line attempt,
        // including a valid constructor whose placement attempts all failed.
        self.game_logic.queue_audio_event(
            AudioEventRequest::new(translate_audio_event("PlaceBuilding")).with_object(builder),
        );
        if !placed {
            return CommandResult::InvalidCommand;
        }
        CommandResult::Success
    }
    /// C++ `BuildAssistant::buildTiledLocations` (BuildAssistant.cpp:1090-1191).
    fn line_build_tiled_positions(
        &mut self,
        builder_id: ObjectId,
        template_name: &str,
        start: Vec3,
        end: Vec3,
        orientation: f32,
    ) -> Vec<Vec3> {
        let Some(spacing) = self.line_build_tile_spacing(template_name) else {
            return Vec::new();
        };
        let dx = end.x - start.x;
        let dz = end.z - start.z;
        let len = (dx * dx + dz * dz).sqrt();
        if !len.is_finite() {
            return Vec::new();
        }
        let tiles_needed = ((len / spacing).floor() as usize)
            .saturating_add(1)
            .min(self.game_logic.line_build_object_limit().max(1) as usize);
        let team = self
            .game_logic
            .host_object(builder_id)
            .map(|o| o.team)
            .unwrap_or(Team::USA);
        let mut positions = Vec::with_capacity(tiles_needed.min(64));
        positions.push(start);
        if tiles_needed == 1 || len == 0.0 {
            return positions;
        }
        // Coord3D::normalize divides each component by length. Preserve the
        // original rounding and multiply spacing by index before direction.
        let dir_x = dx / len;
        let dir_z = dz / len;
        for i in 1..tiles_needed {
            let pos = Vec3::new(
                start.x + dir_x * (spacing * i as f32),
                self.game_logic
                    .terrain_height_at(Vec3::new(
                        start.x + dir_x * (spacing * i as f32),
                        0.0,
                        start.z + dir_z * (spacing * i as f32),
                    ))
                    .unwrap_or(0.0),
                start.z + dir_z * (spacing * i as f32),
            );
            if !self
                .game_logic
                .is_location_legal_to_build_at_angle_for_builder_with_effects(
                    team,
                    pos,
                    template_name,
                    orientation,
                    Some(builder_id),
                    &mut self.ui,
                )
            {
                break;
            }
            positions.push(pos);
        }
        positions
    }

    /// C++ `what->getTemplateGeometryInfo().getMajorRadius() * 2.0f`.
    fn line_build_tile_spacing(&self, template_name: &str) -> Option<f32> {
        let radius = self
            .game_logic
            .get_templates()
            .get(template_name)?
            .geometry_info
            .major_radius;
        let spacing = radius * 2.0;
        (spacing.is_finite() && spacing > 0.0).then_some(spacing)
    }
}
