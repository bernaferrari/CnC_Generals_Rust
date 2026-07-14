# Ownership and Authority Policy

> Borrow first, stable IDs second, `Arc` only at real sharing boundaries.

Preserve C++ **behavior**. Do not preserve C++ **pointer ownership**.

## Authoritative simulation (production surface, 2026-07-14)

```
OS input → normalized commands → Main GameLogic (30 Hz host sim)
  → host_* logs (damage/economy/spawn/destroy/attack)
  → GameWorldShadow session (always-on) → WorldMutations last-writer
  → host writeback (HP/cash) → PresentationFrame + shadow overlay
  → GameClient / audio / renderer
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
| Full `GameClient::update()` | **not** called (Main owns input/audio) | — |

- Target end state: `gamelogic::GameWorld` sole host; Main = composition root only.
- Current honesty: Main still owns mid-frame AI/path/combat *execution*; GameWorld is last-writer for HP/cash/pose/targets + presentation overlay.

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
| `executable_smoke_gate` | Real binary Menu→InGame + runtime-host select/move (`gameplay_cmd`); **not** WND click path (`playable_claim=false`) |
| `shell_smoke_gate` | Headless host skirmish stack; not windowed WND |

## Presentation residual (unit mesh)

When `PresentationFrame` is set, `RenderPipeline::collect_render_items` drives the main unit mesh pass from `unit_render_inputs` only (`debug_last_live_unit_identity_reads == 0`). Live `game_logic.get_objects()` remains only for boot/loading frames without a snapshot. Terrain/prewarm prefer frozen `PresentationWorldEnv` and fall back to live map metadata if absent.

## Still Main mid-frame (not sole GameWorld)

Host still executes AI decision, pathfinding step, and combat resolution mid-frame. AI `launch_attack` now prefers `set_target` (host_attack_log) plus move so the shadow attack channel sees AI aggression. Shadow session runs after host `update` + projectiles + pathfinding (same frame logs), then PresentationFrame overlay. GameWorld shadow is last-writer for HP/cash/pose/targets/move destinations and presentation overlay — not yet the sole simulation owner.

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



### GameWorld shadow (2026-07-14)

`gameworld_shadow` maintains a `GameWorldShadow` session: stable host `ObjectId`→
`EntityId` map, delta sync (health/transform/economy), and `WorldMutation` damage
parity (`queue_damage_for_host` / `apply_pending`). Opt-in runtime: `GENERALS_GAMEWORLD_SHADOW=1` holds a session on `CnCGameEngine`.
`Object::take_damage_from` records `host_damage_log` events drained each tick.
Spawn/destroy: `host_spawn_log` / `host_destroy_log` drained each tick; shadow maps spawns and applies Destroy mutations. `WorldMutation::Spawn` exists for the mutation channel.

Presentation: when engine holds a shadow session, `PresentationFrame` is built from host then `overlay_gameworld_shadow` so HP/pose/supplies prefer GameWorld.
Move channel: `SetTransform` mutations + `apply_host_positions_as_transforms`.

Shadow session **defaults on** in the engine (`GENERALS_GAMEWORLD_SHADOW=0` to disable). Entity.attack_target mirrors host Object::target; SetAttackTarget mutation available.

**Production defaults (2026-07-14):** shadow session, damage authority, and economy authority are **on** when env unset. Opt out with `=0` / `false`. `host_attack_log` from `Object::set_target` → SetAttackTarget each tick.

Still not sole GameWorld production authority for AI/path/full combat sim.

### Damage authority cutover (opt-in)

Gates call `ensure_gate_damage_authority()` so damage authority defaults on (set `GENERALS_GAMEWORLD_DAMAGE_AUTHORITY=0` to opt out). `GENERALS_GAMEWORLD_ECONOMY_AUTHORITY=1` (gates default-on): `host_economy_log` from Player spend/add/bounty/refund → SetSupplies/SetPower mutations → host writeback. `GENERALS_GAMEWORLD_DAMAGE_AUTHORITY=1` (implies shadow session): end-of-tick
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
2. `PresentationFrame.laser_beams` freezes active Patriot assist lasers + Line3D segments
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

