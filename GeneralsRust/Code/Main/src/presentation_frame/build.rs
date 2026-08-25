use super::*;
use crate::fow_rendering::{ProjectedShroudMetadata, ProjectedShroudSnapshot};

/// Freeze only the source facts C++ resolves for a direct Object-backed
/// Drawable. Host object FOW is the PartitionData COI mix stored by
/// `update_main_crate_vision` (Clear / PartialClear / Fogged / Shrouded).
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
        if let Some(status) = shroud.get_host_object_shroud_status(local_player_id, obj.id.0) {
            PresentationObjectShroudStatus::from(status)
        } else {
            let runtime_active = !shroud.get_visible_objects(local_player_id).is_empty()
                || !shroud.get_explored_objects(local_player_id).is_empty();
            if !runtime_active || shroud.can_see_object(local_player_id, obj.id.0) {
                PresentationObjectShroudStatus::Clear
            } else if shroud.has_explored_object(local_player_id, obj.id.0) {
                PresentationObjectShroudStatus::Fogged
            } else {
                PresentationObjectShroudStatus::Shrouded
            }
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

pub(super) fn object_is_mine_kind(obj: &crate::game_logic::Object) -> bool {
    use crate::game_logic::KindOf;
    obj.mine_data.is_some() || obj.is_kind_of(KindOf::Mine) || obj.is_kind_of(KindOf::DemoTrap)
}

/// C++ mines force `setEffectiveOpacity(0,0)` every frame; other units keep
/// template FriendlyOpacityMin for the friendly pulse.
pub(super) fn freeze_friendly_stealth_opacity(obj: &crate::game_logic::Object) -> f32 {
    if object_is_mine_kind(obj) {
        obj.camo_friendly_opacity
    } else {
        obj.thing.template.stealth_friendly_opacity_min
    }
}

/// C++ drawBombed StickyBombUpdate: timed vs remote + countdown seconds.
pub(super) fn freeze_sticky_bomb_overlay(obj: &crate::game_logic::Object, now: u32) -> (u8, u32) {
    let Some(md) = obj.mine_data.as_ref() else {
        return (0, 0);
    };
    if md.attached_to.is_none() {
        return (0, 0);
    }
    match md.kind {
        crate::game_logic::host_mines::HostMineKind::TimedDemoCharge => {
            let seconds = md
                .detonate_at_frame
                .map(|die| {
                    if die <= now {
                        0
                    } else {
                        ((die - now) as f32 / 30.0).ceil() as u32
                    }
                })
                .unwrap_or(0);
            (1, seconds)
        }
        crate::game_logic::host_mines::HostMineKind::RemoteDemoCharge => (2, 0),
        _ => (0, 0),
    }
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

/// C++ `Drawable` construction takes the replacement visual template's
/// `getAssetScale()`, not the source Object's scale.  Main's direct visual
/// identity is a frozen name, so resolve the matching loaded host template at
/// the same snapshot boundary and fail closed to the source scale when an
/// incomplete test/mod table lacks that named template.
fn direct_host_visual_mesh_scale(
    logic: &GameLogic,
    obj: &crate::game_logic::Object,
    visual_template_name: &str,
) -> f32 {
    let authored = logic
        .templates
        .get(visual_template_name)
        .map(crate::assets::mesh_asset_resolve::mesh_scale_from_template)
        .unwrap_or_else(|| {
            crate::assets::mesh_asset_resolve::mesh_scale_from_template(obj.get_template())
        });
    let instance = if obj.drawable_instance_scale.is_finite() && obj.drawable_instance_scale > 0.0 {
        obj.drawable_instance_scale
    } else {
        1.0
    };
    authored * instance
}

/// Freeze the local player's energy grid from host Player fields, falling
/// back to that player's constructed objects when the residual is still 0.
///
/// `update_player_resources` writes `power_produced` / `power_consumed` from
/// object EnergyProduction. Live freeze can run before that scan, so a
/// standing CC + plant would otherwise stamp 0 into the HUD overlay.
fn freeze_host_player_power(
    logic: &GameLogic,
    local_player_id: u32,
    local: Option<&crate::game_logic::Player>,
) -> (i32, i32, i32) {
    let power_src = local.or_else(|| {
        logic
            .get_players()
            .values()
            .find(|player| player.is_local && player.team != Team::Neutral)
    });
    let power_player_id = power_src.map(|player| player.id).unwrap_or(local_player_id);
    let power_team = power_src.map(|player| player.team);
    let (obj_produced, obj_consumed) =
        logic
            .host_objects()
            .values()
            .fold((0_i32, 0_i32), |(produced, consumed), object| {
                let owned = match object.owner_player_id {
                    Some(id) => id == power_player_id,
                    None => power_team == Some(object.team),
                };
                if !owned || !object.is_constructed() || !object.is_alive() {
                    return (produced, consumed);
                }
                (
                    produced.saturating_add(object.power_provided),
                    consumed.saturating_add(object.power_consumed.abs()),
                )
            });
    let overcharge = power_src
        .map(|player| player.captured_overcharge_power_delta)
        .unwrap_or(0);
    let (mut produced, consumed) = match power_src {
        Some(player) if player.power_produced != 0 || player.power_consumed != 0 => {
            (player.power_produced, player.power_consumed)
        }
        _ => (obj_produced.saturating_add(overcharge), obj_consumed),
    };
    let sabotaged = power_src.is_some_and(|player| {
        player.power_sabotaged_till_frame > 0 && logic.frame < player.power_sabotaged_till_frame
    });
    if sabotaged {
        produced = 0;
    }
    let available = match power_src {
        Some(player)
            if !sabotaged && (player.power_produced != 0 || player.power_consumed != 0) =>
        {
            player.power_available
        }
        _ => produced - consumed,
    };
    (available, produced, consumed)
}

/// C++ `Object::getControllingPlayer` for PublicTimer ownership.
/// Prefer persistent `owner_player_id`; faction-only leftovers match only
/// when that team has exactly one living host player.
fn superweapon_object_owned_by_player(
    obj: &crate::game_logic::Object,
    player: &crate::game_logic::Player,
    logic: &GameLogic,
) -> bool {
    match obj.owner_player_id {
        Some(id) => id == player.id,
        None => {
            obj.team == player.team
                && player.team != Team::Neutral
                && logic.unique_player_id_for_team(player.team) == Some(player.id)
        }
    }
}

/// C++ InGameUI SuperweaponInfo name + `m_color` (player color).
/// Local rows stay unadorned; enemy/ally rows carry relationship + `#RRGGBB`.
fn public_timer_row_name(
    display: &str,
    is_local: bool,
    rel: gamelogic::common::Relationship,
    color: (u8, u8, u8),
) -> String {
    if is_local {
        return display.to_string();
    }
    let rel = match rel {
        gamelogic::common::Relationship::Enemies => "Enemy",
        gamelogic::common::Relationship::Allies => "Ally",
        gamelogic::common::Relationship::Neutral => "Neutral",
    };
    let (r, g, b) = color;
    format!("{display} ({rel} #{r:02X}{g:02X}{b:02X})")
}

/// Local player keeps a bare `SpecialPowerType` Debug key so overlay tests
/// that match `"ParticleCannon"` stay valid. Other players suffix `#id`.
fn public_timer_power_key(
    power: &crate::command_system::SpecialPowerType,
    player_id: u32,
    local_player_id: u32,
) -> String {
    if player_id == local_player_id {
        format!("{power:?}")
    } else {
        format!("{power:?}#{player_id}")
    }
}

/// Split overlay/HUD power_key into `(owner_player_id, SpecialPowerType Debug)`.
pub(crate) fn split_superweapon_power_key(power_key: &str, local_player_id: u32) -> (u32, &str) {
    match power_key.rsplit_once('#') {
        Some((key, id)) if !key.is_empty() => {
            if let Ok(pid) = id.parse::<u32>() {
                return (pid, key);
            }
        }
        _ => {}
    }
    (local_player_id, power_key)
}

fn object_public_timer_remaining(
    obj: &crate::game_logic::Object,
    power: &crate::command_system::SpecialPowerType,
) -> f32 {
    obj.special_power_cooldowns
        .get(power)
        .copied()
        .unwrap_or(obj.special_power_cooldown_remaining)
        .max(0.0)
}

/// C++ `SpecialPowerModule::isReady` plus HUD bold/flash: remaining 0 is not
/// ready when the source is disabled or pauseCountdown is held.
fn object_public_timer_ready(
    obj: &crate::game_logic::Object,
    power: &crate::command_system::SpecialPowerType,
    remaining: f32,
) -> bool {
    remaining <= 0.0 && !obj.is_disabled() && !obj.is_special_power_countdown_paused(power)
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
            // C++ Drawable::setDrawableHidden — ride-hide hijacker has no mesh.
            if obj.drawable_hidden {
                continue;
            }

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
                    HostBodyDamageType, host_calc_body_damage_state,
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
                    HostBodyDamageType, MC_BIT_ATTACKING, MC_BIT_DYING, MC_BIT_MOVING,
                    host_apply_body_damage_model_bits, host_calc_body_damage_state,
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
            let draw_models = crate::assets::resolve_presentation_draw_models_for_live_object(
                obj.id.0,
                &obj.template_name,
                fallback_draw_models.as_slice(),
                model_condition_bits,
            );
            let model_key = draw_models.first().map(|model| model.model_key.clone());
            // Wave 75: freeze mesh scale residual (common combat = 1.0; CINE/weapon peels).
            let mesh_scale = {
                let authored =
                    crate::assets::mesh_asset_resolve::mesh_scale_from_template(obj.get_template());
                let instance = if obj.drawable_instance_scale.is_finite()
                    && obj.drawable_instance_scale > 0.0
                {
                    obj.drawable_instance_scale
                } else {
                    1.0
                };
                authored * instance
            };
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
            let visual_mesh_scale =
                direct_host_visual_mesh_scale(logic, obj, &visual_template_name);
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
            // C++ GarrisonContain.cpp:1231-1236 — MODELCONDITION_GARRISONED if
            // first occupant is DETECTED or local player is apparent controller.
            let first_occupant_detected = obj
                .contained_units()
                .first()
                .and_then(|id| logic.host_object(*id))
                .is_some_and(|o| o.status.detected);
            let local_is_apparent_controller = obj.team == local_team;
            let garrison_hide_from_local = obj
                .building_data
                .as_ref()
                .is_some_and(|b| b.hide_garrisoned_state)
                && !first_occupant_detected
                && !local_is_apparent_controller;
            let garrison_apparent_team = if garrison_hide_from_local {
                obj.building_data
                    .as_ref()
                    .and_then(|b| b.original_team)
                    .unwrap_or(obj.team)
            } else {
                obj.team
            };
            let renderable = RenderableObject {
                id: obj.id,
                template_name: obj.template_name.clone(),
                team: garrison_apparent_team,
                owner_player_id: obj.owner_player_id,
                team_color: {
                    if garrison_hide_from_local {
                        garrison_apparent_team.get_color()
                    } else if obj.status.disguised && obj.team != local_team {
                        // Wave 503: C++ enemies see disguise player color; allies see true colors.
                        if let Some(dt) = obj.disguise_as_team {
                            dt.get_color()
                        } else {
                            logic
                                .player_house_color_rgba(obj.owner_player_id)
                                .unwrap_or(obj.team_color)
                        }
                    } else {
                        // C++ getIndicatorColor: controlling player's house color,
                        // so same-faction slots and captured units recolor.
                        logic
                            .player_house_color_rgba(obj.owner_player_id)
                            .unwrap_or(obj.team_color)
                    }
                },
                // Use accessors so presentation matches authoritative transform state.
                position: {
                    let mut p = auth_pose
                        .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                        .unwrap_or(pos);
                    p.y += obj.presentation_collapse_height_offset();
                    p.y += obj.presentation_slow_death_sink_offset();
                    if crate::assets::authored_draw_adjusts_height_by_construction(&draw_models) {
                        let geom = &obj.thing.template.geometry_info;
                        let height = if geom.authored {
                            geom.max_height_above_position()
                        } else {
                            obj.selection_radius.max(1.0)
                        };
                        let cpp_percent =
                            if auth_under_construction || auth_construction_percent + 1e-4 < 1.0 {
                                auth_construction_percent * 100.0
                            } else {
                                -1.0
                            };
                        if let Some(dy) =
                            crate::assets::construction_percent_height_delta(cpp_percent, height)
                        {
                            p.y += dy;
                        }
                    }
                    let (sx, sz) = obj.presentation_collapse_shudder();
                    p.x += sx;
                    p.z += sz;
                    p
                },
                orientation: obj.get_orientation(),
                float_yaw: {
                    let yaw = obj.float_update.as_ref().map(|f| f.yaw).unwrap_or(0.0);
                    let pitch = obj.float_update.as_ref().map(|f| f.pitch).unwrap_or(0.0);
                    crate::game_logic::host_float_update::publish_sway(obj.id.0, yaw, pitch);
                    yaw
                },
                float_pitch: obj.float_update.as_ref().map(|f| f.pitch).unwrap_or(0.0),
                topple_lean_radians: obj.presentation_topple_lean_radians(),
                topple_dir_x: obj.presentation_topple_dir().0,
                topple_dir_y: obj.presentation_topple_dir().1,
                shadows_enabled: obj.presentation_shadows_enabled(),
                terrain_decal_type: obj.terrain_decal_type,
                terrain_decal_size: obj.terrain_decal_size,
                terrain_decal_opacity: obj.terrain_decal_opacity,
                move_destination: auth_dest,
                target_location: obj.target_location,
                guard_target: obj.guard_target,
                using_ability: obj.status.using_ability,
                airborne_target: obj.status.airborne_target,
                producer_id: obj.producer_id,
                show_healing: {
                    // C++ HEALING_ICON_DISPLAY_TIME residual via sole-benefactor
                    // claim window, plus Drawable::xfer keepTillFrame icons.
                    let now = logic.get_current_frame() as u32;
                    (obj.sole_healing_benefactor_expiration_frame > now
                        && obj.sole_healing_benefactor_expiration_frame != 0)
                        || obj.overlay_icon_active(
                            &["DefaultHeal", "StructureHeal", "VehicleHeal"],
                            now,
                        )
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
                object_weather: obj.object_weather,

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
                occupant_count: if garrison_hide_from_local {
                    0
                } else {
                    auth_occupants.unwrap_or(obj.occupants.len().min(u16::MAX as usize) as u16)
                },
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
                garrisoned_units: if garrison_hide_from_local {
                    Vec::new()
                } else {
                    logic
                        .host_authoritative_contained_units(obj.id)
                        .into_iter()
                        .take(32)
                        .collect()
                },

                stealth_garrison_occupant_count: obj
                    .contained_units()
                    .iter()
                    .filter(|id| {
                        logic
                            .host_object(**id)
                            .is_some_and(|occupant| occupant.is_kind_of(KindOf::StealthGarrison))
                    })
                    .count()
                    .min(u16::MAX as usize) as u16,

                max_garrison: obj
                    .building_data
                    .as_ref()
                    .map(|b| b.max_garrison)
                    .unwrap_or(0),
                power_provided: obj.power_provided,
                power_consumed: obj.power_consumed,
                stored_supplies: obj.stored_resources.supplies,
                drawable_supply_boxes: obj.drawable_supply_boxes,
                drawable_supply_max_boxes: obj.drawable_supply_max_boxes,
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
                    .as_ref()
                    .is_some_and(|metadata| metadata.is_hacker_command()),
                hacker_disable_building_ready: logic.is_hacker_disable_building_ready(obj.id),
                special_power_ready_template_name: ready_structure_special_power_module
                    .map(|module| module.special_power_template.clone()),
                special_power_ready_template_id: ready_structure_special_power_module
                    .map(|module| module.special_power_template_id),
                special_power_override_destination: obj.special_power_override_destination,
                health_current: auth_health,
                health_max: obj.health.maximum,
                selected: (obj.selected || obj.status.selected)
                    && !obj.drawable_is_effectively_hidden(),
                is_deployed: obj.status.deployed,
                selection_flash_remaining: obj.selection_flash_remaining,
                selection_flash_color: obj.selection_flash_color,

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
                is_dozer_task_pending: obj.dozer_task_build_target.is_some(),
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
                script_unsellable: obj.script_unsellable,
                single_use_command_used: obj.single_use_command_used,
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
                disabled_underpowered: obj.status.disabled_underpowered,
                disabled_hacked: obj.status.disabled_hacked,
                disabled_unmanned: obj.status.disabled_unmanned,
                disabled_freefall: obj.status.disabled_freefall,
                disabled_default: obj.status.disabled_default,
                disabled_script_underpowered: obj.status.disabled_script_underpowered,
                disabled_script_disabled: obj.status.disabled_script_disabled,
                hacking_packing_or_unpacking: logic.hacker_income.is_hacking(obj.id),
                weapons_jammed: obj.status.weapons_jammed,
                masked: obj.status.masked,
                unattackable: obj.is_kind_of(KindOf::Unattackable),
                is_force_attackable: obj.is_kind_of(KindOf::ForceAttackable),
                always_selectable: obj.is_kind_of(KindOf::AlwaysSelectable),

                is_crate: obj.is_kind_of(KindOf::Crate) || logic.host_money_crates.contains(obj.id),
                is_salvage_crate: logic
                    .host_money_crates
                    .get(obj.id)
                    .is_some_and(|entry| entry.is_salvage)
                    || obj.template_name.eq_ignore_ascii_case("SalvageCrate"),
                ignoring_stealth: obj.status.ignoring_stealth,
                repulsor: obj.status.repulsor,
                stealthed: obj.status.stealthed,
                detected: obj.status.detected,
                effectively_stealthed: obj.is_effectively_stealthed(),
                can_disguise_as_team: freeze_direct_can_disguise_as_team(obj),
                friendly_stealth_opacity: freeze_friendly_stealth_opacity(obj),
                friendly_stealth_opacity_max: obj.thing.template.stealth_friendly_opacity_max,
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
                weapon_set_carbomb: obj.weapon_set_carbomb,
                bomb_type: freeze_sticky_bomb_overlay(obj, logic.get_current_frame() as u32).0,
                bomb_timer_seconds: freeze_sticky_bomb_overlay(
                    obj,
                    logic.get_current_frame() as u32,
                )
                .1,
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
                sub_object_visibility: obj.sub_object_visibility.clone(),

                upgrade_cameo_names: obj.thing.template.upgrade_cameo_names.clone(),
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
                safe_occlusion_frame: obj.safe_occlusion_frame,

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
                health_box_width: obj.get_health_box_dimensions().1,
                health_box_z_offset: obj.health_box_world_z_offset(),
                max_height_above_position: {
                    let geom = &obj.thing.template.geometry_info;
                    if geom.authored {
                        geom.max_height_above_position()
                    } else {
                        0.0
                    }
                },

                engine_bridged: false,
                fow_visibility,
                drawable_shroud,
                ground_height,
                ground_height_from_terrain,
                drawable_fade_mode: obj.drawable_fade_mode,
                drawable_fade_start_frame: obj.drawable_fade_start_frame,
                drawable_fade_frames: obj.drawable_fade_frames,
                gaining_subdual: obj.subdual_damage > 0.0,
                drawable_explicit_opacity: obj.drawable_explicit_opacity,
                camo_heat_vision_opacity: obj.camo_heat_vision_opacity,
            };
            direct_host_drawables.push(PresentationDirectHostDrawable {
                object: renderable.clone(),
                visual_template_name,
                visual_mesh_scale,
                // Direct Object lifetime is host roster presence.  Do not
                // derive it from health or gameplay destruction flags.
                resident: true,
            });
            objects.push(renderable);
        }
        sync_live_terrain_decals(&objects, local_team);
        #[cfg(feature = "game_client")]
        {
            let script_frozen = logic.is_script_time_frozen();
            let camera_frozen = logic.is_script_camera_time_frozen();
            let frozen = script_frozen || camera_frozen;
            super::unit_render::host_draw_schedule::begin_presented_frame(
                super::unit_render::host_draw_schedule::HostPresentVisualInput {
                    visual_dt_ms: if frozen {
                        0
                    } else {
                        super::unit_render::host_draw_schedule::HOST_VISUAL_FRAME_MS
                    },
                    frozen,
                },
            );
            let host_objects = logic.host_objects();
            for obj in host_objects.values() {
                if obj.drawable_hidden {
                    continue;
                }
                super::unit_render::physics_visual_host::freeze_for_object(
                    obj,
                    host_objects,
                    script_frozen,
                    camera_frozen,
                    logic,
                );
            }
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
        let (local_power, local_power_produced, local_power_consumed) =
            freeze_host_player_power(logic, local_player_id, local);
        #[cfg(feature = "game_client")]
        game_client::gui::control_bar::ControlBar::stamp_presentation_power(
            local_power_produced,
            local_power_consumed,
        );
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
                RankSkillStateResidual, rank_level_down_threshold_residual,
                rank_level_up_threshold_residual, rank_progress_percent_residual,
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
        let mut local_completed_upgrades: Vec<String> = local
            .map(|p| {
                let mut v: Vec<String> = p.completed_upgrades.iter().cloned().collect();
                v.sort();
                v.truncate(MAX_UPGRADE_NAMES);
                v
            })
            .unwrap_or_default();
        let _ = (
            &mut local_unlocked_sciences,
            &mut local_queued_upgrades,
            &mut local_completed_upgrades,
        );

        // PublicTimer superweapon residual.
        // C++ InGameUI.cpp:3503 iterates SuperweaponInfo per object; skip UC;
        // SharedNSync breaks after the first valid; honor m_superweaponHiddenByScript
        // and per-info m_hiddenByScript. Sell keeps the row until destroy.
        let mut superweapon_timers: Vec<PresentationSuperweaponTimer> = Vec::new();
        if logic.peek_script_superweapon_display_enabled() {
            let hidden_objects = logic.peek_script_superweapon_hidden_objects();
            use crate::command_system::SpecialPowerType as P;
            use crate::game_logic::host_special_power_enum_residual::{
                special_power_has_public_timer, special_power_is_structure_bound_public_timer,
                special_power_public_timer_display_name, special_power_public_timer_icon,
                special_power_reload_seconds, special_power_required_science,
                special_power_uses_shared_synced_timer,
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
            let mut player_ids: Vec<u32> = logic.get_players().keys().copied().collect();
            player_ids.sort_unstable();
            for pid in player_ids {
                let Some(p) = logic.get_player(pid) else {
                    continue;
                };
                let is_local = pid == local_player_id;
                let rel = logic.player_relationship(local_player_id, pid);
                let owned_structure_modules: Vec<_> = logic
                    .host_objects()
                    .values()
                    .filter(|o| {
                        superweapon_object_owned_by_player(o, p, logic)
                            && o.is_alive()
                            && !o.status.under_construction
                            && !hidden_objects.contains(&o.id)
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
                    if !unlocked {
                        continue;
                    }
                    let shared_n_sync = matching_modules
                        .iter()
                        .any(|(_, module)| module.shared_n_sync)
                        || special_power_uses_shared_synced_timer(power);
                    if shared_n_sync {
                        // C++ SpecialPowerModule.cpp:756-765 + InGameUI.cpp:3684 —
                        // one row per player+template from the player ready frame.
                        let module = matching_modules.first().map(|(_, module)| *module);
                        let remaining = p.shared_special_power_remaining(power);
                        let ready = match matching_modules.first() {
                            Some((obj, _)) => object_public_timer_ready(obj, power, remaining),
                            None => remaining <= 0.0,
                        };
                        superweapon_timers.push(PresentationSuperweaponTimer {
                            name: public_timer_row_name(
                                special_power_public_timer_display_name(power),
                                is_local,
                                rel,
                                p.color_rgb,
                            ),
                            template_name: module
                                .map(|module| module.special_power_template.clone())
                                .unwrap_or_else(|| template.clone()),
                            icon: special_power_public_timer_icon(power).to_string(),
                            recharge_time: module
                                .map(|module| (module.reload_time_frames as f32 / 30.0).max(0.0))
                                .unwrap_or_else(|| {
                                    special_power_reload_seconds(power).unwrap_or(0.0).max(0.0)
                                }),
                            remaining,
                            unlocked,
                            ready,
                            power_key: public_timer_power_key(power, pid, local_player_id),
                        });
                    } else if structure_bound {
                        // C++ one SuperweaponInfo per object; no min() merge.
                        for (obj, module) in matching_modules {
                            let remaining = object_public_timer_remaining(obj, power);
                            let ready = object_public_timer_ready(obj, power, remaining);
                            superweapon_timers.push(PresentationSuperweaponTimer {
                                name: public_timer_row_name(
                                    special_power_public_timer_display_name(power),
                                    is_local,
                                    rel,
                                    p.color_rgb,
                                ),
                                template_name: module.special_power_template.clone(),
                                icon: special_power_public_timer_icon(power).to_string(),
                                recharge_time: (module.reload_time_frames as f32 / 30.0).max(0.0),
                                remaining,
                                unlocked,
                                ready,
                                power_key: public_timer_power_key(power, pid, local_player_id),
                            });
                        }
                    } else {
                        let remaining = p.shared_special_power_remaining(power);
                        superweapon_timers.push(PresentationSuperweaponTimer {
                            name: public_timer_row_name(
                                special_power_public_timer_display_name(power),
                                is_local,
                                rel,
                                p.color_rgb,
                            ),
                            template_name: template.clone(),
                            icon: special_power_public_timer_icon(power).to_string(),
                            recharge_time: special_power_reload_seconds(power)
                                .unwrap_or(0.0)
                                .max(0.0),
                            remaining,
                            unlocked,
                            ready: remaining <= 0.0,
                            power_key: public_timer_power_key(power, pid, local_player_id),
                        });
                    }
                }
            }
            superweapon_timers.sort_by(|a, b| {
                a.power_key
                    .cmp(&b.power_key)
                    .then_with(|| a.name.cmp(&b.name))
            });
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
                let is_factory = o.building_data.is_some();
                let is_dozer = o.is_kind_of(crate::game_logic::KindOf::Dozer)
                    || o.is_kind_of(crate::game_logic::KindOf::Worker);
                o.team == p.team
                    && o.is_alive()
                    && !o.status.destroyed
                    && (is_factory || is_dozer)
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
                    let is_dozer = o.is_some_and(|o| {
                        o.is_kind_of(crate::game_logic::KindOf::Dozer)
                            || o.is_kind_of(crate::game_logic::KindOf::Worker)
                    });
                    if is_dozer {
                        // CommandSet construct slots are appended below.
                        &[]
                    } else {
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
                            Some(BuildingType::CommandCenter) => {
                                &["AmericaVehicleDozer", "TestDozer"]
                            }
                            _ => &["TestInfantry", "TestRaptor", "AmericaInfantryColonelBurton"],
                        }
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
                        buildable_hidden: logic
                            .templates
                            .get(*name)
                            .is_some_and(|t| t.human_control_bar_buildable_hidden()),
                    });
                }
                if let Some(o) = logic.host_object(pid) {
                    use crate::game_logic::host_production_buildable_command_residual::factory_command_set_for_producer;
                    if let Some(pack) = factory_command_set_for_producer(&o.template_name) {
                        for (_, producible) in pack.slots {
                            if can_make_cameos
                                .iter()
                                .any(|c| c.template_name.eq_ignore_ascii_case(producible))
                            {
                                continue;
                            }
                            let status = logic.can_make_unit(pid, producible);
                            let is_struct = logic
                                .templates
                                .get(*producible)
                                .map(|t| t.is_kind_of(crate::game_logic::KindOf::Structure))
                                .unwrap_or(false);
                            let help = can_make_type_help_box_message_residual(status, is_struct)
                                .map(|s| s.to_string());
                            can_make_cameos.push(PresentationCanMakeCameo {
                                template_name: (*producible).to_string(),
                                can_make: status,
                                available: status
                                    == crate::game_logic::host_production_buildable_command_residual::CANMAKE_OK,
                                help_status: help,
                                buildable_hidden: logic
                                    .templates
                                    .get(*producible)
                                    .is_some_and(|t| t.human_control_bar_buildable_hidden()),
                            });
                        }
                    }
                }
                can_make_cameos.truncate(32);
            }
        }
        let selected: Vec<ObjectId> = local
            .map(|p| {
                p.selected_objects
                    .iter()
                    .copied()
                    .filter(|&id| {
                        logic
                            .host_object(id)
                            .is_some_and(|o| !o.drawable_is_effectively_hidden())
                    })
                    .collect()
            })
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
                    scroll_rate: line.scroll_rate,
                })
                .collect();
        #[cfg(not(feature = "game_client"))]
        let scene_lines: Vec<PresentationSceneLine> = Vec::new();

        #[cfg(feature = "game_client")]
        let camera_fade = {
            let _ = game_client::display::status_circle::render_camera_fade();
            game_client::display::status_circle::current_camera_fade()
                .map(|overlay| PresentationCameraFade {
                    fade: overlay.fade as u8,
                    intensity: overlay.intensity,
                    diffuse: overlay.diffuse,
                })
                .unwrap_or_default()
        };
        #[cfg(not(feature = "game_client"))]
        let camera_fade = PresentationCameraFade::default();

        #[cfg(all(feature = "game_client", feature = "game_engine_device"))]
        {
            let context = game_engine_device::w3_d_device::game_client::w3_d_waypoint_buffer::WaypointDrawContext {
                in_waypoint_mode: game_client::helpers::TheInGameUI::is_in_waypoint_mode(),
                selected: Vec::new(),
                moused_over: None,
            };
            let _ = game_engine_device::w3_d_device::game_client::w3_d_waypoint_buffer::draw_live_waypoints(
                Some(&context),
            );
        }

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
                visual_plan: ev.visual_plan,
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
            // C++ addFloatingText is cash-only (Player.cpp / AutoDeposit /
            // SupplyCenter / crates). Do not invent CombatDamage -N floaters.
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
            local_completed_upgrades,
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
            restrict_a: PresentationRestrictA::default(),
            local_queued_upgrades,
            selected,
            events,
            match_over: false,
            victory_label: None,
            defeated_player_ids: Vec::new(),
            alliance_events: Vec::new(),
            victory_summary: None,
            beacons: {
                // Visible host beacon objects win (C++ hideBeacon drops drawable + minimap).
                let from_objects: Vec<glam::Vec3> = logic
                    .host_objects()
                    .iter()
                    .filter_map(|(id, obj)| {
                        if !obj.is_alive()
                            || !obj.template_name.to_ascii_lowercase().contains("beacon")
                            || obj.drawable_hidden
                            || crate::command_executor::host_beacon_is_hidden(*id)
                        {
                            return None;
                        }
                        Some(obj.get_position())
                    })
                    .take(64)
                    .collect();
                let has_host_beacon_objs = logic.host_objects().values().any(|obj| {
                    obj.is_alive() && obj.template_name.to_ascii_lowercase().contains("beacon")
                });
                if has_host_beacon_objs {
                    from_objects
                } else {
                    // Wave 211: prefer host-owned beacon list (no Mutex dual-read).
                    let visible = |p: &glam::Vec3| {
                        !crate::command_executor::host_beacon_position_is_hidden(logic, *p)
                    };
                    let host = logic.host_beacons();
                    if !host.is_empty() {
                        host.iter().copied().filter(visible).take(64).collect()
                    } else {
                        #[cfg(feature = "game_client")]
                        {
                            use gamelogic::system::beacon_manager::snapshot_beacons;
                            snapshot_beacons()
                                .into_iter()
                                .map(|b| glam::Vec3::new(b.position.x, b.position.y, b.position.z))
                                .filter(visible)
                                .take(64)
                                .collect()
                        }
                        #[cfg(not(feature = "game_client"))]
                        {
                            Vec::new()
                        }
                    }
                }
            },
            new_beacons: logic
                .recent_beacons()
                .iter()
                .copied()
                .filter(|p| !crate::command_executor::host_beacon_position_is_hidden(logic, *p))
                .take(32)
                .collect(),
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
            cinematic_font: logic.cinematic_font().map(|s| s.to_string()),
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
                .map(|s| {
                    (
                        [s.position.x, s.position.y, s.position.z],
                        s.amplitude,
                        s.duration_seconds,
                        s.radius,
                    )
                })
                .take(8)
                .collect(),
            camera_motion_blur_count: logic.peek_pending_camera_motion_blur_count(),
            camera_zoom: logic
                .peek_pending_camera_zoom()
                .map(|z| (z.zoom, z.duration_seconds)),
            camera_zoom_reset: logic.peek_pending_camera_zoom_reset(),
            camera_zoom_reset_duration: logic.peek_pending_camera_zoom_reset_duration(),
            camera_zoom_reset_ease: logic.peek_pending_camera_zoom_reset_ease(),
            camera_zoom_ease: logic
                .peek_pending_camera_zoom()
                .map(|z| (z.ease_in_seconds, z.ease_out_seconds))
                .unwrap_or((0.0, 0.0)),
            camera_pitch: logic
                .peek_pending_camera_pitch()
                .map(|p| (p.pitch, p.duration_seconds)),
            camera_pitch_ease: logic
                .peek_pending_camera_pitch()
                .map(|p| (p.ease_in_seconds, p.ease_out_seconds))
                .unwrap_or((0.0, 0.0)),
            camera_rotate: logic
                .peek_pending_camera_rotate()
                .map(|r| (r.rotations, r.duration_seconds)),
            camera_rotate_ease: logic
                .peek_pending_camera_rotate()
                .map(|r| (r.ease_in_seconds, r.ease_out_seconds))
                .unwrap_or((0.0, 0.0)),
            camera_look_toward: logic
                .peek_pending_camera_look_toward()
                .map(|l| [l.position.x, l.position.y, l.position.z]),
            camera_look_toward_duration: logic
                .peek_pending_camera_look_toward()
                .map(|l| l.duration_seconds)
                .unwrap_or(0.0),
            camera_look_toward_ease: logic
                .peek_pending_camera_look_toward()
                .map(|l| (l.ease_in_seconds, l.ease_out_seconds))
                .unwrap_or((0.0, 0.0)),
            camera_look_toward_reverse_rotation: logic
                .peek_pending_camera_look_toward()
                .map(|l| l.reverse_rotation)
                .unwrap_or(false),
            camera_tether_play: logic.peek_camera_tether_play(),
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
            camera_fade,
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
        frame.stamp_restrict_a(logic);
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
            o.friendly_stealth_opacity.to_bits().hash(&mut h);
            o.friendly_stealth_opacity_max.to_bits().hash(&mut h);
            o.drawable_fade_mode.hash(&mut h);
            o.drawable_fade_start_frame.hash(&mut h);
            o.drawable_fade_frames.hash(&mut h);
            o.gaining_subdual.hash(&mut h);
        }
        // This transient source can diverge from the GameWorld-primary object
        // roster during deferred death, so include the direct visual identity
        // in the deterministic presentation fingerprint as well.
        self.direct_host_drawables.len().hash(&mut h);
        for drawable in &self.direct_host_drawables {
            drawable.object.id.0.hash(&mut h);
            drawable.visual_template_name.hash(&mut h);
            drawable.visual_mesh_scale.to_bits().hash(&mut h);
            drawable.resident.hash(&mut h);
            drawable.object.position.x.to_bits().hash(&mut h);
            drawable.object.position.y.to_bits().hash(&mut h);
            drawable.object.position.z.to_bits().hash(&mut h);
            drawable.object.orientation.to_bits().hash(&mut h);
            drawable.object.destroyed.hash(&mut h);
            drawable.object.drawable_shroud.hash(&mut h);
            drawable.object.can_disguise_as_team.hash(&mut h);
            drawable
                .object
                .friendly_stealth_opacity
                .to_bits()
                .hash(&mut h);
            drawable
                .object
                .friendly_stealth_opacity_max
                .to_bits()
                .hash(&mut h);
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

fn sync_live_terrain_decals(objects: &[RenderableObject], local_team: crate::game_logic::Team) {
    #[cfg(feature = "game_client")]
    {
        use crate::game_logic::host_battlemaster::{
            TERRAIN_DECAL_NONE, TERRAIN_DECAL_SHADOW_TEXTURE,
            leftover_infantry_horde_decal_size_or_bbox, terrain_decal_texture_name,
        };
        use game_client::radius_decal::{ShadowHandle, enqueue_delivery_decal};
        use std::sync::Mutex;
        static HANDLES: Mutex<Vec<ShadowHandle>> = Mutex::new(Vec::new());
        let Ok(mut handles) = HANDLES.lock() else {
            return;
        };
        for handle in handles.drain(..) {
            handle.release();
        }
        for obj in objects {
            if obj.terrain_decal_type == TERRAIN_DECAL_NONE || obj.terrain_decal_opacity <= 0.0 {
                continue;
            }
            if obj.terrain_decal_type == TERRAIN_DECAL_SHADOW_TEXTURE
                && obj.team != local_team
                && obj.team != crate::game_logic::Team::Neutral
            {
                continue;
            }
            let texture = terrain_decal_texture_name(obj.terrain_decal_type);
            if texture.is_empty() {
                continue;
            }
            let size = if obj.terrain_decal_size > 0.0 {
                obj.terrain_decal_size
            } else {
                // C++ W3DProjectedShadow::addDecal: bbox Extent*2 when ShadowSize 0.
                leftover_infantry_horde_decal_size_or_bbox(
                    0.0,
                    0.0,
                    obj.selection_radius,
                    obj.selection_radius,
                )
            };
            if size <= 0.0 {
                continue;
            }

            let color = match obj.terrain_decal_type {
                5 => [255, 210, 64],
                7 => [64, 220, 96],
                TERRAIN_DECAL_SHADOW_TEXTURE => [16, 16, 16],
                _ => [255, 255, 255],
            };
            if let Some(handle) = enqueue_delivery_decal(
                texture,
                size * 0.5,
                obj.position.x,
                obj.position.y,
                obj.position.z,
                color,
                obj.terrain_decal_opacity,
            ) {
                handles.push(handle);
            }
        }
    }
    #[cfg(not(feature = "game_client"))]
    {
        let _ = (objects, local_team);
    }
}

#[cfg(test)]
mod sw_hud_tests {
    use super::*;
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::{
        GameLogic, KindOf, Player, SpecialPowerModuleKind, SpecialPowerModuleMetadata, Team,
        ThingTemplate,
    };
    fn sw_module(
        template: &str,
        power: SpecialPowerType,
        shared: bool,
    ) -> SpecialPowerModuleMetadata {
        SpecialPowerModuleMetadata {
            source_index: 0,
            module_tag: Some("ModuleTag_SpecialPower".into()),
            module_kind: SpecialPowerModuleKind::OclSpecialPower,
            special_power_template: template.into(),
            special_power_template_id: 1,
            command_power: Some(power),
            reload_time_frames: 9000,
            required_science: None,
            public_timer: true,
            shared_n_sync: shared,
            shortcut_power: false,
            update_module_starts_attack: false,
            starts_paused: false,
            scripted_special_power_only: false,
        }
    }

    fn spawn_sw(
        logic: &mut GameLogic,
        name: &str,
        owner: u32,
        power: SpecialPowerType,
        shared: bool,
        object_remaining: f32,
    ) -> crate::game_logic::ObjectId {
        let mut tpl = ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::FSSuperweapon)
            .set_health(4000.0);
        tpl.special_power_modules.push(sw_module(
            &format!("Superweapon{name}"),
            power.clone(),
            shared,
        ));
        logic.templates.insert(name.into(), tpl);
        let id = logic
            .create_object_for_player(name, owner, glam::Vec3::ZERO)
            .expect("sw");
        if let Some(o) = logic.host_object_mut(id) {
            o.status.under_construction = false;
            o.construction_percent = 1.0;
            o.special_power_cooldowns.insert(power, object_remaining);
            o.special_power_cooldown_remaining = object_remaining;
            o.special_power_ready = object_remaining <= 0.0;
        }
        id
    }

    #[test]
    fn superweapon_strip_iterates_all_players_with_enemy_color() {
        // C++ InGameUI.cpp:3503 iterates MAX_PLAYER_COUNT so enemy rows
        // draw in SuperweaponInfo.m_color (player color).
        let mut logic = GameLogic::new();
        let mut local = Player::new(0, Team::USA, "USA", true);
        local.alliance_team = 0;
        local.color_rgb = (0x22, 0x44, 0xCC);
        local.apply_faction_intrinsic_sciences();
        logic.add_player(local);
        let mut enemy = Player::new(1, Team::China, "China", false);
        enemy.alliance_team = 1;
        enemy.color_rgb = (0xCC, 0x22, 0x22);
        enemy.apply_faction_intrinsic_sciences();
        logic.add_player(enemy);

        spawn_sw(
            &mut logic,
            "LocalPuc",
            0,
            SpecialPowerType::ParticleCannon,
            false,
            30.0,
        );
        spawn_sw(
            &mut logic,
            "EnemyNuke",
            1,
            SpecialPowerType::NuclearMissile,
            false,
            90.0,
        );

        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let local_row = frame
            .superweapon_timers
            .iter()
            .find(|t| t.name.contains("Particle"))
            .expect("local PUC row");
        assert!(
            !local_row.name.contains("Enemy"),
            "local row must stay unadorned: {}",
            local_row.name
        );
        let enemy_row = frame
            .superweapon_timers
            .iter()
            .find(|t| t.name.contains("Nuclear") || t.name.contains("Enemy"))
            .expect("enemy nuke row");
        assert!(
            enemy_row.name.contains("Enemy"),
            "enemy row {}",
            enemy_row.name
        );
        assert!(
            enemy_row.name.contains("#CC2222"),
            "player color on enemy row {}",
            enemy_row.name
        );
        assert_eq!(enemy_row.power_key, "NuclearMissile#1");
        assert!(
            (enemy_row.remaining - 90.0).abs() < 0.5,
            "enemy remaining {}",
            enemy_row.remaining
        );
    }

    #[test]
    fn shared_nsync_uses_player_shared_ready_frame_not_object_min() {
        // C++ SpecialPowerModule::getReadyFrame (SpecialPowerModule.cpp:756-765)
        // returns Player::getOrStartSpecialPowerReadyFrame when SharedNSync.
        // Two CCs with 10s/90s object timers must not min() to 10s.
        let mut logic = GameLogic::new();
        let mut p = Player::new(0, Team::USA, "USA", true);
        p.apply_faction_intrinsic_sciences();
        p.shared_special_power_cooldowns
            .insert(SpecialPowerType::ParticleCannon, 45.0);
        logic.add_player(p);

        spawn_sw(
            &mut logic,
            "PucA",
            0,
            SpecialPowerType::ParticleCannon,
            true,
            10.0,
        );
        spawn_sw(
            &mut logic,
            "PucB",
            0,
            SpecialPowerType::ParticleCannon,
            true,
            90.0,
        );

        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let rows: Vec<_> = frame
            .superweapon_timers
            .iter()
            .filter(|t| t.name.contains("Particle"))
            .collect();
        assert_eq!(
            rows.len(),
            1,
            "SharedNSync is one row per player (InGameUI.cpp:3684): {:?}",
            rows.iter().map(|t| &t.name).collect::<Vec<_>>()
        );
        assert!(
            (rows[0].remaining - 45.0).abs() < 0.01,
            "shared ready frame 45, not object min 10: {}",
            rows[0].remaining
        );
        assert!(!rows[0].ready);
    }

    #[test]
    fn two_nonshared_pucs_emit_two_rows_not_min_merge() {
        let mut logic = GameLogic::new();
        let mut p = Player::new(0, Team::USA, "USA", true);
        p.apply_faction_intrinsic_sciences();
        logic.add_player(p);
        spawn_sw(
            &mut logic,
            "PucA",
            0,
            SpecialPowerType::ParticleCannon,
            false,
            10.0,
        );
        spawn_sw(
            &mut logic,
            "PucB",
            0,
            SpecialPowerType::ParticleCannon,
            false,
            90.0,
        );
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let mut rem: Vec<f32> = frame
            .superweapon_timers
            .iter()
            .filter(|t| t.name.contains("Particle"))
            .map(|t| t.remaining)
            .collect();
        rem.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(rem.len(), 2, "C++ SuperweaponInfo is per-object");
        assert!((rem[0] - 10.0).abs() < 0.01, "{rem:?}");
        assert!((rem[1] - 90.0).abs() < 0.01, "{rem:?}");
    }

    #[test]
    fn script_hide_and_uc_skip_sold_keeps_row() {
        let mut logic = GameLogic::new();
        let mut p = Player::new(0, Team::USA, "USA", true);
        p.apply_faction_intrinsic_sciences();
        logic.add_player(p);
        let ready = spawn_sw(
            &mut logic,
            "PucReady",
            0,
            SpecialPowerType::ParticleCannon,
            false,
            0.0,
        );
        let hidden = spawn_sw(
            &mut logic,
            "PucHidden",
            0,
            SpecialPowerType::ParticleCannon,
            false,
            20.0,
        );
        let uc = spawn_sw(
            &mut logic,
            "PucUc",
            0,
            SpecialPowerType::ParticleCannon,
            false,
            0.0,
        );
        if let Some(o) = logic.host_object_mut(uc) {
            o.status.under_construction = true;
            o.construction_percent = 0.4;
        }
        if let Some(o) = logic.host_object_mut(ready) {
            o.status.sold = true;
            o.construction_percent = 0.999;
            o.status.under_construction = false;
        }
        logic.hide_script_superweapon_object_for_test(hidden);

        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let rows: Vec<_> = frame
            .superweapon_timers
            .iter()
            .filter(|t| t.name.contains("Particle"))
            .collect();
        assert_eq!(
            rows.len(),
            1,
            "UC skipped, hidden skipped, sold kept: {:?}",
            rows.iter().map(|t| t.remaining).collect::<Vec<_>>()
        );
        assert!(
            (rows[0].remaining - 0.0).abs() < 0.01,
            "sold ready PUC stays until destroy"
        );
        assert!(
            rows[0].ready,
            "C++ HUD keeps sold SuperweaponInfo; Player sold skip is fire only"
        );

        logic.set_script_superweapon_display_enabled_for_test(false);
        let hidden_frame = PresentationFrame::build_from_logic(&logic, 0);
        assert!(
            hidden_frame.superweapon_timers.is_empty(),
            "HideSuperweaponDisplay emits no strip"
        );
    }

    #[test]
    fn disabled_ready_puc_is_not_bold() {
        let mut logic = GameLogic::new();
        let mut p = Player::new(0, Team::USA, "USA", true);
        p.apply_faction_intrinsic_sciences();
        logic.add_player(p);
        let id = spawn_sw(
            &mut logic,
            "PucBrownout",
            0,
            SpecialPowerType::ParticleCannon,
            false,
            0.0,
        );
        if let Some(o) = logic.host_object_mut(id) {
            o.status.disabled_underpowered = true;
        }
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let row = frame
            .superweapon_timers
            .iter()
            .find(|t| t.name.contains("Particle"))
            .expect("row");
        assert!(row.remaining <= 0.0);
        assert!(!row.ready, "brownout ready PUC is 0:00 not bold");
    }
}
