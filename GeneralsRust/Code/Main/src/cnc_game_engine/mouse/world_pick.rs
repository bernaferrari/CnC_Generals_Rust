#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;

impl CnCGameEngine {
    pub(in crate::cnc_game_engine) fn update_mouse_world_position(&mut self) {
        // C++ maps device coordinates through the active W3D camera.  The former
        // whole-map linear interpolation only happened to work while the camera
        // was centered and made ordinary selection/orders drift after panning,
        // rotation, or zoom.
        // A script/minimap/hotkey can change the orbit between normal camera
        // ticks and an OS mouse event.  Pick from the pose that will actually
        // be rendered, rather than the previous frame's matrix.
        if self.camera_transform_needs_rebuild() {
            self.apply_camera_orbit_transform();
        }
        let (view_w, view_h) = self.tactical_viewport_size();
        if self.mouse_position.1 > view_h {
            return;
        }
        let (world_min, world_max) = self.presentation_world_bounds();
        let picked = {
            let world_env = self
                .render_pipeline
                .presentation_frame()
                .or(self.last_presentation_frame.as_ref())
                .map(|frame| &frame.world_env);
            unproject_mouse_ray(
                self.view_matrix,
                self.projection_matrix,
                self.mouse_position,
                view_w,
                view_h,
            )
            .and_then(|(near, far)| {
                raycast_frozen_terrain(near, far, world_min, world_max, world_env).or_else(|| {
                    raycast_ground_plane_clamped(near, far, world_min, world_max, world_env)
                })
            })
        };
        if let Some(position) = picked {
            self.mouse_world_position = position;
        }
    }

    /// Presentation-only world pick. Returns `None` when no snapshot is installed
    /// (no live GameLogic dual-read residual). InGame always seeds
    /// `last_presentation_frame` before input.

    /// Wave 228: build presentation target hint for RMB classification.

    /// Wave 229: presentation-frozen selected-unit capabilities for RMB classification.
    pub(in crate::cnc_game_engine) fn presentation_selected_unit_hints(
        &self,
        ids: &[crate::game_logic::ObjectId],
    ) -> Vec<crate::command_system::PresentationSelectedUnitHint> {
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return Vec::new();
        };
        let mut out = Vec::with_capacity(ids.len());
        for &id in ids {
            let Some(o) = frame.objects.iter().find(|x| x.id == id) else {
                continue;
            };
            // Wave 1097: selected-hint residual fail-closed on unusable sources.
            if o.destroyed
                || o.health_current <= 0.0
                || o.sold
                || o.under_construction
                || o.masked
                || o.disabled
                || o.unselectable
            {
                continue;
            }
            // C++ construction/structure repair authority is `KINDOF_DOZER`.
            // Preserve it in the frozen input instead of classifying a unit
            // by its UI name (a harvester/worker is not necessarily a dozer).
            let is_worker = crate::presentation_frame::PresentationFrame::object_has_kind(
                o,
                crate::game_logic::KindOf::Dozer,
            );
            // Gather authorization is an authored capability, not a template
            // naming convention.  C++ marks Chinooks, Supply Trucks, and GLA
            // Workers with KINDOF_HARVESTER.
            let is_resource_collector =
                crate::presentation_frame::PresentationFrame::object_has_kind(
                    o,
                    crate::game_logic::KindOf::Harvester,
                );
            let can_attack = o.has_weapon;
            let can_move = o.is_mobile;
            let capture_power = o.capture_power;
            let capture_power_ready = o.capture_power_ready;
            let can_capture =
                capture_power != crate::game_logic::CapturePowerKind::None && capture_power_ready;
            let can_repair = is_worker;
            let is_damaged = o.health_max > 0.0 && o.health_current + 0.01 < o.health_max;
            let is_vehicle = crate::presentation_frame::PresentationFrame::object_has_kind(
                o,
                crate::game_logic::KindOf::Vehicle,
            );
            let is_aircraft = crate::presentation_frame::PresentationFrame::object_has_kind(
                o,
                crate::game_logic::KindOf::Aircraft,
            );
            let is_infantry = crate::presentation_frame::PresentationFrame::object_has_kind(
                o,
                crate::game_logic::KindOf::Infantry,
            );
            let is_above_terrain = o.airborne_target
                || (o.ground_height_from_terrain && o.position.y > o.ground_height + 0.01);
            out.push(crate::command_system::PresentationSelectedUnitHint {
                id,
                is_alive: true,
                is_resource_collector,
                is_worker,
                can_attack,
                can_move,
                can_request_service: o.contained_by.is_none(),
                can_capture,
                template_name: o.template_name.clone(),
                can_repair,
                is_damaged,
                is_vehicle,
                is_aircraft,
                is_above_terrain,
                is_infantry,
                transport_slot_count: o.transport_slot_count,
                stored_supplies: o.stored_supplies,
                is_controlled_by_local: frame.is_owned_by_local(o),
                capture_power,
                capture_power_ready,
                is_salvager: crate::presentation_frame::PresentationFrame::object_has_kind(
                    o,
                    crate::game_logic::KindOf::Salvager,
                ),
                can_override_special_power_destination: o
                    .special_power_override_destination
                    .is_some(),
            });
        }
        out
    }

    pub(in crate::cnc_game_engine) fn presentation_target_hint(
        &self,
        id: crate::game_logic::ObjectId,
    ) -> Option<crate::command_system::PresentationTargetHint> {
        let frame = self.last_presentation_frame.as_ref()?;
        // Wave 1097: target-hint residual fail-closed on sold/masked and non-local
        // FOW unless Clear (matches pick peels 1093–1096).
        let o = frame.objects.iter().find(|x| {
            x.id == id && !x.destroyed && !x.sold && !x.masked && !frame.box_pick_hides_non_local(x)
        })?;
        let is_neutral = o.team == crate::game_logic::Team::Neutral;
        let is_enemy = frame.is_enemy_of_local(o);
        let is_structure = o.object_type
            == crate::presentation_frame::PresentationObjectType::Building
            || crate::presentation_frame::PresentationFrame::object_has_kind(
                o,
                crate::game_logic::KindOf::Structure,
            );
        let is_resource = crate::presentation_frame::PresentationFrame::object_has_kind(
            o,
            crate::game_logic::KindOf::Harvestable,
        ) || crate::presentation_frame::PresentationFrame::object_has_kind(
            o,
            crate::game_logic::KindOf::Resource,
        );
        let enter_available_capacity = frame
            .normal_enter_available_capacity_for_local(o)
            .unwrap_or(0);
        let can_be_entered = enter_available_capacity > 0;
        let is_damaged = o.health_max > 0.0 && o.health_current + 0.01 < o.health_max;
        let is_friendly = !is_neutral && frame.is_allied_with_local(o);
        // Freeze exact Object INI KindOf service tags.  The executor repeats
        // these pairings against live authority when consuming the command.
        let provides_heal = crate::presentation_frame::PresentationFrame::object_has_kind(
            o,
            crate::game_logic::KindOf::HealPad,
        );
        let provides_aircraft_repair =
            crate::presentation_frame::PresentationFrame::object_has_kind(
                o,
                crate::game_logic::KindOf::FSAirfield,
            );
        let provides_vehicle_repair = crate::presentation_frame::PresentationFrame::object_has_kind(
            o,
            crate::game_logic::KindOf::RepairPad,
        );
        // C++ treats a non-stealthed occupant as a GarrisonContain gate, but
        // checks friendly contained occupants separately for every target.
        // Freeze both, including stale references which must fail closed.
        let (capture_nonstealthed_garrison_count, capture_friendly_garrison_count) = o
            .garrisoned_units
            .iter()
            .fold((0u16, 0u16), |counts, occupant_id| {
                let Some(occupant) = frame
                    .objects
                    .iter()
                    .find(|candidate| candidate.id == *occupant_id)
                else {
                    return (
                        counts.0.saturating_add(o.capture_garrisonable as u16),
                        counts.1.saturating_add(1),
                    );
                };
                (
                    counts
                        .0
                        .saturating_add((o.capture_garrisonable && !occupant.stealthed) as u16),
                    counts
                        .1
                        .saturating_add(frame.is_allied_with_local(occupant) as u16),
                )
            });
        Some(crate::command_system::PresentationTargetHint {
            id,
            // Wave 1098: is_alive residual excludes sold/masked.
            is_alive: !o.destroyed && o.health_current > 0.0 && !o.sold && !o.masked,
            is_structure,
            is_resource,
            under_construction: o.under_construction,
            sold: o.sold,
            team: o.team,
            is_enemy_of_local: is_enemy,
            is_neutral,
            template_name: o.template_name.clone(),
            can_be_entered,
            enter_available_capacity,
            enter_uses_transport_slots: o.normal_enter_uses_transport_slots(),
            enter_requires_infantry: o.normal_enter_requires_infantry(),
            enter_forbids_aircraft: o.normal_enter_forbids_aircraft(),
            enter_disabled_subdued: o.disabled_subdued,
            enter_is_rider_change: o.contain_module_kind
                == crate::game_logic::ContainModuleKind::RiderChange,
            rider_change_allowed_templates: o.rider_change_allowed_templates.clone(),
            is_damaged,
            is_friendly_of_local: is_friendly,
            // ActionManager keys service on KindOf, not ObjectType::Building.
            provides_vehicle_repair,
            provides_aircraft_repair,
            provides_heal,
            can_provide_service: o.contained_by.is_none(),
            dock_kind: o.dock_kind,
            dock_controller_is_local: frame.is_owned_by_local(o),
            stored_supplies: o.stored_supplies,
            capturable: o.capturable,
            immune_to_capture: o.immune_to_capture,
            capture_garrisonable: o.capture_garrisonable,
            capture_nonstealthed_garrison_count,
            capture_friendly_garrison_count,
            capture_target_effectively_stealthed: o.effectively_stealthed,
            is_crate: o.is_crate,
            is_salvage_crate: o.is_salvage_crate,
            is_vehicle: crate::presentation_frame::PresentationFrame::object_has_kind(
                o,
                crate::game_logic::KindOf::Vehicle,
            ),
            is_aircraft: crate::presentation_frame::PresentationFrame::object_has_kind(
                o,
                crate::game_logic::KindOf::Aircraft,
            ),
            is_drone: crate::presentation_frame::PresentationFrame::object_has_kind(
                o,
                crate::game_logic::KindOf::Drone,
            ),
            is_carbomb: o.is_carbomb,
            is_unmanned: o.disabled_unmanned,
            is_mine: o.has_mine
                || crate::presentation_frame::PresentationFrame::object_has_kind(
                    o,
                    crate::game_logic::KindOf::Mine,
                )
                || crate::presentation_frame::PresentationFrame::object_has_kind(
                    o,
                    crate::game_logic::KindOf::DemoTrap,
                )
                || crate::game_logic::host_car_bomb::object_definition_has_kind(
                    &o.template_name,
                    "MINE",
                ),
        })
    }

    pub(in crate::cnc_game_engine) fn find_object_at_position(
        &self,
        position: Vec3,
        command_context: bool,
    ) -> Option<ObjectId> {
        // Wave 222: presentation-only pick (no GameLogic dual-read residual).
        // hq-bzidj: widen with mines/shrubbery via SelectionInfo pick types.
        self.host_find_object_at_position(position, command_context)
    }

    pub(in crate::cnc_game_engine) fn find_object_at_cursor(
        &self,
        command_context: bool,
    ) -> Option<ObjectId> {
        self.host_pick_object_at_cursor(command_context)
    }

    /// C++ `InGameUI::setPreventLeftClickDeselectionInAlternateMouseModeForOneClick`.
    pub(in crate::cnc_game_engine) fn host_set_prevent_left_click_deselection(
        &mut self,
        enabled: bool,
    ) {
        self.prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click = enabled;
        game_client::helpers::TheInGameUI::set_prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click(
            enabled,
        );
    }

    /// C++ `SelectionXlat.cpp:935-943`: consume the one-click keep-selection flag.
    pub(in crate::cnc_game_engine) fn host_consume_prevent_left_click_deselection(
        &mut self,
    ) -> bool {
        let leftover = game_client::helpers::TheInGameUI::get_prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click();
        let prevent =
            self.prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click || leftover;
        if prevent {
            self.host_set_prevent_left_click_deselection(false);
        }
        prevent
    }

    /// Path following is authoritative in `GameLogic::update_movement`.
    /// Retained as a no-op compatibility hook for older call sites.

    /// Legacy render stub -- NOT called from the active render path.
    /// Actual rendering is handled by RenderPipeline::execute() -> ForwardPass::render()
    /// which queues MeshClass instances into the WW3D Renderer and issues real draw calls.
    #[allow(dead_code)] // Legacy stub: superseded by RenderPipeline, retained for reference
    pub(in crate::cnc_game_engine) fn render_game_objects<'a>(
        &'a self,
        _render_pass: &mut wgpu::RenderPass<'a>,
    ) {
        // Presentation-only stub: RenderPipeline is the sole draw path.
        let n = self
            .last_presentation_frame
            .as_ref()
            .map(|f| f.objects.len())
            .unwrap_or(0);
        log::trace!(
            "Legacy stub: presentation has {} objects (RenderPipeline is sole draw path)",
            n
        );
    }

    /// Legacy per-object render stub -- logs model status but does NOT submit draw calls.
    /// The active render path is RenderPipeline::collect_render_items() which builds
    /// RenderItem list and ForwardPass::prepare_mesh_instance() which creates actual
    /// MeshClass instances submitted to the WW3D Renderer.
    #[allow(dead_code)] // Legacy stub: superseded by RenderPipeline, retained for reference
    pub(in crate::cnc_game_engine) fn render_object<'a>(
        &'a self,
        obj: &Object,
        _render_pass: &mut wgpu::RenderPass<'a>,
    ) {
        let model_name = obj.get_template().get_model_name();

        log::trace!(
            "Render object {} template '{}' model '{}' (cached={})",
            obj.id,
            obj.template_name,
            model_name,
            self.graphics_system.get_model(model_name).is_some()
        );

        let w3d_model = self
            .graphics_system
            .get_model(model_name)
            .or_else(|| self.graphics_system.get_model(&obj.template_name));

        if let Some(w3d_model) = w3d_model {
            let total_vertices: usize = w3d_model
                .meshes
                .iter()
                .map(|mesh| mesh.vertices.len())
                .sum();
            let total_indices: usize = w3d_model.meshes.iter().map(|mesh| mesh.indices.len()).sum();

            log::trace!(
                "Rendering W3D model: {} (template: {}) with {} vertices, {} indices across {} meshes",
                model_name,
                obj.template_name,
                total_vertices,
                total_indices,
                w3d_model.meshes.len()
            );
            log::trace!("Resolved W3D model '{}' for object {}", model_name, obj.id);
        } else {
            log::debug!(
                "No W3D model resolved for object {} template '{}' (model '{}') -- fallback cube will be used by RenderPipeline",
                obj.id,
                obj.template_name,
                model_name
            );
        }
    }

    #[allow(dead_code)] // Legacy stub: selection_renderer + PresentationFrame own production path
    pub(in crate::cnc_game_engine) fn render_selection_indicators(
        &self,
        _render_pass: &mut wgpu::RenderPass,
    ) {
        // Prefer presentation selected residual when installed (no live find_object dual-read).
        if let Some(frame) = self.last_presentation_frame.as_ref() {
            let n = frame
                .objects
                .iter()
                .filter(|o| o.selected && !o.destroyed)
                .count();
            log::trace!(
                "Legacy stub: presentation selected count={n} (selection_renderer is sole path)"
            );
            return;
        }
        // Boot residual only.
        for &object_id in &self.selected_objects {
            let _ = object_id;
        }
    }

    pub(in crate::cnc_game_engine) fn render_projectiles(
        &self,
        _render_pass: &mut wgpu::RenderPass,
    ) {
        // Projectiles render from PresentationFrame (host CombatSystem freeze).
    }

    pub(in crate::cnc_game_engine) fn render_ui(&self, _render_pass: &mut wgpu::RenderPass) {
        if let Err(err) = self.ui_manager.render() {
            log::warn!("UI manager render failed: {}", err);
        }
        log::trace!(
            "UI overlay rendered for {} selected units",
            self.selected_objects.len()
        );
    }
}
