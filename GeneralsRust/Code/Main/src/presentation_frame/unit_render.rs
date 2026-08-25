use super::*;

#[cfg(feature = "game_client")]
#[path = "physics_visual_host_inputs.rs"]
pub(crate) mod physics_visual_host_inputs;

#[cfg(feature = "game_client")]
#[path = "host_draw_schedule.rs"]
pub(crate) mod host_draw_schedule;

#[cfg(feature = "game_client")]
#[path = "physics_visual_host.rs"]
pub(crate) mod physics_visual_host;

#[cfg(all(test, feature = "game_client"))]
#[path = "physics_visual_host_tests.rs"]
mod physics_visual_host_tests;

#[cfg(all(test, feature = "game_client"))]
#[path = "host_draw_schedule_tests.rs"]
mod host_draw_schedule_tests;

/// Build the dynamic part of one selected Draw module's C++ projectile-bone
/// visibility vector.  The caller appends it after static state directives so
/// the existing exact-HLOD resolver preserves C++ last-write behavior.
fn authored_projectile_clip_subobject_visibility(
    draw_model: &crate::assets::AuthoredDrawModel,
    statuses: &[Option<PresentationProjectileClipStatus>; 3],
) -> Vec<crate::assets::AuthoredDrawSubobjectVisibility> {
    let feedback = &draw_model.projectile_bone_feedback;
    let bindings = &draw_model.weapon_bone_bindings;
    if !feedback.source_fields_valid || !bindings.source_fields_valid {
        return Vec::new();
    }

    let mut directives = Vec::new();
    for slot in [0u8, 1, 2] {
        if !feedback.is_enabled_for_slot(slot) {
            continue;
        }
        let Some(status) = statuses.get(usize::from(slot)).copied().flatten() else {
            continue;
        };
        // C++ debug-crashes then returns when `maxCount < showCount`.
        if status.max_shots == 0 || status.max_shots < status.shots_remaining {
            continue;
        }
        let Some(binding) = bindings.slot(slot) else {
            continue;
        };
        let hide_count = status.max_shots - status.shots_remaining;
        if let Some(name) = binding.projectile_hide_show_bone.as_deref() {
            if !name.trim().is_empty() {
                directives.push(crate::assets::AuthoredDrawSubobjectVisibility {
                    name: name.to_string(),
                    hidden: hide_count > 0,
                });
            }
            continue;
        }
        // C++ formats an absent/empty `WeaponLaunchBone` as `01`, `02`, …;
        // retain that exact result instead of adding a Rust-only source gate.
        // The HLOD resolver still applies it only to an exact retained child.
        let launch_bone_base = binding.launch_bone_base.as_deref().unwrap_or_default();
        for projectile_index in 0..status.max_shots {
            let ordinal = projectile_index + 1;
            directives.push(crate::assets::AuthoredDrawSubobjectVisibility {
                name: format!("{launch_bone_base}{ordinal:02}"),
                hidden: ordinal <= hide_count,
            });
        }
    }
    directives
}

/// C++ `W3DSupplyDraw::updateDrawModuleSupplyStatus` crate-bone hide.
fn authored_supply_crate_subobject_visibility(
    draw_model: &crate::assets::AuthoredDrawModel,
    dock_kind: crate::game_logic::DockKind,
    current_boxes: u32,
    max_boxes: u32,
) -> Vec<crate::assets::AuthoredDrawSubobjectVisibility> {
    if dock_kind != crate::game_logic::DockKind::SupplyWarehouse || max_boxes == 0 {
        return Vec::new();
    }
    let names: Vec<&str> = draw_model
        .subobject_visibility
        .iter()
        .map(|d| d.name.as_str())
        .collect();
    crate::game_logic::host_supply_gather::supply_draw_hide_directives(
        crate::game_logic::host_supply_gather::DEFAULT_SUPPLY_BONE_PREFIX,
        &names,
        current_boxes,
        max_boxes,
    )
    .into_iter()
    .map(|(name, hidden)| crate::assets::AuthoredDrawSubobjectVisibility { name, hidden })
    .collect()
}

/// Snapshot-owned unit mesh/position/selection/FOW input for the main unit render pass.
///
/// Built only from `PresentationFrame` — no live `GameLogic` or shroud borrow.
/// W3D asset resolve uses `assets::mesh_asset_resolve` from `model_key`
/// (see OWNERSHIP residual notes — fail-closed vs full material/animation parity).
#[derive(Debug, Clone, PartialEq)]
pub struct UnitRenderInput {
    pub id: ObjectId,
    pub template_name: String,
    /// Primary model retained for backwards-compatible callers. The WGPU mesh
    /// pass renders every entry in `draw_models`, not just this first key.
    pub model_key: String,
    /// Exact selected models for all source-authored W3D Draw modules. Source
    /// order and module identity are preserved; equal basenames are not merged.
    pub draw_models: Vec<crate::assets::AuthoredDrawModel>,
    /// Frozen C++ `Drawable::updateDrawableClipStatus` payloads.  They retain
    /// concrete WeaponSet slot identity rather than collapsing to the active
    /// weapon, because C++ broadcasts every slot to every Draw module.
    pub projectile_clip_statuses: [Option<PresentationProjectileClipStatus>; 3],
    /// Mesh scale residual frozen from presentation (default 1.0).
    pub mesh_scale: f32,
    pub team: Team,
    pub team_color: [f32; 4],
    pub position: Vec3,
    pub orientation: f32,
    /// C++ ToppleUpdate lean residual for mesh tilt.
    pub topple_lean_radians: f32,
    /// C++ `m_toppleDirection.x` (host X).
    pub topple_dir_x: f32,
    /// C++ `m_toppleDirection.y` (host Z).
    pub topple_dir_y: f32,
    /// C++ `Drawable::setShadowsEnabled` residual (false while toppling).
    pub shadows_enabled: bool,
    /// C++ `Drawable::m_terrainDecalType` residual.
    pub terrain_decal_type: u8,
    /// C++ `Drawable::setTerrainDecalSize` residual (major axis).
    pub terrain_decal_size: f32,
    /// C++ `Drawable::m_decalOpacity` after fade LERP.
    pub terrain_decal_opacity: f32,
    /// Frozen host primary-turret yaw (degrees). The active collector applies
    /// it only to an exact source-authored HLOD `Turret` pivot; this field
    /// must never alter the hull world matrix.
    pub turret_angle_deg: f32,
    /// Frozen host primary-turret pitch (degrees), consumed only by an exact
    /// source-authored HLOD `TurretPitch` pivot.
    pub turret_pitch_deg: f32,
    pub selected: bool,
    pub selection_radius: f32,
    /// C++ Drawable selection flash envelope residual frames remaining.
    pub selection_flash_remaining: u32,
    /// C++ `flashAsSelected(&color)` envelope RGB. `None` is white default.
    pub selection_flash_color: Option<[f32; 3]>,

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
    /// C++ `GeometryInfo::getMaxHeightAbovePosition` frozen for construction sink.
    pub max_height_above_position: f32,

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
    /// C++ `TheKey_objectWeather`: 0 follow `world_is_snow`, 1 force clear, 2 force set.
    pub object_weather: i32,

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
    /// Frozen Drawable StealthLook opacity.  This is deliberately independent
    /// from FOW alpha: C++ friendly stealth uses alpha blending while FOW
    /// remains fully visible for the allied viewer.
    pub presentation_opacity: f32,
    /// C++ `Drawable::m_secondMaterialPassOpacity` heat-vision overlay.
    pub second_material_pass_opacity: f32,
    /// Frozen C++ Drawable tint-status RGB (signed additive; 0 = none).
    pub status_tint: [f32; 3],
    /// Frozen stored supply boxes/cash used to stamp MODELCONDITION_CARRYING.
    pub stored_supplies: u32,
    /// Frozen warehouse crate-box count for W3DSupplyDraw hide.
    pub drawable_supply_boxes: u32,
    /// Frozen warehouse startingBoxes for W3DSupplyDraw hide.
    pub drawable_supply_max_boxes: u32,
    /// Frozen dock kind so crate hide only runs on warehouses.
    pub dock_kind: crate::game_logic::DockKind,
    /// Frozen direct-object C++ shroud facts.  Renderer code receives this
    /// owned input and must not derive an ordinal status from FOW alpha.
    pub drawable_shroud: PresentationDrawableShroudFacts,
    /// C++ SubObjectsUpgrade show/hide residual (Bombload / BombWing).
    /// Applied after INI ConditionState so disguise-reveal restore wins last.
    pub sub_object_visibility: crate::game_logic::host_sub_objects_upgrade::HostSubObjectVisibility,
}

impl UnitRenderInput {
    pub fn from_renderable(ro: &RenderableObject) -> Self {
        // `None` is an intentional fail-closed result from the retained
        // Object INI ConditionState table (for example `Model = None` or an
        // unsupported source token). Do not turn it back into a bare template
        // name here; that would render a pristine proxy instead. Older saved
        // presentation frames only carry `model_key`, so synthesize one module
        // for that back-compatible representation.
        let mut draw_models = ro.draw_models.clone();
        if draw_models.is_empty() {
            if let Some(model_key) = ro.model_key.as_deref().filter(|key| !key.trim().is_empty()) {
                draw_models.push(crate::assets::AuthoredDrawModel {
                    module_index: 0,
                    model_key: model_key.to_string(),
                    ..Default::default()
                });
            }
        }
        let model_key = draw_models
            .first()
            .map(|model| model.model_key.clone())
            .unwrap_or_default();
        Self {
            id: ro.id,
            template_name: ro.template_name.clone(),
            model_key,
            draw_models,
            projectile_clip_statuses: ro.projectile_clip_statuses,
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
            topple_dir_x: ro.topple_dir_x,
            topple_dir_y: ro.topple_dir_y,
            shadows_enabled: ro.shadows_enabled,
            terrain_decal_type: ro.terrain_decal_type,
            terrain_decal_size: ro.terrain_decal_size,
            terrain_decal_opacity: ro.terrain_decal_opacity,
            turret_angle_deg: ro.turret_angle_deg,
            turret_pitch_deg: ro.turret_pitch_deg,
            selection_flash_color: ro.selection_flash_color,

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
            max_height_above_position: ro.max_height_above_position,

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
            object_weather: ro.object_weather,
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
            presentation_opacity: 1.0,
            second_material_pass_opacity: 0.0,
            status_tint: crate::game_logic::sample_drawable_status_tint(
                ro.id.0,
                0,
                crate::game_logic::drawable_disabled_dark_tint(
                    ro.disabled_emp,
                    ro.disabled_hacked,
                    ro.disabled_paralyzed,
                    ro.disabled_underpowered,
                    ro.disabled_freefall,
                    ro.disabled_subdued,
                    ro.disabled_default,
                    ro.disabled_script_underpowered,
                ),
                ro.gaining_subdual,
                ro.weapon_bonus_frenzy,
                matches!(ro.object_type, PresentationObjectType::Infantry),
            ),
            stored_supplies: ro.stored_supplies,
            drawable_supply_boxes: if ro.dock_kind == crate::game_logic::DockKind::SupplyWarehouse {
                ro.drawable_supply_boxes
            } else {
                0
            },
            drawable_supply_max_boxes: if ro.dock_kind
                == crate::game_logic::DockKind::SupplyWarehouse
            {
                ro.drawable_supply_max_boxes
            } else {
                0
            },
            dock_kind: ro.dock_kind,
            drawable_shroud: ro.drawable_shroud,
            sub_object_visibility: ro.sub_object_visibility.clone(),
        }
    }

    /// Construct the main mesh-pass input after the frame-level environment
    /// channels have been frozen.  Keeping selection inside this constructor
    /// makes selected Draw models opaque to presentation queries; they only
    /// replace them for an actual disguise.
    pub fn from_renderable_with_environment(
        ro: &RenderableObject,
        world_is_snow: bool,
        world_is_night: bool,
        logic_frame: u32,
    ) -> Self {
        let mut input = Self::from_renderable(ro);
        input.world_is_snow = world_is_snow;
        input.world_is_night = world_is_night;
        input.logic_frame = logic_frame;
        input.status_tint = crate::game_logic::sample_drawable_status_tint(
            ro.id.0,
            logic_frame,
            crate::game_logic::drawable_disabled_dark_tint(
                ro.disabled_emp,
                ro.disabled_hacked,
                ro.disabled_paralyzed,
                ro.disabled_underpowered,
                ro.disabled_freefall,
                ro.disabled_subdued,
                ro.disabled_default,
                ro.disabled_script_underpowered,
            ),
            ro.gaining_subdual,
            ro.weapon_bonus_frenzy,
            matches!(ro.object_type, PresentationObjectType::Infantry),
        );
        input.resolve_draw_models_for_frozen_conditions();
        input
    }

    /// Apply the retained Object INI Draw-state selector after every frozen
    /// presentation condition has been stamped. This is deliberately before
    /// the query-layer disguise override: an actual disguise owns its Object
    /// identity, while ordinary inputs keep opaque exact W3D module keys for
    /// the renderer.
    pub(crate) fn resolve_draw_models_for_frozen_conditions(&mut self) {
        let fallback_draw_models = self.draw_models.clone();
        let dest = crate::assets::resolve_presentation_draw_models_for_conditions(
            &self.template_name,
            &fallback_draw_models,
            self.model_condition_bits_with_combat_flags(),
        );
        self.draw_models = crate::assets::apply_live_draw_transition_playback_for_object(
            self.id.0,
            &self.template_name,
            dest,
        );
        self.model_key = self
            .draw_models
            .first()
            .map(|model| model.model_key.clone())
            .unwrap_or_default();
    }

    /// Resolve C++ W3DModelDraw child visibility for exactly one selected
    /// Draw module.
    ///
    /// `Drawable::setModelConditionState` applies the selected state's static
    /// Show/Hide vector, then `Object::adjustModelConditionForWeaponStatus`
    /// broadcasts clip status in PRIMARY/SECONDARY/TERTIARY order.  Appending
    /// the dynamic directives after the static vector preserves that actual
    /// invocation order for the existing exact-HLOD last-write resolver.
    pub(crate) fn authored_subobject_visibility_for_draw_model(
        &self,
        draw_model: &crate::assets::AuthoredDrawModel,
    ) -> Vec<crate::assets::AuthoredDrawSubobjectVisibility> {
        let mut directives = draw_model.subobject_visibility.clone();
        directives.extend(authored_projectile_clip_subobject_visibility(
            draw_model,
            &self.projectile_clip_statuses,
        ));
        directives.extend(authored_supply_crate_subobject_visibility(
            draw_model,
            self.dock_kind,
            self.drawable_supply_boxes,
            self.drawable_supply_max_boxes,
        ));
        // C++ SubObjectsUpgrade::upgradeImplementation / forceRefresh runs
        // after ConditionState hide/show. Skip while disguised: the live
        // visual is the replacement template, which has no Bombload children.
        if !self.disguised {
            for name in &self.sub_object_visibility.hidden {
                directives.push(crate::assets::AuthoredDrawSubobjectVisibility {
                    name: name.clone(),
                    hidden: true,
                });
            }
            for name in &self.sub_object_visibility.shown {
                directives.push(crate::assets::AuthoredDrawSubobjectVisibility {
                    name: name.clone(),
                    hidden: false,
                });
            }
        }
        directives
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
        // C++ applies turret yaw/pitch through ModelConditionInfo's exact
        // HTree bones after it has positioned the Drawable. Do not fold those
        // gameplay residuals into this hull matrix: a missing/unsupported
        // HLOD binding must keep the chassis orientation intact.
        // C++ ToppleUpdate::update In_Place_Pre_Rotate_X(-vel * dir.y) +
        // In_Place_Pre_Rotate_Y(vel * dir.x). Host Y-up: C++ Y → host Z.
        let mut dx = if self.topple_dir_x.is_finite() {
            self.topple_dir_x
        } else {
            0.0
        };
        let mut dy = if self.topple_dir_y.is_finite() {
            self.topple_dir_y
        } else {
            0.0
        };
        let dir_len = (dx * dx + dy * dy).sqrt();
        if dir_len > 1e-6 {
            dx /= dir_len;
            dy /= dir_len;
        } else {
            dx = 1.0;
            dy = 0.0;
        }
        let fall = if lean.abs() > 1e-8 {
            glam::Mat4::from_rotation_x(-lean * dy) * glam::Mat4::from_rotation_z(lean * dx)
        } else {
            glam::Mat4::IDENTITY
        };
        let (fy, fp) = crate::game_logic::host_float_update::sway_for(self.id.0);
        // C++ FloatUpdate: Rotate_Z(heading); Rotate_Y(yaw); Rotate_X(pitch).
        // Host Y-up: heading is already Ry; C++ Ry → host Rz; Rx stays Rx.
        let sway = if fy.abs() > 1e-8 || fp.abs() > 1e-8 {
            glam::Mat4::from_rotation_z(fy) * glam::Mat4::from_rotation_x(fp)
        } else {
            glam::Mat4::IDENTITY
        };
        let mut base = glam::Mat4::from_translation(self.position)
            * glam::Mat4::from_rotation_y(self.orientation)
            * sway
            * fall
            * glam::Mat4::from_scale(glam::Vec3::splat(scale));
        // C++ W3DModelDraw.cpp:2000-2012 Translate_Z after instance scale.
        // Host Y-up: local Y is object up after heading.
        if crate::assets::authored_draw_adjusts_height_by_construction(&self.draw_models) {
            let cpp_percent = if self.under_construction {
                self.construction_percent.clamp(0.0, 1.0) * 100.0
            } else {
                -1.0
            };
            if let Some(dz) = crate::assets::construction_percent_height_delta(
                cpp_percent,
                self.max_height_above_position,
            ) {
                base *= glam::Mat4::from_translation(glam::Vec3::new(0.0, dz, 0.0));
            }
        }
        #[cfg(feature = "game_client")]
        {
            physics_visual_host::apply_to_world_matrix(self.id, base)
        }
        #[cfg(not(feature = "game_client"))]
        {
            base
        }
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
        // hq-tjodx: only the reserved hangar is driven by production_door_phase;
        // other hangars keep pose-authored DOOR_N bits (ProductionUpdate m_doors[]).
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
            // Door 1 follows the legacy shared phase. Doors 2-4 keep pose bits.
            let (open_b, wait_b, wait_close_b, close_b) = banks[0];
            bits &= !(1u128 << open_b);
            bits &= !(1u128 << wait_b);
            bits &= !(1u128 << wait_close_b);
            bits &= !(1u128 << close_b);
            match self.production_door_phase {
                1 => bits |= 1u128 << open_b,
                2 => bits |= 1u128 << wait_b,
                // C++ ProductionUpdate never plays WAITING_TO_CLOSE.
                3 | 4 => bits |= 1u128 << close_b,
                _ => {}
            }
            let _ = banks;
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
            // C++ JetAIUpdate::update owns JETEXHAUST (velocity>0 && ALLOW_AIR_LOCO).
            // Do not invent exhaust from ground taxi or zero-velocity hover.
            if !matches!(self.object_type, PresentationObjectType::Aircraft) {
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
        // Wave 519: exploded flail/bounce residual bits.
        {
            use crate::game_logic::host_enum_table_residual::{
                exploded_bouncing_model_bit, exploded_flailing_model_bit, jetafterburner_model_bit,
                splatted_model_bit,
            };
            let flail_b = exploded_flailing_model_bit();
            let bounce_b = exploded_bouncing_model_bit();
            let splat_b = splatted_model_bit();
            let jet_ab = jetafterburner_model_bit();
            for b in [flail_b, bounce_b, splat_b] {
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
            // Keep live JETAFTERBURNER from PauseBeforeTakeoff; crash still forces it.
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
                HostBodyDamageType, disguised_model_bit, host_apply_body_damage_model_bits,
                stunned_model_bit,
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
            let snow = crate::game_logic::script_loader::resolve_object_weather_snow(
                self.object_weather,
                self.world_is_snow,
            );
            if snow {
                bits |= 1u128 << snow_b;
            } else {
                bits &= !(1u128 << snow_b);
            }
        }
        // Wave 510: captured / loaded transport residual bits.
        {
            use crate::game_logic::host_enum_table_residual::{
                captured_model_bit, loaded_model_bit,
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
            // `PowerPlantUpdate::extendRods` owns POWER_PLANT_UPGRADING and
            // POWER_PLANT_UPGRADED.  Those exact bits are already frozen in
            // `model_condition_bits`; do not infer them merely from an active
            // OverchargeBehavior (which C++ permits without that interface).
        }
        // Wave 511: burned/aflame death pose + special cheering + carrying residual.
        {
            use crate::game_logic::host_enum_table_residual::{
                aflame_model_bit, burned_model_bit, carrying_model_bit, special_cheering_model_bit,
            };
            // C++ FlammableUpdate owns AFLAME / SMOLDERING / BURNED on the live
            // object. Host `refresh_model_condition_bits` already stamped them.
            // Do not rewrite from death_type_name (alive burners have an empty
            // name; death.contains("burn") is unrelated to tryToIgnite).
            let _death = self.death_type_name.to_ascii_lowercase();
            let _burn_b = burned_model_bit();
            let _flame_b = aflame_model_bit();
            let _smolder_b = {
                use crate::game_logic::host_enum_table_residual::smoldering_model_bit;
                smoldering_model_bit()
            };
            // Wave 524: SMOLDERING when burned residual without active flame.
            // Host bits stay as-is; death.contains("smolder") is not a stamp.
            let _ = _death;
            let _ = (_burn_b, _flame_b, _smolder_b);
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
            // C++ W3DModelDraw::updateDrawModuleSupplyStatus — boxes > 0.
            // Infantry ability residual remains for flag/crate poses without supplies.
            if self.stored_supplies > 0
                || (self.using_ability && infantry && !self.attacking && !self.is_firing_weapon)
            {
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

    /// C++ `flashAsSelected(&color)` RGB, or white when unspecified.
    #[inline]
    pub fn selection_flash_color_rgba(&self) -> [f32; 4] {
        match self.selection_flash_color {
            Some(c) => [c[0], c[1], c[2], 1.0],
            None => [1.0, 1.0, 1.0, 1.0],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projectile_clip_visibility_uses_cxx_slot_order_override_and_last_write_order() {
        let mut bindings = crate::assets::AuthoredDrawWeaponBoneBindings::default();
        bindings.slots[0].launch_bone_base = Some("unusedlaunch".to_string());
        bindings.slots[0].projectile_hide_show_bone = Some("missile".to_string());
        bindings.slots[1].launch_bone_base = Some("rack".to_string());
        let draw_model = crate::assets::AuthoredDrawModel {
            subobject_visibility: vec![crate::assets::AuthoredDrawSubobjectVisibility {
                name: "missile".to_string(),
                hidden: false,
            }],
            weapon_bone_bindings: bindings,
            projectile_bone_feedback: crate::assets::AuthoredDrawProjectileBoneFeedback {
                enabled_slots: 0b011,
                source_fields_valid: true,
            },
            ..Default::default()
        };
        let statuses = [
            Some(PresentationProjectileClipStatus {
                shots_remaining: 2,
                max_shots: 3,
            }),
            Some(PresentationProjectileClipStatus {
                shots_remaining: 2,
                max_shots: 3,
            }),
            None,
        ];

        let dynamic = authored_projectile_clip_subobject_visibility(&draw_model, &statuses);
        assert_eq!(
            dynamic,
            vec![
                crate::assets::AuthoredDrawSubobjectVisibility {
                    name: "missile".to_string(),
                    hidden: true,
                },
                crate::assets::AuthoredDrawSubobjectVisibility {
                    name: "rack01".to_string(),
                    hidden: true,
                },
                crate::assets::AuthoredDrawSubobjectVisibility {
                    name: "rack02".to_string(),
                    hidden: false,
                },
                crate::assets::AuthoredDrawSubobjectVisibility {
                    name: "rack03".to_string(),
                    hidden: false,
                },
            ],
            "C++ sends PRIMARY first, emits one direct HideShow override, then emits SECONDARY numbered children"
        );
        assert!(
            dynamic
                .iter()
                .all(|directive| directive.name != "missile01"),
            "WeaponHideShowBone is an exact C++ child name rather than a numbered launch base"
        );

        let mut combined = draw_model.subobject_visibility.clone();
        combined.extend(dynamic);
        assert_eq!(
            combined[0].hidden, false,
            "the selected condition state's static ShowSubObject remains first"
        );
        assert!(
            combined[1].hidden,
            "the later clip callback wins for the same exact HLOD child"
        );
    }

    #[test]
    fn projectile_clip_visibility_handles_invalid_counts_and_exact_generated_names() {
        let mut bindings = crate::assets::AuthoredDrawWeaponBoneBindings::default();
        bindings.slots[0].launch_bone_base = Some("rack".to_string());
        let valid_draw = crate::assets::AuthoredDrawModel {
            weapon_bone_bindings: bindings.clone(),
            projectile_bone_feedback: crate::assets::AuthoredDrawProjectileBoneFeedback {
                enabled_slots: 1,
                source_fields_valid: true,
            },
            ..Default::default()
        };
        let malformed_count = [
            Some(PresentationProjectileClipStatus {
                shots_remaining: 3,
                max_shots: 2,
            }),
            None,
            None,
        ];
        assert!(
            authored_projectile_clip_subobject_visibility(&valid_draw, &malformed_count).is_empty(),
            "C++ rejects showCount above maxCount instead of selecting an arbitrary child"
        );
        let large_count = [
            Some(PresentationProjectileClipStatus {
                shots_remaining: 99,
                max_shots: 100,
            }),
            None,
            None,
        ];
        let large_directives =
            authored_projectile_clip_subobject_visibility(&valid_draw, &large_count);
        assert_eq!(large_directives.len(), 100);
        assert_eq!(large_directives[0].name, "rack01");
        assert!(large_directives[0].hidden);
        assert_eq!(large_directives[1].name, "rack02");
        assert!(!large_directives[1].hidden);
        assert_eq!(large_directives[99].name, "rack100");
        assert!(
            !large_directives[99].hidden,
            "C++ `%02d` has a minimum width, so clip feedback must retain a 100th exact child"
        );

        let empty_launch_directives = authored_projectile_clip_subobject_visibility(
            &crate::assets::AuthoredDrawModel {
                projectile_bone_feedback: crate::assets::AuthoredDrawProjectileBoneFeedback {
                    enabled_slots: 1,
                    source_fields_valid: true,
                },
                ..Default::default()
            },
            &[
                Some(PresentationProjectileClipStatus {
                    shots_remaining: 1,
                    max_shots: 2,
                }),
                None,
                None,
            ],
        );
        assert_eq!(
            empty_launch_directives,
            vec![
                crate::assets::AuthoredDrawSubobjectVisibility {
                    name: "01".to_string(),
                    hidden: true,
                },
                crate::assets::AuthoredDrawSubobjectVisibility {
                    name: "02".to_string(),
                    hidden: false,
                },
            ],
            "C++ formats an empty launch base rather than suppressing enabled feedback"
        );

        let direct_override = crate::assets::AuthoredDrawModel {
            weapon_bone_bindings: crate::assets::AuthoredDrawWeaponBoneBindings {
                slots: [
                    crate::assets::AuthoredDrawWeaponBoneSlot {
                        projectile_hide_show_bone: Some("missile".to_string()),
                        ..Default::default()
                    },
                    crate::assets::AuthoredDrawWeaponBoneSlot::default(),
                    crate::assets::AuthoredDrawWeaponBoneSlot::default(),
                ],
                source_fields_valid: true,
            },
            projectile_bone_feedback: crate::assets::AuthoredDrawProjectileBoneFeedback {
                enabled_slots: 1,
                source_fields_valid: true,
            },
            ..Default::default()
        };
        assert_eq!(
            authored_projectile_clip_subobject_visibility(&direct_override, &large_count),
            vec![crate::assets::AuthoredDrawSubobjectVisibility {
                name: "missile".to_string(),
                hidden: true,
            }],
            "C++ WeaponHideShowBone emits its one exact child without numbered topology"
        );

        let invalid_draw = crate::assets::AuthoredDrawModel {
            weapon_bone_bindings: bindings,
            projectile_bone_feedback: crate::assets::AuthoredDrawProjectileBoneFeedback {
                enabled_slots: 1,
                source_fields_valid: false,
            },
            ..Default::default()
        };
        let otherwise_valid = [
            Some(PresentationProjectileClipStatus {
                shots_remaining: 1,
                max_shots: 2,
            }),
            None,
            None,
        ];
        assert!(
            authored_projectile_clip_subobject_visibility(&invalid_draw, &otherwise_valid)
                .is_empty(),
            "an invalid retained module mask cannot author dynamic visibility"
        );
    }

    #[test]
    fn projectile_clip_visibility_retail_scud_hides_first_exact_launch_children() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let ini_path = [
            root.join("windows_game/extracted_big_files/INIZH/Data/INI/Object/FactionBuilding.ini"),
            root.join(
                "windows_game/extracted_big_files_v2/INIZH/Data/INI/Object/FactionBuilding.ini",
            ),
        ]
        .into_iter()
        .find(|candidate| candidate.is_file());
        let Some(ini_path) = ini_path else {
            eprintln!("skip: retail FactionBuilding.ini is not available on disk");
            return;
        };

        let source = std::fs::read_to_string(&ini_path)
            .unwrap_or_else(|error| panic!("read retail {}: {error}", ini_path.display()));
        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(&source, "FactionBuilding.ini")
            .expect("parse retail Scud Storm Draw state");
        let attacking_bit_index =
            crate::game_logic::host_enum_table_residual::model_condition_bit_name_index(
                "ATTACKING",
            )
            .expect("retail ATTACKING condition bit");
        let attacking_bits = 1u128
            .checked_shl(
                u32::try_from(attacking_bit_index).expect("ATTACKING condition bit fits u32"),
            )
            .expect("ATTACKING condition bit fits retained bank");
        let scud = parser
            .get_definition("GLAScudStorm")
            .expect("retail Scud Storm definition")
            .select_draw_models_for_conditions(attacking_bits)
            .expect("retail attacking Scud Storm state")
            .into_iter()
            .find(|draw| draw.model_key.eq_ignore_ascii_case("UBScudStrm_A2"))
            .expect("retail attacking Scud Storm model");
        let directives = authored_projectile_clip_subobject_visibility(
            &scud,
            &[
                Some(PresentationProjectileClipStatus {
                    shots_remaining: 6,
                    max_shots: 9,
                }),
                None,
                None,
            ],
        );

        assert_eq!(directives.len(), 9);
        for (index, directive) in directives.iter().enumerate() {
            let ordinal = index + 1;
            assert_eq!(directive.name, format!("weapona{ordinal:02}"));
            assert_eq!(
                directive.hidden,
                ordinal <= 3,
                "C++ hides the first max-minus-remaining Scud missiles"
            );
        }
    }

    #[test]
    fn carrying_bit_stamps_from_stored_supplies() {
        use crate::game_logic::{GameLogic, Player, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        logic.add_player(Player::new(0, Team::USA, "Local", true));
        let mut template = ThingTemplate::new("AmericaVehicleChinook");
        template.set_health(100.0);
        logic
            .templates
            .insert("AmericaVehicleChinook".into(), template);
        let id = logic
            .create_object(
                "AmericaVehicleChinook",
                Team::USA,
                glam::Vec3::new(1.0, 0.0, 1.0),
            )
            .expect("chinook");
        logic
            .host_object_mut(id)
            .expect("chinook obj")
            .set_stored_supplies(300);
        let frame = crate::presentation_frame::PresentationFrame::build_from_logic(&logic, 0);
        let input = frame
            .unit_render_inputs()
            .into_iter()
            .find(|u| u.id == id)
            .expect("chinook input");
        assert!(input.stored_supplies > 0);
        let carry = 1u128 << crate::game_logic::host_enum_table_residual::carrying_model_bit();
        assert_ne!(
            input.model_condition_bits_with_combat_flags() & carry,
            0,
            "C++ CARRYING while stored supplies > 0"
        );
    }

    #[test]
    fn warehouse_crate_bones_hide_from_drawable_supply_status() {
        use crate::game_logic::{DockKind, GameLogic, Player, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        logic.add_player(Player::new(0, Team::Neutral, "N", false));
        let mut warehouse = ThingTemplate::new("SupplyWarehouse");
        warehouse.dock_kind = DockKind::SupplyWarehouse;
        warehouse.dock_starting_boxes = Some(10);
        warehouse.set_health(1000.0);
        logic.templates.insert(warehouse.name.clone(), warehouse);
        let id = logic
            .create_object("SupplyWarehouse", Team::Neutral, glam::Vec3::ZERO)
            .expect("warehouse");
        logic
            .host_object_mut(id)
            .expect("wh")
            .set_stored_supplies(5 * 75);
        let frame = crate::presentation_frame::PresentationFrame::build_from_logic(&logic, 0);
        let input = frame
            .unit_render_inputs()
            .into_iter()
            .find(|u| u.id == id)
            .expect("warehouse input");
        assert_eq!(input.drawable_supply_boxes, 5);
        assert_eq!(input.drawable_supply_max_boxes, 10);
        let draw_model = crate::assets::AuthoredDrawModel::default();
        let dirs = input.authored_subobject_visibility_for_draw_model(&draw_model);
        let hidden = dirs
            .iter()
            .filter(|d| d.name.starts_with("SupplyBox") && d.hidden)
            .count();
        assert!(
            hidden >= 4,
            "half stock must hide later crate bones, hidden={hidden}"
        );
        assert!(dirs.iter().any(|d| d.name == "SupplyBox05" && d.hidden));
        assert!(dirs.iter().any(|d| d.name == "SupplyBox04" && !d.hidden));
    }

    #[test]
    fn status_tint_gate_skips_unmanned_and_tints_underpowered() {
        use crate::game_logic::{
            GameLogic, Player, Team, ThingTemplate, reset_drawable_tint_envelopes,
        };
        reset_drawable_tint_envelopes();
        let mut logic = GameLogic::new();
        logic.add_player(Player::new(0, Team::USA, "Local", true));
        let mut template = ThingTemplate::new("AmericaTankCrusader");
        template.set_health(100.0);
        logic
            .templates
            .insert("AmericaTankCrusader".into(), template);
        let id = logic
            .create_object(
                "AmericaTankCrusader",
                Team::USA,
                glam::Vec3::new(2.0, 0.0, 2.0),
            )
            .expect("tank");

        {
            let o = logic.host_object_mut(id).expect("tank obj");
            o.status.disabled_unmanned = true;
        }
        let unmanned = crate::presentation_frame::PresentationFrame::build_from_logic(&logic, 0)
            .unit_render_inputs()
            .into_iter()
            .find(|u| u.id == id)
            .expect("unmanned input");
        assert_eq!(unmanned.status_tint, [0.0, 0.0, 0.0]);

        reset_drawable_tint_envelopes();
        {
            let o = logic.host_object_mut(id).expect("tank obj");
            o.status.disabled_unmanned = false;
            o.status.disabled_underpowered = true;
        }
        let underpowered =
            crate::presentation_frame::PresentationFrame::build_from_logic(&logic, 0)
                .unit_render_inputs()
                .into_iter()
                .find(|u| u.id == id)
                .expect("underpowered input");
        assert!(underpowered.status_tint[0] < -0.01);
        assert!(underpowered.status_tint[0] > -0.5);

        reset_drawable_tint_envelopes();
        {
            let o = logic.host_object_mut(id).expect("tank obj");
            o.status.disabled_underpowered = false;
            o.status.disabled_subdued = true;
        }
        let subdued = crate::presentation_frame::PresentationFrame::build_from_logic(&logic, 0)
            .unit_render_inputs()
            .into_iter()
            .find(|u| u.id == id)
            .expect("subdued input");
        assert!(
            subdued.status_tint[0] < -0.01,
            "DISABLED_SUBDUED is dark gray"
        );
        assert!(subdued.status_tint[2] < 0.0);

        reset_drawable_tint_envelopes();
        {
            let o = logic.host_object_mut(id).expect("tank obj");
            o.status.disabled_subdued = false;
            o.subdual_damage = 10.0;
        }
        let subdual = crate::presentation_frame::PresentationFrame::build_from_logic(&logic, 0)
            .unit_render_inputs()
            .into_iter()
            .find(|u| u.id == id)
            .expect("subdual input");
        assert!(subdual.status_tint[2] > 0.01);

        reset_drawable_tint_envelopes();
        {
            let o = logic.host_object_mut(id).expect("tank obj");
            o.subdual_damage = 0.0;
            o.weapon_bonus_frenzy = true;
        }
        let frenzy = crate::presentation_frame::PresentationFrame::build_from_logic(&logic, 0)
            .unit_render_inputs()
            .into_iter()
            .find(|u| u.id == id)
            .expect("frenzy input");
        assert!(frenzy.status_tint[0] > 0.0);
    }

    #[test]
    fn detected_stealth_drops_blob_shadow() {
        use crate::game_logic::host_upgrades::HostCamoStealthLook;
        use crate::game_logic::{GameLogic, Player, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        logic.add_player(Player::new(0, Team::USA, "Local", true));
        let mut template = ThingTemplate::new("GLAInfantryJarmenKell");
        template.set_health(100.0);
        logic
            .templates
            .insert("GLAInfantryJarmenKell".into(), template);
        let id = logic
            .create_object(
                "GLAInfantryJarmenKell",
                Team::GLA,
                glam::Vec3::new(4.0, 0.0, 4.0),
            )
            .expect("kell");
        {
            let o = logic.host_object_mut(id).expect("kell obj");
            o.status.stealthed = true;
            o.status.detected = true;
            o.camo_heat_vision_opacity = 1.0;
            o.camo_stealth_look = HostCamoStealthLook::VisibleDetected as u8;
        }
        let input = crate::presentation_frame::PresentationFrame::build_from_logic(&logic, 0)
            .unit_render_inputs()
            .into_iter()
            .find(|u| u.id == id)
            .expect("detected input");
        assert!(!input.shadows_enabled);
        assert!(
            (input.second_material_pass_opacity - 1.0).abs() < 1e-5,
            "detected stealthed unit must use heat-vision second pass"
        );
    }

    #[test]
    fn own_mines_freeze_zero_friendly_opacity() {
        use crate::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        logic.add_player(Player::new(0, Team::USA, "Local", true));
        let mut template = ThingTemplate::new("ChinaStandardMine");
        template.set_health(10.0);
        template.add_kind_of(KindOf::Mine);
        logic.templates.insert("ChinaStandardMine".into(), template);
        let id = logic
            .create_object(
                "ChinaStandardMine",
                Team::USA,
                glam::Vec3::new(6.0, 0.0, 6.0),
            )
            .expect("mine");
        {
            let o = logic.host_object_mut(id).expect("mine obj");
            o.mine_data = Some(crate::game_logic::host_mines::HostMineData::land_mine());
            o.apply_mine_innate_stealth();
        }
        let frame = crate::presentation_frame::PresentationFrame::build_from_logic(&logic, 0);
        let object = frame
            .objects
            .iter()
            .find(|o| o.id == id)
            .expect("mine object");
        assert!(object.friendly_stealth_opacity.abs() < 1e-5);
        let input = frame
            .unit_render_inputs()
            .into_iter()
            .find(|u| u.id == id)
            .expect("mine input");
        assert!(input.presentation_opacity.abs() < 1e-5);
    }

    #[test]
    fn sticky_and_carbomb_overlay_follow_cpp_gates() {
        use crate::game_logic::host_mines::HostMineData;
        use crate::game_logic::{GameLogic, Player, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        logic.add_player(Player::new(0, Team::USA, "Local", true));
        logic.add_player(Player::new(1, Team::GLA, "Enemy", false));
        let mut car = ThingTemplate::new("CivilianCar");
        car.set_health(100.0);
        logic.templates.insert("CivilianCar".into(), car);
        let mut charge = ThingTemplate::new("TNTStickyBomb");
        charge.set_health(10.0);
        logic.templates.insert("TNTStickyBomb".into(), charge);

        let local_car = logic
            .create_object("CivilianCar", Team::USA, glam::Vec3::new(1.0, 0.0, 1.0))
            .expect("local car");
        {
            let o = logic.host_object_mut(local_car).expect("local car obj");
            o.owner_player_id = Some(0);
            o.weapon_set_carbomb = true;
            o.status.is_carbomb = true;
        }
        let enemy_car = logic
            .create_object("CivilianCar", Team::GLA, glam::Vec3::new(2.0, 0.0, 2.0))
            .expect("enemy car");
        {
            let o = logic.host_object_mut(enemy_car).expect("enemy car obj");
            o.owner_player_id = Some(1);
            o.weapon_set_carbomb = true;
            o.status.is_carbomb = true;
        }
        let target = logic
            .create_object("CivilianCar", Team::GLA, glam::Vec3::new(3.0, 0.0, 3.0))
            .expect("target");
        let timed = logic
            .create_object("TNTStickyBomb", Team::USA, glam::Vec3::new(3.0, 0.0, 3.0))
            .expect("timed");
        {
            let o = logic.host_object_mut(timed).expect("timed obj");
            o.mine_data = Some(HostMineData::timed_demo_charge(0).with_attach(target));
        }
        let remote = logic
            .create_object("TNTStickyBomb", Team::USA, glam::Vec3::new(4.0, 0.0, 4.0))
            .expect("remote");
        {
            let o = logic.host_object_mut(remote).expect("remote obj");
            o.mine_data = Some(HostMineData::remote_demo_charge().with_attach(target));
        }

        let frame = crate::presentation_frame::PresentationFrame::build_from_logic(&logic, 0);
        let local = frame.objects.iter().find(|o| o.id == local_car).unwrap();
        let enemy = frame.objects.iter().find(|o| o.id == enemy_car).unwrap();
        let timed_o = frame.objects.iter().find(|o| o.id == timed).unwrap();
        let remote_o = frame.objects.iter().find(|o| o.id == remote).unwrap();
        assert!(local.weapon_set_carbomb);
        assert!(enemy.weapon_set_carbomb);
        assert_eq!(timed_o.bomb_type, 1);
        assert!(timed_o.bomb_timer_seconds > 0);
        assert_eq!(remote_o.bomb_type, 2);
        assert_eq!(remote_o.bomb_timer_seconds, 0);
    }
}
