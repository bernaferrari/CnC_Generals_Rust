//! Production host smoke: SkirmishMenu → config → apply → map load → frames → presentation.
//!
//! Full windowed shell/WND + GPU boot still requires a display; this path exercises the
//! same production APIs `start_game_from_ui` uses after menu StartGame.
//!
//! Honesty: no tautological host flag, no silent golden_skirmish_config fallback.
//! Opponent slot is configured through SkirmishMenu::configure_slot_medium_ai.
//!
//! Claim flags (do not conflate):
//! - `playable_claim` — **always false**. Headless host APIs are not retail W3D /
//!   windowed shell playthrough. Fail-closed pending full GPU/WND match play.
//! - `shell_host_playable_ok` — limited honesty claim: when true, the headless
//!   shell→config→map→dual-tick presentation→HUD selection/minimap→ControlBar.wnd
//!   ensure path is operational. Still **not** a retail playthrough claim.
//!
//! Residual honesty (do **not** flip `playable_claim`):
//! - `dual_tick_presentation_ok` — seed + logic update + multi-consumer presentation apply
//! - `dual_tick_counters_ok` — presentation dual-tick residual counters (build/apply)
//! - `minimap_fow_presentation_ok` — FOW grid snapshot usable for minimap texture path
//! - `laser_segment_upload_ok` — presentation → CPU SegLine pack residual (incl. synthetic)
//! - `projectile_segment_upload_ok` — presentation projectiles → CPU trail pack residual
//! - `multi_beam_soft_edge_ok` — OrbitalLaser NumBeams soft-edge CPU pack residual
//! - `laser_presentation_residual_ok` — ground-height + soft-edge presentation fields
//! - `floating_text_layout_ok` — presentation → CPU InGameUI floating-text layout residual
//! - `floating_text_vanish_ok` — vanish-rate alpha residual presentation field honesty
//! - `world_anim_presentation_ok` — MoneyPickUp Anim2D residual frozen on presentation
//! - `world_anim_layout_ok` — presentation → CPU Anim2D layout pack residual
//! - `world_anim_fade_ok` — world-anim fade residual presentation field honesty
//! - `anim2d_frame_ok` — MoneyPickUp Anim2D frame advance residual
//! - `anim2d_collection_residual_ok` — Anim2DCollection template/instance residual
//! - `translate_copy_residual_ok` — GameText translate_copy escape table residual
//! - `game_text_caption_ok` — GUI:AddCash caption residual on floating-text pack
//! - `game_text_csf_str_ok` — CSF/STR parse + retail `$%d` printf + DisplayString measure
//! - `display_string_measure_ok` — monospaced glyph measure residual on floating-text pack
//! - `rng_stream_residual_ok` — GameLogic/GameClient RandomValue ADC stream residual
//! - `mesh_asset_residual_ok` — W3D mesh resolve residual (keys/scale/search; no GPU)
//! - `rng_residual_pack_ok` — Wave 72 host RNG residual pack honesty
//! - `special_power_wave72_residual_ok` — Daisy/A10 special-power residual pack
//! - `special_power_wave73_residual_ok` — Spectre/Nuke/SupW residual pack
//! - `special_power_wave76_residual_ok` — A10 science-tier FormationSize residual pack
//! - `paradrop_wave76_residual_ok` — Paradrop science-tier payload residual pack
//! - `control_bar_wave76_residual_ok` — ControlBar window-count/named/font residual pack
//! - `graphics_wave76_residual_ok` — InGameUI font table + vanish color-alpha residual
//! - `spectre_orbit_decal_presentation_ok` — Wave 73 presentation Spectre decal residual
//! - `special_power_wave77_residual_ok` — Wave 77 audio name tables residual pack
//! - `special_power_wave78_residual_ok` — Wave 78 reload table / CarpetBomb / Artillery residual pack
//! - `cluster_mines_wave78_residual_ok` — Wave 78 ClusterMines DeliveryDecal / science residual pack
//! - `gps_scrambler_wave78_residual_ok` — Wave 78 GPS science / marker particle residual pack
//! - `cash_bounty_wave78_residual_ok` — Wave 78 CashBountyScienceTier residual pack
//! - `fow_residual_pack_ok` — Wave 77 FOW cell/R8/inactive residual honesty
//! - `ground_height_presentation_ok` — Wave 77 unit ground-height presentation residual
//! - `weapon_store_seed_residual_ok` — Wave 77 host WeaponStore seed residual pack
//! - `ai_skirmish_residual_ok` — Wave 77 AI skirmish timer/wealth residual pack
//! - `minimap_residual_pack_ok` — Wave 79 minimap FOW shade/size residual pack
//! - `selection_hud_residual_pack_ok` — Wave 79 selection/HUD color residual pack
//! - `input_residual_pack_ok` — Wave 79 drag/double-click input residual pack
//! - `drawable_residual_fields_ok` — Wave 79 Drawable StealthLook save/load residual
//! - `unit_training_wave79_residual_ok` — Wave 79 veterancy bonus / AdvancedTraining XP
//! - `upgrades_cost_time_application_ok` — Wave 79 upgrade cost/time application residual
//! - `command_button_wave80_residual_ok` — Wave 80 superweapon CommandButton label/cursor residual
//! - `science_rank_wave80_residual_ok` — Wave 80 Rank.ini SCIENCE rank residual table
//! - `superweapon_kindof_wave80_residual_ok` — Wave 80 superweapon building KindOf residual
//! - `special_power_enum_wave80_residual_ok` — Wave 80 SpecialPower enum discriminant residual
//! - `terrain_height_sample_wave81_ok` — Wave 81 map height sample residual pack
//! - `pathfinder_wave81_residual_ok` — Wave 81 Pathfinder body/locomotor residual deepen
//! - `locomotor_table_wave81_ok` — Wave 81 common-unit locomotor residual table
//! - `armor_table_wave81_ok` — Wave 81 ProjectileArmor/HazardousMaterial residual table
//! - `puc_flare_table_wave81_ok` — Wave 81 PUC outer-node flare name table residual
//! - `damage_type_wave82_ok` — Wave 82 DamageType residual enum table
//! - `death_type_wave82_ok` — Wave 82 DeathType residual enum table
//! - `model_condition_wave82_ok` — Wave 82 ModelCondition residual flags (CONTINUOUS_FIRE_*)
//! - `weapon_bonus_wave82_ok` — Wave 82 WeaponBonus residual type table
//! - `object_status_wave82_ok` — Wave 82 ObjectStatus / StatusBits residual table
//! - `prod_queue83_ok` — Wave 83 production queue residual (MaxQueue/energy/refund)
//! - `supply_wh83_ok` — Wave 83 supply warehouse residual (boxes/value/cripple heal)
//! - `dozer_build83_ok` — Wave 83 dozer build residual (DozerAI/build pads)
//! - `capture83_ok` — Wave 83 capture building residual (Ranger infantry capture)
//! - `power_plant83_ok` — Wave 83 power plant residual energy pack
//! - `cmd_center83_ok` — Wave 83 command center residual peels
//! - `kindof_wave84_ok` — Wave 84 KindOf residual bit-name table (KINDOF_COUNT 116)
//! - `weapon_slot_wave84_ok` — Wave 84 WeaponSlot PRIMARY/SECONDARY/TERTIARY table
//! - `veterancy_wave84_ok` — Wave 84 Veterancy residual level table
//! - `relationship_wave84_ok` — Wave 84 Relationship ENEMIES/NEUTRAL/ALLIES table
//! - `geometry_wave84_ok` — Wave 84 Geometry SPHERE/CYLINDER/BOX table
//! - `shadow_wave84_ok` — Wave 84 Shadow residual type bit-name table
//! - `faction85_ok` — Wave 85 faction side residual table (America/China/GLA + generals)
//! - `ptpl85_ok` — Wave 85 player template residual peels
//! - `cash85_ok` — Wave 85 starting cash residual (+ difficulty health bonus)
//! - `aiperson85_ok` — Wave 85 skirmish AI personality / SideInfo residual
//! - `victory85_ok` — Wave 85 victory condition residual peels
//! - `cam86_ok` — Wave 86 GameData camera/FPS residual pack
//! - `world86_ok` — Wave 86 GameData world constants residual pack
//! - `mpopt86_ok` — Wave 86 multiplayer options residual pack (host-only)
//! - `mapsel86_ok` — Wave 86 map selection residual pack
//! - `crate86_ok` — Wave 86 crate residual deepen pack
//! - `weather87_ok` — Wave 87 weather (snow) residual pack
//! - `water87_ok` — Wave 87 water / TimeOfDay residual pack
//! - `bridge87_ok` — Wave 87 bridge tower / scaffold residual pack
//! - `tunnel87_ok` — Wave 87 tunnel residual deepen pack
//! - `garrison87_ok` — Wave 87 garrison residual pack
//! - `transport87_ok` — Wave 87 transport residual pack
//! - `radius88_ok` — Wave 88 RadiusCursor residual name table
//! - `mouse88_ok` — Wave 88 MouseCursor residual name table
//! - `fxlist88_ok` — Wave 88 superweapon FXList residual name table
//! - `ocl88_ok` — Wave 88 superweapon OCL residual name table
//! - `particle88_ok` — Wave 88 superweapon particle residual name table expand
//! - `audio88_ok` — Wave 88 superweapon audio event residual name table expand
//! - `rank_skill89_ok` — Wave 89 rank skill-points application residual deepen
//! - `exp89_ok` — Wave 89 experience residual tables pack
//! - `hotkey89_ok` — Wave 89 hotkey CommandMap residual table
//! - `chat89_ok` — Wave 89 chat residual host peels
//! - `replay89_ok` — Wave 89 local replay residual host peels
//! - `options89_ok` — Wave 89 options residual peels
//! - `gamespeed90_ok` — Wave 90 GameSpeed residual pack
//! - `framerate90_ok` — Wave 90 frame rate residual deepen pack
//! - `debug90_ok` — Wave 90 debug residual tables pack (host-only)
//! - `lang90_ok` — Wave 90 language residual deepen pack
//! - `credits90_ok` — Wave 90 credits residual pack
//! - `particle93_ok` — Wave 93 particle emit-rate residual deepen pack
//! - `drawable93_ok` — Wave 93 drawable opacity/shroud residual deepen pack
//! - `shadow93_ok` — Wave 93 shadow residual deepen pack
//! - `terrain_tex93_ok` — Wave 93 terrain texture residual pack
//! - `road93_ok` — Wave 93 road residual pack
//! - `ai_state94_ok` — Wave 94 AI state residual table
//! - `special_ability94_ok` — Wave 94 special ability residual deepen
//! - `upgrade_names94_ok` — Wave 94 upgrade full name table
//! - `command_set94_ok` — Wave 94 CommandSet superweapon residual
//! - `script_action95_ok` — Wave 95 script action name table residual
//! - `script_cond95_ok` — Wave 95 script condition name table residual
//! - `map_object95_ok` — Wave 95 map object residual pack
//! - `waypoint95_ok` — Wave 95 waypoint residual pack
//! - `team95_ok` — Wave 95 team residual pack
//! - `player95_ok` — Wave 95 player residual deepen pack
//! - `partition96_ok` — Wave 96 partition residual pack
//! - `collision96_ok` — Wave 96 collision / GeometryInfo residual pack
//! - `physics96_ok` — Wave 96 physics residual pack
//! - `projectile96_ok` — Wave 96 projectile residual deepen pack
//! - `radar97_ok` — Wave 97 radar residual deepen pack
//! - `spotter97_ok` — Wave 97 spotter residual pack
//! - `stealth97_ok` — Wave 97 stealth residual deepen pack
//! - `detector97_ok` — Wave 97 detector residual deepen pack
//! - `vision97_ok` — Wave 97 vision residual pack
//! - `dock98_ok` — Wave 98 dock residual pack
//! - `contain98_ok` — Wave 98 contain residual deepen pack
//! - `exit98_ok` — Wave 98 exit residual pack
//! - `heal98_ok` — Wave 98 heal residual deepen pack
//! - `production99_ok` — Wave 99 production residual deepen pack
//! - `buildable99_ok` — Wave 99 buildable residual pack
//! - `prereq99_ok` — Wave 99 prerequisite residual pack
//! - `cmdbtn99_ok` — Wave 99 command button residual deepen pack
//! - `controlbar99_ok` — Wave 99 control bar residual deepen pack
//! - `thing_factory100_ok` — Wave 100 ThingFactory residual deepen pack
//! - `module_type100_ok` — Wave 100 Module type table residual pack
//! - `xfer100_ok` — Wave 100 Xfer residual deepen pack
//! - `tf_crosslink100_ok` — Wave 100 ThingFactory spawn cross-link residual pack
//! - `module_factory101_ok` — Wave 101 ModuleFactory residual deepen pack
//! - `thing_factory101_ok` — Wave 101 ThingFactory create residual deepen pack
//! - `partition_register101_ok` — Wave 101 PartitionManager register residual pack
//! - `mf_crosslink101_ok` — Wave 101 ThingFactory/Module/Partition cross-link pack
//! - `display102_ok` — Wave 102 DisplayString FontChars/StretchRect residual pack
//! - `anim2d102_ok` — Wave 102 Anim2D full template table / Collection init pack
//! - `laser102_ok` — Wave 102 laser SegLine UV atlas residual pack
//! - `csf102_ok` — Wave 102 multi-locale CSF residual pack (expanded locales)
//! - `pres102_ok` — Wave 102 presentation dual-tick residual deepen pack
//! - `weapon103_ok` — Wave 103 weapon residual deepen pack
//! - `armor103_ok` — Wave 103 armor residual expand pack
//! - `loco103_ok` — Wave 103 locomotor residual expand pack
//! - `sp103_ok` — Wave 103 special-power superweapon residual deepen pack
//! - `kindof103_ok` — Wave 103 object KindOf residual pack
//! - `object_status104_ok` — Wave 104 Object status-mask residual state machine pack
//! - `object_create104_ok` — Wave 104 Object create residual order pack
//! - `active_body104_ok` — Wave 104 ActiveBody MaxHealth apply residual pack
//! - `drawable_create104_ok` — Wave 104 Drawable create residual bookkeeping pack
//! - `register_object104_ok` — Wave 104 GameLogic registerObject m_objList residual pack
//! - `ai_group105_ok` — Wave 105 AI group residual peels pack
//! - `ai_path105_ok` — Wave 105 AI path residual deepen pack
//! - `weapon_fire105_ok` — Wave 105 weapon fire residual deepen pack
//! - `damage_app105_ok` — Wave 105 damage application residual deepen pack
//! - `veterancy105_ok` — Wave 105 veterancy residual deepen pack
//! - `control_bar_path_resolved` / `control_bar_wnd_validated` — ControlBar.wnd residual
//! - `control_bar_window_loaded` — headless WindowManager parse when WindowZH present

//! Wave 957: host_object/host_objects authority dual-read seal.

mod helpers;
#[path = "result/mod.rs"]
mod result;
#[path = "imports/mod.rs"]
mod imports;
mod maps;
mod host;
mod presentation;
#[path = "honesty/mod.rs"]
mod honesty;
#[path = "honesty_waves/mod.rs"]
mod honesty_waves;
mod hud;
mod shell;
mod claim;
mod report;

/// Concatenated shell_smoke sources for residual `include_str` scans.
///
/// External residual packs previously read `shell_smoke.rs`. After the directory
/// split they should compare against this pack instead of a single file.
#[cfg(any(test, feature = "host-residuals"))]
pub const SHELL_SMOKE_SRC: &str = concat!(
    include_str!("mod.rs"),
    include_str!("helpers.rs"),
    include_str!("result.rs"),
    include_str!("imports.rs"),
    include_str!("maps.rs"),
    include_str!("host.rs"),
    include_str!("presentation.rs"),
    include_str!("honesty.rs"),
    include_str!("honesty_waves.rs"),
    include_str!("hud.rs"),
    include_str!("shell.rs"),
    include_str!("claim.rs"),
    include_str!("report.rs"),
);

pub use report::format_shell_smoke_report;
pub use result::ShellSmokeResult;

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;

/// Exercise production host entry points headlessly (no window required).
/// Builds config from live SkirmishMenu (including Medium AI slot via menu cycle),
/// applies it, loads retail map when present, advances logic frames, builds presentation,
/// applies dual-tick presentation → GameHUD selection/minimap, ensures ControlBar.wnd,
/// and exercises shell→InGame screen ownership (start_game_from_ui parity).
pub fn run_shell_smoke(frames: u32) -> ShellSmokeResult {
    let host = host::run_host_session(frames);
    let presentation = presentation::evaluate_presentation_residuals(&host.pres, host.presentation_ok);
    let early = honesty::evaluate_early_honesty(&host.pres, host.presentation_ok);
    let waves = honesty_waves::evaluate_honesty_waves();
    let (hud_selection_ok, selection_consumers_ok) = hud::evaluate_selection_honesty(&host);
    let shell_ui = shell::evaluate_shell_ui();
    // Never claim full retail playability from headless smoke (no W3D/window/GPU).
    let playable_claim = false;
    result::assemble(
        host,
        presentation,
        early,
        waves,
        hud_selection_ok,
        selection_consumers_ok,
        shell_ui,
        playable_claim,
    )
}
