use super::*;
use crate::fow_rendering::ProjectedShroudSnapshot;

const fn default_presentation_alliance_team() -> i32 {
    -1
}

/// Snapshot-owned player roster residual (defeat/alliance UI / radar team).
/// Fail-closed: not full Player science/upgrade/diplomacy matrix.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresentationPlayerInfo {
    pub id: u32,
    pub name: String,
    pub team: Team,
    /// Skirmish alliance slot. This is distinct from faction `team` and lets
    /// a frozen frame distinguish same-faction opponents from allied players.
    #[serde(default = "default_presentation_alliance_team")]
    pub alliance_team: i32,
    pub is_alive: bool,
    pub is_local: bool,
    /// True when host AI manager owns this player (skirmish AI residual).
    pub is_ai: bool,
    /// Skirmish/UI color residual (RGB).
    pub color_rgb: (u8, u8, u8),
}

/// Frozen script popup residual (C++ ScriptPopupMessageRequest parity).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresentationPopupMessage {
    pub message: String,
    pub x_percent: i32,
    pub y_percent: i32,
    pub width: i32,
    pub pause: bool,
    pub pause_music: bool,
}

/// Frozen InGameUI PublicTimer superweapon countdown residual.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresentationSuperweaponTimer {
    pub name: String,
    pub template_name: String,
    pub icon: String,
    /// Full recharge duration seconds residual.
    pub recharge_time: f32,
    /// Seconds remaining (0 = ready).
    pub remaining: f32,
    /// Science/prereq unlocked residual.
    pub unlocked: bool,
    /// Ready residual (unlocked && remaining <= 0).
    pub ready: bool,
    /// `SpecialPowerType` Debug name for shadow cooldown overlay.
    pub power_key: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresentationFrame {
    pub frame: LogicFrame,
    /// Host sim clock residual (seconds) for UI time readout.
    pub total_play_time_seconds: f32,
    /// Host AI difficulty residual (save metadata).
    pub ai_difficulty: crate::ai::AIDifficulty,
    /// Host game mode residual (restart/save metadata).
    pub game_mode: crate::game_logic::GameMode,
    pub objects: Vec<RenderableObject>,
    /// Transient direct Object→Drawable source roster.  This survives a
    /// GameWorld-primary replacement of `objects` so deferred death/rubble
    /// visuals retain their C++ Object lifetime.  It is deliberately not part
    /// of serialized presentation/save state; GameClient owns binding state.
    #[serde(skip)]
    pub direct_host_drawables: Vec<PresentationDirectHostDrawable>,
    pub local_player_id: u32,
    /// Local player team frozen at snapshot time (selection/hotkey residual).
    /// Prefer this over live `GameLogic::get_player` dual-reads when a frame is installed.
    pub local_team: Team,
    /// Host team base / command-center pose residual for camera snap proximity.
    pub local_team_base_position: Option<Vec3>,
    /// Full player roster frozen at snapshot time (defeat/alliance UI residual).
    pub players: Vec<PresentationPlayerInfo>,
    pub local_supplies: u32,
    pub local_power: i32,
    /// Host Player::power_produced residual (energy bar numerator side).
    pub local_power_produced: i32,
    /// Host Player::power_consumed residual (energy bar demand side).
    pub local_power_consumed: i32,
    pub local_color_rgb: (u8, u8, u8),
    /// Local player still alive residual.
    pub local_is_alive: bool,
    /// Radar provider count residual (CommandCenter / RadarVan).
    pub local_radar_count: i32,
    /// Script/power radar disable residual.
    pub local_radar_disabled: bool,
    /// GLA cash bounty percent residual (0..1).
    pub local_cash_bounty_percent: f32,
    /// C++ Player::m_rankLevel residual (1-based).
    pub local_rank_level: u32,
    /// C++ Player::m_skillPoints residual (GeneralsExperience).
    pub local_skill_points: i32,
    /// C++ Player::m_sciencePurchasePoints residual.
    pub local_science_purchase_points: i32,
    /// ControlBar rank progress residual 0..100
    /// (`(skill - levelDown) * 100 / (levelUp - levelDown)`).
    pub local_rank_progress_percent: i32,
    /// Unlocked science names residual (capped).
    pub local_unlocked_sciences: Vec<String>,
    /// InGameUI PublicTimer superweapon countdown residual (local player).
    /// Fail-closed: not full font flash / multi-CC SW map / script hide.
    pub superweapon_timers: Vec<PresentationSuperweaponTimer>,
    /// Selected producer CanMake residual cameos (ControlBar HelpBox feed).
    pub can_make_cameos: Vec<PresentationCanMakeCameo>,
    /// Selected producer object id residual for can_make_cameos.
    pub can_make_producer_id: Option<u32>,

    /// Queued upgrade template names residual (capped).
    pub local_queued_upgrades: Vec<String>,
    pub selected: Vec<ObjectId>,
    pub events: Vec<PresentationEvent>,
    pub match_over: bool,
    pub victory_label: Option<String>,
    /// Players defeated this evaluate residual (C++ defeat notification queue).
    pub defeated_player_ids: Vec<u32>,
    /// Alliance state-change residual from victory evaluate.
    pub alliance_events: Vec<crate::game_logic::AllianceNotification>,
    /// Host VictorySummary residual (mission/duration/player results).
    /// Fail-closed: stats tables frozen at evaluate; not live re-aggregate.
    /// Skipped in serde (Duration/player payload is host snapshot residual only).
    #[serde(skip)]
    pub victory_summary: Option<crate::game_logic::VictorySummary>,
    /// Beacon world positions residual (host_beacons preferred; manager snapshot fallback).
    pub beacons: Vec<Vec3>,
    /// Beacons placed this frame (HUD bloom residual).
    pub new_beacons: Vec<Vec3>,
    /// Active script broadcast texts residual.
    pub script_messages: Vec<String>,
    /// New script messages this frame residual.
    pub new_script_messages: Vec<String>,
    /// Cinematic letterbox residual.
    pub cinematic_letterbox: bool,
    /// Cinematic overlay text residual.
    pub cinematic_text: Option<String>,
    /// Remaining lifetime for cinematic text (ms residual).
    pub cinematic_text_remaining_ms: Option<i32>,
    /// Military caption residual.
    pub military_caption: Option<String>,
    /// Remaining lifetime for military caption (ms residual).
    pub military_caption_remaining_ms: Option<i32>,
    /// Effective radar available residual (forced || enabled && has_radar).
    pub radar_ui_enabled: bool,
    /// Script radar forced residual.
    pub radar_forced: bool,
    /// Mission objectives residual (ObjectiveDisplay clone).
    pub objectives: Vec<crate::ui::objectives::ObjectiveDisplay>,
    /// Pending script movie name residual.
    pub pending_movie: Option<String>,
    /// Pending radar movie name residual.
    pub pending_radar_movie: Option<String>,
    /// Pending music-stop request residual.
    pub pending_music_stop: bool,
    /// Pending popup message texts residual (fail-closed layout).
    pub pending_popup_messages: Vec<PresentationPopupMessage>,
    /// Script time-freeze residual.
    pub script_time_frozen: bool,
    /// Script camera time-freeze residual.
    pub script_camera_time_frozen: bool,
    /// Combined simulation freeze residual.
    pub time_frozen_for_simulation: bool,
    /// Wave 251: host visual speed residual (render/update timing).
    pub visual_speed_multiplier: f32,
    /// Wave 252: script default camera max height residual.
    pub script_default_camera_max_height: f32,
    /// Wave 252: script default camera pitch residual.
    pub script_default_camera_pitch: f32,
    /// Pending script FPS limit residual.
    pub script_fps_limit: Option<i32>,
    /// Pending view guardband residual (x,y bias).
    pub view_guardband: Option<(f32, f32)>,
    /// Pending camera focus residual.
    pub camera_focus: Option<[f32; 3]>,
    /// Camera-follow object world position residual (live follow still resolves host id).
    pub camera_follow_position: Option<[f32; 3]>,
    /// Pending BW mode residual (enabled, frames).
    pub camera_bw_mode: Option<(bool, i32)>,
    /// Pending camera shaker residual (epicenter xyz, amplitude, duration, radius).
    pub camera_shakers: Vec<([f32; 3], f32, f32, f32)>,
    /// Pending camera motion-blur request count residual.
    pub camera_motion_blur_count: usize,
    /// Pending camera zoom residual (zoom, duration).
    pub camera_zoom: Option<(f32, f32)>,
    pub camera_zoom_reset: bool,
    /// RESET_CAMERA duration in seconds. 0 = snap (legacy).
    #[serde(default)]
    pub camera_zoom_reset_duration: f32,
    /// RESET_CAMERA ease-in/out seconds.
    #[serde(default)]
    pub camera_zoom_reset_ease: (f32, f32),
    /// CAMERA_ZOOM ease-in/out seconds.
    #[serde(default)]
    pub camera_zoom_ease: (f32, f32),
    /// Pending camera pitch residual (pitch, duration).
    pub camera_pitch: Option<(f32, f32)>,
    /// CAMERA_PITCH ease-in/out seconds.
    #[serde(default)]
    pub camera_pitch_ease: (f32, f32),
    /// Pending camera rotate residual (rotations, duration).
    pub camera_rotate: Option<(f32, f32)>,
    /// ROTATE_CAMERA ease-in/out seconds.
    #[serde(default)]
    pub camera_rotate_ease: (f32, f32),
    /// Pending look-toward residual.
    pub camera_look_toward: Option<[f32; 3]>,
    /// Seconds remaining for the pending look-toward rotate (0 = snap).
    #[serde(default)]
    pub camera_look_toward_duration: f32,
    /// LOOK_TOWARD ease-in/out seconds.
    #[serde(default)]
    pub camera_look_toward_ease: (f32, f32),
    /// LOCK_TETHER play radius. None means LOCK_FOLLOW.
    #[serde(default)]
    pub camera_tether_play: Option<f32>,
    /// Pending slave-mode enable residual (template, bone).
    pub camera_slave_enable: Option<(String, String)>,
    pub camera_slave_disable: bool,
    /// Active script named timers residual (name, text, countdown).
    pub named_timers: Vec<(String, String, bool)>,
    /// Cameo flash residual (button, count).
    pub cameo_flash: Vec<(String, i32)>,
    /// Pending screen-shake intensities residual.
    pub screen_shakes: Vec<i32>,
    /// Script skybox enable residual.
    pub script_skybox_enabled: bool,
    /// Superweapon display enable residual.
    pub superweapon_display_enabled: bool,
    /// Named-timer display shown residual.
    pub named_timer_display_shown: bool,
    /// Hidden superweapon object ids residual.
    pub superweapon_hidden_objects: Vec<u32>,
    /// Shell-map FOW bypass (`GameLogic::isInShellGame`) frozen at snapshot time.
    /// When true, unit FOW is forced fully visible and never-explored skip is off.
    /// EVA residual counters frozen at snapshot (C++ Eva message queue deltas).
    pub eva_low_power_count: u32,
    pub eva_insufficient_funds_count: u32,
    pub eva_base_under_attack_count: u32,
    pub eva_ally_under_attack_count: u32,
    pub fow_shell_bypass: bool,
    /// Wave 557: host replay-mode residual (`GameLogic::isInReplayGame`) frozen at
    /// snapshot time for FPS-limit / TiVO residual without live dual-read.
    pub in_replay_game: bool,
    /// Wave 561: host fixed-step catch-up residual (`steps_run`) frozen at snapshot
    /// for runtime status without live dual-read mid-frame.
    pub logic_steps_run: u32,
    /// Wave 564: host fixed-step budget residual frozen at snapshot.
    pub logic_steps_budget_hit: bool,
    /// Wave 564: host fixed-step accumulator residual (seconds) frozen at snapshot.
    pub logic_steps_accumulated_seconds: f32,
    /// Wave 563: host ThingTemplate name keys frozen for train/UI residual
    /// contains checks without dual-reading live `GameLogic::templates` mid-frame.
    /// Sorted; capped. Fail-closed: not full template body freeze / playable_claim.
    pub known_template_names: Vec<String>,
    /// Compact local-player cell-grid FOW for terrain overlay / minimap texture.
    /// Frozen at build so GPU upload does not re-query shroud mid-render.
    /// Fail-closed: not full SAGE dirty-rect / multi-layer shroud streaming.
    pub fow_grid: PresentationFowGrid,
    /// Source-shaped terrain shroud projection texture, frozen from `fow_grid`
    /// with its own border, origin, R8 levels, and tint metadata.  Renderer
    /// passes consume this owned value; they must not query live GameLogic/FOW.
    #[serde(default)]
    pub projected_shroud: ProjectedShroudSnapshot,
    /// Active combat particle systems from host registry (observe path for client).
    pub particle_systems: Vec<PresentationParticleSystem>,
    /// Active Patriot assist / BinaryDataStream lasers + Line3D segments.
    /// Frozen so WGPU laser segment pack does not re-read live host mid-render.
    /// Fail-closed: not full SegLineRenderer GPU texture draw.
    pub laser_beams: Vec<PresentationLaserBeam>,
    /// W3DLaserDraw / Tracer / Rope scene lines frozen from the client RenderBridge.
    /// Fail-closed: packed through the existing LaserSegmentUpload line path.
    #[serde(default)]
    pub scene_lines: Vec<PresentationSceneLine>,
    /// C++ `W3DStatusCircle` fullscreen fade overlay.
    #[serde(default)]
    pub camera_fade: PresentationCameraFade,
    /// C++ ProjectileStreamUpdate residual trails.
    pub projectile_streams: Vec<PresentationProjectileStream>,
    /// In-flight combat projectiles frozen from host CombatSystem.
    /// Fail-closed: not full W3D projectile draw / trail mesh.
    pub projectiles: Vec<PresentationProjectile>,
    /// InGameUI floating cash / caption texts frozen from host residual registries.
    /// Fail-closed: not full DisplayString GPU / Unicode GameText draw.
    pub floating_texts: Vec<PresentationFloatingText>,
    /// InGameUI world animations (MoneyPickUp Anim2D residual) frozen from host.
    /// Fail-closed: not full Anim2DCollection GPU draw.
    pub world_anims: Vec<PresentationWorldAnim>,
    /// Dual-tick residual counters (build / apply / content counts).
    pub dual_tick: PresentationDualTickResidual,
    /// World/environment identity for lighting/shell/bounds/heightmap residual.
    /// Prefer this over live `GameLogic` during GPU collect/execute.
    pub world_env: PresentationWorldEnv,
    /// Objects stamped by the last `overlay_gameworld_shadow` call (0 if none).
    /// Architecture residual: GameWorld last-writer presentation identity count.
    #[serde(default)]
    pub gameworld_overlay_stamped: usize,
    /// Count of RenderableObjects created from GameWorld entities missing on host frame
    /// (Wave 192 append_missing_from_gameworld). Fail-closed: not full build_from_gameworld cutover.
    #[serde(default)]
    pub gameworld_appended: usize,
    /// Count of objects after `rebuild_objects_from_gameworld` (Wave 193).
    /// Fail-closed: opt-in path; not full host cutover / playable_claim.
    #[serde(default)]
    pub gameworld_rebuilt: usize,
    /// True when objects were rebuilt from GameWorld (Wave 196 engine primary path).
    #[serde(default)]
    pub gameworld_primary_objects: bool,
}

/// Whether presentation object rosters should be rebuilt from GameWorld (Wave 194).
///
/// **Default ON** when shadow is present. Opt out with
/// `GENERALS_PRESENTATION_FROM_GAMEWORLD=0` (or false/no/off).
/// Fail-closed: does not flip shell `playable_claim`; host still supplies
/// non-object residual (scripts/FX/camera) via `build_from_logic` when used.
pub fn presentation_from_gameworld_enabled() -> bool {
    match std::env::var("GENERALS_PRESENTATION_FROM_GAMEWORLD") {
        Ok(v) => {
            let t = v.trim();
            !matches!(t, "0" | "false" | "FALSE" | "no" | "NO" | "off" | "OFF")
        }
        Err(_) => true,
    }
}
