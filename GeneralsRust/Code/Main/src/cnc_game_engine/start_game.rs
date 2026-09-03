#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::mouse::{
    PATHFIND_CELL_SIZE_F, SHAKE_AXIS_PITCH, SHAKE_AXIS_ROLL, SHAKE_AXIS_YAW, SHAKE_END_OMEGA,
    SHAKE_MAX_OMEGA, SHAKE_MIN_OMEGA, airborne_look_at_ground, w3d_camera_constraint_offset,
};
use super::*;
use crate::graphics::render_pipeline::CachedLighting;
use crate::presentation_frame::PresentationWorldEnv;

/// C++ `W3DView.cpp:3097-3212` — union a scripted look into `m_cameraConstraint`.
/// Live stores `(lo_x, hi_x, lo_z, hi_z)` in Y-up (C++ Y → live Z).
fn widen_scripted_camera_constraint(
    current: Option<(f32, f32, f32, f32)>,
    look_x: f32,
    look_z: f32,
) -> (f32, f32, f32, f32) {
    match current {
        Some((lo_x, hi_x, lo_z, hi_z)) => (
            lo_x.min(look_x),
            hi_x.max(look_x),
            lo_z.min(look_z),
            hi_z.max(look_z),
        ),
        None => (look_x, look_x, look_z, look_z),
    }
}

fn apply_scripted_camera_constraint_widen(
    lo_x: f32,
    hi_x: f32,
    lo_z: f32,
    hi_z: f32,
    widen: Option<(f32, f32, f32, f32)>,
) -> (f32, f32, f32, f32) {
    let Some((wlo_x, whi_x, wlo_z, whi_z)) = widen else {
        return (lo_x, hi_x, lo_z, hi_z);
    };
    (
        lo_x.min(wlo_x),
        hi_x.max(whi_x),
        lo_z.min(wlo_z),
        hi_z.max(whi_z),
    )
}

fn shaker_hash_signed(seed: u32, elapsed_bits: u32, axis: u32, pass: u32) -> f32 {
    let mut x = seed
        ^ elapsed_bits.wrapping_mul(0x9E37_79B9)
        ^ axis.wrapping_mul(0x85EB_CA6B)
        ^ pass.wrapping_mul(0xC2B2_AE35);
    x ^= x >> 16;
    x = x.wrapping_mul(0x7FEB_352D);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846C_A68B);
    x ^= x >> 16;
    let unit = (x >> 8) as f32 / ((1u32 << 24) as f32);
    unit * 2.0 - 1.0
}

fn script_camera_shaker_rotations(shaker: &ScriptCameraShaker, camera_position: Vec3) -> Vec3 {
    // C++ CameraShakerClass::Compute_Rotations: 3D eye-to-epicenter falloff,
    // omega(t)=omega+(END_OMEGA-omega)*elapsed, plus ±0.5*intensity fudge
    // accumulated three times (once per axis loop).
    let offset = camera_position - shaker.epicenter;
    let dist_sq = offset.length_squared();
    if dist_sq > shaker.radius * shaker.radius {
        return Vec3::ZERO;
    }
    let dist = dist_sq.sqrt();
    let life = (1.0 - shaker.elapsed_seconds / shaker.duration_seconds).clamp(0.0, 1.0);
    let intensity = shaker.intensity * (1.0 - dist / shaker.radius) * life;
    if intensity <= f32::EPSILON {
        return Vec3::ZERO;
    }
    let t = shaker.elapsed_seconds.max(0.0);
    let elapsed_bits = t.to_bits();
    let axis = [SHAKE_AXIS_PITCH, SHAKE_AXIS_YAW, SHAKE_AXIS_ROLL];
    let mut angles = Vec3::ZERO;
    for i in 0..3 {
        let omega = shaker.omega[i] + (SHAKE_END_OMEGA - shaker.omega[i]) * t;
        angles[i] += axis[i] * intensity * (omega * t + shaker.phi[i]).sin();
        let minor = intensity * 0.5;
        angles.x += shaker_hash_signed(shaker.rng_seed, elapsed_bits, i as u32, 0) * minor;
        angles.y += shaker_hash_signed(shaker.rng_seed, elapsed_bits, i as u32, 1) * minor;
        angles.z += shaker_hash_signed(shaker.rng_seed, elapsed_bits, i as u32, 2) * minor;
    }
    angles
}

/// Object-scene and terrain lighting resolved at the presentation/map
/// activation boundary. They are intentionally independent: C++ selects
/// `TerrainObjectsLighting[tod][0]` for W3D scene lights and
/// `TerrainLighting[tod][0]` for TerrainVisual.
#[derive(Debug, Clone, Default)]
struct MapActivationLighting {
    object: Option<CachedLighting>,
    terrain: Option<CachedLighting>,
    sky_color: Option<[f32; 3]>,
}

fn has_explicit_map_lighting(env: &PresentationWorldEnv) -> bool {
    env.sun_direction.is_some()
        || env.sun_color.is_some()
        || env.ambient_color.is_some()
        || env.fog_color.is_some()
        || env.fog_start.zip(env.fog_end).is_some()
}

/// Resolve the frozen map frame without deriving lighting from map names or a
/// guessed time of day. Explicit `MapMetadata` values override only their own
/// channels; absent channels fall through to the authored GameData primary
/// record. With neither source present we leave the existing renderer state
/// intact rather than introducing a synthetic light.
fn resolve_map_activation_lighting(env: Option<&PresentationWorldEnv>) -> MapActivationLighting {
    let Some(env) = env else {
        return MapActivationLighting::default();
    };

    let map_has_lighting = has_explicit_map_lighting(env);
    let fog_color = env.fog_color;
    let fog_range = env.fog_start.zip(env.fog_end);

    // NumberGlobalLights controls the W3D object-scene light list. If it is
    // zero, retain Main's existing forward/Graphics lighting instead of
    // constructing an artificial zero-direction scene light.
    let object_primary = env
        .primary_object_lighting
        .filter(|lighting| lighting.object_light_active);
    let object = (map_has_lighting || object_primary.is_some()).then(|| CachedLighting {
        sun_direction: env
            .sun_direction
            .or_else(|| object_primary.map(|lighting| lighting.render_light_pos())),
        sun_color: env
            .sun_color
            .or_else(|| object_primary.map(|lighting| lighting.diffuse)),
        ambient_color: env
            .ambient_color
            .or_else(|| object_primary.map(|lighting| lighting.ambient)),
        fog_color,
        fog_range,
        fogged_light_fraction: Some(env.fogged_light_fraction()),
    });

    // TerrainVisual follows the authored terrain index-zero record directly,
    // independently of whether a W3D object directional light is enabled.
    let terrain_primary = env.primary_terrain_lighting;
    let terrain = (map_has_lighting || terrain_primary.is_some()).then(|| CachedLighting {
        sun_direction: env
            .sun_direction
            .or_else(|| terrain_primary.map(|lighting| lighting.render_light_pos())),
        sun_color: env
            .sun_color
            .or_else(|| terrain_primary.map(|lighting| lighting.diffuse)),
        ambient_color: env
            .ambient_color
            .or_else(|| terrain_primary.map(|lighting| lighting.ambient)),
        fog_color,
        fog_range,
        fogged_light_fraction: Some(env.fogged_light_fraction()),
    });

    MapActivationLighting {
        object,
        terrain,
        // Retain the existing GraphicsSystem sky residual only when supplied
        // explicitly by map metadata; GameData lighting has no sky fallback.
        sky_color: env.fog_color,
    }
}

/// C++ `InGameUI.cpp:77` `placementOpacity`.
const STRUCTURE_PLACEMENT_GHOST_OPACITY: f32 = 0.45;
/// C++ `InGameUI.cpp:78` `illegalBuildColor`.
const STRUCTURE_PLACEMENT_ILLEGAL_TINT: [f32; 3] = [1.0, 0.0, 0.0];
/// Standalone client DrawableID for the live host place-icon (not an ObjectID).
const STRUCTURE_PLACEMENT_GHOST_DRAWABLE_ID: u32 = 0x504C_4143;

/// C++ `Drawable::colorTint`: red only when illegal; legal is untinted.
fn structure_placement_ghost_tint(illegal: bool) -> Option<[f32; 3]> {
    illegal.then_some(STRUCTURE_PLACEMENT_ILLEGAL_TINT)
}

/// Same hull matrix as `UnitRenderInput::world_matrix` (Y-up + yaw).
fn structure_placement_model_transform(
    world_x: f32,
    world_y: f32,
    world_z: f32,
    facing_radians: f32,
) -> Mat4 {
    Mat4::from_translation(Vec3::new(world_x, world_y, world_z))
        * Mat4::from_rotation_y(facing_radians)
}

/// C++ `addFactionBibDrawable` local corners are ground-plane (x, y, 0).
/// TerrainVisual overlay uploads `[corner.x, height, corner.z]`, so map C++ Y → world Z.
fn structure_placement_bib_transform(world_x: f32, world_z: f32, facing_radians: f32) -> Mat4 {
    let c = facing_radians.cos();
    let s = facing_radians.sin();
    Mat4::from_cols(
        glam::Vec4::new(c, 0.0, s, 0.0),
        glam::Vec4::new(-s, 0.0, c, 0.0),
        glam::Vec4::new(0.0, 1.0, 0.0, 0.0),
        glam::Vec4::new(world_x, 0.0, world_z, 1.0),
    )
}

/// Authored W3D key for the building being placed (never a footprint circle).
fn structure_placement_model_key(template_name: &str) -> String {
    let key = crate::assets::mesh_asset_resolve::drawable_w3d_model_key(template_name);
    if key.is_empty() {
        template_name.trim().to_string()
    } else {
        key
    }
}

impl CnCGameEngine {
    pub(super) fn play_ui_sound_effect(&mut self, path: String) {
        let Some(bytes) = self.ui_sound_cache.get(&path).cloned() else {
            return;
        };
        let Some(handle) = self.audio_handle.as_ref() else {
            return;
        };
        let Ok(sink) = Sink::try_new(handle) else {
            return;
        };

        let cursor = std::io::Cursor::new(bytes);
        let Ok(decoder) = rodio::Decoder::new(cursor) else {
            return;
        };
        let source = decoder.convert_samples::<f32>();
        sink.append(source);
        self.sound_effects.push(sink);
    }

    #[inline]
    pub(super) fn map_name_is_shell_residual(map: &str) -> bool {
        let t = map.trim();
        if t.is_empty() {
            return true;
        }
        let lower = t.to_ascii_lowercase();
        lower.contains("shellmap")
            || lower == "default map"
            || lower.ends_with("shellmapmd.map")
            || lower.contains("\\shellmapmd\\")
            || lower.contains("/shellmapmd/")
    }

    /// Wave 840: skirmish start must not keep boot ShellMapMD when a real control/
    /// setup map is available (or when empty → DEFAULT_SKIRMISH_MAP).
    pub(super) fn resolve_skirmish_start_map_name(mode: GameMode, map: String) -> String {
        let mut map_name = if map.trim().is_empty() {
            DEFAULT_SKIRMISH_MAP.to_string()
        } else {
            map
        };
        if mode != GameMode::Skirmish {
            return map_name;
        }
        if !Self::map_name_is_shell_residual(&map_name) {
            return map_name;
        }
        #[cfg(feature = "game_client")]
        {
            let setup = game_client::gui::get_skirmish_setup();
            let selected = setup.selected_map().trim().to_string();
            if !Self::map_name_is_shell_residual(&selected) {
                return selected;
            }
            let info_map = setup.game_info().game_info().get_map().trim().to_string();
            if !Self::map_name_is_shell_residual(&info_map) {
                return info_map;
            }
        }
        // Last resort: still refuse shell residual for skirmish authority start.
        if Self::map_name_is_shell_residual(&map_name) {
            warn!(
                "Wave 840: rejecting shell residual map '{}' for skirmish start; using {}",
                map_name, DEFAULT_SKIRMISH_MAP
            );
            return DEFAULT_SKIRMISH_MAP.to_string();
        }
        map_name
    }

    /// Restart the simulation with UI-selected parameters and refresh view/minimap.
    /// Wave 611: via `host_start_game_from_ui`.
    pub(super) fn start_game_from_ui(&mut self, request: HostStartRequest) {
        // Wave 611: thin wrapper — residual via host helper.
        self.host_start_game_from_ui(request)
    }

    /// Restart the simulation with UI-selected parameters and refresh view/minimap.
    pub(super) fn host_start_game_from_ui(&mut self, request: HostStartRequest) {
        let HostStartRequest {
            mode,
            faction,
            map,
            skirmish,
            player_template,
        } = request;

        // C++ MSG_NEW_GAME GAME_SHELL is Shell::showShellMap, not a match start.
        // Loading/headless/runtime-host drains that treat it as UI start load
        // Defcon6 and abandon the ShellMapMD worker.
        if matches!(mode, GameMode::Shell) {
            info!("host_start_game_from_ui: ignore GAME_SHELL (shell map, not a match)");
            return;
        }

        // A typed shell descriptor is only meaningful for C++ single-player
        // Campaign/Challenge GameInfo.  Do not let an accidental direct
        // skirmish/runtime call attach it to a different start path.
        if player_template.is_some() && !matches!(mode, GameMode::SinglePlayer) {
            warn!(
                "Rejecting PlayerTemplate identity on non-single-player start mode {:?}",
                mode
            );
            return;
        }

        let faction_team = Self::team_from_faction(&faction);
        if let Some(player_template) = player_template.as_ref() {
            if player_template.base_team() != Some(faction_team) {
                warn!(
                    "Rejecting PlayerTemplate '{}' whose exact base side no longer matches requested team {:?}",
                    player_template.template_name, faction_team
                );
                return;
            }
        }

        // Capture the provenance before this function moves Menu → Loading.  The
        // flag may only be completed after the normal map/bootstrap path reaches
        // InGame; a runtime-host `start_game` command has no physical WND click and
        // therefore cannot satisfy it.
        let interactive_start_from_menu = self.current_state == GameState::Menu;
        // Offline provenance is applied when the parked start reaches InGame.
        // Wave 611: host residual helper.
        // A still-running boot worker must not overwrite this match start (or
        // keep finalize_startup_map_load pointed at Menu) after we leave Loading.
        self.abandon_startup_load_worker();
        info!("host_start_game_from_ui: abandoned boot worker");
        // Mark MainMenu as starting a match so shutdown skips reverse-animate
        // (windowed host start was SIGSEGV'ing mid MainMenuShutdown / hide_shell).
        #[cfg(feature = "game_client")]
        game_client::gui::mark_host_match_start();
        // Show loading screen before starting map load (matches C++ loading screen flow)
        #[cfg(feature = "game_client")]
        self.prepare_cpp_load_screen_for_mode(mode, false);
        info!("host_start_game_from_ui: load screen prepared");
        self.transition_to_state(GameState::Loading);
        info!("host_start_game_from_ui: state=Loading");
        // C++ SinglePlayerLoadScreen/ChallengeLoadScreen consume their
        // authored prelude before session and map work.  This is a direct
        // WindowManager/display pump, not a re-entrant host event dispatch.
        #[cfg(feature = "game_client")]
        self.run_cpp_load_screen_prelude();
        // Park the blocking map load for the next Loading tick. Runtime-host
        // publishes status after each command (run_loop.rs); returning here
        // lets smoke observe `state=Loading` instead of remaining on Menu while
        // `host_load_map_or_default` / load_map runs. C++ `GameLogic::startNewGame`
        // also shows the load screen before `loadMap`. The next Loading tick
        // still calls `host_load_map_or_default` then
        // `seed_presentation_after_match_start` (no get_difficulty dual-read).
        // This does not skip load_map.
        self.pending_match_start = Some(PendingMatchStart {
            request: HostStartRequest {
                mode,
                faction,
                map,
                skirmish,
                player_template,
            },
            interactive_start_from_menu,
        });
        info!("host_start_game_from_ui: parked match start for next Loading tick");
    }

    /// Finish a parked UI start: `host_load_map_or_default` then InGame.
    ///
    /// Called from the Loading tick after status has been published. Still
    /// the live `load_map` path (hq-ibnf owns making Lone Eagle return).
    pub(super) fn complete_parked_match_start(&mut self, pending: PendingMatchStart) {
        let HostStartRequest {
            mode,
            faction,
            map,
            skirmish,
            player_template,
        } = pending.request;
        let interactive_start_from_menu = pending.interactive_start_from_menu;
        let offline_mode = matches!(mode, GameMode::SinglePlayer | GameMode::Skirmish);
        let faction_team = Self::team_from_faction(&faction);

        // Wave 842: retain the selected host-owned match mode through map load
        // / presentation seed.
        // The load boundary deliberately clears every old-world residual, so
        // the selected mode is stamped only after that boundary succeeds.
        // C++ GameLogicDispatch.cpp:256 TheGameEngine->reset() before startNewGame.
        self.host_game_engine_reset();
        // Wave 843/844/871: clear prior match residuals until load completes.
        self.host_clear_match_residuals();
        info!("host_start_game_from_ui: match residuals cleared");

        // Wave 169/840: empty UI map → DEFAULT_SKIRMISH_MAP (Defcon6) before
        // shell-residual rejection. Matches C++ startNewGame default map residual.
        let map = if map.trim().is_empty() {
            DEFAULT_SKIRMISH_MAP.to_string()
        } else {
            map
        };
        // Wave 840: never start skirmish on boot ShellMapMD residual when a real map exists.
        let map_name = Self::resolve_skirmish_start_map_name(mode, map);

        info!(
            "UI requested start: mode={:?}, faction={}, map={}, skirmish_slots={}",
            mode,
            faction_team.get_name(),
            map_name,
            skirmish
                .as_ref()
                .map(|c| c.slots.iter().filter(|s| s.is_active).count())
                .unwrap_or(0)
        );

        if mode == GameMode::Skirmish {
            if let Some(ref config) = skirmish {
                if let Err(e) = self.host_apply_skirmish_config_authority(config) {
                    // A supplied C++ GameInfo-derived config has selected
                    // exact PlayerTemplate identities. Never substitute a
                    // Team-only legacy start when one is stale/locked: that
                    // would launch a plausible but wrong General match.
                    warn!("Rejecting configured Skirmish start before map load: {e}");
                    self.return_to_main_menu_after_match();
                    return;
                } else if let Some(human) = config.slots.iter().find(|s| s.is_human && s.is_active)
                {
                    self.current_player_id = human.slot_index as u32;
                }
            } else {
                // Wave 577: host start residual via helper.
                info!("host_start_game_from_ui: host_start_new_game_with_faction(skirmish)");
                self.host_start_new_game_with_faction(mode, faction_team, true);
            }
        } else {
            match player_template {
                Some(player_template) => {
                    // Campaign/Challenge starts rebuild C++ SidesList from
                    // its selected PlayerTemplate; they must not reuse a
                    // non-zero human slot left by a prior skirmish match.
                    // Main's fresh single-player bootstrap owns local player
                    // zero, which is the player `start_new_game` creates
                    // before the exact template is bound.
                    self.current_player_id = 0;
                    info!(
                        "host_start_game_from_ui: exact PlayerTemplate start '{}'",
                        player_template.template_name
                    );
                    if !self.host_start_new_game_with_player_template(
                        mode,
                        faction_team,
                        player_template,
                    ) {
                        // C++ never substitutes a base Team after a selected
                        // GameSlot PlayerTemplate fails to resolve.  Return to
                        // the shell before map load so Main cannot create a
                        // plausible-looking but wrong General match.
                        self.return_to_main_menu_after_match();
                        return;
                    }
                }
                None => {
                    // Wave 577: host start residual via helper (non-skirmish).
                    info!(
                        "host_start_game_from_ui: host_start_new_game_with_faction(non-skirmish)"
                    );
                    self.host_start_new_game_with_faction(mode, faction_team, false);
                }
            }
        }
        info!("host_start_game_from_ui: new game started, loading requested map={map_name}");

        // A selected map may be absent or corrupt. Only continue when either
        // that map or the explicit default reached GameLogic's successful load
        // tail; otherwise a Loading screen must return to Menu instead of
        // entering an empty/stale world as the requested map.
        let Some(loaded_map_name) = self.host_load_map_or_default(&map_name) else {
            error!(
                "Cannot start {:?}: neither requested map '{}' nor fallback '{}' loaded",
                mode, map_name, DEFAULT_SKIRMISH_MAP
            );
            self.return_to_main_menu_after_match();
            return;
        };
        info!(
            "host_start_game_from_ui: map load done (requested='{}', loaded='{}')",
            map_name, loaded_map_name
        );
        // `host_load_map_or_default` clears stale match-owned residuals before
        // installing a world.  A selected mode is authoritative only after a
        // successful load; failed starts return to Menu with no stale offline
        // mode that could make physical-evidence gates eligible.
        self.host_match_game_mode = Some(mode);
        // Wave 840: drop shell presentation freeze so match seed cannot keep ShellMapMD.
        self.render_pipeline.set_presentation_frame(None);
        self.last_presentation_frame = None;
        // Wave 843/844: host-owned match residuals for presentation_or_boot peels.
        self.host_match_map_name = Some(loaded_map_name.clone());
        self.host_match_local_player_id = Some(self.current_player_id);
        // Wave 907: AI difficulty residual stays cold until presentation seed stamps it
        // (no get_difficulty dual-read). Fail-closed default covers interim probes.
        self.host_refresh_match_sim_residuals_from_logic();

        // Reset transient state.
        self.host_set_paused(false);
        self.match_over = false;
        self.victory_summary = None;
        self.selected_objects.clear();

        // Dual-tick residual close: map load → presentation seed → InGame HUD/units
        // without waiting for the first logic frame (render collect uses snapshot IDs).
        // Leave Loading BEFORE heightmap/minimap GPU. Windowed sit-through was
        // stuck in Loading because terrain visual never returned.
        info!("host_start_game_from_ui: seeding presentation");
        self.seed_presentation_after_match_start();
        self.snap_camera_to_local_units_if_needed();
        info!("host_start_game_from_ui: transition to InGame");
        self.transition_to_state(GameState::InGame);
        info!(
            "host_start_game_from_ui: pre-note menu_click={} was_menu={} offline={}",
            self.interactive_playability.menu_wnd_click, interactive_start_from_menu, offline_mode
        );
        self.interactive_playability
            .note_offline_match_started(interactive_start_from_menu, offline_mode);
        info!(
            "host_start_game_from_ui: InGame menu_match={} menu_click={}",
            self.interactive_playability.match_started_from_menu_wnd,
            self.interactive_playability.menu_wnd_click
        );

        // Update minimap/world bounds and camera to the new map (post-InGame).
        // Wave 455: seed presentation env then apply presentation-only heightmap/skybox hints.
        self.ensure_presentation_env_seeded();
        Self::apply_heightmap_hint(&mut self.render_pipeline);
        Self::apply_skybox_hint(&mut self.render_pipeline);
        self.ensure_presentation_env_seeded();
        Self::sync_render_terrain_visual(
            &mut self.render_pipeline,
            &self.graphics_system,
            loaded_map_name.as_str(),
        );
        if let Err(err) = self.reinitialize_minimap_renderer() {
            warn!("Failed to reinitialize minimap renderer: {}", err);
        }

        // Apply map lighting if provided by map settings.
        self.ensure_presentation_env_seeded();
        Self::apply_map_lighting(&mut self.graphics_system, &mut self.render_pipeline);

        let startup_camera_defaults = Self::configured_startup_camera_defaults();
        // Wave 458: prefer pipeline presentation freeze; live GameLogic only if missing.
        let startup_camera_presentation = self
            .render_pipeline
            .presentation_frame()
            .or(self.last_presentation_frame.as_ref());
        // Wave 540/552: prefer presentation fow_shell_bypass when freeze present.
        let in_shell_camera = self.shell_bypass_from_presentation(startup_camera_presentation);
        (self.camera_target, self.camera_position, self.camera_zoom) =
            Self::bootstrap_camera_for_loaded_map(
                in_shell_camera,
                self.current_player_id,
                startup_camera_defaults,
                startup_camera_presentation,
            );
        self.sync_orbit_from_camera_transform();
        self.snap_camera_to_local_units_if_needed();
    }

    /// Prefer a local base focus for the match camera when bootstrap aim is far
    /// from the human force (common Lone Eagle residual).
    ///
    /// Do **not** average every local object — map-wide centroid pulls the camera
    /// between bases and frustum-culls everything.
    ///
    /// C++ `LookAtXlat` / `W3DView::scrollBy` never snap back to own units while
    /// the player pans. Call this only at match start / first-frame origin hitch,
    /// never from `update_camera` during play.
    pub(super) fn snap_camera_to_local_units_if_needed(&mut self) {
        // Presentation-only: compute focus from snapshot, then apply camera mutably.
        let Some(focus) = (|| {
            let frame = self.last_presentation_frame.as_ref()?;
            let team = frame.local_team();
            let start_hint = frame
                .local_team_base_position
                .or_else(|| {
                    frame
                        .objects
                        .iter()
                        .filter(|o| o.team == team && !o.destroyed && o.is_structure)
                        .map(|o| o.position)
                        .next()
                })
                .or_else(|| {
                    let mut sum = Vec3::ZERO;
                    let mut n = 0u32;
                    for o in &frame.objects {
                        if o.team == team && !o.destroyed && o.is_structure {
                            sum += o.position;
                            n += 1;
                            if n >= 8 {
                                break;
                            }
                        }
                    }
                    (n > 0).then_some(sum / n as f32)
                })
                .unwrap_or(self.camera_target);

            let mut command_center: Option<Vec3> = None;
            let mut nearest_structure: Option<(f32, Vec3)> = None;
            let mut nearest_mobile: Option<(f32, Vec3)> = None;
            let mut structure_sum = Vec3::ZERO;
            let mut structure_n = 0u32;

            use crate::presentation_frame::PresentationBuildingType;
            for o in &frame.objects {
                if o.team != team || o.destroyed {
                    continue;
                }
                let pos = o.position;
                let d2 = {
                    let dx = pos.x - start_hint.x;
                    let dz = pos.z - start_hint.z;
                    dx * dx + dz * dz
                };
                if o.is_structure {
                    structure_sum += pos;
                    structure_n += 1;
                    let is_cc = matches!(
                        o.building_type,
                        Some(PresentationBuildingType::CommandCenter)
                    ) || {
                        let name = o.template_name.to_ascii_lowercase();
                        name.contains("commandcenter") || name.contains("command_center")
                    };
                    if is_cc {
                        match command_center {
                            None => command_center = Some(pos),
                            Some(prev) => {
                                let pdx = prev.x - start_hint.x;
                                let pdz = prev.z - start_hint.z;
                                if d2 < pdx * pdx + pdz * pdz {
                                    command_center = Some(pos);
                                }
                            }
                        }
                    }
                    nearest_structure = Some(match nearest_structure {
                        None => (d2, pos),
                        Some((best, _)) if d2 < best => (d2, pos),
                        Some(other) => other,
                    });
                } else if o.is_mobile || o.is_unit {
                    nearest_mobile = Some(match nearest_mobile {
                        None => (d2, pos),
                        Some((best, _)) if d2 < best => (d2, pos),
                        Some(other) => other,
                    });
                }
            }

            command_center
                .or_else(|| nearest_structure.map(|(_, p)| p))
                .or_else(|| {
                    if structure_n > 0 {
                        Some(structure_sum / structure_n as f32)
                    } else {
                        None
                    }
                })
                .or_else(|| nearest_mobile.map(|(_, p)| p))
        })() else {
            return;
        };

        let dx = focus.x - self.camera_target.x;
        let dz = focus.z - self.camera_target.z;
        let dist_sq = dx * dx + dz * dz;
        let looking_at_boot_origin = {
            let tx = self.camera_target.x;
            let tz = self.camera_target.z;
            tx * tx + tz * tz < 80.0 * 80.0
        };
        let focus_is_real_base = {
            let fx = focus.x;
            let fz = focus.z;
            fx * fx + fz * fz > 200.0 * 200.0
        };
        // If already aimed near the local base, keep bootstrap (C++ InitialCamera).
        // Boot camera sits at the origin; never treat that as "already on the base".
        if dist_sq < 250.0 * 250.0 && !(looking_at_boot_origin && focus_is_real_base) {
            // Still force a sane orbit so frustum is not degenerate after shell→match.
            if self.camera_orbit_distance < 40.0 || self.camera_orbit_distance > 800.0 {
                self.camera_orbit_distance = 280.0;
                self.camera_pitch_radians = 35.0_f32.to_radians();
                self.camera_yaw_radians = 0.0;
                self.apply_camera_orbit_transform();
            }
            return;
        }

        self.camera_target = Vec3::new(focus.x, focus.y, focus.z);
        // Retail-ish elevated look: fixed orbit, not a one-off look_at that
        // update_camera immediately overwrites with a broken pitch/distance.
        self.camera_orbit_distance = 320.0;
        self.camera_pitch_radians = 38.0_f32.to_radians();
        self.camera_yaw_radians = 0.35; // slight yaw so bases aren't edge-on
        self.camera_zoom = 1.0;
        self.apply_camera_orbit_transform();
    }

    pub(super) fn apply_map_lighting(
        graphics_system: &mut GraphicsSystem,
        render_pipeline: &mut RenderPipeline,
    ) {
        // Wave 456 / hq-0za: presentation-only map lighting (no live
        // GameLogic dual-read). GameData index-zero object and terrain values
        // were frozen into this frame at map activation.
        let env = render_pipeline.presentation_frame().map(|p| &p.world_env);
        let resolved = resolve_map_activation_lighting(env);

        if let Some(env) = env {
            info!(
                "Applying frozen map/GameData lighting: map_metadata={} object_primary={} terrain_primary={} object={:?} terrain={:?}",
                env.has_map_metadata,
                env.primary_object_lighting.is_some(),
                env.primary_terrain_lighting.is_some(),
                resolved.object,
                resolved.terrain,
            );
        }

        if resolved.object.is_none() && resolved.terrain.is_none() {
            warn!(
                "Presentation env has no authored GameData or map lighting; preserving existing renderer lighting"
            );
            return;
        }

        render_pipeline
            .set_environment_lighting_with_terrain(resolved.object.clone(), resolved.terrain);
        if let Some(object) = resolved.object {
            graphics_system.set_lighting(
                object.ambient_color,
                object.sun_color,
                object.sun_direction,
                resolved.sky_color,
            );
        }
    }

    /// Wave 467: seed pipeline presentation (host+GW) and mirror into last_presentation_frame
    pub(super) fn ensure_presentation_env_seeded(&mut self) {
        // Wave 467/474: seed pipeline presentation (host+GW) and mirror into last_presentation_frame
        self.ensure_presentation_env_for_hints();
        if self.last_presentation_frame.is_none() {
            self.last_presentation_frame = self.render_pipeline.presentation_frame().cloned();
        }
    }

    pub(super) fn ensure_presentation_env_for_hints(&mut self) {
        // Wave 474: instance seed only — no free-fn GameLogic dual-read surface.
        // Wave 474/466/455/590: pipeline env seed via host helper.
        self.host_ensure_presentation_env_for_hints();
    }

    pub(super) fn apply_heightmap_hint(render_pipeline: &mut RenderPipeline) {
        // Wave 455: presentation-only env boundary — no live GameLogic dual-read.
        let Some(pres) = render_pipeline.presentation_frame() else {
            return;
        };
        let path = pres.world_env.heightmap_hint.clone();
        if let Some(path) = path {
            // Keep renderer parity-safe: map-adjacent TGA companions are frequently preview art.
            // Feeding those into terrain elevation creates severe startup terrain corruption.
            if path.to_ascii_lowercase().ends_with(".tga") {
                render_pipeline.set_heightmap_hint(None);
                return;
            }
            render_pipeline.set_heightmap_hint(Some(path));
        } else {
            render_pipeline.set_heightmap_hint(None);
        }
    }

    pub(super) fn sync_render_terrain_visual(
        render_pipeline: &mut RenderPipeline,
        graphics_system: &GraphicsSystem,
        map_name: &str,
    ) {
        // Wave 459: presentation-only terrain visual sync (no live GameLogic dual-read).
        // Call sites must seed presentation via ensure_presentation_env_for_hints first.
        let Some(bounds) = render_pipeline
            .presentation_frame()
            .map(|p| p.world_env.world_bounds_vec3())
        else {
            warn!(
                "No presentation frame for terrain visual sync on '{}'; skipping",
                map_name
            );
            return;
        };

        let hint_loaded = if render_pipeline.heightmap_hint().is_some() {
            match render_pipeline.load_heightmap_from_hint(
                &graphics_system.device_arc(),
                &graphics_system.queue_arc(),
                Some(bounds),
            ) {
                Ok(()) => true,
                Err(err) => {
                    warn!(
                        "Failed to load terrain visual from heightmap hint for '{}': {}",
                        map_name, err
                    );
                    false
                }
            }
        } else {
            false
        };

        if !hint_loaded {
            // Presentation frame is seeded by caller with runtime_heightmap freeze.
            // Pass None so terrain visual bake cannot dual-read live GameLogic.
            match render_pipeline.load_heightmap_from_runtime_terrain(
                &graphics_system.device_arc(),
                &graphics_system.queue_arc(),
            ) {
                Ok(true) => {}
                Ok(false) => {
                    warn!(
                        "No runtime terrain heightmap available for '{}'; terrain visual may remain empty",
                        map_name
                    );
                }
                Err(err) => {
                    warn!(
                        "Failed to load terrain visual from runtime terrain for '{}': {}",
                        map_name, err
                    );
                }
            }
        }

        // C++ W3DRadar.cpp:977-993 builds the radar terrain texture at newMap
        // with WorldHeightMap tiles already resident; here the render pipeline
        // hydrates the visual (heightmap + source tiles) asynchronously, so
        // Radar::newMap may have painted the band black. Re-run
        // buildTerrainTexture once hydration completes (C++ parity:
        // W3DRadar::refreshTerrain, W3DRadar.cpp:1421-1432).
        if let Ok(mut radar) =
            game_engine::common::system::radar::get_radar_system().write()
        {
            radar.refresh_terrain();
        }

        // Presentation already seeded by caller; road bake cannot dual-read live GameLogic.
        if let Err(err) = render_pipeline.sync_runtime_map_roads() {
            warn!(
                "Failed to sync runtime map roads for '{}': {}",
                map_name, err
            );
        }
    }

    pub(super) fn apply_skybox_hint(render_pipeline: &mut RenderPipeline) {
        // Wave 455: presentation-only env boundary — no live GameLogic dual-read.
        let Some(pres) = render_pipeline.presentation_frame() else {
            return;
        };
        let enabled = pres.world_env.skybox_enabled;
        let textures = pres.world_env.skybox_textures.clone();
        render_pipeline.set_skybox_enabled(enabled);
        if let Some(textures) = textures {
            render_pipeline.set_skybox_hint(textures);
        }
    }

    pub(super) fn reinitialize_minimap_renderer(&mut self) -> anyhow::Result<()> {
        // Wave 468: instance path — presentation-first bounds via shared probe;
        // heightmap repair stamps freeze then mirrors host world size for pathfinding.
        // Wave 594: heightmap repair + last_presentation align via host helper.
        self.ensure_presentation_env_seeded();
        let mut world_bounds = self.presentation_world_bounds();
        self.render_pipeline.initialize_minimap_renderer(
            self.graphics_system.device_arc(),
            self.graphics_system.queue_arc(),
            world_bounds,
        )?;

        world_bounds = self.host_repair_minimap_presentation_bounds(world_bounds);

        self.render_pipeline
            .sync_heightmap_world_bounds(world_bounds);
        self.render_pipeline
            .update_minimap_world_bounds(world_bounds);
        Ok(())
    }

    /// Wave 594: minimap heightmap repair + presentation stamp residual.
    ///
    /// When presentation world bounds are degenerate, stamps heightmap-derived
    /// extents into the pipeline freeze, mirrors host world size for pathfinding,
    /// and keeps `last_presentation_frame` aligned with the stamp.
    pub(super) fn host_repair_minimap_presentation_bounds(
        &mut self,
        mut world_bounds: (Vec3, Vec3),
    ) -> (Vec3, Vec3) {
        // Wave 594: minimap presentation bounds repair residual.
        let world_width = (world_bounds.1.x - world_bounds.0.x).abs();
        let world_height = (world_bounds.1.z - world_bounds.0.z).abs();
        if world_width <= 1.0 || world_height <= 1.0 {
            if let Some((w, h)) = self.render_pipeline.heightmap_world_size() {
                let half_w = w * 0.5;
                let half_h = h * 0.5;
                world_bounds = (
                    Vec3::new(-half_w, 0.0, -half_h),
                    Vec3::new(half_w, 0.0, half_h),
                );
                // Stamp repaired bounds into presentation freeze when installed.
                if let Some(pres) = self.render_pipeline.presentation_frame_mut() {
                    pres.world_env.world_min = world_bounds.0.to_array();
                    pres.world_env.world_max = world_bounds.1.to_array();
                }
                // Host pathfinding/world size residual (sim still needs repaired extents).
                self.host_override_world_size(w, h);
                // Keep last_presentation aligned after stamp, but never replace a
                // newer logic freeze with the match-start pipeline seed (frame 0).
                if let Some(pres) = self.render_pipeline.presentation_frame() {
                    let pipeline_frame = pres.frame.0;
                    let last_frame = self
                        .last_presentation_frame
                        .as_ref()
                        .map(|p| p.frame.0)
                        .unwrap_or(0);
                    if self.last_presentation_frame.is_none() || pipeline_frame >= last_frame {
                        self.last_presentation_frame = Some(pres.clone());
                    }
                }
            }
        }
        world_bounds
    }

    /// Convert a UI faction string into a Team.
    pub(super) fn team_from_faction(faction: &str) -> Team {
        match faction.to_ascii_lowercase().as_str() {
            "usa" | "us" | "america" => Team::USA,
            "gla" => Team::GLA,
            "china" => Team::China,
            _ => Team::USA,
        }
    }

    pub(super) fn handle_minimap_interaction(&mut self, interaction: MinimapInteraction) {
        let pointer = Vec2::new(interaction.screen_position.x, interaction.screen_position.y);
        let Some(world_pos) = self.render_pipeline.handle_minimap_click(pointer) else {
            return;
        };

        match interaction.kind {
            MinimapActionKind::LeftClick | MinimapActionKind::LeftDrag => {
                // C++ ControlBarCallback lookAt — cancel scripted rotate/path/lock.
                self.host_player_look_at(world_pos);
            }
            MinimapActionKind::RightClick => {
                // C++ LeftHUD handles an armed map-target command before its
                // ordinary context-sensitive move/attack/gather order.  The
                // typed Main bridge now makes this path live, so preserve the
                // same ordering instead of silently turning a special power
                // or attack-move click into an ordinary move.
                if self.pending_map_command.is_some() {
                    let location = self.clamp_to_world_bounds(world_pos);
                    let target = self.find_object_at_position(location, true);
                    self.commit_pending_map_command(location, target);
                } else {
                    self.issue_minimap_move(world_pos);
                }
            }
        }
    }

    /// C++ `W3DView::pitchCamera` writes `m_FXPitch`, not orbit pitch radians.
    pub(super) fn script_pitch_is_fx_pitch(pitch: f32) -> f32 {
        if pitch.is_finite() { pitch } else { 1.0 }
    }

    /// C++ `TheTacticalView->getHeight() / TheDisplay->getHeight()`.
    pub(super) fn tactical_view_height_frac(&self) -> f32 {
        #[cfg(feature = "game_client")]
        {
            game_client::display::view::tactical_view_height_frac()
        }
        #[cfg(not(feature = "game_client"))]
        {
            1.0
        }
    }

    /// Live 3D tactical viewport in surface points (top-left origin).
    ///
    /// Matches the swapchain extent (logical window size) so the 3D view, the
    /// WND/UI draw space, and mouse unprojection share one coordinate space.
    pub(super) fn tactical_viewport_size(&self) -> (f32, f32) {
        let (w, h) = super::types::render_surface_extent(&self.window);
        (
            w as f32,
            ((h as f32) * self.tactical_view_height_frac()).max(1.0),
        )
    }

    pub(super) fn rebuild_tactical_projection(&mut self) {
        let (width, height) = self.tactical_viewport_size();
        self.projection_matrix = perspective_rh_from_horizontal_fov(
            DEFAULT_VIEW_FOV_RADIANS,
            width / height,
            DEFAULT_VIEW_NEAR_CLIP,
            DEFAULT_VIEW_FAR_CLIP,
        );
    }

    pub(super) fn parabolic_ease(param: f32, ease_in_time: f32, ease_out_time: f32) -> f32 {
        let param = param.clamp(0.0, 1.0);
        let mut in_t = ease_in_time.clamp(0.0, 1.0);
        let out_t = 1.0 - ease_out_time.clamp(0.0, 1.0);
        if in_t > out_t {
            in_t = out_t;
        }
        let v0 = 1.0 + out_t - in_t;
        if param < in_t {
            if in_t <= 0.0 {
                0.0
            } else {
                param * param / (v0 * in_t)
            }
        } else if param <= out_t {
            (in_t + 2.0 * (param - in_t)) / v0
        } else {
            let denom = (1.0 - out_t).max(f32::EPSILON);
            (in_t
                + 2.0 * (out_t - in_t)
                + (2.0 * (param - out_t) + out_t * out_t - param * param) / denom)
                / v0
        }
    }

    pub(super) fn apply_camera_orbit_transform(&mut self) {
        if self.camera_slave_mode.is_some() {
            return;
        }
        self.camera_pitch_radians = self
            .camera_pitch_radians
            .clamp(5.0_f32.to_radians(), 85.0_f32.to_radians());
        self.camera_orbit_distance = self.camera_orbit_distance.max(1.0);
        let offset = self.camera_orbit_offset();
        let (source, look) = self.camera_fx_adjusted_eye_and_look(
            self.camera_target + offset + self.camera_shake_offset,
            self.camera_target,
        );
        self.camera_position = source;
        let shake = self.camera_shake_rotation;
        if shake.length_squared() > 1.0e-8 {
            let forward = (look - source).try_normalize().unwrap_or(Vec3::NEG_Z);
            let rot = glam::Quat::from_rotation_y(shake.y)
                * glam::Quat::from_rotation_x(shake.x)
                * glam::Quat::from_rotation_z(shake.z);
            let look = source + rot * forward;
            let up = rot * Vec3::Y;
            self.view_matrix = Mat4::look_at_rh(source, look, up);
        } else {
            self.view_matrix = Mat4::look_at_rh(source, look, Vec3::Y);
        }
    }

    /// C++ `W3DView::buildCameraTransform` offset in Main's X/Z-ground basis.
    /// `camera_orbit_distance` is deliberately unzoomed; multiplying it here
    /// makes wheel/key/script zoom affect the real WGPU camera exactly once.
    pub(super) fn camera_orbit_offset(&self) -> Vec3 {
        let zoom = if self.camera_zoom.is_finite() {
            self.camera_zoom.max(0.05)
        } else {
            1.0
        };
        let distance = if self.camera_orbit_distance.is_finite() {
            self.camera_orbit_distance.max(1.0) * zoom
        } else {
            zoom
        };
        let pitch = self
            .camera_pitch_radians
            .clamp(5.0_f32.to_radians(), 85.0_f32.to_radians());
        let horizontal = distance * pitch.cos();
        Vec3::new(
            horizontal * self.camera_yaw_radians.sin(),
            distance * pitch.sin(),
            horizontal * self.camera_yaw_radians.cos(),
        )
    }

    /// C++ `W3DView::buildCameraTransform` FXPitch (W3DView.cpp:362-381).
    /// Host Y-up: C++ height Z → Y, C++ ground XY → XZ.
    pub(super) fn camera_fx_adjusted_eye_and_look(&self, source: Vec3, look: Vec3) -> (Vec3, Vec3) {
        let fx = if self.camera_fx_pitch.is_finite() {
            self.camera_fx_pitch
        } else {
            1.0
        };
        if (fx - 1.0).abs() < 1.0e-6 {
            return (source, look);
        }
        if fx <= 1.0 {
            let height = (source.y - look.y) * fx;
            (source, Vec3::new(look.x, source.y - height, look.z))
        } else {
            (
                Vec3::new(
                    look.x + (source.x - look.x) / fx,
                    source.y,
                    look.z + (source.z - look.z) / fx,
                ),
                look,
            )
        }
    }

    /// Catch camera mutations that arrived outside `update_camera` (minimap,
    /// selection hotkeys, script request drain).  They must not wait for an
    /// unrelated pan or screen shake before changing the displayed view.
    pub(super) fn camera_transform_needs_rebuild(&self) -> bool {
        let expected = self
            .camera_fx_adjusted_eye_and_look(
                self.camera_target + self.camera_orbit_offset() + self.camera_shake_offset,
                self.camera_target,
            )
            .0;
        !expected.is_finite()
            || !self.camera_position.is_finite()
            || (self.camera_position - expected).length_squared() > 1.0e-6
    }

    pub(super) fn sync_orbit_from_camera_transform(&mut self) {
        let offset = self.camera_position - self.camera_target;
        // Inverse of `apply_camera_orbit_transform`: a supplied camera pose
        // (startup/save/script) is already zoomed, while the stored orbit is
        // the C++ camera-offset basis used by future `m_zoom` changes.
        self.camera_orbit_distance = (offset.length() / self.camera_zoom.max(0.05)).max(1.0);
        let horizontal = Vec2::new(offset.x, offset.z).length();
        self.camera_pitch_radians = offset
            .y
            .atan2(horizontal.max(f32::EPSILON))
            .clamp(5.0_f32.to_radians(), 85.0_f32.to_radians());
        self.camera_yaw_radians = offset.x.atan2(offset.z);

        self.camera_pitch_target = None;
        self.camera_pitch_start = self.camera_pitch_radians;
        self.camera_pitch_duration = 0.0;
        self.camera_pitch_elapsed = 0.0;
        self.camera_pitch_ease_in = 0.0;
        self.camera_pitch_ease_out = 0.0;

        self.camera_yaw_target = None;
        self.camera_yaw_start = self.camera_yaw_radians;
        self.camera_yaw_duration = 0.0;
        self.camera_yaw_elapsed = 0.0;
        self.camera_yaw_ease_in = 0.0;
        self.camera_yaw_ease_out = 0.0;

        self.apply_camera_orbit_transform();
    }

    pub(super) fn apply_script_camera_pitch_request(&mut self, request: CameraPitchRequest) {
        // C++ pitchCamera lerps m_FXPitch (W3DView.cpp:2520-2531 / 3049-3068).
        let target = Self::script_pitch_is_fx_pitch(request.pitch);
        if request.duration_seconds <= 0.0 {
            self.camera_fx_pitch = target;
            self.camera_pitch_target = None;
            self.camera_pitch_start = target;
            self.camera_pitch_duration = 0.0;
            self.camera_pitch_elapsed = 0.0;
            self.camera_pitch_ease_in = 0.0;
            self.camera_pitch_ease_out = 0.0;
            self.apply_camera_orbit_transform();
            return;
        }

        self.camera_pitch_start = self.camera_fx_pitch;
        self.camera_pitch_target = Some(target);
        self.camera_pitch_duration = request.duration_seconds;
        self.camera_pitch_elapsed = 0.0;
        self.camera_pitch_ease_in = request.ease_in_seconds.max(0.0);
        self.camera_pitch_ease_out = request.ease_out_seconds.max(0.0);
    }

    pub(super) fn apply_script_camera_rotate_request(&mut self, request: CameraRotateRequest) {
        let target_yaw = self.camera_yaw_radians + request.rotations * TAU;
        if request.duration_seconds <= 0.0 {
            self.camera_yaw_radians = target_yaw;
            self.camera_yaw_target = None;
            self.camera_yaw_start = self.camera_yaw_radians;
            self.camera_yaw_duration = 0.0;
            self.camera_yaw_elapsed = 0.0;
            self.camera_yaw_ease_in = 0.0;
            self.camera_yaw_ease_out = 0.0;
            self.apply_camera_orbit_transform();
            return;
        }

        self.camera_yaw_start = self.camera_yaw_radians;
        self.camera_yaw_target = Some(target_yaw);
        self.camera_yaw_duration = request.duration_seconds;
        self.camera_yaw_elapsed = 0.0;
        self.camera_yaw_ease_in = request.ease_in_seconds.max(0.0);
        self.camera_yaw_ease_out = request.ease_out_seconds.max(0.0);
    }

    /// Wave 568: InGame script FPS residual — prefer presentation freeze, always drain
    /// live queue after apply (peeked freeze must not re-apply next frame).
    pub(super) fn apply_ingame_script_fps_limit_residual(&mut self) {
        // Wave 568/907/910: presentation freeze owns script FPS residual when present.
        if let Some(fps) = self
            .last_presentation_frame
            .as_ref()
            .and_then(|p| p.script_fps_limit)
        {
            self.apply_script_fps_limit_request(fps);
            // Freeze installed: do not dual-read/drain live GameLogic queue.
            return;
        }
        // Wave 910: cold residual fail-closed — no take_script_fps_limit_request dual-read.
        // Script FPS applies only via presentation freeze residual.
    }

    /// Wave 568: shell/menu script FPS residual — only trust freeze when it affirms
    /// shell-map (`fow_shell_bypass`); otherwise boot take_script_fps_limit_request.
    pub(super) fn apply_shell_script_fps_limit_residual(&mut self) {
        // Wave 568/900: shell freeze owns FPS residual; no live take dual-read.
        if let Some(fps) = self
            .last_presentation_frame
            .as_ref()
            .filter(|p| p.fow_shell_bypass)
            .and_then(|p| p.script_fps_limit)
        {
            self.apply_script_fps_limit_request(fps);
        }
        // Wave 900: boot/no-freeze fail-closed (no take_script_fps_limit_request dual-read).
    }

    pub(super) fn apply_script_fps_limit_request(&mut self, fps: i32) {
        let global_default = {
            let mut global = game_engine::common::global_data::write();
            global.writable.use_fps_limit = true;
            Some(global.writable.frames_per_second_limit)
        };

        let resolved_fps = if fps <= 0 {
            global_default.unwrap_or_else(|| {
                game_engine::common::global_data::read()
                    .writable
                    .frames_per_second_limit
            })
        } else {
            fps
        };

        self.script_fps_limit = u32::try_from(resolved_fps).ok().filter(|fps| *fps > 0);
        self.script_fps_limit_last_tick = None;
    }

    pub(super) fn effective_fps_limit_for_frame(
        script_fps_limit: Option<u32>,
        global_use_fps_limit: bool,
        global_frames_per_second_limit: i32,
        visual_speed_multiplier: f32,
        tivo_fast_mode: bool,
        in_replay_game: bool,
    ) -> Option<u32> {
        if let Some(script_fps) = script_fps_limit.filter(|fps| *fps > 0) {
            return Some(script_fps);
        }

        // C++ parity: skip frame limiting when tactical time multiplier is above normal.
        if visual_speed_multiplier > 1.0 {
            return None;
        }

        if !global_use_fps_limit {
            return None;
        }

        // C++ parity: TiVO fast mode disables frame limiting for replay playback.
        if tivo_fast_mode && in_replay_game {
            return None;
        }

        u32::try_from(global_frames_per_second_limit)
            .ok()
            .filter(|fps| *fps > 0)
    }

    pub(super) fn apply_script_frame_limit(&mut self) {
        let global_data = game_engine::common::global_data::read();
        // Wave 550: presentation freeze owns FPS-limit visual_speed residual when
        // installed (no host visual_speed_multiplier dual-read).
        // Wave 557: presentation freeze owns replay-mode residual when installed.
        let visual_speed = self.presentation_or_boot_visual_speed();
        let in_replay = self.presentation_or_boot_in_replay_game();

        let max_fps = Self::effective_fps_limit_for_frame(
            self.script_fps_limit,
            global_data.writable.use_fps_limit,
            global_data.writable.frames_per_second_limit,
            visual_speed,
            global_data.tivo_fast_mode,
            in_replay,
        );
        drop(global_data);

        let Some(max_fps) = max_fps else {
            self.script_fps_limit_last_tick = None;
            return;
        };

        // Mirrors C++ GameEngine::execute frame pacing: (1000 / fps) - 1, Sleep(0) loop.
        let limit_ms = (1000.0 / max_fps as f32 - 1.0).max(0.0);
        if limit_ms <= 0.0 {
            self.script_fps_limit_last_tick = Some(Instant::now());
            return;
        }

        let limit = Duration::from_millis(limit_ms as u64);
        if let Some(previous) = self.script_fps_limit_last_tick {
            let now = Instant::now();
            if now.duration_since(previous) < limit {
                // C++ GameEngine::execute Sleep(0) spin until (1000/fps)-1.
                let remaining = limit - now.duration_since(previous);
                if remaining > Duration::ZERO {
                    std::thread::sleep(remaining);
                }
            }
            self.script_fps_limit_last_tick = Some(Instant::now());
        } else {
            self.script_fps_limit_last_tick = Some(Instant::now());
        }
    }

    pub(super) fn screen_shake_value_for_type(shake_type: i32) -> f32 {
        let data = game_engine::common::global_data::read();
        match shake_type.clamp(0, 5) {
            0 => data.shake_subtle_intensity,
            1 => data.shake_normal_intensity,
            2 => data.shake_strong_intensity,
            3 => data.shake_severe_intensity,
            4 => data.shake_cine_extreme_intensity,
            _ => data.shake_cine_insane_intensity,
        }
    }

    pub(super) fn enqueue_script_screen_shake(&mut self, intensity: i32) {
        let shake_value = Self::screen_shake_value_for_type(intensity);
        if !shake_value.is_finite() || shake_value <= 0.0 {
            return;
        }

        let seed = self
            .frame_counter
            .wrapping_mul(1_664_525)
            .wrapping_add((intensity as u32).wrapping_mul(1_013_904_223));
        let angle = (seed as f32 / u32::MAX as f32) * TAU;
        self.screen_shake_angle_cos = angle.cos();
        self.screen_shake_angle_sin = angle.sin();

        self.screen_shake_intensity += shake_value;
        let data = game_engine::common::global_data::read();
        if self.screen_shake_intensity > data.max_shake_intensity {
            // C++ parity from W3DView::shake: overflow clamps to fixed 3.0.
            self.screen_shake_intensity = 3.0;
        }
    }

    pub(super) fn enqueue_script_camera_shaker(&mut self, request: CameraAddShakerRequest) {
        if !request.position.is_finite()
            || !request.amplitude.is_finite()
            || !request.duration_seconds.is_finite()
            || !request.radius.is_finite()
        {
            return;
        }
        if request.duration_seconds <= 0.0 || request.radius <= 0.0 || request.amplitude <= 0.0 {
            return;
        }

        self.script_camera_shakers.push(ScriptCameraShaker::new(
            request.position,
            request.radius,
            request.duration_seconds,
            request.amplitude,
        ));
    }

    pub(super) fn update_script_camera_shake(&mut self, dt: f32) -> bool {
        let previous_offset = self.camera_shake_offset;
        let previous_rot = self.camera_shake_rotation;
        let mut offset = Vec3::ZERO;
        let mut rotation = Vec3::ZERO;

        if self.screen_shake_intensity > 0.01 {
            offset.x += self.screen_shake_intensity * self.screen_shake_angle_cos;
            offset.z += self.screen_shake_intensity * self.screen_shake_angle_sin;
            self.screen_shake_intensity *= 0.75;
            self.screen_shake_angle_cos = -self.screen_shake_angle_cos;
            self.screen_shake_angle_sin = -self.screen_shake_angle_sin;
        } else {
            self.screen_shake_intensity = 0.0;
            self.screen_shake_angle_cos = 0.0;
            self.screen_shake_angle_sin = 0.0;
        }

        // C++ W3DView::update: CameraShakerSystem.Timestep(1/30) once per present.
        // Do not use visual_dt — at 60 fps that halves the envelope/omega sweep.
        const SCRIPT_CAMERA_SHAKER_STEP: f32 = 1.0 / 30.0;
        for shaker in &mut self.script_camera_shakers {
            shaker.elapsed_seconds += SCRIPT_CAMERA_SHAKER_STEP;
        }
        self.script_camera_shakers
            .retain(|s| s.elapsed_seconds < s.duration_seconds);

        #[cfg(feature = "game_client")]
        {
            // C++ W3DView::update: offset = intensity*(cos,sin), intensity *= 0.75,
            // flip sign. Leftover update_effects (presentation shell) already ticks
            // the thread-local View once per InGame frame; only sample the offset here.
            game_client::display::view::with_tactical_view_ref(|view| {
                let impulse = view.impulse_shake_offset();
                offset.x += impulse.x;
                offset.z += impulse.y;
            });
        }

        let camera_position = self.camera_position;
        for shaker in &self.script_camera_shakers {
            rotation += script_camera_shaker_rotations(shaker, camera_position);
        }
        rotation.x = rotation.x.clamp(-SHAKE_AXIS_PITCH, SHAKE_AXIS_PITCH);
        rotation.y = rotation.y.clamp(-SHAKE_AXIS_YAW, SHAKE_AXIS_YAW);
        rotation.z = rotation.z.clamp(-SHAKE_AXIS_ROLL, SHAKE_AXIS_ROLL);

        self.camera_shake_offset = offset;
        self.camera_shake_rotation = rotation;
        (self.camera_shake_offset - previous_offset).length_squared() > 0.000001
            || (self.camera_shake_rotation - previous_rot).length_squared() > 1.0e-8
    }

    pub(super) fn normalize_signed_angle(mut angle: f32) -> f32 {
        while angle > PI {
            angle -= TAU;
        }
        while angle < -PI {
            angle += TAU;
        }
        angle
    }

    pub(super) fn apply_camera_look_toward_request(
        &mut self,
        request: CameraLookTowardWaypointRequest,
    ) {
        let to_target = request.position - self.camera_target;
        let horiz = Vec2::new(to_target.x, to_target.z);
        if horiz.length_squared() <= f32::EPSILON {
            return;
        }

        let target_yaw = to_target.x.atan2(to_target.z);
        let mut delta = Self::normalize_signed_angle(target_yaw - self.camera_yaw_radians);
        if request.reverse_rotation {
            if delta >= 0.0 {
                delta -= TAU;
            } else {
                delta += TAU;
            }
        }
        let target_yaw = self.camera_yaw_radians + delta;

        if request.duration_seconds <= 0.0 {
            self.camera_yaw_radians = target_yaw;
            self.camera_yaw_target = None;
            self.camera_yaw_start = self.camera_yaw_radians;
            self.camera_yaw_duration = 0.0;
            self.camera_yaw_elapsed = 0.0;
            self.camera_yaw_ease_in = 0.0;
            self.camera_yaw_ease_out = 0.0;
            self.apply_camera_orbit_transform();
            return;
        }
        if self.camera_yaw_target.is_some() && self.camera_yaw_duration > 0.0 {
            self.camera_yaw_target = Some(target_yaw);
            self.camera_yaw_duration = request.duration_seconds.max(self.camera_yaw_duration);
            return;
        }

        self.camera_yaw_start = self.camera_yaw_radians;
        self.camera_yaw_target = Some(target_yaw);
        self.camera_yaw_duration = request.duration_seconds;
        self.camera_yaw_elapsed = 0.0;
        self.camera_yaw_ease_in = request.ease_in_seconds.max(0.0);
        self.camera_yaw_ease_out = request.ease_out_seconds.max(0.0);
    }

    /// Wave 611: via `host_center_camera_on`.
    ///
    /// Scripted pans (C++ `W3DView.cpp:3097-3212`) widen `m_cameraConstraint`
    /// so a cinematic can leave the map. Player `lookAt` still clamps.
    pub(super) fn center_camera_on(&mut self, world_pos: Vec3) {
        self.host_center_camera_on_impl(world_pos, false)
    }

    /// C++ player `W3DView::lookAt` — cancel scripted rotate/path then snap.
    pub(super) fn host_player_look_at(&mut self, world_pos: Vec3) {
        self.cancel_scripted_camera_from_player_look_at();
        self.host_center_camera_on_impl(world_pos, true);
    }

    /// Presentation-frozen ground height under `world_pos`.
    /// Same sampler `host_center_camera_on` uses — no live GameLogic dual-read.
    pub(super) fn sample_presentation_height_under(&self, world_pos: Vec3) -> f32 {
        let clamped = self.clamp_to_world_bounds(world_pos);
        if let Some(pres) = self
            .render_pipeline
            .presentation_frame()
            .or(self.last_presentation_frame.as_ref())
        {
            pres.world_env
                .sample_height(clamped.x, clamped.z)
                .unwrap_or(self.camera_target.y)
        } else {
            // Wave 905: fail-closed boot height (no terrain_height_at dual-read).
            self.camera_target.y
        }
    }

    pub(super) fn host_center_camera_on(&mut self, world_pos: Vec3) {
        self.host_center_camera_on_impl(world_pos, true)
    }

    fn host_center_camera_on_impl(&mut self, world_pos: Vec3, clamp_to_world: bool) {
        let mut look = world_pos;
        let ground_height = self.sample_presentation_height_under(look);
        // C++ W3DView::lookAt: elevated targets ray-cast onto the heightmap.
        if world_pos.y > PATHFIND_CELL_SIZE_F + ground_height {
            let (world_min, world_max) = self.presentation_world_bounds();
            let env = self
                .render_pipeline
                .presentation_frame()
                .or(self.last_presentation_frame.as_ref())
                .map(|f| &f.world_env);
            if let Some(hit) = airborne_look_at_ground(
                self.camera_position,
                world_pos,
                self.camera_target - self.camera_position,
                DEFAULT_VIEW_FAR_CLIP,
                world_min,
                world_max,
                env,
            ) {
                look.x = hit.x;
                look.z = hit.z;
            }
        }
        // C++ scripted waypoint pans widen m_cameraConstraint (W3DView.cpp:3097-3212)
        // so the look-at can leave the map. Player lookAt still clamps to the
        // (possibly widened) constraint.
        if !clamp_to_world {
            self.scripted_camera_constraint_widen = Some(widen_scripted_camera_constraint(
                self.scripted_camera_constraint_widen,
                look.x,
                look.z,
            ));
        }
        let look = self.clamp_to_world_bounds(look);
        let ground_height = self.sample_presentation_height_under(look);
        self.camera_target.x = look.x;
        self.camera_target.y = ground_height;
        self.camera_target.z = look.z;
        self.apply_camera_orbit_transform();
    }

    /// Radius-cursor / guard rings only. Structure placement is the C++ 3D model ghost.
    pub(super) fn collect_ground_marker_circles(
        &self,
    ) -> Vec<crate::graphics::selection_renderer::SelectedUnit> {
        use crate::graphics::selection_renderer::SelectedUnit;
        let mut out = Vec::new();

        // C++ InGameUI::placeBuildAvailable / handleBuildPlacements: translucent
        // building Drawable at placementOpacity 0.45 + faction bibs. Not a circle.
        self.submit_structure_placement_model_ghost();

        // Special-power / AttackMove / Guard radius cursor residual.
        if let Some(ov) = self.game_hud.construction_panel.radius_overlay() {
            if ov.radius > 0.0 {
                let color = if ov.is_legal {
                    [ov.color.0, ov.color.1, ov.color.2, ov.color.3.max(0.35)]
                } else {
                    [1.0, 0.1, 0.1, 0.5]
                };
                out.push(SelectedUnit {
                    position: glam::Vec3::new(ov.centre.0, 0.0, ov.centre.1),
                    radius: ov.radius.max(1.0),
                    team_color: color,
                });
            }
        }

        // Active guard-area residual for selected units (C++ Guard area ring).
        const GUARD_AREA_RADIUS: f32 = 100.0; // matches RadiusCursor GUARD_AREA
        if let Some(frame) = self.last_presentation_frame.as_ref() {
            for o in &frame.objects {
                if o.destroyed || !o.selected {
                    continue;
                }
                let Some(gp) = o.guard_position else {
                    continue;
                };
                out.push(SelectedUnit {
                    position: glam::Vec3::new(gp.x, 0.0, gp.z),
                    radius: GUARD_AREA_RADIUS,
                    team_color: [0.35, 0.75, 1.0, 0.35],
                });
            }
        }

        out
    }

    /// C++ `InGameUI::placeBuildAvailable` + `handleBuildPlacements`:
    /// spawn the building model at `placementOpacity` 0.45, red-tint when
    /// `isLegalBuildLocation` fails, and add/remove faction bibs.
    fn submit_structure_placement_model_ghost(&self) {
        let placement = self.game_hud.construction_panel.placement_preview();
        if !placement.is_active() {
            self.sync_structure_placement_faction_bibs(None);
            return;
        }

        let world_x = placement.world_pos.0;
        let world_z = placement.world_pos.1;
        let world_y = self.sample_presentation_height_under(Vec3::new(world_x, 0.0, world_z));
        let facing = placement.facing_radians;
        let illegal = !placement.is_legal;
        let model_name = structure_placement_model_key(&placement.template_name);
        let world_transform =
            structure_placement_model_transform(world_x, world_y, world_z, facing);
        let half = placement.footprint_half_extents;
        let radius = half.0.max(half.1).max(30.0) * 2.0;

        #[cfg(feature = "game_client")]
        {
            let submission = game_client::render_bridge::DrawSubmission {
                drawable_id: game_client::render_bridge::DrawableId(
                    STRUCTURE_PLACEMENT_GHOST_DRAWABLE_ID,
                ),
                model_name,
                world_transform,
                render_state: game_client::render_bridge::RenderStateOverrides {
                    opacity: STRUCTURE_PLACEMENT_GHOST_OPACITY,
                    construction_tint: structure_placement_ghost_tint(illegal),
                    ..Default::default()
                },
                bounding_sphere: ww3d_core::BoundingSphere::new(
                    ww3d_core::glam::Vec3::ZERO,
                    radius,
                ),
                opaque: false,
                transparent: true,
                cast_shadow: false,
                ..Default::default()
            };
            if let Ok(mut bridge_guard) = game_client::render_bridge::get_render_bridge().lock() {
                if let Some(bridge) = bridge_guard.as_mut() {
                    bridge.submit(submission);
                }
            }
        }

        self.sync_structure_placement_faction_bibs(Some((world_x, world_z, facing, half, illegal)));
    }

    /// C++ `TheTerrainVisual->addFactionBibDrawable` when LBC != OK,
    /// `removeFactionBibDrawable` when legal.
    fn sync_structure_placement_faction_bibs(
        &self,
        active: Option<(f32, f32, f32, (f32, f32), bool)>,
    ) {
        #[cfg(feature = "game_client")]
        {
            let Ok(mut guard) = game_client::terrain::terrain_visual::get_terrain_visual() else {
                return;
            };
            let Some(visual) = guard.as_mut() else {
                return;
            };
            let Some((world_x, world_z, facing, half, illegal)) = active else {
                visual.remove_faction_bib(
                    STRUCTURE_PLACEMENT_GHOST_DRAWABLE_ID,
                    game_client::terrain::terrain_visual::TerrainBibOwnerKind::Drawable,
                );
                return;
            };
            if illegal {
                visual.add_faction_bib(
                    STRUCTURE_PLACEMENT_GHOST_DRAWABLE_ID,
                    game_client::terrain::terrain_visual::TerrainBibOwnerKind::Drawable,
                    structure_placement_bib_transform(world_x, world_z, facing),
                    half.0,
                    half.1,
                    true,
                    0.0,
                    0.0,
                    true,
                    0.0,
                );
            } else {
                visual.remove_faction_bib(
                    STRUCTURE_PLACEMENT_GHOST_DRAWABLE_ID,
                    game_client::terrain::terrain_visual::TerrainBibOwnerKind::Drawable,
                );
            }
        }
        #[cfg(not(feature = "game_client"))]
        {
            let _ = active;
        }
    }

    pub(super) fn issue_minimap_move(&mut self, world_pos: Vec3) {
        // Wave 219: selection via presentation-first ui_selected_ids.
        let selected = self.ui_selected_ids(self.current_player_id);
        if selected.is_empty() {
            return;
        }

        let clamped = self.clamp_to_world_bounds(world_pos);

        // C++ InGameUI minimap RMB residual: same context-sensitive path as world
        // right-click (attack enemy / gather / enter / move).
        let target_object = self.find_object_at_position(clamped, true);
        let ctrl = self.keys_pressed.iter().any(|k| {
            matches!(
                k,
                winit::keyboard::Key::Named(winit::keyboard::NamedKey::Control)
            )
        });
        let shift = self.keys_pressed.iter().any(|k| {
            matches!(
                k,
                winit::keyboard::Key::Named(winit::keyboard::NamedKey::Shift)
            )
        });
        let alt = self.sticky_waypoint_mode
            || self.keys_pressed.iter().any(|k| {
                matches!(
                    k,
                    winit::keyboard::Key::Named(winit::keyboard::NamedKey::Alt)
                )
            });

        let context = crate::command_system::MouseCommandContext {
            world_position: clamped,
            target_object,
            target_presentation: target_object.and_then(|id| self.presentation_target_hint(id)),
            selected_presentation: self.presentation_selected_unit_hints(&selected),
            presentation_box_select_units: Vec::new(),
            presentation_select_similar_units: Vec::new(),
            screen_position: glam::Vec2::new(self.mouse_position.0, self.mouse_position.1),
            viewport_size: None,
            world_min: None,
            world_max: None,
            mouse_button: crate::command_system::MouseButton::Right,
            modifier_keys: crate::command_system::ModifierKeys { ctrl, shift, alt },
            is_drag: false,
            drag_start: None,
            drag_end: None,
            drag_start_world: None,
            drag_end_world: None,
        };

        let mut cmd_sys = crate::command_system::CommandSystem::new();
        // Wave 236: presentation-only mouse path when frame installed.
        let command = cmd_sys.process_mouse_input(
            &context,
            &selected,
            self.current_player_id,
            self.presentation_mouse_game_logic(),
        );

        if let Some(mut command) = command {
            if self.sticky_auto_attack {
                if let crate::command_system::CommandType::MoveTo { destination, .. } =
                    command.command_type
                {
                    command.command_type = crate::command_system::CommandType::AttackMoveTo {
                        destination,
                        max_shots: -1,
                    };
                }
            }
            self.host_queue_and_process_command(command);
            return;
        }

        // Fail-closed fallback residual: move if context path produced nothing.
        if self.sticky_auto_attack {
            self.host_command_attack_move(self.current_player_id, clamped);
        } else {
            self.host_command_move(self.current_player_id, clamped);
        }
        self.play_sound_effect(SoundType::Command);
    }

    /// Wave 461: single presentation-first world bounds probe for camera/HUD/minimap.
    /// Prefers pipeline freeze, then last_presentation_frame, then host GameLogic.
    pub(super) fn presentation_world_bounds(&self) -> (Vec3, Vec3) {
        if let Some(frame) = self
            .render_pipeline
            .presentation_frame()
            .or(self.last_presentation_frame.as_ref())
        {
            frame.world_env.world_bounds_vec3()
        } else {
            self.host_world_bounds()
        }
    }

    pub(super) fn clamp_to_world_bounds(&self, mut position: Vec3) -> Vec3 {
        // Wave 461: presentation-first bounds via shared probe.
        let (world_min, world_max) = self.presentation_world_bounds();
        let size = self.window.inner_size();
        let inset = w3d_camera_constraint_offset(
            self.view_matrix,
            self.projection_matrix,
            (size.width as f32, size.height as f32),
            position.y,
        );
        let lo_x = world_min.x + inset;
        let hi_x = world_max.x - inset;
        let lo_z = world_min.z + inset;
        let hi_z = world_max.z - inset;
        let (lo_x, hi_x, lo_z, hi_z) = apply_scripted_camera_constraint_widen(
            lo_x,
            hi_x,
            lo_z,
            hi_z,
            self.scripted_camera_constraint_widen,
        );
        if lo_x <= hi_x {
            position.x = position.x.clamp(lo_x, hi_x);
        } else {
            position.x = (world_min.x + world_max.x) * 0.5;
        }
        if lo_z <= hi_z {
            position.z = position.z.clamp(lo_z, hi_z);
        } else {
            position.z = (world_min.z + world_max.z) * 0.5;
        }
        position
    }

    pub(super) fn drain_renderer_attachments(&mut self) {
        match ww3d_renderer_3d::Renderer::with_global_mut(|renderer| {
            Ok(renderer.take_pending_attachments())
        }) {
            Ok(records) if !records.is_empty() => {
                AttachmentDispatcher::dispatch(records);
            }
            Ok(_) => {}
            Err(err) => {
                warn!("Failed to dispatch WW3D attachments: {err}");
            }
        }
    }

    pub(super) fn debug_show_victory(&mut self, winner: Option<u32>) {
        info!("Debug: showing victory screen (winner: {:?})", winner);
        self.show_victory_screen(winner);
    }

    pub(super) fn show_victory_screen(&mut self, winner: Option<u32>) {
        if let Some(entered) = self.ingame_entered_at {
            if entered.elapsed() < std::time::Duration::from_secs(15) {
                log::info!(
                    "suppress victory {}ms after InGame enter",
                    entered.elapsed().as_millis()
                );
                return;
            }
        }
        // Wave 584: victory summary residual via helper.
        let summary = self.presentation_or_boot_victory_summary(winner);
        let queued_summary = summary.clone();
        self.victory_summary = Some(summary.clone());
        if let Err(err) = crate::game_results_queue::queue_victory_summary(queued_summary) {
            warn!("Failed to enqueue victory summary: {err}");
        }
        self.game_paused = true;
        self.match_over = true;
        match winner {
            Some(id) if id == self.current_player_id => {
                self.ui_manager.set_victory_with_summary(id, Some(summary));
                self.request_state_change(GameState::Victory);
            }
            Some(_) => {
                self.ui_manager.set_defeat_with_summary(Some(summary));
                self.request_state_change(GameState::Defeat);
            }
            None => {
                self.ui_manager.set_draw_with_summary(Some(summary));
                // Draw freezes with Defeat residual (no separate Draw state).
                self.request_state_change(GameState::Defeat);
            }
        }
    }

    /// C++ `GameEngine::reset` (`GameEngine.cpp:685-711`).
    /// BlankWindow.wnd covers the previous map/UI while subsystems reset;
    /// multiplayer tears down `TheNetwork`.
    pub(super) fn host_game_engine_reset(&mut self) {
        info!("GameEngine::reset — BlankWindow overlay + resetAll + MP network teardown");

        #[cfg(feature = "game_client")]
        let blank_layout = {
            use game_client::gui::game_window::WindowStatus;
            use game_client::gui::with_window_manager;
            with_window_manager(|manager| {
                manager
                    .create_layout_with_windows("Menus/BlankWindow.wnd")
                    .ok()
                    .map(|(layout, _)| {
                        layout.borrow_mut().hide(false);
                        layout.borrow_mut().bring_forward();
                        if let Some(window) = layout.borrow().get_first_window() {
                            window.borrow_mut().clear_status(WindowStatus::IMAGE);
                        }
                        layout
                    })
            })
        };

        let delete_network = self.host_is_in_multiplayer_game();

        if let Some(subsystem_manager) = get_subsystem_manager() {
            let mut manager = subsystem_manager.lock();
            if let Err(err) = manager.reset_all() {
                warn!("GameEngine::reset TheSubsystemList->resetAll failed: {err}");
            }
        }

        #[cfg(feature = "game_client")]
        if let Err(err) = self.game_client.reset() {
            warn!("GameEngine::reset GameClient::reset failed: {err}");
        }

        // TheShell teardown: C++ tears the shell screens down with the engine
        // on match start. Without this, a runtime-host `start_game` issued
        // while the WND menu stack is pushed (MainMenu +
        // SkirmishGameOptionsMenu) leaves those screens on the shell stack:
        // their windows keep drawing over the InGame world and their gadget
        // hit-testing eats every world click (drive11/m12: menu overlay over
        // Defcon6, physical LMB could never select a unit). Shell::reset pops
        // every screen via the immediate path.
        #[cfg(feature = "game_client")]
        if let Err(err) = game_client::gui::ShellHandle::default().reset() {
            warn!("GameEngine::reset TheShell reset failed: {err}");
        }

        if delete_network {
            // C++ GameEngine.cpp:699-704 `delete TheNetwork`.
            crate::network::clear_active_network_interface();
        }

        #[cfg(feature = "game_client")]
        if let Some(layout) = blank_layout {
            game_client::gui::with_window_manager(|manager| {
                manager.destroy_layout(&layout);
            });
        }
    }

    pub(super) fn reset_match_state(&mut self) {
        info!("Resetting gameplay state after match completion");
        // C++ GameLogicDispatch / GameState / PopupSaveLoad call
        // TheGameEngine->reset() before tearing down match state.
        self.host_game_engine_reset();
        self.pending_match_start = None;
        self.drain_renderer_attachments();

        self.host_reset_game_logic();
        self.resource_manager = ResourceManager::new();

        // Path grid rebuild is owned by GameLogic on map load/reset.
        self.selected_objects.clear();
        self.keys_pressed.clear();
        self.mouse_position = (0.0, 0.0);
        self.mouse_cursor_seen = false;

        self.mouse_world_position = Vec3::ZERO;
        self.selection_start = None;
        self.clear_look_at_host_modes();
        self.rmb_scroll_anchor = None;
        self.is_rmb_scrolling = false;
        self.rmb_scroll_started_physically = false;
        self.rmb_deselect_down_at = None;
        self.rmb_deselect_down_screen = None;
        self.rmb_deselect_down_camera = None;

        for sink in &self.sound_effects {
            sink.stop();
        }
        self.sound_effects.clear();
        if let Some(sink) = self.background_music.take() {
            sink.stop();
        }

        self.match_over = false;
        self.game_paused = false;
        self.victory_summary = None;
        self.ui_manager.clear_victory_screen();
        self.diagnostics_overlay = None;

        self.frame_counter = 0;
        self.fps = 0.0;
        self.last_frame_timing = None;
        self.frame_clock = FrameClock::new();
        NetworkClock::clear_override();

        self.game_hud = GameHUD::new();
        let (hud_w, hud_h) = super::types::render_surface_extent(&self.window);
        self.game_hud.resize(hud_w, hud_h);

        self.camera_position = Vec3::new(0.0, 200.0, 200.0);
        self.camera_target = Vec3::new(0.0, 0.0, 0.0);
        self.scripted_camera_constraint_widen = None;
        self.camera_fx_pitch = 1.0;
        self.camera_zoom = 1.0;
        self.camera_zoom_target = None;
        self.camera_zoom_start = self.camera_zoom;
        self.camera_zoom_duration = 0.0;
        self.camera_zoom_elapsed = 0.0;
        self.camera_zoom_ease_in = 0.0;
        self.camera_zoom_ease_out = 0.0;
        self.camera_shake_offset = Vec3::ZERO;
        self.camera_shake_rotation = Vec3::ZERO;
        self.screen_shake_intensity = 0.0;
        self.screen_shake_angle_cos = 0.0;
        self.screen_shake_angle_sin = 0.0;
        self.script_camera_shakers.clear();
        self.script_fps_limit = None;
        self.script_fps_limit_last_tick = None;
        self.camera_slave_mode = None;
        self.sync_orbit_from_camera_transform();
        self.rebuild_tactical_projection();
    }

    pub(super) fn return_to_main_menu_after_match(&mut self) {
        self.reset_match_state();
        self.transition_to_state(GameState::Menu);
    }

    pub(super) fn exit_to_main_menu_from_victory(&mut self) {
        self.return_to_main_menu_after_match();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::common::global_data as runtime_global_data;
    use game_engine::common::ini::INI;
    use game_engine::common::ini::ini_game_data::ensure_global_data;

    fn primary(
        ambient: [f32; 3],
        diffuse: [f32; 3],
        light_pos: [f32; 3],
        object_light_active: bool,
    ) -> crate::presentation_frame::PresentationPrimaryGlobalLight {
        crate::presentation_frame::PresentationPrimaryGlobalLight {
            ambient,
            diffuse,
            light_pos,
            object_light_active,
        }
    }

    #[test]
    fn w3d_global_lighting_parser_freezes_distinct_primary_records_for_renderer() {
        // Exercise the actual `GameData` block parser, then freeze its exact
        // active-TOD index-zero values into a presentation-shaped record.
        let authored_handle = ensure_global_data();
        let previous_authored = authored_handle.read().clone();
        let (parse_result, frozen) = runtime_global_data::with_global_data_restored(|| {
            let mut ini = INI::new();
            let parse_result = ini.with_inline_source(
                r#"
GameData
  TimeOfDay = AFTERNOON
  NumberGlobalLights = 1
  TerrainObjectsLightingAfternoonAmbient = R:25 G:50 B:75
  TerrainObjectsLightingAfternoonDiffuse = R:100 G:125 B:150
  TerrainObjectsLightingAfternoonLightPos = X:1.0 Y:2.0 Z:3.0
  TerrainLightingAfternoonAmbient = R:10 G:20 B:30
  TerrainLightingAfternoonDiffuse = R:40 G:80 B:120
  TerrainLightingAfternoonLightPos = X:4.0 Y:5.0 Z:6.0
End
"#,
                |ini| ini.parse_current_file(),
            );
            let frozen = crate::presentation_frame::freeze_primary_game_data_lighting(
                &authored_handle.read(),
            );
            (parse_result, frozen)
        });
        *authored_handle.write() = previous_authored;

        parse_result.expect("inline GameData lighting must parse");
        let (object, terrain) = frozen.expect("valid authored time of day freezes lighting");
        assert_eq!(object.ambient, [25.0 / 255.0, 50.0 / 255.0, 75.0 / 255.0]);
        assert_eq!(
            object.diffuse,
            [100.0 / 255.0, 125.0 / 255.0, 150.0 / 255.0]
        );
        assert_eq!(object.light_pos, [1.0, 2.0, 3.0]);
        assert_eq!(object.render_light_pos(), [1.0, 3.0, 2.0]);
        assert!(object.object_light_active);

        assert_eq!(terrain.ambient, [10.0 / 255.0, 20.0 / 255.0, 30.0 / 255.0]);
        assert_eq!(terrain.diffuse, [40.0 / 255.0, 80.0 / 255.0, 120.0 / 255.0]);
        assert_eq!(terrain.light_pos, [4.0, 5.0, 6.0]);
        assert_eq!(terrain.render_light_pos(), [4.0, 6.0, 5.0]);

        let env = PresentationWorldEnv {
            primary_object_lighting: Some(object),
            primary_terrain_lighting: Some(terrain),
            ..Default::default()
        };
        let resolved = resolve_map_activation_lighting(Some(&env));
        assert_eq!(
            resolved
                .object
                .as_ref()
                .expect("active object light reaches Forward/Graphics")
                .sun_direction,
            Some([1.0, 3.0, 2.0])
        );
        assert_eq!(
            resolved
                .terrain
                .as_ref()
                .expect("terrain record reaches TerrainVisual")
                .sun_direction,
            Some([4.0, 6.0, 5.0])
        );
    }

    #[test]
    fn w3d_global_lighting_renderer_resolver_preserves_map_overrides_and_zero_light_fallback() {
        let env = PresentationWorldEnv {
            primary_object_lighting: Some(primary(
                [0.10, 0.20, 0.30],
                [0.40, 0.50, 0.60],
                [1.0, 2.0, 3.0],
                true,
            )),
            primary_terrain_lighting: Some(primary(
                [0.70, 0.80, 0.90],
                [0.11, 0.12, 0.13],
                [4.0, 5.0, 6.0],
                true,
            )),
            // Explicit map fields override the matching GameData channels but
            // do not collapse the distinct authored object/terrain diffuse
            // values into a single lighting record.
            ambient_color: Some([0.91, 0.92, 0.93]),
            sun_direction: Some([-1.0, -2.0, -3.0]),
            ..Default::default()
        };
        let resolved = resolve_map_activation_lighting(Some(&env));
        let object = resolved.object.expect("object lighting");
        let terrain = resolved.terrain.expect("terrain lighting");
        assert_eq!(object.ambient_color, Some([0.91, 0.92, 0.93]));
        assert_eq!(terrain.ambient_color, Some([0.91, 0.92, 0.93]));
        assert_eq!(object.sun_direction, Some([-1.0, -2.0, -3.0]));
        assert_eq!(terrain.sun_direction, Some([-1.0, -2.0, -3.0]));
        assert_eq!(object.sun_color, Some([0.40, 0.50, 0.60]));
        assert_eq!(terrain.sun_color, Some([0.11, 0.12, 0.13]));

        let zero_object_light_env = PresentationWorldEnv {
            primary_object_lighting: Some(primary(
                [0.0, 0.0, 0.0],
                [0.0, 0.0, 0.0],
                [0.0, 0.0, 0.0],
                false,
            )),
            primary_terrain_lighting: Some(primary(
                [0.2, 0.3, 0.4],
                [0.5, 0.6, 0.7],
                [7.0, 8.0, 9.0],
                false,
            )),
            ..Default::default()
        };
        let zero_light = resolve_map_activation_lighting(Some(&zero_object_light_env));
        assert!(
            zero_light.object.is_none(),
            "NumberGlobalLights=0 must preserve existing Forward/Graphics lighting"
        );
        assert_eq!(
            zero_light
                .terrain
                .expect("terrain primary remains independently authored")
                .sun_direction,
            Some([7.0, 9.0, 8.0])
        );
    }

    #[test]
    fn live_match_reset_runs_cpp_game_engine_reset() {
        let src = include_str!("start_game.rs");
        let live = src
            .split("#[cfg(test)]")
            .next()
            .expect("start_game live path");
        assert!(
            live.contains("fn host_game_engine_reset")
                && live.contains("Menus/BlankWindow.wnd")
                && live.contains("manager.reset_all()")
                && live.contains("clear_active_network_interface()"),
            "live GameEngine::reset must show BlankWindow, resetAll, and tear down TheNetwork"
        );
        let reset_fn = live
            .find("pub(super) fn reset_match_state")
            .expect("reset_match_state");
        let body = &live[reset_fn..reset_fn + 400];
        assert!(
            body.contains("self.host_game_engine_reset()"),
            "reset_match_state must invoke GameEngine::reset"
        );
        assert!(
            live.contains("TheGameEngine->reset() before startNewGame")
                && live.contains("self.host_game_engine_reset()"),
            "new-game start must call GameEngine::reset like GameLogicDispatch.cpp:256"
        );
    }

    #[test]
    fn snap_camera_is_bootstrap_not_live_scout() {
        // C++ has no 6000wu distance-to-own-units snap during play.
        let src = include_str!("start_game.rs");
        let live = src
            .split("#[cfg(test)]")
            .next()
            .expect("start_game live path");
        assert!(
            !live.contains("camera_is_unreasonably_far_from_local_units")
                && !live.contains("6_000.0 * 6_000.0"),
            "6000wu scout snap helper must not remain"
        );
        let snap = live
            .find("fn snap_camera_to_local_units_if_needed")
            .expect("snap remains for match bootstrap");
        let doc = &live[snap.saturating_sub(400)..snap];
        assert!(
            doc.contains("never from `update_camera` during play"),
            "snap must stay match-start / origin-hitch only"
        );
    }

    #[test]
    fn structure_placement_ghost_matches_cpp_ingameui_opacity_and_illegal_tint() {
        // C++ InGameUI.cpp:77-78 placementOpacity / illegalBuildColor.
        assert!((STRUCTURE_PLACEMENT_GHOST_OPACITY - 0.45).abs() < f32::EPSILON);
        assert_eq!(structure_placement_ghost_tint(false), None);
        assert_eq!(structure_placement_ghost_tint(true), Some([1.0, 0.0, 0.0]));
    }

    #[test]
    fn structure_placement_model_transform_matches_unit_hull_yaw() {
        let m = structure_placement_model_transform(10.0, 4.0, 20.0, 0.0);
        let p = m.transform_point3(Vec3::new(1.0, 0.0, 0.0));
        assert!((p.x - 11.0).abs() < 1e-4);
        assert!((p.y - 4.0).abs() < 1e-4);
        assert!((p.z - 20.0).abs() < 1e-4);
    }

    #[test]
    fn structure_placement_bib_transform_maps_cpp_xy_ground_to_y_up_xz() {
        // C++ addFactionBibDrawable local (-10, -5, 0) at (100, 200) identity yaw.
        let t = structure_placement_bib_transform(100.0, 200.0, 0.0);
        let p = t.transform_point3(Vec3::new(-10.0, -5.0, 0.0));
        assert!((p.x - 90.0).abs() < 1e-4);
        assert!((p.z - 195.0).abs() < 1e-4);
    }

    #[test]
    fn live_placement_preview_is_model_ghost_not_footprint_circle() {
        // C++ InGameUI.cpp:2957 placeBuildAvailable creates a ThingFactory Drawable,
        // not a selection-circle overlay.
        let src = include_str!("start_game.rs");
        let live = src
            .split("#[cfg(test)]")
            .next()
            .expect("start_game live path");
        let start = live
            .find("fn collect_ground_marker_circles")
            .expect("collect_ground_marker_circles");
        let body = &live[start..start + 900];
        assert!(
            body.contains("submit_structure_placement_model_ghost"),
            "live placement must submit the building model ghost"
        );
        assert!(
            !body.contains("[0.15, 0.95, 0.2, 0.45]"),
            "live placement must not draw the green/red footprint circle"
        );
        assert!(
            live.contains("STRUCTURE_PLACEMENT_GHOST_OPACITY")
                && live.contains("add_faction_bib")
                && live.contains("construction_tint"),
            "live ghost must carry C++ 0.45 opacity, illegal tint, and faction bibs"
        );
    }

    #[test]
    fn script_camera_shakers_rotate_not_translate() {
        let src = include_str!("start_game.rs");
        let start = src
            .find("fn update_script_camera_shake")
            .expect("update_script_camera_shake");
        let body = &src[start..start + 1800];
        assert!(
            body.contains("script_camera_shaker_rotations")
                && body.contains("camera_shake_rotation")
                && body.contains("SHAKE_AXIS_PITCH"),
            "C++ CameraShakerSystem Compute_Rotations must drive axis-capped rotation"
        );
        assert!(
            !body.contains("offset.x += phase_a.sin() * magnitude"),
            "script shakers must not translate the look-at"
        );
        assert!(
            body.contains("impulse_shake_offset") && !body.contains("tick_impulse_shake"),
            "live host must apply leftover ViewShake offset after the presentation-shell tick, not decay it a second time"
        );
    }

    #[test]
    fn script_camera_shaker_steps_one_thirtieth_per_present() {
        let src = include_str!("start_game.rs");
        let start = src
            .find("fn update_script_camera_shake")
            .expect("update_script_camera_shake");
        let body = &src[start..start + 900];
        assert!(
            body.contains("1.0 / 30.0"),
            "C++ CameraShakerSystem.Timestep is 1/30 per present"
        );
        assert!(
            !body.contains("elapsed_seconds += dt"),
            "script shaker must not advance by visual_dt"
        );
    }

    #[test]
    fn script_camera_pitch_writes_fx_pitch_not_orbit() {
        let src = include_str!("start_game.rs");
        assert!(
            src.contains("camera_fx_adjusted_eye_and_look")
                && src.contains("script_pitch_is_fx_pitch")
                && src.contains("self.camera_fx_pitch = target"),
            "CAMERA_PITCH must lerp FXPitch, not orbit radians"
        );
        assert!(
            !src.contains("let _ = pitch;"),
            "script pitch must not discard the authored FXPitch"
        );
        assert!(
            src.contains("tactical_view_height_frac")
                && src.contains("rebuild_tactical_projection"),
            "default control bar must shrink the live 3D frustum"
        );
    }

    #[test]
    fn shaker_uses_cpp_intensity_and_omega_band() {
        let shaker = ScriptCameraShaker::new(Vec3::ZERO, 500.0, 2.0, 15.0);
        assert!((shaker.intensity - 15.0_f32.to_radians()).abs() < 1e-5);
        for axis in [shaker.omega.x, shaker.omega.y, shaker.omega.z] {
            assert!(
                axis >= SHAKE_MIN_OMEGA - 1e-4 && axis <= SHAKE_MAX_OMEGA + 1e-4,
                "omega {axis} must be in 12.5-15Hz"
            );
        }
        let mut shaker = shaker;
        shaker.elapsed_seconds = 0.1;
        shaker.phi = Vec3::ZERO;
        let rot = script_camera_shaker_rotations(&shaker, Vec3::new(0.0, 50.0, 0.0));
        let rot = Vec3::new(
            rot.x.clamp(-SHAKE_AXIS_PITCH, SHAKE_AXIS_PITCH),
            rot.y.clamp(-SHAKE_AXIS_YAW, SHAKE_AXIS_YAW),
            rot.z.clamp(-SHAKE_AXIS_ROLL, SHAKE_AXIS_ROLL),
        );
        assert!(rot.length_squared() > 0.0);
    }

    #[test]
    fn shaker_uses_3d_eye_to_epicenter_distance() {
        let mut shaker = ScriptCameraShaker::new(Vec3::ZERO, 350.0, 2.0, 15.0);
        shaker.elapsed_seconds = 0.0;
        shaker.phi = Vec3::splat(std::f32::consts::FRAC_PI_2);
        shaker.omega = Vec3::splat(SHAKE_MIN_OMEGA);
        // 2D (x,z) would treat (0,400,0) as on-epicenter; C++ 3D is out of radius.
        let rot = script_camera_shaker_rotations(&shaker, Vec3::new(0.0, 400.0, 0.0));
        assert_eq!(rot, Vec3::ZERO);
        let on_epicenter = script_camera_shaker_rotations(&shaker, Vec3::ZERO);
        assert!(on_epicenter.length_squared() > 0.0);
    }

    #[test]
    fn look_at_uses_airborne_terrain_ray() {
        let src = include_str!("start_game.rs");
        let start = src
            .find("fn host_center_camera_on")
            .expect("host_center_camera_on");
        let body = &src[start..start + 900];
        assert!(
            body.contains("airborne_look_at_ground") && body.contains("PATHFIND_CELL_SIZE_F"),
            "C++ W3DView::lookAt airborne ray must be live"
        );
        assert!(
            src.contains("w3d_camera_constraint_offset"),
            "clamp must use W3DView inset, not raw map extent"
        );
    }

    #[test]
    fn scripted_camera_constraint_widens_like_cpp() {
        let first = widen_scripted_camera_constraint(None, -50.0, 900.0);
        assert_eq!(first, (-50.0, -50.0, 900.0, 900.0));
        let second = widen_scripted_camera_constraint(Some(first), 10.0, -20.0);
        assert_eq!(second, (-50.0, 10.0, -20.0, 900.0));
        let (lo_x, hi_x, lo_z, hi_z) =
            apply_scripted_camera_constraint_widen(0.0, 100.0, 0.0, 100.0, Some(second));
        assert_eq!((lo_x, hi_x, lo_z, hi_z), (-50.0, 100.0, -20.0, 900.0));
        assert_eq!((-80.0_f32).clamp(lo_x, hi_x), -50.0);
        assert_eq!(900.0_f32.clamp(lo_z, hi_z), 900.0);
    }

    #[test]
    fn scripted_center_widens_camera_constraint() {
        let src = include_str!("start_game.rs");
        let start = src
            .find("fn host_center_camera_on_impl")
            .expect("host_center_camera_on_impl");
        let body = &src[start..start + 1200];
        assert!(
            body.contains("widen_scripted_camera_constraint")
                && body.contains("if !clamp_to_world"),
            "scripted center must widen m_cameraConstraint"
        );
        assert!(
            src.contains("apply_scripted_camera_constraint_widen"),
            "player clamp must honor scripted widen"
        );
    }

    #[test]
    fn scripted_zoom_pitch_yaw_pause_with_game() {
        let src = super::ENGINE_SRC;
        assert!(
            src.contains("scripted_camera_motion_dt")
                && src.contains("GameState::Paused")
                && src.contains("self.game_paused"),
            "scripted zoom/pitch/yaw must freeze while paused"
        );
        assert!(
            src.contains("self.update_script_camera_shake(dt)"),
            "shake must keep ticking with visual dt under time-freeze"
        );
        assert!(
            !src.contains("shake_dt = if self.presentation_or_boot_time_frozen()"),
            "C++ CameraShaker Timestep has no script time-freeze gate"
        );
    }

    #[test]
    fn camera_drain_keeps_shaker_epicenter() {
        let src = include_str!("camera_drain.rs");
        let start = src
            .find("for &(position, amplitude, duration_seconds, radius)")
            .expect("shaker drain tuple");
        let body = &src[start..start + 280];
        assert!(
            body.contains("position: Vec3::new(position[0]")
                && !body.contains("position: self.camera_target"),
            "CAMERA_ADD_SHAKER must keep the scripted waypoint"
        );
    }
}
