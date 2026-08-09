//! ShellSmokeResult residual claim flags extracted from `shell_smoke.rs`.

#[derive(Debug, Clone)]
pub struct ShellSmokeResult {
    /// True only after GameLogic exists and skirmish config applied successfully.
    pub host_constructed: bool,
    pub skirmish_config_ok: bool,
    pub menu_config_ok: bool,
    pub map_resolved: bool,
    pub map_loaded: bool,
    pub frames_advanced: u32,
    pub presentation_ok: bool,
    /// Dual-tick residual: seed presentation, then logic update + build_and_apply order.
    pub dual_tick_presentation_ok: bool,
    /// Dual-tick residual counters (build/apply) on presentation snapshot.
    pub dual_tick_counters_ok: bool,
    /// Host vs gamelogic::GameWorld shadow count parity (migration probe).
    pub gameworld_shadow_ok: bool,
    /// Gate defaulted GENERALS_GAMEWORLD_DAMAGE_AUTHORITY on (or user set).
    pub damage_authority_env_ok: bool,
    /// Gate defaulted GENERALS_GAMEWORLD_ECONOMY_AUTHORITY on.
    pub economy_authority_env_ok: bool,
    /// Gate defaulted GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY on.
    pub production_authority_env_ok: bool,
    /// Dual crate tick is AuthorityOnly (no GENERALS_ALLOW_DUAL_TICK).
    pub dual_tick_policy_authority_only: bool,
    /// OBJECT_REGISTRY bridge env unset (engine_object_id stays None).
    pub engine_bridge_off: bool,
    /// Dual-tick residual: after map load, logic update + presentation seed HUD
    /// selection health / minimap without re-reading live objects.
    pub hud_selection_ok: bool,
    /// Minimap FOW residual: presentation `fow_grid` is host-usable (active R8 or honest inactive).
    pub minimap_fow_presentation_ok: bool,
    /// Laser residual: presentation → CPU SegLine vertex pack (+ synthetic non-empty pack).
    pub laser_segment_upload_ok: bool,
    /// Presentation projectiles → CPU trail pack residual (empty + synthetic fire).
    pub projectile_segment_upload_ok: bool,
    /// Presentation move destinations → CPU path-line pack residual.
    pub move_line_upload_ok: bool,
    /// Presentation attack targets → CPU order-line pack residual.
    pub attack_line_upload_ok: bool,
    /// OrbitalLaser multi-beam soft-edge CPU pack residual (NumBeams width/color lerp).
    pub multi_beam_soft_edge_ok: bool,
    /// Laser presentation residual: ground-height + soft-edge fields honesty.
    pub laser_presentation_residual_ok: bool,
    /// Floating-text residual: presentation freeze + CPU layout pack (+ synthetic).
    pub floating_text_layout_ok: bool,
    /// Floating-text vanish-rate alpha residual presentation field honesty.
    pub floating_text_vanish_ok: bool,
    /// MoneyPickUp Anim2D residual: presentation freeze honesty (empty or template ok).
    pub world_anim_presentation_ok: bool,
    /// World-anim residual: presentation → CPU Anim2D layout pack (+ synthetic).
    pub world_anim_layout_ok: bool,
    /// World-anim fade residual presentation field honesty.
    pub world_anim_fade_ok: bool,
    /// MoneyPickUp Anim2D frame advance residual (LOOP / SCPDollarNNN).
    pub anim2d_frame_ok: bool,
    /// Anim2DCollection template/instance residual (host-testable, no GPU).
    pub anim2d_collection_residual_ok: bool,
    /// GameText translate_copy escape table residual (host-testable, no GPU).
    pub translate_copy_residual_ok: bool,
    /// GameText `GUI:AddCash` caption residual on synthetic floating-text pack.
    pub game_text_caption_ok: bool,
    /// CSF/STR GameText residual + retail `$%d` printf + DisplayString measure.
    pub game_text_csf_str_ok: bool,
    /// DisplayString monospaced measure residual on floating-text pack.
    pub display_string_measure_ok: bool,
    /// GameLogic/GameClient RandomValue ADC stream residual honesty.
    pub rng_stream_residual_ok: bool,
    /// W3D mesh asset resolve residual (common keys / scale / search / basename).
    /// Host-testable; does **not** claim live GPU upload or retail material parity.
    pub mesh_asset_residual_ok: bool,
    /// Wave 72 host RNG residual pack honesty (seed table / pure index / stream).
    pub rng_residual_pack_ok: bool,
    /// Wave 72 special-power residual pack (DaisyCutter / A10 / free pack).
    pub special_power_wave72_residual_ok: bool,
    /// Wave 73 Spectre/Nuke/SupW special-power residual pack honesty.
    pub special_power_wave73_residual_ok: bool,
    /// Wave 76 A10 science-tier FormationSize residual pack honesty.
    pub special_power_wave76_residual_ok: bool,
    /// Wave 76 Paradrop science-tier payload residual pack honesty.
    pub paradrop_wave76_residual_ok: bool,
    /// Wave 76 ControlBar window-count / named-child / font residual pack honesty.
    pub control_bar_wave76_residual_ok: bool,
    /// Wave 76 InGameUI font table + DisplayString vanish color-alpha residual honesty.
    pub graphics_wave76_residual_ok: bool,
    /// Wave 73 presentation Spectre orbit decal residual honesty.
    pub spectre_orbit_decal_presentation_ok: bool,
    /// Wave 77 special-power audio name table residual honesty.
    pub special_power_wave77_residual_ok: bool,
    /// Wave 77 FOW residual honesty pack (cell/R8/inactive fail-open).
    pub fow_residual_pack_ok: bool,
    /// Wave 77 unit/structure ground-height presentation residual honesty.
    pub ground_height_presentation_ok: bool,
    /// Wave 77 host WeaponStore seed residual honesty pack.
    pub weapon_store_seed_residual_ok: bool,
    /// Wave 77 AI skirmish structure/team timer residual honesty pack.
    pub ai_skirmish_residual_ok: bool,
    /// Wave 78 HostSuperweaponKind reload + CarpetBomb/Artillery science residual pack.
    pub special_power_wave78_residual_ok: bool,
    /// Wave 78 ClusterMines DeliveryDecal / science residual pack.
    pub cluster_mines_wave78_residual_ok: bool,
    /// Wave 78 GPS Scrambler science / marker particle residual pack.
    pub gps_scrambler_wave78_residual_ok: bool,
    /// Wave 78 CashBountyScienceTier residual pack.
    pub cash_bounty_wave78_residual_ok: bool,
    /// Wave 79 minimap FOW shade/size residual honesty pack.
    pub minimap_residual_pack_ok: bool,
    /// Wave 79 selection/HUD color residual honesty pack.
    pub selection_hud_residual_pack_ok: bool,
    /// Wave 79 drag/double-click input residual honesty pack.
    pub input_residual_pack_ok: bool,
    /// Wave 79 Drawable StealthLook save/load residual honesty.
    pub drawable_residual_fields_ok: bool,
    /// Wave 79 unit-training/veterancy residual deepen honesty pack.
    pub unit_training_wave79_residual_ok: bool,
    /// Wave 79 upgrade cost/time residual application honesty.
    pub upgrades_cost_time_application_ok: bool,
    /// Wave 80 superweapon CommandButton label/cursor residual honesty pack.
    pub command_button_wave80_residual_ok: bool,
    /// Wave 80 Rank.ini SCIENCE rank residual table honesty pack.
    pub science_rank_wave80_residual_ok: bool,
    /// Wave 80 superweapon building KindOf residual honesty pack.
    pub superweapon_kindof_wave80_residual_ok: bool,
    /// Wave 80 SpecialPower enum discriminant residual honesty pack.
    pub special_power_enum_wave80_residual_ok: bool,
    /// Wave 81 map height sample residual honesty pack.
    pub terrain_height_sample_wave81_ok: bool,
    /// Wave 81 Pathfinder body/locomotor residual deepen honesty pack.
    pub pathfinder_wave81_residual_ok: bool,
    /// Wave 81 common-unit locomotor residual table honesty.
    pub locomotor_table_wave81_ok: bool,
    /// Wave 81 ProjectileArmor / HazardousMaterialArmor residual table honesty.
    pub armor_table_wave81_ok: bool,
    /// Wave 81 PUC outer-node flare particle name table residual honesty.
    pub puc_flare_table_wave81_ok: bool,
    /// Wave 82 DamageType residual enum table honesty.
    pub damage_type_wave82_ok: bool,
    /// Wave 82 DeathType residual enum table honesty.
    pub death_type_wave82_ok: bool,
    /// Wave 82 ModelCondition residual flags honesty (incl. CONTINUOUS_FIRE_*).
    pub model_condition_wave82_ok: bool,
    /// Wave 82 WeaponBonus residual type table honesty.
    pub weapon_bonus_wave82_ok: bool,
    /// Wave 82 ObjectStatus / StatusBits residual table honesty.
    pub object_status_wave82_ok: bool,
    /// Wave 83 production queue residual (MaxQueue/energy/refund/doors).
    pub production_queue_wave83_ok: bool,
    /// Wave 83 supply warehouse residual (boxes/value/cripple heal).
    pub supply_warehouse_wave83_ok: bool,
    /// Wave 83 dozer build residual (DozerAI/build pads/construction rate).
    pub dozer_build_wave83_ok: bool,
    /// Wave 83 capture building residual (Ranger infantry capture pack).
    pub capture_building_wave83_ok: bool,
    /// Wave 83 power plant residual energy pack.
    pub power_plant_wave83_ok: bool,
    /// Wave 83 command center residual peels.
    pub command_center_wave83_ok: bool,
    /// Wave 84 KindOf residual bit-name table honesty (KINDOF_COUNT 116).
    pub kindof_wave84_ok: bool,
    /// Wave 84 WeaponSlot PRIMARY/SECONDARY/TERTIARY residual table honesty.
    pub weapon_slot_wave84_ok: bool,
    /// Wave 84 Veterancy residual level table honesty.
    pub veterancy_wave84_ok: bool,
    /// Wave 84 Relationship ENEMIES/NEUTRAL/ALLIES residual table honesty.
    pub relationship_wave84_ok: bool,
    /// Wave 84 Geometry SPHERE/CYLINDER/BOX residual table honesty.
    pub geometry_wave84_ok: bool,
    /// Wave 84 Shadow residual type bit-name table honesty.
    pub shadow_wave84_ok: bool,
    /// Wave 85 faction side residual table honesty.
    pub faction_side_wave85_ok: bool,
    /// Wave 85 player template residual peels honesty.
    pub player_template_wave85_ok: bool,
    /// Wave 85 starting cash residual (+ difficulty health) honesty.
    pub starting_cash_wave85_ok: bool,
    /// Wave 85 skirmish AI personality / SideInfo residual honesty.
    pub skirmish_ai_personality_wave85_ok: bool,
    /// Wave 85 victory condition residual peels honesty.
    pub victory_condition_wave85_ok: bool,
    /// Wave 86 GameData camera/FPS residual pack honesty.
    pub gamedata_camera_fps_wave86_ok: bool,
    /// Wave 86 GameData world constants residual pack honesty.
    pub gamedata_world_constants_wave86_ok: bool,
    /// Wave 86 multiplayer options residual pack honesty (host-only).
    pub multiplayer_options_wave86_ok: bool,
    /// Wave 86 map selection residual pack honesty.
    pub map_selection_wave86_ok: bool,
    /// Wave 86 crate residual deepen pack honesty.
    pub crate_deepen_wave86_ok: bool,
    /// Wave 87 weather (snow) residual pack honesty.
    pub weather_wave87_ok: bool,
    /// Wave 87 water / TimeOfDay residual pack honesty.
    pub water_wave87_ok: bool,
    /// Wave 87 bridge tower / scaffold residual pack honesty.
    pub bridge_wave87_ok: bool,
    /// Wave 87 tunnel residual deepen pack honesty.
    pub tunnel_wave87_ok: bool,
    /// Wave 87 garrison residual pack honesty.
    pub garrison_wave87_ok: bool,
    /// Wave 87 transport residual pack honesty.
    pub transport_wave87_ok: bool,
    /// Wave 88 RadiusCursor residual name table honesty.
    pub radius_cursor_wave88_ok: bool,
    /// Wave 88 MouseCursor residual name table honesty.
    pub mouse_cursor_wave88_ok: bool,
    /// Wave 88 superweapon FXList residual name table honesty.
    pub superweapon_fxlist_wave88_ok: bool,
    /// Wave 88 superweapon OCL residual name table honesty.
    pub superweapon_ocl_wave88_ok: bool,
    /// Wave 88 superweapon particle residual name table expand honesty.
    pub superweapon_particle_wave88_ok: bool,
    /// Wave 88 superweapon audio event residual name table expand honesty.
    pub superweapon_audio_wave88_ok: bool,
    /// Wave 89 rank skill-points application residual deepen honesty.
    pub rank_skill_wave89_ok: bool,
    /// Wave 89 experience residual tables pack honesty.
    pub experience_wave89_ok: bool,
    /// Wave 89 hotkey CommandMap residual table honesty.
    pub hotkey_wave89_ok: bool,
    /// Wave 89 chat residual host peels honesty.
    pub chat_wave89_ok: bool,
    /// Wave 89 local replay residual host peels honesty.
    pub replay_wave89_ok: bool,
    /// Wave 89 options residual peels honesty.
    pub options_wave89_ok: bool,
    /// Wave 90 GameSpeed residual pack honesty.
    pub gamespeed_wave90_ok: bool,
    /// Wave 90 frame rate residual deepen pack honesty.
    pub frame_rate_wave90_ok: bool,
    /// Wave 90 debug residual tables pack honesty (host-only).
    pub debug_tables_wave90_ok: bool,
    /// Wave 90 language residual deepen pack honesty.
    pub language_wave90_ok: bool,
    /// Wave 90 credits residual pack honesty.
    pub credits_wave90_ok: bool,
    /// Wave 91 tooltip residual pack honesty.
    pub tooltip_wave91_ok: bool,
    /// Wave 91 HelpBox residual pack honesty.
    pub help_box_wave91_ok: bool,
    /// Wave 91 message residual pack honesty.
    pub message_wave91_ok: bool,
    /// Wave 91 EVA residual pack honesty.
    pub eva_wave91_ok: bool,
    /// Wave 91 video residual name table honesty (names only).
    pub video_wave91_ok: bool,
    /// Wave 91 mission briefing residual pack honesty.
    pub mission_briefing_wave91_ok: bool,
    /// Wave 92 weapon template residual deepen honesty.
    pub weapon_deepen_wave92_ok: bool,
    /// Wave 92 armor residual expand honesty.
    pub armor_expand_wave92_ok: bool,
    /// Wave 92 body MaxHealth residual table honesty.
    pub body_health_wave92_ok: bool,
    /// Wave 92 locomotor residual expand honesty.
    pub locomotor_expand_wave92_ok: bool,
    /// Wave 92 science residual full name table honesty.
    pub science_names_wave92_ok: bool,
    /// Wave 93 particle system emit-rate residual deepen honesty.
    pub particle_emit_wave93_ok: bool,
    /// Wave 93 drawable opacity/shroud residual deepen honesty.
    pub drawable_opacity_wave93_ok: bool,
    /// Wave 93 shadow residual deepen honesty.
    pub shadow_deepen_wave93_ok: bool,
    /// Wave 93 terrain texture residual pack honesty.
    pub terrain_texture_wave93_ok: bool,
    /// Wave 93 road residual pack honesty.
    pub road_wave93_ok: bool,
    /// Wave 94 AI state residual table honesty.
    pub ai_state_wave94_ok: bool,
    /// Wave 94 special ability residual deepen honesty.
    pub special_ability_wave94_ok: bool,
    /// Wave 94 upgrade full name table residual honesty.
    pub upgrade_names_wave94_ok: bool,
    /// Wave 94 CommandSet superweapon residual honesty.
    pub command_set_wave94_ok: bool,
    /// Wave 95 script action name table residual honesty.
    pub script_action_wave95_ok: bool,
    /// Wave 95 script condition name table residual honesty.
    pub script_condition_wave95_ok: bool,
    /// Wave 95 map object residual pack honesty.
    pub map_object_wave95_ok: bool,
    /// Wave 95 waypoint residual pack honesty.
    pub waypoint_wave95_ok: bool,
    /// Wave 95 team residual pack honesty.
    pub team_wave95_ok: bool,
    /// Wave 95 player residual deepen pack honesty.
    pub player_deepen_wave95_ok: bool,
    /// Wave 96 partition residual pack honesty.
    pub partition_wave96_ok: bool,
    /// Wave 96 collision residual pack honesty.
    pub collision_wave96_ok: bool,
    /// Wave 96 physics residual pack honesty.
    pub physics_wave96_ok: bool,
    /// Wave 96 projectile residual deepen pack honesty.
    pub projectile_wave96_ok: bool,
    /// Wave 97 radar residual deepen pack honesty.
    pub radar_deepen_wave97_ok: bool,
    /// Wave 97 spotter residual pack honesty.
    pub spotter_wave97_ok: bool,
    /// Wave 97 stealth residual deepen pack honesty.
    pub stealth_deepen_wave97_ok: bool,
    /// Wave 97 detector residual deepen pack honesty.
    pub detector_deepen_wave97_ok: bool,
    /// Wave 97 vision residual pack honesty.
    pub vision_wave97_ok: bool,
    /// Wave 98 dock residual pack honesty.
    pub dock_wave98_ok: bool,
    /// Wave 98 contain residual deepen pack honesty.
    pub contain_wave98_ok: bool,
    /// Wave 98 exit residual pack honesty.
    pub exit_wave98_ok: bool,
    /// Wave 98 heal residual deepen pack honesty.
    pub heal_wave98_ok: bool,
    /// Wave 99 production residual deepen pack honesty.
    pub production_deepen_wave99_ok: bool,
    /// Wave 99 buildable residual pack honesty.
    pub buildable_wave99_ok: bool,
    /// Wave 99 prerequisite residual pack honesty.
    pub prerequisite_wave99_ok: bool,
    /// Wave 99 command button residual deepen pack honesty.
    pub command_button_deepen_wave99_ok: bool,
    /// Wave 99 control bar residual deepen pack honesty.
    pub control_bar_deepen_wave99_ok: bool,
    /// Wave 100 ThingFactory residual deepen pack honesty.
    pub thing_factory_deepen_wave100_ok: bool,
    /// Wave 100 Module type table residual pack honesty.
    pub module_type_wave100_ok: bool,
    /// Wave 100 Xfer residual deepen pack honesty.
    pub xfer_deepen_wave100_ok: bool,
    /// Wave 100 ThingFactory spawn cross-link residual pack honesty.
    pub thing_factory_crosslink_wave100_ok: bool,
    /// Wave 101 ModuleFactory residual deepen pack honesty.
    pub module_factory_deepen_wave101_ok: bool,
    /// Wave 101 ThingFactory create residual deepen pack honesty.
    pub thing_factory_create_wave101_ok: bool,
    /// Wave 101 PartitionManager register residual pack honesty.
    pub partition_register_wave101_ok: bool,
    /// Wave 101 ThingFactory/Module/Partition cross-link residual pack honesty.
    pub mf_crosslink_wave101_ok: bool,
    /// Wave 102 DisplayString FontChars spacing + StretchRect submit residual pack.
    pub display_string_deepen_wave102_ok: bool,
    /// Wave 102 Anim2D full template table / Collection init residual pack.
    pub anim2d_deepen_wave102_ok: bool,
    /// Wave 102 laser SegLine UV atlas / multi-beam soft-edge residual pack.
    pub laser_segliner_deepen_wave102_ok: bool,
    /// Wave 102 multi-locale CSF expanded pack-load residual honesty.
    pub csf_multi_locale_deepen_wave102_ok: bool,
    /// Wave 102 presentation dual-tick residual deepen pack honesty.
    pub presentation_deepen_wave102_ok: bool,
    /// Wave 103 weapon residual deepen pack honesty.
    pub weapon_deepen_wave103_ok: bool,
    /// Wave 103 armor residual expand pack honesty.
    pub armor_expand_wave103_ok: bool,
    /// Wave 103 locomotor residual expand pack honesty.
    pub locomotor_expand_wave103_ok: bool,
    /// Wave 103 special-power superweapon residual deepen pack honesty.
    pub special_power_deepen_wave103_ok: bool,
    /// Wave 103 object KindOf residual pack honesty.
    pub object_kindof_wave103_ok: bool,
    /// Wave 104 Object status-mask residual state machine pack honesty.
    pub object_status_wave104_ok: bool,
    /// Wave 104 Object create residual order pack honesty.
    pub object_create_wave104_ok: bool,
    /// Wave 104 ActiveBody MaxHealth apply residual pack honesty.
    pub active_body_wave104_ok: bool,
    /// Wave 104 Drawable create residual bookkeeping pack honesty.
    pub drawable_create_wave104_ok: bool,
    /// Wave 104 GameLogic registerObject m_objList residual pack honesty.
    pub register_object_wave104_ok: bool,
    /// Wave 105 AI group residual peels pack honesty.
    pub ai_group_wave105_ok: bool,
    /// Wave 105 AI path residual deepen pack honesty.
    pub ai_path_wave105_ok: bool,
    /// Wave 105 weapon fire residual deepen pack honesty.
    pub weapon_fire_wave105_ok: bool,
    /// Wave 105 damage application residual deepen pack honesty.
    pub damage_application_wave105_ok: bool,
    /// Wave 105 veterancy residual deepen pack honesty.
    pub veterancy_wave105_ok: bool,
    /// Wave 106 GameState residual deepen pack honesty.
    pub game_state_deepen_wave106_ok: bool,
    /// Wave 106 campaign mission residual tables pack honesty.
    pub campaign_mission_wave106_ok: bool,
    /// Wave 106 MainMenu residual deepen pack honesty.
    pub main_menu_deepen_wave106_ok: bool,
    /// Wave 106 GameWindow residual deepen pack honesty.
    pub game_window_deepen_wave106_ok: bool,
    /// Wave 106 WindowLayout residual deepen pack honesty.
    pub window_layout_deepen_wave106_ok: bool,
    /// Wave 107 ParticleSystem residual deepen pack honesty.
    pub particle_system_deepen_wave107_ok: bool,
    /// Wave 107 FXList entry residual deepen pack honesty.
    pub fxlist_entry_deepen_wave107_ok: bool,
    /// Wave 107 OCL Create residual deepen pack honesty.
    pub ocl_create_deepen_wave107_ok: bool,
    /// Wave 107 Audio residual deepen pack honesty.
    pub audio_deepen_wave107_ok: bool,
    /// Wave 108 HeightMap residual deepen pack honesty.
    pub heightmap_deepen_wave108_ok: bool,
    /// Wave 108 bridge residual deepen pack honesty.
    pub bridge_deepen_wave108_ok: bool,
    /// Wave 108 water residual deepen pack honesty.
    pub water_deepen_wave108_ok: bool,
    /// Wave 108 road residual deepen pack honesty.
    pub road_deepen_wave108_ok: bool,
    /// Wave 108 cliff residual peels pack honesty.
    pub cliff_peels_wave108_ok: bool,
    /// Wave 109 SpecialPower template store residual pack honesty.
    pub special_power_store_wave109_ok: bool,
    /// Wave 109 Science store residual deepen pack honesty.
    pub science_store_wave109_ok: bool,
    /// Wave 109 Upgrade store residual deepen pack honesty.
    pub upgrade_store_wave109_ok: bool,
    /// Wave 109 Player residual deepen pack honesty.
    pub player_deepen_wave109_ok: bool,
    /// Wave 109 Team residual deepen pack honesty.
    pub team_deepen_wave109_ok: bool,
    /// Wave 110 MessageStream marker residual pack honesty.
    pub message_stream_marker_wave110_ok: bool,
    /// Wave 110 GameMessage argument type residual pack honesty.
    pub game_message_arg_wave110_ok: bool,
    /// Wave 110 MetaEvent category residual pack honesty.
    pub meta_event_category_wave110_ok: bool,
    /// Wave 110 InGameUI residual pack honesty.
    pub ingame_ui_wave110_ok: bool,
    /// Wave 111 Drawable icon/flash residual pack honesty.
    pub drawable_icon_flash_wave111_ok: bool,
    /// Wave 111 Drawable status/stealth residual pack honesty.
    pub drawable_status_stealth_wave111_ok: bool,
    /// Wave 111 Terrain decal residual pack honesty.
    pub terrain_decal_wave111_ok: bool,
    /// Wave 111 Display draw-image mode residual pack honesty.
    pub display_draw_image_wave111_ok: bool,
    /// Wave 111 GameClient translator residual pack honesty.
    pub game_client_translator_wave111_ok: bool,
    /// Wave 111 Particle priority residual pack honesty.
    pub particle_priority_wave111_ok: bool,
    /// Wave 112 Mouse residual pack honesty.
    pub mouse_residual_wave112_ok: bool,
    /// Wave 112 Keyboard residual pack honesty.
    pub keyboard_residual_wave112_ok: bool,
    /// Wave 112 View residual pack honesty.
    pub view_residual_wave112_ok: bool,
    /// Wave 113 GameWindow manager residual pack honesty.
    pub game_window_manager_wave113_ok: bool,
    /// Wave 113 Window style residual pack honesty.
    pub window_style_wave113_ok: bool,
    /// Wave 113 Gadget residual pack honesty.
    pub gadget_wave113_ok: bool,
    /// Wave 113 Video buffer residual pack honesty.
    pub video_buffer_wave113_ok: bool,
    /// Wave 113 Audio event residual pack honesty.
    pub audio_event_wave113_ok: bool,
    /// Wave 114 MainMenu skirmish names residual pack honesty.
    pub main_menu_skirmish_names_wave114_ok: bool,
    /// Wave 114 MainMenu skirmish nav steps residual pack honesty.
    pub main_menu_skirmish_nav_steps_wave114_ok: bool,
    /// Wave 114 MainMenu skirmish message residual pack honesty.
    pub main_menu_skirmish_message_wave114_ok: bool,
    /// Wave 115 Skirmish map-select names residual pack honesty.
    pub map_select_names_wave115_ok: bool,
    /// Wave 115 Skirmish map-select nav steps residual pack honesty.
    pub map_select_nav_steps_wave115_ok: bool,
    /// Wave 115 Skirmish map-select commands residual pack honesty.
    pub map_select_commands_wave115_ok: bool,
    /// Wave 116 Skirmish slot state residual pack honesty.
    pub slot_state_wave116_ok: bool,
    /// Wave 116 Skirmish slot combo names residual pack honesty.
    pub slot_combo_names_wave116_ok: bool,
    /// Wave 116 Skirmish slot nav/commands residual pack honesty.
    pub slot_nav_commands_wave116_ok: bool,
    /// Wave 117 Skirmish starting cash residual pack honesty.
    pub starting_cash_wave117_ok: bool,
    /// Wave 117 Skirmish game speed/controls residual pack honesty.
    pub game_speed_controls_wave117_ok: bool,
    /// Wave 117 Skirmish rules nav/commands residual pack honesty.
    pub rules_nav_commands_wave117_ok: bool,
    /// Wave 118 MainMenu button names residual pack honesty.
    pub main_menu_button_names_wave118_ok: bool,
    /// Wave 118 MainMenu push targets residual pack honesty.
    pub main_menu_push_targets_wave118_ok: bool,
    /// Wave 118 MainMenu button nav/commands residual pack honesty.
    pub main_menu_button_nav_commands_wave118_ok: bool,
    /// Wave 119 MainMenu campaign button names residual pack honesty.
    pub campaign_button_names_wave119_ok: bool,
    /// Wave 119 MainMenu campaign enums residual pack honesty.
    pub campaign_enums_wave119_ok: bool,
    /// Wave 119 MainMenu campaign nav/commands residual pack honesty.
    pub campaign_nav_commands_wave119_ok: bool,
    /// Wave 120 ChallengeMenu control names residual pack honesty.
    pub challenge_control_names_wave120_ok: bool,
    /// Wave 120 ChallengeMenu nav/commands residual pack honesty.
    pub challenge_nav_commands_wave120_ok: bool,
    /// Wave 121 SaveLoad layout residual pack honesty.
    pub save_load_layout_wave121_ok: bool,
    /// Wave 121 SaveLoad control stems residual pack honesty.
    pub save_load_control_stems_wave121_ok: bool,
    /// Wave 121 SaveLoad nav/commands residual pack honesty.
    pub save_load_nav_commands_wave121_ok: bool,
    /// Wave 122 ReplayMenu control names residual pack honesty.
    pub replay_control_names_wave122_ok: bool,
    /// Wave 122 ReplayMenu nav/commands residual pack honesty.
    pub replay_nav_commands_wave122_ok: bool,
    /// Wave 123 QuitMenu control names residual pack honesty.
    pub quit_control_names_wave123_ok: bool,
    /// Wave 123 QuitMenu nav/commands residual pack honesty.
    pub quit_nav_commands_wave123_ok: bool,
    /// Wave 124 KeyboardOptions control names residual pack honesty.
    pub keyboard_control_names_wave124_ok: bool,
    /// Wave 124 KeyboardOptions nav/commands residual pack honesty.
    pub keyboard_nav_commands_wave124_ok: bool,
    /// Wave 125 ScoreScreen control names residual pack honesty.
    pub score_control_names_wave125_ok: bool,
    /// Wave 125 ScoreScreen nav/commands residual pack honesty.
    pub score_nav_commands_wave125_ok: bool,
    /// Wave 126 OptionsMenu control names residual pack honesty.
    pub options_control_names_wave126_ok: bool,
    /// Wave 126 OptionsMenu nav/commands residual pack honesty.
    pub options_nav_commands_wave126_ok: bool,
    /// Wave 127 CreditsMenu control names residual pack honesty.
    pub credits_control_names_wave127_ok: bool,
    /// Wave 127 CreditsMenu nav/commands residual pack honesty.
    pub credits_nav_commands_wave127_ok: bool,
    /// Wave 128 MessageBox control names residual pack honesty.
    pub message_box_control_names_wave128_ok: bool,
    /// Wave 128 MessageBox nav/commands residual pack honesty.
    pub message_box_nav_commands_wave128_ok: bool,
    /// Wave 129 Diplomacy control names residual pack honesty.
    pub diplomacy_control_names_wave129_ok: bool,
    /// Wave 129 Diplomacy nav/commands residual pack honesty.
    pub diplomacy_nav_commands_wave129_ok: bool,
    /// Wave 130 PopupReplay control names residual pack honesty.
    pub popup_replay_control_names_wave130_ok: bool,
    /// Wave 130 PopupReplay nav/commands residual pack honesty.
    pub popup_replay_nav_commands_wave130_ok: bool,
    /// Wave 131 SinglePlayerMenu control names residual pack honesty.
    pub single_player_control_names_wave131_ok: bool,
    /// Wave 131 SinglePlayerMenu nav/commands residual pack honesty.
    pub single_player_nav_commands_wave131_ok: bool,
    /// Wave 132 MapSelectMenu control names residual pack honesty.
    pub map_select_control_names_wave132_ok: bool,
    /// Wave 132 MapSelectMenu nav/commands residual pack honesty.
    pub map_select_nav_commands_wave132_ok: bool,
    /// Wave 133 ControlBar control names residual pack honesty.
    pub control_bar_control_names_wave133_ok: bool,
    /// Wave 133 ControlBar nav/commands residual pack honesty.
    pub control_bar_nav_commands_wave133_ok: bool,
    /// Wave 134 DifficultySelect control names residual pack honesty.
    pub difficulty_select_control_names_wave134_ok: bool,
    /// Wave 134 DifficultySelect nav/commands residual pack honesty.
    pub difficulty_select_nav_commands_wave134_ok: bool,
    /// Wave 135 LoadingScreen stages residual pack honesty.
    pub loading_screen_stages_wave135_ok: bool,
    /// Wave 135 LoadingScreen nav/commands residual pack honesty.
    pub loading_screen_nav_commands_wave135_ok: bool,
    /// Wave 136 InGameChat control names residual pack honesty.
    pub in_game_chat_control_names_wave136_ok: bool,
    /// Wave 136 InGameChat nav/commands residual pack honesty.
    pub in_game_chat_nav_commands_wave136_ok: bool,
    /// Wave 137 IdleWorker control names residual pack honesty.
    pub idle_worker_control_names_wave137_ok: bool,
    /// Wave 137 IdleWorker nav/commands residual pack honesty.
    pub idle_worker_nav_commands_wave137_ok: bool,
    /// Wave 138 GeneralsExp control names residual pack honesty.
    pub generals_exp_control_names_wave138_ok: bool,
    /// Wave 138 GeneralsExp nav/commands residual pack honesty.
    pub generals_exp_nav_commands_wave138_ok: bool,
    /// Wave 139 PopupCommunicator control names residual pack honesty.
    pub popup_communicator_control_names_wave139_ok: bool,
    /// Wave 139 PopupCommunicator nav/commands residual pack honesty.
    pub popup_communicator_nav_commands_wave139_ok: bool,
    /// Wave 140 ReplayControl control names residual pack honesty.
    pub replay_control_control_names_wave140_ok: bool,
    /// Wave 140 ReplayControl nav/commands residual pack honesty.
    pub replay_control_nav_commands_wave140_ok: bool,
    /// Wave 141 ShellMap names residual pack honesty.
    pub shell_map_names_wave141_ok: bool,
    /// Wave 141 ShellMap nav/commands residual pack honesty.
    pub shell_map_nav_commands_wave141_ok: bool,
    /// Wave 142 Beacon control names residual pack honesty.
    pub beacon_control_names_wave142_ok: bool,
    /// Wave 142 Beacon nav/commands residual pack honesty.
    pub beacon_nav_commands_wave142_ok: bool,
    /// Wave 143 EVA message names residual pack honesty.
    pub eva_message_names_wave143_ok: bool,
    /// Wave 143 EVA nav/commands residual pack honesty.
    pub eva_nav_commands_wave143_ok: bool,
    /// Wave 144 IME message names residual pack honesty.
    pub ime_message_names_wave144_ok: bool,
    /// Wave 144 IME nav/commands residual pack honesty.
    pub ime_nav_commands_wave144_ok: bool,
    /// Wave 145 Smudge method names residual pack honesty.
    pub smudge_method_names_wave145_ok: bool,
    /// Wave 145 Smudge nav/commands residual pack honesty.
    pub smudge_nav_commands_wave145_ok: bool,
    /// Wave 146 OCL timer method names residual pack honesty.
    pub ocl_timer_method_names_wave146_ok: bool,
    /// Wave 146 OCL timer nav/commands residual pack honesty.
    pub ocl_timer_nav_commands_wave146_ok: bool,
    /// Wave 147 ControlBarResizer method names residual pack honesty.
    pub control_bar_resizer_method_names_wave147_ok: bool,
    /// Wave 147 ControlBarResizer nav/commands residual pack honesty.
    pub control_bar_resizer_nav_commands_wave147_ok: bool,
    /// Wave 148 UnderConstruction method names residual pack honesty.
    pub under_construction_method_names_wave148_ok: bool,
    /// Wave 148 UnderConstruction nav/commands residual pack honesty.
    pub under_construction_nav_commands_wave148_ok: bool,
    /// Wave 149 StructureInventory command names residual pack honesty.
    pub structure_inventory_command_names_wave149_ok: bool,
    /// Wave 149 StructureInventory nav/commands residual pack honesty.
    pub structure_inventory_nav_commands_wave149_ok: bool,
    /// Wave 150 MultiSelect method names residual pack honesty.
    pub multi_select_method_names_wave150_ok: bool,
    /// Wave 150 MultiSelect nav/commands residual pack honesty.
    pub multi_select_nav_commands_wave150_ok: bool,
    /// Wave 151 Credits style/method names residual pack honesty.
    pub credits_style_method_names_wave151_ok: bool,
    /// Wave 151 Credits nav/commands residual pack honesty.
    pub credits_nav_commands_wave151_ok: bool,
    /// Wave 152 ChallengeGenerals method names residual pack honesty.
    pub challenge_generals_method_names_wave152_ok: bool,
    /// Wave 152 ChallengeGenerals nav/commands residual pack honesty.
    pub challenge_generals_nav_commands_wave152_ok: bool,
    /// Wave 153 GameWorld authority env names residual pack honesty.
    pub gameworld_authority_env_names_wave153_ok: bool,
    /// Wave 153 GameWorld authority method names residual pack honesty.
    pub gameworld_authority_method_names_wave153_ok: bool,
    /// Wave 153 GameWorld authority nav/commands residual pack honesty.
    pub gameworld_authority_nav_commands_wave153_ok: bool,
    /// Wave 154 WindowVideo type/state names residual pack honesty.
    pub window_video_type_state_names_wave154_ok: bool,
    /// Wave 154 WindowVideo method names residual pack honesty.
    pub window_video_method_names_wave154_ok: bool,
    /// Wave 154 WindowVideo nav/commands residual pack honesty.
    pub window_video_nav_commands_wave154_ok: bool,
    /// Wave 155 MainMenu layout names residual pack honesty.
    pub main_menu_layout_names_wave155_ok: bool,
    /// Wave 155 MainMenu layout nav/commands residual pack honesty.
    pub main_menu_layout_nav_commands_wave155_ok: bool,
    /// Wave 156 ControlBarScheme names residual pack honesty.
    pub control_bar_scheme_names_wave156_ok: bool,
    /// Wave 156 ControlBarScheme method names residual pack honesty.
    pub control_bar_scheme_method_names_wave156_ok: bool,
    /// Wave 156 ControlBarScheme nav/commands residual pack honesty.
    pub control_bar_scheme_nav_commands_wave156_ok: bool,
    /// Wave 157 presentation boundary method names residual pack honesty.
    pub presentation_boundary_method_names_wave157_ok: bool,
    /// Wave 157 presentation boundary source markers residual pack honesty.
    pub presentation_boundary_source_markers_wave157_ok: bool,
    /// Wave 157 presentation boundary nav/commands residual pack honesty.
    pub presentation_boundary_nav_commands_wave157_ok: bool,
    /// Wave 157 presentation boundary live source residual honesty.
    pub presentation_boundary_live_wave157_ok: bool,
    /// Wave 158 ControlBar print-positions names residual pack honesty.
    pub control_bar_print_names_wave158_ok: bool,
    /// Wave 158 ControlBar print-positions nav/commands residual pack honesty.
    pub control_bar_print_nav_commands_wave158_ok: bool,
    /// Wave 159 terrain/skybox boundary method names residual pack honesty.
    pub terrain_env_boundary_method_names_wave159_ok: bool,
    /// Wave 159 terrain/skybox boundary source markers residual pack honesty.
    pub terrain_env_boundary_source_markers_wave159_ok: bool,
    /// Wave 159 terrain/skybox boundary nav/commands residual pack honesty.
    pub terrain_env_boundary_nav_commands_wave159_ok: bool,
    /// Wave 159 terrain/skybox boundary live source residual honesty.
    pub terrain_env_boundary_live_wave159_ok: bool,
    /// Wave 160 MainMenu.wnd names residual pack honesty.
    pub main_menu_wnd_names_wave160_ok: bool,
    /// Wave 160 MainMenu.wnd nav/commands residual pack honesty.
    pub main_menu_wnd_nav_commands_wave160_ok: bool,
    /// Wave 160 MainMenu.wnd resolve/validate live residual honesty.
    pub main_menu_wnd_live_wave160_ok: bool,
    /// Wave 161 MainMenu.wnd load method names residual pack honesty.
    pub main_menu_wnd_load_method_names_wave161_ok: bool,
    /// Wave 161 MainMenu.wnd load nav/commands residual pack honesty.
    pub main_menu_wnd_load_nav_commands_wave161_ok: bool,
    /// Wave 161 MainMenu.wnd headless load live residual honesty.
    pub main_menu_wnd_load_live_wave161_ok: bool,
    /// Wave 162 MainMenu.wnd materialise method names residual pack honesty.
    pub main_menu_wnd_materialise_method_names_wave162_ok: bool,
    /// Wave 162 MainMenu.wnd materialise nav/commands residual pack honesty.
    pub main_menu_wnd_materialise_nav_commands_wave162_ok: bool,
    /// Wave 162 MainMenu.wnd materialise live residual honesty.
    pub main_menu_wnd_materialise_live_wave162_ok: bool,
    /// Wave 163 Shell stack push method names residual pack honesty.
    pub shell_stack_push_method_names_wave163_ok: bool,
    /// Wave 163 Shell stack push nav/commands residual pack honesty.
    pub shell_stack_push_nav_commands_wave163_ok: bool,
    /// Wave 163 Shell stack push live residual honesty.
    pub shell_stack_push_live_wave163_ok: bool,
    /// Wave 164 Shell→Skirmish nav method names residual pack honesty.
    pub shell_skirmish_nav_method_names_wave164_ok: bool,
    /// Wave 164 Shell→Skirmish nav/commands residual pack honesty.
    pub shell_skirmish_nav_commands_wave164_ok: bool,
    /// Wave 164 Shell→Skirmish nav live residual honesty.
    pub shell_skirmish_nav_live_wave164_ok: bool,
    /// Wave 165 ControlBar.wnd materialise method names residual pack honesty.
    pub control_bar_materialise_method_names_wave165_ok: bool,
    /// Wave 165 ControlBar.wnd materialise nav/commands residual pack honesty.
    pub control_bar_materialise_nav_commands_wave165_ok: bool,
    /// Wave 165 ControlBar.wnd materialise live residual honesty.
    pub control_bar_materialise_live_wave165_ok: bool,
    /// Wave 166 Skirmish options WND method names residual pack honesty.
    pub skirmish_options_wnd_method_names_wave166_ok: bool,
    /// Wave 166 Skirmish options WND nav/commands residual pack honesty.
    pub skirmish_options_wnd_nav_commands_wave166_ok: bool,
    /// Wave 166 Skirmish options WND live residual honesty.
    pub skirmish_options_wnd_live_wave166_ok: bool,
    /// Wave 167 NewGame stream method names residual pack honesty.
    pub new_game_stream_method_names_wave167_ok: bool,
    /// Wave 167 NewGame stream nav/commands residual pack honesty.
    pub new_game_stream_nav_commands_wave167_ok: bool,
    /// Wave 167 NewGame stream live residual honesty.
    pub new_game_stream_live_wave167_ok: bool,
    /// Wave 168 W3DMainMenuInit method names residual pack honesty.
    pub w3d_main_menu_init_method_names_wave168_ok: bool,
    /// Wave 168 W3DMainMenuInit nav/commands residual pack honesty.
    pub w3d_main_menu_init_nav_commands_wave168_ok: bool,
    /// Wave 168 W3DMainMenuInit live residual honesty.
    pub w3d_main_menu_init_live_wave168_ok: bool,
    /// Wave 169 start_game Loading method names residual pack honesty.
    pub start_game_loading_method_names_wave169_ok: bool,
    /// Wave 169 start_game Loading nav/commands residual pack honesty.
    pub start_game_loading_nav_commands_wave169_ok: bool,
    /// Wave 169 start_game Loading live residual honesty.
    pub start_game_loading_live_wave169_ok: bool,
    /// Wave 170 live map load method names residual pack honesty.
    pub live_map_load_method_names_wave170_ok: bool,
    /// Wave 170 live map load nav/commands residual pack honesty.
    pub live_map_load_nav_commands_wave170_ok: bool,
    /// Wave 170 live map load residual honesty.
    pub live_map_load_live_wave170_ok: bool,
    /// Wave 171 live presentation seed method names residual pack honesty.
    pub live_presentation_seed_method_names_wave171_ok: bool,
    /// Wave 171 live presentation seed nav/commands residual pack honesty.
    pub live_presentation_seed_nav_commands_wave171_ok: bool,
    /// Wave 171 live presentation seed residual honesty.
    pub live_presentation_seed_live_wave171_ok: bool,
    /// Wave 172 live GameWorld shadow overlay method names residual pack honesty.
    pub live_gameworld_shadow_overlay_method_names_wave172_ok: bool,
    /// Wave 172 live GameWorld shadow overlay nav/commands residual pack honesty.
    pub live_gameworld_shadow_overlay_nav_commands_wave172_ok: bool,
    /// Wave 172 live GameWorld shadow overlay residual honesty.
    pub live_gameworld_shadow_overlay_live_wave172_ok: bool,
    /// Wave 173 single-authority combat honesty method names residual pack.
    pub single_authority_combat_method_names_wave173_ok: bool,
    /// Wave 173 single-authority combat honesty nav/commands residual pack.
    pub single_authority_combat_nav_commands_wave173_ok: bool,
    /// Wave 173 single-authority combat honesty live residual.
    pub single_authority_combat_live_wave173_ok: bool,
    /// Wave 174 presentation/GameClient boundary method names residual pack.
    pub presentation_client_boundary_method_names_wave174_ok: bool,
    /// Wave 174 presentation/GameClient boundary nav/commands residual pack.
    pub presentation_client_boundary_nav_commands_wave174_ok: bool,
    /// Wave 174 presentation/GameClient boundary live residual.
    pub presentation_client_boundary_live_wave174_ok: bool,
    /// Wave 175 golden map-host victory method names residual pack.
    pub golden_map_host_victory_method_names_wave175_ok: bool,
    /// Wave 175 golden map-host victory nav/commands residual pack.
    pub golden_map_host_victory_nav_commands_wave175_ok: bool,
    /// Wave 175 golden map-host victory live residual.
    pub golden_map_host_victory_live_wave175_ok: bool,
    /// Wave 176 executable InGame presentation boundary method names residual pack.
    pub executable_presentation_boundary_method_names_wave176_ok: bool,
    /// Wave 176 executable InGame presentation boundary nav/commands residual pack.
    pub executable_presentation_boundary_nav_commands_wave176_ok: bool,
    /// Wave 176 executable InGame presentation boundary live residual.
    pub executable_presentation_boundary_live_wave176_ok: bool,
    /// Wave 177 GameWorld production authority method names residual pack.
    pub gameworld_production_authority_method_names_wave177_ok: bool,
    /// Wave 177 GameWorld production authority nav/commands residual pack.
    pub gameworld_production_authority_nav_commands_wave177_ok: bool,
    /// Wave 177 GameWorld production authority live residual.
    pub gameworld_production_authority_live_wave177_ok: bool,
    /// Wave 178 GameWorld sole-tick coupling method names residual pack.
    pub gameworld_sole_tick_coupling_method_names_wave178_ok: bool,
    /// Wave 178 GameWorld sole-tick coupling nav/commands residual pack.
    pub gameworld_sole_tick_coupling_nav_commands_wave178_ok: bool,
    /// Wave 178 GameWorld sole-tick coupling live residual.
    pub gameworld_sole_tick_coupling_live_wave178_ok: bool,
    /// Gate defaulted GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY on.
    pub movement_authority_env_ok: bool,
    /// Wave 179 GameWorld authority matrix method names residual pack.
    pub gameworld_authority_matrix_method_names_wave179_ok: bool,
    /// Wave 179 GameWorld authority matrix nav/commands residual pack.
    pub gameworld_authority_matrix_nav_commands_wave179_ok: bool,
    /// Wave 179 GameWorld authority matrix live residual.
    pub gameworld_authority_matrix_live_wave179_ok: bool,
    /// AI attack / fire spawn / construction authorities default-on.
    pub ai_fire_construction_authority_env_ok: bool,
    /// Wave 180 live GameWorld production writeback method names residual pack.
    pub live_gameworld_production_writeback_method_names_wave180_ok: bool,
    /// Wave 180 live GameWorld production writeback nav/commands residual pack.
    pub live_gameworld_production_writeback_nav_commands_wave180_ok: bool,
    /// Wave 180 live GameWorld production writeback live residual.
    pub live_gameworld_production_writeback_live_wave180_ok: bool,
    /// Wave 181 live GameWorld construction writeback method names residual pack.
    pub live_gameworld_construction_writeback_method_names_wave181_ok: bool,
    /// Wave 181 live GameWorld construction writeback nav/commands residual pack.
    pub live_gameworld_construction_writeback_nav_commands_wave181_ok: bool,
    /// Wave 181 live GameWorld construction writeback live residual.
    pub live_gameworld_construction_writeback_live_wave181_ok: bool,
    /// Wave 182 live GameWorld damage channel method names residual pack.
    pub live_gameworld_damage_channel_method_names_wave182_ok: bool,
    /// Wave 182 live GameWorld damage channel nav/commands residual pack.
    pub live_gameworld_damage_channel_nav_commands_wave182_ok: bool,
    /// Wave 182 live GameWorld damage channel live residual.
    pub live_gameworld_damage_channel_live_wave182_ok: bool,
    /// Wave 183 live GameWorld economy/movement method names residual pack.
    pub live_gameworld_economy_movement_method_names_wave183_ok: bool,
    /// Wave 183 live GameWorld economy/movement nav/commands residual pack.
    pub live_gameworld_economy_movement_nav_commands_wave183_ok: bool,
    /// Wave 183 live GameWorld economy/movement live residual.
    pub live_gameworld_economy_movement_live_wave183_ok: bool,
    /// Wave 184 live GameWorld projectile/AI method names residual pack.
    pub live_gameworld_projectile_ai_method_names_wave184_ok: bool,
    /// Wave 184 live GameWorld projectile/AI nav/commands residual pack.
    pub live_gameworld_projectile_ai_nav_commands_wave184_ok: bool,
    /// Wave 184 live GameWorld projectile/AI live residual.
    pub live_gameworld_projectile_ai_live_wave184_ok: bool,
    /// Wave 185 live GameWorld fire/special-power method names residual pack.
    pub live_gameworld_fire_special_power_method_names_wave185_ok: bool,
    /// Wave 185 live GameWorld fire/special-power nav/commands residual pack.
    pub live_gameworld_fire_special_power_nav_commands_wave185_ok: bool,
    /// Wave 185 live GameWorld fire/special-power live residual.
    pub live_gameworld_fire_special_power_live_wave185_ok: bool,
    /// Wave 186 live GameWorld presentation view method names residual pack.
    pub live_gameworld_presentation_view_method_names_wave186_ok: bool,
    /// Wave 186 live GameWorld presentation view nav/commands residual pack.
    pub live_gameworld_presentation_view_nav_commands_wave186_ok: bool,
    /// Wave 186 live GameWorld presentation view live residual.
    pub live_gameworld_presentation_view_live_wave186_ok: bool,
    /// Wave 187 live PresentationFrame←GameWorld overlay method names residual pack.
    pub live_presentation_gameworld_overlay_method_names_wave187_ok: bool,
    /// Wave 187 live PresentationFrame←GameWorld overlay nav/commands residual pack.
    pub live_presentation_gameworld_overlay_nav_commands_wave187_ok: bool,
    /// Wave 187 live PresentationFrame←GameWorld overlay live residual.
    pub live_presentation_gameworld_overlay_live_wave187_ok: bool,
    /// Wave 188 executable GameWorld presentation method names residual pack.
    pub executable_gameworld_presentation_method_names_wave188_ok: bool,
    /// Wave 188 executable GameWorld presentation nav/commands residual pack.
    pub executable_gameworld_presentation_nav_commands_wave188_ok: bool,
    /// Wave 188 executable GameWorld presentation live residual.
    pub executable_gameworld_presentation_live_wave188_ok: bool,
    /// Wave 189 live presentation overlay deepen method names residual pack.
    pub live_presentation_overlay_deepen_method_names_wave189_ok: bool,
    /// Wave 189 live presentation overlay deepen nav/commands residual pack.
    pub live_presentation_overlay_deepen_nav_commands_wave189_ok: bool,
    /// Wave 189 live presentation overlay deepen live residual.
    pub live_presentation_overlay_deepen_live_wave189_ok: bool,
    /// Wave 190 live presentation overlay stamp method names residual pack.
    pub live_presentation_overlay_stamp_method_names_wave190_ok: bool,
    /// Wave 190 live presentation overlay stamp nav/commands residual pack.
    pub live_presentation_overlay_stamp_nav_commands_wave190_ok: bool,
    /// Wave 190 live presentation overlay stamp live residual.
    pub live_presentation_overlay_stamp_live_wave190_ok: bool,
    /// Wave 191 live GameWorld entity view deepen method names residual pack.
    pub live_gameworld_entity_view_deepen_method_names_wave191_ok: bool,
    /// Wave 191 live GameWorld entity view deepen nav/commands residual pack.
    pub live_gameworld_entity_view_deepen_nav_commands_wave191_ok: bool,
    /// Wave 191 live GameWorld entity view deepen live residual.
    pub live_gameworld_entity_view_deepen_live_wave191_ok: bool,
    /// Wave 192 live presentation append-missing method names residual pack.
    pub live_presentation_append_missing_method_names_wave192_ok: bool,
    /// Wave 192 live presentation append-missing nav/commands residual pack.
    pub live_presentation_append_missing_nav_commands_wave192_ok: bool,
    /// Wave 192 live presentation append-missing live residual.
    pub live_presentation_append_missing_live_wave192_ok: bool,
    /// Wave 193 live presentation build-from-gameworld method names residual pack.
    pub live_presentation_build_from_gameworld_method_names_wave193_ok: bool,
    /// Wave 193 live presentation build-from-gameworld nav/commands residual pack.
    pub live_presentation_build_from_gameworld_nav_commands_wave193_ok: bool,
    /// Wave 193 live presentation build-from-gameworld live residual.
    pub live_presentation_build_from_gameworld_live_wave193_ok: bool,
    /// Wave 194 live presentation from-gameworld default method names residual pack.
    pub live_presentation_from_gameworld_default_method_names_wave194_ok: bool,
    /// Wave 194 live presentation from-gameworld default nav/commands residual pack.
    pub live_presentation_from_gameworld_default_nav_commands_wave194_ok: bool,
    /// Wave 194 live presentation from-gameworld default live residual.
    pub live_presentation_from_gameworld_default_live_wave194_ok: bool,
    /// Wave 195 live presentation build-for-engine method names residual pack.
    pub live_presentation_build_for_engine_method_names_wave195_ok: bool,
    /// Wave 195 live presentation build-for-engine nav/commands residual pack.
    pub live_presentation_build_for_engine_nav_commands_wave195_ok: bool,
    /// Wave 195 live presentation build-for-engine live residual.
    pub live_presentation_build_for_engine_live_wave195_ok: bool,
    /// Wave 196 live presentation rebuilt vertical gate method names residual pack.
    pub live_presentation_rebuilt_vertical_gate_method_names_wave196_ok: bool,
    /// Wave 196 live presentation rebuilt vertical gate nav/commands residual pack.
    pub live_presentation_rebuilt_vertical_gate_nav_commands_wave196_ok: bool,
    /// Wave 196 live presentation rebuilt vertical gate live residual.
    pub live_presentation_rebuilt_vertical_gate_live_wave196_ok: bool,
    /// Wave 197 live command attack-log method names residual pack.
    pub live_command_attack_log_method_names_wave197_ok: bool,
    /// Wave 197 live command attack-log nav/commands residual pack.
    pub live_command_attack_log_nav_commands_wave197_ok: bool,
    /// Wave 197 live command attack-log live residual.
    pub live_command_attack_log_live_wave197_ok: bool,
    /// Wave 198 live command guard-log method names residual pack.
    pub live_command_guard_log_method_names_wave198_ok: bool,
    /// Wave 198 live command guard-log nav/commands residual pack.
    pub live_command_guard_log_nav_commands_wave198_ok: bool,
    /// Wave 198 live command guard-log live residual.
    pub live_command_guard_log_live_wave198_ok: bool,
    /// Wave 199 live command production/construction log method names residual pack.
    pub live_command_production_construction_log_method_names_wave199_ok: bool,
    /// Wave 199 live command production/construction log nav/commands residual pack.
    pub live_command_production_construction_log_nav_commands_wave199_ok: bool,
    /// Wave 199 live command production/construction log live residual.
    pub live_command_production_construction_log_live_wave199_ok: bool,
    /// Wave 200 live command rally-log method names residual pack.
    pub live_command_rally_log_method_names_wave200_ok: bool,
    /// Wave 200 live command rally-log nav/commands residual pack.
    pub live_command_rally_log_nav_commands_wave200_ok: bool,
    /// Wave 200 live command rally-log live residual.
    pub live_command_rally_log_live_wave200_ok: bool,
    /// Wave 201 live evacuate contain-log method names residual pack.
    pub live_evacuate_contain_log_method_names_wave201_ok: bool,
    /// Wave 201 live evacuate contain-log nav/commands residual pack.
    pub live_evacuate_contain_log_nav_commands_wave201_ok: bool,
    /// Wave 201 live evacuate contain-log live residual.
    pub live_evacuate_contain_log_live_wave201_ok: bool,
    /// Wave 202 live cheer/science log method names residual pack.
    pub live_command_cheer_science_log_method_names_wave202_ok: bool,
    /// Wave 202 live cheer/science log nav/commands residual pack.
    pub live_command_cheer_science_log_nav_commands_wave202_ok: bool,
    /// Wave 202 live cheer/science log live residual.
    pub live_command_cheer_science_log_live_wave202_ok: bool,
    /// Wave 203 live deploy status-log method names residual pack.
    pub live_command_deploy_status_log_method_names_wave203_ok: bool,
    /// Wave 203 live deploy status-log nav/commands residual pack.
    pub live_command_deploy_status_log_nav_commands_wave203_ok: bool,
    /// Wave 203 live deploy status-log live residual.
    pub live_command_deploy_status_log_live_wave203_ok: bool,
    /// Wave 204 live formation log method names residual pack.
    pub live_command_formation_log_method_names_wave204_ok: bool,
    /// Wave 204 live formation log nav/commands residual pack.
    pub live_command_formation_log_nav_commands_wave204_ok: bool,
    /// Wave 204 live formation log live residual.
    pub live_command_formation_log_live_wave204_ok: bool,
    /// Wave 205 live order-target log method names residual pack.
    pub live_command_order_target_log_method_names_wave205_ok: bool,
    /// Wave 205 live order-target log nav/commands residual pack.
    pub live_command_order_target_log_nav_commands_wave205_ok: bool,
    /// Wave 205 live order-target log live residual.
    pub live_command_order_target_log_live_wave205_ok: bool,
    /// Wave 206 live selection log method names residual pack.
    pub live_command_selection_log_method_names_wave206_ok: bool,
    /// Wave 206 live selection log nav/commands residual pack.
    pub live_command_selection_log_nav_commands_wave206_ok: bool,
    /// Wave 206 live selection log live residual.
    pub live_command_selection_log_live_wave206_ok: bool,
    /// Wave 207 live non-attack order-target method names residual pack.
    pub live_command_non_attack_order_target_method_names_wave207_ok: bool,
    /// Wave 207 live non-attack order-target nav/commands residual pack.
    pub live_command_non_attack_order_target_nav_commands_wave207_ok: bool,
    /// Wave 207 live non-attack order-target live residual.
    pub live_command_non_attack_order_target_live_wave207_ok: bool,
    /// Wave 208 live golden mop-up honesty method names residual pack.
    pub live_golden_mopup_honesty_method_names_wave208_ok: bool,
    /// Wave 208 live golden mop-up honesty nav/commands residual pack.
    pub live_golden_mopup_honesty_nav_commands_wave208_ok: bool,
    /// Wave 208 live golden mop-up honesty live residual.
    pub live_golden_mopup_honesty_live_wave208_ok: bool,
    /// Wave 209 live OS-input command path method names residual pack.
    pub live_os_input_command_path_method_names_wave209_ok: bool,
    /// Wave 209 live OS-input command path nav/commands residual pack.
    pub live_os_input_command_path_nav_commands_wave209_ok: bool,
    /// Wave 209 live OS-input command path live residual.
    pub live_os_input_command_path_live_wave209_ok: bool,
    /// Wave 210 live beacon note method names residual pack.
    pub live_command_beacon_note_method_names_wave210_ok: bool,
    /// Wave 210 live beacon note nav/commands residual pack.
    pub live_command_beacon_note_nav_commands_wave210_ok: bool,
    /// Wave 210 live beacon note live residual.
    pub live_command_beacon_note_live_wave210_ok: bool,
    /// Wave 211 live host beacon presentation method names residual pack.
    pub live_host_beacon_presentation_method_names_wave211_ok: bool,
    /// Wave 211 live host beacon presentation nav/commands residual pack.
    pub live_host_beacon_presentation_nav_commands_wave211_ok: bool,
    /// Wave 211 live host beacon presentation live residual.
    pub live_host_beacon_presentation_live_wave211_ok: bool,
    /// Wave 212 live sell deselect log method names residual pack.
    pub live_command_sell_deselect_log_method_names_wave212_ok: bool,
    /// Wave 212 live sell deselect log nav/commands residual pack.
    pub live_command_sell_deselect_log_nav_commands_wave212_ok: bool,
    /// Wave 212 live sell deselect log live residual.
    pub live_command_sell_deselect_log_live_wave212_ok: bool,
    /// Wave 213 live presentation FOW-only method names residual pack.
    pub live_presentation_fow_only_method_names_wave213_ok: bool,
    /// Wave 213 live presentation FOW-only nav/commands residual pack.
    pub live_presentation_fow_only_nav_commands_wave213_ok: bool,
    /// Wave 213 live presentation FOW-only live residual.
    pub live_presentation_fow_only_live_wave213_ok: bool,
    /// Wave 214 live UI producer presentation-only method names residual pack.
    pub live_ui_producer_presentation_only_method_names_wave214_ok: bool,
    /// Wave 214 live UI producer presentation-only nav/commands residual pack.
    pub live_ui_producer_presentation_only_nav_commands_wave214_ok: bool,
    /// Wave 214 live UI producer presentation-only live residual.
    pub live_ui_producer_presentation_only_live_wave214_ok: bool,
    /// Wave 215 live UI helpers presentation-only method names residual pack.
    pub live_ui_helpers_presentation_only_method_names_wave215_ok: bool,
    /// Wave 215 live UI helpers presentation-only nav/commands residual pack.
    pub live_ui_helpers_presentation_only_nav_commands_wave215_ok: bool,
    /// Wave 215 live UI helpers presentation-only live residual.
    pub live_ui_helpers_presentation_only_live_wave215_ok: bool,
    /// Wave 216 control-group/camera presentation-only method names residual pack.
    pub live_control_group_camera_presentation_only_method_names_wave216_ok: bool,
    /// Wave 216 control-group/camera presentation-only nav/commands residual pack.
    pub live_control_group_camera_presentation_only_nav_commands_wave216_ok: bool,
    /// Wave 216 control-group/camera presentation-only live residual.
    pub live_control_group_camera_presentation_only_live_wave216_ok: bool,
    /// Wave 217 cmd-filter/env presentation-only method names residual pack.
    pub live_cmd_filter_env_presentation_only_method_names_wave217_ok: bool,
    /// Wave 217 cmd-filter/env presentation-only nav/commands residual pack.
    pub live_cmd_filter_env_presentation_only_nav_commands_wave217_ok: bool,
    /// Wave 217 cmd-filter/env presentation-only live residual.
    pub live_cmd_filter_env_presentation_only_live_wave217_ok: bool,
    /// Wave 218 selection-commands presentation-only method names residual pack.
    pub live_selection_commands_presentation_only_method_names_wave218_ok: bool,
    /// Wave 218 selection-commands presentation-only nav/commands residual pack.
    pub live_selection_commands_presentation_only_nav_commands_wave218_ok: bool,
    /// Wave 218 selection-commands presentation-only live residual.
    pub live_selection_commands_presentation_only_live_wave218_ok: bool,
    /// Wave 219 UI-command selection presentation-only method names residual pack.
    pub live_ui_command_selection_presentation_only_method_names_wave219_ok: bool,
    /// Wave 219 UI-command selection presentation-only nav/commands residual pack.
    pub live_ui_command_selection_presentation_only_nav_commands_wave219_ok: bool,
    /// Wave 219 UI-command selection presentation-only live residual.
    pub live_ui_command_selection_presentation_only_live_wave219_ok: bool,
    /// Wave 220 local-team presentation-only method names residual pack.
    pub live_local_team_presentation_only_method_names_wave220_ok: bool,
    /// Wave 220 local-team presentation-only nav/commands residual pack.
    pub live_local_team_presentation_only_nav_commands_wave220_ok: bool,
    /// Wave 220 local-team presentation-only live residual.
    pub live_local_team_presentation_only_live_wave220_ok: bool,
    /// Wave 221 hotkey/move/attack selection presentation-only method names residual pack.
    pub live_hotkey_move_attack_selection_presentation_only_method_names_wave221_ok: bool,
    /// Wave 221 hotkey/move/attack selection presentation-only nav/commands residual pack.
    pub live_hotkey_move_attack_selection_presentation_only_nav_commands_wave221_ok: bool,
    /// Wave 221 hotkey/move/attack selection presentation-only live residual.
    pub live_hotkey_move_attack_selection_presentation_only_live_wave221_ok: bool,
    /// Wave 222 pick-object presentation-only method names residual pack.
    pub live_pick_object_presentation_only_method_names_wave222_ok: bool,
    /// Wave 222 pick-object presentation-only nav/commands residual pack.
    pub live_pick_object_presentation_only_nav_commands_wave222_ok: bool,
    /// Wave 222 pick-object presentation-only live residual.
    pub live_pick_object_presentation_only_live_wave222_ok: bool,
    /// Wave 223 bootstrap-camera presentation-only method names residual pack.
    pub live_bootstrap_camera_presentation_only_method_names_wave223_ok: bool,
    /// Wave 223 bootstrap-camera presentation-only nav/commands residual pack.
    pub live_bootstrap_camera_presentation_only_nav_commands_wave223_ok: bool,
    /// Wave 223 bootstrap-camera presentation-only live residual.
    pub live_bootstrap_camera_presentation_only_live_wave223_ok: bool,
    /// Wave 224 force-complete authority API method names residual pack.
    pub live_force_complete_authority_api_method_names_wave224_ok: bool,
    /// Wave 224 force-complete authority API nav/commands residual pack.
    pub live_force_complete_authority_api_nav_commands_wave224_ok: bool,
    /// Wave 224 force-complete authority API live residual.
    pub live_force_complete_authority_api_live_wave224_ok: bool,
    /// Wave 225 path/guard authority API method names residual pack.
    pub live_path_guard_authority_api_method_names_wave225_ok: bool,
    /// Wave 225 path/guard authority API nav/commands residual pack.
    pub live_path_guard_authority_api_nav_commands_wave225_ok: bool,
    /// Wave 225 path/guard authority API live residual.
    pub live_path_guard_authority_api_live_wave225_ok: bool,
    /// Wave 226 hotkey selection/camera presentation-only method names residual pack.
    pub live_hotkey_selection_camera_presentation_only_method_names_wave226_ok: bool,
    /// Wave 226 hotkey selection/camera presentation-only nav/commands residual pack.
    pub live_hotkey_selection_camera_presentation_only_nav_commands_wave226_ok: bool,
    /// Wave 226 hotkey selection/camera presentation-only live residual.
    pub live_hotkey_selection_camera_presentation_only_live_wave226_ok: bool,
    /// Wave 227 construct-spawn pose authority API method names residual pack.
    pub live_construct_spawn_pose_authority_api_method_names_wave227_ok: bool,
    /// Wave 227 construct-spawn pose authority API nav/commands residual pack.
    pub live_construct_spawn_pose_authority_api_nav_commands_wave227_ok: bool,
    /// Wave 227 construct-spawn pose authority API live residual.
    pub live_construct_spawn_pose_authority_api_live_wave227_ok: bool,
    /// Wave 228 RMB target presentation-only method names residual pack.
    pub live_rmb_target_presentation_only_method_names_wave228_ok: bool,
    /// Wave 228 RMB target presentation-only nav/commands residual pack.
    pub live_rmb_target_presentation_only_nav_commands_wave228_ok: bool,
    /// Wave 228 RMB target presentation-only live residual.
    pub live_rmb_target_presentation_only_live_wave228_ok: bool,
    /// Wave 229 RMB selected presentation-only method names residual pack.
    pub live_rmb_selected_presentation_only_method_names_wave229_ok: bool,
    /// Wave 229 RMB selected presentation-only nav/commands residual pack.
    pub live_rmb_selected_presentation_only_nav_commands_wave229_ok: bool,
    /// Wave 229 RMB selected presentation-only live residual.
    pub live_rmb_selected_presentation_only_live_wave229_ok: bool,
    /// Wave 230 command unit authority API method names residual pack.
    pub live_command_unit_authority_api_method_names_wave230_ok: bool,
    /// Wave 230 command unit authority API nav/commands residual pack.
    pub live_command_unit_authority_api_nav_commands_wave230_ok: bool,
    /// Wave 230 command unit authority API live residual.
    pub live_command_unit_authority_api_live_wave230_ok: bool,
    /// Wave 231 command unit more-authority API method names residual pack.
    pub live_command_unit_more_authority_api_method_names_wave231_ok: bool,
    /// Wave 231 command unit more-authority API nav/commands residual pack.
    pub live_command_unit_more_authority_api_nav_commands_wave231_ok: bool,
    /// Wave 231 command unit more-authority API live residual.
    pub live_command_unit_more_authority_api_live_wave231_ok: bool,
    /// Wave 232 command executor authority API method names residual pack.
    pub live_command_executor_authority_api_method_names_wave232_ok: bool,
    /// Wave 232 command executor authority API nav/commands residual pack.
    pub live_command_executor_authority_api_nav_commands_wave232_ok: bool,
    /// Wave 232 command executor authority API live residual.
    pub live_command_executor_authority_api_live_wave232_ok: bool,
    /// Wave 233 command executor more-authority API method names residual pack.
    pub live_command_executor_more_authority_api_method_names_wave233_ok: bool,
    /// Wave 233 command executor more-authority API nav/commands residual pack.
    pub live_command_executor_more_authority_api_nav_commands_wave233_ok: bool,
    /// Wave 233 command executor more-authority API live residual.
    pub live_command_executor_more_authority_api_live_wave233_ok: bool,
    /// Wave 234 engine presentation player UI method names residual pack.
    pub live_engine_presentation_player_ui_method_names_wave234_ok: bool,
    /// Wave 234 engine presentation player UI nav/commands residual pack.
    pub live_engine_presentation_player_ui_nav_commands_wave234_ok: bool,
    /// Wave 234 engine presentation player UI live residual.
    pub live_engine_presentation_player_ui_live_wave234_ok: bool,
    /// Wave 235 RMB presentation full-classify method names residual pack.
    pub live_rmb_presentation_full_classify_method_names_wave235_ok: bool,
    /// Wave 235 RMB presentation full-classify nav/commands residual pack.
    pub live_rmb_presentation_full_classify_nav_commands_wave235_ok: bool,
    /// Wave 235 RMB presentation full-classify live residual.
    pub live_rmb_presentation_full_classify_live_wave235_ok: bool,
    /// Wave 236 mouse input presentation-only method names residual pack.
    pub live_mouse_input_presentation_only_method_names_wave236_ok: bool,
    /// Wave 236 mouse input presentation-only nav/commands residual pack.
    pub live_mouse_input_presentation_only_nav_commands_wave236_ok: bool,
    /// Wave 236 mouse input presentation-only live residual.
    pub live_mouse_input_presentation_only_live_wave236_ok: bool,
    /// Wave 237 engine player UI boot peel method names residual pack.
    pub live_engine_player_ui_boot_peel_method_names_wave237_ok: bool,
    /// Wave 237 engine player UI boot peel nav/commands residual pack.
    pub live_engine_player_ui_boot_peel_nav_commands_wave237_ok: bool,
    /// Wave 237 engine player UI boot peel live residual.
    pub live_engine_player_ui_boot_peel_live_wave237_ok: bool,
    /// Wave 238 player probe API method names residual pack.
    pub live_player_probe_api_method_names_wave238_ok: bool,
    /// Wave 238 player probe API nav/commands residual pack.
    pub live_player_probe_api_nav_commands_wave238_ok: bool,
    /// Wave 238 player probe API live residual.
    pub live_player_probe_api_live_wave238_ok: bool,
    /// Wave 239 player team probe method names residual pack.
    pub live_player_team_probe_method_names_wave239_ok: bool,
    /// Wave 239 player team probe nav/commands residual pack.
    pub live_player_team_probe_nav_commands_wave239_ok: bool,
    /// Wave 239 player team probe live residual.
    pub live_player_team_probe_live_wave239_ok: bool,
    /// Wave 240 player field probe method names residual pack.
    pub live_player_field_probe_method_names_wave240_ok: bool,
    /// Wave 240 player field probe nav/commands residual pack.
    pub live_player_field_probe_nav_commands_wave240_ok: bool,
    /// Wave 240 player field probe live residual.
    pub live_player_field_probe_live_wave240_ok: bool,
    /// Wave 241 camera height probe method names residual pack.
    pub live_camera_height_probe_method_names_wave241_ok: bool,
    /// Wave 241 camera height probe nav/commands residual pack.
    pub live_camera_height_probe_nav_commands_wave241_ok: bool,
    /// Wave 241 camera height probe live residual.
    pub live_camera_height_probe_live_wave241_ok: bool,
    /// Wave 242 command player probe method names residual pack.
    pub live_command_player_probe_method_names_wave242_ok: bool,
    /// Wave 242 command player probe nav/commands residual pack.
    pub live_command_player_probe_nav_commands_wave242_ok: bool,
    /// Wave 242 command player probe live residual.
    pub live_command_player_probe_live_wave242_ok: bool,
    /// Wave 243 construct economy probe method names residual pack.
    pub live_construct_economy_probe_method_names_wave243_ok: bool,
    /// Wave 243 construct economy probe nav/commands residual pack.
    pub live_construct_economy_probe_nav_commands_wave243_ok: bool,
    /// Wave 243 construct economy probe live residual.
    pub live_construct_economy_probe_live_wave243_ok: bool,
    /// Wave 244 command unit probe method names residual pack.
    pub live_command_unit_probe_method_names_wave244_ok: bool,
    /// Wave 244 command unit probe nav/commands residual pack.
    pub live_command_unit_probe_nav_commands_wave244_ok: bool,
    /// Wave 244 command unit probe live residual.
    pub live_command_unit_probe_live_wave244_ok: bool,
    /// Wave 245 selection query probe method names residual pack.
    pub live_selection_query_probe_method_names_wave245_ok: bool,
    /// Wave 245 selection query probe nav/commands residual pack.
    pub live_selection_query_probe_nav_commands_wave245_ok: bool,
    /// Wave 245 selection query probe live residual.
    pub live_selection_query_probe_live_wave245_ok: bool,
    /// Wave 246 world pick probe method names residual pack.
    pub live_world_pick_probe_method_names_wave246_ok: bool,
    /// Wave 246 world pick probe nav/commands residual pack.
    pub live_world_pick_probe_nav_commands_wave246_ok: bool,
    /// Wave 246 world pick probe live residual.
    pub live_world_pick_probe_live_wave246_ok: bool,
    /// Wave 247 object registry empty fastpath method names residual pack.
    pub live_object_registry_empty_fastpath_method_names_wave247_ok: bool,
    /// Wave 247 object registry empty fastpath nav/commands residual pack.
    pub live_object_registry_empty_fastpath_nav_commands_wave247_ok: bool,
    /// Wave 247 object registry empty fastpath live residual.
    pub live_object_registry_empty_fastpath_live_wave247_ok: bool,
    /// Wave 248 legacy object registry fastpath method names residual pack.
    pub live_legacy_object_registry_fastpath_method_names_wave248_ok: bool,
    /// Wave 248 legacy object registry fastpath nav/commands residual pack.
    pub live_legacy_object_registry_fastpath_nav_commands_wave248_ok: bool,
    /// Wave 248 legacy object registry fastpath live residual.
    pub live_legacy_object_registry_fastpath_live_wave248_ok: bool,
    /// Wave 249 client dual-world empty-gate method names residual pack.
    pub live_client_dual_world_empty_gate_method_names_wave249_ok: bool,
    /// Wave 249 client dual-world empty-gate nav/commands residual pack.
    pub live_client_dual_world_empty_gate_nav_commands_wave249_ok: bool,
    /// Wave 249 client dual-world empty-gate live residual.
    pub live_client_dual_world_empty_gate_live_wave249_ok: bool,
    /// Wave 250 presentation time-frozen probe method names residual pack.
    pub live_presentation_time_frozen_probe_method_names_wave250_ok: bool,
    /// Wave 250 presentation time-frozen probe nav/commands residual pack.
    pub live_presentation_time_frozen_probe_nav_commands_wave250_ok: bool,
    /// Wave 250 presentation time-frozen probe live residual.
    pub live_presentation_time_frozen_probe_live_wave250_ok: bool,
    /// Wave 251 presentation visual-speed probe method names residual pack.
    pub live_presentation_visual_speed_probe_method_names_wave251_ok: bool,
    /// Wave 251 presentation visual-speed probe nav/commands residual pack.
    pub live_presentation_visual_speed_probe_nav_commands_wave251_ok: bool,
    /// Wave 251 presentation visual-speed probe live residual.
    pub live_presentation_visual_speed_probe_live_wave251_ok: bool,
    /// Wave 252 presentation script-camera probe method names residual pack.
    pub live_presentation_script_camera_probe_method_names_wave252_ok: bool,
    /// Wave 252 presentation script-camera probe nav/commands residual pack.
    pub live_presentation_script_camera_probe_nav_commands_wave252_ok: bool,
    /// Wave 252 presentation script-camera probe live residual.
    pub live_presentation_script_camera_probe_live_wave252_ok: bool,
    /// Wave 253 AIGroup dual-world empty-gate method names residual pack.
    pub live_ai_group_dual_world_empty_gate_method_names_wave253_ok: bool,
    /// Wave 253 AIGroup dual-world empty-gate nav/commands residual pack.
    pub live_ai_group_dual_world_empty_gate_nav_commands_wave253_ok: bool,
    /// Wave 253 AIGroup dual-world empty-gate live residual.
    pub live_ai_group_dual_world_empty_gate_live_wave253_ok: bool,
    /// Wave 254 AIStates dual-world empty-gate method names residual pack.
    pub live_ai_states_dual_world_empty_gate_method_names_wave254_ok: bool,
    /// Wave 254 AIStates dual-world empty-gate nav/commands residual pack.
    pub live_ai_states_dual_world_empty_gate_nav_commands_wave254_ok: bool,
    /// Wave 254 AIStates dual-world empty-gate live residual.
    pub live_ai_states_dual_world_empty_gate_live_wave254_ok: bool,
    /// Wave 255 AIPlayer dual-world empty-gate method names residual pack.
    pub live_ai_player_dual_world_empty_gate_method_names_wave255_ok: bool,
    /// Wave 255 AIPlayer dual-world empty-gate nav/commands residual pack.
    pub live_ai_player_dual_world_empty_gate_nav_commands_wave255_ok: bool,
    /// Wave 255 AIPlayer dual-world empty-gate live residual.
    pub live_ai_player_dual_world_empty_gate_live_wave255_ok: bool,
    /// Wave 256 Team dual-world empty-gate method names residual pack.
    pub live_team_dual_world_empty_gate_method_names_wave256_ok: bool,
    /// Wave 256 Team dual-world empty-gate nav/commands residual pack.
    pub live_team_dual_world_empty_gate_nav_commands_wave256_ok: bool,
    /// Wave 256 Team dual-world empty-gate live residual.
    pub live_team_dual_world_empty_gate_live_wave256_ok: bool,
    /// Wave 257 legacy AI states dual-world empty-gate method names residual pack.
    pub live_ai_legacy_states_dual_world_empty_gate_method_names_wave257_ok: bool,
    /// Wave 257 legacy AI states dual-world empty-gate nav/commands residual pack.
    pub live_ai_legacy_states_dual_world_empty_gate_nav_commands_wave257_ok: bool,
    /// Wave 257 legacy AI states dual-world empty-gate live residual.
    pub live_ai_legacy_states_dual_world_empty_gate_live_wave257_ok: bool,
    /// Wave 258 Unit dual-world empty-gate method names residual pack.
    pub live_unit_dual_world_empty_gate_method_names_wave258_ok: bool,
    /// Wave 258 Unit dual-world empty-gate nav/commands residual pack.
    pub live_unit_dual_world_empty_gate_nav_commands_wave258_ok: bool,
    /// Wave 258 Unit dual-world empty-gate live residual.
    pub live_unit_dual_world_empty_gate_live_wave258_ok: bool,
    /// Wave 259 Stealth dual-world empty-gate method names residual pack.
    pub live_stealth_dual_world_empty_gate_method_names_wave259_ok: bool,
    /// Wave 259 Stealth dual-world empty-gate nav/commands residual pack.
    pub live_stealth_dual_world_empty_gate_nav_commands_wave259_ok: bool,
    /// Wave 259 Stealth dual-world empty-gate live residual.
    pub live_stealth_dual_world_empty_gate_live_wave259_ok: bool,
    /// Wave 260 Garrison dual-world empty-gate method names residual pack.
    pub live_garrison_dual_world_empty_gate_method_names_wave260_ok: bool,
    /// Wave 260 Garrison dual-world empty-gate nav/commands residual pack.
    pub live_garrison_dual_world_empty_gate_nav_commands_wave260_ok: bool,
    /// Wave 260 Garrison dual-world empty-gate live residual.
    pub live_garrison_dual_world_empty_gate_live_wave260_ok: bool,
    /// Wave 261 OpenContain dual-world empty-gate method names residual pack.
    pub live_open_contain_dual_world_empty_gate_method_names_wave261_ok: bool,
    /// Wave 261 OpenContain dual-world empty-gate nav/commands residual pack.
    pub live_open_contain_dual_world_empty_gate_nav_commands_wave261_ok: bool,
    /// Wave 261 OpenContain dual-world empty-gate live residual.
    pub live_open_contain_dual_world_empty_gate_live_wave261_ok: bool,
    /// Wave 262 Pathfind dual-world empty-gate method names residual pack.
    pub live_pathfind_dual_world_empty_gate_method_names_wave262_ok: bool,
    /// Wave 262 Pathfind dual-world empty-gate nav/commands residual pack.
    pub live_pathfind_dual_world_empty_gate_nav_commands_wave262_ok: bool,
    /// Wave 262 Pathfind dual-world empty-gate live residual.
    pub live_pathfind_dual_world_empty_gate_live_wave262_ok: bool,
    /// Wave 263 AI mod dual-world empty-gate method names residual pack.
    pub live_ai_mod_dual_world_empty_gate_method_names_wave263_ok: bool,
    /// Wave 263 AI mod dual-world empty-gate nav/commands residual pack.
    pub live_ai_mod_dual_world_empty_gate_nav_commands_wave263_ok: bool,
    /// Wave 263 AI mod dual-world empty-gate live residual.
    pub live_ai_mod_dual_world_empty_gate_live_wave263_ok: bool,
    /// Wave 264 Object mod dual-world empty-gate method names residual pack.
    pub live_object_mod_dual_world_empty_gate_method_names_wave264_ok: bool,
    /// Wave 264 Object mod dual-world empty-gate nav/commands residual pack.
    pub live_object_mod_dual_world_empty_gate_nav_commands_wave264_ok: bool,
    /// Wave 264 Object mod dual-world empty-gate live residual.
    pub live_object_mod_dual_world_empty_gate_live_wave264_ok: bool,
    /// Wave 265 Weapon dual-world empty-gate method names residual pack.
    pub live_weapon_dual_world_empty_gate_method_names_wave265_ok: bool,
    /// Wave 265 Weapon dual-world empty-gate nav/commands residual pack.
    pub live_weapon_dual_world_empty_gate_nav_commands_wave265_ok: bool,
    /// Wave 265 Weapon dual-world empty-gate live residual.
    pub live_weapon_dual_world_empty_gate_live_wave265_ok: bool,
    /// Wave 266 Partition filters dual-world empty-gate method names residual pack.
    pub live_partition_filters_dual_world_empty_gate_method_names_wave266_ok: bool,
    /// Wave 266 Partition filters dual-world empty-gate nav/commands residual pack.
    pub live_partition_filters_dual_world_empty_gate_nav_commands_wave266_ok: bool,
    /// Wave 266 Partition filters dual-world empty-gate live residual.
    pub live_partition_filters_dual_world_empty_gate_live_wave266_ok: bool,
    /// Wave 267 AI state machine dual-world empty-gate method names residual pack.
    pub live_ai_state_machine_dual_world_empty_gate_method_names_wave267_ok: bool,
    /// Wave 267 AI state machine dual-world empty-gate nav/commands residual pack.
    pub live_ai_state_machine_dual_world_empty_gate_nav_commands_wave267_ok: bool,
    /// Wave 267 AI state machine dual-world empty-gate live residual.
    pub live_ai_state_machine_dual_world_empty_gate_live_wave267_ok: bool,
    /// Wave 268 Player dual-world empty-gate method names residual pack.
    pub live_player_dual_world_empty_gate_method_names_wave268_ok: bool,
    /// Wave 268 Player dual-world empty-gate nav/commands residual pack.
    pub live_player_dual_world_empty_gate_nav_commands_wave268_ok: bool,
    /// Wave 268 Player dual-world empty-gate live residual.
    pub live_player_dual_world_empty_gate_live_wave268_ok: bool,
    /// Wave 269 GameClient dual-world empty-gate method names residual pack.
    pub live_game_client_dual_world_empty_gate_method_names_wave269_ok: bool,
    /// Wave 269 GameClient dual-world empty-gate nav/commands residual pack.
    pub live_game_client_dual_world_empty_gate_nav_commands_wave269_ok: bool,
    /// Wave 269 GameClient dual-world empty-gate live residual.
    pub live_game_client_dual_world_empty_gate_live_wave269_ok: bool,
    /// Wave 270 Drawable dual-world empty-gate method names residual pack.
    pub live_drawable_dual_world_empty_gate_method_names_wave270_ok: bool,
    /// Wave 270 Drawable dual-world empty-gate nav/commands residual pack.
    pub live_drawable_dual_world_empty_gate_nav_commands_wave270_ok: bool,
    /// Wave 270 Drawable dual-world empty-gate live residual.
    pub live_drawable_dual_world_empty_gate_live_wave270_ok: bool,
    /// Wave 271 script conditions dual-world empty-gate method names residual pack.
    pub live_script_conditions_dual_world_empty_gate_method_names_wave271_ok: bool,
    /// Wave 271 script conditions dual-world empty-gate nav/commands residual pack.
    pub live_script_conditions_dual_world_empty_gate_nav_commands_wave271_ok: bool,
    /// Wave 271 script conditions dual-world empty-gate live residual.
    pub live_script_conditions_dual_world_empty_gate_live_wave271_ok: bool,
    /// Wave 272 TransportContain dual-world empty-gate method names residual pack.
    pub live_transport_contain_dual_world_empty_gate_method_names_wave272_ok: bool,
    /// Wave 272 TransportContain dual-world empty-gate nav/commands residual pack.
    pub live_transport_contain_dual_world_empty_gate_nav_commands_wave272_ok: bool,
    /// Wave 272 TransportContain dual-world empty-gate live residual.
    pub live_transport_contain_dual_world_empty_gate_live_wave272_ok: bool,
    /// Wave 273 InGameUI dual-world empty-gate method names residual pack.
    pub live_ingame_ui_dual_world_empty_gate_method_names_wave273_ok: bool,
    /// Wave 273 InGameUI dual-world empty-gate nav/commands residual pack.
    pub live_ingame_ui_dual_world_empty_gate_nav_commands_wave273_ok: bool,
    /// Wave 273 InGameUI dual-world empty-gate live residual.
    pub live_ingame_ui_dual_world_empty_gate_live_wave273_ok: bool,
    /// Wave 274 HelixContain dual-world empty-gate method names residual pack.
    pub live_helix_contain_dual_world_empty_gate_method_names_wave274_ok: bool,
    /// Wave 274 HelixContain dual-world empty-gate nav/commands residual pack.
    pub live_helix_contain_dual_world_empty_gate_nav_commands_wave274_ok: bool,
    /// Wave 274 HelixContain dual-world empty-gate live residual.
    pub live_helix_contain_dual_world_empty_gate_live_wave274_ok: bool,
    /// Wave 275 CommandProcessor dual-world empty-gate method names residual pack.
    pub live_command_processor_dual_world_empty_gate_method_names_wave275_ok: bool,
    /// Wave 275 CommandProcessor dual-world empty-gate nav/commands residual pack.
    pub live_command_processor_dual_world_empty_gate_nav_commands_wave275_ok: bool,
    /// Wave 275 CommandProcessor dual-world empty-gate live residual.
    pub live_command_processor_dual_world_empty_gate_live_wave275_ok: bool,
    /// Wave 276 Turret dual-world empty-gate method names residual pack.
    pub live_turret_dual_world_empty_gate_method_names_wave276_ok: bool,
    /// Wave 276 Turret dual-world empty-gate nav/commands residual pack.
    pub live_turret_dual_world_empty_gate_nav_commands_wave276_ok: bool,
    /// Wave 276 Turret dual-world empty-gate live residual.
    pub live_turret_dual_world_empty_gate_live_wave276_ok: bool,
    /// Wave 277 RiderChangeContain dual-world empty-gate method names residual pack.
    pub live_rider_change_contain_dual_world_empty_gate_method_names_wave277_ok: bool,
    /// Wave 277 RiderChangeContain dual-world empty-gate nav/commands residual pack.
    pub live_rider_change_contain_dual_world_empty_gate_nav_commands_wave277_ok: bool,
    /// Wave 277 RiderChangeContain dual-world empty-gate live residual.
    pub live_rider_change_contain_dual_world_empty_gate_live_wave277_ok: bool,
    /// Wave 278 Selection dual-world empty-gate method names residual pack.
    pub live_selection_dual_world_empty_gate_method_names_wave278_ok: bool,
    /// Wave 278 Selection dual-world empty-gate nav/commands residual pack.
    pub live_selection_dual_world_empty_gate_nav_commands_wave278_ok: bool,
    /// Wave 278 Selection dual-world empty-gate live residual.
    pub live_selection_dual_world_empty_gate_live_wave278_ok: bool,
    /// Wave 279 CaveContain dual-world empty-gate method names residual pack.
    pub live_cave_contain_dual_world_empty_gate_method_names_wave279_ok: bool,
    /// Wave 279 CaveContain dual-world empty-gate nav/commands residual pack.
    pub live_cave_contain_dual_world_empty_gate_nav_commands_wave279_ok: bool,
    /// Wave 279 CaveContain dual-world empty-gate live residual.
    pub live_cave_contain_dual_world_empty_gate_live_wave279_ok: bool,
    /// Wave 280 TunnelContain dual-world empty-gate method names residual pack.
    pub live_tunnel_contain_dual_world_empty_gate_method_names_wave280_ok: bool,
    /// Wave 280 TunnelContain dual-world empty-gate nav/commands residual pack.
    pub live_tunnel_contain_dual_world_empty_gate_nav_commands_wave280_ok: bool,
    /// Wave 280 TunnelContain dual-world empty-gate live residual.
    pub live_tunnel_contain_dual_world_empty_gate_live_wave280_ok: bool,
    /// Wave 281 helpers dual-world empty-gate method names residual pack.
    pub live_helpers_dual_world_empty_gate_method_names_wave281_ok: bool,
    /// Wave 281 helpers dual-world empty-gate nav/commands residual pack.
    pub live_helpers_dual_world_empty_gate_nav_commands_wave281_ok: bool,
    /// Wave 281 helpers dual-world empty-gate live residual.
    pub live_helpers_dual_world_empty_gate_live_wave281_ok: bool,
    /// Wave 282 AIUpdateInterface dual-world empty-gate method names residual pack.
    pub live_ai_update_interface_dual_world_empty_gate_method_names_wave282_ok: bool,
    /// Wave 282 AIUpdateInterface dual-world empty-gate nav/commands residual pack.
    pub live_ai_update_interface_dual_world_empty_gate_nav_commands_wave282_ok: bool,
    /// Wave 282 AIUpdateInterface dual-world empty-gate live residual.
    pub live_ai_update_interface_dual_world_empty_gate_live_wave282_ok: bool,
    /// Wave 283 StealthUpdate dual-world empty-gate method names residual pack.
    pub live_stealth_update_dual_world_empty_gate_method_names_wave283_ok: bool,
    /// Wave 283 StealthUpdate dual-world empty-gate nav/commands residual pack.
    pub live_stealth_update_dual_world_empty_gate_nav_commands_wave283_ok: bool,
    /// Wave 283 StealthUpdate dual-world empty-gate live residual.
    pub live_stealth_update_dual_world_empty_gate_live_wave283_ok: bool,
    /// Wave 284 script executor dual-world empty-gate method names residual pack.
    pub live_script_executor_dual_world_empty_gate_method_names_wave284_ok: bool,
    /// Wave 284 script executor dual-world empty-gate nav/commands residual pack.
    pub live_script_executor_dual_world_empty_gate_nav_commands_wave284_ok: bool,
    /// Wave 284 script executor dual-world empty-gate live residual.
    pub live_script_executor_dual_world_empty_gate_live_wave284_ok: bool,
    /// Wave 285 AI integration dual-world empty-gate method names residual pack.
    pub live_ai_integration_dual_world_empty_gate_method_names_wave285_ok: bool,
    /// Wave 285 AI integration dual-world empty-gate nav/commands residual pack.
    pub live_ai_integration_dual_world_empty_gate_nav_commands_wave285_ok: bool,
    /// Wave 285 AI integration dual-world empty-gate live residual.
    pub live_ai_integration_dual_world_empty_gate_live_wave285_ok: bool,
    /// Wave 286 DumbProjectile dual-world empty-gate method names residual pack.
    pub live_dumb_projectile_dual_world_empty_gate_method_names_wave286_ok: bool,
    /// Wave 286 DumbProjectile dual-world empty-gate nav/commands residual pack.
    pub live_dumb_projectile_dual_world_empty_gate_nav_commands_wave286_ok: bool,
    /// Wave 286 DumbProjectile dual-world empty-gate live residual.
    pub live_dumb_projectile_dual_world_empty_gate_live_wave286_ok: bool,
    /// Wave 287 enhanced player dual-world empty-gate method names residual pack.
    pub live_enhanced_player_dual_world_empty_gate_method_names_wave287_ok: bool,
    /// Wave 287 enhanced player dual-world empty-gate nav/commands residual pack.
    pub live_enhanced_player_dual_world_empty_gate_nav_commands_wave287_ok: bool,
    /// Wave 287 enhanced player dual-world empty-gate live residual.
    pub live_enhanced_player_dual_world_empty_gate_live_wave287_ok: bool,
    /// Wave 288 hijacker update dual-world empty-gate method names residual pack.
    pub live_hijacker_update_dual_world_empty_gate_method_names_wave288_ok: bool,
    /// Wave 288 hijacker update dual-world empty-gate nav/commands residual pack.
    pub live_hijacker_update_dual_world_empty_gate_nav_commands_wave288_ok: bool,
    /// Wave 288 hijacker update dual-world empty-gate live residual.
    pub live_hijacker_update_dual_world_empty_gate_live_wave288_ok: bool,
    /// Wave 289 weapon dual-world empty-gate method names residual pack.
    pub live_weapon_impl_dual_world_empty_gate_method_names_wave289_ok: bool,
    /// Wave 289 weapon dual-world empty-gate nav/commands residual pack.
    pub live_weapon_impl_dual_world_empty_gate_nav_commands_wave289_ok: bool,
    /// Wave 289 weapon dual-world empty-gate live residual.
    pub live_weapon_impl_dual_world_empty_gate_live_wave289_ok: bool,
    /// Wave 290 async player dual-world empty-gate method names residual pack.
    pub live_async_player_dual_world_empty_gate_method_names_wave290_ok: bool,
    /// Wave 290 async player dual-world empty-gate nav/commands residual pack.
    pub live_async_player_dual_world_empty_gate_nav_commands_wave290_ok: bool,
    /// Wave 290 async player dual-world empty-gate live residual.
    pub live_async_player_dual_world_empty_gate_live_wave290_ok: bool,
    /// Wave 291 active body dual-world empty-gate method names residual pack.
    pub live_active_body_dual_world_empty_gate_method_names_wave291_ok: bool,
    /// Wave 291 active body dual-world empty-gate nav/commands residual pack.
    pub live_active_body_dual_world_empty_gate_nav_commands_wave291_ok: bool,
    /// Wave 291 active body dual-world empty-gate live residual.
    pub live_active_body_dual_world_empty_gate_live_wave291_ok: bool,
    /// Wave 292 skirmish conditions dual-world empty-gate method names residual pack.
    pub live_skirmish_conditions_dual_world_empty_gate_method_names_wave292_ok: bool,
    /// Wave 292 skirmish conditions dual-world empty-gate nav/commands residual pack.
    pub live_skirmish_conditions_dual_world_empty_gate_nav_commands_wave292_ok: bool,
    /// Wave 292 skirmish conditions dual-world empty-gate live residual.
    pub live_skirmish_conditions_dual_world_empty_gate_live_wave292_ok: bool,
    /// Wave 293 AI build list dual-world empty-gate method names residual pack.
    pub live_ai_build_list_dual_world_empty_gate_method_names_wave293_ok: bool,
    /// Wave 293 AI build list dual-world empty-gate nav/commands residual pack.
    pub live_ai_build_list_dual_world_empty_gate_nav_commands_wave293_ok: bool,
    /// Wave 293 AI build list dual-world empty-gate live residual.
    pub live_ai_build_list_dual_world_empty_gate_live_wave293_ok: bool,
    /// Wave 294 victory dual-world empty-gate method names residual pack.
    pub live_victory_dual_world_empty_gate_method_names_wave294_ok: bool,
    /// Wave 294 victory dual-world empty-gate nav/commands residual pack.
    pub live_victory_dual_world_empty_gate_nav_commands_wave294_ok: bool,
    /// Wave 294 victory dual-world empty-gate live residual.
    pub live_victory_dual_world_empty_gate_live_wave294_ok: bool,
    /// Wave 295 script actions dual-world empty-gate method names residual pack.
    pub live_script_actions_dual_world_empty_gate_method_names_wave295_ok: bool,
    /// Wave 295 script actions dual-world empty-gate nav/commands residual pack.
    pub live_script_actions_dual_world_empty_gate_nav_commands_wave295_ok: bool,
    /// Wave 295 script actions dual-world empty-gate live residual.
    pub live_script_actions_dual_world_empty_gate_live_wave295_ok: bool,
    /// Wave 296 special ability dual-world empty-gate method names residual pack.
    pub live_special_ability_dual_world_empty_gate_method_names_wave296_ok: bool,
    /// Wave 296 special ability dual-world empty-gate nav/commands residual pack.
    pub live_special_ability_dual_world_empty_gate_nav_commands_wave296_ok: bool,
    /// Wave 296 special ability dual-world empty-gate live residual.
    pub live_special_ability_dual_world_empty_gate_live_wave296_ok: bool,
    /// Wave 297 stealth detector dual-world empty-gate method names residual pack.
    pub live_stealth_detector_dual_world_empty_gate_method_names_wave297_ok: bool,
    /// Wave 297 stealth detector dual-world empty-gate nav/commands residual pack.
    pub live_stealth_detector_dual_world_empty_gate_nav_commands_wave297_ok: bool,
    /// Wave 297 stealth detector dual-world empty-gate live residual.
    pub live_stealth_detector_dual_world_empty_gate_live_wave297_ok: bool,
    /// Wave 298 supply system dual-world empty-gate method names residual pack.
    pub live_supply_system_dual_world_empty_gate_method_names_wave298_ok: bool,
    /// Wave 298 supply system dual-world empty-gate nav/commands residual pack.
    pub live_supply_system_dual_world_empty_gate_nav_commands_wave298_ok: bool,
    /// Wave 298 supply system dual-world empty-gate live residual.
    pub live_supply_system_dual_world_empty_gate_live_wave298_ok: bool,
    /// Wave 299 particle uplink dual-world empty-gate method names residual pack.
    pub live_particle_uplink_dual_world_empty_gate_method_names_wave299_ok: bool,
    /// Wave 299 particle uplink dual-world empty-gate nav/commands residual pack.
    pub live_particle_uplink_dual_world_empty_gate_nav_commands_wave299_ok: bool,
    /// Wave 299 particle uplink dual-world empty-gate live residual.
    pub live_particle_uplink_dual_world_empty_gate_live_wave299_ok: bool,
    /// Wave 300 overlord contain dual-world empty-gate method names residual pack.
    pub live_overlord_contain_dual_world_empty_gate_method_names_wave300_ok: bool,
    /// Wave 300 overlord contain dual-world empty-gate nav/commands residual pack.
    pub live_overlord_contain_dual_world_empty_gate_nav_commands_wave300_ok: bool,
    /// Wave 300 overlord contain dual-world empty-gate live residual.
    pub live_overlord_contain_dual_world_empty_gate_live_wave300_ok: bool,
    /// Wave 301 bridge behavior dual-world empty-gate method names residual pack.
    pub live_bridge_behavior_dual_world_empty_gate_method_names_wave301_ok: bool,
    /// Wave 301 bridge behavior dual-world empty-gate nav/commands residual pack.
    pub live_bridge_behavior_dual_world_empty_gate_nav_commands_wave301_ok: bool,
    /// Wave 301 bridge behavior dual-world empty-gate live residual.
    pub live_bridge_behavior_dual_world_empty_gate_live_wave301_ok: bool,
    /// Wave 302 stealth behavior dual-world empty-gate method names residual pack.
    pub live_stealth_behavior_dual_world_empty_gate_method_names_wave302_ok: bool,
    /// Wave 302 stealth behavior dual-world empty-gate nav/commands residual pack.
    pub live_stealth_behavior_dual_world_empty_gate_nav_commands_wave302_ok: bool,
    /// Wave 302 stealth behavior dual-world empty-gate live residual.
    pub live_stealth_behavior_dual_world_empty_gate_live_wave302_ok: bool,
    /// Wave 303 crate collide dual-world empty-gate method names residual pack.
    pub live_crate_collide_dual_world_empty_gate_method_names_wave303_ok: bool,
    /// Wave 303 crate collide dual-world empty-gate nav/commands residual pack.
    pub live_crate_collide_dual_world_empty_gate_nav_commands_wave303_ok: bool,
    /// Wave 303 crate collide dual-world empty-gate live residual.
    pub live_crate_collide_dual_world_empty_gate_live_wave303_ok: bool,
    /// Wave 304 object manager dual-world empty-gate method names residual pack.
    pub live_object_manager_dual_world_empty_gate_method_names_wave304_ok: bool,
    /// Wave 304 object manager dual-world empty-gate nav/commands residual pack.
    pub live_object_manager_dual_world_empty_gate_nav_commands_wave304_ok: bool,
    /// Wave 304 object manager dual-world empty-gate live residual.
    pub live_object_manager_dual_world_empty_gate_live_wave304_ok: bool,
    /// Wave 305 sticky bomb dual-world empty-gate method names residual pack.
    pub live_sticky_bomb_dual_world_empty_gate_method_names_wave305_ok: bool,
    /// Wave 305 sticky bomb dual-world empty-gate nav/commands residual pack.
    pub live_sticky_bomb_dual_world_empty_gate_nav_commands_wave305_ok: bool,
    /// Wave 305 sticky bomb dual-world empty-gate live residual.
    pub live_sticky_bomb_dual_world_empty_gate_live_wave305_ok: bool,
    /// Wave 306 auto heal dual-world empty-gate method names residual pack.
    pub live_auto_heal_dual_world_empty_gate_method_names_wave306_ok: bool,
    /// Wave 306 auto heal dual-world empty-gate nav/commands residual pack.
    pub live_auto_heal_dual_world_empty_gate_nav_commands_wave306_ok: bool,
    /// Wave 306 auto heal dual-world empty-gate live residual.
    pub live_auto_heal_dual_world_empty_gate_live_wave306_ok: bool,
    /// Wave 307 grant stealth dual-world empty-gate method names residual pack.
    pub live_grant_stealth_dual_world_empty_gate_method_names_wave307_ok: bool,
    /// Wave 307 grant stealth dual-world empty-gate nav/commands residual pack.
    pub live_grant_stealth_dual_world_empty_gate_nav_commands_wave307_ok: bool,
    /// Wave 307 grant stealth dual-world empty-gate live residual.
    pub live_grant_stealth_dual_world_empty_gate_live_wave307_ok: bool,
    /// Wave 308 status bits upgrade dual-world empty-gate method names residual pack.
    pub live_status_bits_upgrade_dual_world_empty_gate_method_names_wave308_ok: bool,
    /// Wave 308 status bits upgrade dual-world empty-gate nav/commands residual pack.
    pub live_status_bits_upgrade_dual_world_empty_gate_nav_commands_wave308_ok: bool,
    /// Wave 308 status bits upgrade dual-world empty-gate live residual.
    pub live_status_bits_upgrade_dual_world_empty_gate_live_wave308_ok: bool,
    /// Wave 309 jet AI dual-world empty-gate method names residual pack.
    pub live_jet_ai_dual_world_empty_gate_method_names_wave309_ok: bool,
    /// Wave 309 jet AI dual-world empty-gate nav/commands residual pack.
    pub live_jet_ai_dual_world_empty_gate_nav_commands_wave309_ok: bool,
    /// Wave 309 jet AI dual-world empty-gate live residual.
    pub live_jet_ai_dual_world_empty_gate_live_wave309_ok: bool,
    /// Wave 310 parking place dual-world empty-gate method names residual pack.
    pub live_parking_place_dual_world_empty_gate_method_names_wave310_ok: bool,
    /// Wave 310 parking place dual-world empty-gate nav/commands residual pack.
    pub live_parking_place_dual_world_empty_gate_nav_commands_wave310_ok: bool,
    /// Wave 310 parking place dual-world empty-gate live residual.
    pub live_parking_place_dual_world_empty_gate_live_wave310_ok: bool,
    /// Wave 311 flight deck dual-world empty-gate method names residual pack.
    pub live_flight_deck_dual_world_empty_gate_method_names_wave311_ok: bool,
    /// Wave 311 flight deck dual-world empty-gate nav/commands residual pack.
    pub live_flight_deck_dual_world_empty_gate_nav_commands_wave311_ok: bool,
    /// Wave 311 flight deck dual-world empty-gate live residual.
    pub live_flight_deck_dual_world_empty_gate_live_wave311_ok: bool,
    /// Wave 312 exit strategies dual-world empty-gate method names residual pack.
    pub live_exit_strategies_dual_world_empty_gate_method_names_wave312_ok: bool,
    /// Wave 312 exit strategies dual-world empty-gate nav/commands residual pack.
    pub live_exit_strategies_dual_world_empty_gate_nav_commands_wave312_ok: bool,
    /// Wave 312 exit strategies dual-world empty-gate live residual.
    pub live_exit_strategies_dual_world_empty_gate_live_wave312_ok: bool,
    /// Wave 313 collision system dual-world empty-gate method names residual pack.
    pub live_collision_system_dual_world_empty_gate_method_names_wave313_ok: bool,
    /// Wave 313 collision system dual-world empty-gate nav/commands residual pack.
    pub live_collision_system_dual_world_empty_gate_nav_commands_wave313_ok: bool,
    /// Wave 313 collision system dual-world empty-gate live residual.
    pub live_collision_system_dual_world_empty_gate_live_wave313_ok: bool,
    /// Wave 314 max health upgrade dual-world empty-gate method names residual pack.
    pub live_max_health_upgrade_dual_world_empty_gate_method_names_wave314_ok: bool,
    /// Wave 314 max health upgrade dual-world empty-gate nav/commands residual pack.
    pub live_max_health_upgrade_dual_world_empty_gate_nav_commands_wave314_ok: bool,
    /// Wave 314 max health upgrade dual-world empty-gate live residual.
    pub live_max_health_upgrade_dual_world_empty_gate_live_wave314_ok: bool,
    /// Wave 315 structure topple dual-world empty-gate method names residual pack.
    pub live_structure_topple_dual_world_empty_gate_method_names_wave315_ok: bool,
    /// Wave 315 structure topple dual-world empty-gate nav/commands residual pack.
    pub live_structure_topple_dual_world_empty_gate_nav_commands_wave315_ok: bool,
    /// Wave 315 structure topple dual-world empty-gate live residual.
    pub live_structure_topple_dual_world_empty_gate_live_wave315_ok: bool,
    /// Wave 316 physics update dual-world empty-gate method names residual pack.
    pub live_physics_update_dual_world_empty_gate_method_names_wave316_ok: bool,
    /// Wave 316 physics update dual-world empty-gate nav/commands residual pack.
    pub live_physics_update_dual_world_empty_gate_nav_commands_wave316_ok: bool,
    /// Wave 316 physics update dual-world empty-gate live residual.
    pub live_physics_update_dual_world_empty_gate_live_wave316_ok: bool,
    /// Wave 317 cleanup hazard dual-world empty-gate method names residual pack.
    pub live_cleanup_hazard_dual_world_empty_gate_method_names_wave317_ok: bool,
    /// Wave 317 cleanup hazard dual-world empty-gate nav/commands residual pack.
    pub live_cleanup_hazard_dual_world_empty_gate_nav_commands_wave317_ok: bool,
    /// Wave 317 cleanup hazard dual-world empty-gate live residual.
    pub live_cleanup_hazard_dual_world_empty_gate_live_wave317_ok: bool,
    /// Wave 318 bridge tower dual-world empty-gate method names residual pack.
    pub live_bridge_tower_dual_world_empty_gate_method_names_wave318_ok: bool,
    /// Wave 318 bridge tower dual-world empty-gate nav/commands residual pack.
    pub live_bridge_tower_dual_world_empty_gate_nav_commands_wave318_ok: bool,
    /// Wave 318 bridge tower dual-world empty-gate live residual.
    pub live_bridge_tower_dual_world_empty_gate_live_wave318_ok: bool,
    /// Wave 319 armor upgrade dual-world empty-gate method names residual pack.
    pub live_armor_upgrade_dual_world_empty_gate_method_names_wave319_ok: bool,
    /// Wave 319 armor upgrade dual-world empty-gate nav/commands residual pack.
    pub live_armor_upgrade_dual_world_empty_gate_nav_commands_wave319_ok: bool,
    /// Wave 319 armor upgrade dual-world empty-gate live residual.
    pub live_armor_upgrade_dual_world_empty_gate_live_wave319_ok: bool,
    /// Wave 320 paradrop power dual-world empty-gate method names residual pack.
    pub live_paradrop_power_dual_world_empty_gate_method_names_wave320_ok: bool,
    /// Wave 320 paradrop power dual-world empty-gate nav/commands residual pack.
    pub live_paradrop_power_dual_world_empty_gate_nav_commands_wave320_ok: bool,
    /// Wave 320 paradrop power dual-world empty-gate live residual.
    pub live_paradrop_power_dual_world_empty_gate_live_wave320_ok: bool,
    /// Wave 321 fuel air bomb dual-world empty-gate method names residual pack.
    pub live_fuel_air_bomb_dual_world_empty_gate_method_names_wave321_ok: bool,
    /// Wave 321 fuel air bomb dual-world empty-gate nav/commands residual pack.
    pub live_fuel_air_bomb_dual_world_empty_gate_nav_commands_wave321_ok: bool,
    /// Wave 321 fuel air bomb dual-world empty-gate live residual.
    pub live_fuel_air_bomb_dual_world_empty_gate_live_wave321_ok: bool,
    /// Wave 322 tensile formation dual-world empty-gate method names residual pack.
    pub live_tensile_formation_dual_world_empty_gate_method_names_wave322_ok: bool,
    /// Wave 322 tensile formation dual-world empty-gate nav/commands residual pack.
    pub live_tensile_formation_dual_world_empty_gate_nav_commands_wave322_ok: bool,
    /// Wave 322 tensile formation dual-world empty-gate live residual.
    pub live_tensile_formation_dual_world_empty_gate_live_wave322_ok: bool,
    /// Wave 323 die mod dual-world empty-gate method names residual pack.
    pub live_die_mod_dual_world_empty_gate_method_names_wave323_ok: bool,
    /// Wave 323 die mod dual-world empty-gate nav/commands residual pack.
    pub live_die_mod_dual_world_empty_gate_nav_commands_wave323_ok: bool,
    /// Wave 323 die mod dual-world empty-gate live residual.
    pub live_die_mod_dual_world_empty_gate_live_wave323_ok: bool,
    /// Wave 324 partition manager dual-world empty-gate method names residual pack.
    pub live_partition_manager_dual_world_empty_gate_method_names_wave324_ok: bool,
    /// Wave 324 partition manager dual-world empty-gate nav/commands residual pack.
    pub live_partition_manager_dual_world_empty_gate_nav_commands_wave324_ok: bool,
    /// Wave 324 partition manager dual-world empty-gate live residual.
    pub live_partition_manager_dual_world_empty_gate_live_wave324_ok: bool,
    /// Wave 325 spectre gunship dual-world empty-gate method names residual pack.
    pub live_spectre_gunship_dual_world_empty_gate_method_names_wave325_ok: bool,
    /// Wave 325 spectre gunship dual-world empty-gate nav/commands residual pack.
    pub live_spectre_gunship_dual_world_empty_gate_nav_commands_wave325_ok: bool,
    /// Wave 325 spectre gunship dual-world empty-gate live residual.
    pub live_spectre_gunship_dual_world_empty_gate_live_wave325_ok: bool,
    /// Wave 326 production update dual-world empty-gate method names residual pack.
    pub live_production_update_dual_world_empty_gate_method_names_wave326_ok: bool,
    /// Wave 326 production update dual-world empty-gate nav/commands residual pack.
    pub live_production_update_dual_world_empty_gate_nav_commands_wave326_ok: bool,
    /// Wave 326 production update dual-world empty-gate live residual.
    pub live_production_update_dual_world_empty_gate_live_wave326_ok: bool,
    /// Wave 327 neutron blast dual-world empty-gate method names residual pack.
    pub live_neutron_blast_dual_world_empty_gate_method_names_wave327_ok: bool,
    /// Wave 327 neutron blast dual-world empty-gate nav/commands residual pack.
    pub live_neutron_blast_dual_world_empty_gate_nav_commands_wave327_ok: bool,
    /// Wave 327 neutron blast dual-world empty-gate live residual.
    pub live_neutron_blast_dual_world_empty_gate_live_wave327_ok: bool,
    /// Wave 328 countermeasures dual-world empty-gate method names residual pack.
    pub live_countermeasures_dual_world_empty_gate_method_names_wave328_ok: bool,
    /// Wave 328 countermeasures dual-world empty-gate nav/commands residual pack.
    pub live_countermeasures_dual_world_empty_gate_nav_commands_wave328_ok: bool,
    /// Wave 328 countermeasures dual-world empty-gate live residual.
    pub live_countermeasures_dual_world_empty_gate_live_wave328_ok: bool,
    /// Wave 329 skirmish player dual-world empty-gate method names residual pack.
    pub live_skirmish_player_dual_world_empty_gate_method_names_wave329_ok: bool,
    /// Wave 329 skirmish player dual-world empty-gate nav/commands residual pack.
    pub live_skirmish_player_dual_world_empty_gate_nav_commands_wave329_ok: bool,
    /// Wave 329 skirmish player dual-world empty-gate live residual.
    pub live_skirmish_player_dual_world_empty_gate_live_wave329_ok: bool,
    /// Wave 330 a10 strike dual-world empty-gate method names residual pack.
    pub live_a10_strike_dual_world_empty_gate_method_names_wave330_ok: bool,
    /// Wave 330 a10 strike dual-world empty-gate nav/commands residual pack.
    pub live_a10_strike_dual_world_empty_gate_nav_commands_wave330_ok: bool,
    /// Wave 330 a10 strike dual-world empty-gate live residual.
    pub live_a10_strike_dual_world_empty_gate_live_wave330_ok: bool,
    /// Wave 331 rebuild hole dual-world empty-gate method names residual pack.
    pub live_rebuild_hole_dual_world_empty_gate_method_names_wave331_ok: bool,
    /// Wave 331 rebuild hole dual-world empty-gate nav/commands residual pack.
    pub live_rebuild_hole_dual_world_empty_gate_nav_commands_wave331_ok: bool,
    /// Wave 331 rebuild hole dual-world empty-gate live residual.
    pub live_rebuild_hole_dual_world_empty_gate_live_wave331_ok: bool,
    /// Wave 332 wave guide dual-world empty-gate method names residual pack.
    pub live_wave_guide_dual_world_empty_gate_method_names_wave332_ok: bool,
    /// Wave 332 wave guide dual-world empty-gate nav/commands residual pack.
    pub live_wave_guide_dual_world_empty_gate_nav_commands_wave332_ok: bool,
    /// Wave 332 wave guide dual-world empty-gate live residual.
    pub live_wave_guide_dual_world_empty_gate_live_wave332_ok: bool,
    /// Wave 333 emp update dual-world empty-gate method names residual pack.
    pub live_emp_update_dual_world_empty_gate_method_names_wave333_ok: bool,
    /// Wave 333 emp update dual-world empty-gate nav/commands residual pack.
    pub live_emp_update_dual_world_empty_gate_nav_commands_wave333_ok: bool,
    /// Wave 333 emp update dual-world empty-gate live residual.
    pub live_emp_update_dual_world_empty_gate_live_wave333_ok: bool,
    /// Wave 334 bunker buster dual-world empty-gate method names residual pack.
    pub live_bunker_buster_dual_world_empty_gate_method_names_wave334_ok: bool,
    /// Wave 334 bunker buster dual-world empty-gate nav/commands residual pack.
    pub live_bunker_buster_dual_world_empty_gate_nav_commands_wave334_ok: bool,
    /// Wave 334 bunker buster dual-world empty-gate live residual.
    pub live_bunker_buster_dual_world_empty_gate_live_wave334_ok: bool,
    /// Wave 335 bridge scaffold dual-world empty-gate method names residual pack.
    pub live_bridge_scaffold_dual_world_empty_gate_method_names_wave335_ok: bool,
    /// Wave 335 bridge scaffold dual-world empty-gate nav/commands residual pack.
    pub live_bridge_scaffold_dual_world_empty_gate_nav_commands_wave335_ok: bool,
    /// Wave 335 bridge scaffold dual-world empty-gate live residual.
    pub live_bridge_scaffold_dual_world_empty_gate_live_wave335_ok: bool,
    /// Wave 336 assisted targeting dual-world empty-gate method names residual pack.
    pub live_assisted_targeting_dual_world_empty_gate_method_names_wave336_ok: bool,
    /// Wave 336 assisted targeting dual-world empty-gate nav/commands residual pack.
    pub live_assisted_targeting_dual_world_empty_gate_nav_commands_wave336_ok: bool,
    /// Wave 336 assisted targeting dual-world empty-gate live residual.
    pub live_assisted_targeting_dual_world_empty_gate_live_wave336_ok: bool,
    /// Wave 337 economy dual-world empty-gate method names residual pack.
    pub live_economy_dual_world_empty_gate_method_names_wave337_ok: bool,
    /// Wave 337 economy dual-world empty-gate nav/commands residual pack.
    pub live_economy_dual_world_empty_gate_nav_commands_wave337_ok: bool,
    /// Wave 337 economy dual-world empty-gate live residual.
    pub live_economy_dual_world_empty_gate_live_wave337_ok: bool,
    /// Wave 338 turret ai dual-world empty-gate method names residual pack.
    pub live_turret_ai_dual_world_empty_gate_method_names_wave338_ok: bool,
    /// Wave 338 turret ai dual-world empty-gate nav/commands residual pack.
    pub live_turret_ai_dual_world_empty_gate_nav_commands_wave338_ok: bool,
    /// Wave 338 turret ai dual-world empty-gate live residual.
    pub live_turret_ai_dual_world_empty_gate_live_wave338_ok: bool,
    /// Wave 339 stealth detector module dual-world empty-gate method names residual pack.
    pub live_stealth_detector_module_dual_world_empty_gate_method_names_wave339_ok: bool,
    /// Wave 339 stealth detector module dual-world empty-gate nav/commands residual pack.
    pub live_stealth_detector_module_dual_world_empty_gate_nav_commands_wave339_ok: bool,
    /// Wave 339 stealth detector module dual-world empty-gate live residual.
    pub live_stealth_detector_module_dual_world_empty_gate_live_wave339_ok: bool,
    /// Wave 340 modules dual-world empty-gate method names residual pack.
    pub live_modules_dual_world_empty_gate_method_names_wave340_ok: bool,
    /// Wave 340 modules dual-world empty-gate nav/commands residual pack.
    pub live_modules_dual_world_empty_gate_nav_commands_wave340_ok: bool,
    /// Wave 340 modules dual-world empty-gate live residual.
    pub live_modules_dual_world_empty_gate_live_wave340_ok: bool,
    /// Wave 341 terrain dual-world empty-gate method names residual pack.
    pub live_terrain_dual_world_empty_gate_method_names_wave341_ok: bool,
    /// Wave 341 terrain dual-world empty-gate nav/commands residual pack.
    pub live_terrain_dual_world_empty_gate_nav_commands_wave341_ok: bool,
    /// Wave 341 terrain dual-world empty-gate live residual.
    pub live_terrain_dual_world_empty_gate_live_wave341_ok: bool,
    /// Wave 342 special power template dual-world empty-gate method names residual pack.
    pub live_special_power_template_dual_world_empty_gate_method_names_wave342_ok: bool,
    /// Wave 342 special power template dual-world empty-gate nav/commands residual pack.
    pub live_special_power_template_dual_world_empty_gate_nav_commands_wave342_ok: bool,
    /// Wave 342 special power template dual-world empty-gate live residual.
    pub live_special_power_template_dual_world_empty_gate_live_wave342_ok: bool,
    /// Wave 343 script evaluator dual-world empty-gate method names residual pack.
    pub live_script_evaluator_dual_world_empty_gate_method_names_wave343_ok: bool,
    /// Wave 343 script evaluator dual-world empty-gate nav/commands residual pack.
    pub live_script_evaluator_dual_world_empty_gate_nav_commands_wave343_ok: bool,
    /// Wave 343 script evaluator dual-world empty-gate live residual.
    pub live_script_evaluator_dual_world_empty_gate_live_wave343_ok: bool,
    /// Wave 344 system game logic dual-world empty-gate method names residual pack.
    pub live_system_game_logic_dual_world_empty_gate_method_names_wave344_ok: bool,
    /// Wave 344 system game logic dual-world empty-gate nav/commands residual pack.
    pub live_system_game_logic_dual_world_empty_gate_nav_commands_wave344_ok: bool,
    /// Wave 344 system game logic dual-world empty-gate live residual.
    pub live_system_game_logic_dual_world_empty_gate_live_wave344_ok: bool,
    /// Wave 345 meta event dual-world empty-gate method names residual pack.
    pub live_meta_event_dual_world_empty_gate_method_names_wave345_ok: bool,
    /// Wave 345 meta event dual-world empty-gate nav/commands residual pack.
    pub live_meta_event_dual_world_empty_gate_nav_commands_wave345_ok: bool,
    /// Wave 345 meta event dual-world empty-gate live residual.
    pub live_meta_event_dual_world_empty_gate_live_wave345_ok: bool,
    /// Wave 346 spawn behavior dual-world empty-gate method names residual pack.
    pub live_spawn_behavior_dual_world_empty_gate_method_names_wave346_ok: bool,
    /// Wave 346 spawn behavior dual-world empty-gate nav/commands residual pack.
    pub live_spawn_behavior_dual_world_empty_gate_nav_commands_wave346_ok: bool,
    /// Wave 346 spawn behavior dual-world empty-gate live residual.
    pub live_spawn_behavior_dual_world_empty_gate_live_wave346_ok: bool,
    /// Wave 347 action manager dual-world empty-gate method names residual pack.
    pub live_action_manager_dual_world_empty_gate_method_names_wave347_ok: bool,
    /// Wave 347 action manager dual-world empty-gate nav/commands residual pack.
    pub live_action_manager_dual_world_empty_gate_nav_commands_wave347_ok: bool,
    /// Wave 347 action manager dual-world empty-gate live residual.
    pub live_action_manager_dual_world_empty_gate_live_wave347_ok: bool,
    /// Wave 348 script engine dual-world empty-gate method names residual pack.
    pub live_script_engine_dual_world_empty_gate_method_names_wave348_ok: bool,
    /// Wave 348 script engine dual-world empty-gate nav/commands residual pack.
    pub live_script_engine_dual_world_empty_gate_nav_commands_wave348_ok: bool,
    /// Wave 348 script engine dual-world empty-gate live residual.
    pub live_script_engine_dual_world_empty_gate_live_wave348_ok: bool,
    /// Wave 349 chinook ai dual-world empty-gate method names residual pack.
    pub live_chinook_ai_dual_world_empty_gate_method_names_wave349_ok: bool,
    /// Wave 349 chinook ai dual-world empty-gate nav/commands residual pack.
    pub live_chinook_ai_dual_world_empty_gate_nav_commands_wave349_ok: bool,
    /// Wave 349 chinook ai dual-world empty-gate live residual.
    pub live_chinook_ai_dual_world_empty_gate_live_wave349_ok: bool,
    /// Wave 350 missile ai dual-world empty-gate method names residual pack.
    pub live_missile_ai_dual_world_empty_gate_method_names_wave350_ok: bool,
    /// Wave 350 missile ai dual-world empty-gate nav/commands residual pack.
    pub live_missile_ai_dual_world_empty_gate_nav_commands_wave350_ok: bool,
    /// Wave 350 missile ai dual-world empty-gate live residual.
    pub live_missile_ai_dual_world_empty_gate_live_wave350_ok: bool,
    /// Wave 351 dozer ai dual-world empty-gate method names residual pack.
    pub live_dozer_ai_dual_world_empty_gate_method_names_wave351_ok: bool,
    /// Wave 351 dozer ai dual-world empty-gate nav/commands residual pack.
    pub live_dozer_ai_dual_world_empty_gate_nav_commands_wave351_ok: bool,
    /// Wave 351 dozer ai dual-world empty-gate live residual.
    pub live_dozer_ai_dual_world_empty_gate_live_wave351_ok: bool,
    /// Wave 352 deliver payload ai dual-world empty-gate method names residual pack.
    pub live_deliver_payload_ai_dual_world_empty_gate_method_names_wave352_ok: bool,
    /// Wave 352 deliver payload ai dual-world empty-gate nav/commands residual pack.
    pub live_deliver_payload_ai_dual_world_empty_gate_nav_commands_wave352_ok: bool,
    /// Wave 352 deliver payload ai dual-world empty-gate live residual.
    pub live_deliver_payload_ai_dual_world_empty_gate_live_wave352_ok: bool,
    /// Wave 353 special power module dual-world empty-gate method names residual pack.
    pub live_special_power_module_dual_world_empty_gate_method_names_wave353_ok: bool,
    /// Wave 353 special power module dual-world empty-gate nav/commands residual pack.
    pub live_special_power_module_dual_world_empty_gate_nav_commands_wave353_ok: bool,
    /// Wave 353 special power module dual-world empty-gate live residual.
    pub live_special_power_module_dual_world_empty_gate_live_wave353_ok: bool,
    /// Wave 354 pow truck ai dual-world empty-gate method names residual pack.
    pub live_pow_truck_ai_dual_world_empty_gate_method_names_wave354_ok: bool,
    /// Wave 354 pow truck ai dual-world empty-gate nav/commands residual pack.
    pub live_pow_truck_ai_dual_world_empty_gate_nav_commands_wave354_ok: bool,
    /// Wave 354 pow truck ai dual-world empty-gate live residual.
    pub live_pow_truck_ai_dual_world_empty_gate_live_wave354_ok: bool,
    /// Wave 355 dock update dual-world empty-gate method names residual pack.
    pub live_dock_update_dual_world_empty_gate_method_names_wave355_ok: bool,
    /// Wave 355 dock update dual-world empty-gate nav/commands residual pack.
    pub live_dock_update_dual_world_empty_gate_nav_commands_wave355_ok: bool,
    /// Wave 355 dock update dual-world empty-gate live residual.
    pub live_dock_update_dual_world_empty_gate_live_wave355_ok: bool,
    /// Wave 356 weapon template dual-world empty-gate method names residual pack.
    pub live_weapon_template_dual_world_empty_gate_method_names_wave356_ok: bool,
    /// Wave 356 weapon template dual-world empty-gate nav/commands residual pack.
    pub live_weapon_template_dual_world_empty_gate_nav_commands_wave356_ok: bool,
    /// Wave 356 weapon template dual-world empty-gate live residual.
    pub live_weapon_template_dual_world_empty_gate_live_wave356_ok: bool,
    /// Wave 357 railroad guide ai dual-world empty-gate method names residual pack.
    pub live_railroad_guide_ai_dual_world_empty_gate_method_names_wave357_ok: bool,
    /// Wave 357 railroad guide ai dual-world empty-gate nav/commands residual pack.
    pub live_railroad_guide_ai_dual_world_empty_gate_nav_commands_wave357_ok: bool,
    /// Wave 357 railroad guide ai dual-world empty-gate live residual.
    pub live_railroad_guide_ai_dual_world_empty_gate_live_wave357_ok: bool,
    /// Wave 358 hack internet ai dual-world empty-gate method names residual pack.
    pub live_hack_internet_ai_dual_world_empty_gate_method_names_wave358_ok: bool,
    /// Wave 358 hack internet ai dual-world empty-gate nav/commands residual pack.
    pub live_hack_internet_ai_dual_world_empty_gate_nav_commands_wave358_ok: bool,
    /// Wave 358 hack internet ai dual-world empty-gate live residual.
    pub live_hack_internet_ai_dual_world_empty_gate_live_wave358_ok: bool,
    /// Wave 359 spectre gunship deployment dual-world empty-gate method names residual pack.
    pub live_spectre_gunship_deployment_dual_world_empty_gate_method_names_wave359_ok: bool,
    /// Wave 359 spectre gunship deployment dual-world empty-gate nav/commands residual pack.
    pub live_spectre_gunship_deployment_dual_world_empty_gate_nav_commands_wave359_ok: bool,
    /// Wave 359 spectre gunship deployment dual-world empty-gate live residual.
    pub live_spectre_gunship_deployment_dual_world_empty_gate_live_wave359_ok: bool,
    /// Wave 360 radius decal update dual-world empty-gate method names residual pack.
    pub live_radius_decal_update_dual_world_empty_gate_method_names_wave360_ok: bool,
    /// Wave 360 radius decal update dual-world empty-gate nav/commands residual pack.
    pub live_radius_decal_update_dual_world_empty_gate_nav_commands_wave360_ok: bool,
    /// Wave 360 radius decal update dual-world empty-gate live residual.
    pub live_radius_decal_update_dual_world_empty_gate_live_wave360_ok: bool,
    /// Wave 361 railed transport dock dual-world empty-gate method names residual pack.
    pub live_railed_transport_dock_dual_world_empty_gate_method_names_wave361_ok: bool,
    /// Wave 361 railed transport dock dual-world empty-gate nav/commands residual pack.
    pub live_railed_transport_dock_dual_world_empty_gate_nav_commands_wave361_ok: bool,
    /// Wave 361 railed transport dock dual-world empty-gate live residual.
    pub live_railed_transport_dock_dual_world_empty_gate_live_wave361_ok: bool,
    /// Wave 362 structure collapse update dual-world empty-gate method names residual pack.
    pub live_structure_collapse_update_dual_world_empty_gate_method_names_wave362_ok: bool,
    /// Wave 362 structure collapse update dual-world empty-gate nav/commands residual pack.
    pub live_structure_collapse_update_dual_world_empty_gate_nav_commands_wave362_ok: bool,
    /// Wave 362 structure collapse update dual-world empty-gate live residual.
    pub live_structure_collapse_update_dual_world_empty_gate_live_wave362_ok: bool,
    /// Wave 363 propaganda tower behavior dual-world empty-gate method names residual pack.
    pub live_propaganda_tower_behavior_dual_world_empty_gate_method_names_wave363_ok: bool,
    /// Wave 363 propaganda tower behavior dual-world empty-gate nav/commands residual pack.
    pub live_propaganda_tower_behavior_dual_world_empty_gate_nav_commands_wave363_ok: bool,
    /// Wave 363 propaganda tower behavior dual-world empty-gate live residual.
    pub live_propaganda_tower_behavior_dual_world_empty_gate_live_wave363_ok: bool,
    /// Wave 364 propaganda center behavior dual-world empty-gate method names residual pack.
    pub live_propaganda_center_behavior_dual_world_empty_gate_method_names_wave364_ok: bool,
    /// Wave 364 propaganda center behavior dual-world empty-gate nav/commands residual pack.
    pub live_propaganda_center_behavior_dual_world_empty_gate_nav_commands_wave364_ok: bool,
    /// Wave 364 propaganda center behavior dual-world empty-gate live residual.
    pub live_propaganda_center_behavior_dual_world_empty_gate_live_wave364_ok: bool,
    /// Wave 365 production update complete dual-world empty-gate method names residual pack.
    pub live_production_update_complete_dual_world_empty_gate_method_names_wave365_ok: bool,
    /// Wave 365 production update complete dual-world empty-gate nav/commands residual pack.
    pub live_production_update_complete_dual_world_empty_gate_nav_commands_wave365_ok: bool,
    /// Wave 365 production update complete dual-world empty-gate live residual.
    pub live_production_update_complete_dual_world_empty_gate_live_wave365_ok: bool,
    /// Wave 366 pow truck behavior dual-world empty-gate method names residual pack.
    pub live_pow_truck_behavior_dual_world_empty_gate_method_names_wave366_ok: bool,
    /// Wave 366 pow truck behavior dual-world empty-gate nav/commands residual pack.
    pub live_pow_truck_behavior_dual_world_empty_gate_nav_commands_wave366_ok: bool,
    /// Wave 366 pow truck behavior dual-world empty-gate live residual.
    pub live_pow_truck_behavior_dual_world_empty_gate_live_wave366_ok: bool,
    /// Wave 367 fire weapon when damaged dual-world empty-gate method names residual pack.
    pub live_fire_weapon_when_damaged_behavior_dual_world_empty_gate_method_names_wave367_ok: bool,
    /// Wave 367 fire weapon when damaged dual-world empty-gate nav/commands residual pack.
    pub live_fire_weapon_when_damaged_behavior_dual_world_empty_gate_nav_commands_wave367_ok: bool,
    /// Wave 367 fire weapon when damaged dual-world empty-gate live residual.
    pub live_fire_weapon_when_damaged_behavior_dual_world_empty_gate_live_wave367_ok: bool,
    /// Wave 368 veterancy crate collide dual-world empty-gate method names residual pack.
    pub live_veterancy_crate_collide_dual_world_empty_gate_method_names_wave368_ok: bool,
    /// Wave 368 veterancy crate collide dual-world empty-gate nav/commands residual pack.
    pub live_veterancy_crate_collide_dual_world_empty_gate_nav_commands_wave368_ok: bool,
    /// Wave 368 veterancy crate collide dual-world empty-gate live residual.
    pub live_veterancy_crate_collide_dual_world_empty_gate_live_wave368_ok: bool,
    /// Wave 369 assault transport AI update dual-world empty-gate method names residual pack.
    pub live_assault_transport_ai_update_dual_world_empty_gate_method_names_wave369_ok: bool,
    /// Wave 369 assault transport AI update dual-world empty-gate nav/commands residual pack.
    pub live_assault_transport_ai_update_dual_world_empty_gate_nav_commands_wave369_ok: bool,
    /// Wave 369 assault transport AI update dual-world empty-gate live residual.
    pub live_assault_transport_ai_update_dual_world_empty_gate_live_wave369_ok: bool,
    /// Wave 370 heal contain dual-world empty-gate method names residual pack.
    pub live_heal_contain_dual_world_empty_gate_method_names_wave370_ok: bool,
    /// Wave 370 heal contain dual-world empty-gate nav/commands residual pack.
    pub live_heal_contain_dual_world_empty_gate_nav_commands_wave370_ok: bool,
    /// Wave 370 heal contain dual-world empty-gate live residual.
    pub live_heal_contain_dual_world_empty_gate_live_wave370_ok: bool,
    /// Wave 371 topple update dual-world empty-gate method names residual pack.
    pub live_topple_update_dual_world_empty_gate_method_names_wave371_ok: bool,
    /// Wave 371 topple update dual-world empty-gate nav/commands residual pack.
    pub live_topple_update_dual_world_empty_gate_nav_commands_wave371_ok: bool,
    /// Wave 371 topple update dual-world empty-gate live residual.
    pub live_topple_update_dual_world_empty_gate_live_wave371_ok: bool,
    /// Wave 372 projectile stream update dual-world empty-gate method names residual pack.
    pub live_projectile_stream_update_dual_world_empty_gate_method_names_wave372_ok: bool,
    /// Wave 372 projectile stream update dual-world empty-gate nav/commands residual pack.
    pub live_projectile_stream_update_dual_world_empty_gate_nav_commands_wave372_ok: bool,
    /// Wave 372 projectile stream update dual-world empty-gate live residual.
    pub live_projectile_stream_update_dual_world_empty_gate_live_wave372_ok: bool,
    /// Wave 373 demo trap update dual-world empty-gate method names residual pack.
    pub live_demo_trap_update_dual_world_empty_gate_method_names_wave373_ok: bool,
    /// Wave 373 demo trap update dual-world empty-gate nav/commands residual pack.
    pub live_demo_trap_update_dual_world_empty_gate_nav_commands_wave373_ok: bool,
    /// Wave 373 demo trap update dual-world empty-gate live residual.
    pub live_demo_trap_update_dual_world_empty_gate_live_wave373_ok: bool,
    /// Wave 374 mob member slaved update dual-world empty-gate method names residual pack.
    pub live_mob_member_slaved_update_dual_world_empty_gate_method_names_wave374_ok: bool,
    /// Wave 374 mob member slaved update dual-world empty-gate nav/commands residual pack.
    pub live_mob_member_slaved_update_dual_world_empty_gate_nav_commands_wave374_ok: bool,
    /// Wave 374 mob member slaved update dual-world empty-gate live residual.
    pub live_mob_member_slaved_update_dual_world_empty_gate_live_wave374_ok: bool,
    /// Wave 375 TN guard dual-world empty-gate method names residual pack.
    pub live_tn_guard_dual_world_empty_gate_method_names_wave375_ok: bool,
    /// Wave 375 TN guard dual-world empty-gate nav/commands residual pack.
    pub live_tn_guard_dual_world_empty_gate_nav_commands_wave375_ok: bool,
    /// Wave 375 TN guard dual-world empty-gate live residual.
    pub live_tn_guard_dual_world_empty_gate_live_wave375_ok: bool,
    /// Wave 376 production update dual-world empty-gate method names residual pack.
    pub live_production_update_dual_world_empty_gate_method_names_wave376_ok: bool,
    /// Wave 376 production update dual-world empty-gate nav/commands residual pack.
    pub live_production_update_dual_world_empty_gate_nav_commands_wave376_ok: bool,
    /// Wave 376 production update dual-world empty-gate live residual.
    pub live_production_update_dual_world_empty_gate_live_wave376_ok: bool,
    /// Wave 377 poisoned behavior dual-world empty-gate method names residual pack.
    pub live_poisoned_behavior_dual_world_empty_gate_method_names_wave377_ok: bool,
    /// Wave 377 poisoned behavior dual-world empty-gate nav/commands residual pack.
    pub live_poisoned_behavior_dual_world_empty_gate_nav_commands_wave377_ok: bool,
    /// Wave 377 poisoned behavior dual-world empty-gate live residual.
    pub live_poisoned_behavior_dual_world_empty_gate_live_wave377_ok: bool,
    /// Wave 378 horde update dual-world empty-gate method names residual pack.
    pub live_horde_update_dual_world_empty_gate_method_names_wave378_ok: bool,
    /// Wave 378 horde update dual-world empty-gate nav/commands residual pack.
    pub live_horde_update_dual_world_empty_gate_nav_commands_wave378_ok: bool,
    /// Wave 378 horde update dual-world empty-gate live residual.
    pub live_horde_update_dual_world_empty_gate_live_wave378_ok: bool,
    /// Wave 379 flammable update dual-world empty-gate method names residual pack.
    pub live_flammable_update_dual_world_empty_gate_method_names_wave379_ok: bool,
    /// Wave 379 flammable update dual-world empty-gate nav/commands residual pack.
    pub live_flammable_update_dual_world_empty_gate_nav_commands_wave379_ok: bool,
    /// Wave 379 flammable update dual-world empty-gate live residual.
    pub live_flammable_update_dual_world_empty_gate_live_wave379_ok: bool,
    /// Wave 380 base regenerate update dual-world empty-gate method names residual pack.
    pub live_base_regenerate_update_dual_world_empty_gate_method_names_wave380_ok: bool,
    /// Wave 380 base regenerate update dual-world empty-gate nav/commands residual pack.
    pub live_base_regenerate_update_dual_world_empty_gate_nav_commands_wave380_ok: bool,
    /// Wave 380 base regenerate update dual-world empty-gate live residual.
    pub live_base_regenerate_update_dual_world_empty_gate_live_wave380_ok: bool,
    /// Wave 381 queue production exit dual-world empty-gate method names residual pack.
    pub live_queue_production_exit_behavior_dual_world_empty_gate_method_names_wave381_ok: bool,
    /// Wave 381 queue production exit dual-world empty-gate nav/commands residual pack.
    pub live_queue_production_exit_behavior_dual_world_empty_gate_nav_commands_wave381_ok: bool,
    /// Wave 381 queue production exit dual-world empty-gate live residual.
    pub live_queue_production_exit_behavior_dual_world_empty_gate_live_wave381_ok: bool,
    /// Wave 382 missile launcher building dual-world empty-gate method names residual pack.
    pub live_missile_launcher_building_update_dual_world_empty_gate_method_names_wave382_ok: bool,
    /// Wave 382 missile launcher building dual-world empty-gate nav/commands residual pack.
    pub live_missile_launcher_building_update_dual_world_empty_gate_nav_commands_wave382_ok: bool,
    /// Wave 382 missile launcher building dual-world empty-gate live residual.
    pub live_missile_launcher_building_update_dual_world_empty_gate_live_wave382_ok: bool,
    /// Wave 383 dynamic shroud clearing dual-world empty-gate method names residual pack.
    pub live_dynamic_shroud_clearing_range_update_dual_world_empty_gate_method_names_wave383_ok:
        bool,
    /// Wave 383 dynamic shroud clearing dual-world empty-gate nav/commands residual pack.
    pub live_dynamic_shroud_clearing_range_update_dual_world_empty_gate_nav_commands_wave383_ok:
        bool,
    /// Wave 383 dynamic shroud clearing dual-world empty-gate live residual.
    pub live_dynamic_shroud_clearing_range_update_dual_world_empty_gate_live_wave383_ok: bool,
    /// Wave 384 command button hunt dual-world empty-gate method names residual pack.
    pub live_command_button_hunt_update_dual_world_empty_gate_method_names_wave384_ok: bool,
    /// Wave 384 command button hunt dual-world empty-gate nav/commands residual pack.
    pub live_command_button_hunt_update_dual_world_empty_gate_nav_commands_wave384_ok: bool,
    /// Wave 384 command button hunt dual-world empty-gate live residual.
    pub live_command_button_hunt_update_dual_world_empty_gate_live_wave384_ok: bool,
    /// Wave 385 prison behavior dual-world empty-gate method names residual pack.
    pub live_prison_behavior_dual_world_empty_gate_method_names_wave385_ok: bool,
    /// Wave 385 prison behavior dual-world empty-gate nav/commands residual pack.
    pub live_prison_behavior_dual_world_empty_gate_nav_commands_wave385_ok: bool,
    /// Wave 385 prison behavior dual-world empty-gate live residual.
    pub live_prison_behavior_dual_world_empty_gate_live_wave385_ok: bool,
    /// Wave 386 generate minefield dual-world empty-gate method names residual pack.
    pub live_generate_minefield_behavior_dual_world_empty_gate_method_names_wave386_ok: bool,
    /// Wave 386 generate minefield dual-world empty-gate nav/commands residual pack.
    pub live_generate_minefield_behavior_dual_world_empty_gate_nav_commands_wave386_ok: bool,
    /// Wave 386 generate minefield dual-world empty-gate live residual.
    pub live_generate_minefield_behavior_dual_world_empty_gate_live_wave386_ok: bool,
    /// Wave 387 demoralize special power dual-world empty-gate method names residual pack.
    pub live_demoralize_special_power_dual_world_empty_gate_method_names_wave387_ok: bool,
    /// Wave 387 demoralize special power dual-world empty-gate nav/commands residual pack.
    pub live_demoralize_special_power_dual_world_empty_gate_nav_commands_wave387_ok: bool,
    /// Wave 387 demoralize special power dual-world empty-gate live residual.
    pub live_demoralize_special_power_dual_world_empty_gate_live_wave387_ok: bool,
    /// Wave 388 stealth detector dual-world empty-gate method names residual pack.
    pub live_stealth_detector_update_dual_world_empty_gate_method_names_wave388_ok: bool,
    /// Wave 388 stealth detector dual-world empty-gate nav/commands residual pack.
    pub live_stealth_detector_update_dual_world_empty_gate_nav_commands_wave388_ok: bool,
    /// Wave 388 stealth detector dual-world empty-gate live residual.
    pub live_stealth_detector_update_dual_world_empty_gate_live_wave388_ok: bool,
    /// Wave 389 hive structure body dual-world empty-gate method names residual pack.
    pub live_hive_structure_body_dual_world_empty_gate_method_names_wave389_ok: bool,
    /// Wave 389 hive structure body dual-world empty-gate nav/commands residual pack.
    pub live_hive_structure_body_dual_world_empty_gate_nav_commands_wave389_ok: bool,
    /// Wave 389 hive structure body dual-world empty-gate live residual.
    pub live_hive_structure_body_dual_world_empty_gate_live_wave389_ok: bool,
    /// Wave 390 salvage crate collide dual-world empty-gate method names residual pack.
    pub live_salvage_crate_collide_dual_world_empty_gate_method_names_wave390_ok: bool,
    /// Wave 390 salvage crate collide dual-world empty-gate nav/commands residual pack.
    pub live_salvage_crate_collide_dual_world_empty_gate_nav_commands_wave390_ok: bool,
    /// Wave 390 salvage crate collide dual-world empty-gate live residual.
    pub live_salvage_crate_collide_dual_world_empty_gate_live_wave390_ok: bool,
    /// Wave 391 sabotage internet center dual-world empty-gate method names residual pack.
    pub live_sabotage_internet_center_crate_collide_dual_world_empty_gate_method_names_wave391_ok:
        bool,
    /// Wave 391 sabotage internet center dual-world empty-gate nav/commands residual pack.
    pub live_sabotage_internet_center_crate_collide_dual_world_empty_gate_nav_commands_wave391_ok:
        bool,
    /// Wave 391 sabotage internet center dual-world empty-gate live residual.
    pub live_sabotage_internet_center_crate_collide_dual_world_empty_gate_live_wave391_ok: bool,
    /// Wave 392 power plant update dual-world empty-gate method names residual pack.
    pub live_power_plant_update_dual_world_empty_gate_method_names_wave392_ok: bool,
    /// Wave 392 power plant update dual-world empty-gate nav/commands residual pack.
    pub live_power_plant_update_dual_world_empty_gate_nav_commands_wave392_ok: bool,
    /// Wave 392 power plant update dual-world empty-gate live residual.
    pub live_power_plant_update_dual_world_empty_gate_live_wave392_ok: bool,
    /// Wave 393 leaflet drop dual-world empty-gate method names residual pack.
    pub live_leaflet_drop_behavior_dual_world_empty_gate_method_names_wave393_ok: bool,
    /// Wave 393 leaflet drop dual-world empty-gate nav/commands residual pack.
    pub live_leaflet_drop_behavior_dual_world_empty_gate_nav_commands_wave393_ok: bool,
    /// Wave 393 leaflet drop dual-world empty-gate live residual.
    pub live_leaflet_drop_behavior_dual_world_empty_gate_live_wave393_ok: bool,
    /// Wave 394 auto deposit dual-world empty-gate method names residual pack.
    pub live_auto_deposit_update_dual_world_empty_gate_method_names_wave394_ok: bool,
    /// Wave 394 auto deposit dual-world empty-gate nav/commands residual pack.
    pub live_auto_deposit_update_dual_world_empty_gate_nav_commands_wave394_ok: bool,
    /// Wave 394 auto deposit dual-world empty-gate live residual.
    pub live_auto_deposit_update_dual_world_empty_gate_live_wave394_ok: bool,
    /// Wave 395 supply warehouse crippling dual-world empty-gate method names residual pack.
    pub live_supply_warehouse_crippling_behavior_dual_world_empty_gate_method_names_wave395_ok:
        bool,
    /// Wave 395 supply warehouse crippling dual-world empty-gate nav/commands residual pack.
    pub live_supply_warehouse_crippling_behavior_dual_world_empty_gate_nav_commands_wave395_ok:
        bool,
    /// Wave 395 supply warehouse crippling dual-world empty-gate live residual.
    pub live_supply_warehouse_crippling_behavior_dual_world_empty_gate_live_wave395_ok: bool,
    /// Wave 396 neutron missile slow death dual-world empty-gate method names residual pack.
    pub live_neutron_missile_slow_death_update_dual_world_empty_gate_method_names_wave396_ok: bool,
    /// Wave 396 neutron missile slow death dual-world empty-gate nav/commands residual pack.
    pub live_neutron_missile_slow_death_update_dual_world_empty_gate_nav_commands_wave396_ok: bool,
    /// Wave 396 neutron missile slow death dual-world empty-gate live residual.
    pub live_neutron_missile_slow_death_update_dual_world_empty_gate_live_wave396_ok: bool,
    /// Wave 397 AI dock dual-world empty-gate method names residual pack.
    pub live_ai_dock_dual_world_empty_gate_method_names_wave397_ok: bool,
    /// Wave 397 AI dock dual-world empty-gate nav/commands residual pack.
    pub live_ai_dock_dual_world_empty_gate_nav_commands_wave397_ok: bool,
    /// Wave 397 AI dock dual-world empty-gate live residual.
    pub live_ai_dock_dual_world_empty_gate_live_wave397_ok: bool,
    /// Wave 398 AI groups dual-world empty-gate method names residual pack.
    pub live_ai_groups_dual_world_empty_gate_method_names_wave398_ok: bool,
    /// Wave 398 AI groups dual-world empty-gate nav/commands residual pack.
    pub live_ai_groups_dual_world_empty_gate_nav_commands_wave398_ok: bool,
    /// Wave 398 AI groups dual-world empty-gate live residual.
    pub live_ai_groups_dual_world_empty_gate_live_wave398_ok: bool,
    /// Wave 399 artillery barrage dual-world empty-gate method names residual pack.
    pub live_artillery_barrage_power_dual_world_empty_gate_method_names_wave399_ok: bool,
    /// Wave 399 artillery barrage dual-world empty-gate nav/commands residual pack.
    pub live_artillery_barrage_power_dual_world_empty_gate_nav_commands_wave399_ok: bool,
    /// Wave 399 artillery barrage dual-world empty-gate live residual.
    pub live_artillery_barrage_power_dual_world_empty_gate_live_wave399_ok: bool,
    /// Wave 400 baikonur launch dual-world empty-gate method names residual pack.
    pub live_baikonur_launch_power_dual_world_empty_gate_method_names_wave400_ok: bool,
    /// Wave 400 baikonur launch dual-world empty-gate nav/commands residual pack.
    pub live_baikonur_launch_power_dual_world_empty_gate_nav_commands_wave400_ok: bool,
    /// Wave 400 baikonur launch dual-world empty-gate live residual.
    pub live_baikonur_launch_power_dual_world_empty_gate_live_wave400_ok: bool,
    /// Wave 401 AI group dual-world empty-gate method names residual pack.
    pub live_ai_group_dual_world_empty_gate_method_names_wave401_ok: bool,
    /// Wave 401 AI group dual-world empty-gate nav/commands residual pack.
    pub live_ai_group_dual_world_empty_gate_nav_commands_wave401_ok: bool,
    /// Wave 401 AI group dual-world empty-gate live residual.
    pub live_ai_group_dual_world_empty_gate_live_wave401_ok: bool,
    /// Wave 402 slaved update dual-world empty-gate method names residual pack.
    pub live_slaved_update_dual_world_empty_gate_method_names_wave402_ok: bool,
    /// Wave 402 slaved update dual-world empty-gate nav/commands residual pack.
    pub live_slaved_update_dual_world_empty_gate_nav_commands_wave402_ok: bool,
    /// Wave 402 slaved update dual-world empty-gate live residual.
    pub live_slaved_update_dual_world_empty_gate_live_wave402_ok: bool,
    /// Wave 403 demoralize power dual-world empty-gate method names residual pack.
    pub live_demoralize_power_dual_world_empty_gate_method_names_wave403_ok: bool,
    /// Wave 403 demoralize power dual-world empty-gate nav/commands residual pack.
    pub live_demoralize_power_dual_world_empty_gate_nav_commands_wave403_ok: bool,
    /// Wave 403 demoralize power dual-world empty-gate live residual.
    pub live_demoralize_power_dual_world_empty_gate_live_wave403_ok: bool,
    /// Wave 404 bone fx update dual-world empty-gate method names residual pack.
    pub live_bone_fx_update_dual_world_empty_gate_method_names_wave404_ok: bool,
    /// Wave 404 bone fx update dual-world empty-gate nav/commands residual pack.
    pub live_bone_fx_update_dual_world_empty_gate_nav_commands_wave404_ok: bool,
    /// Wave 404 bone fx update dual-world empty-gate live residual.
    pub live_bone_fx_update_dual_world_empty_gate_live_wave404_ok: bool,
    /// Wave 405 supply warehouse dock dual-world empty-gate method names residual pack.
    pub live_supply_warehouse_dock_dual_world_empty_gate_method_names_wave405_ok: bool,
    /// Wave 405 supply warehouse dock dual-world empty-gate nav/commands residual pack.
    pub live_supply_warehouse_dock_dual_world_empty_gate_nav_commands_wave405_ok: bool,
    /// Wave 405 supply warehouse dock dual-world empty-gate live residual.
    pub live_supply_warehouse_dock_dual_world_empty_gate_live_wave405_ok: bool,
    /// Wave 406 OCL special power dual-world empty-gate method names residual pack.
    pub live_ocl_special_power_dual_world_empty_gate_method_names_wave406_ok: bool,
    /// Wave 406 OCL special power dual-world empty-gate nav/commands residual pack.
    pub live_ocl_special_power_dual_world_empty_gate_nav_commands_wave406_ok: bool,
    /// Wave 406 OCL special power dual-world empty-gate live residual.
    pub live_ocl_special_power_dual_world_empty_gate_live_wave406_ok: bool,
    /// Wave 407 railed transport AI dual-world empty-gate method names residual pack.
    pub live_railed_transport_ai_update_dual_world_empty_gate_method_names_wave407_ok: bool,
    /// Wave 407 railed transport AI dual-world empty-gate nav/commands residual pack.
    pub live_railed_transport_ai_update_dual_world_empty_gate_nav_commands_wave407_ok: bool,
    /// Wave 407 railed transport AI dual-world empty-gate live residual.
    pub live_railed_transport_ai_update_dual_world_empty_gate_live_wave407_ok: bool,
    /// Wave 408 squish collide dual-world empty-gate method names residual pack.
    pub live_squish_collide_dual_world_empty_gate_method_names_wave408_ok: bool,
    /// Wave 408 squish collide dual-world empty-gate nav/commands residual pack.
    pub live_squish_collide_dual_world_empty_gate_nav_commands_wave408_ok: bool,
    /// Wave 408 squish collide dual-world empty-gate live residual.
    pub live_squish_collide_dual_world_empty_gate_live_wave408_ok: bool,
    /// Wave 409 weapon bonus update dual-world empty-gate method names residual pack.
    pub live_weapon_bonus_update_dual_world_empty_gate_method_names_wave409_ok: bool,
    /// Wave 409 weapon bonus update dual-world empty-gate nav/commands residual pack.
    pub live_weapon_bonus_update_dual_world_empty_gate_nav_commands_wave409_ok: bool,
    /// Wave 409 weapon bonus update dual-world empty-gate live residual.
    pub live_weapon_bonus_update_dual_world_empty_gate_live_wave409_ok: bool,
    /// Wave 410 minefield behavior dual-world empty-gate method names residual pack.
    pub live_minefield_behavior_dual_world_empty_gate_method_names_wave410_ok: bool,
    /// Wave 410 minefield behavior dual-world empty-gate nav/commands residual pack.
    pub live_minefield_behavior_dual_world_empty_gate_nav_commands_wave410_ok: bool,
    /// Wave 410 minefield behavior dual-world empty-gate live residual.
    pub live_minefield_behavior_dual_world_empty_gate_live_wave410_ok: bool,
    /// Wave 411 point defense laser dual-world empty-gate method names residual pack.
    pub live_point_defense_laser_update_dual_world_empty_gate_method_names_wave411_ok: bool,
    /// Wave 411 point defense laser dual-world empty-gate nav/commands residual pack.
    pub live_point_defense_laser_update_dual_world_empty_gate_nav_commands_wave411_ok: bool,
    /// Wave 411 point defense laser dual-world empty-gate live residual.
    pub live_point_defense_laser_update_dual_world_empty_gate_live_wave411_ok: bool,
    /// Wave 412 lifetime update dual-world empty-gate method names residual pack.
    pub live_lifetime_update_dual_world_empty_gate_method_names_wave412_ok: bool,
    /// Wave 412 lifetime update dual-world empty-gate nav/commands residual pack.
    pub live_lifetime_update_dual_world_empty_gate_nav_commands_wave412_ok: bool,
    /// Wave 412 lifetime update dual-world empty-gate live residual.
    pub live_lifetime_update_dual_world_empty_gate_live_wave412_ok: bool,
    /// Wave 413 slow death behavior dual-world empty-gate method names residual pack.
    pub live_slow_death_behavior_dual_world_empty_gate_method_names_wave413_ok: bool,
    /// Wave 413 slow death behavior dual-world empty-gate nav/commands residual pack.
    pub live_slow_death_behavior_dual_world_empty_gate_nav_commands_wave413_ok: bool,
    /// Wave 413 slow death behavior dual-world empty-gate live residual.
    pub live_slow_death_behavior_dual_world_empty_gate_live_wave413_ok: bool,
    /// Wave 414 battle bus slow death dual-world empty-gate method names residual pack.
    pub live_battle_bus_slow_death_behavior_dual_world_empty_gate_method_names_wave414_ok: bool,
    /// Wave 414 battle bus slow death dual-world empty-gate nav/commands residual pack.
    pub live_battle_bus_slow_death_behavior_dual_world_empty_gate_nav_commands_wave414_ok: bool,
    /// Wave 414 battle bus slow death dual-world empty-gate live residual.
    pub live_battle_bus_slow_death_behavior_dual_world_empty_gate_live_wave414_ok: bool,
    /// Wave 415 damage module dual-world empty-gate method names residual pack.
    pub live_damage_module_dual_world_empty_gate_method_names_wave415_ok: bool,
    /// Wave 415 damage module dual-world empty-gate nav/commands residual pack.
    pub live_damage_module_dual_world_empty_gate_nav_commands_wave415_ok: bool,
    /// Wave 415 damage module dual-world empty-gate live residual.
    pub live_damage_module_dual_world_empty_gate_live_wave415_ok: bool,
    /// Wave 416 transition damage fx dual-world empty-gate method names residual pack.
    pub live_transition_damage_fx_dual_world_empty_gate_method_names_wave416_ok: bool,
    /// Wave 416 transition damage fx dual-world empty-gate nav/commands residual pack.
    pub live_transition_damage_fx_dual_world_empty_gate_nav_commands_wave416_ok: bool,
    /// Wave 416 transition damage fx dual-world empty-gate live residual.
    pub live_transition_damage_fx_dual_world_empty_gate_live_wave416_ok: bool,
    /// Wave 417 spawn point production exit dual-world empty-gate method names residual pack.
    pub live_spawn_point_production_exit_behavior_dual_world_empty_gate_method_names_wave417_ok:
        bool,
    /// Wave 417 spawn point production exit dual-world empty-gate nav/commands residual pack.
    pub live_spawn_point_production_exit_behavior_dual_world_empty_gate_nav_commands_wave417_ok:
        bool,
    /// Wave 417 spawn point production exit dual-world empty-gate live residual.
    pub live_spawn_point_production_exit_behavior_dual_world_empty_gate_live_wave417_ok: bool,
    /// Wave 418 build placement dual-world empty-gate method names residual pack.
    pub live_build_placement_dual_world_empty_gate_method_names_wave418_ok: bool,
    /// Wave 418 build placement dual-world empty-gate nav/commands residual pack.
    pub live_build_placement_dual_world_empty_gate_nav_commands_wave418_ok: bool,
    /// Wave 418 build placement dual-world empty-gate live residual.
    pub live_build_placement_dual_world_empty_gate_live_wave418_ok: bool,
    /// Wave 419 weapon set dual-world empty-gate method names residual pack.
    pub live_weapon_set_dual_world_empty_gate_method_names_wave419_ok: bool,
    /// Wave 419 weapon set dual-world empty-gate nav/commands residual pack.
    pub live_weapon_set_dual_world_empty_gate_nav_commands_wave419_ok: bool,
    /// Wave 419 weapon set dual-world empty-gate live residual.
    pub live_weapon_set_dual_world_empty_gate_live_wave419_ok: bool,
    /// Wave 420 experience tracker dual-world empty-gate method names residual pack.
    pub live_experience_tracker_dual_world_empty_gate_method_names_wave420_ok: bool,
    /// Wave 420 experience tracker dual-world empty-gate nav/commands residual pack.
    pub live_experience_tracker_dual_world_empty_gate_nav_commands_wave420_ok: bool,
    /// Wave 420 experience tracker dual-world empty-gate live residual.
    pub live_experience_tracker_dual_world_empty_gate_live_wave420_ok: bool,
    /// Wave 421 AI targeting dual-world empty-gate method names residual pack.
    pub live_ai_targeting_dual_world_empty_gate_method_names_wave421_ok: bool,
    /// Wave 421 AI targeting dual-world empty-gate nav/commands residual pack.
    pub live_ai_targeting_dual_world_empty_gate_nav_commands_wave421_ok: bool,
    /// Wave 421 AI targeting dual-world empty-gate live residual.
    pub live_ai_targeting_dual_world_empty_gate_live_wave421_ok: bool,
    /// Wave 422 move-to state dual-world empty-gate method names residual pack.
    pub live_move_to_state_dual_world_empty_gate_method_names_wave422_ok: bool,
    /// Wave 422 move-to state dual-world empty-gate nav/commands residual pack.
    pub live_move_to_state_dual_world_empty_gate_nav_commands_wave422_ok: bool,
    /// Wave 422 move-to state dual-world empty-gate live residual.
    pub live_move_to_state_dual_world_empty_gate_live_wave422_ok: bool,
    /// Wave 423 locomotor core dual-world empty-gate method names residual pack.
    pub live_locomotor_core_dual_world_empty_gate_method_names_wave423_ok: bool,
    /// Wave 423 locomotor core dual-world empty-gate nav/commands residual pack.
    pub live_locomotor_core_dual_world_empty_gate_nav_commands_wave423_ok: bool,
    /// Wave 423 locomotor core dual-world empty-gate live residual.
    pub live_locomotor_core_dual_world_empty_gate_live_wave423_ok: bool,
    /// Wave 424 path following dual-world empty-gate method names residual pack.
    pub live_path_following_dual_world_empty_gate_method_names_wave424_ok: bool,
    /// Wave 424 path following dual-world empty-gate nav/commands residual pack.
    pub live_path_following_dual_world_empty_gate_nav_commands_wave424_ok: bool,
    /// Wave 424 path following dual-world empty-gate live residual.
    pub live_path_following_dual_world_empty_gate_live_wave424_ok: bool,
    /// Wave 425 AI manager dual-world empty-gate method names residual pack.
    pub live_ai_manager_dual_world_empty_gate_method_names_wave425_ok: bool,
    /// Wave 425 AI manager dual-world empty-gate nav/commands residual pack.
    pub live_ai_manager_dual_world_empty_gate_nav_commands_wave425_ok: bool,
    /// Wave 425 AI manager dual-world empty-gate live residual.
    pub live_ai_manager_dual_world_empty_gate_live_wave425_ok: bool,
    /// Wave 426 pathfind dual-world empty-gate method names residual pack.
    pub live_pathfind_dual_world_empty_gate_method_names_wave426_ok: bool,
    /// Wave 426 pathfind dual-world empty-gate nav/commands residual pack.
    pub live_pathfind_dual_world_empty_gate_nav_commands_wave426_ok: bool,
    /// Wave 426 pathfind dual-world empty-gate live residual.
    pub live_pathfind_dual_world_empty_gate_live_wave426_ok: bool,
    /// Wave 427 fire-weapon-when-dead dual-world empty-gate method names residual pack.
    pub live_fire_weapon_when_dead_behavior_dual_world_empty_gate_method_names_wave427_ok: bool,
    /// Wave 427 fire-weapon-when-dead dual-world empty-gate nav/commands residual pack.
    pub live_fire_weapon_when_dead_behavior_dual_world_empty_gate_nav_commands_wave427_ok: bool,
    /// Wave 427 fire-weapon-when-dead dual-world empty-gate live residual.
    pub live_fire_weapon_when_dead_behavior_dual_world_empty_gate_live_wave427_ok: bool,
    /// Wave 428 guard dual-world empty-gate method names residual pack.
    pub live_guard_dual_world_empty_gate_method_names_wave428_ok: bool,
    /// Wave 428 guard dual-world empty-gate nav/commands residual pack.
    pub live_guard_dual_world_empty_gate_nav_commands_wave428_ok: bool,
    /// Wave 428 guard dual-world empty-gate live residual.
    pub live_guard_dual_world_empty_gate_live_wave428_ok: bool,
    /// Wave 429 guard retaliate dual-world empty-gate method names residual pack.
    pub live_guard_retaliate_dual_world_empty_gate_method_names_wave429_ok: bool,
    /// Wave 429 guard retaliate dual-world empty-gate nav/commands residual pack.
    pub live_guard_retaliate_dual_world_empty_gate_nav_commands_wave429_ok: bool,
    /// Wave 429 guard retaliate dual-world empty-gate live residual.
    pub live_guard_retaliate_dual_world_empty_gate_live_wave429_ok: bool,
    /// Wave 430 wander AI dual-world empty-gate method names residual pack.
    pub live_wander_ai_dual_world_empty_gate_method_names_wave430_ok: bool,
    /// Wave 430 wander AI dual-world empty-gate nav/commands residual pack.
    pub live_wander_ai_dual_world_empty_gate_nav_commands_wave430_ok: bool,
    /// Wave 430 wander AI dual-world empty-gate live residual.
    pub live_wander_ai_dual_world_empty_gate_live_wave430_ok: bool,
    /// Wave 431 subobjects upgrade dual-world empty-gate method names residual pack.
    pub live_subobjects_upgrade_dual_world_empty_gate_method_names_wave431_ok: bool,
    /// Wave 431 subobjects upgrade dual-world empty-gate nav/commands residual pack.
    pub live_subobjects_upgrade_dual_world_empty_gate_nav_commands_wave431_ok: bool,
    /// Wave 431 subobjects upgrade dual-world empty-gate live residual.
    pub live_subobjects_upgrade_dual_world_empty_gate_live_wave431_ok: bool,
    /// Wave 432 unit exit dual-world empty-gate method names residual pack.
    pub live_unit_exit_dual_world_empty_gate_method_names_wave432_ok: bool,
    /// Wave 432 unit exit dual-world empty-gate nav/commands residual pack.
    pub live_unit_exit_dual_world_empty_gate_nav_commands_wave432_ok: bool,
    /// Wave 432 unit exit dual-world empty-gate live residual.
    pub live_unit_exit_dual_world_empty_gate_live_wave432_ok: bool,
    /// Wave 433 owner resolve dual-world empty-gate method names residual pack.
    pub live_owner_resolve_dual_world_empty_gate_method_names_wave433_ok: bool,
    /// Wave 433 owner resolve dual-world empty-gate nav/commands residual pack.
    pub live_owner_resolve_dual_world_empty_gate_nav_commands_wave433_ok: bool,
    /// Wave 433 owner resolve dual-world empty-gate live residual.
    pub live_owner_resolve_dual_world_empty_gate_live_wave433_ok: bool,
    /// Wave 434 spy vision update dual-world empty-gate method names residual pack.
    pub live_spy_vision_update_dual_world_empty_gate_method_names_wave434_ok: bool,
    /// Wave 434 spy vision update dual-world empty-gate nav/commands residual pack.
    pub live_spy_vision_update_dual_world_empty_gate_nav_commands_wave434_ok: bool,
    /// Wave 434 spy vision update dual-world empty-gate live residual.
    pub live_spy_vision_update_dual_world_empty_gate_live_wave434_ok: bool,
    /// Wave 435 overcharge behavior dual-world empty-gate method names residual pack.
    pub live_overcharge_behavior_dual_world_empty_gate_method_names_wave435_ok: bool,
    /// Wave 435 overcharge behavior dual-world empty-gate nav/commands residual pack.
    pub live_overcharge_behavior_dual_world_empty_gate_nav_commands_wave435_ok: bool,
    /// Wave 435 overcharge behavior dual-world empty-gate live residual.
    pub live_overcharge_behavior_dual_world_empty_gate_live_wave435_ok: bool,
    /// Wave 436 tech building behavior dual-world empty-gate method names residual pack.
    pub live_tech_building_behavior_dual_world_empty_gate_method_names_wave436_ok: bool,
    /// Wave 436 tech building behavior dual-world empty-gate nav/commands residual pack.
    pub live_tech_building_behavior_dual_world_empty_gate_nav_commands_wave436_ok: bool,
    /// Wave 436 tech building behavior dual-world empty-gate live residual.
    pub live_tech_building_behavior_dual_world_empty_gate_live_wave436_ok: bool,
    /// Wave 437 power plant upgrade dual-world empty-gate method names residual pack.
    pub live_power_plant_upgrade_dual_world_empty_gate_method_names_wave437_ok: bool,
    /// Wave 437 power plant upgrade dual-world empty-gate nav/commands residual pack.
    pub live_power_plant_upgrade_dual_world_empty_gate_nav_commands_wave437_ok: bool,
    /// Wave 437 power plant upgrade dual-world empty-gate live residual.
    pub live_power_plant_upgrade_dual_world_empty_gate_live_wave437_ok: bool,
    /// Wave 438 stealth upgrade dual-world empty-gate method names residual pack.
    pub live_stealth_upgrade_dual_world_empty_gate_method_names_wave438_ok: bool,
    /// Wave 438 stealth upgrade dual-world empty-gate nav/commands residual pack.
    pub live_stealth_upgrade_dual_world_empty_gate_nav_commands_wave438_ok: bool,
    /// Wave 438 stealth upgrade dual-world empty-gate live residual.
    pub live_stealth_upgrade_dual_world_empty_gate_live_wave438_ok: bool,
    /// Wave 439 aurora strike power dual-world empty-gate method names residual pack.
    pub live_aurora_strike_power_dual_world_empty_gate_method_names_wave439_ok: bool,
    /// Wave 439 aurora strike power dual-world empty-gate nav/commands residual pack.
    pub live_aurora_strike_power_dual_world_empty_gate_nav_commands_wave439_ok: bool,
    /// Wave 439 aurora strike power dual-world empty-gate live residual.
    pub live_aurora_strike_power_dual_world_empty_gate_live_wave439_ok: bool,
    /// Wave 440 carpet bomb power dual-world empty-gate method names residual pack.
    pub live_carpet_bomb_power_dual_world_empty_gate_method_names_wave440_ok: bool,
    /// Wave 440 carpet bomb power dual-world empty-gate nav/commands residual pack.
    pub live_carpet_bomb_power_dual_world_empty_gate_nav_commands_wave440_ok: bool,
    /// Wave 440 carpet bomb power dual-world empty-gate live residual.
    pub live_carpet_bomb_power_dual_world_empty_gate_live_wave440_ok: bool,
    /// Wave 441 nuclear missile power dual-world empty-gate method names residual pack.
    pub live_nuclear_missile_power_dual_world_empty_gate_method_names_wave441_ok: bool,
    /// Wave 441 nuclear missile power dual-world empty-gate nav/commands residual pack.
    pub live_nuclear_missile_power_dual_world_empty_gate_nav_commands_wave441_ok: bool,
    /// Wave 441 nuclear missile power dual-world empty-gate live residual.
    pub live_nuclear_missile_power_dual_world_empty_gate_live_wave441_ok: bool,
    /// Wave 442 overlord draw dual-world empty-gate method names residual pack.
    pub live_overlord_draw_dual_world_empty_gate_method_names_wave442_ok: bool,
    /// Wave 442 overlord draw dual-world empty-gate nav/commands residual pack.
    pub live_overlord_draw_dual_world_empty_gate_nav_commands_wave442_ok: bool,
    /// Wave 442 overlord draw dual-world empty-gate live residual.
    pub live_overlord_draw_dual_world_empty_gate_live_wave442_ok: bool,
    /// Wave 443 stealth integration dual-world empty-gate method names residual pack.
    pub live_stealth_integration_dual_world_empty_gate_method_names_wave443_ok: bool,
    /// Wave 443 stealth integration dual-world empty-gate nav/commands residual pack.
    pub live_stealth_integration_dual_world_empty_gate_nav_commands_wave443_ok: bool,
    /// Wave 443 stealth integration dual-world empty-gate live residual.
    pub live_stealth_integration_dual_world_empty_gate_live_wave443_ok: bool,
    /// Wave 444 player upgrade manager dual-world empty-gate method names residual pack.
    pub live_player_upgrade_manager_dual_world_empty_gate_method_names_wave444_ok: bool,
    /// Wave 444 player upgrade manager dual-world empty-gate nav/commands residual pack.
    pub live_player_upgrade_manager_dual_world_empty_gate_nav_commands_wave444_ok: bool,
    /// Wave 444 player upgrade manager dual-world empty-gate live residual.
    pub live_player_upgrade_manager_dual_world_empty_gate_live_wave444_ok: bool,
    /// Wave 445 advanced nuggets dual-world empty-gate method names residual pack.
    pub live_advanced_nuggets_dual_world_empty_gate_method_names_wave445_ok: bool,
    /// Wave 445 advanced nuggets dual-world empty-gate nav/commands residual pack.
    pub live_advanced_nuggets_dual_world_empty_gate_nav_commands_wave445_ok: bool,
    /// Wave 445 advanced nuggets dual-world empty-gate live residual.
    pub live_advanced_nuggets_dual_world_empty_gate_live_wave445_ok: bool,
    /// Wave 446 replace object upgrade dual-world empty-gate method names residual pack.
    pub live_replace_object_upgrade_dual_world_empty_gate_method_names_wave446_ok: bool,
    /// Wave 446 replace object upgrade dual-world empty-gate nav/commands residual pack.
    pub live_replace_object_upgrade_dual_world_empty_gate_nav_commands_wave446_ok: bool,
    /// Wave 446 replace object upgrade dual-world empty-gate live residual.
    pub live_replace_object_upgrade_dual_world_empty_gate_live_wave446_ok: bool,
    /// Wave 447 fire spread update dual-world empty-gate method names residual pack.
    pub live_fire_spread_update_dual_world_empty_gate_method_names_wave447_ok: bool,
    /// Wave 447 fire spread update dual-world empty-gate nav/commands residual pack.
    pub live_fire_spread_update_dual_world_empty_gate_nav_commands_wave447_ok: bool,
    /// Wave 447 fire spread update dual-world empty-gate live residual.
    pub live_fire_spread_update_dual_world_empty_gate_live_wave447_ok: bool,
    /// Wave 448 object upgrade batch dual-world empty-gate method names residual pack.
    pub live_object_upgrade_batch_dual_world_empty_gate_method_names_wave448_ok: bool,
    /// Wave 448 object upgrade batch dual-world empty-gate nav/commands residual pack.
    pub live_object_upgrade_batch_dual_world_empty_gate_nav_commands_wave448_ok: bool,
    /// Wave 448 object upgrade batch dual-world empty-gate live residual.
    pub live_object_upgrade_batch_dual_world_empty_gate_live_wave448_ok: bool,
    /// Wave 449 contain module overrides fail-closed method names residual pack.
    pub live_contain_module_overrides_fail_closed_method_names_wave449_ok: bool,
    /// Wave 449 contain module overrides fail-closed nav/commands residual pack.
    pub live_contain_module_overrides_fail_closed_nav_commands_wave449_ok: bool,
    /// Wave 449 contain module overrides fail-closed live residual.
    pub live_contain_module_overrides_fail_closed_live_wave449_ok: bool,
    /// Wave 450 core-sim dual-world empty-gate method names residual pack.
    pub live_core_sim_dual_world_empty_gate_method_names_wave450_ok: bool,
    /// Wave 450 core-sim dual-world empty-gate nav/commands residual pack.
    pub live_core_sim_dual_world_empty_gate_nav_commands_wave450_ok: bool,
    /// Wave 450 core-sim dual-world empty-gate live residual.
    pub live_core_sim_dual_world_empty_gate_live_wave450_ok: bool,
    /// Wave 451 golden mop-up honesty method names residual pack.
    pub live_golden_mopup_honesty_method_names_wave451_ok: bool,
    /// Wave 451 golden mop-up honesty nav/commands residual pack.
    pub live_golden_mopup_honesty_nav_commands_wave451_ok: bool,
    /// Wave 451 golden mop-up honesty live residual.
    pub live_golden_mopup_honesty_live_wave451_ok: bool,
    /// Wave 452 die/command dual-world empty-gate method names residual pack.
    pub live_die_command_dual_world_empty_gate_method_names_wave452_ok: bool,
    /// Wave 452 die/command dual-world empty-gate nav/commands residual pack.
    pub live_die_command_dual_world_empty_gate_nav_commands_wave452_ok: bool,
    /// Wave 452 die/command dual-world empty-gate live residual.
    pub live_die_command_dual_world_empty_gate_live_wave452_ok: bool,
    /// Wave 453 upgrade/behavior dual-world empty-gate method names residual pack.
    pub live_upgrade_behavior_dual_world_empty_gate_method_names_wave453_ok: bool,
    /// Wave 453 upgrade/behavior dual-world empty-gate nav/commands residual pack.
    pub live_upgrade_behavior_dual_world_empty_gate_nav_commands_wave453_ok: bool,
    /// Wave 453 upgrade/behavior dual-world empty-gate live residual.
    pub live_upgrade_behavior_dual_world_empty_gate_live_wave453_ok: bool,
    /// Wave 454 construction placement dual-world empty-gate method names residual pack.
    pub live_construction_placement_dual_world_empty_gate_method_names_wave454_ok: bool,
    /// Wave 454 construction placement dual-world empty-gate nav/commands residual pack.
    pub live_construction_placement_dual_world_empty_gate_nav_commands_wave454_ok: bool,
    /// Wave 454 construction placement dual-world empty-gate live residual.
    pub live_construction_placement_dual_world_empty_gate_live_wave454_ok: bool,
    /// Wave 455 presentation env-only method names residual pack.
    pub live_presentation_env_only_method_names_wave455_ok: bool,
    /// Wave 455 presentation env-only nav/commands residual pack.
    pub live_presentation_env_only_nav_commands_wave455_ok: bool,
    /// Wave 455 presentation env-only live residual.
    pub live_presentation_env_only_live_wave455_ok: bool,
    /// Wave 456 map lighting presentation-only method names residual pack.
    pub map_lighting_presentation_only_method_names_wave456_ok: bool,
    /// Wave 456 map lighting presentation-only nav/commands residual pack.
    pub map_lighting_presentation_only_nav_commands_wave456_ok: bool,
    /// Wave 456 map lighting presentation-only live residual.
    pub map_lighting_presentation_only_live_wave456_ok: bool,
    /// Wave 457 minimap bounds presentation-first method names residual pack.
    pub minimap_bounds_presentation_first_method_names_wave457_ok: bool,
    /// Wave 457 minimap bounds presentation-first nav/commands residual pack.
    pub minimap_bounds_presentation_first_nav_commands_wave457_ok: bool,
    /// Wave 457 minimap bounds presentation-first live residual.
    pub minimap_bounds_presentation_first_live_wave457_ok: bool,
    /// Wave 458 bootstrap camera no-live dual-read method names residual pack.
    pub bootstrap_camera_no_live_dual_read_method_names_wave458_ok: bool,
    /// Wave 458 bootstrap camera no-live dual-read nav/commands residual pack.
    pub bootstrap_camera_no_live_dual_read_nav_commands_wave458_ok: bool,
    /// Wave 458 bootstrap camera no-live dual-read live residual.
    pub bootstrap_camera_no_live_dual_read_live_wave458_ok: bool,
    /// Wave 459 terrain visual presentation-only method names residual pack.
    pub terrain_visual_presentation_only_method_names_wave459_ok: bool,
    /// Wave 459 terrain visual presentation-only nav/commands residual pack.
    pub terrain_visual_presentation_only_nav_commands_wave459_ok: bool,
    /// Wave 459 terrain visual presentation-only live residual.
    pub terrain_visual_presentation_only_live_wave459_ok: bool,
    /// Wave 460 camera center presentation height method names residual pack.
    pub camera_center_presentation_height_method_names_wave460_ok: bool,
    /// Wave 460 camera center presentation height nav/commands residual pack.
    pub camera_center_presentation_height_nav_commands_wave460_ok: bool,
    /// Wave 460 camera center presentation height live residual.
    pub camera_center_presentation_height_live_wave460_ok: bool,
    /// Wave 461 presentation world bounds probe method names residual pack.
    pub presentation_world_bounds_probe_method_names_wave461_ok: bool,
    /// Wave 461 presentation world bounds probe nav/commands residual pack.
    pub presentation_world_bounds_probe_nav_commands_wave461_ok: bool,
    /// Wave 461 presentation world bounds probe live residual.
    pub presentation_world_bounds_probe_live_wave461_ok: bool,
    /// Wave 462 render UI pipeline presentation method names residual pack.
    pub render_ui_pipeline_presentation_method_names_wave462_ok: bool,
    /// Wave 462 render UI pipeline presentation nav/commands residual pack.
    pub render_ui_pipeline_presentation_nav_commands_wave462_ok: bool,
    /// Wave 462 render UI pipeline presentation live residual.
    pub render_ui_pipeline_presentation_live_wave462_ok: bool,
    /// Wave 463 production quantity writeback method names residual pack.
    pub production_quantity_writeback_method_names_wave463_ok: bool,
    /// Wave 463 production quantity writeback nav/commands residual pack.
    pub production_quantity_writeback_nav_commands_wave463_ok: bool,
    /// Wave 463 production quantity writeback live residual.
    pub production_quantity_writeback_live_wave463_ok: bool,
    /// Wave 464 production exit delay sole-tick method names residual pack.
    pub production_exit_delay_sole_tick_method_names_wave464_ok: bool,
    /// Wave 464 production exit delay sole-tick nav/commands residual pack.
    pub production_exit_delay_sole_tick_nav_commands_wave464_ok: bool,
    /// Wave 464 production exit delay sole-tick live residual.
    pub production_exit_delay_sole_tick_live_wave464_ok: bool,
    /// Wave 465 minimap heightmap repair presentation-first method names residual pack.
    pub minimap_heightmap_repair_presentation_first_method_names_wave465_ok: bool,
    /// Wave 465 minimap heightmap repair presentation-first nav/commands residual pack.
    pub minimap_heightmap_repair_presentation_first_nav_commands_wave465_ok: bool,
    /// Wave 465 minimap heightmap repair presentation-first live residual.
    pub minimap_heightmap_repair_presentation_first_live_wave465_ok: bool,
    /// Wave 466 presentation env seed gameworld method names residual pack.
    pub presentation_env_seed_gameworld_method_names_wave466_ok: bool,
    /// Wave 466 presentation env seed gameworld nav/commands residual pack.
    pub presentation_env_seed_gameworld_nav_commands_wave466_ok: bool,
    /// Wave 466 presentation env seed gameworld live residual.
    pub presentation_env_seed_gameworld_live_wave466_ok: bool,
    /// Wave 467 presentation env seed mirror last method names residual pack.
    pub presentation_env_seed_mirror_last_method_names_wave467_ok: bool,
    /// Wave 467 presentation env seed mirror last nav/commands residual pack.
    pub presentation_env_seed_mirror_last_nav_commands_wave467_ok: bool,
    /// Wave 467 presentation env seed mirror last live residual.
    pub presentation_env_seed_mirror_last_live_wave467_ok: bool,
    /// Wave 468 minimap reinit instance presentation method names residual pack.
    pub minimap_reinit_instance_presentation_method_names_wave468_ok: bool,
    /// Wave 468 minimap reinit instance presentation nav/commands residual pack.
    pub minimap_reinit_instance_presentation_nav_commands_wave468_ok: bool,
    /// Wave 468 minimap reinit instance presentation live residual.
    pub minimap_reinit_instance_presentation_live_wave468_ok: bool,
    /// Wave 469 pathfind midframe stub removed method names residual pack.
    pub pathfind_midframe_stub_removed_method_names_wave469_ok: bool,
    /// Wave 469 pathfind midframe stub removed nav/commands residual pack.
    pub pathfind_midframe_stub_removed_nav_commands_wave469_ok: bool,
    /// Wave 469 pathfind midframe stub removed live residual.
    pub pathfind_midframe_stub_removed_live_wave469_ok: bool,
    /// Wave 470 projectile authority flare host method names residual pack.
    pub projectile_authority_flare_host_method_names_wave470_ok: bool,
    /// Wave 470 projectile authority flare host nav/commands residual pack.
    pub projectile_authority_flare_host_nav_commands_wave470_ok: bool,
    /// Wave 470 projectile authority flare host live residual.
    pub projectile_authority_flare_host_live_wave470_ok: bool,
    /// Wave 471 engine env free-fn GameLogic-only-seed method names residual pack.
    pub engine_env_free_fn_game_logic_only_seed_method_names_wave471_ok: bool,
    /// Wave 471 engine env free-fn GameLogic-only-seed nav/commands residual pack.
    pub engine_env_free_fn_game_logic_only_seed_nav_commands_wave471_ok: bool,
    /// Wave 471 engine env free-fn GameLogic-only-seed live residual.
    pub engine_env_free_fn_game_logic_only_seed_live_wave471_ok: bool,
    /// Wave 472 dead model preload removed method names residual pack.
    pub dead_model_preload_removed_method_names_wave472_ok: bool,
    /// Wave 472 dead model preload removed nav/commands residual pack.
    pub dead_model_preload_removed_nav_commands_wave472_ok: bool,
    /// Wave 472 dead model preload removed live residual.
    pub dead_model_preload_removed_live_wave472_ok: bool,
    /// Wave 473 camera bootstrap presentation-only method names residual pack.
    pub camera_bootstrap_presentation_only_method_names_wave473_ok: bool,
    /// Wave 473 camera bootstrap presentation-only nav/commands residual pack.
    pub camera_bootstrap_presentation_only_nav_commands_wave473_ok: bool,
    /// Wave 473 camera bootstrap presentation-only live residual.
    pub camera_bootstrap_presentation_only_live_wave473_ok: bool,
    /// Wave 474 ensure presentation env instance method names residual pack.
    pub ensure_presentation_env_instance_method_names_wave474_ok: bool,
    /// Wave 474 ensure presentation env instance nav/commands residual pack.
    pub ensure_presentation_env_instance_nav_commands_wave474_ok: bool,
    /// Wave 474 ensure presentation env instance live residual.
    pub ensure_presentation_env_instance_live_wave474_ok: bool,
    /// Wave 475 map ground no registry pose dual-write method names residual pack.
    pub map_ground_no_registry_pose_dual_write_method_names_wave475_ok: bool,
    /// Wave 475 map ground no registry pose dual-write nav/commands residual pack.
    pub map_ground_no_registry_pose_dual_write_nav_commands_wave475_ok: bool,
    /// Wave 475 map ground no registry pose dual-write live residual.
    pub map_ground_no_registry_pose_dual_write_live_wave475_ok: bool,
    /// Wave 476 named shell host-only tracker method names residual pack.
    pub named_shell_host_only_tracker_method_names_wave476_ok: bool,
    /// Wave 476 named shell host-only tracker nav/commands residual pack.
    pub named_shell_host_only_tracker_nav_commands_wave476_ok: bool,
    /// Wave 476 named shell host-only tracker live residual.
    pub named_shell_host_only_tracker_live_wave476_ok: bool,
    /// Wave 477 production sole-tick no progress stomp method names residual pack.
    pub production_sole_tick_no_progress_stomp_method_names_wave477_ok: bool,
    /// Wave 477 production sole-tick no progress stomp nav/commands residual pack.
    pub production_sole_tick_no_progress_stomp_nav_commands_wave477_ok: bool,
    /// Wave 477 production sole-tick no progress stomp live residual.
    pub production_sole_tick_no_progress_stomp_live_wave477_ok: bool,
    /// Wave 478 construction sole-tick no progress stomp method names residual pack.
    pub construction_sole_tick_no_progress_stomp_method_names_wave478_ok: bool,
    /// Wave 478 construction sole-tick no progress stomp nav/commands residual pack.
    pub construction_sole_tick_no_progress_stomp_nav_commands_wave478_ok: bool,
    /// Wave 478 construction sole-tick no progress stomp live residual.
    pub construction_sole_tick_no_progress_stomp_live_wave478_ok: bool,
    /// Wave 479 special-power sole-tick no cooldown stomp method names residual pack.
    pub special_power_sole_tick_no_cooldown_stomp_method_names_wave479_ok: bool,
    /// Wave 479 special-power sole-tick no cooldown stomp nav/commands residual pack.
    pub special_power_sole_tick_no_cooldown_stomp_nav_commands_wave479_ok: bool,
    /// Wave 479 special-power sole-tick no cooldown stomp live residual.
    pub special_power_sole_tick_no_cooldown_stomp_live_wave479_ok: bool,
    /// Wave 480 production sole-tick exit delay arm method names residual pack.
    pub production_sole_tick_exit_delay_arm_method_names_wave480_ok: bool,
    /// Wave 480 production sole-tick exit delay arm nav/commands residual pack.
    pub production_sole_tick_exit_delay_arm_nav_commands_wave480_ok: bool,
    /// Wave 480 production sole-tick exit delay arm live residual.
    pub production_sole_tick_exit_delay_arm_live_wave480_ok: bool,
    /// Wave 481 sell deconstruction sole-tick no-stomp method names residual pack.
    pub sell_deconstruction_sole_tick_no_stomp_method_names_wave481_ok: bool,
    /// Wave 481 sell deconstruction sole-tick no-stomp nav/commands residual pack.
    pub sell_deconstruction_sole_tick_no_stomp_nav_commands_wave481_ok: bool,
    /// Wave 481 sell deconstruction sole-tick no-stomp live residual.
    pub sell_deconstruction_sole_tick_no_stomp_live_wave481_ok: bool,
    /// Wave 482 sell finish skips topple destroy method names residual pack.
    pub sell_finish_skips_topple_destroy_method_names_wave482_ok: bool,
    /// Wave 482 sell finish skips topple destroy nav/commands residual pack.
    pub sell_finish_skips_topple_destroy_nav_commands_wave482_ok: bool,
    /// Wave 482 sell finish skips topple destroy live residual.
    pub sell_finish_skips_topple_destroy_live_wave482_ok: bool,
    /// Wave 483 production upgrade complete queue refresh method names residual pack.
    pub production_upgrade_complete_queue_refresh_method_names_wave483_ok: bool,
    /// Wave 483 production upgrade complete queue refresh nav/commands residual pack.
    pub production_upgrade_complete_queue_refresh_nav_commands_wave483_ok: bool,
    /// Wave 483 production upgrade complete queue refresh live residual.
    pub production_upgrade_complete_queue_refresh_live_wave483_ok: bool,
    /// Wave 484 cancel_all production queue refresh method names residual pack.
    pub cancel_all_production_queue_refresh_method_names_wave484_ok: bool,
    /// Wave 484 cancel_all production queue refresh nav/commands residual pack.
    pub cancel_all_production_queue_refresh_nav_commands_wave484_ok: bool,
    /// Wave 484 cancel_all production queue refresh live residual.
    pub cancel_all_production_queue_refresh_live_wave484_ok: bool,
    /// Wave 485 cancel clears exit delay method names residual pack.
    pub cancel_clears_exit_delay_method_names_wave485_ok: bool,
    /// Wave 485 cancel clears exit delay nav/commands residual pack.
    pub cancel_clears_exit_delay_nav_commands_wave485_ok: bool,
    /// Wave 485 cancel clears exit delay live residual.
    pub cancel_clears_exit_delay_live_wave485_ok: bool,
    /// Wave 486 production door model condition log method names residual pack.
    pub production_door_model_condition_log_method_names_wave486_ok: bool,
    /// Wave 486 production door model condition log nav/commands residual pack.
    pub production_door_model_condition_log_nav_commands_wave486_ok: bool,
    /// Wave 486 production door model condition log live residual.
    pub production_door_model_condition_log_live_wave486_ok: bool,
    /// Wave 487 combat model condition channel method names residual pack.
    pub combat_model_condition_channel_method_names_wave487_ok: bool,
    /// Wave 487 combat model condition channel nav/commands residual pack.
    pub combat_model_condition_channel_nav_commands_wave487_ok: bool,
    /// Wave 487 combat model condition channel live residual.
    pub combat_model_condition_channel_live_wave487_ok: bool,
    /// Wave 488 entity presentation model condition method names residual pack.
    pub entity_presentation_model_condition_method_names_wave488_ok: bool,
    /// Wave 488 entity presentation model condition nav/commands residual pack.
    pub entity_presentation_model_condition_nav_commands_wave488_ok: bool,
    /// Wave 488 entity presentation model condition live residual.
    pub entity_presentation_model_condition_live_wave488_ok: bool,
    /// Wave 489 entity presentation combat UI method names residual pack.
    pub entity_presentation_combat_ui_method_names_wave489_ok: bool,
    /// Wave 489 entity presentation combat UI nav/commands residual pack.
    pub entity_presentation_combat_ui_nav_commands_wave489_ok: bool,
    /// Wave 489 entity presentation combat UI live residual.
    pub entity_presentation_combat_ui_live_wave489_ok: bool,
    /// Wave 490 entity presentation structure UI method names residual pack.
    pub entity_presentation_structure_ui_method_names_wave490_ok: bool,
    /// Wave 490 entity presentation structure UI nav/commands residual pack.
    pub entity_presentation_structure_ui_nav_commands_wave490_ok: bool,
    /// Wave 490 entity presentation structure UI live residual.
    pub entity_presentation_structure_ui_live_wave490_ok: bool,
    /// Wave 491 presentation mesh sold condition method names residual pack.
    pub presentation_mesh_sold_condition_method_names_wave491_ok: bool,
    /// Wave 491 presentation mesh sold condition nav/commands residual pack.
    pub presentation_mesh_sold_condition_nav_commands_wave491_ok: bool,
    /// Wave 491 presentation mesh sold condition live residual.
    pub presentation_mesh_sold_condition_live_wave491_ok: bool,
    /// Wave 492 entity presentation mesh/FOW method names residual pack.
    pub entity_presentation_mesh_fow_method_names_wave492_ok: bool,
    /// Wave 492 entity presentation mesh/FOW nav/commands residual pack.
    pub entity_presentation_mesh_fow_nav_commands_wave492_ok: bool,
    /// Wave 492 entity presentation mesh/FOW live residual.
    pub entity_presentation_mesh_fow_live_wave492_ok: bool,
    /// Wave 493 entity presentation ground/bridge method names residual pack.
    pub entity_presentation_ground_bridge_method_names_wave493_ok: bool,
    /// Wave 493 entity presentation ground/bridge nav/commands residual pack.
    pub entity_presentation_ground_bridge_nav_commands_wave493_ok: bool,
    /// Wave 493 entity presentation ground/bridge live residual.
    pub entity_presentation_ground_bridge_live_wave493_ok: bool,
    /// Wave 494 presentation mesh turret method names residual pack.
    pub presentation_mesh_turret_method_names_wave494_ok: bool,
    /// Wave 494 presentation mesh turret nav/commands residual pack.
    pub presentation_mesh_turret_nav_commands_wave494_ok: bool,
    /// Wave 494 presentation mesh turret live residual.
    pub presentation_mesh_turret_live_wave494_ok: bool,
    /// Wave 495 presentation mesh combat flags method names residual pack.
    pub presentation_mesh_combat_flags_method_names_wave495_ok: bool,
    /// Wave 495 presentation mesh combat flags nav/commands residual pack.
    pub presentation_mesh_combat_flags_nav_commands_wave495_ok: bool,
    /// Wave 495 presentation mesh combat flags live residual.
    pub presentation_mesh_combat_flags_live_wave495_ok: bool,
    /// Wave 496 presentation mesh door phase method names residual pack.
    pub presentation_mesh_door_phase_method_names_wave496_ok: bool,
    /// Wave 496 presentation mesh door phase nav/commands residual pack.
    pub presentation_mesh_door_phase_nav_commands_wave496_ok: bool,
    /// Wave 496 presentation mesh door phase live residual.
    pub presentation_mesh_door_phase_live_wave496_ok: bool,
    /// Wave 497 presentation mesh condition resolve method names residual pack.
    pub presentation_mesh_condition_resolve_method_names_wave497_ok: bool,
    /// Wave 497 presentation mesh condition resolve nav/commands residual pack.
    pub presentation_mesh_condition_resolve_nav_commands_wave497_ok: bool,
    /// Wave 497 presentation mesh condition resolve live residual.
    pub presentation_mesh_condition_resolve_live_wave497_ok: bool,
    /// Wave 498 presentation host FX overlay method names residual pack.
    pub presentation_host_fx_overlay_method_names_wave498_ok: bool,
    /// Wave 498 presentation host FX overlay nav/commands residual pack.
    pub presentation_host_fx_overlay_nav_commands_wave498_ok: bool,
    /// Wave 498 presentation host FX overlay live residual.
    pub presentation_host_fx_overlay_live_wave498_ok: bool,
    /// Wave 499 presentation poison/defector tint method names residual pack.
    pub presentation_poison_defector_tint_method_names_wave499_ok: bool,
    /// Wave 499 presentation poison/defector tint nav/commands residual pack.
    pub presentation_poison_defector_tint_nav_commands_wave499_ok: bool,
    /// Wave 499 presentation poison/defector tint live residual.
    pub presentation_poison_defector_tint_live_wave499_ok: bool,
    /// Wave 500 presentation object FX particles method names residual pack.
    pub presentation_object_fx_particles_method_names_wave500_ok: bool,
    /// Wave 500 presentation object FX particles nav/commands residual pack.
    pub presentation_object_fx_particles_nav_commands_wave500_ok: bool,
    /// Wave 500 presentation object FX particles live residual.
    pub presentation_object_fx_particles_live_wave500_ok: bool,
    /// Wave 501 presentation mesh deploy/radar method names residual pack.
    pub presentation_mesh_deploy_radar_method_names_wave501_ok: bool,
    /// Wave 501 presentation mesh deploy/radar nav/commands residual pack.
    pub presentation_mesh_deploy_radar_nav_commands_wave501_ok: bool,
    /// Wave 501 presentation mesh deploy/radar live residual.
    pub presentation_mesh_deploy_radar_live_wave501_ok: bool,
    /// Wave 502 presentation stealth mesh method names residual pack.
    pub presentation_stealth_mesh_method_names_wave502_ok: bool,
    /// Wave 502 presentation stealth mesh nav/commands residual pack.
    pub presentation_stealth_mesh_nav_commands_wave502_ok: bool,
    /// Wave 502 presentation stealth mesh live residual.
    pub presentation_stealth_mesh_live_wave502_ok: bool,
    /// Wave 503 presentation construction/disguise method names residual pack.
    pub presentation_construction_disguise_method_names_wave503_ok: bool,
    /// Wave 503 presentation construction/disguise nav/commands residual pack.
    pub presentation_construction_disguise_nav_commands_wave503_ok: bool,
    /// Wave 503 presentation construction/disguise live residual.
    pub presentation_construction_disguise_live_wave503_ok: bool,
    /// Wave 504 presentation garrison/contain method names residual pack.
    pub presentation_garrison_contain_method_names_wave504_ok: bool,
    /// Wave 504 presentation garrison/contain nav/commands residual pack.
    pub presentation_garrison_contain_nav_commands_wave504_ok: bool,
    /// Wave 504 presentation garrison/contain live residual.
    pub presentation_garrison_contain_live_wave504_ok: bool,
    /// Wave 505 presentation air/parachute method names residual pack.
    pub presentation_air_parachute_method_names_wave505_ok: bool,
    /// Wave 505 presentation air/parachute nav/commands residual pack.
    pub presentation_air_parachute_nav_commands_wave505_ok: bool,
    /// Wave 505 presentation air/parachute live residual.
    pub presentation_air_parachute_live_wave505_ok: bool,
    /// Wave 506 presentation weaponset veterancy method names residual pack.
    pub presentation_weaponset_veterancy_method_names_wave506_ok: bool,
    /// Wave 506 presentation weaponset veterancy nav/commands residual pack.
    pub presentation_weaponset_veterancy_nav_commands_wave506_ok: bool,
    /// Wave 506 presentation weaponset veterancy live residual.
    pub presentation_weaponset_veterancy_live_wave506_ok: bool,
    /// Wave 507 presentation water/rider method names residual pack.
    pub presentation_water_rider_method_names_wave507_ok: bool,
    /// Wave 507 presentation water/rider nav/commands residual pack.
    pub presentation_water_rider_nav_commands_wave507_ok: bool,
    /// Wave 507 presentation water/rider live residual.
    pub presentation_water_rider_live_wave507_ok: bool,
    /// Wave 508 presentation body/disguise/stun method names residual pack.
    pub presentation_body_disguise_stun_method_names_wave508_ok: bool,
    /// Wave 508 presentation body/disguise/stun nav/commands residual pack.
    pub presentation_body_disguise_stun_nav_commands_wave508_ok: bool,
    /// Wave 508 presentation body/disguise/stun live residual.
    pub presentation_body_disguise_stun_live_wave508_ok: bool,
    /// Wave 509 presentation topple/freefall/weather method names residual pack.
    pub presentation_topple_freefall_weather_method_names_wave509_ok: bool,
    /// Wave 509 presentation topple/freefall/weather nav/commands residual pack.
    pub presentation_topple_freefall_weather_nav_commands_wave509_ok: bool,
    /// Wave 509 presentation topple/freefall/weather live residual.
    pub presentation_topple_freefall_weather_live_wave509_ok: bool,
    /// Wave 510 presentation capture/load/overcharge method names residual pack.
    pub presentation_capture_load_overcharge_method_names_wave510_ok: bool,
    /// Wave 510 presentation capture/load/overcharge nav/commands residual pack.
    pub presentation_capture_load_overcharge_nav_commands_wave510_ok: bool,
    /// Wave 510 presentation capture/load/overcharge live residual.
    pub presentation_capture_load_overcharge_live_wave510_ok: bool,
    /// Wave 511 presentation burn/cheer/carry method names residual pack.
    pub presentation_burn_cheer_carry_method_names_wave511_ok: bool,
    /// Wave 511 presentation burn/cheer/carry nav/commands residual pack.
    pub presentation_burn_cheer_carry_nav_commands_wave511_ok: bool,
    /// Wave 511 presentation burn/cheer/carry live residual.
    pub presentation_burn_cheer_carry_live_wave511_ok: bool,
    /// Wave 512 presentation fire/prone/turret method names residual pack.
    pub presentation_fire_prone_turret_method_names_wave512_ok: bool,
    /// Wave 512 presentation fire/prone/turret nav/commands residual pack.
    pub presentation_fire_prone_turret_nav_commands_wave512_ok: bool,
    /// Wave 512 presentation fire/prone/turret live residual.
    pub presentation_fire_prone_turret_live_wave512_ok: bool,
    /// Wave 513 presentation jam/die/reload/pack method names residual pack.
    pub presentation_jam_die_reload_pack_method_names_wave513_ok: bool,
    /// Wave 513 presentation jam/die/reload/pack nav/commands residual pack.
    pub presentation_jam_die_reload_pack_nav_commands_wave513_ok: bool,
    /// Wave 513 presentation jam/die/reload/pack live residual.
    pub presentation_jam_die_reload_pack_live_wave513_ok: bool,
    /// Wave 514 presentation emoticon float method names residual pack.
    pub presentation_emoticon_float_method_names_wave514_ok: bool,
    /// Wave 514 presentation emoticon float nav/commands residual pack.
    pub presentation_emoticon_float_nav_commands_wave514_ok: bool,
    /// Wave 514 presentation emoticon float live residual.
    pub presentation_emoticon_float_live_wave514_ok: bool,
    /// Wave 515 presentation surrender/formation method names residual pack.
    pub presentation_surrender_formation_method_names_wave515_ok: bool,
    /// Wave 515 presentation surrender/formation nav/commands residual pack.
    pub presentation_surrender_formation_nav_commands_wave515_ok: bool,
    /// Wave 515 presentation surrender/formation live residual.
    pub presentation_surrender_formation_live_wave515_ok: bool,
    /// Wave 516 presentation formation-link method names residual pack.
    pub presentation_formation_link_method_names_wave516_ok: bool,
    /// Wave 516 presentation formation-link nav/commands residual pack.
    pub presentation_formation_link_nav_commands_wave516_ok: bool,
    /// Wave 516 presentation formation-link live residual.
    pub presentation_formation_link_live_wave516_ok: bool,
    /// Wave 517 presentation weapon-fire-slot method names residual pack.
    pub presentation_weapon_fire_slot_method_names_wave517_ok: bool,
    /// Wave 517 presentation weapon-fire-slot nav/commands residual pack.
    pub presentation_weapon_fire_slot_nav_commands_wave517_ok: bool,
    /// Wave 517 presentation weapon-fire-slot live residual.
    pub presentation_weapon_fire_slot_live_wave517_ok: bool,
    /// Wave 518 presentation weaponset/enemy-near method names residual pack.
    pub presentation_weaponset_enemy_near_method_names_wave518_ok: bool,
    /// Wave 518 presentation weaponset/enemy-near nav/commands residual pack.
    pub presentation_weaponset_enemy_near_nav_commands_wave518_ok: bool,
    /// Wave 518 presentation weaponset/enemy-near live residual.
    pub presentation_weaponset_enemy_near_live_wave518_ok: bool,
    /// Wave 519 presentation shock/power/jet method names residual pack.
    pub presentation_shock_power_jet_method_names_wave519_ok: bool,
    /// Wave 519 presentation shock/power/jet nav/commands residual pack.
    pub presentation_shock_power_jet_nav_commands_wave519_ok: bool,
    /// Wave 519 presentation shock/power/jet live residual.
    pub presentation_shock_power_jet_live_wave519_ok: bool,
    /// Wave 520 presentation anim-steer method names residual pack.
    pub presentation_anim_steer_method_names_wave520_ok: bool,
    /// Wave 520 presentation anim-steer nav/commands residual pack.
    pub presentation_anim_steer_nav_commands_wave520_ok: bool,
    /// Wave 520 presentation anim-steer live residual.
    pub presentation_anim_steer_live_wave520_ok: bool,
    /// Wave 521 presentation dock/rider method names residual pack.
    pub presentation_dock_rider_method_names_wave521_ok: bool,
    /// Wave 521 presentation dock/rider nav/commands residual pack.
    pub presentation_dock_rider_nav_commands_wave521_ok: bool,
    /// Wave 521 presentation dock/rider live residual.
    pub presentation_dock_rider_live_wave521_ok: bool,
    /// Wave 522 presentation cliff/flood method names residual pack.
    pub presentation_cliff_flood_method_names_wave522_ok: bool,
    /// Wave 522 presentation cliff/flood nav/commands residual pack.
    pub presentation_cliff_flood_nav_commands_wave522_ok: bool,
    /// Wave 522 presentation cliff/flood live residual.
    pub presentation_cliff_flood_live_wave522_ok: bool,
    /// Wave 523 presentation second-life/stun method names residual pack.
    pub presentation_second_life_stun_method_names_wave523_ok: bool,
    /// Wave 523 presentation second-life/stun nav/commands residual pack.
    pub presentation_second_life_stun_nav_commands_wave523_ok: bool,
    /// Wave 523 presentation second-life/stun live residual.
    pub presentation_second_life_stun_live_wave523_ok: bool,
    /// Wave 524 presentation multi-door/smolder method names residual pack.
    pub presentation_multi_door_smolder_method_names_wave524_ok: bool,
    /// Wave 524 presentation multi-door/smolder nav/commands residual pack.
    pub presentation_multi_door_smolder_nav_commands_wave524_ok: bool,
    /// Wave 524 presentation multi-door/smolder live residual.
    pub presentation_multi_door_smolder_live_wave524_ok: bool,
    /// Wave 525 presentation crush/user method names residual pack.
    pub presentation_crush_user_method_names_wave525_ok: bool,
    /// Wave 525 presentation crush/user nav/commands residual pack.
    pub presentation_crush_user_nav_commands_wave525_ok: bool,
    /// Wave 525 presentation crush/user live residual.
    pub presentation_crush_user_live_wave525_ok: bool,
    /// Wave 526 presentation move/attack helper method names residual pack.
    pub presentation_move_attack_helper_method_names_wave526_ok: bool,
    /// Wave 526 presentation move/attack helper nav/commands residual pack.
    pub presentation_move_attack_helper_nav_commands_wave526_ok: bool,
    /// Wave 526 presentation move/attack helper live residual.
    pub presentation_move_attack_helper_live_wave526_ok: bool,
    /// Wave 527 presentation FireSound audio method names residual pack.
    pub presentation_firesound_audio_method_names_wave527_ok: bool,
    /// Wave 527 presentation FireSound audio nav/commands residual pack.
    pub presentation_firesound_audio_nav_commands_wave527_ok: bool,
    /// Wave 527 presentation FireSound audio live residual.
    pub presentation_firesound_audio_live_wave527_ok: bool,
    /// Wave 528 presentation FireSound stop method names residual pack.
    pub presentation_firesound_stop_method_names_wave528_ok: bool,
    /// Wave 528 presentation FireSound stop nav/commands residual pack.
    pub presentation_firesound_stop_nav_commands_wave528_ok: bool,
    /// Wave 528 presentation FireSound stop live residual.
    pub presentation_firesound_stop_live_wave528_ok: bool,
    /// Wave 529 presentation radar/EVA audio method names residual pack.
    pub presentation_radar_eva_audio_method_names_wave529_ok: bool,
    /// Wave 529 presentation radar/EVA audio nav/commands residual pack.
    pub presentation_radar_eva_audio_nav_commands_wave529_ok: bool,
    /// Wave 529 presentation radar/EVA audio live residual.
    pub presentation_radar_eva_audio_live_wave529_ok: bool,
    /// Wave 530 presentation capture audio method names residual pack.
    pub presentation_capture_audio_method_names_wave530_ok: bool,
    /// Wave 530 presentation capture audio nav/commands residual pack.
    pub presentation_capture_audio_nav_commands_wave530_ok: bool,
    /// Wave 530 presentation capture audio live residual.
    pub presentation_capture_audio_live_wave530_ok: bool,
    pub command_integration_presentation_fill_method_names_wave531_ok: bool,
    pub command_integration_presentation_fill_nav_commands_wave531_ok: bool,
    pub command_integration_presentation_fill_live_wave531_ok: bool,
    pub presentation_firesound_drain_sibling_method_names_wave532_ok: bool,
    pub presentation_firesound_drain_sibling_nav_commands_wave532_ok: bool,
    pub presentation_firesound_drain_sibling_live_wave532_ok: bool,
    pub presentation_eva_pulse_audio_method_names_wave533_ok: bool,
    pub presentation_eva_pulse_audio_nav_commands_wave533_ok: bool,
    pub presentation_eva_pulse_audio_live_wave533_ok: bool,
    pub presentation_eva_full_matrix_method_names_wave534_ok: bool,
    pub presentation_eva_full_matrix_nav_commands_wave534_ok: bool,
    pub presentation_eva_full_matrix_live_wave534_ok: bool,
    pub presentation_particle_spawn_audio_method_names_wave535_ok: bool,
    pub presentation_particle_spawn_audio_nav_commands_wave535_ok: bool,
    pub presentation_particle_spawn_audio_live_wave535_ok: bool,
    pub presentation_eva_client_dispatch_method_names_wave536_ok: bool,
    pub presentation_eva_client_dispatch_nav_commands_wave536_ok: bool,
    pub presentation_eva_client_dispatch_live_wave536_ok: bool,
    pub presentation_eva_alert_counter_dedupe_method_names_wave537_ok: bool,
    pub presentation_eva_alert_counter_dedupe_nav_commands_wave537_ok: bool,
    pub presentation_eva_alert_counter_dedupe_live_wave537_ok: bool,
    pub presentation_alliance_notify_method_names_wave538_ok: bool,
    pub presentation_alliance_notify_nav_commands_wave538_ok: bool,
    pub presentation_alliance_notify_live_wave538_ok: bool,
    pub presentation_defeat_notify_method_names_wave539_ok: bool,
    pub presentation_defeat_notify_nav_commands_wave539_ok: bool,
    pub presentation_defeat_notify_live_wave539_ok: bool,
    pub presentation_camera_shell_flag_method_names_wave540_ok: bool,
    pub presentation_camera_shell_flag_nav_commands_wave540_ok: bool,
    pub presentation_camera_shell_flag_live_wave540_ok: bool,
    pub rmb_presentation_no_dual_read_method_names_wave541_ok: bool,
    pub rmb_presentation_no_dual_read_nav_commands_wave541_ok: bool,
    pub rmb_presentation_no_dual_read_live_wave541_ok: bool,
    pub presentation_mouse_and_defeat_gate_method_names_wave542_ok: bool,
    pub presentation_mouse_and_defeat_gate_nav_commands_wave542_ok: bool,
    pub presentation_mouse_and_defeat_gate_live_wave542_ok: bool,
    pub ui_selected_presentation_fail_closed_method_names_wave543_ok: bool,
    pub ui_selected_presentation_fail_closed_nav_commands_wave543_ok: bool,
    pub ui_selected_presentation_fail_closed_live_wave543_ok: bool,
    pub ui_selection_seed_presentation_fail_closed_method_names_wave544_ok: bool,
    pub ui_selection_seed_presentation_fail_closed_nav_commands_wave544_ok: bool,
    pub ui_selection_seed_presentation_fail_closed_live_wave544_ok: bool,
    pub save_restart_presentation_fail_closed_method_names_wave545_ok: bool,
    pub save_restart_presentation_fail_closed_nav_commands_wave545_ok: bool,
    pub save_restart_presentation_fail_closed_live_wave545_ok: bool,
    pub host_status_map_presentation_fail_closed_method_names_wave546_ok: bool,
    pub host_status_map_presentation_fail_closed_nav_commands_wave546_ok: bool,
    pub host_status_map_presentation_fail_closed_live_wave546_ok: bool,
    pub host_status_selected_presentation_fail_closed_method_names_wave547_ok: bool,
    pub host_status_selected_presentation_fail_closed_nav_commands_wave547_ok: bool,
    pub host_status_selected_presentation_fail_closed_live_wave547_ok: bool,
    pub camera_follow_presentation_fail_closed_method_names_wave548_ok: bool,
    pub camera_follow_presentation_fail_closed_nav_commands_wave548_ok: bool,
    pub camera_follow_presentation_fail_closed_live_wave548_ok: bool,
    pub ui_player_info_presentation_fail_closed_method_names_wave549_ok: bool,
    pub ui_player_info_presentation_fail_closed_nav_commands_wave549_ok: bool,
    pub ui_player_info_presentation_fail_closed_live_wave549_ok: bool,
    pub visual_speed_presentation_helper_method_names_wave550_ok: bool,
    pub visual_speed_presentation_helper_nav_commands_wave550_ok: bool,
    pub visual_speed_presentation_helper_live_wave550_ok: bool,
    pub time_frozen_presentation_helper_method_names_wave551_ok: bool,
    pub time_frozen_presentation_helper_nav_commands_wave551_ok: bool,
    pub time_frozen_presentation_helper_live_wave551_ok: bool,
    pub shell_bypass_presentation_helper_method_names_wave552_ok: bool,
    pub shell_bypass_presentation_helper_nav_commands_wave552_ok: bool,
    pub shell_bypass_presentation_helper_live_wave552_ok: bool,
    pub play_time_local_player_presentation_helper_method_names_wave553_ok: bool,
    pub play_time_local_player_presentation_helper_nav_commands_wave553_ok: bool,
    pub play_time_local_player_presentation_helper_live_wave553_ok: bool,
    pub map_difficulty_presentation_helper_method_names_wave554_ok: bool,
    pub map_difficulty_presentation_helper_nav_commands_wave554_ok: bool,
    pub map_difficulty_presentation_helper_live_wave554_ok: bool,
    pub science_team_presentation_helper_method_names_wave555_ok: bool,
    pub science_team_presentation_helper_nav_commands_wave555_ok: bool,
    pub science_team_presentation_helper_live_wave555_ok: bool,
    pub victory_presentation_helper_method_names_wave556_ok: bool,
    pub victory_presentation_helper_nav_commands_wave556_ok: bool,
    pub victory_presentation_helper_live_wave556_ok: bool,
    pub replay_presentation_helper_method_names_wave557_ok: bool,
    pub replay_presentation_helper_nav_commands_wave557_ok: bool,
    pub replay_presentation_helper_live_wave557_ok: bool,
    pub diplomacy_presentation_helper_method_names_wave558_ok: bool,
    pub diplomacy_presentation_helper_nav_commands_wave558_ok: bool,
    pub diplomacy_presentation_helper_live_wave558_ok: bool,
    pub presentation_honesty_align_method_names_wave559_ok: bool,
    pub presentation_honesty_align_nav_commands_wave559_ok: bool,
    pub presentation_honesty_align_live_wave559_ok: bool,
    pub logic_frame_presentation_helper_method_names_wave560_ok: bool,
    pub logic_frame_presentation_helper_nav_commands_wave560_ok: bool,
    pub logic_frame_presentation_helper_live_wave560_ok: bool,
    pub logic_steps_presentation_helper_method_names_wave561_ok: bool,
    pub logic_steps_presentation_helper_nav_commands_wave561_ok: bool,
    pub logic_steps_presentation_helper_live_wave561_ok: bool,
    pub combat_kill_particle_observe_method_names_wave562_ok: bool,
    pub combat_kill_particle_observe_nav_commands_wave562_ok: bool,
    pub combat_kill_particle_observe_live_wave562_ok: bool,
    pub template_name_presentation_helper_method_names_wave563_ok: bool,
    pub template_name_presentation_helper_nav_commands_wave563_ok: bool,
    pub template_name_presentation_helper_live_wave563_ok: bool,
    pub fixed_step_diag_presentation_helper_method_names_wave564_ok: bool,
    pub fixed_step_diag_presentation_helper_nav_commands_wave564_ok: bool,
    pub fixed_step_diag_presentation_helper_live_wave564_ok: bool,
    pub construct_template_presentation_helper_method_names_wave565_ok: bool,
    pub construct_template_presentation_helper_nav_commands_wave565_ok: bool,
    pub construct_template_presentation_helper_live_wave565_ok: bool,
    pub boot_ui_message_helper_method_names_wave566_ok: bool,
    pub boot_ui_message_helper_nav_commands_wave566_ok: bool,
    pub boot_ui_message_helper_live_wave566_ok: bool,
    pub boot_movie_helper_method_names_wave567_ok: bool,
    pub boot_movie_helper_nav_commands_wave567_ok: bool,
    pub boot_movie_helper_live_wave567_ok: bool,
    pub script_fps_helper_method_names_wave568_ok: bool,
    pub script_fps_helper_nav_commands_wave568_ok: bool,
    pub script_fps_helper_live_wave568_ok: bool,
    pub defeat_alliance_helper_method_names_wave569_ok: bool,
    pub defeat_alliance_helper_nav_commands_wave569_ok: bool,
    pub defeat_alliance_helper_live_wave569_ok: bool,
    pub script_msg_helper_method_names_wave570_ok: bool,
    pub script_msg_helper_nav_commands_wave570_ok: bool,
    pub script_msg_helper_live_wave570_ok: bool,
    pub popup_music_helper_method_names_wave571_ok: bool,
    pub popup_music_helper_nav_commands_wave571_ok: bool,
    pub popup_music_helper_live_wave571_ok: bool,
    pub boot_camera_helper_method_names_wave572_ok: bool,
    pub boot_camera_helper_nav_commands_wave572_ok: bool,
    pub boot_camera_helper_live_wave572_ok: bool,
    pub boot_player_info_helper_method_names_wave573_ok: bool,
    pub boot_player_info_helper_nav_commands_wave573_ok: bool,
    pub boot_player_info_helper_live_wave573_ok: bool,
    pub boot_local_player_helper_method_names_wave574_ok: bool,
    pub boot_local_player_helper_nav_commands_wave574_ok: bool,
    pub boot_local_player_helper_live_wave574_ok: bool,
    pub host_pause_team_helper_method_names_wave575_ok: bool,
    pub host_pause_team_helper_nav_commands_wave575_ok: bool,
    pub host_pause_team_helper_live_wave575_ok: bool,
    pub host_command_flush_helper_method_names_wave576_ok: bool,
    pub host_command_flush_helper_nav_commands_wave576_ok: bool,
    pub host_command_flush_helper_live_wave576_ok: bool,
    pub host_camera_start_helper_method_names_wave577_ok: bool,
    pub host_camera_start_helper_nav_commands_wave577_ok: bool,
    pub host_camera_start_helper_live_wave577_ok: bool,
    pub host_silent_command_peel_method_names_wave578_ok: bool,
    pub host_silent_command_peel_nav_commands_wave578_ok: bool,
    pub host_silent_command_peel_live_wave578_ok: bool,
    pub host_selection_map_helper_method_names_wave579_ok: bool,
    pub host_selection_map_helper_nav_commands_wave579_ok: bool,
    pub host_selection_map_helper_live_wave579_ok: bool,
    pub host_cancel_selection_helper_method_names_wave580_ok: bool,
    pub host_cancel_selection_helper_nav_commands_wave580_ok: bool,
    pub host_cancel_selection_helper_live_wave580_ok: bool,
    pub host_template_spawn_helper_method_names_wave581_ok: bool,
    pub host_template_spawn_helper_nav_commands_wave581_ok: bool,
    pub host_template_spawn_helper_live_wave581_ok: bool,
    pub host_enqueue_shell_cmd_helper_method_names_wave582_ok: bool,
    pub host_enqueue_shell_cmd_helper_nav_commands_wave582_ok: bool,
    pub host_enqueue_shell_cmd_helper_live_wave582_ok: bool,
    pub host_runtime_cmd_helper_method_names_wave583_ok: bool,
    pub host_runtime_cmd_helper_nav_commands_wave583_ok: bool,
    pub host_runtime_cmd_helper_live_wave583_ok: bool,
    pub host_tick_mutation_helper_method_names_wave584_ok: bool,
    pub host_tick_mutation_helper_nav_commands_wave584_ok: bool,
    pub host_tick_mutation_helper_live_wave584_ok: bool,
    pub host_ui_shell_world_helper_method_names_wave585_ok: bool,
    pub host_ui_shell_world_helper_nav_commands_wave585_ok: bool,
    pub host_ui_shell_world_helper_live_wave585_ok: bool,
    pub host_game_client_shell_tick_helper_method_names_wave586_ok: bool,
    pub host_game_client_shell_tick_helper_nav_commands_wave586_ok: bool,
    pub host_game_client_shell_tick_helper_live_wave586_ok: bool,
    pub host_game_client_device_tick_helper_method_names_wave587_ok: bool,
    pub host_game_client_device_tick_helper_nav_commands_wave587_ok: bool,
    pub host_game_client_device_tick_helper_live_wave587_ok: bool,
    pub host_game_client_menu_shell_helper_method_names_wave588_ok: bool,
    pub host_game_client_menu_shell_helper_nav_commands_wave588_ok: bool,
    pub host_game_client_menu_shell_helper_live_wave588_ok: bool,
    pub host_presentation_finalize_helper_method_names_wave589_ok: bool,
    pub host_presentation_finalize_helper_nav_commands_wave589_ok: bool,
    pub host_presentation_finalize_helper_live_wave589_ok: bool,
    pub host_presentation_seed_helper_method_names_wave590_ok: bool,
    pub host_presentation_seed_helper_nav_commands_wave590_ok: bool,
    pub host_presentation_seed_helper_live_wave590_ok: bool,
    pub host_render_ui_presentation_helper_method_names_wave591_ok: bool,
    pub host_render_ui_presentation_helper_nav_commands_wave591_ok: bool,
    pub host_render_ui_presentation_helper_live_wave591_ok: bool,
    pub host_render_ui_overlays_helper_method_names_wave592_ok: bool,
    pub host_render_ui_overlays_helper_nav_commands_wave592_ok: bool,
    pub host_render_ui_overlays_helper_live_wave592_ok: bool,
    pub host_render_ui_finalize_helper_method_names_wave593_ok: bool,
    pub host_render_ui_finalize_helper_nav_commands_wave593_ok: bool,
    pub host_render_ui_finalize_helper_live_wave593_ok: bool,
    pub host_minimap_bounds_repair_helper_method_names_wave594_ok: bool,
    pub host_minimap_bounds_repair_helper_nav_commands_wave594_ok: bool,
    pub host_minimap_bounds_repair_helper_live_wave594_ok: bool,
    pub host_production_complete_apply_helper_method_names_wave595_ok: bool,
    pub host_production_complete_apply_helper_nav_commands_wave595_ok: bool,
    pub host_production_complete_apply_helper_live_wave595_ok: bool,
    pub host_camera_queue_drain_helper_method_names_wave596_ok: bool,
    pub host_camera_queue_drain_helper_nav_commands_wave596_ok: bool,
    pub host_camera_queue_drain_helper_live_wave596_ok: bool,
    pub host_gameworld_shadow_session_helper_method_names_wave597_ok: bool,
    pub host_gameworld_shadow_session_helper_nav_commands_wave597_ok: bool,
    pub host_gameworld_shadow_session_helper_live_wave597_ok: bool,
    pub host_ingame_hud_helper_method_names_wave598_ok: bool,
    pub host_ingame_hud_helper_nav_commands_wave598_ok: bool,
    pub host_ingame_hud_helper_live_wave598_ok: bool,
    pub host_match_outcome_helper_method_names_wave599_ok: bool,
    pub host_match_outcome_helper_nav_commands_wave599_ok: bool,
    pub host_match_outcome_helper_live_wave599_ok: bool,
    pub host_post_presentation_client_helper_method_names_wave600_ok: bool,
    pub host_post_presentation_client_helper_nav_commands_wave600_ok: bool,
    pub host_post_presentation_client_helper_live_wave600_ok: bool,
    pub host_restart_pause_helper_method_names_wave601_ok: bool,
    pub host_restart_pause_helper_nav_commands_wave601_ok: bool,
    pub host_restart_pause_helper_live_wave601_ok: bool,
    pub host_ingame_logic_shell_helper_method_names_wave602_ok: bool,
    pub host_ingame_logic_shell_helper_nav_commands_wave602_ok: bool,
    pub host_ingame_logic_shell_helper_live_wave602_ok: bool,
    pub host_paused_endgame_boot_ui_helper_method_names_wave603_ok: bool,
    pub host_paused_endgame_boot_ui_helper_nav_commands_wave603_ok: bool,
    pub host_paused_endgame_boot_ui_helper_live_wave603_ok: bool,
    pub host_loading_sfx_helper_method_names_wave604_ok: bool,
    pub host_loading_sfx_helper_nav_commands_wave604_ok: bool,
    pub host_loading_sfx_helper_live_wave604_ok: bool,
    pub host_menu_client_helper_method_names_wave605_ok: bool,
    pub host_menu_client_helper_nav_commands_wave605_ok: bool,
    pub host_menu_client_helper_live_wave605_ok: bool,
    pub host_os_inject_presentation_notify_helper_method_names_wave606_ok: bool,
    pub host_os_inject_presentation_notify_helper_nav_commands_wave606_ok: bool,
    pub host_os_inject_presentation_notify_helper_live_wave606_ok: bool,
    pub host_ui_presentation_drain_helper_method_names_wave607_ok: bool,
    pub host_ui_presentation_drain_helper_nav_commands_wave607_ok: bool,
    pub host_ui_presentation_drain_helper_live_wave607_ok: bool,
    pub host_production_complete_host_apply_helper_method_names_wave608_ok: bool,
    pub host_production_complete_host_apply_helper_nav_commands_wave608_ok: bool,
    pub host_production_complete_host_apply_helper_live_wave608_ok: bool,
    pub host_ui_economy_mouse_mode_helper_method_names_wave609_ok: bool,
    pub host_ui_economy_mouse_mode_helper_nav_commands_wave609_ok: bool,
    pub host_ui_economy_mouse_mode_helper_live_wave609_ok: bool,
    pub host_ui_selection_startup_helper_method_names_wave610_ok: bool,
    pub host_ui_selection_startup_helper_nav_commands_wave610_ok: bool,
    pub host_ui_selection_startup_helper_live_wave610_ok: bool,
    pub host_start_save_load_helper_method_names_wave611_ok: bool,
    pub host_start_save_load_helper_nav_commands_wave611_ok: bool,
    pub host_start_save_load_helper_live_wave611_ok: bool,
    pub host_combat_cursor_transition_helper_method_names_wave612_ok: bool,
    pub host_combat_cursor_transition_helper_nav_commands_wave612_ok: bool,
    pub host_combat_cursor_transition_helper_live_wave612_ok: bool,
    pub host_production_complete_collect_helper_method_names_wave613_ok: bool,
    pub host_production_complete_collect_helper_nav_commands_wave613_ok: bool,
    pub host_production_complete_collect_helper_live_wave613_ok: bool,
    pub host_production_ready_log_helper_method_names_wave614_ok: bool,
    pub host_production_ready_log_helper_nav_commands_wave614_ok: bool,
    pub host_production_ready_log_helper_live_wave614_ok: bool,
    pub host_production_spawn_helper_method_names_wave615_ok: bool,
    pub host_production_spawn_helper_nav_commands_wave615_ok: bool,
    pub host_production_spawn_helper_live_wave615_ok: bool,
    pub ai_attack_recheck_production_authority_chain_method_names_wave616_ok: bool,
    pub ai_attack_recheck_production_authority_chain_nav_commands_wave616_ok: bool,
    pub ai_attack_recheck_production_authority_chain_live_wave616_ok: bool,
    pub host_construction_ready_log_helper_method_names_wave617_ok: bool,
    pub host_construction_ready_log_helper_nav_commands_wave617_ok: bool,
    pub host_construction_ready_log_helper_live_wave617_ok: bool,
    pub host_special_power_ready_log_helper_method_names_wave618_ok: bool,
    pub host_special_power_ready_log_helper_nav_commands_wave618_ok: bool,
    pub host_special_power_ready_log_helper_live_wave618_ok: bool,
    pub host_sell_ready_log_helper_method_names_wave619_ok: bool,
    pub host_sell_ready_log_helper_nav_commands_wave619_ok: bool,
    pub host_sell_ready_log_helper_live_wave619_ok: bool,
    pub host_rebuild_ready_log_helper_method_names_wave620_ok: bool,
    pub host_rebuild_ready_log_helper_nav_commands_wave620_ok: bool,
    pub host_rebuild_ready_log_helper_live_wave620_ok: bool,
    pub host_destroy_ready_log_helper_method_names_wave621_ok: bool,
    pub host_destroy_ready_log_helper_nav_commands_wave621_ok: bool,
    pub host_destroy_ready_log_helper_live_wave621_ok: bool,
    pub host_veterancy_ready_log_helper_method_names_wave622_ok: bool,
    pub host_veterancy_ready_log_helper_nav_commands_wave622_ok: bool,
    pub host_veterancy_ready_log_helper_live_wave622_ok: bool,
    pub host_body_damage_ready_log_helper_method_names_wave623_ok: bool,
    pub host_body_damage_ready_log_helper_nav_commands_wave623_ok: bool,
    pub host_body_damage_ready_log_helper_live_wave623_ok: bool,
    pub host_upgrade_ready_log_helper_method_names_wave624_ok: bool,
    pub host_upgrade_ready_log_helper_nav_commands_wave624_ok: bool,
    pub host_upgrade_ready_log_helper_live_wave624_ok: bool,
    pub host_radar_extend_ready_log_helper_method_names_wave625_ok: bool,
    pub host_radar_extend_ready_log_helper_nav_commands_wave625_ok: bool,
    pub host_radar_extend_ready_log_helper_live_wave625_ok: bool,
    pub host_construction_complete_clear_ready_log_helper_method_names_wave626_ok: bool,
    pub host_construction_complete_clear_ready_log_helper_nav_commands_wave626_ok: bool,
    pub host_construction_complete_clear_ready_log_helper_live_wave626_ok: bool,
    pub host_production_door_ready_log_helper_method_names_wave627_ok: bool,
    pub host_production_door_ready_log_helper_nav_commands_wave627_ok: bool,
    pub host_production_door_ready_log_helper_live_wave627_ok: bool,
    pub host_contain_ready_log_helper_method_names_wave628_ok: bool,
    pub host_contain_ready_log_helper_nav_commands_wave628_ok: bool,
    pub host_contain_ready_log_helper_live_wave628_ok: bool,
    pub host_owner_ready_log_helper_method_names_wave629_ok: bool,
    pub host_owner_ready_log_helper_nav_commands_wave629_ok: bool,
    pub host_owner_ready_log_helper_live_wave629_ok: bool,
    pub host_ai_state_ready_log_helper_method_names_wave630_ok: bool,
    pub host_ai_state_ready_log_helper_nav_commands_wave630_ok: bool,
    pub host_ai_state_ready_log_helper_live_wave630_ok: bool,
    pub host_economy_ready_log_helper_method_names_wave631_ok: bool,
    pub host_economy_ready_log_helper_nav_commands_wave631_ok: bool,
    pub host_economy_ready_log_helper_live_wave631_ok: bool,
    pub host_death_type_ready_log_helper_method_names_wave632_ok: bool,
    pub host_death_type_ready_log_helper_nav_commands_wave632_ok: bool,
    pub host_death_type_ready_log_helper_live_wave632_ok: bool,
    pub host_model_condition_ready_log_helper_method_names_wave633_ok: bool,
    pub host_model_condition_ready_log_helper_nav_commands_wave633_ok: bool,
    pub host_model_condition_ready_log_helper_live_wave633_ok: bool,
    pub host_combat_status_ready_log_helper_method_names_wave634_ok: bool,
    pub host_combat_status_ready_log_helper_nav_commands_wave634_ok: bool,
    pub host_combat_status_ready_log_helper_live_wave634_ok: bool,
    pub host_weapon_stats_ready_log_helper_method_names_wave635_ok: bool,
    pub host_weapon_stats_ready_log_helper_nav_commands_wave635_ok: bool,
    pub host_weapon_stats_ready_log_helper_live_wave635_ok: bool,
    pub host_transform_ready_log_helper_method_names_wave636_ok: bool,
    pub host_transform_ready_log_helper_nav_commands_wave636_ok: bool,
    pub host_transform_ready_log_helper_live_wave636_ok: bool,
    pub host_movement_ready_log_helper_method_names_wave637_ok: bool,
    pub host_movement_ready_log_helper_nav_commands_wave637_ok: bool,
    pub host_movement_ready_log_helper_live_wave637_ok: bool,
    pub host_attack_target_ready_log_helper_method_names_wave638_ok: bool,
    pub host_attack_target_ready_log_helper_nav_commands_wave638_ok: bool,
    pub host_attack_target_ready_log_helper_live_wave638_ok: bool,
    pub host_move_target_ready_log_helper_method_names_wave639_ok: bool,
    pub host_move_target_ready_log_helper_nav_commands_wave639_ok: bool,
    pub host_move_target_ready_log_helper_live_wave639_ok: bool,
    pub host_fire_intent_ready_log_helper_method_names_wave640_ok: bool,
    pub host_fire_intent_ready_log_helper_nav_commands_wave640_ok: bool,
    pub host_fire_intent_ready_log_helper_live_wave640_ok: bool,
    pub host_stored_supplies_ready_log_helper_method_names_wave641_ok: bool,
    pub host_stored_supplies_ready_log_helper_nav_commands_wave641_ok: bool,
    pub host_stored_supplies_ready_log_helper_live_wave641_ok: bool,
    pub host_weapon_set_ready_log_helper_method_names_wave642_ok: bool,
    pub host_weapon_set_ready_log_helper_nav_commands_wave642_ok: bool,
    pub host_weapon_set_ready_log_helper_live_wave642_ok: bool,
    pub host_combat_attack_ready_log_helper_method_names_wave643_ok: bool,
    pub host_combat_attack_ready_log_helper_nav_commands_wave643_ok: bool,
    pub host_combat_attack_ready_log_helper_live_wave643_ok: bool,
    pub host_command_set_ready_log_helper_method_names_wave644_ok: bool,
    pub host_command_set_ready_log_helper_nav_commands_wave644_ok: bool,
    pub host_command_set_ready_log_helper_live_wave644_ok: bool,
    pub host_ai_mood_ready_log_helper_method_names_wave645_ok: bool,
    pub host_ai_mood_ready_log_helper_nav_commands_wave645_ok: bool,
    pub host_ai_mood_ready_log_helper_live_wave645_ok: bool,
    pub host_locomotor_ready_log_helper_method_names_wave646_ok: bool,
    pub host_locomotor_ready_log_helper_nav_commands_wave646_ok: bool,
    pub host_locomotor_ready_log_helper_live_wave646_ok: bool,
    pub host_hijacker_ready_log_helper_method_names_wave647_ok: bool,
    pub host_hijacker_ready_log_helper_nav_commands_wave647_ok: bool,
    pub host_hijacker_ready_log_helper_live_wave647_ok: bool,
    pub host_ai_request_ready_log_helper_method_names_wave648_ok: bool,
    pub host_ai_request_ready_log_helper_nav_commands_wave648_ok: bool,
    pub host_ai_request_ready_log_helper_live_wave648_ok: bool,
    pub host_physics_motive_ready_log_helper_method_names_wave649_ok: bool,
    pub host_physics_motive_ready_log_helper_nav_commands_wave649_ok: bool,
    pub host_physics_motive_ready_log_helper_live_wave649_ok: bool,
    pub host_bounce_land_ready_log_helper_method_names_wave650_ok: bool,
    pub host_bounce_land_ready_log_helper_nav_commands_wave650_ok: bool,
    pub host_bounce_land_ready_log_helper_live_wave650_ok: bool,
    pub host_stealth_delay_ready_log_helper_method_names_wave651_ok: bool,
    pub host_stealth_delay_ready_log_helper_nav_commands_wave651_ok: bool,
    pub host_stealth_delay_ready_log_helper_live_wave651_ok: bool,
    pub host_stealth_flags_ready_log_helper_method_names_wave652_ok: bool,
    pub host_stealth_flags_ready_log_helper_nav_commands_wave652_ok: bool,
    pub host_stealth_flags_ready_log_helper_live_wave652_ok: bool,
    pub host_disguise_ready_log_helper_method_names_wave653_ok: bool,
    pub host_disguise_ready_log_helper_nav_commands_wave653_ok: bool,
    pub host_disguise_ready_log_helper_live_wave653_ok: bool,
    pub host_vision_camo_ready_log_helper_method_names_wave654_ok: bool,
    pub host_vision_camo_ready_log_helper_nav_commands_wave654_ok: bool,
    pub host_vision_camo_ready_log_helper_live_wave654_ok: bool,
    pub host_selection_radius_ready_log_helper_method_names_wave655_ok: bool,
    pub host_selection_radius_ready_log_helper_nav_commands_wave655_ok: bool,
    pub host_selection_radius_ready_log_helper_live_wave655_ok: bool,
    pub host_ground_height_ready_log_helper_method_names_wave656_ok: bool,
    pub host_ground_height_ready_log_helper_nav_commands_wave656_ok: bool,
    pub host_ground_height_ready_log_helper_live_wave656_ok: bool,
    pub host_weapon_slot_ready_log_helper_method_names_wave657_ok: bool,
    pub host_weapon_slot_ready_log_helper_nav_commands_wave657_ok: bool,
    pub host_weapon_slot_ready_log_helper_live_wave657_ok: bool,
    pub host_weapon_bonus_ready_log_helper_method_names_wave658_ok: bool,
    pub host_weapon_bonus_ready_log_helper_nav_commands_wave658_ok: bool,
    pub host_weapon_bonus_ready_log_helper_live_wave658_ok: bool,
    pub host_ai_attitude_ready_log_helper_method_names_wave659_ok: bool,
    pub host_ai_attitude_ready_log_helper_nav_commands_wave659_ok: bool,
    pub host_ai_attitude_ready_log_helper_live_wave659_ok: bool,
    pub host_identity_ready_log_helper_method_names_wave660_ok: bool,
    pub host_identity_ready_log_helper_nav_commands_wave660_ok: bool,
    pub host_identity_ready_log_helper_live_wave660_ok: bool,
    pub host_repulsor_ready_log_helper_method_names_wave661_ok: bool,
    pub host_repulsor_ready_log_helper_nav_commands_wave661_ok: bool,
    pub host_repulsor_ready_log_helper_live_wave661_ok: bool,
    pub host_shock_stun_ready_log_helper_method_names_wave662_ok: bool,
    pub host_shock_stun_ready_log_helper_nav_commands_wave662_ok: bool,
    pub host_shock_stun_ready_log_helper_live_wave662_ok: bool,
    pub host_sole_healing_ready_log_helper_method_names_wave663_ok: bool,
    pub host_sole_healing_ready_log_helper_nav_commands_wave663_ok: bool,
    pub host_sole_healing_ready_log_helper_live_wave663_ok: bool,
    pub host_crush_vision_ready_log_helper_method_names_wave664_ok: bool,
    pub host_crush_vision_ready_log_helper_nav_commands_wave664_ok: bool,
    pub host_crush_vision_ready_log_helper_live_wave664_ok: bool,
    pub host_demo_mine_cheer_ready_log_helper_method_names_wave665_ok: bool,
    pub host_demo_mine_cheer_ready_log_helper_nav_commands_wave665_ok: bool,
    pub host_demo_mine_cheer_ready_log_helper_live_wave665_ok: bool,
    pub host_overlord_ready_log_helper_method_names_wave666_ok: bool,
    pub host_overlord_ready_log_helper_nav_commands_wave666_ok: bool,
    pub host_overlord_ready_log_helper_live_wave666_ok: bool,
    pub host_hive_ready_log_helper_method_names_wave667_ok: bool,
    pub host_hive_ready_log_helper_nav_commands_wave667_ok: bool,
    pub host_hive_ready_log_helper_live_wave667_ok: bool,
    pub host_overcharge_ready_log_helper_method_names_wave668_ok: bool,
    pub host_overcharge_ready_log_helper_nav_commands_wave668_ok: bool,
    pub host_overcharge_ready_log_helper_live_wave668_ok: bool,
    pub host_guard_ready_log_helper_method_names_wave669_ok: bool,
    pub host_guard_ready_log_helper_nav_commands_wave669_ok: bool,
    pub host_guard_ready_log_helper_live_wave669_ok: bool,
    pub host_continuous_fire_ready_log_helper_method_names_wave670_ok: bool,
    pub host_continuous_fire_ready_log_helper_nav_commands_wave670_ok: bool,
    pub host_continuous_fire_ready_log_helper_live_wave670_ok: bool,
    pub host_detector_ready_log_helper_method_names_wave671_ok: bool,
    pub host_detector_ready_log_helper_nav_commands_wave671_ok: bool,
    pub host_detector_ready_log_helper_live_wave671_ok: bool,
    pub host_target_location_ready_log_helper_method_names_wave672_ok: bool,
    pub host_target_location_ready_log_helper_nav_commands_wave672_ok: bool,
    pub host_target_location_ready_log_helper_live_wave672_ok: bool,
    pub host_turret_ready_log_helper_method_names_wave673_ok: bool,
    pub host_turret_ready_log_helper_nav_commands_wave673_ok: bool,
    pub host_turret_ready_log_helper_live_wave673_ok: bool,
    pub host_entity_power_ready_log_helper_method_names_wave674_ok: bool,
    pub host_entity_power_ready_log_helper_nav_commands_wave674_ok: bool,
    pub host_entity_power_ready_log_helper_live_wave674_ok: bool,
    pub host_building_type_ready_log_helper_method_names_wave675_ok: bool,
    pub host_building_type_ready_log_helper_nav_commands_wave675_ok: bool,
    pub host_building_type_ready_log_helper_live_wave675_ok: bool,
    pub host_faerie_fire_ready_log_helper_method_names_wave676_ok: bool,
    pub host_faerie_fire_ready_log_helper_nav_commands_wave676_ok: bool,
    pub host_faerie_fire_ready_log_helper_live_wave676_ok: bool,
    pub host_disable_timers_ready_log_helper_method_names_wave677_ok: bool,
    pub host_disable_timers_ready_log_helper_nav_commands_wave677_ok: bool,
    pub host_disable_timers_ready_log_helper_live_wave677_ok: bool,
    pub host_projectiles_ready_log_helper_method_names_wave678_ok: bool,
    pub host_projectiles_ready_log_helper_nav_commands_wave678_ok: bool,
    pub host_projectiles_ready_log_helper_live_wave678_ok: bool,
    pub host_production_spawn_ready_log_helper_method_names_wave679_ok: bool,
    pub host_production_spawn_ready_log_helper_nav_commands_wave679_ok: bool,
    pub host_production_spawn_ready_log_helper_live_wave679_ok: bool,
    pub host_eager_spawn_map_helper_method_names_wave680_ok: bool,
    pub host_eager_spawn_map_helper_nav_commands_wave680_ok: bool,
    pub host_eager_spawn_map_helper_live_wave680_ok: bool,
    pub host_eager_destroy_unmap_helper_method_names_wave681_ok: bool,
    pub host_eager_destroy_unmap_helper_nav_commands_wave681_ok: bool,
    pub host_eager_destroy_unmap_helper_live_wave681_ok: bool,
    pub host_eager_fire_spawn_helper_method_names_wave682_ok: bool,
    pub host_eager_fire_spawn_helper_nav_commands_wave682_ok: bool,
    pub host_eager_fire_spawn_helper_live_wave682_ok: bool,
    pub host_eager_move_attack_helper_method_names_wave683_ok: bool,
    pub host_eager_move_attack_helper_nav_commands_wave683_ok: bool,
    pub host_eager_move_attack_helper_live_wave683_ok: bool,
    pub host_eager_damage_helper_method_names_wave684_ok: bool,
    pub host_eager_damage_helper_nav_commands_wave684_ok: bool,
    pub host_eager_damage_helper_live_wave684_ok: bool,
    pub host_eager_heal_helper_method_names_wave685_ok: bool,
    pub host_eager_heal_helper_nav_commands_wave685_ok: bool,
    pub host_eager_heal_helper_live_wave685_ok: bool,
    pub host_eager_max_health_xp_helper_method_names_wave686_ok: bool,
    pub host_eager_max_health_xp_helper_nav_commands_wave686_ok: bool,
    pub host_eager_max_health_xp_helper_live_wave686_ok: bool,
    pub host_eager_ai_fire_intent_helper_method_names_wave687_ok: bool,
    pub host_eager_ai_fire_intent_helper_nav_commands_wave687_ok: bool,
    pub host_eager_ai_fire_intent_helper_live_wave687_ok: bool,
    pub host_eager_owner_movement_helper_method_names_wave688_ok: bool,
    pub host_eager_owner_movement_helper_nav_commands_wave688_ok: bool,
    pub host_eager_owner_movement_helper_live_wave688_ok: bool,
    pub host_eager_status_veterancy_helper_method_names_wave689_ok: bool,
    pub host_eager_status_veterancy_helper_nav_commands_wave689_ok: bool,
    pub host_eager_status_veterancy_helper_live_wave689_ok: bool,
    pub host_eager_weapon_bonus_slot_helper_method_names_wave690_ok: bool,
    pub host_eager_weapon_bonus_slot_helper_nav_commands_wave690_ok: bool,
    pub host_eager_weapon_bonus_slot_helper_live_wave690_ok: bool,
    pub host_eager_weapon_set_power_helper_method_names_wave691_ok: bool,
    pub host_eager_weapon_set_power_helper_nav_commands_wave691_ok: bool,
    pub host_eager_weapon_set_power_helper_live_wave691_ok: bool,
    pub host_eager_turret_guard_rally_helper_method_names_wave692_ok: bool,
    pub host_eager_turret_guard_rally_helper_nav_commands_wave692_ok: bool,
    pub host_eager_turret_guard_rally_helper_live_wave692_ok: bool,
    pub host_eager_tloc_detector_cf_helper_method_names_wave693_ok: bool,
    pub host_eager_tloc_detector_cf_helper_nav_commands_wave693_ok: bool,
    pub host_eager_tloc_detector_cf_helper_live_wave693_ok: bool,
    pub host_eager_attitude_overcharge_stealth_helper_method_names_wave694_ok: bool,
    pub host_eager_attitude_overcharge_stealth_helper_nav_commands_wave694_ok: bool,
    pub host_eager_attitude_overcharge_stealth_helper_live_wave694_ok: bool,
    pub host_eager_contain_hive_overlord_helper_method_names_wave695_ok: bool,
    pub host_eager_contain_hive_overlord_helper_nav_commands_wave695_ok: bool,
    pub host_eager_contain_hive_overlord_helper_live_wave695_ok: bool,
    pub host_eager_cmdset_disguise_camo_helper_method_names_wave696_ok: bool,
    pub host_eager_cmdset_disguise_camo_helper_nav_commands_wave696_ok: bool,
    pub host_eager_cmdset_disguise_camo_helper_live_wave696_ok: bool,
    pub host_eager_wstats_sel_model_helper_method_names_wave697_ok: bool,
    pub host_eager_wstats_sel_model_helper_nav_commands_wave697_ok: bool,
    pub host_eager_wstats_sel_model_helper_live_wave697_ok: bool,
    pub host_eager_demo_form_crush_helper_method_names_wave698_ok: bool,
    pub host_eager_demo_form_crush_helper_nav_commands_wave698_ok: bool,
    pub host_eager_demo_form_crush_helper_live_wave698_ok: bool,
    pub host_eager_btype_identity_ground_helper_method_names_wave699_ok: bool,
    pub host_eager_btype_identity_ground_helper_nav_commands_wave699_ok: bool,
    pub host_eager_btype_identity_ground_helper_live_wave699_ok: bool,
    pub host_eager_mesh_fow_kindof_helper_method_names_wave700_ok: bool,
    pub host_eager_mesh_fow_kindof_helper_nav_commands_wave700_ok: bool,
    pub host_eager_mesh_fow_kindof_helper_live_wave700_ok: bool,
    pub host_eager_faerie_repulsor_disable_helper_method_names_wave701_ok: bool,
    pub host_eager_faerie_repulsor_disable_helper_nav_commands_wave701_ok: bool,
    pub host_eager_faerie_repulsor_disable_helper_live_wave701_ok: bool,
    pub host_eager_body_death_physics_helper_method_names_wave702_ok: bool,
    pub host_eager_body_death_physics_helper_nav_commands_wave702_ok: bool,
    pub host_eager_body_death_physics_helper_live_wave702_ok: bool,
    pub host_eager_loco_bounce_helper_method_names_wave703_ok: bool,
    pub host_eager_loco_bounce_helper_nav_commands_wave703_ok: bool,
    pub host_eager_loco_bounce_helper_live_wave703_ok: bool,
    pub host_eager_aimood_request_shock_helper_method_names_wave704_ok: bool,
    pub host_eager_aimood_request_shock_helper_nav_commands_wave704_ok: bool,
    pub host_eager_aimood_request_shock_helper_live_wave704_ok: bool,
    pub host_eager_stealth_sole_radar_helper_method_names_wave705_ok: bool,
    pub host_eager_stealth_sole_radar_helper_nav_commands_wave705_ok: bool,
    pub host_eager_stealth_sole_radar_helper_live_wave705_ok: bool,
    pub host_eager_hijack_rebuild_supplies_helper_method_names_wave706_ok: bool,
    pub host_eager_hijack_rebuild_supplies_helper_nav_commands_wave706_ok: bool,
    pub host_eager_hijack_rebuild_supplies_helper_live_wave706_ok: bool,
    pub host_eager_sp_radar_progress_helper_method_names_wave707_ok: bool,
    pub host_eager_sp_radar_progress_helper_nav_commands_wave707_ok: bool,
    pub host_eager_sp_radar_progress_helper_live_wave707_ok: bool,
    pub host_eager_meta_cooldown_door_helper_method_names_wave708_ok: bool,
    pub host_eager_meta_cooldown_door_helper_nav_commands_wave708_ok: bool,
    pub host_eager_meta_cooldown_door_helper_live_wave708_ok: bool,
    pub host_eager_prod_construction_helper_method_names_wave709_ok: bool,
    pub host_eager_prod_construction_helper_nav_commands_wave709_ok: bool,
    pub host_eager_prod_construction_helper_live_wave709_ok: bool,
    pub host_eager_combat_projectile_helper_method_names_wave710_ok: bool,
    pub host_eager_combat_projectile_helper_nav_commands_wave710_ok: bool,
    pub host_eager_combat_projectile_helper_live_wave710_ok: bool,
    pub host_eager_destroy_contain_ai_helper_method_names_wave711_ok: bool,
    pub host_eager_destroy_contain_ai_helper_nav_commands_wave711_ok: bool,
    pub host_eager_destroy_contain_ai_helper_live_wave711_ok: bool,
    pub host_eager_spawn_move_attack_helper_method_names_wave712_ok: bool,
    pub host_eager_spawn_move_attack_helper_nav_commands_wave712_ok: bool,
    pub host_eager_spawn_move_attack_helper_live_wave712_ok: bool,
    pub host_production_ready_no_empty_scan_method_names_wave713_ok: bool,
    pub host_production_ready_no_empty_scan_nav_commands_wave713_ok: bool,
    pub host_production_ready_no_empty_scan_live_wave713_ok: bool,
    pub host_production_same_frame_ready_complete_method_names_wave714_ok: bool,
    pub host_production_same_frame_ready_complete_nav_commands_wave714_ok: bool,
    pub host_production_same_frame_ready_complete_live_wave714_ok: bool,
    pub host_construction_same_frame_ready_complete_method_names_wave715_ok: bool,
    pub host_construction_same_frame_ready_complete_nav_commands_wave715_ok: bool,
    pub host_construction_same_frame_ready_complete_live_wave715_ok: bool,
    pub host_sell_same_frame_ready_complete_method_names_wave716_ok: bool,
    pub host_sell_same_frame_ready_complete_nav_commands_wave716_ok: bool,
    pub host_sell_same_frame_ready_complete_live_wave716_ok: bool,
    pub host_special_power_same_frame_ready_eva_method_names_wave717_ok: bool,
    pub host_special_power_same_frame_ready_eva_nav_commands_wave717_ok: bool,
    pub host_special_power_same_frame_ready_eva_live_wave717_ok: bool,
    pub host_train_force_complete_opt_in_method_names_wave718_ok: bool,
    pub host_train_force_complete_opt_in_nav_commands_wave718_ok: bool,
    pub host_train_force_complete_opt_in_live_wave718_ok: bool,
    pub host_construct_spawn_dozer_opt_in_method_names_wave719_ok: bool,
    pub host_construct_spawn_dozer_opt_in_nav_commands_wave719_ok: bool,
    pub host_construct_spawn_dozer_opt_in_live_wave719_ok: bool,
    pub host_formation_spawn_buddy_opt_in_method_names_wave720_ok: bool,
    pub host_formation_spawn_buddy_opt_in_nav_commands_wave720_ok: bool,
    pub host_formation_spawn_buddy_opt_in_live_wave720_ok: bool,
    pub host_grant_min_supplies_opt_in_method_names_wave721_ok: bool,
    pub host_grant_min_supplies_opt_in_nav_commands_wave721_ok: bool,
    pub host_grant_min_supplies_opt_in_live_wave721_ok: bool,
    pub host_golden_ranger_template_opt_in_method_names_wave722_ok: bool,
    pub host_golden_ranger_template_opt_in_nav_commands_wave722_ok: bool,
    pub host_golden_ranger_template_opt_in_live_wave722_ok: bool,
    pub host_ensure_barracks_opt_in_method_names_wave723_ok: bool,
    pub host_ensure_barracks_opt_in_nav_commands_wave723_ok: bool,
    pub host_ensure_barracks_opt_in_live_wave723_ok: bool,
    pub host_train_try_names_golden_opt_in_method_names_wave724_ok: bool,
    pub host_train_try_names_golden_opt_in_nav_commands_wave724_ok: bool,
    pub host_train_try_names_golden_opt_in_live_wave724_ok: bool,
    pub host_alias_fallback_opt_in_method_names_wave725_ok: bool,
    pub host_alias_fallback_opt_in_nav_commands_wave725_ok: bool,
    pub host_alias_fallback_opt_in_live_wave725_ok: bool,
    pub host_auto_select_mobile_opt_in_method_names_wave726_ok: bool,
    pub host_auto_select_mobile_opt_in_nav_commands_wave726_ok: bool,
    pub host_auto_select_mobile_opt_in_live_wave726_ok: bool,
    pub host_default_template_opt_in_method_names_wave727_ok: bool,
    pub host_default_template_opt_in_nav_commands_wave727_ok: bool,
    pub host_default_template_opt_in_live_wave727_ok: bool,
    pub host_sell_auto_target_opt_in_method_names_wave728_ok: bool,
    pub host_sell_auto_target_opt_in_nav_commands_wave728_ok: bool,
    pub host_sell_auto_target_opt_in_live_wave728_ok: bool,
    pub host_auto_target_opt_in_method_names_wave729_ok: bool,
    pub host_auto_target_opt_in_nav_commands_wave729_ok: bool,
    pub host_auto_target_opt_in_live_wave729_ok: bool,
    pub host_cmd_auto_select_opt_in_method_names_wave730_ok: bool,
    pub host_cmd_auto_select_opt_in_nav_commands_wave730_ok: bool,
    pub host_cmd_auto_select_opt_in_live_wave730_ok: bool,
    pub host_cmd_auto_pick_opt_in_method_names_wave731_ok: bool,
    pub host_cmd_auto_pick_opt_in_nav_commands_wave731_ok: bool,
    pub host_cmd_auto_pick_opt_in_live_wave731_ok: bool,
    pub host_seed_start_presence_opt_in_method_names_wave732_ok: bool,
    pub host_seed_start_presence_opt_in_nav_commands_wave732_ok: bool,
    pub host_seed_start_presence_opt_in_live_wave732_ok: bool,
    pub host_spawn_faction_base_opt_in_method_names_wave733_ok: bool,
    pub host_spawn_faction_base_opt_in_nav_commands_wave733_ok: bool,
    pub host_spawn_faction_base_opt_in_live_wave733_ok: bool,
    pub host_seed_starting_building_opt_in_method_names_wave734_ok: bool,
    pub host_seed_starting_building_opt_in_nav_commands_wave734_ok: bool,
    pub host_seed_starting_building_opt_in_live_wave734_ok: bool,
    pub host_production_ready_pose_authority_method_names_wave735_ok: bool,
    pub host_production_ready_pose_authority_nav_commands_wave735_ok: bool,
    pub host_production_ready_pose_authority_live_wave735_ok: bool,
    pub host_production_spawn_entity_first_method_names_wave736_ok: bool,
    pub host_production_spawn_entity_first_nav_commands_wave736_ok: bool,
    pub host_production_spawn_entity_first_live_wave736_ok: bool,
    pub host_production_object_id_prefers_gw_entity_method_names_wave737_ok: bool,
    pub host_production_object_id_prefers_gw_entity_nav_commands_wave737_ok: bool,
    pub host_production_object_id_prefers_gw_entity_live_wave737_ok: bool,
    pub host_production_spawn_requires_gw_bind_method_names_wave738_ok: bool,
    pub host_production_spawn_requires_gw_bind_nav_commands_wave738_ok: bool,
    pub host_production_spawn_requires_gw_bind_live_wave738_ok: bool,
    pub host_production_spawn_pose_no_rejitter_method_names_wave739_ok: bool,
    pub host_production_spawn_pose_no_rejitter_nav_commands_wave739_ok: bool,
    pub host_production_spawn_pose_no_rejitter_live_wave739_ok: bool,
    pub host_rebuild_spawn_entity_first_method_names_wave740_ok: bool,
    pub host_rebuild_spawn_entity_first_nav_commands_wave740_ok: bool,
    pub host_rebuild_spawn_entity_first_live_wave740_ok: bool,
    pub host_rebuild_spawn_requires_gw_bind_method_names_wave741_ok: bool,
    pub host_rebuild_spawn_requires_gw_bind_nav_commands_wave741_ok: bool,
    pub host_rebuild_spawn_requires_gw_bind_live_wave741_ok: bool,
    pub host_rebuild_hole_expose_entity_first_method_names_wave742_ok: bool,
    pub host_rebuild_hole_expose_entity_first_nav_commands_wave742_ok: bool,
    pub host_rebuild_hole_expose_entity_first_live_wave742_ok: bool,
    pub host_production_door_sole_no_dual_tick_method_names_wave743_ok: bool,
    pub host_production_door_sole_no_dual_tick_nav_commands_wave743_ok: bool,
    pub host_production_door_sole_no_dual_tick_live_wave743_ok: bool,
    pub host_radar_extend_no_dual_complete_method_names_wave744_ok: bool,
    pub host_radar_extend_no_dual_complete_nav_commands_wave744_ok: bool,
    pub host_radar_extend_no_dual_complete_live_wave744_ok: bool,
    pub host_lifetime_kill_no_damage_auth_hp_stomp_method_names_wave745_ok: bool,
    pub host_lifetime_kill_no_damage_auth_hp_stomp_nav_commands_wave745_ok: bool,
    pub host_lifetime_kill_no_damage_auth_hp_stomp_live_wave745_ok: bool,
    pub host_crush_failclosed_no_damage_auth_hp_stomp_method_names_wave746_ok: bool,
    pub host_crush_failclosed_no_damage_auth_hp_stomp_nav_commands_wave746_ok: bool,
    pub host_crush_failclosed_no_damage_auth_hp_stomp_live_wave746_ok: bool,
    pub host_evacuate_exit_no_damage_auth_hp_stomp_method_names_wave747_ok: bool,
    pub host_evacuate_exit_no_damage_auth_hp_stomp_nav_commands_wave747_ok: bool,
    pub host_evacuate_exit_no_damage_auth_hp_stomp_live_wave747_ok: bool,
    pub host_hive_struct_damage_no_damage_auth_hp_stomp_method_names_wave748_ok: bool,
    pub host_hive_struct_damage_no_damage_auth_hp_stomp_nav_commands_wave748_ok: bool,
    pub host_hive_struct_damage_no_damage_auth_hp_stomp_live_wave748_ok: bool,
    pub host_tensile_rubble_no_damage_auth_hp_stomp_method_names_wave749_ok: bool,
    pub host_tensile_rubble_no_damage_auth_hp_stomp_nav_commands_wave749_ok: bool,
    pub host_tensile_rubble_no_damage_auth_hp_stomp_live_wave749_ok: bool,
    pub host_spectre_prior_clear_no_damage_auth_hp_stomp_method_names_wave750_ok: bool,
    pub host_spectre_prior_clear_no_damage_auth_hp_stomp_nav_commands_wave750_ok: bool,
    pub host_spectre_prior_clear_no_damage_auth_hp_stomp_live_wave750_ok: bool,
    pub host_booby_trap_destroy_no_damage_auth_hp_stomp_method_names_wave751_ok: bool,
    pub host_booby_trap_destroy_no_damage_auth_hp_stomp_nav_commands_wave751_ok: bool,
    pub host_booby_trap_destroy_no_damage_auth_hp_stomp_live_wave751_ok: bool,
    pub host_lethal_finish_bulk_no_damage_auth_hp_stomp_method_names_wave752_ok: bool,
    pub host_lethal_finish_bulk_no_damage_auth_hp_stomp_nav_commands_wave752_ok: bool,
    pub host_lethal_finish_bulk_no_damage_auth_hp_stomp_live_wave752_ok: bool,
    pub host_dual_line_lethal_no_damage_auth_hp_stomp_method_names_wave753_ok: bool,
    pub host_dual_line_lethal_no_damage_auth_hp_stomp_nav_commands_wave753_ok: bool,
    pub host_dual_line_lethal_no_damage_auth_hp_stomp_live_wave753_ok: bool,
    pub host_eject_pilot_die_death_start_method_names_wave754_ok: bool,
    pub host_eject_pilot_die_death_start_nav_commands_wave754_ok: bool,
    pub host_eject_pilot_die_death_start_live_wave754_ok: bool,
    pub host_writeback_skip_pending_host_logs_method_names_wave755_ok: bool,
    pub host_writeback_skip_pending_host_logs_nav_commands_wave755_ok: bool,
    pub host_writeback_skip_pending_host_logs_live_wave755_ok: bool,
    pub host_writeback_skip_pending_shock_disable_repulsor_method_names_wave756_ok: bool,
    pub host_writeback_skip_pending_shock_disable_repulsor_nav_commands_wave756_ok: bool,
    pub host_writeback_skip_pending_shock_disable_repulsor_live_wave756_ok: bool,
    pub host_writeback_skip_pending_combat_movement_logs_method_names_wave757_ok: bool,
    pub host_writeback_skip_pending_combat_movement_logs_nav_commands_wave757_ok: bool,
    pub host_writeback_skip_pending_combat_movement_logs_live_wave757_ok: bool,
    pub host_writeback_skip_pending_remaining_logs_method_names_wave758_ok: bool,
    pub host_writeback_skip_pending_remaining_logs_nav_commands_wave758_ok: bool,
    pub host_writeback_skip_pending_remaining_logs_live_wave758_ok: bool,
    pub host_writeback_skip_pending_move_transform_logs_method_names_wave759_ok: bool,
    pub host_writeback_skip_pending_move_transform_logs_nav_commands_wave759_ok: bool,
    pub host_writeback_skip_pending_move_transform_logs_live_wave759_ok: bool,
    pub host_writeback_skip_pending_player_projectile_logs_method_names_wave760_ok: bool,
    pub host_writeback_skip_pending_player_projectile_logs_nav_commands_wave760_ok: bool,
    pub host_writeback_skip_pending_player_projectile_logs_live_wave760_ok: bool,
    pub host_status_timer_dual_peel_method_names_wave761_ok: bool,
    pub host_status_timer_dual_peel_nav_commands_wave761_ok: bool,
    pub host_status_timer_dual_peel_live_wave761_ok: bool,
    pub host_eject_invuln_dual_peel_method_names_wave762_ok: bool,
    pub host_eject_invuln_dual_peel_nav_commands_wave762_ok: bool,
    pub host_eject_invuln_dual_peel_live_wave762_ok: bool,
    pub host_force_reload_dual_peel_method_names_wave763_ok: bool,
    pub host_force_reload_dual_peel_nav_commands_wave763_ok: bool,
    pub host_force_reload_dual_peel_live_wave763_ok: bool,
    pub host_shock_stun_dual_peel_method_names_wave764_ok: bool,
    pub host_shock_stun_dual_peel_nav_commands_wave764_ok: bool,
    pub host_shock_stun_dual_peel_live_wave764_ok: bool,
    pub host_subdual_heal_dual_peel_method_names_wave765_ok: bool,
    pub host_subdual_heal_dual_peel_nav_commands_wave765_ok: bool,
    pub host_subdual_heal_dual_peel_live_wave765_ok: bool,
    pub host_defection_timer_dual_peel_method_names_wave766_ok: bool,
    pub host_defection_timer_dual_peel_nav_commands_wave766_ok: bool,
    pub host_defection_timer_dual_peel_live_wave766_ok: bool,
    pub host_fire_sound_loop_dual_peel_method_names_wave767_ok: bool,
    pub host_fire_sound_loop_dual_peel_nav_commands_wave767_ok: bool,
    pub host_fire_sound_loop_dual_peel_live_wave767_ok: bool,
    pub host_lifetime_expire_dual_peel_method_names_wave768_ok: bool,
    pub host_lifetime_expire_dual_peel_nav_commands_wave768_ok: bool,
    pub host_lifetime_expire_dual_peel_live_wave768_ok: bool,
    pub host_poison_dot_dual_peel_method_names_wave769_ok: bool,
    pub host_poison_dot_dual_peel_nav_commands_wave769_ok: bool,
    pub host_poison_dot_dual_peel_live_wave769_ok: bool,
    pub host_topple_fall_dual_peel_method_names_wave770_ok: bool,
    pub host_topple_fall_dual_peel_nav_commands_wave770_ok: bool,
    pub host_topple_fall_dual_peel_live_wave770_ok: bool,
    pub host_height_die_dual_peel_method_names_wave771_ok: bool,
    pub host_height_die_dual_peel_nav_commands_wave771_ok: bool,
    pub host_height_die_dual_peel_live_wave771_ok: bool,
    pub host_jet_slow_death_dual_peel_method_names_wave772_ok: bool,
    pub host_jet_slow_death_dual_peel_nav_commands_wave772_ok: bool,
    pub host_jet_slow_death_dual_peel_live_wave772_ok: bool,
    pub host_heli_slow_death_dual_peel_method_names_wave773_ok: bool,
    pub host_heli_slow_death_dual_peel_nav_commands_wave773_ok: bool,
    pub host_heli_slow_death_dual_peel_live_wave773_ok: bool,
    pub host_slow_death_dual_peel_method_names_wave774_ok: bool,
    pub host_slow_death_dual_peel_nav_commands_wave774_ok: bool,
    pub host_slow_death_dual_peel_live_wave774_ok: bool,
    pub host_structure_collapse_dual_peel_method_names_wave775_ok: bool,
    pub host_structure_collapse_dual_peel_nav_commands_wave775_ok: bool,
    pub host_structure_collapse_dual_peel_live_wave775_ok: bool,
    pub host_structure_topple_dual_peel_method_names_wave776_ok: bool,
    pub host_structure_topple_dual_peel_nav_commands_wave776_ok: bool,
    pub host_structure_topple_dual_peel_live_wave776_ok: bool,
    pub host_structure_topple_crush_dual_peel_method_names_wave777_ok: bool,
    pub host_structure_topple_crush_dual_peel_nav_commands_wave777_ok: bool,
    pub host_structure_topple_crush_dual_peel_live_wave777_ok: bool,
    pub host_fwwd_continuous_dual_peel_method_names_wave778_ok: bool,
    pub host_fwwd_continuous_dual_peel_nav_commands_wave778_ok: bool,
    pub host_fwwd_continuous_dual_peel_live_wave778_ok: bool,
    pub host_fwwd_reaction_dual_peel_method_names_wave779_ok: bool,
    pub host_fwwd_reaction_dual_peel_nav_commands_wave779_ok: bool,
    pub host_fwwd_reaction_dual_peel_live_wave779_ok: bool,
    pub host_base_regen_dual_peel_method_names_wave780_ok: bool,
    pub host_base_regen_dual_peel_nav_commands_wave780_ok: bool,
    pub host_base_regen_dual_peel_live_wave780_ok: bool,
    pub host_enemy_near_dual_peel_method_names_wave781_ok: bool,
    pub host_enemy_near_dual_peel_nav_commands_wave781_ok: bool,
    pub host_enemy_near_dual_peel_live_wave781_ok: bool,
    pub host_prone_update_dual_peel_method_names_wave782_ok: bool,
    pub host_prone_update_dual_peel_nav_commands_wave782_ok: bool,
    pub host_prone_update_dual_peel_live_wave782_ok: bool,
    pub host_float_update_dual_peel_method_names_wave783_ok: bool,
    pub host_float_update_dual_peel_nav_commands_wave783_ok: bool,
    pub host_float_update_dual_peel_live_wave783_ok: bool,
    pub host_anim_steer_dual_peel_method_names_wave784_ok: bool,
    pub host_anim_steer_dual_peel_nav_commands_wave784_ok: bool,
    pub host_anim_steer_dual_peel_live_wave784_ok: bool,
    pub host_radius_decal_dual_peel_method_names_wave785_ok: bool,
    pub host_radius_decal_dual_peel_nav_commands_wave785_ok: bool,
    pub host_radius_decal_dual_peel_live_wave785_ok: bool,
    pub host_checkpoint_dual_peel_method_names_wave786_ok: bool,
    pub host_checkpoint_dual_peel_nav_commands_wave786_ok: bool,
    pub host_checkpoint_dual_peel_live_wave786_ok: bool,
    pub host_smart_bomb_homing_dual_peel_method_names_wave787_ok: bool,
    pub host_smart_bomb_homing_dual_peel_nav_commands_wave787_ok: bool,
    pub host_smart_bomb_homing_dual_peel_live_wave787_ok: bool,
    pub host_daisy_cutter_flight_dual_peel_method_names_wave788_ok: bool,
    pub host_daisy_cutter_flight_dual_peel_nav_commands_wave788_ok: bool,
    pub host_daisy_cutter_flight_dual_peel_live_wave788_ok: bool,
    pub host_anthrax_bomb_flight_dual_peel_method_names_wave789_ok: bool,
    pub host_anthrax_bomb_flight_dual_peel_nav_commands_wave789_ok: bool,
    pub host_anthrax_bomb_flight_dual_peel_live_wave789_ok: bool,
    pub host_cluster_mines_flight_dual_peel_method_names_wave790_ok: bool,
    pub host_cluster_mines_flight_dual_peel_nav_commands_wave790_ok: bool,
    pub host_cluster_mines_flight_dual_peel_live_wave790_ok: bool,
    pub host_emp_pulse_flight_dual_peel_method_names_wave791_ok: bool,
    pub host_emp_pulse_flight_dual_peel_nav_commands_wave791_ok: bool,
    pub host_emp_pulse_flight_dual_peel_live_wave791_ok: bool,
    pub host_a10_strike_flight_dual_peel_method_names_wave792_ok: bool,
    pub host_a10_strike_flight_dual_peel_nav_commands_wave792_ok: bool,
    pub host_a10_strike_flight_dual_peel_live_wave792_ok: bool,
    pub host_artillery_barrage_flight_dual_peel_method_names_wave793_ok: bool,
    pub host_artillery_barrage_flight_dual_peel_nav_commands_wave793_ok: bool,
    pub host_artillery_barrage_flight_dual_peel_live_wave793_ok: bool,
    pub host_carpet_bomb_flight_dual_peel_method_names_wave794_ok: bool,
    pub host_carpet_bomb_flight_dual_peel_nav_commands_wave794_ok: bool,
    pub host_carpet_bomb_flight_dual_peel_live_wave794_ok: bool,
    pub host_leaflet_b52_flight_dual_peel_method_names_wave795_ok: bool,
    pub host_leaflet_b52_flight_dual_peel_nav_commands_wave795_ok: bool,
    pub host_leaflet_b52_flight_dual_peel_live_wave795_ok: bool,
    pub host_paradrop_cargo_flight_dual_peel_method_names_wave796_ok: bool,
    pub host_paradrop_cargo_flight_dual_peel_nav_commands_wave796_ok: bool,
    pub host_paradrop_cargo_flight_dual_peel_live_wave796_ok: bool,
    pub host_aurora_bomb_projectile_dual_peel_method_names_wave797_ok: bool,
    pub host_aurora_bomb_projectile_dual_peel_nav_commands_wave797_ok: bool,
    pub host_aurora_bomb_projectile_dual_peel_live_wave797_ok: bool,
    pub host_toxin_stream_projectile_dual_peel_method_names_wave798_ok: bool,
    pub host_toxin_stream_projectile_dual_peel_nav_commands_wave798_ok: bool,
    pub host_toxin_stream_projectile_dual_peel_live_wave798_ok: bool,
    pub host_angry_mob_projectile_dual_peel_method_names_wave799_ok: bool,
    pub host_angry_mob_projectile_dual_peel_nav_commands_wave799_ok: bool,
    pub host_angry_mob_projectile_dual_peel_live_wave799_ok: bool,
    pub host_cannon_shell_projectile_dual_peel_method_names_wave800_ok: bool,
    pub host_cannon_shell_projectile_dual_peel_nav_commands_wave800_ok: bool,
    pub host_cannon_shell_projectile_dual_peel_live_wave800_ok: bool,
    pub host_angry_mob_member_follow_dual_peel_method_names_wave801_ok: bool,
    pub host_angry_mob_member_follow_dual_peel_nav_commands_wave801_ok: bool,
    pub host_angry_mob_member_follow_dual_peel_live_wave801_ok: bool,
    pub host_field_object_expire_dual_peel_method_names_wave802_ok: bool,
    pub host_field_object_expire_dual_peel_nav_commands_wave802_ok: bool,
    pub host_field_object_expire_dual_peel_live_wave802_ok: bool,
    pub host_inferno_shell_spy_ping_dual_peel_method_names_wave803_ok: bool,
    pub host_inferno_shell_spy_ping_dual_peel_nav_commands_wave803_ok: bool,
    pub host_inferno_shell_spy_ping_dual_peel_live_wave803_ok: bool,
    pub host_flashbang_comanche_helix_dual_peel_method_names_wave804_ok: bool,
    pub host_flashbang_comanche_helix_dual_peel_nav_commands_wave804_ok: bool,
    pub host_flashbang_comanche_helix_dual_peel_live_wave804_ok: bool,
    pub host_scorpion_missile_dual_peel_method_names_wave805_ok: bool,
    pub host_scorpion_missile_dual_peel_nav_commands_wave805_ok: bool,
    pub host_scorpion_missile_dual_peel_live_wave805_ok: bool,
    pub host_beam_flare_shell_dual_peel_method_names_wave806_ok: bool,
    pub host_beam_flare_shell_dual_peel_nav_commands_wave806_ok: bool,
    pub host_beam_flare_shell_dual_peel_live_wave806_ok: bool,
    pub host_sticky_booby_attach_dual_peel_method_names_wave807_ok: bool,
    pub host_sticky_booby_attach_dual_peel_nav_commands_wave807_ok: bool,
    pub host_sticky_booby_attach_dual_peel_live_wave807_ok: bool,
    pub host_particle_laser_object_dual_peel_method_names_wave808_ok: bool,
    pub host_particle_laser_object_dual_peel_nav_commands_wave808_ok: bool,
    pub host_particle_laser_object_dual_peel_live_wave808_ok: bool,
    pub host_firewall_radar_dual_peel_method_names_wave809_ok: bool,
    pub host_firewall_radar_dual_peel_nav_commands_wave809_ok: bool,
    pub host_firewall_radar_dual_peel_live_wave809_ok: bool,
    pub host_power_plant_rods_dual_peel_method_names_wave810_ok: bool,
    pub host_power_plant_rods_dual_peel_nav_commands_wave810_ok: bool,
    pub host_power_plant_rods_dual_peel_live_wave810_ok: bool,
    pub host_power_disabled_dual_peel_method_names_wave811_ok: bool,
    pub host_power_disabled_dual_peel_nav_commands_wave811_ok: bool,
    pub host_power_disabled_dual_peel_live_wave811_ok: bool,
    pub host_battlemaster_horde_dual_peel_method_names_wave812_ok: bool,
    pub host_battlemaster_horde_dual_peel_nav_commands_wave812_ok: bool,
    pub host_battlemaster_horde_dual_peel_live_wave812_ok: bool,
    pub host_china_infantry_horde_dual_peel_method_names_wave813_ok: bool,
    pub host_china_infantry_horde_dual_peel_nav_commands_wave813_ok: bool,
    pub host_china_infantry_horde_dual_peel_live_wave813_ok: bool,
    pub host_stinger_hive_dual_peel_method_names_wave814_ok: bool,
    pub host_stinger_hive_dual_peel_nav_commands_wave814_ok: bool,
    pub host_stinger_hive_dual_peel_live_wave814_ok: bool,
    pub host_actively_constructing_dual_peel_method_names_wave815_ok: bool,
    pub host_actively_constructing_dual_peel_nav_commands_wave815_ok: bool,
    pub host_actively_constructing_dual_peel_live_wave815_ok: bool,
    pub host_player_alive_dual_peel_method_names_wave816_ok: bool,
    pub host_player_alive_dual_peel_nav_commands_wave816_ok: bool,
    pub host_player_alive_dual_peel_live_wave816_ok: bool,
    pub host_money_crate_delete_dual_peel_method_names_wave817_ok: bool,
    pub host_money_crate_delete_dual_peel_nav_commands_wave817_ok: bool,
    pub host_money_crate_delete_dual_peel_live_wave817_ok: bool,
    pub host_player_radar_dual_peel_method_names_wave818_ok: bool,
    pub host_player_radar_dual_peel_nav_commands_wave818_ok: bool,
    pub host_player_radar_dual_peel_live_wave818_ok: bool,
    pub host_dozer_bored_dual_peel_method_names_wave819_ok: bool,
    pub host_dozer_bored_dual_peel_nav_commands_wave819_ok: bool,
    pub host_dozer_bored_dual_peel_live_wave819_ok: bool,
    pub host_fire_spread_dual_peel_method_names_wave820_ok: bool,
    pub host_fire_spread_dual_peel_nav_commands_wave820_ok: bool,
    pub host_fire_spread_dual_peel_live_wave820_ok: bool,
    pub host_auto_deposit_dual_peel_method_names_wave821_ok: bool,
    pub host_auto_deposit_dual_peel_nav_commands_wave821_ok: bool,
    pub host_auto_deposit_dual_peel_live_wave821_ok: bool,
    pub host_hacker_income_dual_peel_method_names_wave822_ok: bool,
    pub host_hacker_income_dual_peel_nav_commands_wave822_ok: bool,
    pub host_hacker_income_dual_peel_live_wave822_ok: bool,
    pub host_patriot_laser_dual_peel_method_names_wave823_ok: bool,
    pub host_patriot_laser_dual_peel_nav_commands_wave823_ok: bool,
    pub host_patriot_laser_dual_peel_live_wave823_ok: bool,
    pub host_pending_patriot_dual_peel_method_names_wave824_ok: bool,
    pub host_pending_patriot_dual_peel_nav_commands_wave824_ok: bool,
    pub host_pending_patriot_dual_peel_live_wave824_ok: bool,
    pub host_zone_damage_dual_peel_method_names_wave825_ok: bool,
    pub host_zone_damage_dual_peel_nav_commands_wave825_ok: bool,
    pub host_zone_damage_dual_peel_live_wave825_ok: bool,
    pub host_combat_field_dual_peel_method_names_wave826_ok: bool,
    pub host_combat_field_dual_peel_nav_commands_wave826_ok: bool,
    pub host_combat_field_dual_peel_live_wave826_ok: bool,
    pub host_systems_dual_peel_method_names_wave827_ok: bool,
    pub host_systems_dual_peel_nav_commands_wave827_ok: bool,
    pub host_systems_dual_peel_live_wave827_ok: bool,
    pub host_actively_constructing_complete_peel_method_names_wave828_ok: bool,
    pub host_actively_constructing_complete_peel_nav_commands_wave828_ok: bool,
    pub host_actively_constructing_complete_peel_live_wave828_ok: bool,
    pub host_build_edge_margin_method_names_wave829_ok: bool,
    pub host_build_edge_margin_nav_commands_wave829_ok: bool,
    pub host_build_edge_margin_live_wave829_ok: bool,
    pub host_map_primary_enemy_method_names_wave830_ok: bool,
    pub host_map_primary_enemy_nav_commands_wave830_ok: bool,
    pub host_map_primary_enemy_live_wave830_ok: bool,
    pub host_map_start_army_spawn_method_names_wave831_ok: bool,
    pub host_map_start_army_spawn_nav_commands_wave831_ok: bool,
    pub host_map_start_army_spawn_live_wave831_ok: bool,
    pub host_starting_units_table_method_names_wave832_ok: bool,
    pub host_starting_units_table_nav_commands_wave832_ok: bool,
    pub host_starting_units_table_live_wave832_ok: bool,
    pub host_exec_smoke_release_prefer_method_names_wave833_ok: bool,
    pub host_exec_smoke_release_prefer_nav_commands_wave833_ok: bool,
    pub host_exec_smoke_release_prefer_live_wave833_ok: bool,
    pub host_train_auto_target_host_fallback_method_names_wave834_ok: bool,
    pub host_train_auto_target_host_fallback_nav_commands_wave834_ok: bool,
    pub host_train_auto_target_host_fallback_live_wave834_ok: bool,
    pub host_skirmish_wnd_latch_peels_method_names_wave835_ok: bool,
    pub host_skirmish_wnd_latch_peels_nav_commands_wave835_ok: bool,
    pub host_skirmish_wnd_latch_peels_live_wave835_ok: bool,
    pub host_skirmish_map_force_lone_eagle_method_names_wave837_ok: bool,
    pub host_skirmish_map_force_lone_eagle_nav_commands_wave837_ok: bool,
    pub host_skirmish_map_force_lone_eagle_live_wave837_ok: bool,
    pub presentation_empty_shadow_failopen_method_names_wave838_ok: bool,
    pub presentation_empty_shadow_failopen_nav_commands_wave838_ok: bool,
    pub presentation_empty_shadow_failopen_live_wave838_ok: bool,
    pub host_vertical_render_mesh_gate_method_names_wave839_ok: bool,
    pub host_vertical_render_mesh_gate_nav_commands_wave839_ok: bool,
    pub host_vertical_render_mesh_gate_live_wave839_ok: bool,
    pub host_skirmish_map_reject_shell_method_names_wave840_ok: bool,
    pub host_skirmish_map_reject_shell_nav_commands_wave840_ok: bool,
    pub host_skirmish_map_reject_shell_live_wave840_ok: bool,
    pub presentation_mouse_ingame_failclosed_method_names_wave841_ok: bool,
    pub presentation_mouse_ingame_failclosed_nav_commands_wave841_ok: bool,
    pub presentation_mouse_ingame_failclosed_live_wave841_ok: bool,
    pub host_match_game_mode_method_names_wave842_ok: bool,
    pub host_match_game_mode_nav_commands_wave842_ok: bool,
    pub host_match_game_mode_live_wave842_ok: bool,
    pub host_match_presentation_residuals_method_names_wave843_ok: bool,
    pub host_match_presentation_residuals_nav_commands_wave843_ok: bool,
    pub host_match_presentation_residuals_live_wave843_ok: bool,
    pub host_match_sim_timing_residuals_method_names_wave844_ok: bool,
    pub host_match_sim_timing_residuals_nav_commands_wave844_ok: bool,
    pub host_match_sim_timing_residuals_live_wave844_ok: bool,
    pub host_match_shell_team_residuals_method_names_wave845_ok: bool,
    pub host_match_shell_team_residuals_nav_commands_wave845_ok: bool,
    pub host_match_shell_team_residuals_live_wave845_ok: bool,
    pub host_match_diplomacy_template_residuals_method_names_wave846_ok: bool,
    pub host_match_diplomacy_template_residuals_nav_commands_wave846_ok: bool,
    pub host_match_diplomacy_template_residuals_live_wave846_ok: bool,
    pub host_match_camera_follow_residuals_method_names_wave847_ok: bool,
    pub host_match_camera_follow_residuals_nav_commands_wave847_ok: bool,
    pub host_match_camera_follow_residuals_live_wave847_ok: bool,
    pub host_train_producer_residual_method_names_wave848_ok: bool,
    pub host_train_producer_residual_nav_commands_wave848_ok: bool,
    pub host_train_producer_residual_live_wave848_ok: bool,
    pub host_match_outcome_residuals_method_names_wave849_ok: bool,
    pub host_match_outcome_residuals_nav_commands_wave849_ok: bool,
    pub host_match_outcome_residuals_live_wave849_ok: bool,
    pub host_match_selection_residuals_method_names_wave850_ok: bool,
    pub host_match_selection_residuals_nav_commands_wave850_ok: bool,
    pub host_match_selection_residuals_live_wave850_ok: bool,
    pub host_match_alive_object_residuals_method_names_wave851_ok: bool,
    pub host_match_alive_object_residuals_nav_commands_wave851_ok: bool,
    pub host_match_alive_object_residuals_live_wave851_ok: bool,
    pub host_match_purchasable_science_residuals_method_names_wave852_ok: bool,
    pub host_match_purchasable_science_residuals_nav_commands_wave852_ok: bool,
    pub host_match_purchasable_science_residuals_live_wave852_ok: bool,
    pub host_object_scan_unify_method_names_wave853_ok: bool,
    pub host_object_scan_unify_nav_commands_wave853_ok: bool,
    pub host_object_scan_unify_live_wave853_ok: bool,
    pub host_match_special_power_ready_residuals_method_names_wave854_ok: bool,
    pub host_match_special_power_ready_residuals_nav_commands_wave854_ok: bool,
    pub host_match_special_power_ready_residuals_live_wave854_ok: bool,
    pub host_boot_victory_condition_residual_method_names_wave855_ok: bool,
    pub host_boot_victory_condition_residual_nav_commands_wave855_ok: bool,
    pub host_boot_victory_condition_residual_live_wave855_ok: bool,
    pub host_sell_auto_target_residual_method_names_wave856_ok: bool,
    pub host_sell_auto_target_residual_nav_commands_wave856_ok: bool,
    pub host_sell_auto_target_residual_live_wave856_ok: bool,
    pub host_special_power_scan_unify_method_names_wave857_ok: bool,
    pub host_special_power_scan_unify_nav_commands_wave857_ok: bool,
    pub host_special_power_scan_unify_live_wave857_ok: bool,
    pub host_script_camera_residuals_method_names_wave858_ok: bool,
    pub host_script_camera_residuals_nav_commands_wave858_ok: bool,
    pub host_script_camera_residuals_live_wave858_ok: bool,
    pub host_residual_failclosed_peels_method_names_wave859_ok: bool,
    pub host_residual_failclosed_peels_nav_commands_wave859_ok: bool,
    pub host_residual_failclosed_peels_live_wave859_ok: bool,
    pub host_map_name_failclosed_method_names_wave860_ok: bool,
    pub host_map_name_failclosed_nav_commands_wave860_ok: bool,
    pub host_map_name_failclosed_live_wave860_ok: bool,
    pub host_multiplayer_science_failclosed_method_names_wave861_ok: bool,
    pub host_multiplayer_science_failclosed_nav_commands_wave861_ok: bool,
    pub host_multiplayer_science_failclosed_live_wave861_ok: bool,
    pub host_world_bounds_ui_residual_method_names_wave862_ok: bool,
    pub host_world_bounds_ui_residual_nav_commands_wave862_ok: bool,
    pub host_world_bounds_ui_residual_live_wave862_ok: bool,
    pub host_first_opponent_residual_method_names_wave863_ok: bool,
    pub host_first_opponent_residual_nav_commands_wave863_ok: bool,
    pub host_first_opponent_residual_live_wave863_ok: bool,
    pub exec_smoke_early_combat_method_names_wave864_ok: bool,
    pub exec_smoke_early_combat_nav_commands_wave864_ok: bool,
    pub exec_smoke_early_combat_live_wave864_ok: bool,
    pub host_camera_drain_freeze_skip_method_names_wave865_ok: bool,
    pub host_camera_drain_freeze_skip_nav_commands_wave865_ok: bool,
    pub host_camera_drain_freeze_skip_live_wave865_ok: bool,
    pub host_selection_stamp_method_names_wave866_ok: bool,
    pub host_selection_stamp_nav_commands_wave866_ok: bool,
    pub host_selection_stamp_live_wave866_ok: bool,
    pub host_mutation_residual_refresh_method_names_wave867_ok: bool,
    pub host_mutation_residual_refresh_nav_commands_wave867_ok: bool,
    pub host_mutation_residual_refresh_live_wave867_ok: bool,
    pub host_science_points_method_names_wave868_ok: bool,
    pub host_science_points_nav_commands_wave868_ok: bool,
    pub host_science_points_live_wave868_ok: bool,
    pub host_boot_ui_freeze_route_method_names_wave869_ok: bool,
    pub host_boot_ui_freeze_route_nav_commands_wave869_ok: bool,
    pub host_boot_ui_freeze_route_live_wave869_ok: bool,
    pub host_sim_timing_stamp_method_names_wave870_ok: bool,
    pub host_sim_timing_stamp_nav_commands_wave870_ok: bool,
    pub host_sim_timing_stamp_live_wave870_ok: bool,
    pub host_match_residual_clear_method_names_wave871_ok: bool,
    pub host_match_residual_clear_nav_commands_wave871_ok: bool,
    pub host_match_residual_clear_live_wave871_ok: bool,
    pub host_template_ui_method_names_wave872_ok: bool,
    pub host_template_ui_nav_commands_wave872_ok: bool,
    pub host_template_ui_live_wave872_ok: bool,
    pub host_queue_stamp_method_names_wave874_ok: bool,
    pub host_queue_stamp_nav_commands_wave874_ok: bool,
    pub host_queue_stamp_live_wave874_ok: bool,
    pub host_dual_read_zero_sole_tick_method_names_wave875_ok: bool,
    pub host_dual_read_zero_sole_tick_nav_commands_wave875_ok: bool,
    pub host_dual_read_zero_sole_tick_live_wave875_ok: bool,
    pub host_shell_no_dual_pace_method_names_wave876_ok: bool,
    pub host_shell_no_dual_pace_nav_commands_wave876_ok: bool,
    pub host_shell_no_dual_pace_live_wave876_ok: bool,
    pub host_gw_flight_over_assign_method_names_wave877_ok: bool,
    pub host_gw_flight_over_assign_nav_commands_wave877_ok: bool,
    pub host_gw_flight_over_assign_live_wave877_ok: bool,
    pub host_ci_clippy_peel_method_names_wave878_ok: bool,
    pub host_ci_clippy_peel_nav_commands_wave878_ok: bool,
    pub host_ci_clippy_peel_live_wave878_ok: bool,
    pub host_wwdownload_clippy_method_names_wave879_ok: bool,
    pub host_wwdownload_clippy_nav_commands_wave879_ok: bool,
    pub host_wwdownload_clippy_live_wave879_ok: bool,
    pub host_ui_pres_rebuild_method_names_wave880_ok: bool,
    pub host_ui_pres_rebuild_nav_commands_wave880_ok: bool,
    pub host_ui_pres_rebuild_live_wave880_ok: bool,
    pub host_ui_framework_clippy_method_names_wave881_ok: bool,
    pub host_ui_framework_clippy_nav_commands_wave881_ok: bool,
    pub host_ui_framework_clippy_live_wave881_ok: bool,
    pub host_assets_big_unpack_method_names_wave882_ok: bool,
    pub host_assets_big_unpack_nav_commands_wave882_ok: bool,
    pub host_assets_big_unpack_live_wave882_ok: bool,
    pub host_wwshade_clippy_method_names_wave883_ok: bool,
    pub host_wwshade_clippy_nav_commands_wave883_ok: bool,
    pub host_wwshade_clippy_live_wave883_ok: bool,
    pub host_zlib_asset_debug_method_names_wave884_ok: bool,
    pub host_zlib_asset_debug_nav_commands_wave884_ok: bool,
    pub host_zlib_asset_debug_live_wave884_ok: bool,
    pub host_profile_clippy_method_names_wave885_ok: bool,
    pub host_profile_clippy_nav_commands_wave885_ok: bool,
    pub host_profile_clippy_live_wave885_ok: bool,
    pub host_ww3d_particles_anim_gui_method_names_wave886_ok: bool,
    pub host_ww3d_particles_anim_gui_nav_commands_wave886_ok: bool,
    pub host_ww3d_particles_anim_gui_live_wave886_ok: bool,
    pub host_particle_world_builder_method_names_wave887_ok: bool,
    pub host_particle_world_builder_nav_commands_wave887_ok: bool,
    pub host_particle_world_builder_live_wave887_ok: bool,
    pub host_wwlib_map_cache_method_names_wave888_ok: bool,
    pub host_wwlib_map_cache_nav_commands_wave888_ok: bool,
    pub host_wwlib_map_cache_live_wave888_ok: bool,
    pub host_wp_audio_clippy_method_names_wave889_ok: bool,
    pub host_wp_audio_clippy_nav_commands_wave889_ok: bool,
    pub host_wp_audio_clippy_live_wave889_ok: bool,
    pub host_remaining_clippy_method_names_wave890_ok: bool,
    pub host_remaining_clippy_nav_commands_wave890_ok: bool,
    pub host_remaining_clippy_live_wave890_ok: bool,
    pub host_override_camera_follow_method_names_wave891_ok: bool,
    pub host_override_camera_follow_nav_commands_wave891_ok: bool,
    pub host_override_camera_follow_live_wave891_ok: bool,
    pub host_pause_boot_player_method_names_wave892_ok: bool,
    pub host_pause_boot_player_nav_commands_wave892_ok: bool,
    pub host_pause_boot_player_live_wave892_ok: bool,
    pub host_sim_timing_presentation_method_names_wave893_ok: bool,
    pub host_sim_timing_presentation_nav_commands_wave893_ok: bool,
    pub host_sim_timing_presentation_live_wave893_ok: bool,
    pub host_sciences_ai_method_names_wave894_ok: bool,
    pub host_sciences_ai_nav_commands_wave894_ok: bool,
    pub host_sciences_ai_live_wave894_ok: bool,
    pub host_pob_failclosed_boot_method_names_wave895_ok: bool,
    pub host_pob_failclosed_boot_nav_commands_wave895_ok: bool,
    pub host_pob_failclosed_boot_live_wave895_ok: bool,
    pub host_map_shell_failclosed_method_names_wave896_ok: bool,
    pub host_map_shell_failclosed_nav_commands_wave896_ok: bool,
    pub host_map_shell_failclosed_live_wave896_ok: bool,
    pub host_boot_player_alive_science_method_names_wave897_ok: bool,
    pub host_boot_player_alive_science_nav_commands_wave897_ok: bool,
    pub host_boot_player_alive_science_live_wave897_ok: bool,
    pub host_observe_failclosed_method_names_wave898_ok: bool,
    pub host_observe_failclosed_nav_commands_wave898_ok: bool,
    pub host_observe_failclosed_live_wave898_ok: bool,
    pub host_boot_camera_ui_failclosed_method_names_wave899_ok: bool,
    pub host_boot_camera_ui_failclosed_nav_commands_wave899_ok: bool,
    pub host_boot_camera_ui_failclosed_live_wave899_ok: bool,
    pub host_event_drain_failclosed_method_names_wave900_ok: bool,
    pub host_event_drain_failclosed_nav_commands_wave900_ok: bool,
    pub host_event_drain_failclosed_live_wave900_ok: bool,
    pub host_refresh_sim_failclosed_method_names_wave901_ok: bool,
    pub host_refresh_sim_failclosed_nav_commands_wave901_ok: bool,
    pub host_refresh_sim_failclosed_live_wave901_ok: bool,
    pub host_selection_stamp_train_method_names_wave902_ok: bool,
    pub host_selection_stamp_train_nav_commands_wave902_ok: bool,
    pub host_selection_stamp_train_live_wave902_ok: bool,
    pub host_camera_focus_failclosed_method_names_wave903_ok: bool,
    pub host_camera_focus_failclosed_nav_commands_wave903_ok: bool,
    pub host_camera_focus_failclosed_live_wave903_ok: bool,
    pub host_single_authority_camera_method_names_wave904_ok: bool,
    pub host_single_authority_camera_nav_commands_wave904_ok: bool,
    pub host_single_authority_camera_live_wave904_ok: bool,
    pub host_ui_observe_failclosed_method_names_wave905_ok: bool,
    pub host_ui_observe_failclosed_nav_commands_wave905_ok: bool,
    pub host_ui_observe_failclosed_live_wave905_ok: bool,
    pub host_mouse_presentation_only_method_names_wave906_ok: bool,
    pub host_mouse_presentation_only_nav_commands_wave906_ok: bool,
    pub host_mouse_presentation_only_live_wave906_ok: bool,
    pub host_victory_fps_failclosed_method_names_wave907_ok: bool,
    pub host_victory_fps_failclosed_nav_commands_wave907_ok: bool,
    pub host_victory_fps_failclosed_live_wave907_ok: bool,
    pub host_sim_timing_snapshot_method_names_wave908_ok: bool,
    pub host_sim_timing_snapshot_nav_commands_wave908_ok: bool,
    pub host_sim_timing_snapshot_live_wave908_ok: bool,
    pub host_cold_stamp_supplies_failclosed_method_names_wave909_ok: bool,
    pub host_cold_stamp_supplies_failclosed_nav_commands_wave909_ok: bool,
    pub host_cold_stamp_supplies_failclosed_live_wave909_ok: bool,
    pub host_victory_fps_legal_failclosed_method_names_wave910_ok: bool,
    pub host_victory_fps_legal_failclosed_nav_commands_wave910_ok: bool,
    pub host_victory_fps_legal_failclosed_live_wave910_ok: bool,
    pub host_legal_build_cache_method_names_wave911_ok: bool,
    pub host_legal_build_cache_nav_commands_wave911_ok: bool,
    pub host_legal_build_cache_live_wave911_ok: bool,
    pub host_destroy_list_if_needed_method_names_wave912_ok: bool,
    pub host_destroy_list_if_needed_nav_commands_wave912_ok: bool,
    pub host_destroy_list_if_needed_live_wave912_ok: bool,
    pub host_redundant_authority_write_skip_method_names_wave913_ok: bool,
    pub host_redundant_authority_write_skip_nav_commands_wave913_ok: bool,
    pub host_redundant_authority_write_skip_live_wave913_ok: bool,
    pub host_process_commands_if_needed_method_names_wave914_ok: bool,
    pub host_process_commands_if_needed_nav_commands_wave914_ok: bool,
    pub host_process_commands_if_needed_live_wave914_ok: bool,
    pub host_process_sfx_world_template_peels_method_names_wave915_ok: bool,
    pub host_process_sfx_world_template_peels_nav_commands_wave915_ok: bool,
    pub host_process_sfx_world_template_peels_live_wave915_ok: bool,
    pub host_dual_tick_queue_destroy_peels_method_names_wave916_ok: bool,
    pub host_dual_tick_queue_destroy_peels_nav_commands_wave916_ok: bool,
    pub host_dual_tick_queue_destroy_peels_live_wave916_ok: bool,
    pub host_command_barracks_complete_peels_method_names_wave917_ok: bool,
    pub host_command_barracks_complete_peels_nav_commands_wave917_ok: bool,
    pub host_command_barracks_complete_peels_live_wave917_ok: bool,
    pub host_load_path_stamp_peels_method_names_wave918_ok: bool,
    pub host_load_path_stamp_peels_nav_commands_wave918_ok: bool,
    pub host_load_path_stamp_peels_live_wave918_ok: bool,
    pub host_paused_tick_guard_refresh_peels_method_names_wave919_ok: bool,
    pub host_paused_tick_guard_refresh_peels_nav_commands_wave919_ok: bool,
    pub host_paused_tick_guard_refresh_peels_live_wave919_ok: bool,
    pub host_producer_refresh_freeze_peels_method_names_wave920_ok: bool,
    pub host_producer_refresh_freeze_peels_nav_commands_wave920_ok: bool,
    pub host_producer_refresh_freeze_peels_live_wave920_ok: bool,
    pub host_start_faction_supplies_method_names_wave921_ok: bool,
    pub host_start_faction_supplies_nav_commands_wave921_ok: bool,
    pub host_start_faction_supplies_live_wave921_ok: bool,
    pub host_load_queue_process_boundaries_method_names_wave922_ok: bool,
    pub host_load_queue_process_boundaries_nav_commands_wave922_ok: bool,
    pub host_load_queue_process_boundaries_live_wave922_ok: bool,
    pub host_tick_logic_frame_boundary_method_names_wave923_ok: bool,
    pub host_tick_logic_frame_boundary_nav_commands_wave923_ok: bool,
    pub host_tick_logic_frame_boundary_live_wave923_ok: bool,
    pub host_placement_legal_build_cache_method_names_wave924_ok: bool,
    pub host_placement_legal_build_cache_nav_commands_wave924_ok: bool,
    pub host_placement_legal_build_cache_live_wave924_ok: bool,
    pub host_eager_apply_batch_method_names_wave925_ok: bool,
    pub host_eager_apply_batch_nav_commands_wave925_ok: bool,
    pub host_eager_apply_batch_live_wave925_ok: bool,
    pub host_presentation_build_boundary_method_names_wave926_ok: bool,
    pub host_presentation_build_boundary_nav_commands_wave926_ok: bool,
    pub host_presentation_build_boundary_live_wave926_ok: bool,
    pub host_post_logic_shadow_boundary_method_names_wave927_ok: bool,
    pub host_post_logic_shadow_boundary_nav_commands_wave927_ok: bool,
    pub host_post_logic_shadow_boundary_live_wave927_ok: bool,
    pub host_save_load_skirmish_boundaries_method_names_wave928_ok: bool,
    pub host_save_load_skirmish_boundaries_nav_commands_wave928_ok: bool,
    pub host_save_load_skirmish_boundaries_live_wave928_ok: bool,
    pub host_direct_order_boundary_method_names_wave929_ok: bool,
    pub host_direct_order_boundary_nav_commands_wave929_ok: bool,
    pub host_direct_order_boundary_live_wave929_ok: bool,
    pub host_direct_order_gamelogic_boundary_method_names_wave930_ok: bool,
    pub host_direct_order_gamelogic_boundary_nav_commands_wave930_ok: bool,
    pub host_direct_order_gamelogic_boundary_live_wave930_ok: bool,
    pub host_object_lifecycle_boundary_method_names_wave931_ok: bool,
    pub host_object_lifecycle_boundary_nav_commands_wave931_ok: bool,
    pub host_object_lifecycle_boundary_live_wave931_ok: bool,
    pub host_command_pipeline_boundary_method_names_wave932_ok: bool,
    pub host_command_pipeline_boundary_nav_commands_wave932_ok: bool,
    pub host_command_pipeline_boundary_live_wave932_ok: bool,
    pub host_session_control_boundary_method_names_wave933_ok: bool,
    pub host_session_control_boundary_nav_commands_wave933_ok: bool,
    pub host_session_control_boundary_live_wave933_ok: bool,
    pub host_support_boundary_method_names_wave934_ok: bool,
    pub host_support_boundary_nav_commands_wave934_ok: bool,
    pub host_support_boundary_live_wave934_ok: bool,
    pub host_gamelogic_borrow_boundary_method_names_wave935_ok: bool,
    pub host_gamelogic_borrow_boundary_nav_commands_wave935_ok: bool,
    pub host_gamelogic_borrow_boundary_live_wave935_ok: bool,
    pub host_sole_authority_surface_method_names_wave936_ok: bool,
    pub host_sole_authority_surface_nav_commands_wave936_ok: bool,
    pub host_sole_authority_surface_live_wave936_ok: bool,
    pub host_production_authority_boundary_method_names_wave937_ok: bool,
    pub host_production_authority_boundary_nav_commands_wave937_ok: bool,
    pub host_production_authority_boundary_live_wave937_ok: bool,
    pub host_post_writeback_complete_boundary_method_names_wave938_ok: bool,
    pub host_post_writeback_complete_boundary_nav_commands_wave938_ok: bool,
    pub host_post_writeback_complete_boundary_live_wave938_ok: bool,
    pub host_ready_log_drain_boundary_method_names_wave939_ok: bool,
    pub host_ready_log_drain_boundary_nav_commands_wave939_ok: bool,
    pub host_ready_log_drain_boundary_live_wave939_ok: bool,
    pub host_sole_tick_object_id_boundary_method_names_wave940_ok: bool,
    pub host_sole_tick_object_id_boundary_nav_commands_wave940_ok: bool,
    pub host_sole_tick_object_id_boundary_live_wave940_ok: bool,
    pub host_residual_mutation_boundary_method_names_wave941_ok: bool,
    pub host_residual_mutation_boundary_nav_commands_wave941_ok: bool,
    pub host_residual_mutation_boundary_live_wave941_ok: bool,
    /// Shell Skirmish → Loading → GameHUD ownership transition (StartGame parity).
    pub screen_skirmish_ok: bool,
    /// ControlBar.wnd resolve/validate path (C++ ShowControlBar / ensure_gameplay_layouts).
    /// True when layout Ready, or assets honestly unavailable (CI without WindowZH).
    pub control_bar_layout_ok: bool,
    /// ControlBar.wnd path found on disk.
    pub control_bar_path_resolved: bool,
    /// ControlBar.wnd structural validate (FILE_VERSION / WINDOW / ControlBar tokens).
    pub control_bar_wnd_validated: bool,
    /// Headless WindowManager parse materialised GameWindows (assets present path).
    /// False when WindowZH missing or parse deferred — still honest residual.
    pub control_bar_window_loaded: bool,
    /// Window count from headless WindowManager load (0 when not loaded).
    pub control_bar_window_count: usize,
    /// Dual-tick residual: selection panel applied to HUD + UIState + RTS + command panel.
    pub selection_consumers_ok: bool,
    /// Limited headless host claim (see module docs). Not retail W3D play.
    pub shell_host_playable_ok: bool,
    /// Always false here: no window/WND/GPU retail playthrough.
    pub playable_claim: bool,
    pub status: String,
    pub detail: String,
}

