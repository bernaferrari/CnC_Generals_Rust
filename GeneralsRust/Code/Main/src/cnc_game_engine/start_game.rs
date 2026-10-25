#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::mouse::{
    LOOKAT_DEFAULT_PITCH_DEG, PATHFIND_CELL_SIZE_F, SHAKE_AXIS_PITCH, SHAKE_AXIS_ROLL,
    SHAKE_AXIS_YAW, SHAKE_END_OMEGA, SHAKE_MAX_OMEGA, SHAKE_MIN_OMEGA,
    w3d_camera_constraint_offset,
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

pub(super) fn constrain_camera_target_to_map(
    mut position: Vec3,
    world_min: Vec3,
    world_max: Vec3,
    inset: f32,
    scripted_widen: Option<(f32, f32, f32, f32)>,
) -> Vec3 {
    crate::presentation_frame::camera::constraints::ConstraintBounds::for_map(
        world_min, world_max, inset,
    )
    .widen(scripted_widen)
    .constrain(position)
}

pub(super) fn native_camera_constraint_inset(
    camera: &super::camera_height::CameraHeight,
    view: Mat4,
    projection: Mat4,
    viewport: crate::presentation_frame::camera::viewport::TacticalViewport,
) -> f32 {
    // C++ calcCameraConstraints uses m_groundLevel, not View::m_pos.z.
    viewport.constraint_inset(view, projection, camera.basis().ground_level())
}

pub(super) use crate::presentation_frame::camera::shaker::script_camera_shaker_rotations;

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
        #[cfg(feature = "game_client")]
        dynamic_lights: Vec::new(),
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
        #[cfg(feature = "game_client")]
        dynamic_lights: Vec::new(),
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

/// Minimum slot count for a synthesized Skirmish: the local human plus at
/// least one registered AI opponent. A C++ GameInfo session never launches a
/// zero-opponent skirmish, so neither may the synthesized fallback.
const HOST_SYNTHESIZED_SKIRMISH_MIN_SLOTS: usize = 2;

/// Slot palette for starts with no C++ GameInfo MultiplayerSettings colors.
/// Slots 0/1 mirror the golden host pair (blue human, red first AI).
const HOST_SYNTHESIZED_SKIRMISH_COLORS: [(u8, u8, u8); 8] = [
    (0, 0, 200),
    (200, 0, 0),
    (0, 200, 0),
    (200, 200, 0),
    (0, 200, 200),
    (200, 0, 200),
    (200, 120, 0),
    (120, 120, 120),
];

/// Map SidesList `playerFaction`/`playerName` identifiers to Main base-side
/// labels. Exact base sides only: civilians, observers, neutral sides, and
/// unknown authored sides are not skirmish competitors.
fn host_base_faction_from_map_side(side_value: &str) -> Option<&'static str> {
    let value = side_value.trim().to_ascii_lowercase();
    let side = value
        .strip_prefix("faction")
        .or_else(|| value.strip_prefix("plyr"))
        .unwrap_or(value.as_str());
    match side {
        "america" | "usa" | "us" => Some("USA"),
        "china" => Some("China"),
        "gla" => Some("GLA"),
        _ => None,
    }
}

/// Resolve a requested local faction label into the pair every synthesized
/// slot needs: a display label plus the authoritative PlayerTemplate
/// selection. `Random` keeps C++ `PLAYERTEMPLATE_RANDOM` semantics and is
/// resolved once by the seeded authority path; an unrecognized label is
/// retained verbatim so `apply_skirmish_config` fails closed on it.
fn host_synthesized_player_template(
    faction: &str,
) -> (
    String,
    crate::skirmish_config::SkirmishPlayerTemplateSelection,
) {
    if faction.trim().eq_ignore_ascii_case("random") {
        return (
            "Random".to_string(),
            crate::skirmish_config::SkirmishPlayerTemplateSelection::Random,
        );
    }
    match host_base_faction_from_map_side(faction) {
        Some(base) => (
            base.to_string(),
            crate::skirmish_config::SkirmishPlayerTemplateSelection::base_faction(base),
        ),
        None => {
            let label = faction.trim().to_string();
            let selection =
                crate::skirmish_config::SkirmishPlayerTemplateSelection::base_faction(&label);
            (label, selection)
        }
    }
}

/// Build Skirmish slots for a start that carried no C++ GameInfo config.
///
/// Slot 0 is the local human on its requested faction; every remaining
/// authored map start becomes a Medium AI on the map's next playable side
/// (cycling through the base sides when the map lists fewer). Alliance teams
/// stay one per slot (free-for-all) like the C++ skirmish lobby defaults.
fn host_synthesize_skirmish_slots(
    start_positions: &[i32],
    map_side_factions: &[&'static str],
    human_faction: &str,
) -> Vec<crate::skirmish_config::SkirmishSlotConfig> {
    use crate::skirmish_config::SkirmishPlayerTemplateSelection;

    let mut starts: Vec<i32> = start_positions.to_vec();
    if starts.len() < HOST_SYNTHESIZED_SKIRMISH_MIN_SLOTS {
        // Unknown map shape: keep the golden two-slot default rather than a
        // zero-opponent match.
        starts = (0..HOST_SYNTHESIZED_SKIRMISH_MIN_SLOTS as i32).collect();
    }
    starts.truncate(crate::ui::skirmish_menu::MAX_SLOTS);

    let (human_label, human_template) = host_synthesized_player_template(human_faction);
    let fallback_factions = ["USA", "GLA", "China"];
    let mut map_factions = map_side_factions.iter().copied();
    let mut fallback_index = 0usize;

    starts
        .iter()
        .enumerate()
        .map(|(slot_index, &start_position)| {
            let is_human = slot_index == 0;
            let (faction, player_template, ai_difficulty, player_name) = if is_human {
                (
                    human_label.clone(),
                    human_template.clone(),
                    None,
                    "Player".to_string(),
                )
            } else {
                let faction = map_factions.next().unwrap_or_else(|| {
                    let fallback = fallback_factions[fallback_index % fallback_factions.len()];
                    fallback_index += 1;
                    fallback
                });
                (
                    faction.to_string(),
                    SkirmishPlayerTemplateSelection::base_faction(faction),
                    Some("Medium".to_string()),
                    format!("{faction} AI"),
                )
            };
            crate::skirmish_config::SkirmishSlotConfig {
                slot_index,
                is_human,
                is_active: true,
                player_template,
                faction,
                color_rgb: HOST_SYNTHESIZED_SKIRMISH_COLORS
                    [slot_index % HOST_SYNTHESIZED_SKIRMISH_COLORS.len()],
                team: slot_index as i32,
                start_position,
                player_name,
                ai_difficulty,
            }
        })
        .collect()
}

/// Read the map's player-start and SidesList shape for slot synthesis.
///
/// Both sources come from a single chunky parse; a missing or unparseable map
/// yields the empty shape and the synthesizer keeps its two-slot default.
fn host_map_skirmish_shape(map_name: &str) -> (Vec<i32>, Vec<&'static str>) {
    use crate::game_logic::script_loader::{
        load_chunky_map, parse_player_start_waypoints_from_chunky, parse_runtime_sides_from_chunky,
    };
    use game_engine::common::well_known_keys::{key_player_faction, key_player_name};

    let Ok(Some(chunky)) = load_chunky_map(map_name) else {
        return (Vec::new(), Vec::new());
    };
    let starts = parse_player_start_waypoints_from_chunky(&chunky).unwrap_or_default();
    let start_positions: Vec<i32> = starts.iter().map(|(idx, _, _)| *idx as i32).collect();
    let side_factions: Vec<&'static str> = parse_runtime_sides_from_chunky(&chunky)
        .map(|sides| {
            sides
                .side_dicts
                .iter()
                .filter_map(|dict| {
                    let faction = dict.get_ascii_string(key_player_faction());
                    let name = dict.get_ascii_string(key_player_name());
                    host_base_faction_from_map_side(&faction)
                        .or_else(|| host_base_faction_from_map_side(&name))
                })
                .collect()
        })
        .unwrap_or_default();
    (start_positions, side_factions)
}

/// Synthesize the C++ GameInfo-equivalent skirmish config for a start that
/// arrived without one (runtime-host `start_game`, WND NewGame drains).
///
/// C++ `GameLogic::startNewGame` (GameLogic.cpp:1494-1532) consumes GameInfo
/// slots — PlayerTemplate, cash, and computer players per slot — for every
/// skirmish launch; this reconstruction keeps that contract against the map
/// itself: one slot per authored player start, the human on its requested
/// faction, Medium AI everywhere else, and the default rules (10k cash).
fn host_synthesize_skirmish_config(
    map_name: &str,
    human_faction: &str,
) -> crate::skirmish_config::SkirmishMatchConfig {
    let (start_positions, map_side_factions) = host_map_skirmish_shape(map_name);
    crate::skirmish_config::SkirmishMatchConfig {
        source_game_info: None,
        map: map_name.to_string(),
        // There is no live GameInfo seed here; 0 is the deterministic seed
        // the golden host config uses for the same situation.
        random_seed: 0,
        old_factions_only: false,
        rules: crate::skirmish_config::GameRulesSnapshot::default_rules(),
        slots: host_synthesize_skirmish_slots(&start_positions, &map_side_factions, human_faction),
    }
}

use crate::presentation_frame::camera::pose::camera_fx_pose;
pub(super) use crate::presentation_frame::camera::pose::{camera_orbit_pose, camera_orbit_view};

impl CnCGameEngine {
    /// GameLogic.cpp1143: each admitted new game disables TiVO. Detached
    /// startup/restore candidates cannot write the receiving Engine; their
    /// reset is admitted at commit, before any saved Native continuation.
    pub(super) fn reset_replay_pacing_for_new_game(&mut self) {
        self.replay_fast_forward = false;
    }

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
        let Ok(decoder) = rodio_compat::Decoder::new(cursor) else {
            return;
        };
        let source = decoder;
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
    pub(super) fn start_game_from_ui(&mut self, request: HostStartRequest) -> Result<(), String> {
        // Wave 611: thin wrapper — residual via host helper.
        self.host_start_game_from_ui(request)
    }

    /// Restart the simulation with UI-selected parameters and refresh view/minimap.
    pub(super) fn host_start_game_from_ui(
        &mut self,
        request: HostStartRequest,
    ) -> Result<(), String> {
        let _ = self.park_host_start_request(request, false)?;
        Ok(())
    }

    pub(super) fn host_start_game_from_command_phase(
        &mut self,
        request: HostStartRequest,
    ) -> Result<bool, String> {
        self.park_host_start_request(request, true)
    }

    fn park_host_start_request(
        &mut self,
        request: HostStartRequest,
        source_command: bool,
    ) -> Result<bool, String> {
        self.require_native_world_boundary("park new match")
            .map_err(|error| error.to_string())?;
        let HostStartRequest {
            mode,
            faction,
            map,
            skirmish,
            player_template,
            game_info_starting_cash,
            is_campaign_session,
            is_challenge_session,
            difficulty_code,
            recorder_dispatch,
        } = request;

        // C++ MSG_NEW_GAME GAME_SHELL is Shell::showShellMap, not a match start.
        // Loading/headless/runtime-host drains that treat it as UI start load
        // Defcon6 and abandon the ShellMapMD worker.
        if matches!(mode, GameMode::Shell) && !source_command {
            info!("host_start_game_from_ui: ignore GAME_SHELL (shell map, not a match)");
            return Ok(false);
        }

        // A typed shell descriptor is only meaningful for C++ single-player
        // Campaign/Challenge GameInfo.  Do not let an accidental direct
        // skirmish/runtime call attach it to a different start path.
        if player_template.is_some() && !matches!(mode, GameMode::SinglePlayer) {
            warn!(
                "Rejecting PlayerTemplate identity on non-single-player start mode {:?}",
                mode
            );
            return Ok(false);
        }

        let faction_team = Self::team_from_faction(&faction);
        if let Some(player_template) = player_template.as_ref() {
            if player_template.base_team() != Some(faction_team) {
                warn!(
                    "Rejecting PlayerTemplate '{}' whose exact base side no longer matches requested team {:?}",
                    player_template.template_name, faction_team
                );
                return Ok(false);
            }
        }

        // Capture the provenance before this function moves Menu → Loading.
        // The Loading NewGame drain (camera_drain.rs) calls this while state is
        // already Loading. Only that state may reuse latched menu evidence.
        // An InGame restart or a host start_game must not.
        let interactive_start_from_menu = self.current_state == GameState::Menu
            || (self.current_state == GameState::Loading
                && self.interactive_playability.menu_wnd_click
                && self.interactive_playability.skirmish_path);
        // Offline provenance is applied when the parked start reaches InGame.
        // Wave 611: host residual helper.
        // A still-running boot worker must not overwrite this match start (or
        // keep finalize_startup_map_load pointed at Menu) after we leave Loading.
        self.abandon_startup_load_worker();
        info!("host_start_game_from_ui: abandoned boot worker");
        // Mark MainMenu as starting a match so shutdown skips reverse-animate
        // (windowed host start was SIGSEGV'ing mid MainMenuShutdown / hide_shell).
        #[cfg(feature = "game_client")]
        if mode != GameMode::Shell {
            let _ = self.with_receiving_menu_effects(|effects| {
                game_client::gui::mark_host_match_start(effects)
            });
        }
        if source_command && mode == GameMode::SinglePlayer {
            self.destroy_source_new_game_background();
        }
        // Show loading screen before starting map load (matches C++ loading screen flow)
        #[cfg(feature = "game_client")]
        self.prepare_cpp_load_screen_for_mode(mode, false);
        info!("host_start_game_from_ui: load screen prepared");
        if mode == GameMode::Shell {
            // Source shell preparation retains MainMenu and does not hideShell.
            // Only the application loading pump is selected for the parked map.
            self.current_state = GameState::Loading;
        } else {
            self.transition_to_state(GameState::Loading)?;
        }
        info!("host_start_game_from_ui: state=Loading");
        // Prelude playback is advanced by subsequent Loading ticks, allowing
        // winit to deliver focus/close events before the next decoder step.
        #[cfg(feature = "game_client")]
        if let Some(kind) = self.active_load_screen {
            game_client::gui::load_screen::begin_nonblocking_load_screen_prelude(
                kind,
                self.game_client.load_screen_runtime_mut(),
            );
        }
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
                game_info_starting_cash,
                is_campaign_session,
                is_challenge_session,
                difficulty_code,
                recorder_dispatch,
            },
            interactive_start_from_menu,
            prelude_retry_at: std::time::Instant::now(),
        });
        info!("host_start_game_from_ui: parked match start for next Loading tick");
        Ok(true)
    }

    /// Finish a parked UI start: `host_load_map_or_default` then InGame.
    ///
    /// Called from the Loading tick after status has been published. Still
    /// the live `load_map` path (hq-ibnf owns making Lone Eagle return).
    pub(super) fn complete_parked_match_start(
        &mut self,
        pending: PendingMatchStart,
    ) -> Result<(), String> {
        self.require_native_world_boundary("complete parked match")
            .map_err(|error| error.to_string())?;
        let HostStartRequest {
            mode,
            faction,
            map,
            skirmish,
            player_template,
            game_info_starting_cash,
            is_campaign_session,
            is_challenge_session,
            difficulty_code,
            recorder_dispatch,
        } = pending.request;
        let interactive_start_from_menu = pending.interactive_start_from_menu;
        let offline_mode = matches!(mode, GameMode::SinglePlayer | GameMode::Skirmish);
        let faction_team = Self::team_from_faction(&faction);
        // Capture playback's retained header before subsystem reset can reset
        // the application Recorder. It is input to this match, not a global
        // broadcast that another already-running world must adopt.
        let replay_inputs = if mode == GameMode::Replay {
            let recorder = self.game_logic.replay_pending.recorder_owned();
            if !recorder.is_playback() {
                warn!("Rejecting Replay start without receiving admitted Recorder");
                self.return_to_main_menu_after_match()?;
                return Ok(());
            }
            let info = recorder.get_game_info();
            Some((info.seed, info.starting_cash))
        } else {
            None
        };

        let reset = if replay_inputs.is_some() {
            self.host_reset_for_admitted_replay_start()
        } else if mode == GameMode::Shell {
            self.host_reset_for_clear_game_data()
        } else {
            self.host_game_engine_reset()
        };
        reset.map_err(|error| error.to_string())?;
        self.reset_replay_pacing_for_new_game();
        // Wave 843/844/871: clear prior match residuals until load completes.
        self.host_clear_match_residuals();
        info!("host_start_game_from_ui: match residuals cleared");
        if let Some((seed, _)) = replay_inputs {
            self.host_game_logic_mut().set_logic_random_seed(seed);
        } else if matches!(mode, GameMode::SinglePlayer | GameMode::Shell) {
            // Campaign/Challenge and Shell starts select seed 0 in C++ menus. Admission
            // targets this session instead of relying on a process broadcast.
            self.host_game_logic_mut().set_logic_random_seed(0);
        }

        // Wave 169/840: empty UI map → DEFAULT_SKIRMISH_MAP (Defcon6) before
        // shell-residual rejection. Matches C++ startNewGame default map residual.
        let map = if map.trim().is_empty() && mode != GameMode::Shell {
            DEFAULT_SKIRMISH_MAP.to_string()
        } else {
            map
        };
        // Wave 840: never start skirmish on boot ShellMapMD residual when a real map exists.
        let map_name = if mode == GameMode::Shell {
            map
        } else {
            Self::resolve_skirmish_start_map_name(mode, map)
        };

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

        if mode == GameMode::Replay && skirmish.is_some() {
            let config = skirmish.as_ref().expect("checked receiving replay slots");
            if let Err(error) = crate::skirmish_config::apply_skirmish_config_for_mode(
                &mut self.game_logic,
                config,
                GameMode::Replay,
            ) {
                warn!("Rejecting recorded GameInfo admission before map load: {error}");
                self.return_to_main_menu_after_match()?;
                return Ok(());
            }
            if let Some(local) = self.game_logic.local_player_id() {
                self.current_player_id = local;
            }
        } else if mode == GameMode::Replay {
            // All-closed original single-player headers are driven by authored
            // map sides, not by the generic USA/AI starter roster.
            self.game_logic.start_new_game_with_recorder_payload(
                GameMode::Replay,
                difficulty_code.unwrap_or(1),
                0,
                30,
            );
            self.game_logic.clear_all_players();
        } else if mode == GameMode::Skirmish {
            if let Some(ref config) = skirmish {
                if let Err(e) = self.host_apply_skirmish_config_authority(config) {
                    // A supplied C++ GameInfo-derived config has selected
                    // exact PlayerTemplate identities. Never substitute a
                    // Team-only legacy start when one is stale/locked: that
                    // would launch a plausible but wrong General match.
                    warn!("Rejecting configured Skirmish start before map load: {e}");
                    self.return_to_main_menu_after_match()?;
                    return Ok(());
                }
            } else {
                // C++ `GameLogic::startNewGame` (GameLogic.cpp:1494-1532)
                // consumes GameInfo slots — PlayerTemplate, cash, and
                // computer players per slot — for every skirmish launch. A
                // start without an explicit config still needs those slots,
                // so synthesize them from the map's player/start data and
                // route the same skirmish-config authority instead of the
                // Team-only legacy bootstrap (hard-coded players, default
                // supplies, and no registered AI).
                let synthesized = host_synthesize_skirmish_config(&map_name, &faction);
                info!(
                    "host_start_game_from_ui: synthesized skirmish config for '{}' ({} active slots, human faction {})",
                    map_name,
                    synthesized.slots.iter().filter(|s| s.is_active).count(),
                    faction
                );
                match self.host_apply_skirmish_config_authority(&synthesized) {
                    Ok(()) => {}
                    Err(e) => {
                        warn!("Rejecting synthesized Skirmish start before map load: {e}");
                        self.return_to_main_menu_after_match()?;
                        return Ok(());
                    }
                }
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
                        is_challenge_session,
                    ) {
                        // C++ never substitutes a base Team after a selected
                        // GameSlot PlayerTemplate fails to resolve.  Return to
                        // the shell before map load so Main cannot create a
                        // plausible-looking but wrong General match.
                        self.return_to_main_menu_after_match()?;
                        return Ok(());
                    }
                }
                None => {
                    // Wave 577: host start residual via helper (non-skirmish).
                    info!(
                        "host_start_game_from_ui: host_start_new_game_with_faction(non-skirmish)"
                    );
                    self.host_start_new_game_with_faction_in_session(
                        mode,
                        faction_team,
                        false,
                        is_campaign_session,
                        is_challenge_session,
                    );
                }
            }
        }
        if let Some(code) = difficulty_code {
            self.host_game_logic_mut()
                .prepare_native_command_difficulty(code);
        }
        self.game_logic
            .admit_campaign_session(is_campaign_session, is_challenge_session);
        #[cfg(feature = "game_client")]
        if !is_campaign_session {
            let campaign_manager = self.game_client.campaign_manager_mut();
            campaign_manager.set_campaign("");
            campaign_manager.clear_challenge_game_info();
            campaign_manager.set_victorious(false);
        } else {
            #[cfg(feature = "game_client")]
            self.game_client
                .campaign_manager_mut()
                .set_victorious(false);
        }
        self.destroy_source_new_game_background();
        if !matches!(mode, GameMode::Replay | GameMode::Shell) {
            let seed = self.game_logic.logic_base_seed;
            self.game_logic
                .replay_pending
                .reset_recorder_for_match_owned(map_name.clone(), seed);
            let difficulty = recorder_dispatch
                .map(|dispatch| dispatch.difficulty_code)
                .or(difficulty_code)
                .unwrap_or(1);
            let rank = recorder_dispatch
                .map(|dispatch| dispatch.rank_points)
                .unwrap_or(0);
            let max_fps = recorder_dispatch
                .and_then(|dispatch| dispatch.max_fps)
                .unwrap_or(30);
            let recordable = if let Some(admission) =
                self.game_logic.initial_skirmish_admission.as_ref()
            {
                match crate::skirmish_config::replay_admission::recording_info(admission) {
                    Ok((info, local)) => {
                        self.game_logic
                            .replay_pending
                            .recording_lobby_owned(info, local);
                        true
                    }
                    Err(error) => {
                        warn!(
                            "Cannot represent native admission in GENREP: {error}; recording disabled for this start"
                        );
                        false
                    }
                }
            } else {
                true
            };
            if recordable {
                self.game_logic
                    .replay_pending
                    .record_new_game_owned(mode, seed, difficulty, rank, max_fps);
            }
        }
        if mode != GameMode::Skirmish {
            let default_starting_cash = self.default_starting_cash;
            self.host_game_logic_mut().admit_new_game_starting_cash(
                replay_inputs
                    .map(|(_, cash)| cash)
                    .or(game_info_starting_cash),
                default_starting_cash,
            );
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
            self.return_to_main_menu_after_match()?;
            return Ok(());
        };
        let initial_rank = recorder_dispatch.map_or(0, |dispatch| dispatch.rank_points);
        self.game_logic
            .set_rank_points_to_add_at_game_start(initial_rank);
        if mode == GameMode::SinglePlayer {
            self.game_logic
                .admit_native_initial_rank_points(initial_rank);
        }
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
        if let Err(error) = self.seed_presentation_after_match_start() {
            error!("Match start cannot complete native presentation: {error}");
            return Ok(());
        }
        if mode != GameMode::Shell {
            self.snap_camera_to_local_units_if_needed();
        }
        info!(
            "host_start_game_from_ui: transition to loaded mode {:?}",
            mode
        );
        self.transition_to_state(if mode == GameMode::Shell {
            GameState::Menu
        } else {
            GameState::InGame
        })?;
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
        if let Err(error) = self.ensure_presentation_env_seeded() {
            error!("Match start presentation environment unavailable: {error}");
            return Ok(());
        }
        Self::apply_heightmap_hint(&mut self.render_pipeline);
        Self::apply_skybox_hint(&mut self.render_pipeline);
        if let Err(error) = self.ensure_presentation_env_seeded() {
            error!("Match start presentation environment unavailable: {error}");
            return Ok(());
        }
        Self::sync_render_terrain_visual(
            &mut self.game_logic,
            &mut self.render_pipeline,
            &self.graphics_system,
            loaded_map_name.as_str(),
        );
        if let Err(err) = self.reinitialize_minimap_renderer() {
            warn!("Failed to reinitialize minimap renderer: {}", err);
        }

        // Apply map lighting if provided by map settings.
        if let Err(error) = self.ensure_presentation_env_seeded() {
            error!("Match start presentation environment unavailable: {error}");
            return Ok(());
        }
        Self::apply_map_lighting(&mut self.graphics_system, &mut self.render_pipeline);

        let startup_camera_defaults = Self::startup_camera_defaults_for(&self.game_logic);
        // Wave 458: prefer pipeline presentation freeze; live GameLogic only if missing.
        let startup_camera_presentation = self
            .render_pipeline
            .presentation_frame()
            .or(self.last_presentation_frame.as_deref());
        // Wave 540/552: prefer presentation fow_shell_bypass when freeze present.
        let in_shell_camera = self.shell_bypass_from_presentation(startup_camera_presentation);
        (
            self.game_logic.session_view.target,
            self.game_logic.session_view.camera.camera_position,
            self.game_logic.session_view.camera.camera_zoom,
            self.game_logic.session_view.camera.native_camera,
        ) = Self::bootstrap_native_camera_for_loaded_map(
            in_shell_camera,
            self.current_player_id,
            startup_camera_defaults,
            startup_camera_presentation,
        );
        self.game_logic.session_view.camera.camera_pitch_radians = 0.0;
        self.game_logic.session_view.camera.camera_yaw_radians = 0.0;
        self.initialize_receiving_replay_controls();
        // W3DView::setCameraTransform calculates camera constraints when
        // lookAt/setZoomToDefault builds the initial pose. The match camera
        // must apply that same viewport inset before its first player scroll.
        if let Err(error) = self.set_native_camera_transform() {
            log::error!("camera transform: {error}");
        }
        let clamped_target = self.clamp_to_world_bounds(self.game_logic.session_view.target);
        if clamped_target != self.game_logic.session_view.target {
            self.game_logic.session_view.target = clamped_target;
            if let Err(error) = self.set_native_camera_transform() {
                log::error!("camera transform: {error}");
            }
        }
        Ok(())
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
                .unwrap_or(self.game_logic.session_view.target);

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

        let dx = focus.x - self.game_logic.session_view.target.x;
        let dz = focus.z - self.game_logic.session_view.target.z;
        let dist_sq = dx * dx + dz * dz;
        let looking_at_boot_origin = {
            let tx = self.game_logic.session_view.target.x;
            let tz = self.game_logic.session_view.target.z;
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
            return;
        }
        // Focusing a location does not create a new map camera basis.
        self.game_logic.session_view.target = Vec3::new(focus.x, 0.0, focus.z);
        if let Err(error) = self.set_native_camera_transform() {
            log::error!("camera transform: {error}");
        }
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
    pub(super) fn ensure_presentation_env_seeded(&mut self) -> Result<(), String> {
        // Wave 467/474: seed pipeline presentation (host+GW) and mirror into last_presentation_frame
        self.ensure_presentation_env_for_hints()?;
        if self.last_presentation_frame.is_none() {
            self.last_presentation_frame = self.render_pipeline.presentation_frame_handle();
        }
        Ok(())
    }

    pub(super) fn ensure_presentation_env_for_hints(&mut self) -> Result<(), String> {
        // Wave 474: instance seed only — no free-fn GameLogic dual-read surface.
        // Wave 474/466/455/590: pipeline env seed via host helper.
        self.host_ensure_presentation_env_for_hints()
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
        game_logic: &mut crate::game_logic::GameLogic,
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

        // C++ WaterRenderObjClass::renderWater traverses the map's authored
        // PolygonTrigger water areas. This runs after both hint and frozen
        // runtime-heightmap hydration, including rematch and save restore.
        if let Some((areas, gpu_meshes)) = render_pipeline.sync_map_water_areas_from_presentation()
        {
            info!(
                "Installed authored map water: areas={} gpu_meshes={}",
                areas, gpu_meshes
            );
        }

        // C++ W3DRadar.cpp:977-993 builds the radar terrain texture at newMap
        // with WorldHeightMap tiles already resident; here the render pipeline
        // hydrates the visual (heightmap + source tiles) asynchronously, so
        // Radar::newMap may have painted the band black. Re-run
        // buildTerrainTexture once hydration completes (C++ parity:
        // W3DRadar::refreshTerrain, W3DRadar.cpp:1421-1432).
        game_logic.admit_radar_terrain_colors(
            render_pipeline.frozen_radar_terrain_colors(game_logic.radar()),
            game_logic.radar_water_color(),
        );

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
        self.ensure_presentation_env_seeded()
            .map_err(anyhow::Error::msg)?;
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
                        self.last_presentation_frame =
                            self.render_pipeline.presentation_frame_handle();
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
                    let location = self.clamp_command_position_to_map(world_pos);
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

    /// Actual receiving View dimensions; never recomputed from a fraction.
    pub(super) fn tactical_viewport_size(&self) -> (f32, f32) {
        let (width, height) = self.game_logic.session_view.camera.viewport.size();
        (width as f32, height as f32)
    }
    pub(super) fn rebuild_tactical_projection(&mut self) {
        self.game_logic.session_view.camera.rebuild_projection();
    }

    pub(super) fn parabolic_ease(param: f32, ease_in_time: f32, ease_out_time: f32) -> f32 {
        super::camera_motion::parabolic_ease(param, ease_in_time, ease_out_time)
    }

    /// The same source pose feeds rendering and transform-shaker observation.
    pub(super) fn native_camera_base_pose(&self) -> (Vec3, Vec3) {
        camera_orbit_pose(
            &self.game_logic.session_view.camera.native_camera,
            self.game_logic.session_view.target,
            self.game_logic.session_view.impulse.offset,
            self.game_logic.session_view.camera.camera_zoom,
            self.game_logic.session_view.camera.camera_pitch_radians,
            self.game_logic.session_view.camera.camera_yaw_radians,
            self.game_logic.session_view.camera.camera_fx_pitch,
            self.game_logic.session_view.camera.constraint_bounds(),
        )
    }

    pub(super) fn apply_camera_orbit_transform(&mut self) {
        if self
            .game_logic
            .session_view
            .camera
            .camera_slave_mode
            .is_some()
        {
            return;
        }
        let (source, look) = self.native_camera_base_pose();
        self.game_logic.session_view.camera.camera_position = source;
        self.game_logic.session_view.camera.view_matrix = camera_orbit_view(
            source,
            look,
            self.game_logic.session_view.camera.camera_shake_rotation,
        );
    }

    /// C++ `W3DView::buildCameraTransform` FXPitch (W3DView.cpp:362-381).
    pub(super) fn camera_fx_adjusted_eye_and_look(&self, source: Vec3, look: Vec3) -> (Vec3, Vec3) {
        camera_fx_pose(
            source,
            look,
            self.game_logic.session_view.camera.camera_fx_pitch,
        )
    }

    /// Catch camera mutations that arrived outside `update_camera` (minimap,
    /// selection hotkeys, script request drain).  They must not wait for an
    /// unrelated pan or screen shake before changing the displayed view.
    pub(super) fn camera_transform_needs_rebuild(&self) -> bool {
        let expected = self.native_camera_base_pose().0;
        !expected.is_finite()
            || !self
                .game_logic
                .session_view
                .camera
                .camera_position
                .is_finite()
            || (self.game_logic.session_view.camera.camera_position - expected).length_squared()
                > 1.0e-6
    }

    pub(super) fn sync_orbit_from_camera_transform(&mut self) {
        // Native v18 supplies a completed eye, not an unzoomed basis. Recover
        // angles against this map's existing basis; never absorb zoom/shake.
        if let Some(pitch) = self
            .game_logic
            .session_view
            .camera
            .native_camera
            .basis()
            .pitch_from_pose(
                self.game_logic.session_view.target,
                self.game_logic.session_view.camera.camera_position,
                self.game_logic.session_view.camera.camera_zoom,
                self.game_logic.session_view.camera.camera_yaw_radians,
            )
        {
            self.game_logic.session_view.camera.camera_pitch_radians = pitch;
        }

        self.game_logic.session_view.camera.camera_pitch_target = None;
        self.game_logic.session_view.camera.camera_pitch_start =
            self.game_logic.session_view.camera.camera_pitch_radians;
        self.game_logic.session_view.camera.camera_pitch_duration = 0.0;
        self.game_logic.session_view.camera.camera_pitch_frame = 0;
        self.game_logic.session_view.camera.camera_pitch_ease_in = 0.0;
        self.game_logic.session_view.camera.camera_pitch_ease_out = 0.0;

        self.game_logic
            .session_view
            .camera
            .rotation_registers
            .finish();
        // Positional View restore cancels activity; rcInfo timing and its
        // persistent union payload are not part of original View::xfer.

        if let Err(error) = self.set_native_camera_transform() {
            log::error!("camera transform: {error}");
        }
    }

    pub(super) fn apply_script_camera_pitch_request(&mut self, request: CameraPitchRequest) {
        self.game_logic.session_view.camera.install_pitch(
            request.pitch,
            request.duration_seconds,
            request.ease_in_seconds,
            request.ease_out_seconds,
        );
    }

    pub(super) fn apply_script_camera_rotate_request(&mut self, request: CameraRotateRequest) {
        self.game_logic.session_view.install_rotate(
            request.rotations,
            request.duration_seconds,
            request.ease_in_seconds,
            request.ease_out_seconds,
            self.game_logic.visual_speed_multiplier(),
        );
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
            self.replay_fast_forward,
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

    pub(super) fn enqueue_script_screen_shake(&mut self, intensity: i32) {
        // Historical queued intents retain once-only intake; all current
        // borrowed script operations have already applied before this phase.
        self.game_logic.admit_script_screen_shake(intensity);
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

        self.game_logic
            .session_view
            .camera
            .script_camera_shakers
            .push(ScriptCameraShaker::new(
                request.position,
                request.radius,
                request.duration_seconds,
                request.amplitude,
            ));
    }

    pub(super) fn update_script_camera_shake(&mut self) -> bool {
        let previous_offset = self.game_logic.session_view.impulse.offset;
        self.game_logic.session_view.advance_impulse();
        (self.game_logic.session_view.impulse.offset - previous_offset).length_squared() > 0.000001
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
        if self.game_logic.session_view.install_look_toward(
            self.game_logic.session_view.target,
            request.position,
            request.duration_seconds,
            request.ease_in_seconds,
            request.ease_out_seconds,
            request.reverse_rotation,
            self.game_logic.visual_speed_multiplier(),
        ) {}
    }

    /// Wave 611: via `host_center_camera_on`.
    ///
    /// LookAt uses the receiving terrain before the source transform operation.
    /// Waypoint motion widens its cache at its separate original update phase.
    pub(super) fn center_camera_on(&mut self, world_pos: Vec3) {
        self.host_center_camera_on_impl(world_pos)
    }

    /// C++ player `W3DView::lookAt` — cancel scripted rotate/path then snap.
    pub(super) fn host_player_look_at(&mut self, world_pos: Vec3) {
        self.cancel_scripted_camera_from_player_look_at();
        self.host_center_camera_on_impl(world_pos);
    }

    /// Presentation-frozen ground height under `world_pos`.
    /// Same sampler `host_center_camera_on` uses — no live GameLogic dual-read.
    pub(super) fn sample_presentation_height_under(&self, world_pos: Vec3) -> f32 {
        if let Some(pres) = self
            .render_pipeline
            .presentation_frame()
            .or(self.last_presentation_frame.as_deref())
        {
            pres.world_env
                .sample_height(world_pos.x, world_pos.z)
                .unwrap_or(self.game_logic.session_view.target.y)
        } else {
            // Wave 905: fail-closed boot height (no terrain_height_at dual-read).
            self.game_logic.session_view.target.y
        }
    }

    pub(super) fn host_center_camera_on(&mut self, world_pos: Vec3) {
        self.host_center_camera_on_impl(world_pos)
    }

    fn host_center_camera_on_impl(&mut self, world_pos: Vec3) {
        // LookAt samples the raw requested position and current receiving ray.
        // Constraint calculation/clamping belongs only to setCameraTransform,
        // after its actual provisional build when the cached bounds are invalid.
        let terrain_owner = Arc::clone(self.game_logic.world_services.terrain());
        let target = match terrain_owner.read() {
            Ok(terrain) => crate::presentation_frame::receiving_look_at_target(
                world_pos,
                &self.game_logic.session_view.camera,
                &terrain,
            ),
            Err(_) => Err("receiving LookAt terrain poisoned".into()),
        };
        match target {
            Ok(target) => {
                self.game_logic.session_view.target = target;
                if let Err(error) = self.set_native_camera_transform() {
                    log::error!("receiving LookAt transform: {error}");
                }
            }
            Err(error) => log::error!("receiving LookAt: {error}"),
        }
    }

    /// Radius-cursor / guard rings only. Structure placement is the C++ 3D model ghost.
    pub(super) fn collect_ground_marker_circles(
        &mut self,
    ) -> Vec<crate::graphics::selection_renderer::SelectedUnit> {
        use crate::graphics::selection_renderer::SelectedUnit;
        let mut out = Vec::new();

        // C++ InGameUI::placeBuildAvailable / handleBuildPlacements: translucent
        // building Drawable at placementOpacity 0.45 + faction bibs. Not a circle.
        self.submit_structure_placement_model_ghost();

        // Special-power / AttackMove / Guard radius cursor residual.
        if let Some(ov) = self.game_hud().construction_panel.radius_overlay() {
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
                    style: crate::graphics::selection_renderer::SelectionCircleStyle::Filled,
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
                    style: crate::graphics::selection_renderer::SelectionCircleStyle::Filled,
                });
            }
        }

        out
    }

    /// C++ `InGameUI::placeBuildAvailable` + `handleBuildPlacements`:
    /// spawn the building model at `placementOpacity` 0.45, red-tint when
    /// `isLegalBuildLocation` fails, and add/remove faction bibs.
    fn submit_structure_placement_model_ghost(&mut self) {
        let placement = self.game_hud().construction_panel.placement_preview();
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
            self.game_client.render_bridge_mut().submit(submission);
        }

        self.sync_structure_placement_faction_bibs(Some((world_x, world_z, facing, half, illegal)));
    }

    /// C++ `TheTerrainVisual->addFactionBibDrawable` when LBC != OK,
    /// `removeFactionBibDrawable` when legal.
    fn sync_structure_placement_faction_bibs(
        &mut self,
        active: Option<(f32, f32, f32, (f32, f32), bool)>,
    ) {
        #[cfg(feature = "game_client")]
        {
            let Some(visual) = self.render_pipeline.native_terrain_mut() else {
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

        let clamped = self.clamp_command_position_to_map(world_pos);

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
            .or(self.last_presentation_frame.as_deref())
        {
            frame.world_env.world_bounds_vec3()
        } else {
            self.host_world_bounds()
        }
    }

    pub(super) fn clamp_to_world_bounds(&self, position: Vec3) -> Vec3 {
        self.game_logic
            .session_view
            .camera
            .constrain_target_if_cached(position)
    }

    /// World-command admission clips to map extents, independently of the camera.
    fn clamp_command_position_to_map(&self, mut position: Vec3) -> Vec3 {
        let (min, max) = self.presentation_world_bounds();
        position.x = position.x.max(min.x).min(max.x);
        position.z = position.z.max(min.z).min(max.z);
        position
    }

    pub(super) fn drain_renderer_attachments(&mut self) {
        match ww3d_renderer_3d::Renderer::with_global_mut(|renderer| {
            Ok(renderer.take_pending_attachments())
        }) {
            Ok(records) if !records.is_empty() => {
                dispatch_attachments(records);
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
        if let Err(err) = self
            .subsystem_manager
            .get_mut::<crate::subsystem_manager::GameResultsQueueSubsystem>()
            .ok_or_else(|| anyhow::anyhow!("GameResultsQueue is not initialized"))
            .and_then(|subsystem| subsystem.queue_mut().enqueue(queued_summary))
        {
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
    pub(super) fn host_game_engine_reset(
        &mut self,
    ) -> std::result::Result<(), NativeBoundaryFailure> {
        self.host_game_engine_reset_with_recorder(true, true)
    }

    /// The admitted file was opened after the old world's actual clear. The
    /// native loading presentation must not close it a second time.
    pub(super) fn host_reset_for_admitted_replay_start(
        &mut self,
    ) -> std::result::Result<(), NativeBoundaryFailure> {
        self.host_game_engine_reset_with_recorder(false, true)
    }

    /// ClearGameData pushes its score layout before reset. C++ GameClient::reset
    /// does not reset Shell; only the next admitted start removes the old stack.
    pub(super) fn host_reset_for_clear_game_data(
        &mut self,
    ) -> std::result::Result<(), NativeBoundaryFailure> {
        self.host_game_engine_reset_with_recorder(true, false)
    }

    fn host_game_engine_reset_with_recorder(
        &mut self,
        reset_recorder: bool,
        reset_shell: bool,
    ) -> std::result::Result<(), NativeBoundaryFailure> {
        self.require_native_world_boundary("Engine reset")?;
        // Source Client/UI reset captures defaults from outgoing map rules,
        // before GlobalData overrides disappear. GameLogic reset may run again.
        self.game_logic.session_view.camera.settings =
            self.game_logic.session_view.camera.settings.reset_seed();
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
        if reset_recorder {
            // Reverse source subsystem reset reaches Recorder before GameLogic.
            self.reset_receiving_recorder_at_engine_boundary();
            // Reset is not a new-game controls creation phase. Hide only the
            // receiving existing transport before outgoing shell layouts reset.
            game_client::gui::with_window_manager(|manager| {
                game_client::gui::callbacks::replay_controls::hide_replay_controls_owned(
                    manager, false,
                );
            });
        }

        #[cfg(feature = "game_client")]
        let reset_result = self
            .subsystem_manager
            .reset_all_receiving_client(&mut self.game_client);
        #[cfg(not(feature = "game_client"))]
        let reset_result = self.subsystem_manager.reset_all();
        if let Err(err) = reset_result {
            warn!("GameEngine::reset TheSubsystemList->resetAll failed: {err}");
        }

        #[cfg(feature = "game_client")]
        self.reset_native_filter_selection();
        #[cfg(feature = "game_client")]
        self.control_bar.discard_pending_command_clicks();
        #[cfg(feature = "game_client")]
        if let Err(err) = self.game_client.reset() {
            warn!("GameEngine::reset GameClient::reset failed: {err}");
        }
        // The GameClient has destroyed its Drawables. Clear the presentation
        // bridge belonging to this engine's world, even if a save/load
        // candidate is temporarily the ambient active world.
        gamelogic::helpers::ClientVisualHandle::new(std::sync::Arc::clone(
            &self.host_game_logic().world_services,
        ))
        .clear_visual_state_for_reset();

        // TheShell teardown: C++ tears the shell screens down with the engine
        // on match start. Without this, a runtime-host `start_game` issued
        // while the WND menu stack is pushed (MainMenu +
        // SkirmishGameOptionsMenu) leaves those screens on the shell stack:
        // their windows keep drawing over the InGame world and their gadget
        // hit-testing eats every world click (drive11/m12: menu overlay over
        // Defcon6, physical LMB could never select a unit). Shell::reset pops
        // every screen via the immediate path.
        #[cfg(feature = "game_client")]
        if reset_shell {
            if let Err(err) = game_client::gui::ShellHandle::default().reset() {
                warn!("GameEngine::reset TheShell reset failed: {err}");
            }
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
        Ok(())
    }

    pub(super) fn reset_match_state(&mut self) -> std::result::Result<(), NativeBoundaryFailure> {
        info!("Resetting gameplay state after match completion");
        // C++ GameLogicDispatch / GameState / PopupSaveLoad call
        // TheGameEngine->reset() before tearing down match state.
        self.host_game_engine_reset()?;
        self.reset_match_state_after_engine_reset()
    }

    pub(super) fn reset_match_state_after_engine_reset(
        &mut self,
    ) -> std::result::Result<(), NativeBoundaryFailure> {
        self.require_native_world_boundary("match reset suffix")?;
        self.pending_match_start = None;
        self.destroy_source_new_game_background();
        self.drain_renderer_attachments();

        self.host_reset_game_logic()?;
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

        *self.game_hud_mut() = GameHUD::new();
        let (hud_w, hud_h) = super::types::render_surface_extent(&self.window);
        self.game_hud_mut().resize(hud_w, hud_h);

        self.game_logic.session_view.camera.camera_position = Vec3::new(0.0, 200.0, 200.0);
        self.game_logic.session_view.target = Vec3::new(0.0, 0.0, 0.0);
        self.game_logic
            .session_view
            .camera
            .scripted_camera_constraint_widen = None;
        self.game_logic.session_view.camera.camera_fx_pitch = 1.0;
        self.game_logic.session_view.camera.invalidate_constraints();
        self.game_logic.session_view.camera.camera_zoom = 1.0;
        self.game_logic.session_view.camera.camera_zoom_target = None;
        self.game_logic.session_view.camera.camera_zoom_start =
            self.game_logic.session_view.camera.camera_zoom;
        self.game_logic.session_view.camera.camera_zoom_duration = 0.0;
        self.game_logic.session_view.camera.camera_zoom_frame = 0;
        self.game_logic.session_view.camera.camera_zoom_ease_in = 0.0;
        self.game_logic.session_view.camera.camera_zoom_ease_out = 0.0;
        self.game_logic.session_view.impulse.offset = Vec3::ZERO;
        self.game_logic.session_view.camera.camera_shake_rotation = Vec3::ZERO;
        self.game_logic.session_view.impulse.intensity = 0.0;
        self.game_logic.session_view.impulse.cosine = 0.0;
        self.game_logic.session_view.impulse.sine = 0.0;
        self.game_logic
            .session_view
            .camera
            .script_camera_shakers
            .clear();
        self.script_fps_limit = None;
        self.script_fps_limit_last_tick = None;
        self.game_logic.session_view.camera.camera_slave_mode = None;
        let defaults = Self::startup_camera_defaults_for(&self.game_logic);
        self.game_logic.session_view.camera.native_camera = super::camera_height::CameraHeight::new(
            defaults.camera_height,
            defaults.pitch_degrees,
            defaults.yaw_degrees,
        );
        self.game_logic.session_view.camera.camera_pitch_radians = 0.0;
        self.game_logic.session_view.camera.camera_yaw_radians = 0.0;
        self.game_logic.session_view.camera.camera_pitch_target = None;
        self.game_logic
            .session_view
            .camera
            .rotation_registers
            .finish();
        if let Err(error) = self.set_native_camera_transform() {
            log::error!("camera transform: {error}");
        }
        self.rebuild_tactical_projection();
        Ok(())
    }

    pub(super) fn return_to_main_menu_after_match(&mut self) -> Result<(), String> {
        self.reset_match_state()
            .map_err(|error| error.to_string())?;
        self.transition_to_state(GameState::Menu)
    }

    pub(super) fn exit_to_main_menu_from_victory(&mut self) -> Result<(), String> {
        self.return_to_main_menu_after_match()
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
    fn initial_camera_target_near_map_edge_uses_w3d_viewport_inset() {
        let world_min = Vec3::new(0.0, 0.0, 0.0);
        let world_max = Vec3::new(3_500.0, 0.0, 3_500.0);
        let target = Vec3::new(50.0, 12.5, 3_450.0);
        let constrained = constrain_camera_target_to_map(target, world_min, world_max, 200.0, None);
        assert_eq!(constrained, Vec3::new(200.0, 12.5, 3_300.0));
        assert_eq!(
            constrain_camera_target_to_map(
                Vec3::new(477.0, 12.5, 1_714.0),
                world_min,
                world_max,
                200.0,
                None,
            ),
            Vec3::new(477.0, 12.5, 1_714.0),
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
    fn script_camera_shakers_rotate_not_translate() {
        let mut shaker = ScriptCameraShaker::new(Vec3::ZERO, 500., 2., 15.);
        shaker.elapsed_seconds = 0.25;
        shaker.phi = Vec3::splat(std::f32::consts::FRAC_PI_2);
        shaker.omega = Vec3::splat(SHAKE_MIN_OMEGA);
        let rotation = script_camera_shaker_rotations(&shaker, Vec3::new(0., 50., 0.));
        assert!(rotation.is_finite() && rotation.length_squared() > 0.);
    }

    #[test]
    fn script_camera_shaker_steps_one_thirtieth_per_actual_build() {
        let (mut intensity, mut cosine, mut sine) = (4., 1., 0.);
        assert_eq!(
            super::super::camera_view_effects::step_view_impulse(
                &mut intensity,
                &mut cosine,
                &mut sine,
                false
            ),
            None
        );
        assert_eq!(intensity, 4.);
        assert_eq!(
            super::super::camera_view_effects::step_view_impulse(
                &mut intensity,
                &mut cosine,
                &mut sine,
                true
            ),
            Some(Vec3::new(4., 0., 0.))
        );
        assert_eq!(intensity, 3.);
    }

    #[test]
    fn script_camera_pitch_writes_fx_pitch_not_orbit() {
        use crate::cnc_game_engine::camera_motion::{
            AxisKind, AxisMotion, MotionAdmission, step_axis,
        };

        let mut camera = crate::presentation_frame::camera::SessionCamera::default();
        camera.camera_pitch_radians = 0.73;
        camera.camera_fx_pitch = 1.25;
        camera.camera_pitch_frame = 17;
        camera.install_pitch(2., 0., 0., 0.);
        assert_eq!(camera.camera_pitch_radians, 0.73);
        assert_eq!(camera.camera_fx_pitch, 1.25);
        assert_eq!(camera.camera_pitch_start, 1.25);
        assert_eq!(camera.camera_pitch_target, Some(2.));
        assert_eq!(camera.camera_pitch_frame, 0);
        assert_eq!(camera.camera_pitch_duration, 0.001);

        let (mut fx_pitch, mut target, mut elapsed) =
            (1., Some(CnCGameEngine::script_pitch_is_fx_pitch(2.)), 0);
        let rejected = step_axis(
            AxisMotion {
                value: &mut fx_pitch,
                target: &mut target,
                frame: &mut elapsed,
                start: 1.,
                duration_seconds: 0.,
                ease_in_seconds: 0.,
                ease_out_seconds: 0.,
            },
            AxisKind::ZoomPitch,
            MotionAdmission {
                update_views: false,
                fast_camera_only: false,
                paused: false,
                debug_frozen: false,
                disable_camera_movement: false,
            },
        );
        assert!(rejected.was_active && !rejected.changed);
        assert_eq!((fx_pitch, target, elapsed), (1., Some(2.), 0));
        let admitted = step_axis(
            AxisMotion {
                value: &mut fx_pitch,
                target: &mut target,
                frame: &mut elapsed,
                start: 1.,
                duration_seconds: 0.,
                ease_in_seconds: 0.,
                ease_out_seconds: 0.,
            },
            AxisKind::ZoomPitch,
            MotionAdmission {
                update_views: true,
                fast_camera_only: false,
                paused: false,
                debug_frozen: false,
                disable_camera_movement: false,
            },
        );
        assert!(admitted.was_active && admitted.completed && admitted.changed);
        assert_eq!((fx_pitch, target, elapsed), (2., None, 1));
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
    fn scripted_zoom_pitch_yaw_pause_with_game() {
        use crate::cnc_game_engine::camera_motion::{
            AxisKind, AxisMotion, MotionAdmission, step_axis,
        };

        for paused in [true, false] {
            let (mut value, mut target, mut elapsed) = (0., Some(1.), 0);
            let step = step_axis(
                AxisMotion {
                    value: &mut value,
                    target: &mut target,
                    frame: &mut elapsed,
                    start: 0.,
                    duration_seconds: 1.,
                    ease_in_seconds: 0.,
                    ease_out_seconds: 0.,
                },
                AxisKind::ZoomPitch,
                MotionAdmission {
                    update_views: true,
                    fast_camera_only: false,
                    paused,
                    debug_frozen: false,
                    disable_camera_movement: false,
                },
            );
            assert!(step.was_active);
            assert_eq!(step.changed, !paused);
            assert_eq!(elapsed, if paused { 0 } else { 1 });
            assert_eq!(value, if paused { 0. } else { 1. / 30. });
        }
    }

    #[test]
    fn synthesized_skirmish_slots_follow_map_player_count_sides_and_difficulty() {
        // One slot per authored player start: slot 0 is the human on the
        // requested faction, the rest are Medium AIs on the map sides with
        // one free-for-all alliance team and one map start each.
        let slots =
            host_synthesize_skirmish_slots(&[0, 1, 2, 3], &["USA", "GLA", "China"], "China");
        assert_eq!(slots.len(), 4);
        assert_eq!(slots.iter().filter(|s| s.is_active).count(), 4);
        let (human, ais) = slots.split_first().expect("slots");
        assert!(human.is_human);
        assert_eq!(human.faction, "China");
        assert_eq!(human.team, 0);
        assert_eq!(human.start_position, 0);
        assert_eq!(human.player_name, "Player");
        assert!(human.ai_difficulty.is_none());
        for (offset, ai) in ais.iter().enumerate() {
            let slot = offset + 1;
            assert!(!ai.is_human, "slot {slot}");
            assert_eq!(ai.ai_difficulty.as_deref(), Some("Medium"), "slot {slot}");
            assert_eq!(ai.team, slot as i32, "slot {slot}");
            assert_eq!(ai.start_position, slot as i32, "slot {slot}");
        }
        assert_eq!(ais[0].faction, "USA");
        assert_eq!(ais[1].faction, "GLA");
        assert_eq!(ais[2].faction, "China");

        // Map side faction names accept both SidesList spellings and skip
        // civilians/observers entirely.
        assert_eq!(
            host_base_faction_from_map_side("FactionAmerica"),
            Some("USA")
        );
        assert_eq!(host_base_faction_from_map_side("PlyrGLA"), Some("GLA"));
        assert_eq!(
            host_base_faction_from_map_side("FactionChina"),
            Some("China")
        );
        assert_eq!(host_base_faction_from_map_side("FactionCivilian"), None);
        assert_eq!(host_base_faction_from_map_side("FactionObserver"), None);
        assert_eq!(host_base_faction_from_map_side(""), None);
    }

    #[test]
    fn synthesized_skirmish_slots_cycle_sides_and_floor_at_two() {
        // Five starts against one map side: the remaining AI slots cycle the
        // base sides rather than repeating the single map side.
        let slots = host_synthesize_skirmish_slots(&[0, 1, 2, 3, 4], &["China"], "USA");
        assert_eq!(slots.len(), 5);
        let factions: Vec<&str> = slots[1..].iter().map(|s| s.faction.as_str()).collect();
        assert_eq!(factions, vec!["China", "USA", "GLA", "China"]);

        // Unknown map shape keeps the golden two-slot floor (human plus one
        // AI) instead of a zero-opponent match.
        let degenerate = host_synthesize_skirmish_slots(&[], &[], "Random");
        assert_eq!(degenerate.len(), HOST_SYNTHESIZED_SKIRMISH_MIN_SLOTS);
        assert_eq!(degenerate[0].faction, "Random");
        assert!(
            matches!(
                degenerate[0].player_template,
                crate::skirmish_config::SkirmishPlayerTemplateSelection::Random
            ),
            "a Random human keeps PLAYERTEMPLATE_RANDOM resolution semantics"
        );
        assert_eq!(degenerate[1].ai_difficulty.as_deref(), Some("Medium"));
        assert_eq!(degenerate[1].faction, "USA");

        // A 10-start map is capped at the C++ MAX_SLOTS slot set.
        let wide = host_synthesize_skirmish_slots(
            &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
            &[
                "USA", "GLA", "China", "USA", "GLA", "China", "USA", "GLA", "China",
            ],
            "USA",
        );
        assert_eq!(wide.len(), crate::ui::skirmish_menu::MAX_SLOTS);
    }

    #[test]
    fn synthesized_skirmish_config_registers_ai_opponents_and_sets_cash() {
        // The contract a plain runtime-host `start_game` must observe after
        // this synthesis: N players registered, N-1 of them real registered
        // AI opponents carrying their difficulty, and the default starting
        // cash applied by the production authority path.
        let slots = host_synthesize_skirmish_slots(&[0, 1, 2], &["GLA", "China"], "USA");
        let config = crate::skirmish_config::SkirmishMatchConfig {
            source_game_info: None,
            map: "SynthMap".to_string(),
            random_seed: 0,
            old_factions_only: false,
            rules: crate::skirmish_config::GameRulesSnapshot::default_rules(),
            slots,
        };
        assert_eq!(config.rules.starting_cash, 10_000);

        let mut logic = crate::game_logic::GameLogic::new();
        crate::skirmish_config::apply_skirmish_config(&mut logic, &config)
            .expect("synthesized config applies through the skirmish authority");

        let active = config.slots.iter().filter(|s| s.is_active).count();
        assert!(
            logic.get_players().len() >= active,
            "every synthesized slot must register a player (got {})",
            logic.get_players().len()
        );
        assert_eq!(
            logic.host_ai_player_count(),
            active - 1,
            "every computer slot must become a registered AI opponent"
        );
        let human = logic.get_player(0).expect("human registered");
        assert_eq!(human.resources.supplies, 10_000);
        for slot in config.slots.iter().filter(|s| !s.is_human) {
            let player = logic
                .get_player(slot.slot_index as u32)
                .expect("ai player registered");
            assert!(
                player.resources.supplies >= 10_000,
                "slot {} cash must come from the synthesized rules",
                slot.slot_index
            );
            assert_eq!(
                logic.host_ai_difficulty(slot.slot_index as u32),
                Some(crate::ai::AIDifficulty::Medium)
            );
        }
    }
}

#[cfg(test)]
#[path = "camera_constraint_tests.rs"]
mod camera_constraint_tests;
