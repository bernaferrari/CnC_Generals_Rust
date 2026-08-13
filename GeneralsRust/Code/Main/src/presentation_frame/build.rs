use super::*;
use crate::fow_rendering::{ProjectedShroudMetadata, ProjectedShroudSnapshot};

/// Freeze only the source facts C++ resolves for a direct Object-backed
/// Drawable.  The host path has direct shroud membership but not a drawable or
/// partition fade alpha, so it must never synthesize `PartialClear` from the
/// presentation's scalar `ObjectVisibility`.  C++'s own drawable dispatch
/// later owns the clear-frame grace and may produce its final PartialClear.
pub(super) fn freeze_direct_object_shroud_facts(
    obj: &crate::game_logic::Object,
    local_player_id: u32,
    fow_shell_bypass: bool,
) -> PresentationDrawableShroudFacts {
    let raw_status = if fow_shell_bypass || obj.owner_player_id == Some(local_player_id) {
        // The host FOW bridge deliberately preserves C++ own-force/no-partition
        // clear behavior even when the standalone manager has no membership.
        PresentationObjectShroudStatus::Clear
    } else if let Ok(shroud) = gamelogic::system::shroud_manager::get_shroud_manager().lock() {
        // Main host objects are not registered in GameLogic's object manager.
        // With no direct membership, retain the same C++ Object fallback used
        // for an object without partition data: Clear, not an invented shroud.
        let runtime_active = !shroud.get_visible_objects(local_player_id).is_empty()
            || !shroud.get_explored_objects(local_player_id).is_empty();
        if !runtime_active || shroud.can_see_object(local_player_id, obj.id.0) {
            PresentationObjectShroudStatus::Clear
        } else if shroud.has_explored_object(local_player_id, obj.id.0) {
            PresentationObjectShroudStatus::Fogged
        } else {
            PresentationObjectShroudStatus::Shrouded
        }
    } else {
        // The C++ Object fallback is Clear when it has no partition data.
        PresentationObjectShroudStatus::Clear
    };

    // `Object::is_alive` also tests HP, which is deliberately not a direct
    // Drawable residency signal: deferred death paths retain a visual Object
    // with a sliver of HP.  Freeze only the C++ effectively-dead/deferred
    // death state that the client-owned shroud grace branch consumes.
    let effectively_dead = obj.status.effectively_dead
        || obj.status.keep_as_rubble
        || obj
            .slow_death
            .as_ref()
            .is_some_and(|slow_death| slow_death.is_active())
        || obj
            .jet_slow_death
            .as_ref()
            .is_some_and(|slow_death| slow_death.is_active())
        || obj
            .helicopter_slow_death
            .as_ref()
            .is_some_and(|slow_death| slow_death.is_active())
        || obj
            .structure_topple_data
            .as_ref()
            .is_some_and(|topple| topple.is_active())
        || obj
            .structure_collapse_data
            .as_ref()
            .is_some_and(|collapse| collapse.is_active());

    PresentationDrawableShroudFacts::direct_host_object(raw_status, effectively_dead)
}

/// Freeze the source capability consumed by C++
/// `StealthUpdate::canDisguise()`. The active host's Bomb Truck residual is
/// the implemented source of `DisguisesAsTeam`; use the immutable
/// ThingTemplate identity rather than mutable Object template bookkeeping or
/// a pending/committed disguise transition.
pub(super) fn freeze_direct_can_disguise_as_team(obj: &crate::game_logic::Object) -> bool {
    crate::game_logic::host_bomb_truck_disguise::BOMB_TRUCK_DISGUISES_AS_TEAM
        && crate::game_logic::host_bomb_truck_disguise::has_disguises_as_team_stealth_residual(
            obj.thing.template.name.as_str(),
        )
}

/// C++ visual disguise is an identity selection, not a mutation of the
/// gameplay Object template.  Keep the source ThingTemplate name for ordinary
/// Objects and use the committed disguise template only once the status says
/// it is active.
fn direct_host_visual_template_name(obj: &crate::game_logic::Object) -> String {
    let actual_template_name = obj.thing.template.name.as_str();
    if obj.status.disguised {
        obj.disguise_as_template
            .as_deref()
            .filter(|template_name| !template_name.trim().is_empty())
            .unwrap_or(actual_template_name)
            .to_owned()
    } else {
        actual_template_name.to_owned()
    }
}

impl PresentationFrame {
    /// Build a snapshot by borrowing the authoritative world for this call only.
    ///
    /// FOW for `local_player_id` is frozen here via the FOW bridge so the unit mesh
    /// pass can apply alpha / never-explored skip without mid-render shroud locks.
    /// Cell-grid FOW is also frozen into `fow_grid` for terrain overlay / minimap.
    /// Fail-closed claim: unit FOW + compact local grid; not full SAGE shroud parity.
    pub fn build_from_logic(logic: &GameLogic, local_player_id: u32) -> Self {
        Self::build_from_logic_with_runtime_heightmap(logic, local_player_id, None)
    }

    /// Engine-only variant which carries the map-lifetime full terrain payload
    /// instead of cloning it again while freezing a presentation frame.
    pub(crate) fn build_from_logic_with_runtime_heightmap(
        logic: &GameLogic,
        local_player_id: u32,
        runtime_heightmap: Option<std::sync::Arc<PresentationRuntimeHeightmap>>,
    ) -> Self {
        // Shell maps render fully visible background scenes (C++ parity).
        let fow_shell_bypass = logic.isInShellGame();
        // Local force residual: always present own-team objects fully visible.
        // C++ always draws the controlling player's units; host FOW membership can
        // miss builders when sight_range / ObjectManager dual-world is incomplete.
        let mut local_team = logic
            .get_player(local_player_id)
            .map(|p| p.team)
            .unwrap_or(Team::Neutral);
        // If the requested slot is missing or Neutral, own-team objects would
        // fall through to shroud HIDDEN. Prefer the first is_local non-Neutral
        // player so builders/CCs stay FULLY_VISIBLE (FOW still applies to
        // Neutral/unexplored enemies).
        if local_team == Team::Neutral {
            if let Some(team) = logic
                .get_players()
                .values()
                .find(|p| p.is_local && p.team != Team::Neutral)
                .map(|p| p.team)
            {
                local_team = team;
            }
        }
        // Freeze team base proximity once (camera snap / host residual).
        let local_team_base_position = logic.team_base_position(local_team);
        // Freeze terrain FOW grid once for this presentation frame (local player only).
        let fow_grid = FOWRenderingBridge::snapshot_terrain_grid(local_player_id, fow_shell_bypass);
        // C++ W3DShroud copies logical cells into a padded destination texture
        // before per-object material passes sample it.  Freeze that complete
        // renderer input here, including GlobalData tint/levels, so WGPU never
        // reads live FOW or GlobalData while drawing.
        let projected_shroud = {
            // `Enable/DisableBorderShroud` can override the constructor's
            // `ShroudAlpha` border.  Read the script-display handoff only
            // while building the frame; the resulting R8 padding is immutable
            // to renderer code.  Headless/no-GameClient builds retain the
            // source constructor default through `None`.
            #[cfg(feature = "game_client")]
            let border_alpha_override =
                game_client::core::script_action_handler::script_display_border_shroud_level();
            #[cfg(not(feature = "game_client"))]
            let border_alpha_override = None;
            let global = game_engine::common::global_data::read();
            ProjectedShroudSnapshot::from_grid(
                &fow_grid,
                ProjectedShroudMetadata::from_global_data_with_border_override(
                    &global,
                    border_alpha_override,
                ),
            )
        };
        let mut objects = Vec::with_capacity(logic.host_objects().len());
        let mut direct_host_drawables = Vec::with_capacity(logic.host_objects().len());
        for obj in logic.host_objects().values() {
            // Coupled GameWorld is truth for sit-through HP / pose / dest / target /
            // ammo / occupants. HashMap poke without commit must not win.
            let auth_health = logic
                .host_authoritative_health(obj.id)
                .unwrap_or(obj.health.current);
            let auth_pose = logic.host_authoritative_pose(obj.id);
            let auth_dest = logic
                .host_authoritative_move_dest(obj.id)
                .map(glam::Vec3::from);
            let auth_target = logic.host_authoritative_target(obj.id);
            let auth_ammo = logic.host_authoritative_weapon_ammo(obj.id);
            let auth_projectile_clip_statuses =
                logic.host_authoritative_projectile_clip_statuses(obj.id);
            let projectile_clip_statuses = std::array::from_fn(|slot| {
                auth_projectile_clip_statuses
                    .get(slot)
                    .copied()
                    .flatten()
                    .map(
                        |(shots_remaining, max_shots)| PresentationProjectileClipStatus {
                            shots_remaining,
                            max_shots,
                        },
                    )
            });
            let auth_occupants = logic.host_authoritative_occupant_count(obj.id);
            let _auth_attack_sub = logic.host_authoritative_attack_substate(obj.id);
            let (auth_construction_percent, auth_under_construction) = logic
                .host_authoritative_construction(obj.id)
                .unwrap_or((obj.construction_percent, obj.status.under_construction));
            let is_structure = obj.is_kind_of(KindOf::Structure);
            let is_unit = obj.is_kind_of(KindOf::Infantry)
                || obj.is_kind_of(KindOf::Vehicle)
                || obj.is_kind_of(KindOf::Aircraft);
            // Prefer the explicitly authored template model only when the
            // loaded Object INI has no retained ConditionState table.  A
            // source-authored Draw state below takes precedence; the render
            // collector must receive that exact W3D key untouched.
            let base_model_key =
                crate::assets::mesh_asset_resolve::model_key_from_template(obj.get_template());
            let destroyed_for_mesh = obj.status.destroyed || auth_health <= 0.01;
            let body_ord = {
                use crate::game_logic::host_enum_table_residual::{
                    host_calc_body_damage_state, HostBodyDamageType,
                };
                let state = if destroyed_for_mesh {
                    HostBodyDamageType::Rubble
                } else {
                    host_calc_body_damage_state(auth_health, obj.health.maximum.max(0.0))
                };
                state as u8
            };
            let model_condition_bits = {
                // Preserve the same frozen condition bank used by the mesh
                // pass.  C++ W3DModelDraw selects an authored ConditionState
                // from these bits, rather than forming a basename suffix.
                let mut bits = obj.model_condition_bits;
                use crate::game_logic::host_enum_table_residual::{
                    host_apply_body_damage_model_bits, host_calc_body_damage_state,
                    HostBodyDamageType, MC_BIT_ATTACKING, MC_BIT_DYING, MC_BIT_MOVING,
                };
                use crate::game_logic::host_neutron_missile_slow_death::{
                    MC_BIT_BACKCRUSHED, MC_BIT_FRONTCRUSHED,
                };
                let state = if destroyed_for_mesh {
                    HostBodyDamageType::Rubble
                } else {
                    host_calc_body_damage_state(auth_health, obj.health.maximum.max(0.0))
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
                if destroyed_for_mesh {
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
            };
            let fallback_draw_models =
                (!base_model_key.trim().is_empty()).then(|| crate::assets::AuthoredDrawModel {
                    module_index: 0,
                    model_key: base_model_key.clone(),
                    ..Default::default()
                });
            let draw_models = crate::assets::resolve_presentation_draw_models_for_conditions(
                &obj.template_name,
                fallback_draw_models.as_slice(),
                model_condition_bits,
            );
            let model_key = draw_models.first().map(|model| model.model_key.clone());
            // Wave 75: freeze mesh scale residual (common combat = 1.0; CINE/weapon peels).
            let mesh_scale =
                crate::assets::mesh_asset_resolve::mesh_scale_from_template(obj.get_template());
            let fow_visibility = if fow_shell_bypass {
                ObjectVisibility::FULLY_VISIBLE
            } else if obj.owner_player_id == Some(local_player_id) {
                // Always see own force (structures + builders + army).
                ObjectVisibility::FULLY_VISIBLE
            } else {
                FOWRenderingBridge::get_object_visibility(local_player_id, obj.id)
            };
            let drawable_shroud =
                freeze_direct_object_shroud_facts(obj, local_player_id, fow_shell_bypass);
            let visual_template_name = direct_host_visual_template_name(obj);
            // Wave 77: freeze ground-height residual at object XY (sample or default-0).
            let pos = obj.get_position();
            let (ground_height, ground_height_from_terrain) =
                sample_presentation_ground_height(logic, pos);
            // Freeze the exact source module identity used by the selected
            // special-power command.  The UI must not rediscover this from a
            // structure basename after the logic snapshot has been taken.
            let ready_structure_special_power_module = obj
                .thing
                .template
                .special_power_modules
                .iter()
                .find(|module| {
                    module.command_power.as_ref().is_some_and(|power| {
                        matches!(
                            power,
                            &crate::command_system::SpecialPowerType::ParticleCannon
                                | &crate::command_system::SpecialPowerType::SuperweaponParticleCannon
                                | &crate::command_system::SpecialPowerType::LaserCannon
                                | &crate::command_system::SpecialPowerType::ScudStorm
                                | &crate::command_system::SpecialPowerType::NuclearMissile
                                | &crate::command_system::SpecialPowerType::NukeNeutronMissile
                                | &crate::command_system::SpecialPowerType::SuperweaponNeutronMissile
                        ) && logic.is_special_power_ready_for(obj.id, power)
                    })
                });
            let renderable = RenderableObject {
                id: obj.id,
                template_name: obj.template_name.clone(),
                team: obj.team,
                owner_player_id: obj.owner_player_id,
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
                    let mut p = auth_pose
                        .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                        .unwrap_or(pos);
                    p.y += obj.presentation_collapse_height_offset();
                    p.y += obj.presentation_slow_death_sink_offset();
                    let (sx, sz) = obj.presentation_collapse_shudder();
                    p.x += sx;
                    p.z += sz;
                    p
                },
                orientation: obj.get_orientation(),
                topple_lean_radians: obj.presentation_topple_lean_radians(),
                move_destination: auth_dest,
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
                attack_target: auth_target,
                path_waypoints: obj.movement.path.iter().copied().take(16).collect(),
                path_len: obj.movement.path.len().min(u16::MAX as usize) as u16,
                path_index: obj.movement.current_path_index.min(u16::MAX as usize) as u16,
                occupant_count: auth_occupants
                    .unwrap_or(obj.occupants.len().min(u16::MAX as usize) as u16),
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
                // Freeze every containment roster, not only structure
                // garrisons.  TransportContain capacity is weighted by each
                // rider's authored TransportSlotCount, so physical RMB input
                // must be able to resolve mobile occupants without a live
                // GameLogic read.
                garrisoned_units: obj.contained_units().into_iter().take(32).collect(),
                max_garrison: obj
                    .building_data
                    .as_ref()
                    .map(|b| b.max_garrison)
                    .unwrap_or(0),
                power_provided: obj.power_provided,
                power_consumed: obj.power_consumed,
                stored_supplies: obj.stored_resources.supplies,
                dock_kind: obj.thing.template.dock_kind,
                capturable: obj.thing.template.capturable,
                immune_to_capture: obj.thing.template.immune_to_capture,
                capture_garrisonable: obj.thing.template.garrison_contain_max.is_some(),
                capture_power: obj.thing.template.capture_power,
                capture_power_ready: obj
                    .thing
                    .template
                    .capture_power
                    .special_power_type()
                    .is_some_and(|power| logic.is_special_power_ready_for(obj.id, &power)),
                hacker_disable_building_capable: obj
                    .thing
                    .template
                    .hacker_disable_building
                    .is_some(),
                hacker_disable_building_ready: logic.is_hacker_disable_building_ready(obj.id),
                special_power_ready_template_name: ready_structure_special_power_module
                    .map(|module| module.special_power_template.clone()),
                special_power_ready_template_id: ready_structure_special_power_module
                    .map(|module| module.special_power_template_id),
                health_current: auth_health,
                health_max: obj.health.maximum,
                selected: obj.selected || obj.status.selected,
                is_deployed: obj.status.deployed,
                selection_flash_remaining: obj.selection_flash_remaining,
                destroyed: obj.status.destroyed || auth_health <= 0.01,
                model_condition_bits,
                radar_active: obj.radar_active,
                radar_extend_complete: obj.radar_extend_complete,
                production_door_phase: obj.production_door_phase,
                body_damage_state: { body_ord },
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
                death_type_name: if obj.status.destroyed || auth_health <= 0.01 {
                    obj.status.death_type.as_name().to_string()
                } else {
                    String::new()
                },
                under_construction: auth_under_construction,
                construction_percent: auth_construction_percent.clamp(0.0, 1.0),
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
                unattackable: obj.is_kind_of(KindOf::Unattackable),
                ignoring_stealth: obj.status.ignoring_stealth,
                repulsor: obj.status.repulsor,
                stealthed: obj.status.stealthed,
                detected: obj.status.detected,
                effectively_stealthed: obj.is_effectively_stealthed(),
                can_disguise_as_team: freeze_direct_can_disguise_as_team(obj),
                disabled: obj.is_disabled(),
                contained_by: obj.contained_by,
                force_attack: obj.force_attack,
                has_weapon: obj.weapon.is_some(),
                weapon_range: obj.weapon.as_ref().map(|w| w.range).unwrap_or(0.0),
                weapon_damage: obj.weapon.as_ref().map(|w| w.damage).unwrap_or(0.0),
                weapon_min_range: obj.weapon.as_ref().map(|w| w.min_range).unwrap_or(0.0),
                weapon_reload_time: obj.weapon.as_ref().map(|w| w.reload_time).unwrap_or(0.0),
                weapon_ammo: auth_ammo.unwrap_or_else(|| {
                    obj.weapon
                        .as_ref()
                        .map(|w| w.ammo.unwrap_or(u32::MAX))
                        .unwrap_or(u32::MAX)
                }),
                projectile_clip_statuses,
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
                contain_module_present: obj.thing.template.contain_module.kind
                    != crate::game_logic::ContainModuleKind::None,
                contain_module_kind: obj.thing.template.contain_module.kind,
                contain_admission: obj.normal_enter_admission(),
                rider_change_allowed_templates: obj
                    .thing
                    .template
                    .contain_module
                    .rider_change_riders
                    .iter()
                    .filter(|rider| rider.physical_enter_supported)
                    .map(|rider| rider.template_name.clone())
                    .collect(),
                contain_allow_allies_inside: obj.thing.template.contain_module.allow_allies_inside,
                contain_allow_enemies_inside: obj
                    .thing
                    .template
                    .contain_module
                    .allow_enemies_inside,
                contain_allow_neutral_inside: obj
                    .thing
                    .template
                    .contain_module
                    .allow_neutral_inside,
                transport_slot_count: obj.transport_slot_count(),
                is_faction_structure: obj.is_faction_structure(),
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
                can_toggle_overcharge: obj.thing.template.supports_overcharge(),
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
                        // Appended so existing presentation bit positions stay stable.
                        KindOf::Dozer,
                        KindOf::Harvester,
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
                    && !auth_under_construction
                    && auth_construction_percent >= 1.0
                    && !obj.status.destroyed
                    && obj.is_alive(),
                building_type: obj
                    .building_data
                    .as_ref()
                    .map(|b| PresentationBuildingType::from_host(b.building_type)),
                model_key,
                draw_models,
                mesh_scale,
                selection_radius: obj.selection_radius.max(5.0),
                engine_bridged: false,
                fow_visibility,
                drawable_shroud,
                ground_height,
                ground_height_from_terrain,
            };
            direct_host_drawables.push(PresentationDirectHostDrawable {
                object: renderable.clone(),
                visual_template_name,
                // Direct Object lifetime is host roster presence.  Do not
                // derive it from health or gameplay destruction flags.
                resident: true,
            });
            objects.push(renderable);
        }
        // Stable presentation order for determinism (by ObjectId).
        objects.sort_by_key(|o| o.id.0);
        direct_host_drawables.sort_by_key(|drawable| drawable.object.id.0);

        let local = logic.get_player(local_player_id);
        // local_team already resolved above for FOW residual (may fall back to
        // the first is_local non-Neutral team when the slot is Neutral/missing).
        let _local_team_check = local.map(|p| p.team).unwrap_or(Team::Neutral);
        debug_assert!(
            _local_team_check == local_team || _local_team_check == Team::Neutral,
            "local_team FOW residual drifted from player slot"
        );
        let mut players: Vec<PresentationPlayerInfo> = logic
            .get_players()
            .iter()
            .map(|(&id, p)| PresentationPlayerInfo {
                id,
                name: p.name.clone(),
                team: p.team,
                alliance_team: p.alliance_team,
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
                special_power_has_public_timer, special_power_is_structure_bound_public_timer,
                special_power_public_timer_display_name, special_power_public_timer_icon,
                special_power_reload_seconds, special_power_required_science,
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
            // C++ addSuperweapon obtains the exact SpecialPowerModule from a
            // living structure.  Keep the module record with its owning
            // object so timer/reload identity cannot be recreated from an
            // Object INI basename later in the HUD path.
            let owned_structure_modules: Vec<_> = logic
                .host_objects()
                .values()
                .filter(|o| {
                    o.team == p.team
                        && o.is_alive()
                        && o.is_constructed()
                        && (o.is_kind_of(crate::game_logic::KindOf::Structure)
                            || o.is_kind_of(crate::game_logic::KindOf::FSSuperweapon))
                })
                .flat_map(|obj| {
                    obj.thing
                        .template
                        .special_power_modules
                        .iter()
                        .filter(|module| module.public_timer)
                        .map(move |module| (obj, module))
                })
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
                let structure_bound = special_power_is_structure_bound_public_timer(power);
                let matching_modules: Vec<_> = owned_structure_modules
                    .iter()
                    .copied()
                    .filter(|(_, module)| {
                        module
                            .command_power
                            .as_ref()
                            .is_some_and(|candidate| candidate == power)
                    })
                    .collect();
                let science_ok = if structure_bound {
                    // An exact module's RequiredScience must resolve on its
                    // actual owner.  This handles authored general variants
                    // without a parallel enum/name prerequisite table.
                    matching_modules.iter().any(|(_, module)| {
                        module
                            .required_science
                            .as_deref()
                            .map(|required| p.has_unlocked_science(required))
                            .unwrap_or(true)
                    })
                } else {
                    match special_power_required_science(power) {
                        Some(req) => p.has_unlocked_science(req),
                        None => true,
                    }
                };
                let unlocked = if structure_bound {
                    science_ok && !matching_modules.is_empty()
                } else {
                    science_ok
                };
                // Only list unlocked PublicTimer rows (C++ addSuperweapon when present).
                // C++ ~SpecialPowerModule removeSuperweapon: destroyed/sold structure drops row.
                if !unlocked {
                    continue;
                }
                let (template_name, reload, remaining) = if structure_bound {
                    // Per-structure module residual: the first ready source
                    // is the earliest timer, but its reload/name remain that
                    // same exact loaded SpecialPowerTemplate.
                    let mut min_rem = f32::MAX;
                    let mut selected = None;
                    for (obj, module) in &matching_modules {
                        let rem = obj
                            .special_power_cooldowns
                            .get(power)
                            .copied()
                            .unwrap_or(obj.special_power_cooldown_remaining)
                            .max(0.0);
                        if rem < min_rem {
                            min_rem = rem;
                            selected = Some(*module);
                        }
                    }
                    let module = selected.expect("unlocked structure module is non-empty");
                    (
                        module.special_power_template.clone(),
                        (module.reload_time_frames as f32 / 30.0).max(0.0),
                        min_rem,
                    )
                } else {
                    (
                        template.clone(),
                        special_power_reload_seconds(power).unwrap_or(0.0).max(0.0),
                        p.shared_special_power_cooldowns
                            .get(power)
                            .copied()
                            .unwrap_or(0.0)
                            .max(0.0),
                    )
                };
                let ready = remaining <= 0.0;
                superweapon_timers.push(PresentationSuperweaponTimer {
                    name: special_power_public_timer_display_name(power).to_string(),
                    template_name,
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
                let under = logic
                    .host_authoritative_construction(o.id)
                    .map(|(_, uc)| uc)
                    .unwrap_or(o.status.under_construction);
                o.team == p.team
                    && o.is_alive()
                    && !o.status.destroyed
                    && o.building_data.is_some()
                    && !under
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

        #[cfg(feature = "game_client")]
        let scene_lines: Vec<PresentationSceneLine> =
            game_client::render_bridge::snapshot_visible_scene_lines()
                .into_iter()
                .map(|line| PresentationSceneLine {
                    start: (line.start[0], line.start[1], line.start[2]),
                    end: (line.end[0], line.end[1], line.end[2]),
                    width: line.width,
                    color: (line.color[0], line.color[1], line.color[2], line.color[3]),
                    texture_name: line.texture_name,
                    tile_factor: line.tile_factor,
                })
                .collect();
        #[cfg(not(feature = "game_client"))]
        let scene_lines: Vec<PresentationSceneLine> = Vec::new();

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
        // A presentation frame can follow several fixed logic steps. Freeze
        // every real accepted discharge in order; the renderer must never
        // infer recoil from the lossy AI fire-intent residual.
        for ev in logic.take_weapon_discharges_for_presentation() {
            events.push(PresentationEvent::WeaponDischarged {
                source: ev.source,
                weapon_slot: ev.weapon_slot,
                fired_barrel: ev.fired_barrel,
                sequence: ev.sequence,
                logic_frame: ev.logic_frame,
            });
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
            direct_host_drawables,
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
            projected_shroud,
            particle_systems,
            laser_beams,
            scene_lines,
            projectile_streams,
            projectiles,
            floating_texts,
            world_anims,
            dual_tick,
            world_env: PresentationWorldEnv::from_logic_with_runtime_heightmap(
                logic,
                runtime_heightmap,
            ),
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
        Self::build_with_victory_with_runtime_heightmap(logic, local_player_id, None)
    }

    /// Engine-only victory build which retains the cached full terrain payload.
    pub(crate) fn build_with_victory_with_runtime_heightmap(
        logic: &mut GameLogic,
        local_player_id: u32,
        runtime_heightmap: Option<std::sync::Arc<PresentationRuntimeHeightmap>>,
    ) -> Self {
        let mut frame = Self::build_from_logic_with_runtime_heightmap(
            logic,
            local_player_id,
            runtime_heightmap,
        );
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
            o.drawable_shroud.hash(&mut h);
            o.can_disguise_as_team.hash(&mut h);
        }
        // This transient source can diverge from the GameWorld-primary object
        // roster during deferred death, so include the direct visual identity
        // in the deterministic presentation fingerprint as well.
        self.direct_host_drawables.len().hash(&mut h);
        for drawable in &self.direct_host_drawables {
            drawable.object.id.0.hash(&mut h);
            drawable.visual_template_name.hash(&mut h);
            drawable.resident.hash(&mut h);
            drawable.object.position.x.to_bits().hash(&mut h);
            drawable.object.position.y.to_bits().hash(&mut h);
            drawable.object.position.z.to_bits().hash(&mut h);
            drawable.object.orientation.to_bits().hash(&mut h);
            drawable.object.destroyed.hash(&mut h);
            drawable.object.drawable_shroud.hash(&mut h);
            drawable.object.can_disguise_as_team.hash(&mut h);
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
        self.projected_shroud.content_fingerprint().hash(&mut h);
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
        self.scene_lines.len().hash(&mut h);
        for line in &self.scene_lines {
            line.start.0.to_bits().hash(&mut h);
            line.start.1.to_bits().hash(&mut h);
            line.start.2.to_bits().hash(&mut h);
            line.end.0.to_bits().hash(&mut h);
            line.end.1.to_bits().hash(&mut h);
            line.end.2.to_bits().hash(&mut h);
            line.width.to_bits().hash(&mut h);
            line.texture_name.hash(&mut h);
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
