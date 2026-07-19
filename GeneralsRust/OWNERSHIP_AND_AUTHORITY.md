# Ownership and Authority Policy

> Borrow first, stable IDs second, `Arc` only at real sharing boundaries.

Preserve C++ **behavior**. Do not preserve C++ **pointer ownership**.

## Authoritative simulation (production surface, 2026-07-14)

```
OS input → normalized commands → Main GameLogic (30 Hz host sim)
  → host_* logs (damage/economy/spawn/destroy/attack)
  → GameWorldShadow session (always-on) → WorldMutations last-writer
  → host writeback (HP/cash/pose/targets)
  → PresentationFrame built from host, then shadow overlay (HP/pose/economy/power)
  → GameClient / audio / renderer  (draw path is presentation-only; no live &GameLogic)
```

| Concern | Production default | Opt out |
|---------|-------------------|---------|
| Dual `gamelogic` crate tick | **off** (`AuthorityOnly`) | `GENERALS_ALLOW_DUAL_TICK` |
| Shadow session | **on** | `GENERALS_GAMEWORLD_SHADOW=0` |
| HP last-writer (damage auth) | **on** | `GENERALS_GAMEWORLD_DAMAGE_AUTHORITY=0` |
| Cash last-writer (economy auth) | **on** | `GENERALS_GAMEWORLD_ECONOMY_AUTHORITY=0` |
| Attack target channel (shadow↔host) | **on** with shadow session | — |
| Move destination channel (shadow↔host) | **on** with shadow session | — |
| `engine_object_id` bridge | **off** unless dual/bridge env | `GENERALS_BRIDGE_ENGINE_OBJECTS` |
| Draw path `&GameLogic` | **removed** — `RenderPipeline::execute` / selection / terrain helpers are presentation-only | — |
| Full `GameClient::update()` | **not** called (`draw_display` dual-own) | shell polls input+audio |

- Target end state: `gamelogic::GameWorld` sole host; Main = composition root only.
- Current honesty: Main still owns mid-frame AI/path/combat *execution*; GameWorld is last-writer for HP/max_health/experience/weapon_bonus/active_weapon_slot/entity_power/turret/target_location/detector/continuous_fire/guard/ai_attitude/weapon_set/overcharge/contain_capacity/hive_slaves/stealth_flags/overlord_addon/command_set/disguise/vision_camo/weapon_stats/movement/cash/power/radar/shared-SP-cooldowns/alive/bounty/rank/pose/targets/production/construction/completed_upgrades/status (stealth/disable/emp/jam/mask/disguise/move/attack/select) via writeback + SetCombatStatus mutations (host_status_log selection/attack/move/fire/aim/stealth/detect/emp/jam/hack/unmanned/paralyze/subdue/mask/disguise/no_collisions/private_captured/disguise_transition/faerie/booby/parachute/force_attack/using_ability/deployed/construction/veterancy/production_queue/owner/construction_pct/special_power+cooldown/active_weapon_slot/stored_supplies/ai_state/contain/player_radar/player_progress/player_meta/completed_upgrades/max_health/experience/weapon_bonus) + presentation overlay.
- Draw/render path is presentation-only (no live `Option<&GameLogic>` dual-read on execute/collect/selection/terrain).

## Ownership rules

1. **Owned values + borrowing** — objects/players/teams live in the world; phases take `&mut GameWorld` / `&mut GameLogic`.
2. **Stable IDs** — `ObjectId` / `EntityId` / `PlayerId`; resolve only for the duration of an operation.
3. **`Arc<T>`** — immutable shared resources (templates, meshes, audio buffers).
4. **`Arc<Mutex/RwLock>`** — only true thread boundaries (asset load, audio device, capture).
5. **Channels** — async I/O boundaries; not for splitting the logic frame into Tokio tasks.

## Key types in-tree

| Type | Location | Role |
|------|----------|------|
| `GameLogic` (Main) | `Code/Main/src/game_logic` | Temporary match host (owned HashMaps) |
| `PresentationFrame` | `Code/Main/src/presentation_frame.rs` | Immutable client feed after logic step |
| `GameWorld` + `WorldMutation` | `GameLogic/src/world` | Borrow-first crate target API |
| `AuthorityProbe` | `authoritative_world.rs` | Gate checkpoints |
| `SkirmishMatchConfig` | `skirmish_config.rs` | UI → match rules/slots |

## Migration order

1. PresentationFrame consumers (render/HUD) — stop locking live sim during GPU passes.
2. Unify IDs across Main and crate.
3. Flip crate `HashMap<ObjectID, Arc<RwLock<Object>>>` → owned store + IDs.
4. Retire ObjectFactory dual registry / `engine_object_id` bridge.
5. Rebind GameClient to Main authority + snapshot only.
6. Promote `gamelogic` as sole authority; Main = event loop.


## Golden / executable honesty (2026-07-14)

| Gate | Honesty |
|------|---------|
| `golden_skirmish_gate` | Map path `playable_claim` requires pure march (`combat_no_teleport_ok`) + retail-ish speed; teleport pull only if `GOLDEN_ALLOW_TELEPORT_PULL=1` |
| `executable_smoke_gate` | Real binary Menu→InGame via `click_skirmish_start` (SkirmishMenu Start button residual) + select/move; **not** full WND widget tree (`playable_claim=false`; `skirmish_start_click_ok` honesty) |
| `shell_smoke_gate` | Headless host skirmish stack; not windowed WND |

## Frame phase order (InGame logic tick)

1. `GameLogic::update` (AI, production, movement, combat modules mid-frame)
2. Dual-crate tick (**off** by default)
3. Projectiles + pathfinding side systems
4. `process_commands` (player/AI command queue)
5. `GameWorldShadow` session (drain logs → mutations → HP/cash/pose/target writeback)
6. `PresentationFrame::build_from_logic` + shadow overlay
7. Client/HUD consumers of snapshot

Production enqueue records `host_production_log (enqueue + complete)`; completions spawn via `host_spawn_log`.

## Presentation residual (unit mesh)

When `PresentationFrame` is set, engine passes `game_logic: None` into `RenderPipeline::execute`. `collect_render_items` drives the main unit mesh pass from `unit_render_inputs` only. Minimap base / map roads / runtime heightmap helpers take `Option<&GameLogic>` and prefer `PresentationWorldEnv` (bounds, height samples, road segments) when the frame is set (`debug_last_live_unit_identity_reads == 0`). Live `game_logic.get_objects()` remains only for boot/loading frames without a snapshot. Terrain/prewarm prefer frozen `PresentationWorldEnv` and fall back to live map metadata if absent.

## GameWorld last-writer surface (current)

| Concern | Last writer | Host still executes |
|---------|-------------|---------------------|
| HP / destroy | GameWorld mutations + writeback | mid-frame combat apply then log |
| Supplies / power | GameWorld economy mutations | spend/gain then log |
| Attack target | shadow SetAttackTarget + writeback | set_target / AI launch_attack |
| Move destination | shadow SetMoveTarget + writeback | move_to / pathfinding |
| World pose (render) | shadow overlay / SetTransform | path integration mid-frame |
| Veterancy level | GameWorld SetVeterancy + writeback | gain_experience / set_min then log |
| Production enqueue | GameWorld SetProductionQueue + writeback | enqueue_production then log |
| Team / owner | GameWorld TransferOwner + writeback | set_team then host_owner_log |
| Construction % | GameWorld SetConstruction + writeback | dozer build progress then log |
| Special power ready | GameWorld SetSpecialPower + writeback | host ready flag then log |
| Stored supplies | GameWorld SetStoredSupplies + writeback | gather/cargo then log |
| AI state | GameWorld SetAiState + writeback | set_ai_state then log |
| Contain/garrison | GameWorld SetContain + writeback | enter/exit then log |
| Player radar | GameWorld SetPlayerRadar + writeback | add/remove radar then log |
| Player rank/bounty | GameWorld SetPlayerProgress + writeback | skill/bounty then log |
| Player sciences/alive | GameWorld SetPlayerSciences/Alive + writeback | unlock/defeat then log |
| Victory / match-over | shadow probe + runtime-host status | `evaluate_victory_condition` on host |
| AI decisions / path step / projectile integrate | host `GameLogic::update_simulation` | GameWorld shadow last-writer |
| OBJECT_REGISTRY pose/HP reads | disabled unless bridge env | only if `engine_object_id` + bridge on |

## OBJECT_REGISTRY residual

Main `Object` methods (`is_alive`, HP%, pose get/set) and mid-frame skips in `update_movement` / combat consult `gamelogic::OBJECT_REGISTRY` **only** when `engine_object_bridge_enabled() (also gates `GameLogic::reset` factory clear and map-ground registry writes)` (`GENERALS_BRIDGE_ENGINE_OBJECTS` / dual-tick). Default production path is host-owned fields only — no `Arc<RwLock<Object>>` dual-world reads.

The gamelogic crate still stores factory objects behind `Arc<RwLock<_>>` for legacy ObjectFactory compatibility; that surface is not on the default host match path.

## Still Main mid-frame (not sole GameWorld)

Host `GameLogic::update_simulation` now owns path follow, projectile drain/step, combat fire, and AI (`update_ai` + `THE_AI` / skirmish AI manager). Engine no longer mid-frame double-steps path or a dual `CombatSystem`.

Remaining engine residual after host update:
- GameWorld shadow session (last-writer HP/cash/pose/targets/move; not production sole authority)
- Presentation build + client/render orchestration
- Input translation and audio device ownership

`gamelogic::GameWorld` is still not the sole production authority. AI `launch_attack` prefers `set_target` (host_attack_log) plus move so the shadow attack channel sees AI aggression.

### Path following consolidation (2026-07-14)

`GameLogic::update_movement` is the sole path-follower for host objects.
Engine mid-frame `pathfinding_system.move_unit_along_path` was removed to stop
double-stepping after `GameLogic::update`. Engine may still hold a
`PathfindingSystem` for map-grid rebuild helpers; it is not the per-frame mover.


## Gates (honest reading)

| Gate | Proves |
|------|--------|
| `playability_audit` | File mapping only — **not** playability |
| `map_frame_gate` | Logic frames advance (map optional unless assets present) |
| `shell_smoke_gate` | SkirmishMenu→config→apply→map→dual-tick PresentationFrame→HUD selection/minimap→ControlBar.wnd ensure→shell→InGame screen (headless host path; not windowed WND/GPU) |
| `golden_skirmish_gate` | Host vertical slice via AttackObject/update_combat only — no take_damage fallback, no HP caps after spawn |
| `breadth_gate` | Category API smokes |
| `release_candidate_gate` | Soak + presentation smoke + campaign hooks |
| `behavior_gate` | Composite of map+golden+breadth+ai+shell+RC — use this for behavior CI |



### Projectile step consolidation (2026-07-14)

`GameLogic::update_simulation` drains `PENDING_PROJECTILES` into the host
`CombatSystem` and steps `update_projectiles`. Engine no longer owns a second
mid-frame projectile CombatSystem. Presentation freezes `logic.combat_system()`.


### Post-AI command flush (2026-07-14)

`GameLogic::update_simulation` drains `command_queue` twice per frame:
1. Phase 5 (pre-object) — player commands for this frame
2. Phase 8b (post-AI) — commands queued by `update_ai` / `ai_manager`

Engine no longer calls `process_commands` after host update. Shadow/presentation
see same-frame AI orders without a second engine drain.


### GameWorld shadow (2026-07-14)

`gameworld_shadow` maintains a `GameWorldShadow` session: stable host `ObjectId`→
`EntityId` map, delta sync (health/transform/economy), and `WorldMutation` damage
parity (`queue_damage_for_host` / `apply_pending`). Opt-in runtime: `GENERALS_GAMEWORLD_SHADOW=1` holds a session on `CnCGameEngine`.
`Object::take_damage_from` records `host_damage_log / host_heal_log / host_owner_log` events drained each tick.
Spawn/destroy: `host_spawn_log` / `host_destroy_log` drained each tick; shadow maps spawns and applies Destroy mutations. `host_spawn_log` applies via `WorldMutation::Spawn` (sole shadow create) then host→entity map.

Production log: `drain` retains `last_drain_snapshot` for PresentationEvent::ProductionComplete after shadow session.

Presentation: when engine holds a shadow session, `PresentationFrame` is built from host then `overlay_gameworld_shadow` so HP/pose/supplies prefer GameWorld.
Move channel: `SetTransform` mutations + `apply_host_positions_as_transforms`.

Shadow session **defaults on** in the engine (`GENERALS_GAMEWORLD_SHADOW=0` to disable). Entity.attack_target mirrors host Object::target; SetAttackTarget mutation available.

**Production defaults (2026-07-14):** shadow session, damage authority, and economy authority are **on** when env unset. Opt out with `=0` / `false`. `host_attack_log` from `Object::set_target` → SetAttackTarget each tick.

Still not sole GameWorld production authority (host Main GameLogic remains match host; shadow is last-writer overlay).

### Damage authority cutover (opt-in)

Gates call `ensure_gate_damage_authority()` so damage authority defaults on (set `GENERALS_GAMEWORLD_DAMAGE_AUTHORITY=0` to opt out). `GENERALS_GAMEWORLD_ECONOMY_AUTHORITY=1` (gates default-on): `host_economy_log (includes power from `update_player_resources`; income via `Player::credit_supplies / steal_cash_from_team`)` from Player spend/add/bounty/refund → SetSupplies/SetPower mutations → host writeback. `GENERALS_GAMEWORLD_DAMAGE_AUTHORITY=1` (implies shadow session): end-of-tick
reapplies `host_damage_log` as `WorldMutation`s on `GameWorldShadow` and
**writebacks** HP/destroyed onto Main objects. GameWorld is last writer for HP;
mid-frame host combat still runs for C++ armor/side-effect parity.

 Opt-in: `GENERALS_GAMEWORLD_SHADOW=1`.
Not production authority — first migration slice toward retiring Main stores.

### Presentation boundary residual (2026-07-14)

When `PresentationFrame` is set, render prefers snapshot for:
- unit mesh identity (position/model/FOW)
- lighting / shell flag / world bounds / heightmap hint / prewarm signature
- minimap base heights via coarse `PresentationWorldEnv` height grid (64×64)

Still live-`GameLogic` residual during execute (acceptable fail-closed):
- Live unit fallback when no presentation frame (boot/loading)
- Full heightmap GPU payload (`terrain_heightmap_snapshot`) — too large for per-frame freeze;
  bounds/hint prefer presentation; load is map-start only
- Road/bridge bake prefers presentation segments; live snapshot only if env empty
- Startup model prewarm prefers capped `prewarm_template_names` on presentation
- Asset/mesh resolve (filesystem/GPU, not sim identity)
- `engine_object_id` / OBJECT_REGISTRY bridge remains opt-in (`GENERALS_BRIDGE_ENGINE_OBJECTS`);
  combat path keeps Main objects authoritative without dual registry by default

Executable smoke (`executable_smoke_gate` / behavior_gate step 8) boots the real
`generals` binary via runtime host and proves Menu→InGame. `playable_claim`
remains false (not full WND widget click / retail GPU match playthrough).

### Honest reading (do not overclaim)

- **Proves**: single-host GameLogic authority, skirmish config propagation, production command/combat/save APIs, presentation snapshot fields, retail map load when assets exist.
- **Does not prove**: windowed shell/WND navigation, full GPU match playthrough, complete GameWorld migration, full SAGE shroud parity, or full W3D material/animation retail (unit *identity* + unit FOW + compact local FOW *grid snapshot* + CPU mesh key resolve/placeholder honesty are presentation/asset residuals closed; GPU terrain FOW pass / dirty-rect streaming / full archive deferred prewarm / retail GPU draw match remain residual).

Gate honesty labels:

| Gate field | Meaning |
|------------|---------|
| `playable_claim=false` | Must not be read as “retail match is playable end-to-end” |
| shell `playable_claim` | **Always false** — headless APIs ≠ retail W3D/windowed playthrough (fail-closed pending GPU/WND) |
| shell `shell_host_playable_ok` | Limited claim: headless shell→config→map→dual-tick presentation→HUD selection/minimap→ControlBar.wnd ensure→InGame screen is operational. **Not** full retail play |
| shell `control_bar_layout_ok` | ControlBar.wnd resolve/validate ran (Ready, or honest AssetsUnavailable when WindowZH missing) |
| shell `control_bar_path_resolved` / `control_bar_wnd_validated` | Path found + structural FILE_VERSION/WINDOW/ControlBar sniff |
| shell `control_bar_window_loaded` / `control_bar_window_count` | Headless WindowManager parse of ControlBar.wnd when assets present (not windowed W3D) |
| shell `selection_consumers_ok` | Dual-tick selection panel applied to HUD + UIState + RTS + unit command panel |
| shell `dual_tick_presentation_ok` | Seed + logic update + multi-consumer presentation apply order after StartGame |
| shell `minimap_fow_presentation_ok` | Presentation FOW grid residual usable for minimap texture path |
| shell `laser_segment_upload_ok` | Presentation → CPU SegLine pack residual (empty + synthetic geometry) |
| `synthetic_combat=true` (golden) | Combat/victory on synthetic host world, not Lone Eagle armies |
| `ai_disabled_for_slice=true` (golden) | Opponent AI off so rebuilds do not mask combat failure |
| shell `host_constructed` | True only after `apply_skirmish_config` succeeds (not a constant) |

### Shell residual notes (2026-07)

Closed toward shell_smoke honesty without overclaiming retail W3D:

1. **Dual-tick after StartGame** — map load seeds PresentationFrame; each smoke frame does logic update then `build_and_apply_for_hud` (parity with `start_game_from_ui` / engine dual-tick order). Host-testable flag: `dual_tick_presentation_ok`.
2. **HUD selection + minimap from presentation** — selection panel health and minimap unit identity come from the snapshot, not live object re-read in the HUD apply path.
3. **Minimap FOW from presentation** — `PresentationFrame.fow_grid` residual usable for minimap R8 path (`minimap_fow_presentation_ok`); production minimap prefers presentation grid over live shroud lock.
4. **ControlBar.wnd ensure + headless WindowManager load** — shell_smoke calls `control_bar_layout_honesty(true)` (with `game_client`) on shell→Loading→GameHUD: structural validate (FILE_VERSION / WINDOW / ControlBar) and headless `WindowManager::load_window` when WindowZH is present. Flags: `control_bar_layout_ok`, `control_bar_path_resolved`, `control_bar_wnd_validated`, `control_bar_window_loaded`, `control_bar_window_count`. AssetsUnavailable remains honest without WindowZH.
5. **WGPU laser segment upload residual (CPU pack)** — presentation freezes assist laser Line3D segments; `LaserSegmentUpload` packs interleaved vertex bytes + honesty; synthetic assist-pair exercises non-empty geometry. Flag: `laser_segment_upload_ok`. Fail-closed: not live `Queue::write_buffer` / texture sample.
6. **Screen ownership** — UIManager exercises Skirmish → Loading → GameHUD; pregame shell ownership flags are checked for real screen values.
7. **Multi-consumer selection panel** — dual-tick `build_and_apply_for_shell_consumers` feeds GameHUD + GameUIState + RTSInterface + UnitCommandPanel (+ ControlBar when game_client). Flag: `selection_consumers_ok`.
8. **`shell_host_playable_ok` vs `playable_claim`** — success sets the limited host flag; `playable_claim` stays false so gates cannot be misread as “windowed retail match is playable.”

Still residual (not claimed by shell_smoke):

- Windowed shell/WND navigation and GPU match playthrough
- Live ControlBar.wnd draw callbacks / image assets / gadget interaction (headless tree only)
- Live WGPU SegLineRenderer buffer write + EXBinaryStream32.tga sample for assist lasers
- Full W3D retail match playthrough (GPU present, mesh assets, drawables)

### Combat particle residual notes (2026-07)

**Closed (host combat feedback registry — not full W3D GPU particles):**

1. Main `GameLogic` owns `CombatParticleRegistry`. Weapon fire and death create
   registry entries (template/position/id) that are not log-only.
2. `PresentationFrame.particle_systems` + `PresentationEvent::ParticleSystemSpawned`
   are the observe path for client/HUD after a logic step.
3. Optional mirror into GameClient `ParticleSystemManager` via combat presets
   (`MediumExplosion`, `SmokePlume`, `MuzzleFlash`, `BulletImpact`).
4. Tests prove kill/fire produce registry entries and presentation snapshot.

**Still residual (fail-closed):**

| Residual | Why still live |
|----------|----------------|
| Full W3D particle GPU / compute | Main effects GPU manager + WW3D particle crates |
| Full FXList.ini / ParticleSystems.ini retail set | INI load + nugget coverage |
| Bone attach / slave cascade for combat residual | Drawable bone query + ParticleSystem slaves |
| Network particle replication | Network deferred |

### Combat audio residual notes (2026-07)

**Closed (host combat fire/death → audio request path — not full Miles retail):**

1. Host `GameLogic::update_combat` queues `AudioEventRequest("WeaponFire")`
   with object id + muzzle position when a weapon slot fires.
2. Host `process_destroy_list` queues `UnitDie` / `BuildingDie` with corpse
   position when an object is destroyed.
3. `process_audio_events` still drains into `AudioManagerSubsystem` (same as UI
   cues). Fail-closed: request exists even when SoundEffectsTable has no path.
4. FXList `SoundFXNugget` falls back to `dispatch_positional_sound` when the
   GameClient FX audio hook is not registered (was silent no-op).
5. Tests: `combat_fire_queues_weapon_fire_audio_event`,
   `combat_kill_queues_unit_die_audio_event`.

**Still residual (fail-closed):**

| Residual | Why still live |
|----------|----------------|
| Per-weapon INI FireSound names on host combat path | Host Weapon lacks fire_sound field; uses generic WeaponFire |
| Full Miles / device handle parity | Audio device backend + event info tables |
| Spatial attenuation / shroud for combat residual | GameAudio locality + shroud resolvers |
| Death scream / voice bank selection by unit type | ThingTemplate audio fields + speech channel |

### Presentation unit-render residual notes (2026-07)

**Closed (unit identity for main mesh pass):**

1. `PresentationFrame` / `RenderableObject` owns position, orientation, team,
   `team_color`, template/model key, selected, aliveness (`destroyed`),
   `selection_radius`, and `engine_bridged` (RenderBridge skip without live re-read).
2. Production `RenderPipeline::collect_render_items` prefers
   `PresentationFrame::unit_render_inputs()` for the main unit mesh pass when a
   frame is set — no live `GameLogic` object transform/model/selected re-read.
3. Selection overlay (`collect_selected_units_from_presentation`) and HUD/ControlBar
   already consume snapshot identity.
4. Tests: presentation build includes positions/model/team; render collection helper
   builds unit inputs from frame after live world mutation (no logic re-read).

**Closed (unit FOW for main mesh pass — partial, fail-closed vs full SAGE FOW):**

1. `PresentationFrame` freezes per-object FOW for `local_player_id` at build time
   (`RenderableObject.fow_visibility` + `fow_shell_bypass` from `isInShellGame`).
2. `UnitRenderInput` carries snapshot FOW; never-explored skip and fog alpha apply
   from the frame without mid-render shroud / ownership re-query.
3. Production collect uses presentation FOW when a frame is set; live FOW bridge
   remains only for the boot/loading path (no presentation frame).
4. Tests: FOW matches FOW bridge at build; unit inputs stay frozen; shell bypass
   forces fully visible; never-explored / fogged encode states.

**Closed (cell-grid FOW snapshot for terrain / minimap — partial, fail-closed):**

1. `ShroudManager::snapshot_grid_for_player` / `grid_dimensions` export a compact
   local-player cell buffer (`0=Hidden`, `1=Explored`, `2=Visible`).
2. `PresentationFowGrid` on `PresentationFrame` freezes that grid at build time
   (shell bypass / shroud-inactive → fully visible fail-open).
3. R8 encoding (`0/128/255`) via `PresentationFowGrid::to_r8_texture` /
   `PresentationFrame::terrain_fow_r8` for `FowTerrainOverlay::update_texture`.
4. Minimap regenerate prefers presentation grid when active
   (`update_texture_from_fow_with_grid`) so GPU upload does not re-lock shroud
   mid-render; live shroud remains boot fallback.
5. Tests: grid matches bridge at build; stays frozen after live reveal; dual-build
   fingerprint; R8 encode; shell overlay inactive.

**Closed (W3D mesh asset resolve — partial, fail-closed):**

1. `assets::mesh_asset_resolve` maps presentation `model_key` /
   `ThingTemplate::get_model_name` → canonical W3D key (aliases: `airanger` /
   `USA_Ranger` → `airanger_s` for shipped `AIRanger_S.W3D`).
2. Resolve order: GraphicsSystem cache → AssetManager load → filesystem
   extracted/sample W3D → honest placeholder cube (`__fallback_cube__`) with
   `MeshResolveHonesty` counters (loaded / placeholder / missing).
3. `RenderPipeline::ensure_render_model_loaded` uses alias remap + residual
   filesystem resolve; placeholder Ready only when missing-model debug cubes
   are opt-in (production fail-closed for silent retail substitution).
4. PresentationFrame freezes aliased model_key so unit mesh pass and residual
   resolve share the same key without re-reading ThingTemplate.
5. Tests: USA_Ranger non-empty key; airanger alias; placeholder honesty;
   load AIRanger_S when assets present, graceful skip when not.

**Closed (W3DLaserDraw Line3D segment presentation + CPU WGPU pack — partial):**

1. Host residual still builds `HostLaserLine3DSegment` descriptors (arc / tile / skim).
2. `PresentationFrame.laser_beams / projectiles` freezes active Patriot assist lasers + Line3D segments
   at build time (`PresentationLaserBeam` / `PresentationLaserSegment`).
3. `graphics::laser_segment_upload::LaserSegmentUpload` packs interleaved CPU vertices
   (pos + uv + color) ready for WGPU buffer write; honesty flags for empty/geometry/upload-ready.
4. `RenderPipeline::pack_presentation_laser_segments` prefers the presentation frame.
5. shell_smoke exercises empty pack + synthetic assist-pair geometry (`laser_segment_upload_ok`).
6. Fail-closed: not live SegLineRenderer `Queue::write_buffer` / texture bind / soft multi-beam.

**Still residual (not claimed as full presentation-only renderer / SAGE FOW):**

| Residual | Why still live / other system |
|----------|-------------------------------|
| Full SAGE dirty-rect / multi-layer shroud streaming | Full grid copy only; no partition dirty-rect queue |
| GPU terrain FOW overlay pass wired every frame | `FowTerrainOverlay` exists; production pass still residual |
| Live WGPU laser SegLine write + EXBinaryStream32 sample | CPU pack residual closed; device queue write residual |
| Stealth detection FOW variants mid-pass | Live stealth managers when presentation absent |
| Terrain / heightmap / skybox / roads | Map/environment systems, not unit identity |
| Deferred model-load budget / full archive prewarm | Startup budget path still incremental |
| Full W3D material / animation / GPU buffer parity | CPU mesh parse + cache only; not retail draw match |
| RenderBridge drawable path | engine-bridged units drawn outside main mesh pass |
| Full retail W3D GPU match / full SAGE FOW | Fail-closed: not claimed by this residual close |

### Drawable residual notes (2026-07)

**Closed (model condition bits from body damage):**

1. `Drawable::react_to_body_damage_state_change` (GameLogic + GameClient) matches C++
   `Drawable::reactToBodyDamageStateChange`: clear DAMAGED/REALLYDAMAGED/RUBBLE,
   set the bit for the new `BodyDamageType` (Pristine clears all three).
2. `ActiveBody::evaluate_visual_condition` now calls `react_to_body_damage_state_change`
   with `m_curDamageState` instead of the non-parity `update_damage_state_for_health` tint path.
3. GameClient `compute_health_region` projects object health-box through the tactical view
   (C++ `computeHealthRegion`); falls back to a seeded region for offline/tests.
4. Tests: exclusive damage bits set/cleared; non-damage flags survive; icon UI region/caption
   remain observable after `draw_icon_ui`.

**Closed (drawable shadow enable/status observability):**

1. GameClient `BasicDrawable` and GameLogic `Drawable` seed `DRAWABLE_STATUS_SHADOWS`
   on create so shadow enable is observable without a full render pass.
2. `get_shadows_enabled` / `set_shadows_enabled` match C++ status-bit semantics;
   GameClient also dispatches enable to draw modules (C++ `Drawable::setShadowsEnabled`).
3. `allocate_shadows` / `release_shadows` match C++ Options-screen hooks: notify modules
   only — they do **not** flip status bits (status is owned by `set_shadows_enabled`).
4. Model-condition / body-damage updates preserve existing shadow status.
5. GameClient `render` toggles shadows for living bound objects from stealth look
   (C++ `Drawable::draw`).
6. Tests: create-time SHADOWS bit; enable toggle; allocate/release leave status alone;
   condition change preserves status; module dispatch for enable/alloc/release.

**Still residual (fail-closed — not claimed as full drawable/animation parity):**

| Residual | Notes |
|----------|-------|
| Full W3D mesh/animation swap on condition change | Draw modules still partial; bits are authoritative input only |
| Anim2D retail icon assets for heal/bomb/etc. | Overlay flags computed; asset binding incomplete |
| Full shadow mesh GPU allocation (volumetric/projected) | Enable/status wired; `allocate_shadows` does not create GPU meshes |
| Full dual Drawable (GameLogic vs GameClient) unification | Two ports still co-exist; condition bits mirrored via body path |



With a presentation frame, GameClient uses `update_drawables_local` (no OBJECT_REGISTRY shroud bind).

- Client: `update_presentation_shell` for presentation path; soft WND via GENERALS_RUNTIME_HOST_WND.

- Presentation freezes pending radar texts as `PresentationEvent::RadarMessage` (UI drain remains authoritative).

### Executable smoke kill pattern (2026-07-14)

`executable_smoke_gate` kills stale `generals` via `pkill -f` matching
`-runtime_host` (underscore). A prior hyphenated `runtime-host` pattern never
matched the real CLI and left GPU-holding processes that failed the next boot.

`executable_host_ok` can go true (Menu→InGame via runtime host). `playable_claim`
stays false until full WND widget click / retail interactive path.

### GameClient presentation shell deepen (2026-07-14)

With a `PresentationFrame`, Main calls `GameClient::update_presentation_shell` which
now advances frame tick, local drawable modules, particle local-player index, FX/
weather residual, script display state, post-draw UI, beacon notifications, and
message pump — still **without** `OBJECT_REGISTRY` shroud binds or Main-owned
input/audio/3D draw. Full `GameClient::update()` remains disconnected.

### Presentation prewarm residual (2026-07-14)

`RenderPipeline::prewarm_startup_models` uses `PresentationWorldEnv.prewarm_template_names`
when a presentation frame is installed. Empty list is fail-closed (no live
`last_parsed_map_settings` dual-read). Live metadata remains boot/loading-only
when `execute(..., game_logic: Some(_))` without presentation.

### Roads/minimap presentation fail-closed (2026-07-14)

With `PresentationFrame` installed:
- `sync_runtime_map_roads` uses snapshot road/bridge lists only (empty = no bake),
  never `terrain_road_segments_snapshot` dual-read.
- Minimap base height samples from presentation coarse grid only; live
  `terrain_height_at` is boot/loading residual when no presentation env.

Full GPU heightmap payload at map-start may still load via
`load_heightmap_from_runtime_terrain` when no file hint exists (too large for
per-frame freeze).

### Skybox presentation residual (2026-07-14)

`PresentationWorldEnv` freezes `skybox_enabled` + optional texture names.
Map-load and per-frame render apply skybox from the snapshot when present;
live `GameLogic` skybox/metadata is boot residual only.

### AI placement deterministic RNG (2026-07-14)

`AIPlayer` building scatter uses `HostRandomState` (retail ADC RandomValue residual)
seeded from `player_id`, replacing `rand::thread_rng`. Same slot/seed → same
placement draws (determinism residual; not full C++ AIData.ini personality parity).

### AIData structure/team timing residual (2026-07-14)

Host `AIPlayer` structure/team decision spacing uses retail Default `AIData.ini`
constants: `StructureSeconds=0`, `TeamSeconds=10`, `RebuildDelayTimeSeconds=30`
(documented residual). Difficulty still scales intervals. Attack launch remains
spaced by `ATTACK_RECHECK_SECONDS=60` (not full C++ scripted team production).
Early `activity_count < 3` zero-interval gate pacing removed for timing honesty.

### AIData wealth/poor rate residual (2026-07-14)

Host `AIPlayer` scales structure/team decision intervals by retail AIData
`Wealthy=7000` / `Poor=2000` and `Structures*Rate` / `Teams*Rate` (speed
multipliers → shorter waits when wealthy). StructureSeconds=0 still yields a
zero wait. Fail-closed: not full C++ wealth-mod on m_structureFrames path.

### AIData TeamResourcesToStart residual (2026-07-14)

Host `AIPlayer::should_build_new_team` / queue path now applies C++
`isPossibleToBuildTeam` money residual: estimate work-order unit costs from
templates, require `supplies >= ceil(cost * TeamResourcesToStart)` (default
0.1). Removes the activity_count first-force cash bypass. Not full TeamPrototype min/max averaging or production-condition scripts.

### AI factory-idle team gate residual (2026-07-14)

Host `is_possible_to_build_team` now mirrors C++ `isPossibleToBuildTeam` factory
half with `requireIdleFactory=true`: every work-order unit type needs a live
constructed factory, and at least one matching factory must be idle (empty
`production_queue`). `find_factory_for_unit` prefers idle then busy. Not full
C++ TeamPrototype production-condition scripts / maxInstances.

### AIData RebuildDelaySeconds residual (2026-07-14)

Host `AIBuildingInfo` tracks `destroyed_at_time` when a queued structure object
vanishes. `process_building_queue` waits `RebuildDelaySeconds=30` before starting
a rebuild (C++ BuildListInfo objectTimestamp + m_rebuildDelaySeconds). Successful
start clears the stamp. Not full C++ rebuild-hole / dozer path.

### Presentation runtime heightmap freeze (2026-07-14)

`PresentationWorldEnv.runtime_heightmap` freezes the full runtime `HeightMap`
POD at snapshot build. Map-load terrain visual bake prefers that freeze and
calls `load_heightmap_from_runtime_terrain(..., None)` so the path cannot
dual-read live `GameLogic`. Boot without presentation may still pass live logic.

### Presentation terrain texture-class freeze (2026-07-14)

`PresentationWorldEnv.terrain_texture_classes` freezes blend-tile texture
classes at snapshot build. Heightmap/source-tile bake prefers that freeze when
a presentation frame is installed (no live `terrain_texture_classes_snapshot`
dual-read). Boot without presentation may still pass live logic.

### Selection overlay presentation-only residual (2026-07-14)

`enqueue_selection_render` / `collect_selected_units` take `Option<&GameLogic>`.
InGame render passes `None` when `last_presentation_frame` is set so selection
circles use snapshot identity only. Live GameLogic remains boot residual without
a presentation frame.

### GameClient presentation shell deepen residual (2026-07-14)

`update_presentation_shell` now applies C++ visual-freeze + script visual-speed
scaling and ticks `DisplayStringManager`. Still skips Main-owned input/audio and
`draw_display` (RenderPipeline remains sole 3D draw). Full `GameClient::update()`
remains disconnected.

### CI clippy residual (2026-07-14)

`math_utilities` grid cull loops no longer use Clippy-denied explicit counter
patterns (`address += 1` over `for _ in range`). Keeps O(1) cell walk semantics.

### GameWorld construction-complete residual (2026-07-14)

`host_construction_log` is drained in the shadow session and applied via
`apply_host_construction_events` so completed structures are mapped (spawn-like
residual). Fail-closed: not full GameWorld construction-module authority.

### Skirmish WND ButtonStart residual (2026-07-14)

`click_skirmish_start` prefers retail `SkirmishGameOptionsMenu.wnd:ButtonStart`
via `GadgetSelected` when `GENERALS_RUNTIME_HOST_WND` allows shell push, then
drains NewGame immediately. Falls back to Main SkirmishMenu mouse residual.
Executable smoke enables WND when `DISPLAY` is set (xvfb). `playable_claim`
still false (not full interactive retail navigation).

### GameClient presentation shell display residual (2026-07-14)

`update_presentation_shell` now runs C++ display UPDATE + drawable icon UI
after effects, still skipping `draw_display` (Main RenderPipeline sole present).
Full `GameClient::update()` remains disconnected for input/audio dual-ownership.

### Presentation mouse world-bounds residual (2026-07-14)

`update_mouse_world_position` prefers `PresentationFrame.world_env` bounds for
screen→world click mapping when a frame is installed. Live `GameLogic::world_bounds`
remains boot residual without presentation.

### Presentation camera clamp residual (2026-07-14)

`clamp_to_world_bounds` prefers `PresentationFrame.world_env` for camera follow
and scroll clamping when a frame is installed. Live `GameLogic::world_bounds`
remains boot residual without presentation.

### Presentation minimap/radar bounds residual (2026-07-14)

`update_minimap_viewport` and radar-ping projection prefer
`PresentationFrame.world_env` when a frame is installed. Live
`GameLogic::world_bounds` remains boot residual without presentation.

### GameWorld shadow orientation sync residual (2026-07-14)

`sync_from_host_with` copies `Object::get_orientation()` into shadow `Transform`
(no longer forces yaw 0). Pose writeback / `apply_host_positions_as_transforms`
remain the last-writer path for mid-tick facing changes.

### Presentation local_team residual (2026-07-14)

`PresentationFrame` freezes `local_team` from the host player at snapshot time.
Selection hotkeys (Ctrl+A, Tab cycle, control groups, box select), pick, and
right-click attack prefer `frame.local_team()` when a frame is installed. Live
`GameLogic::get_player` team reads remain boot residual without presentation.

### Presentation select-similar local_team residual (2026-07-14)

`select_similar_units` prefers `PresentationFrame.local_team()` and
`similar_unit_ids` when a frame is installed. Live `get_player` /
`get_objects` remain boot residual without presentation.

### Presentation player roster residual (2026-07-14)

`PresentationFrame.players` freezes host player id/name/team/alive/local at
snapshot time. Defeat notifications and alliance radar prefer the roster when
a frame is installed. Live `GameLogic::get_player` remains residual without a
matching roster entry.

### GameWorld upgrade-complete residual (2026-07-14)

`WorldMutation::CompleteUpgrade` records completed host upgrade names on
`PlayerData.completed_upgrades`. Shadow session applies
`host_upgrades().completed_this_frame_snapshot()` each tick. Fail-closed: not
full PlayerUpgradeManager / science tree / unit effect matrix authority.

### Presentation victory shell-bypass residual (2026-07-14)

Victory condition evaluation prefers `PresentationFrame.fow_shell_bypass` when a
frame is installed. Live `GameLogic::isInShellGame` remains residual without
presentation.

### GameWorld science/upgrade absolute sync residual (2026-07-14)

`sync_players` copies host `unlocked_sciences` and host-registry completed
upgrade names onto shadow `PlayerData` each tick (absolute residual alongside
the CompleteUpgrade event channel). Fail-closed: not full science purchase /
rank / effect matrix authority.

### GameWorld power bar residual (2026-07-14)

`PlayerData.power_produced` / `power_consumed` mirror host energy-bar sides.
`sync_players` copies them each tick with supplies/power_available. Fail-closed:
not full power plant graph / underpower disable matrix authority.

### GameWorld radar residual (2026-07-14)

`PlayerData.radar_count` / `radar_disabled` mirror host radar providers and
script/power disable. `sync_players` copies them each tick. `any_player_has_radar`
matches C++ `hasRadar` (count > 0 && !disabled). Fail-closed: not full radar
shroud/JarmenKell/spy satellite matrix authority.

### GameWorld alive/bounty/color residual (2026-07-14)

`PlayerData.is_alive`, `cash_bounty_percent`, and `color_rgb` mirror host player
defeat/bounty/tint residual. `sync_players` copies them via
`copy_host_player_residual` each tick. Fail-closed: not full victory-conditions
module / bounty award pipeline / skirmish color table authority.

### GameWorld entity selection/construction residual (2026-07-14)

`Entity` carries `max_health`, `selected`, `destroyed`, and
`construction_percent` host residual. `sync_from_host_with` copies them each
tick. Fail-closed: not full selection-manager / build-queue module authority.

### GameWorld entity team/status residual (2026-07-14)

`Entity` carries `team_ordinal`, `selection_radius`, `under_construction`,
`moving`, and `attacking` host residual. `sync_from_host_with` copies them each
tick. Fail-closed: not full AI state machine / kindof matrix / multi-select
manager authority.

### GameWorld entity color/power/type residual (2026-07-14)

`Entity` carries `team_color`, `power_provided`, `power_consumed`,
`object_type_ordinal`, and `max_transport` host residual. `sync_from_host_with`
copies them each tick. Fail-closed: not full power-plant graph / transport
contain matrix / material palette authority.

### GameWorld entity combat-intent residual (2026-07-14)

`Entity` carries `force_attack`, `show_health_bar`, `target_location`,
`guard_position`, `guard_target_host`, `ai_state_ordinal`, and `occupant_count`
host residual. `sync_from_host_with` copies them each tick. Fail-closed: not full
AI FSM / garrison graph / guard-mode parity.

### GameWorld entity xp/status residual (2026-07-14)

`Entity` carries `experience_points`, `veterancy_ordinal`, `stored_supplies`, and
extended host status flags (`stealthed`, `detected`, `using_ability`,
`airborne_target`, `disabled_*`). `sync_from_host_with` copies them each tick.
Fail-closed: not full veterancy bonus matrix / stealth detector network /
disabled-type FSM authority.

### GameWorld entity building residual (2026-07-14)

`Entity` carries `is_building`, `building_type_ordinal`, production queue head
(`production_queue_len` / `production_progress` / `production_template`),
`rally_point`, and garrison counts. `sync_from_host_with` copies them each tick.
Fail-closed: not full ProductionUpdate / dual-queue / exit-door / garrison AI
authority.

### GameWorld entity weapon/move residual (2026-07-14)

`Entity` carries primary-weapon residual (`has_weapon`, damage/range/ammo/flags),
`has_secondary_weapon`, and movement residual (`move_max_speed`, `velocity`,
`path_len`/`path_index`). `sync_from_host_with` copies them each tick.
Fail-closed: not full WeaponSet slots / projectile spawn / loco pathfinder
authority.

### GameWorld entity transport residual (2026-07-14)

`Entity` carries `display_name`, `overlord_bunker_capacity`, passenger-fire /
weapon-set flags, transport-kind markers (battle bus / technical / combat cycle /
tunnel / combat chinook), `combat_cycle_rider`, and `contained_by_host`.
`sync_from_host_with` copies them each tick. Fail-closed: not full
OpenContain/RiderChangeContain/TunnelContain matrix authority.

### GameWorld entity detector/sp residual (2026-07-14)

`Entity` carries cheer/overcharge/weapon-slot, `guard_radius`,
`applied_upgrade_count`, special-power ready/cooldown residual, and detector /
innate-stealth flags. `sync_from_host_with` copies them each tick. Fail-closed:
not full SpecialPowerTemplate / StealthDetectorUpdate / upgrade-module authority.

### GameWorld entity combat-bonus residual (2026-07-14)

`Entity` carries weapon-bonus flags, continuous-fire/faerie-fire, extra transport
kinds/addons, demo/hive, turret angles, AI attitude, last damage source,
command-set override, disguise, vision-spied mask, and camo residual.
`sync_from_host_with` copies them each tick. Fail-closed: not full
WeaponBonusCondition / TurretAI / disguise drawable / mine module authority.

### Presentation turret/weapon-bonus residual (2026-07-14)

`RenderableObject` freezes `turret_angle_deg`/`turret_pitch_deg`/
`turret_idle_scanning` and weapon-bonus flags from host Object each frame.
Presentation consumers can read turret/bonus residual without live Object
dual-reads. Fail-closed: not full TurretAI drawable bones / WeaponBonusCondition
matrix authority.

### Unit-control select-similar presentation identity residual (2026-07-14)

`select_similar_units` prefers presentation template/team/selectable identity
when a `PresentationFrame` is installed (live GameLogic only as boot fallback).
Fail-closed: not full multiplayer observer / shrouded-template filter matrix.

### Presentation command-set/detector residual (2026-07-14)

`RenderableObject` freezes `command_set_override`, `is_detector`,
`active_weapon_slot`, `overcharge_enabled`, `show_health_bar`, and `guard_radius`
from host Object. ControlBar/UI can resolve override command sets and detector
state without live Object dual-reads. Fail-closed: not full CommandSet INI graph /
detector FOW pulse authority.

### ControlBar command-set presentation residual (2026-07-14)

`PresentationFrame::apply_to_control_bar` feeds
`ControlBar::sync_command_set_from_presentation` using the selected object's
`command_set_override` residual. Prefer this over live OBJECT_REGISTRY
`get_command_set_string` dual-reads when a frame is installed. Fail-closed: not
full multi-select intersection / prerequisite / ScriptOnly filter matrix.

### Presentation battle-plan weapon-bonus residual (2026-07-14)

`RenderableObject` freezes Strategy Center battle-plan weapon-bonus flags
(bombardment / hold-the-line / search-and-destroy). UI/FX can read plan buffs
without live Object dual-reads. Fail-closed: not full BattlePlanUpdate /
sight-scalar / plan-switch FSM authority.

### Unit-control selection center presentation residual (2026-07-14)

`get_selection_center` prefers presentation object poses when a
`PresentationFrame` is installed (live GameLogic only as boot fallback).
Fail-closed: not full camera-follow / multi-group average matrix.

### Presentation hive/continuous/camo residual (2026-07-14)

`RenderableObject` freezes continuous-fire level, faerie-fire frame, hive slave
count/hp, AI attitude, camo opacity, vision-spied mask, and cheer timer.
Fail-closed: not full Gattling spin FSM / HiveStructureBody / stealth camo FX
authority.

### Presentation transport-kind/damage residual (2026-07-14)

`RenderableObject` freezes transport-kind markers (humvee/listening outpost/
troop crawler/helix), overlord addons, demo-suicide detonating, turret holding,
and last damage source host id. Fail-closed: not full TransportContain /
OverlordContain / BodyModule damage-source FSM authority.

### Presentation detector/stealth timing residual (2026-07-14)

`RenderableObject` freezes `detection_rate_frames`, stealth-break flags,
`innate_stealth`, frenzy-until frame, continuous-fire consecutive/coast, and
battle-plan sight scalar applied. Fail-closed: not full StealthDetectorUpdate /
StealthUpdate delay FSM / Frenzy countdown authority.

### GameWorld entity combat-timing residual (2026-07-14)

`Entity` carries `weapon_bonus_frenzy_until_frame`,
`continuous_fire_coast_until_frame`, and `battle_plan_sight_scalar_applied`
host residual (alongside existing consecutive/detector/stealth fields).
`sync_from_host_with` copies them each tick. Fail-closed: not full Frenzy
countdown / Gattling coast / BattlePlan sight FSM authority.

### Construction panel command_set_override residual (2026-07-14)

`ConstructionPanel::show_for_building` accepts optional `command_set_override`
and prefers it over ThingTemplate CommandSet lookup. Presentation consumers can
feed selected `RenderableObject::command_set_override` without live template
dual-reads. Fail-closed: not full CommandSet INI cameo/prerequisite matrix.

### Simple input presentation pick residual (2026-07-14)

`SimpleInputProcessor` caches an optional `PresentationFrame` and
`find_object_at_position` prefers presentation poses/selection radii when a
snapshot is installed (live GameLogic only as boot fallback). Fail-closed: not
full FOW/team/shroud pick filter matrix.

### Input integration presentation pick residual (2026-07-14)

`InputProcessor` caches an optional `PresentationFrame` and world pick prefers
presentation poses when installed (live GameLogic only as boot fallback).
Fail-closed: not full FOW/team pick filter matrix.

### HUD UnitDisplayInfo command_set residual (2026-07-14)

`UnitDisplayInfo` freezes `command_set_override` + `can_produce` from
presentation. HUD `sync_selection_from_presentation` opens the construction
panel with override residual for producer selections. Fail-closed: not full
CommandSet INI tab population from override name.

### Presentation overlay_gameworld_shadow entity residual (2026-07-14)

`PresentationFrame::overlay_gameworld_shadow` applies expanded Entity residual
as last-writer for presentation: pose/HP/selection/construction/status,
team_color, weapon range/damage, stealth/detector flags, force_attack,
show_health_bar, and command_set_override. Fail-closed: not sole GameWorld
authority (host still builds the base freeze).

### Presentation overlay residual deepen (2026-07-14)

`overlay_gameworld_shadow` last-writes remaining Entity residual onto
`RenderableObject`: power, XP, supplies, guard/rally, SP cooldown, detector
timing, weapon-bonus/battle-plan/continuous-fire, transport kinds, hive/turret,
disguise/camo/vision. Fail-closed: host still base-freezes presentation.

### GameWorld path waypoints + overlay complete residual (2026-07-14)

`Entity` carries capped `path_waypoints` and secondary weapon range/damage.
`sync_from_host_with` copies them; `overlay_gameworld_shadow` last-writes path
waypoints, production queue head, secondary weapon, and has_mine onto
presentation. Fail-closed: not full multi-item production queue / garrison ID
list authority.

### Legacy render stub presentation residual (2026-07-14)

Dead `render_game_objects` / `render_selection_indicators` stubs prefer
`last_presentation_frame` identity and avoid live `get_objects`/`find_object`
dual-reads when a snapshot is installed. Production path remains
RenderPipeline + selection_renderer. Fail-closed: stubs still not active draw.

### GameWorld garrison/contain overlay residual (2026-07-14)

`Entity` carries `garrisoned_host_ids`. Shadow sync copies building garrison /
occupants and `contained_by`. `overlay_gameworld_shadow` last-writes
`contained_by`, `garrisoned_units`, `disabled`, and `veterancy` onto presentation.
Fail-closed: not full multi-door garrison AI / contain redirect matrix.

### GameWorld kind_of bits residual (2026-07-14)

`Entity.kind_of_bits` encodes host ThingTemplate KindOf flags in presentation
ORDER bit positions. Shadow sync copies bits; overlay reconstructs
`RenderableObject.kind_of` without live template dual-read. Fail-closed: not
full KindOf matrix beyond the frozen ORDER set.

### GameWorld applied_upgrade_names residual (2026-07-14)

`Entity.applied_upgrade_names` carries capped, sorted host upgrade tags.
Shadow sync copies them; overlay last-writes `RenderableObject.applied_upgrades`.
Fail-closed: not full player science/queued upgrade matrix.

### GameWorld production_queue_items residual (2026-07-14)

`Entity.production_queue_items` carries capped multi-item host production queue
entries (template/progress/total_time/cost). Overlay last-writes full
`RenderableObject.production_queue` (not head-only). Fail-closed: not full
cancel/hold/priority queue control authority.

### Presentation identity residual overlay (2026-07-14)

`overlay_gameworld_shadow` last-writes template_name, team, disguise_as_team,
object_type, is_structure/unit/mobile, can_produce, and building_type from
Entity ordinals/flags. Fail-closed: model_key/mesh_scale/fow still freeze-path.

### GameWorld model_key/mesh_scale residual (2026-07-14)

`Entity.model_key` / `mesh_scale` carry host template mesh resolve residual.
Shadow sync uses `mesh_asset_resolve` helpers; overlay last-writes presentation
mesh identity. Fail-closed: not full bone/animation draw-scale matrix; FOW still
freeze-path.

### GameWorld FOW/ground residual (2026-07-14)

`Entity` carries FOW alpha/explored/falloff and ground_height residual. Shadow
sync samples host FOW bridge + terrain height; overlay last-writes presentation
`fow_visibility` / `ground_height`. Fail-closed: not full shroud cell matrix
authority.

### Defeat/alliance presentation-only residual (2026-07-14)

When `last_presentation_frame` is installed, defeat HUD and alliance radar team
resolve from presentation roster only. Live `get_player` is boot residual when
no frame is set. Fail-closed: roster miss yields id-only defeat log.

### GameWorld engine_bridged residual (2026-07-14)

`Entity.engine_bridged` mirrors host `engine_object_id.is_some()`. Overlay
last-writes `RenderableObject.engine_bridged` so the unit mesh pass can skip
double-draw without live dual-read. Fail-closed: not full ObjectFactory bridge
authority.

### Selection/attack presentation-only residual (2026-07-14)

Control-group, Tab, similar, box-select, double-tap centroid, and attack-click
team/identity resolve from `PresentationFrame` when installed. Live
`get_player` / `find_object` / `get_objects` remain boot residual only.
Fail-closed: not full WND widget selection parity.

### Render execute None honesty residual (2026-07-14)

Engine passes `game_logic: None` into `RenderPipeline::execute` when
`last_presentation_frame` is installed. Live `get_template` / transform collect
is boot residual only. Fail-closed: mesh asset filesystem resolve still outside
snapshot.

### Presentation transport/display residual (2026-07-14)

`RenderableObject` carries battle-bus/technical/combat-cycle/tunnel/chinook,
max_transport, overlord bunker capacity, passengers_allowed_to_fire, and
display_name. Overlay last-writes from Entity residual. Fail-closed: not full
contain redirect / multi-door transport AI matrix.

### Load-screen presentation roster residual (2026-07-14)

`load_screen_init_context` prefers `PresentationFrame` player roster when
installed; live `get_player` is boot/menu residual only. Fail-closed: not full
multi-slot LAN/skirmish load-screen parity from presentation alone.

### Presentation weapon stats residual (2026-07-14)

`RenderableObject` carries primary weapon min_range/reload/ammo/air-ground/
projectile_speed plus armed_riders and player weapon-set upgrade flags. Freeze
and overlay last-write from Entity residual. Fail-closed: not full multi-slot
weapon set / clip reload state machine.

### Presentation movement/target residual (2026-07-14)

`RenderableObject` carries target_location, guard_target, using_ability,
airborne_target, move_max_speed, velocity, and ai_state_ordinal. Freeze +
overlay last-write from Entity residual. Fail-closed: not full pathfinding
replan authority.

### Presentation path_len/occupant residual (2026-07-14)

`RenderableObject` carries path_len, path_index, and occupant_count. Freeze +
overlay last-write from Entity residual. Presentation shell documents Eva via
`update_post_draw_ui` without dual input/audio ownership. Fail-closed: not full
path replan / contain capacity matrix.

### Presentation audio SFX residual (2026-07-14)

`play_sound_effect` queues host `AudioEventRequest` when a presentation frame is
installed (UnitSelect/UnitCommand/…). Synthetic rodio tones remain boot residual
only. `MoveOrdered` maps to `UnitMove` in `apply_events_to_audio`. Fail-closed:
not Miles spatial device parity.

### Same-frame presentation audio process residual (2026-07-14)

After `PresentationFrame::apply_events_to_audio`, engine calls
`GameLogic::process_audio_events` so presentation-queued SFX drain same frame
(not delayed to next host tick). Fail-closed: not Miles device spatial parity.

### Presentation particle client mirror residual (2026-07-14)

`PresentationFrame::apply_particle_systems_to_client` backfills GameClient
ParticleSystemManager for active systems missing `client_system_id` and for
`ParticleSystemSpawned` events. Engine invokes it same-frame after audio drain.
Fail-closed: not full W3D GPU particle parity.

### Presentation victory residual (2026-07-14)

Engine builds `PresentationFrame` via `build_with_victory` (single evaluate) and
prefers `pres.match_over` / Victory event for end-of-match UI. Live
`evaluate_victory_condition` is boot residual only when no frame is installed.
Fail-closed: VictorySummary stats tables still host-built in show_victory_screen.

### Presentation VictorySummary residual (2026-07-14)

`PresentationFrame.victory_summary` freezes host `build_victory_summary` at
evaluate time. `show_victory_screen` prefers that residual; live rebuild is boot
residual only when no frame summary exists. Fail-closed: not live scoreboard
re-aggregate after post-match object churn.

### Presentation shell input device poll residual (2026-07-14)

`GameClient::update_presentation_shell` calls `update_input` to advance client
keyboard/mouse device state machines (C++ update residual). Main still owns OS
`WindowEvent` → gameplay command translation and sole 3D draw. Fail-closed: not
full `GameClient::update` (no audio device dual-own, no `draw_display`).

### Presentation shell client audio residual (2026-07-14)

`GameClient::update_presentation_shell` drains client audio/music/speech via
`update_audio` (C++ update residual). Distinct from Main
`GameLogic::process_audio_events` / presentation SFX residual. Fail-closed: not
full `GameClient::update` (`draw_display` stays Main RenderPipeline-only;
startup movies remain Main-owned).

### Presentation FOW drawable shroud residual (2026-07-14)

Main applies frozen `PresentationFrame` unit FOW to GameClient drawables via
`apply_presentation_shroud_to_drawables` before the presentation shell tick
(C++ Fogged|Shrouded → fully obscured). No live OBJECT_REGISTRY shroud bind.
Fail-closed: not sole GameWorld authority; RenderPipeline may still shade via
presentation alpha independently.

### Presentation alliance local_player residual (2026-07-14)

Alliance notification path prefers `PresentationFrame.local_player_id` when a
frame is installed; live `GameLogic::local_player_id` is boot residual only.
Fail-closed: alliance event source remains host `take_alliance_events`.

### Presentation camera residual (2026-07-14)

InGame `apply_pending_script_camera_requests` prefers frozen `PresentationFrame`
camera fields (focus/zoom/pitch/rotate/look/slave/shakers/screen_shakes) then
drains live `take_*` queues without double-apply. Live take path remains
boot/menu residual. Fail-closed: ease curves not frozen on frame (duration-only);
camera follow prefers presentation freeze (boot residual only).

### Presentation popup/music/fps residual (2026-07-14)

`PresentationPopupMessage` freezes pause/pause_music flags. InGame prefers
presentation popup + `pending_music_stop` + `script_fps_limit`, then drains live
`take_*` queues. Live takes remain boot residual. Fail-closed: movie playback
path not fully presentation-owned yet.

### Presentation script message/movie residual (2026-07-14)

HUD script messages prefer `PresentationFrame.new_script_messages` then drain
live `take_new_script_messages`. Script/radar movies apply from presentation
via GameClient display helpers, then `take_pending_movie*`. Runtime-host status
prefers presentation `match_over`/`victory_label`. Fail-closed: not full BINK
playback parity; boot path still plays movies when no frame.

### Presentation play time residual (2026-07-14)

`PresentationFrame.total_play_time_seconds` freezes host sim clock. UI
`current_game_time` prefers presentation (apply_to_ui_state + engine overwrite).
Live `get_total_play_time` is boot residual only.

### Presentation defeat/save-info residual (2026-07-14)

`build_with_victory` freezes `defeated_player_ids` from `peek_defeat_events`.
Engine defeat broadcast prefers presentation then drains live take.
`build_save_info` prefers presentation map_name, play_time, and local_team.
Fail-closed: difficulty still live host; not sole GameWorld authority.

### Presentation alliance events residual (2026-07-14)

`build_with_victory` freezes `alliance_events` via `peek_alliance_events`.
Engine alliance broadcast prefers presentation then drains live take.
Fail-closed: not sole GameWorld authority.

### Presentation difficulty/game_mode residual (2026-07-14)

`PresentationFrame` freezes `ai_difficulty` and `game_mode`. Save-info,
restart-mission, and runtime-host map status prefer presentation when
installed. Live host reads remain boot residual only.

### Presentation menu shell residual (2026-07-14)

Menu shell-map tick prefers `PresentationFrame.fow_shell_bypass` when true;
stale InGame frames fall through to live `isInShellGame`. Shell script FPS
prefers presentation when shell frame installed. Fail-closed: not sole
GameWorld authority; live path remains when no affirming shell frame.

### Presentation game_mode helper residual (2026-07-14)

`presentation_or_live_game_mode` centralizes freeze preference for load-screen
init, loading overlay, quick-save/load gates, restart, and save load-screen
prep. Live `GameLogic::game_mode` remains boot residual only.

### Presentation iconic game_mode residual (2026-07-14)

Minimized/iconic keep-alive uses `presentation_or_live_game_mode` instead of
live `GameLogic::game_mode`. Fail-closed: still not sole GameWorld authority.

### Presentation load-screen roster residual (2026-07-14)

Load-screen init expands slots from full `PresentationFrame.players` when
installed (not local-only). Live `get_player` remains boot residual.
`is_ai` frozen from host AI manager membership on `PresentationPlayerInfo`.

### Presentation player color residual (2026-07-14)

`PresentationPlayerInfo.color_rgb` freezes host skirmish/UI color. Load-screen
slots pack `apparent_text_color` (0x00RRGGBB). `apparent_color` stays None
(multiplayer color index not frozen). Fail-closed: not sole GameWorld authority.

### Presentation cinematic letterbox residual (2026-07-14)

Main applies `PresentationFrame.cinematic_letterbox` to GameClient
`GraphicsDisplay::enable_letter_box` before the presentation shell tick.
Fail-closed: not full GameClient::update / draw_display dual-own; cinematic
text/military caption remain UI-state projection only.

### Presentation military caption residual (2026-07-14)

`PresentationFrame` freezes `military_caption_remaining_ms` from host expiry.
Main applies caption text+duration to GameClient InGameUI via
`apply_presentation_military_caption`. Cinematic text also pushes InGameUI HUD
messages via `apply_presentation_cinematic_text` (anti-spam on text change).
Fail-closed: not sole GameWorld authority.

### Presentation cinematic text residual (2026-07-14)

Main applies `PresentationFrame.cinematic_text` to GameClient InGameUI HUD
messages (C++ display_cinematic_text → message). Anti-spam on text change.
Fail-closed: remaining_ms frozen but not yet used for timed HUD expiry.

### Presentation camera follow residual (2026-07-14)

`PresentationFrame.camera_follow_position` freezes host follow-object pose via
`peek_camera_follow_target_position`. InGame camera apply prefers presentation;
live `camera_follow_target_position` is boot residual only.

### Presentation timers/cameo/superweapon residual (2026-07-14)

`apply_to_ui_state` projects frozen `named_timers`, `named_timer_display_shown`,
`cameo_flash`, `superweapon_display_enabled`, and `superweapon_hidden_objects`
onto GameUIState. Fail-closed: not sole GameWorld authority; control-bar cameo
flash art still may re-read host command sets.

### AIPlayer update_with_frame C++ phase order (2026-07-14)

`gamelogic` `AIPlayer::update_with_frame` now follows C++ `AIPlayer::update`
order: doBaseBuilding → checkReadyTeams → checkQueuedTeams → doTeamBuilding →
doUpgradesAndSkills → updateBridgeRepair. `process_attack_decisions` remains a
host residual after that block (not in C++ AIPlayer::update).

### checkReadyTeams / checkQueuedTeams C++ parity (2026-07-14)

Compared `GeneralsMD/.../AI/AIPlayer.cpp` to `gamelogic` `AIPlayer`:

- `update_with_frame` phase order matches C++ `AIPlayer::update`.
- `check_ready_teams` now activates ready-queue teams on all-idle / executeActions
  script / 60s timeout (was only promoting build→ready).
- `check_queued_teams` ports build-time expiry (min-built→ready else disband),
  all-built prepend to ready, and any-idle productionCondition action execute.
- `is_minimum_built` counts in-progress factory (+1) like C++.
- `are_builds_complete` is true only when no factory assigned (C++).
- Reinforcement `join_team_reinforcement` residual ports `AIUpdateInterface::joinTeam`
  catch-up move (full state-match residual remains).

Fail-closed: Main host AI (`Main/src/ai.rs`) is still a separate simplified loop;
not sole GameWorld authority.

### doBaseBuilding / doTeamBuilding delay parity (2026-07-14)

Compared C++ `AIPlayer::doBaseBuilding` / `doTeamBuilding` to Rust:

- `do_base_building`: structureTimer→ready; `buildDelay` throttles `process_base_building`
  and defaults to `2 * LOGICFRAMES_PER_SECOND` (C++).
- `do_team_building`: teamTimer→ready; on `teamDelay==0` always `queue_units()`, then
  `process_team_building` if ready; sets `teamDelay = 5 * LOGICFRAMES_PER_SECOND` (C++).
- `process_team_building` is now selectTeamToBuild + queueUnits (not a recurse into do_team).
- `update_with_frame` always calls do_* in C++ order; timers live inside do_* (no double-decrement).

Fail-closed: process_base_building body still simplified vs full C++ rebuild/dozer path;
Main host AI still separate.

### processBaseBuilding rebuild/timer arm (2026-07-14)

`AIPlayer::process_base_building` now walks the player build list like C++
`processBaseBuilding`: clears destroyed/captured IDs, honors
`rebuild_delay_seconds * LOGICFRAMES_PER_SECOND`, requires a dozer (queues one if
missing), starts at most one structure per call, and arms `structure_timer` with
AIData poor/wealthy structure mods (`timer / mod`). Full legal-place wiggle and
dozer `aiResumeConstruction` remain residual.

### selectTeamToBuild hiPri/random (2026-07-14)

Compared C++ `AIPlayer::selectTeamToBuild` to Rust:

- Collect `isAGoodIdeaToBuildTeam` candidates and track hiPri.
- Call `selectTeamToReinforce(hiPri)` before starting a new team.
- Random-pick among hiPri set via `game_logic_random_value` (C++ GameLogicRandomValue).
- `buildSpecificAITeam(..., false)` then arm `teamTimer` with AIData team poor/wealthy mods.
- `isAGoodIdeaToBuildTeam` uses `isPossibleToBuildTeam(..., requireIdleFactory=true)`.

Fail-closed: production-condition script eval still residual; Main host AI still separate.

### selectTeamToReinforce auto unit (2026-07-14)

Compared C++ `AIPlayer::selectTeamToReinforce` to Rust:

- Only `automaticallyReinforce` prototypes with priority **above** minPriority.
- Skip prototypes already in the build queue.
- Pick live team instance missing units (count < maxUnits) with an **idle** factory.
- Prepend single required work order; try `tryToRecruit` then `startTraining`.
- `teamDelay = 0` shortcut after queueing (C++).

Fail-closed: homeLocation origin residual when team has no members; production
condition scripts still residual.

### evaluateProductionCondition (2026-07-14)

Ported C++ `TeamPrototype::evaluateProductionCondition` (Team.cpp) to Rust:

- Empty / missing script → `always_false` cache, return false.
- Difficulty gate via script easy/normal/hard flags vs controlling player difficulty.
- Delay-eval frame honor + `ScriptEngine::evaluateConditions` with controlling player.
- `isAGoodIdeaToBuildTeam` now calls this first (C++ order).

Fail-closed: full OrCondition graph coverage depends on ScriptEngine evaluator depth;
Main host AI still separate.

### queueUnits tryToRecruit (2026-07-14)

Compared C++ `AIPlayer::queueUnits` to Rust:

- For each waiting work order, loop `tryToRecruit` until full or none left.
- Recruited units `setTeam` + `aiMoveToPosition(home)` when base/home known, else `aiIdle`.
- Still waiting → `startTraining`; else `validateFactory`.
- Recruit radius from AIData `max_recruit_distance`.

Fail-closed: TeamPrototype homeLocation field residual (uses base center);
Main host AI still separate.

### isPossibleToBuildTeam anyIdle/avg cost (2026-07-14)

Compared C++ `AIPlayer::isPossibleToBuildTeam`:

- Every unit type needs a factory (`findFactory(..., busyOK=true)`).
- `anyIdle` if any type has an idle factory; when `requireIdleFactory` and none idle → false.
- Cost uses `(minUnits+maxUnits)/2` average × `teamResourcesToBuild`.
- Returns `(possible, notEnoughMoney)` like C++ out-param.

### startTraining queueCreateUnit (2026-07-14)

Compared C++ `AIPlayer::startTraining` / `findFactory`:

- `start_training_internal` calls `request_unique_unit_production_id` +
  `queue_unit_with_production_id` (C++ queueCreateUnit), then sets `order.factory_id`.
- Falls back to `ProductionUpdateInterface::start_production` if object queue path fails.
- `find_factory_internal` walks the player **build list** first (C++), then all objects.
- Shared `factory_candidate` skips under-construction / sold / wrong owner.

Fail-closed: BuildAssistant::isPossibleToMakeUnit residual; Main host AI separate.

### onUnitProduced teamDelay/setTeam (2026-07-14)

Compared C++ `AIPlayer::onUnitProduced`:

- Match work order by factoryID + incomplete + template equivalent.
- Increment completed, clear factoryID, setTeam on unit, reinforcementID.
- SupplyTruck force-wanting when order is resource gatherer (aiDock residual).
- Dozer: repair-dozer handoff or `buildDelay=0` + `structureTimer=1`.
- Always `teamDelay = 0` so queues re-evaluate immediately.

### buildStructureWithDozer (2026-07-14)

Compared C++ `AIPlayer::buildStructureWithDozer`:

- `findDozer` + funds via `calcCostToBuild`; queue dozer if missing.
- Spawn structure, set producer/builder, under-construction, dozer `set_build_task`.
- Stamp BuildListInfo objectID/timestamp/underConstruction + decrement rebuilds.
- `process_base_building` calls this on the first missing buildable entry (USE_DOZER).

Fail-closed: legal-place wiggle / NO_ENEMY overlap residual; Main host AI separate.

### buildStructureNow / newMap / checkForSupplyCenter (2026-07-14)

Compared C++ `AIPlayer::buildStructureNow`, `newMap`, `checkForSupplyCenter`:

- `build_structure_now_at`: inst-spawn, clear UC/Reconstructing, 100% construction,
  upgrade modules, stamp build list, `check_for_supply_center`.
- `new_map`: add placed factories to build list, `compute_center_and_radius_of_base`,
  inst-build `isInitiallyBuilt` entries else `incrementNumRebuilds`.
- `check_for_supply_center`: SupplyCenterDockUpdate → supply building + desired
  gatherers from AISideInfo + 1 freebie, currentGatherers=-1.

Fail-closed: map-property Dict (name/script/health/unsellable) residual; rally
offset residual (C++ gotOffset bug).

### queueSupplyTruck (2026-07-14)

Compared C++ `AIPlayer::queueSupplyTruck`:

- Early-out if a resource-gatherer work order is already queued.
- Walk supply-building build-list entries; skip rebuild holes / empty warehouses.
- Recount gatherers on center; reattach loose harvesters when preferred dock dead.
- Cap at 3× desired harvesters; else priority-queue one harvester via startTraining
  (first automatic freebie assigns factory without training).

Fail-closed: full template linked-list scan residual (candidate name list);
aiDock CMD_FROM_PLAYER residual.

### findDozer / queueDozer / supply safety (2026-07-14)

Compared C++ `AIPlayer::findDozer`, `queueDozer`, `isSupplySourceAttacked`,
`isSupplySourceSafe`, `isLocationSafe`, `guardSupplyCenter`:

- findDozer: skip repair dozer, ferrying workers, BUILD task; prefer idle closest;
  queueDozer when none exist.
- queueDozer: dozerInQueue gate, temp enable canBuildUnits, priority startTraining.
- isSupplySourceAttacked: 10s scan rate, player attacked frame, cash/dozer/harvester
  recent damage latch.
- isSupplySourceSafe: findSupplyCenter + isLocationSafe (safe radius + enemies).
- guardSupplyCenter: force check, attacked warehouse or find, offset toward enemy,
  aiGuardPosition.

Fail-closed: full partition filter set residual; template linked-list residual
(candidate names); AiGroup groupGuardPosition uses per-member ai_guard_position.

### computeSuperweaponTarget (2026-07-14)

Compared C++ `AIPlayer::computeSuperweaponTarget` / `getPlayerSuperweaponValue`:

- Degenerate bounds → map extent; shrink X by radius; ceil grid capped at 10.
- Random scan direction with xStart/yStart = count (not count-1) on reverse axes.
- Fine-tune preserves C++ `(x-5)` on both axes (legacy bug).
- Sneak attack: military/defenses negative; flying aircraft skip only when
  includeMilitaryUnits; CC/superweapon ×5 or /10.

### buildUpgrade / buildBySupplies / repairStructure (2026-07-14)

Compared C++ AIPlayer:

- `buildSpecificAIBuilding`: solo AI logs only (skirmish override owns real work).
- `buildUpgrade`: type/money/progress gates; walk build-list factories; command-set
  match; queueUpgrade.
- `buildBySupplies`: warehouse selection, base/enemy offset, legalize, priority
  build list + curWarehouseID.
- `repairStructure` / `updateBridgeRepair`: pristine skip, queue bound 2, 1 Hz
  timer, findDozer/queueDozer, aiRepair, complete+home move.

### findSupplyCenter / nearest-team / onStructureProduced (2026-07-14)

Compared C++ AIPlayer:

- `findSupplyCenter`: owned cash-generator proximity skip, enemy 60/40 filter,
  cash floor halving to 100.
- `buildSpecificBuildingNearestTeam`: team estimate position → legalize →
  priority build list.
- `calcClosestConstructionZoneLocation`: seed location wiggle via
  find_valid_build_location.
- Solo `buildAIBaseDefense*` stubs (skirmish overrides).
- `onStructureProduced`: build-list match, clear UC + upgrades, supply stamp,
  rebuild-hole retarget residual.

### buildSpecificAITeam / recruitSpecificAITeam (2026-07-14)

Compared C++ AIPlayer:

- `buildSpecificAITeam`: canBuildUnits, singleton+priority, isPossibleToBuildTeam
  (money-only still queues), optional then required work orders, createInactiveTeam,
  priority prepend, teamDelay=0.
- `recruitSpecificAITeam`: createInactiveTeam, tryToRecruit to max, MoveTo home,
  ready-queue if any else disband non-singleton.
- Work-order helper matches optional/required split.

### AISkirmishPlayer base defense / newMap (2026-07-14)

Compared C++ AISkirmishPlayer:

- `buildAIBaseDefenseStructure`: approach path offset, alternating angles,
  legalize, `addToPriorityBuildList` (not generic building queue).
- `newMap`: side build list + `adjustBuildList`, compute base center, initiallyBuilt
  via `buildStructureNow` else `incrementNumRebuilds` (does not call AIPlayer::newMap).

### AISkirmishPlayer processBaseBuilding dozer (2026-07-14)

Compared C++ USE_DOZER path:

- Missing dozer on UC buildings → queueDozer + findDozer + aiResumeConstruction.
- Power plants exclude CASH_GENERATOR; force power when underpowered.
- Selected build uses `buildStructureWithDozer` (not priority-mark only).
- On success: arm structureSeconds timer with wealth mods.

### AISkirmishPlayer selectTeamToBuild delegation (2026-07-14)

C++ skirmish `selectTeamToBuild`/`selectTeamToReinforce`/`isAGoodIdeaToBuildTeam`
delegate to AIPlayer. Rust now matches:

- no skirmish-only enemy scoring override of team pick
- isAGoodIdea uses production condition + max + queue + idle factory
- processTeamBuilding: select then queueUnits (counter-unit residual analysis only)

### AISkirmishPlayer update phase order (2026-07-14)

C++ `AISkirmishPlayer::update` calls `AIPlayer::update` with virtual overrides:

doBaseBuilding → checkReadyTeams → checkQueuedTeams → doTeamBuilding →
doUpgradesAndSkills → updateBridgeRepair.

Skirmish doBase/doTeam tick structure/team timers, clamp to 3s max, and throttle
process* via 2s delays (matching AISkirmishPlayer.cpp).

