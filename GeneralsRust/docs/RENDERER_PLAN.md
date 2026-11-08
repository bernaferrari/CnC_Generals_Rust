# Renderer plan: map of the live wgpu stack, its problems, and migration steps

Status: phase 2 (implementation on `fix/render`, `fix/render2`). See "Progress" below.
Reviewed tree for the analysis: `grok/logic-ownership` @ `71904d369`. Line numbers in
sections 1-2 refer to that tree; several files moved or shrank since.
All paths are relative to `GeneralsRust/Code/` unless they start with `GeneralsMD/`.

Ground rules this plan respects:

- Beads hq-x1j4u.6.4 and .6.5 say "do not write a new renderer, do not add a second
  scene graph". This plan consolidates the renderer that already ships. Every step
  moves or deletes code. None of them starts a parallel stack.
- The renderer must produce the C++ picture: the same draw order, blending, alpha test,
  fog/shroud, lighting, terrain, water, particles, and UI. Internally it does not need to
  look like DX8.
- No process-wide mutable state. One `Renderer` per game instance. The wgpu
  device/queue may be shared through `Arc`.
- When a module is done, it goes into the existing `sealed_modules.toml` /
  `scripts/check_sealed_modules.sh` mechanism.


## Progress (fix/render)

How to verify any step: `cargo build -p generals_main --bin generals --bin golden_capture`,
then `target/debug/golden_capture <out.png> --scene shell|lone-eagle --frame N
[--compare ref.png]` with `GENERALS_ASSETS_DIR` pointing at the retail install
(800x600 headless; see `Code/Main/src/cnc_game_engine/golden_capture.rs`). Each
capture writes `<out>.stats.txt` with the last frame's counters. Captures of the
same binary are byte-identical run to run (verified 2-3x for both scenes).
The terrain clock and water scroll used to advance per draw call, so frozen
shell captures differed run to run on the sea; they now step once per client
frame like C++ (`W3DDisplay::draw` syncTime, `WaterRenderObjClass::update`),
see `ecbfce083`.

Terrain blending now carries separate authored UVs for the base and primary
blend tiles and resolves the extra-blend class and UVs from its authored tile.
Shoreline rendering precedes the extra-blend pass. Mixed-cell and non-blend
controls pass; a native Lone Eagle capture was inspected. Cliff/custom-edge
fidelity, the 16-class binding limit, and a wasm frame remain unverified.

| Step | State | Evidence |
|---|---|---|
| prerequisite | done | `drive_frame` self-deadlocked on the WW3D mutex on the first Menu frame (lock temporary in a `match` scrutinee); the shell then failed on object-less drawables (`missing native material observation`). Both fixed, so headless/windowed runs get past the menu again. |
| R0 | done | `RenderFrameStats` (wgpu HAL counters sampled at begin_render and before submit, recorded passes, record CPU time) published as `render_frame_stats=`; `Engine::set_fixed_clock`; frame encoder created only after a successful acquire; golden freeze/capture mode + `golden_capture` bin; `count_globals.sh` / `check_sealed_modules.sh` count `lazy_static! { static ref }` (baseline 1 -> 35, now 30). |
| R1 | done | GameClient `w3d/{renderer,device,post_processing}` + shaders, WgpuWrapper legacy backend, ww3d-gpu PIPELINE_CACHE/DYNAMIC_BUFFERS/SHADER_CONSTANTS/GPU_CONTEXT, second ShaderClass pipeline cache, GraphicsSystem uniform/material GPU state, `.bak*`, archive crate out of the workspace. ~16.8k lines removed. renderer-3d unit tests compile again (were 60 errors). |
| R2 | done | `SortingRenderer` owned per `MeshRenderManager`; two-renderer isolation test. Golden-R identical. |
| R3 | done | `FramePhase` + `FRAME_SCHEDULE` replace the FIFO callback queues; first commit encodes the old runtime order. Golden-R identical (shell + Lone Eagle). |
| R4 | done | Schedule in `RTS3DScene::Flush` order; sorting flush uses the tactical viewport. Lone Eagle: unit/building ground shadows become visible (2716 px in the base area); shell: 28 px. Ground overlays (selection circles, order lines) are no longer recorded before the terrain clear. |
| R5 | done | Per-manager `MeshBindCache` (texture-stage / vertex-color groups reused across frames, shared samplers and views); `FrameUniformArena` staged in a CPU mirror with one `write_buffer` per page (`1eff7fead`). Model + lighting uniforms bound with dynamic offsets; one model bind group per (pipeline, arena page) instead of per draw (`37c4ea1fe`; shell created bind groups 168 -> 17 per frame). |
| R6 | mostly done | Particle batches share one instance buffer; selection circles / order lines use grow-only buffers (`1eff7fead`); weather, tracer, heat-haze and decal geometry share a per-frame ring (`c3001b868`); the Display occlusion-overlay pass keeps its uniform buffer, bind group and a grow-only vertex buffer (`8b5f104ad`). Still per call: dynamic-light terrain chunk rebakes; ~40 `write_buffer` staging buffers per Lone Eagle frame. |
| R7 | partial | The receiving archive manager owns compressed DDS blocks and C++-visible authored mips; mesh providers borrow them for a single cached GPU upload, using BC1/BC2/BC3 when enabled on the device and otherwise decoding each level with reusable scratch storage. Engine device creation requests BC only when advertised by the adapter. TGA mesh textures use generated box-filtered mips. The unified renderer-wide cache remains open. |
| R8 | done | Gamma space like DX8 (`f5e2784da`): surface prefers `Bgra8Unorm`, every colour texture/target is `*Unorm`, `image_format_to_wgpu` and the mesh upload treat sRGB tags as Unorm. Visible: lit/modulated colours and UI fills are no longer washed out (the radar letterbox line, C++ 50/50/50, was 122/122/122 and is now 50/50/50). |
| R9 | done | Live CSM deleted (`ecbfce083`). `Depth24PlusStencil8` + stencil volumes from welded-edge silhouettes for SHADOW_VOLUME objects, drawn in the mesh flush between opaque and blended meshes, then the 0x7fa0a0a0 multiply (`7772919c9`). The old shadow-pass box prisms read from `OBJECT_REGISTRY` are deleted (`8b5f104ad`). Skinned casters do not cast volumes yet. |
| R10 | partial | Mesh sorting pools triangles by view-space centroid and emits sorted index runs per node. Skinned meshes now sort the posed vertices sampled from the same palette and influences as the live vertex shader; a WGPU readback confirms that reversing bone depth reverses translucent pixel composition. Particles and other sorting producers are not pooled with meshes, equal-depth ordering can differ from C++, and a C++ golden comparison remains open. |
| R11 | partial | Consecutive UI draws with equal pipeline, texture and scissor merge into one draw (`cf92f27eb`). Open: glyph atlas (each string is still its own texture). |
| R12 | partial | A shared scene pass merges compatible terrain, water, mesh and effect draws. Shell and Lone Eagle captures match the pre-change images; reported passes fell from 4 to 2 and 6 to 2 respectively. Full pass-count coverage still needs verification. The integrated commit includes the required unseal justification. |
| R13 | partial | `THE_RENDER_BRIDGE` is gone; published terrain visual remains on `CommonServices`; buffer-usage constants and shadow-pass inputs were migrated. Options texture LOD borrows the receiving renderer rather than publishing a type-erased renderer handle. No renderer module is sealed, and full instance isolation is not established. The workspace-wide mutable-global ratchet is now 19; this count also includes non-rendering subsystems. |

Frame stats, 800x600 debug build, steady frozen frame (created per frame):

| Scene | Step | bind groups | buffers | samplers | record ms |
|---|---|---:|---:|---:|---:|
| ShellMapMD step 60 | R0-R4 | 1344 | 1060 | 151 | 78-87 |
| ShellMapMD step 60 | R5 cache (no sampler cache) | 455 | 603 | 151 | 63 |
| ShellMapMD step 60 | R5 + sampler/view cache | 293 | 603 | 0 | 57 |
| Lone Eagle frame 90 | R0-R4 | 554 | 453 | 63 | 46-48 |
| Lone Eagle frame 90 | R5 cache (no sampler cache) | 182 | 269 | 63 | 40 |
| Lone Eagle frame 90 | R5 + sampler/view cache | 119 | 269 | 0 | 37 |
| ShellMapMD step 60 | + gamma space, no CSM, terrain cells | 168 | 330 | 0 | 34 |
| ShellMapMD step 60 | + staged arena, overlay ring | 168 | 6 | 0 | 29-34 |
| Lone Eagle frame 90 | + terrain cells (32-cell chunks) | 21 | 269 | 0 | 28-29 |
| Lone Eagle frame 90 | + no CSM | -23 | 180 | 0 | 22 |
| Lone Eagle frame 90 | + staged arena, overlay ring | -23 | 46 | 0 | 20 |
| ShellMapMD step 60 | + R5 dynamic offsets, R6 ring, R9 volumes, R11, R13 (`8b5f104ad`) | 17 | 8 | 0 | 28 |
| Lone Eagle frame 90 | + R5 dynamic offsets, R6 ring, R9 volumes, R11, R13 (`8b5f104ad`) | -87 | 40 | 0 | 19-21 |

Render passes per frame: shell 8 -> 4, Lone Eagle 10 -> 6 (CSM depth passes
gone). "bind groups" is a live-count delta, so it goes negative when cached
groups age out in the same frame. "buffers" counts every HAL buffer, including
the staging buffers wgpu allocates for `queue.write_buffer`, which is why
batching writes moved it.

Cross-build nondeterminism (the 28-pixel ShellMapMD ship spot): fixed in
`dbeb197ae`. The frame graph broke sort ties with the mesh `Arc` address and
grouped decal batches by material-pass address; both now use submission /
first-use order. Two different builds produce an IDENTICAL Lone Eagle capture.

Picture fixes after the first merge (visible-correctness pass):

| Fix | Commit | Evidence |
|---|---|---|
| Terrain drew tile 0 on every cell: native map admission froze the height map before BlendTileData was folded in; shared chunk vertices interpolated UVs across cells; UV ignored the tile's slot in its class grid | `c0a6521dc` | Lone Eagle shows varied authored ground instead of one grey pattern; ShellMapMD sea floor/beach detail |
| Control bar missing in headless/golden runs (layout + scheme only materialised for windowed hosts) | `d98f99ec0` | full USA control bar in the Lone Eagle golden |
| White 3x3 lines bottom-right = `W3DCommandBarGridDraw` on `WinUnitSelected`, which C++ hides in `CB_CONTEXT_NONE` | `d98f99ec0` | lines gone with no selection |

Not a bug: the red grid frame and yellow rectangle on the shell menu are the
authored `MainMenu.wnd:MainMenuRuler` art, faded in by the WindowTransitions
system after `MainMenuInit` hides it (same as C++).

| Terrain seams along chunk rows: 64-unit chunks (not a multiple of the 10-unit cell) tessellated by LOD resampled the height field twice (T-junction cracks, mid-cell quads). Now C++ `VERTEX_BUFFER_TILE_LENGTH` 32-cell chunks, one quad per cell at the exact sample heights, map-wide normals, no LOD | `9b869dc02` | Lone Eagle grid lines gone |
| Only 4 terrain classes bound, and the shader ignored `blend_indices` (weight i sampled slot i). Now 16 class slots, slot read flat per cell (slot-sorted union over the cell's corners) | `9b869dc02` | Lone Eagle snow/gravel/grass layout visible instead of one grey class |
| Every cell row cut the tile: C++ `getUVForNdx` vertical quadrant and the class-block row order (`updateTileTexturePositions` stores row j at width-j-1) were both flipped | `9b869dc02` | continuous tiles |

Radar "black in frozen captures" is not a bug: the terrain texture is
complete (16384 non-black cells) and the shroud is 481 clear / 15903
shrouded cells at Lone Eagle frame 90, i.e. only the start area is explored,
which is what C++ draws. The revealed patch, units and view trapezoid render
correctly. The radar shroud revision now only moves on a real cell change,
so the resident shroud texture is not re-uploaded every frame.

Open render items: R7 unified texture cache, R10 sorting parity gaps, R11 glyph atlas,
R12 full pass-count coverage, R13 sealing and remaining instance isolation,
R6 dynamic-light chunk rebakes, and skinned shadow-volume casters.

---

## 1. Current stack map (what actually runs in `generals`)

### 1.1 Device, queue, surface ownership

| Concern | Owner | Evidence |
|---|---|---|
| Instance, adapter, device, queue | `ww3d_engine::Engine` (`Libraries/Source/WWVegas/WW3D2/crates/ww3d-engine/src/lib.rs:801`) through a per-engine `DeviceAuthority` (`ww3d-gpu/src/device_authority.rs`) | `Engine::new_windowed` lib.rs:893-950 |
| Handle type | `SharedEngine = Arc<parking_lot::Mutex<Engine>>` (lib.rs:857) | every frame locks it (`forward_render.rs:616`, `pipeline_execute.rs` filter skip check) |
| Creation | `Main/src/cnc_game_engine/boot.rs:414-426` with `EngineConfig::default()` plus width/height | |
| Surface config | `WindowSurfaceState::new` lib.rs:258-316: sRGB format preferred (`select_surface_format` lib.rs:2164), `Fifo`, `Opaque`, `desired_maximum_frame_latency: 2`, usage `RENDER_ATTACHMENT \| COPY_SRC` | |
| Depth | `Depth32Float`, **no stencil** (lib.rs:89), `sample_count: 1` (no MSAA) | |
| Extent | logical window size, not physical (`boot.rs:410-414` comment) | Retina renders at 1x and is upscaled |
| Resize | `Engine::resize` lib.rs:1421 reconfigures the surface, depth, and occlusion fallback. Called from `run_loop.rs:800/811` and `input.rs:168` | `set_swap_interval` (lib.rs:1434) only stores a value and never reconfigures the present mode |
| Occluded window | Draws into a COPY_SRC fallback target (lib.rs:1175-1209) | needed for smoke gates on macOS |
| Other device creators | Test and demo code only. The census test `ww3d-gpu/tests/game_path_request_device_census.rs` enforces this | OK |

The same device is shared by `GraphicsSystem` (`Main/src/graphics/graphics_system.rs`),
`ForwardPass` (`forward_render.rs:127`), `WgpuMainRenderer::from_engine`
(`ww3d-renderer-3d/src/rendering/wgpu_main_renderer.rs:307`), the GameClient terrain visual,
the particle renderer, the UI renderer (`boot.rs:1259`), and the minimap renderer
(`pipeline_minimap.rs:31`).

### 1.2 Frame call chain

```
CncGameEngine run loop
 └ RenderPipeline::execute_inner          Main/src/graphics/render_pipeline/pipeline_execute.rs:111
    ├ pack lasers/projectiles/lines/particles (CPU) + enqueue laser post-frame callback
    ├ enqueue ClientEffects scoped post pass (decals, particles)
    ├ collect_render_items / drain_render_bridge / sort_render_items
    ├ terrain: hydrate GPU, tree CPU draw, enqueue TerrainGround pre-scene pass
    ├ enqueue shadow+occlusion post-frame callback       (pipeline_execute.rs:493-519)
    ├ execute_water_pass -> enqueue TerrainWater post pass (pipeline_collect.rs:2199)
    └ ForwardPass::render                                  forward_render.rs:542
       ├ projected shroud upload, texture streaming
       ├ WgpuMainRenderer::begin_frame -> Engine::begin_render (encoder + acquire)
       ├ prepare_mesh_instance + queue_mesh per RenderItem
       └ WgpuMainRenderer::end_frame_with_scoped_native_passes   wgpu_main_renderer.rs:535
          ├ pre-scene callbacks (TerrainGround pass)
          ├ Renderer::render_with_targets                ww3d-renderer-3d/src/lib.rs:563
          │   ├ live CSM: 4 depth passes                  render_manager.rs:579-705
          │   ├ "WW3D Main Render Pass": opaque then blended meshes
          │   └ "WW3D Sorting Flush" pass (global SORTING_RENDERER)
          ├ post-frame callbacks in FIFO order: laser, ClientEffects, shadows, water
          ├ finish_scene: view filters, locked-camera meshes, camera fade, UI overlay
          └ Engine::end_render -> FrameSubmitState::submit_owned_frame (one queue.submit)
```

### 1.3 Render passes per in-game frame (order as recorded)

All passes go into one encoder, with one `queue.submit` per frame (`frame_submit.rs:144-170`).
That part is already good. Pass count:

1. `main terrain pass` (clear). `Main/src/graphics/render_pipeline/native_terrain_pass.rs:24`
2. to 5. `live_csm_first_light_depth` x `LIVE_CASCADE_COUNT=4`. `render_manager.rs:670`
6. `WW3D Main Render Pass` (Load because a pre-scene pass ran). `ww3d-renderer-3d/src/lib.rs:639`
7. `WW3D Sorting Flush` when batches exist. lib.rs:706
8. `laser_additive_segliner`. `forward_render.rs:290`
9. Client effects: decals (`particle_renderer.rs:1767`), particles (:1354), weather (:1487), tracers/rays (:1603), heat haze (:1679). These are separate passes.
10. Shadow volume, fill, and occlusion overlay passes (`GameClient/src/display/shadow_pass.rs:994/1032/1081`)
11. `main water pass`. `native_terrain_pass.rs:71`
12. View filter composite (`GameClient/src/display/shader_filter.rs`), camera fade (`status_circle.rs`)
13. UI pass (`Main/src/graphics/ui_render_pass.rs:185`)

That is roughly 14 to 20 passes. Each loads and stores the full color+depth attachment. On TBDR GPUs
(Apple, mobile, and wasm on those) every extra pass is a full tile load/store. Passes 8 to 11
could be one pass.

### 1.4 Independent pipeline owners (live path)

| Feature | Owner (file) | Pipelines | Notes |
|---|---|---:|---|
| W3D meshes | `ww3d-renderer-3d/.../wgpu_renderer/wgpu_pipeline_manager.rs` (`WgpuPipelineManager`) | lazy, keyed | key = `ShaderClass` bits + skinned/lit/fog/topology/formats (:33-44). Shader module compiled per pipeline (:146). One of 4 WGSL files chosen by blend func (:108-131) |
| CSM depth | `render_manager.rs:732-830` | 2 | **not a C++ feature** (C++ uses projected decal shadows + stencil volumes) |
| Projected shroud on meshes | `ww3d-renderer-3d/src/rendering/projected_shroud.rs` + `Main/src/graphics/projected_shroud_upload.rs` | via mesh manager | |
| Terrain | `GameClient/src/terrain/terrain_visual/impl_pipelines.rs` | 8 | plus `river_gpu.rs` (1) and `shroud_gpu.rs` (1) |
| Water | terrain visual (`TerrainWater` scoped pass) | shared | |
| Particles, decals, weather, tracers, heat haze | `GameClient/src/effects/particle_renderer.rs` | 8 | own texture cache (:1840, :1960) |
| Shadows / occlusion | `GameClient/src/display/shadow_pass.rs:534-760` | 4 | volume path dead without stencil (see 1.6) |
| Lasers | `Main/src/graphics/laser_draw.rs` | 1 | pipeline format hard-coded `Bgra8UnormSrgb` (`forward_render.rs:256`) |
| View filters | `GameClient/src/display/shader_filter.rs` | 1+ | |
| Camera fade | `GameClient/src/display/status_circle.rs` | 1 | |
| UI / text | `GameClient/src/gui/ui_renderer.rs:759-907` | 5 | |
| Minimap / radar | `Main/src/graphics/minimap_renderer.rs` | n | |
| Selection | `Main/src/graphics/selection_renderer.rs` | 3 | a second `Main/src/selection_renderer.rs` exists (CPU command builder) |

There are 3 pipeline caches, about 11 `TextureManager` types, and 3 `ShaderClass`/`Shader` definitions.
`ShaderClass` types live in `ww3d-renderer-3d/src/rendering/shader_system/shader.rs:432`,
`rendering/shader_core.rs:22`, `ww3d-gpu/src/shader.rs:298`, `ww3d-core/src/material.rs:175`, and
`ww3d-assets/src/material.rs:144`. `TextureManager` types live in `Main/src/assets/textures.rs:154`,
`Main/src/ui/mod.rs:623`, `GameClient/src/display/texture_system.rs:63`,
`GameClient/src/terrain/textures.rs:460`, ww3d-gpu, ww3d-core, ww3d-assets,
ww3d-renderer-3d, and both GameEngineDevice crates.

### 1.5 Dead or parallel paths (not on the `generals` binary path)

These were checked by Cargo dependency or call graph. Items marked (v) need one more grep before deletion.

- `Code/GameEngineDevice/` (`game_engine_device_archive`). It is a workspace member and does not link into Main. It holds `wthree_d_view.rs`, `Water/water_renderer.rs`, `terrain_rendering.rs`, `Shadow/*`, and a full DX8-style device.
- `Code/GameEngine/GameEngineDevice/`. It is an optional Main dependency that no feature enables. It holds `w3d/renderer.rs` with global `UP_STAGING`/`RENDER_STATE_LOG` statics (:79-80), `video/*`, `w3d/shadow_system.rs:337` `LIGHT_POS_WORLD` static, and the C API.
- `GameClient/src/w3d/{renderer,device,post_processing}.rs`: 13 pipelines with no caller outside `w3d/`.
- `ww3d-render-2d`, `ww3d-effects`, `ww3d-scene`, `ww3d-scene-renderer`, `ww3d-compat`, and `wwshade` are only reachable from `ww3d-compat`/Integration/GameEngineDevice.
- The `WgpuMainRenderer` "legacy backend" mode (`WgpuWrapper`, `wgpu_main_renderer.rs:203-283` and `:606-660`) is only used by `WgpuMainRenderer::new` (tests and demos). It hard-codes `Depth24PlusStencil8` and 4x MSAA, which the engine path never uses.
- In `ww3d-gpu`: `PIPELINE_CACHE` (`pipeline_cache.rs:231`), `DYNAMIC_BUFFERS` (`dynamic_buffer.rs:377`), `GPU_CONTEXT` (`lib.rs:515`), `SHADER_CONSTANTS` (`shader_constants.rs:248`), and the `pipeline`/`texture`/`buffer` managers held by `Renderer` (`ww3d-renderer-3d/src/lib.rs:241-244`) (v).
- `ww3d-renderer-3d/src/rendering/shader_system/pipeline_cache.rs` is a second ShaderClass pipeline cache (v).
- `GraphicsSystem` global uniform buffer, material bind groups, and the `update_global_uniforms` upload (`graphics_system.rs:125-212, 262`) have no draw consumer. Only the model cache and device accessors are used.
- `Main/src/simple_game_engine.rs`, `win32_game_engine.rs`, `src/bin/*` demos, `Experimental/game_runtime`, `Tools/*`, and `*.bak*` files (`wgpu_polygon_renderer.rs.bak2`, `wgpu_pipeline_manager.rs.bak2`, `wgpu_shader.rs.bak2`, `Main/src/effects/lighting_system.rs.bak3/.bak4`).
- `RenderPipeline::execute_ui_pass` (`pipeline_collect.rs:2220`) is an empty body.

### 1.6 Resource churn and hot-path problems (file:line evidence)

Lone Eagle baseline: present takes about 144 ms, of which forward takes 76.5 ms
(`docs/lone_eagle_perf_baseline.md`, bead hq-x1j4u.6.5 notes). Most of the forward cost has
the shape shown below.

**Mesh path. Per draw, per material pass, per frame** (`render_manager.rs:1133-1330`):
- `WgpuMaterialBinds::camera` creates a new bind group (`wgpu_material_binds.rs:158`) even though the camera is constant for the whole frame.
- `WgpuMaterialBinds::model` creates a new bind group (:235) over arena slices.
- `WgpuMaterialBinds::uv_transform` calls `create_buffer_init` and creates a bind group (:370-377) **outside the arena**.
- Skinned meshes create a bones+uv bind group (:310).
- `create_texture_bind_groups` creates `MAX_TEXTURE_STAGE_GROUPS` new bind groups (render_manager.rs:1481-1533). It also calls `pipeline.get_bind_group_layout(i)` per group, which returns a fresh layout handle each call.
- `create_vertex_color_resources` re-uploads **static** vertex colors with `create_buffer_init(STORAGE)` and builds a new bind group (render_manager.rs:1412-1478).
- Root cause: each pipeline owns private bind-group layouts (`wgpu_pipeline_manager.rs:383`), so no bind group can be shared across pipelines or frames. The `FrameUniformArena` (`frame_uniform_arena.rs`) only removed buffer creation. It does not use dynamic offsets and issues one `queue.write_buffer` per allocation.
- Result: about 6 to 10 bind groups plus 1 to 3 buffers per material pass per frame.

**Live CSM** (`render_manager.rs:633-665`): 4 uniform buffers and 4 bind groups per frame, plus
one `create_buffer_init` and one bind group **per opaque caster**, and 4 extra depth passes that redraw every
opaque mesh. It is not a C++ feature (see section 3).

**Shadows** (`shadow_pass.rs:898, 982, 987, 1076`): a new uniform buffer, bind group, vertex buffer,
index buffer, and overlay buffer every frame.

**Particles** (`particle_renderer.rs:369-381`): one `create_buffer_init` per batch per frame
(`ParticleBatch::update_buffer`). Batches are rebuilt every frame (:1324-1328). Weather, tracers,
heat haze, and decals each allocate new buffers per call (:1479, :1597, :1666-1673, :1761).

**Water** (`terrain_visual/traits.rs:482-484`): `sync_polygon_water_meshes` runs every update and
calls `create_buffer_init` per water polygon (`overlay_gpu.rs:581-586`).

**Terrain chunks** (`impl_gpu.rs:410-450`): while any dynamic light is live, every visible chunk is
rebuilt with new VB/IB per frame. The buffers are created `COPY_DST` but never written in place.

**Textures**:
- No mipmaps. `mip_level_count: 1` on mesh pass textures (`render_manager.rs:1722-1734`). Minified
  textures alias and shimmer, while C++ uses DDS mip chains.
- DXT is decoded to RGBA8 on the CPU (`Main/src/graphics/render_pipeline/mod.rs:92-131`). This costs 4x to 8x
  VRAM, CPU time on first use, and a blocking `prime_texture_raw_blocking` under the asset-manager mutex.
  `TEXTURE_COMPRESSION_BC` is never requested (`EngineConfig.features = empty`).
- The same texture name can be uploaded independently by the mesh manager (`gpu_texture_views`),
  the ForwardPass `texture_cache`, the particle renderer, the terrain `TextureManager`, and the UI renderer.
  There is no shared handle.

**UI** (`ui_renderer.rs:1556-1675, 1679-1890`):
- Every distinct string is rasterized into its own texture. Up to 4096 cached textures, evicted in HashMap order, which is effectively random.
- Every label is a separate texture switch and draw.
- One `draw_indexed` per command. Consecutive commands with the same pipeline, texture, and scissor are never merged.
- The texture bind-group cache is cleared wholesale at the cap (:1810-1812).

**Sync points**: screenshot and capture readbacks only (`ww3d-engine/src/lib.rs:1545, 1716, 1933`).
None run per frame on the game path. Good.

**Command-buffer lifetime**: `Engine::begin_render` creates the frame encoder **before** surface
acquire (lib.rs:1145-1147) and drops it on acquire errors. `FrameSubmitState::on_begin_frame`
drops pending, unsubmitted command buffers (`frame_submit.rs:97-102`). Both are candidates for
the intermittent Metal "4097 created-not-submitted command buffers" failure (hq-t8rkx, folded,
root cause unresolved).

### 1.7 Process-wide or thread-bound state on the render path (blocks multiple instances)

This table records the original audit, not current status; see the progress table above.

| State | Where | Live? |
|---|---|---|
| `SORTING_RENDERER: Mutex<SortingRenderer>` (lazy_static) | `ww3d-gpu/src/sorting_renderer.rs:334` | **yes**. Every sorted translucent mesh goes through it (`render_manager.rs:1361`, `ww3d-renderer-3d/src/lib.rs:706-731`). Two instances would flush each other's triangles. |
| `PUBLISHED_TERRAIN_VISUAL` | `GameClient/src/terrain/terrain_visual/api.rs:8` | yes (C++ `TheTerrainVisual` stand-in) |
| shadow pass state through `live_game_client_frame()` / `with_live_game_client_mut` | `shadow_pass.rs:45-62`. GPU pipelines live on the published client | yes. Falls back to an **ephemeral** state if no client is published, which recreates pipelines every call |
| `OBJECT_REGISTRY`, `get_global_data()` read inside the shadow pass | `shadow_pass.rs:126-160` | yes. The renderer reads simulation globals directly |
| `get_asset_manager()`, `init_archive_file_system()` | `Main/src/graphics/render_pipeline/mod.rs:12`, `boot.rs:449` | yes |
| `set_live_renderer_3d` publish | `forward_render.rs:141` | yes |
| `PIPELINE_CACHE`, `DYNAMIC_BUFFERS`, `GPU_CONTEXT`, `SHADER_CONSTANTS`, `VertexMaterial PRESETS` | ww3d-gpu / renderer-3d | mostly dead, but compiled into the game |
| `static LOGGED_STARTUP_RENDER_ITEM_SUMMARY: OnceLock<()>` | `pipeline_execute.rs:405` | harmless, but per-process |

**Budget hole:** `scripts/count_globals.sh` and `check_sealed_modules.sh` match
`static NAME:` but not `lazy_static! { static ref NAME: ... }`, so `SORTING_RENDERER`,
`PIPELINE_CACHE`, `DYNAMIC_BUFFERS`, and `SHADER_CONSTANTS` are invisible to the "all zeros" baseline.

---

## 2. Correctness gaps vs C++ (picture-level)

C++ reference order: `RTS3DScene::Flush`, `GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DScene.cpp` (around lines 808-845):

```
DoShadows(non-stencil decals)  ->  DX8MeshRenderer.Flush (opaque)  ->  occluded objects into stencil
->  DoTrees  ->  DoShadows(stencil volumes)  ->  Render_And_Clear_Static_Sort_Lists (water)
->  flushTranslucentObjects  ->  DoParticles + SortingRendererClass::Flush (per-poly sorted)
```
After that, W3DDisplay draws the 2D/UI on top (`W3DDisplay.cpp:1864` `TheInGameUI->draw()`).

| # | Gap | Rust evidence | Effect |
|---|---|---|---|
| G1 | **Water after translucent meshes and particles** | water is a post-frame pass after the main and sorting passes (`pipeline_execute.rs:560`, `wgpu_main_renderer.rs:588`) | water paints over smoke, explosions, and translucent meshes near shore |
| G2 | **Shadows after particles** | shadow callback is enqueued after ClientEffects (`pipeline_execute.rs:235` vs `:536`) | shadow fill darkens particles and decals |
| G3 | Particles not in the sorting pass | particles draw in their own pass after the sorting flush (`particle_renderer.rs:1354`) | ordering vs translucent meshes is wrong. C++ sorts particle polys together with other sorted polys |
| G4 | **Sorting is per batch, not per triangle** | `add_sorted_triangles_with_state` sorts by mesh-origin distance (`render_manager.rs:1331-1375`). C++ sorts every triangle in an overlapping pool by view z (`GeneralsMD/.../WW3D2/sortingrenderer.cpp:372-476`) | intersecting translucent geometry (tree cards, smoke, glass) pops |
| G5 | **No stencil** | `Depth32Float` (`ww3d-engine/src/lib.rs:89`). `present_volumetric_shadows` returns `SkippedNoStencil` (`shadow_pass.rs:169-176`) | C++ stencil volume shadows and "occluded behind building" stencil silhouettes are missing. Volumes are approximated as box prisms (`push_volume_prism` :805) even when drawn |
| G6 | **sRGB framebuffer and textures** | sRGB surface (lib.rs:2177). Every texture is `*UnormSrgb` (render_manager.rs:1705-1712, terrain `impl_gpu.rs:1524`, UI :992, particles :1850) | blending, vertex-color modulation, and fog all run in linear space. DX8 runs them in gamma space, so additive FX, alpha fades, and lit colors differ visibly. Related: hq-6q5wge (gamma ramp) |
| G7 | No mipmaps | `render_manager.rs:1728` | shimmering and aliasing at RTS zoom |
| G8 | CSM shadows do not exist in C++ | `render_manager.rs:579`, model binds pass `Some(&self.live_csm)` (:1223) | extra shading that the C++ frame does not have, plus large cost |
| G9 | Prelit lightmap passes not loaded | hq-m4hr | buildings look flat |
| G10 | Per-face texture IDs | hq-5jycc (merged into hq-m4hr) | wrong textures on multi-texture meshes |
| G11 | Skinned meshes ignore blend-specific shader | `wgpu_pipeline_manager.rs:109`: `Skinned` always uses `skinned.wgsl` | possible alpha-test/additive mismatch on skinned translucent units (v) |
| G12 | Laser pipeline format fixed to `Bgra8UnormSrgb` | `forward_render.rs:256` | validation error or wrong output on any other surface format |
| G13 | Terrain blend/shroud, roads, bridges, props, water grid fidelity | hq-x1j4u.6.5, .5.1-.5.3 | open feature work. Out of scope for this architecture plan, but the plan gives those features a home |
| G14 | HUD clipping/flashing | hq-j3dq0 | UI batching (step R10) and scissor ownership help with this |

---

## 3. Target architecture

### 3.1 Principles

1. **One `Renderer` per game instance.** It owns every GPU object except the shared
   `Arc<GpuContext>` (instance, adapter, device, queue, caps). No `static`, `thread_local`,
   `lazy_static`, or `OnceLock` holds GPU resources or render state.
2. **Explicit frame schedule.** A frame is a fixed, enum-ordered list of passes that mirrors
   `RTS3DScene::Flush` plus W3DDisplay's 2D phase. No FIFO callback queues decide draw order.
3. **Features never touch `wgpu::Device` directly at draw time.** They record into
   `PassEncoder`s using pre-created pipelines, cached bind groups, and ring-allocated
   dynamic data. All `create_*` calls happen in `prepare` or at load time, and the frame stats prove it.
4. **One material model.** `ShaderKey` (bitflags mirroring W3D `ShaderClass` bits) plus vertex
   layout plus target formats selects a pipeline from one cache. Fixed bind-group layouts are
   shared by all W3D-style pipelines.
5. **Gamma-space blending like DX8.** Use `*Unorm` (non-sRGB) render targets and textures. An optional
   final gamma-ramp pass handles hq-6q5wge.
6. **Sealed modules.** Each module below has a small public API and a private
   implementation, and gets a `sealed_modules.toml` entry when it reaches parity.

### 3.2 Crate and module layout

Keep the existing crates. Shrink them into this shape instead of adding a new crate:

```
ww3d-gpu            -> gpu context only: GpuContext, DeviceAuthority, caps, counters.
                       (delete DX8 compat globals: PIPELINE_CACHE, DYNAMIC_BUFFERS,
                        GPU_CONTEXT, SHADER_CONSTANTS, SORTING_RENDERER)
ww3d-engine         -> render::surface + render::frame (Engine is renamed to Renderer
                       root; keeps begin/end, screenshot, resize)
ww3d-renderer-3d    -> render::{resources, material, queues, mesh}
GameClient          -> feature passes that need game data (terrain, water, particles,
                       shadows, decals, filters, ui) implement `RenderFeature`
Main                -> builds the per-frame `FrameInputs` from the presentation frame;
                       no wgpu calls
```

Module boundaries and public API (sketch):

```rust
// ww3d-gpu
pub struct GpuContext { /* instance, adapter, device, queue, caps */ }
impl GpuContext {
    pub async fn new(desc: &GpuContextDesc) -> Result<Arc<Self>, GpuError>;
    pub fn device(&self) -> &wgpu::Device;
    pub fn queue(&self) -> &wgpu::Queue;
    pub fn caps(&self) -> &GpuCaps;           // bc_compression, depth_stencil formats...
}

// render::surface
pub enum RenderTarget { Window(WindowSurface), Offscreen(OffscreenTarget) }
pub struct TargetDesc { pub size: PhysicalSize, pub color: wgpu::TextureFormat /* Bgra8Unorm */,
                        pub depth: wgpu::TextureFormat /* Depth24PlusStencil8 */,
                        pub present: PresentPolicy }
impl RenderTarget { pub fn resize(&mut self, ..); pub fn set_present_policy(&mut self, ..); }

// render (root, one per game instance)
pub struct Renderer { gpu: Arc<GpuContext>, target: RenderTarget, res: Resources,
                      materials: MaterialCache, queues: FrameQueues,
                      features: Features, stats: FrameStats }
impl Renderer {
    pub fn new(gpu: Arc<GpuContext>, target: RenderTarget, cfg: RendererConfig) -> Self;
    pub fn resources(&mut self) -> &mut Resources;     // load-time uploads
    pub fn render(&mut self, inputs: &FrameInputs) -> Result<FrameStats, RenderError>;
    pub fn capture(&mut self, req: CaptureRequest);    // async readback, never blocks frame
    pub fn resize(&mut self, size: PhysicalSize);
}

// render::frame  (render-graph-lite: fixed enum order, no dynamic graph)
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PassId {
    ShadowCasterTextures,   // projected shadow RTs (W3DProjectedShadow) - offscreen
    WaterReflection,        // m_waterType==2 render-to-texture - offscreen
    Terrain,                // clear + HeightMap + roads/bibs/scorch (tactical viewport)
    DecalShadows,           // DoShadows(false): blob/projected decals under objects
    Opaque,                 // DX8MeshRenderer.Flush
    OccludedStencil,        // flushOccludedObjectsIntoStencil
    Trees,                  // DoTrees
    StencilShadows,         // DoShadows(true)
    StaticSort,             // Render_And_Clear_Static_Sort_Lists: water
    Translucent,            // flushTranslucentObjects
    Sorted,                 // DoParticles + SortingRenderer::Flush (lasers, streaks too)
    ViewFilter,             // W3DShaderManager filters / motion blur / B&W
    LockedDrawable,         // post-filter camera-locked drawable
    ScreenFade,
    Ui,                     // TheInGameUI->draw / window manager / cursor
    GammaRamp,
}
pub trait RenderFeature {
    fn prepare(&mut self, ctx: &mut PrepareCtx, inputs: &FrameInputs);   // uploads, culls
    fn record(&self, pass: PassId, enc: &mut PassEncoder<'_>);           // draws only
    fn passes(&self) -> &'static [PassId];
}
```

`Frame` merges adjacent `PassId`s that share attachments and load/store ops into one
`wgpu::RenderPass`. For example, `Terrain..Sorted` becomes a single color+depth+stencil pass
whose viewport/scissor is set once from the tactical viewport, so most frames have about 3 to 5 real
render passes. `ViewFilter` breaks the pass because it samples the scene color.

```rust
// render::resources
pub struct TextureHandle(NonZeroU32);   // generation-checked slot
pub struct MeshHandle(NonZeroU32);
pub struct Resources {
    textures: TextureCache,     // name -> handle; BC1/2/3 when caps allow, full mips
    meshes: MeshStore,          // static VB/IB/vertex colors per W3D MeshModel
    rings: FrameRings,          // per-frame-in-flight ring: dynamic VB, IB, uniforms
}
impl Resources {
    pub fn texture(&mut self, name: &str, src: &dyn TextureSource) -> TextureHandle;
    pub fn mesh(&mut self, model: &MeshModelClass) -> MeshHandle;
}
// FrameRings: N=frames-in-flight (2) large buffers; `alloc_uniform<T>() -> DynOffset`,
// `alloc_vertices(&[V]) -> BufferSlice`; one write_buffer per ring per frame (staging
// via queue.write_buffer_with or a mapped staging belt).

// render::material
bitflags! { pub struct ShaderKey: u32 { /* exact W3D ShaderClass bit layout:
    depth compare, depth mask, color mask, dst/src blend, fog func, pri/sec gradient,
    texturing, alpha test, cull, post-detail color/alpha */ } }
pub struct PipelineKey { shader: ShaderKey, layout: VertexLayoutId, stages: StageMask,
                         target: TargetFormats, variant: PassVariant /* normal, shroud, ghost, heat */ }
pub struct MaterialCache { /* pipelines: HashMap<PipelineKey, wgpu::RenderPipeline>,
    shader modules compiled once per (wgsl, defines), shared BGLs */ }
// Shared bind group layout scheme for every W3D pipeline:
//   group 0  Frame     : camera, fog, sun/ambient, shroud texture+transform  (1 per frame)
//   group 1  Object    : transform, lighting env, material colors, opacity, fow  (dynamic offset)
//   group 2  Material  : up to 4 stage textures + samplers, uv transforms  (cached by key)
//   group 3  Skin      : bone palette storage                               (dynamic offset)

// render::queues
pub struct FrameQueues { opaque: Vec<DrawItem>, translucent: Vec<DrawItem>,
                         sorted: SortingQueue, static_sort: [Vec<DrawItem>; MAX_SORT_LEVEL] }
// SortingQueue = W3D SortingRendererClass semantics: nodes with view-space bounds,
// overlapping-pool per-triangle z sort (sortingrenderer.cpp:372-476), vertices
// transformed on CPU into the ring VB, max 65535/3 polys per flush chunk.
```

Inputs come in, and nothing calls back out. `FrameInputs` is an immutable snapshot built by Main
from the `PresentationFrame`: camera, tactical viewport, lights, fog, shroud grid,
`DrawItem`s, particles, lasers, decals, terrain deltas, and the UI command list. Feature modules never
read `OBJECT_REGISTRY`, `get_global_data()`, the published GameClient, or the asset manager
during `record`. Asset resolution happens in `prepare` through an injected
`&dyn AssetSource`.

### 3.3 Fidelity decisions (explicit, testable)

- **Formats:** surface `Bgra8Unorm` (falling back to the first non-sRGB format), textures `*Unorm`.
  `Depth24PlusStencil8` (falling back to `Depth32FloatStencil8`). No MSAA by default, as in the C++ default.
- **Alpha test:** `discard if a < 96/255` only when `ShaderKey::ALPHA_TEST` is set. This is already
  correct in `gate_alpha_test_discard`. Move it to a pipeline-overridable constant instead of
  string-patching the WGSL.
- **Fog:** per-vertex linear fog in gamma space (D3DFOG_LINEAR).
- **Render resolution:** keep logical (C++ resolution) as the 3D/UI coordinate space, but allow
  rendering at physical size with a UI scale. This is a policy flag that defaults to the current behavior.
- **Shadows:** the C++ set only, meaning projected texture/decal shadows plus stencil volumes from the real
  silhouette (`W3DVolumetricShadow`). CSM is removed or kept behind a debug flag only.

---

## 4. Migration plan (small, independently mergeable steps)

Every step must pass the existing gates (`present_frame_gate`, default build, LOC/unsafe
ratchets, `count_globals.sh --compare`). Each step adds or uses the verification tools from
R0.

**Verification vocabulary**
- **Golden-R** is a Rust self-golden. A deterministic headless capture (fixed camera, frozen logic
  frame, fixed time, 800x600, Lone Eagle plus ShellMapMD) is taken at the previous commit and the new one.
  A pure refactor must be **pixel-identical**. If not, the diff image is attached and explained.
- **Golden-C** is a C++ reference. The same camera and logic frame are captured from retail ZH under
  Windows (or Wine/DXVK) once and stored under `validation/goldens/`. Comparison uses per-tile
  mean ΔE plus edge SSIM with a documented threshold. It is used for steps that intend to change pixels.
- **Stats** come from `FrameStats` per frame: passes, draws, pipelines bound, and
  `create_{buffer,bind_group,texture,pipeline}` counts (wgpu `counters` feature /
  `Device::get_internal_counters`, plus our own counters), bytes uploaded, and CPU ms per phase.
  Steady-state targets are stated per step.

| Step | Change | Files | Verification |
|---|---|---|---|
| **R0** | **Harness.** (a) `FrameStats` with resource-creation counters, pass/draw counts, upload bytes, exposed in `--present-frame` smoke output. (b) Deterministic `golden_capture` mode in the smoke harness (headless OffscreenTarget, fixed camera/frame/time) writing PNG + JSON stats. (c) Fix `count_globals.sh` / `check_sealed_modules.sh` to count `lazy_static!` `static ref`, then re-baseline (the count goes **up** honestly once and must then go down). | `ww3d-engine/src/lib.rs`, `Main/src/executable_smoke/*`, `scripts/*.sh` | The harness reproduces itself bit-exactly twice in a row. Stats are printed. Globals count includes SORTING_RENDERER and the others |
| **R1** | **Delete dead render paths**: `GameClient/src/w3d/{renderer,device,post_processing}`, the WgpuMainRenderer legacy backend, ww3d-gpu `PIPELINE_CACHE/DYNAMIC_BUFFERS/GPU_CONTEXT/SHADER_CONSTANTS`, `shader_system/pipeline_cache.rs`, `GraphicsSystem` uniform/material GPU state, `*.bak*`, and the empty `execute_ui_pass`. Drop the `Code/GameEngineDevice` archive crate from workspace members. | listed in section 1.5 | Default build. Golden-R identical. LOC ratchet down |
| **R2** | **Per-renderer SortingQueue.** Move `SortingRenderer` from the `ww3d-gpu` global into `Renderer`/`MeshRenderManager`. `add_sorted_*` takes `&mut SortingQueue`. | `ww3d-gpu/src/sorting_renderer.rs`, `render_manager.rs:1331`, `ww3d-renderer-3d/src/lib.rs:706-731` | Golden-R identical. New test: two headless renderers in one process draw different translucent scenes and each matches its single-instance golden. Globals count -1 |
| **R3** | **Explicit frame schedule (order-preserving).** Replace `pre_scene_callbacks`/`post_frame_callbacks`/`ScopedNativePass` FIFO with `PassId`-keyed recording. The first version encodes **today's** order exactly. | `wgpu_main_renderer.rs:535-800`, `pipeline_execute.rs`, `forward_render.rs`, `native_terrain_pass.rs` | Golden-R identical. Stats: same pass count. No boxed closures per frame |
| **R4** | **C++ draw order.** Reorder `PassId` to `RTS3DScene::Flush`: decal shadows before opaque, stencil shadows and water before translucent, and particles plus lasers inside `Sorted`. Fixes G1-G3. | frame schedule only | Golden-C on a shoreline with smoke (Lone Eagle river) plus a unit shadow under particles: ΔE improves vs the R3 capture. Golden-R diff images reviewed and attached |
| **R5** | **Shared bind-group layouts plus dynamic offsets in the mesh path.** Fixed BGL scheme (section 3.2). Frame group built once per frame. Object uniforms via `FrameRings` dynamic offsets (one `write_buffer` per frame). Material bind groups cached by `(texture handles, sampler state, uv mode)`. Static vertex colors move to `MeshStore` at model registration. Remove `get_bind_group_layout` calls. | `wgpu_pipeline_manager.rs`, `wgpu_material_binds.rs`, `render_manager.rs:1133-1533`, `frame_uniform_arena.rs` | Golden-R identical. Stats: steady-state `create_bind_group` == 0 and `create_buffer` == 0 after warm-up. `forward` ms in the Lone Eagle baseline drops (target < 1/3 of 76.5 ms debug) |
| **R6** | **Persistent rings for dynamic geometry**: particles, weather, tracers, heat haze, decals, shadow volumes/overlays, polygon water, lasers. Terrain chunk dynamic-light rebake writes in place (`queue.write_buffer` into existing COPY_DST buffers). | `particle_renderer.rs`, `shadow_pass.rs`, `overlay_gpu.rs:571-590`, `impl_gpu.rs:410-450` | Golden-R identical. Stats: steady-state `create_buffer` == 0 in combat on Lone Eagle |
| **R7** | **Unified TextureCache.** One cache per Renderer with `TextureHandle`. Mesh, terrain, particle, and UI all resolve through it. Upload DDS mip chains. Use BC1/BC2/BC3 when `TEXTURE_COMPRESSION_BC` is available, with CPU decode as fallback (wasm/WebGL). Generate mips for TGA. | `render_manager.rs:1658-1757`, `Main/.../render_pipeline/mod.rs:92-150`, `particle_renderer.rs:1840-2090`, `terrain/textures.rs`, `ui_renderer.rs:1379` | Golden-C (minification fixes G7). Stats: texture memory drops, and each texture uploads once (count by name) |
| **R8** | **Color space (G6).** Non-sRGB surface and textures. Audit every WGSL file for manual gamma math. Optional `GammaRamp` pass (hq-6q5wge). | `ww3d-engine/src/lib.rs:2164`, all `*UnormSrgb` sites (section 1.6), WGSL | Golden-C: additive FX and alpha fades. Histogram comparison on the shell map menu and an in-game explosion frame |
| **R9** | **Depth-stencil and C++ shadows (G5, G8).** `Depth24PlusStencil8`. Stencil volumes from real silhouette edges (port `W3DVolumetricShadow`). Occluded-object stencil silhouettes. Remove the live CSM. | `ww3d-engine` config, `shadow_pass.rs`, `render_manager.rs:579-830` | Golden-C with units behind buildings and tank shadows on a slope. Stats: 4 fewer passes, and CSM caster draws go to 0 |
| **R10** | **Per-triangle SortingQueue (G4)**, matching `sortingrenderer.cpp` (CPU view-z sort over the overlapping pool into a ring VB, 65535-index chunks). | `SortingQueue` | Golden-C on tree cards plus smoke plus a translucent building. Unit test: deterministic triangle order equal to the C++ algorithm on a fixture |
| **R11** | **UI batching**: glyph atlas (cache glyphs per font/size, not strings), mapped-image atlas pages, merge consecutive commands with equal (pipeline, texture, scissor) into one draw, and scissor in the command key (helps hq-j3dq0). | `ui_renderer.rs` | Golden-R on shell menus and the in-game HUD (pixel-identical except text AA, which is reviewed). Stats: UI draws per frame on the main menu drop by more than 5x. Text texture count is bounded |
| **R12** | **Pass merging and surface policy.** Merge adjacent compatible PassIds into one wgpu pass. Wire `set_swap_interval` to reconfigure `present_mode`. Create the encoder only after a successful acquire, and never drop recorded command buffers (hq-t8rkx). | `ww3d-engine/src/lib.rs:1100-1320`, `frame_submit.rs:97-102` | Stats: real render passes per frame at most 6 in game. The 180 s windowed gate has no Metal command-buffer warnings |
| **R13** | **Instance isolation and sealing.** Remove the remaining render-path globals (`PUBLISHED_TERRAIN_VISUAL`, shadow-pass `live_game_client` access, `OBJECT_REGISTRY`/`get_global_data` reads, `set_live_renderer_3d`) by passing them through `FrameInputs` and injected sources. Add modules to `sealed_modules.toml`. | section 1.7 sites | Two `Renderer`s in one process (headless), each with its own map, match their single-instance goldens. `check_sealed_modules.sh` passes. Globals count for render crates is 0 |

Dependencies: R0 comes before everything else. R1 and R2 can merge in either order. R3 comes before R4 and R12.
R5 comes before R6 and R7, because both rely on the shared BGL and ring. R8 needs R7, since there is a single texture
format choice point. R9 needs R3. R10 needs R2 and R5. R11 is independent after R0. R13 comes last.

## 5. First three steps to implement

1. **R0: harness and counters.** Without a deterministic golden and per-frame creation
   counters, no later step can be verified. It also closes the `lazy_static` hole
   in the globals budget.
2. **R2: per-renderer SortingQueue, with R1 deletions folded in.** It removes the only live
   process-global that touches GPU draw state on the mesh path, and it is a small,
   pixel-identical change that proves the multi-instance test.
3. **R3: explicit `PassId` frame schedule (order-preserving).** It replaces three FIFO
   callback mechanisms with one ordered schedule. That is the foundation for the C++ draw-order fix (R4),
   pass merging (R12), and every feature module boundary.

R5 (mesh bind-group churn) is the biggest performance win and comes right after R4.
