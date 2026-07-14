## Residual Host Playability — Wave 105: AI group / AI path / weapon fire / damage / veterancy residual peels (2026-07-14)

**Closed (host-testable residual peels; orthogonal to Object/ThingFactory Waves 100–104):**
1. **AI group residual peels** (`host_ai_path_combat_residual_wave105`):
   - AIGroup ctor residual: speed **0**, dirty **false**, member count **0**, groundPath null.
   - Membership add/remove/isMember/getCount residual; getSpeed = min member speed; getCenter average.
   - AIData.ini group path residual: MinInfantry/VehiclesForGroup **3**, MinDistance **100**,
     DistanceRequiresGroup **500**, MinClumpDensity **0.5**, SkirmishGroupFudge **5**,
     GroupMoveClickToGather **0.5**, ForceIdleMSEC **67** → **2**f.
   - CommandSourceType residual **4** names (PLAYER/SCRIPT/AI/DOZER).
   - AIGroup group* command residual surface **≥40** names.
   - Honesty: `honesty_ai_group_residual_pack_wave105`.
2. **AI path residual deepen**:
   - PATHFIND_CELL_SIZE **10** / CLOSE_ENOUGH **1.0** / CELLS_PER_FRAME **5000** /
     PATH_MAX_PRIORITY **0x7FFFFFFF** / MAX_WALL_PIECES **128** / MAX_CPOP **20**.
   - CellType residual **7** names CLEAR..IMPASSABLE; CellFlags residual occupancy table.
   - PathfindLayerEnum LAYER_INVALID **0** / GROUND **1** / WALL **15**.
   - Infantry/Vehicle PathfindDiameter **6**; world↔cell residual floor(/10).
   - Path residual append/prepend/length/optimize bookkeeping.
   - Honesty: `honesty_ai_path_residual_deepen_pack_wave105`.
3. **Weapon fire residual deepen** (Weapon.cpp / WeaponStatus.h residual):
   - WeaponStatus residual **5** names READY..PRE_ATTACK; NO_MAX_SHOTS_LIMIT **0x7fffffff**.
   - WeaponBonus Field residual **5** (DAMAGE/RADIUS/RANGE/RATE_OF_FIRE/PRE_ATTACK).
   - WeaponReloadType **3** / PrefireType **3**; Anti mask **8** bits; Affects mask **7** bits.
   - privateFireWeapon residual pipeline **10** steps; host fire residual (clip/reload/range/bonus).
   - Honesty: `honesty_weapon_fire_residual_deepen_pack_wave105`.
4. **Damage residual application residual deepen**:
   - BodyDamageType residual **4** (PRISTINE/DAMAGED/REALLYDAMAGED/RUBBLE).
   - UnitDamagedThreshold **0.7** / ReallyDamaged **0.35**; MovementPenalty **REALLYDAMAGED**.
   - calcDamageState residual formula; IsSubdual / IsHealthDamaging residual helpers.
   - apply_damage residual: armor coeff × amount → dealt/clipped/health.
   - Honesty: `honesty_damage_application_residual_deepen_pack_wave105`.
5. **Veterancy residual deepen** (ExperienceTracker / GameData.ini):
   - LEVEL_REGULAR..HEROIC **0..3**, COUNT **4**; HealthBonus **120/130/150%**;
     WeaponBonus DAMAGE **110/120/130%**; ROF **120/140/160%**.
   - ExperienceRequired/Value residual sample rows (Burton/Ranger/MissileDefender/Pathfinder).
   - Tracker residual: ally kill **0** XP, level-up loop, setMinVeterancyLevel, AdvancedTraining ×2 scalar.
   - Honesty: `honesty_veterancy_residual_deepen_pack_wave105`.
6. **Combined pack**: `honesty_ai_path_combat_residual_pack_wave105`.

**Wiring:**
- `game_logic/host_ai_path_combat_residual_wave105.rs` (new)
- `game_logic/mod.rs` — module + pub use honesty
- `shell_smoke.rs` — ai_group105/ai_path105/weapon_fire105/damage_app105/veterancy105 fields
- `shell_smoke_gate.rs` — require wave105 honesty flags; playable_claim stays false

**Gates:**
- Unit: residual_pack_honesty_wave105 tests PASS
- golden_skirmish_gate --frames 8 → playable_claim=true
- shell_smoke_gate → playable_claim=false shell_host_playable_ok=true
  ai_group105=true ai_path105=true weapon_fire105=true damage_app105=true veterancy105=true

**Not claimed:**
- Full AIGroup exclusive group path A* residual
- Full Pathfinder open/closed list exclusive residual
- Full Weapon::privateFireWeapon live projectile residual
- Full ActiveBody attemptDamage exclusive module matrix residual
- Full ExperienceTracker live Object XP sink residual
- shell playable_claim / network (deferred)

**Honesty rules preserved:**
- Shell playable_claim remains **false**
- Golden playable_claim remains **true**
- Network residual deferred

## Residual Host Playability — Wave 106: shell / campaign / save residual deepen (2026-07-14)

**Closed (host-testable residual peels; orthogonal to Wave 103/104 game-logic peels):**
1. **GameState residual deepen** (`host_shell_campaign_save_residual_wave106`):
   - SaveLoadLayoutType residual **4** names (SLLT_INVALID..SLLT_SAVE_ONLY).
   - SNAPSHOT_SAVELOAD CHUNK_* block table **17** (GameState..GhostObject).
   - SNAPSHOT_DEEPCRC_LOGICONLY subset **6** (excludes client/UI chunks).
   - GAME_STATE_BLOCK_STRING / CAMPAIGN_BLOCK_STRING anchors; Save directory leaf.
   - Honesty: `honesty_game_state_residual_deepen_pack_wave106`.
2. **Campaign residual deepen** (mission residual tables):
   - USA / GLA / China **5**-mission map tables (MD_USA* / MD_GLA* / MD_CHI*).
   - TRAINING residual (Training01); CHALLENGE_0 map chain **7**.
   - CampaignNameLabel residual for TRAINING/USA/GLA/China + CHALLENGE_0..8.
   - Honesty: `honesty_campaign_mission_residual_deepen_pack_wave106`.
3. **MainMenu residual deepen**:
   - MainMenu.wnd retail window count **63**; button residual table **≥28**.
   - Faction window residual **16**; shell roots + transition group residual.
   - Host MainMenuState residual **5** names; layout `Menus/MainMenu.wnd`.
   - Honesty: `honesty_main_menu_residual_deepen_pack_wave106`.
4. **GameWindow residual deepen**:
   - WIN_STATUS residual table **28** (NONE + **27** bits through SHORTCUT_BUTTON).
   - GWM_* message residual **27** names; GWM_USER **32768**.
   - MSG_IGNORED / MSG_HANDLED residual.
   - Honesty: `honesty_game_window_residual_deepen_pack_wave106`.
5. **WindowLayout residual deepen**:
   - INIT/UPDATE/SHUTDOWN callback residual; layout operation residual table.
   - Shell layout filename residual table (**≥12** including MainMenu + ControlBar).
   - WindowLayoutPool name + ctor hide/count residual; hide pure residual.
   - Honesty: `honesty_window_layout_residual_deepen_pack_wave106`.
6. **Combined pack**: `honesty_shell_campaign_save_residual_pack_wave106`.

**Wiring:**
- `game_logic/host_shell_campaign_save_residual_wave106.rs` (new)
- `game_logic/mod.rs` — module + pub use honesty
- `shell_smoke.rs` — gamestate106/campaign106/mainmenu106/gamewindow106/layout106
- `shell_smoke_gate.rs` — require wave106 honesty flags; playable_claim stays false

**Gates:**
- Unit: residual_pack_honesty_wave106 tests PASS
- golden_skirmish_gate --frames 8 → playable_claim=true
- shell_smoke_gate → playable_claim=false shell_host_playable_ok=true
  gamestate106=true campaign106=true mainmenu106=true gamewindow106=true layout106=true

**Not claimed:**
- Full GameState xferSaveData file I/O / deep CRC network residual
- Full CampaignManager live INI parse / mission progression residual
- Full MainMenu.wnd W3D TransitionHandler retail UI residual
- Full GameWindow GPU draw / WindowManager exclusive residual
- Full WindowLayout::load .wnd script residual
- shell playable_claim / network (deferred)

**Honesty rules preserved:**
- Shell playable_claim remains **false**
- Golden playable_claim remains **true**
- Network residual deferred

## Residual Host Playability — Wave 103: weapon/armor/locomotor/special-power/KindOf residual peels (2026-07-14)

**Closed (host-testable residual peels; orthogonal to Waves 101/102 ThingFactory/graphics):**
1. **Weapon residual deepen** (`weapon_bootstrap`, beyond Wave 92):
   - **16** deepen residual names: NukeCannon / Inferno / Aurora / FireBase /
     SentryDrone / Hellfire / JarmenKell / TunnelDefender / MiniGunner /
     Overlord / BattleMaster / Comanche AT + rocket pods / Avenger AA /
     SCUD toxin / BlackNapalm.
   - Key damage/range residual (e.g. NukeCannon **400**/r**350**, JarmenKell
     **180**/r**225**, Overlord **80**/r**175**, MiniGunner **10**/r**125**).
   - Honesty: `honesty_weapon_store_deepen_residual_wave103`.
2. **Armor residual expand** (`host_armor_residual`, beyond Wave 92):
   - HazMatHumanArmor / ChemSuitHumanArmor / DozerArmor / UpgradedTankArmor /
     HumveeArmor / DragonTankArmor / ToxinTruckArmor / ComancheArmor /
     StructureArmorTough residual.
   - Key Armor.ini scalars: HazMat POISON **0%** / FLAME **25%**; Dragon FLAME
     **0%**; ToxinTruck POISON **0%**; Humvee JET_MISSILES **30%**; StructureTough
     EXPLOSION **80%**; Comanche EXPLOSION **130%**.
   - Honesty: `honesty_armor_residual_expand_wave103`.
3. **Locomotor residual expand** (`locomotor_bootstrap`, beyond Wave 92):
   - **14** new residual names: BombTruck / TroopCrawler / RadarVan / ToxinTruck /
     Chinook / A10 / B52 / CombatBike / POWTruck / NuclearBattleMaster /
     JarmenKell / BlackLotus / Saboteur / MissileDefender.
   - Retail Speed residual (e.g. Chinook **150**, CombatBike **120**, B52 **125**,
     BombTruck **50**, MissileDefender **20**).
   - Unit template → SET_NORMAL name residual binding expanded.
   - Honesty: `honesty_locomotor_residual_expand_wave103`.
4. **SpecialPower superweapon residual deepen** (`host_game_logic_residual_wave103`):
   - **20** SpecialPower.ini Superweapon residual rows (template / Enum / ReloadTime)
     for powers incomplete on HostSuperweaponKind (MOAB / EMP / Napalm /
     BlackMarketNuke / TerrorCell / CrateDrop / Frenzy / CashHack / DirtyNuke /
     Leaflet / SpySatellite / SpyDrone / RadarVan / EmergencyRepair / GPS / CIA /
     SneakAttack / Ambush / Baikonur / SupW PUC).
   - Enum cross-link to Wave 80 SPECIAL_POWER_BIT_NAME_LIST; ReloadTime → frames.
   - Honesty: `honesty_special_power_superweapon_residual_deepen_wave103`.
5. **Object residual KindOf packs** for more unit types:
   - **17** common unit/structure KindOf residual packs (Ranger / Rebel / Redguard /
     Crusader / BattleMaster / Scorpion / Humvee / Technical / Raptor / Comanche /
     Helix / Overlord / Tomahawk / Hacker / CommandCenter / WarFactory / Barracks).
   - KindOf token residual + BuildCost / BuildTime / MaxHealth residual anchors.
   - Honesty: `honesty_object_kindof_residual_pack_wave103`.
6. **Combined pack**: `honesty_game_logic_residual_pack_wave103`.

**Wiring:**
- `game_logic/host_game_logic_residual_wave103.rs` (new — SP deepen + KindOf + combined)
- `game_logic/weapon_bootstrap.rs` — Wave 103 weapon deepen honesty
- `game_logic/host_armor_residual.rs` — Wave 103 armor expand
- `game_logic/locomotor_bootstrap.rs` — Wave 103 locomotor expand
- `game_logic/mod.rs` — module + pub use honesty
- `shell_smoke.rs` — weapon103/armor103/loco103/sp103/kindof103 fields + detail tokens
- `shell_smoke_gate.rs` — require wave103 honesty flags; playable_claim stays false

**Gates:**
- Unit: residual_pack_honesty_wave103 tests PASS
- golden_skirmish_gate --frames 8 → playable_claim=true
- shell_smoke_gate → playable_claim=false shell_host_playable_ok=true
  weapon103=true armor103=true loco103=true sp103=true kindof103=true

**Not claimed:**
- Full Weapon.ini / Armor.ini / Locomotor.ini archive parse residual
- Full SpecialPowerStore SharedSyncedTimer / PublicTimer UI residual
- Full ThingTemplate KindOf bit matrix / live Object INI parse residual
- shell playable_claim / network (deferred)

**Honesty rules preserved:**
- Shell playable_claim remains **false**
- Golden playable_claim remains **true**
- Network residual deferred

## Residual Host Playability — Wave 102: DisplayString/Anim2D/laser/CSF presentation residual peels (2026-07-14)

**Closed (host-testable residual peels; orthogonal graphics residual toward GPU):**
1. **DisplayString residual deepen** (`floating_text_layout`):
   - FontCharsClass spacing residual table: `CHAR_BUFFER_LEN` **32768**, ASCII array **256**,
     PixelOverlap formula `clamp((-font_height)/8, 0, 4)`, Get_Char_Spacing =
     Width − PixelOverlap − CharOverhang; monospaced Width=8 → spacing=8.
   - ASCII printable 0x20..0x7E spacing residual table (**95** entries).
   - StretchRect submit residual bookkeeping: shadow+text Draw_Sentence host counters,
     optional hotkey submit, Render residual; `gpu_stretch_rect_submitted=false` always.
   - Honesty: `honesty_display_string_residual_deepen_pack_wave102`.
2. **Anim2D residual deepen** (`world_anim_layout`):
   - Full Animation2D.ini template residual table (**14** names) with mode / delay_ms /
     randomize / NumberImages / image-prefix / start-index metadata (not just MoneyPickUp).
   - Collection `init_with_retail_templates` residual registers all 14 with residual metadata.
   - Honesty: `honesty_anim2d_residual_deepen_pack_wave102`.
3. **Laser SegLine residual deepen** (`laser_segment_upload`):
   - Soft-edge UV atlas residual texture bind expand: TextureMapMode UNIFORM_WIDTH **0** /
     TILED **2**, UVOffsetDeltaPerMS = rate×0.001, CurrentUVOffset advance residual,
     atlas bind table EXNoise02 / EXBinaryStream32 / EXLaser.
   - Multi-beam soft-edge residual cross-link retained.
   - Honesty: `honesty_laser_segliner_residual_deepen_pack_wave102`.
4. **Multi-locale CSF residual deepen** (`game_text_residual`):
   - Expanded pack-load residual for all **10** LanguageId (primary 5 + UK/JA/Jabber/KO/Unknown).
   - Fail-closed empty-table honesty when assets absent (CI).
   - Honesty: `honesty_csf_multi_locale_residual_deepen_pack_wave102`.
5. **Presentation residual deepen** (`presentation_frame`):
   - Dual-tick residual deepen: selected_count + particle_count consistency;
     deepen pack cross-links floating/world-anim/laser/spectre/mesh/ground residual.
   - Honesty: `honesty_presentation_residual_deepen_pack_wave102`.
6. **Wiring**:
   - shell_smoke: display102/anim2d102/laser102/csf102/pres102 flags (playable_claim stays false)
   - shell_smoke_gate requires wave102 honesty flags
7. **Tests / gates**:
   - Unit wave102 honesty tests PASS
   - golden_skirmish_gate --frames 8 → PASS playable_claim=true
   - shell_smoke_gate → PASS playable_claim=false with display102=true…pres102=true

**Still residual (fail-closed, not claimed):**
- Full DisplayString GPU font atlas raster / WW3D StretchRect submit
- Full Anim2DCollection GPU texture atlas sample / WW3D Image draw
- Full SegLineRenderer wgpu write_buffer / atlas sampler bind
- Full multi-locale CSF GameTextManager boot UI for all LanguageId
- Shell `playable_claim` remains false (no windowed W3D retail claim)
- Network residual replication (network deferred)

## Residual Host Playability — Wave 101: ModuleFactory / ThingFactory create / Partition register residual peels (2026-07-14)

- Co-present Wave 100 residual (`host_thing_factory_module_xfer_residual`) deepen

**Closed (host-testable residual peels; orthogonal ModuleFactory / ThingFactory create / Partition register):**
1. **ModuleFactory residual deepen** (`host_thing_factory_module_xfer_residual`, beyond Wave 100 type tables):
   - Expanded sample ModuleFactory residual table **≥24** (Wave 100 had **9**).
   - Multi-interface mask composition residual: OpenContain / TunnelContain / MinefieldBehavior /
     AutoHealBehavior / PhysicsBehavior / BridgeBehavior / SpawnBehavior / FXListDie / etc.
   - `module_interface_compose_mask_residual` OR-fold + popcount residual.
   - `ModuleFactoryRegistryResidual` host bookkeeping: addModule / findModuleInterfaceMask /
     m_moduleDataList push / dtor clear residual counters.
   - ModuleData hash residual: NameKey `SOCKET_COUNT` **45007**, `calcHashForString`
     `(result<<5)+result+byte`, decorated-name bucket residual.
   - Empty-name findModule residual still **0**.
   - Honesty: `honesty_module_factory_residual_deepen_pack_wave101`.
2. **ThingFactory create residual deepen** (newObject post-create bookkeeping):
   - Post-create residual steps **5**: GAMELOGIC_CREATE / TEAM_ASSIGN / ON_CREATE_MODULES /
     PARTITION_REGISTER / INIT_OBJECT.
   - `ThingFactoryCreateResidualCounters` residual: null-template reject, drawable-only reject,
     build-variation resolve, create/team/onCreate/partition/init counters + live_object_count.
   - Template `copyFrom` residual: preserves name / template_id / next_link; copies payload;
     `setCopiedFromDefault` residual armor/weapons/modules flags.
   - `findTemplate` residual: case-sensitive name table lookup + AsciiString hash residual
     (`calcHashForString`) honesty; missing+check crash residual flag.
   - Honesty: `honesty_thing_factory_create_residual_deepen_pack_wave101`.
3. **PartitionManager register residual** (host counters; cell size cross-link Wave 96):
   - `PartitionCellSize` residual **40** linked to register residual.
   - registerObject residual steps **5**: SANITY_NULL / REJECT_ALREADY_REG / ALLOC /
     LINK_MODULE_LIST / ATTACH_TO_OBJECT.
   - unRegisterObject residual steps **6** including GHOST_FOG_HOLD.
   - `PartitionRegisterResidualCounters` residual bookkeeping + world→cell residual.
   - Honesty: `honesty_partition_register_residual_pack_wave101`.
4. **Cross-link pack**: CREATE interface + TunnelContain CREATE + post-create order + cell 40.
   - Honesty: `honesty_thing_factory_module_partition_crosslink_wave101`.
5. **Combined pack**: `honesty_thing_factory_module_partition_residual_pack_wave101`
   (includes Wave 100 deepen packs still holding).
6. Tests / gates:
   - Unit: residual_pack_honesty_wave101* tests
   - shell_smoke: module_factory101/thing_factory101/partition_register101/mf_crosslink101
     honesty flags wired (playable_claim stays false)
   - golden_skirmish_gate --frames 8 → PASS playable_claim=true
   - shell_smoke_gate → PASS playable_claim=false shell_host_playable_ok=true
     module_factory101=true thing_factory101=true partition_register101=true mf_crosslink101=true

**Wiring:**
- `game_logic/host_thing_factory_module_xfer_residual.rs` — Wave 101 deepen sections
- `game_logic/mod.rs` — pub use Wave 101 honesty packs
- `shell_smoke.rs` — module_factory101/thing_factory101/partition_register101/mf_crosslink101
- `shell_smoke_gate.rs` — require Wave 101 honesty flags; playable_claim stays false
- Co-present Wave 100 residual packs in shell gate wiring

**Still residual (fail-closed, not claimed):**
- Full live ThingFactory Object GPU / CreateModule instance graph residual
- Full live BehaviorModule createProc / exclusive module graph residual
- Full PartitionData attach / shroud ghost exclusive residual
- Full XferSave/XferLoad file I/O / deep CRC network residual
- Shell `playable_claim` remains false (no windowed W3D retail claim)
- Network residual replication (network deferred)


## Residual Host Playability — Wave 100: ThingFactory residual deepen / Module type tables / Xfer residual deepen peels (2026-07-14)

- Co-present Wave 99 residual (`host_production_buildable_command_residual`) in shell gate wiring

**Closed (host-testable residual peels; orthogonal ThingFactory / Module / Xfer residual):**
1. **ThingFactory residual deepen** (`host_thing_factory_module_xfer_residual`, beyond Wave 65/74 object packs + spawn bookkeeping):
   - `TEMPLATE_HASH_SIZE` **12288**; `m_nextTemplateID` ctor **1** (never zero).
   - `DefaultThingTemplate` residual name; `OBJECT_STATUS_MASK_NONE` **0**.
   - `DRAWABLE_STATUS` residual bits NONE/DRAWS_IN_MIRROR/SHADOWS/TINT_COLOR_LOCKED/
     NO_STATE_PARTICLES/NO_SAVE (**6** names).
   - `KINDOF_DRAWABLE_ONLY` residual index **32** (ALLOW_SURRENDER off KindOf table).
   - `newObject` residual pipeline **7** steps: VALIDATE…INIT_OBJECT.
   - `newDrawable` residual pipeline **2** steps.
   - Build-variation residual index clamp; template ID allocate residual + wrap reject.
   - Drawable-only Object reject residual; null template ERROR residual.
   - Honesty: `honesty_thing_factory_residual_deepen_pack_wave100`.
2. **Module residual type tables** (Module.h / ModuleFactory):
   - `ModuleType` BEHAVIOR **0** / DRAW **1** / CLIENT_UPDATE **2**; `NUM_MODULE_TYPES` **3**.
   - Drawable module range FIRST=DRAW LAST=CLIENT_UPDATE; `NUM_DRAWABLE_MODULE_TYPES` **2**.
   - `ModuleInterfaceType` residual **12** flags UPDATE…CLIENT_UPDATE (0x1…0x800).
   - `makeDecoratedNameKey` residual format `"{type}{name}"` (`0ActiveBody` / `1W3DModelDraw`).
   - Empty-name interface mask residual **0**.
   - Sample ModuleFactory residual table **9** (ActiveBody…BeaconClientUpdate).
   - Honesty: `honesty_module_type_table_residual_pack_wave100`.
3. **Xfer residual deepen** (Xfer.h/.cpp + XferCRC + GameState.h):
   - `XferMode` residual **4** names INVALID/SAVE/LOAD/CRC; `NUM_XFER_TYPES` **4**.
   - `XferStatus` residual **18** names (C++ table; no Rust-only InvalidData).
   - `XferOptions` NONE **0** / NO_POST_PROCESSING **0x1** / ALL **0xFFFFFFFF**.
   - `XferVersion` size **1** byte; ctor residual options=NONE mode=INVALID.
   - XferCRC ctor residual mode=CRC crc=**0**; `addCRC` host pure-arithmetic residual.
   - Object xfer CURRENT_VERSION **9**; Drawable **7**; module-bucket **1**.
   - `MAX_XFER_STRING_LENGTH` **255**.
   - `xferVersion` reject residual (version > current → INVALID_VERSION).
   - `SaveFileType` NORMAL/MISSION; `SnapshotType` SAVELOAD/DEEPCRC_LOGICONLY/DEEPCRC.
   - `SaveCode` SC_INVALID **−1** … SC_ERROR **7** residual.
   - Honesty: `honesty_xfer_residual_deepen_pack_wave100`.
4. **ThingFactory spawn cross-link** (Wave 74 ledger + Wave 100 pipeline):
   - newObject CREATE interface residual CREATE **0x8** ordinal **3**.
   - Honesty: `honesty_thing_factory_spawn_crosslink_wave100`.
5. **Combined pack**: `honesty_thing_factory_module_xfer_residual_pack_wave100`.
6. Tests / gates:
   - Unit: 5 wave100 honesty tests PASS
   - shell_smoke: thing_factory100/module_type100/xfer100/tf_crosslink100 honesty flags wired
     (playable_claim stays false)
   - golden_skirmish_gate --frames 8 → PASS playable_claim=true
   - shell_smoke_gate → PASS playable_claim=false shell_host_playable_ok=true
     thing_factory100=true module_type100=true xfer100=true tf_crosslink100=true
     (plus concurrent wave99 production/buildable/prereq/cmdbtn/controlbar)

**Wiring:**
- `game_logic/host_thing_factory_module_xfer_residual.rs` (new)
- `game_logic/mod.rs` — module + pub use honesty
- `shell_smoke.rs` — thing_factory100/module_type100/xfer100/tf_crosslink100 fields + detail tokens
- `shell_smoke_gate.rs` — require wave100 honesty flags; playable_claim stays false
- Co-present Wave 99 residual (`host_production_buildable_command_residual`) in shell gate wiring

**Still residual (fail-closed, not claimed):**
- Full ThingFactory Object / live CreateModule + PartitionManager register residual
- Full ModuleFactory addModule registry / live BehaviorModule create residual
- Full XferSave/XferLoad file I/O / deep CRC network residual
- Shell `playable_claim` remains false (no windowed W3D retail claim)
- Network residual replication (network deferred)


## Residual Host Playability — Wave 98: dock residual peels / contain residual deepen / exit residual peels / heal residual deepen (2026-07-14)

**Closed (host-testable residual peels; orthogonal dock/contain/exit/heal residual):**
1. **Dock residual peels** (`host_dock_contain_exit_heal_residual`, beyond Wave 52 repair / Wave 83 supply dock):
   - `DEFAULT_APPROACH_VECTOR_SIZE` **10**; `DYNAMIC_APPROACH_VECTOR_FLAG` **-1**.
   - DockUpdateModuleData defaults: NumberApproachPositions **0**, AllowsPassthrough **Yes**.
   - DockUpdate ctor residual: dockOpen **Yes**, dockerInside/crippled/positionsLoaded **No**, approachBones **-1**.
   - AI_DOCK state residual **8** names APPROACH **0** .. MOVE_TO_RALLY **7**.
   - WaitForClearance timeout residual **30×LOGICFRAMES** = **900**f.
   - RepairDock: default framesForFullHeal **1.0**; retail TimeForFullHeal **5000**ms→**150**f;
     NumberApproachPositions **5**; isRallyPointAfterDock **Yes**.
   - SupplyCenterDock: GrantTemporaryStealth default **0**; GLA **20000**ms→**600**f;
     America approach **9**; China/GLA boneless **-1** + AllowsPassthrough **No**.
   - RailedTransportDock: Tolerance default **50**; ferry Pull/Push **4500**ms→**135**f,
     Tolerance **400**, Approach **9**; UNLOAD_ALL **-1**.
   - healthToAddPerFrame residual: `(max−current)/framesForFullHeal`.
   - Honesty: `honesty_dock_residual_pack_wave98`.
2. **Contain residual deepen** (beyond Wave 87 open/garrison/transport):
   - OpenContain deepen: BurnedDeathToUnits **Yes**, PassengersInTurret **No**,
     WeaponBonusPassedToPassengers **No**, KickOutOnCapture **Yes**, ImmuneToClear **Yes**,
     isHealContain/Garrisonable/Bustable/Tunnel **No**; Transport isDisplayedOnControlBar **Yes**.
   - ObjectEnterExitType residual **3**: WANTS_TO_ENTER **0** / EXIT **1** / NEITHER **2**.
   - EvacDisposition residual **4**: INVALID **0** .. BURST_FROM_CENTER **3**.
   - HealContain: isHealContain **Yes**; default frames **0**; barracks TimeForFullHeal
     **2000**ms→**60**f / ContainMax **10** / allies **Yes** / enemies+neutral **No** /
     AllowInsideKindOf **INFANTRY**.
   - Sliver residual: `max_health / frames_for_full_heal`; done when containedFrames ≥ frames.
   - Honesty: `honesty_contain_residual_deepen_pack_wave98`.
3. **Exit residual peels**:
   - ExitDoorType residual: DOOR_1..4 **0..3**, COUNT_MAX **4**, NONE_AVAILABLE **-1**,
     NONE_NEEDED **-2**.
   - OpenContain reserveDoor default **DOOR_1**; NumberOfExitPaths **1**; DoorOpenTime **1**f;
     isExitBusy default **No**.
   - QueueProductionExitUpdate defaults: ExitDelay **0**, AllowAirborne **No**, InitialBurst **0**.
   - ChinaBarracks ExitDelay **300**ms→**9**f; transport ExitDelay sample **500**ms→**15**f.
   - NUM_MODELCONDITION_DOOR_STATES **4**; transport exit countdown tick residual.
   - Honesty: `honesty_exit_residual_pack_wave98`.
4. **Heal residual deepen** (beyond Wave 71 ambulance + Wave 81 AutoFindHealing retail):
   - AutoHealBehavior defaults: StartsActive **No**, SingleBurst **No**, HealingAmount **0**,
     HealingDelay **UINT_MAX**, StartHealingDelay **0**, Radius **0**, SkipSelf **No**,
     AffectsWholePlayer **No**.
   - AutoFindHealing ctor defaults: ScanFrames **0**/Range **0**, NeverHeal **0.95**, AlwaysHeal **0.25**.
   - Retail infantry AutoFindHealing re-anchor: Scan **1000**ms→**30**f / Range **300** /
     Never **0.85** / Always **0.25** (ctor NeverHeal stricter than retail).
   - ParkingPlaceBehavior: default HealAmount **0**; Airfield **10**/sec Rows/Cols **2**/HasRunways
     **Yes**/ApproachHeight **50**; Helix-style sample **20**/sec.
   - DAMAGE_HEALING / DEATH_NONE residual tokens; parking heal per-frame residual.
   - Honesty: `honesty_heal_residual_deepen_pack_wave98`.
5. **Combined pack**: `honesty_dock_contain_exit_heal_residual_pack_wave98`.
6. Tests / gates:
   - Unit: 5 wave98 honesty tests PASS
   - shell_smoke: dock98/contain98/exit98/heal98 honesty flags wired
     (playable_claim stays false)
   - golden_skirmish_gate --frames 8 → PASS playable_claim=true
   - shell_smoke_gate → PASS playable_claim=false shell_host_playable_ok=true
     dock98=true contain98=true exit98=true heal98=true
     (plus concurrent wave99 production/buildable/prereq/cmdbtn/controlbar)

**Wiring:**
- `game_logic/host_dock_contain_exit_heal_residual.rs` (new)
- `game_logic/mod.rs` — module + pub use honesty
- `shell_smoke.rs` — dock98/contain98/exit98/heal98 fields + detail tokens
- `shell_smoke_gate.rs` — require wave98 honesty flags; playable_claim stays false
- Co-present Wave 99 residual (`host_production_buildable_command_residual`) in shell gate wiring

**Still residual (fail-closed, not claimed):**
- Full DockUpdate bone load / approach path AI residual
- Full OpenContain exit-door bone matrix / fire-point garrison residual
- Full ExitInterface production door anim residual
- Full AutoHealBehavior multi-healer exclusive / particle pulse residual
- Full ParkingPlaceBehavior runway taxi / hangar park residual
- Shell `playable_claim` remains false (no windowed W3D retail claim)
- Network residual replication (network deferred)


## Residual Host Playability — Wave 99: production residual deepen / buildable / prerequisite / command-button deepen / control-bar deepen peels (2026-07-14)

- Co-present Wave 98 residual (`host_dock_contain_exit_heal_residual`) in shell gate wiring

**Closed (host-testable residual peels; orthogonal production/buildable/prereq/command/control residual):**
1. **Production residual deepen** (`host_production_buildable_command_residual`, beyond Wave 83 queue/energy/refund):
   - `ProductionType` residual INVALID **0** / UNIT **1** / UPGRADE **2**.
   - Module defaults: MaxQueueEntries **9**, NumDoorAnimations **0**, door/complete durations **0**.
   - `ExitDoorType` DOOR_1..4 / COUNT_MAX **4** / NONE_AVAILABLE **−1** / NONE_NEEDED **−2**.
   - ProductionEntry ctor residual: ID **1**, percent **0**, quantity **0**.
   - DisabledTypesToProcess default HELD bit **0x08** (DISABLED_HELD ordinal **3**).
   - INI field residual table **8** (MaxQueueEntries…DisabledTypesToProcess).
   - QuantityModifier sample ChinaInfantryRedguard **×2**; default count **1**.
   - `BuildCompletionType` INVALID / APPEARS_AT_RALLY_POINT / PLACED_BY_PLAYER residual.
   - CONSTRUCTION_COMPLETE percent **−1**; calcTimeToBuild energy residual (5s→**150**/full, **300**/zero).
   - Honesty: `honesty_production_residual_deepen_pack_wave99`.
2. **Buildable residual peels** (BuildableStatus / CanMakeType / LegalBuildCode):
   - BuildableStatus Yes / Ignore_Prerequisites / No / Only_By_AI residual (**4**).
   - CanMakeType OK..MAXED_OUT_FOR_PLAYER residual (**7**).
   - LegalBuildCode LBC_OK..GENERIC_FAILURE residual (**8**).
   - LocalLegalToBuildOptions bits terrain **0x01**..fail-stealthed **0x80**.
   - TOTAL_FRAMES_TO_SELL_OBJECT **90**; SellPercentage **50%**; sell refund residual.
   - Human allow residual: YES / IGNORE_PREREQ only.
   - Honesty: `honesty_buildable_residual_pack_wave99`.
3. **Prerequisite residual peels** (ProductionPrerequisite):
   - MAX_PREREQ **32**; UNIT_OR_WITH_PREV **0x01**.
   - Prerequisites INI fields Object / Science residual.
   - Sample rows: AmericaWarFactory→SupplyCenter, Barracks→CC, ChinaWarFactory,
     GLABarracks, StrategyCenter OR WarFactory|Airfield residual.
   - Host-testable AND/OR satisfaction residual + science gate.
   - Honesty: `honesty_prerequisite_residual_pack_wave99`.
4. **CommandButton residual deepen** (`host_production_buildable_command_residual`, beyond Wave 80 SW labels):
   - GUICommandType residual **35** names NONE..SELECT_ALL_UNITS_OF_TYPE (no ALLOW_SURRENDER).
   - CommandOption residual **24** bit-names; NEED_TARGET mask **0x227**; NEED_OBJECT **0x7**.
   - CommandButtonMappedBorderType NONE/BUILD/UPGRADE/ACTION/SYSTEM residual.
   - CommandButton INI field residual table **15** (Command…UnitSpecificSound).
   - Honesty: `honesty_command_button_residual_deepen_pack_wave99`.
5. **ControlBar residual deepen** (beyond Wave 76 window/font pack):
   - MAX_COMMANDS_PER_SET **18**; visible ButtonCommand **14**; MAX_BUILD_QUEUE_BUTTONS **9**.
   - MAX_STRUCTURE_INVENTORY_BUTTONS **10**; MAX_SPECIAL_POWER_SHORTCUTS **11**.
   - Science rank windows **4** / **15** / **4**; right HUD upgrade cameos **5**.
   - ControlBarContext residual **9** names NONE..OCL_TIMER.
   - ButtonCommand01..14 + ButtonQueue01..09 residual name tables.
   - Window-count cross-link **98** (Wave 76).
   - Honesty: `honesty_control_bar_residual_deepen_pack_wave99`.
6. Tests / gates:
   - Unit honesty tests for all five wave99 residual packs.
   - shell_smoke: production99/buildable99/prereq99/cmdbtn99/controlbar99 honesty flags wired
     (playable_claim stays false)
   - golden_skirmish_gate --frames 8 → PASS playable_claim=true
   - shell_smoke_gate → PASS playable_claim=false shell_host_playable_ok=true
     production99=true buildable99=true prereq99=true cmdbtn99=true controlbar99=true
     (plus concurrent wave98 dock/contain/exit/heal)

**Still residual (fail-closed, not claimed):**
- Full ProductionUpdate door-anim / parking-place live queue residual
- Full BuildAssistant isLocationLegalToBuild terrain/shroud graph residual
- Full ProductionPrerequisite live player-owned unit scan residual
- Full CommandButton INI parse / science-swap cameo matrix residual
- Full ControlBar DrawCallback / windowed W3D retail UI residual
- Shell `playable_claim` remains false (no windowed W3D retail claim)
- Network residual replication (network deferred)

## Residual Host Playability — Wave 97: radar residual deepen / spotter / stealth deepen / detector deepen / vision residual peels (2026-07-13)

**Closed (host-testable residual peels; orthogonal radar/spotter/stealth/detector/vision residual):**
1. **Radar residual deepen** (`host_radar_stealth_vision_residual`, beyond Wave 63 provider / Wave 72 scan):
   - `RADAR_CELL_WIDTH/HEIGHT` **128**; `MAX_RADAR_EVENTS` **64**.
   - `RadarEventType` residual **11** names INVALID **0** .. FAKE **10** (`RADAR_EVENT_NUM_EVENTS` **11**).
   - `RadarPriorityType` residual **5** names INVALID..LOCAL_UNIT_ONLY; visible STRUCTURE/UNIT/LOCAL_UNIT_ONLY.
   - createEvent default secondsToLive **4.0**; fade **0.5**s before die; terrain refresh delay **90**f.
   - Radar event color table residual (construction/attack/stealth green/…); player darkScale **0.75**.
   - CommandCenter RadarExtendTime **4000**ms → **120**f; RadarUpgrade DisableProof default **No**.
   - Honesty: `honesty_radar_residual_deepen_pack_wave97`.
2. **Spotter residual peels** (StealthDetectorUpdate discovery feedback path):
   - MESSAGE:StealthDiscovered / MESSAGE:StealthNeutralized residual keys.
   - MiscAudio StealthDiscoveredSound / StealthNeutralizedSound residual keys.
   - `tryEvent` residual: closeEnough **250** (sq **62500**), framesBetween **300** (10s).
   - markAsDetected residual: numFrames**0** → now+stealthDelay; else max(expires, now+numFrames).
   - Detector primary mark `updateRate+1`; garrison rider `updateRate+2`.
   - Second material pass opacity **1.0** when spotted (non-mine).
   - RADAR_EVENT_STEALTH_DISCOVERED **8** / STEALTH_NEUTRALIZED **9** cross-link.
   - Honesty: `honesty_spotter_residual_pack_wave97`.
3. **Stealth residual deepen** (StealthUpdate ctor + level bits + samples):
   - StealthLevel bits ATTACKING..RIDERS_ATTACKING residual + TheStealthLevelNames **9**.
   - Ctor residual: StealthDelay **UINT_MAX**, FriendlyOpacityMin **0.5**/Max **1.0**, PulseFrames **30**,
     Innate **Yes**, pulsePhaseRate **0.2**, INVALID_OPACITY **−1**.
   - GameData StealthFriendlyOpacity **50%**; StealthLook ordinals NONE..INVISIBLE residual.
   - Sample rows: Pathfinder delay **0**/MOVING opacity **30–80%**; Burton **2000**/FIRING_PRIMARY;
     Rebel **2500**/ATTACKING|USING_ABILITY; Lotus **2500**/USING_ABILITY; CamoNetting **2500**.
   - Honesty: `honesty_stealth_residual_deepen_pack_wave97`.
4. **Detector residual deepen** (StealthDetectorUpdate defaults + samples):
   - Ctor residual: updateRate **1**, DetectionRange **0** (→ VisionRange), InitiallyDisabled **No**,
     CanDetectWhileGarrisoned/Transported **No**.
   - Common DetectionRate **500**ms → **15**f; slow **900**ms → **27**f.
   - Samples: Pathfinder 500/vision**200**; Hijacker 500/range**200**; ListeningOutpost 900/vision**175**;
     SentryDrone 900/range**225**; StrategyCenter 500/range**150**; RadarVanPing 500/vision**150**.
   - Honesty: `honesty_detector_residual_deepen_pack_wave97`.
5. **Vision residual peels** (VisionRange / AI vision factors / DSCRU):
   - ThingTemplate defaults: VisionRange **0**, ShroudClearingRange **−1** (→ VisionRange),
     ShroudRevealToAllRange **−1**.
   - AI_VISIONFACTOR_OWNERTYPE **0x01** / MOOD **0x02** / GUARDINNER **0x04**.
   - AIData residual: GuardInner/Outer AI **1.1/1.333**, Human **1.8/2.2**; Alert **1.1**; Aggressive **1.5**.
   - getAdjustedVisionRange residual: contained→largest weapon range; AI sleep mood → **0**.
   - DSCRU GRID_FX_DECAL_COUNT **30**; state names NOT_STARTED..SLEEPING residual.
   - Honesty: `honesty_vision_residual_pack_wave97`.
6. **Combined pack**: `honesty_radar_stealth_vision_residual_pack_wave97`.

**Wiring:**
- `game_logic/host_radar_stealth_vision_residual.rs` (new)
- `game_logic/mod.rs` — module + pub use honesty
- `shell_smoke.rs` — radar97/spotter97/stealth97/detector97/vision97 fields + detail tokens
- `shell_smoke_gate.rs` — require wave97 honesty flags; playable_claim stays false
- Wave 96 residual (partition/collision/physics/projectile) co-present in shell gate wiring

**Gates:**
- Unit: 6 wave97 honesty tests PASS (+ wave96 residual tests)
- golden_skirmish_gate --frames 8 → PASS playable_claim=true
- shell_smoke_gate → PASS playable_claim=false shell_host_playable_ok=true
  radar97=true spotter97=true stealth97=true detector97=true vision97=true

*(Verified 2026-07-14: unit 6/6, golden playable_claim=true, shell playable_claim=false.)*

**Not claimed:**
- Full W3DRadar GPU atlas / event marker draw residual
- Full StealthUpdate exclusive allowedToStealth matrix / disguise path residual
- Full StealthDetectorUpdate partition iterate / IR particle GPU residual
- Full PartitionManager looker refresh / FOW multi-layer streaming residual
- shell playable_claim / network (deferred)

**Honesty rules preserved:**
- Shell playable_claim remains **false**
- Golden playable_claim remains **true**
- Network residual deferred

## Residual Host Playability — Wave 96: partition / collision / physics / projectile residual peels (2026-07-13)

**Closed (host-testable residual peels; partition + collision + physics + projectile residual):**
1. **Partition residual peels** (`host_partition_collision_physics_residual`, PartitionManager.h/.cpp):
   - `HUGE_DIST` **1000000** / `HUGE_DIST_SQR` / `RANDOM_START_ANGLE` **−99999.9**.
   - `DistanceCalculationType` residual **4** names FROM_CENTER_2D **0** .. FROM_BOUNDINGSPHERE_3D **3**.
   - `ValueOrThreat` residual VOT_CashValue **1** / VOT_ThreatValue **2** / VOT_NumItems **3**.
   - `FindPositionFlags` residual bits FPF_NONE **0** .. FPF_CLEAR_CELLS_ONLY **0x100** (**9** named bits).
   - PartitionData `DirtyStatus` residual NOT_DIRTY **0** / NEED_COLLISION_CHECK **1** /
     NEED_CELL_UPDATE_AND_COLLISION_CHECK **2**.
   - `PartitionFilterRelationship::RelationshipAllowTypes` residual ALLOW_ENEMIES **1** /
     ALLOW_NEUTRAL **2** / ALLOW_ALLIES **4** (bits from Relationship ordinals).
   - Concrete PartitionFilter residual name table (**33**): IsFlying .. ValidCommandButtonTarget
     (anchors SameMapStatus / Player / UnmannedObject).
   - `PartitionContactList_SOCKET_COUNT` residual **5381**; PartitionCellSize residual **40**
     (cross-link Wave 86 GameData).
   - Honesty: `honesty_partition_residual_pack_wave96`.
2. **Collision residual peels** (Geometry.cpp + PartitionManager `theCollideTestProcs`):
   - GeometryType residual order SPHERE **0** / CYLINDER **1** / BOX **2** (GEOMETRY_FIRST = SPHERE;
     collidesWith matrix **depends** on this order).
   - `theCollideTestProcs` residual **9** entries (3×3 Sphere/Cylinder/Box row-major).
   - Bounding residual: circle (Sphere/Cylinder→major; Box→√(major²+minor²));
     sphere (Sphere→major; Cylinder→max(h/2,major); Box→√(major²+minor²+(h/2)²)).
   - Height residual: maxAbove Sphere→major / Box·Cylinder→height; maxBelow Sphere→major /
     Box·Cylinder→**0**; zDeltaToCenter Sphere→**0** / else height/2.
   - Footprint area residual: Sphere/Cylinder→π r²; Box→**4**·major·minor.
   - CollideModule residual note: `other == NULL` means ground collision.
   - Honesty: `honesty_collision_residual_pack_wave96`.
3. **Physics residual peels** (PhysicsUpdate.cpp / PhysicsUpdate.h):
   - Defaults: Mass **1.0**, ShockYaw **0.05** / Pitch·Roll **0.025**, Forward·Lateral friction **0.15**,
     ZFriction **0.8**, AeroFriction **0.0**.
   - Friction clamps MIN_AERO **0** / MIN_NON_AERO **0.01** / MAX **0.99**; STUN_RELIEF_EPSILON **0.5**.
   - `MOTIVE_FRAMES` residual **10** (`LOGICFRAMES_PER_SECOND/3`); INVALID_VEL_MAG **−1**.
   - Ctor residual: allowBouncing **false**, allowCollideForce **true**, killWhenRestingOnGround **false**,
     pitchRollYawFactor **2.0**, fallHeightDamageFactor **1.0**, MinFallHeight **40** → heightToSpeed.
   - `heightToSpeed` residual `√(|2·g·h|)` with Gravity **−64** → √5120 ≈ **71.55**.
   - `parseFrictionPerSec` residual fric/frame = fric/sec · (1/30) (0.15→**0.005**).
   - `PhysicsTurningType` residual TURN_NEGATIVE **−1** / NONE **0** / POSITIVE **1**.
   - `PhysicsFlagsType` residual **12** bits STICK_TO_GROUND **0x0001** .. IS_STUNNED **0x0800**.
   - Crash weapon residual names VehicleCrashesIntoBuildingWeapon /
     VehicleCrashesIntoNonBuildingWeapon.
   - Honesty: `honesty_physics_residual_pack_wave96`.
4. **Projectile residual deepen** (DumbProjectileBehavior.h/.cpp):
   - `DEFAULT_MAX_LIFESPAN` residual **10**·LOGICFRAMES_PER_SECOND = **300** frames.
   - ModuleData defaults: OrientToFlightPath **true**, TumbleRandomly **false**,
     DetonateCallsKill **false**, First/Second Height·PercentIndent **0**, GarrisonHitKillCount **0**.
   - Ballistic residual: SHALLOW_ANGLE **0.5°**, MIN_ANGLE_DIFF **1/16°**, CLOSE_ENOUGH_RANGE **5**.
   - **13** INI field residual names (MaxLifespan .. FlightPathAdjustDistPerSecond).
   - Lifespan residual: launchFrame + maxLifespan.
   - Honesty: `honesty_projectile_residual_deepen_pack_wave96`.
5. **Combined pack**: `honesty_partition_collision_physics_residual_pack_wave96`.

**Wiring:**
- `game_logic/host_partition_collision_physics_residual.rs` (new)
- `game_logic/mod.rs` — module + pub use honesty
- `shell_smoke.rs` — partition96/collision96/physics96/projectile96 fields
- `shell_smoke_gate.rs` — require wave96 honesty flags; playable_claim stays **false**
- Wave 97 residual (`host_radar_stealth_vision_residual`) co-present in shell gate wiring

**Gates:**
- Unit: 5+ wave96 honesty tests PASS
- golden_skirmish_gate --frames 8 → PASS playable_claim=true
- shell_smoke_gate → PASS playable_claim=false shell_host_playable_ok=true
  partition96=true collision96=true physics96=true projectile96=true

**Not claimed / fail-closed:**
- Full PartitionManager filter stack / live COI registration residual
- Full CollideModule partition pair dispatch / live onCollide graph
- Full PhysicsBehavior motive force / bounce exclusive residual
- Full DumbProjectileBehavior live Bezier flight / ThingFactory Object residual
- shell playable_claim / network (deferred)

**Honesty rules preserved:**
- Shell playable_claim remains **false**
- Golden playable_claim remains **true**
- Network residual deferred

## Residual Host Playability — Wave 94: AI state / special ability / upgrade names / CommandSet superweapon residual peels (2026-07-13)

**Closed (host-testable residual peels; command/AI/ability/upgrade residual):**
1. **AI state residual tables** (`host_ai_ability_upgrade_residual`, AIStateMachine.h):
   - `AIStateType` residual **44** names AI_IDLE **0** .. AI_GUARD_RETALIATE **43** (`NUM_AI_STATES` **44**).
   - Host `AIState` → C++ `AI_*` residual bridge (Moving→MOVE_TO, Attacking→ATTACK_OBJECT,
     AttackMoving→ATTACK_MOVE_TO, Guarding*→GUARD, SpecialAbility/Capturing→BUSY, …).
   - Honesty: `honesty_ai_state_residual_table_wave94`.
2. **Special ability residual deepen** (SpecialPower.ini + Object SpecialAbilityUpdate):
   - **27** SpecialAbility* template residual rows (core 20 + Demo/Nuke/Lazr variants).
   - ReloadTime residual anchors: TankHunter/BoobyTrap **7500**, infantry capture **15000**,
     Hacker **500**, Microwave **4000**, Lotus steal **2000**, Helix napalm **10000**,
     Demo rebel timed **30000**, BattleBus rollout **7500**.
   - ms→frames residual `*0.03` (7500→**225**, 15000→**450**).
   - SpecialAbilityUpdate samples: Lotus capture/disable/steal, TNT MaxSpecialObjects **8**,
     Missile Defender range **200**/prep **1000**, Ranger capture prep **20000**,
     Hacker range **150**/unpack **7300**/prep **3000**.
   - Honesty: `honesty_special_ability_residual_deepen_wave94`.
3. **Upgrade residual full name table** (Upgrade.ini):
   - **81** unique Upgrade.ini internal names (duplicate SupW PointDefenseDrone collapsed).
   - Anchors: SupplyLines / FlashBang / TOW / Capture / CompositeArmor / NuclearTanks /
     Camouflage / CamoNetting / AnthraxGamma / SuicideBomb / Overlord addons / HelixNapalm /
     RocketBuggyToxinUpgrade / general prefixes Chem_/Demo_/Nuke_/AirF_/SupW_/Tank_.
   - Honesty: `honesty_upgrade_name_table_residual_wave94`.
4. **CommandSet residual for superweapon buildings** (CommandSet.ini + Wave 80 kindof cross-link):
   - ParticleUplink: slot1 FireParticleUplinkCannon + Sell.
   - ScudStorm: slot1 ScudStorm + Sell.
   - NuclearMissile: NeutronMissile + Uranium/NuclearTanks/NeutronShells/Mines + Sell.
   - Command centers: USA Daisy/A10/Spectre/Paradrop; China Arty/EMP/Cluster/Frenzy;
     GLA Anthrax/Sneak/Ambush/GPS; Strategy Center battle plans + MOAB/CIA.
   - Honesty: `honesty_command_set_superweapon_residual_wave94`.

**Wiring:**
- `game_logic/host_ai_ability_upgrade_residual.rs` (new)
- `game_logic/mod.rs` — module + pub use honesty
- `shell_smoke.rs` — ai_state94/special_ability94/upgrade_names94/command_set94 fields + detail tokens
- `shell_smoke_gate.rs` — require wave94 honesty flags; playable_claim stays false
- Combined pack: `honesty_ai_ability_upgrade_residual_pack_wave94`
- Co-ships concurrent Wave 95 script/map/team/player residual module + shell wiring for green gates

**Gates:**
- Unit: 5 wave94 honesty tests PASS (+ wave95 residual module tests PASS)
- golden_skirmish_gate --frames 8 → PASS playable_claim=true
- shell_smoke_gate → PASS playable_claim=false shell_host_playable_ok=true
  ai_state94=true special_ability94=true upgrade_names94=true command_set94=true
  (plus concurrent wave95 script/map/waypoint/team/player)

**Not claimed:**
- Full AIStateMachine exclusive enter/exit / path residual
- Full SpecialAbilityUpdate flee-after / MaxSpecialObjects exclusive matrix
- Full UpgradeCenter NameKey purchase / multipleyer upgrade replication
- Full ControlBar CommandSet slot UI matrix / WND cameo residual
- shell playable_claim / network (deferred)

**Honesty rules preserved:**
- Shell playable_claim remains **false**
- Golden playable_claim remains **true**
- Network residual deferred

## Residual Host Playability — Wave 95: script action/condition name tables / map object / waypoint / team / player residual deepen (2026-07-13)

**Closed (host-testable residual peels; orthogonal script/map/team/player residual):**
1. **Script action residual name table** (`host_script_map_team_player_residual`, Scripts.h + ScriptEngine.cpp):
   - Full ordered `ScriptActionType` internal-name residual table (**344** entries; index = discriminant).
   - Caps residual: `MAX_PARMS` **12**, `MAX_COUNTERS`/`MAX_FLAGS`/`MAX_ATTACK_PRIORITIES` **256**.
   - Script tokens residual: `THIS_TEAM` / `TEAM_THE_PLAYER` / `ThePlayer` / skirmish Center/Flank/Backdoor + perimeter names.
   - Anchors: DEBUG_MESSAGE_BOX **0**, VICTORY **3**, NO_OP **5**, CALL_SUBROUTINE **10**, CREATE_OBJECT **23**,
     TEAM_FOLLOW_WAYPOINTS **36**, DISPLAY_TEXT **76**, MAP_REVEAL_ALL **102**, SHOW_WEATHER **342**,
     AI_PLAYER_BUILD_TYPE_NEAREST_TEAM **343**.
   - Honesty: `honesty_script_action_name_table_residual_wave95`.
2. **Script condition residual name table**:
   - Full ordered `ConditionType` internal-name residual table (**109** entries).
   - Anchors: CONDITION_FALSE **0**, COUNTER **1**, FLAG **2**, CONDITION_TRUE **3**, TIMER_EXPIRED **4**,
     NAMED_HAS_FREE_CONTAINER_SLOTS **108**.
   - Honesty: `honesty_script_condition_name_table_residual_wave95`.
3. **MapObject residual peels** (MapObject.h + WellKnownKeys object*):
   - `MAP_XY_FACTOR` **10.0**; `MAP_HEIGHT_SCALE` **0.625** (FACTOR/16).
   - FLAG_* residual bits DRAWS_IN_MIRROR..DONT_RENDER; ROAD_FLAGS/BRIDGE_FLAGS composites.
   - Runtime MO_* residual: SELECTED/LIGHT/WAYPOINT/SCORCH.
   - **32** object* Dict key residual rows (objectName..objectSoundAmbientPriority).
   - World/map keys residual: mapName / InitialCameraPosition / originalOwner / uniqueID.
   - Honesty: `honesty_map_object_residual_pack_wave95`.
4. **Waypoint residual peels** (TerrainLogic.h + waypoint* WellKnownKeys):
   - `INVALID_WAYPOINT_ID` **0x7FFFFFFF**; `Waypoint::MAX_LINKS` **8**.
   - **6** waypoint* Dict keys: waypointName/ID/PathLabel1–3/PathBiDirectional.
   - Cross-link MO_WAYPOINT **0x04**.
   - Honesty: `honesty_waypoint_residual_pack_wave95`.
5. **Team residual peels** (Team.h + team* WellKnownKeys):
   - `TEAM_ID_INVALID` / `TEAM_PROTOTYPE_ID_INVALID` **0**.
   - `MAX_UNIT_TYPES` **7**; `MAX_GENERIC_SCRIPTS` **16**.
   - TBehavior residual NORMAL **0** / IGNORE_DISTRACTIONS **1** / DEAL_AGGRESSIVELY **2**.
   - **54** team* Dict key residual rows (teamName..teamGenericScriptHook).
   - Default team name residual: `"team" + playerName` (e.g. teamAmerica / teamThePlayer).
   - Honesty: `honesty_team_residual_pack_wave95`.
6. **Player residual peels deepen** (beyond Wave 85 faction/template/cash):
   - `MAX_PLAYER_COUNT` **16**; neutral slot index **0**; `NEUTRAL_PLAYER_COLOR` **0xFFFFFFFF**.
   - Ctor residual: skillPointsModifier **1.0**, rankLevel **0**, skillPoints **0**.
   - Relationship residual: self **ALLIES=2**, default unknown **NEUTRAL=1**, ENEMIES **0**.
   - Player mask residual: `1 << playerIndex`.
   - **11** player* Dict keys (playerName..playerIsPreorder).
   - Honesty: `honesty_player_residual_deepen_pack_wave95`.
7. **Combined pack**: `honesty_script_map_team_player_residual_pack_wave95`.

**Wiring:**
- `game_logic/host_script_map_team_player_residual.rs` (new)
- `game_logic/mod.rs` — module + pub use honesty
- `shell_smoke.rs` — script_action95 / script_cond95 / map_object95 / waypoint95 / team95 / player95
- `shell_smoke_gate.rs` — require wave95 honesty flags; playable_claim stays false
- Wave 94 residual (`host_ai_ability_upgrade_residual`) co-present in shell gate wiring

**Gates:**
- Unit: 7 wave95 honesty tests PASS (+ wave94 residual tests)
- golden_skirmish_gate --frames 8 → PASS playable_claim=true
- shell_smoke_gate → PASS playable_claim=false shell_host_playable_ok=true
  script_action95=true script_cond95=true map_object95=true waypoint95=true team95=true player95=true

**Not claimed:**
- Full ScriptAction executor / Condition evaluator residual
- Full MapObject WB validate / WorldDict xfer residual
- Full TerrainLogic waypoint path-label walk residual
- Full TeamFactory production / AI recruit residual
- Full Player science purchase / energy matrix residual
- shell playable_claim / network (deferred)

**Honesty rules preserved:**
- Shell playable_claim remains **false**
- Golden playable_claim remains **true**
- Network residual deferred

## Residual Host Playability — Wave 93: particle emit-rate / drawable opacity+shroud / shadow deepen / terrain texture / road residual peels (2026-07-13)

**Closed (host-testable residual peels; orthogonal render/terrain residual):**
1. **Particle system residual deepen** (`host_render_terrain_residual`, ParticleSys.h/.cpp + ParticleSystem.ini + GameData.ini):
   - ParticlePriorityType residual: INVALID **0** .. ALWAYS_RENDER **13**; NUM_PARTICLE_PRIORITIES **14**.
   - Priority names NONE/WEAPON_EXPLOSION..ALWAYS_RENDER residual.
   - Ctor residual: priority LOWEST, SystemLifetime **0** (forever), countCoeff/delayCoeff **1.0**, IsOneShot **No**.
   - Wind residual angle change **0.15** / min **0.15** / max **0.45**; DEFAULT_VOLUME_PARTICLE_DEPTH **0**.
   - MaxParticleCount retail **2500** (ctor **0** before INI).
   - Sample TsingMaTrailSmoke: BurstDelay **40**, BurstCount **0..2**, InitialDelay **20**, forever system.
   - Emit-rate formula residual: `REAL_TO_INT(burstCount)*countCoeff`, delay frames `*delayCoeff`.
   - Honesty: `honesty_particle_system_emit_rate_residual_deepen_pack_wave93`.
2. **Drawable residual deepen** (opacity + shroud; beyond Wave 79 StealthLook ordinals):
   - StealthLook residual 6 ordinals NONE..INVISIBLE.
   - explicitOpacity default **1.0**; StealthFriendlyOpacity **0.5** (ctor + GameData 50%).
   - `getEffectiveOpacity` = explicit * effectiveStealth residual.
   - `setEffectiveOpacity` pulse residual (floor + margin*pf); sentinel **−1** leaves floor.
   - DrawableStatus bits NONE/MIRROR/SHADOWS/TINT_LOCKED/NO_STATE_PARTICLES/NO_SAVE residual.
   - shroudClearFrame default **0**; heat-vision second-pass opacity on/off residual.
   - Honesty: `honesty_drawable_opacity_shroud_residual_deepen_pack_wave93`.
3. **Shadow residual deepen** (beyond Wave 84 ShadowType enum table):
   - MAX_SHADOW_LIGHTS **1**; m_shadowColor ARGB **0x7fa0a0a0**.
   - addShadow default type SHADOW_VOLUME **0x02**.
   - ShadowType bits DECAL..ADDITIVE_DECAL residual; Drawable STATUS_SHADOWS **0x02** cross-link.
   - Honesty: `honesty_shadow_residual_deepen_pack_wave93`.
4. **Terrain texture residual peels** (TerrainTex/TileData/TerrainTypes + Terrain.ini):
   - TILE_OFFSET **8**; TILE_PIXEL_EXTENT **64**; TEXTURE_WIDTH **2048**.
   - Mip extents **32/16/8**; cloud slide x **−0.02**/s, y **1.5×x**.
   - terrainTypeNames residual **38** rows (NONE..URBAN; INI string labels).
   - FieldParse Texture/BlendEdges/Class/RestrictConstruction residual.
   - Sample AsphaltType1 / GrassRockTransitionType1 residual rows.
   - Honesty: `honesty_terrain_texture_residual_pack_wave93`.
5. **Road residual peels** (TerrainRoads/W3DRoadBuffer + Roads.ini + GameData.ini):
   - DEFAULT_ROAD_SCALE **8**; MAX_SEG_VERTEX **500**; MAX_SEG_INDEX **2000**.
   - NUM_CORNERS **4**; NUM_JOINS **8** (SEGMENT..ALPHA_JOIN).
   - FieldParse Texture/RoadWidth/RoadWidthInTexture; ctor widths **0**; id counter starts **1**.
   - MaxRoad Segments **4000** / Vertex **3000** / Index **5000** / Types **100** (ctor 0).
   - Sample TwoLane **35**/0.9, FourLane **60**/0.9, Cobblestone **30**, GrassStrip **8**.
   - Honesty: `honesty_road_residual_pack_wave93`.

**Wiring:**
- `game_logic/host_render_terrain_residual.rs` (new; co-shipped in Wave 92 commit for green gate)
- `game_logic/mod.rs` — module + pub use honesty
- `shell_smoke.rs` — particle93/drawable93/shadow93/terrain_tex93/road93 fields + detail tokens
- `shell_smoke_gate.rs` — require wave93 honesty flags; playable_claim stays false
- Combined pack: `honesty_render_terrain_residual_pack_wave93`

**Gates:**
- Unit: 6 wave93 honesty tests PASS
- golden_skirmish_gate --frames 8 → PASS playable_claim=true
- shell_smoke_gate → PASS playable_claim=false shell_host_playable_ok=true
  particle93=true drawable93=true shadow93=true terrain_tex93=true road93=true
  (plus concurrent wave92 weapon/armor/body/loco/science)

**Not claimed:**
- Full ParticleSystemManager LOD cull / GPU particle draw residual
- Full Drawable W3D material pass / heat-vision GPU residual
- Full volumetric shadow stencil / projected shadow GPU residual
- Full TerrainTextureClass atlas update / CloudMap GPU residual
- Full W3DRoadBuffer mesh bake / DX8 VB residual
- shell playable_claim / network (deferred)

**Honesty rules preserved:**
- Shell playable_claim remains **false**
- Golden playable_claim remains **true**
- Network residual deferred

## Residual Host Playability — Wave 92: weapon/armor/body/locomotor/science residual peels (2026-07-13)

**Closed (host-testable residual peels; combat/sim residual deepen):**
1. **Weapon template residual deepen** (`weapon_bootstrap`, beyond Wave 77 core seed):
   - **16** common ZH deepen residual names: Marauder / GattlingTank / Comanche /
     Dragon / Scorpion / BuggyRocket / Pathfinder sniper / MissileDefender /
     Paladin / Helix minigun / Technical MG / QuadCannon / ToxinTruck / NapalmMiG /
     StealthJet / TankHunter.
   - Key damage/range residual scalars verified (e.g. Marauder **60**/r**170**,
     Pathfinder **100**/r**300**, Dragon **10**/r**75**, Tomahawk **150**/r**350**).
   - Honesty: `honesty_weapon_store_deepen_residual_wave92`.
2. **Armor residual expand** (`host_armor_residual`, beyond Wave 81 ProjectileArmor):
   - HumanArmor / TankArmor / StructureArmor / AirplaneArmor / TruckArmor residual.
   - Key Armor.ini coefficients: Human SNIPER **200%** / AP **10%**; Tank SMALL_ARMS
     **25%** / GATTLING **10%** / SNIPER **0%**; Structure AURORA_BOMB **250%** /
     PARTICLE_BEAM **200%**; Airplane JET_MISSILES **25%** / SMALL_ARMS **120%**;
     Truck SMALL_ARMS **50%** / SNIPER **0%**.
   - Honesty: `honesty_armor_residual_expand_wave92`.
3. **Body residual MaxHealth table** (`host_combat_sim_residual`):
   - **46** common unit/structure MaxHealth residual rows (infantry/vehicles/air/structures).
   - Anchors: Ranger **180**, Crusader **480**, Paladin **500**, Scorpion **370**,
     Overlord **1100**, Dragon **280**, Comanche **220**, CommandCenter **5000**.
   - Cross-checked against existing host residual constants where present.
   - Honesty: `honesty_body_max_health_residual_table_wave92`.
4. **Locomotor residual expand** (`locomotor_bootstrap`, beyond Wave 81):
   - **13** new residual names: Overlord / Marauder / Dragon / Comanche / MIG /
     RocketBuggy / BattleBus / SupplyTruck / Avenger / GattlingTank / Inferno /
     AmericaDozer / Helix.
   - Retail Speed residual (e.g. Overlord **20**, Comanche **120**, MIG **160**,
     Helix **75**, RocketBuggy **90**).
   - Unit template → SET_NORMAL name residual binding expanded.
   - Honesty: `honesty_locomotor_residual_expand_wave92`.
5. **Science residual full name table** (`host_combat_sim_residual`):
   - Complete Science.ini internal-name residual table (**96** entries).
   - Faction / Rank1–8 / America/China/GLA purchasables / Early_ / Chem_ / Slth_ /
     Nuke_ / AirF_ / Infa_ general expand residual anchors.
   - Honesty: `honesty_science_name_table_residual_wave92`.
6. **Combined pack**: `honesty_combat_sim_residual_pack_wave92`.

**Wiring:**
- `game_logic/host_combat_sim_residual.rs` (new — science + body tables)
- `game_logic/host_armor_residual.rs` — Wave 92 armor expand
- `game_logic/locomotor_bootstrap.rs` — Wave 92 locomotor expand
- `game_logic/weapon_bootstrap.rs` — Wave 92 weapon deepen honesty
- `game_logic/mod.rs` — module + pub use honesty
- `shell_smoke.rs` — weapon92/armor92/body92/loco92/science92 fields + detail tokens
- `shell_smoke_gate.rs` — require wave92 honesty flags; playable_claim stays false
- Wave 93 render/terrain residual module co-shipped; closed in dedicated Wave 93 section

**Gates:**
- Unit: 6 wave92 honesty tests PASS (+ wave93 residual co-ship tests)
- golden_skirmish_gate --frames 8 → playable_claim=true
- shell_smoke_gate → playable_claim=false shell_host_playable_ok=true
  weapon92=true armor92=true body92=true loco92=true science92=true

**Not claimed:**
- Full Weapon.ini parse / full ClipSize volley state machine residual
- Full Armor.ini multi-template / ArmorSet PLAYER_UPGRADE matrix residual
- Full ActiveBody ArmorSet swap / MaxHealthUpgrade exclusive modules
- Full multi-surface SET_PANIC / pitch-roll locomotor matrix residual
- Full ScienceStore NameKey purchase cost / prereq graph evaluation residual
- shell playable_claim / network (deferred)

**Honesty rules preserved:**
- Shell playable_claim remains **false**
- Golden playable_claim remains **true**
- Network residual deferred

## Residual Host Playability — Wave 91: tooltip / HelpBox / message / EVA / video / mission briefing residual peels (2026-07-13)

**Closed (host-testable residual peels; orthogonal UI presentation residual):**
1. **Tooltip residual peels** (`host_ui_presentation_residual`, Mouse.ini + Mouse.cpp):
   - Font Arial **8** not bold; AnimateBackground **No**; FillTime **250**ms; DelayTime **800**ms.
   - Width **20%** (fraction **0.20**); word-wrap **120**px residual.
   - Colors text 220/220/220, highlight 255/255/0, border 60/60/155, background 20/20/20.
   - AltTextColor **No** / AltBackColor **Yes** / AdjustAltColor **Yes**.
   - Override delay residual (`m_tooltipDelay >= 0` replaces INI delay).
   - Honesty: `honesty_tooltip_residual_pack_wave91`.
2. **HelpBox residual peels** (ControlBarPopupDescription build tooltip layout):
   - Layout **ControlBarPopupDescription.wnd**; StaticText Name/Cost/Description ids.
   - Subjects CommandButton / MoneyDisplay / PowerWindow / GeneralsExp residual.
   - CanMakeStatus messages: no-money / queue-full / parking / maxed unit|structure.
   - Honesty: `honesty_help_box_residual_pack_wave91`.
3. **Message residual peels** (InGameUI.ini + InGameUI.h/cpp):
   - MAX_UI_MESSAGES **6**; Color1 white / Color2 180,180,255; pos **(10,10)**; Arial **10** Bold.
   - MessageDelayMS retail **75000**; C++ timeout residual `delay/30/1000` → **2** frames.
   - FloatingText timeout **333**ms → **10**f; move-up **1.0**/f; vanish **0.1**/f.
   - PopupMessage **InGamePopupMessage.wnd** white residual.
   - Honesty: `honesty_message_residual_pack_wave91`.
4. **EVA residual peels** (Eva.h / Eva.cpp TheEvaMessageNames):
   - EVA_COUNT **53**; ordered name table LOWPOWER..SNEAK_ATTACK enemy residual.
   - CheckInfo defaults priority **1**, between **900**f (30s), expire **150**f (5s).
   - Enabled default **Yes**; TRIGGEREDON_NOT **MAX**; NEXT_CHECK_NOW **0**.
   - Honesty: `honesty_eva_residual_pack_wave91`.
5. **Video residual name table** (Video.ini names only — not Bink codec):
   - **41** internal-name → filename residual rows (Sizzle, EALogo, GC portraits, MD campaigns).
   - VideoBuffer::Type residual NUM_TYPES **5** (UNKNOWN..X1R5G5B5).
   - MD_China/GLA/USA 01..05 + GeneralsChallengeBackground anchors.
   - Honesty: `honesty_video_residual_name_table_wave91`.
6. **Mission briefing residual peels** (CampaignManager + Campaign.ini):
   - MAX_OBJECTIVE_LINES **5**; MAX_DISPLAYED_UNITS **3**; MAX_SUBTITLE_LINES **4**.
   - TRAINING Mission01 residual map/intro/BriefingVoice/objectives/units/VoiceLength **17**.
   - USA/GLA/China Mission01+05 IntroMovie residual linked to Video.ini names.
   - Military caption Courier New 12 title bold / body not bold; pos **(10,340)**; randomize typing **Yes**.
   - SinglePlayerLoadScreen.wnd + MilitarySubtitlesTyping audio residual.
   - Honesty: `honesty_mission_briefing_residual_pack_wave91`.

**Wiring:**
- `game_logic/host_ui_presentation_residual.rs` (new)
- `game_logic/mod.rs` — module + pub use honesty
- `shell_smoke.rs` — tooltip91/helpbox91/message91/eva91/video91/briefing91 fields + detail tokens
- `shell_smoke_gate.rs` — require wave91 honesty flags; playable_claim stays false
- Combined pack: `honesty_ui_presentation_residual_pack_wave91`

**Gates:**
- Unit: 7 wave91 honesty tests
- golden_skirmish_gate --frames 8 → playable_claim=true
- shell_smoke_gate → playable_claim=false shell_host_playable_ok=true
  tooltip91=true helpbox91=true message91=true eva91=true video91=true briefing91=true

**Not claimed:**
- Full Mouse tooltip GPU draw / ControlBar help-box animate residual
- Full UIMessage DisplayString / FloatingText GPU residual
- Full Eva speech Miles playback residual
- Full Bink video codec / stream decode residual
- Full LoadScreen objective GPU / briefing voice playback residual
- shell playable_claim / network (deferred)

**Honesty rules preserved:**
- Shell playable_claim remains **false**
- Golden playable_claim remains **true**
- Network residual deferred

## Residual Host Playability — Wave 90: GameSpeed / frame-rate deepen / debug tables / language deepen / credits residual peels (2026-07-13)

**Closed (host-testable residual peels; orthogonal engine timing + shell UI residual):**
1. **GameSpeed residual** (`host_timing_shell_residual`, GameCommon.h / GameEngine.h):
   - LOGICFRAMES_PER_SECOND **30**; MSEC_PER_SECOND **1000**.
   - MSEC_PER_LOGICFRAME_REAL **1000/30**; LOGICFRAMES_PER_MSEC_REAL **0.03**.
   - SECONDS_PER_LOGICFRAME_REAL **1/30**.
   - DEFAULT_MAX_FPS **45** (GameEngine constructor); GameData FramesPerSecondLimit **30**.
   - GlobalData ctor FPS limit **0** / UseFPSLimit **false** before INI.
   - ConvertDurationFromMsecsToFrames residual + ceil call pattern (1000ms→30f, 33ms→1f).
   - Honesty: `honesty_gamespeed_residual_pack_wave90`.
2. **Frame rate residual deepen** (beyond Wave 86 FPSLimit constants):
   - W3DDisplay FPS_HISTORY_SIZE **30** (matches logic FPS).
   - GameEngine sleep residual `(1000/maxFPS)-1` → 30→**32**ms, 45→**21**ms, 60→**15**ms.
   - Average FPS history residual; post-load FPS lock state UseFPSLimit **Yes** / limit **30**.
   - Honesty: `honesty_frame_rate_residual_deepen_pack_wave90`.
3. **Debug residual tables** (host-only; DebugDisplay.h + W3DDisplay.h):
   - DebugDisplay Color residual WHITE..BLUE **6** (NUM_COLORS).
   - W3DDisplay DisplayString slots FPS..TerrainStats **16** (DisplayStringCount).
   - Anchors: Particles **8**, Objects **9**, NetFPSAverages **13**.
   - Honesty: `honesty_debug_residual_tables_pack_wave90`.
4. **Language residual deepen** (beyond CSF multi-locale path tables):
   - LanguageFilter LANGUAGE_XOR_KEY **0x5555**; BadWordFileName **langdata.dat**.
   - unHaxor residual (leet 1/3/4/5/6/7/0/@/$/+/ph→f; strip `-_*'"`).
   - English Language.ini: MilitaryCaptionSpeed **1**, DelayMS **750**, ResolutionFontAdjustment **0.7**.
   - Credits fonts Arial **22** / **16** bold / **14**; NativeDebugDisplay FixedSys **8**.
   - adjustFontSize residual (base 800, clamp 1.0..2.0; 1600→floor(12*1.7)=**20**).
   - Honesty: `honesty_language_residual_deepen_pack_wave90`.
5. **Credits residual** (Credits.h/.cpp + Credits.ini):
   - Styles TITLE/MINORTITLE/NORMAL/COLUMN; MAX_CREDIT_STYLES **5**; SPACE_OFFSET **2**.
   - Ctor ScrollRate **1**/frame every **1**; ScrollDown **Yes**; default white.
   - Credits.ini ScrollRate **2**, ScrollRateEveryFrames **1**, ScrollDown **No**.
   - Title/MinorTitleColor **161,179,255,255**; NormalColor **209,218,255,255**.
   - GameMakeColor residual (A<<24)|(R<<16)|(G<<8)|B; path Data\\INI\\Credits.ini.
   - String-label residual via ':' (CREDITS:ExecutiveProducer vs quoted names).
   - Honesty: `honesty_credits_residual_pack_wave90`.
6. Tests / gates:
   - Combined honesty: `honesty_timing_shell_residual_pack_wave90`.
   - shell_smoke: gamespeed90/framerate90/debug90/lang90/credits90 honesty flags wired
     (playable_claim stays false)
   - Unit: 6 wave90 honesty tests PASS
   - golden_skirmish_gate --frames 8 → PASS playable_claim=true
   - shell_smoke_gate → PASS playable_claim=false shell_host_playable_ok=true
     gamespeed90=true framerate90=true debug90=true lang90=true credits90=true

**Still residual (fail-closed, not claimed):**
- Full GameEngine main-loop sleep / live FPS lock residual
- Full W3DDisplay drawDebugStats GPU residual
- Full LanguageFilter langdata.dat live load / network chat filter residual
- Full CreditsMenu.wnd GPU / scroll DisplayString residual
- Shell `playable_claim` remains false (no windowed W3D retail claim)
- Network residual replication (network deferred)

---

## Residual Host Playability — Wave 89: rank skill-points / experience / hotkey / chat / replay / options residual peels (2026-07-13)

**Closed (host-testable residual peels; orthogonal GeneralsExperience + local UI residual):**
1. **Rank skill-points application residual deepen** (`host_rank_ui_residual`, beyond Wave 80 Rank.ini table):
   - Freezes C++ `Player::addSkillPoints` / `setRankLevel` / `resetRank` residual path.
   - SkillPointsModifier default **1.0**; RankLevelLimit default **1000**.
   - REAL_TO_INT_CEIL modifier scale residual; pointCap at SkillPointsNeeded of cap level
     (Rank5 **5000** under default limit).
   - resetRank: rank **1**, skill **0**, levelUp **800**, SPP = intrinsic + Rank1 grant **1**.
   - Multi-rank while-loop residual (800→2, 5000→5); downgrade resets then re-grants.
   - ControlBar progress residual `((skill-levelDown)*100)/(levelUp-levelDown)`.
   - Honesty: `honesty_rank_skill_points_application_residual_pack_wave89`.
2. **Experience residual tables pack**:
   - Veterancy levels REGULAR/VETERAN/ELITE/HEROIC **0..3**, LEVEL_COUNT **4**.
   - USE_EXP_VALUE_FOR_SKILL_VALUE sentinel **−999** (SkillPointValue defaults to ExperienceValue).
   - ExperienceRequired ladders: light infantry **0/40/60/120**, standard **0/100/200/400**,
     heavy **0/200/400/800**, vehicle alt **0/100/150/300**.
   - ExperienceValue anchors: Ranger **20/20/40/60**, air **50/100/150/200**, structure flat **200**.
   - HealthBonus residual Regular **100%** / Vet **120%** / Elite **130%** / Heroic **150%**.
   - Scalar default **1.0**; ally kill XP residual **0**; scaled addExperience residual.
   - Honesty: `honesty_experience_residual_tables_pack_wave89`.
3. **Hotkey residual table** (English CommandMap.ini):
   - Active CommandMap count residual **96**; control groups **0..9**; save views **1..8**.
   - Anchors: CHAT_ALLIES **KEY_BACKSPACE**, CHAT_EVERYONE **KEY_ENTER**,
     SELECT_MATCHING_UNITS **KEY_E**, CREATE_TEAM0 **CTRL+0**, SELECT_TEAM0 **0**,
     PLACE_BEACON **CTRL+B**, TOGGLE_FAST_FORWARD_REPLAY **KEY_F**, SAVE/VIEW F1 split.
   - CHAT_PLAYERS CommandMap residual **commented out** in retail English map.
   - HotKeyManager residual: lowercase store; raw-key path requires no modifiers.
   - Honesty: `honesty_hotkey_residual_table_pack_wave89`.
4. **Chat residual host peels** (local UI; not network):
   - InGameChatType residual Allies **0** / Everyone **1** / Players **2**.
   - Labels Chat:Everyone/Allies/Players/Observers; default show type **Everyone**.
   - InGameChat.wnd + TextEntryChat / StaticTextChatType id residual.
   - MAX_SLOTS residual **8**; chat blocked in replay residual.
   - Honesty: `honesty_chat_residual_host_pack_wave89`.
5. **Replay residual host peels** (local Recorder; not network):
   - RecorderModeType RECORD **0** / PLAYBACK **1** / NONE **2**.
   - Extension **.rep**; dir **Replays\\**; last replay stem **00000000**.
   - SaveCameraInReplays / UseCameraInReplays default **Yes**.
   - TOGGLE_FAST_FORWARD_REPLAY residual + chat-in-replay block shared residual.
   - Honesty: `honesty_replay_residual_host_pack_wave89`.
6. **Options residual peels** (OptionPreferences defaults):
   - AudioSettings volumes Music **55%** / Speech **70%** / SFX **80%** / SFX3D **80%**;
     Relative2DVolume **−10%** → default 2D SFX residual **72**.
   - Gamma default **50**; ScrollFactor default **0.5** (0..100 clamp).
   - LanguageFilter **Yes**; SendDelay **No**; UseSystemMapDir **Yes**; FPSLimit **Yes**.
   - Particle cap min clamp **100**; TextureReduction max **2**.
   - Honesty: `honesty_options_residual_pack_wave89`.
7. Tests / gates:
   - Combined honesty: `honesty_rank_ui_residual_pack_wave89`.
   - shell_smoke: rank89/exp89/hotkey89/chat89/replay89/options89 honesty flags wired
     (playable_claim stays false)
   - Unit: 7 wave89 honesty tests PASS
   - golden_skirmish_gate --frames 8 → PASS playable_claim=true
   - shell_smoke_gate → PASS playable_claim=false shell_host_playable_ok=true
     rank89=true exp89=true hotkey89=true chat89=true replay89=true options89=true

**Still residual (fail-closed, not claimed):**
- Full RankInfoStore live INI load / GeneralsExperience skill-point UI GPU residual
- Full ExperienceTracker exclusive module matrix / XP sink live path residual
- Full HotKeyManager WND binding / MetaEvent message stream residual
- Full InGameChat.wnd GPU / network chat replication residual
- Full Recorder .rep I/O / TiVo playback GPU residual
- Full OptionsMenu.wnd GPU / Options.ini write residual
- Shell `playable_claim` remains false (no windowed W3D retail claim)
- Network residual replication (network deferred)

---

## Residual Host Playability — Wave 88: FX/OCL/particle/audio/cursor residual name tables (2026-07-13)

**Closed (host-testable residual peels; C++ / INI name-table honesty for superweapon FX stack):**
1. **RadiusCursor residual name table** (`host_fx_audio_cursor_residual`):
   - Freezes C++ `RadiusCursorType` + `TheRadiusCursorNames` (InGameUI.h).
   - `RADIUSCURSOR_COUNT` **30**; ordered names NONE..AMBULANCE (indices 0..29).
   - Superweapon cluster residual: PARTICLECANNON **9**, A10STRIKE **10**,
     CARPETBOMB **11**, DAISYCUTTER **12**, SPECTREGUNSHIP **15**,
     NUCLEARMISSILE **17**, ARTILLERYBARRAGE **19**, SCUDSTORM **22**,
     ANTHRAXBOMB **23**.
   - Honesty: `honesty_radius_cursor_name_table_wave88`.
2. **MouseCursor residual name table**:
   - Freezes C++ `CursorININames` (Mouse.cpp) with ALLOW_SURRENDER /
     ALLOW_DEMORALIZE **off**.
   - `NUM_MOUSE_CURSORS` **40**; None..ParticleUplinkCannon (indices 0..39).
   - Superweapon anchors: GenericInvalid **12** (InvalidCursor residual),
     CaptureBuilding **21**, ParticleUplinkCannon **39**.
   - Honesty: `honesty_mouse_cursor_name_table_wave88`.
3. **FXList residual name table (superweapons)**:
   - Freezes **28** retail `FXList.ini` superweapon FX residual names.
   - Daisy (Explode/Ignite/Final), A10 ignition/explosion, ScudStormIgnition,
     PUC BeamHitsGround / BeamLaunchIteration / DeathInitial, Nuke / NukeGLA /
     BaikonurNuke, AnthraxBomb / AnthraxGammaBomb, CarpetBomb, ArtilleryBarrage,
     Spectre howitzer/gattling/explosion, StructureLarge/Medium/Small/TinyDeath.
   - Honesty: `honesty_superweapon_fxlist_name_table_wave88`.
4. **ObjectCreationList residual names (OCL superweapons)**:
   - Freezes **31** retail `ObjectCreationList.ini` SUPERWEAPON_* + aftermath OCL
     residual names.
   - Host core: DaisyCutter, NeutronMissile, ScudStorm, ArtilleryBarrage1/2/3,
     A10ThunderboltMissileStrike1/2/3, AnthraxBomb(+Gamma), CarpetBomb +
     AirF_/China_/Nuke_ variants, CruiseMissile, SupW_NeutronMissile.
   - Aftermath: OCL_NukeRadiationField, OCL_PoisonFieldAnthraxBomb,
     OCL_ParticleUplinkDeathFinal, OCL_SDILinkLasers, OCL_ABPowerPlantExplode,
     OCL_GenericMissileDisintegrate, OCL_SpectreDeathFinalBlowUp.
   - Honesty: `honesty_superweapon_ocl_name_table_wave88`.
5. **Particle system residual name table expand**:
   - Freezes **45** retail particle / laser residual names (expand Wave 81 PUC
     outer-node flare residual).
   - Full PUC set: OuterNode Light/Medium/Intense flares, InnerConnector
     Medium/Intense flares, LaserBaseReady, LaunchFlare, Fire/Sparks/Magma/
     Shockwave, Medium/IntenseConnectorLaser + OrbitalLaser objects, SupW_
     variants.
   - Superweapon anchors: DaisyExplosion(+Gas/Smoke), NukeMushroomRing/Stem/
     Shockwave(+GLA), ScudMissleExplosion/Smoke/LenzFlare, AnthraxBombExplosion,
     CarpetBombWave, ArtilleryBarrageDust/Shockwave/Trail, Spectre howitzer/
     contrail/engine residual.
   - Honesty: `honesty_superweapon_particle_name_table_wave88`.
6. **Audio event residual name table expand**:
   - Freezes **34** retail `SoundEffects.ini` AudioEvent residual names
     (expand Wave 77 InitiateSound residual).
   - Wave 77 anchors retained: ScudStormInitiated, FireArtilleryCannonSound,
     AirRaidSiren.
   - Expand: ScudStormLaunch/Select, Daisy weapon/gas/ignite/explosion, A10
     weapon/ambient/dive/explosion, Neutron building open/launch/hiss +
     ExplosionNeutron/MiniNuke, PUC Powerup/Unpack/Firing/GroundAnnihilation
     loops (**4**), Anthrax explosion/pool, Carpet/Artillery explosion +
     whistle, Spectre ambient/afterburner/gattling/howitzer,
     Cin_CruiseMissileAmbientLoop.
   - Honesty: `honesty_superweapon_audio_event_name_table_wave88`.
7. Tests / gates:
   - Combined honesty: `honesty_fx_audio_cursor_residual_pack_wave88`.
   - shell_smoke: radius88/mouse88/fxlist88/ocl88/particle88/audio88
     honesty flags wired (playable_claim stays false)
   - Unit: 7 wave88 honesty tests PASS
   - golden_skirmish_gate --frames 8 → PASS playable_claim=true
   - shell_smoke_gate → PASS playable_claim=false shell_host_playable_ok=true
     radius88=true mouse88=true fxlist88=true ocl88=true particle88=true
     audio88=true

**Still residual (fail-closed, not claimed):**
- Full FXListExecutor particle/sound/decal application residual
- Full OCL DeliverPayload / FireWeapon / Attack spawn matrix residual
- Full ParticleSystemManager GPU / W3D bone-world FX attach residual
- Full Miles positional playback / AudioEvent INI load residual
- Full CursorManager / RadiusCursor GPU draw residual
- Shell `playable_claim` remains false (no windowed W3D retail claim)
- Network residual replication (network deferred)

---

## Residual Host Playability — Wave 87: weather/water/bridge/tunnel/garrison/transport residual peels (2026-07-13)

**Closed (host-testable residual peels; orthogonal environment + contain residual):**
1. **Weather residual pack** (`host_env_contain_residual`):
   - Freezes retail `Weather.ini` / C++ `WeatherSetting` snow defaults.
   - SnowEnabled **No**; texture **ExSnowFlake.tga**; BoxDimensions **200** /
     Density **1**; FrequencyScaleX/Y **0.0533/0.0275**; Amplitude **5**;
     Velocity **4**; PointSize **1** / Max **64** / Min **0**; PointSprites **Yes**;
     QuadSize **0.5**; SnowManager noise table **64×64**.
   - Honesty: `honesty_weather_residual_pack_wave87`.
2. **Water residual pack**:
   - Freezes C++ `TimeOfDay` residual: INVALID **0** / MORNING **1** / AFTERNOON **2** /
     EVENING **3** / NIGHT **4** / COUNT **5**; names NONE/MORNING/AFTERNOON/EVENING/NIGHT.
   - WaterTransparency residual: Depth **3.0**, MinOpacity **1.0**, texture
     **TWWater01.tga**, Additive **No**, RadarColor **R140 G140 B255**.
   - WaterSet residual table MORNING..NIGHT: WaterRepeatCount **32**, day scroll
     **0.002**, night scroll **0**; sky textures TSCloudWis / TSCloudSun / TSStarFeld;
     Diffuse anchors (175/185/225/100); EVENING transparent alpha **96**.
   - Honesty: `honesty_water_residual_pack_wave87`.
3. **Bridge residual pack**:
   - BridgeTowerType residual: FROM_LEFT **0** / FROM_RIGHT **1** / TO_LEFT **2** /
     TO_RIGHT **3** / BRIDGE_MAX_TOWERS **4**.
   - MAX_BRIDGE_BODY_FX **3**; Lateral/VerticalScaffoldSpeed defaults **1.0**.
   - Honesty: `honesty_bridge_residual_pack_wave87`.
4. **Tunnel residual deepen** (beyond Wave 64):
   - Wave 64 anchors still hold: MaxTunnelCapacity **10**, TimeForFullHeal
     **5000**ms → **150**f.
   - Deepen: CONTAIN_MAX_UNKNOWN **-1**; KickOutOnCapture **No**; ImmuneToClear **Yes**;
     isGarrisonable **No** / isBustable **Yes** / isTunnelContain **Yes**.
   - Nemesis expiry residual **4×LOGICFRAMES** = **120**f.
   - Heal sliver residual: `max_health / frames_for_full_heal` until complete.
   - Honesty: `honesty_tunnel_residual_deepen_wave87`.
5. **Garrison residual pack**:
   - MAX_GARRISON_POINTS **40**; conditions PRISTINE/DAMAGED/REALLY_DAMAGED **0/1/2**
     (COUNT **3**); GARRISON_INDEX_INVALID **-1**.
   - MUZZLE_FLASH_LIFETIME residual **LOGICFRAMES/7** = **4**f.
   - ContainMax residual: bunker/palace **5**, FireBase **4**, civilian **10**.
   - Bunker ImmuneToClear **Yes**; FireBase IsEnclosing **No** + DamagePercent **100%**.
   - Enter/Exit audio residual GarrisonEnter / GarrisonExit.
   - Honesty: `honesty_garrison_residual_pack_wave87`.
6. **Transport residual pack**:
   - TransportContainModuleData defaults: Slots **0**, ScatterNearby **Yes**,
     HealthRegen **0**, ExitDelay **0**, GoAggressive **No**, ResetMood **Yes**.
   - OpenContain defaults: ExitPaths **1**, PassengersFire **No**, DoorOpenTime **1**.
   - Cross-unit slot residual table: Humvee/Technical **5**, TroopCrawler/Chinook/
     BattleBus **8**, ListeningOutpost **2**, Ambulance **3**.
   - ExitDelay residual: Humvee **250**ms→**8**f, Chinook **100**ms→**3**f.
   - HealthRegen residual formula: `max * regen%/100 * SECONDS_PER_LOGICFRAME`
     (Ambulance **25%**, TroopCrawler/ListeningOutpost **10%**).
   - Honesty: `honesty_transport_residual_pack_wave87`.
7. Tests / gates:
   - Combined honesty: `honesty_env_contain_residual_pack_wave87`.
   - shell_smoke: weather87/water87/bridge87/tunnel87/garrison87/transport87
     honesty flags wired (playable_claim stays false)
   - Unit: 7 wave87 honesty tests PASS
   - golden_skirmish_gate --frames 8 → PASS playable_claim=true
   - shell_smoke_gate → PASS playable_claim=false shell_host_playable_ok=true
     weather87=true water87=true bridge87=true tunnel87=true garrison87=true
     transport87=true (plus wave86 cam/world/mpopt/mapsel/crate)

**Still residual (fail-closed, not claimed):**
- Full SnowManager GPU point-sprite / noise-table residual
- Full W3DWater reflection / skybox mesh residual
- Full BridgeBehavior scaffolding motion / dozer repair path residual
- Full TunnelTracker last-tunnel cave-in / CaveSystem multi-index residual
- Full GarrisonContain fire-point bone matrix / mobile garrison residual
- Full TransportContain exit-door / extra-slots-in-use residual matrix
- Shell `playable_claim` remains false (no windowed W3D retail claim)
- Network residual replication (network deferred)

---

## Residual Host Playability — Wave 86: GameData camera/FPS/world + multiplayer options + map selection + crate deepen residual peels (2026-07-13)

**Closed (host-testable residual peels; orthogonal GameData / lobby / map / crate residual):**
1. **GameData camera / FPS residual pack** (`host_gamedata_lobby_residual`):
   - UseFPSLimit residual **Yes**; FramesPerSecondLimit residual **30**.
   - CameraPitch **37.5** / Yaw **0** / Height **232** / Max **310** / Min **120**.
   - CameraAdjustSpeed **0.3**; ScrollAmountCutoff **50**; EnforceMaxCameraHeight **No**.
   - KeyboardCameraRotateSpeed **0.1**; CameraAudibleRadius **250**.
   - Honesty: `honesty_gamedata_camera_fps_residual_pack_wave86`.
2. **GameData world constants residual pack**:
   - Scroll factors: Horizontal **1.6** / Vertical **2.0** / Keyboard **2.0**.
   - Gravity **-64**; PartitionCellSize **40**; TerrainHeightAtEdgeOfMap **100**.
   - DefaultOcclusionDelay **3000**ms → **90**f; DefaultStructureRubbleHeight **10**.
   - UnitDamagedThreshold **0.7** / ReallyDamaged **0.35**; MovementPenaltyDamageState **REALLYDAMAGED**.
   - MinDistFromEdgeOfMapForBuild **30**; SupplyBuildBorder **20**; AllowedHeightVariationForBuilding **10**.
   - SellPercentage **50%** (sell refund residual); StealthFriendlyOpacity **50%**.
   - BaseRegenHealthPercentPerSecond **0.3%** / BaseRegenDelay **3000**ms → **90**f.
   - UnlookPersistDuration **5000**ms → **150**f.
   - ShroudColor white; ClearAlpha **255** / FogAlpha **127** / ShroudAlpha **0**.
   - MaxParticleCount **2500**; MaxFieldParticleCount **30**; MaxLineBuildObjects **50**.
   - CommandCenterHealRange **500** / HealAmount **0.01** per logic frame.
   - Honesty: `honesty_gamedata_world_constants_residual_pack_wave86`.
3. **Multiplayer options residual pack** (host-only; not network play):
   - MultiplayerSettings: StartCountdownTimer **5**s; MaxBeaconsPerPlayer **3**; UseShroud **No**.
   - ShowRandomPlayerTemplate / StartPos / Color residual **Yes**.
   - MultiplayerColor residual table **8** (Gold/Red/Blue/Green/Orange/SkyBlue/Purple/Pink)
     with day/night RGB (Purple+Pink distinct night).
   - Beacon placement residual: allowed when count < MaxBeaconsPerPlayer.
   - Honesty: `honesty_multiplayer_options_residual_pack_wave86`.
4. **Map selection residual pack**:
   - ShellMapName residual `Maps\ShellMapMD\ShellMapMD.map`; default MapName **Assault.map**.
   - Default skirmish map residual **Defcon6** (`MAP:Defcon6`, 6 players).
   - Official MapCache sample anchors (8): AlpineAssault 2p / ArmoredFury 6p / Defcon6 6p /
     TournamentCity 6p / TournamentContinent 4p / TournamentPlains 2p /
     BarrenBadlands 2p / BitterWinter 2p.
   - Host player-count support residual: 2..numPlayers for multiplayer maps.
   - Honesty: `honesty_map_selection_residual_pack_wave86`.
5. **Crate residual deepen pack**:
   - SalvageCrateCollide: WeaponChance **100%** / LevelChance **25%** / MoneyChance **75%**
     (level+money=100%); Min/MaxMoney **25/75**; PickupScience **SCIENCE_GLA**;
     KilledByType **SALVAGER**; DeletionUpdate lifetime **30000–35000**ms.
   - Dollar crate matrix residual: 100/200/1000/1500/2500 + SupplyDropZone **250**.
   - EliteTankCrateData CreationChance **0.75**; HeroicTankCrateData **1.0**.
   - SmallLevelUp EffectRange **100** / MediumLevelUp **250**.
   - 2FreeCrusadersCrate UnitCount **2** / UnitName **AmericaTankCrusader**.
   - Honesty: `honesty_crate_residual_deepen_pack_wave86`.
6. Combined: `honesty_gamedata_lobby_residual_pack_wave86`.

**Still residual (fail-closed, not claimed):**
- Full GlobalData live INI reload / View camera GPU path residual
- Full MultiplayerSettings live lobby combo / network matchmaking residual
- Full MapCache.ini parse / MapSelect UI GPU residual
- Full SalvageCrateCollide W3D subobject / weapon-set upgrade matrix
- Shell `playable_claim` / network residual (network deferred)

---

## Residual Host Playability — Wave 85: faction side / player template / starting cash / AI personality / victory residual peels (2026-07-13)

**Closed (host-testable residual peels; orthogonal skirmish-lobby / multiplayer setup residual):**
1. **Faction side residual table** (`host_faction_skirmish_residual`):
   - Freezes retail `PlayerTemplate.ini` side table (**15** templates, declaration order).
   - Playable residual **13** (excludes Civilian/Observer); OldFaction playable **3**
     (America/China/GLA); ZH general + Boss residual **10**.
   - BaseSide residual: America→**USA**, China→**China**, GLA→**GLA**;
     ZH generals retain base side (AirF/Lazr/SupW→USA, Tank/Infa/Nuke→China,
     Chem/Demo/Slth→GLA, Boss→China).
   - PreferredColor residual: America **R0 G0 B255**, China **R255 G0 B0**,
     GLA **R0 G255 B0**.
   - MAX_PLAYER_COUNT residual **16** (GameCommon.h).
   - Honesty: `honesty_faction_side_residual_table_wave85`.
2. **Player template residual peels**:
   - StartingBuilding / StartingUnit0 residual for base + all ZH generals
     (AmericaCommandCenter + AmericaVehicleDozer; GLA uses GLAInfantryWorker;
     prefixed SupW_/Lazr_/AirF_/Tank_/Infa_/Nuke_/Chem_/Demo_/Slth_/Boss_).
   - IntrinsicSciences residual SCIENCE_AMERICA / SCIENCE_CHINA / SCIENCE_GLA.
   - SpecialPowerShortcutButtonCount residual **10** default; SuperWeapon/AirForce **11**;
     Boss **9**.
   - Honesty: `honesty_player_template_residual_pack_wave85`.
3. **Starting cash residual (+ difficulty health residual)**:
   - GameData DefaultStartingCash **10000** (matches `GameLogic::DEFAULT_STARTING_MONEY`).
   - multiplayer.ini MultiplayerStartingMoneyChoice residual ordered list:
     **5000 / 10000 (Default) / 20000 / 50000**.
   - PlayerTemplate StartMoney residual **0** (lobby cash wins).
   - HumanSoloPlayerHealthBonus residual by difficulty: Easy **150%** / Normal **100%** /
     Hard **80%** (Brutal host maps to Hard residual).
   - Honesty: `honesty_starting_cash_residual_pack_wave85`.
4. **Skirmish AI personality residual peels** (AIData SideInfo):
   - ResourceGatherers residual: America/China (and USA/China generals) **2**;
     GLA (and GLA generals) **5** — Easy/Normal/Hard equal per side in retail.
   - BaseDefenseStructure1 residual: AmericaPatriotBattery / ChinaGattlingCannon /
     GLAStingerSite (+ general-prefixed defenses).
   - SkillSet1 first-science residual anchors: SCIENCE_PaladinTank / SCIENCE_NukeLauncher /
     SCIENCE_ScudLauncher.
   - Host AIPersonality residual: USA→Aggressive, China→Defensive, GLA→Rush,
     Neutral→Balanced (`ai.rs for_team`).
   - Honesty: `honesty_skirmish_ai_personality_residual_pack_wave85`.
5. **Victory condition residual peels**:
   - VictoryType bits residual: NOBUILDINGS **1**, NOUNITS **2**; default both set (**3**).
   - `hasSinglePlayerBeenDefeated` residual matrix: both→hasAnyObjects; NOUNITS→hasAnyUnits;
     NOBUILDINGS→hasAnyBuildings (MP_COUNT_FOR_VICTORY mask residual not fully claimed).
   - Honesty: `honesty_victory_condition_residual_pack_wave85`.
6. Tests / gates:
   - Combined honesty: `honesty_faction_skirmish_residual_pack_wave85`.
   - shell_smoke: faction85/ptpl85/cash85/aiperson85/victory85 honesty flags wired
     (playable_claim stays false)
   - Unit: 6 wave85 honesty tests PASS
   - golden_skirmish_gate --frames 8 → PASS playable_claim=true
   - shell_smoke_gate → PASS playable_claim=false shell_host_playable_ok=true
     faction85=true ptpl85=true cash85=true aiperson85=true victory85=true

**Still residual (fail-closed, not claimed):**
- Full PlayerTemplateStore INI parse / ControlBarScheme side binding matrix
- Full MultiplayerSettings live lobby combo / SkirmishPreferences starting-cash wiring
- Full AIPlayer SideInfo skill-set purchase / SkirmishBuildList dozer path residual
- Full VictoryConditions multiplayer killPlayer / alliance reveal / observer residual
- Shell `playable_claim` remains false (no windowed W3D retail claim)
- Network residual replication (network deferred)

## Residual Host Playability — Wave 84: KindOf/WeaponSlot/Veterancy/Relationship/Geometry/Shadow enum residual tables (2026-07-13)

**Closed (host-testable residual peels; C++ type-name / bit-name table honesty):**
1. **KindOf residual bit-name table** (`host_enum_table_residual`):
   - Freezes C++ `KindOfMaskType::s_bitNameList` (KindOf.cpp) / KindOf.h enum order.
   - `KINDOF_COUNT` residual **116** (ALLOW_SURRENDER off — no PRISON / CAN_SURRENDER bits).
   - Anchors: STRUCTURE **7**, INFANTRY **8**, VEHICLE **9**, AIRCRAFT **10**,
     COMMANDCENTER **14**, PROJECTILE **22**, NO_COLLIDE **27**, FS_FACTORY **58**,
     HERO **85**, FS_SUPERWEAPON **90**, BOOBY_TRAP **97**, EMP_HARDENED **112**,
     IGNORE_DOCKING_BONES **115** (last).
   - Honesty: `honesty_kindof_enum_table_wave84`.
2. **WeaponSlot residual table**:
   - Freezes C++ `TheWeaponSlotTypeNames` (WeaponSet.h) / GameType.h.
   - `WEAPONSLOT_COUNT` residual **3**: PRIMARY **0** / SECONDARY **1** / TERTIARY **2**.
   - Honesty: `honesty_weapon_slot_enum_table_wave84`.
3. **Veterancy residual level table**:
   - Freezes C++ `TheVeterancyNames` (GameCommon.cpp) / GameCommon.h.
   - `LEVEL_COUNT` residual **4**: REGULAR **0** / VETERAN **1** / ELITE **2** / HEROIC **3**.
   - Fail-closed: ROOKIE is not a C++ name (REGULAR is level 0).
   - Honesty: `honesty_veterancy_level_enum_table_wave84`.
4. **Relationship residual table**:
   - Freezes C++ `TheRelationshipNames` (GameCommon.cpp).
   - ENEMIES **0** / NEUTRAL **1** / ALLIES **2** (order is not alphabetical).
   - Fail-closed: NEUTRALS plural is not the C++ name.
   - Honesty: `honesty_relationship_enum_table_wave84`.
5. **Geometry residual type table**:
   - Freezes C++ `GeometryNames` (Geometry.h).
   - `GEOMETRY_NUM_TYPES` residual **3**: SPHERE **0** / CYLINDER **1** / BOX **2**.
   - Honesty: `honesty_geometry_type_enum_table_wave84`.
6. **Shadow residual type table**:
   - Freezes C++ `TheShadowNames` (Shadow.h) bit-name list (**7** entries).
   - SHADOW_NONE **0** not named; bit 0 = SHADOW_DECAL **0x01** … ADDITIVE_DECAL **0x40**.
   - Honesty: `honesty_shadow_type_enum_table_wave84`.
7. Tests / gates:
   - Combined honesty: `honesty_enum_table_residual_pack_wave84`.
   - shell_smoke: kindof84/wslot84/vet84/rel84/geom84/shadow84 honesty flags wired
     (playable_claim stays false)
   - golden_skirmish_gate --frames 8 → PASS playable_claim=true
   - shell_smoke_gate → PASS playable_claim=false shell_host_playable_ok=true
     kindof84=true wslot84=true vet84=true rel84=true geom84=true shadow84=true

**Still residual (fail-closed, not claimed):**
- Full KindOf mask runtime on every ThingTemplate / isKindOf combat filters
- Full WeaponSet fire/slot selection / TERTIARY weapon combat residual matrix
- Full veterancy XP thresholds / health bonus application residual
- Full relationship matrix Player/Team wiring beyond name table
- Full GeometryInfo collision / partition residual
- Full Shadow volume/decal GPU draw residual
- Shell `playable_claim` remains false (no windowed W3D retail claim)
- Network residual replication (network deferred)

## Residual Host Playability — Wave 83: production/supply/dozer/capture/power/command residual peels (2026-07-13)

**Closed (host-testable residual peels; orthogonal structure/economy residual):**
1. **Production queue residual deepen** (`host_structure_economy_residual`):
   - ProductionUpdate MaxQueueEntries **9** (matches `DEFAULT_PRODUCTION_QUEUE_LIMIT`).
   - GameData.ini RefundPercent **50%**, Min/MaxLowEnergyProductionSpeed **0.5/0.8**,
     LowEnergyPenaltyModifier **1.0**, MultipleFactory **1.0**, BuildSpeed **1.0**.
   - USA CC door residual Opening/Wait/Close **1500/3000/1500**ms → **45/90/45**f;
     China CC door **3000/3000/3000**ms → **90/90/90**f; ConstructionComplete **1500**ms → **45**f.
   - `production_power_factor_from_energy_ratio` + cancel-refund helpers.
   - Honesty: `honesty_production_queue_residual_pack_wave83`.
2. **Supply warehouse residual deepen**:
   - SupplyWarehouse StartingBoxes **400**, ApproachPositions **9**, MaxHealth **1000**.
   - SupplyDock **400** boxes / Approach **-1**; SupplyPile **150**/5 + DeleteWhenEmpty;
     SupplyPileSmall **50** + DeleteWhenEmpty.
   - GameData ValuePerSupplyBox **75** (ZH retail override of C++ default 100).
   - CripplingBehavior SelfHealSupression **3000**ms→**90**f / Delay **500**ms→**15**f / Amount **5**.
   - Cash/box helpers + take-one-box empty-destroy residual.
   - Honesty: `honesty_supply_warehouse_residual_pack_wave83`.
3. **Dozer build residual deepen**:
   - AmericaDozer BuildCost **1000** / BuildTime **5**s→**150**f / MaxHealth **250** /
     Vision **200** / TransportSlotCount **5**.
   - DozerAIUpdate RepairHealthPercent **2%**, BoredTime **5000**ms→**150**f, BoredRange **150**.
   - DozerTask ordinals BUILD/REPAIR/FORTIFY **0/1/2**; BuildSubTask residual ordinals.
   - GameData MinDistFromEdge **30**, SupplyBuildBorder **20**, AllowedHeightVariation **10**,
     MaxLineBuildObjects **50**.
   - Construction progress residual: `1/build_time * dozers * power * BuildSpeed`.
   - Honesty: `honesty_dozer_build_residual_pack_wave83`.
4. **Capture building residual deepen**:
   - SpecialAbilityRangerCaptureBuilding / SPECIAL_INFANTRY_CAPTURE_BUILDING.
   - Reload **15000**ms→**450**f, StartAbilityRange **5**, Unpack **3000**ms→**90**f,
     Prep **20000**ms→**600**f, Pack **2000**ms→**60**f, AwardXP **15**, DoCaptureFX **Yes**.
   - Upgrade_InfantryCaptureBuilding gate + hero bypass residual legality matrix.
   - CommandButton CaptureBuilding / SSCaptureBuilding / CONTROLBAR:CaptureBuilding.
   - Honesty: `honesty_capture_building_residual_pack_wave83`.
5. **Power plant residual energy residual**:
   - AmericaPowerPlant EnergyProduction **5** / EnergyBonus **5** / BuildCost **800** /
     BuildTime **10**s / MaxHealth **800** / RodsExtendTime **600**ms→**18**f.
   - ChinaPowerPlant EnergyProduction **10** / EnergyBonus **5** / BuildCost **1000** /
     MaxHealth **1500** / RodsExtend **1**ms→**1**f / Overcharge drain **3%**/sec.
   - Upgrade_AmericaAdvancedControlRods residual; effective energy = base + bonus when upgraded.
   - Honesty: `honesty_power_plant_residual_pack_wave83`.
6. **Command center residual peels**:
   - America/China/GLA CommandCenter BuildCost **2000** / BuildTime **45**s→**1350**f /
     EnergyProduction **0** / MaxHealth **5000** / Vision **300** / XP **200**.
   - KindOf residual tokens (COMMANDCENTER/FS_FACTORY/AUTO_RALLYPOINT/…).
   - GameData CommandCenterHealRange **500** / HealAmount **0.01** per logic frame.
   - USA radar grant Upgrade_AmericaRadar residual name.
   - Honesty: `honesty_command_center_residual_pack_wave83`.
7. Tests / gates:
   - shell_smoke: prod83/supply83/dozer83/capture83/power83/cc83 honesty flags wired
     (playable_claim stays false)
   - golden_skirmish_gate --frames 8 → PASS playable_claim=true
   - shell_smoke_gate → PASS playable_claim=false shell_host_playable_ok=true
     prod83=true supply83=true dozer83=true capture83=true power83=true cc83=true

**Still residual (fail-closed, not claimed):**
- Full ProductionUpdate door-anim / QuantityModifier / parking-place matrix
- Full SupplyWarehouseDockUpdate approach-bone path / ResourceGatheringManager graph
- Full DozerAIUpdate primary state machine / construct scaffolding / fortify
- Full CaptureBuilding BinaryDataStream / ActionManager edge matrix
- Full PowerPlantUpgrade MODELCONDITION rod draw / OverchargeBehavior live drain
- Full CommandCenter radar-extend anim / PreorderCreate / heal update loop wiring
- Shell `playable_claim` remains false (no windowed W3D retail claim)
- Network residual replication (network deferred)

## Residual Host Playability — Wave 82: DamageType/DeathType/ModelCondition/WeaponBonus/ObjectStatus enum residual tables (2026-07-13)

**Closed (host-testable residual peels; C++ bit-name / enum table honesty):**
1. **DamageType residual enum table** (`host_enum_table_residual`):
   - Freezes C++ `DamageTypeFlags::s_bitNameList` (Damage.cpp) / `Damage.h` enum.
   - `DAMAGE_NUM_TYPES` residual **38** (EXPLOSION=0 … STATUS=37).
   - Subdual residual cluster contiguous: SUBDUAL_MISSILE/VEHICLE/BUILDING/UNRESISTABLE
     **31–34**; MICROWAVE **35**; KILL_GARRISONED **36**; STATUS **37**.
   - PARTICLE_BEAM **22**, HAZARD_CLEANUP **21**, KILL_PILOT **16**.
   - Honesty: `honesty_damage_type_enum_table_wave82`.
2. **DeathType residual enum table**:
   - Freezes C++ `TheDeathNames` (Damage.h DEFINE_DEATH_NAMES).
   - `DEATH_NUM_TYPES` residual **21** (NORMAL=0 … POISONED_GAMMA=20).
   - Death names deliberately diverge from damage names residual sample
     (BURNED ≠ FLAME, EXPLODED ≠ EXPLOSION).
   - POISONED_BETA **12** / EXTRA_2…EXTRA_8 **13–19** / POISONED_GAMMA **20**.
   - Honesty: `honesty_death_type_enum_table_wave82`.
3. **ModelCondition residual flags (incl. CONTINUOUS_FIRE_*)**:
   - Freezes C++ `ModelConditionFlags::s_bitNameList` (BitFlags.cpp) / ModelState.h.
   - `MODELCONDITION_COUNT` residual **117** (ALLOW_SURRENDER off — no SURRENDER
     bit between SOLD **79** and RAPPELLING **80**).
   - CONTINUOUS_FIRE_SLOW **84** / MEAN **85** / FAST **86** residual contiguous
     (FiringTracker speedUp/coolDown residual index honesty).
   - FIRING_A **36**, MOVING **49**, DISGUISED **116** residual anchors.
   - Honesty: `honesty_model_condition_enum_table_wave82`.
4. **WeaponBonus residual type table**:
   - Freezes C++ `TheWeaponBonusNames` (Weapon.h; ALLOW_DEMORALIZE off).
   - `WEAPONBONUSCONDITION_COUNT` residual **27** (GARRISONED=0 … FRENZY_THREE=26).
   - DEMORALIZED_OBSOLETE **7** (not live DEMORALIZED); ENTHUSIASTIC **8**;
     SUBLIMINAL **15**; TARGET_FAERIE_FIRE **22**; FANATICISM **23**;
     FRENZY_ONE/TWO/THREE **24/25/26**.
   - CONTINUOUS_FIRE_MEAN **2** / FAST **3** residual (Spectre/Gattling ROF bonuses).
   - Honesty: `honesty_weapon_bonus_enum_table_wave82`.
5. **ObjectStatus / StatusBits residual table**:
   - Freezes C++ `ObjectStatusMaskType::s_bitNameList` (ObjectStatusTypes.cpp).
   - `OBJECT_STATUS_COUNT` residual **45** (NONE=0 … DEPLOYED=44).
   - STEALTHED **16** / DETECTED **17** / IS_CARBOMB **28** / FAERIE_FIRE **38** /
     BOOBY_TRAPPED **41** / DISGUISED **43** / DEPLOYED **44**.
   - STATUS_RIDER1…8 residual cluster **30–37**.
   - Honesty: `honesty_object_status_enum_table_wave82`.
6. Tests / gates:
   - Combined honesty: `honesty_enum_table_residual_pack_wave82`.
   - shell_smoke: dmg82/death82/mc82/wbonus82/ostatus82 honesty flags wired
     (playable_claim stays false)
   - golden_skirmish_gate --frames 8 → PASS playable_claim=true
   - shell_smoke_gate → PASS playable_claim=false shell_host_playable_ok=true
     dmg82=true death82=true mc82=true wbonus82=true ostatus82=true

**Still residual (fail-closed, not claimed):**
- Full armor/weapon combat application of every DamageType discriminant
- Full W3D MODELCONDITION anim draw / FiringTracker model-condition anim matrix
- Full WeaponBonusConditionFlags ROF multiplier application residual matrix
- Full ObjectStatus Xfer rebind / StatusBitsUpgrade / Object::setStatus matrix
- Shell `playable_claim` remains false (no windowed W3D retail claim)
- Network residual replication (network deferred)

## Residual Host Playability — Wave 81: terrain/pathfinder/locomotor/armor/PUC residual peels (2026-07-13)

**Closed (host-testable residual peels; orthogonal to Wave 80 command-button/science):**
1. **Map height sample residual deepen** (`game_logic/terrain`):
   - `MAP_HEIGHT_SAMPLE_XY_FACTOR` **10** / `MAP_HEIGHT_SAMPLE_SCALE` **0.625** residual.
   - Raw 8-bit sample → world Z (`raw_height_sample_to_world`) + bilinear corner blend residual.
   - Pathfinding cell-center offset residual **0.5**.
   - Honesty: `honesty_map_height_sample_residual_pack_wave81`.
2. **Pathfinder residual peels deepen** (`host_pathfinder`):
   - Locomotor **ColonelBurtonGroundLocomotor** Speed **30** / Damaged **20** / Turn **500** / Accel **100**.
   - Armor **HumanArmor** / ChemSuit **ChemSuitHumanArmor** / DamageFX **InfantryDamageFX**.
   - BuildTime **10**s, ExperienceValue **40/40/60/80**, ExperienceRequired **0/50/100/200**.
   - AutoFindHealing ScanRate **1000**ms→**30**f / Range **300** / Never **0.85** / Always **0.25**.
   - MoodAttackCheckRate **250**ms→**8**f, Physics Mass **5**.
   - Honesty: `honesty_pathfinder_residual_pack_wave81` (includes Wave 54 base pack).
3. **Locomotor residual tables for common units** (`locomotor_bootstrap`):
   - Extended seed residual table (12 names): Pathfinder/Burton, Tomahawk, ScudLauncher,
     QuadCannon, RaptorJet + golden infantry/vehicle set.
   - Template → locomotor name residual bindings for Pathfinder / Tomahawk / Scud / Quad / Raptor.
   - `ensure_host_locomotor_store` always fills missing known seeds (no early exit after BasicHuman).
   - Honesty: `honesty_locomotor_residual_table_wave81`.
4. **Armor residual table honesty** (`host_armor_residual`):
   - **ProjectileArmor**: DEFAULT **25%**, LASER **100%**, SMALL_ARMS/GATTLING **25%**,
     FALLING/MICROWAVE/HAZARD_CLEANUP/POISON/RADIATION/FLAME **0%**, SUBDUAL_MISSILE **100%**.
   - **HazardousMaterialArmor**: DEFAULT **0%**, HAZARD_CLEANUP **100%**, FLAME **0%**.
   - Seed-if-missing into `TheArmorStore`; verify via `adjust_damage` residual matrix.
   - Honesty: `honesty_armor_residual_table_wave81`.
5. **PUC outer-node flare particle name tables deepen** (`special_power_strikes`):
   - Structured intensity → OuterNodeLight/Medium/Intense flare residual table.
   - Connector laser / LaserBaseReady / OrbitalLaser residual name table.
   - Commented ConnectorMedium/Intense flare residual names
     (`ParticleUplinkCannon_InnerConnector*Flare`).
   - FX01..FX05 bone residual honesty.
   - Honesty: `honesty_particle_outer_node_flare_name_table_wave81`.
6. Tests / gates:
   - shell_smoke: height81/path81/loco81/armor81/puc81 honesty flags wired
     (playable_claim stays false)
   - golden_skirmish_gate --frames 8 → PASS playable_claim=true
   - shell_smoke_gate → PASS playable_claim=false shell_host_playable_ok=true
     height81=true path81=true loco81=true armor81=true puc81=true

**Still residual (fail-closed, not claimed):**
- Full SAGE HeightMap bridge/cliff bilinear / live HeightMapData decode matrix
- Full Pathfinder AIUpdate AutoAcquire Stealthed / W3D model draw
- Full multi-surface / SET_PANIC / pitch-roll Locomotor.ini matrix
- Full Armor.ini multi-template / ArmorSet upgrade graph / ActiveBody swap
- Full ParticleSystemManager outer-node FX attach / W3D bone-world extract
- Shell `playable_claim` remains false (no windowed W3D retail claim)
- Network residual replication (network deferred)

## Residual Host Playability — Wave 80: CommandButton / SCIENCE rank / KindOf / SpecialPower enum residual peels (2026-07-13)

**Closed (host-testable residual peels; INI-backed superweapon/science residual):**
1. **CommandButton superweapon residual pack** (`host_command_button_residual`):
   - Retail CommandButton.ini TextLabel / DescriptLabel / ButtonImage /
     RadiusCursorType / CursorName residual for all **10** `HostSuperweaponKind`s.
   - Daisy **DAISYCUTTER** / A10 **A10STRIKE** / Scud **SCUDSTORM** /
     Nuke **NUCLEARMISSILE** / Anthrax **ANTHRAXBOMB** / Spectre **SPECTREGUNSHIP** /
     Carpet **CARPETBOMB** / Artillery **ARTILLERYBARRAGE** / Cruise **NUCLEARMISSILE**.
   - Particle Uplink residual uses **CursorName=ParticleUplinkCannon** (no RadiusCursor).
   - Shortcut command/text residual names frozen per kind.
   - Honesty: `honesty_command_button_superweapon_residual_pack_wave80`.
2. **SCIENCE rank residual table completeness** (`host_science_rank`):
   - Full retail Rank.ini residual table ranks **1–5**:
     SkillPointsNeeded **0 / 800 / 1500 / 2500 / 5000**.
     SciencePurchasePointsGranted **1 / 1 / 1 / 1 / 3**.
     SciencesGranted SCIENCE_Rank1…Rank5; RankName INI:RankLevel1…5.
   - Cumulative SPP through rank 5 = **7**; skill→rank lookup residual.
   - Honesty: `honesty_science_rank_residual_pack_wave80`.
3. **Object KindOf residual packs for common superweapon buildings**
   (`host_superweapon_kindof`):
   - AmericaParticleCannonUplink / GLAScudStorm / ChinaNuclearMissileLauncher.
   - Shared tokens: PRELOAD STRUCTURE SELECTABLE IMMOBILE CAPTURABLE
     FS_TECHNOLOGY FS_SUPERWEAPON MP_COUNT_FOR_VICTORY.
   - Economy residual: BuildCost **5000**, BuildTime **60**s, MaxHealth **4000**.
   - Particle/Nuke: POWERED + SCORE + EnergyProduction **-10**.
   - Scud: SCORE_CREATE, no POWERED, EnergyProduction **0**.
   - Honesty: `honesty_superweapon_kindof_residual_pack_wave80`.
4. **SpecialPower enum residual discriminants** (`host_special_power_enum_residual`):
   - C++ `s_bitNameList` residual table length **67** (SPECIALPOWER_COUNT).
   - HostSuperweaponKind → SPECIAL_* name + ordinal residual
     (Daisy **1**, Carpet **3**, Neutron **8**, Anthrax **14**, Scud **15**,
     A10 **18**, Artillery **20**, Particle **36**, Spectre **42**, Cruise **63**).
   - Host `command_system::SpecialPowerType` → C++ SPECIAL_* bridge residual.
   - Honesty: `honesty_special_power_enum_residual_pack_wave80`.
5. Tests / gates:
   - Unit honesty tests for all four wave80 residual packs.
   - shell_smoke: cmdbtn80/rank80/kindof80/spenum80 honesty flags wired
     (playable_claim stays false)
   - golden_skirmish_gate --frames 8 → PASS playable_claim=true
   - shell_smoke_gate → PASS playable_claim=false shell_host_playable_ok=true
     cmdbtn80=true rank80=true kindof80=true spenum80=true

**Still residual (fail-closed, not claimed):**
- Full CommandButton INI parse / Science-swap cameo matrix / CursorManager GPU
- Full RankInfoStore live INI load / GeneralsExperience skill-point UI
- Full ThingTemplate KindOf bit matrix / MaxSimultaneousOfType SW restriction UI
- Full SpecialPowerStore Xfer rebind / SpecialPowerMask bit ops host-wide
- Shell `playable_claim` remains false (no windowed W3D retail claim)
- Network residual replication (network deferred)

## Residual Host Playability — Wave 79: minimap/selection/input/drawable/training/upgrade residual peels (2026-07-13)

**Closed (host-testable residual peels; orthogonal to Wave 78 special powers):**
1. **Minimap residual honesty pack** (`graphics/minimap_renderer`):
   - Standard size **256**, default world span **1024**, screen origin **10**.
   - FOW shade residual Visible **1.0** / Explored **0.5** / Hidden **0.12**.
   - Pure FOW RGBA Hidden/Explored/Visible residual + soft-edge **3/1** weight residual.
   - CELL 0/1/2 bridge residual.
   - Honesty: `honesty_minimap_residual_pack_wave79`.
2. **Selection/HUD residual pack** (`selection_renderer`):
   - Selection/hover/friendly/enemy/neutral/health-bar color residual defaults.
   - Circle radius **3.0**, thickness **0.2**, health bar **4.0×0.3** offset **2.0**.
   - Pulse speed **2.0** + alpha residual `sin*0.3+0.7` clamp **[0.4,1.0]**.
   - Honesty: `honesty_selection_hud_residual_pack_wave79`.
3. **Input residual pack** (`unit_input_handler` / `unit_control`):
   - Drag-select threshold **5** px residual.
   - Double-click select-type window **0.3** s residual.
   - Honesty: `honesty_input_residual_pack_wave79`.
4. **Drawable residual save/load fields** (`save_load/snapshot`):
   - `ObjectStatusSnapshot.camo_stealth_look` freezes `Drawable::m_stealthLook` ordinal
     residual (`Object::camo_stealth_look`) with Xfer append + restore.
   - Honesty: `honesty_drawable_residual_fields_wave79_ok` +
     `drawable_camo_stealth_look_snapshot_residual_wave79`.
5. **Unit training / veterancy residual deepen** (`host_unit_training` / `object`):
   - GameData.ini HealthBonus / WeaponBonus residual matrix (Vet **120%/110%/120% RoF**,
     Elite **130%/120%/140%**, Heroic **150%/130%/160%**).
   - AdvancedTraining ExperienceScalar **AddXPScalar 1.0** residual **application**
     in `Object::gain_experience` when upgrade tag present (2× XP).
   - Honesty: `honesty_unit_training_residual_pack_wave79_ok`.
6. **Upgrade cost/time residual application** (`host_upgrades` / `command_executor`):
   - `HostUpgradeResearch` stamps `build_cost_paid` + `retail_research_frames` +
     `residual_research_frames` (**1** host path) at queue.
   - `resolve_upgrade_cost_supplies` prefers HostUpgradeKind retail BuildCost matrix
     (WorkerShoes **1000** fix; AdvancedTraining **1500** residual).
   - Host research complete path remains **1**-frame residual (not full ProductionUpdate).
   - Honesty: `honesty_upgrades_cost_time_application_wave79_ok`.
7. Tests / gates:
   - shell_smoke: minimap79/sel79/input79/draw79/train79/upg79 honesty flags wired
     (playable_claim stays false)
   - golden_skirmish_gate --frames 8 → PASS playable_claim=true
   - shell_smoke_gate → PASS playable_claim=false shell_host_playable_ok=true
     minimap79=true sel79=true input79=true draw79=true train79=true upg79=true

**Still residual (fail-closed, not claimed):**
- Full SAGE Radar/Minimap GPU atlas / live click-to-scroll camera
- Full Drawable::drawIcon health-bar bone attach GPU
- Full MessageStream / GUIEdit drag matrix
- Full C++ Drawable Xfer table beyond StealthLook residual
- Full ProductionUpdate retail BuildTime for host upgrade research path (still 1-frame)
- Full ExperienceScalarUpgrade module matrix on all USA templates
- Shell `playable_claim` remains false (no windowed W3D retail claim)
- Network residual replication (network deferred)

## Residual Host Playability — Wave 78: superweapon reload table + CarpetBomb/Artillery/Cluster/GPS/Cash residual peels (2026-07-13)

**Closed (host-testable residual peels; orthogonal to Waves 76–77):**
1. **HostSuperweaponKind reload residual table** (`special_power_strikes`):
   - Complete `reload_ms()` / `reload_frames()` for **all 10** kinds:
     Daisy **360s**, A10 **240s**, Scud **300s**, PUC **240s**, Nuke **360s**,
     Anthrax **360s**, Spectre **240s**, Carpet **150s**, Artillery **300s**,
     Cruise **120s**.
   - New baseline constants: `SCUD_STORM_RELOAD_*`, `PARTICLE_CANNON_RELOAD_*`,
     `ANTHRAX_BOMB_RELOAD_*`.
   - Ordering residual: Cruise < Carpet < A10/Spectre/PUC < Scud/Artillery < Daisy/Nuke/Anthrax.
   - Honesty: `honesty_host_superweapon_reload_table_wave78`.
2. **CarpetBomb faction-tier residual deepen** (`special_power_strikes`):
   - Per-tier ReloadTime: America/China **150000**, AirF **240000**, Nuke_ variant **180000**.
   - RadiusCursor / DeliveryDecalRadius: America **100**, China/AirF **180**.
   - OCL / science names + DeliveryDecal (SCCA10Strike_USA vs SCCCarpBomb;
     Color **(255,156,0)** vs **(255,0,0)**; Opacity **25–50%**; Throb **500**ms).
   - ViewObjectDuration **40000**ms → **1200**f / Range **250**.
   - Honesty: `honesty_carpet_bomb_science_tier_residual_pack_wave78`.
3. **Artillery science-tier residual deepen** (`special_power_strikes`):
   - SCIENCE_ArtilleryBarrage1/2/3 + SUPERWEAPON_ArtilleryBarrage1/2/3 OCL names.
   - `science_name()` / `ocl_name()` on `ArtilleryBarrageScienceTier`.
   - Science point cost **1**; prereqs China+Rank3 / chain.
   - DeliveryDecal: **SCCArtilleryBarrage_China** / Color **(255,156,0)** /
     Opacity **25–50%** / Throb **500**ms / Radius **125**.
   - VisibleNumBones **1**, VisibleItemsDroppedPerInterval **1**.
   - Honesty: `honesty_artillery_science_tier_residual_pack_wave78` +
     `honesty_special_power_residual_pack_wave78_ok`.
4. **ClusterMines residual deepen** (`host_mines`):
   - DeliveryDecal: **SCCClusterMines_China** / SHADOW_ALPHA_DECAL /
     Opacity **25–50%** / Throb **500**ms / Color **(255,156,0)**.
   - DropOffset **(0,0,-2)**, MaxAttempts **4**, Payload count **1**.
   - ViewObjectDuration **30000**ms → **900**f / Range **250**.
   - SCIENCE_ClusterMines prereq **SCIENCE_CHINA + SCIENCE_Rank3**, cost **1**.
   - Honesty: `honesty_cluster_mines_residual_pack_wave78`.
5. **GPSScrambler residual deepen** (`host_gps_scrambler`):
   - SCIENCE prereq **GLA+Rank5** vs Slth **GLA+Rank3**; point cost **1**.
   - Marker KindOf **NO_COLLIDE IMMOBILE UNATTACKABLE**; ImmortalBody MaxHealth **1**.
   - Particles: GPSMicrowaveScambler / GPSRotisserie / gpsScrambleCloud.
   - Enums SPECIAL_GPS_SCRAMBLER / SLTH_SPECIAL_GPS_SCRAMBLER.
   - Honesty: `honesty_gps_scrambler_residual_pack_wave78`.
6. **CashBounty residual deepen** (`host_cash_bounty`):
   - `CashBountyScienceTier` enum: percent **5/10/20%**, science/ability/display names,
     ModuleTag_15/16/17, highest_from_sciences.
   - DisplayName SCIENCE:GLACashBounty1/2/3 + shared Description residual.
   - Honesty: `honesty_cash_bounty_residual_pack_wave78`.
7. Tests / gates:
   - `host_superweapon_reload_table_wave78_honesty` /
     `carpet_bomb_science_tier_residual_pack_wave78_honesty` /
     `artillery_science_tier_residual_pack_wave78_honesty` /
     `cluster_mines_residual_pack_wave78_honesty` /
     `gps_scrambler_residual_pack_wave78_honesty` /
     `cash_bounty_residual_pack_wave78_honesty`
   - shell_smoke: sp78/cluster78/gps78/cash78 honesty flags wired
     (playable_claim stays false)
   - golden_skirmish_gate --frames 8 → PASS playable_claim=true
   - shell_smoke_gate → PASS playable_claim=false shell_host_playable_ok=true
     sp78=true cluster78=true gps78=true cash78=true

**Still residual (fail-closed, not claimed):**
- Full Miles positional InitiateSound / live SpecialPower SharedSyncedTimer UI
- Full AmericaJetB52 / ChinaArtilleryCannon / ClusterMinesBomb DeliverPayload Objects
- Full GrantStealth particle GPU path / StealthUpdate module matrix
- Full CashBountyPower palace science gate matrix / calcCostToBuild handicap
- Shell `playable_claim` remains false (no windowed W3D retail claim)
- Network residual replication (network deferred)

## Residual Host Playability — Wave 77: AI/weapon/FOW/ground-height/audio residual peels (2026-07-13)

**Closed (host-testable residual peels; orthogonal to Wave 76 ControlBar/script):**
1. **SpecialPower audio residual name tables** (`special_power_strikes`):
   - Retail SpecialPower.ini `InitiateSound` / `InitiateAtLocationSound` residual
     name tables on `HostSuperweaponKind`:
     - ScudStorm → **ScudStormInitiated** (source); empty at-location.
     - ArtilleryBarrage → **FireArtilleryCannonSound** (source); empty at-location.
     - CruiseMissile → **AirRaidSiren** (source + at-location).
     - NeutronMissile → empty InitiateSound (commented in retail) + **AirRaidSiren** at-location.
     - Daisy/A10/PUC/Anthrax/Spectre/Carpet → empty residual (no retail fields).
   - `activate_audio()` remains special-power template labels for host residual queues.
   - Honesty: `honesty_special_power_audio_name_table_wave77` +
     `honesty_special_power_residual_pack_wave77_ok`.
2. **FOW residual honesty pack** (`fow_rendering`):
   - CELL buckets **0/1/2** + R8 terrain overlay **0/128/255** residual.
   - Default cell size **50** world units residual.
   - Inactive fail-open + fully-visible shell/observer + from_snapshot pad residual.
   - ObjectVisibility FOW encoding residual (VISIBLE / FOGGED α**0.3** / HIDDEN).
   - Honesty: `honesty_fow_residual_pack_wave77`.
3. **Presentation ground-height residual deepen** (`presentation_frame`):
   - `RenderableObject.ground_height` / `ground_height_from_terrain` frozen at
     object XY via `sample_presentation_ground_height` (default-0 when no map).
   - Does **not** rewrite `position.y` (locomotor ground clamp residual separate).
   - Honesty: `ground_height_presentation_residual_ok`.
4. **WeaponStore delayed-damage Snapshot residual** (`weapon/weapon_store`):
   - `WeaponDelayedDamageSnapshotResidual` freezes template name + frame +
     source/victim + position for save/load bookkeeping consistency.
   - `delayed_damage_snapshot_residual()` +
     `honesty_weapon_store_delayed_damage_residual_ok`.
   - Fail-closed: not full C++ WeaponStore Xfer (templates not reloaded here).
5. **Host WeaponStore seed residual pack** (`weapon_bootstrap`):
   - Core golden/skirmish seed name table residual (**16+** names: Ranger,
     Patriot, Stinger, Humvee, tanks, Raptor, SCUD, Overlord, …).
   - Honesty: `honesty_weapon_store_host_seed_residual_wave77`.
6. **AI skirmish residual pack** (`ai_skirmish_activity`):
   - AIPlayer/AIData defaults residual: StructureSeconds **10** → **300**f,
     TeamSeconds **2** → **60**f, ResourcesPoor **2000** / Wealthy **5000**,
     structures/teams poor/wealthy mods **2.0**, RebuildDelay **5**s,
     SkirmishBaseDefenseExtraDistance **50**.
   - Honesty: `honesty_ai_skirmish_residual_pack_wave77`.
7. Tests / gates:
   - `special_power_audio_name_table_wave77_honesty` /
     `fow_residual_pack_wave77_honesty` /
     `ground_height_presentation_residual_wave77` /
     `delayed_damage_snapshot_residual_wave77_honesty` /
     `weapon_store_host_seed_residual_wave77_honesty` /
     `ai_skirmish_residual_pack_wave77_honesty`
   - shell_smoke: sp77/fow77/gh77/weapon77/ai77 honesty flags wired
     (playable_claim stays false)
   - golden_skirmish_gate --frames 8 → PASS playable_claim=true
   - shell_smoke_gate → PASS playable_claim=false shell_host_playable_ok=true
     sp77=true fow77=true gh77=true weapon77=true ai77=true

**Still residual (fail-closed, not claimed):**
- Full Miles positional InitiateSound playback / AudioEventRTS live event load
- Full SAGE multi-layer FOW dirty-rect shroud texture streaming
- Full HeightMap bilinear / bridge-aware sample + locomotor Y ground clamp
- Full C++ WeaponStore delayed-damage Xfer / dealDamageInternal rebind after load
- Full Weapon.ini parse / ClipSize in-clip volley state machine
- Full AI.ini side build list / live dozer pathfinding matrix
- Shell `playable_claim` remains false (no windowed W3D retail claim)
- Network residual replication (network deferred)

## Residual Host Playability — Wave 76: science-tier / ControlBar / font / vanish-alpha / ScriptEngine residual (2026-07-13)

**Closed (host-testable residual peels):**
1. **A10 science-tier FormationSize residual** (`special_power_strikes`):
   - SCIENCE_A10ThunderboltMissileStrike1/2/3 → FormationSize **1/2/3** jets.
   - OCL SUPERWEAPON_A10ThunderboltMissileStrike1/2/3 residual.
   - Shared DeliverPayload residual: FormationSpacing **35**, DeliveryDistance **450**,
     DropDelay **500**ms → **15**f, VisibleNumBones **6**, VisibleItemsDroppedPerInterval **2**,
     DiveStart **500** / DiveEnd **300**, StrafeLength **450**.
   - DeliveryDecal residual: **SCCA10Strike_USA** / SHADOW_ALPHA_DECAL /
     OpacityMin/Max **25%/50%** / Throb **500**ms / Color **R:255 G:156 B:0** /
     Radius **50** (matches RadiusCursor).
   - `A10StrikeScienceTier` + `highest_from_sciences` residual.
   - Honesty: `honesty_a10_science_tier_residual_pack_wave76` +
     `honesty_special_power_residual_pack_wave76_ok`.
2. **Paradrop science-tier payload residual** (`host_paradrop`):
   - SCIENCE_Paradrop1 → Rangers **5**, DropDelay **150**ms → **5**f, 1 plane.
   - SCIENCE_Paradrop2 → Rangers **10**, DropDelay **80**ms → **2**f, 1 plane.
   - SCIENCE_Paradrop3 → Rangers **20** (2×10 dual DeliverPayload), DropDelay **80**ms,
     plane count **2**.
   - DeliveryDecal residual: **SCCParadrop_USA** / Color **R:227 G:229 B:22** /
     OpacityMin/Max **25%/50%** / Throb **500**ms / Radius **50**.
   - PreOpenDistance **300** residual.
   - Honesty: `honesty_paradrop_science_tier_residual_pack_wave76` +
     `honesty_paradrop_residual_pack_wave76_ok`.
3. **ControlBar residual deepen** (`gameplay_layout`):
   - Retail window-count residual **98** (`CONTROL_BAR_RETAIL_WINDOW_COUNT`).
   - Key named-child residual table (CommandWindow / MoneyDisplay / LeftHUD /
     ButtonCommand01..14 / OCLTimerWindow / WinUnitSelected / …).
   - Font residual table peeled from ControlBar.wnd: Times New Roman **10/14**,
     Arial **8/10/14**, Generals **15/20**.
   - Structural validate requires named + font tokens; load path asserts count **98**.
   - Honesty: `honesty_control_bar_residual_pack_wave76_ok`.
4. **InGameUI Font residual table** (`floating_text_layout`):
   - Message/Superweapon/NamedTimer/DrawableCaption Arial **10** residual.
   - MilitaryCaption Courier New **12** residual.
   - Floating-text setFont Arial POINTSIZE **8** residual (DEBUG_addFloatingText).
   - Honesty: `honesty_ingame_ui_font_table_residual_ok`.
5. **DisplayString vanish-rate integer color-alpha residual** (`presentation_frame`):
   - C++ `updateFloatingText` REAL_TO_INT amount subtract on A channel residual.
   - `vanish_color_alpha_u8_at` / `color_with_vanish_alpha_at` host-testable.
   - past=5 → amount **0** (truncation); past=10 → amount **1** (255→254).
   - Honesty: `honesty_vanish_color_alpha_residual_ok` +
     `honesty_graphics_residual_pack_wave76_ok`.
6. **ScriptEngine campaign residual** (`golden_campaign`):
   - MAX_COUNTERS / MAX_FLAGS / MAX_ATTACK_PRIORITIES = **256** residual honesty
     (C++ ScriptEngine.h table caps).
   - Honesty: `honesty_script_engine_table_capacity_residual_ok` +
     `script_engine_residual_ok` on campaign result (does not gate
     `campaign_playable_claim`).
7. Tests / gates:
   - `a10_science_tier_residual_pack_wave76_honesty` /
     `paradrop_science_tier_residual_pack_wave76_honesty` /
     `control_bar_residual_pack_wave76_honesty` /
     `graphics_residual_pack_wave76_honesty`
   - shell_smoke: sp76/paradrop76/cb76/gfx76 honesty flags wired (playable_claim stays false)
   - golden_skirmish_gate --frames 8 → PASS playable_claim=true
   - shell_smoke_gate → PASS playable_claim=false shell_host_playable_ok=true
     cb_windows=98 sp76=true paradrop76=true cb76=true gfx76=true

**Still residual (fail-closed, not claimed):**
- Full AmericaJetA10Thunderbolt DeliverPayload flight Object / live missile bones
- Full dual-plane Paradrop DeliverPayload cargo plane / AmericaParachute fall physics
- Full ControlBar DrawCallback / command-button bind / windowed W3D retail UI
- Full DisplayString GPU font atlas / live vanish surface blend / StretchRect
- Full ScriptEngine ScriptAction / CALL_SUBROUTINE condition evaluator parity
- Shell `playable_claim` remains false (no windowed W3D retail claim)
- Network residual replication (network deferred)

## Residual Host Playability — Wave 75: mesh/shell residual deepen + campaign residual (2026-07-13)

**Closed (host-testable residual peels):**
1. **mesh_asset_resolve deepen** (`assets/mesh_asset_resolve`):
   - Expanded common unit model_key table (air / hero / defense / ZH host units:
     Raptor, Comanche, Chinook, Spectre, Patriot, Overlord, MiG, Helix, Scud,
     BombTruck, Jarmen, Stinger, etc.) — **50+** template→key pairs.
   - Retail archive basename residual map (`airanger_s` → `AIRanger_S`,
     `avhummer` → `AvHummer`, `avraptorag` → `AVRaptorAG`, …).
   - W3D search residual: **W3DEnglishZH** roots + mixed-case filename variants.
   - Mesh scale residual table: common ZH combat units retail **1.0**; known
     non-default CINE/weapon peels (0.66 / 0.8 / …).
   - Honesty: `honesty_mesh_asset_residual_ok` + `honesty_retail_basename_residual_ok`
     + `honesty_mesh_scale_residual_ok`.
2. **shell_smoke residual honesty (Wave 72–73, no playable_claim flip)**:
   - `mesh_asset_residual_ok` — W3D resolve residual pack.
   - `rng_residual_pack_ok` — Wave 72 host RNG residual pack.
   - `special_power_wave72_residual_ok` — Daisy/A10 special-power pack.
   - `special_power_wave73_residual_ok` — Spectre/Nuke/SupW pack.
   - `spectre_orbit_decal_presentation_ok` — Wave 73 presentation decal residual.
3. **presentation_frame mesh scale residual**:
   - `RenderableObject.mesh_scale` + `UnitRenderInput.mesh_scale` frozen from
     template residual table at presentation build.
   - Honesty: `mesh_scale_presentation_residual_ok`.
4. **golden_campaign residual**:
   - `mesh_asset_residual_ok` / `mesh_scale_presentation_ok` honesty flags on
     campaign result (does not gate `campaign_playable_claim`).
5. **host_base_defense residual deepen** (Weapon.ini / FactionBuilding.ini):
   - Patriot clip residual: ClipSize **4**, ClipReload **2000**ms, DamageRadius **5**,
     ScatterVsInfantry **10**, AutoReloadsClip **Yes**, Projectile **PatriotMissile**.
   - Patriot body: BuildCost **1000**, BuildTime **25**s, Energy **-3**, Vision/Shroud
     **360**, MaxHealth **1000**, model **ABPatriot**.
   - Stinger Site body: BuildCost **900**, BuildTime **15**s, Vision **600**,
     Shroud **400**, MaxHealth **1000**, HoleMaxHealth **500**, model **UBStingerS**.
   - Honesty: `honesty_patriot_weapon_body_residual_ok` +
     `honesty_stinger_site_body_residual_ok` (wired into residual pack).
6. Tests / gates:
   - mesh_asset lib suite: **15** ok (includes wave75 pack)
   - shell_smoke: **4** ok
   - `base_defense_body_clip_residual_honesty_wave75` / `mesh_scale_presentation_residual_wave75`
   - golden_skirmish_gate --frames 8 → PASS playable_claim=true
   - shell_smoke_gate → PASS playable_claim=false shell_host_playable_ok=true mesh=true
   - golden_campaign_gate → PASS campaign_playable_claim=true mesh_asset=true

**Still residual (fail-closed, not claimed):**
- Full ThingTemplate.scale field / draw-scale bone matrix / W3D material-animation GPU
- Full Patriot ClipSize in-clip volley state machine / live projectile Object spawn
- Full Stinger HiveStructureBody slave W3D bone GPU attach
- Shell `playable_claim` remains false (no windowed W3D retail claim)
- Network residual replication (network deferred)

## Residual Host Playability — Wave 74: multi-locale CSF + ThingFactory spawn bookkeeping (2026-07-13)

**Closed (host-testable residual peels):**
1. **Multi-locale CSF pack load residual deepen** (`game_text_residual`):
   - Beyond English pack load: residual path resolve for German/French/Spanish/Italian
     (plus English) via `load_locale_csf_pack_residual` / `PRIMARY_LOCALE_CSF_PACKS`.
   - Label-count residual honesty when a locale path is present and parses.
   - Empty-table honesty when locale pack is absent (fail-closed, not boot UI).
   - Wired into `GameTextResidualHonesty` + `exercise_host_game_text_residual`.
   - Honesty: `honesty_multi_locale_csf_pack_load` +
     `exercise_multi_locale_csf_pack_load_residual`.
2. **ThingFactory object residual spawn bookkeeping** (`special_power_strikes`):
   - `ThingFactoryObjectSpawnResidual` ledger (object name / mass / health /
     geometry / KindOf / armor / body module / spawn frame / position).
   - ScudStormMissile live residual spawn bookkeeping on impact
     (`scud_storm_missile_spawn_residual` +
     `scud_thing_factory_spawn_applications`).
   - SpectreHowitzerShell residual spawn bookkeeping
     (`spectre_howitzer_shell_spawn_residual` +
     `howitzer_shell_thing_factory_spawn_applications`).
   - TrailRemnant residual spawn bookkeeping with ImmortalBody/DeletionUpdate
     already closed (`trail_remnant_spawn_residual` +
     `remnant_thing_factory_spawn_applications`).
   - Honesty: `honesty_thing_factory_spawn_bookkeeping_wave74` + registry
     honesty methods; Snapshot/Xfer default fields appended.
3. **Anim2D MoneyPickUp image list residual bind** (`world_anim_layout`):
   - Complete `SCPDollar000`..`SCPDollar030` image name table bind residual.
   - Collection `findTemplate("MoneyPickUp")` after residual init path with full
     image list residual bind honesty.
   - Honesty: `honesty_money_pickup_image_list_bind` +
     `honesty_money_pickup_find_template_after_init`.
4. **Laser soft-edge multi-beam UV residual deepen** (`laser_segment_upload`):
   - Per-layer scroll_uv + tile_factor + UV_Offset_Rate (U=0, V=ScrollRate×elapsed).
   - Fail-closed vs wgpu `Queue::write_buffer` (ready flag bookkeeping only).
   - Honesty: `honesty_soft_edge_multi_beam_uv_residual_pack`.
5. Tests / gates (not log-only):
   - `multi_locale_csf_pack_load_residual_wave74_honesty`
   - `thing_factory_spawn_bookkeeping_wave74_honesty`
   - `money_pickup_image_list_bind_and_find_template_wave74_honesty`
   - `soft_edge_multi_beam_uv_residual_wave74_honesty`
   - game_text lib: **12** ok; world_anim: **12** ok; special_power_strikes: **103** ok;
     laser_segment: **12** ok
   - golden_skirmish_gate --frames 8 → PASS playable_claim=true
   - shell_smoke_gate → PASS playable_claim=false shell_host_playable_ok=true

**Still residual (fail-closed, not claimed):**
- Full multi-locale CSF/STR GameText table load for all LanguageId at runtime boot UI
- Full ThingFactory Object / live MissileAIUpdate / DumbProjectileBehavior physics
- Full Anim2DCollection GPU texture atlas sample / WW3D Image draw
- Actual `wgpu::Queue::write_buffer` against a live device/pipeline
- Network residual replication (network deferred)

## Residual Host Playability — Wave 73: Spectre/Nuke/SupW residual deepen + presentation decal (2026-07-13)

**Closed (host-testable residual peels):**
1. **SpectreGunship orbit residual pack deepen** (`special_power_strikes`):
   - HowitzerFiringRate **300**ms → **9**f (explicit ms residual) + HowitzerFollowLag
     **400**ms → **12**f.
   - GunshipOrbitRadius **250** vs AttackAreaRadius **200** (science tiers L1/L2/L3 all
     share AttackAreaRadius **200**; only OrbitTime **300/450/600**f scales).
   - TargetingReticleRadius **25**, StrafingIncrement **20**, OrbitInsertionSlope **0.7**,
     GattlingStrafeFX **SpectreGattlingArmsSmoke**.
   - AttackAreaDecal **SCCSpecTarg** / TargetingReticleDecal **SCCSpecRet** /
     Color **R:127 G:177 B:222 A:255** / throb **1500**/ **300**ms.
   - SuperweaponSpectreGunship Reload **240000**ms → **7200**f; AirF Reload
     **180000**ms → **5400**f; ViewObject **30000**ms → **900**f / range **250**.
   - Dual-weapon ROF residual schedule: howitzer base/**9** MEAN/**6** FAST/**4**;
     gattling base/**3** MEAN/**1** FAST/**1**.
   - Honesty: `honesty_spectre_orbit_residual_pack_wave73` +
     `SpectreGunshipScienceTier::attack_area_radius`.
2. **NuclearMissile radiation residual pack deepen** (`special_power_strikes`):
   - NukeRadiationFieldWeapon AttackRange **15** / MinimumAttackRange **10**.
   - KindOf **IMMOBILE CLEANUP_HAZARD INERT NO_COLLIDE**, Armor
     **HazardousMaterialArmor**, Geometry **CYLINDER** h**1** / IsSmall **No**.
   - HazardFieldCoreWeapon + DeathFX **FX_RadiationPoolDie**, InitialHealth **150**.
   - SuperweaponNeutronMissile Reload **360000**ms → **10800**f, RadiusCursor **210**,
     ViewObject **40000**ms → **1200**f / range **250**, InitiateAtLocationSound
     **AirRaidSiren**.
   - Honesty: `honesty_nuke_radiation_residual_pack_wave73` (extends Wave 56 pack).
3. **SupW variants residual pack** (`special_power_strikes`):
   - SupW_SuperweaponNeutronMissile Reload **240000**ms → **7200**f / RadiusCursor **210**.
   - SupW_SuperweaponParticleUplinkCannon Reload **180000**ms → **5400**f.
   - Nuke_SuperweaponNeutronMissile Reload **300000**ms → **9000**f / RadiusCursor **210**.
   - Ordering residual: SupW **240s** < Nuke_ **300s** < China standard **360s**;
     AirF Spectre **180s** < USA Spectre **240s**; SupW Cruise **120s** retained.
   - Honesty: `honesty_supw_variants_residual_pack_wave73` + combined
     `honesty_special_power_residual_pack_wave73_ok`.
4. **Presentation Spectre orbit decal residual** (`presentation_frame`):
   - Snapshot-owned AttackAreaDecal / TargetingReticleDecal residual
     (`PresentationSpectreOrbitDecal::RETAIL`) for dual-tick consumers without live
     SpectreGunshipUpdate re-read.
   - OpacityMin/Max AttackArea **25%/50%**, Reticle **50%/100%**, Style
     **SHADOW_ALPHA_DECAL**, OnlyVisibleToOwningPlayer **Yes**.
   - Honesty: `honesty_spectre_orbit_decal_presentation_ok` +
     `PresentationFrame::spectre_orbit_decal_presentation_residual_ok`.
5. Tests / gates (not log-only):
   - `spectre_orbit_residual_pack_wave73_honesty` /
     `nuke_radiation_residual_pack_wave73_honesty` /
     `supw_variants_residual_pack_wave73_honesty` /
     `spectre_orbit_decal_presentation_residual_wave73`
   - special_power_strikes lib suite: **102** ok
   - golden_skirmish_gate --frames 8 → PASS playable_claim=true
   - shell_smoke_gate → PASS playable_claim=false shell_host_playable_ok=true

**Still residual (fail-closed, not claimed):**
- Full SpectreGunshipUpdate OCL aircraft / live gattling strafe bone path
- Full HazardousMaterialArmor cleanup-hazard stack / live radiation object
- Full SupW ThingFactory Object / general faction select
- Full SHADOW_ALPHA_DECAL GPU throb submit
- Network residual replication (network deferred)

## Residual Host Playability — Wave 72: remaining host residual packs + special_power Daisy/A10 deepen (2026-07-13)

**Closed (host-testable residual peels):**
1. **RNG residual pack** (`host_rng_residual`) — only host missing `honesty_*residual_pack_ok`:
   - MultFactor **1/(2^32-1)** + default six-word ADC seed table residual.
   - Pure index-seeded re-query stability + structure scatter scale **0.3**.
   - Live logic/client/audio stream exercise + ADC parity residual.
   - Honesty: `honesty_rng_residual_pack_ok`.
2. **Mines residual pack gate** (`host_mines`):
   - Existing Wave 51 pack (`honesty_mines_residual_pack_ok`) + wave72 honesty test.
3. **EMP Pulse residual pack gate** (`host_emp_pulse`):
   - Existing Wave 51 pack (`honesty_emp_pulse_residual_pack_ok`) + wave72 honesty test.
4. **Upgrades residual pack gate** (`host_upgrades`):
   - Existing Wave 62 pack (`honesty_upgrades_residual_pack_ok`) + wave72 honesty test.
5. **Unit Training residual pack gate** (`host_unit_training`):
   - Existing Wave 62 pack (`honesty_unit_training_residual_pack_ok`) + wave72 honesty test.
6. **Strategy Center residual pack gate** (`host_strategy_center`):
   - Existing Wave 62 pack (`honesty_strategy_center_residual_pack_ok`) + wave72 honesty test.
7. **Sneak Attack residual pack gate** (`host_sneak_attack`):
   - Existing Wave 62 pack (`honesty_sneak_attack_residual_pack_ok`) + wave72 honesty test.
8. **Special Power residual deepen** (`special_power_strikes`):
   - **DaisyCutter / MOAB** residual pack: ReloadTime **360000**ms → **10800**f,
     RadiusCursor **170**, science SCIENCE_DaisyCutter, ViewObject **30000**/range **250**,
     DaisyCutterDetonationWeapon **2000**/r**100**, flame secondary **5**/r**100**.
   - **A10 Thunderbolt** residual pack: ReloadTime **240000**ms → **7200**f,
     RadiusCursor **50**, science SCIENCE_A10ThunderboltMissileStrike1,
     host aggregate **500**/r**100**, missile **200**/r**50**, ClipReload **20000**ms → **600**f,
     Vulcan **10**/r**4**/delay **60**ms.
   - Combined: `honesty_special_power_residual_pack_ok` (carpet/cruise/artillery/nuke/anthrax
     + DaisyCutter + A10).
   - HostSuperweaponKind Daisy/A10 delay/damage/radius wired to pack constants.
9. Tests / gates (not log-only):
   - `rng_residual_pack_honesty_wave72` / mines / emp_pulse / upgrades /
     unit_training / strategy_center / sneak_attack / special_power
   - golden_skirmish_gate --frames 8 → PASS playable_claim=true
   - shell_smoke_gate → PASS playable_claim=false shell_host_playable_ok=true

## Residual Host Playability — Wave 71: heal/repair/frenzy residual packs + flashbang fix (2026-07-13)

**Closed (host-testable residual peels):**
1. **Heal residual pack** (`host_heal`):
   - Ambulance AutoHeal residual: infantry **4**/s, vehicle **5**/s, radius **100**,
     delay **1000**ms; TransportContain slots **3**, HealthRegen **25%**/s,
     DamagePercentToUnits **10%**.
   - Honesty: `honesty_heal_residual_pack_ok` (wraps ambulance auto-heal constants).
2. **Repair residual pack** (`host_repair`):
   - Dozer RepairHealthPercentPerSecond **2%**; RepairDock TimeForFullHeal **5000**ms → **150**f;
     NumberApproachPositions **5**; TechRepairPad; BoredTime **5000**ms / range **150**.
   - Honesty: `honesty_repair_residual_pack_ok`.
3. **Emergency Repair residual pack** (`host_emergency_repair`):
   - Level heals **100/200/300**, radius **100**, ReloadTime **240000**ms → **7200**f,
     science SCIENCE_EmergencyRepair1/2/3, RepairCloud, KindOf VEHICLE.
   - Honesty: `honesty_emergency_repair_residual_pack_ok`.
4. **Propaganda residual pack** (`host_propaganda`):
   - Radius **150**, Delay **2000**ms → **60**f, heal **2%**/upgraded **4%**,
     ENTHUSIASTIC/SUBLIMINAL discriminants **8/15**, ROF **125%**.
   - Honesty: `honesty_propaganda_residual_pack_ok`.
5. **Frenzy residual pack** (`host_frenzy`):
   - Damage mult **110/120/130%**, duration **300/600/900**f, radius **200**,
     science SCIENCE_Frenzy1/2/3, FrenzyCloud, CAN_ATTACK / !STRUCTURE.
   - Honesty: `honesty_frenzy_residual_pack_ok`.
6. **Spy Satellite residual pack** (`host_spy_satellite`):
   - DynamicShroud grow/shrink residual: vision **300**, duration **13000**ms → **390**f,
     grow **1000**ms → **30**f, shrink delay **10000**ms → **300**f / **5000**ms → **150**f,
     stealth detect **500**ms → **15**f.
   - Honesty: `honesty_spy_satellite_residual_pack_ok`.
7. **Radar Scan residual pack** (`host_radar_scan`):
   - RadarVanPing residual: vision **150**, duration **10000**ms → **300**f,
     shrink delay **7500**ms → **225**f / **2500**ms → **75**f, stealth **500**ms → **15**f.
   - Honesty: `honesty_radar_scan_residual_pack_ok`.
8. **Sentry Drone residual pack** (`host_sentry_drone`):
   - Detector range **225** / rate **900**ms → **27**f; gun **8**/range **150**/delay **6**f;
     pack/unpack **30**f; stealth delay **60**f.
   - Honesty: `honesty_sentry_drone_residual_pack_ok`.
9. **Point Defense residual pack** (`host_point_defense`):
   - Paladin/Avenger/KingRaptor/Chinook PDL ScanRate/ScanRange/VelocityPredict residual.
   - Honesty: `honesty_point_defense_residual_pack_ok`.
10. **Overlord Addons residual pack** (`host_overlord_addons`):
    - Addon slot table + ConflictsWith exclusivity; contain slots **1**; Helix transport **5**.
    - Honesty: `honesty_overlord_addons_residual_pack_ok`.
11. **Comanche Rocket Pods residual pack** (`host_comanche_rocket_pods`):
    - ClipSize **20**, ScatterTargetScalar **50**, reload **900**f, dual-radius splash.
    - Honesty: `honesty_comanche_rocket_pods_residual_pack_ok`.
12. **Deliver Payload residual pack** (`host_deliver_payload`):
    - Supply drop OCL residual (payload **6**, drop delay **11**f, door **15**f) +
      DropVariance + VisiblePayload A10 + crate geometry pack.
    - Honesty: `honesty_deliver_payload_residual_pack_ok`.
13. **Base Defense residual pack** (`host_base_defense`):
    - Patriot ground/air/assist residual + laser punch-through **1.3** / arc segments **20**.
    - Honesty: `honesty_base_defense_residual_pack_ok`.
14. **FlashBang upgrade fix** (`game_logic` create_object):
    - Ranger secondary is PLAYER_UPGRADE only (parity with neutron shells / rocket pods):
      residual map may name flashbang, but create strips it unless research unlocked or
      template explicitly seeds `secondary_weapon_name`. Research still equips via
      `apply_flashbang_unlock_to_team`. Fixes
      `flashbang_upgrade_queue_complete_equips_ranger_secondary`.
15. Tests / gates (not log-only):
   - `heal_residual_pack_honesty_wave71` / repair / emergency_repair / propaganda /
     frenzy / spy_satellite / radar_scan / sentry_drone / point_defense /
     overlord_addons / comanche_rocket_pods / deliver_payload / base_defense
   - `flashbang_upgrade_queue_complete_equips_ranger_secondary` + flashbang suite
   - golden_skirmish_gate --frames 8 → PASS playable_claim=true
   - shell_smoke_gate → PASS playable_claim=false shell_host_playable_ok=true

## Residual Host Playability — Wave 70: helix-napalm/inferno/leaflet/minigunner residual packs (2026-07-13)

**Closed (host-testable residual peels):**
1. **Helix Napalm residual pack** (`host_helix_napalm`):
   - Weapon residual: NapalmBombWeapon Primary **75**/r**5** + Secondary **40**/r**30**,
     DamageType **EXPLOSION**, DeathType **EXPLODED**, FireOCL **OCL_FirestormSmall**.
   - Ability residual: ReloadTime **10000**ms → **300**f, RadiusCursor **100**,
     StartAbilityRange **3**, MaxSpecialObjects **1**.
   - Firestorm residual: Damage **100** / Black **150**, tick **500**ms → **15**f,
     lifetime **6000**ms → **180**f, FinalMajorRadius **90**.
   - Upgrade residual: Upgrade_HelixNapalmBomb BuildCost **800**, BuildTime **20**s → **600**f.
   - Honesty: `honesty_helix_napalm_residual_pack_ok` + layer honesty tests.
2. **Inferno Cannon residual pack** (`host_inferno_cannon`):
   - Weapon residual: InfernoCannonGun Primary **30**/r**15**, range **300**/min **50**,
     Delay **4000**ms → **120**f, ScatterVsInfantry **30**, DamageType **EXPLOSION**,
     Projectile **InfernoTankShell**, FireFX **WeaponFX_GenericTankGunNoTracer**.
   - Fire field residual: SmallFireFieldWeapon **5**/r**30** / tick **250**ms → **8**f /
     lifetime **2500**ms → **75**f; upgraded **7.5** damage; DamageType **FLAME**.
   - Body residual: MaxHealth **120**, BuildCost **900**, BuildTime **15**s → **450**f,
     Vision **180**/Shroud **300**, slots **3**, Geometry BOX **15**/**10**/**15**,
     Speed **30**/Damaged **20**.
   - Honesty: `honesty_inferno_cannon_residual_pack_ok` + layer honesty tests.
3. **Leaflet Drop residual pack** (`host_leaflet_drop`):
   - Special power residual: SuperweaponLeafletDrop ReloadTime **300000**ms → **9000**f,
     RadiusCursor **110**, ViewObjectDuration **30000**ms → **900**f / Range **250**,
     RequiredScience **SCIENCE_LeafletDrop**, SharedSyncedTimer **Yes**.
   - Container residual: Delay **2500**ms → **75**f, DisabledDuration **20000**ms → **600**f,
     AffectRadius **110**, MaxHealth **100**, Geometry radius **30**,
     LeafletFX **LeafletParticles1**.
   - Honesty: `honesty_leaflet_drop_residual_pack_ok` + layer honesty tests.
4. **MiniGunner residual pack** (`host_minigunner`):
   - Weapon residual: Infa_MiniGunnerGun dmg **10**/range **125**, DamageType **Gattling**,
     Delay **500**ms → **15**f, ContinuousFireOne **6**/Two **12**/Coast **1000**ms → **30**f,
     ChainGuns DAMAGE **125%**; AA gun range **350** DamageType **SMALL_ARMS**.
   - Body residual: MaxHealth **120**, Vision **100**/Shroud **200**, BuildCost **350**,
     BuildTime **10**s → **300**f, slots **1**, Geometry CYLINDER **10**/**12**,
     Speed **25**/Damaged **15**, Horde Radius **30**/Count **5**.
   - Honesty: `honesty_minigunner_residual_pack_ok` + layer honesty tests.
5. **Nuclear Tanks residual pack** (`host_nuclear_tanks`):
   - Death weapon residual: NuclearTankDeathWeapon Primary **25**/r**25** + Secondary
     **10**/r**75**, DamageType **EXPLOSION**, FireOCL **OCL_RadiationFieldSmall**;
     Nuke_ general **110**/r**80** + **70**/r**100**.
   - Radiation residual: SmallRadiationFieldWeapon **5**/r**15**, tick **750**ms → **23**f,
     lifetime **2500**ms → **75**f, DamageType **RADIATION**.
   - Speed residual: Battlemaster **25 → 35** / Damaged **32**; Overlord **20 → 30**.
   - Upgrade residual: BuildCost **2000**, BuildTime **60**s → **1800**f.
   - Honesty: `honesty_nuclear_tanks_residual_pack_ok` + layer honesty tests.
6. **Paradrop residual pack** (`host_paradrop`):
   - Special power residual: SuperweaponParadropAmerica ReloadTime **240000**ms → **7200**f,
     RadiusCursor **50**, RequiredScience **SCIENCE_Paradrop1**, SharedSyncedTimer **Yes**.
   - Payload residual: SUPERWEAPON_Paradrop1 → AmericaInfantryRanger × **5**,
     DropDelay **150**ms → **5**f, DropSpacing **30**, approach residual **90**f,
     PutInContainer **AmericaParachute**, Transport **AmericaJetCargoPlane**.
   - Honesty: `honesty_paradrop_residual_pack_ok` + layer honesty tests.
7. **Saboteur residual pack** (`host_saboteur`):
   - Effect residual: Power/Military SabotageDuration **30000**ms → **900**f,
     Internet **15000**ms → **450**f, StealCashAmount **1000**.
   - Body residual: MaxHealth **120**, Vision **150**/Shroud **300**, BuildCost **800**,
     BuildTime **15**s → **450**f, slots **1**, Geometry CYLINDER **10**/**12**,
     Speed **30**/Damaged **20**, IsTrainable **No**, StealthDelay **2500**ms → **75**f.
   - Honesty: `honesty_saboteur_residual_pack_ok` + layer honesty tests.
8. **SCUD Launcher residual pack** (`host_scud_launcher`):
   - Weapon residual: Explosive **300**/r**50** + **50**/r**100**; Toxin **200**/r**30** +
     **25**/r**60**; range **350**/min **200**; Clip **1**/reload **10000**ms → **300**f;
     PreAttack **500**ms → **15**f; Projectile **SCUDMissile**; DamageType **EXPLOSION**.
   - Poison residual: MediumPoisonFieldWeapon **2**/r**80**, tick **500**ms → **15**f,
     lifetime **30000**ms → **900**f; upgraded **2.5**; DamageType **POISON**.
   - Body residual: MaxHealth **180**, BuildCost **1200**, BuildTime **15**s → **450**f,
     Vision **180**/Shroud **300**, slots **3**, Geometry BOX **14**/**7**/**11.5**,
     Speed **20**/Damaged **15**.
   - Honesty: `honesty_scud_launcher_residual_pack_ok` + layer honesty tests.
9. Tests / gates (not log-only):
   - `helix_napalm_residual_pack_honesty_wave70` / weapon / ability / firestorm / upgrade
   - `inferno_cannon_residual_pack_honesty_wave70` / weapon / fire_field / body
   - `leaflet_drop_residual_pack_honesty_wave70` / special_power / container
   - `minigunner_residual_pack_honesty_wave70` / weapon / body
   - `nuclear_tanks_residual_pack_honesty_wave70` / death_weapon / radiation / upgrade_speed
   - `paradrop_residual_pack_honesty_wave70` / special_power / payload
   - `saboteur_residual_pack_honesty_wave70` / effect / body
   - `scud_launcher_residual_pack_honesty_wave70` / weapon / poison / body
   - golden_skirmish_gate --frames 8 → PASS playable_claim=true
   - shell_smoke_gate → PASS playable_claim=false shell_host_playable_ok=true

## Residual Host Playability — Wave 69: ambush/mob/bomb-truck/gattling residual packs (2026-07-13)

**Closed (host-testable residual peels):**
1. **Ambush residual pack** (`host_ambush`):
   - Special-power residual: SuperweaponRebelAmbush ReloadTime **240000**ms → **7200**f,
     RequiredScience **SCIENCE_RebelAmbush1**, RadiusCursor **50**, SharedSyncedTimer **Yes**,
     PublicTimer **No**, ShortcutPower **Yes**, Enum **SPECIAL_AMBUSH**.
   - OCL spawn residual: SUPERWEAPON_RebelAmbush1 Count **4** (Ambush2 **8** / Ambush3 **16**),
     FadeTime **3000**ms → **90**f, FadeIn **Yes**, DiesOnBadLand **Yes**,
     formation MinA **20** / MinB **30** / Max **400**, template GLAInfantryRebel.
   - Honesty: `honesty_ambush_residual_pack_ok` + special_power / spawn_ocl tests.
2. **Angry Mob residual pack** (`host_angry_mob`):
   - Weapon residual: pistol **10**/r**100**/Delay **250**ms → **8**f / Clip **8**/reload **3000**ms,
     DamageType MOLOTOV_COCKTAIL; aggregate tick **4**/member; ArmTheMob × **1.25**.
   - Spawn residual: InitialBurst **5**, SpawnNumber **10**, SpawnReplaceDelay **30000**ms → **900**f.
   - Body residual: MaxHealth **99999**, BuildCost **800**, BuildTime **15**s → **450**f,
     Vision **150**/Shroud **0**, slots **0**, locomotor **18**.
   - Upgrade residual: ArmTheMob cost **1000** / **30**s → **900**f.
   - Honesty: `honesty_angry_mob_residual_pack_ok` + weapon/body/upgrade tests.
3. **Bomb Truck detonate residual pack** (`host_bomb_truck_detonate`):
   - Weapon residual: Default Primary **1000**/r**40** + Secondary **100**/r**65**;
     HE **2000**/r**50** + **200**/r**85**; DamageType EXPLOSION / DeathType EXPLODED.
   - Poison residual: MediumPoisonField **2** (Anthrax **2.5**) / r**80** / tick **500**ms → **15**f /
     lifetime **30000**ms → **900**f.
   - Upgrade residual: HE/Bio object upgrades cost **500** / **5**s → **150**f.
   - Body residual: MaxHealth **220**, BuildCost **1200**, BuildTime **15**s → **450**f,
     Vision **150**/Shroud **200**, slots **3**.
   - Honesty: `honesty_bomb_truck_detonate_residual_pack_ok` + weapon/poison/upgrade/body tests.
4. **Combat Cycle residual pack** (`host_combat_cycle`):
   - Weapon residual: Rebel MG **8**/r**150**/100ms→**3**f/clip **6**; RPG **40**/r**175**/min **5**/
     1000ms→**30**f; Kell sniper **180**/r**225**/750ms→**23**f; SuicideBikeBomb **700**/r**20** +
     **100**/r**50**.
   - Body residual: MaxHealth **100**, BuildCost **500**, BuildTime **4**s → **120**f,
     Vision **180**/Shroud **300**, slots **1**, Speed **120**/Damaged **90**,
     InitialPayload GLAInfantryRebel.
   - Honesty: `honesty_combat_cycle_residual_pack_ok` + weapon/body tests.
5. **Demo Suicide Bomb residual pack** (`host_demo_suicide_bomb`):
   - DestroyedWeapon residual: Primary **50**/r**60** + Secondary **10**/r**70**, DeathType NORMAL.
   - PlusFire residual: Primary **500**/r**18** + Secondary **300**/r**50**, AttackRange **5**,
     DeathType SUICIDED, FireFX PlusFire, FireSound CarBomberDie.
   - Upgrade residual: Demo_Upgrade_SuicideBomb cost **2000** / **30**s → **900**f,
     Demo_Command_TertiarySuicide gate.
   - Honesty: `honesty_demo_suicide_bomb_residual_pack_ok` + destroyed/plus_fire/upgrade tests.
6. **FireWall residual pack** (`host_firewall`):
   - Weapon residual: FireWallSegmentWeapon **4**/r**10**, Delay **250**ms → **8**f,
     DamageType FLAME / DeathType BURNED, AttackRange **15**.
   - Ability residual: DeletionUpdate **4000**ms → **120**f, DragonTankFireWallWeapon
     AttackRange **25**/Primary **10**, OCL_FireWallSegment, segment HP **50**,
     spacing **12**/start offset **20**/max length **120**.
   - Honesty: `honesty_firewall_residual_pack_ok` + weapon/ability tests.
7. **Gattling Tank residual pack** (`host_gattling_tank`):
   - Weapon residual: Ground **15**/r**150** DamageType Gattling; Air **12**/r**350**
     DamageType SMALL_ARMS; Delay **400**ms → **12**f; FireFX GattlingTankMachineGunFire.
   - Continuous-fire residual: ContinuousFireOne **2**/Two **6**/Coast **1000**ms → **30**f;
     ROF **200%**→**6**f / **300%**→**4**f; ChainGuns × **1.25**.
   - Body residual: MaxHealth **300**, BuildCost **800**, BuildTime **10**s → **300**f,
     Vision **150**/Shroud **360**, slots **3**, Speed **40**.
   - Honesty: `honesty_gattling_tank_residual_pack_ok` + weapon/continuous_fire/body tests.
8. **Hacker income residual pack** (`host_hacker_income`):
   - Cash residual: Regular/Veteran/Elite/Heroic **5/6/8/10**, field **2000**ms → **60**f,
     IC fast **1800**ms → **54**f, XpPerCashUpdate **1**, unpack **7300**/pack **5133**.
   - Floating text residual: GUI:AddCash, Z **20**, green (0,255,0,255), IC scatter **0.3**.
   - Body residual: MaxHealth **100**, BuildCost **625**, BuildTime **20**s → **600**f,
     Vision **150**/Shroud **300**, slots **1**.
   - Honesty: `honesty_hacker_income_residual_pack_ok` + cash/floating_text/body tests.
9. Tests / gates (not log-only):
   - `ambush_residual_pack_honesty_wave69` / special_power / spawn_ocl
   - `angry_mob_residual_pack_honesty_wave69` / weapon / body / upgrade
   - `bomb_truck_detonate_residual_pack_honesty_wave69` / weapon / poison / upgrade / body
   - `combat_cycle_residual_pack_honesty_wave69` / weapon / body
   - `demo_suicide_bomb_residual_pack_honesty_wave69` / destroyed / plus_fire / upgrade
   - `firewall_residual_pack_honesty_wave69` / weapon / ability
   - `gattling_tank_residual_pack_honesty_wave69` / weapon / continuous_fire / body
   - `hacker_income_residual_pack_honesty_wave69` / cash / floating_text / body
   - golden_skirmish_gate --frames 8 → PASS playable_claim=true
   - shell_smoke_gate → PASS playable_claim=false shell_host_playable_ok=true

## Residual Host Playability — Wave 68: black-market/booby/listening + graphics residual (2026-07-13)

**Closed (host-testable residual peels):**
1. **Black Market residual pack** (`host_black_market`):
   - AutoDeposit residual: DepositAmount **20**, DepositTiming **2000**ms → **60**f,
     InitialCaptureBonus **0**.
   - Body residual: MaxHealth **1000**, BuildCost **2500**, BuildTime **30**s → **900**f,
     Vision/Shroud **200**, EnergyProduction **0**, Prerequisite GLAPalace,
     KindOf FS_BLACK_MARKET, CommandSet GLABlackMarketCommandSet.
   - Geometry residual BOX **35**/ **35**/ **35**; Hole GLAHoleBlackMarket **500** hp;
     FortifiedStructure armor upgrade residual.
   - Flammable residual: AflameDuration **5000**ms → **150**f, Damage **5** /
     Delay **500**ms → **15**f; death FX FX_StructureSmallDeath.
   - Floating text residual: GUI:AddCash, Z lift **10**, alpha **230**.
   - Honesty: `honesty_black_market_residual_pack_ok` + layer honesty tests.
2. **Booby Trap residual pack** (`host_booby_trap`):
   - Weapon residual: Primary **200**/r**5**, Secondary **50**/r**15**,
     DamageType EXPLOSION, DeathType EXPLODED; GeometryBasedDamageWeapon/FX.
   - Ability residual: StartAbilityRange **5**, ReloadTime **7500**ms → **225**f,
     MaxSpecialObjects **100**, SpecialObjectsPersistent **Yes**, Enum SPECIAL_BOOBY_TRAP.
   - Upgrade residual: BuildCost **1000**, BuildTime **30**s → **900**f,
     ResearchSound RebelVoiceUpgradeBoobyTrap.
   - Object residual: Vision/Shroud **25**, MaxHealth **1**, KindOf BOOBY_TRAP
     NO_COLLIDE MINE, Geometry CYLINDER **8**/ **8**, StealthDelay **0**,
     InnateStealth **Yes**, Physics Mass **5**.
   - Honesty: `honesty_booby_trap_residual_pack_ok` + layer honesty tests.
3. **Listening Outpost residual pack** (`host_listening_outpost`):
   - Detector residual: DetectionRate **900**ms → **27**f, DetectionRange **300**,
     IRPing / IRPingLoud / IRLenzflare.
   - Transport residual: Slots **2**, ExitDelay **250**ms → **8**f, NumberOfExitPaths **3**,
     HealthRegen **10%**/s, DamagePercentToUnits **10%**, InitialPayload TankHunter × **2**,
     PassengersAllowedToFire **Yes**, ArmedRidersUpgradeMyWeaponSet **Yes**.
   - Stealth residual: StealthDelay **2000**ms → **60**f, Forbidden MOVING RIDERS_ATTACKING,
     InnateStealth **Yes**, FriendlyOpacityMin **50%**.
   - Body residual: MaxHealth **240**, BuildCost **800**, BuildTime **15**s → **450**f,
     Vision **250**, Shroud **500**, Locomotor Speed **40**/Damaged **30**,
     Geometry BOX **20**/ **10**/ **22**, Prerequisite ChinaWarFactory.
   - Dummy weapon residual: ListeningOutpostUpgradedDummyWeapon dmg **0.1** / range **90** /
     Delay **1000**ms → **30**f.
   - Honesty: `honesty_listening_outpost_residual_pack_ok` + layer honesty tests.
4. **Graphics residual deepen**:
   - DisplayStringManager free-resource residual (`floating_text_layout`): cleanup **60**f,
     batch **10**/update, checkpoint walk, freeDisplayString checkpoint clear;
     MAX_GROUPS **10**. Honesty: `honesty_display_string_manager_free_pool`.
   - Anim2DCollection MoneyPickUp images list residual (`world_anim_layout`):
     Animation2D.ini template count **14**, MoneyPickUp Image list SCPDollar000..030
     (**31** names) stored on template; NumberImages matches list length.
     Honesty: `honesty_anim2d_ini_template_count_and_money_pickup_images`.
   - Laser MaxIntensityLifetime residual (`laser_segment_upload`): omitted → **0** frames;
     commented sample **2000**ms → **60**f; Fade sample **250**ms → **8**f.
     Honesty: `honesty_laser_max_intensity_duration_residual`.
   - GameText group numeral residual (`game_text_residual`): `NUMBER:0`..`NUMBER:9`,
     `LABEL:FORMATION`, MAX_GROUPS **10**. Honesty: `honesty_game_text_group_numeral_keys`.
5. Tests / gates (not log-only):
   - `black_market_residual_pack_honesty` / deposit / body / geometry_hole / floating_text / flammable_death
   - `booby_trap_residual_pack_honesty` / weapon / ability / upgrade / object
   - `listening_outpost_residual_pack_honesty` / detector / transport / stealth / body / dummy_weapon
   - `display_string_manager_free_pool_residual_honesty`
   - `anim2d_ini_template_count_and_money_pickup_images_residual`
   - `laser_max_intensity_duration_residual_honesty`
   - `game_text_group_numeral_keys_residual_honesty`
   - golden_skirmish_gate --frames 8 → PASS playable_claim=true
   - shell_smoke_gate → PASS playable_claim=false shell_host_playable_ok=true

## Residual Host Playability — Wave 67: usa-tanks/raptor/mig/nuke/battlemaster residual packs (2026-07-13)

**Closed (host-testable residual peels):**
1. **USA tanks residual pack** (`host_usa_tanks`):
   - Weapon residual: PrimaryDamage **60**/radius **5**/range **150**, ScatterVsInfantry **10**,
     DamageType **ARMOR_PIERCING**, Delay **2000**ms → **60**f, Projectile **GenericTankShell**,
     FireFX **WeaponFX_GenericTankGunNoTracer**, Crusader speed **400** / Paladin **300**.
   - Body residual: Crusader HP **480**/cost **900**/10s→**300**f; Paladin HP **500**/cost **1100**/
     12s→**360**f; Vision **150**/Shroud **300**, slots **3**, Turret **180**, Speed **30**/25.
   - Composite Armor residual: AddMaxHealth **100** + ADD_CURRENT_HEALTH_TOO.
   - Honesty: `honesty_usa_tanks_residual_pack_ok` + weapon/body/composite tests.
2. **Raptor residual pack** (`host_raptor`):
   - Weapon residual: DamageType **JET_MISSILES**, DeathType **EXPLODED**, Scatter **10**,
     Projectile **RaptorJetMissile**, DetonationFX **WeaponFX_JetMissileDetonation**,
     AutoReloadsClip **RETURN_TO_BASE**, AntiAir **Yes**/Infantry **No**, Clip **4**/reload **240**f;
     King Clip **6**/reload **60**f.
   - Body residual: MaxHealth **160**, Vision **200**, Shroud **400**, BuildCost **1400**,
     BuildTime **20**s → **600**f, Locomotor Speed **175**/Damaged **120**/Min **60**.
   - Honesty: `honesty_raptor_residual_pack_ok` + weapon/body tests.
3. **MiG residual pack** (`host_mig`):
   - Weapon residual: DamageType **JET_MISSILES**/Death **BURNED**, Projectile **NapalmMissile**,
     FireFX **WeaponFX_NapalmMissile**, Clip **2**/reload **8000**ms→**240**f; BlackNapalm
     secondary **50**/reload **2000**ms→**60**f.
   - Body residual: MaxHealth **160**, Vision **200**, Shroud **300**, BuildCost **1200**,
     BuildTime **10**s → **300**f, Locomotor Speed **160**/Min **60**.
   - Aircraft Armor residual: Upgrade_ChinaAircraftArmor AddMaxHealth **40**.
   - Honesty: `honesty_mig_residual_pack_ok` + weapon/body tests.
4. **Nuke Cannon residual pack** (`host_nuke_cannon`):
   - Weapon residual: DamageType **EXPLOSION**, Scatter **30**, WeaponSpeed **200**,
     Projectile **NukeCannonShell**, FireFX **WeaponFX_NukeCannonMuzzleFlash**,
     DetonationOCL **OCL_RadiationFieldMedium**, Delay **10000**ms → **300**f.
   - Radiation residual: tick **750**ms → **23**f, lifetime **30000**ms → **900**f.
   - Body residual: MaxHealth **240**, Vision **180**, Shroud **350**, BuildCost **1600**,
     BuildTime **20**s → **600**f, slots **10**, Turret **80**, Speed **20**/18.
   - Honesty: `honesty_nuke_cannon_residual_pack_ok` + weapon/radiation/body tests.
5. **Battlemaster residual pack** (`host_battlemaster`):
   - Weapon residual: DamageType **ARMOR_PIERCING**, Scatter **10**, Projectile
     **BattleMasterTankShell**, FireFX **WeaponFX_GenericTankGunNoTracerSmall**,
     Uranium DAMAGE **125%**.
   - Horde residual: ExactMatch **Yes**, Radius **75**, Count **5**, Update **1000**ms→**30**f;
     ROF stack floor delays **40**/**32**.
   - Body residual: MaxHealth **400**, Vision **150**, Shroud **300**, BuildCost **800**,
     BuildTime **10**s → **300**f, Turret **120**, Speed **25**/Nuclear **35**.
   - Honesty: `honesty_battlemaster_residual_pack_ok` + weapon/horde/body tests.
6. **Red Guard residual pack** (`host_red_guard`):
   - Weapon residual: DamageType **SMALL_ARMS**, radius **0**, FireFX
     **WeaponFX_GenericMachineGunFire**; Bayonet MELEE **10000**/range **2**/1900ms→**57**f/
     PreAttack **1400**ms→**42**f.
   - Horde residual: ExactMatch **No**, KindOf **INFANTRY**, Radius **30**, Count **5**;
     ROF stack delays **20**/**16**.
   - Body residual: MaxHealth **120**, Vision **100**, Shroud **200**, BuildCost **300**,
     BuildTime **10**s → **300**f, Speed **25**/15; Capture StartRange **5**/Unpack **90**f.
   - Honesty: `honesty_red_guard_residual_pack_ok` + weapon/horde/body tests.
7. **Tank Hunter residual pack** (`host_tank_hunter`):
   - Weapon residual: DamageType **INFANTRY_MISSILE**, Scatter **10**, Projectile
     **TankHunterMissile**, FireFX **FX_BuggyMissileIgnition**, DetonationFX
     **WeaponFX_RocketBuggyMissileDetonation**, AutoReloadsClip **Yes**.
   - TNT residual: 500/10 + 150/50, Reload **7500**ms→**225**f, Lifetime **10000**ms→**300**f,
     FireSound **BombTruckDefaultBombDetonation**.
   - Body residual: MaxHealth **100**, Vision **150**, Shroud **400**, BuildCost **300**,
     BuildTime **5**s → **150**f, Speed **20**/10, Geometry r**10**/h**12**.
   - Honesty: `honesty_tank_hunter_residual_pack_ok` + weapon/tnt/body tests.
8. Tests / gates (not log-only):
   - `usa_tanks_residual_pack_honesty_wave67` / raptor / mig / nuke_cannon /
     battlemaster / red_guard / tank_hunter residual pack honesty
   - golden_skirmish_gate --frames 8 → PASS playable_claim=true
   - shell_smoke_gate → PASS playable_claim=false shell_host_playable_ok=true

## Residual Host Playability — Wave 66: marauder/ranger/avenger/bunker/cia residual packs (2026-07-13)

**Closed (host-testable residual peels):**
1. **Marauder residual pack** (`host_marauder`):
   - Weapon residual: dmg **60**/radius **5**/range **170**, DamageType ARMOR_PIERCING,
     ScatterVsInfantry **10**, Delay **2000**/**1500**/**750**ms → **60**/**45**/**23**f,
     tier2 ClipSize **2** / ClipReload **100**ms → **3**f, speeds **300**/**400**/**500**,
     FireFX WeaponFX_GenericTankGunNoTracer, Projectile MarauderTankShell.
   - Body residual: MaxHealth **500**, Vision **125**, Shroud **300**, BuildCost **800**,
     BuildTime **10**s → **300**f, TransportSlotCount **3**, SCIENCE_MarauderTank,
     Locomotor Speed **40**/Damaged **30**.
   - Honesty: `honesty_marauder_residual_pack_ok` + layer honesty tests.
2. **Ranger residual pack** (`host_ranger`):
   - Rifle residual: dmg **5**/radius **0**/range **100**, Delay **100**ms → **3**f,
     ClipSize **3**, ClipReload **700**ms → **21**f, DamageType SMALL_ARMS.
   - Flashbang residual: Primary **35**/r**10** + Secondary **10**/r**40**, range **175**,
     min **20**, ClipReload **2000**ms → **60**f, DamageType SURRENDER, Scatter **4**,
     AllowAttackGarrisonedBldgs **Yes**.
   - Body residual: MaxHealth **180**, Vision **100**, Shroud **400**, BuildCost **225**,
     BuildTime **5**s → **150**f, TransportSlotCount **1**, BasicHuman Speed **20**/Damaged **10**.
   - Honesty: `honesty_ranger_residual_pack_ok` + layer honesty tests.
3. **Avenger residual pack** (`host_avenger`):
   - Designator residual: DamageType STATUS / FAERIE_FIRE, duration **200**ms → **6**f,
     range **200**, Delay **200**ms → **6**f, ROF mult **150%**.
   - Air laser residual: dmg **10**/range **300**/Delay **200**ms → **6**f,
     AntiGround **No**, AntiAirborneVehicle **Yes**, AntiAirborneInfantry **No**.
   - Body residual: MaxHealth **300**, Vision **150**, Shroud **300**, BuildCost **2000**,
     BuildTime **10**s → **300**f, TransportSlotCount **3**, Speed **30**/Damaged **20**,
     PDL ScanRange **200** (AvengerPointDefenseLaserOne/Two).
   - Honesty: `honesty_avenger_residual_pack_ok` + layer honesty tests.
4. **Bunker Buster residual pack** (`host_bunker_buster`):
   - Missile residual: StealthJetMissileWeapon dmg **100**/r**5**/range **220**/min **60**,
     Delay **200**ms → **6**f, ClipSize **2**, ClipReload **8000**ms → **240**f,
     DamageType STEALTHJET_MISSILES.
   - Behavior residual: Upgrade_AmericaBunkerBusters, occupant **400**/r**10**,
     shockwave **10**/r**50**, Seismic **200**/mag **5**, structure mult **1.5**.
   - Body residual: MaxHealth **120**, Vision **180**, Shroud **300**, BuildCost **1600**,
     BuildTime **25**s → **750**f, SCIENCE_StealthFighter.
   - Honesty: `honesty_bunker_buster_residual_pack_ok` + layer honesty tests.
5. **CIA Intelligence residual pack** (`host_cia_intelligence`):
   - SpecialPower residual: SuperweaponCIAIntelligence Reload **300000**ms → **9000**f,
     Enum SPECIAL_CIA_INTELLIGENCE, ShortcutPower **Yes**, Academy ACT_SUPERPOWER.
   - Duration residual: Base **30000**ms → **900**f, BonusPerCaptured **10000**ms → **300**f,
     Max **240000**ms → **7200**f, default FOW vision radius **150**.
   - Honesty: `honesty_cia_intelligence_residual_pack_ok` + layer honesty tests.
6. **Bomb Truck disguise residual pack** (`host_bomb_truck_disguise`):
   - Ability residual: SpecialAbilityDisguiseAsVehicle, DisguisesAsTeam **Yes**,
     RevealDistance **100**, Transition **2000**ms → **60**f, RevealTransition **1000**ms → **30**f,
     FX_BombTruckDisguise / FX_BombTruckDisguiseReveal.
   - Body residual: MaxHealth **220**, Vision **150**, Shroud **200**, BuildCost **1200**,
     BuildTime **15**s → **450**f, TransportSlotCount **3**, Speed **50**/Damaged **50**.
   - Honesty: `honesty_bomb_truck_disguise_residual_pack_ok` + layer honesty tests.
7. **Cash Bounty residual pack** (`host_cash_bounty`):
   - Science residual: SCIENCE_CashBounty1/2/3 → **5%**/**10%**/**20%**, PointCost **1**,
     prereq chain GLA+Rank3 → Bounty1 → Bounty2 → Bounty3.
   - SpecialPower residual: SpecialAbilityCashBounty1/2/3, Enum SPECIAL_CASH_BOUNTY.
   - Floating text residual: GUI:AddCash, Z lift **10**, yellow RGBA (255,255,0,255).
   - Honesty: `honesty_cash_bounty_residual_pack_ok` + layer honesty tests.
8. Tests / gates (not log-only):
   - `marauder_residual_pack_honesty_wave66` / weapon / body
   - `ranger_residual_pack_honesty_wave66` / rifle / flashbang / body
   - `avenger_residual_pack_honesty_wave66` / designator / air_laser / body
   - `bunker_buster_residual_pack_honesty_wave66` / missile / behavior / body
   - `cia_intelligence_residual_pack_honesty_wave66` / special_power / duration
   - `bomb_truck_disguise_residual_pack_honesty_wave66` / ability / body
   - `cash_bounty_residual_pack_honesty_wave66` / science / special_power / floating_text
   - golden_skirmish_gate --frames 8 → PASS playable_claim=true
   - shell_smoke_gate → PASS playable_claim=false shell_host_playable_ok=true

## Residual Host Playability — Wave 64: tunnel/oil/crate/technical/troop residual packs (2026-07-13)

**Closed (host-testable residual peels):**
1. **Tunnel Network residual pack** (`host_tunnel_network`):
   - Body residual: MaxHealth **1000**, BuildCost **800**, BuildTime **15**s → **450**f,
     Vision/Shroud **200**, EnergyProduction **0**, TurretTurnRate **180**.
   - TunnelContain: TimeForFullHeal **5000**ms → **150**f, MaxTunnelCapacity **10**.
   - TunnelNetworkGun residual: dmg **15** / range **175** / Delay **250**ms → **8**f /
     WeaponSpeed **600** / FireSound HumveeWeapon / FireFX WeaponFX_TechnicalGunFire.
   - StealthDetector residual: DetectionRate **500**ms → **15**f, DetectionRange **150**.
   - Spawn residual: SpawnNumber **2**, GLAInfantryTunnelDefender OneShot.
   - CamoNetting residual: Upgrade_GLACamoNetting, StealthDelay **2500**ms → **75**f,
     Forbidden ATTACKING USING_ABILITY TAKING_DAMAGE; Hole GLAHoleTunnelNetwork **500**hp.
   - Honesty: `honesty_tunnel_network_residual_pack_ok` + layer honesty tests.
2. **Oil Derrick residual pack** (`host_oil_derrick`):
   - AutoDeposit residual: DepositTiming **12000**ms → **360**f, DepositAmount **200**,
     InitialCaptureBonus **1000**, SupplyLines Boost **+20**.
   - Body residual: MaxHealth **2000**, Shroud **100**, Geometry **23**/ **21**/ **30**.
   - Floating text residual: GUI:AddCash, Z lift **10**, alpha **230**, scatter **0.3**.
   - Flammable residual: FlameDamageLimit **20**, Expiration **2000**ms → **60**f,
     AflameDuration **5000**ms → **150**f, Damage **25** / Delay **500**ms → **15**f;
     death FX WeaponFX_BombTruckDefaultBombDetonation / FX_BuildingDie.
   - Honesty: `honesty_oil_derrick_residual_pack_ok` + layer honesty tests.
3. **Money Crate residual pack** (`host_money_crate`):
   - SupplyDropZoneCrate residual: Money **250**, BuildingPickup **Yes**, SupplyLines **+25**,
     ForbiddenKindOf PROJECTILE, KindOf PARACHUTABLE CRATE.
   - Dollar crate matrix residual: 1000DollarCrate **1000**, 2500DollarCrate **2500**.
   - Geometry residual BOX **12**/ **12**/ **12**, Physics Mass **75**.
   - OCL_AmericaSupplyDropZoneCrateDrop residual: Payload **6**, DropDelay **350**ms → **11**f,
     DeliveryDistance **410**, AmericaJetCargoPlane + AmericaCrateParachute, MaxAttempts **4**.
   - Honesty: `honesty_money_crate_residual_pack_ok` + layer honesty tests.
4. **Technical residual pack** (`host_technical`):
   - Salvage weapon residual: MG **10**/r**150**/200ms→**6**f; Cannon **45**/r**25**/scatter**10**/
     1000ms→**30**f; RPG **50**/r**5**/min**5**/1000ms→**30**f; AP **125%**.
   - Transport residual: Slots **5**, INFANTRY only, DamagePercentToUnits **10%**,
     GoAggressiveOnExit **Yes**, PassengersAllowedToFire **No**.
   - Body residual: MaxHealth **180**, Vision **150**, Shroud **300**, BuildCost **500**,
     BuildTime **5**s → **150**f, TurretTurnRate **240**, Locomotor Speed **90**/Damaged **80**.
   - Training residual: StartingLevel VETERAN + SCIENCE_TechnicalTraining; XP matrices.
   - Honesty: `honesty_technical_residual_pack_ok` + layer honesty tests.
5. **Troop Crawler residual pack** (`host_troop_crawler`):
   - Transport residual: Slots **8**, ExitDelay **250**ms → **8**f, NumberOfExitPaths **3**,
     HealthRegen **10%**/s, DamagePercentToUnits **10%**, GoAggressiveOnExit **Yes**,
     ScatterNearbyOnExit **No**, InitialPayload ChinaInfantryRedguard × **8**.
   - Assault residual: TroopCrawlerAssault DEPLOY dmg~0 / range **175** / Delay **1000**ms → **30**f;
     MembersGetHealedAtLifeRatio **0.5**.
   - Detector residual: DetectionRate **900**ms → **27**f, DetectionRange unset → Vision **175**.
   - Body residual: MaxHealth **240**, BuildCost **1400**, BuildTime **15**s → **450**f,
     Shroud **400**, Locomotor Speed **40**/Damaged **30**.
   - Honesty: `honesty_troop_crawler_residual_pack_ok` + layer honesty tests.
6. **Supply Drop Zone residual pack** (`host_supply_drop_zone`):
   - OCL residual: OCL_AmericaSupplyDropZoneCrateDrop, Min/MaxDelay **120000**ms → **3600**f,
     CreateAtEdge **Yes**.
   - Cash residual: 6 × 250 (+25 SupplyLines) → **1500** / **1650**.
   - Body residual: MaxHealth **1000**, BuildCost **2500**, BuildTime **45**s → **1350**f,
     EnergyProduction **-4**, Shroud **100**, Geometry **27**/ **27**/ **9**,
     Prerequisite AmericaStrategyCenter, KindOf FS_SUPPLY_DROPZONE.
   - Honesty: `honesty_supply_drop_zone_residual_pack_ok` + layer honesty tests.
7. Tests / gates (not log-only):
   - `tunnel_network_residual_pack_honesty` / gun / contain / body / detector_spawn / camo_hole
   - `oil_derrick_residual_pack_honesty` / deposit / body / floating_text / flammable_death
   - `money_crate_residual_pack_honesty` / supply_drop / dollar_matrix / geometry / ocl / presentation
   - `technical_residual_pack_honesty` / weapon / transport / body / training
   - `troop_crawler_residual_pack_honesty` / transport / assault / detector / body
   - `supply_drop_zone_residual_pack_honesty` / ocl / cash / body
   - golden_skirmish_gate --frames 8 → PASS playable_claim=true
   - shell_smoke_gate → PASS playable_claim=false shell_host_playable_ok=true

## Residual Host Playability — Wave 63: helix/fire-base/worker/radar/buggy/quad/overlord residual packs (2026-07-13)

**Closed (host-testable residual peels):**
1. **Helix minigun residual pack** (`host_helix_minigun`):
   - Weapon residual: PrimaryDamage **6**, radius **0**, AttackRange **115**,
     Delay **100**ms → **3**f, DamageType **COMANCHE_VULCAN**, DeathType **NORMAL**,
     ClipSize **0**, FireFX **WeaponFX_Comanche20mmCannonFire**,
     AntiAirborneVehicle **No** / AntiAirborneInfantry **Yes**.
   - Body residual: MaxHealth **300**, Vision **200**, Shroud **600**,
     BuildCost **1500**, BuildTime **20**s → **600**f.
   - Honesty: `honesty_helix_minigun_residual_pack_ok` + layer honesty tests.
2. **Fire Base residual pack** (`host_fire_base`):
   - Weapon residual: Primary **75**/r**10**, range **275**/min **50**,
     Delay **2000**ms → **60**f, ScatterRadiusVsInfantry **15**,
     DamageType **EXPLOSION**, WeaponSpeed **300**, MinWeaponSpeed **75**,
     ScaleWeaponSpeed **Yes**, Projectile **GenericTankShell**,
     DetonationFX **FX_FireBaseHowitzerExplosion**.
   - Body residual: MaxHealth **1000**, Vision/Shroud **360**, BuildCost **1000**,
     BuildTime **25**s → **750**f, EnergyProduction **0**.
   - Honesty: `honesty_fire_base_residual_pack_ok` + layer honesty tests.
3. **GLA Worker residual pack** (`host_gla_worker`):
   - Shoes residual: FastHuman **25** → WorkerShoes **30**, UpgradedSupplyBoost **8**.
   - Body residual: MaxHealth **100**, Vision **100**, Shroud **200**, BuildCost **200**,
     BuildTime **3**s → **90**f, TransportSlotCount **1**.
   - Supply residual: MaxBoxes **1**, SupplyCenterActionDelay **150**ms → **5**f,
     mine-disarm weapon **WorkerMineDisarmingWeapon**.
   - Honesty: `honesty_gla_worker_residual_pack_ok` + layer honesty tests.
4. **Radar residual pack** (`host_radar`):
   - Provider residual: CommandCenter (not Fake*) + RadarVan online path.
   - Radar Van body residual: MaxHealth **200**, Vision **200**, Shroud **500**,
     BuildCost **500**, BuildTime **10**s → **300**f, TransportSlotCount **3**.
   - Grant residual: Upgrade_GLARadar + DisableProof **Yes**.
   - Scan residual: SpecialPowerRadarVanScan Reload **30000**ms → **900**f,
     RadiusCursor **150**, Upgrade_GLARadarVanScan unpause gate.
   - Honesty: `honesty_radar_residual_pack_ok` + layer honesty tests.
5. **Rocket Buggy residual pack** (`host_rocket_buggy`):
   - Rocket residual: Primary **20**/r**0** + Secondary **5**/r**10**,
     range **300**/min **50**, Delay **200**ms → **6**f, Clip **6**/**12**,
     ClipReload **6000**ms → **180**f, AutoReloadWhenIdle **6100**ms → **183**f,
     WeaponSpeed **600**, Projectile **RocketBuggyMissile**, AP Rockets **125%**.
   - Body residual: MaxHealth **120**, Vision **180**, Shroud **300**, BuildCost **900**,
     BuildTime **10**s → **300**f, TransportSlotCount **3**.
   - Honesty: `honesty_rocket_buggy_residual_pack_ok` + layer honesty tests.
6. **Quad Cannon residual pack** (`host_quad_cannon`):
   - Ground residual: **10**/range **150**/Delay **100**ms → **3**f;
     salvage tier1 **8**/50ms→**2**f; tier2 **8**/25ms→**1**f.
   - Air residual: **5**/range **350**, AntiGround **No**, AA vehicle/infantry **Yes**,
     DamageType **SMALL_ARMS**, AP Bullets **125%**.
   - Body residual: MaxHealth **300**, Vision **150**, Shroud **300**, BuildCost **700**,
     BuildTime **6**s → **180**f, TransportSlotCount **3**.
   - Honesty: `honesty_quad_cannon_residual_pack_ok` + layer honesty tests.
7. **Overlord gun residual pack** (`host_overlord_gun`):
   - Gun residual: Primary **80**/r**5** + Secondary **20**/r**10**, range **175**,
     ClipSize **2**, DelayBetweenShots **300**ms → **9**f (honesty),
     ClipReload **2000**ms → **60**f, ScatterRadiusVsInfantry **10**,
     DamageType **ARMOR_PIERCING**, Projectile **OverlordTankShell**.
   - Uranium residual: Upgrade_ChinaUraniumShells DAMAGE **125%** → **100**/**25**.
   - Body residual: MaxHealth **1100**, Vision **150**, Shroud **200**, BuildCost **2000**,
     BuildTime **20**s → **600**f, TransportSlotCount **3**.
   - Nuclear Tanks residual name honesty: Upgrade_ChinaNuclearTanks +
     NuclearTankDeathWeapon (fail-closed not full death weapon matrix).
   - Honesty: `honesty_overlord_gun_residual_pack_ok` + layer honesty tests.
8. Tests / gates (not log-only):
   - `helix_minigun_residual_pack_honesty_wave63`
   - `fire_base_residual_pack_honesty_wave63`
   - `gla_worker_residual_pack_honesty_wave63`
   - `radar_residual_pack_honesty_wave63`
   - `rocket_buggy_residual_pack_honesty_wave63`
   - `quad_cannon_residual_pack_honesty_wave63`
   - `overlord_gun_residual_pack_honesty_wave63`
   - golden_skirmish_gate --frames 8 → `playable_claim=true` **PASS**
   - shell_smoke_gate → `playable_claim=false` / `shell_host_playable_ok=true` **PASS**

**Still residual (fail-closed, not claimed):**
- Full ChinookAIUpdate rotor wash / Helix dual-stream gattling fire matrix
- Full SPAWNS_ARE_THE_WEAPONS / HiveStructureBody Fire Base garrison matrix
- Full WorkerAIUpdate BoredTime auto-task + SupplyWarehouse delay matrix
- Full RadarUpgrade extend-animation / capture-shared radar edge cases
- Full Rocket Buggy projectile flight / AutoReloadWhenIdle live timer matrix
- Full Quad Cannon SalvageCrate W3D turret subobject swap matrix
- Full Overlord dual-volley cadence + Nuclear Tanks death weapon matrix
- Network residual replication (network deferred)

## Residual Host Playability — Wave 65: ThingFactory object packs + CSF load residual peel (2026-07-13)

**Closed (host-testable residual peels):**
1. **ScudStormMissile ThingFactory residual pack** (`special_power_strikes`):
   - Physics Mass **500**, TransportSlotCount **10**, ShroudClearingRange **0**,
     ArmorSet ProjectileArmor / DamageFX **None**, SpecialPowerCompletionDie
     **SuperweaponScudStorm**, HeightDie TargetHeightIncludesStructures **Yes**
     (TargetHeight **15**, OnlyWhenMovingDown, SnapToGroundOnDeath, InitialDelay
     **1000**ms→**30**f), DAMAGED/REALLYDAMAGED/RUBBLE model **NONE**,
     model UBScudStrm_M, Geometry Cylinder r**7**/h**30**, MaxHealth **10000**.
   - Honesty: `honesty_scud_storm_missile_thing_factory_pack`.
   - Fail-closed: not full ThingFactory Object / live MissileAIUpdate physics flight.
2. **SpectreHowitzerShell ThingFactory residual pack** (`special_power_strikes`):
   - InstantDeath death-type residual table:
     DETONATED `NONE +DETONATED` / FX_NukeGLA;
     LASERED `NONE +LASERED` / FX_GenericMissileDisintegrate +
     OCL_GenericMissileDisintegrate;
     GENERIC `ALL -LASERED -DETONATED` / FX_GenericMissileDeath.
   - Scale **0.6**, Geometry Cylinder r**4**/h**4**, Shadow **SHADOW_DECAL**,
     model AVSpectreShell1, Mass **1**, MaxHealth **100**.
   - Honesty: `honesty_spectre_howitzer_shell_thing_factory_pack`.
   - Fail-closed: not full InstantDeathBehavior Object / W3D ModelDraw shell drawable.
3. **TrailRemnant ThingFactory residual pack** (`special_power_strikes`):
   - KindOf residual pack: **NO_COLLIDE UNATTACKABLE IMMOBILE** (individual bit
     residual honesty) + ImmortalBody MaxHealth/InitialHealth **50** +
     FireWeaponUpdate/DeletionUpdate module presence + ImmortalBody floor **1**.
   - Honesty: `honesty_trail_remnant_thing_factory_pack`.
   - Fail-closed: not full ThingFactory ImmortalBody / live DeletionUpdate destroy.
4. **English CSF pack load residual peel** (`game_text_residual`):
   - Attempt load of English CSF pack path residual under windows_game when present.
   - Label-count residual honesty when live file parses (retail pack is large).
   - Missing asset → empty table honesty (label_count **0**, not boot UI claim).
   - Honesty: `load_english_csf_pack_residual` / `honesty_english_csf_pack_load`.
   - Fail-closed: not full multi-locale CSF boot UI for all LanguageId at runtime.
5. **OuterBeamWidth multi-beam residual pack** (`laser_segment_upload`):
   - NumBeams **12**, OuterBeamWidth **26**, InnerBeamWidth **0.6**,
     ScrollRate **-1.75**, TilingScalar **0.15**, Tile **Yes**, EXNoise02.tga,
     additive shader + TILED_TEXTURE_MAP; SoftnessDepth/Distance absent.
   - Honesty: `honesty_outer_beam_width_multi_beam_pack`.
   - Fail-closed: not full GPU soft-edge texture atlas / write_buffer submit.
6. Tests / gates (not log-only):
   - `scud_storm_missile_thing_factory_pack_wave65_honesty`
   - `spectre_howitzer_shell_thing_factory_pack_wave65_honesty`
   - `trail_remnant_thing_factory_pack_wave65_honesty`
   - `english_csf_pack_load_residual_wave65_honesty`
   - `outer_beam_width_multi_beam_pack_wave65_honesty`
   - golden_skirmish_gate --frames 8 → `playable_claim=true` **PASS**
   - shell_smoke_gate → `playable_claim=false` / `shell_host_playable_ok=true` **PASS**

**Still residual (fail-closed, not claimed):**
- Full ScudStormMissile ThingFactory Object / live MissileAIUpdate physics flight
- Full SpectreHowitzerShell ThingFactory Object / W3D ModelDraw shell drawable
- Full TrailRemnant ThingFactory ImmortalBody / live DeletionUpdate module stack
- Full multi-locale CSF/STR GameText table load for all LanguageId at runtime boot UI
- Full OuterBeamWidth multi-beam GPU soft edge / texture atlas sample bind
- Actual `wgpu::Queue::write_buffer` against a live device/pipeline
- Network residual replication (network deferred)
## Residual Host Playability — Wave 62: strategy_center + unit_training + upgrades + sneak_attack residual packs (2026-07-13)

**Closed (host-testable residual peels):**
1. **Strategy Center residual pack** (`host_strategy_center`):
   - Battle plan army residuals: Bombardment DAMAGE **120%**, HoldTheLine armor scalar **0.9**,
     SearchAndDestroy RANGE **120%** + Sight **1.2**; building HoldTheLine MaxHealth **2.0**
     PRESERVE_RATIO; S&D Sight **2.0** + DetectsStealth **Yes**.
   - Pack/unpack AnimationTime **7000**ms → **210**f (all plans); TransitionIdle **0**;
     pack/unpack audio residual names honesty.
   - BattlePlanChangeParalyzeTime **5000**ms → **150**f residual.
   - Strategy Center body residual: BuildCost **2500**, BuildTime **60**s → **1800**f,
     MaxHealth **1500**, Vision/Shroud **400**, EnergyProduction **-2**,
     SpecialAbilityChangeBattlePlans + Valid/Invalid MemberKindOf tokens + message labels.
   - Honesty: `honesty_strategy_center_residual_pack_ok` + layer honesty tests.
2. **Unit training residual pack** (`host_unit_training`):
   - Veterancy StartingLevel residual: RedGuard/Artillery/Technical/Gattling **VETERAN**,
     InfaRedGuard/Battlemaster **ELITE**; SciencePurchasePointCost **1**.
   - SCIENCE_GattlingTankTraining residual → Gattling Tank VETERAN grant path.
   - Training BuildTime residual: RedGuard **10**s→**300**f, Battlemaster **10**s→**300**f,
     Inferno **15**s→**450**f, Technical **5**s→**150**f, MiniGunner **10**s→**300**f,
     Gattling **10**s→**300**f.
   - Free unit residual: always-Veteran pilot path (no ScienceRequired) + AdvancedTraining
     ExperienceScalarUpgrade AddXPScalar **1.0**.
   - Honesty: `honesty_unit_training_residual_pack_ok` + layer honesty tests.
3. **Upgrades residual pack** (`host_upgrades`):
   - Retail Upgrade.ini BuildCost / BuildTime residual matrix for HostUpgradeKind
     (SupplyLines **800**/30s, FlashBang **800**/30s, Capture **1000**/30s,
     Camouflage **2000**/60s, CamoNetting **500**/5s, NeutronShells **2500**/60s, …).
   - Host `residual_research_frames` remains **1** (observable queue); `retail_research_frames`
     documents Upgrade.ini BuildTime → frames honesty.
   - StealthForbiddenConditions residual: Camouflage Rebel = ATTACKING USING_ABILITY;
     CamoNetting structures = ATTACKING USING_ABILITY TAKING_DAMAGE; Camouflage unit
     stealth_desired helper + StealthDelay **2500**ms → **75**f.
   - Honesty: `honesty_upgrades_residual_pack_ok` + layer honesty tests.
4. **Sneak Attack residual pack** (`host_sneak_attack`):
   - SuperweaponSneakAttack: ReloadTime **150000**ms → **4500**f, RadiusCursor **50**,
     RequiredScience SCIENCE_SneakAttack, SharedSyncedTimer **Yes**,
     InitiateAtLocationSound SneakAttackActivated.
   - Tunnel residual: GLASneakAttackTunnelNetwork MaxHealth **1000**, Vision **200**,
     Start Lifetime **5000**ms → **150**f, OCL Start/Tunnel residual names.
   - Multi-shockwave residual matrix: Small **10**/r**35** @ **10**ms→**1**f;
     Big **50**/r**50** @ **1000**ms→**30**f and **2500**ms→**75**f
     (host live apply still Big-only at spawn — fail-closed multi-pulse).
   - Honesty: `honesty_sneak_attack_residual_pack_ok` + layer honesty tests.
5. **Scorpion thin residual pack** (`host_scorpion`, optional):
   - Gun **20**/**25** salvage + rocket **100**/r**5** + **80**/r**25** + AP **125%** honesty pack.
6. Tests / gates (not log-only):
   - `strategy_center_residual_pack_honesty` / battle_plan params / pack_unpack / paralyze / body
   - `unit_training_residual_pack_honesty` / veterancy / time / free unit
   - `upgrades_residual_pack_honesty` / cost / time / stealth forbidden
   - `sneak_attack_residual_pack_honesty` / special power / tunnel / spawn
   - `scorpion_residual_pack_honesty`
   - golden_skirmish_gate --frames 8 → `playable_claim=true` **PASS**
   - shell_smoke_gate → `playable_claim=false` / `shell_host_playable_ok=true` **PASS**

**Still residual (fail-closed, not claimed):**
- Full PartitionManager filter stack / W3D Turret bone GPU draw for Strategy Center
- Full SCIENCE_GattlingTankTraining stock ChinaTankGattling module wire (science residual only)
- Full multi-shockwave live damage apply (host still Big-only at spawn)
- Full ProductionUpdate retail BuildTime for host upgrade research path (still 1-frame host queue)
- Network residual replication (network deferred)

## Residual Host Playability — Wave 60: host_gla_rebel + host_rpg_trooper + host_missile_defender + host_terrorist residual (2026-07-13)
**Closed (host-testable residual peels):**
1. **GLA Rebel residual pack** (`host_gla_rebel`):
   - Gun residual: PrimaryDamage **5**, AttackRange **100**, Delay **100**ms → **3**f,
     ClipSize **3**, ClipReload **700**ms → **21**f, DamageType **SMALL_ARMS**,
     FireSound **RebelWeapon**, FireFX **WeaponFX_GenericMachineGunFire**,
     radius **0** (intended-only).
   - AP Bullets residual: Upgrade_GLAAPBullets DAMAGE **125%** → **6.25**.
   - Capture residual: `SpecialAbilityRebelCaptureBuilding` Reload **15000**ms → **450**f,
     StartAbilityRange **5**, Unpack **3000**ms → **90**f, Prep **20000**ms → **600**f,
     Pack **2000**ms → **60**f, AwardXP **12**, Upgrade_InfantryCaptureBuilding gate.
   - Body residual: MaxHealth **120**, Vision **150**, Shroud **300**, BuildCost **150**.
   - BoobyTrap residual name honesty: SpecialAbilityBoobyTrap Reload **7500**ms → **225**f
     (fail-closed: not full SpecialObject plant matrix).
   - Honesty: `honesty_gla_rebel_residual_pack_ok` + layer honesty tests.
2. **RPG Trooper residual pack** (`host_rpg_trooper`):
   - Rocket residual: Primary **40**/r**5**, range **175**/min **5**, Delay **1000**ms → **30**f,
     WeaponSpeed **600**, DamageType **INFANTRY_MISSILE**, DeathType **EXPLODED**,
     ClipSize **0**, AutoReloadsClip **Yes**, ScatterRadiusVsInfantry **10**,
     Projectile **TunnelDefenderMissile**, FireSound **RPGTrooperWeapon**,
     detonation FX **WeaponFX_RocketBuggyMissileDetonation**.
   - AP Rockets residual: Upgrade_GLAAPRockets DAMAGE **125%** → **50**.
   - Body residual: MaxHealth **100**, Vision **150**, Shroud **400**, BuildCost **300**.
   - Honesty: `honesty_rpg_trooper_residual_pack_ok` + layer honesty tests.
3. **Missile Defender residual pack** (`host_missile_defender`):
   - Primary residual: **40**/r**5**/range **175**/Delay **1000**ms → **30**f,
     DamageType **INFANTRY_MISSILE**, FireFX **FX_BuggyMissileIgnition**,
     WeaponBonus DAMAGE **125%** honesty mult.
   - Laser weapon residual: **40**/r**5**/range **300**/Delay **500**ms → **15**f,
     DamageType **ARMOR_PIERCING**, AutoChooseSources SECONDARY **NONE**.
   - Laser lock residual: StartAbilityRange **200**, AbilityAbortRange **250**,
     PreparationTime **1000**ms → **30**f, PersistentPrepTime **500**ms → **15**f,
     SpecialObject **LaserBeam**, ReloadTime **0**, InitiateSound
     **MissileDefenderVoiceAttackLaser**.
   - Body residual: MaxHealth **100**, Vision **150**, Shroud **400**, BuildCost **300**.
   - Honesty: `honesty_missile_defender_residual_pack_ok` + layer honesty tests.
4. **Terrorist residual pack** (`host_terrorist`):
   - Trigger residual: TerroristSuicideWeapon AttackRange **1**, PrimaryDamage **999999**,
     LeechRangeWeapon **Yes**, ClipSize **1**, AutoReloadsClip **No**; host combat
     path keeps dynamite AttackRange **5** (fail-closed vs exact trigger range).
   - Death weapon residual: dual rings Primary **500**/r**18** + Secondary **300**/r**50**,
     FireFX **WeaponFX_SuicideDynamitePackDetonation**, FireSound **CarBomberDie**,
     RadiusAffects SELF SUICIDE … NOT_SIMILAR, DamageType **EXPLOSION**/DeathType **SUICIDED**.
   - Profile residual: Chem Gamma primary **600**, Demo primary **700**,
     Demo FireFX **WeaponFX_DemoSuicideDynamitePackDetonation**,
     Chem FireOCL poison fields.
   - Body residual: MaxHealth **120**, Vision **150**, Shroud **200**, BuildCost **200**.
   - Honesty: `honesty_terrorist_residual_pack_ok` + layer honesty tests.
5. Tests / gates (not log-only):
   - `gla_rebel_residual_pack_honesty` / capture / gun / body
   - `rpg_trooper_residual_pack_honesty` / rocket / AP / body
   - `missile_defender_residual_pack_honesty` / primary / laser lock / body
   - `terrorist_residual_pack_honesty` / trigger / death weapon / profile
   - golden_skirmish_gate --frames 8 → `playable_claim=true` **PASS**
   - shell_smoke_gate → `playable_claim=false` / `shell_host_playable_ok=true` **PASS**

**Still residual (fail-closed, not claimed):**
- Full ClipSize=3 in-clip DelayBetweenShots 100ms + ClipReload 700ms volley matrix
- Full CaptureBuilding BinaryDataStream attach / packing anim matrix
- Full BoobyTrap SpecialObject plant / MaxSpecialObjects list UI
- Full ScatterRadiusVsInfantry random miss / projectile exhaust FX matrix
- Full LaserBeam special object attach-bone matrix / live WeaponBonus apply
- Full SlowDeath SUICIDED fling / ConvertToCarBombCrateCollide (host_car_bomb)
- Network residual replication (network deferred)

## Residual Host Playability — Wave 61: host_usa_pilot + host_aurora_bomb + host_slave_drones residual (2026-07-13)
**Closed (host-testable residual peels):**
1. **USA Pilot residual pack** (`host_usa_pilot`):
   - Body residual: MaxHealth **100**, VisionRange **150**, ShroudClearingRange **300**,
     StartingLevel **VETERAN**, TransportSlotCount **1**, Mass **5**.
   - Ejection residual: InvulnerableTime **2000**ms → **60**f, Ground force **2–3** /
     Air force **10–12**, pitch **50–60**, PutInContainer **AmericaParachute**,
     DeathTypes **ALL -CRUSHED -SPLATTED**, ExemptStatus **HIJACKED**,
     VeterancyLevels **ALL -REGULAR**, InheritsVeterancy **Yes**.
   - Parachute residual: retail OpenDist **25** dual-track + host/CINE **100**,
     FreeFallDamage **50%**, Pitch/RollRateMax **60** deg/s, LowAltitudeDamping **0.2**,
     GeometryHeight **10** / MajorRadius **15**, open audio **ParachuteOpen**.
   - Return-to-base residual: PilotFindVehicle base-center fallback (`m_didMoveToBase`),
     ScanRate **1000**ms → **30**f / ScanRange **300** / MinHealth **0.5**;
     AutoFindHealing NeverHeal **0.85** / AlwaysHeal **0.25** (busy-interrupt dead).
   - Honesty: `honesty_usa_pilot_residual_pack_ok` + layer honesty tests.
2. **Aurora bomb residual pack** (`host_aurora_bomb`):
   - Bomb damage/range residual: Standard **400**/r**20**/range **300**, AirF detonation
     **1000**/r**100**, SupW **900**/r**70**, primary tiny **2**/r**4**, flame **5**/r**100**,
     DamageType **AURORA_BOMB**, AcceptableAimDelta **45**, RadiusAffects ALLIES…NOT_SIMILAR.
   - RETURN_TO_BASE reload residual: ClipSize **1**, ClipReload **5000**ms → **150**f,
     AutoReloadsClip **RETURN_TO_BASE**, Jet ReturnToBaseIdleTime **10000**ms → **300**f.
   - Projectile residual: **AuroraBomb** MaxHealth **100**, Mass **75**, loco Speed **480** /
     MinSpeed **240** / Accel **960** / TurnRate **960** / MaxThrustAngle **60**;
     Jet body MaxHealth **80**, Vision **180**, Shroud **600**, BuildCost **2500**,
     SneakyOffset **-20**.
   - Honesty: `honesty_aurora_bomb_residual_pack_ok` + layer honesty tests.
3. **Slave drones residual pack** (`host_slave_drones`):
   - SlavedUpdate wander residual: GuardMax/Wander **35**, Attack **75**/Wander **10**,
     Scout **75**/Wander **10**, StayOnSameLayerAsMaster **Yes**, Scout range-bonus **20**,
     DetectionRate **500**ms → **15**f.
   - Spawn residual: OCL Scout Offset **X:-8 Z:10**, Battle/Hellfire **Z:10**, Count **1**,
     Disposition LIKE_EXISTING; Upgrade costs Scout **100** / Battle **300** / Hellfire **500**,
     MaxHealth **100**, DroneArmor +**25**/+**50** Battle.
   - Repair residual: RepairRange **8**, Min/MaxAltitude **18/24**, Rate **10**/s,
     BelowHealth% **60**, Ready **300–750**ms, Weld **250–500**ms, BlueSparks;
     Hellfire **40**/r**5**/range **150** cycle **90**f; Battle gun **1**/range **110**/3f.
   - Honesty: `honesty_slave_drones_residual_pack_ok` + layer honesty tests.
4. Tests / gates (not log-only):
   - `usa_pilot_residual_pack_honesty_wave61` / body / ejection / parachute / return-to-base
   - `aurora_bomb_residual_pack_honesty_wave61` / damage-range / RETURN_TO_BASE / projectile
   - `slave_drones_residual_pack_honesty_wave61` / wander / spawn / repair
   - golden_skirmish_gate --frames 8 → `playable_claim=true` **PASS**
   - shell_smoke_gate → `playable_claim=false` / `shell_host_playable_ok=true` **PASS**

**Still residual (fail-closed, not claimed):**
- Full dual-template ParachuteOpenDist matrix (retail 25 vs CINE 100) / W3D bone extract
- Full JetAIUpdate airfield RETURN_TO_BASE rearm path / AuroraBombLocomotor flight
- Full SlavedUpdate AI wander pathfinding / weld arm pack-unpack anim interleave
- Network residual replication (network deferred)

## Residual Host Playability — Wave 57: host_colonel_burton + host_jarmen_kell + host_hero_abilities residual (2026-07-13)
**Closed (host-testable residual peels):**
1. **Colonel Burton residual pack** (`host_colonel_burton`):
   - Knife PreAttackDelay **833**ms → **25**f, PreAttackType **PER_ATTACK**,
     DamageType **MELEE**, LeechRangeWeapon **Yes**, ClipReload **1367**ms → **41**f,
     PrimaryDamage **10000**, AttackRange **3**, ClipSize **1**.
   - Sniper residual: dmg **40**/range **125**/Delay **100**ms → **3**f, ClipSize **3**,
     ClipReload **500**ms → **15**f, DamageType **SMALL_ARMS**, FireSound/FireFX names.
   - Remote/timed charge residual peel: RemoteC4 Max **8**, TimedC4 Max **10**,
     UnpackTime **5500**ms → **165**f, FleeRange **100**, LoseStealthOnTrigger **Yes**,
     PreTriggerUnstealth **5000**ms → **150**f, special power / object names.
   - StealthUpdate residual: StealthDelay **2000**ms → **60**f, InnateStealth **Yes**,
     Forbidden **FIRING_PRIMARY**, FriendlyOpacity **50–100%**, EVA detect events.
   - Body residual: MaxHealth **200**, Vision **150**, Shroud **500**, BuildCost **1500**.
   - Honesty: `honesty_colonel_burton_residual_pack_ok` + layer honesty tests.
2. **Jarmen Kell residual pack** (`host_jarmen_kell`):
   - Sniper residual: dmg **180**/range **225**/Delay **1000**ms → **30**f,
     DamageType **SNIPER**, radius **0**, AP Bullets **125%** → **225**.
   - Vehicle pilot-snipe residual: `GLAJarmenKellVehiclePilotSniperRifle`
     dmg **1**, DamageType **KILL_PILOT**, range **225**, ClipSize **1**,
     ClipReload **30000**ms → **900**f, AutoReloadsClip **Yes**, legal target gates.
   - StealthUpdate residual: StealthDelay **2000**ms → **60**f, InnateStealth **Yes**,
     Forbidden **ATTACKING**, EVA detect events.
   - Body residual: MaxHealth **200**, Vision **200**, Shroud **400**, BuildCost **1500**.
   - Biker Delay **750**ms → **23**f honesty (fail-closed: infantry stays 1000ms).
   - Honesty: `honesty_jarmen_kell_residual_pack_ok` + layer honesty tests.
3. **Hero abilities residual pack** (`host_hero_abilities`):
   - CashHack science tiers: MoneyAmount **1000**, SCIENCE_CashHack2 **2000**,
     SCIENCE_CashHack3 **4000**, ReloadTime **240000**ms → **7200**f.
   - Black Lotus StealCashHack EffectValue **1000** (unit special; not science-tiered).
   - BlackMarket emergency residual: GLABlackMarket / FS_BLACK_MARKET cash-hack
     target gates + `record_black_market_emergency_steal` honesty counter.
   - Special ability timers from INI:
     - Capture: Unpack **6730**/Pack **2800**/Prep **6000**ms → **202/84/180**f
     - DisableVehicle: Unpack **2000**/Pack **1000**/Prep **2000**ms → **60/30/60**f,
       EffectDuration **15000**ms → **450**f (INI value; prior 30s fail-closed closed)
     - StealCash: Unpack **6730**/Pack **5800**/Prep **6000**/Reload **2000**ms
     - Burton charges: Unpack **5500**ms → **165**f, Flee **100**, PreTrigger **5000**ms
   - Lotus StealthUpdate: StealthDelay **2500**ms → **75**f, Forbidden **USING_ABILITY**.
   - Honesty: `honesty_hero_abilities_residual_pack_ok` + layer honesty tests.
4. Tests / gates (not log-only):
   - `colonel_burton_residual_pack_honesty` / knife / stealth / charges
   - `jarmen_kell_residual_pack_honesty` / pilot snipe / stealth
   - `hero_abilities_residual_pack_honesty` / cash tiers / black market emergency
   - integration: sniper/knife, snipe vehicle, steal cash, disable vehicle, plant charges
   - golden_skirmish_gate --frames 8 → `playable_claim=true` **PASS**
   - shell_smoke_gate → `playable_claim=false` / `shell_host_playable_ok=true` **PASS**

**Still residual (fail-closed, not claimed):**
- Full knife PreAttackDelay anim lock / PER_ATTACK state machine interleave
- Full SECONDARY AutoChooseSources=NONE pilot-sniper WeaponSet chooser matrix
- Full SpecialAbilityUpdate continuous BinaryDataStream attach / packing anim
- Full CashHackSpecialPower victim money clamp / floating text path
- Full StickyBombUpdate attach bones / live max-charge list UI
- Network residual replication (network deferred)

## Residual Host Playability — Wave 58: host_humvee + host_tomahawk + host_combat_chinook + host_battle_bus residual peels (2026-07-13)
**Closed (host-testable residual outside special_power_strikes / host_colonel_burton / host_hero_abilities):**
1. **Humvee residual pack** (`host_humvee`):
   - PRIMARY HumveeGun: **10**/r**150**/Delay **200**ms → **6**f / Speed **600**.
   - Ground TOW HumveeMissileWeapon: **30**/r**5**/range **150**, Delay **1000**ms → **30**f +
     ClipReload **2000**ms → **60**f = cycle **90**f; Projectile HumveeMissile;
     Upgrade_AmericaTOWMissile residual honesty.
   - Air TOW HumveeMissileWeaponAir: **50**/r**5**/range **320**, cycle **90**f,
     Projectile PatriotMissile; pre-TOW AirDummy **0.0001**/r**320**.
   - TransportContain Slots **5**, ExitDelay **250**ms → **8**f, ExitPaths **3**,
     DamagePercentToUnits **100%**, PassengersAllowedToFire **Yes**, infantry-only.
   - Body MaxHealth **240**, Vision **150**, Shroud **320**, BuildCost **700**,
     TurretTurnRate **180**, Recenter **5000**ms → **150**f, Locomotor Speed **60**.
2. **Tomahawk residual pack** (`host_tomahawk`):
   - Weapon dual-radius retained (**150**/r**10** + **50**/r**25**, range **350**,
     min **100**, ClipReload **7000**ms → **210**f, PreAttack **250**ms → **8**f).
   - Missile loft residual: FuelLifetime **4000**ms → **120**f, InitialVelocity **50**,
     DistanceToTravelBeforeTurning **80**, DistanceToTargetBeforeDiving **100**,
     DistanceToTargetForLock **10**, PreferredHeight **120**, Damping **0.7**.
   - TomahawkMissileLocomotor: Speed **200**, MinSpeed **100**, Accel **675**,
     TurnRate **540**, MaxThrustAngle **45**.
   - Launcher FirePitch **70**, TurretTurn/Pitch **60**, MaxHealth **180**,
     Vision **180**, Shroud **200**, BuildCost **1200**, vehicle Speed **30**.
3. **Combat Chinook residual pack** (`host_combat_chinook`):
   - Transport: Slots **8**, ExitDelay **100**ms → **3**f, ExitPaths **1**,
     Allow INFANTRY+VEHICLE, Forbid AIRCRAFT+HUGE_VEHICLE, ArmedRiders **Yes**.
   - Passenger "minigun enable": ListeningOutpostUpgradedDummyWeapon **0.1**/r**90**/
     Delay **1000**ms → **30**f AntiAir **Yes**.
   - PointDefenseLaser residual: AirF_PointDefenseLaser **100**/r**65**,
     Delay **250**ms → **8**f, ScanRange **250**, ScanRate **33**ms → **1**f.
   - ChinookAIUpdate: MaxBoxes **8**, NumRopes **4**, PerRopeDelay **900–1500**ms →
     **27–45**f, RappelSpeed **30**, MinDropHeight **40**, RopeFinalHeight **10**,
     Supply delays **3000/1250**ms → **90/38**f, UpgradedSupplyBoost **60**.
   - Body MaxHealth **350**, Vision **300**, Shroud **600**, BuildCost **1200**,
     Locomotor Speed **150**, PreferredHeight **100**.
4. **Battle Bus residual pack** (`host_battle_bus`):
   - Transport: Slots **8**, ExitDelay **250**ms → **8**f, ExitPaths **5**,
     infantry-only, WeaponBonusPassedToPassengers **Yes**, DelayExitInAir **Yes**.
   - PassengerDummy **0.001**/r**90**/Delay **10000**ms → **300**f;
     BattleBusDummyWeapon AA **0.0001**/r**320**/Delay **500**ms → **15**f.
   - BattleBusSlowDeath suicide/detonate residual: ThrowForce **100**,
     PercentDamageToPassengers **50%**, EmptyHulkDestructionDelay **1000**ms → **30**f,
     ProbabilityModifier **5**, DestructionDelayVariance **200**ms → **6**f,
     FX/OCL StartUndeath + HitGround + Final detonate residual names.
   - UndeadBody MaxHealth **400** / SecondLife **650**, Vision **150**, Shroud **200**,
     BuildCost **1000**, Locomotor Speed **70**.
5. Honesty packs + module tests (not log-only):
   - `honesty_humvee_residual_pack_ok` / `humvee_residual_pack_honesty_wave58`
   - `honesty_tomahawk_residual_pack_ok` / `tomahawk_residual_pack_honesty_wave58`
   - `honesty_combat_chinook_residual_pack_ok` / `combat_chinook_residual_pack_honesty_wave58`
   - `honesty_battle_bus_residual_pack_ok` / `battle_bus_residual_pack_honesty_wave58`

**Still residual (fail-closed, not claimed):**
- Full WeaponSet PLAYER_UPGRADE visual turret swap (Humvee TOW model)
- Full TomahawkMissile projectile lob / waypoint path Object
- Full ChinookAIUpdate ropes / rappel / combat-drop clear matrix
- Full BattleBusSlowDeathBehavior SECOND_LIFE structure hulk undeath Object
- Full PointDefenseLaserUpdate velocity prediction live matrix
- Network residual replication (network deferred)

## Residual Host Playability — Wave 59: car-bomb/dragon/stealth residual + neutron honesty (2026-07-13)
**Closed (host-testable residual):**
1. **Neutron Shell equip honesty fix** (`weapon_bootstrap` + `create_object`):
   - Root cause: `secondary_weapon_name_for_unit` auto-equipped
     `NukeCannonNeutronWeapon` at create for ChinaVehicleNukeCannon, breaking
     `neutron_shell_residual_upgrade_and_blast` (pre-upgrade must lack secondary).
   - Fix: neutron secondary is **PLAYER_UPGRADE residual only** (parity with
     rocket pods). Research `Upgrade_ChinaNeutronShells` equips existing cannons;
     create_object equips only when player has unlocked the upgrade or object
     already carries the tag; explicit template secondary still kept for tests.
2. **Car Bomb residual pack** (`host_car_bomb`):
   - Detonation damage: Primary **700**/r**20**, Secondary **100**/r**50**,
     DamageDealtAtSelf **Yes**, RadiusAffects SELF SUICIDE … NOT_SIMILAR.
   - Convert residual: FX_MakeCarBombSuccess / TerroristCarBomb; Hijack range **5**.
   - FireSound residual: **CarBomberDie** + WeaponFX_SuicideDynamitePackDetonation.
   - Range residual: AttackRange **5**, ClipSize **1**, AutoReloadsClip **No**.
   - Honesty: `honesty_car_bomb_residual_pack_ok` + layer honesty tests.
3. **Dragon Tank residual pack** (`host_dragon_tank`):
   - Fire wall residual: AttackRange **25**, OCL FireWallSegment / Upgraded,
     segment **4**/r**10**/250ms → **8**f; upgraded segment **5**.
   - Napalm residual: BlackNapalm **12.5**/1.25, MinRange **10**,
     FireSoundLoopTime **80**ms → **2**f, garrison Yes.
   - Range residual: flame **75**, firewall **25**, FLAME/BURNED, speed **600**.
   - Honesty: `honesty_dragon_tank_residual_pack_ok` + layer honesty tests.
4. **Stealth Fighter residual pack** (`host_stealth_fighter`):
   - StealthJetMissile residual: **100**/r**5**, range **220**/min **60**,
     Delay **200**ms → **6**f, Clip **2**/reload **8000**ms → **240**f,
     STEALTHJET_MISSILES, RETURN_TO_BASE, anti-air No, upgrade dmg **125%**.
   - KillSelfDelay residual: **2000**ms → **60**f, DetonateCallsKill **Yes**,
     HP **100**, Mass **1**.
   - Bunker-buster related residual: UpgradeRequired, Seismic **200**/mag **5**,
     Shockwave BunkerBusterShockwaveWeaponSmall (shared with host_bunker_buster).
   - Honesty: `honesty_stealth_fighter_residual_pack_ok` + layer honesty tests.
5. Tests / gates:
   - neutron lib: **8** ok (incl. upgrade_and_blast)
   - car_bomb / dragon / stealth_fighter residual pack honesty tests green
   - golden_skirmish_gate --frames 8 → `playable_claim=true` **PASS**
   - shell_smoke_gate → `playable_claim=false` / `shell_host_playable_ok=true` **PASS**

**Still residual (fail-closed, not claimed):**
- Full DumbProjectileBehavior neutron bezier / WeaponSet UI toggle
- Full SuicideCarBomb NOT_SIMILAR ally matrix / HijackerUpdate hide path
- Full InchForward FireWallSegment crawl / ProjectileStream draw
- Full StealthJetMissile AI crash-through / JetAIUpdate RETURN_TO_BASE rearm
- Network residual replication (network deferred)

## Residual Host Playability — Wave 56: CarpetBomb/Cruise/Artillery residual deepen (2026-07-13)
**Closed (host-testable residual in `special_power_strikes`):**
1. **CarpetBomb residual pack deepen**:
   - DropDelay stagger: USA/China **300**ms → **9**f; AirF **130**ms → **4**f.
   - DropVariance residual **30/40/0** (X/Y/Z) honesty + applications.
   - AmericaJetB52 / B52Locomotor PreferredHeight **100**, Speed **125**.
   - Science/faction bomb counts: USA **15** / AirF **12** / China **10** + line length residual.
   - DeliveryDistance **400/500/350**, FireFX **FX_CarpetBomb**, Transport names residual.
   - Application counters armed at queue + FireFX per bomb wave.
2. **CruiseMissile / MOAB residual pack deepen**:
   - Loft: DistanceToTravelBeforeTurning **200**, SpecialSpeedTime **1500**ms → **45**f,
     HeightDie InitialDelay **1000**ms → **30**f, TargetHeight **10**, loft composite **75**f.
   - Projectile object **CruiseMissile**, OCL **SUPERWEAPON_CruiseMissile**,
     DeathWeapon **MOABDetonationWeapon**, FireFX **WeaponFX_MOAB_Blast**.
   - MOAB Primary **2000**/r**150** + ShockWave **250**/r**200**/taper **0.33** +
     MOABFlame **5**/r**100** residual honesty + impact applications.
3. **ArtilleryBarrage residual pack deepen**:
   - FormationSize science tiers **12/24/36** retained + applications.
   - DelayDeliveryMin **0** / Max **3000**ms → **90**f, WeaponErrorRadius **100**.
   - ChinaArtilleryCannon transport honesty: PreferredHeight **500**,
     DeliveryDistance **250**, DecalRadius **125**, Locomotor
     ChinaArtilleryBarrageCannonLocomotor Speed **150**, shell/weapon names.
4. **NuclearMissile radiation residual pack deepen**:
   - SuspendFXDelay **10000**ms → **300**f, FireFX **WeaponFX_LargeRadiationFieldWeapon**,
     DamageType **RADIATION**, OCL_NukeRadiationField / object body residual.
5. **AnthraxBomb poison residual pack deepen**:
   - FireFX **WeaponFX_LargePoisonFieldWeaponUpgraded**, DeathType **POISONED_BETA**,
     WeaponSpeed **600**, OCL_PoisonFieldAnthraxBomb residual + parent-strike applications.
6. Snapshot/Xfer: HostRadiationField + HostToxinField residual pack counters appended;
   HostSpecialPowerStrike default constructors include Wave 56 honesty fields.
7. Tests / gates (not log-only):
   - carpet/cruise/artillery/nuke/anthrax residual pack wave56 honesty tests
   - special_power_strikes lib: **95** ok
   - golden_skirmish_gate --frames 8 → `playable_claim=true` **PASS**
   - shell_smoke_gate → `playable_claim=false` / `shell_host_playable_ok=true` **PASS**

**Still residual (fail-closed, not claimed):**
- Full AmericaJetB52 DeliverPayloadAIUpdate pathfinder / flight Object
- Full NeutronMissileUpdate door/loft physics projectile Object
- Full ChinaArtilleryCannon transport Object / live shell drawable
- Full HazardousMaterialArmor / cleanup-hazard radiation stack
- Network residual replication (network deferred)

## Residual Host Playability — Wave 55: host_toxin_tractor + host_microwave + host_neutron_shell residual peels (2026-07-13)
**Closed (host-testable residual outside special_power_strikes / host_ecm / host_hacker / host_pathfinder):**
1. **Toxin Tractor residual pack** (`host_toxin_tractor`):
   - Contaminate puddle: MediumPoisonField **2**/r**80**/30s/**15**f tick;
     upgraded/gamma **2.5**/tick; Small death field base r**12** / upgraded r**7.5**;
     OCL Medium/Upgraded/Gamma + MinShots **4**, ContinuousFireCoast **300**ms → **9**f,
     OCLLifetimePerSecond **10000**, MaxCap **180000**ms → **5400**f;
     field HP **100**/**120**, geometry r**40**.
   - Spray weapon residual: Secondary **2**/r**75**/range **15**, Delay **200**ms → **6**f;
     salvage spray matrix Base/PlusOne/PlusTwo × anthrax (2/2.5/3, 2.5/3/4, 2.5/3.5/4.5).
   - Upgrade anthrax residual: stream salvage Beta **12.5/15/20**, Gamma **20.5/24.5/28.5**;
     DeathType POISONED/BETA/GAMMA; death weapons/OCL names; stream Clip **30**,
     Delay/ClipReload **40**ms → **2**f, WeaponSpeed **600**, garrison Yes.
   - Clean-up interaction residual: CLEANUP_HAZARD name gate +
     `clear_fields_in_radius` / HazardousMaterialArmor / HazardFieldCoreWeapon.
   - Honesty: `honesty_toxin_tractor_residual_pack_ok` + layer honesty tests.
   - Fail-closed: not full FireOCL continuous-coast live timer / stream projectile draw.
2. **Microwave residual pack** (`host_microwave`):
   - Cook radius: Disabler **200**, Clearer **125**, Emitter r**100** dmg **8**.
   - Disable residual: SUBDUAL_BUILDING pulse **50**, Delay **100**ms → **3**f,
     FireSoundLoop **120**ms → **4**f, Laser MicrowaveDisableStream / WEAPON02.
   - Ally filter residual: INI ALLIES ENEMIES NEUTRALS; host `HOST_MICROWAVE_AFFECTS_ALLIES=false`.
   - Weapon residual: KILL_GARRISONED **1**/shot, emitter Delay **250**ms → **8**f,
     MICROWAVE/BURNED, VehicleDisabler residual disabled.
   - Honesty: `honesty_microwave_residual_pack_ok` + counters.
   - Fail-closed: not full SubdualDamageHelper drain / emitter particle volume.
3. **Neutron Shell residual pack** (`host_neutron_shell`):
   - Neutron blast: BlastRadius **70**, AffectAirborne **No**, AffectAllies default **Yes**;
     shell primary dmg **1**/r**10**.
   - Shell projectile: AttackRange **350**, Min **150**, Delay **10000**ms → **300**f,
     WeaponSpeed **200**, Projectile NeutronCannonShell; flight FirstHeight **50** /
     SecondHeight **150** / indent **30%**/**70%**, DetonateCallsKill **Yes**.
   - Kill infantry residual: KillInfantry/Unman/KillVehicle matrix + registry counters.
   - Honesty: `honesty_neutron_shell_residual_pack_ok` + layer honesty tests.
   - Fail-closed: not full live DumbProjectileBehavior bezier / WeaponSet UI toggle.
4. **CleanupHazardUpdate scan residual** (`host_cleanup_area`, toxin-related):
   - ScanRate **1000**ms → **30**f, ScanRange **100**, WeaponSlot PRIMARY;
     AmbulanceCleanHazardWeapon dmg **100**/r**50**, Delay/ClipReload **40**ms → **2**f,
     Clip **30**, HAZARD_CLEANUP, CleanupStreamProjectile.
   - Honesty: `honesty_cleanup_hazard_scan_residual_ok`.
5. Tests / gates (not log-only):
   - `toxin_residual_pack_honesty` / salvage matrix / cleanup clear
   - `microwave_residual_pack_honesty` / ally filter / emitter delay
   - `neutron_residual_pack_honesty` / blast / projectile / infantry
   - `cleanup_hazard_scan_residual_pack_honesty`
   - golden_skirmish_gate --frames 8 → `playable_claim=true` **PASS**
   - shell_smoke_gate → `playable_claim=false` / `shell_host_playable_ok=true` **PASS**

**Still residual (fail-closed, not claimed):**
- Full FireOCLAfterWeaponCooldown continuous-coast live timer / stream FX bones
- Full SubdualDamageHelper accumulate/heal + MicrowaveDisableStream GPU laser
- Full NeutronCannonShell live flight path / command-button weapon-set UI
- Full CleanupHazardUpdate idle auto-scan loop / CleanupStream projectile draw
- Network residual replication (network deferred)

## Residual Host Playability — Wave 54: host_ecm_jam + host_hacker_disable + host_pathfinder residual (2026-07-13)
**Closed (host-testable residual outside special_power_strikes / host_toxin / host_microwave):**
1. **ECM jam residual pack** (`host_ecm_jam`):
   - ECMTankMissileJammer: PrimaryDamageRadius **150**, PrimaryDamage **100**,
     AttackRange **15**, MinAttackRange **10**, Delay **650**ms → **20**f,
     DamageType SUBDUAL_MISSILE, FireFX FX_ECMTankMissileJammerPulse.
   - ECMTankVehicleDisabler: AttackRange **200**, PrimaryDamage **24**,
     Delay **100**ms → **3**f, DamageType SUBDUAL_VEHICLE, Laser ECMDisableStream,
     FireSound FrequencyJammerWeaponLoop.
   - FireWeaponUpdate ExclusiveWeaponDelay **1000**ms → **30**f.
   - SubdualDamageCap **600**, HealRate **500**ms → **15**f, HealAmount **50**.
   - Vehicle list residual: China / Tank_ / Nuke_ / Infa_ ChinaTankECM.
   - KindOf vehicle-disabler filter (ground vehicle, not aircraft).
   - Honesty: `honesty_ecm_jam_residual_pack_ok` + radius/weapon/subdual/list tests.
2. **Hacker disable residual pack** (`host_hacker_disable`):
   - EffectDuration **2000**ms → **60**f, StartAbilityRange **150**,
     ReloadTime **500**ms → **15**f.
   - UnpackTime **7300**ms → **219**f, PackTime **5133**ms → **154**f,
     PreparationTime **3000**ms → **90**f, PersistentPrepTime **333**ms → **10**f.
   - SpecialObject BinaryDataStream / DisableFX DisabledEffectBinaryShower0.
   - Weapon HackerDisableBuildingHack AttackRange **75**, DamageType HACK.
   - SuperweaponCashHack SCIENCE_CashHack tiers: default **1000**,
     tier2 **2000**, tier3 **4000**, Reload **240000**ms → **7200**f.
   - Honesty: `honesty_hacker_disable_residual_pack_ok` + cash-hack tier tests.
3. **Pathfinder residual pack** (`host_pathfinder`):
   - SCIENCE_Pathfinder gate residual (SCIENCE_AMERICA + SCIENCE_Rank3).
   - StealthUpdate: StealthDelay **0**, InnateStealth **Yes**, Forbidden **MOVING**,
     FriendlyOpacity **30–80%**, PulseFrequency **500**ms → **15**f,
     MoveThresholdSpeed **3**, OrderIdleEnemiesOnReveal **Yes**.
   - StealthDetectorUpdate: DetectionRate **500**ms → **15**f,
     CanDetectWhileGarrisoned/Contained **No**, detect range → Vision **200**.
   - Sniper AP upgrade DAMAGE **125%**; no pack/unpack residual.
   - Honesty: `honesty_pathfinder_residual_pack_ok` + stealth-while-attacking tests.
4. **GPS Scrambler grow-radius residual** (`host_gps_scrambler` thin peel):
   - GrantStealth StartRadius **20**, FinalRadius **100**, GrowRate **10**/frame →
     **8** grow updates to final.
   - Reload **240000**ms → **7200**f (Slth **180000**ms → **5400**f).
   - OCL SUPERWEAPON_GPSScrambler → GPSScrambler_InvisibleMarker.
   - Honesty: `honesty_gps_scrambler_residual_pack_ok` + grow sequence tests.
5. Tests / gates (not log-only):
   - ecm_jam / hacker / pathfinder residual honesty suites
   - golden_skirmish_gate --frames 8 → `playable_claim=true` **PASS**
   - shell_smoke_gate → `playable_claim=false` / `shell_host_playable_ok=true` **PASS**

**Still residual (fail-closed, not claimed):**
- Full subdual damage accumulate / SubdualDamageHelper heal drain / laser attach
- Full SpecialAbilityUpdate BinaryDataStream continuous prep / CashHack floating text
- Full StealthUpdate pulse / FriendlyOpacity drawable interleave / science UI graph
- Full GPSScrambler particle GPU path / flashAsSelected
- Network residual replication (network deferred)

## Residual Host Playability — Wave 51: host_mines + host_emp_pulse residual peels (2026-07-13)
**Closed (host-testable residual outside special_power_strikes / graphics / host_frenzy / host_propaganda / host_heal):**
1. **Mines residual pack** (`host_mines`):
   - DemoTrapUpdate: DefaultProximityMode **Yes**, TriggerDetonationRange **40**,
     DetonateWhenKilled **Yes**, AutoDetonationWithFriendsInvolved **Yes**,
     DestructionDelay **1000**ms → **30**f.
   - Chem/Demo dual-ring honesty: Standard secondary **400**/r**50**, Chem **250**/r**25** +
     **100**/r**50**, Demo **700**/r**25** + **500**/r**50**.
   - DozerMineDisarmingWeapon: AttackRange **5**, PreAttack **1200**ms → **36**f,
     ClipReload **4000**ms → **120**f, ContinueAttackRange **100**.
   - WorkerMineDisarmingWeapon: PreAttack/Delay **1000**ms → **30**f.
   - Burton MaxSpecialObjects: RemoteC4 **8**, TimedC4 **10**, Unique targets **Yes**,
     UnpackTime **5500**ms → **165**f, FleeRange **100**.
   - SUPERWEAPON_ClusterMines OCL: DropVariance **20/20/0**, DeliveryDistance **140**,
     Decal/Cursor **100**, DistanceAroundObject **80**, NumVirtualMines **8**,
     ClusterMinesBomb → ChinaClusterMine, Reload **240000**ms → **7200**f.
   - Honesty: `honesty_mines_residual_pack_ok` + per-layer honesty tests.
   - Fail-closed: not full MinefieldBehavior regen / SmartBorder OCL aircraft path.
2. **EMP Pulse residual pack** (`host_emp_pulse`):
   - SuperweaponEMPPulse RadiusCursor **200**, Reload **360000**ms → **10800**f,
     DisabledDuration **30000**ms → **900**f.
   - OCL: ChinaJetCargoPlane / EMPPulseBomb / EMPPulseEffectSpheroid,
     DropVariance **20/20/0**, DeliveryDistance **150**, DecalRadius **200**.
   - EMPUpdate spheroid: Lifetime **3000**ms → **90**f, StartFade **300**ms → **9**f,
     StartScale **0.01**, TargetScaleMin/Max **3.0**/**4.0**,
     StartColor **R32 G64 B255**, EndColor black, EMPSparks FX.
   - EMP_HARDENED name residual markers expanded (cargo plane / B52 / B3 / A10 /
     Spectre / carpet bomber / napalm MIG / SUPW Patriot).
   - Activate honesty counters retained (`record_activation` path).
   - Honesty: `honesty_emp_pulse_residual_pack_ok` + spheroid/OCL/hardened tests.
   - Fail-closed: not full cargo plane flight / spheroid GPU scale-tint / EMPSparks volume.
3. Tests (not log-only):
   - `mines_residual_pack_honesty` / demo trap / mine clear / burton max / cluster OCL
   - `emp_pulse_residual_pack_honesty` / spheroid / OCL / hardened / activate counters
   - golden_skirmish_gate --frames 8 → `playable_claim=true` **PASS**
   - shell_smoke_gate → `playable_claim=false` / `shell_host_playable_ok=true` **PASS**

**Still residual (fail-closed, not claimed):**
- Full DemoTrapUpdate PreAttack scoop / weapon-lock UI matrix
- Full GenerateMinefieldBehavior SmartBorder / virtual-mine regen slots
- Full EMPPulseEffectSpheroid drawable scale/tint GPU + EMPSparks particle volume
- Full OCL ChinaJetCargoPlane flight path / HeightDie bomb delivery
- Network residual replication (network deferred)

## Residual Host Playability — Wave 53: presentation_frame + mesh_asset + shell residual peels (2026-07-13)
**Closed (host-testable residual outside special_power_strikes / host_* combat packs):**
1. **PresentationFrame ground-height residual** (`presentation_frame`):
   - Default Line3D skim **0.0** when map height missing.
   - Optional `GameLogic::terrain_height_at` sample override + honesty path.
   - `PresentationLaserBeam.ground_height` / `ground_height_from_terrain` frozen fields.
   - Synthetic override: `synthetic_assist_pair_with_ground`.
2. **Laser multi-beam soft-edge presentation fields** (`presentation_frame`):
   - `PresentationLaserSoftEdge` residual (NumBeams **12**, Inner **0.6**, Outer **26**,
     ScrollRate **-1.75**, TilingScalar **0.15**, EXNoise02.tga).
   - `synthetic_orbital_soft_edge` + `pack_endpoints` wire to
     `LaserSegmentUpload::pack_orbital_multi_beam_soft_edge`.
   - Patriot assist beams remain single-beam (`soft_edge=None`) honesty.
3. **Floating-text vanish-rate alpha residual** (`presentation_frame`):
   - `vanish_alpha_at` / `lift_y_at` / `age_frames_at` presentation field honesty.
   - Retail: timeout **10**f, move-up **1.0**, vanish-rate **0.1**.
4. **World-anim fade residual** (`presentation_frame`):
   - `fade_alpha_at` + `PRESENTATION_WORLD_ANIM_FADE_WINDOW_SECONDS` **1.0**.
   - MoneyPickUp fade curve honesty (display 4.0s / ZRise 15 / Fades Yes).
5. **Dual-tick residual counters** (`presentation_frame`):
   - `PresentationDualTickResidual` builds/applies + content counts.
   - `build_and_apply_*` notes apply; honesty self-consistency checks.
6. **mesh_asset_resolve residual peels**:
   - Expanded common unit model_key table (USA/China/GLA top ZH host units).
   - Placeholder last-keys **ring buffer** capacity **32** (drop oldest).
   - `W3D_SEARCH_ROOT_RESIDUALS` honesty (W3DZH/Art/W3D, Art/W3D, tools).
   - Mesh scale residual default **1.0** (ThingTemplate Scale not ported).
7. **shell_smoke residual honesty gates** (playable_claim **stays false**):
   - `dual_tick_counters_ok`, `laser_presentation_residual_ok`,
     `floating_text_vanish_ok`, `world_anim_fade_ok`,
     `anim2d_collection_residual_ok`, `translate_copy_residual_ok`.
8. Tests / gates (not log-only):
   - presentation_frame lib: **21** ok
   - mesh_asset_resolve lib: **13** ok
   - shell_smoke lib: **4** ok
   - golden_skirmish_gate --frames 8 → `playable_claim=true` **PASS**
   - shell_smoke_gate → `playable_claim=false` / `shell_host_playable_ok=true` **PASS**

**Still residual (fail-closed, not claimed):**
- Full HeightMap bilinear / bridge-aware laser ground skim
- Full W3DLaserDraw multi-beam additive GPU soft-edge / texture atlas sample
- Full DisplayString vanish-rate live surface blend
- Full WORLD_ANIM_FADE_ON_EXPIRE GPU Anim2D draw
- Full W3D material / animation / ThingTemplate Scale INI parity
- Network residual replication (network deferred)

## Residual Host Playability — Wave 52: Frenzy / Propaganda / Repair residual packs (2026-07-13)
**Closed (host-testable residual outside special_power_strikes / graphics):**
1. **Frenzy residual pack** (`host_frenzy`):
   - Damage multipliers residual **110% / 120% / 130%** (FRENZY_ONE/TWO/THREE).
   - BonusDuration residual **10000 / 20000 / 30000** ms → **300 / 600 / 900** frames.
   - BonusRange / RadiusCursorRadius residual **200** per level.
   - Science tier gate residual `SCIENCE_Frenzy1/2/3` + Early_* map.
   - OCL `Frenzy_InvisibleMarker_Level1/2/3` + ParticleSysBone **FrenzyCloud**.
   - RequiredAffectKindOf **CAN_ATTACK** / ForbiddenAffectKindOf **STRUCTURE**.
   - Marker DeletionUpdate Min/MaxLifetime **1** msec → **1** frame (one pulse).
   - WeaponBonusCondition discriminants FRENZY_ONE/TWO/THREE = **24/25/26**.
   - Superweapon ReloadTime **240000** ms → **7200** frames.
   - Honesty: `honesty_frenzy_residual_ok` / `frenzy_residual_pack_honesty`.
   - Fail-closed: not full OCL marker spawn / FrenzyCloud particle / TINT_STATUS_FRENZY.
2. **Propaganda residual pack** (`host_propaganda`):
   - Radius **150**, DelayBetweenUpdates **2000** ms → **60** frames.
   - HealPercentEachSecond **2%** / Upgraded **4%** residual honesty.
   - ENTHUSIASTIC / SUBLIMINAL residual discriminants **8 / 15** + ROF **125%**.
   - Sole-benefactor residual map (first-tower-wins; multi-tower reject).
   - UpgradeRequired = `Upgrade_ChinaSubliminalMessaging` residual.
   - PulseFX residual names FX_PropagandaTowerPropagandaPulse / SubliminalPulse.
   - Honesty: `honesty_propaganda_residual_ok` / `propaganda_residual_pack_honesty` /
     `sole_benefactor_first_tower_wins_residual_honesty`.
   - Fail-closed: not full ObjectTracker influence / PulseFX world-anim / POWERED gate.
3. **Repair residual pack** (`host_repair` + `host_emergency_repair`):
   - Dozer RepairHealthPercentPerSecond **2%** residual (`dozer_repair_hp_per_sec`).
   - RepairDockUpdate TimeForFullHeal **5000** ms → **150** f (`repair_dock_hp_per_sec`).
   - NumberApproachPositions **5**; TechRepairPad template residual name.
   - Emergency Repair science SCIENCE_EmergencyRepair1/2/3 + amounts **100/200/300**.
   - Emergency Repair ReloadTime **240000** ms; marker DeletionUpdate lifetime **0**.
   - OCL RepairVehiclesInArea_InvisibleMarker_Level* + RepairCloud + KindOf=VEHICLE.
   - Honesty: `honesty_repair_residual_ok` / `honesty_emergency_repair_residual_ok` /
     `repair_residual_pack_honesty` / `emergency_repair_residual_pack_honesty`.
   - Fail-closed: not full multi-dozer reject / dock bones / OCL marker spawn.
4. Tests (not log-only):
   - `frenzy_residual_pack_honesty` (lib frenzy)
   - `propaganda_residual_pack_honesty` + sole-benefactor (lib propaganda)
   - `repair_residual_pack_honesty` + `emergency_repair_residual_pack_honesty` (lib repair)
   - golden_skirmish_gate --frames 8 → `playable_claim=true` **PASS**
   - shell_smoke_gate → `playable_claim=false` / `shell_host_playable_ok=true` **PASS**

**Still residual (fail-closed, not claimed):**
- Full Frenzy OCL InvisibleMarker spawn / FrenzyCloud particle / drawable tint
- Full PropagandaTowerBehavior ObjectTracker multi-tower / PulseFX world-anim
- Full DozerAIUpdate percent heal wired into GameLogic update path (constants ready)
- Full RepairDockUpdate dock bones / TimeForFullHeal wired runtime path
- Full Emergency Repair OCL marker object + RepairCloud particle
- Network residual replication (network deferred)

## Residual Host Playability — Wave 49: Sentry/PointDefense/Overlord residual packs (2026-07-13)
**Closed (host-testable residual outside special_power_strikes / graphics):**
1. **SentryDrone residual pack** (`host_sentry_drone`):
   - DetectionRange **225** honesty + DetectionRate **900**ms → **27** frames.
   - SentryDroneGun: PrimaryDamage **8**, range **150**, Delay **200**ms → **6**f,
     PrimaryDamageRadius **0**, WeaponSpeed **600**.
   - DeployStyleAIUpdate PackTime/UnpackTime **1000**ms → **30** frames each;
     TurretsFunctionOnlyWhenDeployed / MustCenterBeforePack / AutoAcquire residual.
   - StealthUpdate StealthDelay **2000**ms → **60** frames re-cloak;
     ForbiddenConditions FIRING_PRIMARY + MOVING residual names.
   - Honesty: `honesty_sentry_drone_residual_ok` / `sentry_drone_residual_pack_honesty`.
   - Fail-closed: not full pack/unpack state machine / IR detector FX / ExtraRequiredKindOf.
2. **PointDefenseLaser residual pack** (`host_point_defense`):
   - ScanRate / ScanRange / PredictTargetVelocityFactor residual per carrier:
     - Paladin: ScanRange **120**, ScanRate **500**ms→**15**f, Predict **3.0**.
     - Avenger: ScanRange **200** (fixed from fire×1.2), ScanRate 0/100ms→**3**f, Predict **1.0**.
     - King Raptor: ScanRange **200**, ScanRate 10/0ms→**0**f, Predict **2.0**.
     - Combat Chinook: ScanRange **250**, ScanRate **33**ms→**1**f, Predict **1.0**.
   - PrimaryTargetTypes BALLISTIC_MISSILE SMALL_MISSILE; Secondary INFANTRY (Paladin only).
   - Weapon residual PrimaryDamage **100** / Delay frames retained + honesty.
   - Honesty: `honesty_point_defense_residual_ok` /
     `point_defense_scan_predict_residual_honesty`.
   - Fail-closed: not full velocity-seeker math / laser drawable / TERTIARY WeaponStore.
3. **Overlord addon residual pack** (`host_overlord_addons`):
   - Addon slot table: Gattling **1200**/20s, Propaganda **500**/10s (SpeakerTower),
     Bunker **400**/15s + OCL / payload template residual names.
   - ConflictsWith residual exclusivity matrix (only one portable addon).
   - OverlordContain Slots **1** PORTABLE_STRUCTURE; bunker infantry slots **5**;
     HelixContain Slots **5**; ProductionUpdate MaxQueueEntries **1**.
   - Honesty: `honesty_overlord_addons_residual_ok` /
     `overlord_addon_slot_conflicts_residual_honesty`.
   - Fail-closed: not full OCL passenger object / W3D bone attach / ContinuousFire anim.
4. **Comanche rocket-pod clip residual** (`host_comanche_rocket_pods`, optional room):
   - ClipSize **20**, ClipReload **30000**ms → **900** frames.
   - ScatterTargetScalar **50** + 20-entry ScatterTarget residual table.
   - `rocket_pod_scatter_offset` host index residual.
   - Honesty: `honesty_comanche_rocket_pod_clip_residual_ok` /
     `comanche_rocket_pod_scatter_clip_residual_honesty`.
   - Fail-closed: not full projectile spawn per offset / GameLogicRandom shuffle.
5. Tests (not log-only):
   - `sentry_drone_residual_pack_honesty` (lib sentry_drone: **7**)
   - `point_defense_scan_predict_residual_honesty` (lib point_defense: **9**)
   - `overlord_addon_slot_conflicts_residual_honesty` (lib overlord: **15**)
   - `comanche_rocket_pod_scatter_clip_residual_honesty`
   - golden_skirmish_gate --frames 8 → `playable_claim=true` **PASS**
   - shell_smoke_gate → `playable_claim=false` / `shell_host_playable_ok=true` **PASS**

**Still residual (fail-closed, not claimed):**
- Full DeployStyleAIUpdate pack state machine / turret-only-when-deployed anim
- Full PointDefenseLaserUpdate live velocity prediction seeker
- Full OverlordContain OCL portable-structure passenger / W3DDependencyModelDraw
- Full Comanche ScatterTarget projectile volley spawn stream
- Network residual replication (network deferred)

## Residual Host Playability — Wave 48: Ambulance Vehicle AutoHeal + SpySatellite/RadarVan DynamicShroud (2026-07-13)
**Closed (host-testable residual outside special_power_strikes / graphics):**
1. **Ambulance vehicle AutoHeal residual** (`host_heal`):
   - ModuleTag_23 AmericaVehicleMedic: HealingAmount **5**, HealingDelay **1000**ms,
     Radius **100**, KindOf=VEHICLE, ForbiddenKindOf=AIRCRAFT, SkipSelfForHealing=Yes.
   - ModuleTag_22 infantry residual retained: amount **4** / delay **1000**ms / radius **100**.
   - `is_legal_ambulance_vehicle_heal_target` residual (vehicle && !aircraft).
   - TransportContain residual: Slots **3**, HealthRegen%PerSec **25** while embarked,
     DamagePercentToUnits **10%**.
   - Sole-benefactor residual map (ObjectId → healer_id, first-healer-wins per pulse).
   - `update_ambulance_auto_heal` applies infantry + ground-vehicle heals with exclusivity.
   - Honesty: `honesty_ambulance_auto_heal_constants_ok` / vehicle legality matrix /
     transport regen / sole-benefactor exclusivity tests.
   - Fail-closed: not full multi-module pulse phase / particle pulse FX / network.
2. **SpySatellite DynamicShroud residual** (`host_spy_satellite`):
   - OCL SpySatellitePing: VisionRange **300**, FinalVision **0**.
   - GrowDelay **0** / GrowTime **1000**ms→**30**f / GrowInterval **10**ms→**1**f.
   - ShrinkDelay **10000**ms→**300**f / ShrinkTime **5000**ms→**150**f /
     ChangeInterval **80**ms→**3**f.
   - DeletionUpdate lifetime **13000**ms→**390**f retained.
   - StealthDetector DetectionRate **500**ms→**15**f; DetectionRange 0 → VisionRange **300**.
   - Grow/sustain/shrink radius curve residual + activate application counters.
   - Honesty: `honesty_spy_satellite_dynamic_shroud_constants_ok` /
     `honesty_dynamic_shroud_host_path_ok` / grow-shrink curve tests.
   - Fail-closed: not full OCL Object spawn / GridDecalTemplate GPU / setShroudClearingRange.
3. **RadarVanPing DynamicShroud residual** (`host_radar_scan`, parallel peel):
   - VisionRange **150**, FinalVision **0**, no grow (instant full).
   - ShrinkDelay **7500**ms→**225**f / ShrinkTime **2500**ms→**75**f /
     ChangeInterval **50**ms→**2**f.
   - StealthDetector DetectionRate **500**ms→**15**f; range = VisionRange **150**.
   - Honesty: `honesty_radar_scan_dynamic_shroud_constants_ok` / shrink curve residual.
   - Fail-closed: not full OCL RadarVanPing Object / grid decal GPU.
4. Tests (not log-only):
   - `ambulance_vehicle_auto_heal_constants_residual_honesty`
   - `legal_vehicle_heal_target_matrix_vehicle_vs_infantry_vs_aircraft`
   - `ambulance_transport_health_regen_residual_honesty`
   - `sole_benefactor_first_healer_wins_residual_honesty`
   - `spy_satellite_dynamic_shroud_constants_residual_honesty`
   - `spy_satellite_dynamic_shroud_grow_shrink_curve_residual`
   - `radar_scan_dynamic_shroud_constants_residual_honesty`
   - `radar_scan_dynamic_shroud_shrink_curve_residual`
   - host_heal / host_spy_satellite / host_radar_scan green
   - golden_skirmish_gate --frames 8 → `playable_claim=true` **PASS**
   - shell_smoke_gate → `playable_claim=false` / `shell_host_playable_ok=true` **PASS**

**Still residual (fail-closed, not claimed):**
- Full multi-ambulance pulse phase / particle heal FX / TransportContain door matrix
- Full SpySatellitePing / RadarVanPing Object spawn + setShroudClearingRange live curve
- Full GridDecalTemplate additive decal GPU path
- Network residual replication (network deferred)

## Residual Host Playability — Wave 50: PUC Outer-Node Flares + Death Pack + Laser Texture Bind + Gattling ROF Counters (2026-07-13)
**Closed (host-testable residual not covered by waves 41–47):**
1. **PUC OuterNodes flare particle system residual pack** (`special_power_strikes`):
   - OuterNodesLight/Medium/Intense = `ParticleUplinkCannon_OuterNode*Flare`.
   - LaserBaseLightFlareParticleSystemName = `ParticleUplinkCannon_LaserBaseReadyToFire`.
   - Connector Medium/Intense laser names + OrbitalLaser name residual.
   - Intensity → flare/connector name table residual.
   - Pack armed on beam STATUS_FIRING spawn (`outer_node_flare_pack_armed`).
   - Honesty: `honesty_particle_outer_node_flare_pack` /
     `honesty_beam_outer_node_flare_pack_ok`.
   - Fail-closed: not full ParticleSystemManager spawn / W3D bone-world FX attach.
2. **PUC SlowDeath / InstantDeath residual pack** (`special_power_strikes`):
   - SlowDeath ExemptStatus **UNDER_CONSTRUCTION**, DestructionDelay **2000** ms → **60** frames.
   - INITIAL FX `FX_ParticleUplinkDeathInitial` / OCL `OCL_SDILinkLasers`.
   - FINAL FX `FX_StructureMediumDeath` / OCL `OCL_ParticleUplinkDeathFinal`.
   - InstantDeath RequiredStatus UNDER_CONSTRUCTION: OCL `OCL_ABPowerPlantExplode`
     + FX `FX_StructureMediumDeath`.
   - Pack armed on beam spawn (`death_pack_armed`).
   - Honesty: `honesty_particle_uplink_death_pack` /
     `honesty_particle_uplink_death_pack_ok`.
   - Fail-closed: not full SlowDeathBehavior multi-stage / Object die matrix.
3. **Laser soft-edge texture bind residual pack** (`laser_segment_upload`):
   - EXNoise02.tga (OrbitalLaser) / EXBinaryStream32.tga (BinaryDataStream) /
     EXLaser.tga (connector) bind name residual.
   - MaxIntensityLifetime / FadeLifetime residual defaults **0**.
   - SoftnessDepth / SoftnessDistance residual: **not** W3DLaserDraw INI fields
     (soft edge = multi-beam width/alpha lerp only).
   - `gpu_upload_ready` is host flag only — does **not** claim live
     `wgpu::Queue::write_buffer`.
   - Honesty: `honesty_laser_texture_bind_pack` /
     `honesty_gpu_write_buffer_not_claimed` /
     `laser_soft_edge_texture_bind_pack_residual_honesty`.
4. **Gattling ContinuousFire WeaponBonus ROF application counters**
   (`special_power_strikes`):
   - MEAN ROF **200%** / FAST ROF **300%** residual application counters on orbit ticks.
   - ContinuousFireOne=**1** / ContinuousFireTwo=**2** exclusive thresholds.
   - Honesty: `honesty_gattling_weapon_bonus_rof` /
     `honesty_gattling_weapon_bonus_rof_ok`.
   - Fail-closed: not full FiringTracker WeaponBonusConditionFlags combat matrix.
5. Snapshot/Xfer: `OuterNodeFlarePackArmed` / `DeathPackArmed` on HostParticleBeamField;
   `GattlingRofMeanApplications` / `GattlingRofFastApplications` on HostSpectreOrbitField.
6. Tests (not log-only):
   - `particle_uplink_outer_node_flare_pack_residual_honesty`
   - `particle_uplink_slow_death_instant_death_residual_honesty`
   - `spectre_gattling_weapon_bonus_rof_application_residual_honesty`
   - `laser_soft_edge_texture_bind_pack_residual_honesty`
   - special_power_strikes (**90**) + laser_segment (**9**) green
   - golden_skirmish_gate --frames 8 → `playable_claim=true` **PASS**
   - shell_smoke_gate → `playable_claim=false` / `shell_host_playable_ok=true` **PASS**

**Still residual (fail-closed, not claimed):**
- Full Miles audio event playback / 3D positional PUC sound loops
- Full SlowDeathBehavior multi-stage / Object die / live OCL spawn matrix
- Full ParticleSystemManager outer-node FX attach / W3D bone-world extract
- Full OuterBeamWidth multi-beam GPU soft edge / texture atlas sample bind
- Actual `wgpu::Queue::write_buffer` against a live device/pipeline
- Full ScudStormMissile ThingFactory Object / live MissileAIUpdate physics flight
- Full SpectreHowitzerShell ThingFactory Object / W3D ModelDraw shell drawable
- Full TrailRemnant ThingFactory ImmortalBody / live DeletionUpdate module stack
- Network residual replication (network deferred)

## Residual Host Playability — Wave 47: DeliverPayload DropVariance + VisiblePayload A10 Rack + Crate Geometry Pack + Patriot PunchThroughScalar (2026-07-13)
**Closed (host-testable residual outside special_power_strikes / graphics):**
1. **DropVariance residual** (`host_deliver_payload`):
   - Supply Drop Zone OCL has **no** DropVariance → zero residual (X/Y/Z **0**).
   - ClusterMines / EMPPulse residual: X:**20** Y:**20** Z:**0**.
   - CarpetBomb residual: X:**30** Y:**40** Z:**0**.
   - Apply residual: axes with variance **> 0** add clamped sample `[-1,1] * variance`
     (C++ `GameLogicRandomValueReal(-var,+var)` host unit-sample residual).
   - Honesty: `honesty_drop_variance_ok` / `apply_drop_variance`.
   - Fail-closed: not full GameLogic RNG stream / live OCL parse matrix.
2. **VisiblePayload A10 bomb-rack residual** (`host_deliver_payload`):
   - VisibleNumBones **6**, VisibleItemsDroppedPerInterval **2**.
   - VisibleDropBoneBaseName `WeaponA` → `WeaponA01`…`WeaponA06`.
   - VisibleSubObjectBaseName `Missile` → hide `Missile01`… residual.
   - VisiblePayloadTemplateName `A10ThunderboltMissile` /
     VisiblePayloadWeaponTemplate `A10ThunderboltMissileWeapon`.
   - Rack interval residual empties 6 slots over 3 intervals.
   - Honesty: `honesty_visible_payload_ok` / `HostVisiblePayloadRack`.
   - Fail-closed: not full ThingFactory payload spawn / W3D showSubObject GPU rack.
3. **SupplyDropZoneCrate geometry pack residual** (`host_deliver_payload`):
   - Geometry BOX Major/Minor **12**, Height **12**, IsSmall **Yes**, Mass **75**.
   - MoneyProvided **250**, TransportSlotCount **1**.
   - No ActiveBody MaxHealth residual (collide crate honesty).
   - Honesty: `honesty_crate_geometry_pack_ok`.
4. **Patriot LaserUpdate PunchThroughScalar residual** (`host_base_defense`):
   - Retail PunchThroughScalar **1.3** (PatriotBinaryDataStream / LaserGeneral).
   - Dead/missing target: end = start + (end−start)×**1.3**, then clear to_id.
   - Honesty: `honesty_patriot_laser_punch_through_constants_ok` /
     `punch_through_laser_end` / `patriot_laser_punch_through_scalar_residual_honesty`.
   - Fail-closed: not full LaserUpdate drawable id / bone parent matrix.
5. Tests (not log-only):
   - `drop_variance_residual_honesty`
   - `visible_payload_a10_rack_residual_honesty`
   - `supply_drop_crate_geometry_pack_residual_honesty`
   - `wave47_deliver_payload_residual_cluster_honesty`
   - `patriot_laser_punch_through_scalar_residual_honesty`
   - host_deliver_payload (**18**) + host_base_defense (**16**) green
   - golden_skirmish_gate --frames 8 → `playable_claim=true` **PASS**
   - shell_smoke_gate → `playable_claim=false` / `shell_host_playable_ok=true` **PASS**

**Still residual (fail-closed, not claimed):**
- Full GameLogic RNG DropVariance stream / live OCL DeliverPayload parse
- Full VisiblePayload ThingFactory spawn / W3D pristine bone extract GPU
- Full AmericaCrateParachute container Object / pathfinder CreateAtEdge Object
- Full LaserUpdate bone parent / WGPU SegLine punch-through draw path
- Full Comanche tertiary weapon slot / OverlordContain capacity matrix
- Network residual replication (network deferred)

## Residual Host Playability — Wave 45: PUC Sound/Scorch Pack + PointDefense Lifetime + FlammableUpdate (2026-07-13)
**Closed (host-testable residual not covered by wave 44 SupW/DeletionUpdate residual):**
1. **PUC sound residual pack** (`special_power_strikes`):
   - PoweringUpSoundLoop / UnpackToIdleSoundLoop / FiringToPackSoundLoop /
     GroundAnnihilationSoundLoop retail name residual.
   - BeamLaunchFX **FX_ParticleUplinkCannon_BeamLaunchIteration** +
     DelayBetweenLaunchFX **1000 ms → 30** frames.
   - GroundHitFX **FX_ParticleUplinkCannon_BeamHitsGround**.
   - Honesty counters: powerup on STATUS_CHARGING, unpack on PREPARING,
     annihilation + firing-to-pack + sound pack armed on beam spawn.
   - Honesty: `honesty_particle_sound_loops` / `honesty_beam_sound_residual_ok`.
   - Fail-closed: not full Miles 3D positional loop / stop on POSTFIRE.
2. **Scorch residual pack** (`special_power_strikes`):
   - ScorchMarkScalar **2.4** (scorch radius = laser_r × scalar × width).
   - SwathOfDeathDistance **200** / Amplitude **50** residual.
   - ManualDrivingSpeed **20** / ManualFastDrivingSpeed **40** /
     DoubleClickToFastDriveDelay **500 ms → 15** frames.
   - TotalScorchMarks **20** + GroundHitFX cadence residual.
   - Honesty: `honesty_particle_scorch_pack` / strengthened `honesty_beam_scorch_ok`.
   - Fail-closed: not full TheGameClient::addScorch GPU decal path.
3. **PointDefense laser LifetimeUpdate residual** (`special_power_strikes`):
   - SupW_PointDefenseDroneLaserBeam / PointDefenseLaserBeam
     MinLifetime=MaxLifetime **95** ms → ceil(95×30/1000) = **3** frames
     (`duration_ms_to_logic_frames` / `ConvertDurationFromMsecsToFrames`).
   - Honesty: `honesty_point_defense_laser_lifetime`.
   - Fail-closed: not full LifetimeUpdate destroyObject / ThingFactory laser Object.
4. **PUC FlammableUpdate residual** (`special_power_strikes`):
   - AflameDuration **5000** ms → **150** frames.
   - AflameDamageAmount **5** / AflameDamageDelay **500** ms → **15** frames.
   - Honesty: `honesty_particle_uplink_flammable`.
   - Fail-closed: not full aflame object status bit / live DoT module.
5. Snapshot/Xfer: beam sound residual + scorch_scalar_pack_armed fields appended
   on HostParticleBeamField; strike powerup/unpack audio default residual fields.
6. Tests (not log-only):
   - `particle_uplink_sound_residual_pack_honesty`
   - `particle_uplink_scorch_pack_residual_honesty`
   - `point_defense_laser_lifetime_update_residual_honesty`
   - `particle_uplink_flammable_update_residual_honesty`
   - special_power_strikes lib tests **87** green
   - golden_skirmish_gate --frames 8 → `playable_claim=true` **PASS**
   - shell_smoke_gate → `playable_claim=false` / `shell_host_playable_ok=true` **PASS**

**Still residual (fail-closed, not claimed):**
- Full Miles audio event playback / 3D positional PUC sound loops
- Full LifetimeUpdate destroyObject on PointDefense laser dieFrame
- Full FlammableUpdate aflame status / live damage-over-time on PUC building
- Full multi-locale CSF/STR GameText table load for all LanguageId at runtime boot UI
- Full Anim2DCollection GPU texture atlas / DisplayString font raster draw
- Full OuterBeamWidth multi-beam GPU soft edge / texture atlas submit
- Full ScudStormMissile ThingFactory Object / live MissileAIUpdate physics flight
- Full SpectreHowitzerShell ThingFactory Object / W3D ModelDraw shell drawable
- Full W3D bone-extract outer-node / connector LaserUpdate GPU drawables
- Full TrailRemnant ThingFactory ImmortalBody / live DeletionUpdate module stack
- Network residual replication (network deferred)

## Residual Host Playability — Wave 46: Anim2DCollection + GameText translateCopy + DisplayString Deepen (2026-07-13)
**Closed (host-testable residual not covered by wave 44 collection/status residual):**
1. **Anim2DCollection residual** (`world_anim_layout`):
   - Template list head-insert `newTemplate` / `findTemplate` by name residual.
   - `registerAnimation` / `unRegisterAnimation` doubly-linked instance list residual.
   - `update()` skips `tryNextFrame` when `ANIM_2D_STATUS_FROZEN` set.
   - `init` residual path constant `Data/INI/Animation2D.ini` (fail-closed vs full INI parse).
   - MoneyPickUp `RandomizeStartFrame = No` residual.
   - `getCurrentFrameWidth`/`Height` monospaced placeholder size residual (32×32).
   - Honesty: `honesty_anim2d_collection_residual`.
   - Fail-closed: not full Anim2DCollection GPU texture atlas / Image draw.
2. **GameText translateCopy residual** (`game_text_residual`):
   - Backslash escape table matching C++ `GameTextManager::translateCopy`
     (`\n` newline, `\t` tab, `\\` backslash, `\'` `\"` `\?`, default keep char).
   - Honesty: `honesty_translate_copy_escape_table`.
   - Fail-closed: not Jabber reverseWord debug residual / multi-locale boot UI.
3. **DisplayString deepen residual** (`floating_text_layout`):
   - `usingResources(frame)` residual — last-used frame stamp when draw succeeds.
   - `computeExtents` residual honesty: empty/no-font → (0,0); non-empty monospaced.
   - Hotkey Build_Sentence residual: when useHotkey, extract `&` letter + monospaced pos.
   - Honesty: `honesty_display_string_deepen_residual`.
   - Fail-closed: not full Render2DSentence GPU StretchRect / hotkey underline draw.
4. Tests (not log-only):
   - `anim2d_collection_residual_honesty`
   - `translate_copy_escape_table_residual_honesty`
   - `display_string_using_resources_and_hotkey_residual_honesty`
   - golden_skirmish_gate --frames 8 → `playable_claim=true` **PASS**
   - shell_smoke_gate → `playable_claim=false` / `shell_host_playable_ok=true` **PASS**

**Still residual (fail-closed, not claimed):**
- Full multi-locale CSF/STR GameText table load for all LanguageId at runtime boot UI
- Full Anim2DCollection GPU texture atlas / DisplayString font raster draw
- Full OuterBeamWidth multi-beam GPU soft edge / texture atlas submit
- Full ScudStormMissile ThingFactory Object / live MissileAIUpdate physics flight
- Full SpectreHowitzerShell ThingFactory Object / W3D ModelDraw shell drawable
- Full W3D bone-extract outer-node / connector LaserUpdate GPU drawables
- Full TrailRemnant ThingFactory ImmortalBody / live DeletionUpdate module stack
- Network residual replication (network deferred)

## Residual Host Playability — Wave 44: SupW Magenta OuterColor + DeletionUpdate Sleep + GameText MISSING Fetch + DisplayStringManager Link + Anim2D Status/Alpha (2026-07-13)
**Closed (host-testable residual not covered by wave 43 special/gattling/immortal residual):**
1. **SupW ParticleUplink magenta OuterColor residual** (`special_power_strikes`):
   - SupW connector/orbital OuterColor **R:255 G:0 B:255 A:150** (magenta).
   - Normal residual OuterColor remains blue **R:0 G:0 B:255 A:150**.
   - Object names SupW_ParticleUplinkCannon_* residual.
   - Honesty: `honesty_particle_supw_outer_color`.
   - Fail-closed: not full SupW ThingFactory laser drawable / GPU SegLine submit.
2. **DeletionUpdate calcSleepDelay residual** (`special_power_strikes`):
   - TrailRemnant MinLifetime==MaxLifetime **4000** ms → **120** frames fixed.
   - Random range residual when min≠max; clamp delay **≥1**.
   - Honesty: `honesty_deletion_update_sleep_delay`.
   - Fail-closed: not full ThingFactory destroyObject on dieFrame.
3. **GameText fetch missing-label residual** (`game_text_residual`):
   - Miss → `MISSING: 'label'` + exists=false; hit → value + exists=true.
   - Missing-string list de-dupe residual (`m_noStringList`).
   - Honesty: `honesty_game_text_fetch_missing`.
   - Fail-closed: not full multi-locale CSF boot UI.
4. **DisplayStringManager link/unlink residual** (`floating_text_layout`):
   - Head-insert doubly-linked factory list residual.
   - Honesty: `honesty_display_string_manager_link`.
   - Fail-closed: not full W3DDisplayStringManager GPU pool.
5. **Anim2D status bits + setAlpha residual** (`world_anim_layout`):
   - NONE/FROZEN/REVERSED/COMPLETE bit flags; set/clear/test residual.
   - Default alpha **1.0**; draw color alpha = **255 × m_alpha**.
   - Honesty: `honesty_anim2d_status_alpha`.
   - Fail-closed: not full Anim2DCollection GPU atlas draw.
6. Tests (not log-only):
   - `particle_supw_outer_color_residual_honesty`
   - `deletion_update_sleep_delay_residual_honesty`
   - `game_text_fetch_missing_residual_honesty`
   - `display_string_manager_link_residual_honesty`
   - `anim2d_status_alpha_residual_honesty`
   - golden_skirmish_gate --frames 8 → `playable_claim=true` **PASS**
   - shell_smoke_gate → `playable_claim=false` / `shell_host_playable_ok=true` **PASS**

**Still residual (fail-closed, not claimed):**
- Full multi-locale CSF/STR GameText table load for all LanguageId at runtime boot UI
- Full Anim2DCollection GPU texture atlas / DisplayString font raster draw
- Full OuterBeamWidth multi-beam GPU soft edge / texture atlas submit
- Full ScudStormMissile ThingFactory Object / live MissileAIUpdate physics flight
- Full SpectreHowitzerShell ThingFactory Object / W3D ModelDraw shell drawable
- Full W3D bone-extract outer-node / connector LaserUpdate GPU drawables
- Full TrailRemnant ThingFactory ImmortalBody / live DeletionUpdate module stack
- Network residual replication (network deferred)

## Residual Host Playability — Wave 43: Scud Special + Gattling Params + MissileAI Defaults + ImmortalBody Floor + DisplayString getFont/Draw + Anim2DMode Table (2026-07-13)
**Closed (host-testable residual not covered by wave 41 launch/anti/getSize residual):**
1. **ScudStormWeapon special residual** (`special_power_strikes`):
   - PrimaryDamage **0** / PrimaryDamageRadius **0** / AttackRange **999999**.
   - DamageType **EXPLOSION** / DeathType **EXPLODED** / WeaponSpeed **99999**.
   - ScatterRadius **0** / PreAttackType **PER_CLIP** / PreAttackDelay **3000** ms → **90** frames.
   - ProjectileDetonationFX **ScudStormMissileDetonation** / RadiusDamageAffects ALLIES/ENEMIES/NEUTRALS.
   - Honesty: `honesty_scud_weapon_special_ok`.
   - Fail-closed: not full WeaponTemplate store / live pad launch matrix.
2. **SpectreGattlingGun anti/fire residual** (`special_power_strikes`):
   - AntiAirborne*/AntiMissile **No**, AntiGround **Yes**, ProjectileObject **NONE**.
   - PrimaryDamageRadius **0**, DamageType **Gattling**, DeathType **NORMAL**, WeaponSpeed **999999**.
   - ClipSize **0** / ClipReloadTime **0** / DelayBetweenShots **100** ms / AttackRange **2222**.
   - Honesty: `honesty_gattling_gun_params_ok`.
   - Fail-closed: not full WeaponTemplate store / live gattling turret matrix.
3. **MissileAIUpdate defaults residual** (`special_power_strikes`):
   - IgnitionDelay **0**, UseWeaponSpeed **No**, DetonateOnNoFuel **No**.
   - DistanceToTargetForLock **75**, DistanceScatterWhenJammed **75**.
   - DetonateCallsKill **No**, KillSelfDelay **3** frames (C++ module defaults).
   - Honesty: `honesty_scud_missile_ai_defaults_ok`.
   - Fail-closed: not full MissileAIUpdate state machine / live fuel/jam path.
4. **TrailRemnant ImmortalBody health-floor residual** (`special_power_strikes`):
   - Floor **1** HP (`internalChangeHealth` clamp) / never-dead residual.
   - Honesty: `honesty_beam_remnant_immortal_body_ok` / `immortal_body_apply_health_delta`.
   - Fail-closed: not full ThingFactory ImmortalBody / Object death flag stack.
5. **DisplayString getFont + draw residual** (`floating_text_layout`):
   - getFont identity residual; draw empty early-out; default drop **(1,1)**.
   - Shadow-then-text order residual (`shadow_x = x+xDrop`); pos/color rebuild dirty.
   - Honesty: get_font / draw residual tests.
   - Fail-closed: not full Render2DSentence GPU StretchRect / hotkey underline.
6. **Anim2DMode full residual table** (`world_anim_layout`):
   - Discriminants INVALID=0..PING_PONG_BACKWARDS=6 + Anim2DModeNames[] residual.
   - Default template mode LOOP; MoneyPickUp AnimationMode LOOP residual.
   - Honesty: `honesty_anim2d_mode_table` / anim2d_mode_table_residual_honesty.
   - Fail-closed: not full ONCE/PING_PONG live Anim2D reverse state machine / GPU atlas.
7. Snapshot/Xfer: scud special/defaults + gattling gun params + remnant immortal residual fields.
8. Tests (not log-only):
   - `scud_weapon_special_residual_honesty`
   - `spectre_gattling_gun_params_residual_honesty`
   - `scud_missile_ai_defaults_residual_honesty`
   - `particle_uplink_remnant_immortal_body_residual_honesty`
   - `display_string_get_font_residual_honesty` / `display_string_draw_residual_honesty`
   - `anim2d_mode_table_residual_honesty`
   - all `special_power_strikes::` (**81**) green
   - floating_text (**16**) + world_anim (**7**) + game_text (**7**) green
   - golden_skirmish_gate --frames 8 → `playable_claim=true` **PASS**
   - shell_smoke_gate → `playable_claim=false` / `shell_host_playable_ok=true` **PASS**

**Still residual (fail-closed, not claimed):**
- Full multi-locale CSF/STR GameText table load for all LanguageId at runtime boot UI
- Full Anim2DCollection GPU texture atlas / DisplayString font raster draw
- Full OuterBeamWidth multi-beam GPU soft edge / texture atlas submit
- Full ScudStormMissile ThingFactory Object / live MissileAIUpdate physics flight
- Full SpectreHowitzerShell ThingFactory Object / W3D ModelDraw shell drawable
- Full W3D bone-extract outer-node / connector LaserUpdate GPU drawables
- Full TrailRemnant ThingFactory ImmortalBody / live DeletionUpdate module stack
- Network residual replication (network deferred)

## Residual Host Playability — Wave 42: ScudStormWeapon Special + SpectreGattlingGun Params + DisplayString getFont/draw + Anim2D Mode Residual (2026-07-13)
**Closed (host-testable residual not covered by wave 41 launch/anti/getSize residual):**
1. **ScudStormWeapon special residual** (`special_power_strikes`):
   - PrimaryDamage **0**, PrimaryDamageRadius **0**, AttackRange **999999**.
   - DamageType **EXPLOSION**, DeathType **EXPLODED**, WeaponSpeed **99999**.
   - ScatterRadius **0**, PreAttackType **PER_CLIP**, PreAttackDelay **3000** ms → **90** frames.
   - Honesty: `honesty_scud_weapon_special_ok` (application counter on impact wave).
   - Fail-closed: not full WeaponTemplate store / live pad launch matrix.
2. **SpectreGattlingGun anti/fire residual** (`special_power_strikes`):
   - AntiAirborneVehicle/Infantry **No**, AntiSmallMissile/AntiBallisticMissile **No**.
   - AntiGround **Yes**, ProjectileObject **NONE**, PrimaryDamageRadius **0**.
   - DamageType **Gattling**, DeathType **NORMAL**, WeaponSpeed **999999**, AttackRange **2222**.
   - FireFX / VeterancyFireFX residual, ClipSize **0**, ClipReloadTime **0**, DelayBetweenShots **100** ms.
   - Honesty: `honesty_gattling_gun_params_ok` (application counter on gattling tick).
   - Fail-closed: not full WeaponTemplate anti matrix / live hitscan aim.
3. **DisplayString getFont / draw residual** (`floating_text_layout`):
   - getFont identity residual.
   - draw empty early-out; default drop shadow **(1,1)**; rebuild on text/font/pos/color dirty.
   - draw_with_drop explicit offset residual.
   - Honesty: get_font / draw residual tests.
   - Fail-closed: not full FontCharsClass / WW3D StretchRect / live hotkey underline draw.
4. **Anim2D mode residual** (`world_anim_layout`):
   - ONCE / ONCE_BACKWARDS / LOOP / LOOP_BACKWARDS / PING_PONG / PING_PONG_BACKWARDS.
   - Host `anim2d_try_next_frame` matches C++ `Anim2D::tryNextFrame` residual.
   - Honesty: `honesty_anim2d_mode_residual` + unit test.
   - Fail-closed: not full Anim2DCollection GPU texture atlas / WW3D Image draw.
5. Snapshot/Xfer: scud weapon special default + gattling gun params residual fields appended.
6. Tests (not log-only):
   - `scud_weapon_special_residual_honesty`
   - `spectre_gattling_gun_params_residual_honesty`
   - `display_string_get_font_residual_honesty`
   - `display_string_draw_residual_honesty`
   - `anim2d_mode_residual_honesty`
   - all `special_power_strikes::` green
   - residual_honesty suite green
   - golden_skirmish_gate --frames 8 → `playable_claim=true` **PASS**
   - shell_smoke_gate → `playable_claim=false` / `shell_host_playable_ok=true` **PASS**

**Still residual (fail-closed, not claimed):**
- Full multi-locale CSF/STR GameText table load for all LanguageId at runtime boot UI
- Full Anim2DCollection GPU texture atlas / DisplayString font raster draw
- Full OuterBeamWidth multi-beam GPU soft edge / texture atlas submit
  (host residual packs NumBeams width/color/UV/additive/tiled/premul; combat still r50)
- Full ScudStormMissile ThingFactory Object / live MissileAIUpdate physics flight
- Full SpectreHowitzerShell ThingFactory Object / W3D ModelDraw shell drawable
- Full W3D bone-extract outer-node / connector LaserUpdate GPU drawables
- Full TrailRemnant ThingFactory ImmortalBody / live DeletionUpdate module stack
- Network residual replication (network deferred)

## Residual Host Playability — Wave 41: ScudStormWeapon Launch + Howitzer Gun Anti + DisplayString getSize/WordWrap/Hotkey/Clip + Multi-locale STR Paths (2026-07-13)
**Closed (host-testable residual not covered by wave 39 death-damage/fire-params/remnant residual):**
1. **ScudStormWeapon launch residual** (`special_power_strikes`):
   - ClipSize **9**, ClipReloadTime **10000** ms → **300** frames, AutoReloadsClip **Yes**.
   - ScatterTargetScalar **120**, ScatterTarget count **9**, AcceptableAimDelta **180**.
   - ProjectileCollidesWith **STRUCTURES**, ProjectileObject **ScudStormMissile**.
   - DelayBetweenShots Min/Max **100**/**1000** ms → **3**/**30** frames.
   - Death weapon ClipReloadTime **0** residual.
   - Honesty: `honesty_scud_weapon_launch_ok` (application counter on impact wave).
   - Fail-closed: not full WeaponTemplate store / live pad reload matrix.
2. **SpectreHowitzerGun anti residual** (`special_power_strikes`):
   - AntiAirborneVehicle/Infantry **No**, AntiSmallMissile/AntiBallisticMissile **No**.
   - AntiGround **Yes**, ProjectileObject **SpectreHowitzerShell**.
   - ContinuousFireCoast **2000** ms, ContinuousFireOne/Two **1**/**2**.
   - VeterancyFireFX residual (HEROIC GenericTankGunNoTracer).
   - Honesty: `honesty_howitzer_gun_anti_params_ok` (application counter on orbit tick).
   - Fail-closed: not full WeaponTemplate anti matrix / live turret aim.
3. **DisplayString getSize / setWordWrap / setWordWrapCentered / setUseHotkey / setClipRegion residual** (`floating_text_layout`):
   - getSize monospaced width×height; empty/no-font → 0×0.
   - setWordWrap / setWordWrapCentered change → notify residual.
   - setUseHotkey always notifies with flag+color residual.
   - setClipRegion region equality early-out residual.
   - Honesty: `honesty_display_string_get_size` / word-wrap / hotkey / clip residual tests.
   - Fail-closed: not full FontCharsClass / WW3D StretchRect / live hotkey underline draw.
4. **Multi-locale LanguageId STR path residual table** (`game_text_residual`):
   - generals.str / map.str relatives for all 10 LanguageId discriminants.
   - English-family residual includes W3DEnglishZH paths; Jabber/Unknown share English.
   - Honesty: multi_locale_str_path_residual_table (10 locales).
   - Fail-closed: not full multi-locale STR boot UI for all LanguageId assets.
5. Snapshot/Xfer: scud weapon launch default + howitzer gun anti residual fields appended.
6. Tests (not log-only):
   - `scud_weapon_launch_residual_honesty`
   - `spectre_howitzer_gun_anti_params_residual_honesty`
   - `display_string_get_size_residual_honesty`
   - `display_string_set_word_wrap_residual_honesty`
   - `display_string_set_use_hotkey_and_clip_residual_honesty`
   - `multi_locale_str_path_residual_table` (10 locales)
   - all `special_power_strikes::` (**77**) green
   - residual_honesty suite (**70**) green
   - golden_skirmish_gate --frames 8 → `playable_claim=true` **PASS**
   - shell_smoke_gate → `playable_claim=false` / `shell_host_playable_ok=true` **PASS**

**Still residual (fail-closed, not claimed):**
- Full multi-locale CSF/STR GameText table load for all LanguageId at runtime boot UI
- Full Anim2DCollection GPU texture atlas / DisplayString font raster draw
- Full OuterBeamWidth multi-beam GPU soft edge / texture atlas submit
  (host residual packs NumBeams width/color/UV/additive/tiled/premul; combat still r50)
- Full ScudStormMissile ThingFactory Object / live MissileAIUpdate physics flight
- Full SpectreHowitzerShell ThingFactory Object / W3D ModelDraw shell drawable
- Full W3D bone-extract outer-node / connector LaserUpdate GPU drawables
- Full TrailRemnant ThingFactory ImmortalBody / live DeletionUpdate module stack
- Network residual replication (network deferred)

## Residual Host Playability — Scud Death Damage Table + Howitzer Gun Fire Params + Remnant Fire/Deletion + DisplayString Text Ops + Full LanguageId Table (2026-07-13)
**Closed (host-testable residual not covered by wave 38 FireOCL/aim/getTextLength residual):**
1. **ScudStormDamageWeapon damage table residual** (`special_power_strikes`):
   - PrimaryDamage **500** / PrimaryRadius **50** / Secondary **150**/**200** / SecondaryRadius **200**.
   - DamageType **EXPLOSION**, DeathType **EXPLODED**, WeaponSpeed **600**, AttackRange **200**,
     FireFX **ScudStormMissileDetonation**, RadiusDamageAffects **ALLIES ENEMIES NEUTRALS**,
     DelayBetweenShots **0**, ClipSize **0**.
   - Honesty: `honesty_scud_death_damage_table_ok`.
   - Fail-closed: not full FireWeaponWhenDeadBehavior exclusive module matrix.
2. **SpectreHowitzerGun fire params residual** (`special_power_strikes`):
   - PrimaryDamage **80** / radius **25**, DelayBetweenShots **777** ms (frames **23**),
     DamageType **EXPLOSION**, DeathType **EXPLODED**, RadiusDamageAffects ALLIES/ENEMIES/NEUTRALS,
     FireFX/FireSound/DetonationFX residual, ClipSize **0**, ClipReloadTime **0**,
     ShellLocomotor GroupMovementPriority **MOVES_BACK**.
   - Distinct from HowitzerFiringRate **300** ms orbit residual cadence.
   - Honesty: `honesty_howitzer_gun_fire_params_ok`.
   - Fail-closed: not full WeaponTemplate store / live turret fire matrix.
3. **TrailRemnant FireWeaponUpdate + DeletionUpdate residual** (`special_power_strikes`):
   - FireWeaponUpdate Weapon **ParticleUplinkCannonBeamTrailRemnantWeapon**.
   - PrimaryDamage **15** / radius **10** / DelayBetweenShots **250** ms → **7** frames.
   - DamageType **PARTICLE_BEAM**, DeathType **BURNED**, WeaponSpeed **250**,
     RadiusDamageAffects **ALLIES ENEMIES NEUTRALS**.
   - DeletionUpdate MinLifetime **4000** == MaxLifetime **4000** → **120** frames.
   - Honesty: `honesty_beam_remnant_fire_deletion_ok`.
   - Fail-closed: not full ThingFactory ImmortalBody / live DeletionUpdate module stack.
4. **DisplayString text ops residual** (`floating_text_layout`):
   - getText identity residual; reset clears text+font without notify.
   - appendChar / removeLastChar mutate + notifyTextChanged residual.
   - getWidth monospaced charPos residual (skip `\n`; 8px glyph).
   - Honesty: `honesty_display_string_get_text` / `reset` / `append_char` / `remove_last_char` / `get_width`.
   - Fail-closed: not full FontCharsClass spacing / WW3D StretchRect draw.
5. **Full LanguageId residual table** (`game_text_residual`):
   - Discriminants US=0 UK=1 German=2 French=3 Spanish=4 Italian=5 Japanese=6 Jabber=7 Korean=8 Unknown=9.
   - Japanese/Korean path tables residual; Jabber/Unknown fail-closed English pack paths.
   - Honesty: multi_locale_path_count **10**.
   - Fail-closed: not full multi-locale CSF boot UI for all LanguageId assets.
6. Snapshot/Xfer: scud death damage table default + howitzer gun fire params + remnant fire/deletion residual fields appended.
7. Tests (not log-only):
   - `scud_death_damage_table_residual_honesty`
   - `spectre_howitzer_gun_fire_params_residual_honesty`
   - `particle_uplink_remnant_fire_deletion_residual_honesty`
   - `display_string_get_text_and_reset_residual_honesty`
   - `display_string_append_remove_char_residual_honesty`
   - `display_string_get_width_residual_honesty`
   - `multi_locale_csf_path_residual_table` (10 locales)
   - all `special_power_strikes::` (**75**)
   - graphics residual tests green
   - golden_skirmish_gate --frames 8 → `playable_claim=true`
   - shell_smoke_gate → `playable_claim=false` / `shell_host_playable_ok=true`

**Still residual (fail-closed, not claimed):**
- Full multi-locale CSF/STR GameText table load for all LanguageId at runtime boot UI
- Full Anim2DCollection GPU texture atlas / DisplayString font raster draw
- Full OuterBeamWidth multi-beam GPU soft edge / texture atlas submit
  (host residual packs NumBeams width/color/UV/additive/tiled/premul; combat still r50)
- Full ScudStormMissile ThingFactory Object / live MissileAIUpdate physics flight
- Full SpectreHowitzerShell ThingFactory Object / W3D ModelDraw shell drawable
- Full W3D bone-extract outer-node / connector LaserUpdate GPU drawables
- Full TrailRemnant ThingFactory ImmortalBody / live DeletionUpdate module stack
- Network residual replication (network deferred)

## Residual Host Playability — Scud FireOCL/Speed Table + Howitzer Aim Params + getTextLength + UK Locale (2026-07-13)
**Closed (host-testable residual not covered by wave 36/37 DestroyDie/loco/connector residual):**
1. **ScudStormMissile DeathWeapon FireOCL residual** (`special_power_strikes`):
   - Base FireOCL **OCL_PoisonFieldLarge**, upgraded **OCL_PoisonFieldUpgradedLarge**.
   - Honesty: `honesty_scud_death_fire_ocl_ok`.
   - Fail-closed: not full FireWeaponWhenDead OCL spawn Object path.
2. **SCUDStormMissileLocomotor SpeedDamaged/MinSpeed/MaxThrustAngle residual** (`special_power_strikes`):
   - SpeedDamaged **200**, MinSpeed **100**, MaxThrustAngle **45** (plus Speed/Accel/TurnRate honesty).
   - Honesty: `honesty_scud_locomotor_speed_table_ok`.
   - Fail-closed: not full Locomotor thrust motive force matrix.
3. **SpectreHowitzerGun AcceptableAimDelta/AttackRange residual** (`special_power_strikes`):
   - AcceptableAimDelta **180**, AttackRange **2222**, ProjectileCollidesWith **STRUCTURES WALLS**,
     AntiGround **Yes**.
   - Honesty: `honesty_howitzer_gun_aim_params_ok`.
   - Fail-closed: not full WeaponTemplate store / live turret aim matrix.
4. **DisplayString getTextLength residual** (`floating_text_layout`):
   - C++ `m_textString.getLength()` → host char-count residual.
   - Honesty: `honesty_display_string_get_text_length`.
   - Fail-closed: not full UTF-16 WideChar length on live Display surface.
5. **Multi-locale LANGUAGE_ID_UK residual** (`game_text_residual`):
   - LanguageId UK residual maps to English CSF pack paths (retail UK share).
   - Discriminant residual US=0 UK=1 German=2 …; path table **6** locales.
   - Honesty: multi_locale_path_count **6** / `ResidualLanguageId::Uk`.
   - Fail-closed: not full multi-locale CSF boot UI for all LanguageId assets.
6. Snapshot/Xfer: scud FireOCL + speed table / howitzer gun aim residual fields appended.
7. Tests (not log-only):
   - `scud_death_fire_ocl_and_speed_table_residual_honesty`
   - `spectre_howitzer_gun_aim_params_residual_honesty`
   - `display_string_get_text_length_residual_honesty`
   - `multi_locale_csf_path_residual_table` (UK)
   - all `special_power_strikes::` (**72**)
   - graphics residual tests green
   - golden_skirmish_gate --frames 8 → `playable_claim=true`
   - shell_smoke_gate → `playable_claim=false` / `shell_host_playable_ok=true`

**Still residual (fail-closed, not claimed):**
- Full multi-locale CSF/STR GameText table load for all LanguageId at runtime boot UI
- Full Anim2DCollection GPU texture atlas / DisplayString font raster draw
- Full OuterBeamWidth multi-beam GPU soft edge / texture atlas submit
  (host residual packs NumBeams width/color/UV/additive/tiled/premul; combat still r50)
- Full ScudStormMissile ThingFactory Object / live MissileAIUpdate physics flight
- Full SpectreHowitzerShell ThingFactory Object / W3D ModelDraw shell drawable
- Full W3D bone-extract outer-node / connector LaserUpdate GPU drawables
- Network residual replication (network deferred)

## Residual Host Playability — Scud DestroyDie/Loco Name + Howitzer Shell Loco + Connector KindOf Defaults + TrailRemnant + DisplayString setFont (2026-07-13)
**Closed (host-testable residual not covered by wave 35 FireWeaponWhenDead/body/loco residual):**
1. **ScudStormMissile DestroyDie + Locomotor template name residual** (`special_power_strikes`):
   - Empty DestroyDie module residual present on ScudStormMissile.
   - Locomotor SET_NORMAL template name **SCUDStormMissileLocomotor**.
   - ArmorSet DamageFX **None**.
   - Honesty: `honesty_scud_destroy_die_locomotor_name_ok`.
   - Fail-closed: not full DestroyDie Object / Locomotor store matrix.
2. **SpectreHowitzerShellLocomotor template residual** (`special_power_strikes`):
   - Surfaces **AIR**, Appearance **THRUST**, MinSpeed **1111**, Accel **9160**,
     TurnRate **99999**, MaxThrustAngle **90**, Braking **0**, AllowAirborne Yes.
   - ArmorSet DamageFX **None** residual.
   - Honesty: `honesty_howitzer_shell_locomotor_template_ok` / `honesty_howitzer_shell_damage_fx_ok`.
   - Fail-closed: not full Locomotor store / live motive force (Object comments out Locomotor).
3. **Connector KindOf IMMOBILE + MaxIntensity/Fade/Tile residual** (`special_power_strikes`):
   - Medium/Intense connectors: KindOf **IMMOBILE**, Segments **1**, ArcHeight **0**,
     MaxIntensityLifetime **0**, FadeLifetime **0**, Tile **No**.
   - Honesty: `honesty_beam_connector_kindof_defaults_ok`.
   - Fail-closed: not full LaserUpdate GPU drawable / ThingFactory connector Object.
4. **ParticleUplinkCannonTrailRemnant KindOf/ImmortalBody residual** (`special_power_strikes`):
   - KindOf **NO_COLLIDE UNATTACKABLE IMMOBILE**, ImmortalBody Max/Initial **50**,
     EditorSorting **SYSTEM**.
   - Honesty: `honesty_beam_remnant_object_params_ok`.
   - Fail-closed: not full ThingFactory Object / DeletionUpdate module stack.
5. **DisplayString setFont residual** (`floating_text_layout`):
   - Equal font early-out / different font m_fontChanged residual.
   - Empty/NULL font fail-closed early return residual.
   - Honesty: `honesty_display_string_set_font`.
   - Fail-closed: not full FontCharsClass re-raster / WW3D StretchRect.
6. **Connector/Orbital W3DLaserDraw omitted-field defaults residual** (`laser_segment_upload`):
   - MaxIntensityLifetime **0**, FadeLifetime **0**, connector Tile **No**, Segments **1**, ArcHeight **0**.
   - Honesty: `honesty_connector_laser_defaults`.
   - Fail-closed: not full LaserUpdate drawable lifetime / fade-delete path.
7. **MoneyPickUp ExecuteAnimationTime/ZRise/Fades residual** (`world_anim_layout`):
   - DisplayTime **4.0s**, ZRise **15**, Fades **Yes**, fade window **1.0s**.
   - Honesty: `honesty_money_pickup_fade_params` / pack alpha mid-fade residual.
   - Fail-closed: not full WORLD_ANIM_FADE_ON_EXPIRE live Display blend.
8. Snapshot/Xfer: howitzer locomotor template + damage FX / connector kindof defaults /
   remnant object params residual fields appended.
9. Tests (not log-only):
   - `scud_destroy_die_locomotor_name_residual_honesty`
   - `spectre_howitzer_shell_locomotor_template_residual_honesty`
   - `particle_uplink_connector_kindof_defaults_residual_honesty`
   - `particle_uplink_remnant_object_params_residual_honesty`
   - `display_string_set_font_residual_honesty`
   - `connector_laser_defaults_residual_honesty`
   - `money_pickup_fade_params_residual_honesty`
   - all `special_power_strikes::` (**70**)
   - graphics residual tests green
   - golden_skirmish_gate --frames 8 → `playable_claim=true`
   - shell_smoke_gate → `playable_claim=false` / `shell_host_playable_ok=true`

**Still residual (fail-closed, not claimed):**
- Full multi-locale CSF/STR GameText table load for all LanguageId at runtime boot UI
- Full Anim2DCollection GPU texture atlas / DisplayString font raster draw
- Full OuterBeamWidth multi-beam GPU soft edge / texture atlas submit
  (host residual packs NumBeams width/color/UV/additive/tiled/premul; combat still r50)
- Full ScudStormMissile ThingFactory Object / live MissileAIUpdate physics flight
- Full SpectreHowitzerShell ThingFactory Object / W3D ModelDraw shell drawable
- Full W3D bone-extract outer-node / connector LaserUpdate GPU drawables
- Network residual replication (network deferred)

## Residual Host Playability — Scud DestroyDie/Locomotor Name + Howitzer Locomotor Template + Connector KindOf Defaults + Remnant ImmortalBody + DisplayString setFont + MoneyPickUp Fade (2026-07-13)
**Closed (host-testable residual not covered by wave 34–35 FireWeaponWhenDead/body/loco residual):**
1. **ScudStormMissile DestroyDie + Locomotor name + Armor DamageFX residual** (`special_power_strikes`):
   - Empty DestroyDie module residual present.
   - Locomotor SET_NORMAL template name **SCUDStormMissileLocomotor**.
   - ArmorSet DamageFX **None**.
   - Honesty: `honesty_scud_destroy_die_locomotor_name_ok`.
   - Fail-closed: not full DestroyDie Object / Locomotor store matrix.
2. **SpectreHowitzerShellLocomotor template residual** (`special_power_strikes`):
   - Surfaces **AIR**, Appearance **THRUST**, MinSpeed **1111**, Accel **9160**,
     TurnRate **99999**, MaxThrustAngle **90**, Braking **0**, AllowAirborne Yes.
   - Honesty: `honesty_howitzer_shell_locomotor_template_ok`.
   - Fail-closed: not full Locomotor store / live motive force (Object comments out Locomotor).
3. **SpectreHowitzerShell Armor DamageFX residual** (`special_power_strikes`):
   - ArmorSet DamageFX **None**.
   - Honesty: `honesty_howitzer_shell_damage_fx_ok`.
   - Fail-closed: not full DamageFXStore path.
4. **Connector KindOf IMMOBILE + Segments/MaxIntensity/Fade/Tile residual** (`special_power_strikes` + `laser_segment_upload`):
   - KindOf **IMMOBILE**, Segments **1**, MaxIntensityLifetime **0**, FadeLifetime **0**, Tile **No**.
   - Honesty: `honesty_beam_connector_kindof_defaults_ok` / `honesty_connector_laser_defaults`.
   - Fail-closed: not full LaserUpdate GPU drawable / fade-delete path.
5. **TrailRemnant KindOf + ImmortalBody residual** (`special_power_strikes`):
   - KindOf **NO_COLLIDE UNATTACKABLE IMMOBILE**, ImmortalBody MaxHealth **50**,
     InitialHealth **50**, EditorSorting **SYSTEM**.
   - Honesty: `honesty_beam_remnant_object_params_ok`.
   - Fail-closed: not full ThingFactory Object / ImmortalBody / DeletionUpdate stack.
6. **DisplayString setFont residual** (`floating_text_layout`):
   - Equal font early-out / different font m_fontChanged residual.
   - Honesty: `honesty_display_string_set_font`.
   - Fail-closed: not full FontCharsClass re-raster / hotkey underline font.
7. **MoneyPickUp ExecuteAnimation fade residual** (`world_anim_layout`):
   - DisplayTime **4.0s**, ZRise **15**, Fades **Yes**, fade window **1.0s**.
   - Honesty: `honesty_money_pickup_fade_params`.
   - Fail-closed: not full Anim2DCollection GPU / WORLD_ANIM_FADE_ON_EXPIRE Display blend.
8. Snapshot/Xfer: howitzer locomotor template + damage FX / connector KindOf defaults /
   remnant object params residual fields appended.
9. Tests (not log-only):
   - `scud_destroy_die_locomotor_name_residual_honesty`
   - `spectre_howitzer_shell_locomotor_template_residual_honesty`
   - `particle_uplink_connector_kindof_defaults_residual_honesty`
   - `particle_uplink_remnant_object_params_residual_honesty`
   - `display_string_set_font_residual_honesty`
   - `money_pickup_fade_params_residual_honesty`
   - `connector_laser_defaults_residual_honesty`
   - all `special_power_strikes::` (**70**)
   - graphics residual tests green
   - golden_skirmish_gate --frames 8 → `playable_claim=true`
   - shell_smoke_gate → `playable_claim=false` / `shell_host_playable_ok=true`

**Still residual (fail-closed, not claimed):**
- Full multi-locale CSF/STR GameText table load for all LanguageId at runtime boot UI
- Full Anim2DCollection GPU texture atlas / DisplayString font raster draw
- Full OuterBeamWidth multi-beam GPU soft edge / texture atlas submit
  (host residual packs NumBeams width/color/UV/additive/tiled/premul/defaults; combat still r50)
- Full ScudStormMissile ThingFactory Object / live MissileAIUpdate physics flight
- Full SpectreHowitzerShell ThingFactory Object / W3D ModelDraw shell drawable
- Full W3D bone-extract outer-node / connector LaserUpdate GPU drawables
- Full TrailRemnant ThingFactory ImmortalBody / DeletionUpdate module stack
- Network residual replication (network deferred)

## Residual Host Playability — Scud FireWeaponWhenDead/Body/Loco + Howitzer Design/GENERIC Death + Connector Premul + Orbital KindOf/Segments + DisplayString setText (2026-07-13)
**Closed (host-testable residual not covered by wave 33 soft-edge premul/object-params residual):**
1. **ScudStormMissile FireWeaponWhenDead residual** (`special_power_strikes`):
   - Base DeathWeapon **ScudStormDamageWeapon** (StartsActive Yes, ConflictsWith AnthraxBeta).
   - Upgraded DeathWeapon **ScudStormDamageWeaponUpgraded** (StartsActive No, TriggeredBy AnthraxBeta).
   - Honesty: `honesty_scud_fire_weapon_when_dead_ok`.
   - Fail-closed: not full FireWeaponWhenDeadBehavior exclusive module matrix.
2. **ScudStormMissile body/draw + MissileAIUpdate residual** (`special_power_strikes`):
   - InitialHealth **10000**, EditorSorting **SYSTEM**, OkToChangeModelColor Yes, DAMAGED model **NONE**.
   - MissileAIUpdate TryToFollow **No** / FuelLifetime **0** / DistTurning **500** / DistDiving **200**.
   - Honesty: `honesty_scud_body_draw_params_ok` / `honesty_scud_missile_ai_ok`.
   - Fail-closed: not full ActiveBody / live MissileAIUpdate physics Object.
3. **SCUDStormMissileLocomotor Appearance residual** (`special_power_strikes`):
   - Surfaces **AIR**, Appearance **THRUST**, AllowAirborneMotiveForce Yes, Braking **0**.
   - Honesty: `honesty_scud_locomotor_appearance_ok`.
   - Fail-closed: not full Locomotor physics motive force matrix.
4. **SpectreHowitzerShell design-params + InstantDeath GENERIC residual** (`special_power_strikes`):
   - TargetHeightIncludesStructures **No**, InitialHealth **100**, DisplayName **OBJECT:Missile**,
     EditorSorting **SYSTEM**, OkToChangeModelColor Yes.
   - InstantDeath ALL -LASERED -DETONATED → **FX_GenericMissileDeath**.
   - Honesty: `honesty_howitzer_shell_design_params_ok` / `honesty_howitzer_shell_death_generic_ok`.
   - Fail-closed: not full InstantDeathBehavior Object / HeightDie module matrix.
5. **Connector soft-edge RGB innerAlpha premul residual** (`special_power_strikes`):
   - Intense/Medium connector channel-delta × innerAlpha residual.
   - Honesty: `honesty_beam_connector_soft_edge_premul_ok`.
   - Fail-closed: not full LaserUpdate GPU drawable / SegLine submit.
6. **OrbitalLaser KindOf IMMOBILE + Segments/ArcHeight residual** (`special_power_strikes`):
   - KindOf **IMMOBILE**, Segments **1**, ArcHeight **0**, SegmentOverlap **0**.
   - Honesty: `honesty_beam_orbital_kindof_segments_ok`.
   - Fail-closed: not full multi-segment arc LaserUpdate GPU path.
7. **W3DLaserDraw multi-beam pack premul + single-beam RGB×alpha residual** (`laser_segment_upload`):
   - Multi-beam pack uses C++ innerAlpha premultiply on RGB.
   - Single-beam (NumBeams==1) full RGB × innerAlpha residual.
   - Honesty: `honesty_soft_edge_premul_pack_ok` / `honesty_beam_single_beam_premul_ok`.
   - Fail-closed: not live WGPU texture atlas / additive GPU submit.
8. **DisplayString setText residual** (`floating_text_layout`):
   - Equal text early-out / different text notifyTextChanged residual.
   - Honesty: `honesty_display_string_set_text`.
   - Fail-closed: not full DisplayString GPU font atlas re-raster.
9. Snapshot/Xfer: death_generic / design_params / connector premul / orbital kind-segments residual fields appended.
10. Tests (not log-only):
   - `scud_fire_weapon_when_dead_residual_honesty`
   - `scud_body_draw_and_locomotor_appearance_residual_honesty`
   - `scud_missile_ai_residual_honesty`
   - `spectre_howitzer_shell_design_params_residual_honesty`
   - `spectre_howitzer_shell_death_generic_residual_honesty`
   - `particle_uplink_connector_soft_edge_premul_residual_honesty`
   - `particle_uplink_orbital_kindof_segments_residual_honesty`
   - `particle_uplink_single_beam_premul_residual_honesty`
   - `orbital_soft_edge_premul_pack_residual_honesty`
   - `display_string_set_text_residual_honesty`
   - all `special_power_strikes::` (**66**)
   - graphics residual tests green
   - golden_skirmish_gate --frames 8 → `playable_claim=true`
   - shell_smoke_gate → `playable_claim=false` / `shell_host_playable_ok=true`

**Still residual (fail-closed, not claimed):**
- Full multi-locale CSF/STR GameText table load for all LanguageId at runtime boot UI
- Full Anim2DCollection GPU texture atlas / DisplayString font raster draw
- Full OuterBeamWidth multi-beam GPU soft edge / texture atlas submit
  (host residual packs NumBeams width/color/UV/additive/tiled/premul; combat still r50)
- Full ScudStormMissile ThingFactory Object / live MissileAIUpdate physics flight
- Full SpectreHowitzerShell ThingFactory Object / W3D ModelDraw shell drawable
- Full W3D bone-extract outer-node / connector LaserUpdate GPU drawables
- Network residual replication (network deferred)

## Residual Host Playability — Soft-Edge Premul + Additive/Tiled Laser + Object Params + DisplayString Color + MoneyPickUp Sequence (2026-07-13)
**Closed (host-testable residual not covered by wave 32 LaserUpdate/medium/multi-locale residual):**
1. **W3DLaserDraw soft-edge RGB innerAlpha premultiply residual** (`special_power_strikes`):
   - C++ channel-delta × innerAlpha: `red = inner + scale*(outer-inner)*innerAlpha`.
   - Honesty: `honesty_beam_soft_edge_premul_ok` / `particle_orbital_soft_edge_color_premul`.
   - Fail-closed: not full SegLineRenderer additive GPU submit.
2. **W3DLaserDraw additive shader + TILED_TEXTURE_MAP residual** (`laser_segment_upload`):
   - Shader residual `_PresetAdditiveShader`.
   - Texture mapping residual `TILED_TEXTURE_MAP` when Tile=Yes.
   - UV_Offset_Rate residual Vector2(0, ScrollRate).
   - Honesty: `honesty_additive_tiled_ok`.
   - Fail-closed: not live WGPU shader bind / texture atlas sample.
3. **ScudStormMissile VisionRange / KindOf / Armor residual** (`special_power_strikes`):
   - VisionRange **300**, ShroudClearingRange **0**, KindOf PROJECTILE, Armor ProjectileArmor, TransportSlotCount **10**.
   - Honesty: `honesty_scud_object_params_ok`.
   - Fail-closed: not full ThingFactory Object / partition KindOf matrix.
4. **SpectreHowitzerShell KindOf / VisionRange / Armor residual** (`special_power_strikes`):
   - KindOf PROJECTILE, VisionRange **0**, Armor ProjectileArmor.
   - Honesty: `honesty_howitzer_shell_object_params_ok`.
   - Fail-closed: not full ThingFactory Object / ArmorSet module matrix.
5. **MoneyPickUp RandomizeStartFrame + full image sequence residual** (`world_anim_layout`):
   - RandomizeStartFrame **No**; SCPDollar000..030 sequence residual table.
   - Honesty: `honesty_money_pickup_image_sequence`.
   - Fail-closed: not full Anim2DCollection GPU texture atlas sample.
6. **DisplayString color residual** (`floating_text_layout`):
   - Normalize GameMakeColor u8 RGBA → f32 (0..1).
   - Retail green (0,255,0,255) / yellow (255,255,0,255) honesty samples.
   - Honesty: `display_string_color_ok` / `honesty_display_string_color`.
   - Fail-closed: not full DisplayString GPU font atlas / WW3D StretchRect.
7. Snapshot/Xfer: soft-edge premul / scud object params / howitzer object params residual fields appended.
8. Tests (not log-only):
   - `particle_uplink_soft_edge_premul_residual_honesty`
   - `scud_object_params_residual_honesty`
   - `spectre_howitzer_shell_object_params_residual_honesty`
   - `orbital_additive_tiled_texture_residual_honesty`
   - `money_pickup_image_sequence_and_randomize_residual`
   - `display_string_color_residual_normalize`
   - all `special_power_strikes::` (**58**)
   - graphics residual tests green
   - golden_skirmish_gate --frames 8 → `playable_claim=true`
   - shell_smoke_gate → `playable_claim=false` / `shell_host_playable_ok=true`

**Still residual (fail-closed, not claimed):**
- Full multi-locale CSF/STR GameText table load for all LanguageId at runtime boot UI
- Full Anim2DCollection GPU texture atlas / DisplayString font raster draw
- Full OuterBeamWidth multi-beam GPU soft edge / texture atlas submit
  (host residual packs NumBeams width/color/UV/additive/tiled; combat still r50)
- Full ScudStormMissile ThingFactory Object / live MissileAIUpdate physics flight
- Full SpectreHowitzerShell ThingFactory Object / W3D ModelDraw shell drawable
- Full W3D bone-extract outer-node / connector LaserUpdate GPU drawables
- Network residual replication (network deferred)

## Residual Host Playability — LaserUpdate Client + Medium Connector + Multi-Locale CSF Path + Scud Geometry + Howitzer Lasered OCL (2026-07-13)
**Closed (host-testable residual not covered by wave 30–31 connector/thrust/loft residual):**
1. **LaserUpdate client residual** (`special_power_strikes`):
   - `initLaser` ground-to-orbit + orbit-to-target residual (2 applications).
   - Orbit altitude residual **500** (Y-up host / C++ Z-up height).
   - Drawable midpoint residual `(start+end)*0.5`.
   - WidthGrow sizeDelta widen/decay `m_currentWidthScalar` residual + dirty.
   - `getCurrentLaserRadius` residual = OuterBeamWidth×0.5 × scalar (peak **13**).
   - Honesty: `honesty_beam_laser_update_ok`.
   - Fail-closed: not full LaserUpdate GPU drawable / client shroud path.
2. **Medium connector soft-edge residual** (`special_power_strikes`):
   - POSTFIRE Medium intensity NumBeams **4**, Inner **0.4** → Outer **1.2**.
   - Soft-edge scale/color lerp residual + EXLaser.tga texture honesty.
   - Honesty: `honesty_beam_connector_medium_soft_edge_ok`.
   - Fail-closed: not full LaserUpdate drawable matrix / GPU SegLine submit.
3. **OrbitalLaser VisionRange / ShroudClearing residual** (`special_power_strikes`):
   - VisionRange **100** / ShroudClearingRange **120** design params residual.
   - Honesty: `honesty_beam_vision_shroud_ok`.
   - Fail-closed: not full client FOW reveal grid path.
4. **ScudStormMissile Geometry residual** (`special_power_strikes`):
   - Cylinder / GeometryIsSmall / MajorRadius **7** / Height **30** / Mass **500**.
   - Honesty: `honesty_scud_geometry_ok`.
   - Fail-closed: not full ThingFactory Object / partition GeometryInfo matrix.
5. **SpectreHowitzerShell InstantDeath LASERED OCL residual** (`special_power_strikes`):
   - OCL_GenericMissileDisintegrate residual honesty.
   - Honesty: extended `honesty_howitzer_shell_dumb_projectile_ok`.
   - Fail-closed: not full ThingFactory Object / live OCL spawn.
6. **Multi-locale LanguageId CSF path residual** (`game_text_residual`):
   - English/German/French/Spanish/Italian residual path table
     (`Data/<Locale>/generals.csf` + `*ZH` big-file roots).
   - Optional live multi-locale CSF probe when assets present.
   - Honesty: `multi_locale_path_ok` / `exercise_multi_locale_csf_residual`.
   - Fail-closed: not full multi-locale CSF boot UI for all LanguageId assets.
7. Snapshot/Xfer: medium connector / vision-shroud / LaserUpdate residual fields appended.
8. Tests (not log-only):
   - `particle_uplink_laser_update_client_residual_honesty`
   - `particle_uplink_medium_connector_soft_edge_residual_honesty`
   - `particle_uplink_orbital_vision_shroud_residual_honesty`
   - `scud_geometry_residual_honesty`
   - `spectre_howitzer_shell_lasered_ocl_residual_honesty`
   - `game_text_residual::*` (**6**)
   - all `special_power_strikes::` (**55**)
   - golden_skirmish_gate --frames 8 → `playable_claim=true`
   - shell_smoke_gate → `playable_claim=false` / `shell_host_playable_ok=true`

**Still residual (fail-closed, not claimed):**
- Full multi-locale CSF/STR GameText table load for all LanguageId at runtime boot UI
- Full Anim2DCollection GPU texture atlas / DisplayString font raster draw
- Full OuterBeamWidth multi-beam GPU soft edge / texture atlas submit
  (host residual packs NumBeams width/color/UV; combat still r50)
- Full ScudStormMissile ThingFactory Object / live MissileAIUpdate physics flight
- Full SpectreHowitzerShell ThingFactory Object / W3D ModelDraw shell drawable
- Full W3D bone-extract outer-node / connector LaserUpdate GPU drawables
- Network residual replication (network deferred)

## Residual Host Playability — Connector Soft Edge + Scud Thrust Wobble + Howitzer Shell Loft Flight (2026-07-13)
**Closed (host-testable combat residual not covered by wave 29–31 soft-edge/ballistic residual):**
1. **Intense connector soft-edge + laser segments residual** (`special_power_strikes`):
   - Intense connector NumBeams **5**, InnerBeamWidth **0.6** → OuterBeamWidth **2.0**.
   - Soft-edge scale/color lerp residual + EXLaser.tga texture honesty.
   - Outer-node → connector bone laser segments (5 residual segments).
   - Honesty: `honesty_beam_connector_soft_edge_ok`.
   - Fail-closed: not full LaserUpdate drawable matrix / client shroud path.
2. **ScudStormMissile ThrustRoll / ThrustWobble residual** (`special_power_strikes`):
   - ThrustRoll **0.06**, ThrustWobbleRate **0.008**, Min/Max ±0.040.
   - Deterministic sine wobble sample residual on ballistic flight path.
   - CloseEnoughDist3D residual honesty.
   - Honesty: `honesty_scud_thrust_wobble_ok`.
   - Fail-closed: not full Locomotor thrust matrix / Physics motive force.
3. **SpectreHowitzerShell loft flight residual** (`special_power_strikes`):
   - Pad-safe HeightDie InitialDelay (**30**f) loft sample residual.
   - Sink to TargetHeight **1** with OnlyWhenMovingDown + ground impact.
   - Honesty: `honesty_howitzer_shell_loft_flight_ok`.
   - Fail-closed: not full DumbProjectileBehavior Object / live Physics flight.
4. Snapshot/Xfer: connector soft-edge / loft flight residual fields appended.
5. Tests (not log-only):
   - `particle_uplink_connector_soft_edge_residual_honesty`
   - `scud_thrust_wobble_residual_honesty`
   - `spectre_howitzer_shell_loft_flight_residual_honesty`
   - all `special_power_strikes::` (**50**)
   - golden_skirmish_gate --frames 8 → `playable_claim=true`
   - shell_smoke_gate → `playable_claim=false` / `shell_host_playable_ok=true`

**Still residual (fail-closed, not claimed):**
- Full multi-locale CSF/STR GameText table load for all LanguageId at runtime boot
- Full Anim2DCollection GPU texture atlas / DisplayString font raster draw
- Full OuterBeamWidth multi-beam GPU soft edge / texture atlas submit
  (host residual packs NumBeams width/color/UV; combat still r50)
- Full ScudStormMissile ThingFactory Object / live MissileAIUpdate physics flight
- Full SpectreHowitzerShell ThingFactory Object / W3D ModelDraw shell drawable
- Full W3D bone-extract outer-node / connector LaserUpdate GPU drawables
- Network residual replication (network deferred)

## Residual Host Playability — Presentation CSF/STR + Multi-Beam Soft Edge + Anim2D Frame + DisplayString Measure (2026-07-13)
**Closed (host-testable presentation residual not covered by wave 27–29 RNG/combat residual):**
1. **CSF/STR Unicode GameText residual** (`game_text_residual`):
   - Pure STR residual parse matching C++ map-string blocks.
   - Pure CSF residual parse matching `generals.csf` LBL/RTS blocks (UTF-16 xor).
   - Retail English `GUI:AddCash` template residual **`$%d`** + printf-d format → `$N`.
   - Optional live English CSF load when assets present (GUI:Back=BACK honesty).
   - Synthetic CSF fixture path for CI without assets.
   - Honesty: `exercise_host_game_text_residual` / `game_text_csf_str_ok`.
   - Fail-closed: not full multi-locale CSF boot for all LanguageId / live Display surface.
2. **DisplayString monospaced measure residual** (`floating_text_layout` + `game_text_residual`):
   - 8×8 glyph extents residual for packed captions (`+$N` host frozen text).
   - Honesty: `display_string_measure_ok` on shell smoke + layout pack.
   - Fail-closed: not full GPU font atlas raster / WW3D StretchRect submit.
3. **OrbitalLaser multi-beam soft-edge CPU pack residual** (`laser_segment_upload`):
   - NumBeams **12** width/color lerp: Inner 0.6/white → Outer 26/blue.
   - ScrollRate **-1.75** UV + TilingScalar **0.15** tile factor residual.
   - Texture residual `EXNoise02.tga`.
   - Honesty: `honesty_multi_beam_soft_edge_ok` / `multi_beam_soft_edge_ok`.
   - Fail-closed: not live WGPU texture atlas / additive soft-edge shader submit.
4. **MoneyPickUp Anim2D frame advance residual** (`world_anim_layout`):
   - NumberImages **31**, AnimationDelay **30**ms → frames_between **1**, LOOP mode.
   - Frame image residual `SCPDollar000`..`SCPDollar030`.
   - Honesty: `anim2d_frame_ok` / `honesty_money_pickup_frame`.
   - Fail-closed: not full Anim2DCollection GPU texture atlas sample.
5. Shell smoke residual flags (do not flip playable_claim):
   - `multi_beam_soft_edge_ok`, `anim2d_frame_ok`, `game_text_csf_str_ok`,
     `display_string_measure_ok` (plus prior presentation flags).
7. **Co-landed combat residual honesty** (already green in-tree with this wave):
   - Multi-beam soft-edge width/alpha/color lerp residual on beam field
     (`particle_uplink_soft_edge_residual_honesty`).
   - ScudStormMissile ballistic flight residual (locomotor speed/accel +
     OnlyWhenMovingDown/SnapToGround + model UBScudStrm_M).
   - SpectreHowitzerShell W3D ModelDraw residual (AVSpectreShell1 Scale/Shadow/
     MaxHealth/Geometry honesty).
   - Outer-node bone layout residual (FX01..FX05 ring positions host residual).
   - Snapshot/Xfer fields appended for soft-edge / outer-node / ballistic / ModelDraw.

6. Tests (not log-only):
   - `game_text_residual::*` (5)
   - `laser_segment_upload::orbital_multi_beam_soft_edge_residual_honesty`
   - `world_anim_layout::money_pickup_anim2d_frame_advance_residual`
   - updated `floating_text_layout` measure residual
   - shell_smoke (4)
   - golden_skirmish_gate --frames 8 → `playable_claim=true`
   - shell_smoke_gate → `playable_claim=false` / `shell_host_playable_ok=true`

**Still residual (fail-closed, not claimed):**
- Full multi-locale CSF/STR GameText table load for all LanguageId at runtime boot
- Full Anim2DCollection GPU texture atlas / DisplayString font raster draw
- Full OuterBeamWidth multi-beam GPU soft edge / texture atlas submit
  (host residual packs NumBeams width/color/UV; combat still r50)
- Full ScudStormMissile ThingFactory Object / live MissileAIUpdate physics flight
- Full SpectreHowitzerShell ThingFactory Object / W3D ModelDraw shell drawable
- Full W3D bone-extract outer-node / connector LaserUpdate GPU drawables
- Network residual replication (network deferred)

## Residual Host Playability — Once-at-Queue OCL + Scud PreferredHeight Spring + NumBeams Scroll + Helpers RNG Unify (2026-07-13)
**Closed (host-testable combat residual not covered by wave 26–27 OuterBeam/loft/RNG residual):**
1. **Once-at-queue multi-strike OCL residual** (`special_power_strikes`):
   - ArtilleryBarrage / CarpetBomb / ScudStorm store epicenters + absolute shell
     frames on the strike at queue time (pure ADC draws matching retail
     once-at-create GameLogic stream residual).
   - `plan_due_impacts` prefers stored `ocl_points` / `ocl_shell_frames` (falls
     back to re-query for older empty snapshots).
   - Honesty: `honesty_once_at_queue_ocl_ok`.
   - Fail-closed: not live mid-sim global stream mutation / full ChinaArtilleryCannon
     / AmericaJetB52 transport Object.
2. **ScudStormMissile PreferredHeight spring residual** (`special_power_strikes` /
   `Locomotor::locoUpdate_moveTowards`):
   - Spawn height residual = PreferredHeight **240**.
   - Spring residual: `new = current + (preferred - current) * damping(0.7)`.
   - Loft phase residual: Loft → Turn (DistanceBeforeTurning **500**) → Dive
     (DistanceBeforeDiving **200**) → HeightDie (TargetHeight **15**).
   - Per-missile wave counters + peak phase / last spring height honesty.
   - Honesty: `honesty_scud_preferred_height_spring_ok`.
   - Fail-closed: not full ThingFactory Object / live MissileAIUpdate physics.
3. **OuterBeam multi-beam NumBeams + ScrollRate residual** (`special_power_strikes` /
   `W3DLaserDraw`):
   - NumBeams **12** + TilingScalar **0.15** armed at STATUS_FIRING.
   - ScrollRate **-1.75** UV residual sampled each `sample_width_honesty`
     (`ScrollRate * elapsed_seconds`).
   - Honesty: `honesty_beam_num_beams_scroll_ok`.
   - Fail-closed: not full GPU multi-beam soft edge / texture atlas submit.
4. **GameLogic helpers RNG unified with Common stream** (`helpers.rs` /
   `common/random_value.rs`):
   - Removed parallel `GAME_LOGIC_SEED` / `GAME_CLIENT_SEED` in GameLogic helpers.
   - Helpers `get_game_logic_random_value(_real)` / `game_client_random_value(_real)`
     bridge to Common ADC stream.
   - `set_game_logic_random_seed` / CRC read-write Common 6-word seed state.
5. Snapshot/Xfer: NumBeams/Scroll residual fields appended on `HostParticleBeamField`.
6. Tests (not log-only):
   - `once_at_queue_multi_strike_ocl_residual_honesty`
   - `scud_preferred_height_spring_residual_honesty`
   - `particle_uplink_num_beams_scroll_residual_honesty`
   - all `special_power_strikes::` (**43**)
   - golden_skirmish_gate --frames 8 → `playable_claim=true`
   - shell_smoke_gate → `playable_claim=false` / `shell_host_playable_ok=true`

**Still residual (fail-closed, not claimed):**
- Full ScudStormMissile ThingFactory Object / live MissileAIUpdate physics flight
- Full SpectreHowitzerShell ThingFactory Object / W3D ModelDraw shell drawable
- Full W3D bone-extract outer-node / connector LaserUpdate GPU drawables
- Full OuterBeamWidth multi-beam GPU soft edge / texture atlas submit
  (host residual tracks NumBeams + ScrollRate UV; combat still r50)
- Full CSF/STR Unicode GameText table load for all locales
- Full Anim2DCollection GPU / DisplayString font raster draw
- Network residual replication (network deferred)

## Residual Host Playability — RNG Streams + Presentation Caption/WorldAnim (2026-07-13)
**Closed (host-testable presentation/RNG residual not covered by wave 24–25 combat residual):**
1. **GameLogic/GameClient RandomValue ADC stream residual** (`host_rng_residual`):
   - Local `HostRandomState` matches C++ `RandomValue.cpp` add-with-carry algorithm.
   - Pure index-seeded residual for re-query-stable combat scatter/delay:
     WeaponErrorRadius, DelayDelivery, DropVariance, Scud DelayBetweenShots.
   - Live client/logic stream residual helpers + honesty exercise
     (`exercise_host_rng_residual`) verifying pure ADC parity, stream separation,
     seed CRC, structure scatter + error-radius stream draws.
   - Presentation structure scatter (oil derrick / black market / Internet Center)
     uses pure ADC `GameClientRandomValue` integer residual (±0.3 major/minor).
   - GameClient `client_random_value` rewired from `thread_rng` to Common client stream.
2. **GameText `GUI:AddCash` caption residual** (`floating_text_layout`):
   - `resolve_add_cash_caption` → `+$N` format parity with host frozen text.
   - Layout entries carry caption + text_key; honesty `game_text_caption_ok`.
3. **MoneyPickUp Anim2D world-anim CPU layout residual** (`world_anim_layout`):
   - Pack presentation world anims with Z-rise / display-time / fade residual.
   - Honesty: `world_anim_layout_ok` on shell smoke (empty + synthetic).
4. Shell smoke residual flags:
   - `rng_stream_residual_ok`, `game_text_caption_ok`, `world_anim_layout_ok`
5. Tests (not log-only):
   - `host_rng_residual::*` (4)
   - `floating_text_layout::*` (4) including caption residual
   - `world_anim_layout::*` (2)
   - updated structure scatter / weapon_error / drop_variance residual honesty
   - all `special_power_strikes::` (**40**)
   - shell_smoke (4)
   - golden_skirmish_gate --frames 8 → `playable_claim=true`
   - shell_smoke_gate → `playable_claim=false` / `shell_host_playable_ok=true`

**Still residual (fail-closed, not claimed):**
- Full once-at-queue global GameLogic stream storage for multi-strike OCL draws
- Full GameLogic crate helper seed unified with Common stream
- Full CSF/STR Unicode GameText table load for all locales
- Full Anim2DCollection GPU / DisplayString font raster draw
- Full ScudStormMissile / SpectreHowitzerShell ThingFactory Object paths
- Full W3D bone-extract outer-node / connector LaserUpdate drawables
- Full OuterBeamWidth × scalar GPU laser radius
- Network residual replication (network deferred)

## Residual Host Playability — OuterBeamWidth + Scud MissileAI Loft + Howitzer DumbProjectile (2026-07-13)
**Closed (host-testable modules/powers residual not covered by wave 25 intensity/PreAttack/shell spawn):**
1. **Particle Uplink OuterBeamWidth × width_scalar residual** (`special_power_strikes` /
   `W3DLaserDraw` / `LaserUpdate::getCurrentLaserRadius`):
   - Retail OrbitalLaser OuterBeamWidth **26.0**, InnerBeamWidth **0.6**, NumBeams **12**,
     ScrollRate **-1.75**, TilingScalar **0.15**, Texture `EXNoise02.tga`.
   - Retail `getLaserTemplateWidth() = OuterBeamWidth * 0.5` → peak laser r **13.0**;
     retail damage formula `laserRadius × DamageRadiusScalar(3.4)` → peak **44.2**.
   - Host combat residual still caps at `PARTICLE_BEAM_RADIUS` **50** × width_scalar
     (fail-closed vs flipping combat radius mid-parity).
   - Draw width residual: OuterBeamWidth × width_scalar (peak **26**).
   - Connector OuterBeamWidth residual: Medium **1.2** / Intense **2.0**.
   - Honesty: `honesty_beam_outer_beam_width_ok` (peak draw/laser/retail-damage samples).
2. **ScudStormMissile MissileAIUpdate loft residual** (`special_power_strikes` /
   `ScudStormMissile` Object):
   - TryToFollowTarget **No**, FuelLifetime **0** (infinite), InitialVelocity **0**,
     DistanceToTravelBeforeTurning **500**, DistanceToTargetBeforeDiving **200**.
   - HeightDie TargetHeight **15.0** / InitialDelay **30**f / structures included.
   - Locomotor Speed **300**, PreferredHeight **240**, damping **0.7**, Mass **500**.
   - IgnitionFX `FX_ScudStormIgnition`, FireSound `ScudStormLaunch`,
     Exhaust `ScudMissileExhaust`, SpecialPowerCompletionDie `SuperweaponScudStorm`.
   - Per-missile wave counters: loft / ignition / launch sound / exhaust / HeightDie /
     completion residual.
   - Honesty: `honesty_scud_missile_loft_ok`.
   - Fail-closed: not full ThingFactory projectile Object / live MissileAIUpdate flight.
3. **SpectreHowitzerShell DumbProjectile residual** (`special_power_strikes` /
   `SpectreHowitzerShell` Object):
   - DumbProjectileBehavior + Physics Mass **1.0** + GeometryHeight **4.0** +
     Model `AVSpectreShell1` residual honesty per howitzer tick.
   - HeightDie OnlyWhenMovingDown residual; InstantDeath DETONATED (`FX_NukeGLA`)
     + LASERED (`FX_GenericMissileDisintegrate`) path residual counters.
   - Honesty: `honesty_howitzer_shell_dumb_projectile_ok` (extends shell spawn honesty).
   - Fail-closed: not full W3D shell drawable / live Physics / ThingFactory Object.
4. Snapshot/Xfer: OuterBeamWidth residual fields on `HostParticleBeamField`;
   DumbProjectile residual fields on `HostSpectreOrbitField`.
5. Tests (not log-only):
   - `particle_uplink_outer_beam_width_retail_radius_residual_honesty`
   - `scud_storm_missile_loft_residual_honesty`
   - updated `spectre_howitzer_shell_projectile_residual_honesty`
   - updated `particle_cannon_params_match_retail_continuous_beam` (OuterBeam matrix)
   - all `special_power_strikes::` (**40**)
   - golden_skirmish_gate --frames 8 → `playable_claim=true`
   - shell_smoke_gate → `playable_claim=false` / `shell_host_playable_ok=true`

**Still residual (fail-closed, not claimed):**
- Full ScudStormMissile ThingFactory Object / live MissileAIUpdate PreferredHeight spring
- Full SpectreHowitzerShell ThingFactory Object / W3D ModelDraw shell drawable
- Full W3D bone-extract outer-node / connector LaserUpdate drawable objects
- Full OuterBeamWidth multi-beam NumBeams GPU laser / texture scroll submit
  (host residual tracks draw width + retail formula; combat still r50)
- Full GameLogicRandomValueReal / GameClientRandomValue RNG streams
- Full InGameUI::addFloatingText GPU draw / Unicode GameText
- Full Anim2DCollection GPU / world-anim draw path
- Network combat/power residual replication (network deferred)

## Residual Host Playability — PUC Intensity + Scud PreAttack + SpectreHowitzerShell (2026-07-13)
**Closed (host-testable modules/powers residual not covered by wave 22–23):**
1. **Particle Uplink intensity schedule residual** (`special_power_strikes` /
   `ParticleUplinkCannonUpdate::setClientStatus`):
   - Retail pre-fire windows: BeginChargeTime **5000** ms → **150**f (CHARGING /
     IT_LIGHT outer), RaiseAntennaTime **4667** ms → **140**f (PREPARING /
     IT_MEDIUM outer + MODELCONDITION_UNPACKING), ReadyDelayTime **2000** ms →
     **60**f (ALMOST_READY / READY_TO_FIRE IT_MEDIUM connectors + laser-base Light).
   - Host residual anchors ready-to-fire at ParticleCannon `impact_frame`;
     host impact_delay **120**f covers PREPARING→ALMOST_READY→READY (full
     CHARGING needs the full BeginCharge+RaiseAntenna window).
   - Attack residual: FIRING (IT_INTENSE all + ground↔orbit) → POSTFIRE
     (IT_MEDIUM + laser still up) after TotalFiringTime → PACKING (effects clear)
     after WidthGrow decay tail.
   - Model-condition honesty: UNPACKING / DEPLOYED / PACKING residual counters.
   - Honesty: `honesty_beam_intensity_schedule_ok` / `honesty_beam_postfire_ok`.
2. **BeamLaunchFX residual** (`DelayBetweenLaunchFX` **1000** ms → **30**f):
   - First application on STATUS_FIRING entry; refresh every 30 frames while FIRING.
   - Retail name `FX_ParticleUplinkCannon_BeamLaunchIteration`.
   - Honesty: `honesty_beam_launch_fx_ok`.
3. **ScudStorm PreAttack + Chem FX residual**:
   - PreAttack PER_CLIP window (first missile delay) residual frame counter +
     `scud_pre_attack_active` until first wave.
   - Chem FXBone residual: **3** × `ScudStormBuildingGoo` (`FXBone01..03`).
   - FireFX `WeaponFX_ScudStormMissile` + detonation `ScudStormMissileDetonation`
     + launch bone `WeaponA` honesty per missile wave.
   - Honesty: `honesty_scud_pre_attack_and_chem_fx_ok`.
   - Fail-closed: not full ScudStormMissile Object / MissileAIUpdate loft path.
4. **SpectreHowitzerShell projectile residual** (`special_power_strikes` /
   `SpectreHowitzerGun` / `Object SpectreHowitzerShell`):
   - Each howitzer orbit tick records shell spawn + FireFX + detonation FX +
     FireSound + HeightDie InitialDelay (**30**f) residual honesty.
   - Retail anchors: ProjectileObject `SpectreHowitzerShell`, WeaponSpeed **999**,
     HeightDie TargetHeight **1.0**, GeometryRadius **4.0**, Scale **0.6**,
     locomotor speed **1111** honesty residual.
   - Honesty: `honesty_howitzer_shell_ok`.
   - Fail-closed: not full DumbProjectileBehavior Object / W3D shell drawable /
     PhysicsBehavior mass path.
5. Snapshot/Xfer: intensity schedule fields on `HostParticleBeamField`; shell
   residual fields on `HostSpectreOrbitField`.
6. Tests (not log-only):
   - `particle_uplink_intensity_schedule_and_beam_launch_fx_residual_honesty`
   - `scud_storm_pre_attack_and_chem_fx_residual_honesty`
   - `spectre_howitzer_shell_projectile_residual_honesty`
   - updated `particle_cannon_params_match_retail_continuous_beam` (intensity matrix)
   - all `special_power_strikes::` (**38**)
   - golden_skirmish_gate --frames 8 → `playable_claim=true`
   - shell_smoke_gate → `playable_claim=false` / `shell_host_playable_ok=true`

**Still residual (fail-closed, not claimed):**
- Full ScudStormMissile projectile Object / MissileAIUpdate loft path
- Full SpectreHowitzerShell DumbProjectileBehavior Object / W3D shell drawable
- Full W3D bone-extract outer-node / connector LaserUpdate drawable objects
- Full OuterBeamWidth × scalar GPU laser radius (host residual caps at r50)
- Full GameLogicRandomValueReal / GameClientRandomValue RNG streams
- Full InGameUI::addFloatingText GPU draw / Unicode GameText
- Full Anim2DCollection GPU / world-anim draw path
- Network combat/power residual replication (network deferred)

## Residual Host Playability — PUC WidthGrow Decay + Manual Drive + Outer Nodes (2026-07-13)
**Closed (host-testable combat/presentation residual not covered by wave 21 WidthGrow grow + Scorch/Reveal):**
1. **Particle Uplink WidthGrow decay shrink residual** (`special_power_strikes` /
   `ParticleUplinkCannonUpdate` / `LaserUpdate::setDecayFrames`):
   - Retail lifecycle relative to orbital birth: **grow** 0→1 over WidthGrowTime
     (**60**f), **hold** full through TotalFiringTime (**105**f), **decay** 1→0
     over WidthGrowTime after `orbitalDecayStart` (LASERSTATUS_DECAYING).
   - Host residual: `particle_width_scalar` / `particle_beam_damage_radius`
     implement grow/hold/decay; `particle_decay_start_frame` /
     `particle_death_frame` / `PARTICLE_BEAM_ORBITAL_LIFETIME_FRAMES` (**165**).
   - Beam `expires_frame` = orbital death (TotalFiring + WidthGrow decay tail);
     damage pulses stop at TotalDamagePulses but field lives through decay for
     width honesty sampling (`sample_beam_width_honesty` each logic frame).
   - Host-testable: hold end r50 hits unit at dist 30; half-decay r25 misses.
   - Honesty: `last_width_scalar` / `trough_width_scalar` / `decay_samples` /
     `honesty_beam_width_decay_ok`.
2. **Manual beam driving residual** (`setSpecialPowerOverridableDestination`):
   - `set_beam_override_destination` arms `manual_target_mode`, seeds
     `current_target_position` from last swath/click, records double-click frames.
   - `advance_manual_beam_drive` moves toward override at ManualDrivingSpeed
     **20**/s or ManualFastDrivingSpeed **40**/s (÷30 frames) when double-click
     gap < DoubleClickToFastDriveDelay (**15**f).
   - Damage/scorch epicenters use `current_target_position` under manual mode
     (not SwathOfDeath).
   - Honesty: `honesty_beam_manual_drive_ok` / `honesty_beam_fast_drive_ok`.
3. **Outer-node + connector laser residual** (STATUS_FIRING honesty):
   - Retail OuterEffectNumBones **5**, bone names FX / FXConnector / FXMain,
     Intense outer-node flare + Intense connector laser name residual.
   - On beam spawn: `outer_node_systems_created` / `connector_lasers_created` =
     5, laser-base ready flare + ground↔orbit laser honesty counters.
   - Honesty: `honesty_beam_outer_nodes_ok`.
   - Fail-closed: not full W3D bone extract matrix / GPU OuterBeamWidth draw /
     live ParticleSystem / LaserUpdate drawable objects.
4. Snapshot/Xfer: decay + manual + outer residual fields appended on
   `HostParticleBeamField`.
5. Tests (not log-only):
   - `particle_uplink_width_grow_decay_shrink_residual_honesty`
   - `particle_uplink_manual_drive_and_outer_nodes_residual_honesty`
   - updated `particle_cannon_params_match_retail_continuous_beam` (decay hold)
   - all `special_power_strikes::` (**35**)
   - golden_skirmish_gate --frames 8 → `playable_claim=true`
   - shell_smoke_gate → `playable_claim=false` / `shell_host_playable_ok=true`

**Still residual (fail-closed, not claimed):**
- Full ScudStormMissile projectile Object / PreAttack animation / Chem FX bones
- Full SpectreHowitzerShell projectile Object / full W3D CONTINUOUS_FIRE anim draw
- Full W3D bone-extract outer-node / connector LaserUpdate drawable objects
- Full OuterBeamWidth × scalar GPU laser radius (host residual caps at r50)
- Full intensity schedule client residual for CHARGING/PREPARING/ALMOST_READY
- Full GameLogicRandomValueReal / GameClientRandomValue RNG streams
- Full InGameUI::addFloatingText GPU draw / Unicode GameText
- Full Anim2DCollection GPU / world-anim draw path
- Network combat/power residual replication (network deferred)

## Residual Host Playability — PUC WidthGrow Damage Radius + Scorch/Reveal (2026-07-13)
**Closed (host-testable combat/presentation residual not covered by wave 18 remnant/model-condition):**
1. **Particle Uplink WidthGrow damage-radius residual** (`special_power_strikes` /
   `ParticleUplinkCannonUpdate` / `LaserUpdate`):
   - Retail `WidthGrowTime` = **2000** ms → **60** frames; laser
     `m_currentWidthScalar` ramps 0→1 from orbital birth.
   - Host residual: `particle_width_scalar` / `particle_beam_damage_radius`
     scale [`PARTICLE_BEAM_RADIUS`] (50) by width scalar per pulse.
   - Early pulses: radius 0 at spawn (miss off-epicenter); half-grow radius 25;
     full-grow radius 50.
   - Honesty: `peak_width_scalar` / `last_damage_radius` /
     `honesty_beam_width_grow_ok`.
   - WidthGrow **decay shrink** + manual drive + outer-node residual closed
     wave 22 (see section above).
2. **TotalScorchMarks + GroundHitFX + RevealRange residual**
   (`ParticleUplinkCannonUpdate` STATUS_FIRING):
   - Retail TotalScorchMarks **20**, ScorchMarkScalar **2.4**, RevealRange **50**,
     GroundHitFX `FX_ParticleUplinkCannon_BeamHitsGround`.
   - Host residual: nextFactor scorch schedule over TotalFiringTime; scorch
     epicenters walk SwathOfDeath via pulse-index residual; GroundHitFX +
     reveal honesty counters per mark.
   - GameLogic: doShroudReveal + undoShroudReveal same-frame pulse (retail
     gratuitous vision) via ShroudManager when players exist for source team.
   - Honesty: `honesty_beam_scorch_ok` / `honesty_beam_reveal_ok`.
   - Fail-closed: not full TheGameClient::addScorch GPU decals / full FOW grid
     cell matrix without shroud init.
3. Snapshot/Xfer: WidthGrow + scorch residual fields appended on
   `HostParticleBeamField`.
4. Tests (not log-only):
   - `particle_uplink_width_grow_damage_radius_residual_honesty`
   - `particle_uplink_scorch_reveal_residual_honesty`
   - updated `particle_cannon_params_match_retail_continuous_beam`
   - updated `particle_cannon_impact_spawns_beam_and_ticks_damage` (WidthGrow)
   - all `special_power_strikes::` (**33** at wave 20; **35** after wave 22)
   - golden_skirmish_gate --frames 8 → `playable_claim=true`
   - shell_smoke_gate → `playable_claim=false` / `shell_host_playable_ok=true`

**Still residual (fail-closed, not claimed):**
- Full ScudStormMissile projectile Object / PreAttack animation / Chem FX bones
- Full SpectreHowitzerShell projectile Object / full W3D CONTINUOUS_FIRE anim draw
- Full W3D bone-extract outer-node / connector LaserUpdate drawable objects
- Full OuterBeamWidth × scalar GPU laser (host residual closed wave 22)
- Full GameLogicRandomValueReal / GameClientRandomValue RNG streams
- Full InGameUI::addFloatingText GPU draw / Unicode GameText
- Full Anim2DCollection GPU / world-anim draw path
- Network combat/power residual replication (network deferred)

## Residual Host Playability — InGameUI Floating Text Freeze + MoneyPickUp Anim2D Presentation (2026-07-13)
**Closed (host-testable presentation residual not covered by laser SegLine / ControlBar dual-tick):**
1. **PresentationFrame floating text freeze residual** (`presentation_frame`):
   - Freeze host residual registries into snapshot-owned `PresentationFloatingText`:
     oil derrick / black market AutoDeposit, HackInternet, CashBounty, MoneyCrate.
   - Retail timeout residual: `DEFAULT_FLOATING_TEXT_TIMEOUT` = **10** frames
     (`LOGICFRAMES_PER_SECOND/3`); move-up **1.0**, vanish rate **0.1**.
   - Stable sort by spawn frame / source id / kind; included in `presentation_hash`.
   - Host inject helpers for dual-tick freeze tests (`push_residual_*_for_presentation`).
   - Fail-closed: not full DisplayString GPU / Unicode GameText localization.
2. **MoneyPickUp Anim2D world-anim freeze residual** (`presentation_frame`):
   - Freeze `HostMoneyPickUpAnim` → `PresentationWorldAnim` (template MoneyPickUp,
     display **4.0**s, ZRise **15**, fades **Yes**).
   - Honesty: `world_anim_presentation_ok` (empty honest; non-empty template check).
   - Fail-closed: not full Anim2DCollection GPU / WORLD_ANIM_FADE_ON_EXPIRE draw.
3. **CPU floating-text layout pack residual** (`graphics/floating_text_layout`):
   - Interleaved layout samples: pos + lift_y + color + alpha + amount + age/timeout.
   - Move-up / vanish residual matches C++ `updateFloatingText` / `drawFloatingText`.
   - Honesty: `cpu_pack_ok` / `has_geometry` / `gpu_upload_ready` (ready mark only).
   - Shell smoke: empty host pack + synthetic cash geometry pack residual.
4. Tests (not log-only):
   - `presentation_frame_freezes_floating_text_and_world_anim`
   - `graphics::floating_text_layout::*` (empty / synthetic / vanish)
   - shell_smoke residual flags `floating_text_layout_ok` + `world_anim_presentation_ok`
   - golden_skirmish_gate --frames 8 → `playable_claim=true`
   - shell_smoke_gate → `playable_claim=false` / `shell_host_playable_ok=true`

**Still residual (fail-closed, not claimed):**
- Full DisplayString GPU raster / font atlas submit for floating cash
- Full Unicode GameText localization of `GUI:AddCash`
- Full Anim2DCollection GPU / world-anim draw path
- Full ScudStormMissile projectile Object / PreAttack animation / Chem FX bones
- Full SpectreHowitzerShell projectile Object / model-condition CONTINUOUS_FIRE_*
- Network floating-text / world-anim residual replication (network deferred)

## Residual Host Playability — PUC DamagePulseRemnant + Spectre CONTINUOUS_FIRE ModelCondition (2026-07-13)
**Closed (host-testable combat/power residual not covered by wave 16 swath/VoiceRapidFire):**
1. **Particle Uplink DamagePulseRemnant trail residual** (`special_power_strikes` /
   `ParticleUplinkCannonUpdate`):
   - Retail `DamagePulseRemnantObjectName` = `ParticleUplinkCannonTrailRemnant`
     with `ParticleUplinkCannonBeamTrailRemnantWeapon` Primary **15** / r**10**,
     DelayBetweenShots **250** ms → **7** frames, DeletionUpdate lifetime **4000** ms
     → **120** frames.
   - Host residual: each completed beam pulse spawns a `HostParticleRemnantField`
     at the pulse SwathOfDeath epicenter; ticks residual damage (ALLIES ENEMIES
     NEUTRALS — all living except source) until lifetime expires.
   - Honesty: `remnant_fields_spawned_total` / `remnant_damage_applications_total`
     / `honesty_beam_remnant_ok` / `honesty_beam_remnant_damage_ok`.
   - Snapshot/Xfer: remnant fields + totals appended after beam residual.
   - Fail-closed: not full ThingFactory Object / ImmortalBody / outer-node lasers /
     manual beam driving / laser width grow GPU.
2. **Spectre MODELCONDITION_CONTINUOUS_FIRE residual** (`special_power_strikes` /
   FiringTracker):
   - Retail `FiringTracker::speedUp` sets MODELCONDITION_CONTINUOUS_FIRE_MEAN /
     FAST; `coolDown` sets CONTINUOUS_FIRE_SLOW and clears MEAN/FAST.
   - Host residual honesty counters on orbit field:
     `model_condition_mean_sets` / `model_condition_fast_sets` /
     `model_condition_slow_sets` (incremented on fire-level transitions + coast).
   - Honesty: `honesty_model_condition_continuous_fire_ok` /
     `honesty_model_condition_slow_ok`.
   - Fail-closed: not full drawable W3D model-condition anim / SpectreHowitzerShell
     projectile Object.
3. Tests (not log-only):
   - `particle_uplink_damage_pulse_remnant_residual_honesty`
   - `spectre_model_condition_continuous_fire_residual_honesty`
   - updated `spectre_continuous_fire_rof_residual_honesty` (model-condition)
   - updated `spectre_continuous_fire_coast_cooldown_residual` (SLOW residual)
   - all `special_power_strikes::` (**31**)
   - golden_skirmish_gate --frames 8 → `playable_claim=true`
   - shell_smoke_gate → `playable_claim=false` / `shell_host_playable_ok=true`

**Still residual (fail-closed, not claimed):**
- Full ScudStormMissile projectile Object / PreAttack animation / Chem FX bones
- Full SpectreHowitzerShell projectile Object / full W3D CONTINUOUS_FIRE anim draw
- Full PUC outer-node lasers / manual beam driving / laser width grow matrix
- Full GameLogicRandomValueReal / GameClientRandomValue RNG streams
- Full InGameUI::addFloatingText GPU draw / Unicode GameText
- Network combat/power residual replication (network deferred)

## Residual Host Playability — Particle Uplink SwathOfDeath + Spectre VoiceRapidFire (2026-07-13)
**Closed (host-testable combat/power residual not covered by wave 16 coast/SupW/structure):**
1. **Particle Uplink SwathOfDeath residual** (`special_power_strikes` /
   `ParticleUplinkCannonUpdate`):
   - Retail SwathOfDeathDistance **200** / Amplitude **50**; DamageRadiusScalar
     **3.4** honesty constant.
   - Host residual: each damage pulse epicenter walks S-curve around click point
     (`particle_swath_offset` / `particle_swath_epicenter`); first pulse at
     `x = -distance/2`, mid pulse near click, lateral sine peaks ~amplitude.
   - Fractional nextFactor pulse schedule residual:
     `particle_next_pulse_frame = spawn + floor(pulses_made/total * duration)`.
   - Honesty: `swath_applications` / `max_swath_offset` / `honesty_beam_swath_ok`.
   - Host-testable: click-epicenter unit misses first pulse; mid swath hits; offset
     honesty > 50.
   - Fail-closed: not full building→target rotation matrix / manual driving /
     outer-node lasers / remnant trail Objects / laser width grow GPU.
2. **Spectre VoiceRapidFire residual** (`special_power_strikes` / FiringTracker):
   - Retail `FiringTracker::speedUp` plays PerUnitSound `"VoiceRapidFire"` when
     entering CONTINUOUS_FIRE_FAST.
   - Host residual: `rapid_fire_voice_cues` increments on gattling/howitzer MEAN→FAST
     transition; honesty `honesty_voice_rapid_fire_ok` + audio name residual
     `SpectreGunshipVoiceRapidFire`.
   - Fail-closed: not full audio bank stream / model-condition CONTINUOUS_FIRE_* anim.
3. Tests (not log-only):
   - `particle_uplink_swath_of_death_residual_honesty`
   - updated `particle_cannon_params_match_retail_continuous_beam`
   - updated `particle_cannon_impact_spawns_beam_and_ticks_damage`
   - updated `spectre_continuous_fire_rof_residual_honesty` (VoiceRapidFire)
   - all `special_power_strikes::` (**29**)
   - golden_skirmish_gate --frames 8 → `playable_claim=true`
   - shell_smoke_gate → `playable_claim=false` / `shell_host_playable_ok=true`

**Still residual (fail-closed, not claimed):**
- Full ScudStormMissile projectile Object / PreAttack animation / Chem FX bones
- Full SpectreHowitzerShell projectile Object / model-condition CONTINUOUS_FIRE_*
- Full PUC outer-node lasers / manual beam driving / remnant trail Objects
- Full GameLogicRandomValueReal / GameClientRandomValue RNG streams
- Full InGameUI::addFloatingText GPU draw / Unicode GameText
- Network combat/power residual replication (network deferred)

## Residual Host Playability — Spectre Coast + SupW FuelAir + Structure Scatter (2026-07-13)
**Closed (host-testable combat/power/economy residual not covered by wave 15 anthrax/ROF):**
1. **Spectre ContinuousFireCoast residual** (`special_power_strikes` / FiringTracker):
   - Retail ContinuousFireCoast **2000** ms → **60** frames for both
     `SpectreGattlingGun` and `SpectreHowitzerGun`.
   - On each residual shot: `coast_until = frame + interval + 60`.
   - After idle past coast: coolDown zeros consecutive + fire level (base ROF).
   - Host tracks `gattling_coast_until_frame` / `howitzer_coast_until_frame` +
     `*_coast_applications` honesty; post-coast restarts at base interval.
   - VoiceRapidFire residual closed wave 16b (enter FAST honesty cue).
   - Fail-closed: not full model-condition CONTINUOUS_FIRE_* anim / howitzer shell
     projectile Object.
2. **SupW FuelAir 900/r70 matrix residual** (`host_aurora_bomb`):
   - `HostAuroraBombKind::FuelAirSupW` = retail `SupW_FuelBombDetonationWeapon`
     Primary **900** / r**70** (AirF keeps **1000**/r**100**).
   - Template classifier: `SupW_*` / `TestAuroraFuelAirSupW` → FuelAirSupW;
     `AirF_*` → FuelAir; standard Aurora → Standard.
   - Same gas delay + DaisyCutterFlame secondary **5**/r**100**; r80 is flame-only
     under SupW (outside primary 70) but still in AirF primary (matrix differs).
   - Fail-closed: not full SlowDeath multi-stage / tree burn / OCL gas Object.
3. **AutoDeposit structure geometry scatter residual** (economy / floating cash):
   - C++ KINDOF_STRUCTURE: `±0.3 * major/minor radius` on floating cash text pos.
   - Host residual `structure_floating_text_scatter` (deterministic golden-ratio
     phase); applied on oil derrick deposit/capture and black market deposit paths.
   - Honesty: `geometry_scatter_applications` / `honesty_geometry_scatter_ok`.
   - Fail-closed: not full GeometryInfo matrix / GameClientRandomValue stream /
     InGameUI GPU draw.
4. Tests (not log-only):
   - `spectre_continuous_fire_coast_cooldown_residual`
   - `supw_fuel_bomb_900_r70_matrix_residual_honesty`
   - `structure_geometry_scatter_residual`
   - all `special_power_strikes::` (28 → 29 with swath residual)
   - aurora / oil / black market host unit + integration residual paths green

**Still residual (fail-closed, not claimed):**
- Full ScudStormMissile projectile Object / PreAttack animation / Chem FX bones
- Full SpectreHowitzerShell projectile Object / model-condition CONTINUOUS_FIRE_*
- Full GameLogicRandomValueReal / GameClientRandomValue RNG streams
- Full InGameUI::addFloatingText GPU draw / Unicode GameText
- Network combat/power/economy residual replication (network deferred)

## Residual Host Playability — Scud Anthrax Upgrade + Spectre ContinuousFire ROF (2026-07-13)
**Closed (host-testable combat/power residual not covered by wave 13 dual-weapon / per-missile poison):**
1. **ScudStorm anthrax-upgrade residual** (`special_power_strikes` / `ScudStormAnthraxTier`):
   - Base `ScudStormDamageWeapon`: Primary **500** / Secondary **150** + LargePoison **15**.
   - Anthrax Beta (`Upgrade_GLAAnthraxBeta` / `ScudStormDamageWeaponUpgraded`):
     Secondary **200** + LargePoisonFieldWeaponUpgraded **25**/tick (radius **140**).
   - Chem Gamma (`Chem_Upgrade_GLAAnthraxGamma` / `Chem_ScudStormDamageWeaponGamma`):
     Primary **550** / Secondary **200** + poison **25**.
   - Queue path selects tier from player unlocked sciences/upgrades; strike stores
     `scud_anthrax_tier`; per-missile poison uses tier damage residual.
   - Host-testable: secondary ring hit = 200 under Beta; poison field 25/tick.
   - Fail-closed: not full ScudStormMissile projectile Object / PreAttack animation /
     Chem particle bone matrix.
2. **Spectre ContinuousFire ROF residual** (`special_power_strikes`):
   - Gattling ContinuousFireOne/**1** / Two/**2**: base interval **3** frames → MEAN
     ROF **200%** → **1** frame → FAST ROF **300%** → **1** frame.
   - Howitzer ContinuousFireOne/**1** / Two/**2** on HowitzerFiringRate base **9**:
     MEAN ROF **150%** → **6** frames; FAST ROF **200%** → **4** frames.
   - Host tracks `gattling_consecutive` / `howitzer_consecutive` + peak fire levels;
     honesty gates for MEAN reached.
   - ContinuousFireCoast residual closed wave 16 (was fail-closed here).
3. **Unit-host splash last_damage_source residual** (related combat residual):
   - Many unit host splash / fire paths now call `take_damage_from(..., Some(source))`
     so CashBounty killer residual prefers BodyModule last_damage_source.
   - Fail-closed: not full DestructionEvent killer ObjectId matrix on every path.
4. Tests (not log-only):
   - `scud_storm_anthrax_upgrade_secondary_and_poison_residual`
   - `spectre_continuous_fire_rof_residual_honesty`
   - all `special_power_strikes::` (27+)
   - `scud_storm_host_path_queues_and_completes`
   - `spectre_gunship_host_path_queues_orbit_damage_over_time`

**Still residual (fail-closed, not claimed):**
- Full ScudStormMissile projectile Object / PreAttack animation / Chem FX bones
- Full SpectreHowitzerShell projectile Object / model-condition continuous-fire flags
- Full GameLogicRandomValueReal RNG stream
- Network Scud anthrax / Spectre ROF replication (network deferred)

## Residual Host Playability — Spectre Gattling/Howitzer Dual Residual + Scud Per-Missile Poison (2026-07-13)
**Closed (host-testable combat/power residual not covered by Scud multi-missile / Spectre OrbitTime / Aurora wave):**
1. **Spectre dual-weapon orbit residual** (`special_power_strikes`):
   - Howitzer (`SpectreHowitzerGun`): PrimaryDamage **80** in PrimaryDamageRadius
     **25** around reticle + deterministic `RandomOffsetForHowitzer` residual (**±20**).
   - Gattling (`SpectreGattlingGun`): PrimaryDamage **90** to nearest living enemy in
     AttackAreaRadius **200**, DelayBetweenShots **100** ms → **3** frames.
   - HowitzerFiringRate residual remains **9** frames; both streams exclude source
     + same-team friendlies.
   - Host-testable: first insertion tick = 80+90 at reticle; gattling-only at +3;
     far units outside orbit untouched; honesty_gattling_ok.
   - Fail-closed: not full projectile path / gunner Object (continuous-fire residual closed wave 15).
2. **ScudStorm per-missile LargePoisonField residual**:
   - Each multi-missile wave spawns `OCL_PoisonFieldLarge` residual at wave
     epicenters (retail FireOCL each detonation), not only on final wave.
   - Host-testable: poison fields grow across waves; up to ClipSize **9** fields.
3. Tests (not log-only):
   - `spectre_gattling_and_howitzer_residual_honesty`
   - updated `spectre_gunship_impact_spawns_orbit_and_ticks_damage`
   - updated `scud_storm_multi_missile_scatter_and_poison_residual`
   - updated `spectre_gunship_host_path_queues_orbit_damage_over_time`

**Still residual (fail-closed, not claimed):**
- Full SpectreHowitzerShell projectile / FiringTracker coast cool-down
- Full ScudStormMissile Object / PreAttack animation
- Full GameLogicRandomValueReal RNG stream
- Network Spectre dual-weapon / Scud poison replication (network deferred)

## Residual Host Playability — ScudStorm Multi-Missile + Spectre OrbitTime + Aurora ALLIES/Flame (2026-07-13)
**Closed (host-testable combat/power residual not covered by wave 11):**
1. **ScudStorm ClipSize-9 ScatterTarget multi-missile residual** (`special_power_strikes`):
   - Retail `ScudStormWeapon` ClipSize **9** + ScatterTarget table × ScatterTargetScalar
     **120**; PreAttackDelay **3000** ms → **90** frames; DelayBetweenShots Min/Max
     **100**/1000 ms → **3**/30 frames deterministic residual stagger.
   - Per-missile blast: `ScudStormDamageWeapon` Primary **500**/r**50** + Secondary
     **150**/r**200** (two-ring residual).
   - Each missile impact also spawns residual LargePoisonField toxin ticks
     (Primary **15** / r**140** / 500 ms / Lifetime **45** s).
   - Host-testable: first missile at PreAttack frame; all 9 applied by last delay;
     poison honesty gate.
   - Fail-closed: not full ScudStormMissile projectile Object / PreAttack animation /
     anthrax-upgraded poison weapon matrix / GameLogicRandomValueReal stream.
2. **SpectreGunship science-tier OrbitTime residual**:
   - Retail SCIENCE_SpectreGunship1/2/3 OrbitTime **10s / 15s / 20s** → **300 / 450 / 600**
     frames; `SpectreGunshipScienceTier` + `highest_from_sciences`.
   - Strike stores `spectre_tier`; orbit field duration uses tier residual.
   - Queue selects highest unlocked Spectre science on source team (default L2).
   - Fail-closed: not full SpectreGunshipUpdate gattling-strafe / howitzer projectile.
3. **Aurora bomb RadiusDamageAffects ALLIES residual** (`host_aurora_bomb`):
   - Retail `AuroraBombWeapon` / `AirF_AuroraBombDetonationWeapon` list
     `ALLIES ENEMIES NEUTRALS` (Standard also `NOT_SIMILAR`).
   - Host residual hits living non-self units of any team; source aircraft Object
     still excluded.
   - Host-testable: friend at epicenter takes primary blast; source never hit.
   - Fail-closed: not full NOT_SIMILAR / Relationship matrix.
4. **FuelAir DaisyCutterFlameWeapon secondary residual**:
   - Retail AirF_AuroraBombGas / SupW_AuroraFuelAirGas SlowDeath MIDPOINT
     `DaisyCutterFlameWeapon` PrimaryDamage **5.0** / PrimaryDamageRadius **100**.
   - Applied additively on FuelAir impact when horizontal distance ≤ flame radius.
   - Honesty constants: `AURORA_FUEL_AIR_FLAME_DAMAGE` / `AURORA_FUEL_AIR_FLAME_RADIUS`
     / `AURORA_FUEL_AIR_FLAME_AUDIO`; `HostAuroraBombKind::spawns_daisy_cutter_flame()`.
   - Host-testable: FuelAir epicenter = primary + 5; Standard Aurora has no flame.
   - Fail-closed: not full SlowDeath MIDPOINT timing / tree burn state / FX GPU;
     SupW 900/r70 collapse still uses AirF 1000/r100 host numbers.
5. **last_damage_source residual on superweapon / Aurora / continuous field paths**:
   - `update_aurora_bombs` and `update_special_power_strikes` (primary blast +
     radiation / toxin / Spectre orbit / Particle beam ticks) now call
     `take_damage_from(..., Some(source_object))` so CashBounty killer residual
     prefers BodyModule last_damage_source over nearest same-team fallback.
   - Fail-closed: not full DestructionEvent killer ObjectId on every residual
     kill path (unit host splash residual paths still open).
6. Tests (not log-only):
   - `scud_storm_multi_missile_scatter_and_poison_residual`
   - `spectre_orbit_time_science_tier_residual`
   - updated ScudStorm host-path integration (ClipSize 9 + poison honesty)
   - `allies_residual_and_legal_target_honesty`
   - `fuel_air_flame_and_allies_residual_honesty`
   - updated `queue_and_complete_delayed_dive_plan`
   - updated `aurora_bomb_host_path_queues_and_applies_delayed_area_damage`
   - snapshot xfer carries `spectre_tier` + toxin field damage/radius/tick params

**Still residual (fail-closed, not claimed):**
- Full ScudStormMissile projectile / PreAttack / anthrax-upgraded LargePoison
- Full Spectre gattling-strafe / howitzer projectile / edge-spawn flight
- Full SlowDeath multi-stage gas object / tree burn state machine
- Full SupW_FuelBombDetonationWeapon 900/r70 vs AirF 1000/r100 matrix
- Full AuroraBombLocomotor / MissileAIUpdate dive path
- Full last_damage_source on every unit-host residual splash path
- Network ScudStorm / Spectre / Aurora / superweapon blast replication (network deferred)

## Residual Host Playability — MOABFlame Secondary + RadiusDamageAffects ALLIES (2026-07-13)
**Closed (host-testable combat/power residual not covered by wave 10):**
1. **MOABFlameWeapon secondary residual** (`special_power_strikes`):
   - Retail `MOABFlameWeapon` PrimaryDamage **5.0** / PrimaryDamageRadius **100**
     (MOABGas SlowDeath MIDPOINT — tree-ignite flame).
   - Applied additively on `DaisyCutter` and `CruiseMissile` primary impact when
     horizontal distance ≤ flame radius.
   - Honesty constants: `MOAB_FLAME_DAMAGE` / `MOAB_FLAME_RADIUS` / `MOAB_FLAME_AUDIO`.
   - Host-testable: epicenter damage = primary + 5; outer ally beyond flame gets
     falloff primary only.
   - Fail-closed: not full SlowDeath MIDPOINT timing / tree burn state / FX GPU.
2. **RadiusDamageAffects ALLIES residual** for retail blast kinds:
   - ArtilleryBarrage / CarpetBomb / DaisyCutter / CruiseMissile / NuclearMissile /
     AnthraxBomb / A10Strike / ScudStorm primary blasts now hit same-team units
     (retail `RadiusDamageAffects = ALLIES ENEMIES NEUTRALS`).
   - Source launcher Object still excluded.
   - Spectre orbit / Particle Uplink continuous fields keep their own filters.
   - Host-testable: friendly at epicenter takes damage; A10 hits ally + enemy.
3. Tests (not log-only):
   - `moab_flame_and_allies_residual_honesty`
   - updated `cruise_missile_*` / `queue_and_complete_daisy_cutter_*`
   - updated anthrax/nuclear impact ally hits
   - `friendly_fire_allies_residual_and_source_excluded`
   - updated GameLogic host-path integration (Daisy/Carpet/Artillery/Cruise)

**Still residual (fail-closed, not claimed):**
- Full SlowDeath multi-stage wave / MOABGas object / tree burn state machine
- Full ChinaArtilleryCannon / AmericaJetB52 DeliverPayload transport Objects
- Full GameLogicRandomValueReal RNG stream
- Network multi-strike / ally-blast / flame replication (network deferred)

## Residual Host Playability — STEALTHED Float Gate + IC Scatter + last_damage_source Killer (2026-07-13)
**Closed (host-testable STEALTHED floating-text local display gate + Internet Center scatter + CashBounty last_damage_source killer residual):**
1. **STEALTHED local-player floating cash display gate residual**:
   - C++ AutoDepositUpdate / HackInternetAIUpdate:
     `if STEALTHED && !isLocallyControlled && !DETECTED → displayMoney = FALSE`.
   - Wired for oil derrick / black market AutoDeposit and hacker cash pings.
   - Hacker also gates on containedBy Internet Center STEALTHED residual.
   - Cash still deposits; only floating text presentation is gated.
   - Host-testable: gate matrix unit tests; suppressed honesty counters.
   - Fail-closed: not full InGameUI GPU draw / Unicode GameText.
2. **Internet Center floating-text geometry scatter residual** (`host_hacker_income`):
   - C++ ±0.3 major/minor radius GameClientRandomValue when depositing inside IC.
   - Host residual: deterministic scatter; honesty `ic_scatter_applications`.
   - Fail-closed: not full GeometryInfo major/minor matrix / client RNG stream.
3. **CashBounty last_damage_source killer residual** (`host_cash_bounty`):
   - Prefer victim BodyModule `last_damage_source` for killer ObjectId + float pos.
   - Main combat fire path uses `take_damage_from(dmg, Some(attacker_id))`.
   - Fallback nearest living same-team residual when source unset.
   - Host-testable: bounty float `killer_id == attacker`; last_damage_source honesty.
   - Fail-closed: not full DestructionEvent killer ObjectId on every residual kill path.
4. Tests (not log-only):
   - `stealthed_local_display_gate_residual` (oil)
   - `stealthed_local_display_gate_and_ic_scatter_residual` (hacker)
   - `last_damage_source_killer_residual_honesty` (cash bounty)
   - updated `cash_bounty_increases_cash_on_enemy_kill` (last_damage_source killer)

**Still residual (fail-closed, not claimed):**
- Full InGameUI::addFloatingText GPU draw / Unicode GameText localization
- Full DestructionEvent killer ObjectId matrix on every non-combat residual kill path
- Network floating-text / last_damage_source replication (network deferred)

## Residual Host Playability — Artillery WeaponErrorRadius/DelayDelivery + Carpet DropDelay Stagger (2026-07-13)
**Closed (host-testable combat/power multi-strike residual):**
1. **ArtilleryBarrage WeaponErrorRadius residual** (`special_power_strikes`):
   - C++ `DeliverPayloadNugget`: formationIndex **0** spot-on; others
     `GameLogicRandomValueReal(0, WeaponErrorRadius=100)` + random angle.
   - Host residual: deterministic `weapon_error_radius_offset` (golden-ratio phase).
   - Replaces fixed ring placement; shells stay inside **100** error radius.
   - Host-testable: lead shell at click; non-lead scatter ≤ error radius.
2. **ArtilleryBarrage per-shell DelayDelivery stagger residual**:
   - Retail `DelayDeliveryMax` = 3000 ms → **90** frames; C++ disables each
     ChinaArtilleryCannon transport until `Random(0, max)`.
   - Host residual: base approach **90** + per-shell `delay_delivery_frames`
     (lead **0**; others deterministic in `[0, 90]`).
   - Multi-wave impact: shells apply when due; strike completes on last shell.
   - Host-testable: first wave at base delay; complete at last shell frame.
3. **CarpetBomb DropDelay stagger residual**:
   - Retail OCL `DropDelay` = 300 ms → **9** frames between bombs.
   - Bomb `i` impacts at approach **90** + `i × 9`; multi-wave complete after
     last bomb (index 14).
   - Jump past several frames applies all overdue bombs in one wave (save/load).
   - Host-testable: first bomb only at frame 90; center/outer later; complete last.
4. Tests (not log-only):
   - `weapon_error_radius_and_delay_delivery_residual_honesty`
   - updated `artillery_barrage_params_match_retail_multi_shell`
   - updated `artillery_barrage_delayed_multi_shell_scatter_damage`
   - updated `carpet_bomb_params_match_retail_multi_strike` (DropDelay **9**)
   - updated `carpet_bomb_delayed_line_multi_strike_damage` (stagger waves)
   - updated GameLogic carpet/artillery host-path integration tests

**Still residual (fail-closed, not claimed):**
- Full ChinaArtilleryCannon / AmericaJetB52 DeliverPayload transport Objects
- Full GameLogicRandomValueReal RNG stream (host deterministic residual)
- Full MOABFlameWeapon secondary / NeutronMissile loft projectile
- Network multi-strike stagger replication (network deferred)

## Residual Host Playability — Hacker/CashBounty Floating Text + Artillery FormationSize Tiers (2026-07-13)
**Closed (host-testable HackInternet + CashBounty floating cash text + ArtilleryBarrage science-tier FormationSize residual):**
1. **HackInternet floating cash text residual** (`host_hacker_income`):
   - Host `+$N` presentation at hacker pos + Z offset **20** (C++ `pos.z += 20`).
   - Green RGBA **(0,255,0,255)** (retail `GameMakeColor(0,255,0,255)`).
   - GameText key honesty `GUI:AddCash`.
   - Recorded on every residual cash ping (field + Internet Center).
   - Host-testable: floating text on deposit; amount/color/key constants.
   - Fail-closed: not full InGameUI GPU draw / STEALTHED local-player display gate /
     Internet Center geometry scatter random.
2. **CashBounty floating cash text residual** (`host_cash_bounty`):
   - Host `+$N` at killer residual pos + Z offset **10** (C++ killer `pos.z += 10`).
   - Yellow RGBA **(255,255,0,255)** (retail `GameMakeColor(255,255,0,255)`).
   - Killer pos residual: nearest living unit on killer team (destruction event carries team only).
   - Host-testable: floating text on bounty award; yellow color / Z+10 honesty.
   - Fail-closed: not full killer ObjectId on destruction event / InGameUI GPU.
3. **ArtilleryBarrage science-tier FormationSize residual** (`special_power_strikes`):
   - Retail OCL FormationSize **12 / 24 / 36** for SCIENCE_ArtilleryBarrage1/2/3.
   - `ArtilleryBarrageScienceTier` + `artillery_barrage_points_for_tier`.
   - Queue path selects highest unlocked SCIENCE_ArtilleryBarrage* on source team.
   - Strike stores `artillery_tier`; impact uses tier shell count for multi-shell scatter.
   - Host-testable: L1=12 / L2=24 / L3=36 points; science name matrix; highest-wins.
   - Fail-closed: not full random WeaponErrorRadius draw / per-shell DelayDelivery stagger /
     ChinaArtilleryCannon DeliverPayload transport Object.
4. Tests (not log-only):
   - `floating_text_residual_green_z20` (hacker)
   - `floating_text_residual_yellow_z10` (cash bounty)
   - updated `artillery_barrage_params_match_retail_multi_shell` (tier FormationSize)
   - host unit tests + existing integration residual paths still green

**Still residual (fail-closed, not claimed):**
- Full InGameUI::addFloatingText GPU draw / STEALTHED local display gate (hacker)
- Full killer ObjectId on DestructionEvent (cash bounty uses nearest same-team residual)
- Full ArtilleryBarrage random WeaponErrorRadius / OCL DeliverPayload transport path
- Network floating-text / artillery-tier replication (network deferred)

## Residual Host Playability — AutoDeposit Floating Text + Oil SupplyLines Boost (2026-07-13)
**Closed (host-testable AutoDepositUpdate floating cash text + TechOilDerrick UpgradedBoost residual):**
1. **AutoDeposit floating cash text residual** (`host_oil_derrick` / `host_black_market`):
   - Host `+$N` presentation at building pos + Z offset **10** (C++ `pos.z += 10`).
   - Player color RGB OR alpha **230** (C++ `GameMakeColor(0,0,0,230)`).
   - GameText key honesty `GUI:AddCash`.
   - Recorded on oil derrick deposit / capture bonus and black market deposit.
   - Host-testable: floating text on deposit; amount/color/key constants.
   - Fail-closed: not full InGameUI GPU draw / STEALTHED local-player display gate /
     Unicode GameText localization.
2. **TechOilDerrick UpgradedBoost residual** (`host_oil_derrick`):
   - Retail `Upgrade_AmericaSupplyLines` Boost **+20** on DepositAmount **200** → **220**.
   - Host-testable: base 200 without upgrade; 220 with SupplyLines; boost honesty counter.
   - Fail-closed: not full upgrade-mux edge matrix beyond SupplyLines name residual.
3. Tests (not log-only):
   - host_oil_derrick floating text + SupplyLines boost unit tests
   - host_black_market floating text unit tests
   - updated oil derrick / black market GameLogic residual integration tests

**Still residual (fail-closed, not claimed):**
- Full InGameUI::addFloatingText GPU draw / STEALTHED local display gate
- Full AutoDeposit upgrade-mux beyond SupplyLines residual
- Network AutoDeposit floating-text replication (network deferred)

## Residual Host Playability — calcMinTurnRadius + MaxAttempts Re-Approach + Off-Map Recover (2026-07-13)
**Closed (host-testable DeliverPayloadAIUpdate calcMinTurnRadius + ConsiderNewApproach + HeadOffMap/RecoverFromOffMap residual):**
1. **calcMinTurnRadius residual** (`host_deliver_payload`):
   - C++ `maxSpeed / maxTurnRate` (both per logic frame).
   - B52Locomotor: Speed **125**/sec, TurnRate **25**°/sec → radius ≈ **286.48**
     (`5 × 180/π`); zero turn rate → sentinel **999999**.
   - ConsiderNewApproach min re-approach dist = radius × **DIST_FUDGE 2.2**.
   - RecoverFromOffMap delay frames = `ceil(radius / maxSpeed)` ≈ **69**.
   - Host-testable: formula constants; B52 radius / reapproach / recover frames.
   - Fail-closed: not full locomotor damage-state matrix / pathfinder turn arcs.
2. **MaxAttempts ConsiderNewApproach residual** (`HostCargoPlaneFlight`):
   - Leave DeliveryDistance band mid-stagger (items incomplete) → re-approach.
   - Re-approach waypoint = position + heading × minReApproachDist.
   - `reapproach_count` increments; when **> MaxAttempts (4)** → HeadOffMap give-up.
   - Delivery phases fly-through on residual heading (not home-to-target).
   - Host-testable: band exit → ConsideringNewApproach; 4 re-approaches then Departing.
   - Fail-closed: not full AI_MOVE_TO pathfinder / setAllowInvalidPosition locomotor.
3. **isOffMap / HeadOffMap / RecoverFromOffMap residual**:
   - isOffMap residual: XZ outside residual map extent (no Z).
   - HeadOffMap: fly straight on heading until off-map → Complete; `accepting_commands=false`.
   - RecoverFromOffMap: hide + turn-radius frame delay → closest-edge re-entry → Approaching.
   - Host-testable: depart completes off-map; recover unhides at PreferredHeight edge.
   - Fail-closed: not full Partition unRegister / drawable GPU hide / TerrainLogic border.
4. Tests (not log-only):
   - `calc_min_turn_radius_b52_residual`
   - `consider_new_approach_max_attempts_residual`
   - `head_off_map_and_recover_from_off_map_residual`
   - `is_off_map_and_reapproach_point_residual`
   - updated CreateAtEdge flight residual (HeadOffMap depart still host-testable)

**Still residual (fail-closed, not claimed):**
- Full CreateAtEdge AmericaJetCargoPlane Object / full pathfinder locomotor matrix
  (calcMinTurnRadius + MaxAttempts re-approach + HeadOffMap/Recover residual closed
  2026-07-13)
- Full DropVariance RNG stream for non-carpet OCLs (carpet residual closed;
  supply OCL has none)
- Full AmericaCrateParachute container Object / W3D pristine bone extract GPU
- Full CollideModule partition / Anim2DCollection GPU / InGameUI floating text draw
- Full Campaign.ini parse into Main manager (seeded residual table closed 2026-07-13)
- Full W3D pristine bone extract for cargo plane doors (DOOR_1 condition residual closed)
- Full ControlBar OCL timer UI / SabotageSupplyDropzone timer-reset (retail saboteur
  module commented in base INI)
- Network DeliverPayload / MoneyCrate / CreateAtEdge / re-approach replication
  (network deferred)

## Residual Host Playability — CreateAtEdge Flight + Crate Bone Attach + Money Floating Text (2026-07-13)
**Closed (host-testable CreateAtEdge cargo-plane flight + AmericaCrateParachute bones + floating cash text residual):**
1. **CreateAtEdge AmericaJetCargoPlane flight residual** (`host_deliver_payload`):
   - Edge spawn residual via closest-edge on residual map extent (XZ) at
     PreferredHeight **100** (StartAtPreferredHeight **Yes**).
   - B52Locomotor Speed **125**/sec → **~4.167**/frame approach toward target.
   - `isCloseEnoughToTarget` residual: DeliveryDistance **410** (+ PreOpen when inbound).
   - Flight phases: EdgeSpawn → Approaching → InDeliveryBand → DoorOpening →
     Delivering → Departing/Complete.
   - Door residual: DoorDelay → MODELCONDITION `DOOR_1_OPENING` / `AVCargoPln_A2`.
   - Honesty: model `AVCargoPln`, ExitBone `WeaponA01`, ExitPitchRate **30**,
     StartAtMaxSpeed **Yes**, MaxAttempts **4**.
   - Host-testable: edge spawn Y=100; approach into band; door open; complete departs.
   - Fail-closed: not full aircraft Object / pathfinder re-approach / calcMinTurnRadius
     / off-map recover / W3D door GPU.
2. **AmericaCrateParachute bone attach residual** (`host_deliver_payload`):
   - PARA_COG / PARA_ATTCH pristine bone residual (GeometryHeight **10** layout).
   - Crate hang height-fallback: SupplyDropZoneCrate GeometryHeight **12** (no PARA_MAN).
   - Open-chute sway residual about PARA_COG (presentation; logic hang unswayed).
   - Built each open-chute residual tick for cargo crates.
   - Host-testable: COG above ATTCH; hang below origin; open sway non-zero.
   - Fail-closed: not full W3D pristine bone extract / container Object GPU.
3. **Money floating cash text residual** (`host_money_crate`):
   - Host `+$N` presentation at crate pos + Z offset **20** (green RGBA 0,255,0,255).
   - GameText key honesty `GUI:AddCash` (caption not fully localized).
   - Recorded on unit MoneyCrateCollide residual collect (with MoneyPickUp Anim2D).
   - Host-testable: floating text on pickup; amount/color/key constants.
   - Fail-closed: not full InGameUI draw / Unicode GameText / EVA voice events.
4. Tests (not log-only):
   - `create_at_edge_closest_edge_and_preferred_height`
   - `is_close_enough_delivery_band_inbound_preopen`
   - `cargo_plane_flight_create_at_edge_approach_and_door`
   - `crate_parachute_bone_attach_residual`
   - updated `supply_drop_zone_residual_credits_cash_on_interval` (CreateAtEdge + bones)
   - updated `money_crate_collide_unit_pickup_residual` (floating text)
   - updated `money_crate_above_terrain_and_forbidden_kindof_residual` (bones + float)
   - host_money_crate floating text unit tests

**Still residual (fail-closed, not claimed):**
- Full CreateAtEdge AmericaJetCargoPlane Object / full pathfinder locomotor matrix
  (edge spawn + approach band + door residual closed 2026-07-13; calcMinTurnRadius +
  MaxAttempts re-approach + HeadOffMap/Recover residual closed 2026-07-13 — see
  calcMinTurnRadius + MaxAttempts Re-Approach + Off-Map Recover section)
- Full DropVariance RNG stream for non-carpet OCLs (carpet residual closed;
  supply OCL has none)
- Full AmericaCrateParachute container Object / W3D pristine bone extract GPU
  (PARA_COG/PARA_ATTCH host residual closed 2026-07-13)
- Full CollideModule partition / Anim2DCollection GPU / InGameUI floating text draw
  (MoneyPickUp + floating cash presentation residual closed 2026-07-13)
- Full Campaign.ini parse into Main manager (seeded residual table closed 2026-07-13)
- Full W3D pristine bone extract for cargo plane doors (DOOR_1 condition residual closed)
- Network DeliverPayload / MoneyCrate / CreateAtEdge flight replication
  (network deferred)

## Residual Host Playability — Crate Parachute Fall + MoneyPickUp Anim2D + Carpet DropVariance (2026-07-13)
**Closed (host-testable AmericaCrateParachute fall-physics + MoneyPickUp Anim2D + CarpetBomb DropVariance residual):**
1. **AmericaCrateParachute cargo fall-physics residual** (`host_deliver_payload`):
   - Spawn at B52 PreferredHeight **100** + DropOffset Y:-5 → host Y **95**.
   - Freefall until fallen `ParachuteOpenDist` **12.5**, then open chute (slower sink).
   - Low-altitude open fudge residual: start − ground ≥ **2×** OpenDist.
   - `ParachuteDirectly = Yes` residual honesty; open audio `ParachuteOpen`.
   - Host-testable: elevated spawn + parachuting; OpenDist open; land clears chute;
     unit MoneyCrateCollide blocked while airborne; BuildingPickup still legal.
   - Fail-closed: not full PutInContainer AmericaCrateParachute Object / W3D bones /
     CrateParachuteLocomotor force matrix / PreferredHeight aircraft Object.
2. **MoneyCrateCollide ForbiddenKindOf + above-terrain + MoneyPickUp residual**:
   - ForbiddenKindOf **PROJECTILE** residual (+ parachuting pickers rejected).
   - Above-terrain residual: unit path blocked while crate airborne (C++
     `isAboveTerrain` except BuildingPickup).
   - ExecuteAnimation residual: `MoneyPickUp` Anim2D presentation descriptor
     (display **4.0**s, ZRise **15**, fades **Yes**) — presentation state, not GPU.
   - Host-testable: airborne unit reject honesty; MoneyPickUp on collect; PROJECTILE
     cannot pick up.
   - Fail-closed: not full CollideModule partition pairs / Anim2DCollection GPU /
     InGameUI world-anim draw / EVA floating text / science gate matrix.
3. **CarpetBomb DropVariance residual** (`special_power_strikes`):
   - Retail OCL DropVariance X:**30** Y:**40** Z:**0** (C++ X/Y → host X/Z).
   - Deterministic host scatter residual in ±variance (not GameLogicRandomValueReal).
   - Applied per bomb epicenter on line formation residual.
   - Host-testable: scatter bounds; Z lateral non-zero; damage still hits variance
     points; zero variance identity (supply OCL has none).
   - Fail-closed: not full AmericaJetB52 CreateAtEdge / RNG stream / per-bomb
     DropDelay stagger flight path.
4. Tests (not log-only):
   - `crate_parachute_open_dist_and_sink_residual`
   - `money_crate_above_terrain_and_forbidden_kindof_residual`
   - updated `supply_drop_zone_residual_credits_cash_on_interval` (parachute land)
   - updated `money_crate_collide_unit_pickup_residual` (MoneyPickUp anim)
   - updated `queue_and_stagger_supply_drop_cargo` (elevated spawn Y)
   - `carpet_bomb_drop_variance_residual_bounds`
   - updated `carpet_bomb_params_match_retail_multi_strike` / delayed damage
   - host_money_crate MoneyPickUp unit tests

**Still residual (fail-closed, not claimed):**
- Full CreateAtEdge AmericaJetCargoPlane Object / pathfinder re-approach /
  calcMinTurnRadius / off-map recover (edge spawn + DeliveryDistance band + door
  residual closed 2026-07-13 — see CreateAtEdge Flight + Crate Bone Attach section)
- Full DropVariance RNG stream for non-carpet OCLs (carpet residual closed;
  supply OCL has none)
- Full AmericaCrateParachute container Object / W3D pristine bone extract GPU
  (OpenDist freefall/open/land + PARA_COG/PARA_ATTCH host residual closed 2026-07-13
  — see CreateAtEdge Flight + Crate Bone Attach section)
- Full CollideModule partition / Anim2DCollection GPU / InGameUI floating text draw
  (MoneyPickUp + floating cash + ForbiddenKindOf + above-terrain residual closed
  2026-07-13 — see CreateAtEdge Flight + Crate Bone Attach section)
- Full Campaign.ini parse into Main manager (seeded residual table closed 2026-07-13)
- Full W3D door GPU for cargo plane (DOOR_1 condition residual closed 2026-07-13)
- Network DeliverPayload / MoneyCrate / CreateAtEdge / carpet DropVariance
  replication (network deferred)

## Residual Host Playability — DropDelay Stagger + MoneyCrateCollide + Campaign.ini Table (2026-07-13)
**Closed (host-testable DeliverPayload DropDelay stagger + MoneyCrateCollide + Campaign.ini residual):**
1. **DeliverPayload DropDelay per-item stagger residual** (`host_deliver_payload`):
   - Retail OCL `DropDelay = 350` ms → **11** frames between items.
   - AmericaJetCargoPlane `DoorDelay = 500` ms → **15** frames before first item.
   - First item frame = activate + approach (**90**) + door (**15**); item *i* = first + *i*×11.
   - Host spawns **one** payload item per due frame (C++ DeliveringState exit tick).
   - **DropOffset** residual X:0 Y:0 Z:-5 applied (host Y-up → Y=-5).
   - Honesty constants: MaxAttempts **4**, PreOpenDistance **0** (supply OCL),
     DeliveryDistance **410**, Paradrop PreOpenDistance **300**.
   - Host-testable: no crates before first item; 1 crate on first due; full 6 after stagger;
     bulk BuildingPickup cash on **final** item only.
   - Fail-closed: not full CreateAtEdge cargo-plane Object / flight locomotor /
     DropVariance (supply OCL has none) / VisiblePayload bones / parachute fall physics
     (AmericaCrateParachute fall residual closed 2026-07-13 — see Crate Parachute Fall
     + MoneyPickUp Anim2D + Carpet DropVariance section).
2. **MoneyCrateCollide unit + BuildingPickup residual** (`host_money_crate`):
   - MoneyProvided **250**, SupplyLines boost **+25**, BuildingPickup **Yes**.
   - Unit residual: non-structure non-neutral within radius **20** credits cash + destroys crate.
   - BuildingPickup residual radius **80** (structure collect path).
   - Supply Drop Zone bulk BuildingPickup residual still credits $1500/$1650 on mission
     complete and marks crates paid (no unit double-credit).
   - Host-testable: unit pickup cash + CrateMoney audio; SupplyLines boost residual.
   - Fail-closed: not full CollideModule partition pairs / Anim2D MoneyPickUp / EVA text
     (MoneyPickUp presentation + ForbiddenKindOf + above-terrain residual closed
     2026-07-13 — see Crate Parachute Fall + MoneyPickUp Anim2D section).
3. **Campaign.ini residual mission table** (Main `CampaignManager`):
   - USA campaign MD_USA01…MD_USA05 residual chain + required-mission links.
   - CHALLENGE_0 residual map chain (GC_Chem…GC_ChinaBoss) + `usa_gen_01` alias.
   - Honesty: `honesty_campaign_ini_table_ok()`.
   - Fail-closed: not full Campaign.ini INI parse / GameClient manager parity /
     end-to-end cinematic score-screen flow.
4. Tests (not log-only):
   - `queue_and_stagger_supply_drop_cargo` / `drop_delay_stagger_item_frames`
   - updated `supply_drop_zone_residual_credits_cash_on_interval` (stagger timing)
   - updated `deliver_payload_cargo_residual_constants_and_skip` (DoorDelay/MaxAttempts)
   - `money_crate_collide_unit_pickup_residual`
   - `campaign_ini_residual_mission_table`
   - host_money_crate unit tests

**Still residual (fail-closed, not claimed):**
- Full CreateAtEdge AmericaJetCargoPlane Object / DeliverPayloadAIUpdate flight
  state machine / approach geometry (constants + DoorDelay/DropDelay stagger closed)
- Full DropVariance random scatter (supply OCL has none; carpet residual closed
  2026-07-13 — see Crate Parachute Fall + MoneyPickUp Anim2D + Carpet DropVariance)
- Full AmericaCrateParachute container fall-physics for cargo
  (host residual closed 2026-07-13 — see Crate Parachute Fall section)
- Full CollideModule partition / Anim2D / ForbiddenKindOf matrix beyond residual gates
  (MoneyPickUp + PROJECTILE + above-terrain host residual closed 2026-07-13)
- Full Campaign.ini parse into Main manager (seeded residual table closed 2026-07-13)
- Full W3D pristine bone extract for cargo plane doors
- Network DeliverPayload / MoneyCrate / campaign replication (network deferred)

## Residual Host Playability — DeliverPayload Cargo Plane Delayed Spawn (2026-07-13)
**Closed (host-testable DeliverPayload cargo residual — delayed payload spawn):**
1. **DeliverPayload cargo plane residual** (`host_deliver_payload`):
   - Models retail OCL `DeliverPayload` cargo missions without full aircraft
     edge-spawn / locomotor flight / door animation.
   - Approach delay residual **90** frames @ 30 FPS, then spawn payload units
     at target in line formation.
   - Retail OCL honesty constants: Transport `AmericaJetCargoPlane`,
     PutInContainer `AmericaCrateParachute` / `AmericaParachute`,
     DropDelay **350** ms → **11** frames (stagger fail-closed; constant retained),
     DeliveryDistance **410**, Payload count **6** for supply crates.
   - Host-testable: queue → inbound; approach delay → spawn; transport name honesty.
   - Fail-closed: not full CreateAtEdge aircraft Object / DeliverPayloadAIUpdate
     state machine / DropDelay per-item stagger / parachute fall physics.
2. **America Supply Drop Zone cargo path residual**:
   - OCL interval (**3600** frames) queues cargo DeliverPayload flight (not
     immediate cash).
   - After approach delay: spawn **6** residual crates near zone + BuildingPickup
     residual cash **$1500** (SupplyLines **$1650**).
   - Template guard: crate / cargo / parachute names do **not** match zone
     structure residual (false-positive fix).
   - Honesty: `honesty_supply_drop_zone_flight_ok`,
     `honesty_supply_drop_cargo_deliver_payload_ok`,
     `honesty_deliver_payload_cargo_ok`.
   - Host-testable: no cash/crates during approach; spawn + cash on drop frame.
   - Fail-closed: not full MoneyCrateCollide unit path / ControlBar OCL timer UI.
3. **America Paradrop DeliverPayload cargo bookkeeping residual**:
   - `queue_paradrop` also records AmericaParadrop cargo mission residual
     (AmericaJetCargoPlane honesty). Infantry spawn remains host_paradrop.
   - Host-testable: cargo mission completes with paradrop drop frame.
4. Tests (not log-only):
   - `supply_drop_zone_residual_credits_cash_on_interval` (cargo delay + crates)
   - `deliver_payload_cargo_residual_constants_and_skip`
   - `host_deliver_payload` unit tests (queue/spawn/constants)
   - updated supply drop zone template false-positive guard
   - `america_paradrop_host_path_queues_and_spawns_infantry` (cargo bookkeeping)

**Still residual (fail-closed, not claimed):**
- Full CreateAtEdge AmericaJetCargoPlane Object / DeliverPayloadAIUpdate flight
  state machine (DropDelay stagger + DoorDelay + MaxAttempts/PreOpenDistance/DropOffset
  constants residual closed 2026-07-13 — see DropDelay Stagger + MoneyCrateCollide section)
- Full DropVariance / VisiblePayload bone matrix (supply OCL has no DropVariance)
- Full AmericaCrateParachute / AmericaParachute container fall-physics for cargo
  (PARA_COG host residual already closed for eject path)
- Full CollideModule partition / Anim2D MoneyPickUp (MoneyCrateCollide unit residual
  closed 2026-07-13 — see DropDelay Stagger + MoneyCrateCollide section)
- Full W3D pristine bone extract for cargo plane doors
- Network DeliverPayload / cargo replication (network deferred)

## Residual Host Playability — CamoNet Sub-Object + Physical Soldier Attach + Vision Mood + SegLine (2026-07-13)
**Closed (host-testable CamoNetting sub-object + stinger soldier attach + Partition vision mood + SegLine UV residual):**
1. **CamoNetting sub-object net mesh residual** (presentation state, not GPU):
   - Host residual mesh name honesty `CamoNet` (`CAMO_NETTING_SUB_OBJECT_MESH_NAME`).
   - Upgrade_GLACamoNetting apply → `camo_net_sub_object_shown` + presentation
     descriptor (`HostCamoNetSubObject`: shown / opacity / heat_vision_pass /
     StealthLook).
   - Enemy-undetected Invisible StealthLook → observer-hidden residual mesh;
     detected residual → heat-vision second-pass + observer-visible.
   - Host-testable: upgrade shows mesh; Invisible hides; detected heat-vision pass.
   - Fail-closed: not full W3D SubObjectsUpgrade / mesh GPU / second-pass shader.
2. **Physical SpawnBehavior soldier attach residual** (GLAStingerSite):
   - SpawnPoint bone Z-rotation facing residual (outward `atan2` ring layout).
   - `orderSlavesToAttackTarget` / `orderSlavesToGoIdle` residual on roster.
   - Live residual fire path orders alive slaves at target ObjectId.
   - `HostHiveSlaveAttach` presentation: world XZ + facing + AI order + template
     `GLAInfantryStingerSoldier`.
   - Host-testable: facings on init; fire → order; attach presentation positions.
   - Fail-closed: not full GLAInfantryStingerSoldier Object / full AI module /
     W3D model bone attach GPU.
3. **Partition / AI vision mood range residual** (Strategy Center Bombardment):
   - VisionRange **400** residual filter on mood-target acquire/clear
     (`strategy_center_mood_vision_range`; S&D × **2.0** host-testable).
   - Acquire requires vision residual **and** StrategyCenterGun range band.
   - Host-testable: vision constants; out-of-vision illegal; gun band still gated.
   - Fail-closed: not full PartitionManager filter stack / pathfinder mood matrix.
4. **SegLineRenderer UV / polyline residual** (Patriot BinaryDataStream math path):
   - `HostSegLineRendererState`: texture `EXBinaryStream32.tga`, tile factor,
     UV scroll offset, InnerColor, width, polyline of segment endpoints.
   - Built from Line3D residual segments (Segments+1 points).
   - Host-testable: polyline length; texture/tile/UV/color honesty.
   - Fail-closed: not full WGPU SegLineRenderer texture upload / GPU draw.
5. Tests (not log-only):
   - `stinger_physical_soldier_attach_facing_order_residual`
   - updated `stinger_get_closest_slave_physical_roster_residual` (facing/order/attach)
   - updated `stinger_site_residual_dual_fire_and_ap_rockets` (order on fire)
   - updated `camo_netting_structure_attack_and_damage_reveal_residual` (sub-object)
   - updated `camo_netting_structure_stealth_delay_matrix` (sub-object matrix)
   - updated `patriot_laser_arc_segment_honesty` (SegLine polyline)
   - updated `battle_plan_constants_match_retail_residual` (vision mood)
   - updated `strategy_center_turret_mood_target_residual` (vision residual)

**Still residual (fail-closed, not claimed):**
- Full PartitionManager filter stack / pathfinder mood matrix
  (vision range + weapon band host residual closed 2026-07-13)
- Full W3DLaserDraw WGPU SegLineRenderer texture upload for Patriot assist beams
  (CPU UV polyline host residual closed 2026-07-13)
- Full VisionObjectName spawn residual (createVisionObject disabled in retail C++)
- Full W3D pristine bone extract / DeliverPayload cargo plane path
- Full physical SpawnBehavior soldier Object / full AI / W3D model GPU attach
  (facing + order + attach presentation host residual closed 2026-07-13)
- Full CamoNetting W3D sub-object mesh GPU / heat-vision shader pass
  (host sub-object presentation residual closed 2026-07-13)
- Network camo-subobject / slave-order / vision-mood / SegLine replication
  (network deferred)

## Residual Host Playability — TurretAI Sleep/Passive + PARA_COG Bones + Laser Tile/Line3D (2026-07-13)
**Closed (host-testable mood matrix + parachute bones + laser texture residual):**
1. **TurretAI mood matrix Sleep/Passive residual** (Strategy Center Bombardment):
   - C++ `AttitudeType` / `getMoodMatrixActionAdjustment(MM_Action_Idle)` residual:
     - **Sleep** → MAA_Affect_Range_IgnoreAll (no idle mood-target acquire)
     - **Passive** → MAA_Affect_Range_WaitForAttack (only `last_damage_source` retaliate)
     - **Normal/Alert/Aggressive** → free idle mood-target residual
   - Object stores `ai_attitude` + `last_damage_source` (BodyModule residual via
     `take_damage_from`).
   - Turret bone pitch/yaw drawable residual: host exposes TurretAI angles for
     presentation consumers (`HostTurretBoneDrawable`).
   - Host-testable: Sleep blocks acquire; Passive free-acquire blocked; Passive +
     last damage retaliates + aims FirePitch; bone drawable pitch honesty.
   - Fail-closed: not full PartitionManager filter stack / W3D Turret bones
     (vision mood range host residual closed 2026-07-13 — see CamoNet Sub-Object
     + Physical Soldier Attach + Vision Mood + SegLine section).
2. **AmericaParachute bone PARA_COG / PARA_ATTCH / PARA_MAN residual**:
   - Host pristine bone offsets from GeometryHeight **10** layout (no W3D extract).
   - `updateOffsetsFromBones` residual → rider attach + rider/para sway pivots.
   - `calcSwayMtx` residual: pitch/roll about pivot → rider presentation displace
     + chute COG sway delta (logic position stays attach offset; sway is drawable).
   - Host-testable: COG above ATTCH; rider attach below origin; non-zero open sway;
     closed chute delta zero.
   - Fail-closed: not full W3D pristine bone extract / DeliverPayload cargo path.
3. **W3DLaserDraw texture / Line3D residual** (PatriotBinaryDataStream math path):
   - Texture `EXBinaryStream32.tga`, Tile **Yes**, TilingScalar **0.25**,
     InnerColor green A**180**.
   - tileFactor = length/width×aspect×scalar (retail width **4**, aspect **1** →
     length 100 → **6.25**).
   - Ground-skim residual: Z = max(z, ground+**2**).
   - Host builds **20** `HostLaserLine3DSegment` descriptors (width/tile/scroll/points).
   - Host-testable: tile factor math; skim pad; 20-segment list mid-arc elevated.
   - Fail-closed: not full WGPU SegLineRenderer / texture upload.
4. **VisionObjectName residual** (document only):
   - `strategy_center_vision_object_spawn_enabled_in_retail() == false`
     (C++ `//createVisionObject();` disabled). No spawn claim.
5. Tests (not log-only):
   - `strategy_center_turret_mood_matrix_sleep_passive_residual`
   - updated `battle_plan_constants_match_retail_residual` (Sleep/Passive/bone/vision)
   - updated `air_eject_parachute_gates` (PARA_COG bone matrix)
   - updated `patriot_laser_arc_segment_honesty` (tile/Line3D/skim)

**Still residual (fail-closed, not claimed):**
- Full PartitionManager filter stack / pathfinder mood matrix / W3D Turret bone GPU
  (Sleep/Passive + bone drawable + vision mood host residual closed 2026-07-13)
- Full W3DLaserDraw WGPU SegLineRenderer / texture upload for Patriot assist beams
  (tile factor + Line3D + ground skim + SegLine UV polyline host residual closed
  2026-07-13)
- Full VisionObjectName spawn residual (createVisionObject disabled in retail C++
  — honesty residual closed 2026-07-13; no spawn claim)
- Full W3D pristine bone extract / DeliverPayload cargo plane path
  (PARA_COG host offsets + calcSwayMtx residual closed 2026-07-13)
- Full physical SpawnBehavior soldier Object / full AI / W3D model GPU attach
  (getClosestSlave + facing/order/attach presentation host residual closed
  2026-07-13)
- Full CamoNetting W3D sub-object mesh GPU / heat-vision shader pass
  (StealthLook + host sub-object presentation residual closed 2026-07-13)
- Network mood-matrix / PARA_COG / laser-tile replication (network deferred)

## Residual Host Playability — getClosestSlave + W3DLaser Arc + TurretAI Mood-Target + Camo StealthLook (2026-07-13)
**Closed (host-testable physical hive slaves + laser arc + mood-target + heat-vision residual):**
1. **Physical SpawnBehavior slave roster + getClosestSlave residual** (GLAStingerSite):
   - SpawnNumber **3** residual slots at SpawnPoint bone offsets (radius **12**, 120° ring).
   - Per-slave HP residual (MaxHealth **100**); `getClosestSlave` picks nearest alive
     slot to shooter world XZ for HiveStructureBody propagate via **host API**
     (`apply_host_hive_damage_from`) — not live skirmish `Object::take_damage` combat.
   - Kill marks slot dead; SpawnReplaceDelay respawn revives first dead slot.
   - Host-testable: closest-slot damage; independent HP; kill + respawn schedule.
   - Fail-closed: not full GLAInfantryStingerSoldier Object / AI / W3D bone attach;
     live combat still structure HP until damage-class routing is wired.
2. **W3DLaserDraw arc segment residual** (PatriotBinaryDataStream):
   - Cosine arc sample residual: mid peak = ArcHeight **30**, endpoints **0**.
   - Segments **20** residual segment endpoints host-sampled (C++ doDrawModule).
   - Host-testable: mid Z = base + ArcHeight; segment 0 start near base Z.
   - Fail-closed: not full texture / Line3D GPU draw / ground-height skim
     (tile/Line3D descriptor + ground skim host residual closed 2026-07-13 — see
     TurretAI Sleep/Passive + PARA_COG + Laser Tile section).
3. **TurretAI idle mood-target residual** (Strategy Center Bombardment):
   - `friend_checkForIdleMoodTarget` residual: idle gun acquires enemy in
     StrategyCenterGun range band (min **100** / max **400**), aims FirePitch **45**,
     flags `m_targetWasSetByIdleMood`.
   - Mood target leaving range / dying clears target so IDLESCAN can resume.
   - Host-testable: acquire + aim; out-of-range clear honesty.
   - Fail-closed: not full mood matrix Sleep/Passive / bone pitch drawable
     (Sleep/Passive + bone drawable host residual closed 2026-07-13 — see
     TurretAI Sleep/Passive + PARA_COG + Laser Tile section).
4. **CamoNetting StealthLook / heat-vision residual**:
   - Host of Drawable::setStealthLook: None / VisibleFriendly /
     VisibleFriendlyDetected / VisibleDetected / Invisible.
   - Detected residual → heat-vision second material pass opacity **1.0**.
   - Host-testable: detect cloaked structure → VISIBLE_DETECTED + opacity 1.
   - Fail-closed: not full W3D second material pass GPU / mine heat-vision hack.
5. **AlwaysHeal busy-interrupt residual**: documented as **dead code in retail C++**
   (early-return before AlwaysHeal branch). Host keeps AlwaysHeal **0.25** honesty
   constant and idle-only scan (matches unreachable retail path). Closed as
   fail-closed parity — not a missing live path.
6. Tests (not log-only):
   - `stinger_get_closest_slave_physical_roster_residual`
   - `stinger_get_closest_slave_roster_honesty` (module)
   - `patriot_laser_arc_segment_honesty` + updated binary laser residual (arc sample)
   - `strategy_center_turret_mood_target_residual`
   - updated `camo_netting_structure_attack_and_damage_reveal_residual` (StealthLook)
   - AlwaysHeal dead-path honesty in `auto_find_healing_gates`

**Still residual (fail-closed, not claimed):**
- Full TurretAI mood matrix Sleep/Passive / bone pitch drawable matrix
  (host residual closed 2026-07-13 — see TurretAI Sleep/Passive + PARA_COG section)
- Full W3DLaserDraw texture / Line3D GPU draw for Patriot assist beams
  (tile/Line3D host residual closed 2026-07-13 — see TurretAI Sleep/Passive section)
- Full VisionObjectName spawn residual (createVisionObject disabled in retail C++)
- Full AmericaParachute bone PARA_COG / DeliverPayload residual
  (PARA_COG host residual closed 2026-07-13 — see TurretAI Sleep/Passive section)
- Full physical SpawnBehavior soldier Object / AI / W3D model attach
  (getClosestSlave + per-slave HP/position host residual closed 2026-07-13)
- Full CamoNetting sub-object net mesh visual / W3D heat-vision GPU pass
  (StealthLook + second-pass opacity host residual closed 2026-07-13)
- Network mood-target / closest-slave / laser-arc / heat-vision replication
  (network deferred)

## Residual Host Playability — Parachute Pitch/Roll Sway + LaserUpdate Endpoint Track (2026-07-13)
**Closed (host-testable AmericaParachute pitch/roll sway + Patriot LaserUpdate residual):**
1. **AmericaParachute pitch/roll sway residual** (ParachuteContain + ParachuteLocomotor):
   - PitchRateMax / RollRateMax **60** deg/s → **π/90** rad/frame seed band.
   - Deterministic host seed: pitch **+½ max**, roll **−½ max** on chute open.
   - Open-chute spring/damper residual each frame:
     stiffness **0.02**, damping **0.01** (ParachuteLocomotor).
   - LowAltitudeDamping **0.2** when height ≤ **20** (ALTITUDE_DAMP_START).
   - Freefall residual does not sway; land clears pitch/roll state.
   - Host-testable: freefall zero; open → non-zero pitch/roll; land clear.
   - Fail-closed: not full bone PARA_COG / rider sway / DeliverPayload matrix.
2. **LaserUpdate endpoint track + W3DLaserDraw param residual**
   (PatriotBinaryDataStream assist beams):
   - Each frame refreshes start/end from live `from_id` / `to_id` positions
     (C++ `LaserUpdate::updateStartPos` / `updateEndPos` without bone).
   - Dead/missing target freezes last end residual.
   - W3DLaserDraw honesty: NumBeams **1**, ScrollRate **-0.25**, ArcHeight **30**,
     InnerBeamWidth **4**, Segments **20**; host advances scroll residual/frame.
   - Host-testable: endpoint follows moved victim; scroll advances; lifetime still 18.
   - Fail-closed: not full texture / arc segment GPU draw.
3. Tests (not log-only):
   - `eject_pilot_parachute_pitch_roll_sway_residual`
   - updated `patriot_assist_binary_data_stream_laser_residual`
     (endpoint track + ScrollRate + draw params)
   - module unit tests in `host_usa_pilot` / `host_base_defense`
     (sway matrix / LowAltitudeDamping / track + freeze / scroll)

**Still residual (fail-closed, not claimed):**
- Full TurretAI mood-target / bone pitch matrix
- Full W3DLaserDraw texture / arc GPU draw for Patriot assist beams
  (endpoint track + draw-param honesty host residual closed 2026-07-13)
- Full VisionObjectName spawn residual (createVisionObject disabled in retail C++)
- Full AmericaParachute bone PARA_COG / DeliverPayload residual
  (pitch/roll spring-damper host residual closed 2026-07-13)
- Full AutoFindHealingUpdate AlwaysHeal busy-interrupt path (dead code in retail C++)
- Full physical SpawnBehavior slave objects / HiveStructureBody getClosestSlave matrix
- Full CamoNetting sub-object net visual / W3D heat-vision drawable matrix
- Network parachute-sway / laser-endpoint replication (network deferred)

## Residual Host Playability — TurretAI HoldTurret/Idle-Recenter + CamoNetting FriendlyOpacity (2026-07-13)
**Closed (host-testable Strategy Center TurretAI HoldTurret + idle-recenter + CamoNetting FriendlyOpacity residual):**
1. **TurretAI HoldTurret + idle-recenter residual** (Strategy Center Turret):
   - After idle-scan complete: HOLD for RecenterTime default
     **2×LOGICFRAMES_PER_SECOND → 60** frames (Strategy Center does not override).
   - Angles freeze at scan desired during Hold; busy cancels hold.
   - Hold complete → RECENTER residual steps pitch/yaw to NaturalTurretAngle
     **-90** / NaturalTurretPitch **45** at Turn/PitchRate **2** deg/frame.
   - Idle-recenter complete schedules next idle-scan (C++ IDLESCAN → HOLD →
     RECENTER → IDLE residual chain).
   - Host-testable: hold freeze; elapse → recenter; natural + reschedule; busy cancel.
   - Fail-closed: not full TurretAI mood-target / bone pitch matrix.
2. **CamoNetting FriendlyOpacity residual** (StealthUpdate on CamoNetting structures):
   - FriendlyOpacityMin **50%** / Max **100%** residual on cloaked / revealed.
   - Upgrade apply + re-cloak set min; attack/damage reveal sets max.
   - Cloaked pulse residual: `min + (max-min)×(0.5+0.5·sin(phase))`,
     phase rate **0.2** (C++ setEffectiveOpacity sin path mapped to min..max).
   - Host-testable: cloak min; damage reveal max; re-cloak + pulse in range.
   - Fail-closed: not full W3D heat-vision / sub-object camo net visual matrix.
3. Tests (not log-only):
   - `strategy_center_turret_hold_and_idle_recenter_residual`
   - updated `strategy_center_turret_idle_scan_residual` (Hold after complete)
   - updated `camo_netting_structure_attack_and_damage_reveal_residual`
     (FriendlyOpacity min/max/pulse)
   - module unit tests in `host_strategy_center` / `host_upgrades`
     (RecenterTime / Hold elapsed / FriendlyOpacity / pulse matrix)

**Still residual (fail-closed, not claimed):**
- Full TurretAI mood-target / bone pitch matrix
- Full W3DLaserDraw / LaserUpdate client drawable for Patriot assist beams
- Full VisionObjectName spawn residual (createVisionObject disabled in retail C++)
- Full AmericaParachute sway / pitch-roll / DeliverPayload residual
- Full AutoFindHealingUpdate AlwaysHeal busy-interrupt path (dead code in retail C++)
- Full physical SpawnBehavior slave objects / HiveStructureBody getClosestSlave matrix
- Full CamoNetting sub-object net visual / W3D heat-vision drawable matrix
- Network HoldTurret / camo-opacity replication (network deferred)

## Residual Host Playability — TurretAI Idle-Scan + CamoNetting USING_ABILITY/OrderIdle (2026-07-13)
**Closed (host-testable Strategy Center TurretAI idle-scan + CamoNetting USING_ABILITY / OrderIdleEnemies residual):**
1. **TurretAI idle-scan residual** (Strategy Center AIUpdateInterface Turret):
   - MinIdleScanInterval **500**ms → **15** frames, MaxIdleScanInterval
     **1000**ms → **30** frames.
   - MinIdleScanAngle **0**, MaxIdleScanAngle **60** deg off NaturalTurretAngle.
   - Bombardment ACTIVE schedules first scan after Min interval; idle gun
     rotates toward NaturalTurretAngle ± deterministic mid offset (**±30**),
     pitch holds NaturalTurretPitch **45**.
   - Complete enters HoldTurret residual (see HoldTurret/Idle-Recenter section).
   - Busy (attacking / target / recenter / fire) cancels mid-scan residual.
   - Host-testable: schedule; step off natural; complete → hold; busy cancel.
   - Fail-closed: not full TurretAI mood-target / bone pitch matrix.
2. **CamoNetting USING_ABILITY + OrderIdleEnemies residual**:
   - StealthForbiddenConditions residual now includes **USING_ABILITY**
     (`OBJECT_STATUS_IS_USING_ABILITY` / SpecialAbility AI residual).
   - OrderIdleEnemiesToAttackMeUponReveal residual: on CamoNetting structure
     reveal, idle enemy units within their vision range wake and AttemptToTarget
     (host residual: set target + Attacking).
   - Host-testable: using_ability uncloaks; idle enemy orders on reveal.
   - Fail-closed: not full sub-object camo net visual (FriendlyOpacity host residual closed 2026-07-13).
3. Tests (not log-only):
   - `strategy_center_turret_idle_scan_residual`
   - updated `camo_netting_structure_attack_and_damage_reveal_residual`
     (USING_ABILITY + OrderIdle)
   - module unit tests in `host_strategy_center` / `host_upgrades`
     (idle-scan matrix / USING_ABILITY / OrderIdle range gates)

**Still residual (fail-closed, not claimed):**
- Full TurretAI mood-target / bone pitch matrix
  (HoldTurret + idle-recenter host residual closed 2026-07-13 — see HoldTurret section)
- Full W3DLaserDraw / LaserUpdate client drawable for Patriot assist beams
- Full VisionObjectName spawn residual (createVisionObject disabled in retail C++)
- Full AmericaParachute sway / pitch-roll / DeliverPayload residual
- Full AutoFindHealingUpdate AlwaysHeal busy-interrupt path (dead code in retail C++)
- Full physical SpawnBehavior slave objects / HiveStructureBody getClosestSlave matrix
- Full CamoNetting sub-object net visual / W3D heat-vision drawable matrix
  (FriendlyOpacity host residual closed 2026-07-13 — see HoldTurret + FriendlyOpacity section)
- Network idle-scan / camo-order-idle replication (network deferred)

## Residual Host Playability — Turret Natural Pitch/Yaw + Parachute FreeFallDamage (2026-07-13)
**Closed (host-testable Strategy Center turret natural-position angles + AmericaParachute FreeFallDamage/fudge residual):**
1. **Turret natural-position pitch/yaw residual** (Strategy Center AIUpdateInterface Turret):
   - NaturalTurretAngle **-90**, NaturalTurretPitch **45**, FirePitch **45**.
   - TurretTurnRate / TurretPitchRate **60** deg/s → **2** deg/frame @ 30 FPS.
   - StrategyCenterGun fire residual aims pitch/yaw at target (atan2 + FirePitch).
   - `isTurretInNaturalPosition` residual: natural angles + idle/busy coast gate.
   - Recenter residual steps angles toward natural; frame count is angle-based
     (60° → 30 frames) with busy-coast fallback of **30** frames.
   - Host-testable: fire leaves natural; recenter restores NaturalTurretAngle/Pitch;
     pack deferred for non-natural; pack immediate when natural.
   - Fail-closed: not full TurretAI idle-scan state machine / bone pitch matrix.
2. **AmericaParachute FreeFallDamage + low-altitude open fudge residual**:
   - FreeFallDamagePercent default **0.5** → 50% max-health residual when chute
     is destroyed mid-air while significantly above terrain
     (`destroy_eject_parachute_midair` residual).
   - Low-altitude open fudge: if startZ − ground < **2×OpenDist**, fudge start
     height to `ground + 2×OpenDist` so chute can still open (C++ ParachuteContain).
   - Host-testable: fudge rewrites start height; FreeFallDamage closes chute + HP;
     ground path rejects FreeFallDamage.
   - Fail-closed: not full sway / pitch-roll / DeliverPayload bone matrix.
3. Tests (not log-only):
   - `eject_pilot_parachute_open_fudge_and_free_fall_damage_residual`
   - updated `strategy_center_bombardment_turret_fire_residual` (aim angles)
   - updated `strategy_center_delayed_set_battle_plan_and_turret_recenter_residual`
     (angle step + restore)
   - module unit tests in `host_strategy_center` / `host_usa_pilot`
     (angle matrix / fudge / FreeFallDamage gates)

**Still residual (fail-closed, not claimed):**
- Full TurretAI idle-scan Min/MaxIdleScanAngle state machine / bone matrix
  (host residual closed 2026-07-13 — see TurretAI Idle-Scan + CamoNetting section;
  full HoldTurret / mood-target / bone pitch still open)
- Full W3DLaserDraw / LaserUpdate client drawable for Patriot assist beams
- Full VisionObjectName spawn residual (createVisionObject disabled in retail C++)
- Full AmericaParachute sway / pitch-roll / DeliverPayload residual
- Full AutoFindHealingUpdate AlwaysHeal busy-interrupt path (dead code in retail C++)
- Full physical SpawnBehavior slave objects / HiveStructureBody getClosestSlave matrix
- Full CamoNetting USING_ABILITY / OrderIdleEnemies / opacity drawable matrix
  (USING_ABILITY + OrderIdle host residual closed 2026-07-13 — see TurretAI Idle-Scan
  + CamoNetting section; opacity / sub-object visual still open)
- Network turret-angle / FreeFallDamage replication (network deferred)

## Residual Host Playability — BinaryDataStream Laser + DetectionRate Sleep (2026-07-13)
**Closed (host-testable Patriot assist laser feedback + StealthDetector DetectionRate residual):**
1. **BinaryDataStream laser residual** (`AssistedTargetingUpdate::makeFeedbackLaser`):
   - On assist accept, residual spawns two beams with template
     `PatriotBinaryDataStream`.
   - `LaserFromAssisted`: requestor → assistant.
   - `LaserToTarget`: assistant → victim.
   - DeletionUpdate Min/MaxLifetime **600**ms → **18** frames residual lifetime.
   - Host-testable: pair endpoints; lifetime expiry; honesty counters.
   - Fail-closed: not full W3DLaserDraw / LaserUpdate client drawable
     (arc / scroll / texture / NumBeams).
2. **StealthDetectorUpdate DetectionRate residual** (Strategy Center ModuleTag_16):
   - DetectionRate **500**ms → **15** frames sleep between scans.
   - `setSDEnabled(true)` residual: first scan immediate (`next_scan_frame = 0`).
   - After scan: `next_detection_scan_frame = frame + 15`.
   - `markAsDetected(updateRate + 1)` residual hold **16** frames.
   - Host-testable: mid-rate no re-scan; re-scan at +15; hold honesty.
   - Fail-closed: VisionObjectName spawn residual still not claimed
     (createVisionObject disabled in retail C++); non-SC detectors keep
     continuous legacy scan (rate_frames=0).
3. Tests (not log-only):
   - `patriot_assist_binary_data_stream_laser_residual`
   - `strategy_center_stealth_detector_detection_rate_residual`
   - updated `patriot_assisted_targeting_request_assist_range_residual`
     (laser honesty)
   - updated `strategy_center_stealth_detector_enable_residual` (rate fields)
   - module unit tests in `host_base_defense` / `host_strategy_center`
     (laser matrix / DetectionRate gates)

**Still residual (fail-closed, not claimed):**
- Full W3DLaserDraw / LaserUpdate client drawable for Patriot assist beams
- Full AI turret pitch/yaw natural-position angle matrix (recenter frame gate closed)
- Full VisionObjectName spawn residual (createVisionObject disabled in retail C++)
- Full AmericaParachute sway / pitch-roll / DeliverPayload residual
- Full AutoFindHealingUpdate AlwaysHeal busy-interrupt path (dead code in retail C++)
- Full physical SpawnBehavior slave objects / HiveStructureBody getClosestSlave matrix
- Full CamoNetting sub-object net visual / opacity drawable matrix
- Network laser / DetectionRate replication (network deferred)

## Residual Host Playability — Stinger HiveStructureBody + CamoNetting Structure Reveal (2026-07-13)
**Closed (host-testable HiveStructureBody/SpawnBehavior + CamoNetting structure StealthUpdate residual):**
1. **HiveStructureBody + SpawnBehavior residual** (`GLAStingerSite` ModuleTag_04/06):
   - SpawnNumber **3** residual soldiers (MaxHealth **100** each).
   - Propagate residual (SMALL_ARMS / SNIPER / POISON / RADIATION / SURRENDER /
     MICROWAVE class) damages active slave, not structure HP.
   - Swallow residual (SNIPER / POISON / SURRENDER) eats damage when **0** slaves.
   - HitStructure residual always damages the building body.
   - SPAWNS_ARE_THE_WEAPONS residual: site cannot fire with **0** soldiers.
   - SpawnReplaceDelay **30000**ms → **900** frames residual respawn.
   - Host-testable: propagate preserves structure HP; kill slave; swallow at 0;
     HitStructure damages; 0-soldier no-fire; respawn restores capacity.
   - Fail-closed: not full physical slave objects / bone attach / getClosestSlave.
2. **CamoNetting structure attack/damage reveal residual** (`StealthUpdate` +
   `Upgrade_GLACamoNetting` on Tunnel Network / Stinger / Slth buildings):
   - StealthForbiddenConditions residual: **ATTACKING** + **TAKING_DAMAGE**.
   - StealthDelay **2500**ms → **75** frames re-cloak residual.
   - Host-testable: damage uncloaks; attacking uncloaks; idle after delay re-cloaks.
   - Fail-closed: not full USING_ABILITY / opacity / OrderIdleEnemiesToAttackMe /
     sub-object camo net visual.
3. Tests (not log-only):
   - `stinger_hive_structure_body_and_spawn_respawn_residual`
   - `camo_netting_structure_attack_and_damage_reveal_residual`
   - updated `stinger_site_residual_dual_fire_and_ap_rockets` (SpawnNumber honesty)
   - module unit tests in `host_base_defense` / `host_upgrades` (hive matrix /
     StealthDelay residual honesty)

**Still residual (fail-closed, not claimed):**
- Full BinaryDataStream laser drawable feedback for Patriot assist beams
- Full AI turret pitch/yaw natural-position angle matrix (recenter frame gate closed)
- Full VisionObjectName spawn residual (createVisionObject disabled in retail C++)
- Full AmericaParachute sway / pitch-roll / DeliverPayload residual
- Full AutoFindHealingUpdate AlwaysHeal busy-interrupt path (dead code in retail C++)
- Full physical SpawnBehavior slave objects / HiveStructureBody getClosestSlave matrix
- Full CamoNetting sub-object net visual / opacity drawable matrix
- Network hive / camo-structure-stealth replication (network deferred)

## Residual Host Playability — Patriot AssistedTargeting + DemoTrap Mode (2026-07-13)
**Closed (host-testable AssistedTargetingUpdate + DemoTrapUpdate weapon-slot mode residual):**
1. **AssistedTargetingUpdate residual** (AmericaPatriotBattery ModuleTag_07):
   - PRIMARY/AA fire with `RequestAssistRange` **200** issues assist request residual.
   - Same-team equivalent Patriots (stock / Lazr / SupW family) within range that are
     free to assist accept a clip of **AssistingClipSize = 4** assist-weapon shots
     (`PatriotMissileAssistWeapon` dmg **25** / range **450**; Lazr dmg **35**;
     SupW dmg **25** + EMPPatriotEffectSpheroid residual).
   - Clip fires on DelayBetweenShots **250**ms → **8** frames residual cadence.
   - Host-testable: near assistant accepts + damages; far (range > 200) rejects;
     Lazr↔stock non-equivalent rejects; AssistingClipSize honesty.
   - Fail-closed: not full BinaryDataStream laser drawable feedback
     (`LaserFromAssisted` / `LaserToTarget` residual audio cue only).
2. **DemoTrapUpdate weapon-slot mode residual**:
   - DefaultProximityMode **Yes** → starts Proximity (SECONDARY residual).
   - ManualModeWeaponSlot (TERTIARY residual) disables proximity scan.
   - DetonationWeaponSlot (PRIMARY residual) → manual detonate residual.
   - Host-testable: manual mode enemy-in-range does not detonate; switch to
     Proximity detonates; Detonate mode manual-detonates.
   - Fail-closed: not full PreAttack scoop animation / weapon-lock UI matrix.
3. Tests (not log-only):
   - `patriot_assisted_targeting_request_assist_range_residual`
   - `lazr_patriot_assist_equivalent_family_residual`
   - `demo_trap_weapon_slot_mode_residual`
   - module unit tests in `host_base_defense` / `host_mines` (assist matrix /
     mode residual honesty)

**Still residual (fail-closed, not claimed):**
- Full BinaryDataStream laser drawable feedback for Patriot assist beams
- Full AI turret pitch/yaw natural-position angle matrix (recenter frame gate closed)
- Full VisionObjectName spawn residual (createVisionObject disabled in retail C++)
- Full AmericaParachute sway / pitch-roll / DeliverPayload residual
- Full AutoFindHealingUpdate AlwaysHeal busy-interrupt path (dead code in retail C++)
- Full SpawnBehavior / HiveStructureBody / Stinger soldier death matrix
  (host residual closed 2026-07-13 — see Stinger HiveStructureBody + CamoNetting section)
- Full CamoNetting structure attack reveal matrix
  (host residual closed 2026-07-13 — see Stinger HiveStructureBody + CamoNetting section)
- Network assist / demo-trap-mode replication (network deferred)

## Residual Host Playability — Delayed setBattlePlan ACTIVE + Turret Recenter (2026-07-13)
**Closed (host-testable BattlePlanUpdate delayed ACTIVE setBattlePlan + Bombardment recenter residual):**
1. **Delayed ACTIVE-after-unpack setBattlePlan residual** (`BattlePlanUpdate::setStatus`):
   - Plan select only starts door residual (UNPACKING OPENING); army buffs,
     building bonuses, StealthDetector enable, StrategyCenterGun equip apply
     **only** when door reaches ACTIVE / WAITING_TO_CLOSE (`setBattlePlan(plan)`).
   - Plan switch → PACKING / CLOSING → `setBattlePlan(NONE)` clears army/building
     residual + BattlePlanChangeParalyze (150 frames) on legal members.
   - Pack complete (TransitionIdleTime **0**) → unpack new plan → ACTIVE apply.
   - Host-testable: mid-unpack no army buffs; ACTIVE grants; pack clears before
     new ACTIVE; turret/stealth only while ACTIVE.
   - Fail-closed: not full AI turret pitch/yaw angle matrix / VisionObjectName.
2. **Bombardment turret recenter residual** (`recenterTurret` / `isTurretInNaturalPosition`):
   - Leaving Bombardment while gun residual is non-natural (attacking / has target /
     fired within **30** frames) delays pack by **TurretRecenterFrames = 30**.
   - During recenter door stays ACTIVE (buffs still applied); then PACKING clears.
   - Natural idle gun packs immediately.
   - Fail-closed: not full pitch scan / natural-position angle matrix.
3. Tests (not log-only):
   - `strategy_center_delayed_set_battle_plan_and_turret_recenter_residual`
   - updated door / buff / paralyze / turret / stealth residual tests
   - module unit tests in `host_strategy_center` (BecameActive / BeganPacking /
     recenter matrix / natural gate)

**Still residual (fail-closed, not claimed):**
- Full AI turret pitch/yaw natural-position angle matrix (recenter frame gate closed)
- Full VisionObjectName spawn residual (createVisionObject disabled in retail C++)
- Full AmericaParachute sway / pitch-roll / DeliverPayload residual
- Full AutoFindHealingUpdate AlwaysHeal busy-interrupt path (dead code in retail C++)
- Network delayed-battle-plan / recenter replication (network deferred)

## Residual Host Playability — StealthDetector Enable + BattlePlan Door Animation (2026-07-13)
**Closed (host-testable Strategy Center StealthDetectorUpdate + pack/unpack door residual):**
1. **StealthDetectorUpdate residual** (`AmericaStrategyCenter` ModuleTag_16):
   - DetectionRange **500**, DetectionRate **500**ms → **15** frames,
     InitiallyDisabled **Yes**.
   - SearchAndDestroy → `setSDEnabled(true)` residual: `is_detector` + range **500**.
   - Leaving S&D → `setSDEnabled(false)` residual: clear detector + range.
   - Host-testable: stealthed enemy at dist 400 detected under S&D; at 600 not;
     before S&D / after leave not detected.
   - Fail-closed: VisionObjectName createVisionObject disabled in retail C++
     (ShroudRevealToAllRange path); not full DetectionRate sleep phasing.
2. **Pack/unpack door model-condition residual** (`BattlePlanUpdate`):
   - AnimationTime **7000**ms → **210** frames; TransitionIdleTime **0**.
   - DOOR_1 Bombardment / DOOR_2 HoldTheLine / DOOR_3 SearchAndDestroy.
   - Select → OPENING residual; after 210 frames → WAITING_TO_CLOSE (ACTIVE).
   - Plan switch → CLOSING residual → IDLE → new OPENING (TransitionIdleTime 0).
   - Pack/unpack audio residual queued.
   - Delayed setBattlePlan ordering closed 2026-07-13 (see Delayed setBattlePlan
     ACTIVE + Turret Recenter section).
3. Tests (not log-only):
   - `strategy_center_stealth_detector_enable_residual`
   - `strategy_center_battle_plan_door_animation_residual`
   - module unit tests in `host_strategy_center` (door matrix / stealth honesty /
     AnimationTime / DetectionRange constants)

**Still residual (fail-closed, not claimed):**
- Full BattlePlanUpdate delayed ACTIVE-after-unpack setBattlePlan ordering
  (host residual closed 2026-07-13 — see Delayed setBattlePlan ACTIVE + Turret Recenter)
- Full Bombardment turret natural-position recenter / pitch scan matrix
  (recenter host residual closed 2026-07-13 — see Delayed setBattlePlan ACTIVE +
  Turret Recenter; full pitch/yaw angle matrix still open)
- Full VisionObjectName spawn residual (createVisionObject disabled in retail C++)
- Full AmericaParachute sway / pitch-roll / DeliverPayload residual
- Full AutoFindHealingUpdate AlwaysHeal busy-interrupt path (dead code in retail C++)
- Network stealth-detector / door-animation replication (network deferred)

## Residual Host Playability — Same-Player PartitionFilter + Parachute OpenDist (2026-07-13)
**Closed (host-testable PilotFindVehicle PartitionFilterPlayer + AmericaParachute OpenDist residual):**
1. **PartitionFilterPlayer residual** (`PilotFindVehicleUpdate::scanClosestTarget`):
   - C++ `PartitionFilterPlayer(me->getControllingPlayer(), true)` residual.
   - Host killpilot captures `unmanned_owner_team` then sets Neutral.
   - AI auto-scan accepts same-team or Neutral with matching owner; rejects
     foreign-owner Neutral unmanned (China sniped hull for USA pilot).
   - Player Enter recrew path is not gated (AI scan residual only).
   - Fail-closed: not full same-map PartitionFilterSameMapStatus.
2. **AmericaParachute OpenDist residual** (`ParachuteContain` / OpenDist **100**):
   - Air-ejected pilot freefalls faster (`40` u/frame) until fallen ≥ **100**.
   - Then opens chute residual (slower `20` u/frame sink + `ParachuteOpen` audio).
   - Lands and clears parachuting residual as before.
   - Fail-closed: not full sway / pitch-roll / DeliverPayload matrix.
3. Tests (not log-only):
   - `pilot_find_vehicle_same_player_partition_filter_residual`
   - `eject_pilot_parachute_open_dist_residual`
   - updated `eject_pilot_air_ocl_parachute_residual` (OpenDist honesty)
   - module unit tests in `host_usa_pilot` (same-player / OpenDist gates / honesty)

**Still residual (fail-closed, not claimed):**
- Full BattlePlanUpdate pack/unpack door model-condition / 7s animation matrix
  (host residual closed 2026-07-13 — see StealthDetector Enable + BattlePlan Door Animation;
  delayed setBattlePlan ordering closed 2026-07-13 — see Delayed setBattlePlan ACTIVE +
  Turret Recenter)
- Full Bombardment turret natural-position recenter / pitch scan matrix
  (recenter host residual closed 2026-07-13 — see Delayed setBattlePlan ACTIVE +
  Turret Recenter; full pitch/yaw still open)
- Full StealthDetectorUpdate module enable stack / VisionObjectName spawn residual
  (StealthDetector enable host residual closed 2026-07-13 — see StealthDetector + Door section;
  VisionObjectName still open / createVisionObject disabled in retail C++)
- Full AmericaParachute sway / pitch-roll / DeliverPayload residual
- Full AutoFindHealingUpdate AlwaysHeal busy-interrupt path (dead code in retail C++)
- Network same-player / parachute-open replication (network deferred)

## Residual Host Playability — Eject DieMux + PilotFindVehicle CollideModule (2026-07-13)
**Closed (host-testable EjectPilotDie DeathTypes/ExemptStatus + CollideModule residual):**
1. **DieMux residual** (`EjectPilotDie` / `DieMuxData`):
   - `DeathTypes = ALL -CRUSHED -SPLATTED`: Crushed / Splatted residual deaths
     do **not** eject (`HostDeathType` on `ObjectStatus`).
   - `ExemptStatus = HIJACKED`: hijacked vehicles do **not** eject.
   - Normal combat residual death still ejects (Veteran+ gates preserved).
   - Fail-closed: not full DeathType enum / multi-bit status matrix.
2. **PilotFindVehicle CollideModule residual** (`VeterancyCrateCollide`):
   - wouldLikeToCollideWith host gates: RequiredKindOf VEHICLE / ForbiddenKindOf
     DOZER, not significantly above terrain, not airborne locomotor, trainable,
     canGainExpForLevel(pilot levels) — Heroic vehicle rejects Veteran pilot.
   - Nearest valid Rookie unmanned still Enter → recrew residual.
   - Fail-closed: same-player PartitionFilter residual closed 2026-07-13
     (see Same-Player PartitionFilter + Parachute OpenDist section).
3. Tests (not log-only):
   - `eject_pilot_die_mux_death_types_and_hijacked_residual`
   - `pilot_find_vehicle_collide_module_would_like_residual`
   - module unit tests in `host_usa_pilot` (death types / collide gates / honesty)

**Still residual (fail-closed, not claimed):**
- Full BattlePlanUpdate pack/unpack door model-condition / 7s animation matrix
- Full Bombardment turret natural-position recenter / pitch scan matrix
- Full StealthDetectorUpdate module enable stack / VisionObjectName spawn residual
  (createVisionObject disabled in retail C++ — ShroudRevealToAllRange path)
- Full AmericaParachute container OpenClose / DeliverPayload fall-physics matrix
  (OpenDist host residual closed 2026-07-13 — see Same-Player + OpenDist section)
- Full AutoFindHealingUpdate AlwaysHeal busy-interrupt path (dead code in retail C++)
- Full same-player PartitionFilter for PilotFindVehicle (host Neutral unmanned path)
  (host residual closed 2026-07-13 — see Same-Player PartitionFilter + Parachute OpenDist)
- Network die-mux / collide-module replication (network deferred)

## Residual Host Playability — Air Parachute Eject + USA Infantry AutoFindHealing (2026-07-13)
**Closed (host-testable EjectPilotDie air OCL + non-pilot AutoFindHealing residual):**
1. **Air OCL parachute residual** (`EjectPilotDie` / `isSignificantlyAboveTerrain`):
   - When dying eligible vehicle height > **576** (`-(3*3)*Gravity`, Gravity=**-64**)
     or `airborne_target`, residual uses OCL_EjectPilotViaParachute path.
   - Pilot spawns elevated + `OBJECT_STATUS_PARACHUTING` residual; linear sink
     **20** u/frame until ground; InvulnerableTime **60** frames still applies.
   - Ground death (y≈0, not airborne) stays OCL_EjectPilotOnGround residual.
   - Fail-closed: not full AmericaParachute container OpenClose / fall physics.
2. **USA infantry AutoFindHealing residual** (beyond pilot-only):
   - Templates: Pilot / Ranger / MissileDefender / Pathfinder / ColonelBurton
     (+ general variants) residual AutoFindHealingUpdate module.
   - Same AI-only idle gates: ScanRate **30** frames / ScanRange **300** /
     NeverHeal **0.85** → SeekingHealing at HealPad.
   - Fail-closed: AlwaysHeal busy path still not claimed (C++ early-return unreachable);
     China/GLA infantry not claimed.
3. Tests (not log-only):
   - `eject_pilot_air_ocl_parachute_residual`
   - `usa_infantry_auto_find_healing_hospital_path_residual`
   - module unit tests in `host_usa_pilot` (air OCL gates / infantry template / honesty)

**Still residual (fail-closed, not claimed):**
- Full BattlePlanUpdate pack/unpack door model-condition / 7s animation matrix
- Full Bombardment turret natural-position recenter / pitch scan matrix
- Full StealthDetectorUpdate module enable stack / VisionObjectName spawn residual
  (createVisionObject disabled in retail C++ — ShroudRevealToAllRange path)
- Full AmericaParachute container OpenClose / DeliverPayload fall-physics matrix
- Full PilotFindVehicleUpdate CollideModule wouldLikeToCollideWith matrix
  (host residual closed 2026-07-13 — see Eject DieMux + PilotFindVehicle CollideModule)
- Full AutoFindHealingUpdate AlwaysHeal busy-interrupt path (dead code in retail C++)
- Network air-eject / infantry-auto-heal replication (network deferred)

## Residual Host Playability — Pilot Base-Center + AutoFindHealing (2026-07-13)
**Closed (host-testable PilotFindVehicle base-center fallback + AutoFindHealingUpdate residual):**
1. **Base-center fallback residual** (`PilotFindVehicleUpdate::m_didMoveToBase`):
   - When AI idle pilot scan finds **no** recrewable vehicle, residual issues one
     Move toward team CommandCenter (`getAiBaseCenter` host residual).
   - Latches `pilot_did_move_to_base` (no repeat); clears on successful vehicle Enter.
   - Fail-closed: CommandCenter-only (not any-structure fallback); not full CollideModule.
2. **AutoFindHealingUpdate residual** (`AmericaInfantryPilot` ModuleTag_06):
   - AI-only idle injured pilot auto-scan (C++ human early-return; host `is_local` gate).
   - ScanRate **1000**ms → **30** frames; ScanRange **300**; NeverHeal **0.85**.
   - Nearest HealPad residual → SeekingHealing → existing HealPad HP ticks.
   - Fail-closed: AlwaysHeal busy-interrupt path not claimed; pilot-template residual only
     (non-pilot USA infantry residual closed 2026-07-13 — see Air Parachute Eject +
     USA Infantry AutoFindHealing section).
3. Tests (not log-only):
   - `pilot_find_vehicle_base_center_fallback_residual`
   - `pilot_auto_find_healing_hospital_path_residual`
   - module unit tests in `host_usa_pilot` (base-center / auto-heal gates / honesty)

**Still residual (fail-closed, not claimed):**
- Full BattlePlanUpdate pack/unpack door model-condition / 7s animation matrix
- Full Bombardment turret natural-position recenter / pitch scan matrix
- Full StealthDetectorUpdate module enable stack / VisionObjectName spawn residual
- Full EjectPilotDie air OCL parachute / isSignificantlyAboveTerrain matrix
  (host residual closed 2026-07-13 — see Air Parachute Eject + USA Infantry AutoFindHealing)
- Full PilotFindVehicleUpdate CollideModule wouldLikeToCollideWith matrix
- Full AutoFindHealingUpdate AlwaysHeal busy-interrupt / non-pilot infantry matrix
  (non-pilot host residual closed 2026-07-13 — see Air Parachute Eject section)
- Network base-center / auto-heal replication (network deferred)

## Residual Host Playability — Eject VeterancyLevels Gate + PilotFindVehicle (2026-07-13)
**Closed (host-testable EjectPilotDie VeterancyLevels + PilotFindVehicleUpdate residual):**
1. **VeterancyLevels residual** (`EjectPilotDie` / DieMux `VeterancyLevels = ALL -REGULAR`):
   - Rookie / LEVEL_REGULAR eligible vehicles do **not** eject pilots on death.
   - Veteran / Elite / Heroic residual still ejects `AmericaInfantryPilot`.
   - Honesty records REGULAR-gate blocks vs successful ejects.
   - Fail-closed: not full DeathTypes / ExemptStatus DieMux matrix.
2. **PilotFindVehicleUpdate residual** (`AmericaInfantryPilot` ModuleTag_07):
   - AI-only idle pilot auto-scan (C++ human → sleep forever; host `is_local` gate).
   - ScanRate **1000**ms → **30** frames; ScanRange **300**; MinHealth **0.5**.
   - Nearest recrewable unmanned vehicle meeting MinHealth → Enter → recrew path.
   - Fail-closed: not full base-center fallback / VeterancyCrate same-team exp matrix
     (base-center host residual closed 2026-07-13 — see Pilot Base-Center + AutoFindHealing).
3. Tests (not log-only):
   - `eject_pilot_veterancy_levels_all_minus_regular_residual`
   - `pilot_find_vehicle_ai_auto_scan_min_health_residual`
   - module unit tests in `host_usa_pilot` (vet gate / find-vehicle gates / honesty)

**Still residual (fail-closed, not claimed):**
- Full BattlePlanUpdate pack/unpack door model-condition / 7s animation matrix
- Full Bombardment turret natural-position recenter / pitch scan matrix
- Full StealthDetectorUpdate module enable stack / VisionObjectName spawn residual
- Full EjectPilotDie air OCL parachute / isSignificantlyAboveTerrain matrix
- Full PilotFindVehicleUpdate base-center fallback / CollideModule matrix
  (base-center host residual closed 2026-07-13 — see Pilot Base-Center + AutoFindHealing;
  CollideModule still open)
- Full AutoFindHealingUpdate hospital path residual
  (host residual closed 2026-07-13 — see Pilot Base-Center + AutoFindHealing)
- Network eject-vet-gate / pilot-find-vehicle replication (network deferred)

## Residual Host Playability — Bombardment Turret + Eject InvulnerableTime (2026-07-13)
**Closed (host-testable StrategyCenterGun Bombardment turret + OCL InvulnerableTime residual):**
1. **Bombardment turret residual** (`BattlePlanUpdate::enableTurret` / StrategyCenterGun):
   - Strategy Center spawns with turret residual **disabled** (strip kind-based weapon).
   - Bombardment plan equips PRIMARY `StrategyCenterGun`: PrimaryDamage **200** /
     radius **25**, range **400**, min **100**, Delay **7000**ms → **210** frames.
   - Auto-fire residual while Bombardment active (nearest in-band enemy + splash).
   - Leaving Bombardment clears weapon residual.
   - Fail-closed: not full natural-position recenter / pack-unpack door matrix.
2. **InvulnerableTime residual** (`OCL_EjectPilotOnGround` InvulnerableTime = **2000**ms):
   - Ejected pilot residual is damage-immune for **60** frames (`goInvulnerable` host).
   - Enemies skip auto-fire on invulnerable pilots (ALLIES relationship residual).
   - Timer expiry restores normal damage.
   - Fail-closed: not full UNDETECTED_DEFECTOR FX flash matrix.
3. Tests (not log-only):
   - `strategy_center_bombardment_turret_fire_residual`
   - `eject_pilot_invulnerable_time_residual`
   - module unit tests in `host_strategy_center` / `host_usa_pilot`

**Still residual (fail-closed, not claimed):**
- Full BattlePlanUpdate pack/unpack door model-condition / 7s animation matrix
- Full Bombardment turret natural-position recenter / pitch scan matrix
- Full StealthDetectorUpdate module enable stack / VisionObjectName spawn residual
- Full EjectPilotDie air OCL parachute / isSignificantlyAboveTerrain matrix
- Full PilotFindVehicleUpdate AI auto-scan / MinHealth enter matrix
  (host residual closed 2026-07-13 — see Eject VeterancyLevels Gate + PilotFindVehicle section)
- Full EjectPilotDie VeterancyLevels ALL -REGULAR gate residual
  (host residual closed 2026-07-13 — see Eject VeterancyLevels Gate + PilotFindVehicle section)
- Network battle-plan turret / eject-invuln replication (network deferred)

## Residual Host Playability — BattlePlan Paralyze + EjectPilotDie (2026-07-13)
**Closed (host-testable BattlePlanChangeParalyzeTime + USA EjectPilotDie residual):**
1. **BattlePlanChangeParalyze residual** (`BattlePlanUpdate` / Strategy Center):
   - First plan select: army residual bonuses only (no paralyze — matches C++
     PLANSTATUS_NONE only on pack-for-change).
   - Plan **switch**: legal army members receive DISABLED_PARALYZED for
     **5000 ms → 150 frames** (`BattlePlanChangeParalyzeTime`).
   - Paralyzed units cannot move/attack until timer expires (`tick_disabled_paralyzed`).
   - Fail-closed: not full pack/unpack door animation / turret recenter matrix.
2. **EjectPilotDie residual** (`EjectPilotDie` / OCL_EjectPilotOnGround):
   - Eligible USA ground vehicles (Humvee / Tomahawk / Crusader / Paladin /
     Avenger / Microwave + general variants) spawn `AmericaInfantryPilot` on death.
   - Pilot residual starts **VETERAN**; can recrew via existing pilot residual path.
   - Fail-closed: unmanned vehicles do **not** eject; aircraft air-parachute OCL
     not claimed (ground residual only).
3. Tests (not log-only):
   - `strategy_center_battle_plan_paralyze_residual_on_plan_change`
   - `eject_pilot_die_spawns_pilot_on_vehicle_death_residual`
   - module unit tests in `host_strategy_center` / `host_usa_pilot`

**Still residual (fail-closed, not claimed):**
- Full BattlePlanUpdate pack/unpack door model-condition / 7s animation matrix
- Full Bombardment turret enable residual (host residual closed 2026-07-13 —
  see Bombardment Turret + Eject InvulnerableTime section; natural-position recenter still open)
- Full StealthDetectorUpdate module enable stack / VisionObjectName spawn residual
- Full EjectPilotDie air OCL parachute / isSignificantlyAboveTerrain matrix
- Full InvulnerableTime post-eject residual (host residual closed 2026-07-13 —
  see Bombardment Turret + Eject InvulnerableTime section)
- Full PilotFindVehicleUpdate AI auto-scan / MinHealth enter matrix
  (host residual closed 2026-07-13 — see Eject VeterancyLevels Gate + PilotFindVehicle section)
- Full EjectPilotDie VeterancyLevels ALL -REGULAR gate residual
  (host residual closed 2026-07-13 — see Eject VeterancyLevels Gate + PilotFindVehicle section)
- Network battle-plan paralyze / eject-pilot replication (network deferred)

## Residual Host Playability — Demo PlusFire SUICIDED + CommandSetUpgrade (2026-07-13)
**Closed (host-testable Demo_SuicideDynamitePackPlusFire intentional suicide + CommandSetUpgrade residual):**
1. **PlusFire SUICIDED residual** (`Demo_SuicideDynamitePackPlusFire`):
   - Intentional TertiarySuicide (`Demo_Command_TertiarySuicide` / `CommandType::DemoTertiarySuicide`)
     on SuicideBomb-tagged non-terrorist Demo units applies Primary **500**/r**18** +
     Secondary **300**/r**50**.
   - Self is consumed; process_destroy_list skips `Demo_DestroyedWeapon` double-fire
     (`demo_suicided_detonating` residual flag).
   - Fail-closed: Terrorist SUICIDED path stays `Demo_SuicideDynamitePack` (700).
2. **CommandSetUpgrade residual** (`CommandSetUpgrade` TriggeredBy SuicideBomb):
   - QueueUpgrade → complete sets `command_set_override` to retail
     `Demo_*CommandSetUpgrade` (e.g. `Demo_GLAInfantryRebelCommandSetUpgrade`).
   - TertiarySuicide gated by upgrade tag + command_set_override residual.
   - Post-research spawns inherit tag + command_set_override residual.
3. Tests (not log-only):
   - `demo_tertiary_suicide_plus_fire_command_set_residual`
   - module unit tests: tertiary gate / command set names / PlusFire rings

**Still residual (fail-closed, not claimed):**
- Full PrerequisiteSciences rank tree / control-bar science visibility matrix
- Full IsTrainable exclusive ExperienceTracker module matrix
- Full SlowDeath SUICIDED fling / OCL poison particle bone matrix
- Full control-bar CommandSet slot UI matrix (host residual is override + gate)
- Network unit-training / suicide-bomb replication (network deferred)

## Residual Host Playability — SCIENCE Unit Training + Demo SuicideBomb (2026-07-13)
**Closed (host-testable VeterancyGainCreate science spawn + Demo_Upgrade_SuicideBomb death residual):**
1. **SCIENCE unit-training residual** (`VeterancyGainCreate`):
   - `SCIENCE_RedGuardTraining` → Red Guard spawn **VETERAN** (+20% HP residual)
   - `SCIENCE_BattlemasterTraining` → Battlemaster spawn **ELITE** (+30% HP residual)
   - `SCIENCE_ArtilleryTraining` → Inferno / Nuke Cannon spawn **VETERAN**
   - `SCIENCE_TechnicalTraining` → Technical spawn **VETERAN**
   - `Infa_SCIENCE_RedGuardTraining` → MiniGunner spawn **ELITE**
   - `unlock_team_science` / production `create_object` grant honesty
2. **Demo SuicideBomb residual** (`Demo_Upgrade_SuicideBomb`):
   - QueueUpgrade → complete tags eligible Demo units/structures
   - Death residual fires `Demo_DestroyedWeapon`: Primary **50**/r**60** +
     Secondary **10**/r**70**
   - New Demo spawns inherit tag after research
   - Fail-closed: Terrorist SUICIDED path stays `Demo_SuicideDynamitePack` (700)
3. Tests (not log-only):
   - `unit_training_science_veterancy_grant_residual`
   - `demo_suicide_bomb_structure_death_residual`
   - module unit tests in `host_unit_training` / `host_demo_suicide_bomb`

**Still residual (fail-closed, not claimed):**
- Full PrerequisiteSciences rank tree / control-bar science visibility matrix
- Full IsTrainable exclusive ExperienceTracker module matrix
- Full SUICIDED → `Demo_SuicideDynamitePackPlusFire` (500 primary) non-terrorist path
  (host residual closed 2026-07-13 — see Demo PlusFire SUICIDED + CommandSetUpgrade section)
- Full CommandSetUpgrade residual for suicide-bomb command sets
  (host residual closed 2026-07-13 — see Demo PlusFire SUICIDED + CommandSetUpgrade section)
- Network unit-training / suicide-bomb replication (network deferred)

## Residual Host Playability — SCIENCE_StealthFighter Gate + Chem/Demo Death Weapons (2026-07-13)
**Closed (host-testable Stealth Fighter science production gate + Chem/Demo death-weapon residual):**
1. **SCIENCE_StealthFighter production gate residual**:
   - `enqueue_production` denies science-gated Stealth Fighter without unlock
     (`AmericaJetStealthFighter` / SupW_/Lazr_/USA_ — **not** `AirF_*`).
   - `unlock_team_science` / `PurchaseScience` records unlock honesty.
   - Successful enqueue + production spawn record residual honesty.
   - Honesty: `honesty_stealth_fighter_science_*` / host path.
2. **Chem / Demo Terrorist death-weapon residual**:
   - Standard: SuicideDynamitePack **500**/r**18** + **300**/r**50**
   - Chem Beta baseline: same rings + MediumPoisonFieldUpgraded residual
   - Chem Gamma (`Chem_Upgrade_GLAAnthraxGamma` tag): Primary **600** + gamma poison field
   - Demo (`Demo_*`): Primary **700** (no poison)
3. **Chem / Demo DemoTrap death-weapon residual**:
   - Standard: DemoTrapDetonationWeapon **600**/r**25** (legacy falloff residual)
   - Chem Beta/Gamma: Primary **250**/r**25** + Secondary **100**/r**50** + poison field
   - Demo: Primary **700**/r**25** + Secondary **500**/r**50** (no poison)
4. Tests (not log-only):
   - `stealth_fighter_science_production_gate_residual`
   - `chem_terrorist_gamma_and_demo_death_weapon_residual`
   - `chem_demo_trap_gamma_and_demo_he_residual`
   - module unit tests in `host_stealth_fighter` / `host_terrorist` / `host_mines`

**Still residual (fail-closed, not claimed):**
- Full PrerequisiteSciences rank tree / control-bar science visibility matrix
- Full SlowDeath SUICIDED fling / OCL poison particle bone matrix
- Full Demo_SuicideDynamitePackPlusFire SUICIDED path for non-terrorist units
  (host residual closed 2026-07-13 — see Demo PlusFire SUICIDED + CommandSetUpgrade section;
  Demo_DestroyedWeapon normal-death path closed earlier same day)
- Full DemoTrapUpdate weapon-slot mode / PreAttack scoop animation
- Network science-gate / death-weapon replication (network deferred)

## Residual Host Playability — Chem Anthrax Gamma + GLA CamoNetting (2026-07-13)
**Closed (host-testable Anthrax Gamma toxin combat + CamoNetting structure stealth residual):**
1. **Anthrax Gamma residual** (`Chem_Upgrade_GLAAnthraxGamma` / `Upgrade_GLAAnthraxGamma`):
   - QueueUpgrade → complete tags toxin combat units (Toxin Tractor / SCUD / Bomb Truck).
   - Toxin Tractor stream residual: Chem baseline **12.5** (Beta); Gamma → **20.5**
     (`Chem_ToxinTruckGunGamma`).
   - Contaminate spray + death/SCUD MediumPoisonField DoT: Gamma/Beta → **2.5**/tick
     (base remains **2.0**).
   - Chem SCUD residual: primary is anthrax warhead (slot 0 toxin path + poison field).
   - Honesty: `honesty_gamma_ok` / stream / spray / host_path AnthraxGamma.
2. **CamoNetting residual** (`Upgrade_GLACamoNetting`):
   - QueueUpgrade → complete grants STEALTHED + `innate_stealth` to eligible structures:
     Stealth General buildings (`Slth_*` CC/Barracks/ArmsDealer/etc.),
     `GLATunnelNetwork`, `GLAStingerSite` (+ general variants).
   - Fail-closed: Rebel infantry does **not** receive CamoNetting (Camouflage residual
     remains separate).
   - Honesty: CamoNetting complete / host_path units_affected.
3. Tests (not log-only):
   - `anthrax_gamma_residual_toxin_stream_and_field`
   - `camo_netting_upgrade_stealths_gla_structures`
   - module unit tests in `host_toxin_tractor` / `host_scud_launcher` / `host_upgrades`

**Still residual (fail-closed, not claimed):**
- Full gamma particle bones / PlusOne-Two anthrax salvage weapon-set matrix
- Full CamoNetting sub-object net visual / structure attack reveal matrix
- Full Chem_DemoTrap / Terrorist suicide gamma death-weapon matrix (host residual closed 2026-07-13 — see SCIENCE_StealthFighter Gate + Chem/Demo Death Weapons section)
- Network anthrax / camo-netting replication (network deferred)

## Residual Host Playability — SupW EMP Patriot + GLA Saboteur (2026-07-13)
**Closed (host-testable SupW Patriot EMP dual-slot + Saboteur type-specific sabotage residual):**
1. **Superweapon General EMP Patriot residual** (`SupW_AmericaPatriotBattery` / TestSupWPatriot):
   - PRIMARY `SupW_PatriotMissileWeapon` residual: PrimaryDamage **15** / range **275**,
     ClipReload residual cadence (60 frames).
   - AA residual `SupW_PatriotMissileWeaponAir`: PrimaryDamage **30** / range **400**.
   - Impact residual seeds EMPPatriotEffectSpheroid: DISABLED_EMP **10000** ms
     (**300** frames) / EffectRadius **10** (DoesNotAffectMyOwnBuildings residual).
   - Dual-slot base-defense auto-fire residual path (ground + AA + EMP).
2. **GLA Saboteur residual** (`GLAInfantrySaboteur` / Chem_/Demo_/Slth_):
   - Walk to enemy structure → type-specific Sabotage*CrateCollide residual:
     - **Power plant**: player `power_sabotaged_till_frame` **900** frames brownout
     - **Supply center**: steal **1000** cash residual
     - **Military factory** (barracks/warfactory/airfield): DISABLED_HACKED **900** frames
     - **Superweapon / Strategy / Command**: special-power recharge reset residual
     - **Internet Center**: DISABLED_HACKED **450** frames
     - **Fake building**: kill structure residual
   - Saboteur consumed on success (mobile-crate residual).
   - Fail-closed: non-saboteur cannot issue; non-matching structures cancel residual.
3. Tests (not log-only):
   - `supw_patriot_emp_residual_dual_slot_and_disable`
   - `sabotage_command_applies_only_after_unit_reaches_target` (power residual)
   - `saboteur_military_factory_residual_disables_production`
   - `sabotage_command_rejects_non_saboteur_units`
   - module unit tests in `host_base_defense` (SupW) / `host_saboteur`

**Still residual (fail-closed, not claimed):**
- Full SupW AssistedTargetingModule assist clip / RequestAssistRange matrix
- Full EMPPatriotEffectSpheroid drawable scale/tint / EMPSparks particle volume
- Full BuildingPickup CrateCollide goal-object / EVA floating-text matrix
- Full internet-center spy-vision / contained-hacker disable iterate
- Network SupW EMP / saboteur replication (network deferred)

## Residual Host Playability — Laser General Tanks/Patriot + Tunnel Network Gun (2026-07-13)
**Closed (host-testable Lazr tank lasers / Lazr Patriot dual-slot + TunnelNetworkGun residual):**
1. **Laser General tank residual** (`Lazr_AmericaTankCrusader` / `Lazr_AmericaTankPaladin`):
   - PRIMARY `Lazr_CrusaderTankGun` residual: PrimaryDamage **80** / radius **5**,
     range **150**, Delay **2000**ms (60 frames). Instant laser residual.
   - PRIMARY `Lazr_PaladinTankGun` residual: PrimaryDamage **70** / radius **3**,
     range **150**, Delay **1000**ms (30 frames). Instant laser residual.
   - Stock Crusader/Paladin residual still uses shell guns (**60**/2000ms).
   - Composite Armor residual still applies to both chassis.
2. **Laser General Patriot residual** (`Lazr_AmericaPatriotBattery` / Lazr_Patriot*):
   - PRIMARY `Lazr_PatriotMissileWeapon` residual: PrimaryDamage **40** / r**3**,
     range **225** (vs stock **30**).
   - AA residual `Lazr_PatriotMissileWeaponAir` (retail TERTIARY collapsed to secondary):
     PrimaryDamage **35** / r**3**, range **350** (vs stock **25**).
   - Dual-slot base-defense auto-fire residual path (ground + AA).
3. **Tunnel Network gun residual** (`GLATunnelNetwork` / Chem_/Demo_/Slth_ / sneak):
   - PRIMARY `TunnelNetworkGun` residual: PrimaryDamage **15** / range **175** /
     Delay **250**ms (8 frames). Ground residual only.
   - Auto-acquire residual via base-defense residual fire path.
   - Enter/exit residual remains closed (prior slice).

**Tests:**
- `lazr_tank_residual_laser_guns`
- `lazr_patriot_residual_laser_dual_slot`
- `tunnel_network_gun_residual_auto_fires`
- host unit tests for laser tank / laser patriot / tunnel gun stats

**Still residual (fail-closed, not claimed):**
- Full LaserName / LaserBoneName drawable beam matrix for Lazr tanks / Patriot
- Full Lazr Patriot AssistedTargetingModule SECONDARY assist clip / RequestAssistRange
- Full SneakAttack TunnelNetworkGunDUMMY zero-damage matrix (real gun residual)
- Full GuardTunnelNetwork AI / CaveSystem / heal matrix
- Network laser-general / tunnel-gun replication (network deferred)

## Residual Host Playability — China MiG + USA Fire Base (2026-07-13)
**Closed (host-testable MiG napalm/BlackNapalm/Nuke + Fire Base howitzer residual):**
1. **China MiG residual** (`ChinaJetMIG` / `China_MiG` / Tank_/Infa_/Boss_ + Nuke_):
   - PRIMARY `NapalmMissileWeapon` residual: Primary **75**/r**5** + Secondary **40**/r**30**,
     range **320**, min **80**, Delay **300**ms (9 frames). ClipSize **2** honesty
     (RETURN_TO_BASE rearm matrix fail-closed). AA + ground residual.
   - Impact residual seeds FireFieldSmall DoT (Inferno fire-zone registry residual).
   - BlackNapalm PLAYER_UPGRADE residual (`Upgrade_ChinaBlackNapalm`):
     Secondary **50** + FireFieldUpgradedSmall residual.
   - Nuke General (`Nuke_ChinaJetMIG`): base `Nuke_MiGMissileWeapon` Primary **100**
     + SmallRadiationField residual; Tactical Nuke MiG PLAYER_UPGRADE residual
     (`Upgrade_ChinaTacticalNukeMig`) → `Nuke_NukeMissileWeapon` Primary **150**/r**50**
     + Secondary **50**/r**60** + radiation residual.
   - Honesty: `honesty_mig_ok` / black_napalm / tactical_nuke / fires / fields.
2. **Fire Base residual** (`AmericaFireBase` / AirF_/SupW_/Lazr_):
   - PRIMARY `FireBaseHowitzerGun` residual: PrimaryDamage **75** / radius **10**,
     range **275**, min **50**, Delay **2000**ms (60 frames). Ground residual only.
   - Fire residual: intended + PrimaryDamageRadius **10** splash.
   - Honesty: `honesty_fire_base_ok` / fires / units_hit.
3. Tests (not log-only):
   - `mig_residual_napalm_and_black_napalm`
   - `mig_nuke_residual_tactical_nuke`
   - `fire_base_residual_howitzer`
   - module unit tests in `host_mig.rs` / `host_fire_base.rs`

**Still residual (fail-closed, not claimed):**
- Full JetAIUpdate RETURN_TO_BASE / ClipReload airfield rearm matrix for MiG
- Full HistoricBonus FirestormSmallCreationWeapon multi-missile matrix
- Full MediumRadiationField for Nuke_NukeMissileWeapon residual (SmallRadiation reused)
- Full SPAWNS_ARE_THE_WEAPONS / garrison HiveStructureBody matrix for Fire Base
- Full Turret pitch / ScaleWeaponSpeed lob / ScatterRadiusVsInfantry matrix
- Network MiG / Fire Base replication (network deferred)

## Residual Host Playability — USA Strategy Center Battle Plans (2026-07-13)
**Closed (host-testable Bombardment / HoldTheLine / SearchAndDestroy residual):**
1. **Strategy Center residual** (`AmericaStrategyCenter` / *StrategyCenter):
   - Select plan via residual `SpecialAbilityChangeBattlePlans` (OPTION_ONE/TWO/THREE):
     - **Bombardment**: army WeaponBonus DAMAGE **120%** on legal members
     - **HoldTheLine**: army armor damage scalar **0.9** + center max-health **×2**
     - **SearchAndDestroy**: army RANGE **120%** + sight **1.2**; center stealth detect residual
   - ValidMember residual: INFANTRY | CAN_ATTACK | VEHICLE
   - InvalidMember residual: DOZER | STRUCTURE | AIRCRAFT | DRONE
   - Honesty: `honesty_battle_plan_ok` / select / buffs
2. Tests (not log-only):
   - `strategy_center_battle_plan_residual_applies_unit_bonuses`
   - module unit tests in `host_strategy_center.rs`

**Still residual (fail-closed, not claimed):**
- Full BattlePlanUpdate pack/unpack door model-condition / 7s animation matrix
- Full 5000ms army BattlePlanChangeParalyzeTime on plan change
  (host residual closed 2026-07-13 — see BattlePlan Paralyze + EjectPilotDie section)
- Full Bombardment turret enable / recenter natural-position residual path
- Full StealthDetectorUpdate module enable stack / VisionObjectName spawn residual
- Network battle-plan replication (network deferred)

## Residual Host Playability — Helix Minigun + Inferno BlackNapalm (2026-07-13)
**Closed (host-testable Helix PRIMARY minigun + Inferno BlackNapalm fire-field residual):**
1. **Helix minigun residual** (`ChinaVehicleHelix` / Nuke_/Infa_/Tank_ / TestHelix):
   - PRIMARY `HelixMinigunWeapon` residual: PrimaryDamage **6** / radius **0**
     (intended-only), range **115**, Delay **100**ms (3 frames).
   - AntiAirborneInfantry residual honesty (`can_target_air`); AntiAirborneVehicle = No.
   - Minigun retained with portable gattling/propaganda/bunker addons (retail keeps
     HelixMinigun always — gattling addon remains separate residual path).
   - Honesty: `honesty_helix_minigun_ok` / fires / units_hit.
2. **Inferno BlackNapalm residual** (`Upgrade_ChinaBlackNapalm` on Inferno Cannon):
   - WeaponSet PLAYER_UPGRADE residual → FireFieldUpgradedSmall on shell impact.
   - Upgraded fire DoT: **7.5** / r**30** / tick **250**ms / lifetime **2500**ms
     (`SmallFireFieldWeaponUpgraded`; base FireFieldSmall remains **5**).
   - Shell impact PrimaryDamage **30** / r**15** / range **300** unchanged.
   - Honesty: `honesty_inferno_black_napalm_ok` / upgrades / upgraded zones;
     existing `honesty_inferno_cannon_ok` still green.
3. Tests (not log-only):
   - `helix_minigun_residual_intended_only`
   - `inferno_black_napalm_upgraded_fire_field_residual`
   - module unit tests in `host_helix_minigun.rs` / `host_inferno_cannon.rs`

**Still residual (fail-closed, not claimed):**
- Full ChinookAIUpdate rotor wash / AutoAcquire idle / COMANCHE_VULCAN Stinger-site matrix
- Full Helix portable gattling dual-stream simultaneous fire with minigun matrix
- Full HistoricBonus FirestormSmallCreationWeapon multi-shell Inferno matrix
- Full InfernoTankShell DumbProjectileBehavior bezier lob / upgraded particle bones
- Network Helix minigun / Inferno BlackNapalm replication (network deferred)

## Residual Host Playability — USA Stealth Fighter Combat + Comanche Cannon/AT (2026-07-13)
**Closed (host-testable Stealth Fighter missiles + Comanche 20mm/anti-tank residual):**
1. **Stealth Fighter residual** (`AmericaJetStealthFighter` / USA_ / SupW_/Lazr_ + AirF_):
   - PRIMARY `StealthJetMissileWeapon` residual: PrimaryDamage **100** / radius **5**,
     range **220**, min **60**, Delay **200**ms (6 frames). ClipSize **2** honesty
     (RETURN_TO_BASE rearm matrix fail-closed). Ground residual only (no AA).
   - Fire residual: intended + PrimaryDamageRadius **5** splash.
   - Bunker Buster PLAYER_UPGRADE residual remains in host_bunker_buster and is
     applied from the residual fire path for structure targets (garrison kill + bunker mult).
   - Honesty: `honesty_stealth_fighter_ok` / fires / units_hit.
2. **Comanche residual** (`AmericaVehicleComanche` / USA_ / AirF_/SupW_/Lazr_):
   - PRIMARY `Comanche20mmCannonWeapon` residual: PrimaryDamage **6** / intended-only,
     range **200**, Delay **100**ms (3 frames). AntiAirborneInfantry residual honesty.
   - SECONDARY `ComancheAntiTankMissileWeapon` residual at spawn:
     Primary **50**/r**5** + Secondary **30**/r**25**, range **200**, Delay **500**ms
     (15 frames), ClipSize **4** honesty.
   - Rocket pods PLAYER_UPGRADE residual still replaces secondary (retail TERTIARY
     collapse — existing host_comanche_rocket_pods path).
   - Honesty: `honesty_comanche_ok` / cannon / antitank / rocket pods.
3. Tests (not log-only):
   - `stealth_fighter_residual_missiles_and_splash`
   - `comanche_residual_cannon_and_antitank`
   - module unit tests in `host_stealth_fighter.rs` / `host_comanche_rocket_pods.rs`
   - existing `bunker_buster_residual_kills_garrison_and_damages_bunker` still green

**Still residual (fail-closed, not claimed):**
- Full SCIENCE_StealthFighter production enqueue gate residual (host residual closed 2026-07-13 — see SCIENCE_StealthFighter Gate section; full rank tree still open)
- Full JetAIUpdate RETURN_TO_BASE / ClipReload airfield rearm matrix
- Full WeaponSet PRIMARY/SECONDARY/TERTIARY chooser (host collapses tertiary rocket pods into secondary)
- Full ScatterTarget / 20-rocket volley spacing / JetAIUpdate turret matrix
- Network stealth-fighter / comanche / rocket-pods replication (network deferred)

## Residual Host Playability — USA Raptor + Battle Drone (2026-07-13)
**Closed (host-testable Raptor missiles/Laser Missiles + Battle Drone attach/gun/repair residual):**
1. **Raptor residual** (`AmericaJetRaptor` / USA_ / SupW_/Lazr_ + King Raptor AirF_):
   - PRIMARY `RaptorJetMissileWeapon` residual: PrimaryDamage **100** / radius **5**,
     range **320**, min **100**, Delay **150**ms (5 frames). ClipSize **4** honesty
     (RETURN_TO_BASE rearm matrix fail-closed). AA + ground residual.
   - King Raptor (`AirF_AmericaJetRaptor`): `AirF_RaptorJetMissileWeapon` residual
     Primary **125** / range **350** / Delay **75**ms (3 frames) / ClipSize **6**.
     PDL residual remains in host_point_defense (not re-opened).
   - Laser Missiles PLAYER_UPGRADE residual (`Upgrade_AmericaLaserMissiles`):
     standard DAMAGE **125%** → **125**; King Raptor DAMAGE **112%** → **140**.
   - Fire residual: intended + PrimaryDamageRadius **5** splash.
   - Honesty: `honesty_raptor_ok` / laser_missiles / fires / units_hit.
2. **Battle Drone residual** (`AmericaVehicleBattleDrone` / SupW_/AirF_/Lazr_):
   - Attach residual via `residual_attach_slave_drone(Battle)` on Humvee/compatible
     masters → tags `Upgrade_AmericaBattleDrone`.
   - PRIMARY `BattleDroneMachineGun` residual: PrimaryDamage **1** / range **110** /
     Delay **100**ms (3 frames). Intended-only fire residual.
   - Master repair residual: when master HP < **60%**, heal **10** HP/s
     (`RepairRatePerSecond` residual) within repair band.
   - Honesty: `honesty_battle_drone_ok` / attach / fire / repair.
3. Tests (not log-only):
   - `raptor_residual_missiles_and_laser_missiles`
   - `battle_drone_residual_attach_fire_and_repair`
   - module unit tests in `host_raptor.rs` / `host_slave_drones.rs` Battle path

**Still residual (fail-closed, not claimed):**
- Full JetAIUpdate RETURN_TO_BASE / ClipReload 8000ms airfield rearm matrix
- Full ScatterRadiusVsInfantry / projectile exhaust / Countermeasures flare matrix
- Full SlavedUpdate arm pack/unpack weld FX / RepairMinAltitude matrix
- Full ObjectCreationUpgrade ConflictsWith Battle/Scout/Hellfire exclusive matrix
- Network laser-missiles / battle-drone replication (network deferred)

## Residual Host Playability — China Nuclear Tanks + GLA Rebel BoobyTrap (2026-07-13)
**Closed (host-testable Nuclear Tanks death/speed/radiation + Rebel BoobyTrap residual):**
1. **Nuclear Tanks residual** (`Upgrade_ChinaNuclearTanks` on Battlemaster / Overlord / Emperor):
   - Locomotor residual speed: Battlemaster **25 → 35**, Overlord/Emperor **20 → 30**.
   - On death: dual-radius `NuclearTankDeathWeapon` residual
     Primary **25**/r**25** + Secondary **10**/r**75** (Nuke_ general: **110**/r**80** +
     **70**/r**100**).
   - Spawns residual `OCL_RadiationFieldSmall` / `SmallRadiationFieldWeapon`:
     **5** dmg / r**15** / tick **750**ms / lifetime **2500**ms.
   - Honesty: `honesty_nuclear_tanks_ok` / upgrade / death / radiation.
2. **Rebel BoobyTrap residual** (`Upgrade_GLAInfantryRebelBoobyTrapAttack` +
   `SpecialAbilityBoobyTrap` on `GLAInfantryRebel` / variants):
   - Plant residual: walk to enemy structure within StartAbilityRange **5** →
     mark `OBJECT_STATUS_BOOBY_TRAPPED` (host residual).
   - Reload residual: **7500** ms (**225** frames).
   - Detonate residual on enemy capture-trigger / structure death:
     Primary **200** / (r**5**+geometry) + Secondary **50** / (r**15**+geometry).
   - Allies of planter do not trigger detonation (C++ checkAndDetonateBoobyTrap).
   - Honesty: `honesty_booby_trap_ok` / plant / detonate / upgrade.
3. Tests (not log-only):
   - `nuclear_tanks_residual_speed_death_and_radiation`
   - `rebel_booby_trap_plant_and_capture_detonate_residual`
   - module unit tests in `host_nuclear_tanks.rs` / `host_booby_trap.rs`

**Still residual (fail-closed, not claimed):**
- Full FireWeaponWhenDeadBehavior exclusive module / RequiresAllTriggers matrix
- Full LocomotorSetUpgrade Nuclear*Locomotor pitch-roll visual matrix
- Full SpecialObject BoobyTrap StickyBombUpdate bone attach / stealth matrix
- Full MaxSpecialObjects=100 / UniqueSpecialObjectTargets list matrix
- Network nuclear-tanks / booby-trap replication (network deferred)

## Residual Host Playability — China Overlord Main Gun + GLA Jarmen Kell Sniper (2026-07-13)
**Closed (host-testable Overlord dual-radius/Uranium + Jarmen Kell sniper/AP residual):**
1. **Overlord main gun residual** (`ChinaTankOverlord` / Nuke_/Infa_/Tank_ + Emperor):
   - PRIMARY `OverlordTankGun` residual: PrimaryDamage **80** / radius **5** +
     SecondaryDamage **20** / radius **10**, range **175**, ClipReload **2000**ms
     (60 frames). ClipSize **2** honesty (dual-volley cadence fail-closed).
   - Dual-radius splash residual on fire (intended + primary/secondary rings).
   - Uranium Shells PLAYER_UPGRADE residual (`Upgrade_ChinaUraniumShells`):
     WeaponBonus DAMAGE **125%** → Primary **100** / Secondary **25**.
   - Portable gattling addon exclusive fire path unchanged (still deals weapon.damage
     + passenger gattling when addon installed).
   - Honesty: `honesty_overlord_gun_ok` / uranium / fires / units_hit.
2. **Jarmen Kell sniper residual** (`GLAInfantryJarmenKell` / Chem_/Demo_/Slth_/GC_*):
   - PRIMARY `GLAJarmenKellRifle` residual: PrimaryDamage **180** / range **225** /
     Delay **1000**ms (30 frames). Intended-only SNIPER residual (radius **0**).
   - AP Bullets PLAYER_UPGRADE residual (`Upgrade_GLAAPBullets`): DAMAGE **125%** → **225**.
   - Vehicle pilot-snipe special already closed via host_hero_abilities (not re-opened).
   - Honesty: `honesty_jarmen_kell_ok` / ap / fires / units_hit.
3. Tests (not log-only):
   - `overlord_gun_residual_dual_radius_and_uranium`
   - `jarmen_kell_residual_sniper_and_ap_bullets`
   - module unit tests in `host_overlord_gun.rs` / `host_jarmen_kell.rs`

**Still residual (fail-closed, not claimed):**
- Full ClipSize=2 DelayBetweenShots 300ms dual-volley cadence for Overlord
- Full ScatterRadiusVsInfantry / projectile shell lob / W3D turret matrix
- Full Nuclear Tanks death weapon residual (host residual closed 2026-07-13 — see Nuclear Tanks + BoobyTrap section; full exclusive module still open)
- Full SECONDARY AutoChooseSources=NONE pilot-sniper WeaponSet chooser matrix
- Full StealthUpdate / Camouflage / Science prereq residual matrix for Kell
- Network uranium / sniper / AP Bullets replication (network deferred)

## Residual Host Playability — GLA Scorpion + USA Tomahawk (2026-07-13)
**Closed (host-testable Scorpion gun/salvage/rocket + Tomahawk dual-radius residual):**
1. **Scorpion residual** (`GLATankScorpion` / `GLA_ScorpionTank` / Chem_/Demo_/Slth_):
   - PRIMARY `ScorpionTankGun` residual: PrimaryDamage **20** / radius **5** /
     range **150** / Delay **1000**ms (30 frames).
   - Salvage residual (CRATEUPGRADE): gun damage **20** → **25** (`ScorpionTankGunPlusOne`;
     PlusTwo keeps PlusOne gun — no further primary bonus).
   - `Upgrade_GLAScorpionRocket` PLAYER_UPGRADE residual equips SECONDARY
     `ScorpionMissileWeapon`: Primary **100**/r**5** + Secondary **80**/r**25**,
     min **40**, ClipReload **15000**ms (450 frames).
   - AP Rockets PLAYER_UPGRADE residual (`Upgrade_GLAAPRockets`): missile rings × **1.25**.
   - Honesty: `honesty_scorpion_ok` / rocket / missile / fires.
2. **Tomahawk residual** (`AmericaVehicleTomahawk` / `USA_Tomahawk` / SupW_):
   - PRIMARY `TomahawkMissileWeapon` residual: Primary **150**/r**10** +
     Secondary **50**/r**25**, range **350**, min **100**, ClipReload **7000**ms
     (210 frames).
   - Dual-radius splash residual on fire (intended + primary/secondary rings).
   - Honesty: `honesty_tomahawk_ok` / fires / units_hit.
3. Tests (not log-only):
   - `scorpion_residual_gun_salvage_and_rocket`
   - `tomahawk_residual_dual_radius_missile`
   - module unit tests in `host_scorpion.rs` / `host_tomahawk.rs`

**Still residual (fail-closed, not claimed):**
- Full SalvageCrate W3D turret / missile-rack subobject swap matrix
- Full ClipSize=2 DelayBetweenShots 200ms dual-volley cadence for rocket+tier2
- Full TomahawkMissile projectile lob / CapableOfFollowingWaypoints path
- Full PreAttackDelay PER_SHOT anim / hide-show missile bone matrix
- Network scorpion rocket / tomahawk replication (network deferred)

## Residual Host Playability — USA Pilot + GLA WorkerShoes (2026-07-13)
**Closed (host-testable Pilot recrew + WorkerShoes residual):**
1. **USA Pilot residual** (`AmericaInfantryPilot` / AirF_ / CINE_ / TestPilot):
   - Enter unmanned ground vehicle (DISABLED_UNMANNED residual from snipe/neutron) →
     recrew: clear unmanned, transfer team to pilot team, merge pilot veterancy
     (retail `VeterancyCrateCollide` IsPilot + AddsOwnerVeterancy), consume pilot.
   - Pilots residual-start at VETERAN (`VeterancyGainCreate StartingLevel`).
   - ForbiddenKindOf residual: Dozer / worker vehicles not recrewed.
   - Honesty: `honesty_pilot_recrew_ok` / veterancy_transfer / `honesty_pilot_ok`.
2. **GLA Worker residual** (`GLAInfantryWorker` / Chem_/Slth_/GC_* / GLA_Worker):
   - `Upgrade_GLAWorkerShoes` PLAYER_UPGRADE residual:
     - Locomotor residual Speed **25** → **30** (FastHuman → WorkerShoesLocomotor).
     - WorkerAIUpdate UpgradedSupplyBoost **8** cash per supply drop-off.
   - Construction / repair / mine-clear already residual (not re-opened).
   - Honesty: `honesty_worker_shoes_apply_ok` / boost / `honesty_worker_ok`.
3. Tests (not log-only):
   - `pilot_recrew_unmanned_vehicle_after_enter_reach`
   - `pilot_recrew_rejects_manned_vehicle`
   - `worker_shoes_upgrade_speed_and_supply_boost_residual`
   - module unit tests in `host_usa_pilot.rs` / `host_gla_worker.rs`

**Still residual (fail-closed, not claimed):**
- Full EjectPilotDie air/ground OCL parachute spawn matrix
  (host ground residual closed 2026-07-13 — see BattlePlan Paralyze + EjectPilotDie section;
  air parachute / invuln timer still open)
- Full PilotFindVehicleUpdate AI auto-scan / MinHealth enter matrix
- Full AutoFindHealingUpdate hospital path residual
- Full WorkerAIUpdate BoredTime/Range auto-task matrix
- Full SupplyWarehouseActionDelay / SupplyCenterActionDelay timing matrix
- Full fake-building CommandSetUpgrade residual for Worker
- Network pilot recrew / WorkerShoes replication (network deferred)

## Residual Host Playability — USA Ranger + China Hacker DisableBuilding (2026-07-13)
**Closed (host-testable Ranger rifle/FlashBang splash + Hacker DisableBuilding residual):**
1. **USA Ranger residual polish** (`AmericaInfantryRanger` / USA_ / AirF_ / Lazr_ / SupW_ / GoldenRanger):
   - PRIMARY `RangerAdvancedCombatRifle` residual: PrimaryDamage **5** / range **100** /
     Delay **100**ms (3 frames). ClipSize **3** honesty (volley matrix fail-closed).
   - SECONDARY `RangerFlashBangGrenadeWeapon` residual (when equipped / FlashBang upgrade):
     PrimaryDamage **35** / radius **10** + SecondaryDamage **10** / radius **40**,
     range **175**, min **20**, ClipReload **2000**ms (60 frames).
   - Dual-radius splash residual on secondary fire (intended + primary/secondary rings).
   - PreferredAgainst residual (existing slot chooser): secondary preferred vs infantry /
     structures when damage 35 > 5. FlashBang upgrade equip path still via host_upgrades.
   - Honesty: `honesty_ranger_ok` / flashbang / rifle_fires / units_hit.
2. **China Hacker DisableBuilding residual combat polish** (`ChinaInfantryHacker` /
   Tank_/Nuke_ / TestHacker):
   - Special ability residual `SpecialAbilityHackerDisableBuilding`:
     walk to enemy structure within StartAbilityRange **150** → apply
     DISABLED_HACKED for EffectDuration **2000**ms (**60** frames).
   - Disabled structures count as `is_disabled()` (production stop residual).
   - Internet cash residual remains in host_hacker_income (not re-opened).
   - Honesty: `honesty_hacker_disable_building_ok` / disable count.
3. Tests (not log-only):
   - `ranger_residual_rifle_and_flashbang_splash`
   - `hacker_disable_building_command_disables_after_reach`
   - module unit tests in `host_ranger.rs` / `host_hacker_disable.rs`

**Still residual (fail-closed, not claimed):**
- Full SURRENDER DamageType infantry-surrender AI / garrison clear matrix
- Full ClipSize=3 in-clip DelayBetweenShots + ClipReload 700ms volley matrix
- Full FlashBang ScatterRadius projectile lob / PreAttackDelay anim lock
- Full Hacker unpack/pack/prep/PersistentPrepTime continuous refresh stream
- Full BinaryDataStream special object / DisableFX particle interleave
- Network flashbang / disable-building replication (network deferred)

## Residual Host Playability — China MiniGunner + Colonel Burton Combat (2026-07-13)
**Closed (host-testable MiniGunner dual gun/ramp/chain/horde + Burton sniper/knife residual):**
1. **MiniGunner residual** (`Infa_ChinaInfantryMiniGunner` / ChinaInfantryMiniGunner / variants):
   - PRIMARY `Infa_MiniGunnerGun` residual: PrimaryDamage **10** / range **125** /
     Delay **500**ms (15 frames) + SECONDARY `Infa_MiniGunnerGunAir` (10 / 350 / AA).
   - Continuous-fire ramp residual (`FiringTracker` ContinuousFireOne=**6** / Two=**12** /
     Coast=**1000**ms): Base **15** → MEAN **7** (200% RoF) → FAST **5** (300% RoF).
   - Chain Guns PLAYER_UPGRADE residual (`Upgrade_ChinaChainGuns`): DAMAGE **125%** → **12.5**.
   - Horde residual (China infantry HordeUpdate Radius **30** Count **5**): ROF **150%**
     stacks with continuous fire; Nationalism **125%** while in horde.
   - Honesty: `honesty_minigunner_ok` / ramp / aa / horde / nationalism / ground_fires.
2. **Colonel Burton combat residual** (`AmericaInfantryColonelBurton` / SupW_/CINE_):
   - PRIMARY `ColonelBurtonSniperRifleWeapon` residual: PrimaryDamage **40** / range **125** /
     Delay **100**ms (3 frames). ClipSize **3** honesty (volley matrix fail-closed).
   - Knife residual (`ColonelBurtonKnifeWeapon`): close-range infantry within **3** →
     MELEE one-shot PrimaryDamage **10000** (vehicles still take sniper damage).
   - Timed/remote demo charges already closed via host_mines / hero abilities (not re-opened).
   - Honesty: `honesty_burton_ok` / knife / sniper_fires.
3. Tests (not log-only):
   - `minigunner_residual_gun_ramp_aa_horde_and_chain_guns`
   - `colonel_burton_residual_sniper_and_knife`
   - module unit tests in `host_minigunner.rs` / `host_colonel_burton.rs`

**Still residual (fail-closed, not claimed):**
- Full FiringTracker model-condition CONTINUOUS_FIRE_* animation matrix
- Full MiniGunner bayonet tertiary / CaptureBuilding special residual
- Full Burton ClipSize=3 in-clip DelayBetweenShots + ClipReload 500ms volley matrix
- Full knife PreAttackDelay 833ms / PER_ATTACK anim lock matrix
- Full StealthUpdate / ChemicalSuits / AdvancedTraining residual matrix
- Network continuous-fire / sniper / knife replication (network deferred)

## Residual Host Playability — GLA Rebel + RPG Trooper (2026-07-13)
**Closed (host-testable GLA Rebel gun/AP Bullets + RPG Trooper rocket/AP Rockets residual):**
1. **Rebel residual** (`GLAInfantryRebel` / Chem_/Demo_/Slth_/GC_* / GLA_Soldier / TestRebel):
   - PRIMARY `GLARebelMachineGun` residual: PrimaryDamage **5** / range **100** /
     Delay **100**ms (3 frames). ClipSize **3** honesty (volley matrix fail-closed).
   - AP Bullets PLAYER_UPGRADE residual (`Upgrade_GLAAPBullets`): DAMAGE **125%** → **6.25**.
   - Camouflage residual already closed via host_upgrades (not re-opened).
   - Honesty: `honesty_rebel_ok` / ap / fires.
2. **RPG Trooper residual** (`GLAInfantryTunnelDefender` / variants / GLA_RPG / TestRPGTrooper):
   - PRIMARY `TunnelDefenderRocketWeapon` residual: PrimaryDamage **40** /
     radius **5** / range **175** / min **5** / Delay **1000**ms / AA+ground.
   - Fire residual: intended + PrimaryDamageRadius **5** splash take full PrimaryDamage.
   - AP Rockets PLAYER_UPGRADE residual (`Upgrade_GLAAPRockets`): DAMAGE **125%** → **50**.
   - Honesty: `honesty_rpg_trooper_ok` / ap / fires / units_hit.
3. Tests (not log-only):
   - `rebel_residual_gun_and_ap_bullets`
   - `rpg_trooper_residual_rocket_and_ap_rockets`
   - module unit tests in `host_gla_rebel.rs` / `host_rpg_trooper.rs`

**Still residual (fail-closed, not claimed):**
- Full ClipSize=3 in-clip DelayBetweenShots + ClipReload 700ms volley matrix
- CaptureBuilding special ability residual for Rebel (BoobyTrap host residual closed 2026-07-13)
- Full ScatterRadiusVsInfantry / projectile exhaust FX matrix for RPG Trooper
- Network AP / fire replication (network deferred)

## Residual Host Playability — China Red Guard + Tank Hunter (2026-07-13)
**Closed (host-testable China Red Guard gun/horde/nationalism/bayonet + Tank Hunter RPG/TNT residual):**
1. **Red Guard residual** (`ChinaInfantryRedguard` / China_RedGuard / Tank_/Nuke_):
   - PRIMARY `RedguardMachineGun` residual: PrimaryDamage **15** / range **100** /
     Delay **1000**ms (30 frames).
   - Horde residual (`HordeUpdate` KindOf INFANTRY, AlliesOnly, ExactMatch=No,
     Radius **30**, Count **5**): RATE_OF_FIRE **150%** → delay floor(30/1.5)=**20** frames.
   - Nationalism residual (`Upgrade_Nationalism` while in horde): additional ROF **125%**
     (stacks) → floor(30/1.875)=**16** frames.
   - Bayonet residual (`RedguardBayonet` stats): close-range infantry within **2** →
     MELEE one-shot PrimaryDamage **10000** (retail ZH WeaponSet is PRIMARY-only;
     residual from weapon def + PREATTACK_C/FIRING_C / CINE TERTIARY).
   - Honesty: `honesty_red_guard_ok` / horde / nationalism / bayonet / fires.
2. **Tank Hunter residual** (`ChinaInfantryTankHunter` / China_TankHunter / variants):
   - PRIMARY `ChinaInfantryTankHunterMissileLauncher` residual: PrimaryDamage **40** /
     radius **5** / range **175** / min **5** / Delay **1000**ms / AA+ground.
   - Fire residual: intended + PrimaryDamageRadius **5** splash take full PrimaryDamage.
   - Horde + Nationalism residual (same China infantry HordeUpdate params as Red Guard).
   - TNT special residual (`SpecialAbilityTankHunterTNTAttack` / TNTStickyBomb):
     plant timed demo charge (TNTDetonationWeapon 500/10 + 150/50) with ReloadTime
     **7500**ms (225 frames) residual cooldown + StartAbilityRange **5**.
   - Honesty: `honesty_tank_hunter_ok` / tnt / horde / nationalism / fires.
3. Tests (not log-only):
   - `red_guard_residual_gun_horde_nationalism_and_bayonet`
   - `tank_hunter_residual_rpg_horde_and_tnt`
   - module unit tests in `host_red_guard.rs` / `host_tank_hunter.rs`

**Still residual (fail-closed, not claimed):**
- Full HordeUpdate RubOffRadius honorary-member / terrain-decal flag matrix
- Full Fanaticism infantry-general nationalism branch
- Full WeaponSet tertiary auto-choose / pre-attack anim lock for bayonet
- Full SpecialAbilityUpdate flee-after / MaxSpecialObjects=8 list / attach bones
- Full ScatterRadiusVsInfantry / projectile exhaust FX matrix
- SCIENCE_RedGuardTraining VETERAN spawn residual (host residual closed 2026-07-13 — see SCIENCE Unit Training + Demo SuicideBomb section)
- Network horde / TNT / RPG replication (network deferred)

## Residual Host Playability — Stinger Site + Patriot AA Polish (2026-07-13)
**Closed (host-testable GLA Stinger Site SPAWNS_ARE_THE_WEAPONS residual + USA Patriot AA polish):**
1. **Stinger Site residual** (`GLA_StingerSite` / Chem_/Demo_/Slth_ / GC_* variants):
   - Retail SPAWNS_ARE_THE_WEAPONS abstraction: structure fires soldier weapons
     (SpawnNumber=**3** honesty residual — not full SpawnBehavior / HiveStructureBody).
   - PRIMARY `StingerMissileWeapon` (20 / 225 / ClipReload **2000**ms → 60 frames).
   - SECONDARY `StingerMissileWeaponAir` (30 / 400 / AA only).
   - AP Rockets PLAYER_UPGRADE residual (`Upgrade_GLAAPRockets`): damage × **1.25**.
   - Auto-acquire residual (base-defense dual-slot path) chooses air/ground weapon.
   - Honesty: `honesty_stinger_site_ok` / aa / ground_fires / ap_rockets_upgrades.
2. **Patriot residual polish** (`USA_Patriot` / AmericaPatriotBattery / Lazr_…):
   - PRIMARY `PatriotMissileWeapon` (30 / 225) already residual; now also
     SECONDARY `PatriotMissileWeaponAir` (25 / 350 / AA).
   - Dual-slot auto-acquire residual (same base-defense path as Gattling/Stinger).
   - Honesty: `honesty_patriot_ok` / aa / ground_fires (plus existing base-defense fires).
3. Tests (not log-only):
   - `stinger_site_residual_dual_fire_and_ap_rockets`
   - `patriot_residual_aa_secondary_auto_fires`
   - `base_defense_residual_patriot_auto_fires_without_attack_object` (updated)
   - module unit tests in `host_base_defense.rs`

**Still residual (fail-closed, not claimed):**
- Full SpawnBehavior / HiveStructureBody / 3 Stinger soldiers / CamoNetting stealth
- Full AssistedTargetingModule Patriot assist clips / RequestAssistRange=200
- Full Patriot ClipSize=4 in-clip DelayBetweenShots 250ms volley matrix
- Full PointDefenseLaser for structure / anti-ballistic full matrix
- Network base-defense replication (network deferred)

## Residual Host Playability — China Battlemaster (2026-07-13)
**Closed (host-testable China Battlemaster main gun + Uranium + horde/nationalism residual):**
1. **Battlemaster residual** (`ChinaTankBattleMaster` / `China_BattlemasterTank` / Tank_/Nuke_):
   - PRIMARY `BattleMasterTankGun` residual: PrimaryDamage **60** / radius **5** /
     AttackRange **150** / Delay **2000**ms (60 frames) / WeaponSpeed 400.
   - Fire residual: intended + PrimaryDamageRadius **5** splash take full PrimaryDamage.
   - Uranium Shells PLAYER_UPGRADE residual (`Upgrade_ChinaUraniumShells`):
     WeaponBonus DAMAGE **125%** → PrimaryDamage **75**.
   - Horde residual (`HordeUpdate` ExactMatch allies Radius **75** Count **5**):
     WeaponBonus HORDE RATE_OF_FIRE **150%** → delay floor(60/1.5)=**40** frames.
   - Nationalism residual (`Upgrade_Nationalism` while in horde):
     additional RATE_OF_FIRE **125%** (stacks) → delay floor(60/1.875)=**32** frames.
   - Honesty: `honesty_battlemaster_ok` / uranium / horde / nationalism / fires.
2. Tests (not log-only):
   - `battlemaster_residual_gun_uranium_and_horde_nationalism`
   - module unit tests in `host_battlemaster.rs`

**Still residual (fail-closed, not claimed):**
- Full HordeUpdate RubOffRadius honorary-member / terrain-decal flag matrix
- Full Fanaticism infantry-general nationalism branch
- Full Nuclear Tanks exclusive FireWeaponWhenDead / NuclearBattleMasterLocomotor visual matrix (host residual closed 2026-07-13)
- SCIENCE_BattlemasterTraining ELITE spawn residual (host residual closed 2026-07-13 — see SCIENCE Unit Training + Demo SuicideBomb section)
- Network uranium / horde replication (network deferred)

## Residual Host Playability — Helix NapalmBomb + Bomb Truck HE/Bio (2026-07-13)
**Closed (host-testable Helix NapalmBomb special power + Bomb Truck detonation residual):**
1. **Helix NapalmBomb residual** (`SpecialAbilityHelixNapalmBomb` /
   `SPECIAL_HELIX_NAPALM_BOMB` on `ChinaVehicleHelix` / `TestHelix`):
   - Requires `Upgrade_HelixNapalmBomb` residual unlock (TestHelix always unlocked).
   - Instant blast residual: PrimaryDamage **75** / radius **5** + Secondary **40** / **30**
     (`NapalmBombWeapon` / BlackNapalm same blast numbers).
   - Spawns residual FirestormSmall DoT: DamageAmount **100** (BlackNapalm **150**) /
     tick **500**ms / lifetime **6000**ms / radius **90**.
   - Reload residual: **10000** ms (300 frames).
   - Honesty: `honesty_helix_napalm_drop_ok` / blast / firestorm / `honesty_helix_napalm_ok`.
2. **Bomb Truck HE/Bio detonation residual** (`GLAVehicleBombTruck` FireWeaponWhenDead):
   - Default death: Primary **1000**/radius **40** + Secondary **100**/radius **65**.
   - HE upgrade (`Upgrade_GLABombTruckHighExplosiveBomb`): **2000**/50 + **200**/85.
   - Bio upgrade (`Upgrade_GLABombTruckBioBomb`): + MediumPoisonField DoT
     (2 / 80 / 30s / 500ms); Bio+Anthrax → 2.5 upgraded poison.
   - HE+Bio combos supported (HE blast + bio poison residual).
   - Honesty: `honesty_bomb_truck_detonate_ok` / he / bio / path.
3. Tests (not log-only):
   - `helix_napalm_bomb_special_power_residual_blast_and_firestorm`
   - `helix_napalm_bomb_requires_upgrade_on_production_helix`
   - `bomb_truck_default_detonation_residual_damages_nearby`
   - `bomb_truck_he_and_bio_detonation_residual`
   - module unit tests in `host_helix_napalm.rs` / `host_bomb_truck_detonate.rs`

**Still residual (fail-closed, not claimed):**
- Full SpecialObject NapalmBomb HeightDie fall / UnpackTime charge matrix
- Full FirestormDynamicGeometryInfoUpdate expand/reverse radius animation
- Full FireWeaponWhenDead exclusive RequiresAllTriggers / SubObjectsUpgrade Bombload visuals
- Full Anthrax Gamma / Demo_ red FX / WeaponBonus PLAYER_UPGRADE 125% HE matrix
- Network Helix napalm / bomb-truck detonation replication (network deferred)

## Residual Host Playability — Overlord/Helix Addons + Nuke Cannon Primary (2026-07-13)
**Closed (host-testable China Overlord/Helix/Emperor addons + Nuke Cannon primary residual):**
1. **Overlord gattling / propaganda tower addons residual**
   (`ChinaTankOverlord` / general variants; Helix / Emperor hosts):
   - `Upgrade_ChinaOverlordGattlingCannon` / Helix equivalent installs residual
     portable gattling: SECONDARY AA `GattlingBuildingGunAir` (5 / 400 / 250ms)
     + passenger ground residual `GattlingBuildingGun` (10) on PRIMARY fires.
   - `Upgrade_ChinaOverlordPropagandaTower` / Helix equivalent enables propaganda
     pulse on the host (Radius **150**, heal **1%**/s base, **2%**/s upgraded).
   - ConflictsWith residual: exclusive gattling / propaganda / bunker install
     (Emperor keeps innate propaganda when gattling is installed).
   - Honesty: `honesty_overlord_gattling_ok` / propaganda install + heal.
2. **Emperor tank residual** (`Tank_ChinaTankEmperor`):
   - Innate `PropagandaTowerBehavior` residual on spawn (AffectsSelf heal rates 1%/2%).
   - Optional gattling upgrade residual (same portable path).
3. **Helix residual** (`ChinaVehicleHelix`):
   - `HelixContain` Slots=**5** transport residual on spawn.
   - Same gattling / propaganda / bunker addon residual install path.
   - **Helix NapalmBomb special ability closed 2026-07-13** (see section above).
4. **Nuke Cannon primary residual** (`ChinaVehicleNukeCannon`):
   - PRIMARY `NukeCannonGun` area residual: PrimaryDamage **400** / radius **50**
     + SecondaryDamage **20** / radius **60**, range **350**, delay **300** frames.
   - Impact spawns residual `MediumRadiationField` DoT (15 / 50 / 750ms ticks / 30s).
   - Neutron secondary remains existing host residual (unchanged).
   - Honesty: `honesty_nuke_cannon_primary_ok` / radiation / host_path.
5. Tests (not log-only):
   - `overlord_gattling_addon_residual_install_and_fire`
   - `overlord_propaganda_addon_residual_heals_allies`
   - `emperor_innate_propaganda_and_helix_transport_residual`
   - `nuke_cannon_primary_residual_area_and_radiation`
   - module unit tests in `host_overlord_addons.rs` / `host_nuke_cannon.rs`
6. golden_skirmish lib tests: PASS (full vertical slice + retail map + synthetic).

**Still residual (fail-closed, not claimed):**
- Full OCL portable-structure passenger object + DamageModule share matrix
- Full W3DOverlord*Draw / W3DDependencyModelDraw bone attach / CONTINUOUS_FIRE_* anim
- Full NukeCannon DeployStyleAIUpdate unpack / projectile lob / ScatterRadiusVsInfantry
- Network addon / radiation replication (network deferred)

## Residual Host Playability — China Troop Crawler (2026-07-13)
**Closed (host-testable China Troop Crawler transport + detector + assault deploy residual):**
1. **Troop Crawler residual** (`ChinaVehicleTroopCrawler` / Tank_/Nuke_ variants):
   - `TransportContain` Slots=**8**, AllowInsideKindOf=INFANTRY (no passenger fire-from-inside).
   - `InitialPayload` residual: Redguard × **8** docked on spawn.
   - `StealthDetectorUpdate` residual: DetectionRange unset → VisionRange = **175**.
   - `AssaultTransportAIUpdate` + `TroopCrawlerAssault` DEPLOY residual:
     fire in range unloads docked infantry and orders them to attack the designated target.
   - Honesty: `honesty_troop_crawler_ok` / load_unload / assault_deploy / detect / initial_payload.
2. Tests (not log-only):
   - `troop_crawler_residual_capacity_detect_and_payload`
   - `troop_crawler_residual_detect_stealth_in_range`
   - `troop_crawler_residual_transport_load_unload`
   - `troop_crawler_residual_assault_deploy_unloads_and_attacks`
   - `troop_crawler_residual_rejects_vehicle_enter`
   - module unit tests in `host_troop_crawler.rs`

**Still residual (fail-closed, not claimed):**
- Full multi-exit-path ExitStart01-nn / ExitDelay 250ms stagger
- Full HealthRegen%PerSec / DamagePercentToUnits / MembersGetHealedAtLifeRatio retrieve matrix
- Full IR detector FX / IRParticleSys bones
- Network transport / deploy replication (network deferred)

## Residual Host Playability — China Dragon Tank + Gattling Tank (2026-07-13)
**Closed (host-testable China vehicle flame + continuous-fire ramp residual):**
1. **Dragon Tank residual** (`ChinaTankDragon` / general variants):
   - PRIMARY `DragonTankFlameWeapon` residual: PrimaryDamage **10** / radius **5**,
     SecondaryDamage **1** / radius **10**, AttackRange **75**, Delay **40**ms (2 frames).
   - Flame residual: intended + primary-radius units take full primary; secondary ring takes secondary dmg.
   - BlackNapalm PLAYER_UPGRADE residual (`Upgrade_ChinaBlackNapalm`): dmg **12.5** / sec **1.25**.
   - FireWall / Firestorm secondary remains `host_firewall` special-power residual (not re-opened).
   - Honesty: `honesty_dragon_tank_ok` / `honesty_dragon_tank_black_napalm_ok` / fires / units_hit.
2. **Gattling Tank residual** (`ChinaTankGattling` / vehicle variants):
   - PRIMARY `GattlingTankGun` (15 / 150 / 400ms) + SECONDARY `GattlingTankGunAir` (12 / 350 / AA).
   - Continuous-fire ramp residual (`FiringTracker` ContinuousFireOne=2 / Two=6 / Coast=1000ms):
     - Base delay **12** frames → MEAN **6** (200% RoF) → FAST **4** (300% RoF).
   - Chain Guns PLAYER_UPGRADE residual (`Upgrade_ChinaChainGuns`): damage × **1.25**.
   - Honesty: `honesty_gattling_tank_ok` / ramp / aa / ground_fires / ramp_fast / chain_gun_upgrades.
3. Tests (not log-only):
   - `dragon_tank_residual_flame_and_black_napalm`
   - `gattling_tank_residual_ramp_fire_rate_and_aa`
   - module unit tests in `host_dragon_tank.rs` / `host_gattling_tank.rs`

**Still residual (fail-closed, not claimed):**
- Full flamethrower ProjectileStream / garrison-clear AllowAttackGarrisonedBldgs matrix
- Full FiringTracker model-condition CONTINUOUS_FIRE_* animation / VoiceRapidFire full matrix
- Listening Outpost residual (closed 2026-07-13 — detect+transport)
- Overlord/Helix gattling payload residual (closed 2026-07-13 — portable addon path)
- Building Gattling continuous-fire ramp residual closed 2026-07-13 (see section below)
- Network flame / continuous-fire replication (network deferred)

## Residual Host Playability — China Gattling Cannon Structure Ramp (2026-07-13)
**Closed (host-testable structure gattling continuous-fire + AA + Chain Guns residual):**
1. **Gattling Cannon residual** (`ChinaGattlingCannon` / China_ / Nuke_ / Tank_ / Infa_):
   - PRIMARY `GattlingBuildingGun` (10 / 225 / 250ms → 8 frames).
   - SECONDARY `GattlingBuildingGunAir` (5 / 400 / AA only).
   - Continuous-fire ramp residual (`FiringTracker` ContinuousFireOne=**1** / Two=**5** /
     Coast=**2000**ms → 60 frames):
     - Base delay **8** frames → MEAN **4** (200% RoF) → FAST **2** (300% RoF).
   - Chain Guns PLAYER_UPGRADE residual (`Upgrade_ChinaChainGuns`): damage × **1.25**.
   - Auto-acquire residual (base-defense path) uses air/ground slot chooser.
   - Honesty: `honesty_gattling_building_ok` / ramp / aa / ground_fires / ramp_fast.
2. Tests (not log-only):
   - `gattling_building_residual_ramp_fire_rate_and_aa`
   - `base_defense_residual_gattling_auto_fires_without_attack_object` (updated)
   - module unit tests in `host_base_defense.rs`

**Still residual (fail-closed, not claimed):**
- Full CONTINUOUS_FIRE_* model-condition animation / turret pitch / VoiceRapidFire full matrix
- Full AssistedTargetingModule Patriot assist clips / PointDefenseLaser for structure
  (Patriot AA dual-slot polish closed 2026-07-13 — see Stinger Site + Patriot section)
- Network structure continuous-fire / chain-gun replication (network deferred)

## Residual Host Playability — GLA Marauder + Combat Cycle (2026-07-13)
**Closed (host-testable GLA Marauder salvage fire-rate + Combat Cycle rider weapon residual):**
1. **Marauder residual** (`GLATankMarauder` / Chem_/Demo_/Slth_ / `GLA_MarauderTank`):
   - Salvage fire-rate tiers (`WEAPON_SALVAGER` / CRATEUPGRADE), **same PrimaryDamage 60**:
     - Tier 0: `MarauderTankGun` — Delay **2000**ms (60 frames), speed 300
     - Tier 1: `MarauderTankGunUpgradeOne` — Delay **1500**ms (45 frames), speed 400
     - Tier 2: `MarauderTankGunUpgradeTwo` — Delay **750**ms (23 frames), speed 500
   - Range **170**, PrimaryDamageRadius **5** splash residual on fire.
   - Honesty: `honesty_marauder_ok` / `honesty_marauder_weapon_upgrade_ok` / fires.
2. **Combat Cycle residual** (`GLAVehicleCombatBike` / Rocket / Terrorist variants):
   - `RiderChangeContain` Slots=**1**, infantry only; bike fires (passengers do not).
   - WeaponSet residual: Conditions=None → PRIMARY **NONE**; InitialPayload Rebel →
     `GLARebelBikerMachineGun` (8 / 150 / 100ms).
   - Rider weapon switch residual:
     - Rebel → `GLARebelBikerMachineGun`
     - TunnelDefender → `TunnelDefenderBikerRocketWeapon` (40 / 175 / AA / min 5)
     - Jarmen Kell → `GLABikerKellSniperRifle` (180 / 225)
     - Terrorist → `SuicideBikeBomb` area residual (700/20 + 100/50) + self-destroy
     - Worker / Hijacker / Saboteur → no combat weapon residual
   - Enter residual refreshes rider weapon from occupant template.
   - Honesty: `honesty_combat_cycle_ok` / rider_switch / fire / loads / suicides.
3. Tests (not log-only):
   - `marauder_residual_salvage_fire_rate_tiers`
   - `combat_cycle_residual_rider_weapon_switch`
   - module unit tests in `host_marauder.rs` / `host_combat_cycle.rs`

**Still residual (fail-closed, not claimed):**
- Full SalvageCrate W3D turret subobject swap (Turret / TurretUp01 / TurretUp02)
- Full Marauder ClipSize=2 / ClipReloadTime=100 dual-shot cadence matrix
- Full RiderChangeContain STATUS_RIDER* death OCL / ScuttleDelay TOPPLED matrix
- Full UseRiderStealth / Jarmen Kell secondary pilot-sniper AutoChoose matrix
- Network salvage / rider replication (network deferred)

## Residual Host Playability — GLA Technical + Toxin Tractor (2026-07-13)
**Closed (host-testable GLA Technical transport/salvage + Toxin Tractor poison residual):**
1. **Technical residual** (`GLAVehicleTechnical` / chassis reskins / Chem_/Demo_/Slth_):
   - `TransportContain` Slots=**5**, infantry only (passengers “garrison” the truck bed).
   - Passengers do **not** fire (`PassengersAllowedToFire` unset in retail).
   - Salvage weapon upgrade residual (WEAPON_SALVAGER / CRATEUPGRADE):
     - Tier 0: `TechnicalMachineGunWeapon` (10 dmg / 150 range / 200ms)
     - Tier 1: `TechnicalCannonWeapon` (45 dmg / 150 range / 1000ms / radius 25 splash)
     - Tier 2: `TechnicalRPGWeapon` (50 dmg / 150 range / min 5 / 1000ms / radius 5)
   - Honesty: `honesty_technical_ok` / weapon_upgrades / loads / unloads / fires.
2. **Toxin Tractor residual** (`GLAVehicleToxinTruck`):
   - PRIMARY `ToxinTruckGun` poison stream (10 dmg / radius 10 / range 100).
   - SECONDARY `ToxinTruckSprayer` contaminate residual: SecondaryDamage **2** /
     radius **75** + spawn MediumPoisonField DoT (2 / 80 / 30s / 500ms ticks).
   - Death residual: SmallPoisonField (2 / 12 / 10s) via ToxinShellWeapon path.
   - Honesty: `honesty_toxin_tractor_stream_ok` / `honesty_toxin_tractor_spray_ok` /
     `honesty_toxin_tractor_death_field_ok` / `honesty_toxin_tractor_ok`.
3. Tests (not log-only):
   - `technical_residual_transport_and_salvage_weapon`
   - `toxin_tractor_residual_stream_spray_and_death_field`
   - module unit tests in `host_technical.rs` / `host_toxin_tractor.rs`

**Still residual (fail-closed, not claimed):**
- Full SalvageCrate W3D gunner/turret subobject swap matrix (Technical/Toxin)
- Full FireOCLAfterWeaponCooldown MinShots=4 continuous-coast timer for spray
- Full stream projectile drawing / spigot bone / chassis reskin visual matrix
- Network salvage / toxin replication (network deferred)

## Residual Host Playability — GLA Rocket Buggy / Quad Cannon / SCUD (2026-07-13)
**Closed (host-testable GLA combat vehicle residuals):**
1. **Rocket Buggy residual** (`GLAVehicleRocketBuggy` / Chem_/Demo_/Slth_):
   - Seeds `BuggyRocketWeapon` (range **300**, min **50**, dmg **20**, clip **6**).
   - Fire residual: intended target PrimaryDamage; units in SecondaryDamageRadius
     **10** take SecondaryDamage **5** splash.
   - `ScatterRadiusVsInfantry` residual (deterministic miss vs infantry).
   - Honesty: `honesty_rocket_buggy_ok` / fires / units_hit / scatter_misses.
2. **Quad Cannon residual** (`GLAVehicleQuadCannon`):
   - PRIMARY `QuadCannonGun` ground (range **150**, dmg **10**, no air).
   - SECONDARY `QuadCannonGunAir` AA (range **350**, dmg **5**, AntiGround=No).
   - Chooser residual: airborne → secondary; ground → primary.
   - Multi-barrel salvage tier residual (`apply_quad_cannon_barrel_tier` 0/1/2
     fire-rate + weapon-name residual).
   - Honesty: ground_fires / aa_fires / barrel_upgrades.
3. **SCUD launcher residual** (`GLAVehicleScudLauncher`):
   - PRIMARY explosive area (300/50 + 50/100), range **350**, min **200**.
   - SECONDARY toxin area (200/30 + 25/60) + **MediumPoisonField** DoT
     (2 dmg / radius 80 / 30s / 500ms ticks).
   - PreferredAgainst residual: secondary vs infantry (toxin).
   - Honesty: `honesty_scud_area_ok` / `honesty_scud_toxin_ok` /
     `honesty_scud_launcher_ok`.
4. Tests (not log-only):
   - `rocket_buggy_residual_long_range_splash`
   - `quad_cannon_residual_anti_air_and_multi_barrel`
   - `scud_launcher_residual_area_and_toxin`
   - module unit tests in `host_rocket_buggy.rs` / `host_quad_cannon.rs` /
     `host_scud_launcher.rs`

**Still residual (fail-closed, not claimed):**
- Full projectile lob / MissileCallsOnDie / PreAttack PER_SHOT animation matrix
- Full SalvageCrate W3D turret subobject swap / AP rocket mult matrix
- Full Anthrax Beta upgraded poison particle / gamma field matrix
- Network weapon / toxin replication (network deferred)

## Residual Host Playability — Bomb Truck Disguise + GLA Camouflage (2026-07-12)
**Closed (host-testable disguise + Camouflage stealth upgrade):**
1. **Bomb Truck disguise residual** (`SpecialAbilityDisguiseAsVehicle` /
   `GLAVehicleBombTruck` StealthUpdate DisguisesAsTeam):
   - `CommandType::DisguiseAsVehicle` on bomb-truck residual casters
     (`*BombTruck*` / `TestBombTruck`) targeting any living ground vehicle
     (ally/enemy/neutral) except bomb trucks / trains / aircraft.
   - Instant complete residual (retail `StartAbilityRange = 1e6`).
   - Sets `OBJECT_STATUS_DISGUISED` + `STEALTHED`, stores disguise template + team.
   - Enemies auto-target via **apparent team** residual (disguised-as-USA skips USA
     auto-target; China still targets as enemy). Not pure-stealth hide
     (`is_effectively_stealthed` excludes DISGUISED — C++ `!DISGUISED` gate).
   - Reveal residual: while attacking, if 2D distance to victim ≤ **100**
     (`RevealDistanceFromTarget`) → clear disguise + stealth + honesty reveal.
   - Honesty: `honesty_bomb_truck_disguise_ok` / `honesty_bomb_truck_reveal_ok` /
     `honesty_bomb_truck_disguise_path_ok`.
2. **GLA Camouflage residual** (`Upgrade_GLACamouflage` / Rebel `StealthUpgrade`):
   - `HostUpgradeKind::Camouflage` QueueUpgrade → complete grants residual
     STEALTHED + `innate_stealth` to Rebel infantry templates
     (`GLAInfantryRebel` / Chem_/Demo_ variants / `TestRebel`).
   - Attack breaks stealth (`stealth_breaks_on_attack`); idle re-cloak residual
     in `update_stealth_and_detection`.
   - **Workers fail-closed**: `GLAInfantryWorker` has no StealthUpgrade for
     Camouflage in retail INI — residual correctly skips workers.
   - Honesty: host_upgrades `honesty_host_path_ok(Camouflage)` with units_affected.
3. Tests (not log-only):
   - `bomb_truck_disguise_residual_applies_and_hides_from_disguise_team`
   - `bomb_truck_disguise_residual_reveals_near_attack_target`
   - `bomb_truck_disguise_residual_rejects_non_bomb_truck_caster`
   - `camouflage_upgrade_queue_complete_stealths_rebel`
   - `camouflage_residual_attack_breaks_and_idle_recloaks`
   - module unit tests in `host_bomb_truck_disguise.rs` / `host_upgrades` camouflage helpers

**Still residual (fail-closed, not claimed):**
- Full StealthUpdate disguise transition opacity / half-point model swap / FX
- Full drawable indicator-color night/day matrix for disguised players
- Full 2500ms StealthDelay re-cloak timer / FriendlyOpacity pulse for Camouflage
- Bomb truck FireWeaponWhenDead HE/Bio residual closed 2026-07-13 (see Helix Napalm + Bomb Truck HE/Bio section)
- Network disguise / camouflage replication (network deferred)

## Residual Host Playability — GLA Tunnel Network Enter/Exit (2026-07-12)
**Closed (host-testable TunnelContain shared pool + cross-tunnel exit):**
1. **Tunnel Network residual** (`GLATunnelNetwork` / general variants / SneakAttack tunnel):
   - `TunnelContain` shared passenger pool per team (`GameData.ini MaxTunnelCapacity = 10`).
   - Enter any allied tunnel network structure (all ground units; aircraft residual-skip).
   - Exit / Evacuate on **any** allied tunnel dumps the shared pool at that tunnel
     (enter A → evacuate B places unit at B — key residual path).
   - Honesty: `honesty_tunnel_network_enter_exit_ok` /
     `honesty_tunnel_network_cross_exit_ok` / `honesty_tunnel_network_ok`.
2. Tests (not log-only):
   - `tunnel_network_residual_flags_and_capacity_installed`
   - `tunnel_network_residual_enter_sets_garrisoned_and_shared_pool`
   - `tunnel_network_residual_cross_exit_enter_a_evacuate_b`
   - `tunnel_network_residual_shared_capacity_full_rejects_enter`
   - `tunnel_network_residual_rejects_aircraft`
   - module unit tests in `host_tunnel_network.rs`

**Still residual (fail-closed, not claimed):**
- Full GuardTunnelNetwork / AITNGuard nemesis AI path
- Full TimeForFullHeal / healObjects tick while contained
- Full CaveSystem multi-index / last-tunnel cave-in destroy matrix
- Full ExitStart bone / multi-door exit interface
- TunnelNetworkGun residual closed 2026-07-13 — see Laser General + Tunnel Network Gun section
- Network tunnel-network replication (network deferred)

## Residual Host Playability — Pathfinder Stealth Detect + Scout/Hellfire Drones (2026-07-12)
**Closed (host-testable detect + stealth + drone attach/auto-fire):**
1. **Pathfinder residual** (`AmericaInfantryPathfinder`):
   - Spawns as stealth detector residual (DetectionRange unset → VisionRange **200**).
   - Innate stealth; `stealth_breaks_on_attack = false` (stays stealthed while sniping).
   - `stealth_breaks_on_move = true`: uncloaks while Moving/AttackMoving; re-cloaks when stopped
     (StealthDelay = 0 residual).
   - PRIMARY `USAPathfinderSniperRifle` (100 dmg / 300 range / 2000 ms).
   - Honesty: `honesty_pathfinder_detect_ok` / `honesty_pathfinder_sniper_ok`.
2. **Scout Drone residual** (`AmericaVehicleScoutDrone` / attach from Humvee):
   - Spawns as detector residual (VisionRange **150**); no primary weapon.
   - `residual_attach_slave_drone(master, Scout)` tags master with `Upgrade_AmericaScoutDrone`.
   - Honesty: `honesty_scout_drone_attach_ok` / `honesty_scout_drone_detect_ok`.
3. **Hellfire Drone residual** (`AmericaVehicleHellfireDrone` / attach from Humvee):
   - PRIMARY `HellfireMissileWeapon` (40 dmg / 150 range / ~3s cycle).
   - AutoAcquireEnemiesWhenIdle residual auto-fire (same idle-gate pattern as Sentry).
   - `residual_attach_slave_drone(master, Hellfire)` tags master with `Upgrade_AmericaHellfireDrone`.
   - Honesty: `honesty_hellfire_drone_attach_ok` / `honesty_hellfire_drone_auto_fire_ok`.
4. Tests (not log-only):
   - `pathfinder_residual_detect_stealth_and_sniper`
   - `scout_and_hellfire_drone_residual_attach_detect_and_fire`
   - `slave_drone_residual_rejects_non_master_attach`
   - module unit tests in `host_pathfinder.rs` / `host_slave_drones.rs`

**Closed (host-testable Combat Chinook passenger fire residual):**
1. **Combat Chinook residual** (`AirF_AmericaVehicleChinook`):
   - TransportContain Slots=**8**, `PassengersAllowedToFire=Yes`,
     `ArmedRidersUpgradeMyWeaponSet=Yes` (ListeningOutpostUpgradedDummyWeapon bind).
   - AllowInsideKindOf residual: infantry + vehicle (rejects aircraft).
   - Docked riders residual-fire from chinook origin (Battle Bus pattern).
   - PointDefenseLaser residual name matrix includes AirF Combat Chinook
     (ScanRange 250 / AttackRange 65 / Delay ~8 frames).
   - Honesty: `honesty_combat_chinook_load_unload_ok` /
     `honesty_combat_chinook_passenger_fire_ok` /
     `honesty_combat_chinook_weapon_set_upgrade_ok`.
   - Fail-closed: not ChinookAIUpdate ropes / supply / rappel / combat drop.
2. Tests:
   - `combat_chinook_residual_capacity_and_flags_installed`
   - `combat_chinook_residual_enter_sets_docked_and_upgrades_weapon_set`
   - `combat_chinook_residual_load_two_unload_both_free`
   - `combat_chinook_residual_passenger_fire_damages_nearby_enemy`
   - `combat_chinook_residual_capacity_full_rejects_enter`
   - `combat_chinook_residual_allows_vehicle_enter`
   - module unit tests in `host_combat_chinook.rs`

**Still residual (fail-closed, not claimed):**
- Full SlavedUpdate guard/scout wander ranges / master layer lock
- Full ObjectCreationUpgrade ConflictsWith / ProductionUpdate door UI per-vehicle
- Full Pathfinder SCIENCE_Pathfinder prereq gate / FriendlyOpacity pulse / IR detector FX
- Full ChinookAIUpdate ropes / supply warehouse boxes / rappel / combat drop clear
- Humvee TOW / FlashBang remain partial via existing host upgrades

## Residual Host Playability — USA Crusader / Paladin / Avenger / Humvee (2026-07-13)
**Closed (host-testable tank guns, FAERIE_FIRE paint, Composite Armor, Humvee polish):**
1. **Crusader residual** (`AmericaTankCrusader` / `USA_Crusader`):
   - PRIMARY `CrusaderTankGun` (60 dmg / 150 range / 2000 ms → 60 frames).
   - Upgrade_AmericaCompositeArmor MaxHealthUpgrade residual **+100** HP
     (ADD_CURRENT_HEALTH_TOO) on Crusader / Paladin.
   - Honesty: CompositeArmor host upgrade kind + unit max_health observability.
2. **Paladin residual** (point defense already partial; this pass):
   - PRIMARY `PaladinTankGun` (same gun residual as Crusader).
   - PDL secondary infantry intercept fail-closed to **Paladin only**
     (Avenger / King Raptor / Combat Chinook: missiles only).
   - Shares Composite Armor residual with Crusader.
3. **Avenger residual** (`AmericaTankAvenger` / Target Designator):
   - PRIMARY paints `OBJECT_STATUS_FAERIE_FIRE` (status residual; no HP damage).
   - Duration 200 ms → 6 frames residual (retail continuous refresh).
   - Allies shooting painted targets get TARGET_FAERIE_FIRE **150%** ROF residual.
   - SECONDARY air laser residual (dual-turret collapse): 10 dmg / 300 range / AA only.
   - PDL intercept remains via `host_point_defense`.
   - Honesty: `honesty_avenger_paint_ok` / `honesty_avenger_air_laser_ok` /
     `honesty_avenger_rof_ok` / `honesty_avenger_ok`.
4. **Humvee residual polish** (`AmericaVehicleHumvee`):
   - TransportContain Slots=**5**, PassengersAllowedToFire=Yes residual install.
   - TOW secondary residual gains air tertiary capability after research
     (range 320 / can_target_air; damage boost **50** vs aircraft residual).
5. Tests (not log-only):
   - `avenger_residual_designator_paint_and_rof`
   - `avenger_residual_air_laser_damages_aircraft`
   - `crusader_residual_tank_gun_and_composite_armor`
   - `humvee_residual_transport_and_air_tow`
   - module unit tests in `host_avenger.rs` / `host_usa_tanks.rs` / `host_humvee.rs`

**Still residual (fail-closed, not claimed):**
- Full portable AmericaTankAvengerLaserTurret OverlordContain dual-stream matrix
- Full StatusDamageHelper Xfer / multi-status exclusivity
- Full ArmorSet PLAYER_UPGRADE UpgradedTankArmor matrix
- Full SCIENCE_PaladinTank prereq gate / ProductionUpdate door UI
- Full WeaponSet PLAYER_UPGRADE Humvee turret visual swap
- Network FAERIE_FIRE / composite / TOW air replication (network deferred)

- Network detector / drone replication (network deferred)

# GeneralsRust Playability State (2026-04-02)

## Residual Host Playability — Microwave Tank + King Raptor Laser (2026-07-12)
**Closed (host-testable Microwave disable/clear + King Raptor PDL):**
1. **Microwave Tank residual** (`MicrowaveTankBuildingDisabler` / `BuildingClearer`):
   - AmericaTankMicrowave / *Microwave* residual sources, while **attacking** an
     enemy/neutral structure within AttackRange residual **200**, apply
     `DISABLED_SUBDUED` (`disabled_subdued` → `is_disabled()` so production
     / powered functions stop while cooked).
   - Clears when microwave stops targeting / leaves range / dies.
   - Garrison clear residual (KILL_GARRISONED / PrimaryDamage **1** occupant per
     shot, AttackRange **125**) via existing combat path +
     `host_bunker_buster` clearer residual (no structure HP damage).
   - Ally structures residual-skip (fail-closed vs retail RadiusDamageAffects ALLIES).
   - Honesty: `honesty_microwave_disable_ok` / `honesty_microwave_ok` +
     `honesty_kill_garrisoned_ok` for clear path.
2. **King Raptor residual laser** (`PointDefenseLaserUpdate` dual modules):
   - AirF_AmericaJetRaptor / TestKingRaptor residual carriers added to
     `host_point_defense` PDL path (regular AmericaJetRaptor fail-closed skip).
   - Dual lasers residual collapse: AirF_RaptorPointDefenseLaser +
     AirF_PointDefenseLaser → fire range **65**, delay **4** frames
     (250ms each dual-stream collapse), damage **100**, scan **200**.
   - Intercepts enemy missiles / projectiles in fire range (same primary
     residual as Paladin/Avenger).
3. Tests (not log-only):
   - `microwave_tank_residual_disables_enemy_structure`
   - `microwave_tank_residual_skips_non_microwave`
   - `kill_garrisoned_residual_microwave_clears_occupants`
   - `king_raptor_residual_laser_intercepts_missile`
   - `king_raptor_residual_skips_regular_raptor`
   - unit tests in `host_microwave.rs` / `host_point_defense.rs`

**Still residual (fail-closed, not claimed):**
- Full subdual damage accumulate / SubdualDamageHelper heal drain matrix
- Full MicrowaveDisableStream laser attach / FireWeaponUpdate emitter
  infantry MICROWAVE field (MicrowaveTankEmitterWeapon)
- Vehicle disabler (retail WeaponSet has MicrowaveTankVehicleDisabler commented out)
- Full dual independent PointDefenseLaserUpdate scan-rate / PredictTargetVelocity
- Network microwave / King Raptor PDL replication (network deferred)

## Residual Host Playability — Particle Uplink Continuous Beam + Cleanup Area (2026-07-12)
**Closed (host-testable ParticleCannon multi-pulse beam + Ambulance CleanupArea):**
1. **Particle Uplink continuous beam residual** (`ParticleUplinkCannonUpdate` host path):
   - `DoSpecialPower(ParticleCannon)` still queues charge residual (impact_frame
     delay **120** frames).
   - On complete: **no one-shot blast** (was residual 3000 flat); spawns
     `HostParticleBeamField` at target.
   - Beam residual: TotalFiringTime **3500 ms → 105 frames**, TotalDamagePulses
     **40**, DamagePerSecond **400** → **35 dmg/pulse**, pulse interval **3**
     frames, radius **50** (fail-closed vs DamageRadiusScalar grow matrix /
     swath sine / manual beam driving).
   - First pulse on beam-start frame; subsequent pulses until duration/pulse cap.
   - Honesty: `honesty_beam_ok` / `honesty_beam_damage_ok` /
     `honesty_host_path_ok(ParticleCannon)` requires beam field spawn.
2. **Cleanup Area residual** (`CleanupAreaPower` / AmbulanceCleanHazardWeapon):
   - `DoSpecialPower(CleanupArea)` from ambulance/medic/dozer/worker clears
     residual **toxin + radiation fields** and **enemy/neutral mines** in radius
     **50** (PrimaryDamageRadius residual) when caster is within
     MaxMoveDistanceFromLocation **300**.
   - Mine clear is safe disarm (no splash) via existing `clear_mine_internal`.
   - Honesty: `honesty_cleanup_area_activate_ok` / `honesty_cleanup_area_clear_ok` /
     `honesty_cleanup_area_ok`.
3. Tests (not log-only):
   - `particle_cannon_params_match_retail_continuous_beam`
   - `particle_cannon_impact_spawns_beam_and_ticks_damage`
   - `particle_cannon_host_path_queues_and_completes` (multi-pulse E2E)
   - `cleanup_area_residual_clears_hazards_and_mines`
   - `cleanup_area_does_not_queue_superweapon_strike`
   - unit tests in `host_cleanup_area.rs`

**Still residual (fail-closed, not claimed):**
- Full ParticleUplinkCannonUpdate outer-node lasers / WidthGrowTime / SwathOfDeath
  sine path / ManualDrivingSpeed / DamagePulseRemnant trail objects
- Full CleanupHazardUpdate scan/shot/clip / CleanupStreamProjectile path
- Full HazardousMaterialArmor CLEANUP_HAZARD KindOf object stack / rubble pathfind
- Network Particle Uplink / CleanupArea replication (network deferred)

## Residual Host Playability — Oil Derrick + Hacker Internet Center Cash (2026-07-12)
**Closed (host-testable TechOilDerrick AutoDeposit + China Hacker/Internet Center cash):**
1. **Oil Derrick residual income** (`TechOilDerrick` AutoDepositUpdate residual):
   - Captured (non-neutral) constructed derrick deposits **$200** every **360** logic
     frames (retail DepositAmount=200, DepositTiming=12000 ms @ 30 FPS).
   - **InitialCaptureBonus $1000** once when a derrick first becomes non-neutral owned
     (Player::gainObject → awardInitialCaptureBonus residual).
   - Neutral / under-construction residual-skip.
   - Honesty: `honesty_oil_derrick_deposit_ok` / `honesty_oil_derrick_capture_bonus_ok`
     / `honesty_oil_derrick_ok`.
2. **Hacker / Internet Center residual cash** (`HackInternetAIUpdate` residual):
   - Field: `start_hacker_internet_hack` starts residual hacking → **$5** every **60**
     frames (RegularCashAmount / CashUpdateDelay 2000 ms).
   - Internet Center: hackers `contained_by` FSInternetCenter auto-start and use
     CashUpdateDelayFast → **$5** every **54** frames (1800 ms).
   - Veterancy residual: Regular/Veteran/Elite/Heroic = 5/6/8/10; XpPerCashUpdate +1.
   - DISABLED_HACKED residual-skip (no deposit while disabled).
   - Honesty: `honesty_hacker_income_ok` / `honesty_hacker_internet_center_ok`.
3. Tests (not log-only):
   - `oil_derrick_residual_deposits_cash_on_interval`
   - `oil_derrick_residual_skips_under_construction`
   - `hacker_internet_center_residual_deposits_cash`
   - `hacker_field_residual_deposits_cash_on_interval`
   - `hacker_residual_rejects_non_hacker`
   - unit tests in `host_oil_derrick.rs` / `host_hacker_income.rs`

**Still residual (fail-closed, not claimed):**
- Full InGameUI AutoDeposit floating text GPU / STEALTHED gate (host floating text + SupplyLines +20 residual closed 2026-07-13 — see AutoDeposit Floating Cash + Oil SupplyLines Boost section)
- Full HackInternet unpack/pack state machine / variation factor / model conditions
- Full InternetHackContain passenger order matrix / microwave resume path
- Supply Drop Zone residual (OCL crate plane every 120s — not this slice)
- Network oil-derrick / hacker cash replication (network deferred)

## Residual Host Playability — Remote Demo Charge + Black Market Cash (2026-07-12)
**Closed (host-testable plant/detonate remote C4 + GLA Black Market AutoDeposit):**
1. **Remote Demo Charge residual** (Colonel Burton `SPECIAL_REMOTE_CHARGES`):
   - `PlantRemoteDemoCharge { target_id }` walks to structure/vehicle → plants sticky
     `HostMineKind::RemoteDemoCharge` (no auto-timer; TNTDetonationWeapon residual
     damage **500** / radius **50**).
   - `DetonateRemoteDemoCharges` detonates all remote charges with matching
     `producer_id` (C++ no-target SPECIAL_REMOTE_CHARGES path).
   - Timed charges remain separate (`PlantTimedDemoCharge` + LifetimeUpdate residual).
   - Honesty: `honesty_plant_remote_demo_charge_ok` /
     `honesty_remote_demo_charge_detonate_ok` (+ hero registry counters).
2. **GLA Black Market residual cash** (`AutoDepositUpdate` ModuleTag_05 residual):
   - Constructed `FSBlackMarket` / `*BlackMarket*` deposits **$20** every **60** logic
     frames (retail DepositAmount=20, DepositTiming=2000 ms @ 30 FPS).
   - Fake markets (`*Fake*`) residual-skip (ActualMoney=No).
   - Neutral / under-construction residual-skip.
   - Honesty: `honesty_black_market_deposit_ok` / `honesty_black_market_ok`.
3. Tests (not log-only):
   - `plant_and_detonate_remote_demo_charge_residual`
   - `black_market_residual_deposits_cash_on_interval`
   - `black_market_residual_cash_increases_over_frames`
   - `black_market_residual_skips_under_construction`
   - `black_market_residual_skips_fake_market`
   - unit tests in `host_mines.rs` / `host_black_market.rs` / `host_hero_abilities.rs`

**Still residual (fail-closed, not claimed):**
- Full StickyBombUpdate attach bones / max-charge special-object list / packing anim
- Full InGameUI AutoDeposit floating text GPU (host floating text residual closed 2026-07-13 — see AutoDeposit Floating Cash section)
- Supply Drop Zone residual (OCL crate plane — still open)
- Fuel Air Bomb is already covered by DaisyCutter host strike path (not reopened)
- Network remote-charge / black-market replication (network deferred)

## Residual Host Playability — Artillery Barrage (2026-07-12)
**Closed (host-testable Artillery Barrage → delayed multi-shell scatter damage):**
1. **Artillery Barrage residual** (`SUPERWEAPON_ArtilleryBarrage1` host path):
   - `DoSpecialPower(Artillery)` queues a delayed strike (retail DelayDeliveryMax
     3000 ms → 90 frames @ 30 FPS).
   - On impact: multi-shell scatter area damage — Level1 FormationSize **12** shells
     (center + deterministic ring inside WeaponErrorRadius **100**).
   - Per-shell residual: `ArtilleryBarrageDamageWeapon` PrimaryDamage **105** /
     PrimaryDamageRadius **50**.
   - Each living enemy takes max damage from any shell epicenter (not single-point only).
   - Friendlies residual-skip (fail-closed vs retail RadiusDamageAffects ALLIES).
2. Honesty counters (`special_power_strikes.rs` HostSuperweaponKind::ArtilleryBarrage):
   - queue / complete / host_path honesty flags (same strike residual path as CarpetBomb)
3. Tests (not log-only):
   - `artillery_barrage_host_path_queues_and_applies_delayed_multi_shell_damage`
   - `artillery_barrage_params_match_retail_multi_shell`
   - `artillery_barrage_delayed_multi_shell_scatter_damage`

**Still residual (fail-closed, not claimed):**
- Full ChinaArtilleryCannon OCL DeliverPayload transport / shell projectile path
- Random WeaponErrorRadius scatter draw + per-shell DelayDelivery stagger
- Science tier FormationSize 24/36 upgrade matrix
- Friendly-fire (retail hits ALLIES NEUTRALS ENEMIES)
- Network Artillery Barrage replication (network deferred)

## Residual Host Playability — Emergency Repair (2026-07-12)
**Closed (host-testable Emergency Repair → SingleBurst ally vehicle heal):**
1. **Emergency Repair residual** (`SuperweaponEmergencyRepair` host path):
   - `DoSpecialPower(EmergencyRepair)` at a world location heals damaged **same-team VEHICLE**
     units in residual radius **100** (retail RadiusCursorRadius /
     RepairVehiclesInArea_InvisibleMarker AutoHealBehavior Radius).
   - HealingAmount residual **100 / 200 / 300** by Level1/2/3 (default Level1 fail-closed).
   - KindOf VEHICLE only; infantry / enemies / full-HP / out-of-radius residual-skip.
2. Honesty counters (`host_emergency_repair.rs` + GameLogic):
   - `activation_count` / `heal_count` / `heal_amount_total`
   - `honesty_emergency_repair_activate_ok` / `honesty_emergency_repair_heal_ok` /
     `honesty_emergency_repair_ok`
3. Tests (not log-only):
   - `emergency_repair_residual_heals_damaged_ally_vehicles`
   - `emergency_repair_does_not_queue_superweapon_strike`
   - unit tests in `host_emergency_repair.rs`

**Still residual (fail-closed, not claimed):**
- Full OCL RepairVehicles invisible marker / RepairCloud particle path
- Full science tier upgrade matrix (Level2/3 selection from player sciences)
- Full ally relationship filter (uses same-team residual)
- Network Emergency Repair replication (network deferred)

## Residual Host Playability — GPS Scrambler (2026-07-12)
**Closed (host-testable GPS Scrambler → GrantStealth ally vehicles/infantry):**
1. **GPS Scrambler residual** (`SuperweaponGPSScrambler` host path):
   - `DoSpecialPower(GpsScrambler)` at a world location grants **STEALTHED** to
     same-team **VEHICLE|INFANTRY** in residual FinalRadius **100**
     (retail GrantStealthBehavior receiveGrant → CAN_STEALTH + STEALTHED).
   - Stealthed-and-undetected units are not enemy-targetable / not visible to enemies
     (existing host stealth gates). Attack still breaks stealth
     (STEALTH_NOT_WHILE_ATTACKING residual).
   - Skips bomb-truck disguise residual by name (C++ canDisguise skip).
   - Note: older module comments claiming "disables enemy radar" are incorrect for
     ZH retail — this is GrantStealth on allies, not radar jam.
2. Honesty counters (`host_gps_scrambler.rs` + GameLogic):
   - `activation_count` / `grant_count`
   - `honesty_gps_scrambler_activate_ok` / `honesty_gps_scrambler_grant_ok` /
     `honesty_gps_scrambler_ok`
3. Tests (not log-only):
   - `gps_scrambler_residual_grants_stealth_to_ally_units`
   - `gps_scrambler_does_not_queue_superweapon_strike`
   - unit tests in `host_gps_scrambler.rs`

**Still residual (fail-closed, not claimed):**
- Full OCL GPSScrambler_InvisibleMarker grow-from-StartRadius pulse scan
- Full StealthUpdate module framesGranted / opacity drawable / flashAsSelected
- Full ally relationship filter (uses same-team residual)
- Network GPS Scrambler replication (network deferred)

## Residual Host Playability — Leaflet Drop (2026-07-12)
**Closed (host-testable USA Leaflet Drop → delayed enemy infantry/vehicle disable):**
1. **Leaflet Drop residual** (`LeafletDropBehavior` host path):
   - `DoSpecialPower(LeafletDrop)` queues a delayed mission (retail Delay=2500 ms → 75 frames).
   - On impact: enemy **INFANTRY|VEHICLE** in AffectRadius residual **110** receive
     `DISABLED_EMP` for DisabledDuration residual **20000 ms → 600 frames**.
   - Allies / Neutral / structures residual-skip (C++ relationship ENEMIES + kind filter).
   - Reuses host `disabled_emp` tick path (cannot move/attack until expiry).
2. Honesty counters (`host_leaflet_drop.rs` + GameLogic):
   - `activation_count` / `disable_count`
   - `honesty_leaflet_drop_activate_ok` / `honesty_leaflet_drop_disable_ok` /
     `honesty_leaflet_drop_ok`
3. Tests (not log-only):
   - `leaflet_drop_residual_disables_enemy_infantry_and_vehicles`
   - unit tests in `host_leaflet_drop.rs`

**Still residual (fail-closed, not claimed):**
- Full OCL AmericaJetB52 / LeafletContainer drawable / LeafletFX particle path
- Full EarlyLeafletDrop science shortcut timer matrix
- Full multiplayer SharedSyncedTimer / academy classification
- Network leaflet replication (network deferred)

## Residual Host Playability — GLA Sneak Attack (2026-07-12)
**Closed (host-testable Sneak Attack → delayed tunnel spawn + shockwave):**
1. **Sneak Attack residual** (`SuperweaponSneakAttack` host path):
   - `DoSpecialPower(SneakAttack)` queues mission with Lifetime residual **5000 ms → 150 frames**
     (retail GLASneakAttackTunnelNetworkStart LifetimeUpdate).
   - On spawn: creates tunnel structure (`GLASneakAttackTunnelNetwork` if loaded, else
     residual `TestSneakTunnel`) for casting team at target location.
   - Residual shockwave pulse at spawn (SneakAttackShockwaveWeaponBig: **50 dmg / radius 50**).
2. Honesty counters (`host_sneak_attack.rs` + GameLogic):
   - `activation_count` / `tunnel_spawn_count` / `shockwave_hit_count`
   - `honesty_sneak_attack_activate_ok` / `honesty_sneak_attack_tunnel_ok` /
     `honesty_sneak_attack_shockwave_ok` / `honesty_sneak_attack_ok`
3. Tests (not log-only):
   - `sneak_attack_residual_spawns_tunnel_and_shockwave`
   - unit tests in `host_sneak_attack.rs`

**Still residual (fail-closed, not claimed):**
- Full OCL Start model animation / crack-dust particle stack
- Full multi-shockwave FireWeaponUpdate timing (10ms / 1000ms / 2500ms)
- Full GuardTunnelNetwork AI path (enter/exit residual closed above)
- SharedSyncedTimer / multiplayer academy classification
- Network sneak-attack replication (network deferred)

## Residual Host Playability — PointDefenseLaser Intercept (2026-07-12)
**Closed (host-testable Paladin/Avenger auto-destroy nearby enemy missiles):**
1. **PointDefenseLaser residual** (`PointDefenseLaserUpdate` host path):
   - `GameLogic::update_point_defense_intercept` each frame (from `update_simulation`).
   - Carriers: name residual `paladin` / `avenger` (incl. general variants / TestPaladin).
   - Primary targets: `KindOf::Projectile` or missile name residual
     (`missile` / `rocket` / `scud` / `tomahawk` / `TestMissile`); skips
     MissileDefender / Patriot Battery / Stinger Site false positives.
   - Secondary residual: enemy infantry in fire range (Paladin SecondaryTargetTypes).
   - Stats residual: Paladin range 65 / delay 30 frames / dmg 100;
     Avenger range 100 / delay 15 frames / dmg 100.
2. Honesty counters:
   - `point_defense_residual_intercepts`
   - `honesty_point_defense_intercept_ok`
3. Tests (not log-only):
   - `point_defense_laser_residual_intercepts_missile`
   - `point_defense_laser_residual_skips_non_carrier`
   - unit tests in `host_point_defense.rs`

**Still residual (fail-closed, not claimed):**
- Full PointDefenseLaserUpdate velocity prediction / scan-rate matrix
- Full KindOf BALLISTIC_MISSILE SMALL_MISSILE mask parse path
- Full TERTIARY WeaponStore allocateNewWeapon + laser beam drawable FX
- Network PDL replication (network deferred)

## Residual Host Playability — Neutron Shells (2026-07-12)
**Closed (host-testable Upgrade_ChinaNeutronShells → Nuke Cannon secondary blast):**
1. **Neutron shell residual** (`Upgrade_ChinaNeutronShells` + `NeutronBlastBehavior`):
   - QueueUpgrade complete equips Nuke Cannon SECONDARY `NukeCannonNeutronWeapon`
     (range 350, dmg 1 seed; blast does the work).
   - Combat secondary fire (`active_weapon_slot == 1` or only-ready secondary)
     applies residual blast radius 70 at impact:
     - Infantry killed
     - Vehicles unmanned + Neutral (HP preserved; combat-bike residual killed)
   - Primary NukeCannonGun still uses normal HP damage (no blast).
2. Honesty counters:
   - `neutron_shell_residual_blasts` / `infantry_kills` / `vehicles_unmanned`
   - `honesty_neutron_shell_ok`
3. PreferredAgainst AutoChoose residual expansion:
   - Secondary preferred vs infantry when secondary damage > primary (FlashBang)
   - Secondary preferred vs vehicles when secondary damage > primary (TOW)
   - Structures path unchanged (secondary damage ≥ primary)
4. Tests (not log-only):
   - `neutron_shell_residual_upgrade_and_blast`
   - `neutron_shell_residual_primary_does_not_blast`
   - `update_combat_prefers_secondary_damage_vs_infantry`
   - unit tests in `host_neutron_shell.rs` / `host_upgrades` classification

**Still residual (fail-closed, not claimed):**
- Full DumbProjectileBehavior bezier flight / min-range deploy matrix
- Full AffectAirborne / ally Relationship / contained-passenger kill matrix
- Full WeaponSet command-button toggle UI parity beyond `active_weapon_slot`
- Full INI PreferredAgainst / AutoChoose tables beyond damage+kind residual
- Network neutron replication (network deferred)

## Residual Host Playability — Cash Bounty on Kill (2026-07-12)
**Closed (host-testable kill with cash_bounty_percent → killer cash increases):**
1. **GLA SCIENCE_CashBounty residual** (`Player::doBountyForKill` + CashBountyPower):
   - Player holds `cash_bounty_percent` (default 0; tiers 5% / 10% / 20%).
   - `SCIENCE_CashBounty1/2/3` unlock (or `set_player_cash_bounty`) raises percent.
   - On enemy kill (`record_destruction` after combat): awards
     `ceil(victim_build_cost * cash_bounty_percent)` to killer supplies.
   - Skips under-construction, same-team, Neutral, and zero-percent cases.
2. Honesty (`host_cash_bounty.rs` + GameLogic):
   - `cash_bounty.bounty_kills` / `bounty_earned_total` / `max_bounty_percent`
   - `honesty_cash_bounty_ok` / `honesty_cash_bounty_award_ok` / `cash_bounty_earned_total`
3. Tests (not log-only):
   - `cash_bounty_increases_cash_on_enemy_kill`
   - `cash_bounty_zero_percent_does_not_award`
   - `cash_bounty_science_unlock_sets_percent`
   - unit tests in `host_cash_bounty.rs`
4. Engine parity: GameLogic `Player::do_bounty_for_kill` also `score_keeper.add_money_earned`.

**Still residual (fail-closed, not claimed):**
- Full CashBountyPower palace module + RequiredScience gate matrix
- Floating text / InGameUI AddCash over killer
- calcCostToBuild faction handicap matrix (uses template build_cost)
- Network bounty replication (network deferred)

## Residual Host Playability — Propaganda / Speaker Tower (2026-07-12)
**Closed (host-testable damaged ally near tower → HP recovers + ENTHUSIASTIC buff):**
1. **China Speaker Tower / PropagandaTower residual** (`PropagandaTowerBehavior`):
   - `GameLogic::update_propaganda_tower_pulse` each frame (from `update_simulation`).
   - Heals damaged **same-team non-structure** units within residual radius 150 at
     2% max-health/sec (4% with `Upgrade_ChinaSubliminalMessaging`).
   - Sets `weapon_bonus_enthusiastic` (base) / `weapon_bonus_subliminal` (upgrade);
     clears flags when unit leaves radius or tower is gone.
   - Name residual: `speakertower` / `propagandatower` / `listeningoutpost` /
     `tankemperor` / ends with `emperor`; excludes `propagandacenter`.
2. Honesty counters (`host_propaganda.rs` + GameLogic):
   - `propaganda_residual_heals` / `propaganda_residual_buffs`
   - `honesty_propaganda_heal_ok` / `honesty_propaganda_buff_ok` / `honesty_propaganda_ok`
3. Tests (not log-only):
   - `propaganda_tower_residual_recovers_hp_and_sets_enthusiastic`
   - `propaganda_tower_residual_out_of_range_then_in_range`
   - `propaganda_tower_residual_skips_enemy_units`
   - `propaganda_tower_residual_subliminal_upgrade_buff_and_faster_heal`
   - `propaganda_tower_name_residual_helix_propaganda_heals`
   - unit tests in `host_propaganda.rs`

**Still residual (fail-closed, not claimed):**
- Full sole-benefactor exclusivity / multi-tower reject matrix
- Full ally relationship filter (uses same-team residual)
- Full double-contain / stealthed FX suppress / POWERED underpower gate
- Full WeaponBonusConditionFlags ROF multiplier application in combat residual
- Full PulseFX / world-anim propaganda pulse
- Network propaganda replication (network deferred)

## Residual Host Playability — Ambulance / Infantry Heal (2026-07-12)
**Closed (host-testable damage → nearby ambulance / HealPad → infantry HP recovers):**
1. **USA Ambulance AutoHeal** (`AmericaVehicleMedic` residual):
   - `GameLogic::update_ambulance_auto_heal` each frame (from `update_simulation`).
   - Heals damaged **same-team infantry** within residual radius 100 at 4 HP/sec
     (C++ AutoHealBehavior ModuleTag_22: HealingAmount=4 / Delay=1000ms / KindOf=INFANTRY).
   - Name residual: template contains `ambulance` / `vehiclemedic` / ends with `medic`.
2. **HealPad command path** (`CommandType::GetHealed` → `AIState::SeekingHealing`):
   - Existing support-state dock heal now records `heal_residual_heal_pad_heals` honesty.
3. Honesty counters (`host_heal.rs` + GameLogic):
   - `heal_residual_ambulance_heals` / `heal_residual_heal_pad_heals`
   - `honesty_ambulance_heal_ok` / `honesty_heal_pad_ok` / `honesty_heal_ok`
4. Tests (not log-only):
   - `ambulance_auto_heal_residual_recovers_infantry_hp`
   - `ambulance_auto_heal_residual_out_of_range_then_in_range`
   - `ambulance_auto_heal_residual_skips_enemy_infantry`
   - `heal_pad_seeking_healing_residual_recovers_infantry_hp`
   - unit tests in `host_heal.rs`

**Still residual (fail-closed, not claimed):**
- Full sole-benefactor exclusivity / multi-ambulance reject
- Vehicle AutoHeal ModuleTag_23 (VEHICLE, ForbiddenKindOf=AIRCRAFT, SkipSelf)
- TransportContain HealthRegen%PerSec while embarked
- Particle / world-anim heal pulse FX
- Network heal replication (network deferred)

## Residual Host Playability — Structure / Vehicle Repair (2026-07-12)
**Closed (host-testable damage → Repair / GetRepaired → HP recovers):**
1. **Dozer structure repair** (`CommandType::Repair`):
   - `CommandExecutor::execute_repair` accepts dozer/worker on damaged ally/neutral structure
     (not under construction) → `AIState::Repairing` + destination.
   - `GameLogic::update_support_states` Repairing branch: approach within interact range (14),
     then heal structure HP over time (`HOST_REPAIR_RATE_HP_PER_SEC` flat residual).
   - `stop_moving` preserves Repairing on arrival; `update_combat` skips fire/chase while Repairing.
   - Covers WarFactory-as-structure (dozer repairs a damaged WarFactory).
2. **Vehicle pad / WarFactory repair** (`CommandType::GetRepaired`):
   - Damaged vehicles accept RepairPad **or WarFactory** (China `RepairDockUpdate` residual).
   - Aircraft accept Airfield.
   - `AIState::SeekingRepair` → approach → self-heal over time at same residual rate.
3. Honesty counters (`host_repair.rs` helpers + GameLogic):
   - `repair_residual_structure_commands` / `repair_residual_structure_heals`
   - `repair_residual_vehicle_heals`
   - `honesty_structure_repair_ok` / `honesty_vehicle_repair_ok` / `honesty_repair_ok`
4. Tests (not log-only):
   - `dozer_structure_repair_residual_recovers_hp_over_time`
   - `dozer_structure_repair_residual_walk_into_range_recovers_hp`
   - `war_factory_vehicle_repair_residual_recovers_hp`
   - existing `repairing_state_heals_target_in_range` / repair command suite
   - unit tests in `host_repair.rs`

**Still residual (fail-closed, not claimed):**
- Full C++ `RepairHealthPercentPerSecond` INI matrix / sole-benefactor healing reject
- Full `RepairDockUpdate` `TimeForFullHeal` dock bones / drone heal
- Bridge scaffolding gate during repair
- Multi-dozer task queue / `privateRepair` accept-same-bridge matrix
- Network repair replication (network deferred)

## Residual Host Playability — Mine / Demo Trap / Demo Charge (2026-07-12)
**Closed (host-testable place → enemy trigger damage / timed detonation):**
1. Host residual on Main `GameLogic` + `Object.mine_data` (`host_mines.rs`):
   - **Land mine** place (`place_land_mine` / ClusterMines special power ring)
   - **Demo trap** place (`place_demo_trap`, GLADemoTrap proximity residual)
   - **Timed demo charge** place (`place_timed_demo_charge`, TNTStickyBomb residual)
   - **Remote demo charge** place (`place_remote_demo_charge` / PlantRemoteDemoCharge)
     + **DetonateRemoteDemoCharges** command (no auto-timer; producer-scoped)
2. `update_mines_and_demo_traps` each frame:
   - proximity: enemy (not ally) in trigger range → area damage + destroy mine/trap
   - timed: absolute frame deadline → detonation
   - manual: `manual_detonate_mine` for demo-trap / remote-charge command residual
   - **dozer/worker clear**: Worker/Dozer within clear range (5) of enemy/neutral mine
     → `clear_mine_internal` (DAMAGE_DISARM residual) destroys mine with no splash;
     clearers never proximity-detonate; idle clearers approach within scan range (100)
3. `SpecialPowerType::ClusterMines` via `DoSpecialPower` places residual mine ring
   (not full OCL ClusterMinesBomb / GenerateMinefieldBehavior density).
4. Honesty counters: places / proximity / timed / manual detonations / clears;
   `honesty_mine_place_trigger_ok` / `honesty_timed_demo_charge_ok` / `honesty_mine_clear_ok`
   / `honesty_plant_remote_demo_charge_ok` / `honesty_remote_demo_charge_detonate_ok`.
5. Tests (not log-only):
   - `mine_residual_place_enemy_triggers_damage`
   - `mine_residual_ally_does_not_trigger_land_mine`
   - `demo_trap_residual_proximity_detonates_on_enemy`
   - `timed_demo_charge_residual_detonates_after_delay`
   - `cluster_mines_special_power_places_mines`
   - `demo_trap_manual_detonate_residual`
   - `plant_and_detonate_remote_demo_charge_residual`
   - `dozer_mine_clear_residual_disarms_enemy_mine_safely`
   - `dozer_mine_clear_residual_approaches_then_clears`
   - `dozer_mine_clear_residual_skips_ally_mine`
   - `dozer_mine_clear_residual_infantry_still_triggers`
   - unit tests in `host_mines.rs`

**Still residual (fail-closed, not claimed):**
- Full C++ MinefieldBehavior virtual-mine regen / scoot / multi-slot immunity
- Full DemoTrapUpdate weapon-slot mode matrix / PreAttack scoop animation
- Full WEAPONSET_MINE_CLEARING_DETAIL / Weapon AntiMine targeting matrix
- Full StickyBombUpdate attach bones / geometry-based secondary splash / max-charge list
- Full OCL ClusterMinesBomb aircraft delivery path
- Full StickyBombUpdate bone attach for BoobyTrap (host plant/detonate residual closed 2026-07-13)
- Network mine replication (network deferred)

## Residual Host Playability — W3D Mesh Asset Resolve (2026-07-12)
- Closed highest-value mesh residual after PresentationFrame unit identity:
  - `assets/mesh_asset_resolve.rs`: `model_key` / `get_model_name` → canonical W3D key
  - USA_Ranger / airanger → `airanger_s` (shipped `AIRanger_S.W3D`)
  - Load real mesh when AssetManager or filesystem extract/sample present
  - Placeholder cube + `MeshResolveHonesty` when missing (opt-in draw via debug cubes)
  - PresentationFrame freezes aliased model_key for unit mesh pass
- Fail-closed: not full W3D material / animation / GPU retail parity.
- Tests: non-empty USA_Ranger key; placeholder honesty; load when assets present else skip.

## Scope
- Non-network parity only (project policy).
- Multiplayer/network behavior remains deferred until non-network systems are complete.

## Current Assessment
- The Rust port is materially further along than the older snapshot in this file, but it is still not fully playable end-to-end.
- Recent parity work has closed several high-value slices in rendering, audio, terrain, and gameplay flow.
- Core build health is currently good for the main gameplay crates:
  - `cargo check -q -p gamelogic`
  - `cargo check -q -p game_engine`
  - `cargo check -q -p game-client-rust --features internal`
- The file parity tracker remains at 100% for existence/mapping, so the remaining work is now behavior parity, not file coverage.

## Residual Host Playability — Save/Load Secondary Weapon Xfer (2026-07-12)
**Closed (host-testable object secondary survives snapshot + file save/load):**
1. Gap: `SnapshotBuilder::snapshot_object` only stored `object.weapon` in
   `ObjectSnapshot.weapons[0]`; `restore_object` set primary only →
   `Object.secondary_weapon` always `None` after load (FlashBang/TOW/combat dual-slot
   desync). `active_weapon_slot` was also dropped.
2. Fix (fail-closed residual layout, not full C++ WeaponSet Xfer table):
   - Capture: `weapons[0]=primary`, `weapons[1]=secondary` when present
   - Secondary-only: zero-damage primary pad so secondary restores at index 1
   - `ObjectStatusSnapshot.active_weapon_slot` Xfer + capture/restore
3. Tests:
   - `snapshot_restore_preserves_secondary_weapon_and_active_slot`
   - `snapshot_restore_preserves_secondary_only_weapon_slot`
   - `snapshot_weapon_layout_helpers_round_trip`
   - `save_file_roundtrip_preserves_secondary_weapon` (SaveFileManager path)

**Still residual (fail-closed, not claimed):**
- Full C++ per-module WeaponSet / SpecialPowerModule / particle Xfer tables
- Network save/load (network deferred)

## Residual Host Playability — Special Power Strike Save/Load Xfer (2026-07-12)
**Closed (host-testable mid-flight strike survives snapshot + file save/load):**
1. Gap: `HostSpecialPowerStrikeRegistry` and `CombatParticleRegistry` lived only on
   live `GameLogic` — `WorldSnapshot` never captured pending strikes, so save
   mid-flight dropped the queue and impact never fired after load.
2. Fix (fail-closed residual, not full retail OCL / SpecialPowerModule Xfer):
   - `WorldSnapshot.special_power_strikes` stores `next_id` + all strike records
     (queued / completed / cancelled) including absolute `impact_frame`
   - `WorldSnapshot.combat_particles` stores host particle system entries
     (template + pose + spawn frame; not full W3D GPU state)
   - `SnapshotBuilder` capture/restore + Xfer markers after `GlobalAIState`
   - Registry `restore_from_snapshot` rebinds allocator + entries
3. Tests:
   - `special_power_daisy_cutter_mid_flight_save_load_still_impacts`
   - `special_power_a10_mid_flight_save_load_still_impacts`
   - `save_file_roundtrip_preserves_pending_special_power_strike`
   - registry unit restore tests in `special_power_strikes.rs` / `combat_particles.rs`

**Still residual (fail-closed, not claimed):**
- Full retail OCL aircraft / beam / multiplayer superweapon Xfer tables
- Client `ParticleSystemManager` GPU rebind after load (host registry only)
- Full C++ per-module SpecialPowerModule / particle Xfer tables
- Network save/load (network deferred)

## Residual Host Playability — Host Upgrade Research Save/Load Xfer (2026-07-12)
**Closed (host-testable mid-flight upgrade research survives snapshot + file save/load):**
1. Gap: `HostUpgradeRegistry` lived only on live `GameLogic` — `WorldSnapshot` never
   captured pending research records/honesty. Player `queued_upgrades` already survived
   via `PlayerSnapshot.research_queue`, but host residual queue honesty + entry
   bookkeeping (source object, queue_frame, pending ids) was dropped mid-flight.
2. Fix (fail-closed residual, not full retail Upgrade.ini BuildTime / ProductionUpdate):
   - `WorldSnapshot.host_upgrades` stores `next_id` + all research records
     (queued / completed / cancelled) including `queue_frame` / `complete_frame`
   - `SnapshotBuilder` capture/restore + Xfer marker after `CombatParticles`
   - `HostUpgradeRegistry::restore_from_snapshot` rebinds allocator, entries, and
     `pending_index` so mid-research completes on the next update with unlocks
3. Tests:
   - `host_upgrade_capture_mid_flight_save_load_completes_unlock`
   - `save_file_roundtrip_preserves_pending_host_upgrade`
   - `restore_from_snapshot_keeps_pending_queue` in `host_upgrades.rs`

**Still residual (fail-closed, not claimed):**
- Full retail Upgrade.ini BuildTime (30s) research timers / ProductionUpdate door UI
- Full science tree purchase / prerequisite graph / academy stats
- Full WeaponSetUpgrade / CommandSetUpgrade module matrices for every unit
- Object-type upgrades (`UPGRADE_TYPE_OBJECT`) vs player-type split beyond residual tags
- Network upgrade replication (network deferred)
- Economy effect of SupplyLines beyond tag observability

## Residual Host Playability — Upgrade Queue/Complete Host Path (2026-07-12)
**Closed (host-testable QueueUpgrade → complete → observable unlock):**
1. Host `HostUpgradeRegistry` on Main `GameLogic` records queue/complete for residual
   kinds: CaptureBuilding, FlashBangGrenade, TowMissile, SupplyLines (+ Other flag-only).
2. `CommandExecutor::execute_queue_upgrade` still deducts cost once per team and inserts
   into `Player.queued_upgrades`; now also records host residual queue honesty.
3. `GameLogic::update_player_upgrades` completes research into `unlocked_sciences` and
   applies observable unlocks:
   - **Capture**: player unlock flag gates `CaptureBuilding`; infantry receive upgrade tags
   - **FlashBang**: equips Ranger `secondary_weapon` (store `RangerFlashBangGrenadeWeapon`)
     + upgrade tag on team rangers missing secondary
   - **TOW**: equips Humvee secondary + tag
   - **SupplyLines**: tags supply centers
4. Local-player complete queues `EVA_UpgradeComplete` audio residual.
5. Honesty flags (registry API; do **not** claim full science/ProductionUpdate parity):
   - `honesty_queue_ok(kind)` / `honesty_complete_ok(kind)` / `honesty_host_path_ok(kind)`
   - `honesty_capture_unlock_ok` / `honesty_flashbang_equipped_ok`
6. Save/load: `WorldSnapshot.host_upgrades` persists in-flight research (see section above).
7. Tests (not log-only):
   - `capture_building_upgrade_queue_complete_unlocks_capture_ability`
   - `flashbang_upgrade_queue_complete_equips_ranger_secondary`
   - `supply_lines_upgrade_queue_complete_tags_supply_center`
   - `host_upgrade_capture_mid_flight_save_load_completes_unlock`
   - `save_file_roundtrip_preserves_pending_host_upgrade`
   - module unit tests in `host_upgrades.rs`
   - existing `queued_upgrade_completes_during_simulation_update` still holds

**Still residual (fail-closed, not claimed):**
- Full retail Upgrade.ini BuildTime (30s) research timers / ProductionUpdate door UI
- Full science tree purchase / prerequisite graph / academy stats
- Full WeaponSetUpgrade / CommandSetUpgrade module matrices for every unit
- Object-type upgrades (`UPGRADE_TYPE_OBJECT`) vs player-type split beyond residual tags
- Network upgrade replication (network deferred)
- Economy effect of SupplyLines beyond tag observability

## Residual Host Playability — CaptureBuilding Action (2026-07-12)
**Closed (host-testable unlock → CaptureBuilding → ownership transfer):**
1. Gate: `team_has_completed_capture_upgrade` / hero path; infantry without Capture
   research cannot enter `AIState::Capturing` when a player exists for the team.
2. Command: `CommandType::CaptureBuilding` → `CommandExecutor::execute_capture_building`
   sets target + destination + `AIState::Capturing` (does **not** flip ownership).
3. Complete: `GameLogic::update_support_states` Capturing branch — when in range of a
   live enemy structure (not under construction), cancels old production, `set_team`
   to captor team, heals to max, radar "Building captured" residual, captor → Idle.
4. Walk residual: out-of-range Capturing keeps destination; `Object::stop_moving` no
   longer clobbers interaction AI states on arrival (only `Moving`/`AttackMoving` → Idle).
5. Combat isolation: `update_combat` skips fire/chase while in Capturing (and other
   interaction states). Capture sets `target` without being an attack order — without
   this, default-weapon chase rewrote Capturing → Attacking mid-walk.
6. Instant residual when in range (fail-closed vs C++ SpecialAbilityUpdate prep timer /
   capture progress bar / defect flash).
7. Tests (not log-only):
   - `capture_building_upgrade_queue_complete_unlocks_capture_ability` (includes ownership)
   - `capture_building_walk_into_range_transfers_ownership_after_upgrade`
   - `capturing_state_transfers_building_when_in_range`
   - `infantry_capture_requires_completed_capture_upgrade_when_player_exists`
   - breadth scenario capture asserts team transfer after `update`

**Still residual (fail-closed, not claimed):**
- Full C++ capture progress bar / SpecialAbilityUpdate packing / prep duration
- Object defection flash / undetected-defector helper timing
- Network capture replication (network deferred)
- Full ActionManager canCaptureBuilding edge matrix (stealthed, garrison, etc.)

## Residual Host Playability — Black Lotus Specials (2026-07-13)
**Closed (host-testable Lotus Capture / StealCash / DisableVehicle residual):**
1. Template gate: `is_black_lotus_template` (ChinaInfantryBlackLotus / TestBlackLotus / general variants).
2. **CaptureBuilding** without infantry Capture research (hero + Lotus residual); StartAbilityRange **150**;
   reuses Capturing ownership-transfer residual + honesty `building_captures`.
3. **StealCashHack**: only Lotus; cash-generator targets only (supply / black market / drop zone /
   TestBuilding residual); amount **1000**; range **150**; honesty `cash_steals` / `cash_stolen_total`.
4. **DisableVehicleHack**: only Lotus; enemy manned ground vehicle → DISABLED_HACKED for **900** frames;
   range **150**; honesty `vehicle_disables`.
5. Fail-closed: non-Lotus units cannot issue StealCash / DisableVehicle; non-cash structures reject steal.
6. Tests (not log-only):
   - `black_lotus_capture_building_without_upgrade`
   - `steal_cash_hack_command_transfers_cash_after_reach`
   - `steal_cash_hack_rejects_non_lotus_and_non_cash_targets`
   - `disable_vehicle_hack_command_disables_after_reach`
   - `disable_vehicle_hack_rejects_non_lotus`
   - host_hero_abilities module unit tests

**Still residual (fail-closed, not claimed):**
- Full SpecialAbilityUpdate Unpack/Pack/Prep timers + laser FX interleave
- CashHack science upgrade money matrix (2000 / 4000)
- One-at-a-time Lotus special busy matrix
- Network special replication (network deferred)

## Residual Host Playability — Campaign SinglePlayer Path (2026-07-12)
**Closed (host-testable campaign residual):**
1. `golden_campaign` / `golden_campaign_gate` — SinglePlayer start, CampaignManager
   start/complete, mission `victory_rule` override (`nounits` via
   `victory_rules_for_map`), logic frames advance, mission script counter ticks
   without panic.
2. Real campaign map **script decode + install**: MD_USA01 `load_map` /
   `initialize_scripts` → 291 scripts decoded; dense lists installed under budget
   + heavy-utility skip (`mission_scripts_installed_count` honesty).
3. Sample mission seeds use retail map identities (`MD_USA01`, `GC_ChemGeneral`)
   instead of placeholder `usa_mission_01`.
4. Wired into `release_candidate` (`campaign_runtime_ok`) and `behavior_gate`.
5. **Full retail map load hang fixed (2026-07-12 residual):**
   - Root cause: `CALL_SUBROUTINE` held `ScriptEngine` global `RwLock` write while
     nested flag/timer/subroutine paths re-acquired the same lock (MD_USA01
     `SUB-Generate Random Number [1-2]` → deadlock). TLS active-engine re-entry
     (`with_script_engine_mut` / `with_script_engine_ref`) + dense-script budget
     + decorative AI skip on large worlds.
   - Object spawn was already fine (~0.6s for 2846 placements / ~2429 spawned).
6. **Default retail campaign map load (2026-07-13 residual):**
   - Default residual prefers MD_*/GC_* `load_map` when assets resolve
     (`retail_campaign_map_loaded=true`).
   - Opt-out: `GEN_CAMPAIGN_HOST_SAFE=1` or `GEN_CAMPAIGN_FULL_LOAD=0` → Lone Eagle.
   - `GEN_CAMPAIGN_FULL_LOAD=1` remains explicit force (legacy).
7. **Mission objectives residual (2026-07-13):**
   - Path-stem match (`map_name_matches_mission` / `find_mission_for_map`) so
     `.../MD_USA01.map` resolves CampaignManager mission metadata.
   - Residual primary objectives seeded; honesty:
     `objectives_loaded`, `objective_count`, `objectives_from_campaign`.
8. Honesty flags:
   - `campaign_playable_claim=true` — SP path advances with scripts/victory (not full
     retail mission playthrough)
   - `retail_campaign_map_loaded` — true by default when retail MD_*/GC_* load
     succeeds; false under host-safe opt-out or missing assets
   - `mission_scripts_installed_count` / `objectives_from_campaign` — install +
     objective residual honesty (not full cinematic / score-screen claim)

**Still residual (fail-closed, not claimed):**
- Dense campaign script evaluation is budgeted (24/frame when ≥48 scripts), not
  full same-frame C++ parity for all 291 scripts
- End-to-end mission objective completion / cinematic / score-screen campaign flow
- Full Campaign.ini INI parse into Main `CampaignManager` (seeded USA MD_USA01–05 +
  CHALLENGE_0 residual table closed 2026-07-13 — see DropDelay Stagger + MoneyCrateCollide
  + Campaign.ini Table section; GameClient manager already loads INI)
- Live retail mission playthrough with all script actions / EVA / camera chains

## Residual Host Playability — Special Power Superweapon Host Path (2026-07-12)
**Closed (host-testable DoSpecialPower → queue → impact complete path):**
1. Host `HostSpecialPowerStrikeRegistry` on Main `GameLogic` queues real strikes for
   DaisyCutter / FuelAirBomb, A10 (`Airstrike`), ScudStorm, and ParticleCannon.
2. `CommandExecutor::execute_special_power` still consumes charge + `AIState::SpecialAbility`,
   then enqueues residual strikes with retail-ish impact delay frames (90 / 60 / 150 / 120).
3. `GameLogic::update_special_power_strikes` (logic update phase) applies two-stage area
   damage to host objects, skips friendlies, marks kills for destroy list, and records
   completion stats (`objects_hit`, `total_damage_applied`).
4. Activation + impact queue `AudioEventRequest`s; impact also registers a
   `DeathExplosion` combat particle residual entry.
5. Honesty flags (registry API; do **not** claim full retail superweapon parity):
   - `honesty_queue_ok(kind)` — strike pending after command
   - `honesty_complete_ok(kind)` / `honesty_host_path_ok(kind)` — impact resolved
6. Tests (not log-only):
   - `daisy_cutter_host_path_queues_and_completes_area_damage`
   - `a10_strike_host_path_queues_and_completes`
   - `scud_storm_host_path_queues_and_completes`
   - `particle_cannon_host_path_queues_and_completes`
   - `radar_scan_does_not_queue_superweapon_strike`
   - module unit tests in `special_power_strikes.rs` (map, falloff, friendly exclusion)

**Still residual (fail-closed, not claimed):**
- Full retail OCL aircraft spawn / flight / bomber AI for DaisyCutter and A10
- Multi-missile SCUD barrage timing; Particle Uplink full uplink sequence /
  outer-node lasers / swath driving (continuous beam residual closed above)
- SharedSyncedTimer / science / public timer UI / EVA superweapon ready lines on host path
- Weapon.ini / SpecialPower.ini damage tables beyond residual constants
- Network superweapon replication (network deferred)
- Non-superweapon special abilities beyond existing PendingSpecialAbility (hijack/etc.)
- *(Pending-strike save/load residual closed — see Special Power Strike Save/Load Xfer)*

## Residual Host Playability — Combat Particle Feedback (2026-07-12)
**Closed (host-testable kill/fire → particle registry observe path):**
1. Host `CombatParticleRegistry` on Main `GameLogic` registers real particle-system
   entries (stable id + template + position) on weapon fire and combat death.
2. Death path (`process_destroy_list`) spawns `MediumExplosion` + `SmokePlume`; fire path
   spawns `MuzzleFlash` + optional `BulletImpact`.
3. With `game_client`, entries mirror into `ParticleSystemManager` via combat presets
   (`create_preset_system_xyz`) so client registry is non-empty after kill/fire.
4. `PresentationFrame` freezes `particle_systems` and emits
   `ParticleSystemSpawned` / `ObjectDestroyed` events for client/HUD observation.
5. Tests (not log-only):
   - `combat_kill_spawns_particle_system_registry_entries`
   - `combat_fire_without_kill_still_spawns_muzzle_particle`
   - `presentation_frame_observes_combat_kill_particle_systems`
   - `create_preset_system_at_registers_combat_death_and_muzzle_entries`

**Still residual (fail-closed, not claimed):**
- Full W3D GPU particle render/compute parity (Main GPU ParticleSystemManager path)
- Full ParticleSystems.ini / FXList.ini retail coverage for every combat FX
- Bone-attached / slave systems / LOD culling for combat residual path
- Client GPU rebind of mirrored systems after save/load (host registry is restored)
- Network particle replication (network deferred)
- *(Host registry systems now survive WorldSnapshot — see Special Power Strike Save/Load Xfer)*

## Residual Host Playability — Combat March Honesty (2026-07-12)
- Map-world golden skirmish prefers pure `assign_unit_path` / Move march into weapon range,
  then `AttackObject`. Narrow `set_position` range pull remains only after per-focus stall.
- Honesty flags (do **not** gate `playable_claim` when map victory still works):
  - `combat_no_teleport_ok` — damage/kills without any combat `set_position` pull
  - `combat_realistic_speed_ok` — march speed ≤ retail BasicHumanLocomotor (20 u/s)
  - `combat_store_damage_ok` — no slice damage floor; WeaponStore/template damage (ranger ~5)

### Pathfinding / pure-march closure (2026-07-12)
**Root causes fixed:**
1. `GameLogic::update_movement` now **persists velocity** (was re-accelerating from 0 every
   frame → units crawled ~0.5 u/s). Also uses horizontal XZ distance for waypoints so
   terrain height does not false-stall path advance.
2. `assign_unit_path` kicks full speed toward goal; rejects absurd A* detours for direct march;
   path waypoints lerp Y start→goal.
3. Path grid: flatter slope mask (MAX_SLOPE 4.0) + auto-clear if >35% blocked; A* closed set;
   nearest-open goal when building footprints block the cell.
4. Golden fight: stable focus (no HashMap thrash), structure-prefer targeting, distance-scaled
   march budgets for 3.5k maps, AI paused during clear; longer windows + more rangers replace
   the old 80 u/s / damage-floor-40 assists.
5. GLA/faction structures marked `Attackable` so combat targeting is consistent.

**Lone Eagle gate (target / validated when green):**
`playable_claim=true`, `retail_prod=true`, `retail_gather=true`, **`combat_no_teleport_ok=true`**,
`combat_realistic_speed_ok=true`, `combat_store_damage_ok=true`, `victory=true`, `map_combat=true`.

**Residual (honest, non-blocking):**
- Host `locomotor_bootstrap` binds SET_NORMAL speeds at create_object (BasicHumanLocomotor ~20 u/s
  for USA_Ranger / GLA infantry; Humvee 60; tanks 30; Redguard 25) from Locomotor.ini on disk or
  seed. Golden slice lift is residual only when catalog bind is missing (still ≤ 20 →
  `combat_realistic_speed_ok`).
- Fail-closed: not full multi-locomotor set / surface-type matrix / SET_PANIC upgrades —
  single primary SET_NORMAL name → max_speed/accel/turn only.
- Reintroduce SLICE_MARCH_SPEED > 20 or SLICE_DAMAGE_FLOOR > 0 only if pure-march budgets fail;
  that would clear the matching honesty flag without flipping claim off.
- Full Weapon.ini / multi-locomotor parity and SAGE passability remain open for real-time play.
- `set_position` stall fallback code remains for pathological maps; currently unused on Lone Eagle
  when pure march succeeds.
- Network still deferred.

## Recent Validated Closures (2026-04-02)
- Terrain bridge runtime parity:
  - `hq-p58f` closed.
  - `TerrainLogic::addBridgeToLogic()` / `addLandmarkBridgeToLogic()` now register bridges with the pathfinder and assign the returned bridge layer.
  - `deleteBridge()` / `deleteBridgeAt()` now disable bridge state in the pathfinder and destroy the bridge object when present.
  - Validation: `cargo check -q -p gamelogic` plus targeted bridge tests passed.
- W3D renderer batch submission parity:
  - `hq-xo41` closed.
  - `W3DRenderer` now binds real mesh/material snapshots, vertex/index buffers, and indexed/non-indexed draws instead of the placeholder fullscreen triangle path.
  - Validation: `cargo check -q -p game_engine_device`, `cargo check -q -p game_engine`, and `cargo check -q -p game-client-rust --features internal` passed.
- Audio view-resolution parity:
  - `hq-dur9` closed.
  - `AudioViewResolver` is already wired through GameClient init and GameLogic resolver registration, and the new regression test proves it is consumed by `GameAudio::update()`.
  - Validation: `cargo test -q -p game_engine --features internal --test audio_view_resolver_tests -- --exact --test-threads=1` passed.
- Audio suspend/resume parity:
  - `hq-dxxk` closed.
  - Script suspend/resume now pauses and resumes active handles instead of only toggling audio enable state.
- Host combat audio residual (fire/death request path):
  - Weapon fire queues `AudioEventRequest("WeaponFire")`; death queues `UnitDie`/`BuildingDie`.
  - FXList SoundFX falls back to gameplay dispatch when the client audio hook is absent.
  - Validation: `cargo test -q -p generals_main --lib combat_fire_queues` and `combat_kill_queues` passed; FXList fallback: `cargo test -q -p game-client-rust --features internal --lib sound_fx_nugget`
- Shell/flow parity:
  - `hq-dwel`, `hq-c8a0`, `hq-b53m`, and `hq-afm` are closed.
  - That means map-select legacy keying, underlying-options visibility, shell scheme ordering, and command translator context routing are all improved on the gameplay-flow side.
- Terrain parity slices already closed earlier in this session remain validated:
  - dynamic water dedupe and authoritative z updates
  - wall-hit source parity
  - identity-stable water-handle mapping follow-up work

## Current Subsystem Status
- Rendering: improved, but not complete. Real mesh/material submission is now in place, yet the deeper W3D/material/state parity backlog still exists.
- Terrain: improved materially. Bridge runtime and water-related slices are closer to C++ behavior, but the broader terrain visuals/water/snow work remains open.
- Audio: improved materially. Core resolver, suspend/resume, and host combat fire/death audio requests are in place; broader Miles device/handle and per-weapon INI FireSound residual remain open.
- UI/flow: better than before, especially shell and map-select behavior, but bootstrap/menu/game-start parity still has open work.
- AI: still a major blocker. The system remains far from the breadth of the C++ implementation.
- Drawable: still a major blocker. Icon management, model conditions, shadows, and animation-state parity are incomplete.
- Particles: still a major blocker. The system remains heavily placeholder-based.
- Save/load: improved in multiple slices, but still not a clean full parity story for all gameplay objects.
- Memory: still a structural concern because the Rust allocation model intentionally diverges from the C++ pool allocator behavior.

## What Is Still Blocking Playability
1. `hq-eo4` and related startup/bootstrap flow work:
   - Common startup still needs to consistently drive the real GameClient bootstrap path under the default game flow.
2. `hq-81ok` AI:
   - Skirmish AI is still the largest single gameplay blocker.
3. `hq-fos8` Drawable:
   - Missing visual state, animation, shadow, and icon parity still prevents full visual playability.
4. `hq-gq7n` Particles:
   - Explosions, smoke, fire, and other feedback effects are still incomplete.
5. `hq-zhvn` GameMemory:
   - The memory model still diverges from the C++ pool allocator semantics.
6. `hq-7zxm` Audio:
   - Core audio is improving, but the broader system still has placeholder and routing gaps.
7. `hq-aqxu` Terrain visuals:
   - Terrain rendering, water animation, snow, deformation, and pathfinding integration remain incomplete.

## Recommended Next Order
1. Finish bootstrap/startup parity (`hq-eo4`) so the real client path is the default.
2. Push AI parity (`hq-81ok`) enough to make skirmish gameplay viable.
3. Continue Drawable parity (`hq-fos8`) because it gates visible gameplay feedback.
4. Fill the particle system gaps (`hq-gq7n`) after Drawable is stable.
5. Reduce the memory-model divergence (`hq-zhvn`) to lower risk across save/load and runtime behavior.
6. Continue terrain/audio edge cases in `hq-aqxu` and `hq-7zxm`.

## Earlier Validated Closures
- Shared W3D gadget text/clip parity improved (2026-03-12):
  - `GameClient/src/gui/w3d_gadget_draw.rs` now matches more of the original shared gadget behavior instead of carrying Rust-specific text state across draws.
  - `W3DGadgetStaticText*` now applies hotkey highlighting when `WIN_STATUS_HOTKEY_TEXT` is set, clips to the full window rect during draw, and clears the clip state afterward.
  - text-entry and list-box text paths now clear their temporary `DisplayString` clip regions after each draw instead of leaking clipping into later controls.
  - checkbox image draw now honors `image_offset.y`, matching the C++ checkbox image path.
  - image-backed radio buttons now use the C++-style left/capped/tiled-center strip behavior instead of drawing only a single image across the whole control.
  - image-backed list-box hilite bars now clip the repeated center/tail pieces instead of stretching the tail segment into the remaining gap.
- Main-menu button drop-shadow text parity improved (2026-03-12):
  - `GameClient/src/gui/w3d_gadget_draw.rs::w3d_main_menu_button_drop_shadow_draw(...)` no longer routes through the generic button-text helper.
  - Rust now uses a dedicated main-menu button text path with the C++-style centered label layout and visible drop-shadow offset used by `W3DMainMenuButtonDropShadowDraw`.
  - This matters for the runtime-overridden shell buttons installed by `W3DMainMenuInit`, where generic Rust text rendering was still flatter than the original menu presentation.
- Main-menu random text draw parity improved (2026-03-12):
  - `GameClient/src/gui/w3d_gadget_draw.rs::w3d_main_menu_random_text_draw(...)` now uses the dedicated C++-style path instead of the generic static-text helper.
  - The Rust draw now left-aligns at the window origin, vertically centers the text, and applies the same clipped disabled-text rendering behavior as the original `W3DMainMenuRandomTextDraw`.
  - This matters for main-menu runtime labels in data sets that still include the random-text windows C++ configures during `W3DMainMenuInit`.
- Main-menu runtime draw override parity improved (2026-03-12):
  - `GameClient/src/gui/window_manager.rs` now applies the extra `W3DMainMenuInit` runtime draw-callback overrides that C++ installs after loading `MainMenu.wnd`.
  - This restores `W3DMainMenuButtonDropShadowDraw` on the main shell buttons and `W3DMainMenuRandomTextDraw` on the optional random-text labels when present.
  - The `.wnd` file alone leaves those windows on generic `[None]` draw callbacks, so without the runtime override Rust was missing part of the authored main-menu presentation even when assets and transitions were otherwise correct.
- Local filesystem lookup parity improved (2026-03-11):
  - `Common/src/common/system/local_file_system.rs` now resolves file paths case-insensitively across direct paths and configured search roots.
  - directory enumeration through the same backend now also resolves search directories case-insensitively before scanning them.
  - This better matches the effective C++ asset behavior on shipped data, where many extracted files are lowercased on disk while gameplay/UI code still requests mixed-case legacy names.
  - The fix is broader than shell art and should reduce repeated missing-asset drift across UI, INI, and other local filesystem-backed lookups.
- Shell startup asset lookup parity improved (2026-03-11):
  - `GameClient/src/display/image.rs` now resolves local fallback texture paths case-insensitively before opening them.
  - This closes a real repo/platform gap where extracted assets are often lowercased on disk, while mapped-image filenames preserve original mixed case from the C++ data set.
  - That matters directly for shell art such as `MainMenuRuleruserinterface.tga` and other extracted texture lookups on case-sensitive filesystems.
- Main-menu ruler visibility parity improved (2026-03-11):
  - `GameClient/src/gui/shell/main_menu.rs::sync_cpp_startup_visibility(...)` no longer re-hides `MainMenu.wnd:MainMenuRuler` every update based on `not_shown`.
  - The C++ code decides ruler visibility during `MainMenuInit()` and does not override it in the startup visibility sync.
  - This removes one more Rust-only suppression of the intro/main-menu composition, especially after returning to the main menu within the same process.
- Shell generic image draw parity re-closed (2026-03-11):
  - `GameClient/src/gui/game_window.rs::default_draw_callback(...)` now renders `WIN_STATUS_IMAGE` windows with neutral color instead of multiplying the mapped image by the `.wnd` draw color.
  - This is the exact C++ `W3DGameWinDefaultDraw` behavior for generic image-backed windows and matters directly for `MainMenu.wnd:MainMenuParent`, `MainMenu.wnd:MainMenuRuler`, and `MainMenu.wnd:Logo`.
  - The prior Rust tint path could still darken valid menu art into blue/black even when texture resolution succeeded.
- Shell default image-window parity improved:
  - `GameClient/src/gui/game_window.rs::default_draw_callback(...)` now mirrors C++ `W3DGameWinDefaultDraw` semantics for `WIN_STATUS_IMAGE` windows.
  - Image-backed windows now draw only the mapped image with neutral color in the default path; Rust no longer fills the configured color first, multiplies the image by that color, and adds a border on top.
  - This specifically removes a real shell/menu drift affecting `MainMenu.wnd:MainMenuParent`, `MainMenu.wnd:Logo`, and `MainMenu.wnd:MainMenuRuler`, where valid art could still appear darkened or blackened because Rust treated `COLOR` as image tint in the image path.
- Shell legacy window draw parity improved:
  - `GameClient/src/gui/window_manager.rs` now assigns the correct default draw path for windows with empty / `[None]` draw callbacks.
  - Generic `USER` windows no longer fall through to push-button draw semantics merely because they carry image draw data.
  - Fresh rebuilt shell runs now show:
    - `GeneralsLogo` hydrating and GPU-loading from `SCSmShellUserInterface512_001.tga`
    - `MainMenuRuler` hydrating and GPU-loading from `MainMenuRuleruserinterface.tga`
  - This directly closes a live shell/menu regression where those windows existed in the runtime tree but were not painting through the same generic window path as C++.
- Shell callback image lookup parity improved:
  - `GameClient/src/gui/game_window_global.rs::win_find_image(...)` now lazily hydrates mapped images from the common collection before callback-driven lookup.
  - This aligns callback-driven shell draw paths more closely with C++ and removes another dependency on fragile startup bulk-sync timing.
- Shell backdrop asset status is now proven, not guessed:
  - direct BIG-TOC inspection confirms the currently mounted asset set contains:
    - `TitleScreenuserinterface.tga`
    - `MainMenuRuleruserinterface.tga`
    - `SCShellUserInterface512_001.tga`
    - `SCSmShellUserInterface512_001.tga`
  - and does **not** contain `MainMenuBackdropuserinterface.tga`
  - so the current `MainMenuBackdrop -> TitleScreenuserinterface.tga` compatibility fallback is covering a real asset absence in this repo data set rather than a broken Rust archive lookup.
- Startup event-loop parity improved:
  - `Main/src/main.rs`, `.../win_main.rs`, and `.../cnc_game_engine.rs` now create the real `winit` window inside the active event loop/resume path instead of before the handler exists.
  - Fresh rebuilt shell runs no longer emit the early macOS `winit` startup errors `tried to run event handler, but no handler was set`.
- Shell ambient/scorch marker parity improved:
  - `Main/src/game_logic/game_logic.rs` now applies the original C++ illegal map-template skip list before template/fallback synthesis.
  - Fresh rebuilt shell runs no longer emit `Skipping unsupported decorative map object template 'Amb_*'` or `'Scorch'`.
- Animated-sound metadata parity improved:
  - `Common/src/common/game_engine.rs` now resolves `w3danimsound.ini` through the mounted virtual file system first and only falls back to direct path/archive probing when necessary.
  - `ww3d-animation/src/animated_sound.rs` now matches the original C++ default name semantics (`w3danimsound.ini`) instead of hardcoding `Data/INI/...`.
  - animated sound metadata can now initialize directly from mounted INI bytes instead of writing a temporary compatibility file just to read it back.
- Shell callback/object-definition parity improved:
  - `Main/src/assets/ww3d_asset_manager.rs` now includes stock top-level `Data/INI/Crate.ini` in WW3D object-definition discovery, so shell/runtime crate objects like `2FreeCrusadersCrate` resolve from real stock INI definitions instead of synthetic fallback templates.
  - `GameClient/src/gui/window_manager.rs` now wires `W3DShellMenuSchemeDraw` and `W3DClockDraw`.
  - `GameClient/src/gui/w3d_gadget_draw.rs` now implements both draw callbacks instead of falling back to generic/default draw.
  - `GameClient/src/gui/shell/base.rs` shell menu scheme draw now renders its configured lines/images through the live window manager instead of being a trace-only stub.
- Shell startup/menu presentation parity improved:
  - `GameClient/src/gui/game_window.rs` and `.../w3d_gadget_draw.rs` no longer draw a fallback solid color when a mapped image exists but its GPU texture upload fails.
  - This removes a real Rust-only behavior drift that could black-fill `MainMenu.wnd:MainMenuParent` when `MainMenuBackdrop` failed to materialize.
- Shell startup prewarm no longer blocks first menu frame:
  - `Main/src/cnc_game_engine.rs` now queues nearby shell-scene models for incremental post-startup prewarm instead of blocking startup on synchronous shell-scene model loads.
  - Fresh bounded startup runs now reach the legacy shell overlay before shell-scene prewarm work resumes.
- Common/BIG shell asset diagnostics improved:
  - `Common/src/common/system/big_file_system.rs` now exposes mounted virtual paths for parity/debug tests.
  - `Common/tests/mapped_image_parity_tests.rs` now verifies mounted shell image candidates and mapped-image metadata against real repo assets.
- Shell startup legacy-runtime deadlock resolved:
  - `Main/src/game_logic/game_logic.rs` now releases `PlayerTemplateStore` read-locks before `player.init(...)`, allowing lazy template hydration without lock inversion.
- Player template parity/availability improved:
  - `Common/src/common/ini/mod.rs` now supports robust `PlayerTemplate.ini` discovery across base, extracted, and mod roots.
  - `GameLogic/src/player.rs` now lazily ensures template-store population during runtime hydration.
  - Shell startup no longer emits `PlayerTemplate '...' not found in store` warnings in the active validated run.
- Script/object shell parity improved:
  - `GameLogic/src/scripting/executor.rs` team/named follow-waypoint actions now resolve path starts by closest waypoint-to-position (C++ shape) rather than strict name lookup.
  - `Main/src/game_logic/script_loader.rs` + `.../game_logic.rs` now mirror parsed named object placements into legacy runtime tracking for script resolution.
- Factory/bootstrap parity improved:
  - `GameLogic/src/helpers.rs` now retries `ThingFactory` init only when uninitialized, eliminating repeated heavy re-inits on normal misses.
  - Common ThingFactory now loads object declarations from base/extracted/mod INI roots.
  - ModuleFactory install paths now perform on-demand initialization retries for behavior/draw/client-update modules.
- Current shell startup reachability (validated):
  - `Fast legacy runtime sync complete ... elapsed=0.02s`
  - `Mission script runtime registered 82 WW3D scripts`
  - `shell_initialized active=true screens=1`
  - `menu_overlay_paint_jobs=5`
  - `MainMenu.wnd:MainMenuParent` and child shell windows are present in the legacy runtime overlay queue.

## Current Highest-Value Todo
- `GameClient/src/display/image.rs` + shell mapped-image path
  - close the remaining `MainMenuBackdrop` materialization gap. The mapped image exists and resolves to `MainMenuBackdropuserinterface.tga`, but the actual GPU texture payload is still missing in active mounted content and needs the same mounted-file/BIG-backed behavior the original engine expects.
- `GameEngineDevice/src/w3d/w3d_c_api.rs`
  - finish the remaining true multi-texture / texture-dependent fixed-function combiner parity beyond the current constrained fallback evaluator, now that lighting-disable and material-source state are wired through the active material/shader path.
- `GameEngineDevice/src/w3d/performance_optimizer.rs`
  - close the remaining exact heuristic threshold/tuning parity against the original C++ runtime policy now that optimizer batching preserves renderer-specialized priority/material state instead of collapsing it.
- `GameEngineDevice/src/w3d/renderer.rs`
  - close the remaining deeper render-pass/material-state specialization parity beyond the current default/unlit path and optimizer queue-state preservation.
- `GameClient/src/video_player.rs` + device-side movie backend
  - wire a real active stream provider/decoder backend behind the common player hook.
- `Common/src/common/audio/*.rs` + `GameLogic/src/helpers.rs`
  - close broader audio routing side-effect parity and malformed/streaming media edge behavior.

## What Is Good
- Core gameplay/runtime crates compile:
  - `cargo check -p game_engine`
  - `cargo check -p gamelogic`
  - `cargo check -p game-client-rust`
  - `cargo check -p game_engine_device`
- Video-device parity lane is build-clean:
  - `cargo check -p game_engine_device --features video`
- W3D-device parity lane is build-clean:
  - `cargo check -p game_engine_device --features w3d`
- W3D texture mip generation parity improved:
  - `Code/GameEngine/GameEngineDevice/src/w3d/texture_manager.rs` now generates/uploads real mip chains via iterative RGBA downsample instead of leaving mip levels as a simplified placeholder path.
- Shadow renderer caster accounting parity improved:
  - `ww3d-renderer-3d/rendering/shadow_system/shadow_renderer.rs` now uses registered `ShadowCasterSubmission` lists as the primary per-pass caster accounting source.
  - `shadow_caster_count_hint` is now fallback-only when no runtime submissions are registered.
  - Added per-light submission integration path (`render_shadows_with_submissions(...)`) so directional/point/spot passes can consume explicit per-light caster sets directly.
  - Added persistent per-light submission registry in `ShadowRenderer`; default `render_shadows(...)` now consumes per-light registered submissions without requiring external map passing each frame.
- Renderer frame-graph shadow submission bridge improved:
  - `ww3d-renderer-3d/src/lib.rs` now queues eligible meshes into the frame-graph shadow-caster queue and materializes runtime `ShadowCasterSubmission` records each frame from prepared shadow-caster meshes.
  - Added runtime accessors (`shadow_caster_submissions`, `take_pending_shadow_caster_submissions`) for direct handoff into shadow passes.
- WGPU main-renderer shadow handoff improved:
  - `ww3d-renderer-3d/rendering/wgpu_main_renderer.rs` now auto-drains renderer-emitted shadow submissions during `end_frame(...)` (engine and legacy paths) instead of requiring external polling-only glue.
  - Per-frame submission state is now exposed directly from `WgpuMainRenderer` through `shadow_caster_submissions()` and `shadow_caster_count_hint()`.
- Main forward-pass transparent routing improved:
  - `Main/src/graphics/render_pipeline.rs` now classifies render items by material blend/opacity into `ForwardOpaque` vs `ForwardTransparent` instead of hard-forcing all items to opaque.
  - Transparent items are now sorted back-to-front and included in `ForwardPass::render(...)` queue submission (previously only opaque items were submitted).
- Main effects shadow callback integration improved:
  - `Main/src/effects/lighting_system.rs` now provides `render_shadow_maps_with_context(...)` (per-light/per-layer callback context) and upgrades `render_shadow_maps(...)` to `FnMut` callback flow.
  - `Main/src/effects/integration.rs` now exposes `render_with_shadow_scene(...)`, so the active effects pipeline can accept real shadow-caster draw callbacks instead of embedding a fixed no-op-only callback shape.
  - `Main/src/effects/integration.rs` now also exposes `render_with_shadow_scene_context(...)` for direct per-light/per-layer callback wiring at effects call-sites.
  - Shadow-layer assignment in dynamic lighting now uses deterministic light-id ordering instead of direct `HashMap` iteration order, improving frame-to-frame stability for multi-light shadow array usage.
  - Shadow-pass callback now owns pipeline/bind-state setup (internal pre-bind removed), enabling callback-driven scene draws to use correct mesh-compatible render state.
- Main render-item ordering determinism improved:
  - `Main/src/graphics/render_pipeline.rs` now sorts object-id iteration during item collection and includes explicit comparator tie-breakers (`object_id`, `model_name`, `mesh_index`) after pass/material/distance keys.
  - This removes remaining equal-key ordering drift from hash-map insertion order.
- GameLogic test-build lane is unblocked again:
  - `GameLogic/src/common/types.rs` test module now imports `CoordOrigin`, restoring `Coord3D::origin()` resolution in lib-test compilation.
  - `cargo test -p gamelogic --no-run` now completes successfully.
- Dynamic-lighting shader shadow path improved:
  - `Code/Main/src/shaders/dynamic_lighting.wgsl` now performs cascade-aware depth-array shadow sampling with PCF in `sample_shadow_map(...)` instead of fixed full-light placeholder behavior.
  - Directional-light path now uses sampled shadow factor, reducing obvious sun-shadow parity drift.
- Shadow map CPU query parity improved:
  - `ww3d-renderer-3d/rendering/shadow_system/shadow_map.rs::is_point_in_shadow(...)` no longer returns a fixed constant.
  - CPU fallback now applies bias-aware/filter-aware visibility approximation, reducing hardcoded shadow-value drift.
- W3D optimizer runtime policy lane is now active instead of inert:
  - `Code/GameEngine/GameEngineDevice/src/w3d/performance_optimizer.rs` now records frame-history/memory telemetry and applies periodic auto-quality adjustment (FPS + memory pressure) to LOD bias, culling distance, and batching aggressiveness.
  - LOD runtime metrics (`average_lod_level`, `lod_transitions`) are now populated from active camera/object distances.
  - Instancing/batching now preserves transparent-vs-opaque separation (`RenderBatch.transparent`) instead of forcing opaque output.
- W3D renderer transparent queue parity improved:
  - `Code/GameEngine/GameEngineDevice/src/w3d/renderer.rs` now routes transparent materials into `transparent_queue` (instead of always `opaque_queue`) and submits both queues each frame.
  - Transparent queue sorting now uses back-to-front camera-distance ordering with per-batch distance computed from mesh bounds and active camera.
- W3D renderer object-transform submission parity improved:
  - `Code/GameEngine/GameEngineDevice/src/w3d/w3d_device.rs::render_scene(...)` now forwards per-object world transforms into renderer submission instead of effectively forcing identity transforms.
  - `Code/GameEngine/GameEngineDevice/src/w3d/renderer.rs` now uses model-matrix inverse-transpose normal matrices in submitted instance data for this path.
  - Direct scene-render submissions now also honor `RenderObject.visible` and explicit `RenderObject.transparent` override state.
- W3D init entrypoint parity is improved:
  - `GameEngineDevice::init_w3d_device(...)` / `init_video_device(...)` now call device `.init()`,
  - `W3DDevice::init_with_window(&Window)` now configures a real window-bound surface.
- Runtime audio device enumeration now uses driver-backed capabilities:
  - `GameEngineDevice::audio::DeviceManager::enumerate_devices()` no longer returns mock fixed entries.
- macOS platform CPU usage reporting parity improved:
  - `GameEngineDevice/src/platform/macos_device.rs::get_cpu_usage()` now reports live process CPU usage (via `ps`) instead of fixed zero, mapped to engine metric range.
- Miles audio capability reporting now derives from runtime backend data:
  - `MilesAudioDevice` capability snapshots are now mapped from active Kira backend capabilities in `audio` builds.
- Basic audio playback lifecycle parity improved:
  - `Common/src/common/basic_audio_manager.rs` now tracks non-looping sound lifetime by elapsed playback time vs estimated duration (WAV parsed duration when available + bitrate fallback), replacing prior handle-id retirement behavior.
  - Master-volume updates now recompute from source base volumes, removing compounding volume drift across repeated master-volume changes.
- Radius-decal icon visibility/throb parity improved:
  - `GameLogic/src/common/types.rs` `RadiusDecal::update()` now follows C++ icon UI semantics by forcing decal opacity to zero when draw-icon UI is disabled.
  - Throb interpolation now normalizes min/max opacity ordering and treats zero throb-time as stable max-opacity output.
- Duration parsing parity improved in active decal/camera-delivery lanes:
  - `NeutronMissileUpdate`, `SpectreGunshipUpdate`, `DynamicShroudClearingRangeUpdate`, `WeaponBonusUpdate`, `StructureToppleUpdate`, `StructureCollapseUpdate`, and `HordeUpdate` now use canonical `INI::parse_duration_unsigned_int(...)` for frame-duration fields instead of ad-hoc conversion logic.
- Additional C++-mapped duration parser parity closures landed in active gameplay modules:
  - `CountermeasuresBehavior`, `BattleBusSlowDeathBehavior`, `W3DLaserDraw`, `BeaconClientUpdate`, and `SpecialPowerTemplate` duration fields now use canonical `INI::parse_duration_unsigned_int(...)`.
  - `WaveGuideUpdate::WaveDelay` now uses canonical `INI::parse_duration_real(...)`.
  - This removes remaining ad-hoc `ms/s` parsing drift in these paths and aligns with C++ `parseDurationUnsignedInt` / `parseDurationReal` field mappings.
- Additional duration parser parity closures landed across behavior/update/contain paths:
  - `AutoDepositUpdate`, `AutoFindHealingUpdate`, `EnemyNearUpdate`, `CommandButtonHuntUpdate`, and `DumbProjectileBehavior` duration fields now use canonical `INI::parse_duration_unsigned_int(...)`.
  - `RebuildHoleBehavior` and shared contain duration helper paths now use canonical `INI::parse_duration_real(...)` (with truncation-faithful frame-counter handoff for respawn wait semantics).
  - This removes another set of legacy ad-hoc conversions and aligns these modules with C++ `parseDurationUnsignedInt` / `parseDurationReal` mapping behavior.
- Additional legacy behavior/contain duration parser closures landed:
  - `SlowDeathBehavior`, `SpawnBehavior`, and `OpenContain::DoorOpenTime` now use canonical `INI::parse_duration_unsigned_int(...)` in place of manual digit-only conversion paths.
  - This removes suffix-handling drift in these lanes and aligns with C++ `parseDurationUnsignedInt` field mappings.
- Additional duration-real parser closures landed in AI/dock lanes:
  - `WorkerAIUpdate::BoredTime`, `DozerAIUpdate::BoredTime`, and `RepairDockUpdate::TimeForFullHeal` now use canonical `INI::parse_duration_real(...)`.
  - This removes real-duration suffix drift in these paths and aligns with C++ `parseDurationReal` mappings.
- Main gameplay shell/template parity improved:
  - `isInShellGame()` now reflects real in-game state,
  - team template availability now avoids obvious cross-faction leakage for faction-tagged template names.
- Main gameplay fallback AI parity improved:
  - `process_ai_behavior(...)` fallback branches now use `AIDecisionSystem` enemy scan/decision flow (attack/retreat/retarget) instead of explicit placeholder/random behavior.
  - Patrol fallback destination changes now use deterministic frame/object-id keyed movement for better replay consistency.
- Game initialization script parity improved:
  - `Code/GameEngine/GameLogic/src/system/game_initialization.rs` now synchronizes `SidesList` script lists into `ScriptEngine` during startup, so side scripts loaded from map/side data are actually registered before runtime script updates.
  - Startup script extraction from world dict now supports multiple key variants and list-form values instead of forcing a synthetic default script filename.
- Startup sequence script behavior parity improved:
  - `Code/GameEngine/GameLogic/src/system/game_start.rs::run_startup_scripts(...)` now activates matching runtime scripts through `ScriptEngine` instead of no-op success.
- Single-instance runtime behavior parity improved:
  - `create_generals_mutex()` now retains the acquired `SingleInstanceGuard` for process lifetime via a global guard slot, instead of dropping it immediately after acquisition.
  - Unix process liveness checks now treat `kill(pid, 0)` permission-denied (`EPERM`) as alive, reducing stale-lock false cleanup when a running process cannot be signaled.
- FOW/shroud visibility query parity improved:
  - `GameLogic::get_visible_objects(...)` and `get_visual_object_info(...)` now use shroud visible/explored object sets (when runtime shroud state is active) instead of effectively treating all non-stealthed objects as visible.
- Player relationship initialization parity improved:
  - `GameLogic/src/system/player_init.rs::PlayerList::init_team_alliances()` now applies template allies/enemies directives in map-init paths (instead of forcing enemy defaults), and preserves observer neutrality.
  - `GameLogic/src/system/player_init.rs::init_relationships_from_allies_enemies()` now also enforces observer-neutral relationships for direct initialization callers.
- Command targeting parity improved:
  - `InputCommandProcessor::find_object_at_position(...)` now uses context-aware click picking (selected-unit command context vs raw selection context), prioritizing enemy attackable targets during issued commands.
- Input projection parity improved:
  - `InputCommandProcessor` now updates screen-to-world mapping from runtime viewport resize events and current world bounds instead of fixed `800x600` assumptions.
- Locomotor terrain-height parity improved:
  - `GameLogic/src/locomotor/core.rs::Locomotor::get_terrain_height(...)` now samples runtime terrain layer height for non-air locomotors instead of preserving stale current Z.
- Thrust locomotor helper parity improved:
  - `GameLogic/src/locomotor_impl.rs::calc_direction_to_apply_thrust(...)` now uses gravity-adjusted quadratic thrust-direction solving with C++-style fallback behavior.
  - `GameLogic/src/locomotor_impl.rs::move_towards_position_thrust(...)` now applies `max_thrust_angle` via constrained vector rotation instead of using the previous simplified direct-thrust branch.
- GrantStealth behavior parity improved:
  - `GameLogic/src/object/behavior/grant_stealth_behavior.rs` now performs C++-style final scan shutdown by destroying the grantor object at max radius instead of periodically rescanning forever.
  - `RadiusParticleSystemName` is now parsed and applied with proper create/destroy lifecycle handling.
  - Stealth grant visual feedback now flashes recipient drawables as selected instead of skipping this lane.
  - Grant scans now honor ally relationships (`Friend/Ally/Allies`) and include contain rider forwarding via `friend_get_rider`.
- QueueProductionExitUpdate parity improved:
  - `GameLogic/src/object/behavior/queue_production_exit_behavior.rs` now performs C++-style airborne creation checks and applies terrain snap when `AllowAirborneCreation = No`.
  - Door-exit path now propagates owner layer, registers spawned units with AI pathfinder, and issues `ai_follow_exit_production_path(...)` with natural-rally sequencing (including doubled-natural-rally anti-stacking fallback).
  - Airborne producer exits now apply inherited motive force from producer velocity, reducing spawn drift for in-air production.
  - Budding exit now supports C++ no-host fallback to producer position/orientation (instead of failing), copies host layer when host exists, and issues post-spawn `ai_move_to_position(...)`.
  - Queue exit update now keeps `UPDATE_SLEEP_NONE` semantics and save/load now includes queue runtime state (`current_delay`, rally point state, burst count, creation clear distance).
- SupplyCenterProductionExitUpdate parity improved:
  - `GameLogic/src/object/behavior/supply_center_production_exit_behavior.rs` now uses `ai_follow_exit_production_path(...)` for spawned truck exit handoff (matching C++ path-command behavior instead of movement-target-only assignment).
  - Supply truck force-wanting activation (`SupplyTruckAIInterface::set_force_wanting_state(true)`) is now aligned with this exit-path handoff lane.
  - `GrantTemporaryStealth` runtime gating now matches C++ conditions (owner stealthed + target temporary-grant or no `CAN_STEALTH` capability).
  - `GrantTemporaryStealth` INI now parses duration values (`parse_duration_unsigned_int`) rather than raw integers.
  - Save/load now carries supply-center rally-point runtime state through behavior snapshot/xfer (`m_rallyPoint`, `m_rallyPointExists`).
- SpawnPointProductionExitUpdate parity improved:
  - `GameLogic/src/object/behavior/spawn_point_production_exit_behavior.rs` now registers spawned units with the AI pathfinder map after spawn-point placement, matching C++ spawn activation behavior.
  - Door reservation now performs C++-style lazy spawn-bone initialization and occupier revalidation before availability checks.
  - Save/load now preserves spawn-point occupier state (`m_spawnPointOccupier[MAX_SPAWN_POINTS]`) through behavior snapshot/xfer instead of losing occupancy ownership across restore.
  - `SpawnPointBoneName` parse path now ignores `=` tokens in field token streams, improving parity with C++ INI field parsing behavior.
- DefaultProductionExitUpdate save/load parity improved:
  - `GameLogic/src/object/behavior/default_production_exit_behavior.rs` now snapshots/restores runtime rally state (`m_rallyPoint`, `m_rallyPointExists`) at behavior level, matching C++ xfer coverage.
  - Module snapshot paths now route through behavior state, eliminating the prior module-data-only save/load drift for this exit behavior.
  - `use_spawn_rally_point()` is now exposed directly from module data to support downstream parity hooks.
- Terrain water shoreline parity improved:
  - `GameClient/src/terrain/water.rs` now generates shoreline blend geometry for rectangle/circle/polygon/path/spline water segments instead of leaving `shore_geometry` empty.
  - Shore bands now include inner/outer ring vertices and triangle strips with outer alpha fade, enabling blend-friendly terrain-water transition rendering.
  - Water flow updates now also propagate per-vertex flow vectors into generated shoreline geometry.
- Minimap terrain/FOW parity improved:
  - `Main/src/graphics/minimap_renderer.rs` now supports composed minimap textures (terrain base + FOW darkening) instead of FOW-only grayscale output when terrain data is available.
  - Minimap mapping now clamps and guards against degenerate spans for world<->minimap conversions, improving click/projection stability at map bounds and during resize.
  - Minimap refresh now forces immediate upload after base/world/screen updates, reducing stale minimap frames after map/world changes.
- Minimap visibility/readability parity improved:
  - `Main/src/game_logic/game_logic.rs::update_ui_state(...)` now applies shroud-aware minimap visibility filtering for object dots (instead of unconditional all-object dots).
  - Minimap now keeps explored structures for continuity while requiring live visibility for non-structure opponent dots.
  - `Main/src/graphics/minimap_renderer.rs` now softens FOW transition edges to reduce hard Hidden/Explored/Visible banding artifacts.
- Minimap terrain-base generation parity improved:
  - `Main/src/graphics/render_pipeline.rs` now generates minimap terrain base from live terrain-height sampling and injects it into minimap rendering on initialization/world-bounds changes.
  - Base pass now includes elevation gradient shading, light embossing, and low-elevation water tinting for more faithful minimap readability.
- Minimap static-road overlay parity improved:
  - `GameClient/src/terrain/roads.rs` now exports minimap road samples with road-type tint metadata (`RoadMinimapSample::tint_rgb`).
  - `Main/src/graphics/render_pipeline.rs` now draws road overlays in the minimap terrain base pass using road-type-aware tints (instead of one hardcoded road color) and width-scaled blend/radius.
- Radar terrain-texture parity improved:
  - `Common/src/common/system/radar.rs::refresh_terrain()` now uses sampled terrain cell data (height + water flags) to generate radar terrain texture shading instead of a single flat terrain color.
  - Radar terrain texture now includes elevation-driven tinting, local slope shading, and water-cell blue/depth tint modulation when full radar-resolution terrain samples are available.
- Radar coordinate mapping parity improved:
  - `Common/src/common/system/radar.rs` world<->radar conversion now respects non-zero map origins (`map_extent.lo`) instead of assuming origin at `(0,0)`.
  - `radar_to_world(...)` now returns sampled terrain-cell height when available, reducing average-height Z drift in radar-to-world jumps.
- Terrain roads normal parity improved:
  - `GameClient/src/terrain/roads.rs` now supports terrain-sampled normal projection (`apply_terrain_normals`) for road surface/edge/marking geometry.
  - `GameClient/src/terrain/terrain_visual.rs` now applies live heightmap normals to roads during update, replacing tangent-only normal drift.
- Stealth/FOW parity improved:
  - shroud-driven visibility snapshots now apply `can_see_object_with_stealth(...)` filtering for currently visible objects.
- Drag-selection parity improved:
  - drag world bounds are now propagated through `MouseCommandContext`, and area selection now uses live object transform positions (`get_position`) instead of stale cached `position` fields.
- Main engine cursor mapping parity improved:
  - `CnCGameEngine::update_mouse_world_position()` now maps viewport coordinates into current world bounds (map-size aware), replacing fixed-span projection constants.
- Main texture decode parity improved:
  - `Code/Main/src/assets/textures.rs` now performs real DXT3/DXT5 decompression (BC2/BC3 alpha+color block decode) instead of solid-color placeholder output.
  - DXT block decode output now writes in row-major destination order (fixed block-append ordering drift).
- Replay command stream parity improved:
  - `Code/Main/src/save_load/replay.rs` now records `CommandType::Sell` as explicit `ReplayEventType::SellCommand` instead of aliasing it to `BuildCommand`.
  - Command-event classification now maps additional lanes with explicit replay types:
    - selection commands -> `SelectCommand`,
    - upgrade/economy commands (`PurchaseScience`/`QueueUpgrade`/`CancelUpgrade`) -> `Upgrade`,
    - dozer/construct variants -> `BuildCommand`.
  - Playback command deserialization now accepts replay `SelectCommand` and `Upgrade` event lanes.
- Economy command parity improved:
  - `Code/Main/src/command_executor.rs` no longer uses string-length placeholder pricing for science/upgrade commands.
  - Queue/cancel upgrade commands now use deterministic cost resolution and player-side queued-upgrade tracking, preventing duplicate queue and repeated cancel-refund exploits.
- Save/load snapshot parity improved for production and upgrade state:
  - `Code/Main/src/save_load/snapshot.rs` now snapshots/restores object module data for:
    - building production queue entries/progress/rally point,
    - object applied-upgrade tags.
  - Player snapshots now include deterministic, richer parity data:
    - computed unit population usage,
    - per-team build queue capture from building production queues,
    - non-empty tech-tree capture for owned units/buildings plus unlocked/queued upgrade markers.
- Save/load weather/path cache parity improved:
  - `Code/Main/src/game_logic/game_logic.rs` now carries runtime weather state (`current_weather`, `intensity`, `duration_remaining`, `next_change_time`) with reset/default lifecycle parity.
  - `Code/Main/src/save_load/snapshot.rs` now snapshots/restores real weather state instead of default/no-op behavior.
  - `PathfindingCacheSnapshot` is now populated from active movement paths and used to rehydrate missing movement-path payloads during restore.
- Sync model-load parity improved:
  - `Code/Main/src/assets/manager.rs::load_w3d_model(...)` now executes the real W3D loader path (with timeout + runtime-aware blocking) instead of unconditional fallback-mesh return on cache miss.
- Quoted-printable and map-cache text parity improved:
  - `Code/GameEngine/Common/src/common/system/quoted_printable.rs` now uses C++-aligned UTF-16LE byte semantics for unicode encode/decode paths (instead of approximate UTF-8 behavior).
  - `Code/GameEngine/Common/src/common/ini/ini_map_cache.rs` now routes map-cache quoted-printable decoding through shared Common helpers and no longer emits map-cache insert debug spam.
- Webpage URL INI/runtime parity improved:
  - `Code/GameEngine/Common/src/common/ini/ini_webpage_url.rs` now resolves registry language from runtime language mapping (+ env override path) instead of fixed `English`.
  - `file://` conversion now follows C++ path semantics using encoded cwd and `\\Data\\<language>\\...` formatting.
  - URL parse/open logging moved off stdout (`println!`) to debug logging.
  - `Code/GameEngine/Common/src/common/ini/ini.rs` now actively routes `WebpageURL` blocks, and `ini_webpage_url` now consumes block properties from the core INI stream (`...from_ini`) before URL registration/conversion.
- Object INI block coverage improved in Common parser:
  - `Code/GameEngine/Common/src/common/ini/ini.rs` now includes active block handlers for `Object` and `ObjectReskin`.
  - `Code/GameEngine/Common/src/common/ini/ini_object.rs` now bridges those handlers through the runtime global `ThingFactory` and applies source-existence-aware reskin validation.
- Terrain bridge INI transition parsing parity improved:
  - `Code/GameEngine/Common/src/common/ini/ini_road.rs` now parses `TransitionToOCL` / `TransitionToFX` values with C++-style `Transition`/`ToState`/`EffectNum` semantics and writes transition entries into damage/repair FX/OCL arrays.
  - Bridge `RadarColor` parsing now accepts legacy component format (`R:100 G:114 B:245`) instead of only compact colon triples.
  - `Code/GameEngine/Common/src/common/ini/ini.rs` now actively routes `Road` and `Bridge` blocks into the terrain-road parser, so these definitions are consumed through the primary INI block pipeline.
- Core INI top-level coverage improved in Common parser:
  - `Code/GameEngine/Common/src/common/ini/ini.rs` now actively routes and registers additional major gameplay/content blocks: `FXList`, `Locomotor`, `ParticleSystem`, `SpecialPower`, `Terrain`, `Upgrade`, `Video`, `WaterSet`, `WaterTransparency`, `Weapon`, and `CrateData`.
  - Existing parser lanes in command/audio/control/crate modules now read definition names from block value tokens instead of incorrectly using the block keyword.
  - Remaining stock top-level tokens are now also covered with fallback/nested-safe parsing (`Armor`, `ObjectCreationList`, shell/button/UI map tokens, `WindowTransition`, etc.), with external parsers still overriding when registered.
- ObjectCreationList runtime loading parity improved:
  - `Code/GameEngine/GameLogic/src/object_creation_list/store.rs` now includes a default-path OCL loader/parser for `ObjectCreationList.ini` and ingests all stock nugget headers used by Zero Hour data:
    - `CreateObject`
    - `CreateDebris`
    - `DeliverPayload`
    - `FireWeapon`
    - `Attack`
    - `ApplyRandomForce`
  - Nested `DeliveryDecal ... End` parsing is now handled correctly inside `DeliverPayload`/`Attack` nuggets (inner `End` no longer closes the parent nugget).
  - Parser field coverage now includes the stock advanced lanes used in `ObjectCreationList.ini` (delivery payload data/decal/weapon-slot fields, fire-weapon template field, attack shot/slot/decal fields, random-force fields, and expanded generic lifetime/fade/formation/container/fx/lod/shadow fields).
  - Added regression coverage against real stock data:
    - `test_parse_stock_object_creation_list_file_when_present`
    - `test_parse_advanced_nuggets_and_nested_delivery_decal`
  - OCL load semantics now support incremental layering:
    - per-name OCL replacement no longer clears the full OCL store,
    - previously loaded unrelated OCL definitions are preserved across subsequent loads.
  - `DeliverPayload` runtime behavior is now closer to C++:
    - `create_owner=false` uses the primary object as transport (instead of skipping),
    - payload population only executes when `DeliverPayloadAIUpdate` exists,
    - preferred-height reposition is applied after `deliver_payload(...)` call,
    - delayed delivery uses default disable lane parity (`DisabledDefault`).
  - `Code/GameEngine/GameLogic/src/helpers.rs` now ensures default OCL definitions are loaded before returning OCL lookups, reducing empty placeholder OCL fallback at runtime.
- Bridge damage/repair resource resolution parity improved:
  - `Code/GameEngine/GameLogic/src/object/behavior/bridge_behavior.rs` no longer auto-creates placeholder FX/OCL resources when names are unresolved.
  - Missing bridge transition resources now remain null/skip at runtime (with debug logging), matching C++ null-reference behavior more closely.
- OCL reference parsing parity improved across gameplay modules:
  - Multiple GameLogic parsers no longer auto-create placeholder OCLs for unresolved names during INI parse.
  - Unresolved references now remain `None` and are skipped by runtime execution paths, reducing synthetic side effects from phantom empty OCL entries.
  - Updated modules include OCLUpdate, TransitionDamageFX, ObjectCreationUpgrade, FireOCLAfterWeaponCooldownUpdate, BattleBusSlowDeathBehavior, and BoneFXUpdate.
  - `TheObjectCreationListStore` helper semantics were also tightened:
    - `find/lookup/ensure` now resolve only existing OCLs,
    - implicit empty-list fallback creation was removed from helper lookup paths.
- FX reference handling parity improved across gameplay modules:
  - Added strict FX lookup path (`TheFXListStore::lookup_fx_list`) and made `find_fx_list` lookup-only (no implicit creation); `ensure_fx_list` is now explicit-create only.
  - Removed parse/runtime fallback creation paths that synthesized missing FX entries.
  - Unresolved FX names now remain null and are skipped naturally instead of auto-materializing placeholder FX lists.
  - Updated modules include TransitionDamageFX, BoneFXUpdate, BattleBusSlowDeathBehavior, DumbProjectileBehavior, W3DTreeDraw, ParticleUplinkCannonUpdate, and Unit rappel kill FX.
- Delayed weapon damage processing parity improved:
  - In the active weapon runtime path (`GameLogic/src/weapon/mod.rs`), queued delayed damage now executes real damage logic at processing time (temporary weapon + detonation fire path), instead of no-op completion.
  - Added coverage for delayed-damage enqueue path from template references.
  - Projectileless travel-time weapons in active runtime `Weapon::private_fire_weapon(...)` now use delayed-damage scheduling (or immediate damage for sub-frame delay) instead of failing on missing projectile templates when `projectile_name` is empty.
  - Delayed detonation execution now preserves queued schedule-time `WeaponBonus` snapshots instead of recomputing bonuses from empty execution-time flag inputs.
  - Parallel `weapon/weapon_template.rs` delayed scheduler marker is now replaced with active-store scheduling bridge + immediate fallback on scheduling failure, reducing cross-stack delayed-damage drift.
  - Parallel `weapon/weapon_template.rs` laser branch now creates runtime laser objects (`create_laser_object`) for hit/miss lanes instead of no-op comment paths.
  - Parallel `weapon/weapon_template.rs` now applies real `WEAPON_KILLS_SELF` self-damage with correct mask-bit semantics and huge-damage kill behavior.
  - Parallel `weapon/weapon_template.rs` area-damage source-self filtering now also uses the correct `KILLS_SELF` (SUICIDE) bit, matching non-area lane semantics.
  - Parallel `weapon/weapon_template.rs::deal_damage_internal(...)` now activates historic bonus tracking/triggering at runtime, including chained bonus-weapon fire when configured thresholds are met.
  - Parallel `weapon/weapon_template.rs::deal_damage_internal(...)` now resolves live victim object position for damage centering (fallback to provided target only if lookup fails), reducing moving-target detonation drift.
  - Parallel `weapon/weapon_template.rs` infantry-target scatter path now resolves target `KindOf::Infantry` at runtime and applies `infantry_inaccuracy_dist` to scatter radius (instead of leaving that branch inert).
  - Parallel `weapon/weapon_template.rs` scatter destination now samples terrain layer height (`TheTerrainLogic::get_layer_height`) after X/Y scatter offsets instead of keeping stale victim Z.
  - `weapon/weapon.rs` scatter-target firing path now samples terrain ground height (`TheTerrainLogic::get_ground_height`) after X/Y scatter offsets instead of preserving stale Z.
  - Parallel `weapon/weapon_template.rs` fire path now routes through source `Drawable::handle_weapon_fire_fx(...)`, enabling module-side barrel recoil/muzzle-fire handling instead of leaving this lane skipped.
  - Added coverage:
    - `test_projectileless_weapon_queues_delayed_damage`
    - `test_projectileless_weapon_skips_queue_when_damage_disabled`
- Tree-draw topple animation parity improved:
  - `GameLogic/src/object/draw/w3d_tree_draw.rs` now applies topple rotation into the draw transform path instead of skipping the rotation lane.
  - Topple axis now resolves deterministically from push direction (perpendicular axis in X/Y plane, X-axis fallback).
- Debris draw landed-transition parity improved:
  - `GameLogic/src/object/draw/w3d_debris_draw.rs` now transitions debris from flying to final state based on owner-object terrain contact (`is_above_terrain`) with minimum-frame gating, instead of relying only on external final-transition calls.
  - `GameLogic/src/object/drawable.rs` now binds owner IDs into debris draw modules to support that runtime terrain-state query path.
- Model-draw sub-object visibility state parity improved:
  - `GameLogic/src/object/draw/w3d_model_draw.rs` now treats sub-object hide/show updates case-insensitively and canonicalizes/deduplicates queued sub-object visibility entries before clearing dirty state.
- Full device feature matrix compiles:
  - `cargo check -p game_engine_device --all-features`
- Main package is now build-clean including auxiliary/dev bins:
  - `cargo check -p generals_main --all-features`
- Main playable executable remains build-clean:
  - `cargo check -p generals_main --all-features --bin generals`
- Hard runtime stubs in active non-test source remain at zero:
  - `todo!`: 0
  - `unimplemented!`: 0
  - panic-not-implemented patterns: 0

## What Is Bad / Not Perfect
- Marker debt remains (`TODO|FIXME|placeholder|stub`) in non-test engine/client/gameplay code:
  - Common: 49
  - GameLogic: 16
  - GameClient: 37
  - GameEngineDevice: 9
  - Total: 111
- ObjectCreationList parse coverage now matches stock `ObjectCreationList.ini` structure and active fields; remaining OCL parity risk is now primarily in deeper runtime execution behavior (nugget side-effects/timing edge-cases), not parser block coverage.

## Not Yet Fully Implemented (High Impact)
- `GameEngineDevice/src/w3d/performance_optimizer.rs`
  - Compile-clean and now honors runtime optimizer settings (LOD/culling/instancing/batching + cull distance) with fuller stats accounting.
  - GPU culling now executes real compute-shader dispatch + visibility readback filtering, with resilient CPU fallback for unavailable GPU resources/readback failures.
  - Optimizer telemetry now records real GPU cull and batching stage timings (`gpu_cull_time_ms`, `batch_time_ms`).
  - Optimizer frame-history/memory/quality-control fields are now actively consumed at runtime (periodic telemetry capture + auto-quality decision lane), instead of remaining dormant declarations.
  - LOD selection now applies global LOD bias and avoids implicit default `mesh_lodN` rewrites when no explicit LOD mapping exists (reduces mesh-miss behavior).
  - Instancing now computes inverse-transpose normal matrices for per-instance lighting parity.
  - Instancing/batch grouping now preserves transparent-vs-opaque separation so transparent ordering is not lost during optimization.
  - Remaining gap is exact threshold/tuning parity against C++ policy for all heuristic branches (especially dynamic-resolution integration details and cross-map content tuning).
- `GameEngineDevice/src/w3d/renderer.rs`
  - Transparent materials are now emitted through a dedicated transparent queue with back-to-front sorting.
  - Per-object transform propagation is now wired in the scene render path (no identity-transform fallback for all objects).
  - Remaining gap is deeper render-pass specialization parity (full deferred/forward+ phase behavior and material-state side effects), not transparent queue/transform omission.
- `GameEngineDevice/src/w3d/w3d_c_api.rs`
  - Compile-clean; texture resolution/decoding, state round-trip behavior, transform-to-scene camera sync, and immediate draw submission are improved.
  - Legacy scene lifecycle entry points are now exported (`W3DDevice_BeginScene` / `W3DDevice_EndScene`).
  - Scene lifecycle calls are now stateful (`BeginScene`/`EndScene` reject invalid order) with resilient implicit begin/end bridging for legacy caller drift.
  - Added additional legacy compatibility entry points:
    - `W3DDevice_DrawPrimitiveUP(...)` (primitive-count + strided immediate-mode draw),
    - `W3DDevice_DrawIndexedPrimitiveUP(...)` (indexed immediate-mode draw with 16/32-bit index formats),
    - `W3DDevice_DrawPrimitive(...)` (staged non-indexed primitive submission with start-vertex semantics),
    - `W3DDevice_SetStreamSource(...)` / `W3DDevice_SetStreamSourceUP(...)` / `W3DDevice_SetStreamSourceEx(...)` / `W3DDevice_SetIndices(...)` (staged stream/index geometry compatibility, including explicit stream byte offset state),
    - `W3DDevice_GetStreamSource(...)` / `W3DDevice_GetStreamSourceEx(...)` / `W3DDevice_GetIndices(...)` (staged geometry state readback),
    - `W3DDevice_DrawIndexedPrimitiveLegacy(...)` (DX8-style staged indexed draw args: `min_vertex_index/start_index/primitive_count`),
    - `W3DDevice_SetFVF(...)` / `W3DDevice_GetFVF(...)` and `W3DDevice_SetVertexShader(...)` / `W3DDevice_GetVertexShader(...)` + `W3DDevice_SetPixelShader(...)` / `W3DDevice_GetPixelShader(...)` (fixed-function/shader-state round-trip compatibility),
    - `W3DDevice_SetVertexDeclaration(...)` / `W3DDevice_GetVertexDeclaration(...)` (vertex declaration state compatibility),
    - `W3DDevice_SetTexture(...)` / `W3DDevice_GetTexture(...)` (stage texture bind/readback path),
    - `W3DDevice_SetViewport(...)` / `W3DDevice_GetViewport(...)` (viewport state round-trip),
    - `W3DDevice_SetTextureStageState(...)` / `W3DDevice_GetTextureStageState(...)` (texture-stage state round-trip),
    - `W3DDevice_SetMaterial(...)` / `W3DDevice_GetMaterial(...)` and `W3DDevice_SetLight(...)` / `W3DDevice_GetLight(...)`,
    - `W3DDevice_LightEnable(...)` / `W3DDevice_SetLightEnable(...)` / `W3DDevice_GetLightEnable(...)`.
  - `DrawIndexedPrimitive(...)` now supports staged geometry fallback when direct pointers are absent, including staged base-vertex index behavior.
  - Staged stream decode/readback paths now apply per-stream byte offsets and staged vertex counts consistently; indexed staged decode no longer rejects valid non-`W3D_VERTEX` FVF strides.
  - `DrawPrimitiveUP(...)` + staged stream decode now support common fixed-function non-`W3D_VERTEX` payloads through FVF-aware decoding (including transformed-lit 32-byte vertices).
  - Stage-0 texture-coordinate routing now affects staged/declaration decode:
    - `D3DTSS_TEXCOORDINDEX` now selects declaration `TEXCOORD[n]` usage for UV extraction,
    - staged multi-stream UV overlay now tries the stage-requested stream first before generic fallback.
  - Stage-0 texture transform is now applied to submitted UVs in immediate/staged draw paths using:
    - `D3DTSS_TEXTURETRANSFORMFLAGS` (`D3DTTFF_COUNT1..COUNT4`),
    - projected divide behavior (`D3DTTFF_PROJECTED`),
    - current `W3DTS_TEXTURE0` transform matrix state.
  - Stage-0 texcoord generation now supports camera-space sources:
    - `D3DTSS_TCI_CAMERASPACEPOSITION`,
    - `D3DTSS_TCI_CAMERASPACENORMAL`,
    - `D3DTSS_TCI_CAMERASPACEREFLECTIONVECTOR`,
    - `D3DTSS_TCI_SPHEREMAP`,
    - with source vectors derived from current world/view transform state before texture-matrix application.
  - Camera-space texcoord generation now also applies when `D3DTSS_TEXTURETRANSFORMFLAGS` is disabled (not only in transformed stages), improving fixed-function TCI parity.
  - Texture-stage state queries now return D3D-style defaults for unset keys (`COLOROP`/`ALPHAOP`, args, `TEXCOORDINDEX`, transform flags), improving compatibility with legacy caller assumptions.
  - Draw-time texture/UV/material stage routing now follows the first enabled bound texture stage (stage 0 preferred when enabled) instead of hardcoding stage 0.
  - Draw-time stage routing is now texture-sampling aware:
    - stage selection now skips enabled stages that do not actually sample texture args (for example `SELECTARG2 CURRENT`),
    - includes explicit `D3DTOP_SELECTARG2` argument-usage semantics in stage input analysis.
    - includes triadic texture-op arg0 coverage (`COLORARG0`/`ALPHAARG0`, `D3DTOP_LERP`/`D3DTOP_MULTIPLYADD`) so stage selection recognizes texture sampling from arg0 in multi-argument ops.
    - extended fixed-function op families now participate in texture-usage detection (`MODULATE2X/4X`, signed/smooth/add/subtract variants, blend-alpha variants, premodulate, modulate+add variants, bump-env variants), reducing multi-stage combiner under-detection drift.
    - `D3DTA_*` source detection now respects the selector mask, so `TFACTOR`-only combiner stages no longer masquerade as texture-sampling stages while `COMPLEMENT` / `ALPHAREPLICATE` modifiers on real texture args still work.
    - simple `SELECTARG*` / `MODULATE*` active stages now propagate `D3DRS_TEXTUREFACTOR` into the fallback material tint, including default white texture-factor state and `ALPHAREPLICATE` / `COMPLEMENT` handling.
    - simple enabled no-texture stages can now also contribute `TEXTUREFACTOR` tint to fallback materials instead of always collapsing to an untinted base material.
    - simple additive `TEXTUREFACTOR` alpha stages where both args are `TFACTOR`-derived now also contribute fallback alpha tint, covering common `TFACTOR + (1-TFACTOR)` mask setups used by legacy terrain/shader-manager paths.
    - supported fixed-function fallback stages now propagate `CURRENT` across simple stage chains, so later `MODULATE(TEXTURE, CURRENT)` style stages preserve earlier tint contributions instead of resetting to a stage-local approximation.
    - supported fallback chains now also handle narrow `BLENDCURRENTALPHA` cases, preserving simple `CURRENT`-alpha blends used by legacy terrain/cloud composition paths.
    - modified neutral `TEXTURE` / `DIFFUSE` args and narrow `BLENDFACTORALPHA` cases now participate in the same fallback chain evaluator, improving diffuse-forced white/black terms and texture-factor alpha blends.
    - triadic arg0 combiner cases now also participate in the fallback chain evaluator (`MULTIPLYADD`, `LERP`), covering the W3D shader-manager grayscale path’s `COLORARG0` usage.
    - narrow `DOTPRODUCT3` chains now also participate in that evaluator, preserving the shader-manager grayscale path where `MULTIPLYADD` feeds `CURRENT` into a following dot-product stage.
    - narrow arithmetic combiner chains (`ADDSIGNED`, `ADDSIGNED2X`, `SUBTRACT`, `ADDSMOOTH`) now also participate, reducing stage-selection/material-approximation drift for legacy `CURRENT`-carrying fixed-function passes.
    - neutral-source blend-alpha chains (`BLENDDIFFUSEALPHA`, `BLENDTEXTUREALPHA`, `BLENDTEXTUREALPHAPM`) now also participate in the same approximation lane, preventing those legacy stages from being dropped when the fallback is already operating in its neutral texture/diffuse domain.
    - stage-local `MODULATE*ADD*` combiner chains (`MODULATEALPHA_ADDCOLOR`, `MODULATECOLOR_ADDALPHA`, `MODULATEINVALPHA_ADDCOLOR`, `MODULATEINVCOLOR_ADDALPHA`) now also participate in that evaluator, reducing another class of active-stage/material-resolution drift in legacy `CURRENT`-carrying fixed-function passes.
    - `PREMODULATE` now acts as an explicit pass-through stage in that same evaluator, preserving previously established tint/alpha across legacy premodulate chains instead of treating the stage as unsupported.
    - `BUMPENVMAP` and `BUMPENVMAPLUMINANCE` now also act as pass-through stages in that evaluator, preserving previously established tint/alpha across legacy bump-env coordinate-manipulation stages.
    - `BLENDCURRENTALPHA` is now supported in the alpha lane as well as the color lane, reducing another alpha-only fallback drop case in legacy multi-stage chains.
    - unknown/unsupported op values are now treated as non-sampling in this lane, reducing false-positive stage picks under invalid legacy state payloads.
    - stage selection now prioritizes color-sampling stages over alpha-only sampling stages, reducing legacy multi-stage cases where stage 0 alpha-mask usage incorrectly overrode later color-texture stages.
    - alpha-only texture stage selection remains as fallback when no enabled stage samples texture in color ops.
  - Immediate draw material binding now resolves active-stage texture + current material into effective cached bound materials, improving legacy `SetTexture`/`SetMaterial` call-order parity.
  - Declaration-stream decode now supports additional legacy element types (`FLOAT1`, `USHORT4N`, `UDEC3`, `DEC3N`) across UV/position/normal reconstruction, reducing multi-stream/declaration fidelity drift.
  - Immediate/staged C-API transient draws now honor `W3DRS_ALPHABLENDENABLE` for transparency routing (in addition to material transparency), improving fixed-function render-state parity for legacy draw paths.
  - Fixed-function lighting/material-source state is now tracked in the active bridge:
    - `W3DRS_LIGHTING`, `W3DRS_AMBIENT`, `W3DRS_COLORVERTEX`, `W3DRS_LOCALVIEWER`, `W3DRS_NORMALIZENORMALS`,
    - `W3DRS_DIFFUSEMATERIALSOURCE`, `W3DRS_SPECULARMATERIALSOURCE`, `W3DRS_AMBIENTMATERIALSOURCE`, `W3DRS_EMISSIVEMATERIALSOURCE`.
    - Bound-material cache keys now include those states, ambient render-state color now feeds fallback material approximation, and specular/emissive behavior no longer ignores material-source toggles.
    - No-texture unlit fixed-function materials now remain visible when lighting is disabled instead of collapsing toward black.
  - Fixed-function unlit textured parity is now wired through the active render path:
    - fallback material generation now marks `LIGHTING = FALSE` materials as explicit unlit variants instead of only approximating them through emissive/specular edits,
    - queued renderer material params now derive from actual `MaterialProperties` instead of hardcoded defaults,
    - the default WGSL shader now bypasses dynamic PBR lighting for those unlit fixed-function materials instead of relighting textured prelit passes.
  - Optimizer/instancing paths no longer collapse renderer-specialized material state back to generic defaults:
    - transient `RenderObject`s now carry material-derived batch params/priority into the optimized path,
    - `performance_optimizer.rs` now preserves those params/priority for instanced and non-instanced batches instead of forcing dummy defaults,
    - optimizer batch keys now preserve transparency lane and specialized priority so opaque/transparent or specialized-pass batches do not merge back together incorrectly.
  - Multi-texture fallback approximation is now slightly safer in the active bridge:
    - when a genuine multi-texture chain is detected and a base material already exists, Rust no longer fabricates a false single-stage texture override for the bound-material fallback.
  - Remaining gap is now the broader legacy C-API compatibility surface:
    - full texture-dependent multi-stage combiner evaluation across genuinely multi-texture stage chains,
    - deeper render-pass/material-state specialization outside the current default shader path,
    - fuller material/light semantics where exact legacy behavior depends on richer per-vertex/per-pass state than the current bounded fallback material approximation carries.
- `GameClient/src/core/subsystems.rs`
  - `movie_play_radar` now routes through `TheInGameUI` and in-game window playback (via `WindowVideoManager`) instead of fullscreen display playback, matching C++ action routing.
  - In-game UI movie playback manager is now updated each frame through subsystem lifecycle.
  - Script `is_video_complete` now tracks both fullscreen and in-game movie playback activity before resolving completion waits.
  - Window-video startup now uses real stream-open success/failure (no synthetic fallback stream), improving C++ parity for failed movie opens.
  - Rust movie startup/stop control flow now matches the C++ path more closely:
    - failed or unavailable fullscreen/radar movie starts no longer auto-complete script waits,
    - stop/reset paths no longer synthesize video completion events,
    - pending script waits are only created when playback actually starts.
  - Fullscreen and radar movie wait state is now tracked separately:
    - fullscreen waits alone control the `intro movie playing` hack path,
    - radar/in-game waits no longer perturb intro movie state,
    - client reset/shutdown now clears residual movie/media wait state.
  - Same-name movie restarts now preserve fullscreen vs radar wait-lane separation:
    - fullscreen restarts clear only stale fullscreen pending state,
    - radar restarts clear only stale radar pending state,
    - same-name retries in one lane no longer erase the other lane's pending completion tracking.
  - Global video-player lifecycle is now wired through client init/reset/shutdown:
    - the singleton can be re-created after shutdown,
    - reset now closes global video streams,
    - client teardown now shuts the movie singleton down explicitly.
  - `TheVideoPlayer` subsystem path now behaves more like C++:
    - legacy `Video.ini` metadata is loaded into the Rust GameClient movie registry,
    - subsystem `init/reset/update` now delegate to the real global video-player singleton instead of inert wrapper state,
    - the client update loop now services the video player after the window manager, matching the original runtime ordering more closely.
  - Common movie open/load resolution now also follows the original C++ backend split more closely:
    - movie names are resolved through `Video.ini` to localized/shared `.bik` asset paths,
    - `GlobalData::mod_dir` and extracted Windows asset-tree search participate in that resolution,
    - stream creation is now delegated through a backend-provider hook rather than being permanently hardcoded to `None` in the common player layer.
  - Active movie-stream ownership now also follows the original shape more closely:
    - provider-created streams are tracked by the common player layer,
    - `TheVideoPlayer` update/reset now services and closes those tracked streams centrally,
    - this removes another Rust-only gap where backend-created movie streams would previously bypass common player lifecycle ownership.
  - Provider-validity lifecycle now also follows the original shape more closely:
    - registering or clearing a backend stream provider now notifies the live player immediately,
    - singleton recreation after shutdown restores provider-validity state,
    - shutdown now deinitializes the player and closes tracked provider-backed streams before dropping it.
  - Startup logo/sizzle sequencing now follows the original client more closely:
    - active `GameClient::update()` now drives the EA logo -> `after_intro` -> optional sizzle startup path instead of skipping it,
    - low-res movie preference now also selects the `640` startup variants on that boot path.
  - Common startup-movie defaults/runtime sync now follow the original shape more closely:
    - `play_sizzle` now defaults to enabled again instead of being silently off by default in active runtime data,
    - runtime-global-data application now carries `play_sizzle`, `after_intro`, and `allow_exit_out_of_movies` across the Common handoff instead of dropping them.
  - Common engine init now also follows the original startup transition more closely:
    - if intro playback is already disabled before boot finishes, Rust now promotes `after_intro` during engine init instead of relying only on command-line paths to do so.
  - Direct startup file boot now follows the original engine path more closely:
    - the legacy `-file <path>` switch is now parsed into runtime `initial_file`,
    - `.map` startup targets now disable shell-map/intro startup, stage `pending_file`, enqueue `NewGame`, and reseed logic random with `0`,
    - `.rep` startup targets now route through recorder playback startup instead of being ignored,
    - replay startup now explicitly initializes the recorder singleton before playback bootstrap instead of depending on prior runtime side effects.
  - Movie registry/asset resolution now follows the original path more closely:
    - direct movie names/filenames now still resolve even when no `Video.ini` registry entry exists,
    - direct `.bik` asset paths are accepted as-is,
    - `.bik` filename inputs now resolve by file name or stem,
    - mod-local `Data/INI/Default/Video.ini` and `Data/INI/Video.ini` overlays are now included in movie registry discovery.
  - Active client startup frame behavior now also follows the original boot path more closely:
    - while `play_intro` / `after_intro` is active, Rust now short-circuits the normal frame update stack and only services startup movie display work,
    - boot-path display servicing now follows `DRAW()` then `UPDATE()` ordering for that startup-only path.
  - Score-screen campaign completion now follows the original cinematic flow more closely:
    - end-of-campaign single-player now stages `Campaign::get_final_victory_movie()` instead of finalizing immediately,
    - score-screen finalization now waits for fullscreen movie playback to complete,
    - failed movie startup falls back to immediate finalization instead of leaving the score screen in a pending state,
    - low-detail preference state now suppresses the final-victory movie path instead of always attempting fullscreen playback.
  - Global/per-window pause+resume now follows C++ `WindowVideoManager` state semantics directly (`Pause`/`Play` state assignment), with hidden transitions handled by the runtime update loop.
  - Stop-path parity improved: `stop_movie` / `stop_all_movies` now set `Stop` state without removing entries (matching C++), while `stop_and_remove_movie` remains the explicit resource-release/removal lane.
  - `WindowVideoManager::update()` now matches C++ global-toggle and hidden-state transition semantics:
    - update short-circuits when `pause_all_movies` or `stop_all_movies` is active,
    - only `Play -> Hidden` and `Hidden -> Play` state transitions are applied from window visibility changes.
  - Subsystem lifecycle parity improved: `InGameUISubsystem::init/reset` now clear transient in-game UI state (queued UI events, placement anchors/templates, command/special-power pendings, and mode flags), reducing stale state carry-over between sessions.
  - Remaining gap is primarily rare edge-case timing nuance across mixed cinematic/UI transition paths.
- `Common/src/common/ini/ini_mapped_image.rs`
  - `ImageCollection::load` now follows C++ directory-flow (`UserData` overrides + `TextureSize_<N>` + `HandCreated`) and parse behavior is aligned for `Coords`/`Status`.
  - `parse_mapped_image_definition(...)` now parses in place against existing mapped-image entries (create/add-then-parse behavior) instead of clone-and-replace semantics.
  - Existing entries with `raw_texture_data` no longer hard-fail during reparse; path now warns and proceeds, matching C++ debug-assert-only behavior in release builds.
  - `parse_image_coords(...)` now accepts both colon and space-separated subtoken forms (`Left:10` and `Left 10`), matching C++ subtoken parsing tolerance.
- `GameClient/src/display/image.rs`
  - Client mapped-image metadata now syncs from Common at startup and lazily loads textures on first GPU upload from:
    - engine `FileSystem` (including BIG archive backend),
    - direct OS file paths as fallback.
  - Runtime now auto-ensures Local/BIG backends + search paths + FileSystem init on first mapped-image resource read, reducing startup ordering sensitivity.
- `Common/src/common/audio/audio_event_rts.rs`
  - Core C++ filename/play-info/localization behavior is now ported and owner/player resolver hooks are wired from `GameLogic` helper bridge.
  - C++ control/type bit masks and `AudioPriority` ordering are now corrected.
  - Async playback path now uses backend playback hook dispatch + runtime playback polling/cancel-stop semantics instead of fixed-duration simulation when backend is present.
  - Cached event-info lookup now enforces C++ validity gating (`audio_name` must match current `event_name`) so stale cache entries are ignored.
  - Remaining parity debt is in broader audio routing side-effects.
- `Common/src/common/audio/game_audio.rs` + `GameLogic/src/helpers.rs`
  - C++-style `shouldPlayLocally` player/allies/enemies gating is now active in runtime add-event path via resolver bridge.
  - Observer look-at fallback for dead local player is now wired via GameClient control-bar observer target hooks.
  - `add_audio_event` now returns C++-style sentinel handles (`NoSound`/`Error`/`NotForLocal`/`Muted`) rather than flattening failure paths to `0`.
  - `remove_audio_event` now honors stop-music sentinel handles and avoids sending stop requests for non-playing failure sentinels.
  - `is_on`/`set_on`/`set_volume`/`get_volume` now use C++ bitmask semantics, including combined affects and `SystemSetting` behavior.
  - Runtime now instantiates a music manager by default, avoiding false-positive music-play success on a missing manager.
  - C++ `getAudioLengthMS` flow is now implemented for script timing (`attack + main + decay` generated filenames), removing always-zero duration behavior.
  - Duration probing now covers WAV/MP3/OGG for script timing waits.
  - Remaining gap is rare/streaming codec edge behavior and exact legacy decode-time parity under malformed media.
- `GameLogic/src/action_manager.rs` + `GameLogic/src/commands/command_processor.rs` + `GameLogic/src/ai/ai_player.rs`
  - Playable map-extent parity improved by switching C++ `getExtent(...)`-equivalent checks to `get_maximum_pathfind_extent()` in action validation, beacon clamping, and AI superweapon fallback targeting.
- `GameLogic/src/object/unit.rs`
  - Player move-goal clipping now uses playable extent (`get_maximum_pathfind_extent`) instead of border-inclusive bounds, reducing edge-command drift.
- `GameLogic/src/terrain.rs`
  - `find_closest_edge_point` tie-break ordering now matches C++ (`top/right/bottom/left` precedence), reducing edge-selection drift in tie cases.
  - Bridge area checks now use actual bridge quad containment (not only bridge AABB), reducing false bridge-layer hits near rotated-bridge bounds.
  - `TerrainLogic::load_map(...)` now performs real map-file resolution + parse via `MapLoader`, and correctly fails on missing/invalid maps instead of unconditional success.
- `Common/src/common/audio/game_sounds.rs`
  - Sound admission path now applies limit/voice/channel gates through both concrete and trait-call paths.
  - Active-sound lifecycle cleanup now updates counters from backend playback completion.
  - AudioManager lifecycle/listener/sample configuration now flows into SoundManager trait path.
  - `ST_SHROUDED` visibility cull now uses live shroud-state resolver from GameLogic.
- `Common/src/common/audio/game_speech.rs`
  - `say_name` now resolves against loaded speech definitions instead of fabricating placeholder speech objects.
  - Speaker pause gating bug in update path is fixed.
- `Code/Main/src/game_logic/game_logic.rs` + `Code/Main/src/assets/models.rs` + `Code/Main/src/assets/manager.rs`
  - Map-object template synthesis now attempts real WW3D object-definition mapping for all unresolved template names (including decorative map objects) before fallback gating.
  - Decorative templates now perform archive-open model availability checks before insertion; unresolved decorative objects are skipped early instead of spawning fallback-triangle entities.
  - W3D model loading now probes broader mixed-case archive path/extension variants, and INI parser parity now captures `ModelName` + `Draw` keys used by Nature/Civilian map props.
  - Audio-only ambient objects (`SoundAmbient` + no model) are filtered from visual template synthesis.
  - Current quickstart/multi-map smokes in this lane report zero fallback-mesh warnings; remaining parity gap is visual completeness for decorative objects currently skipped as non-loadable instead of exact legacy remaps.
- `Code/Main/src/assets/textures.rs`
  - DDS compressed decode parity in `Main` is improved:
    - DXT3 explicit-alpha decode is implemented,
    - DXT5 interpolated-alpha decode is implemented,
    - DXT decode now writes row-major output pixels across multi-block textures.
  - Added challenge-map texture alias coverage for residual missing IDs:
    - `PMBarbwire2`, `UBStingerS02`, `UVTechWeap`, `UVToxinTrk`, `CBMogWell01`, `UBUndTunn01`, `UBUndTunnD`.
  - 9-map quickstart sweep now reports zero missing-texture warnings.

## Missing For Fully Playable (Single-Player)
- Close the high-impact non-network parity gaps above (W3D optimizer/C-API and remaining INI/audio/movie edge behavior).
- Complete runtime parity validation passes (campaign/skirmish/save-load/scripts/object visuals) against C++ reference behavior.
- Decide whether to fully port the legacy `game_engine_device` W3D feature lane or retire/gate it if superseded by the active WW3D stack.
- Improve decorative map-prop fidelity by resolving currently skipped non-loadable model IDs via stability-safe remaps (avoiding WW3D frame-state regressions observed with aggressive alias sets).

## Validation Run In This Pass
- `cargo test -q -p game_engine --test mapped_image_parity_tests -- --nocapture` (pass, `2 passed`)
- `cargo test -q -p game_engine --test audio_event_parity_tests -- --nocapture` (pass, `2 passed`)
- `cargo check -q -p game_engine` (pass)
- `cargo run -q -p generals_main --bin playability_audit` (pass, `0 unresolved blocker events`)
- `cargo test -q -p game_engine --test audio_event_parity_tests -- --nocapture` (pass, `2 passed`)
- `cargo check -q -p game_engine` (pass)
- `cargo check -q -p generals_main --all-features --bin generals` (pass)
- `cargo run -q -p generals_main --bin playability_audit` (pass, `0 unresolved blocker events`)
- `cargo check -q -p game_engine` (pass)
- `cargo check -q -p generals_main --all-features --bin generals` (pass)
- `cargo run -q -p generals_main --bin playability_audit` (pass, `0 unresolved blocker events`)
- `RUSTFLAGS='-Awarnings' cargo test -q -p game_engine_device --features w3d resolve_active_texture_stage_ -- --nocapture` (pass, `3 passed`)
- `RUSTFLAGS='-Awarnings' cargo test -q -p game_engine_device --features w3d op_uses_texture_arg_ -- --nocapture` (pass, `2 passed`)
- `RUSTFLAGS='-Awarnings' cargo check -q -p game_engine_device --features w3d` (pass)
- `RUSTFLAGS='-Awarnings' cargo check -q -p generals_main --all-features --bin generals` (pass)
- `RUSTFLAGS='-Awarnings' cargo run -q -p generals_main --bin playability_audit` (pass, `0 unresolved blocker events`)
- `RUSTFLAGS='-Awarnings' cargo test -q -p gamelogic topple_axis_ -- --nocapture` (pass, `2 passed`)
- `RUSTFLAGS='-Awarnings' cargo test -q -p gamelogic topple_rotation_matrix_absent_when_not_toppling -- --nocapture` (pass, `1 passed`)
- `RUSTFLAGS='-Awarnings' cargo check -q -p gamelogic` (pass)
- `RUSTFLAGS='-Awarnings' cargo check -q -p generals_main --all-features --bin generals` (pass)
- `RUSTFLAGS='-Awarnings' cargo run -q -p generals_main --bin playability_audit` (pass, `0 unresolved blocker events`)
- `RUSTFLAGS='-Awarnings' cargo check -q -p gamelogic` (pass)
- `RUSTFLAGS='-Awarnings' cargo test -q -p gamelogic test_effective_scatter_radius_ -- --nocapture` (pass, `3 passed`)
- `RUSTFLAGS='-Awarnings' cargo check -q -p generals_main --all-features --bin generals` (pass)
- `RUSTFLAGS='-Awarnings' cargo run -q -p generals_main --bin playability_audit` (pass, `0 unresolved blocker events`)
- `RUSTFLAGS='-Awarnings' cargo test -q -p game_engine_device --features w3d resolve_active_texture_stage_ -- --nocapture` (pass, `2 passed`)
- `RUSTFLAGS='-Awarnings' cargo test -q -p game_engine_device --features w3d op_uses_texture_arg_respects_selectarg2_current -- --nocapture` (pass, `1 passed`)
- `RUSTFLAGS='-Awarnings' cargo check -q -p game_engine_device --features w3d` (pass)
- `RUSTFLAGS='-Awarnings' cargo check -q -p generals_main --all-features --bin generals` (pass)
- `RUSTFLAGS='-Awarnings' cargo run -q -p generals_main --bin playability_audit` (pass, `0 unresolved blocker events`)
- `RUSTFLAGS='-Awarnings' cargo test -q -p gamelogic test_effective_scatter_radius_ -- --nocapture` (pass, `3 passed`)
- `RUSTFLAGS='-Awarnings' cargo check -q -p gamelogic` (pass)
- `RUSTFLAGS='-Awarnings' cargo check -q -p generals_main --all-features --bin generals` (pass)
- `RUSTFLAGS='-Awarnings' cargo run -q -p generals_main --bin playability_audit` (pass, `0 unresolved blocker events`)
- `RUSTFLAGS='-Awarnings' cargo check -q -p game_engine_device --features w3d` (pass)
- `RUSTFLAGS='-Awarnings' cargo test -q -p game_engine_device --features w3d quality_direction_` (pass, `4 passed`)
- `RUSTFLAGS='-Awarnings' cargo test -q -p game_engine_device --features w3d average_frame_time_uses_history_entries` (pass, `1 passed`)
- `RUSTFLAGS='-Awarnings' cargo check -q -p generals_main --all-features --bin generals` (pass)
- `RUSTFLAGS='-Awarnings' cargo run -q -p generals_main --bin playability_audit` (pass, `0 unresolved blocker events`)
- `cargo check -p game_engine_device --all-features --message-format short` (pass)
- `cargo check -p game-client-rust --message-format short` (pass)
- `cargo check -p gamelogic --message-format short` (pass)
- `RUSTFLAGS='-Awarnings' cargo check -q -p generals_main --all-features` (pass)
- `RUSTFLAGS='-Awarnings' cargo test -q -p generals_main decode_dxt` (pass, `3 passed`)
- `RUSTFLAGS='-Awarnings' cargo test -q -p generals_main replay::tests::` (pass, `5 passed`)
- `cargo test -q -p generals_main snapshot_restore_preserves_building_production_modules_and_object_upgrades` (pass)
- `cargo test -q -p generals_main snapshot_player_state_captures_population_build_queue_and_research` (pass)
- `cargo test -q -p generals_main snapshot_restore_preserves_weather_state` (pass)
- `cargo test -q -p generals_main snapshot_restore_rehydrates_paths_from_pathfinding_cache` (pass)
- `cargo test -q -p generals_main save_load::snapshot::tests::` (pass)
- `RUSTFLAGS='-Awarnings' cargo test -q -p generals_main queue_upgrade_deducts_once_per_team_and_prevents_duplicate_queue` (pass)
- `RUSTFLAGS='-Awarnings' cargo test -q -p generals_main cancel_upgrade_refunds_only_when_upgrade_is_queued` (pass)
- `RUSTFLAGS='-Awarnings' cargo test -q -p generals_main process_ai_behavior_` (pass, `3 passed`)
- `RUSTFLAGS='-Awarnings' cargo test -q -p generals_main single_instance::tests::test_create_generals_mutex_retains_guard` (pass)
- `RUSTFLAGS='-Awarnings' cargo test -q -p generals_main visibility_filter_` (pass, `2 passed`)
- `RUSTFLAGS='-Awarnings' cargo test -q -p generals_main find_object_` (pass, `2 passed`)
- `RUSTFLAGS='-Awarnings' cargo test -q -p generals_main mouse_position` (pass, `2 passed`)
- `RUSTFLAGS='-Awarnings' cargo test -q -p generals_main drag_selection_prefers_world_drag_bounds_when_provided` (pass)
- `RUSTFLAGS='-Awarnings' cargo run -q -p generals_main --bin playability_audit` (pass, `0 unresolved blocker events`)
- `RUSTFLAGS='-Awarnings' RUST_LOG=warn timeout 35s cargo run -q -p generals_main --bin generals -- -quickstart` (alive until timeout; `Using fallback mesh: 0`; `No texture found for: 0`; no frame begin/end failure logs)
- 9-map 20s quickstart sweep (`defcon6`, `tournamenta`, `tournamentb`, `gc_nukegeneral`, `gc_superweaponsgeneral`, `gc_chemgeneral`, `forgottenforestzh`, `hostile dawn`, `homeland alliance`):
  - `Using fallback mesh`: 0
  - `No texture found for:`: 0
  - `begin_frame failed` / `engine frame already active`: 0
  - `end_frame failed`: 0

## 2026-03-02 Stability-Safe Decorative Alias Progress

### What Landed
- Added conservative non-vegetation decorative model aliases in:
  - `GeneralsRust/Code/Main/src/game_logic/game_logic.rs`
  - `GeneralsRust/Code/Main/src/assets/manager.rs`
- Closed map-prop remaps for:
  - `PMBoulders`, `PMlclusters`, `PMmcluster`, `pmcluster`, `PMRocks02/03/05/06/07`
  - `PMTrshPp03`, `PMTrshPl02`, `PMCrates`, `PMPUMP`
  - `CBSandBW2`, `CBSandBW4c`, `CVTRUCK`, `CBNShack`

### Stability Result
- Quickstart debug smoke remains stable after this subset:
  - `begin_frame failed`: `0`
  - `end_frame failed`: `0`
  - `Using fallback mesh`: `0`
  - `No texture found for:`: `0`

### Important Guardrail
- A broader alias attempt that included `PT*` vegetation (`PTBush*`, `PTPine*`, `PTSpruce*`, `PTXPine05`) reproduced WW3D frame-state regression.
- Those vegetation aliases were rolled back.
- Remaining `due unavailable model` debug logs are now concentrated on that vegetation-only set.

### Current Residual Decorative Gaps (Debug Only)
- `Bush02 -> PTBush02`
- `Bush03 -> PTBush03`
- `Bush08 -> PTBush08`
- `Bush11 -> PTBush11`
- `TreePine -> PTPine01`
- `TreePine3 -> PTPine02`
- `TreeSpruce2 -> PTSpruce01_hi`
- `TreeSpruce05 -> PTXPine05`

## 2026-03-02 PT Vegetation Safe-Coverage Update

### What Changed
- Added runtime PT vegetation alias mode handling in:
  - `GeneralsRust/Code/Main/src/assets/manager.rs`
  - `GeneralsRust/Code/Main/src/game_logic/game_logic.rs`
- Default mode is now `trees_pines` (used when `GENERALS_PT_VEGETATION_ALIAS_MODE` is unset).

### Why
- `trees_pines` gives higher stable visual coverage than prior `bushes` default.
- It remains frame-stable in long quickstart smokes, while larger mixed modes regress WW3D frame state.

### Stability Matrix (80s Quickstart)
- Stable: `tree_pine1`, `tree_pine2`, `tree_spruce2`, `tree_spruce05`, `trees_pines`, `trees_spruces`.
- Unstable: `trees_three`, `all_fir`, `all_birch`, `all_oak`, `all_palm`, `all_maple`, `bushes_pines`, `bushes_spruces`.

### Current Effect
- `due unavailable model` (debug quickstart) improved to `176` with default `trees_pines`.
- Residual unresolved decorative set:
  - `Bush02 -> PTBush02`
  - `Bush03 -> PTBush03`
  - `Bush08 -> PTBush08`
  - `Bush11 -> PTBush11`
  - `TreeSpruce2 -> PTSpruce01_hi`
  - `TreeSpruce05 -> PTXPine05`

### Validation
- `cargo check -q -p generals_main --all-features` (pass)
- `cargo run -q -p generals_main --bin playability_audit` (pass, `0 unresolved blocker events`)
- `RUST_LOG=warn timeout 80s cargo run -q -p generals_main --bin generals -- -quickstart` (stable; no frame begin/end failures)

## 2026-03-02 WW3D Frame-State Recovery Hardening

### Runtime Fixes
- Hardened frame lifecycle in WW3D paths:
  - `ww3d-engine begin_render`: `frame_active` now toggles only on successful frame acquisition.
  - `ww3d-engine end_render`: `frame_active` now clears on all end-frame paths.
  - `WgpuMainRenderer end_frame`: always attempts engine-frame unwind even if frame rendering/callback stages error.

### Result
- Removed the persistent `begin_frame failed: engine frame already active` poison loop after a render failure.
- In aggressive stress mode (`GENERALS_PT_VEGETATION_ALIAS_MODE=all_fir`), failures now remain bounded to `end_frame` (no begin-frame lockout storm).

### Default Playability Status (unchanged stable mode)
- Default (`trees_pines`) quickstart remains stable:
  - `begin_frame failed`: `0`
  - `end_frame failed`: `0`
  - `Using fallback mesh`: `0`
  - `No texture found for:`: `0`

## 2026-03-02 WW3D Uniform Arena Growth + PT Default Promotion

### What Changed
- `GeneralsRust/Code/Libraries/Source/WWVegas/WW3D2/crates/ww3d-renderer-3d/src/rendering/frame_uniform_arena.rs`
  - Frame uniform allocator moved from fixed single-buffer capacity to dynamic multi-page growth.
  - This closes render-frame failures under high mesh density (`frame uniform arena exhausted`).
- `GeneralsRust/Code/Libraries/Source/WWVegas/WW3D2/crates/ww3d-renderer-3d/src/core/error.rs`
  - Error conversion now preserves concrete renderer failure context instead of collapsing many paths into `Unknown`.
- `GeneralsRust/Code/Main/src/assets/manager.rs`
- `GeneralsRust/Code/Main/src/game_logic/game_logic.rs`
  - Default `GENERALS_PT_VEGETATION_ALIAS_MODE` is now `all_fir` (no env var needed).

### Stability Result
- Stress lane (`60s`, `GENERALS_PT_VEGETATION_ALIAS_MODE=all_fir`, `RUST_LOG=warn`):
  - `begin_frame failed`: `0`
  - `end_frame failed`: `0`
  - `frame uniform arena exhausted`: `0`
- Default lane (`35s`, `RUST_LOG=debug`):
  - `due unavailable model`: `0`
  - `begin_frame failed`: `0`
  - `end_frame failed`: `0`
  - `Using fallback mesh`: `0`
  - `No texture found for:`: `0`

### Previously Unstable PT Modes (Now Stable)
Validated at `25s` each (`RUST_LOG=warn`):
- `trees_three`, `all_fir`, `all_birch`, `all_oak`, `all_palm`, `all_maple`, `bushes_pines`, `bushes_spruces`
- All above now show:
  - `begin_frame failed=0`
  - `end_frame failed=0`

Coverage note from this sweep:
- `all_fir/all_birch/all_oak/all_palm/all_maple` each report `due unavailable model=0`.

### Validation
- `RUSTFLAGS='-Awarnings' cargo check -q -p ww3d-renderer-3d` (pass)
- `RUSTFLAGS='-Awarnings' cargo check -q -p generals_main --all-features` (pass)
- `RUSTFLAGS='-Awarnings' cargo run -q -p generals_main --bin playability_audit` (pass; `0 unresolved blocker events`)
- 9-map quickstart sweep (`20s` each):
  - `begin_frame failed`: `0`
  - `end_frame failed`: `0`
  - `Using fallback mesh`: `0`
  - `No texture found for:`: `0`

### Build Cache
- `GeneralsRust/target`: `15G`.
- No cleanup run this pass (below heavy-threshold concern).

## 2026-03-02 Map Bounds Metadata Parity Update

### What Changed
- `GeneralsRust/Code/Main/src/game_logic/script_loader.rs`
  - `parse_world_bounds(...)` now falls back to `HeightMapData` dimensions when waypoint bounds are missing/degenerate.
  - Bounds are derived from playable terrain dimensions with `MAP_XY_FACTOR` scaling and returned at metadata parse time.

### Why It Matters
- Several map setup paths consume `MapMetadata.world_min/world_max` before later runtime terrain fallback.
- Early playable bounds prevent transient fallback extents and remove noisy degenerate-bounds startup behavior.

### Validation
- `cargo check -q -p generals_main --all-features` (pass)
- `cargo run -q -p generals_main --bin playability_audit` (pass, `0 unresolved blocker events`)
- Quickstart smoke (`40s`, warn logs):
  - `reported degenerate bounds`: `0`
  - `begin_frame failed`: `0`
  - `end_frame failed`: `0`
  - `Using fallback mesh`: `0`
  - `No texture found for:`: `0`
- 9-map 20s sweep:
  - aggregate `degenerate bounds`: `0`
  - frame/fallback/texture failure counters remain `0`.

## 2026-03-02 W3D C-API UV-Set Selection Parity Update

### What Changed
- `GeneralsRust/Code/GameEngine/GameEngineDevice/src/w3d/w3d_c_api.rs`
  - FVF decode now selects UV set based on active draw stage `D3DTSS_TEXCOORDINDEX` when `TEXCOUNT > 1`.
  - Applied to:
    - `DrawPrimitiveUP` / `DrawIndexedPrimitiveUP` immediate decode,
    - staged stream decode helpers before declaration/overlay pass.

### Why It Matters
- Some legacy callers use multi-UV FVF streams and select non-zero UV channels through stage state.
- Previous behavior always consumed UV set 0 in FVF decode, causing stage-state UV routing drift.

### Validation
- `cargo check -q -p game_engine_device --features w3d` (pass)
- `cargo check -q -p generals_main --all-features` (pass)
- `cargo run -q -p generals_main --bin playability_audit` (pass, `0 unresolved blocker events`)
- quickstart smoke remains clean (`begin/end_frame` failures `0`, fallback mesh `0`, missing texture `0`, degenerate-bounds warns `0`).

## 2026-03-02 W3D C-API FVF Texcoord-Dimension Parity Update

### What Changed
- `GeneralsRust/Code/GameEngine/GameEngineDevice/src/w3d/w3d_c_api.rs`
  - FVF decode now reads per-set texcoord dimensions from FVF texture-format bits (`D3DFVF_TEXTUREFORMAT*`) and advances decode offsets accordingly.
  - Combined with active-stage `TEXCOORDINDEX` routing, this now handles multi-set FVF streams with mixed texcoord dimensions more faithfully.

### Why It Matters
- Legacy fixed-function callers can use non-float2 texcoord sets.
- Hardcoded float2 decode can desynchronize vertex parsing and feed wrong UVs/states to draw submission.

### Validation
- `cargo check -q -p game_engine_device --features w3d` (pass)
- `cargo check -q -p generals_main --all-features` (pass)
- `cargo run -q -p generals_main --bin playability_audit` (pass, `0 unresolved blocker events`)
- quickstart smoke remains clean:
  - `begin_frame failed`: `0`
  - `end_frame failed`: `0`
  - `Using fallback mesh`: `0`
  - `No texture found for:`: `0`
  - `reported degenerate bounds`: `0`

### Test Caveat
- `cargo test -p game_engine_device --features w3d` currently fails on an unrelated pre-existing parse error in `video/cpp_bindings.rs`; this blocks executing targeted new unit tests until that independent test issue is corrected.

## 2026-03-02 W3D C-API Declaration Stream Fidelity Update

### What Changed
- `GeneralsRust/Code/GameEngine/GameEngineDevice/src/w3d/w3d_c_api.rs`
  - Staged draw decode now executes a declaration-first path when a vertex declaration is active.
  - This closes a parity gap where stream 0 was implicitly treated as the base source even when declaration semantics place position on another stream.
  - New behavior reconstructs `W3D_VERTEX` from declaration usages:
    - required: `POSITION`/`POSITIONT`,
    - optional overlays: `NORMAL`, `COLOR0`, `TEXCOORD[n]` (active-stage usage index, fallback `TEXCOORD0`).
  - Added position decode support for common declaration encodings (`FLOAT2/3/4`, `SHORT2/4`, normalized short/ushort forms).

### Test-Lane Fix
- `GeneralsRust/Code/GameEngine/GameEngineDevice/src/video/cpp_bindings.rs`
  - Fixed pre-existing test parser issue in enum-cast comparison assertion.
  - This removes the previously reported unrelated blocker for targeted `game_engine_device` w3d tests.

### Validation
- `cargo check -q -p game_engine_device --features w3d` (pass)
- `cargo test -q -p game_engine_device --features w3d w3d_c_api::tests::declaration_stream_decode_uses_nonzero_position_stream` (pass)
- `RUSTFLAGS='-Awarnings' cargo check -q -p generals_main --all-features` (pass)
- `RUSTFLAGS='-Awarnings' cargo run -q -p generals_main --bin playability_audit` (pass, `0 unresolved blocker events`)
- quickstart smoke (`25s`, warn logs):
  - `begin_frame failed`: `0`
  - `end_frame failed`: `0`
  - `Using fallback mesh`: `0`
  - `No texture found for:`: `0`

### Cache Check
- `GeneralsRust/target`: `19G`.
- No cleanup run this pass (below heavy cache threshold).

## 2026-03-02 GameEngineDevice Robustness Follow-Up

### What Changed
- `GeneralsRust/Code/GameEngine/GameEngineDevice/examples/video_device_demo.rs`
- `GeneralsRust/Code/GameEngine/GameEngineDevice/examples/w3d_device_demo.rs`
  - Fixed example-build compile failures seen in full crate test runs by using `tracing_subscriber::fmt::init()` and correcting missing imports.
- `GeneralsRust/Code/GameEngine/GameEngineDevice/src/video/video_device.rs`
  - `Drop` path no longer uses runtime-blocking calls that panic under current-thread Tokio test contexts.
  - Teardown now performs non-blocking lock attempt on `initialized` and clears internal handle maps directly.

### Validation
- `cargo check -q -p game_engine_device --features w3d` (pass)
- `cargo test -q -p game_engine_device --features w3d w3d_c_api::tests::declaration_stream_decode_uses_nonzero_position_stream` (pass)
- `cargo test -q -p game_engine_device --features w3d` (still failing, but now on environment/assumption-heavy integration tests, not compile blockers):
  - macOS main-thread `winit` event loop requirement,
  - current-thread runtime assumption mismatch in `VideoDevice::clone`,
  - strict `w3d` init assertion mismatch on current environment.
- `RUSTFLAGS='-Awarnings' cargo check -q -p generals_main --all-features` (pass)
- `RUSTFLAGS='-Awarnings' cargo run -q -p generals_main --bin playability_audit` (pass; `0 unresolved blocker events`)
- quickstart smoke (`25s`, warn logs):
  - `begin_frame failed`: `0`
  - `end_frame failed`: `0`
  - `Using fallback mesh`: `0`
  - `No texture found for:`: `0`

## 2026-03-02 Device Init/Clone Parity Stabilization

### What Changed
- `GeneralsRust/Code/GameEngine/GameEngineDevice/src/lib.rs`
  - `init_video_device(...)` now calls `VideoDevice::init()` before storing/returning the device.
  - `init_w3d_device(...)` now calls `W3DDevice::init()` before storing/returning the device.
- `GeneralsRust/Code/GameEngine/GameEngineDevice/src/w3d/w3d_device.rs`
  - Added `init()` as the explicit headless/default initialization entrypoint.
  - `init_with_window(...)` now forwards to `init()` (window-independent parity path remains intact).
- `GeneralsRust/Code/GameEngine/GameEngineDevice/src/video/video_device.rs`
  - Replaced clone-by-recreation with structural clone of shared state (`Arc`/locks).
  - Prevents clone-time initialization reset and avoids platform-specific display re-enumeration side effects.

### Validation
- `cargo test -q -p game_engine_device --features w3d --test integration_tests`
  - `19 passed; 0 failed`.
- `RUSTFLAGS='-Awarnings' cargo check -q -p generals_main --all-features` (pass)
- `RUSTFLAGS='-Awarnings' cargo run -q -p generals_main --bin playability_audit` (pass; `0 unresolved blocker events`)
- quickstart smoke (`25s`, warn logs):
  - `begin_frame_failed=0`
  - `end_frame_failed=0`
  - `fallback_mesh=0`
  - `missing_texture=0`
  - `degenerate_bounds=0`

### Current Read
- Device initialization status parity for integration tests is now closed.
- Immediate playability signals remain clean in this environment after the fix set.

## 2026-03-02 Mission Script Weather Visibility (`doWeather`) Parity

### What Changed
- `GeneralsRust/Code/Main/src/game_logic/mission_scripts.rs`
  - Added mission-script weather visibility queueing (`push_weather_visible` / `drain_weather_visibility_updates`).
  - Implemented `MissionScriptActionHandler::set_weather_visible` to forward `Show Weather` script actions into runtime hooks.
  - Added regression test `handler_forwards_weather_visibility_requests`.
- `GeneralsRust/Code/Main/src/game_logic/game_logic.rs`
  - Added runtime weather visibility state (`RuntimeWeatherState.visible`, default `true`).
  - Added `GameLogic::set_weather_visible(...)`.
  - Drains weather visibility requests in script evaluation and applies last-value-wins behavior per frame.
  - Added regression test `weather_visibility_script_requests_apply_last_value`.
- `GeneralsRust/Code/Main/src/save_load/snapshot.rs`
  - Extended `WeatherSnapshot` with `visible` and defaulting (`weather_visible_default`) for serialization compatibility.
  - Snapshot capture/restore now round-trips weather visibility.
  - Updated `snapshot_restore_preserves_weather_state` to assert visibility persistence.

### Validation
- `cargo test -q -p generals_main handler_forwards_weather_visibility_requests` (pass)
- `cargo test -q -p generals_main weather_visibility_script_requests_apply_last_value` (pass)
- `cargo test -q -p generals_main snapshot_restore_preserves_weather_state` (pass)
- `cargo test -q -p generals_main save_load::snapshot::tests::` (pass)
- `cargo check -q -p generals_main --all-features` (pass)
- `cargo run -q -p generals_main --bin playability_audit` (pass; `0 unresolved blocker events`)

## 2026-03-02 Renderer Runtime Parity Update (HLOD + Shadow)

### What Improved
- `ww3d-renderer-3d/rendering/hlod_system.rs`
  - `optimize_tree` is now active runtime logic (no longer a no-op).
  - `generate_impostors` now generates concrete far-distance billboard impostor meshes.
- `ww3d-renderer-3d/rendering/shadow_system/shadow_map.rs`
  - Shadow caster rendering now has a typed submission API that executes real draw calls (`draw`/`draw_indexed`) instead of inert placeholder flow.
- `ww3d-renderer-3d/rendering/shadow_system/shadow_renderer.rs`
  - Shadow map allocation now creates/stores real GPU map resources per light.
  - Shadow caster statistics are now driven by deterministic pass accounting (`shadow_caster_count_hint`) instead of hardcoded fake increments.

### Why This Matters For Playability
- HLOD and shadow lanes now have concrete runtime behavior in key places that previously masked parity gaps behind placeholders.
- Rendering telemetry from these systems is now grounded in actual submissions/resources, reducing false confidence from synthetic counters.

### Current Remaining Gaps (Still Not Final)
- `shadow_map::is_point_in_shadow` still uses CPU approximation (now bias/filter aware) rather than direct sampled depth-map comparison outside shader execution.
- Shadow submissions are now auto-captured in `WgpuMainRenderer`, but full light-space shadow pass raster integration still needs deeper end-to-end hookup so captured submissions directly drive concrete per-light map rendering.
- Broader full-game parity (UI/mission/campaign scripting edge cases, some C-API breadth, and subsystem integration depth) still requires continued batch closure.

### Validation
- `cargo test -q -p ww3d-renderer-3d hlod_system::tests:: -- --nocapture` -> PASS
- `cargo test -q -p ww3d-renderer-3d shadow_system::shadow_map::tests:: -- --nocapture` -> PASS
- `cargo test -q -p ww3d-renderer-3d shadow_system::shadow_renderer::tests:: -- --nocapture` -> PASS

## 2026-03-02 Thumbnail Cache Runtime Fidelity Update

### What Improved
- Thumbnail cache files now use explicit encoded payload modes (`RAW0` / `RLE1`) with deterministic decode validation.
- Runtime load checks now validate decompressed pixel payload against expected RGBA thumbnail size (`width * height * 4`) rather than compressed byte count.

### Why It Matters
- Prevents silent acceptance of malformed compressed thumbnail payloads.
- Enables actual thumbnail cache size reduction behavior while keeping deterministic round-trip correctness.

### Validation
- `cargo test -q -p ww3d-renderer-3d texture_system::texturethumbnail::tests:: -- --nocapture` -> PASS

## 2026-03-02 Scripting Runtime Parity Update

### What Is Good
- GameLogic script runtime now handles C++ sequential wait patterns more faithfully:
  - wait/retry script actions recheck on the next frame,
  - framecount actions wait before advancing to the next sequential instruction.
- Team-iterated one-shot script behavior now matches C++ for false-action paths (no one-shot deactivation there).
- `TEAM_GUARD_FOR_FRAMECOUNT` dispatch now mirrors C++ behavior.

### What Was Bad (Now Fixed)
- Sequential `Pending(1)` behavior effectively retriggered every two frames.
- Framecount pending actions replayed the same instruction instead of delaying next-instruction progress.
- Team-iterated false-action one-shot handling incorrectly deactivated scripts.

### Still Missing / Not Yet Perfect
- Full mission/campaign parity still needs continued closure across remaining scripting edge cases and subsystem integration paths.
- Multiplayer/network scope remains deferred until all non-network parity targets are closed.

### 2026-03-02 Script Install Parity Improvement
- Script runtime initialization now matches C++ behavior when script lists are installed:
  - delayed-evaluation scripts start with randomized initial evaluation offset,
  - condition team inference is precomputed from team-typed condition parameters.
- This reduces mission-script burst/evaluation cadence drift and improves per-team condition targeting consistency.

### 2026-03-02 Subroutine Execution Parity Improvement
- `CALL_SUBROUTINE` now executes against live scripts/groups, preserving one-shot deactivation and script runtime state across repeated calls.
- Subroutine group names are now resolved before script-name fallback, aligning runtime behavior with C++ mission scripting.

### 2026-03-02 Victory Elimination Parity Improvement
- `system/victory_conditions.rs` no longer has a hardcoded elimination placeholder.
- Elimination checks now honor C++ multiplayer defeat semantics (`NOUNITS`, `NOBUILDINGS`, or both) through explicit flags.
- Legacy auto-elimination now also consumes live runtime player census (`has_any_units`, `has_any_buildings_counts_for_victory`) when available.
- Added targeted regression tests for:
  - default combined flag behavior,
  - single-flag elimination behavior,
  - no-flag behavior,
  - census-driven player auto-elimination.

### 2026-03-02 Remaining Limitation (This Lane)
- Full end-to-end elimination parity in this lane still depends on consistent caller usage of the system-layer elimination path in all relevant game flows.
- Safety fallback remains intentionally non-destructive for entries missing runtime census.

### 2026-03-02 Startup Minimap Parity Improvement
- Startup minimap generation is no longer a no-op in `system/game_start.rs`.
- `MinimapGenerator::generate_from_heightmap` now emits deterministic terrain minimap pixels from loaded map height data with:
  - elevation normalization,
  - source-to-minimap resampling,
  - fixed-light terrain shading.
- `GameStartSequence::generate_minimap` now forwards source map dimensions to the generator.

### 2026-03-02 Remaining Limitation (Startup/UI Lane)
- Minimap terrain layer currently covers height-derived base shading only.
- Full parity overlays still need additional closure for richer static map-feature layers beyond roads (for example bridge/special-map annotation depth) and finer dynamic reveal detail fidelity.

### 2026-03-02 Terrain Roads Parity Improvement
- `GameClient/src/terrain/roads.rs` no longer forces road normals to world-up.
- Road geometry now computes tangent-based right/normal frames for:
  - road surface mesh,
  - edge strips,
  - marking strips.
- This improves sloped-road lighting parity and avoids degenerate-normal artifacts on steep segments.

### 2026-03-02 Remaining Limitation (Terrain Lane)
- Additional terrain-texture parity and higher-fidelity water shading (foam/shoreline material response depth) remain open.
 forces road normals to world-up.
- Road geometry now computes tangent-based right/normal frames for:
  - road surface mesh,
  - edge strips,
  - marking strips.
- This improves sloped-road lighting parity and avoids degenerate-normal artifacts on steep segments.

### 2026-03-02 Remaining Limitation (Terrain Lane)
- Additional terrain-texture parity and higher-fidelity water shading (foam/shoreline material response depth) remain open.
