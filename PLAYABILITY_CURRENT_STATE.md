# GeneralsRust Playability State (2026-04-02)

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
2. `update_mines_and_demo_traps` each frame:
   - proximity: enemy (not ally) in trigger range → area damage + destroy mine/trap
   - timed: absolute frame deadline → detonation
   - manual: `manual_detonate_mine` for demo-trap command residual
   - **dozer/worker clear**: Worker/Dozer within clear range (5) of enemy/neutral mine
     → `clear_mine_internal` (DAMAGE_DISARM residual) destroys mine with no splash;
     clearers never proximity-detonate; idle clearers approach within scan range (100)
3. `SpecialPowerType::ClusterMines` via `DoSpecialPower` places residual mine ring
   (not full OCL ClusterMinesBomb / GenerateMinefieldBehavior density).
4. Honesty counters: places / proximity / timed / manual detonations / clears;
   `honesty_mine_place_trigger_ok` / `honesty_timed_demo_charge_ok` / `honesty_mine_clear_ok`.
5. Tests (not log-only):
   - `mine_residual_place_enemy_triggers_damage`
   - `mine_residual_ally_does_not_trigger_land_mine`
   - `demo_trap_residual_proximity_detonates_on_enemy`
   - `timed_demo_charge_residual_detonates_after_delay`
   - `cluster_mines_special_power_places_mines`
   - `demo_trap_manual_detonate_residual`
   - `dozer_mine_clear_residual_disarms_enemy_mine_safely`
   - `dozer_mine_clear_residual_approaches_then_clears`
   - `dozer_mine_clear_residual_skips_ally_mine`
   - `dozer_mine_clear_residual_infantry_still_triggers`
   - unit tests in `host_mines.rs`

**Still residual (fail-closed, not claimed):**
- Full C++ MinefieldBehavior virtual-mine regen / scoot / multi-slot immunity
- Full DemoTrapUpdate weapon-slot mode matrix / PreAttack scoop animation
- Full WEAPONSET_MINE_CLEARING_DETAIL / Weapon AntiMine targeting matrix
- Full StickyBombUpdate attach bones / geometry-based secondary splash
- Full OCL ClusterMinesBomb aircraft delivery path
- BoobyTrap install-on-building residual (related StickyBomb path)
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
- Black Lotus special capture path parity beyond hero gate residual
- Network capture replication (network deferred)
- Full ActionManager canCaptureBuilding edge matrix (stealthed, garrison, etc.)

## Residual Host Playability — Campaign SinglePlayer Path (2026-07-12)
**Closed (host-testable campaign residual):**
1. `golden_campaign` / `golden_campaign_gate` — SinglePlayer start, CampaignManager
   start/complete, mission `victory_rule` override (`nounits` via
   `victory_rules_for_map`), host-safe map `load_map` (Lone Eagle), logic frames
   advance, mission script counter ticks without panic.
2. Real campaign map **script decode**: MD_USA01 `load_map_scripts` → 291 scripts
   proven on residual path (`campaign_scripts_resolved` / `script_count`).
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
   - `GEN_CAMPAIGN_FULL_LOAD=1` loads MD_USA01 and flips
     `retail_campaign_map_loaded=true`; default gate stays host-safe (fast).
6. Honesty flags:
   - `campaign_playable_claim=true` — SP path advances with scripts/victory (not full
     retail mission playthrough)
   - `retail_campaign_map_loaded` — true under `GEN_CAMPAIGN_FULL_LOAD=1` when
     retail MD_*/GC_* `load_map` succeeds; false by default (host-safe map)

**Still residual (fail-closed, not claimed):**
- Default gate still uses host-safe map (Lone Eagle) for speed; full retail load is
  opt-in (`GEN_CAMPAIGN_FULL_LOAD=1`) rather than always-on
- Dense campaign script evaluation is budgeted (24/frame when ≥48 scripts), not
  full same-frame C++ parity for all 291 scripts
- End-to-end mission objective completion / cinematic / score-screen campaign flow
- Campaign.ini → Main `CampaignManager` mission table (GameClient manager already
  loads INI; Main save_load manager still uses seeded definitions)

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
- Multi-missile SCUD barrage timing, particle-cannon continuous beam / uplink sequence
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
