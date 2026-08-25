use super::*;

impl Snapshotable for ThingTemplate {
    /// C++ Reference: ThingTemplate::crc (auto-generated via Snapshot macro)
    /// Iterates all module data entries and CRCs each one.
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let xfer_modules = |info: &ModuleInfo, xfer: &mut dyn Xfer| -> Result<(), String> {
            let count = info.get_count() as u32;
            xfer.xfer_unsigned_int(&mut count.clone())
                .map_err(|e| e.to_string())?;
            for i in 0..info.get_count() {
                if let Some(data) = info.get_nth_data(i) {
                    data.crc(xfer)?;
                }
            }
            Ok(())
        };

        xfer_modules(&self.behavior_module_info, xfer)?;
        xfer_modules(&self.draw_module_info, xfer)?;
        xfer_modules(&self.client_update_module_info, xfer)?;

        Ok(())
    }

    /// C++ Reference: ThingTemplate::xfer (auto-generated via Snapshot macro)
    /// Xfers all persistent template fields in C++ field order.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let xfer_err = |e: std::io::Error| e.to_string();

        xfer.xfer_unsigned_short(&mut self.template_id)
            .map_err(xfer_err)?;
        xfer.xfer_ascii_string(&mut self.name_string)
            .map_err(xfer_err)?;
        xfer.xfer_unicode_string(&mut self.display_name)
            .map_err(xfer_err)?;
        let mut display_color_u32 = self.display_color.0;
        xfer.xfer_unsigned_int(&mut display_color_u32)
            .map_err(xfer_err)?;
        self.display_color = Color(display_color_u32);

        // EditorSorting as u8
        let mut editor_sorting = self.editor_sorting as u8;
        xfer.xfer_unsigned_byte(&mut editor_sorting)
            .map_err(xfer_err)?;
        self.editor_sorting = match editor_sorting {
            1 => EditorSortingType::Structure,
            2 => EditorSortingType::Infantry,
            3 => EditorSortingType::Vehicle,
            4 => EditorSortingType::Shrubbery,
            5 => EditorSortingType::MiscManMade,
            6 => EditorSortingType::MiscNatural,
            7 => EditorSortingType::Debris,
            8 => EditorSortingType::System,
            9 => EditorSortingType::Audio,
            10 => EditorSortingType::Test,
            11 => EditorSortingType::ForReview,
            12 => EditorSortingType::Road,
            13 => EditorSortingType::Waypoint,
            _ => EditorSortingType::None,
        };

        // GeometryInfo fields (xfered inline since GeometryInfo is not Snapshotable)
        let mut geom_type_u8 = match self.geometry_info.geometry_type {
            GeometryType::Box => 0u8,
            GeometryType::Sphere => 1u8,
            GeometryType::Cylinder => 2u8,
        };
        xfer.xfer_unsigned_byte(&mut geom_type_u8)
            .map_err(xfer_err)?;
        self.geometry_info.geometry_type = match geom_type_u8 {
            0 => GeometryType::Box,
            1 => GeometryType::Sphere,
            _ => GeometryType::Cylinder,
        };
        xfer.xfer_bool(&mut self.geometry_info.is_small)
            .map_err(xfer_err)?;
        xfer.xfer_real(&mut self.geometry_info.width)
            .map_err(xfer_err)?;
        xfer.xfer_real(&mut self.geometry_info.height)
            .map_err(xfer_err)?;
        xfer.xfer_real(&mut self.geometry_info.depth)
            .map_err(xfer_err)?;

        xfer.xfer_real(&mut self.asset_scale).map_err(xfer_err)?;
        xfer.xfer_real(&mut self.instance_scale_fuzziness)
            .map_err(xfer_err)?;

        // Build properties
        xfer.xfer_unsigned_short(&mut self.build_cost)
            .map_err(xfer_err)?;
        xfer.xfer_real(&mut self.build_time).map_err(xfer_err)?;
        xfer.xfer_unsigned_short(&mut self.refund_value)
            .map_err(xfer_err)?;

        // Buildable as u8
        let mut buildable_u8 = self.buildable as u8;
        xfer.xfer_unsigned_byte(&mut buildable_u8)
            .map_err(xfer_err)?;
        self.buildable = match buildable_u8 {
            0 => BuildableStatus::Yes,
            1 => BuildableStatus::IgnorePrerequisites,
            2 => BuildableStatus::No,
            3 => BuildableStatus::OnlyByAi,
            _ => BuildableStatus::Yes,
        };

        // BuildCompletion as u8
        let mut completion_u8 = self.build_completion as u8;
        xfer.xfer_unsigned_byte(&mut completion_u8)
            .map_err(xfer_err)?;
        self.build_completion = match completion_u8 {
            1 => BuildCompletionType::AppearsAtRallyPoint,
            2 => BuildCompletionType::PlacedByPlayer,
            _ => BuildCompletionType::Invalid,
        };

        xfer.xfer_bool(&mut self.is_build_facility)
            .map_err(xfer_err)?;
        xfer.xfer_bool(&mut self.is_prerequisite)
            .map_err(xfer_err)?;
        xfer.xfer_bool(&mut self.is_forbidden).map_err(xfer_err)?;

        // KindOf mask as u128 (retail KINDOF_COUNT = 116)
        let mut kindof = self.kindof;
        xfer.xfer_u128(&mut kindof).map_err(xfer_err)?;
        self.kindof = kindof;

        xfer.xfer_ascii_string(&mut self.default_owning_side)
            .map_err(xfer_err)?;
        xfer.xfer_ascii_string(&mut self.command_set_string)
            .map_err(xfer_err)?;

        // Experience/skill arrays
        for val in self.skill_point_values.iter_mut() {
            xfer.xfer_int(val).map_err(xfer_err)?;
        }
        for val in self.experience_values.iter_mut() {
            xfer.xfer_int(val).map_err(xfer_err)?;
        }
        for val in self.experience_required.iter_mut() {
            xfer.xfer_int(val).map_err(xfer_err)?;
        }

        xfer.xfer_bool(&mut self.is_trainable).map_err(xfer_err)?;
        xfer.xfer_bool(&mut self.enter_guard).map_err(xfer_err)?;
        xfer.xfer_bool(&mut self.hijack_guard).map_err(xfer_err)?;

        // Shadow properties — raw C++ bit values (SHADOW_DECAL=1, VOLUME=2, …)
        let mut shadow_type_u8 = self.shadow_type.bits();
        xfer.xfer_unsigned_byte(&mut shadow_type_u8)
            .map_err(xfer_err)?;
        self.shadow_type = ShadowType(shadow_type_u8);

        xfer.xfer_real(&mut self.shadow_size_x).map_err(xfer_err)?;
        xfer.xfer_real(&mut self.shadow_size_y).map_err(xfer_err)?;
        xfer.xfer_real(&mut self.shadow_offset_x)
            .map_err(xfer_err)?;
        xfer.xfer_real(&mut self.shadow_offset_y)
            .map_err(xfer_err)?;
        xfer.xfer_ascii_string(&mut self.shadow_texture_name)
            .map_err(xfer_err)?;
        xfer.xfer_unsigned_int(&mut self.occlusion_delay)
            .map_err(xfer_err)?;

        // Radar priority as u8
        let mut radar_u8 = self.radar_priority as u8;
        xfer.xfer_unsigned_byte(&mut radar_u8).map_err(xfer_err)?;
        self.radar_priority = match radar_u8 {
            1 => RadarPriorityType::NotOnRadar,
            2 => RadarPriorityType::Structure,
            3 => RadarPriorityType::Unit,
            4 => RadarPriorityType::LocalUnitOnly,
            _ => RadarPriorityType::Invalid,
        };

        xfer.xfer_unsigned_byte(&mut self.transport_slot_count)
            .map_err(xfer_err)?;
        xfer.xfer_real(&mut self.fence_width).map_err(xfer_err)?;
        xfer.xfer_real(&mut self.fence_x_offset).map_err(xfer_err)?;
        xfer.xfer_bool(&mut self.is_bridge).map_err(xfer_err)?;
        xfer.xfer_real(&mut self.vision_range).map_err(xfer_err)?;
        xfer.xfer_real(&mut self.shroud_clearing_range)
            .map_err(xfer_err)?;
        xfer.xfer_real(&mut self.shroud_reveal_to_all_range)
            .map_err(xfer_err)?;
        xfer.xfer_real(&mut self.placement_view_angle)
            .map_err(xfer_err)?;
        xfer.xfer_real(&mut self.factory_exit_width)
            .map_err(xfer_err)?;
        xfer.xfer_real(&mut self.factory_extra_bib_width)
            .map_err(xfer_err)?;

        // Energy
        xfer.xfer_int(&mut self.energy_production)
            .map_err(xfer_err)?;
        xfer.xfer_int(&mut self.energy_bonus).map_err(xfer_err)?;

        // Combat
        xfer.xfer_unsigned_short(&mut self.threat_value)
            .map_err(xfer_err)?;
        xfer.xfer_unsigned_short(&mut self.max_simultaneous_of_type)
            .map_err(xfer_err)?;
        xfer.xfer_unsigned_int(&mut self.max_simultaneous_link_key)
            .map_err(xfer_err)?;
        xfer.xfer_bool(&mut self.max_simultaneous_determined_by_superweapon_restriction)
            .map_err(xfer_err)?;
        xfer.xfer_unsigned_byte(&mut self.crusher_level)
            .map_err(xfer_err)?;
        xfer.xfer_unsigned_byte(&mut self.crushable_level)
            .map_err(xfer_err)?;
        xfer.xfer_unsigned_byte(&mut self.structure_rubble_height)
            .map_err(xfer_err)?;

        // Module infos
        let xfer_module_info = |info: &mut ModuleInfo, xfer: &mut dyn Xfer| -> Result<(), String> {
            let mut count = info.get_count() as u32;
            xfer.xfer_unsigned_int(&mut count).map_err(xfer_err)?;
            for i in 0..info.get_count() {
                if let Some(name_str) = info.get_nth_name(i).cloned() {
                    let mut name = name_str;
                    xfer.xfer_ascii_string(&mut name).map_err(xfer_err)?;
                }
                if let Some(tag_str) = info.get_nth_tag(i).cloned() {
                    let mut tag = tag_str;
                    xfer.xfer_ascii_string(&mut tag).map_err(xfer_err)?;
                }
            }
            Ok(())
        };

        xfer_module_info(&mut self.behavior_module_info, xfer)?;
        xfer_module_info(&mut self.draw_module_info, xfer)?;
        xfer_module_info(&mut self.client_update_module_info, xfer)?;

        Ok(())
    }

    /// C++ Reference: ThingTemplate::loadPostProcess
    /// Resolves name references after loading from save game.
    fn load_post_process(&mut self) -> Result<(), String> {
        self.resolve_names();
        Ok(())
    }
}
