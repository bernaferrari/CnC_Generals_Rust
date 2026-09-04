# Windowed Smoke — viewport extent fix + five-flag drive frontier (WindowedSmoke2, 2026-09-02)

Scope: verify WindowedSmoke's depth-format fix; land the viewport/scissor/swapchain fix
(parent-directed priority); attempt the physical (CGEvent) five-flag sit-through; document
blockers precisely. Serial runs only, no git writes, no formatters. Harness tooling kept
under `/tmp/wsmoke/` (durable, documented below) — repo tree carries only the render fixes.

## 1. Depth-format fix — VERIFIED in tree (no edit needed)

- `Code/GameEngine/src/terrain/terrain_visual/visual_struct.rs` —
  `TERRAIN_PIPELINES_DEPTH_FORMAT = Depth32Float` with the wgpu validation-crash rationale
  (pipeline authored `Depth24PlusStencil8` fatalled against the `Depth32Float` attachment).
  All terrain pipelines + river_gpu + shroud_gpu use the shared constant.
- `Code/Main/src/graphics/laser_draw.rs:151` — post-frame laser pass pinned to
  `Depth32Float` ("Must match the pass this pipeline draws into … wgpu fatals on
  pipeline/attachment mismatch").
- Confirmed live: windowed runs present frames (`live_frame_ok=true` latched from windowed
  surface present), `render_item_count=31`, 760 gameworld presentation entities in InGame.

## 2. Viewport/swapchain extent fix — LANDED (my lane)

**Root cause (measured, not inferred):** `WW3D surface configured: size=1280x960` while the
window is 640x480 logical (scale factor 2). The WND window manager, UI renderer, and gadget
draw paths all author/draw in logical points, so the UI painted the top-left 640x480
quadrant of a 1280x960 surface — exactly the "~500x500 render, rest black" defect.

**Fix — swapchain extent follows the logical window size (C++ `TheDisplay` parity):**
- `cnc_game_engine/types.rs` — new `render_surface_extent(&Window) -> (u32,u32)`
  (`inner_size / scale_factor`), the one shared definition of render extent.
- `cnc_game_engine/boot.rs` — engine config (surface init) uses the logical extent;
  the later camera-aspect/UIManager seed now receives `LogicalSize` (logical).
- `cnc_game_engine/input.rs` `resize()` — converts the winit physical size to logical
  before `ww3d_engine::resize` (surface reconfigure + depth resize) and
  `ui_manager.resize`; per-frame `set_tactical_3d_viewport` uses the logical extent.
- `cnc_game_engine/start_game.rs` — `tactical_viewport_size()` (3D view, projection,
  mouse unprojection) and the GameHUD seed use the logical extent.
- `cnc_game_engine/hotkeys.rs` — diplomacy panel resize uses the logical extent.
- Input space untouched: `mouse_position` is already logical (`apply_cursor_position`
  divides by scale), so mouse math is now in the SAME space as the render target.

**Scissor fatal guard — LANDED (blocks every windowed run otherwise):** with a correct
640x480 surface, the shell draw path's out-of-layout scissors became fatal:
`Scissor Rect {x:140,y:540,w:500,h:1} is not contained in the render target (640,480)`
(abort trap 6; release `panic=abort`). These scissors arrive in raw 800x600
CREATIONRESOLUTION coordinates (MenuWidgets' lane: layout→WM screen scaling).
`GameEngine/GameClient/src/gui/ui_renderer.rs` (`render`, ~1523): a scissor whose origin is
outside `screen_size` now skips the draw (`continue`) instead of issuing the fatal command;
partially-overlapping scissors still clamp. This is additive and stays correct once the
layout scaling lands.

**Verification:** `cargo check -p generals_main --bin generals` clean; release build;
windowed boot → Menu → present → `request_capture` PNG. With my extent fix (pre-MenuWidgets
snapshot) the C&C logo drew full-size at its correct 640x480 position (previous runs: it
rendered inside the top-left quadrant). Evidence PNGs: `/tmp/wsmoke/menu_frame.png`,
`check_menu.png`, `fiveflag_frame.png` (latest snapshot black — caught MenuWidgets'
mid-refactor shell layout; their scaling fix is in flight).

## 3. Five-flag status (honest)

| flag | state | evidence |
|---|---|---|
| window_visible | **TRUE** | winit query published `window_visible=true`; window on screen (desktop screenshot) |
| live_frame_ok | **TRUE** | `Runtime host latched live_frame_ok from windowed surface present`; promoted `frame.png` captures |
| ingame | **TRUE** (automated chain) | runs reach `state=InGame`, `select_ok`, presentation entities 760 |
| wnd_widget_tree_nav | FALSE | requires physical Menu→match click chain — not yet completed (see blockers) |
| gameplay (interactive) | FALSE | requires `match_started_from_menu_wnd` (same physical chain) |

`retail_sit_through_missing=wnd_widget_tree_nav,gameplay` on every observed run — the two
flags that by design require real OS mouse input.

## 4. Physical drive: mechanics proven, final chain blocked — precise blockers

Working, durable harness (all under `/tmp/wsmoke/`, reusable):
- `generals_clicker7/8` (swiftc-built): activate-by-pid + HID move/down/up; clicker8 does
  click-to-activate at an unoccluded point then double-clicks the target.
- `window_move|x=..|y=..` control command (existing host feature): parks the OS window in
  point space so gadgets sit outside overlapping IDE windows — VERIFIED
  (`window_move: requested 1151,260 pt -> winit outer 1151,260 640x512`, drain logged).
- `winlist`/`osrect.sh`: live CG window rect for truth-vs-child-coordinate mapping
  (macOS constrained the frame to 577x462 @ (1122,289) in one observation while the child
  believed 640x512 @ (1151,260) — clicks must map through the OS rect).
- `spawnchild.sh`, `drive6-10.sh`: child launch exactly per `bootstrap.rs`
  (`-runtime_host=windowed -windowed -width=640 -height=480` + gpui_* files), status-driven
  phase machine (Menu → SP reveal → SK → `start_game` → InGame RMB), with evidence logging.

Physical evidence that DID latch earlier (pre-sleep): physical Menu clicks latched
`menu_wnd_click` (reveal fired — `ButtonSinglePlayer` became hittable only after first
input, matching the C++ first-run reveal), and InGame physical RMBs were issued
(`activated=true` windows; `selected_count=1`).

**Blocker 1 (environmental, resolved late):** the display SLEPT mid-session
(`screencapture` returned an all-black 3456x2234 frame). A sleeping display blackholes HID
clicks and app activation — every "silently ignored click" after ~03:00 is explained by
this. Wake with `caffeinate -u -t 3`; drivers should do this at start. The two remaining
flags were not completed post-wake within budget.

**Blocker 2 (cadence):** Menu-state frames take ~3.7 s in release
(`RenderPipeline breakdown: total=3.74s … render_items=0` — forward-pass pathology, not
mine). Control-file writes are per-frame drained, so drivers must write one command and
wait ≥6 s; drive7's back-to-back writes clobbered `window_move` before the drain (proven by
the drain log). Perf ownership: graphics/render_pipeline (not the smoke harness).

**Blocker 3 (sibling churn):** the shell menu content is MenuWidgets' active lane
(GadgetButton/StaticText children, 800x600→WM scaling); snapshots of
`target/release/generals` taken mid-refactor render a black menu and publish zero
`gadget_hit` lines (pre-reveal dropdown hidden + layout in flux). My drive runs against a
snapshot binary (`/tmp/wsmoke/generals_snap`); re-run `drive10.sh` after MenuWidgets lands,
with `caffeinate -u -t 3` first.

## 5. Handoff for the next driver run (five-flag completion)

1. `caffeinate -u -t 3 && caffeinate -i -t 900 &` (keep display awake).
2. Build release with MenuWidgets' landed scaling; snapshot the binary.
3. `/tmp/wsmoke/drive10.sh` (or drive9 for OS-rect-mapped clicks) — it already:
   boots → `window_move` → reveal click at the unoccluded strip (x>1635) → SP (frac
   805,290) → SK (805,323) → `start_game` → InGame physical RMB (select via the automated
   `select_local_unit` or physical LMB) → polls `gameplay=true`, copies `frame.png`.
4. Expected latch order: `menu_wnd_click`+`skirmish_path` (SP/SK clicks) →
   `match_started_from_menu_wnd` (start from Menu) → `wnd_widget_tree_nav=true` →
   physical RMB with selection → `gameplay=true` → `playable_claim=true` in status.

Changed files (this pass): `types.rs`, `boot.rs`, `input.rs`, `start_game.rs`,
`hotkeys.rs`, `ui_renderer.rs` (GameClient). No test greens touched: all edits are
platform/bootstrap/render-lane; `cargo check -p generals_main --bin generals` clean.

---

## MenuWidgets — WND GadgetButton/StaticText shell-menu reveal (2026-09-02)

### Trace (evidence)
- Shell push/init runs: `MainMenuInit: Initializing main menu` → `window ids built` →
  `layout shown and dropdown hide attempted` (game_client_rust::gui::shell::main_menu, 06:00:06Z).
- First-run path fires in windowed runs: `First time running the game - hiding mouse and
  fading` → `not_shown=true`, dropdowns hidden by `apply_cpp_init_hide_pack`.
- `MainMenuUpdate` runs; first-run auto-reveal is C++-commented-out
  (GeneralsMD MainMenu.cpp:886-899): menu stays hidden until >20px mouse move or
  GWM_CHAR (MainMenu.cpp:982-1005). With no physical input, MapBorder2 + buttons stay
  HIDDEN → draw_window_hierarchy early-returns → no buttons/text on screen. This is the
  button/text defect root cause (verified: live capture shows ruler+backdrop drawing —
  i.e. the WM image-window draw dispatch works — while all PUSHBUTTON/STATICTEXT
  children are hidden).
- Layout scaling is NOT the bug (verified in debug logs): `resolve_window_rect` scales
  800x600 CREATIONRESOLUTION rects to WM screen_size (ButtonDiffBack 208x36 → 166x29 at
  640x480; MainMenuParent → 640x480). C++ parity: GameWindowManagerScript.cpp
  parseScreenRect (GameWindowManagerScript.cpp:467-540) scales by
  TheDisplay/createRes at parse time.
- Reveal chain once unhidden: WindowTransitions.ini `MainMenuDefaultMenuLogoFade`
  (FLASH MapBorder2 frames 4-7 → unhide; BUTTONFLASH buttons frames 11-15 → unhide),
  loaded via fallback `GeneralsRust/windows_game/extracted_big_files_v2/INI/WindowTransitions.ini`
  (has MainMenuDefaultMenuLogoFade; log line 243). WM::update → transitions.update
  advances per frame (probe verified: update#0..#29, #120, #240 while roots=0 pre-menu).
- Fonts init fine (FontLibrary + cosmic-text, 951 faces parsed).

### Fix landed (my lane)
`Code/GameEngine/GameClient/src/gui/shell/main_menu.rs` (+13): in MainMenuUpdate, when
`not_shown` after the just_entered delay AND `GENERALS_RUNTIME_HOST_WND=1` (unattended
runtime-host windowed run), run `reveal_hidden_main_menu` — the exact C++ reveal
sequence (winHide(FALSE) MapBorder2 + MainMenuFade immediate + MainMenuDefaultMenu +
cursor visible + not_shown=false). Interactive sessions keep C++ wait-for-input behavior.

### Sibling lanes (coordinated, do not duplicate)
- MenuTextures: button atlases `Buttons-Left/Middle/Right` live in EnglishZH.big
  (SCSmShellUserInterface512); hydration fails without zh_install_roots BIG paths →
  win_draw_image_ex paints raw draw-data tint 255 0 0 255 = the RED rectangles. Their
  fix (image.rs backend paths + honest fallback fill) lands the button/textures.
- WindowedSmoke2: swapchain extent + ui_renderer.rs scissor guard (out-of-target
  scissors skipped instead of wgpu fatal).

### Blocker (pre-existing, outside my lane): silent process death
5 of 6 windowed runs died silently (no panic in log; macOS report
`generals-2026-09-02-031123.ips`: SIGABRT via abort() in-process during WGPU asset init
or shortly after Menu entry; also seen mid-boot and ~40-90s after Menu). Menu render
currently ~3.7s/frame with 0 render items under sibling CPU contention. Because of this
the post-fix screenshot could NOT be captured this session (mw7 run died in boot).
Binary with fix built (release, 03:49) and launches; rerun
`GENERALS_RUNTIME_HOST_WND=1 ./target/release/generals -runtime_host=windowed -windowed
-width=640 -height=480 ... -gpui_frame=$D/frame.png` when the abort is fixed; expect
"revealing main menu" INFO then transition-driven unhide within ~15 transition frames.

---

## DriveRunner — physical five-flag drive + SIGSEGV/wedge root cause (2026-09-02 04:05-05:40)

### Drive result (honest): wnd_widget_tree_nav latch reached, final publication blocked by a render-path deadlock — now fixed in tree

Chain progress this session (release, GENERALS_RUNTIME_HOST_WND=1, window parked 1151,260 640x512,
clicker7 physical clicks aimed at engine-published `gadget_hit=Name@x,y` centers):
- `window_move` ok; menu reveal fired (`MainMenuUpdate: unattended runtime-host run — revealing main menu`).
- Physical click `ButtonSinglePlayer@1666,399` → "Single Player button selected" +
  **`InteractivePlayabilityEvidence: latched menu_wnd_click (windowed=true consumed=true hit=true)`**.
- Dropdown published `ButtonSkirmish@1666,527`; physical click →
  "physical GBM_SELECTED notify name=MainMenu.wnd:ButtonSkirmish" → skirmish_path latch set;
  SkirmishGameOptionsMenu pushed (WM 67→365 windows).
- `start_game` from Menu state → **`host_start_game_from_ui: InGame menu_match=true menu_click=true`**
  ⇒ menu_wnd_click + skirmish_path + match_started_from_menu_wnd all latched ⇒
  `wnd_widget_tree_nav` evidence chain COMPLETE (twice: runs at 08:13:43 and 08:21:30).
- InGame physical RMB/build + status publication of `wnd_widget_tree_nav=true`/`gameplay=true`
  NOT captured: the process died at InGame entry both times (below). Screenshots this session:
  /tmp/wsmoke/{ev_menu_revealed.png, skirmish_menu.png, menu_reveal_check.png, live_check.png, wake_desktop.png}.

### Blocker 1 (environmental, solved): snapshot binary cwd broke WND resolve — explained the "empty WM"
`main.rs set_working_directory_to_executable()` chdirs to the EXE dir. `/tmp/wsmoke/generals_snap`
→ cwd=/tmp/wsmoke → `resolve_window_script_path` finds no windows_game ancestor → first MainMenu
push fails LayoutError(InvalidParameter), and `do_push` LEAVES THE BROKEN SCREEN IN THE STACK;
the retry "succeeds" vacuously (screens=1) without creating any window → black menu, zero
`gadget_hit`, all named lookups "missing" (this was the real cause of the black-menu runs).
Fix used: run the snapshot from `GeneralsRust/target/release{,-with-debug}/generals_snap`
(exe-dir ancestor reaches windows_game). Suggested code fix (shell lifecycle, not done here):
pop the broken screen when `run_init` fails inside `do_push`.

### Blocker 2 (code, ROOT-CAUSED + FIXED in tree): same-thread RwLock read→write deadlock in list-box draw
Deterministic hang at the SkirmishGameOptionsMenu push and SIGSEGV(PAC) at InGame entry
(3 kills: 2 wedges + 1 crash; .ips generals_snap-2026-09-02-{050658,051348}).
Symbolicated sample (release-with-debug): main thread
`render (input.rs:1807) → RenderPipeline::execute (pipeline_execute.rs:300)
→ WgpuMainRenderer::end_frame (wgpu_main_renderer.rs:407) → flush_ui_to_frame (ui_render_pass.rs:221)
→ WindowManager::draw_all → draw_window_hierarchy_internal → w3d_gadget_list_box_image_draw
→ draw_mapped_image_clipped (list_box.rs:689) → parking_lot RwLock lock_exclusive_slow → cond_wait`.
Cause: callers held `get_mapped_image_collection().try_read()` while `draw_mapped_image_clipped`
re-took the same collection `.write()` on the same thread (std/parking_lot RwLocks are not
reentrant) — read→write self-deadlock; the release SIGSEGV is the sibling corruption of the
same pattern. Fix (list_box.rs only, minimal): `draw_mapped_image_clipped` now takes
`image_name: &str` (body looked up by name anyway); both list-box image sites compute geometry
inside a short read guard, DROP it, then draw by name; `draw_window_image_clipped` gates
existence, drops the guard, then calls. `cargo check -p generals_main --bin generals` clean.

### Handoff (next driver, ~10 min to green flags)
1. `cargo build --release --bin generals`; cp to `GeneralsRust/target/release/generals_snap`.
2. `caffeinate -u -t 3`; keep `caffeinate -dis` alive for the whole session (it died once and
   clicks stopped delivering until re-woken).
3. spawnchild (now points at release-with-debug — repoint to release), then when
   `gadget_hit=MainMenu.wnd:ButtonSinglePlayer@1666,399` publishes: click it; WAIT for
   `ButtonSkirmish@1666,527` to publish; click it; confirm GBM notify; send
   `start_game|mode=skirmish|faction=USA` immediately (drains within ~1-2 frames).
4. On InGame: physical RMB (select + move order) → `gameplay` latch; capture status + frame.png.

---

## DeadlockAuditRedrive — InGame SIGSEGV root-caused + CreditsMenu wedge root-caused + guard audit (2026-09-02 05:45-08:10)

### 1. InGame-entry SIGSEGV (generals_snap-2026-09-02-{050658,051348}.ips) — ROOT-CAUSED + FIXED, VERIFIED SURVIVING
Symbolicated both .ips against release-with-debug: NOT the gadget-draw path. Both crashes:
`run_loop.rs:221 → host_run_ingame_logic_presentation_frame (camera_drain.rs:773) → GameClient::update_presentation_shell (impl_update.rs:1389) → ParticleSystemManager::update (particle_manager.rs:906) → ParticleSystem::begin_frame_emit (particle_system.rs:1507, resolve_attached_parent) → query_live_drawable_fx_pose (live_slot.rs:200)` — EXC_BAD_ACCESS wild read (0x0c0888{8a,89}3a091818).
Root cause: `register_live_game_client` stores `&mut engine.game_client` as a raw address during `mark_initialized` — while the engine is still inside the boxed boot future (`run_loop.rs` `engine_init_future`). The engine is then MOVED out (`engine = Some(new_engine)`, run_loop.rs:494) and the box dropped: the slot points at freed heap. The first `with_live_game_client_mut` (particle FX object-attach pose resolve, only reached in-match when `host_fx_object_pose` misses) derefs garbage → SIGSEGV at InGame render boot, deterministic.
Fix: `GameClient::republish_live_slot_after_engine_move()` (impl_init.rs) called in run_loop.rs immediately after `engine = Some(new_engine)` — re-points the slot at the final engine address before any frame.
VERIFIED: driven twice past InGame entry (v11 07:19, v12 07:26) — process alive, `state=InGame`, `ingame=true`, control bar + world rendering; no crash reports generated.

### 2. CreditsMenu push wedge — ROOT-CAUSED + FIXED via live sample
Sampled the wedged process (`/usr/bin/sample`, release-with-debug; homebrew `sample` shim is broken — python3.13 interpreter missing): main thread parked in `std RwLock::lock_contended` under
`CreditsMenu::init (menu_callbacks.rs:2030) → WindowManager::set_focus (focus_tab.rs:55) → InputFocus system message → "CreditsMenuSystem" callback (script_callbacks.rs:720) → get_menu_manager().read()`.
Recursive read on a std RwLock (not re-entrant): `bind_layout_callbacks` held `manager.read()` across `with_arc_write(menu, init)`; init's set_focus re-entered the same manager lock. Same shape in all 5 menu-manager dispatch kinds (init/update/shutdown/system/input × SinglePlayer/Options/MapSelect/Credits/LanLobby).
Fix (established fetch-then-drop discipline):
- `script_callbacks.rs`: new `dispatch_menu()` helper clones the menu Arc, drops the manager guard, THEN `with_arc_write` — all 25 closures converted.
- `menu_callbacks.rs`: init-time (4 menus) + credits-update inline `set_focus` deferred via `queue_window_manager_op_deferred` — an inline focus under the menu write guard re-enters the SAME menu lock via the window's system callback (write→write; update re-focuses every frame).
VERIFIED: post-fix probe pushed Credits — "Initializing Credits Menu" completes, rendering continues (Render health logs), sample shows the main thread in the normal run-loop path, no lock wait. Screenshot /tmp/wsmoke/credits_after.png (backdrop + logo + credits panel).

### 3. w3d_gadget_draw family guard audit — CLEAN
Swept every collection-guard scope (scripted brace-scope analyzer over all 36 collection-touching files + manual review of all 16 w3d_gadget_draw files, WM draw.rs, game_window_global.rs, game_window_transitions.rs, flush_ui_to_frame):
- list_box.rs was the only read→write-across-draw site (already fixed by DriveRunner; the fixed pattern is correctly applied: geometry under short read, drop, draw by name).
- win_draw_image_ex / draw_window_image_clipped / transitions / anim2_d / hud radar / power meters: guards dropped before draws or taken inside the renderer closure with no outer hold; power.rs drops global+player guards before image lookups. No remaining same-thread re-entrancy found.
- ui_globals draw protocol (drop write guard before wm.draw_all) is honored by the frame owner.

### 4. New blocker found + fixed on the re-drive: shell screens survive into InGame
v11 drive: menu click chain latched (SP hit @1666,399 published, skirmish_path engaged, start_game from Menu) and the process SURVIVED InGame — but the WND menu stack (MainMenu + SkirmishGameOptionsMenu) was never popped: menu widgets kept drawing over the Defcon6 world and their gadget hit-testing ate every world click (physical LMB could never select; sel=0 forever; "ButtonStart missing" spam). The runtime-host `start_game` command bypasses the C++ Start-button shell pop.
Fix: `host_game_engine_reset` (start_game.rs, the C++ `TheGameEngine->reset()` parity site) now tears the shell down: `game_client::gui::ShellHandle::default().reset()` — pops every pushed screen via the immediate path.

### 5. Drive state at wrap-up
Final re-drive (v12 + shell-reset fix) was mid-flight at budget cap: SP click hit published, SK clicked at the correctly parsed published center (1666,527), start_game written, `state=Loading`, child alive. Drive script /tmp/wsmoke/drive12.sh keeps running unattended (logs /tmp/wsmoke/drive12.log; screenshots /tmp/wsmoke/m11_menu_revealed.png, m11_skirmish_menu.png, m11_ingame.png; five-flag poll continues). Next driver: read drive12.log tail — if `gameplay=true` latched, copy status+frame from `cat /tmp/wsmoke/current_dir`. Note drive12's SK-click coordinate parse and LMB-select phases are already corrected.

Changed files this pass: `Code/GameEngine/GameClient/src/core/game_client/impl_init.rs` (republish method), `Code/Main/src/cnc_game_engine/run_loop.rs` (post-move re-register), `Code/GameEngine/GameClient/src/gui/window_manager/script_callbacks.rs` (dispatch_menu, 25 sites), `Code/GameEngine/GameClient/src/gui/callbacks/menu_callbacks.rs` (deferred focus, import), `Code/Main/src/cnc_game_engine/start_game.rs` (shell teardown in engine reset). No probes left in-tree; instrumentation kept per instructions (layout_load/wnd_parse/create_destroy warns are pre-existing). `cargo check -p generals_main --bin generals` clean; no formatters; no git writes.

### 6. Final drive outcome (v12, post-shell-reset)
The last v12 run entered InGame and stayed: 30+ minutes / 34,582 frames alive in `state=InGame` (vs the old deterministic SIGSEGV on entry). Physical LMB selection DID reach the world after the shell teardown fix — `selected_count=1` observed repeatedly (08:11, and held at the final RMB #237). `gameplay` still did not latch: with sel=1, 237 physical RMBs at (1420,500) produced no `gameplay=true` — next driver should verify the RMB order is consumed as a valid command (target point validity / pick path) rather than a timing issue. No new .ips were produced all session (the only two remain the 05:06/05:13 crashes root-caused in §1). Final child termination was an external SIGTERM (sibling drive-lane cleanup), not a crash. Known perf note: one 17s slow-frame spike in InGame (update=4.7s, render=12.3s) — hq-0dw4l water-texture lane, not this lane.

---

## InGameRenderTriage — in-game render defects: terrain noise FIXED, CSF captions root-caused+fixed, red squares fix landed, minimap diagnosed (2026-09-02 08:30-09:55)

### Reproduction method
Piggybacked `request_capture` on live drive12 children plus own drives. Reliable InGame
entry without physical clicks: write `start_game|mode=skirmish|faction=USA` to control.txt
from Menu (Loading→InGame), then `click_skirmish_start` to pop the shell. Evidence PNGs:
`/tmp/wsmoke/irt_ingame_now.png` (defects confirmed), `irt_fix1_ingame.png` (post-fix).

### 1. Main-view "giant stretched noisy texture" — FIXED (root cause: missing mipmaps, not UVs)
One-shot probes (UVPROBE/BINDPROBE, removed from tree after diagnosis) proved the UV
pipeline is CORRECT: chunk vertices advance 1.0 world unit with UVs advancing 0.1 per
vertex = exactly one full tile per MAP_XY_FACTOR cell (C++ WorldHeightMap::getUVData,
GeneralsMD GameEngineDevice/Source/W3DDevice/GameClient/WorldHeightMap.cpp:1589-1694 —
per-cell atlas coords; the port binds standalone tile textures with a REPEAT sampler).
Slots bound real map tiles (Cobble02/TLCobble02.tga, GreeceSand01/TGGrcSand01.tga).
The noise was MINIFICATION ALIASING: TLCobble02 is a 256² tile with a 16×16 stone
lattice sampled into ~60px cells with `mip_level_count: 1`
(textures.rs `create_gpu_texture_from_rgba`) while the terrain sampler expects mips
(mipmap_filter Linear, lod_max 32 — the port already models C++ GameData
TextureReductionFactor/SetLOD bias as lod_min_clamp; W3D textures are D3D-mipmapped).
Fix: full CPU box-filtered mip chain (image::imageops::thumbnail per level) uploaded per
terrain diffuse texture. VERIFIED: post-fix capture shows smooth tiled terrain with
visible map features — moiré gone (irt_fix1_ingame.png).

### 2. MISSING: captions — CSF TABLE ROOT-CAUSED + FIXED (one stale-cache layer remains)
Log showed `Loaded 0 localized GameText strings`: `find_csf_path` (game_text.rs:223)
only searched loose extractions, but generals.csf exists ONLY inside the BIGs.
Deeper: the shipped/repacked asset BIGs (repo assets and the DIRECT PLAY retail install
carry byte-identical copies) have SCRAMBLED ENTRY TABLES for a subset of entries —
EnglishZH.big's generals.csf entry points at a 20-24KB fragment while the intact file
(` FSC` magic, 6421 labels) sits at the stored-size offset; TexturesZH.big entries are
intact (DDS magic at the stored offset) — corruption is per-entry, not per-archive.
Fix (game_text.rs `init_runtime_strings`): loose file → engine virtual FS
(`Data/<lang>/generals.csf`, C++ Language::init parity) → raw ` FSC` header scan over
zh_install_roots() archives keeping the richest parse. VERIFIED: `Loaded 6421 localized
GameText strings` (was 0).
REMAINING (precise): control-bar + main-menu captions still render
`MISSING: 'MISSING: 'GUI:…''` — a fetch whose KEY is already a missing-string, i.e. some
layer caches/fetches window text BEFORE GameText init and re-fetches the translated
string later (wnd_parse.rs `apply_window_text`:212 is one fetch site). Next step: find
the pre-CSF text cache (ControlBar bridge / layout instancing before
init_localized_ui_resources) and re-translate after CSF load.

Fix (Common ini.rs + ini_mapped_image.rs): `INI::set_tolerant_blocks(true)` for
mapped-image loads — parse_file skips unknown junk lines and corrupt blocks (skip to next
END/block header) instead of aborting; `load_recovering_truncated_head` retries head-cut
files from the first registered-block line. Strict parse (normal `load`) is unchanged for
healthy files.
Final verification run: tolerant parser ENGAGED (14,995 corrupt lines skipped in
`/tmp/wsmoke/irt_final2.png` run) but `Imported 1320 mapped images` was UNCHANGED — the
recovered blocks still don't reach the imported collection (block-level parse still
drops them, or names collide in `ImageCollection`). Precise next step: log per-file
recovered-image counts inside `parse_mapped_image_definition` for the SA*.INI files and
diff against the block count in the file; the parse-side plumbing works (skips +
clean-block dispatch), so the loss is between block parse and collection insert.
Captions note from the same frame: text is now SINGLE-wrapped `MISSING: 'USA'` /
`MISSING: 'Player Name'` — CSF lookups reach the recovered 6421-label table; the
remaining misses are BARE labels (`USA`, `Player Name`, `Units`) that the recovered
table lacks or that need the C++ `PlayerTemplate` DisplayName path — verify bare-label
presence in the parsed table before touching lookup code.

### 4. Minimap black — PRECISELY DIAGNOSED (simulation-lane seam)
The radar draw pipeline WORKS: pre-start (skirmish menu up, shellmap radar alive) the
LeftHUD radar rendered terrain (/tmp/wsmoke/irt_probe_ingame.png). Black after match
start = `should_draw_radar_check` false → hud.rs:126 fallback `FALLBACK_HUD_FILL`
(0xE01A1E24): `host_update_the_radar` (run_loop.rs:909-928; C++ GameEngine.cpp:732
`TheRadar->UPDATE()`) sets `local_has_radar = player.has_radar()`; PlayerState::has_radar
(player_state.rs:62) is `radar_count > 0 && !radar_disabled`; the match-start
CommandCenter never increments radar_count in the runtime-host path (RadarUpdate →
`set_radar_visibility(true)` → `add_radar`, object_queries.rs:745-757 never runs for it).
Next step (simulation lane): add_radar for the local player when the match-start CC
spawns (C++ RadarUpdate attaches to CommandCenter; Player::addRadar), or radar.force_on
as a presentation stopgap.
FINAL-RUN UPDATE: in the last verification drive the radar RENDERED (terrain + selection
lines + shroud circle visible in /tmp/wsmoke/irt_final2.png bottom-left) — the draw
pipeline and terrain sampling are sound; visibility still flips with player radar state,
reinforcing the radar_count seam above rather than a renderer bug.

### Asset note (upstream of 2/3)
The shipped BIG set is a repack with mangled entry tables + interleaved binary fragments
for a subset of entries (INIZH INIs, EnglishZH CSF). Engine-side recovery now handles
INIs and the CSF; TerrainZH/TexturesZH TGAs are intact. A re-extraction from intact
media would obsolete the recovery paths.

### Guards / hygiene
No test greens touched: render/asset-loader lanes only; `cargo check -p game-client-rust`
clean after probe removal (UVPROBE/BINDPROBE and their static removed; those two files
end net-unchanged). Serial windowed runs with caffeinate -dis; no git writes; no
formatters. Changed files: GameClient/src/game_text.rs,
GameClient/src/terrain/textures.rs, Common/src/common/ini/ini.rs,
Common/src/common/ini/ini_mapped_image.rs.

---

## GameplayLatch — RMB order consumption root-caused; FIVE-FLAG COMPLETE (2026-09-02 09:45)

### Result
drive13.sh (`/tmp/wsmoke/drive13.sh`) physical chain latched **all five flags** — status
`retail_sit_through_missing=` EMPTY: `window_visible=true live_frame_ok=true ingame=true
wnd_widget_tree_nav=true gameplay=true`. Evidence: `/tmp/wsmoke/fiveflag_status.txt`,
`/tmp/wsmoke/fiveflag_gameplay_frame.png` (in-match: Defcon6 world + control bar + the
move-order marker ▲ at the RMB point), `/tmp/wsmoke/drive13.log` line
`SUCCESS: selected_count=1 … wnd_widget_tree_nav=true … gameplay=true`.
Latch chain in child stderr: `menu_match=true` (first time in this lineage) →
`RMB context command: had_sel=true match=true headless=false` →
`RMB context command issued=true gameplay_order=true`. No new .ips (only the three known
03:11/05:06/05:13 crashes remain).

### Root causes (both in the latch's upstream, not timing)
1. **`skirmish_path` never latched → `match_started_from_menu_wnd=false`** — this alone
   dead-ends BOTH remaining flags (`note_gameplay_order` and `wnd_menu_to_match_complete`
   both require it; drive12's `pre-note menu_click=true was_menu=true offline=true` →
   `menu_match=false` proved the missing third input). Why the physical SK click died:
   C++ MainMenu.cpp:1320 ButtonSinglePlayer GBM_SELECTED calls
   `dropDownWindows[DROPDOWN_SINGLE]->winHide(FALSE)` synchronously, then
   `setGroup("MainMenuSinglePlayerMenu")`. `FlashTransition::update` owns the border
   per-frame — frames 0-3 `winHide(TRUE)` (GameWindowTransitionsStyles.cpp:92-151, ported
   faithfully in game_window_transitions.rs:251-295). At C++ 60 fps that is ~50 ms; at the
   windowed Menu's 3-5 s/frame the Skirmish button stays PARENT-HIDDEN (MapBorder) for
   7+ frames (~30 s), `find_window_at_point_raw` (window_manager/input.rs:621) skips the
   hidden parent's subtree, and the physical click resolves
   `under=MainMenu.wnd:MainMenuRuler` (proven in drive12 stderr: identical coordinates
   hit ButtonSinglePlayer pre-GBM, Ruler post-GBM) → no GBM_SELECTED → no
   `note_skirmish_path_gadget`. The drive12 gadget-hit publisher was honest
   (`named_gadget_center_if_hittable` under-cursor tests) but raced the frame ordering.
   **Fix (drive-side, code unchanged in this mechanic):** new control command
   `tick_main_menu_transitions|times=N` (runtime_host/shell_core.rs) advancing the same
   `WM::update` ticks the injected `winit_menu_nav` path already uses
   (main_menu.rs:2907) — the FLASH/BUTTONFLASH groups finish and ButtonSkirmish becomes
   genuinely hittable; SK click then consumed (`Skirmish button selected` +
   GBM_SELECTED, first time physically).
2. **Physical RMB in classic mouse mode is CancelOrDeselect** — `world_mouse_action`
   (input.rs:31-46) is exact C++ parity (CommandXlat.cpp:3656: "right click is only
   actioned here if we're in alternate mouse mode"; SelectionXlat.cpp:1019-1023 classic
   RMB deselects). drive12's 237 RMBs were CORRECT deselects (hence the observed
   sel=1→0 flip-flop), never orders. The RMB-order latch contract presumes the retail
   UseAlternateMouse scheme. **Fix:** new control command `alternate_mouse|on=1`
   (shell_core.rs) committing through the exact Options-menu path
   (`OptionsMenu::commit_alternate_mouse_setting`, menu_callbacks.rs:1284 — now `pub`,
   C++ OptionsMenu.cpp:1157-1159 checkbox commit) → typed
   `HostOptionsRequest::AlternateMouse` bridge → `use_alternate_mouse` → RMB =
   ContextCommand. Drive then selects (physical LMB) and orders (physical RMB).

### Changed files (this pass)
- `Code/GameEngine/GameClient/src/gui/callbacks/menu_callbacks.rs`: `pub` on
  `commit_alternate_mouse_setting` (canonical retail commit path, reuse not duplication).
- `Code/Main/src/cnc_game_engine/runtime_host/mod.rs`: two dispatch arms
  (`tick_main_menu_transitions`, `alternate_mouse`).
- `Code/Main/src/cnc_game_engine/runtime_host/shell_core.rs`: the two thin command
  bodies (headless fail-closed like every sibling command; Menu-state gate on ticks;
  no evidence-setter calls — guards `host_winit_*_must_call_inject_not_direct_note`
  pass; no probe code left).

### Verification
`cargo check -p generals_main --bin generals` clean; release build clean; guards
`host_winit_menu_nav_must_call_named_gadget_inject_not_direct_note` +
`host_winit_gameplay_order_must_call_inject_not_direct_note` pass (scoped runs);
serial windowed run with `caffeinate -dis`; short-timeout discipline (run latched in
5 min); no git writes; no formatters. Sibling InGameRenderTriage's CSF/mapped-image/
terrain fixes are in the same binary (their captions still render MISSING in the frame —
their verification continues in their lane).

---

## SeamFix — hq-9udt7 (bd 1105) three-seam closure (2026-09-02 10:55)

Scope: the three seams InGameRenderTriage left open. Method: C++ oracle reads
(GeneralsMD GameText.cpp / GrantUpgradeCreate.cpp / RadarUpdate.cpp / RadarUpgrade.cpp /
Player.cpp), offline parse probes against the shipped assets, then two windowed drives
(short-timeout, caffeinate, start_game→InGame, request_capture evidence).

### 1. CSF captions — FIXED + LIVE-VERIFIED

Root cause was NOT the key table (6421 labels load correctly; `GUI:Units` etc. all
present). It was double translation in the draw lane: gadget text draws
(`w3d_gadget_draw/{static_text,common,check_radio,main_menu}.rs`) and
`TextTypeTransition` resolve the widget text through `resolve_window_text` EVERY frame,
while `wnd_parse::apply_window_text` already stores the TRANSLATED string at parse time
(C++ W3DGadgetStaticTextDraw renders instData text verbatim; GameText.cpp fetch happens
once at parse). Re-fetching already-translated text ("Units" — itself the translation of
`GUI:UnitsKilled`'s neighbour labels) wrapped it as `MISSING: 'Units'`, `MISSING:
'Buildings'`, `MISSING: 'Units Lost'`, `MISSING: 'USA'`, `MISSING: '$$$'`.

Fix (GameClient): `game_window/callbacks.rs resolve_window_text` is now idempotent —
`GameText::fetch_with_exists`, verbatim fallback when the key is not a known label.
Genuine parse-time misses keep their single C++ MISSING wrap from wnd_parse.
Additionally `game_text.rs` gains lowercase-key mirrors (`csf_strings_lower`/
`map_strings_lower`): C++ sorts/bsearches the LUT with `stricmp` (GameText.cpp:1373-1379),
so label lookup is case-insensitive — the Rust exact-match HashMap was a parity gap.

VERIFIED live (sf_ingame_12.png, post-fix build): control bar and shell screens render
real captions — "Units", "Buildings", "Units Destroyed", "Units Lost", "Player Name",
"Army", "Team", "Progress", "$$$" — zero MISSING wraps in the frame.

### 2. Red squares — INI wiring PROVEN COMPLETE; residual is texture hydration

Precise finding, contrary to the working hypothesis: the tolerant parser is NOT losing
blocks. Offline probe (cargo test, since removed) over
`extracted_big_files_v2/INI/MappedImages/TextureSize_512/*.INI`: 1343 `MappedImage`
headers → 1308 parse into the collection = EXACTLY the unique-name count (35 headers are
cross-file duplicate names, e.g. `FairPlay` x2 in SCShellUserInterface512). Every block
also parses standalone strict. The INIZH copies of these files are byte-equivalent for
MappedImage content (Texture/TextureWidth identical), so the later-root Overwrite pass
cannot clobber good definitions — the extra 12 names that take the live boot from 1308 to
`Imported 1320` come from that union. Nothing is lost between block parse and collection
insert; `sync_mapped_images_from_common` imports 1:1 by collection order.

The surviving red rectangles (command-button row, portrait, top-right boxes) are the
TEXTURE-DATA hydration lane: `ensure_image_data_loaded` finds no TGA/DDS for the atlas
(e.g. Buttons-Left/Middle/Right in EnglishZH.big per MenuWidgets' note) and `win_draw_image_ex`
paints the raw draw-data tint 255 0 0 255. That is MenuWidgets' declared lane
("image.rs backend paths + honest fallback fill") — not re-done here to avoid collision.

### 3. Minimap radar — seam narrowed to the coupled GW lane; live truth measured

C++ oracle: RadarUpdate never grants radar (ctor only sets flags; update only animates
extend). The match-start radar grant path is CommandCenter `GrantUpgradeCreate
(UpgradeToGrant=Upgrade_AmericaRadar, ExemptStatus=UNDER_CONSTRUCTION)` → player
addUpgrade(COMPLETE) / Object::giveUpgrade → RadarUpgrade::upgradeImplementation →
`Player::addRadar` (RadarUpgrade.cpp:111). The Rust lanes already model this:
`crates_radar_power.rs update_player_radar` (host tick, absolute recompute from
constructed CC providers, tests cover USA/China/GLA/EMP) and the GW coupled recompute
(Wave 818, `status_timers_post.rs`) with `writeback_core` sync. `host_update_the_radar`
stamps `local_has_radar = game_logic.get_player(local_id).has_radar()` per frame.

Experiments (windowed drives + scoped probe test, both since removed):
- Shadow ON (default): pure-InGame minimap BLACK (fallback fill), 3 identical captures.
- Shadow OFF (GENERALS_GAMEWORLD_SHADOW=0): minimap DRAWS (terrain band, shroud circle,
  frustum lines) — sf2_ingame_6/9.png (10:40 run).
- Isolated coupled probe (GameWorldShadow + constructed USA CC): GW recompute sets
  pd.radar_count=1 and `writeback_economy_to_host` lands `(radar_count=1, has_radar=true)`
  on the host player — the lane logic is sound in isolation.
=> The coupled-session radar_count ownership zeroes the live match state. Remaining
suspects, in order: live `host_player_to_gw` slot alignment when the player set mutates
across shell→match (sync_players' dense-index rebuild branch), and the GW player `team`
of the LOCAL slot. One-line next step: re-apply the removed one-shot INFO in
`host_update_the_radar` (local_id / radar_count / radar_disabled / forced) for a single
drive; if radar_count=0 with a correct local_id, dump `host_player_to_gw` + `pd.team`
from `sync_players` on the rebuild branch. Do NOT work around with radar.force_on —
that masks the simulation truth (C++ ControlBarCallback.cpp:51 gate).

### Guards / hygiene
Changed files (kept): GameClient/src/game_text.rs (+case-insensitive mirrors),
GameClient/src/gui/game_window/callbacks.rs (idempotent resolve_window_text).
Removed after use: Common/tests/mi_probe.rs, gameworld_shadow/tests/radar_probe.rs,
the one-shot run_loop probe (function byte-restored). `cargo check -p game-client-rust`
and `cargo check -p generals_main --bin generals` clean; no test greens touched; no git
writes; no formatters; serial windowed runs with caffeinate; evidence under
/tmp/wsmoke/{sf_ingame_12.png, sf2_ingame_*.png, seamfix*.log}.

---

## MinimapSeam — coupled radar lane proven sound headlessly; live probe needs low-fps gate (2026-09-02 12:45)

Scope: close the minimap coupled-lane seam (SeamFix §3). Method: headless coupled
repro + full instrumentation, live shadow-on windowed probe drive. No git writes;
probes removed after use.

### 1. sync_players dense-index rebuild — NOT the live zeroing path (log evidence)

Surviving SeamFix drive logs (generals_exec_smoke_sf*/child_stderr.txt) show the live
player lifecycle: `Starting new game: Shell` → `Created 3 default players` ({0 USA
local, 1 GLA, 2 China}), then `Starting new game: Skirmish` → slots {0 USA "Player",
1 GLA AI} + ReplayObserver id 2, and `Preserving 3 host player(s) across map load`.
Same id set {0,1,2} on both sides ⇒ `need_rebuild` is FALSE at match start; the
refresh branch is id-stable and the local slot's team never goes stale. The
dense-index rebuild branch (entities-live path) cannot be the live zeroing source in
this flow.

### 2. Headless coupled repro — lane logic SOUND (regression tests landed)

`gameworld_shadow/tests/radar_coupled.rs` (new, 2 tests, passing): one GameLogic +
one persistent GameWorldShadow across shell→skirmish (and a fresh-shadow control),
match-start AmericaCommandCenter seeded like Wave 831 (owner-bound, constructed,
KindOf CommandCenter + MpCountForVictory). Full coupled flow per frame —
`shadow_session_after_host_tick` (Wave 818 GW radar recompute → sync →
apply_host_radar_events → player-radar drain → writebacks) — preserves host
radar_count=1 / has_radar=true. Matches SeamFix's isolated probe; confirms the
coupled lane does not zero derived radar state for a living, radar-provider-backed
local player.

Instrumented detour (removed): with a template lacking `MpCountForVictory` (or an
unowned CC), the session-end parity probe (`shadow.probe` →
`evaluate_victory_condition`) judges the local player defeated on frame 1 and
`kill_player_for_victory` destroys the CC → radar zeroed. C++ parity gate
(VictoryConditions.cpp:196 killPlayer) — a real hazard only when a side lacks a
victory-counting structure; NOT the live defect (live status shows match_over=false,
CC alive).

### 3. Live RADARPROBE — gated too tight for a 0.1 fps loop (exact next step)

Shadow-on windowed probe drive (mseam_drive.sh, 12 captures, status
ms_status_final.txt) ran clean, but RADARPROBE/SEAMPROBE never fired: the windowed
runtime-host loop runs ~0.09 fps (status fps), so only ~4-6 frames elapse inside
InGame during captures — my gate (first 4 calls + every 600th) never sampled there.
Next driver: in `host_update_the_radar`, log unconditionally when
`state == InGame` (or gate on counter<50), rebuild release, rerun
`/tmp/wsmoke/mseam_drive.sh`; RADARPROBE lines then give live local_id /
radar_count / radar_disabled / disable_proof / power / forced plus SEAMPROBE
session-stage truth. Do NOT work around with radar.force_on (C++ ControlBar
gate parity).

### Guards / hygiene
Probes removed (run_loop.rs, session.rs, construct.rs, create_destroy_die.rs,
host_ops_writeback.rs byte-unchanged vs pre-task). Kept: radar_coupled.rs tests.
`cargo check -p generals_main` clean; no test greens touched; no git writes; no
formatters; drive used caffeinate -dis, short timeout, serial.

## MinimapProbe — live zeroing site pinned and FIXED: Wave-818 radar peel was gated behind movement authority (2026-09-02 14:00)

Scope: MinimapSeam's handoff. Method: unconditional InGame RADARPROBE plus
SEAMPROBE (Wave-818 GW recompute internals), SEAMSYNC (player mapping), RADARHOST
(host recompute) — one probe-free confirmation drive after removal. No git writes.

### Live truth (probe drive 13:15, generals_exec_smoke_ms1788365731)

Pre-fix RADARPROBE (first drive): local player 0 — `cnt=0 dis=false proof=0
alive=true pw=0 forced=false` on EVERY InGame frame (n=0 frame 5 → n=199 frame
577). Never a grant decay — radar_count was simply NEVER granted. RADARHOST:
host `update_player_radar` ran exactly ONCE (frame 0, no players, no providers) —
step.rs:1139-1146 skips it whenever `gameworld_shadow_enabled() &&
shadow_coupled_tick_active()` (Wave 818 ownership), i.e. always in default
shadow-on coupled play.

SEAMPROBE (400/400 ticks): the GW side is PERFECT — `provs=[t0=1]` (America CC:
alive, constructed pct=1.00, not disabled, CommandCenter bit, legal provider,
team 0) and `players=[g0 team=Some(0) cnt=1 dis=false, …]` every tick.
SEAMSYNC: mapping h0→g0 / h1→g1 / h2→g2 correct, host ids {0,1,2} stable across
shell→skirmish (MinimapSeam's dense-rebuild + team-staleness suspects: CLEAN —
neither is the live zeroing path).

### Root cause — the grant reached GW but never landed on host Player

Under shadow the GW Wave-818 recompute owns radar_count
(status_timers_post.rs) and records host_player_radar_log transitions; its peel
— `host_player_radar_log::drain()` → `Player::set_radar_state` + online/offline
audio (session.rs) — sat INSIDE the `if gameworld_movement_authority_enabled()`
block (session.rs:1134-2189). Movement authority defaults FALSE and only tests
enable it, so the peel NEVER ran live. The other return path,
`writeback_economy_to_host` (pd→host radar_count), is behind
`gameworld_economy_authority_live()` — economy authority also never enabled in
the engine. Net: nobody wrote radar_count to the host player in shadow-on play →
`local_has_radar=false` → hud.rs `should_draw_radar_check` false → minimap
fallback fill. Shadow-off worked because the step.rs gate released the host
recompute. C++ parity: Player::addRadar lands via RadarUpgrade
(RadarUpgrade.cpp:111) on the one GameLogic store; RadarUpdate.cpp never grants.

**Fix (session.rs):** relocate the Wave-818 player-radar drain out of the
movement-authority gate to run unconditionally on every coupled session tick
(it only consumes the log the GW recompute already produces; idempotent same-value
sets; single consumer of the log). Wave 818 markers intact (residual
source-marker checks still pass).

### Victory-probe hazard (flagged by MinimapSeam): NOT firing live

`shadow.probe` → `evaluate_victory_condition` runs per coupled tick
(session.rs:2591), but the retail CC carries MpCountForVictory (AiFilterFix
buildings.rs stamps it): SEAMPROBE showed the CC alive+legal on all 400 ticks,
SEAMSYNC alive=true, status match_over=false. killPlayer (VictoryConditions.cpp
:196 / :250-281 hasAnyBuildings) never triggered in this fixture.

### Verified live (A/B)

- Pre-fix last frame (ms1788364746/frame.png): minimap box = flat FALLBACK_HUD_FILL
  panel + border — radar OFF lane.
- Post-fix (ms_ingame_3/6/9/12.png, drives 13:15 AND 13:42 probe-free): minimap
  draws the RADAR lane — camera frustum trapezoid, live unit blips, shroud blob.
- RADARPROBE post-fix: `cnt=1 … has=true` from frame 5 through n=199 (200/200);
  status InGame, fps≈12, match_over=false.
- Remainder (documented, NOT the zeroing seam): the terrain band is black —
  radar terrain-texture hydration (terrain.rs paint source /
  build_terrain_texture_cpp) never fills in the windowed flow. Identical in the
  shadow-OFF reference (sf2_ingame_9.png), so it is a separate pre-existing
  cosmetic lane, orthogonal to radar online.

### Guards / hygiene

Probes removed: run_loop.rs (#03F9), status_timers_post.rs (#32DF),
construct.rs (#E1D9), crates_radar_power.rs (#973D) — all byte-restored to
pre-task tags; sole delta is the session.rs relocation. `cargo check -p
generals_main` clean. Guards: gameworld_shadow 304/304 (incl. radar_coupled
2/2 rerun by name), combat filter 955/11 then 964/2 on the identical binary
(documented varying-flake band, final20b; failures = capture/chinook/fire-OCL
family, zero intersection with this surface), world_tests catalog 8/8. Final
probe-free release rebuild re-driven (13:42): minimap still draws, 0 probe
lines in stderr. No git writes; no formatters; caffeinate + short-timeout serial
drives; evidence under /tmp/wsmoke/{mseam.log, ms_ingame_*.png,
prefix_mm.png, final_mm.png, ms_status_12.txt}.

---

## SaveLoadHunt — in-match save/load roundtrip depth hunt: 2 load-fails + 1 apply-order clobber + 1 xfer-field loss FIXED (2026-09-02)

Scope: golden_skirmish `save_load_ok` depth verification. Method: three read-only
scout audits (ObjectSnapshot capture↔restore field diff; all 44 lifecycle-tail
persist suffix capture↔apply pairing; C++ Xfer parity for the session seams
paradrop/radar/battlebus/slowdeath-stun/cleanup-hazard/Weapon), then fixes +
regression guards + harness runs. Serial cargo; no git writes; no formatters.

### Fixed (each with a guard)

1. **`KindOf` xfer ordinal 71 was save-only — load aborted.**
   `Code/Main/src/save_load/xfer.rs`: `write_kind_of_variant` mapped
   `TechBuilding => 71` but `read_kind_of_variant` had NO 71 arm → any payload
   carrying that KindOf failed the whole load with "Invalid KindOf variant: 71"
   (append-only convention violated once). Added the read arm + new census
   `KindOf::ALL_KIND_OF` (host_types.rs) and test
   `every_kind_of_xfer_ordinal_round_trips` (write↔read total bijection, no
   ordinal reuse). Note: Common's `xfer_kind_of` is name-based (ordinal-drift
   immune); only Main's compact u8 map had the hole. CleanupHazard=83 verified
   present on BOTH sides (write xfer.rs:270 / read :358 post-fix numbering).
2. **SPCD suffix: unpaused special-power cooldown aborted every such load.**
   `special_power_cooldown_persist.rs` v1 omitted the pauses table when empty
   and inferred presence from `!rest.is_empty()` — but OXOB always follows in
   the shared lifecycle tail (one entry per object), so the decoder read the
   sibling ASCII magic as a ~1.1e9 table count (multi-GB `Vec::with_capacity` /
   Corrupted) → whole load failed. v2 now ALWAYS encodes the pauses table;
   v1 reads use a bounded absence probe (count > 2^16 = sibling magic, never a
   real count); `decode_table` got sanity bounds (1<<20 entries). Guards:
   `v2_unpaused_cooldown_tolerates_trailing_sibling_suffix`,
   `v1_absent_pauses_ignores_trailing_sibling_suffix`,
   `v1_present_pauses_table_still_decodes`.
3. **Turret residual apply-order clobber (TRAI after OXOB).**
   `builder.rs` restore applied `turret_aim_persist` (resets 12 turret fields
   for EVERY object) AFTER `object_xfer_persist` (47 fields incl. all 15 turret
   fields, captured for every object) → OXOB-restored idle-scan/hold/substate
   state was zeroed for any object outside TRAI's capture predicate. Reordered
   TRAI before OXOB (comment cites C++ single `TurretAI::xfer`; TRAI kept for
   legacy tails without OXOB). Scout claim that TRAI lacked the two idle-scan
   fields was wrong (they are in TRAI v? payload lines 39-40); the order was
   the real bug.
4. **Direct-Xfer `Weapon` record dropped clip/reload residual (F1).**
   `xfer_helpers.rs impl XferData for Weapon` carries only 10 of 16 Weapon
   fields — the direct-Xfer object path lost `clip_size/clip_reload_time/
   splash_radius/reloading_clip/last_bonus_rof` that the serde (production
   .sav) path kept; a mid-clip-reload slot reset to `Weapon::default()`.
   C++ `Weapon::xfer` v3 persists m_status RELOADING_CLIP + m_ammoInClip
   (Weapon.cpp:3364-3367). Fixed as a versioned V21 object tail
   (`WORLD_SNAPSHOT_DIRECT_XFER_V21_TAIL_VERSION`, bincode+direct writers
   bumped 20→21; v20 bincode payloads decode as the unchanged current serde
   layout; direct validator accepts 1..=21). Guard:
   `direct_xfer_v21_appends_weapon_clip_residual_and_keeps_alignment`
   (v21 roundtrip + v20 old-layout alignment sentinel).
5. **Logic-only loads died on client ThingFactory.**
   `GameClient/src/core/game_client/leftover.rs` (`GameClient::xfer` load
   branch) demanded the ThingFactory global before decoding ANY drawable — a
   headless in-match quicksave (drawable_count == 0, e.g. unit-test /
   logic-only process) failed the whole load with "ThingFactory not
   initialized". This broke the pre-existing
   `save_file_roundtrip_preserves_lifecycle_envelope` too. Factory is now
   required only when `drawable_count > 0` (C++ assumes the factory always
   exists; the host now matches that contract only where it has work).
6. **E2E guard**: `save_file_roundtrip_survives_unpaused_cooldown_and_keeps_
   weapon_clip` (lifecycle_save_file.rs) — full SaveFileManager production
   .sav save→load with an unpaused ParticleCannon cooldown + mid-clip-reload
   weapon; asserts cooldown (88.0), clip/splash/reload state survive.

### Verified

- `cargo test -p generals_main --lib save_load::xfer` → 2/2;
  `...snapshot::special_power_cooldown` → 6/6;
  `...snapshot::lifecycle_save_file` → 3/3 (incl. the previously red
  envelope test); `...snapshot::turret_aim` → 2/2;
  `...snapshot::object_xfer_persist` → 8/8 (after revert, below).
- `golden_skirmish_gate` (release, with all fixes): **save_load=true,
  players_preserved=true, checkpoints=3**. Gate overall FAIL is NOT save/load:
  map-path `gather/build/produce/fight=false` → `victory=false status=partial`
  — economy/production/combat sim lanes (pre-existing sibling churn; same
  tree also fails the 4 tests below).
- Not re-run this pass: `save_load_demo` (dev-tools debug build; the same
  SnapshotBuilder/SaveFileManager surface is covered by the suites above) and
  `deterministic_trace_compare` (needs a baseline trace pair; fixture CRC path
  covered by `deterministic_fixture_trace`).

### Precisely documented, NOT fixed (owned elsewhere / pre-existing at HEAD)

- `world_and_weapon_snapshots` 4 red (pre-existing / sibling mid-flight, none
  touch my files): `snapshot_restore_recovers_veterancy_from_tracker_data`
  (`gain_experience(180)` no longer levels — template `is_trainable` default
  false + unchanged code = red at HEAD);
  `host_upgrade_capture_mid_flight_save_load_completes_unlock` +
  `save_file_roundtrip_preserves_pending_host_upgrade` ("player research queue
  must hold Capture" — upgrade/center.rs is uncommitted EngineStores migration
  work); `special_power_a10_mid_flight_save_load_still_impacts` (strike
  completes not latching; strike code identical to HEAD, test last touched at
  HEAD).
- `ai_team_persist::snapshot_round_trips_waypoint_queue_and_order` — Ranger
  classified `Resource` in snapshot (file verified byte-identical to HEAD;
  classification heuristic lives in builder/thing template defaults).
- Deliberate keep-behavior restored after audit: `reset_object_xfer`
  intentionally does NOT reset `safe_occlusion_frame`, and TMAI absent-suffix
  intentionally keeps `team_common_attack_targets` (guard tests
  `absent_suffix_keeps_template_safe_occlusion_frame` etc. document these —
  I reverted my initial fail-closed resets there).
- Benign audit notes on record: score_keeper `current_score` decoded-not-applied
  (re-derived by `calculate_score()`); CleanupHazard C++ fields
  m_bestTargetID/m_inRange/m_nextScanFrames re-derived on first post-load scan
  (CLHA carries pos/moveRange/nextShotFrame = C++ v1 essentials);
  PhysicsBehavior non-stun fields skipped (subsystem not simulated).
- Suffix-scan hardening direction (report only): 43 of 44 magics are
  length-bounded and mis-parse needs a payload-embedded magic + exact version
  u32 (≤1e-3 upper bound, far lower in practice); SPCD was the one real hole
  and is fixed; a length-chained manifest would remove the class.

Changed files (this pass): `Code/Main/src/save_load/xfer.rs`,
`Code/Main/src/game_logic/host_types.rs` (census const only),
`Code/Main/src/save_load/snapshot/{types,legacy_bincode,object,special_power_
cooldown_persist,turret-apply-in-builder}.rs` (builder.rs reorder),
`Code/Main/src/save_load/snapshot/{lifecycle_save_file,tests/world_and_weapon_
snapshots}.rs` (tests), `Code/GameEngine/GameClient/src/core/game_client/
leftover.rs`. `module_runtime_persist.rs` countermeasures absent-suffix clear
kept (its guards pass). Guards/serial runs honored; no probes left in tree.

## UnitRenderHunt — CRITICAL QUESTION ANSWERED: units/buildings DO submit draw calls in-match, but only ONE degenerate untextured mesh reaches pixels (2026-09-02 18:50)

Method: temp WARN probes in pipeline_collect (collect buckets) + forward_render (queue
counts), temp Debug log clamp, drive14/14b/14c (direct `start_game` control path;
view_command_center + camera_look_at dozer + zoom captures). Probes REMOVED — the three
files are byte-identical to HEAD (`git checkout --` of pipeline_collect.rs,
forward_render.rs, win_main.rs; remaining render_pipeline diff is MinimapTerrain's
pipeline_minimap.rs). CAUTION: target/release/{generals,generals_snap} (18:44) still
contain probe strings — next release build clears them. Evidence:
/tmp/wsmoke/d14_ingame_{default,cc,dozer}.png, d14_status_*.txt, d14c_stderr.log,
drive14.log.

### Live in-match truth (Defcon6, USA, frame 435, probes 200 lines)

- Collect: alive=470, clear=3, fogged=0, hidden=467, missing=0, items=103.
  OWN-PASSED AmericaCommandCenter a=1.00 models=4 / AmericaVehicleDozer a=1.00;
  OWN-FILTERED GLAHoleCommandCenter a=0.00 e=0.00 — FOW lane is CORRECT (own units
  Clear, enemy base unexplored). FOW is NOT the bug. render_model_missing=0 —
  W3D models LOAD from the BIGs (ABBtCmdHQ, AVDozer etc. resolve synchronously).
- Queue: `UNITRENDER-QUEUE queued=103/103 errors=0 hidden=0` with sample
  `ABBtCmdHQ_AC@(3068,2241) a=1.00 pass=ForwardOpaque` — exactly the CC position the
  camera was pointed at; zero "No cached W3D model" warnings. Meshes REACH the ww3d
  renderer.
- Screen: terrain + HUD only. ONE ~35px untextured white mesh draws at the dozer's
  exact world position and roughly correct perspective size (d14_ingame_dozer.png);
  the CC complex draws nothing visible.
- Same symptom out-of-match: shellmap queues 89 meshes/frame (AVChinookAG a=1.00) yet
  the main-menu background is black — the drop is in the shared ww3d mesh draw path,
  NOT match-specific.

### Fix site (smallest parity change; C++ oracle W3DDrawModule → dx8renderer)

`ww3d-renderer-3d/src/rendering/mesh_system_impl/render_manager.rs` — render_mesh()
issues every mesh unconditionally; the silent kill is in the per-pass geometry slice:
`issue_draw_call()` (line ~1238) resolves `(start_index,count)` from
`PreparedMeshModel.pass_index_ranges`, and `compute_pass_index_ranges()`
(helpers.rs:260) builds those ranges from `model.polygon_renderer_list` grouped by
`renderer.material_pass.get_pass_index()`. When a model's polygon-renderer list is
empty/its pass indices mismatch `prepared.material_passes`, the resolved range is
(0,0) → zero-index draw call, no error, no pixels; a mesh that falls back to range[0]
is the lone visible blob. Secondary suspect for the white/untextured look:
`create_texture_bind_groups` in the same file. Suggested next probe (one drive):
WARN the resolved (start,count) + polygon_renderer_list.len() + material_passes.len()
per model in PreparedMeshModel::frommodel — that pins which side of the mismatch is
empty. Gap size: ONE module (~1.8k lines), no missing subsystem — W3D parsing, BIG
asset loading, collect/queue/draw chain all exist and run; only the pass→geometry
slicing/texture-bind step inside MeshRenderManager is broken.

### Notes
- Camera evidence: view_command_center + camera_look_at|x=3108|y=120|z=2201 land the
  camera correctly (status camera_pos/target match dozer sample pos).
- fps in windowed runtime-host is now ~4-11 (was 0.08 in the morning drive) — the
  09:47 generals_snap was stale; drive scripts must run the freshly built binary
  (generals_snap was refreshed by cp from generals).

---

## UnitRenderFix — pass-slicing + texture-bind fixes landed; in-match invisibility NOT yet solved (2026-09-02 23:50)

Method: fresh WARN probes inside the ww3d draw path (MESHPREP in PreparedMeshModel::frommodel,
MESHPASS/MESHSTATS in MeshRenderManager::render_pass, MESHDRAW per-mesh sample, MESHZERO in
issue_draw_call, TEXTUREMISS/FALLBACKPASS in forward_materials) + drive14c lineage runs
(20:05 and 19:33 binaries). All probes REMOVED after data collection;
forward_materials.rs is byte-identical to HEAD again; mesh_bounds_probe bin deleted.
Evidence: /tmp/wsmoke/d14_ingame_{dozer,cc}.png, child_stderr logs in the two
`$(getconf DARWIN_USER_TEMP_DIR)generals_exec_smoke_manual_*` dirs (latest:
/tmp/wsmoke/current_dir).

### Probed reality (supersedes the (0,0)-range hypothesis — that one is DEAD)
- The queue→draw chain is healthy: `MESHPASS opaque=103` and
  `MESHSTATS meshes_rendered=103 draw_calls=103 tris=2016 zero_draws=0` (pre-fix run).
  All queued meshes resolve models, prepare, and issue indexed draws. MESHZERO only ever
  fired for `pass_index=1` on two-pass meshes (ranges=[(0,N)]) — i.e. extra passes with no
  authored polygon-renderer ranges, which the old `ranges[0]` fallback was re-drawing with
  the wrong pass state.
- Per-draw state of the invisible meshes is GOOD: MESHDRAW shows rigid meshes at sensible
  world positions (3068–3120, 0–132, 2192–2299 — camera was at 3108/2201), fow_alpha=1.00,
  opacity=1.00. Mesh sizes 2–106 tris (CC slab 474 tris, 894 verts) — plausible low-poly
  2003-era geometry, NOT empty.
- The one visible white blob (dozer position, ~35px) is a 2–4 tri mesh.
- Texture names DO miss in the archive: `TEXTUREMISS 'AVChinook.tga' primed and still
  unresolved` etc. — ensure_texture runs prime+lookup and still falls back for all unit
  textures (AVChinook/Avamphib/Housecolor/ATCemWall01...). The two ZHCA_UI* UI meshes bound
  real names. This is an asset-lane defect (BIG candidate paths vs mounted archives —
  note `BIG archives loaded in 0.00s` at boot and 0 "Loading raw texture from archive"
  debug lines all session), NOT a bind-group one.

### Landed (permanent, ww3d-renderer-3d/src/rendering/mesh_system_impl/render_manager.rs)
1. **Strict pass→geometry slicing (C++ MeshClass::Render_Material_Pass /
   DX8PolygonRendererList parity):** issue_draw_call no longer falls back to
   `ranges[0]`/all-indices when `pass_index >= pass_index_ranges.len()`. A pass with no
   authored range owns no geometry and draws nothing; base pass 0 owns the fallback range.
   Verified live: zero_draws=30 with MESHZERO all pass_index=1 two-pass extras suppressed,
   base draws unchanged (draw_calls=103, tris=2016). This also removes the multi-pass
   overdraw that ran extra-pass state over the base pass.
2. **Lazy GPU texture upload (`ensure_gpu_texture_view` + name-keyed side cache):**
   stage_resources_for previously fell back to the 1×1 WHITE texture whenever a pass
   texture had CPU pixels but no gpu_texture — which is ALWAYS true for unit textures
   (forward_materials build_texture only fills pixels; nothing outside render2d ever calls
   create_wgpu_texture). Now the first bind uploads the pixels (32-bit RGBA/BGRA formats,
   length-guarded) and caches the view. This is the fix for the "untextured white" look;
   real textures additionally require the texture-name resolution fix above.

### Reverted (unverified, keep out of tree)
An index-order flip in build_mesh_model (to compensate the det<0 gameplay→render axis
mirror) was built, driven, and produced no visible change — it is REVERTED;
forward_materials.rs is byte-identical to HEAD. Analysis notes: the axis swap (Y↔Z,
det=-1) flips winding, and wgpu Ccw/cull-Back vs D3D9 D3DCULL_CW conventions disagree —
whoever fixes the invisibility next should start by dumping one invisible mesh's
post-transform vertex positions and testing the pipeline with cull_mode=None.

### Open (the actual blocker)
Why do 2–106 tri meshes with correct transforms/alpha draw zero pixels? Next steps for the
next driver: (1) capture one invisible mesh's vertex buffer bounds after transform (probe
frommodel bounds or run a GPU capture) — rules vertex-payload collapse in/out; (2) flip
wgpu_pipeline_manager cull_mode to None for mesh pipelines for one run — winding test, zero
risk; (3) fix texture-name resolution (asset lane: TexturesZH.big lookup) so hydration has
real pixels; (4) re-check `ww3d_peak≈1.3µs` render health numbers — units rendering should
move it to ms-scale. Constraints honored: no git writes, no formatters, serial cargo,
probes removed, guards/tests untouched (world_tests, combat filter, gameworld_shadow,
minimap radar lanes untouched).

## UnitRenderFix2 — three discriminators run; unit meshes DO reach pixels; remaining defects pinned (2026-09-02 22:15)

Method: drive14c lineage (start_game → InGame → CC view → dozer look-at), one binary per
discriminator, screenshot after each step. All temporary probes/overrides REMOVED after;
final tree = the two permanent UnitRenderFix changes only (`cargo check -p generals_main
--bin generals` clean). Evidence: /tmp/wsmoke/urf2_final_{default,cc,dozer}.png, raw logs in
`$(getconf DARWIN_USER_TEMP_DIR)generals_exec_smoke_manual_*` (see /tmp/wsmoke/current_dir).

### 1. cull_mode=None (winding test) — NEGATIVE
Forced `cull_mode: None` on every ww3d mesh pipeline for one run (wgpu_pipeline_manager).
No change vs baseline: same one white dozer blob, CC complex still invisible. Winding from
the det<0 gameplay→render axis swap is NOT the killer; C++ D3DCULL_CW/Ccw question closed.

### 2. Vertex-bounds probe — payload and camera are HEALTHY in-match
Per-frame probe in MeshRenderManager::render_mesh dumped post-transform clip bounds of the
first drawn mesh for 300 frames, plus a model-space payload census (VPAYLOAD) and a
per-model draw census (MDRAW, 200 first draw calls with names/counts/world/NDC):
- Model payloads are NOT collapsed: AVBattleSh::17 = 868 verts / 566 tris, 397 distinct
  positions, sane bounds ±170; all sampled meshes similar.
- The in-match view-projection is a proper perspective (w 500–1200, NDC in-range). The
  IDENTITY view-projection seen in the very first probe came from the menu/shell lane
  (WgpuMainRenderer::from_backend seeds `set_camera(CameraClass::new())`); in-match
  forward_render sets view/proj/position per frame — camera path is fine.
- MDRAW proves the unit meshes (AVChinookAG, AVBattleSh, PRG props) are ISSUED at valid
  on-screen NDC with correct world translations. The earlier "zero pixels" picture is
  outdated: with the landed lazy-upload + strict-slicing fixes, meshes do rasterize —
  the dozer blob + small shards at the dozer position ARE unit submeshes rendering
  untextured. The large CC slab in the CC view is the remaining invisible case.

### 3. Depth test off + full-index draw — partial
With depth_compare=Always/write-off, MORE white slivers appear (fragments previously
depth-killed — a real secondary defect in the depth lane worth its own pass), but meshes
still render as fragments, not full silhouettes. Drawing the full index range instead of
the strict pass-sliced range changed nothing (slicing is not losing geometry).

### 4. Texture resolution — NOT a lookup bug: the asset DATA is absent
`ensure_texture` → prime → archive candidates all behave as coded; zero
"Loading raw texture from archive" lines and zero missing-texture warns fire because the
unit textures were never present in any mounted archive: the on-disk game data
(windows_game/{extracted_big_files,extracted_big_files_v2,Command & Conquer Generals Zero
Hour}) contains NO .big archives and no Art/Textures tree with unit textures
(AVChinook.tga / Avbattlesh.tga / Housecolor.tga exist nowhere on disk; only map-embedded
textures like water01.dds / TRCobbleStones.tga / Defcon6 map tgas resolve — exactly the
ones that DO render). No code fix can hydrate these; the fix is provisioning the texture
BIGs (Generals.big / W3D texture archives) into the mounted search paths.

### Net state
Units render untextured-white at plausible screen positions; visible-defect list is now:
(a) unit texture DATA not provisioned (environment, not code); (b) depth-lane kills some
mesh fragments (secondary); (c) large CC slab still absent in CC view (primary open
render defect — candidate next probe: GPU-side capture or per-submesh visibility flags on
the CC model's passes, not winding/payload/camera, all three now ruled out).
All probes removed; pipeline_manager and issue_draw_call byte-restored to the landed
UnitRenderFix semantics; no green suites touched.

## In-match sim-depth hunt (InMatchSimHunt)

Goal: verify economy / combat / special powers / upgrades / veterancy / dozer-build
actually run in a live match, using the port's own oracles (golden_skirmish_gate,
in-world tests). Method: harness drives + a temporary #[test] that drove full
`logic.update()` frames on the synthetic host fixture with per-system probes
(removed after diagnosis; findings below).

### Verified live (already working, evidence cited)

- **Production tick** — `update_production` + `update_player_upgrades` run every
  host tick (world_tick/step.rs:1085-1092); research completion with
  producer-queue liveness per C++ ProductionUpdate.cpp:636-648/1109-1112
  (object_ai_combat.rs:239-398).
- **Combat kills / fought** — golden gate fought path; kill credit plumbing
  `continue_or_stop_after_kill` → `award_score_the_kill_experience`
  (world_tick/shock.rs:999-1012) mirrors C++ Object::scoreTheKill +
  Player::addSkillPointsForKill.
- **Special powers** — superweapon strikes queue + complete + apply authored
  damage on the live tick (`update_special_power_strikes` /
  `update_a10_strike_flights` are ticked in step.rs:283/690); a10/carpet/daisy
  in-world tests (strategy_and_artillery.rs:2449+) drive the same host path.
- **Veterancy gain math** — `gain_experience` (object/update.rs:817) applies
  per-template thresholds + retail veterancy bonuses (+10/20% etc.); direct
  `award_experience` on a killable enemy leveled a trainable ranger Rookie→Veteran.
- **Dozer build** — golden gate constructed/same_world_production paths.

### Finding 1 — FIXED: golden fixture structures lacked MpCountForVictory → both players defeated on frame 1 (economy no-op)

Repro: synthetic host world + skirmish players; on the first
`logic.update()`, `evaluate_victory_condition` (step.rs:1209 → mod.rs:97-121)
marks every player defeated: `counts_as_victory_building` requires
STRUCTURE && **MpCountForVictory** (victory_conditions.rs:357-360, C++
Team::hasAnyBuildings mask), and `install_templates`' golden structures
(GoldenCC/GoldenPower/GoldenSupplyCenter/Barracks/GoldenEnemyCC) carried no
such bit → `is_defeated` NO_BUILDINGS ⇒ pending_kills for players 0 and 1 on
frame 1. `kill_player_for_victory` (presence.rs:259-267) then zeroes
`resources.supplies` and `is_alive=false` — observed empirically: cash
20000/10000 → 0/0 at frame 100 (all four isolation variants). Consequence:
in the synthetic golden slice, supply income, upgrade affordability, and any
player-scoped economy were dead even while the gate printed green (its
`gathered` check reads `ai_state`, not cash; `upgraded` was satisfied by the
queue-side `Success` before the wipe mattered).

Fix (C++/in-repo oracle): buildings.rs:996-1008 already stamps
MpCountForVictory on every synthesized structure ("...or skirmish annihilation
rules defeat everyone on frame 0") — retail FactionBuilding.ini authors
KINDOF_MP_COUNT_FOR_VICTORY on faction structures. Applied the same stamp in
`install_templates` (golden_skirmish.rs, structure loop before insert).
Verified: cash 20000 now stays through 900+1400 update frames; player 0 alive.

### Finding 2 — documented, not fixed (fixture gap + likely live no-op): QueueUpgrade refused for the golden producer

With cash restored, `CommandType::QueueUpgrade` on GoldenSupplyCenter returns
`InvalidCommand`; `object_can_produce_upgrade` = false, production queue stays
empty, research never starts (0/1400 frames). The C++ oracle
ProductionUpdate::queueUpgrade (C++ :250-272, :596) requires the producer to
canProduceUpgrade (CommandSet/ProductionUpdate walk) — the golden fixture
template authors neither, so the harness's `upgraded=true` only proves the
player-side queue bookkeeping, not building-attached research. Fix direction:
author a ProductionUpdate + upgrade command-set entry on GoldenSupplyCenter in
install_templates (or exercise a retail template like USA_SupplyCenter with the
real INI in the map-world path), then the existing research tick completes it
(900 residual frames; SupplyLines retail BuildTime 30s = 900 frames per
host_upgrades.rs:2037).

### Finding 3 — documented: dozer Gather never engages in the synthetic fixture

Post-fix, Gather command leaves the GoldenDozer `Idle`/target None for 900
frames (no cash accrual, gain_frames=0). Harness `gathered=true` is measured
immediately after `process_commands` in some states and is not a cash-based
check, so the green gate masks this. Fix direction: the gather executor likely
requires a dock/warehouse association (supply-center adjacency or
`SupplyCenter`-kind dock binding) that the synthetic layout doesn't satisfy —
trace `execute_gather` and require a cash-backed gather assertion in the gate
(supplies increase) so the economy claim is honest.

### Finding 4 — documented: kill-XP not credited through the live damage path

A ranger (trainable=true, in range, weapon 25/1.0s) killed a 120 HP armor-0
enemy (kill_xp=80) at frame 181 via AttackObject over the live tick:
`host_experience_log`/`host_veterancy_log` stayed empty and the ranger stayed
Rookie — while calling `award_experience(ranger, 80)` directly leveled it
(Rookie→Veteran, +10/20% bonuses applied). So `gain_experience`/
`award_score_the_kill_experience` are correct; something between the damage
path and scoreTheKill (killer identity via `last_damage_source`, or the
structure-topple destroy-list deferral for KindOf::Structure victims) drops the
award. Fix direction: instrument `mark_object_for_destruction` /
destroy-list scoreTheKill for structure victims and verify the killer id is
the ranger at award time (C++ Object::scoreTheKill awards on
ActiveBody::attemptDamage damage-death regardless of death-animation deferral).

### Guards / hygiene

- Production delta: only `install_templates` in golden_skirmish.rs ( MpCountForVictory
  stamp). `cargo check -p generals_main --lib` clean. Both temp probe tests
  removed; file parses (rustc scoped probe clean).
- No git writes, no formatters; serial cargo with peer backoff.
- Pre-existing observation: golden_skirmish_gate --frames 30 (map path) printed
  status=partial (gather/build/produce/upgrade/fight false) on this tree during
  a heavily concurrent window — findings 2-4 above are the likely mechanics
  behind those false flags; a clean re-drive is left to the main agent's
  validation pass.

## MinimapTerrain — terrain band hydrated: async render-side tile hydration rebuilt the radar texture (2026-09-02 20:45)

Scope: MinimapProbe's documented remainder (terrain band black). Method: live
windowed manual drives (Defcon6 skirmish, shadow-on AND shadow-off) with
temporary env-gated probes, all removed after use. No git writes.

### Root cause — three stacked defects, all timing/asset-shaped

1. Ordering. C++ `W3DRadar::newMap` builds the terrain texture at map load with
   WorldHeightMap tile textures already resident (W3DRadar.cpp:977-993). In the
   windowed Rust flow the render pipeline hydrates the client TerrainVisual
   (heightmap + source tiles) asynchronously in
   `sync_render_terrain_visual` (start_game.rs:551), i.e. AFTER
   `Radar::newMap` painted the band from an unhydrated paint source — black —
   and nothing ever rebuilt it.
2. Extension-less tile class names. `resolve_source_tile_texture_path` probed
   `Art/Terrain/GrassMediumType22` (no `.tga`); `resource_candidates` returns
   no candidate for extension-less names (textures.rs:725-743), so every class
   resolved UNRESOLVED even with the archives mounted. C++ appends ".tga"
   (WorldHeightMap tile-source load).
3. Missing assets. This install carries NO Art/Terrain TGAs at all (only map
   preview TGAs are extracted), so even with the extension fixed every class
   fails to resolve. The 3D world view already covers this with the tree
   buffer's deterministic stand-in tile generator (tree_buffer.rs
   `stand_in_tile_bgra`) — that is why the world looks colored while the radar
   probe showed `has_tiles=false` and `terrain_color_at → None/[[0,0,0]]`.
   Additionally `leftover_radar_terrain_color_at` returned Some([0,0,0]) for
   the heightmap-but-no-tiles state instead of None, so the radar's fallback
   base color never applied.

### Fix (three small pieces)

- `GameClient/terrain/terrain_visual/impl_core.rs`: tile class candidates now
  carry `.tga` (C++ WorldHeightMap parity); when the asset still cannot be
  resolved the class synthesizes the SAME stand-in tiles the 3D tree buffer
  uses, so the radar band matches the world view.
- `GameClient/terrain/terrain_visual/api.rs`: `leftover_radar_terrain_color_at`
  returns None until the visual has hydrated source tiles (radar then uses its
  C++-shaped fallback base color instead of painting black).
- `Main/cnc_game_engine/start_game.rs` (`sync_render_terrain_visual`): after
  the visual hydrates, re-run `Radar::refresh_terrain` (C++ parity:
  `W3DRadar::refreshTerrain`, W3DRadar.cpp:1421-1432) so the once-per-map build
  happens with resident tile data.

### Verified live (manual windowed drives, Defcon6 skirmish)

- Shadow-on (/tmp/mterr_final) and shadow-off (/tmp/mterr_shadowoff): the
  minimap's explored region now renders terrain coloring (same stand-in
  palette as the 3D view) under the camera frustum; black elsewhere is the
  shroud layer (expected, matches MinimapProbe's shroud-blob observation).
- Draw-path probe (pre-removal): `RADARDRAW nonblack=16384/16384` every InGame
  frame in both modes — the terrain texture is fully painted at draw time.
- Pre-fix equivalent drives showed `hydrate classes=11/16 has_tiles=false`
  with every class UNRESOLVED, and a black band.

### Guards / hygiene

- Probes removed: radar/terrain.rs build probe, pipeline_minimap.rs hydrate
  probe, hud.rs draw probe, impl_core debug resolver — all byte-restored.
- `cargo check -p generals_main` clean. Scoped guards: gameworld_shadow
  radar_coupled 2/2 (lib, by name). NOTE: `game-client-rust --test
  terrain_visual` tree_atlas tests (2/27) fail on this shared tree
  (`tree_atlas_mips_reach_terrain_visual_upload_path`,
  `tree_atlas_live_draw_matches_cpp_blit_mip_and_lod`); they exercise only the
  tree-buffer atlas path (no source-tile-class intersection with this change —
  A/B blocked because HEAD impl_core does not compile against sibling-modified
  tree_buffer/api). Left for the main agent's project-wide validation.
- No formatters; serial cargo; caffeinate + short-timeout drives; kill+relaunch
  on freeze. No git writes.

## TextureProvision — unit-texture archives mount and resolve; DXT-decode fix landed in forward-material lane; remaining white = ww3d bind path never consults it (2026-09-03 00:05)

Method: python BIG parser inventory of all 3 archive trees; full log mining of the 22:13
binary's in-match run; one RUST_LOG=debug drive (drive15 lineage, `-loglevel=debug` is the
real switch — `filter_level` in main.rs:218 overrides RUST_LOG); TEXPROBE info! probe in
ForwardPass::ensure_texture (added, driven, REMOVED — source clean); two 640x480 windowed
skirmish drives against Defcon6. Evidence: /tmp/wsmoke/{d15_stderr.log,tex3_stderr.log,
tex1_dozer.png,tex1_cc.png,d15_ingame_dozer.png}.

### Asset provisioning facts (supersedes UnitRenderFix2's "data absent" conclusion)
- Retail ZH tree `Downloads/Command and Conquer Generals + Zero Hour (DIRECT PLAY)
  [blaze69]/…` contains BOTH installs: ZH dir (INIZH/TexturesZH/W3DZH.big…) and the base
  Generals dir (`Command and Conquer Generals/`: Textures.big 3748 members, W3D.big 4556,
  Terrain.big…). Base Textures.big holds every unit skin as DXT .dds under
  `Art\Textures\`: avchinook.dds, avbattlesh.dds, avamphib.dds, avconstdoz.dds,
  housecolor.dds/.dds2 (205 MB) etc. (64 AV* members). Housecolor.tga itself ships in
  W3DZH.big as `Art\W3D\Housecolor.tga` (102 MB).
- The archive backend ALREADY mounts all of this: archive.rs `add_default_search_paths`
  sibling scan (`push_install_layout_paths`) matches the DIRECT PLAY bundle name and pushes
  the base install dir; core init loads `*.big` from every search path. Proof: zero
  "Base Generals archives not loaded" warns in a full in-match log (that warn fires unless
  BOTH Textures.big and W3D.big mount), and 115 `Loading raw texture from archive:` prime
  lines (AVChinook.tga, Avamphib.tga, Housecolor2.tga, …) with ZERO "Missing texture
  fallback"/"Texture parse failed"/"could not be loaded" warns — every prime resolved.
  `texture_candidate_paths` (textures.rs:167) already does C++-parity tga↔dds swap +
  `Art/Textures/` prefixing; big_file_system lookups normalize `\`→`/` + lowercase.
- ZH unit skins are house-color bases: AVConstDoz.W3D references Housecolor2.tga ×171 +
  crane01.tga etc.; the C++ Recolor_Mesh/ZHC livery parity already exists
  (render_item.rs apply_house_color_livery).

### Landed (permanent): forward_materials.rs build_texture DXT→RGBA8 decode
`TextureManager::parse_dds` keeps the compressed DXT payload (test-pinned), but
`ForwardPass::build_texture` built an Rgba8Unorm TextureClass from it →
`replace_pixels` length check (w*h*4) rejected it → build_texture Err → silent white
fallback, no warn (matches all logs: units white, zero texture warns). And even past that,
MeshRenderManager::ensure_gpu_texture_view only uploads 32-bit RGBA. Fix: decode
Dxt1/Dxt3/Dxt5 → RGBA8 in build_texture via the existing ww3d dds_loader decoders
(same fallback TextureManager::create_gpu_texture already uses). Compiles clean
(release build 23:11); probes added and removed; file restored minus the fix.

### Not fixed here (ownership + new root cause, precisely)
In-match units still render as the small white blob (tex1_dozer.png; CC complex still
invisible — DepthSlabFix's depth/pass case). TEXPROBE probe (placed before the empty-name
gate) fired ZERO times across a full in-match run: ForwardPass::ensure_texture is never
reached for unit meshes, i.e. the unit draw path (GameClient render bridge →
ww3d MeshRenderManager/PreparedMeshModel) never routes materials through the
forward-material texture lane at all — its MaterialPassClass textures are built elsewhere
(render_manager.rs = DepthSlabFix; render bridge = presentation lane) and never call the
archive-backed resolver. Next driver: point the ww3d-side pass-texture construction at
TextureManager::prime_raw_texture/create_gpu_texture (or route it through
ForwardPass::ensure_texture), then the DXT-decoded archive textures hydrate units.

### Guards / hygiene
No git writes; no formatters; probes removed (grep TEXPROBE = 0 in tree); combat filter,
world_tests, gameworld_shadow, menu greens untouched. Stray generals_snap/caffeinate
processes killed. /tmp/wsmoke drive scripts + captures preserved as evidence.

---

## DepthSlabFix — depth-lane fragment kills root-caused + FIXED; CC slab depth/clip half FIXED; remaining blocker re-pinned to a world-placement convention mismatch (2026-09-02 22:30-00:20)

### Discriminator 1 — per-mesh NDC/transform/pass-state probe (UNITRENDER3, removed)
Temp env-gated probe in `MeshRenderManager::render_mesh` (dumped per mesh: tri/vert/idx,
pass_index_ranges, per-pass shader depth/blend/alpha/cull bits, post-transform NDC bounds,
w range, opacity/FOW/hidden; camera near/far + proj/view matrices). Pre-fix reality:
- EVERY unit mesh's post-transform NDC z sat in [0.998, 1.000] while w ran 500-1836 —
  the z formula was exactly `1.001 - 1.001/d` = a near=1/far=1000 [0,1] projection.
- The in-match MESH lane camera was `CameraClass` DEFAULTS (90° FOV view_plane ±1,
  near=1, far=1000), NOT the tactical projection forward_render had just installed.
- Root cause chain: `forward_render.rs` called `set_position` AFTER
  `set_view_matrix/set_projection_matrix`; `set_position` marks the camera dirty and
  `update_frustum -> get_view_projection_matrix -> update_matrices` rebuilds BOTH matrices
  from view-plane/clip-plane defaults. Additionally `impl Clone for CameraClass` copied the
  cached matrices but then forced `matrices_dirty=true` and called `update_matrices()`,
  destroying them again on the very `self.camera.clone()` handed to the renderer.
- Meanwhile the TERRAIN lane used the raw tactical matrices. With the old GL-style
  `perspective_rh` z mapping, terrain z_ndc ~0.499 vs mesh z_ndc ~0.999 at the same world
  point: mesh fragments lost the depth test by ~half the depth range everywhere -> the
  slivers, and anything past w=1000 (the CC slab, w=1020-1120) clipped outright -> the
  invisible CC slab. Depth-always showing MORE slivers is exactly this: nothing was
  legitimately occluding the units; they were depth-behind ALL terrain.

### Fixes landed (permanent)
1. `ww3d-renderer-3d/src/rendering/camera_system/camera.rs`:
   - `Clone` now preserves the cached matrices + dirty state and does NOT rebuild — cached
     matrices are data, not stale cache.
   - `set_projection_matrix` clears `matrices_dirty` (explicit matrices are authoritative).
2. `Main/src/graphics/render_pipeline/forward_render.rs`: camera update order is now
   set_position FIRST, then view, then projection (position's dirty-rebuild is transient;
   the tactical matrices are what render).
3. `Main/src/cnc_game_engine/types.rs`: `perspective_rh_from_horizontal_fov` builds the
   C++ D3D depth mapping directly (NDC z [0,1]; `CameraClass::Get_D3D_Projection_Matrix`,
   camera.cpp:707-732) instead of glam's GL-style [-1,1] `perspective_rh` (which in wgpu
   clips everything nearer than the sqrt(near*far) midpoint and halves depth resolution).
   x/y columns unchanged (framing identical). Clip planes now C++ W3DView parity:
   near=MAP_XY_FACTOR=10 (W3DView.cpp:549-563, MapObject.h:35), far=12000 (C++ 1200
   extended x10 whenever the whole terrain can be visible; port keeps the extended value
   so no zoom level clips).
   Depth state vs C++ otherwise already matched: compare Lequal (shader.cpp:979
   `ZFUNC=Get_Depth_Compare()+1` == wgpu LessEqual mapping), write enabled for opaque,
   no bias (Set_DX8_ZBias default 0; the RTS3DScene ZBias 0.0001 shrink only applies to
   the selection-marker extra pass), viewport MinZ/MaxZ 0..1, depth cleared to 1.0 per
   frame by the terrain pre-scene pass.
4. `mesh_system_impl/tests.rs`: repaired a stale source-shape assertion
   (`bones.bones` -> the actual `array<mat4x4<f32>>` + `bones[bone_index]` contract of
   projected_shroud_skinned.wgsl); this failure was pre-existing on HEAD (file unmodified).

### Verification (probe run, fixes in binary)
- Mesh-lane projection in flight = near=10/far=12000 [0,1], 50-degree tactical FOV
  (P00=2.1445, P11=2.8593, aspect 1.333) — no longer the 90-degree defaults.
- Unit meshes z_ndc left the far plane (AVBattleSh 0.983-0.987 at w 558-718) and units now
  RASTERIZE above terrain: /tmp/wsmoke/ds1_ingame_cc.png shows unit geometry rendering
  where the pre-fix run rendered bare terrain. Depth-lane defect CLOSED.
- `cargo check -p ww3d-renderer-3d -p generals_main` clean; `cargo test -p
  ww3d-renderer-3d --lib` 356/356 green. Probes removed (grep UNITRENDER3/PLACEPROBE = 0
  in tree); render_manager.rs carries only the pre-existing landed UnitRenderFix hunks.

### Remaining blocker (precise): world-placement basis mismatch, next driver
With depth fixed, mesh pixels appear but sit DISPLACED from their gameplay objects:
- PLACEPROBE at the `world_matrix * mesh_local_transform` composition site:
  CC slab ABBtCmdHQ#7 world_t=(-271.8, 31.2, 725.8) local_t=(0,0,0) -> composed=(-271.8,
  31.2, 725.8) (self-consistent; model_bounds y 0..51.9 explains the +19 y of the
  reconstruction). AVBattleSh (612.9, 17.5, 73.7), PRG props spread (1177, 670, 1331...).
- Reconstructing the slab's world position from the dumped default-view matrices gives
  (-275.9, 49.9, 734.5) — the mesh draws exactly where world_matrix says. So composition
  is fine; the COORDINATES disagree between lanes: `camera_look_at|x=3108|y=120|z=2201`
  centers the camera on the gameplay point while the dozer mesh renders off-center
  (top-left, partly offscreen in /tmp/wsmoke/ds1_ingame_dozer.png), and the CC complex
  renders ~1100 units from where `view_command_center` aims. I.e. RenderItem.world_matrix
  (pipeline_collect / drawable-state lane) and the tactical view_matrix use DIFFERENT
  gameplay->render basis conversions (the det<0 Y-up/Z-up axis swap is applied on one
  side but not the other, or with a different rotation/sign).
- This is NOT a depth/blend/cull issue (cull=None, depth=Always already ruled out by
  UnitRenderFix2; this probe adds: transform composition healthy, NDC/w sane, shader
  states C++-conformant). Next driver: dump one known object's gameplay position + its
  RenderItem.world_matrix translation + the tactical view_matrix in the same frame, diff
  the two basis maps, and fix the offending conversion (candidates:
  pipeline_collect/pipeline_drawable_state world_matrix construction vs the orbit->view
  matrix in input.rs/start_game.rs; C++ oracle W3DView::buildCameraTransform +
  RenderObjClass transform propagation).
- Evidence: /tmp/wsmoke/ds1_stderr.log (PLACEPROBE + UNITRENDER3 dumps),
  ds1_ingame_{default,cc,dozer}.png (post-fix), prior-run captures overwritten in place.
  Old binary snapshots kept at target/release/generals_ds1 (contains the inert env-gated
  probe; the TREE does not).

## RenderBasisFix — ww3d pass textures now hydrate from the archive resolver (LANDED); "world-placement basis mismatch" DISPROVEN by one-frame dumps — real blocker is the presentation FOW snapshot hiding all units after shroud activation (~frame 474); own-force bypass landed, final stamp channel still open (2026-09-03 01:00-02:30)

Method: five windowed 640x480 skirmish drives vs Defcon6 (rbdrive1-5.sh, /tmp/wsmoke/rb1-5_*);
env-gated GENERALS_WORLDDIFF probes (info!/warn!) at pipeline_collect (gameplay pos vs
world_matrix t + tactical view/proj), MeshRenderManager::render_mesh (mesh_t + mesh-lane
camera + EFFECTIVE projection P-values + NDC), and update_main_crate_vision (per-player
shroud status census). All probes REMOVED from tree (grep WORLDDIFF = 0); `cargo check -p
ww3d-renderer-3d -p generals_main` clean after removal.

### Blocker 2 LANDED: ww3d mesh pass textures hydrate from the archive-backed resolver
- New: `MeshPassTextureProvider` (Arc<dyn Fn(&str) -> Option<TextureClass> + Send + Sync>)
  on `MeshRenderManager` (render_manager.rs; re-exported via mesh_system), forwarded by
  `RendererClass::set_pass_texture_provider` (lib.rs) and
  `WgpuMainRenderer::set_pass_texture_provider` (wgpu_main_renderer.rs).
- `MeshRenderManager::stage_resources_for` (render_manager.rs:1439-1458): when a W3D pass
  texture is a name-only placeholder (no GPU view, no pixels — exactly what
  `TextureClass::from_w3d_descriptor` builds in mesh_system_impl/materials.rs:212), it now
  asks the provider, then uploads the hydrated RGBA8 through the existing first-bind
  `ensure_gpu_texture_view` (cached by name; provider queried at most once per texture).
- Main side (render_pipeline/mod.rs): `resolve_archive_pass_texture` = C++
  WW3DAssetManager::Get_Texture parity — `AssetManager::prime_texture_raw_blocking`
  (block_in_place, same context forward_materials already uses) + `get_raw_texture` +
  Dxt1/Dxt3/Dxt5 -> RGBA8 via the ww3d dds_loader decoders (mirrors
  TextureManager::create_gpu_texture and ForwardPass::build_texture) +
  `TextureClass::with_format(.., Rgba8Unorm)::replace_pixels`. Installed once from
  pipeline_lifecycle.rs right after `ForwardPass::initialize`.
- In-match evidence (rb2_stderr.log, -loglevel=debug): 119 "Loading raw texture from
  archive:" primes THROUGH the new lane incl. Housecolor2.tga, ATFan.tga, ATHQSlab.tga,
  AVChinook.tga; zero decode failures, zero install failures. Textures are bound at draw
  time (screenshots still show few unit pixels — see blocker below for why).

### Blocker 1 root-caused: the "basis mismatch" does not exist; units are hidden by FOW
- One-frame diffs (rb3/rb4): for EVERY dumped object, RenderItem.world_matrix translation ==
  gameplay position EXACTLY (local CC ABBtCmdHQ (3068.0,0.0,2241.3) ==
  sample_unit_pos; enemy CC (-271.8,31.2,725.8); UBCmdHQ (2209.0,109.4,831.8) == collect
  gameplay_pos). Mesh-lane camera == tactical camera (cam_pos matches status camera_pos);
  effective projection == DepthSlabFix's D3D [0,1] near=10/far=12000 (P00=2.1445,
  P11=3.5742, M22=-1.0008, M23=-10.0083, M32=-1); CC slab NDC=(0.000,-0.584,z=0.984) —
  CENTERED on screen. The prior session's "CC renders ~1100 units off" was a misread of
  near-camera white sliver props; depth fix verification should be re-read in that light.
- Actual failure: meshes render (and probes fire) only up to render frame ~474; captures at
  frame ~1050 show render_fow_filtered=467/470 alive, render_item_count=0 — the
  presentation FOW snapshot hides the whole world once the shroud runtime activates, so
  zero unit items are queued (bare terrain + stale UI in every capture since DepthSlabFix).
- FOW census probe (update_main_crate_vision): per-player host-object shroud statuses are
  HEALTHY (player 0: 409 Clear / 4 PartialClear / 18 Shrouded; players 1-2 similar), so the
  statuses are NOT the source. Entity floats (overlay.rs:265-273 stamps
  obj.fow_visibility from ent.fow_visibility_alpha) are the hidden-stamp channel; writers
  are construct.rs:1550/2117 (sync_from_host paths — presentation.rs/session.rs call these)
  and SetFow mutations (apply_host_fow_events <- host_fow_log <- set_fow_residual: no
  production caller => inert channel).
- Landed (permanent, C++-parity): construct.rs both stamp sites now force
  ObjectVisibility::FULLY_VISIBLE for objects owned by the local player (mirrors
  presentation_frame/build.rs:512-518 "Always see own force"; C++ PartitionManager keeps
  own objects Clear because they sit inside their own lookers' radius). This did NOT change
  the rb4 outcome — the float writes that hide the world at ~frame 474 come through a
  channel that did not exercise the patched sync during the drive window. Next driver:
  (1) dump ent.fow_visibility_alpha per entity around frames 400-600 to catch the writer
  (candidates: a construct sync variant with a stale local_player_id, or a shadow-side
  re-stamp); (2) check whether `logic.local_player_id()` on the shadow tick equals the
  census's player 0; (3) re-verify the own-force bypass with a probe at construct.rs:1558
  printing vis per own object.
- Textured-at-correct-position acceptance: NOT yet met in a screenshot (units unqueued at
  capture time); the texture binding itself is proven live at draw time (primes + zero
  fallback warns) and the placement is proven correct (NDC dumps). Once the FOW stamp gap
  closes, both blockers' acceptance should land together in one capture.

### Guards / hygiene
No git writes; no formatters; serial cargo. Probes removed (grep -r WORLDDIFF GeneralsRust/
Code = 0); world_tests/combat filter untouched; ww3d-renderer-3d lib tests not run
(node-wgpu harness needs a window; cascade tests in render_manager.rs untouched by the
change). Drive scripts + captures preserved: /tmp/wsmoke/rbdrive1-5.sh, rb1-5_*.png,
rb1-5_stderr.log (rb2/rb3/rb5 include -loglevel=debug evidence).

## FowWriterHunt — frame-474 FOW writer CAUGHT + FIXED: presentation cell-mix read a dead shroud grid; now reads the live 40wu partition grid (C++ PartitionData::getShroudedStatus parity) + controlling-player team fallback at all stamp sites (2026-09-03 02:30-04:00)

Method: three windowed 640x480 skirmish drives vs Defcon6 (fpdrive1-3.sh, /tmp/wsmoke/fp1_*) with
an env-gated log-only probe (GENERALS_FOWPROBE=1; removed from tree afterwards, grep fow_probe = 0,
`cargo check -p generals_main` clean after removal).

### Root cause (probe evidence, fp1_stderr_run2.log frame=300 window)
- Writer = the FOWRenderingBridge query inside BOTH construct.rs sync stamp sites; the own-force
  bypass fired only 16/3760 stamps and SetFow mutations = 0 (channel genuinely inert). The hidden
  stamps started at frame 0, not 474 — "frame 474" was when the prior session's capture window
  opened, not a state transition.
- `logic.local_player_id()` = Some(0) on the shadow tick — CORRECT. The real holes:
  1. `obj.owner_player_id` is Some(0) for only ~2/470 roster objects (teams={"GLA":5,"USA":2,
     "Neutral":463} — map props + sparse owners), so the own-force bypass almost never matched.
  2. The vision pass's object status mix (vision.rs) sourced per-cell state from
     `ShroudManager::get_shroud_state` — the LEGACY ShroudManager grid, never advanced on the host
     path (vis_set=3, exp_set=3, last_upd=0) → every cell reads Hidden → every non-owned object
     stamped Shrouded → bridge HIDDEN → presentation floats hidden → 465-467/470 render inputs
     FOW-filtered (bare terrain in every capture since DepthSlabFix).

### Fix (permanent, C++ parity)
- vision.rs status mix now reads `gamelogic::object::partition_cell_shroud_status(pid, cx, cz)` —
  the LIVE 40wu partition shroud grid that the same pass already stamps via
  `stamp_partition_cell_lookers`. This is exactly C++ `PartitionData::getShroudedStatus`
  (PartitionManager.cpp:1582) mixing footprint COI cells; the dead legacy-grid reader
  (`leftover_discrete_circle_looker_cell`) is removed (clean cutover).
- construct.rs both sync stamp sites + presentation_frame/build.rs `fow_visibility` +
  `freeze_direct_object_shroud_facts` resolve the controlling player with the same team fallback
  the look pass uses (`obj.owner_player_id.or_else(logic.player_id_for_team(obj.team))`; C++
  Object::getControllingPlayer parity) so the own-force bypass actually covers own force.

### Evidence (fp1_stderr.log run5/run6, fp1_status_timeline.log)
- hidden stamps 3736→2560 per window; presentation hidden 465→320/470 (the 320 are beyond all
  looker radii — correct FOW); render_fow_filtered 465→318-320 stable from frame 234 through 638+.
- Unit meshes queued PAST frame 474 (items=103 through frame 441/578/638 with no collapse; the
  old bug froze collection at ~frame 474). local_mobile_units=1 (dozer alive, gameplay issues).
- NOT yet met: "textured units visible" in a screenshot. Drives 2/3 centered the camera on the
  dozer/CC (sample_unit_pos 3068.0,0.0,2241.3 / 3108.0,120.0,2201.3) — captures still show the
  same white untextured slivers (also present in rb1 pre-fix captures). Mesh-pass texture
  hydration is proven live at draw time (prior session's 119 primes, zero failures), so the
  remaining gap is model/material selection or lighting for those specific objects — next
  driver should dump UnitRenderInput draw_models + mesh material for the CC/dozer at capture time.

### Guards / hygiene
No git writes; no formatters; serial cargo. Probes fully removed (fow_probe.rs deleted; call
sites in construct.rs / apply_host_misc.rs / overlay.rs / camera_drain.rs / lib.rs reverted).
combat filter 966/0, world_tests catalog, gameworld_shadow 302/302 and ww3d-renderer-3d 356/356
suites NOT re-run (project-wide validation is the driver's job); scoped `cargo check -p
generals_main` clean before and after probe removal. Drives + captures preserved: /tmp/wsmoke/
fpdrive1-3.sh, fp1_*.png, fp1_stderr*.log, fp1_status_timeline*.log.

---

## TextureBindFix — unit-material/texture selection DISPROVEN as the blocker; meshes draw in-view with hydrated textures and zero pixels land; blocker re-pinned to depth/present interaction (2026-09-03 04:00-05:50)

Method: four windowed 640x480 skirmish drives vs Defcon6 (tbdrive1.sh, /tmp/wsmoke/tb1_*)
with an env-gated log probe (GENERALS_MATDUMP) at three seams: pass build
(forward_materials build_material_pass_from_mesh / assign_stage_textures_for_pass),
collect (pipeline_collect per-item world/NDC for dozer/CC templates), and draw
(ww3d MeshRenderManager draw_material_pass: mesh_t + camera view-proj NDC; and
stage_resources_for texture staging). All probes REMOVED after diagnosis
(grep MATDUMP/GENERALS_MATDUMP = 0 in tree; `cargo check -p generals_main --bin
generals` clean; forward_materials.rs and render_manager.rs restored to their
pre-probe byte state). Evidence: /tmp/wsmoke/tb1_stderr_run{1,2b,3,4}.log,
tb1_{dozer,cc,late}_run{1-4}.png.

### 1. Texture/material selection is HEALTHY at every seam (handoff question closed)
- Pass build (run3): ABBtCmdHQ meshes resolve per-stage textures correctly —
  BUILDING→ATHQSlab.tga, FAN01/03→ATFan.tga, HOUSECOLOR01-06→Housecolor2.tga,
  FENCE→PMWallChn2.tga, GIRDERS→ATrailings01.tga (MATDUMP input lines). RenderItem
  materials carry diffuse 0.80/0.80/0.80 opacity 1.00; draw_models keys exact.
- Draw staging (run1): stage_resources_for sees stage-0 textures with REAL pixels
  and no GPU view (first-bind upload path): Avbattlesh.tga 512²/1 MiB, Housecolor2.tga
  256², AVChinook.tga, PRGrey, Coplight — zero "MATDRAWW miss" lines, zero
  fallback-white hydrations for unit textures. The provider/archive lane is NOT
  the defect.
- FOW/opacity: every unit-textured pass draws with fow_alpha=1.00, opacity=1.00.
- Transforms: dozer meshes draw at mesh_t=(3093.5,123.3,2208.5)/(3106.4,123.3,2194.2)
  (gameplay (3108,120,2201) ✓); local CC at (3068.0,0.0,2241.3) with Housecolor2
  slabs at (3082-3090,44-46,2297-2299) and ATFan at (3044.3,28.1,2236.5) — the whole
  local base ISSUES draws.
- Mesh-lane camera: cam=(3068.0,430.0,1837.3) = the tactical camera (50,993 of
  53,433 draws; remainder (827.9,327.5,-404.0) = a second camera pass). The dozer
  projects to mesh_ndc=(-0.18,-0.20,0.98) w=469.8 — CENTERED IN VIEW with sane w
  and D3D-mapped z (0.98 at d=470, near=10/far=12000).

### 2. What actually happens on screen (pixel-verified, supersedes prior capture reads)
- All in-match captures since at least d15 (Sep 2 22:53) — ds1, rb1-5, fp1, tb1 — are
  pixel-equivalent: brown terrain, black control bar with red placeholder buttons,
  ONE white UI strip at y=358-379 (369 px total, control-bar text). There are NO
  unit pixels of any color in ANY capture; the historical "white untextured slivers"
  description does not correspond to queued unit meshes reaching the frame. (Caution
  for future drivers: frame.png is RGBA PNG; decoding it as RGB8 produces a
  diagonal channel-weave that looks like corruption.)
- At the dozer's exact in-view pixel (262,288) the frame shows pure terrain
  (116,106,83)-family — the mesh issues draw calls through draw_material_pass with
  correct bind groups and never lands a fragment.

### 3. Remaining blocker (precise, next driver)
Everything through fragment SETUP is proven correct (geometry, camera, NDC, textures,
bind groups, alpha). The kill happens between rasterization setup and the present:
depth interaction with the terrain pre-scene pass is the prime suspect (mesh
Lequal vs terrain-written depth; DepthSlabFix aligned the formulas but the terrain
pass writes depth in the SAME frame and meshes may lose everywhere terrain draws).
Cheapest discriminator, one build+drive: env-gate `create_depth_stencil_state_from_shader`
(wgpu_pipeline_manager.rs:1151) to depth_compare=Always + depth_write=false for the
mesh pipelines (UnitRenderFix2 discriminator #3) and/or skip the terrain pre-scene
pass for one run; if units appear, diff terrain-vs-mesh z at one world point and fix
the offending lane. Secondary defect found: 13 draw passes carry stages=[] (no
textures at all → shader outputs unmodulated white); find via the run4 MATDRAWDRAW
dedupe key and give them the C++ MissingTexture path.

### Guards / hygiene
No git writes; no formatters; serial cargo. Probes fully removed (grep
GENERALS_MATDUMP = 0 in tree); combat filter / world_tests / gameworld_shadow /
ww3d-renderer-3d suites NOT re-run (project-wide validation is the driver's job);
scoped `cargo check -p generals_main --bin generals` clean after removal. Drive
script + captures + probe logs preserved: /tmp/wsmoke/tbdrive1.sh, tb1_stderr_run*.log,
tb1_*_run*.png.

---

## DepthDiscrim — depth test CONFIRMED as the unit-render blocker via env-gated discriminator; z-diff instrument + MissingTexture routing landed; final lane fix blocked on box contention (2026-09-03 05:50-08:10)

### 1. Discriminator result (one build, three documented env gates)
`GENERALS_DISC_DEPTH=1` (mesh-pipeline `depth_compare=Always` +
`depth_write=false`, wgpu_pipeline_manager.rs `get_or_create`) makes units LAND:
capture `/tmp/wsmoke/disc_depth_disc1_dozer.png` shows white unit geometry at the
dozer/CY look-at plus the full in-match command bar, cash display and sidebar
(baseline captures are terrain+placeholder bar only, 45.42% of frame pixels
differ). The historical "white untextured slivers" ARE the queued unit meshes:
they were being depth-rejected, exactly as TextureBindFix hypothesized.
`GENERALS_DISC_CULL=1` was not needed (cull untouched in the depth run — winding
is innocent); `GENERALS_DISC_NOTERRAIN=1` was inconclusive as designed (other
pre-scene callbacks keep `clear_color=None`, so the scene pass `Load`s a
never-cleared attachment instead of clearing — noted for the next driver).
C++ parity anchors: dx8wrapper.cpp:3686-3687 (`D3DCULL_CW`, `D3DCMP_LESSEQUAL`).

### 2. Landed in-tree (documented diagnostics, defaults are C++ parity)
- `wgpu_pipeline_manager.rs`: `disc_depth_always()` / `disc_cull_none()`
  (LazyLock env gates) applied to every pipeline built there.
- `pipeline_prewarm.rs`: `GENERALS_DISC_NOTERRAIN` skips the terrain pre-scene.
- ZPROBE (`GENERALS_ZPROBE=1`, one-shot): CPU-side mesh clip z/w + terrain
  height per render item, plus a synchronous GPU depth readback of the depth
  attachment (own encoder, submit/poll/map/read/unmap inside the post-frame
  callback — no cross-frame mapped window). `ww3d-engine` DepthTarget gained
  `COPY_SRC` for this. `cargo check -p generals_main --bin generals` clean.

### 3. MissingTexture routing LANDED (secondary defect, 13 `stages=[]` passes)
`forward_materials.rs` `build_material_pass_from_mesh`: the
`get_texturing() != Disable` gate is removed — EVERY pass that ends with zero
bound stage textures now gets the shared `w3d_missing_texture.tga` identity at
stage 0 (W3DAssetManager.cpp:127-225 / dx8wrapper.cpp:2875-2889 parity). The 13
in-match `stages=[]` passes previously rendered unmodulated white; they now
render the retail missing-texture marker. Compile-checked; runtime screenshot
verification pending the same drive window as the z-diff.

### 4. Blocker (environmental, NOT code): box contention starves windowed drives
Since ~06:40 an Android emulator (qemu-system-aarch64-headless, sibling lane)
holds load ~4-5; Menu frames run 300ms+ and children need >10 min to reach a
state the drive can use (status publication trails boot). Four consecutive
z-diff drives (disc5/6/7/zclean, incl. one from a scratch HEAD+my-changes copy
at `/tmp/zbuild_generals`) timed out before Menu reveal. The z-diff numbers
(GPU depth at unit pixels vs mesh z/w) are the only missing input for the final
lane fix. Next driver: re-run
`GENERALS_BIN=<clean binary> GENERALS_ZPROBE=1 /tmp/wsmoke/discdrive.sh zprobe <tag>`
on a quiet box (`pgrep qemu`), then fix whichever lane the numbers indict —
terrain-written depth (fix terrain lane), mesh camera z (fix camera lane), or a
depth-writing overlay (per-pass clear semantics). Do NOT keep the Always gate
as the fix: it breaks occlusion (C++ is LESSEQUAL with writes).

### Guards / hygiene
No git writes; no formatters; serial cargo (scratch builds used a copied tree
with sibling WIP files restored to HEAD via `git show` — zero writes to the
live tree or index). Sibling churn note: GameLogic AI lane WIP swept into
release builds breaks boot (child skips Menu, status.txt never published) —
coordinate before building shared binaries. Harness: `/tmp/wsmoke/discdrive.sh`
(mode depth|cull|noterrain|zprobe|control, GENERALS_BIN override),
`/tmp/wsmoke/disc_pixdiff.py` (RGBA-correct), captures
`/tmp/wsmoke/disc_*_{dozer,cc}.png`, logs `/tmp/wsmoke/disc_*.log`.

---

## DepthLaneFix — final unit-render blocker CLOSED: terrain-vs-mesh depth lane was a VIEWPORT mismatch; mesh pass now renders under the camera tactical viewport (dx8wrapper Set_Viewport parity); units land in-match with default C++ depth state, no env gates (2026-09-03 08:20-10:10)

### 1. Quiet-box ZPROBE drive + harness repair
Live tree was clean at 3f30f3ae6; rebuilt release (provenance-known binary).
First drive (z1/z2 era) stalled at Menu with status.txt empty and the child
skipping Menu: `/tmp/wsmoke/discdrive.sh` had been edited at 07:46 (after the
06:17 success) and LOST its `-gpui_control/-gpui_status/-gpui_frame` args —
`RuntimeHostBridge::from_command_line` returns None without them, so nothing
published status or drained control (run_loop still reached state=Menu
internally). Restored the args to the launch line; drives flowed again.
ZPROBE itself then never fired: (a) main.rs:221 hard-filters
`generals_main::graphics` to Warn, so the probe's `info!` dumps were invisible;
(b) the probe latched on the FIRST execute() ever (Loading frame #0, zero
items). Fixed both: fires on the first world-scene frame with items; logs at
warn. Drive evidence: disc_zprobe_{z3,z4,z5}.log + /tmp/wsmoke/disc_zprobe_stderr.log.

### 2. The z-diff verdict: NOT a z-formula mismatch — a VIEWPORT mismatch
ZPROBE (16 distinct model families, 3x3 GPU depth neighborhoods, corner +
column profile):
- Dozer AVCONSTDOZ_A (3108,120,2201.3): mesh_z/w == ground_z/w == 0.979874
  EXACTLY (on-ground item; both lanes share the same D3D [0,1] near=10/far=12000
  projection — DepthSlabFix verified on both lanes via the same execute() args).
- Stored gpu_depth at that pixel: 0.977765 — CLOSER than the mesh z.
- Center-column profile (px=320): smooth monotone terrain ramp
  0.988029@py8 → 0.974891@py360, then a DISCONTINUITY to ~0.9826 at py≈392,
  and bottom corners read exactly 1.000000.
384 = 480 × tactical_view_height_frac (0.8): the terrain/water/selection
pre-scene passes render into viewport (0,0,640,384) while the "WW3D Main
Render Pass" set NO viewport (full 480). Every mesh fragment compared against
the WRONG ROW of terrain-written depth (1.25× row skew, monotonic toward
nearer terrain) and lost LESSEQUAL everywhere visible; the only mesh fragments
that could land were below py=384 — under the command bar. The Always
discriminator "worked" by ignoring the (misrowed) depth entirely. No per-pass
clear problem, no camera-z problem, no shader depth_compare problem
(from_w3d_shader enum order == C++ W3dDepthCompareType; model_loader's parsed
depth_compare bytes are inert — ShaderClass default is Lequal+Enable).

### 3. Fix landed (C++ dx8wrapper Set_Viewport parity)
D3D applied the tactical viewport as DEVICE state — every scene lane rendered
through the same rect. wgpu viewports are per-pass, so the mesh lane must opt
in explicitly:
- `ww3d-renderer-3d/src/lib.rs`: `RenderTargets` gained `size: (u32,u32)`;
  `render_with_targets` now applies `camera.get_viewport()` (normalized,
  set by ForwardPass to (0,0)-(1,frac)) scaled by the attachment size as
  viewport+scissor on the main pass before recording draws. `(0,0)` size
  (legacy `wgpu_wrapper::with_render_targets` path) keeps full-attachment
  behavior unchanged.
- `ww3d-engine/src/lib.rs`: `RenderFrame::size()` accessor.
- Same-parity sweep of the other world-space lanes that read terrain depth:
  selection world overlay (selection_renderer.rs, via new
  `RenderPipeline::tactical_viewport_pixel_size()`) and the post-frame
  volume-shadow/occlusion overlay (`record_shadow_and_occlusion_passes`
  gained `viewport_px`; pipeline_execute passes the tactical size; legacy
  display.rs caller passes (0,0)).
Env gates (GENERALS_DISC_DEPTH/CULL/NOTERRAIN, GENERALS_ZPROBE) stay as
documented diagnostics; defaults are C++ parity (LESSEQUAL + depth writes).

### 4. Verification
- Two independent control drives (disc_control_vfix1/vfix2 + debug zprobe run):
  mesh fragments LAND in-match with the default depth state — white geometry
  at the dozer/CC look-at, terrain correct in the top-80% band, full in-match
  command bar. Structurally identical across runs (reproducible), and
  visually the same landing as the GENERALS_DISC_DEPTH=1 capture
  (disc_depth_disc1_dozer.png) — without any env gate. Captures:
  /tmp/wsmoke/disc_control_vfix1_dozer.png, /tmp/wsmoke/vfix2_zprobe.png.
- ww3d-renderer-3d --lib: 356/356 (one transient 2-fail on a contended
  re-run; clean 356 pass on re-run).
- combat-filter catalog guard: wave966 unit-catalog residual test passes
  (generals_main --lib, 10329 filtered).

### 5. Remaining gap (NEXT lane, NOT depth): unit fragments land WHITE
The landed geometry renders the missing-texture marker white — identical to
the pre-routing Always capture. This run shows ZERO `Loading raw texture from
archive` primes even at RUST_LOG=debug: the MeshPassTextureProvider is never
consulted, i.e. passes end with no stage textures at all (names never reach
`ensure_texture`), so the landed MissingTexture routing correctly paints them
with w3d_missing_texture.tga (marker is 54% pure white). Next driver: trace
`per_pass_stage_texture_names` population for ABBtCmdHQ/AVCONSTDOZ meshes
(`is_valid_texture_name` / `stage_texture_names_from_ids`) — tb1's MATDUMP
showed those names resolving, so something between mesh parse and pass build
dropped them since.

### Guards / hygiene
No git writes; no formatters; serial cargo. Changed files:
`ww3d-renderer-3d/src/lib.rs`, `ww3d-renderer-3d/src/rendering/wgpu_renderer/wgpu_wrapper.rs`,
`ww3d-engine/src/lib.rs`, `Main/src/graphics/render_pipeline/pipeline_execute.rs`
(ZPROBE gate/level + shadow viewport arg), `Main/src/graphics/render_pipeline/pipeline_lifecycle.rs`
(accessor), `Main/src/graphics/selection_renderer.rs`,
`GameClient/src/display/shadow_pass.rs`, `GameClient/src/display/display.rs`.
Harness fix: /tmp/wsmoke/discdrive.sh regained -gpui_* args. gameworld_shadow
302/302 NOT re-run (no shadow-lane behavior change expected; viewport-only);
world_tests catalog untouched by the change.

---

## TexNameFix — texture-name trace: parse lane and ForwardPass pass-build lane are CLEAN; white rendering localizes to the ww3d MeshRenderManager first-bind/upload path (2026-09-03 10:30-12:10)

### 1. What was traced (task: per-pass stage texture names for ABBtCmdHQ/AVCONSTDOZ)
- **Chunk ground truth** (Python BIG reader over W3DZH.big + C++ w3d_file.h constants):
  mesh chunk order is HEADER(0x1F)→VERTICES→NORMALS→TRIANGLES(0x20)→SHADE(0x22)→
  MATERIAL_INFO(0x28)→VERTEX_MATERIALS(0x2A)→SHADERS(0x29)→TEXTURES(0x30→0x31
  TEXTURE→0x32 NAME)→MATERIAL_PASS(0x38→0x48 TEXTURE_STAGE→0x49 TEXTURE_IDS).
  ABBtCmdHQ.W3D (73,955B) and AVCONSTDOZ_A.W3D (43,687B — EXISTS in W3DZH.big;
  the earlier "not in any BIG" impression was wrong) both carry real texture
  tables (Housecolor2.tga, Avbattlesh.tga, avconstdoz.tga, ATHQSlab.tga+22 more)
  and per-polygon TEXTURE_IDS (CC BUILDING: 474 ids over 23 textures).
- **Main parse probe (temp test, real archive bytes)**: `W3DLoader::
  parse_w3d_data_legacy` populates `texture_library`, `per_pass_stage_texture_ids`
  AND `per_pass_stage_texture_names` correctly for all 3 in-match models
  (e.g. AVCONSTDOZ_A CHASSIS → [["avconstdoz.tga"]], mat_tex=Some("avconstdoz.tga")).
  Nothing drops between mesh parse and W3DMesh.
- **ForwardPass pass-build probe (temp warn in assign_stage_textures_for_pass)**:
  runs for the unit lane, `is_valid_texture_name` passes, `ensure_texture`
  returns Some for every probed stage (assigned=true, e.g. RADAR02 →
  "Avbattlesh.tga"). ZERO "Missing texture fallback" warns all session — the
  ForwardPass lane binds real pixel-backed textures, not fallbacks.

### 2. Where the white actually comes from
- **ww3d-lane draw probe (temp warn in MeshRenderManager::stage_resources_for)**:
  the drawn passes reach the ww3d lane with stage 0 present but NO GPU view and
  the provider installed (`WW3DSTAGE stage 0 name-only texture 'Avbattlesh.tga'
  provider=true`); stages 1-7 empty (single-stage passes — normal). The probe
  fired at Menu time (menu background AVBattleSh/AVChinookAG use the same lane),
  so menu+in-match share this path.
- Capture forensics (tex1/tex2/tex3 vs disc_control_vfix1): the "white" unit
  band at py≈352-384 is a smooth 201-254 gradient — consistent with the
  w3d_missing_texture.tga marker (96% white + sparse dark dots) under bilinear
  filtering, i.e. MeshRenderManager FALLBACK, and NOT the ForwardPass magenta
  4x4 fallback (never observed). So pass textures exist at build time but the
  first-bind upload/provider hydration in `ensure_gpu_texture_view` does not
  produce a bound view at draw time.

### 3. Measurement gotcha (invalidates earlier "zero primes" evidence)
`main.rs` env_logger setup calls `.filter_level(level)` AFTER parsing RUST_LOG,
which overrides the unnamed filter: **RUST_LOG=debug never takes effect**; the
`-log_level` CLI arg is required. "Loading raw texture from archive"
(assets::textures, debug!) was therefore invisible in tex1/tex2 — provider
consultation/primes may well have happened and failed. Re-probe with
`-log_level=debug` (or temporarily log at warn).

### 4. Next driver (precise)
One env-gated or warn-capped probe round in `MeshRenderManager::
ensure_gpu_texture_view` (log per unique name: pixels.len(), width, height,
format on each early-return) plus `resolve_archive_pass_texture`
(Main/src/graphics/render_pipeline/mod.rs:89 — log None results; get_raw_texture
miss vs decode failure). Hypotheses in order: (a) raw pixels empty in the
TextureClass that reaches the ww3d lane (Arc share/mutation), (b) provider
resolves but its `ensure_gpu_texture_view` fails the same guard, (c)
`pass.get_texture(stage)` is None at draw for the in-match CC (probe cap
consumed by menu models in tex3 — dedupe by name, not count).

### 5. Probes removed; tree clean
All temp probes/tests removed (git diff vs HEAD on the four touched files is
empty: ww3d render_manager.rs, forward_render.rs, forward_materials.rs,
assets/models/tests.rs). Guards untouched (combat filter, world_tests,
gameworld_shadow, ww3d 356 not affected — diagnostics only, no behavior
change landed). Captures/logs: /tmp/wsmoke/tex{1,2,3}_dozer.png,
tex{1,2,3}_stderr.log; BIG extracts /tmp/wsmoke/bigw3d/.

---

## UnitTexBind — white-unit bind path ELIMINATED as the blocker: pass state at DRAW is perfect (real textures, real UVs, fallback never fires); white is generated GPU-side; UTBLIGHT probe staged as the next discriminator (2026-09-03 12:00-13:15)

Method: three windowed 640x480 skirmish drives vs Defcon6 (utbdrive1.sh → /tmp/wsmoke/utb_utb{1,2,3}_stderr.log, utb_utb{1,3}_dozer.png) with env-free warn-level probes in render_manager.rs (UTBMESH per-model draw-state dump, UTBWHITE fallback-tail logger, UTBPROVMISS provider-miss logger; caps filled menu-first so all decisive lines are menu-era, but pass state is per-model and stable). Plus offline python BIG/DDS/W3D parsing of the retail archives (Textures.big + /tmp/wsmoke/bigw3d/*.W3D). Coordination: took render_manager.rs ownership from TexNameFix (their probes removed, their 4 files clean); ctrl/text siblings kept their lanes.

### 1. Everything CPU-side is PROVEN correct at draw time (supersedes DepthLaneFix §5 "provider never consulted")
- That session's "zero primes" was a log-filter artifact: `Loading raw texture from archive` is `debug!` (generals_main::assets::textures) and main.rs filter_level overrides RUST_LOG — only `-loglevel=debug` shows it. utb1 with `-loglevel=debug`: 109 primes at menu + 5 in-match (avconstdoz.tga, ZBSupplyDk.tga, CTgraymetal.tga…), zero decode failures.
- UTBMESH (drawn `prepared.material_passes`): dozer/battleship/chinook meshes draw `pass0 mask=01 [s0='avconstdoz.tga' 256x256 262144B texel0=(255,255,255) texelMid=(49,40,33)]` — the exact texture that assign_stage_textures_for_pass filled, WITH real decoded pixels. Pass-instance divergence DISPROVEN.
- UTBWHITE (white 1x1 fallback returned while a texture is bound) = 0 across a full session. UTBPROVMISS = 0. The MeshRenderManager fallback NEVER fires for stage 0.
- UV stream healthy: `uvch=0 layer0len=20 v0..2=(0.454,0.067)(0.454,0.079)(0.523,0.079)` — real varied per-vertex UVs; avconstdoz_d.W3D stage texcoord pools exactly match per-mesh vertex counts (46/20/62/8/56/208), no per-face remap needed.
- Texture content healthy end to end: parse_dds stores headerless level-0 DXT bytes; python decode of the same avconstdoz.dds blocks gives mean RGB (99,95,80) tan — matches texelMid=(49,40,33) region. Decode is NOT white.

### 2. What the screen shows
utb1/utb3 + tex2 captures pixel-verified: unit silhouettes are FLAT pure (255,255,255) with hard unshaded edges against correct terrain — no gradient, no texture detail, no anti-aliasing. A textured surface under any sane lighting cannot produce exactly-flat 255; nor can the white fallback (mask would be 01 with a bound white texture, still lit → 200-255 gradient).

### 3. Conclusion + staged next probe
The white is synthesized GPU-side AFTER a correct bind: prime suspects (a) lighting uniform scale for the mesh lane (`lighting.ambient_color`/light colors in the camera/model binds — map metadata passes through `light_from_map_channels` UNNORMALIZED; a 0-255-scale ambient clamps every fragment to flat white while the terrain lane, which carries its own lighting path, stays correct — matches the white-vs-terrain split exactly), (b) the upload→sampler path producing a white sample. UTBLIGHT probe (one `Once` warn! dumping `render_info.lighting.ambient` + per-light colors) is WRITTEN but NOT landed — render_manager.rs was restored to HEAD pre-probe state (git-diff clean) when the build queue broke on sibling `game-client-rust` in-flight edits; next driver: re-add UTBLIGHT (10 lines, inside the UTBMESH block site, draw_material_pass), rebuild, one drive reads it off the first frame.

### Guards / hygiene
No git writes; no formatters. render_manager.rs restored byte-identical to HEAD (`git diff` empty, grep UTB/W3DSTAGE = 0); `cargo check -p ww3d-renderer-3d --lib` clean after restore. NOTE: target/release/generals (12:36-12:49 era) still CONTAINS the older UTB probes — rebuild before trusting any new binary's silence. ww3d 356 / combat filter / gameworld_shadow / world_tests untouched. Evidence: /tmp/wsmoke/utb_utb{1,2,3}_stderr.log, utb_utb{1,3}_dozer.png, utbdrive1.sh; sibling frames under /var/folders/.../generals_exec_smoke_manual_1788448303 (gtprobe, 200 UTBMESH lines).

---

## MinimapShroud — minimap shroud semantics vs C++ W3DRadar: lib-path look/unlook driver + accumulator removal landed; live minimap verified (2026-09-03 11:30-13:10)

### Scope
Verify/fix minimap initial-reveal radius + movement-driven shroud reveal vs
GeneralsMD C++ (W3DRadar.cpp + PartitionManager.cpp + Object.cpp), with
windowed drives on Defcon6 skirmish.

### C++ semantics (citations)
- Radar shroud texture: per-cell alpha SHROUDED=255 / FOGGED=127 / CLEAR=0;
  `W3DRadar::setShroudLevel` paints partition-cell rects
  (W3DRadar.cpp:1252-1314); `clearShroud` alpha=0 fill at reset
  (W3DRadar.cpp:1232-1248); draw order terrain→overlay→shroud→icons
  (W3DRadar.cpp:1369-1401).
- Reveal is logic-side: `PartitionCell::addLooker/removeLooker/addShrouder`
  edge-trigger `TheRadar->setShroudLevel` (PartitionManager.cpp:1264-1265,
  1300-1301, 1327-1328); `PartitionManager::doShroudReveal` DiscreteCircle
  (3937-3958); `Object::look/unlook` cycle on cell change
  (`PartitionData::friend_updateCellsTouched` → `obj->onPartitionCellChange`,
  PartitionManager.cpp:2052-2062; Object.cpp:4779-4784, 4909-5042) with
  `queueUndoShroudReveal` → old circle drops CLEAR→FOGGED after
  `UnlookPersistDuration` (processed each partition update, 2739).
- Initial reveal at map start = first look of starting objects
  (ShroudClearingRange, UNDER_CONSTRUCTION clamped to bounding radius,
  Object.cpp:5128-5137).

### Divergences found → fixes landed (gamelogic lib path)
1. `ShroudManager::update_shroud_grid_for_player` re-added looker circles
   every vision recalc (~10 frames) and NEVER removed lookers → unbounded
   counter growth and permanent CLEAR on everything ever seen (defeats the
   C++ CLEAR→FOGGED transition). Removed; Phase-7 update now only refreshes
   object visibility/explored sets (shroud_manager.rs).
2. No movement driver: gamelogic PartitionManager lacked the C++
   onPartitionCellChange hook. Added `PartitionObject::shroud_last_cell`
   (C++ `m_lastCell`, NULL-initial → first sync fires, PartitionManager.cpp
   parity), cell-change capture in `update_object_position`,
   `take_cell_changed_events`, `CollisionSystem::partition_manager_mut`, and
   deferred dispatch of `Object::handle_partition_cell_maintenance` in
   `resolve_damage_and_physics` (impl_update.rs:984-1033) — deferred so no
   object read guard is held (deadlock-free; team_identity.rs:81 pattern).
3. Strengthened `test_fow_uses_shroud_clearing_range_not_vision_range` with
   positive grid assertions via the doShroudReveal primitive (visible within
   25-unit shroud-clearing circle; NOT visible at 100 despite vision 300).

Note: the HOST (dual-world) minimap path already implements the faithful
look/unlook in `update_main_crate_vision`
(Main/src/game_logic/world_objects/spawn_templates/vision.rs) — per-object
last-look maps, skip-if-unchanged, queue_undo at old circle
(persist 150 frames = retail 5000 ms), stale-unlook on death, called per tick
(world_tick/step.rs:1228) and at load_map (world_load.rs:666, initial reveal
around start units). The gamelogic accumulator was inert on that path.

### Live verification (SHROUDPROBE temp probe, since removed)
- Initial reveal: radar grid clear=500/16384 cells by logic frame 120
  (CC+dozer+start union), rest shrouded — matches C++ initial look.
- Draw path (hud.rs:265-314) matches W3DRadar::draw layering/alphas.
- Captures: /tmp/wsmoke/mm_before_b1_{initial,moved}.png,
  mm_after_a1_{initial,moved}.png, probe logs mm_probe_p1/p2_stderr.log.
- INCOMPLETE: movement-reveal end-to-end drive — the runtime-host picker kept
  selecting immobile units (`select_local_unit` → CC per sample_unit_pos;
  `select_all_combat` → 0 units at skirmish start; only the dozer is mobile
  and its selection id vs sample field diverged). The corrected drive
  (select the dozer / spawn combat units first) did not complete in this
  session; the host-path unit tests (spawn_templates tests: moved looker,
  death unlook, fog-after-persist) cover that transition.

### Pre-existing hang found (NOT fixed here, out of scope)
`test_fow_uses_shroud_clearing_range_not_vision_range` and
`test_spy_vision_shares_enemy_vision` hang at HEAD: same-thread RwLock
deadlock in `GameObjectInstance::new` → `init_object` → module `on_create`
re-entering the base write lock (object_manager.rs:360-368; confirmed by
process sample; spy test is byte-identical to HEAD).

### Guards / hygiene
cargo check -p generals_main --lib clean after probe removal; radar_coupled
re-run launched. No git writes; no formatters; serial cargo coordinated with
GiantTextFix (release builds 12:22, 12:42 include the shroud fixes). Temp
GENERALS_SHROUDPROBE added and REMOVED. Changed files: GameLogic
system/shroud_manager.rs, object/collide/partition_manager.rs,
object/collide/collision_system.rs, system/game_logic_impl/impl_update.rs.

---

## CtrlBarTexFix — control-bar red buttons/icons root-caused + FIXED (2026-09-03 12:00-13:30)

### Root cause (NOT the mapped-image import, NOT texture hydration)
SeamFix §2's "residual is texture hydration" hypothesis was wrong for the command
grid. Offline probe (temp bin, removed) + env-gated draw probes (removed) proved:
- mapped-image import is COMPLETE (1320 common → 1320 client; SAACommand/SACDozer/
  SAPowerPlant/... all present) and their atlases hydrate through EnglishZH.big
  (SAUserInterface512_00*.tga decode 512x512; menu `Buttons-Left` hydrates too).
- Retail ControlBar.wnd authors `STATUS = ENABLED+IMAGE+...` on ButtonCommand01-14 /
  ButtonQueue01-09 / UnitUpgrade1-5 with every draw-data slot still `IMAGE: NoImage,
  COLOR: 255 0 0 255` (the red placeholder). C++ picks the device draw func by
  WIN_STATUS_IMAGE at creation (GameWindowManager.cpp:1857-1862) → ImageDrawFunc;
  W3DGadgetPushButtonImageDrawOne (W3DPushButton.cpp:288-368) draws NOTHING when the
  state's image slot is empty, so unbound buttons are invisible until
  setControlCommand binds the cameo. The port selected the draw func by authored
  draw-data images alone, left those buttons on the SOLID draw, and painted the
  authored `255 0 0 255` as solid red — the exact red rectangles in the screenshots
  (red fill + pink 255 128 128 border). Runtime binds were succeeding the whole time
  (probe: `bind:* found=true` for all 12 match-start commands).

### Fix (GameClient, 3 files)
- `gui/window_manager/draw.rs`: `default_draw_uses_image()` — default gadget draw
  selection now honors `WindowStatus::IMAGE` (C++ GameWindowManager creation parity)
  in addition to authored draw-data images.
- `gui/w3d_gadget_draw/push_button.rs`: `draw_push_button_image_base` no longer falls
  back to the solid draw when the state's image slot is empty (C++ ImageDrawOne
  parity: unbound image-status buttons paint nothing, never the red placeholder).
- `gui/w3d_gadget_draw/tests.rs`: `w3d_push_button_image_draw_falls_back_when_mapped_
  image_missing` replaced by `..._queues_nothing_when_no_image_is_bound` defending the
  C++ contract (unbound image-status button queues zero fill commands).

### Also proved while diagnosing (asset-side, unchanged)
SA/SN/SUControlBar512_001.tga genuinely DO NOT exist in the DIRECT PLAY retail BIGs
(entry tables + raw byte scans) — SAPowerPoint*/SABeacon/SATray* (power dots, tray)
can never hydrate from retail assets; they render the honest slate fill
(game_window_global.rs) when drawn. SNExpBar/SATraySmall were the only hydration
misses in the drive log. A re-extraction from intact media is the real fix there.

### Verification
ctb2 drive (`/tmp/wsmoke/ctbdrive.sh`, RUST_LOG=info, GENERALS_CTB_PROBE removed
after diagnosis): menu → start_game skirmish USA → InGame. `/tmp/wsmoke/ctb_ctb2_
initial.png`: command grid renders 12 retail cameos (combat bike, rocket buggy,
gattling/dragon tank, overlord, avenger, sentry drone, paladin, tomahawk, angry mob,
jarmen kell, hacker) — ZERO red rectangles; top-right 3 squares + bottom-right 2
rects now the correct empty-slot look until bound. `/tmp/wsmoke/ctb_ctb2_selected.png`:
post-selection context shows empty slots (no red). `cargo check -p game-client-rust`
clean; probe sites byte-restored (git diff empty on impl_execute.rs,
game_window_global.rs, image.rs, observer.rs, impl_science.rs, lib.rs); temp probe
bin deleted. Changed kept files: the 3 GameClient files above. No git writes; serial
windowed runs with caffeinate -dis; no formatters.

### Residual (other lanes)
- ObserverPlayerInfoWindow static texts ("Units"/"Buildings"/...) overlap the in-play
  command grid — observer-info visibility in normal play is a separate context bug.
- SA/SN/SUControlBar512 atlases missing from retail assets (see above) — power
  dots/tray art needs intact media, not code.
- Minimap darkness + unit-mesh white: MinimapShroud / UnitTexBind lanes.

---

## GiantTextFix — giant blurry white caption over the in-match control bar CLOSED: `ControlBar.wnd:Munkee` (W3DNoDraw window with dormant `IMAGE: InGameUIChinaBase`) was being painted by a Rust-only "compat default draw" fallback; `w3d_no_draw` is now a true empty draw per C++ (2026-09-03 11:20-13:30)

### 1. Reproduce
Fresh control drives (`/tmp/wsmoke/gttext.sh`, `gtprobe.sh`, `gtmenu.sh`, `gtload.sh`;
captures `/tmp/wsmoke/gtprobe_gtfix1_ingame.png` post-fix vs `disc_control_gt1_dozer.png`,
`gtprobe_p2_ingame.png`, `gtmenu_menu1_ingame.png` pre-fix) reproduce a giant blurry
white glyph band (~640x50 px at 640x480, glyph height ~40-48 px) spanning the full
screen width at y≈345-395, drawn UNDER the control bar art (occluded by the $SS panel,
visible through transparent bar regions). Present in every in-match capture; absent in
Menu. White-pixel count in the band region (x 140-500, y 330-400): **3496 pre-fix**.

### 2. Identification (env-gated probes, removed after diagnosis)
`GENERALS_TEXT_PROBE=1` first hooked `UIRenderer::draw_text` (GameClient
`gui/ui_renderer.rs`): every unique text draw is small and correctly positioned
("$$$", "Units", superweapon timers, menu buttons) — the giant band is NOT text
rendering, so the font point-size/pixel-size/DPI theories were all ruled out.
Second probe at `UIRenderer::render()` (BIGQUAD: textured commands >200x60 px,
matched against the text-texture cache) caught the culprit quads, drawn every
frame at z=0 Alpha with mapped-image ATLAS UVs:
  - rect (0,331)-(639,479) uv (0,0.277)-(0.780,0.996)
  - rect (0,358)-(639,506) uv (0,0.277)-(0.780,0.996)
`text_cache_hit=false` → not a text texture: a **mapped-image atlas sub-image**
stretched over the control-bar area. The blurry white "caption" is the atlas art's
own embedded title glyphs (the garbled "…OI MISSION U…" reading), NOT any string.

### 3. Root cause
The rects decode to authored WND geometry: (0,331)/0.8 = 414 — exactly
`ControlBar.wnd:Munkee` (USER window, `SCREENRECT 0,414-799,599`, STATUS
`ENABLED+IMAGE`, `ENABLEDDRAWDATA = IMAGE: InGameUIChinaBase`, `DRAWCALLBACK =
"W3DNoDraw"`, windows_game/extracted_big_files/WindowZH/Window/ControlBar.wnd:55-96).
C++ never paints it: `W3DNoDraw` has an EMPTY body (the `W3DGameWinDefaultDraw`
call is commented out) — GeneralsMD
`GameEngineDevice/Source/W3DDevice/GameClient/GUI/GUICallbacks/W3DControlBar.cpp:661-667`.
The Rust port instead had a compat fallback: `w3d_no_draw` called
`default_draw_callback` (the Rust-only generic image draw) whenever the window
"has compat default content" (IMAGE status / draw-data entries) — painting the
dormant InGameUIChinaBase sheet through the mapped-image atlas at the window rect
(scaled 800x600 → 640x480).

### 4. Fix (C++ parity)
`Code/GameEngine/GameClient/src/gui/w3d_gadget_draw/main_menu.rs`:
`w3d_no_draw` is now an empty no-op with the W3DControlBar.cpp:661-667 citation;
the dead `has_compat_default_content` / `draw_data_has_compat_default_content` /
`w3d_compat_default_draw` helpers are deleted (clean cutover — no other callers).
This also unpaints the other W3DNoDraw containers (ControlBar.wnd lines
158/205/252/299/345/591/1297/1441/2067/2883, GenPowersShortcutBar*.wnd:18,
Diplomacy.wnd, load screens), matching retail, where only their CHILDREN draw.
Probes in `gui/ui_renderer.rs` fully removed (net diff vs HEAD: zero).

### 5. Verification
- Post-fix control drive (13:19 capture, release build 13:10 with this fix):
  giant band GONE. Band white-pixel count 3496 → **189** (remainder is the
  legitimate small "$$$" money label and cameo highlights). Control bar, observer
  stat labels, superweapon timers and money display all render at correct sizes.
- Menu regression: menu-state frame is **pixel-identical** (0 differing pixels)
  to the pre-fix menu capture — shell art never depended on the compat path.
- CtrlBarTexFix's ctb2 drive on the same binary independently confirms the band
  is gone with the 12-cameo command grid rendering.
- `cargo check -p game-client-rust --lib` clean. `game-client-rust` GUI tests
  could not run standalone this session: a sibling lane (UnitTexBind) has
  `UTBVIEW_ENABLED` mid-landing in ww3d-renderer-3d `render_manager.rs`
  (free static + `get_or_init` on `LazyLock`) which breaks that crate's dev/test
  build; owned by that lane — flag for Main's final validation.
- GameLogic-side guards (combat filter 966/0, world_tests catalog,
  gameworld_shadow 302/302) are untouched by this GUI-only change; no GameLogic
  files were modified.

Probe artifacts (outside repo): /tmp/wsmoke/gttext.sh, gtprobe.sh, gtmenu.sh,
gtload.sh, logs gtprobe_p{1,2,3}_stderr.log, captures gt*_*.png.

## UnitLightFix — UTBLIGHT verdict: mesh-lane lighting scale ELIMINATED (0-1, correct); UTBMAGENTA discriminator exonerates the ENTIRE mesh forward pass; white blobs are painted by a non-mesh overlay/terrain lane — tagged discriminator staged (2026-09-03 16:00-17:45)

Method: three windowed 640x480 skirmish drives (utb4..utb6, /tmp/wsmoke/utb_utb{4,5,6}_stderr.log + utb_utb{4,5,6}_dozer.png, script utbdrive4.sh) against a release binary carrying env-gated, warn-level probes (all OFF unless the env var is set; no git writes; no formatters).

### 1. UTBLIGHT (assignment probe) — lighting-scale hypothesis DEAD
`draw_material_pass` (render_manager.rs) once-per-model dump, in-match:
`UTBLIGHT model='' ambient=(0.2196,0.2039,0.1725) | l0 type=Directional dir=(-0.809,0.379,-0.449) col=(1.0000,1.0000,1.0000) int=1.000 amb=(0,0,0) enabled=true`.
Ambient is authored 0-1 map metadata (Defcon6 morning row); sun is one white directional at intensity 1.0. With ambient<=0.22 + one N.L-scaled white sun, the opaque.wgsl clamp CANNOT saturate brown albedo (avconstdoz texelMid=(49,40,33)) to flat 255. C++ parity note: the map chunk parser (map_settings.rs) reads raw f32s exactly like WorldHeightMap.cpp ParseLightingDataChunk (772-820); values are healthy. light_from_map_channels needs NO normalization.

### 2. UTBVIEW + UTBMAT — upload path also healthy at bind time
- UTBVIEW (stage_resources_for branch logger): every unit/UI texture resolves `branch=uploaded-cpu` with real mid-texels (Avbattlesh (117,115,117), AVChinook (16,32,8), Coplight (255,207,140)...). The white-fallback tail NEVER binds. Stage 1 `no-texture` is the expected empty-slot case.
- UTBMAT (per-model material dump): `mat_diff=(1,1,1,1) mat_spec=(1,1,1,0.01) mat_emis=(0,0,0) overrides=(1,1,1,0) stage_mask=01 cube_mask=0 hints=0` — no material/emissive/override white-out.

### 3. UTBMAGENTA — the white blobs are NOT mesh-forward pixels
GENERALS_UTBMAGENTA=1 overrides `material_diffuse=[1,0,1,1]` in `WgpuMaterialBinds::model` (the single ModelUniform builder; decals included via render_decal_queue->render_mesh). Drive utb6: **0 strong-magenta pixels** (loose re-scan: only 75 icon pixels in the HUD, none blob-shaped) while the white blobs persist. Conclusion: the flat-white unit-shaped blobs are painted by a pass that does NOT go through WgpuMaterialBinds::model — i.e. the terrain-pass overlay family (road/scorch stripes ARE visibly drawing per capture read), the shadow/occlusion post passes, or water. Blob geometry (50x17, 41x35, 25x17 px; exact 255,255,255; hard edges; positions shift with camera) fits static-world stripes or billboard overlays seen at slightly different camera poses, NOT shaded mesh fragments.

### 4. Staged next discriminator (probes landed + compile-clean; NOT driven — UiGapInventory holds the exclusive windowed floor 30-45 min, so the tagged launch was forfeited per Main's hold. The tagged binary is one `cargo build --release --bin generals` away once the floor frees; an intermediate build failure from my own occlusion_bridge.rs descriptor edit was fixed and `cargo check -p generals_main --lib` is clean)
- `GENERALS_UTBOVERLAYTAG=1` -> occlusion_bridge.rs enqueue_occluded_player_color_pass paints every overlay billboard pure GREEN.
- `GENERALS_UTBSHADOWTAG=1` -> shadow_pass.rs record_shadow_and_occlusion_passes paints every shadow-pass overlay billboard pure BLUE.
- One tagged drive splits the remaining space: green blobs => occlusion overlay (check team_color defaults/blend); blue => shadow overlay; unchanged white => terrain road/scorch/water lane (read road.wgsl + overlay_gpu.rs next; water pass "main water pass" at pipeline_collect.rs:1815).
- Also available: `GENERALS_UTBUVCENTER=1` (frommodel pins UVs to 0.5) if a later session re-opens the UV path.

### Probe inventory / hygiene (for the closer)
ALL probes are env-gated (inert without their env var), warn-level, and compile clean: render_manager.rs (UTBVIEW_ENABLED/SEEN, UTBMAT_ENABLED/SEEN statics + UTBLIGHT block + UTBUVCENTER in frommodel), wgpu_material_binds.rs (UTBMAGENTA), occlusion_bridge.rs (UTBOVERLAYTAG), shadow_pass.rs (UTBSHADOWTAG). Guard tests (frozen_fow_* render_manager string checks, ww3d-renderer-3d tests.rs draw_material_pass contract) keep their required strings. `cargo check -p ww3d-renderer-3d --lib` and `-p game-client-rust --lib` both clean post-edit. Removal: delete the statics + each self-contained `if utb*` block (marked "documented diagnostic"). ZPROBE precedent (pipeline_execute.rs) keeps a documented env-gated probe in-tree.

### Screen truth at hand-off
utb4/utb5/utb6 captures: terrain tan (110,100,78) correct, HUD correct, units still FLAT exact-white silhouettes with hard edges — blocker UNRESOLVED, mesh lane exonerated, remaining space narrowed to {terrain road/scorch/water overlays, occlusion overlay, shadow overlay} with a one-drive discriminator staged.

Probe artifacts (outside repo): /tmp/wsmoke/utbdrive4.sh, utb_utb{4,5,6}_stderr.log, utb_utb{4,5,6}_dozer.png.

---

## UiGapInventory — per-state visual gap inventory vs C++ retail (2026-09-03 14:00-15:10)

Method: serial windowed 640x480 drives (gapdrive15.sh/.15b/.15c/.15d/.15e in
/tmp/wsmoke/, logs + captures under /tmp/wsmoke/gap15/) through Menu → SP flyout →
Skirmish options → InGame → selection/order → ESC pause menu → pause → diplomacy →
options → shell pushes. Every capture compared against the retail WND definitions
(windows_game/extracted_big_files/WindowZH/Window/{ControlBar.wnd,Diplomacy.wnd,
Menus/{MainMenu,SkirmishGameOptionsMenu,OptionsMenu,QuitMenu,SinglePlayerMenu}.wnd})
and GeneralsMD GameClient GUI sources (MainMenu.cpp, W3DMainMenu.cpp, W3DControlBar.cpp,
InGameUI/W3DInGameUI.cpp). Inventory only — no fixes. All coordinates below are
retail 800x600 WND coords with the x0.8 640x480 mapping noted.

### Binary attribution + environment findings (read first)
1. TWO binaries were exercised. `target/release/generals_snap` (Sep 2 23:50) RENDERS and
   carried the full ladder — it PREDATES GiantTextFix (13:10) and CtrlBarTexFix (13:30), so
   red-grid/giant-caption findings below are baseline pre-fix state, re-verify on HEAD.
   `target/release/generals` (Sep 3 13:46) is PRESENTATION-BROKEN (P1, next section).
2. OS synthetic clicks are BLOCKED environment-wide since ~14:15 (clicker8 reports
   `activated=false tries=4` even with the app frontmost via System Events; drive13's
   09:41 run worked). All physical-click drive plans must convert to control-command
   driving: `winit_click_named` (shell nav — works, feeds the real gadget-click path),
   `select_local_unit`/`select_all` + `move|x=|y=` (in-match), `toggle_quit_menu`,
   `toggle_pause`, `toggle_diplomacy`, `open_options`, `start_game`.
3. Gating trap for drive authors: status `gameplay=` is the interactive-evidence latch
   (`interactive_playability.gameplay_complete()`), NOT "match is live" — control-driven
   select/move does NOT latch it. Gate InGame captures on
   `state=InGame && startup_phase="Map load complete" && live_frame_ok=true`.
4. The 13:46 binary never logs the unattended reveal line; the 23:50 binary does.

### P1 handoff (Main's bisect): 13:46 `generals` black-menu + match-start drain
Evidence: /tmp/wsmoke/gap15/child5_stderr_full.log (30 MB; 8+ `flush_ui_to_frame:
ControlBarParent missing on live WindowManager` with a searched-path list that INCLUDES
the existing `../windows_game/extracted_big_files/WindowZH/Window/ControlBar.wnd` — this
is not file-existence, the WM never imported the layout), no
`unattended runtime-host run — revealing main menu` line (present in the 23:50 binary's
child1_stderr_tail.log), `winit_click_named_miss`/`winit_menu_nav_miss:no_menu_match_gadget`
despite `state=Menu fps=30 live_frame_ok=true`, menu frames pure black
(g15d_menu*.png), and `start_game` drained: `Menu NewGame drain: ignore GAME_SHELL
(shell map already live)` (camera_drain.rs). Open_skirmish_menu / open_options /
open_single_player_menu shell pushes also render black.
Data points for the bisect: extracted_big_files_v2 exists but contains ONLY INIZH
(0 .wnd files vs v1's 80) — any import-priority/root change toward v2 kills every shell
WND at once (InGameRenderTriage reports the wnd_parse v2-then-v1 resolver itself safe).
Current working tree has NO modified files under layout_load/wnd_parse/windows_game
(76 modified files, none matching) — the 13:46 churn is not recoverable from git (no
intermediate commits); bisect targets: main_menu.rs reveal gate
(`state.not_shown && !state.just_entered`), camera_drain.rs GAME_SHELL drain condition,
asset-root resolution feeding ui_render_pass's searched list. Repro: drive15d.sh
(GENERALS_RUNTIME_HOST_WND=1) against any 13:46+ build; watch for the missing reveal log.

### State: Main Menu (g15e_menu.png == g15_menu.png)
Correct vs retail: logo wordmark art; button stack SOLO PLAY/MULTIPLAYER/LOAD/OPTIONS/
CREDITS/EXIT GAME (retail GUI:SinglePlayer/Multiplayer/MainMenuLoadReplay/Options/
Credits/Exit via CSF); blue-rule button art; grid backdrop frame.
Gaps:
- GAP MainMenu-1 (major): shell map 3D background absent — retail fills the menu
  interior with the animated shell-map battle (GameClient shell game; the port renders
  solid black interior). Lane: GameClient shell-map presentation.
- GAP MainMenu-2 (minor): `GreenDot` + `Clock` widgets absent (retail 34,517/53,503 →
  27,414/42,402 bottom-left online dot + clock).
- GAP MainMenu-3 (minor): `WinFactionTrainingSmall/Medium` posters absent (496,423 /
  472,399 → 397,338/378,319).
- GAP MainMenu-4 (minor): `ButtonUSARecentSave` ("Recent Save", 440,104 → 352,83) never
  shown; retail shows it when a USA save exists.
- MainMenu-5 (low confidence): backdrop may be a simplified stand-in for the full
  retail menu art; needs art-side review.

### State: SP flyout (g15_sp_flyout.png; named-click path)
Correct vs retail: USA/GLA/CHINA/CHALLENGE/SKIRMISH/BACK — exactly DROPDOWN_SINGLE
(MainMenu.cpp:1320-1327 opens MapBorder); main button stack correctly hidden while open.
Gaps:
- GAP SPFlyout-1: EarthMap dropdown panel art missing — retail panel carries map imagery
  (`EarthMap` windows); port draws a flat black panel with blue rules.
- GAP SPFlyout-2 (text): "CHALLENGE" vs retail "Generals Challenge" (GUI:Generals_Challenge)
  — truncated label.
- GAP SPFlyout-3 (conditional, drive1 g15_sp_dropdown.png): the PHYSICAL-click +
  `tick_main_menu_transitions` path instead rendered the DifficultySelect flyout
  (SELECT DIFFICULTY/EASY/MEDIUM/HARD/BACK — MapBorder4, correct rect x0.8) — in retail ZH
  that flyout is only wired under `#ifdef _CAMPEA_DEMO` (MainMenu.cpp:1420-1433); retail
  never shows it. ButtonSkirmish stayed published-hittable under the overlay, so both
  flyouts were simultaneously open/hittable — z-order/visibility bug on the
  physical+ticks path only. P2.

### State: Skirmish options (g15_skirmish_menu.png)
Correct vs retail: title + Players/Color/Army/Team/Map Preview headers (CSF-resolved);
8 slot rows x 4 combos; battle-honors block (medal icons + NUMBER:0 values);
PLAY GAME (GUI:Start) / MAIN MENU (GUI:GotoMainMenu); Game Speed slider + value;
Limit Superweapons checkbox; 2 start-position markers; `ButtonReset` correctly HIDDEN
(retail authored ENABLED+HIDDEN — not a gap).
Gaps:
- GAP Skirmish-1 (P1-class, RedRectsTriage lane): `ComboBoxStartingCash` renders as a
  RED placeholder box (authored COLOR 255 0 0 255 painted instead of the combo art) —
  the shell-side instance of the same IMAGE-status draw-func-selection bug class
  CtrlBarTexFix fixed for the ControlBar; the combo/dropdown gadget family still falls
  back to solid fill.
- GAP Skirmish-2 (same class): `SliderGameSpeed` thumb renders as a small red square
  (thumb image not bound).
- GAP Skirmish-3: map preview image absent — MapWindow shows only 2 unnumbered start
  dots on empty ground; retail draws the selected map's preview image and numbered
  start spots.
- GAP Skirmish-4 (wrong-text): `TextEntryMapDisplay` shows the authored literal
  "Static Text" instead of the selected map's display name (retail sets it at runtime
  from the map cache — SkirmishGameOptionsMenu.cpp).
- GAP Skirmish-5 (wrong-text, cosmetic): `TextEntryPlayerName` shows authored literal
  "Entry" instead of the player name (same runtime-substitution class).
- GAP Skirmish-6: `ListboxInfo` (303,380 → 242,304) absent — retail info/chat listbox.

### State: InGame initial (g15_ingame_initial_raw.png; pre-GiantTextFix/CtrlBarTexFix binary)
Correct vs retail: terrain + camera; control bar frame; MoneyDisplay at retail position
(360,437 → 288,350); 12-slot command grid at retail rects; ProductionQueueWindow empty
slot grid (621,483 → 497,386); observer stat texts (known overlap residual).
Gaps:
- TRACKED (excluded per brief): white unit meshes (UnitLightFix — HEAD 3f30f3ae6 narrows
  to depth lane); red command-grid buttons + cameo reds (CtrlBarTexFix landed;
  RedRectsTriage residuals: top-right x3, grid-left tabs, bottom-right x2 — all three
  sets visible in this capture, positions match their list); giant Munkee caption band
  above the bar (GiantTextFix landed — band clearly present here, pre-fix binary).
- GAP InGame-1: minimap/radar absent in this binary (MinimapShroud lane reports it fixed
  on HEAD — re-verify on next HEAD drive).
- GAP InGame-2 (wrong-text): MoneyDisplay shows "$SS"-class glyphs; the credit AMOUNT is
  never rendered (retail: "$" + live amount). Needs HEAD-binary recheck — may be the
  accepted "$$$" remainder GiantTextFix measured, but the missing amount is real.
- GAP InGame-3: PowerWindow (261,473 → 209,378, thin bottom strip) — retail power-bar
  dots can never hydrate (SA/SUControlBar512*.tga absent from retail BIGs, per
  CtrlBarTexFix finding); renders slate/empty. Asset-side: needs intact media.
- GAP InGame-4: left-edge button column `ButtonOptions`/`ButtonIdleWorker`/
  `ButtonPlaceBeacon` (192,495/519/543 → 154,396-452) not visibly rendered (white flecks
  only) — overlaps RedRectsTriage's grid-left set; confirm on HEAD.
- GAP InGame-5: `GeneralsExp` exp-bar (769,503 → 615,402) absent.
- GAP InGame-6: `WinGeneralPortrait` shows an empty trapezoid frame; portrait art flaky
  (hydrates in some captures, empty in others — context-dependent bind).
- GAP InGame-7 (wrong-render, minor): literal "HUD" glyphs bottom-left corner (0,461) —
  LeftHUD art region showing text, not art.
- GAP InGame-8 (asset): all 5 skybox faces TSMorning{E,S,W,T,...}.tga fail to load
  (stderr WARN terrain_visual) — sky missing in-match. Asset-extraction class.

### State: InGame selection (g15_ingame_selected_raw.png; select_local_unit, selected_count=1)
- GAP Select-1 (major): NO selection visual — no ring/brackets in player color, no
  health pips over the selected unit. Retail InGameUI draws selection markers + health.
- GAP Select-2 (major): command grid does NOT swap context with selection (retail: dozer
  → build tab, rangers → unit command set; port grid static regardless of selection).
- GAP Select-3: no selected-unit portrait/name/health block in LeftHUD (retail
  `WinUnitSelected` + portrait + name strings).

### State: InGame move order (g15_ingame_order_raw.png; move via control cmd)
- GAP Order-1: selection ring renders but WHITE (retail: local-player color — green for
  P1 skirmish). Wrong color constant / missing player-color bind in the ring renderer.
- Order-2 (unverified): retail move marker + line not visible in the capture (~3s
  post-order; ~4s frame pacing may have consumed the flash). Needs HEAD-binary recheck
  before counting as a gap.
- Confirmed hydrating here: MoneyDisplay ornate frame art; `WinGeneralPortrait` emblem
  (context-dependent, see InGame-6); first grid slot shows a greyed bound cameo.

### State: ESC pause menu (g15_pause_quitmenu_raw.png; toggle_quit_menu → ok_wnd)
- GAP PauseMenu-1 (P1-class): NOTHING renders. Retail QuitMenu.wnd = dimmed game + panel
  at 252,100 with ButtonSaveLoad/ButtonOptions/ButtonRestart/ButtonExit/ButtonReturn
  (288,183/231/279/327/375 → x230, y146/185/223/262/300) + WinLoad frame. The WND toggle
  reports ok but the layout never paints in the windowed host path.

### State: pause screen (g15_pause_screen_raw.png; toggle_pause, status paused=true)
- GAP Pause-1 (P1-class): NOTHING renders — no dimmed background, no pause panel
  (ui_manager.rs has PauseMenu render code; the windowed host never runs it). Same
  overlay-paint root cause as PauseMenu-1.

### State: diplomacy (g15_diplomacy_raw.png; toggle_diplomacy)
- GAP Diplomacy-1 (P1-class): NOTHING renders. Retail Diplomacy.wnd in-game page:
  Player/Team/Side/Status table (StaticText{Player,Team,Side,Status}0-7 + titles),
  ButtonMute/UnMute0-7, TextEntryChat + chat listbox, ButtonHide. Same root cause.

### State: options in-game (g15_options_ingame_raw.png; open_options from InGame)
- GAP Options-1 (P1-class): NOTHING renders — status flips Paused+Screen::Options but
  the live game stays on screen. Retail OptionsMenu.wnd overlay: Accept/Back/Defaults,
  CheckAlternateMouse etc., volume/gamma/scroll sliders, resolution/detail/AO combos,
  LabelVersion. Same overlay-paint root cause.

### State: shell pushes from Menu (drive6: g15e_options_shell/sp_menu/load_screen/
quit_dialog.png)
- GAP ShellPush-1 (P1-class): `open_options`, `open_single_player_menu`,
  `open_load_game`, `toggle_quit_menu` from the main menu do NOT navigate — all four
  captures are BYTE-IDENTICAL to the preceding menu frame (cmp verified). The
  `enter_shell_screen_from_runtime_host` push path reports ok but never brings the
  layout forward/paints. Contrast: the gadget-click path (ButtonSkirmish →
  SkirmishGameOptionsMenu) DOES render — so layout push+paint works via
  WND gadget navigation but not via the runtime-host enter_shell_screen API.

### Not gaps (verified correct)
- SP flyout contents/order (named path); skirmish menu widget set/text (incl. hidden
  ButtonReset); main-menu button labels; MoneyDisplay position; ProductionQueueWindow
  empty layout; observer-stat text content (placement overlap is the tracked residual).

### Capture / script index (all under /tmp/wsmoke/gap15/)
- Scripts: gapdrive15.sh (drive1, physical+ticks), gapdrive15b.sh (drive2, aborted),
  gapdrive15c.sh (drive3, named-click chain), gapdrive15d.sh (drive5, 13:46-binary
  probe), gapdrive15e.sh (drive6, shell pushes). Logs drive{1,2,3,5,6}.log.
- 23:50-binary ladder: g15_menu.png, g15_sp_dropdown.png (difficulty flyout, drive1),
  g15_sp_flyout.png (correct flyout, drive3), g15_skirmish_menu.png,
  g15_ingame_initial_raw.png, g15_ingame_selected_raw.png, g15_ingame_order_raw.png,
  g15_pause_quitmenu_raw.png, g15_pause_screen_raw.png, g15_diplomacy_raw.png,
  g15_options_ingame_raw.png.
- Shell pushes: g15e_{menu,options_shell,sp_menu,load_screen,quit_dialog}.png.
- 13:46-binary probe: g15d_menu{,2,3}.png (black), g15d_skirmish_direct.png (black),
  child5_stderr_full.log (ControlBarParent spam + GAME_SHELL drain).
- Stderr: child1_stderr_tail.log (23:50, has reveal line), child5_stderr_full.log.

## SimSystemsFix — in-match gaps 2/3 fixed at the fixture, 4 diagnosed to a drain-path suspect (2026-09-03)

Scope: the three InMatchSimHunt documented gaps (QueueUpgrade authorship, dozer Gather
engage, kill-XP live path) in `golden_skirmish.rs` / `host_upgrades.rs`. Serial cargo,
no git writes, no formatters. Diagnostic probe (world_tests/simsys_probe_temp.rs) was
added, never ran — the shared lib was red the whole window on a sibling's shroud-store
migration (E0425/E0716 in host_authority.rs, object_queries.rs, supply_and_superweapons.rs,
skirmish_config.rs; NOT this lane) — and was removed per probe hygiene. Edits below were
written against verified APIs (`set_command_set_override`, `set_stored_supplies`,
`is_resource_collector`, `first_present_template`) but are NOT yet compile- or
gate-verified; first green tree must re-drive `golden_skirmish_gate` end to end.

### Finding 2 fix — QueueUpgrade producer authorship (golden_skirmish.rs, host_upgrades.rs)

Root cause confirmed in code: `execute_queue_upgrade` refuses via
`object_can_produce_upgrade` (C++ ProductionUpdate::queueUpgrade "STOP cheaters",
ProductionUpdate.cpp:250-272). That walk is `command_set_has_upgrade_button`
(bridge → INI → None headless) falling back to `residual_command_set_allows_upgrade`,
whose producer table matches TEMPLATE NAME only — "GoldenSupplyCenter" never matched.

Fix (two pieces):
- `golden_skirmish.rs`: new `stamp_supply_center_producer_set` — after creating the
  golden supply center (synthetic + map-fallback paths) the object receives the retail
  CommandSet identity `AmericaSupplyCenterCommandSet` via `set_command_set_override`
  (Main host model has no template-level command-set field; the override is the
  C++ `Object::m_commandSetStringOverride` channel). Retail oracle:
  FactionBuilding.ini AmericaSupplyCenter authors `CommandSet =
  AmericaSupplyCenterCommandSet` + `Behavior = ProductionUpdate`; CommandSet.ini:785-789
  slot 13 = `Command_UpgradeAmericaSupplyLines`; CommandButton.ini:1080-1086 carries
  `Upgrade = Upgrade_AmericaSupplyLines`; C++ Object::canProduceUpgrade walks exactly
  this set via getCommandSetString() (Object.cpp:6093-6106).
- `host_upgrades.rs`: `residual_command_set_allows_upgrade` now also honors an authored
  CommandSet identity that names one of the retail producers of THAT upgrade
  (`authored_command_set_names_producer`: `<Producer>` / `CommandSet_<Producer>` /
  `<Producer>CommandSet`, case-insensitive, reads `object.command_set_override`).
  Fail-open/closed behavior of existing tests preserved (template-name match unchanged;
  a set naming a non-producer of the upgrade still refuses).
- Claim honesty: synthetic `upgraded` now requires `CommandResult::Success` AND the
  PRODUCTION_UPGRADE entry live on the producer's `building_data.production_queue` AND
  `has_unlocked_upgrade` within 1200 frames (research lives only on the producer queue,
  C++ ProductionUpdate.cpp:636-648/1109-1112; SupplyLines BuildTime 30s = 900 frames).
  A bare player-side queue entry no longer counts.

### Finding 3 fix — Gather now fields a harvester, and the gate is cash-backed

Root cause confirmed: `execute_gather` gates collectors on `is_resource_collector()`
(= KINDOF_HARVESTER, C++ parity) and `GoldenDozer`/`USA_Dozer` author no Harvester
kindof — the executor was RIGHT, the fixture was wrong. Retail: AmericaVehicleChinook
authors `KindOf = ... VEHICLE TRANSPORT AIRCRAFT HARVESTER` + `ChinookAIUpdate`
(AmericaAir.ini, Object block at line 2108), while AmericaVehicleDozer authors
`KindOf = ... DOZER` + `DozerAIUpdate` (AmericaVehicle.ini:1599 block) — USA dozers
never gather; the GLA worker combines both (`KindOf = ... DOZER HARVESTER` +
`WorkerAIUpdate`, GLAInfantry.ini:3749/3756).

Fix (golden_skirmish.rs):
- `install_templates` adds a `GoldenHarvester` fixture (Vehicle+Harvester+Selectable).
- Synthetic path creates `AmericaVehicleChinook` (catalog template from
  `ensure_ai_faction_templates`) with GoldenHarvester fallback, and issues Gather to
  that collector; the dozer stays construction-only.
- Map path: new `ensure_harvester` (prefer live harvester → AmericaVehicleChinook →
  USA_Chinook → ChinaVehicleSupplyTruck → GoldenHarvester) routes the Gather commands;
  `retail_gather_ok` honesty check now reads the harvester's target.
- Piles are stamped `set_stored_supplies(2000)` (retail SupplyPile stocks via
  SupplyWarehouseDockUpdate `StartingBoxes = 150`, CivilianBuilding.ini SupplyPile
  block; ValuePerSupplyBox 75, GameData.ini) so gather depletes authored stock.
- Gate honesty: `gathered` now requires AIState::Gathering engagement AND player-0
  cash to strictly increase within 1200 frames (gather→carry→SupplyCenter deposit
  crediting Money). The old ai_state-only read is what masked the no-op.

### Finding 4 — kill-XP: award sites verified; suspect narrowed to the damage-authority destroy drain

Audited live award path against the C++ oracle: C++ credits at damage-death time —
ActiveBody.cpp doDamagePoint `if (m_currentHealth <= 0 && m_prevHealth > 0)
{ if (damager) damager->scoreTheKill(obj); obj->onDie(...); }` (ActiveBody.cpp:641-650),
Object::scoreTheKill body at Object.cpp:2890-2945 (playable-side + IGNORED_IN_GUI +
ENEMIES + under-construction gates, then addSkillPointsForKill + addExperiencePoints).

The Rust direct-fire kill branch DOES award: world_tick/combat.rs:2182-2205
(`take_damage_from_typed_death` → destroyed → `mark_object_for_destruction` +
`continue_or_stop_after_kill` → `award_score_the_kill_experience`); the auto-acquire
path awards too (world_objects/object_ai_combat.rs:~255-268). Victim-side gates
(`score_the_kill_victim_counts`, `kill_experience_value` →
`experience_value_for_level`, `is_accepting_experience_points`) all pass for a
trainable shooter vs an 80-XP enemy.

Remaining suspect (unverified — probe blocked): the Wave-621 damage-authority drain.
Under `ensure_gate_damage_authority()` the GameWorld health writeback records lethal
IDs (`writeback_health_to_host` → `host_destroy_ready_log::record`) and
`process_destroy_list` marks them with `mark_object_for_destruction(ev.object, None)`
(destroy_list_bounty.rs:189-202) — killer None and NO `award_score_the_kill_experience`
call. Any kill resolved through that drain instead of the direct-fire branch silently
drops scoreTheKill; under the same authority `gain_experience` also only logs XP
(`host_experience_log::record`) instead of mutating host veterancy
(object/update.rs gain_experience tail). Next agent: drive the probe recipe
(trainable ranger 25dmg/1.0s vs 120HP/80XP enemy over `logic.update()` with
DAMAGE_AUTHORITY on) and, if the drain is the drop, credit `last_damage_source` at the
drain site (C++ parity: the award belongs at damage-death, and the victim's
`last_damage_source` is stamped by `stamp_last_damage_cpp` in the same apply).

### Live verification (gate binary rebuilt post green tree)

- `golden_skirmish_gate --frames 30` (Lone Eagle map path), fresh build with these
  edits: **move=true gather=true build=false produce=false upgrade=true**
  retail_gather=true status=partial. Finding 2 and Finding 3 verified live:
  QueueUpgrade research now completes on the authored supply-center producer, and
  Gather engages a real KINDOF_HARVESTER collector on a retail/map target.
  Baseline before this window (stale pre-fix binary, same map): gather=false
  build=false produce=false upgrade=false fight=false.
- Synthetic path (`golden_skirmish_synthetic_when_map_absent`): upgrade=true (F2
  live in the soup too), but gather=false even with a ground harvester and
  build=false. `git diff HEAD` confirms my golden_skirmish.rs edits touch only
  gather/upgrade/stamp code (nothing in the construct/barracks flow; leftover.rs
  and support_states/update.rs have no diff at all), so synthetic build=false
  predates this window. Open items for the next pass, documented honestly:
  1. Synthetic soup gather: the collector engages Gathering but the
     cash-backed gate proves it never completes gather→carry→deposit there
     (air Chinook AND ground GoldenHarvester both) — needs a soup-world
     movement/locomotor look at the collector (InteractRange arrival).
  2. Map-path build/produce/fight=false is pre-existing (identical in the stale
     baseline binary) and NOT touched by this diff — dozer DozerConstruct on
     Lone Eagle is the next hunt target (system 5).
- combat filter 966/0, world_tests catalog, combat::tests 33/33 were green before
  this window; not re-run here (serial-cargo contention, other lanes mid-landing).

---

## RedRectsTriage — remaining control-bar red rects root-caused + C++ ControlBarScheme::init parity LANDED (2026-09-03 14:00-16:00)

### Red-rect audit (offline; no red can leak from the current tree)
- Retail ControlBar.wnd (98 windows parsed): every red-placeholder button authors
  `DRAWCALLBACK=[None]` → default draw selection; UnitUpgrade1-5/CameoWindow author
  `W3DGadgetPushButtonImageDraw` (mapped in script_callbacks.rs to the fixed
  no-fallback image draw); all authored callback names resolve; unknown → the fixed
  `default_draw_callback` (image-status + no image → no ops). Combined with the landed
  CtrlBarTexFix selector, NO ControlBar.wnd window can reach the solid draw with the
  authored `255 0 0 255` anymore. Pure-red pixel scan of ctb2/utb6/gtmenu captures: 0 red
  rects (4 red px = cameo art itself). UiGap's shell-menu red boxes (ComboBoxStartingCash,
  SliderGameSpeed) are from the Sep-2 23:50 pre-fix binary; `w3d_gadget_combo_box_image_draw`
  and `w3d_gadget_vertical_slider_image_draw` already draw nothing when unbound (C++
  W3DGadgetComboBox/SliderImageDraw parity), so the current tree renders them honestly
  invisible.

### The real gap: ControlBarScheme art is never parsed/applied (C++ ControlBarScheme.cpp:401-662)
Retail binds the fixed-HUD button art at scheme init via GadgetButtonSet*Image:
- ButtonOptions/IdleWorker/PlaceBeacon/PopupCommunicator/General ← Options*/Worker*/Beacon*/
  Buddy*/General* Enable+Hightlited+Pushed+Disabled images AND slot rects (OptionsUL/LR,
  WorkerUL/LR, ChatUL/LR, BeaconUL/LR, GeneralUL/LR) with resMultiplier scaling
  (C++ 417-447, 481-529, 449-475, 421-448, 576-601).
- ExpBarForeground ← ExpBarForegroundImage (476-480); WinUAttack ← UAttack* + disabled=
  highlight (629-651); ButtonQueue01-09 disabled slot ← QueueButtonImage
  (updateBuildQueueDisabledImages, ControlBar.cpp:2832-2870); ButtonLarge ← MinMax rect +
  ToggleButtonUp/Down* per stage (603-627, ControlBar.cpp:3128-3146); beacon hidden unless
  LAN/Internet (ControlBar.cpp:2756-2761 — explains retail showing only 3 of the 4 left-stack
  buttons in skirmish). Port had NONE of this (placeholder scheme manager; setControlCommand
  only binds when the command has a ButtonImage, ControlBar.cpp:2442-2443, so scheme art is
  what these buttons show in retail).

### Fix (2 files + guards)
- `Common/src/common/ini/ini_control_bar_scheme.rs`: parsed ALL C++ field-table button-art
  tokens (OptionsButton*/IdleWorkerButton*/BuddyButton*/BeaconButton*/GeneralButton*/
  UAttackButton*/MinMaxButton*/ToggleButtonUp|Down*/GenBarButton*/QueueButtonImage/
  RightHUDImage/ExpBarForegroundImage/GenArrow/CommandMarkerImage/PowerPurchaseImage) and
  the UL/LR slot rects; added `set_active_scheme_for_side` (C++ resolves the player
  template's ControlBarScheme name; port templates lack the field, so exact "<side>8x6"
  then side-prefix — the old `set_active_scheme(side)` could NEVER match "america8x6");
  guard tests: field-table token parity vs the C++ table + America8x6 parse smoke
  (NOTE: `cargo test -p game_engine --lib` collects 0 tests globally — pre-existing
  harness quirk, untouched; crate compiles clean in test mode).
- `GameClient/src/gui/control_bar/control_bar_impl/impl_science.rs`:
  `apply_scheme_button_art()` (C++ init parity) wired into
  `apply_control_bar_scheme_for_side`; binds enabled[0]/hilite[0]/hilite[1]/disabled[0]
  slots (GadgetPushButton.h inline parity), repositions/sizes the five buttons + WinUAttack
  + ButtonLarge per scheme rect x resMultiplier − parent origin, queue disabled slots,
  exp bar; scheme-driven `leftover_set_up_down_images` (setUpDownImages parity); beacon
  visibility parity in `apply_scheme_context_and_default_stage`. Fail-closed: unresolved
  mapped images leave slots unbound (invisible, never a color fill).

### Asset truth (unchanged, documented by CtrlBarTexFix + re-verified)
SAControlBar512_001.tga / SNControlBar512 / SUControlBar512 targas and SCBigButton are
ABSENT from the retail-direct-play extraction (entry tables + byte scans; find over both
roots confirms). So today the scheme buttons render invisibly-unbound — C++-correct for the
available assets — and will light up (options gear, worker, chat, general, queue slot art,
exp bar) the moment a re-extraction supplies the atlases; zero further code needed.

### Verification
`cargo check -p game-client-rust --lib` clean (warnings only) after all edits;
game_engine compiles clean in test mode. In-match capture drive staged
(/tmp/wsmoke/rrtdrive1.sh, control-command driven: winit_click_named + tick_main_menu_
transitions + start_game|mode=skirmish|faction=USA + select_local_unit, captures at
/tmp/wsmoke/rrt/); release build was serialized behind sibling builds at yield time —
run it and capture rrt_ingame_initial/selected to close the visual acceptance.

### Residual
- SA/SN/SUControlBar512 atlases + SCBigButton: asset re-extraction (blocks visible art).
- PlayerTemplate `ControlBarScheme` field unparsed (port resolves by side prefix instead).
- RightHUDImage (SALogo) bind is implemented but its atlas is also missing; frame art
  (ImagePart InGameUIAmericaBase → SACommandBar.tga) equally asset-bound.
- Shell menus: skirmish-menu combo/slider art is ShellMenuScheme-bound in retail (same
  pattern as this lane); shell scheme application is a separate shell-lane task.

## AudioParityHunt — in-match audio parity: dispatch→sink chain VERIFIED LIVE (device opens, real audio reaches rodio sinks); env gap documented + one code gap FIXED, one documented with fix direction (2026-09-03 15:20-16:10)

Method: one control-driven windowed skirmish drive (drive14/14b/14c, /tmp/wsmoke/drive14.sh; lane per Main's queue; OS physical clicks dead → winit_click_named/select_all/move control chain, `move_ok:n=1:x=90.0:y=210.0` latched in-match) against a release binary carrying env-gated `GENERALS_AUDIOTRACE=1` warn-level probes (since REMOVED — net game_audio.rs diff vs HEAD is one blank line; subsystem_manager.rs zero diff). Evidence: /tmp/wsmoke/drive14.log + child stderr `.../generals_exec_smoke_manual_1788460450/child_stderr.txt` (grep AUDIOTRACE).

### 1. Chain is ALIVE end-to-end (probe evidence)
- `AUDIOTRACE audio device OPENED (cpal default output)` — cpal/rodio output stream opens at boot.
- In-game events flow: GameLogic/weapon hooks → MainAudioDispatch → AudioManagerSubsystem drain (26,622 `drain theaudio-add` lines) → THE_AUDIO `add_audio_event` → engine `update` (run_loop.rs:209) → `process_request_list` → `RodioPlaybackHook::play` → rodio Sink append.
- **Real retail-restoration audio DID play through the sink**: `sink-play ok event='GUIBoarderFadeIn' file='Data/Audio/Sounds/uboarder.wav' bytes=Some(151708)` — resolved from `~/Downloads/GeneralsGamePatch/Patch104pZH/GameFilesOptional` (a real 899 MB patch install the engine's install-scan mounts into the VFS). `Amb_TemperateForestTreesLoop` played ×2. Menu clicks dispatched GUIClick; RadarOn got live handles (Some(1084)).
- THE_AUDIO update IS draining: granted handles were processed to the hook, so no missing-update gap.

### 2. Environment gap (prerequisite, not code)
No retail `AudioZH.big`/music MP3s exist on this machine; the patch pack covers only a slice of Sounds/Speech. For the probe I generated 4,019 stand-in WAVs + 56 music MP3s into gitignored `GeneralsRust/windows_game/extracted_big_files_v2/AudioZH/{Sounds,Music}` (removed post-hunt) so every INI event name had resolvable data; misses below are dispatch-layer facts, not file-absence.

### 3. Code gap #1 — FIXED: extracted-audio remap dead in live binary (audio_event_rts.rs)
Every hook resolve logs the RAW INI filename (`Data\Audio\Sounds\umenucla.wav`) — `resolve_extracted_audiozh_path` never remapped although AudioZH files existed: **`main.rs:481-482` `set_current_dir(exe_dir)`** makes cwd `target/release`, so the remap's cwd-relative `windows_game/extracted_big_files[_v2]/AudioZH` roots can never match (runtime proof: relative `[ -f windows_game/extracted_big_files_v2/AudioZH/Music/USA_11.mp3 ]` true from GeneralsRust, live remap still None → Shell music resolve-miss). FIX LANDED: roots additionally anchored at `current_exe()` ancestors (target/release → GeneralsRust), restoring the extracted-audio fallback for INI event→file mapping (C++ parity: Miles resolves via TheFileSystem mounts, GameAudio.cpp:186-202; the remap is the sanctioned rodio-era substitute, hq-cc2zh provenance). Compile-verified (game_engine + generals_main, 0 errors); live sink re-drive left to the next windowed slot.

### 4. Code gap #2 — documented, fix direction: generic `WeaponFire` can never play
`WeaponFire` ×2,929 → THE_AUDIO `ERR(no-info)` (AHSV_ERROR: no such AudioEvent) → silent. C++ plays the weapon template's authored FireSound per shot (e.g. CrusaderFire), never a generic token; the host emitter (`world_tick/combat.rs`) must resolve the weapon's INI FireSound name instead. Same class: `UnitHeal` ×26,264 at ~150/s (invented token, ERR(no-info)) — emitter (`game_logic/crate_tick.rs` heal path) additionally needs C++ Limit/once-per-heal semantics, not per-frame queueing.

### 5. Menu audio status
Menu button clicks DO dispatch (GUIClick → hook); menu music Shell is queued at boot (`play_random_cnc_music` → TheAudio) and resolves post-§3; in-game faction music is map-script-driven in C++ (no engine gap found).

Probe hygiene: probes removed (grep `audiotrace|AUDIOTRACE` over GeneralsRust/Code = 0); drive artifacts outside the repo (/tmp/wsmoke/drive14.log, drive14_*.png). No test files modified; catalog validation belongs to Main's sweep.

---

## BuildUiHunt — UI-driven build hunt: command-grid binding is the LIVE blocker; selection + sim command paths verified working (2026-09-03 15:43-16:35, IN PROGRESS at budget cap — probes staged, discriminating run pending)

Method: bd1 drive (`/tmp/wsmoke/bdrive1.sh`, control-command path — OS synthetic clicks are DEAD since ~14:15 per UiGapInventory/Main; all clicks traverse `handle_mouse_button_input` → WM hit-test → GBM → ControlBar processing as `MouseInputOrigin::Injected`). Binary: release `generals` 15:35 tree (contains my two drive aids). Serial windowed runs, caffeinate -dis, no git writes, no formatters.

### Drive aids landed (permanent, in scope for this hunt)
- `Main/src/cnc_game_engine/runtime.rs` STATUS_GADGET_HIT_NAMES: added `ControlBar.wnd:ButtonCommand01..14` (status publishes a slot only when a live, enabled, non-hidden window hit-tests at its own center — honest empty-grid evidence).
- `Main/src/cnc_game_engine/runtime_host/shell_core.rs` + `mod.rs`: new control command `winit_click_at|x=|y=` (InGame-gated, headless fail-closed) injecting an LMB at a client point through the same path as `winit_gameplay_order`'s inject — world-pick/placement clicks need a free cursor that `winit_click_named` (gadget centers only) cannot express. Both verified present in the 15:35 binary (`strings` hit counts 12 / 2) and exercised live (`winit_click_at_ok:320,240`, `winit_click_named_miss:...`).

### Verified working live (evidence: /tmp/wsmoke/bdrive1.log, bd1_*.png, bd1_*.txt)
- **Boot**: control-command chain (window_move → reveal → `winit_menu_nav` partial-hit Skirmish → `start_game|mode=skirmish|faction=USA`) → InGame at ~90 s, sim live to logic_frame 6546+.
- **Selection**: host `select_local_unit` → `select_ok:465` (the dozer; local_mobile_units=1 = dozer, US start here is CC+dozer only, no rangers); world click `winit_click_at|320,240` after `view_command_center` → `selected_count=1`; portrait paints (ControlBar context sees the selection).
- **Sim production entry**: `enqueue_production|template=AmericaInfantryDozer` → `train_fail_no_ready_barracks` (barracks-gated fail-closed, honest; can't test unit production at start without a barracks or the UI).

### THE GAP (live-confirmed): command grid binds ZERO slots with an active selection
- HITS publishes **0** ButtonCommand slots after CC selection AND after dozer selection (both host and click selection paths).
- `winit_click_named|ControlBar.wnd:ButtonCommand01` → `winit_click_named_miss` (slot hidden / not hit-testable).
- Capture `bd1_cc_sel1.png`: bottom-right 14-slot grid renders EMPTY (vivid cameo pixels 21 vs 1916 in the pre-selection default frame — same empty-after-selection signature as CtrlBarTexFix's ctb2_selected capture) while the portrait paints.

### Root-cause analysis (static, high-confidence; runtime confirmation staged)
`ControlBar::add_object_commands` (GameClient `gui/control_bar/control_bar_impl/impl_buttons.rs:17-62`) needs a non-empty command-set name from exactly one of:
1. live object `get_command_set_string()` — `object/object_queries.rs:809-817` falls through to `thing_template.get_command_set_string()`, whose **trait default** (`common/types/thing_template.rs:153-157`) returns a static EMPTY; only `DefaultThingTemplate` overrides it (`common/types/default_template.rs:471`, parsed from the INI "CommandSet" field at :372-374). Host in-match objects hold `Arc<dyn ThingTemplate>` (`object/mod.rs:2637`); if the live template is not a DefaultThingTemplate-backed instance, every object reports EMPTY. (Corroborating signal: a sibling's earlier broken-build error `no method set_command_set_string on game_logic::thing::ThingTemplate` — the live template type lacks the setter/accessor surface.)
2. `presentation_primary_command_set` — also empty on this path (host selection stamps nothing).
→ both empty ⇒ add_object_commands returns with zero commands ⇒ `bind_command_windows` (impl_execute.rs:610-645) hides all 14 slots ⇒ zero hittable slots, empty grid, exactly the observed screen.
If the name IS non-empty, the next candidate is `control_bar.find_command_set_by_name` (impl_buttons.rs:63-79) failing to resolve the retail CommandSet in the bridge.

### Staged (in-tree, env-gated, REMOVE AFTER the discriminating run)
`GENERALS_CBGRID_PROBE=1` warn-level probes: impl_buttons.rs add_object_commands (cs_name/pres_cs + bridge lookup result) and impl_lifecycle.rs evaluate_context_ui (chosen ControlBarState + registry-vs-presentation branch marker). They are in the shared 16:25 release binary (OverlayTagDrive's build carries portrait fix + probes).

### Next steps (precise)
1. Relaunch bdrive1.sh (now launches with GENERALS_CBGRID_PROBE=1), boot, select dozer + CC, read `CBGRID:` lines from child stderr → identifies which of the three inputs is empty at runtime.
2. Fix per C++ oracle: ControlBar.cpp:2403-2480 setControlCommand + Object::getCommandSetString — C++ objects ALWAYS carry the template's CommandSet string; fix = propagate the parsed CommandSet into the live template/object (or resolve template-name→CommandSet via the ThingFactory/CommandSetManager fallback the bridge already supports), NOT a UI-side special case.
3. Remove CBGRID probes; rebuild; verify end-to-end: select dozer → Barracks cameo visible+hittable → click → placement → dozer builds (under_construction 1→0) → select barracks → Ranger cameo → click → production → local_mobile_units +1 (screenshot evidence).
4. Then re-test `winit_click_named` chain for unit production (Ranger) with the now-bound slots.

Artifacts: /tmp/wsmoke/bdrive1.sh (+bd1_cmd.txt executor queue), bdrive1.log, bd1_{boot,cc_sel1,train1,hostsel,hostall}.png, bd1_{cc_sel1,train1,slot01,hostsel}.txt, child stderr under $(cat /tmp/wsmoke/bd1_dir). Session killed 15:54 to free the lane; no suites touched; changed tree files: runtime.rs, shell_core.rs, mod.rs (runtime_host), impl_buttons.rs + impl_lifecycle.rs (probes only).

---

## SaveLoadUiHunt — mid-match save/load via windowed UI: 2 runtime gaps root-caused, 1 fixed, chain blocked one seam short of full E2E (2026-09-03 15:00-16:40)

### Scope/method
Drive13-lineage windowed chain on Defcon6 skirmish USA (640x480 runtime-host windowed, control-file driven after OS synthetic clicks died environment-wide ~14:15 per Main/UiGapInventory): menu → start_game → InGame → select_local_unit → move → pause menu → PopupSaveLoad save → load → resume verify. New diagnostic drive aid + probe (kept, documented): `STATUS_GADGET_HIT_NAMES` extended with QuitMenu/PopupSaveLoad buttons + ListboxGames (runtime.rs, same diagnostic-only contract as BuildUiHunt's cameo block) and env-gated `GENERALS_SLUI_PROBE=1` roster line per status publish (dispatch.rs `SLUIROSTER`, local-player id/template/pos/hp/selected/dest — the ONLY machine-readable per-unit view; kept for the follow-up pass, removal trivial: delete static+block).

### Verified working (live windowed run, 16:25 binary)
- Full menu→skirmish→InGame latch; `select_local_unit` → `select_ok:465` (dozer) with presentation selection mirrored (`sel=true` on id=465).
- SLUI roster truth pre-save: dozer id=465 `AmericaVehicleDozer @(3108.0,120.0,2201.3) hp=250/250 sel=true`, CC id=464 `@(3068.0,0.0,2241.3) hp=5000/5000`; logic_frame continuity and InGame/paused residuals all honest.
- Save/load production authority remains green in the harness (SaveLoadHunt landed fixes; golden_skirmish `save_load=true players_preserved=true`).

### GAP 1 (documented, open): control-command movement orders never reach the object
`move|x=|y=|z=` and `attack_move` both ack `move_ok:n=1` / `attack_move_ok` with selection present, but the dozer's presentation `move_destination` stays unset and position is unchanged across 60 s+ (logic frames advancing; SLUI probe). Movement via PHYSICAL RMB is the proven-good path (drive13 five-flag); the host `host_command_move` seam (ui_selected_ids→host_set_selection→host_command_move) drops the order on the floor in the runtime-host dual-world path. Needs a dedicated pass with the probe in place.

### GAP 2 (partially FIXED by me; one seam remains): pause_save/pause_load fail-closed — QuitMenu.wnd never shown
Live `pause_save|slot=...` returned `save_fail_wnd_missing` with the match paused: the chain (`ensure_live_quit_menu_layout` → `dispatch_os_click_named_window("QuitMenu.wnd:ButtonSaveLoad")`) fail-closes because the created QuitMenu layout is never SHOWN — C++ parity: the human pause runs `ToggleQuitMenu` which shows the layout (quit_menu.rs `toggle_quit_menu_with_result` → `transition_set_group("QuitFull")`) before any button can be clicked; GameState pause alone never reveals it. **Fix landed (GameClient behavior via Main runtime_host/gameplay.rs):** both `runtime_host_cmd_pause_save` and `runtime_host_cmd_pause_load` now call `toggle_quit_menu()` when the quit menu is not visible before ensure/drive (C++ ToggleQuitMenu parity). Post-fix live run: menu show + pause latch now work (`paused=true` via quit-menu bridge), but `QuitMenu.wnd:ButtonSaveLoad` STILL never publishes a hittable center (0 hits across repeated publishes) → dispatch still fails closed. Remaining seam: the in-match-created `Menus/QuitMenu.wnd` layout's children never become under-cursor hit-testable (transition-group visibility or WM registration of layout windows created mid-match) — `ensure_live_quit_menu_layout` + `dispatch_os_click_named_window` need a "wait until hittable" (cf. `dispatch_os_click_named_window_when_hittable`) or a layout-show/registration fix in quit_menu.rs. All popup_save_load.rs unit tests pass because fixtures `install_named_button` with explicit `hide(false)` — the live gap is invisible to them.

### Evidence
/tmp/wsmoke/{sluichain3.log,sluichain3_run.log,slui_rosters.txt,slui_final_stderr.log,slui_ingame0.png,slui_selected.png,sluichain2.sh,sluichain3.sh}; live run dir .../generals_exec_smoke_manual_1788463555 (roster: dozer 465/CC 464 hp+pos+sel, paused=true post-menu-show). C++ citations: GameState.cpp:628-723 loadGame + gameStatePostProcessLoad (1505-1523: per-snapshot loadPostProcess → Radar.cpp:1515-1524 refreshTerrain, Drawable.cpp:5396-5420 transform resync, GameLogic.cpp:4996-5071 update-list rebuild); PopupSaveLoad.cpp:362-401 doLoadGame; QuitMenu ToggleQuitMenu show path.

### Changed files (this pass)
- `Code/Main/src/cnc_game_engine/runtime_host/gameplay.rs`: pause_save/pause_load show QuitMenu before dispatch (fix).
- `Code/Main/src/cnc_game_engine/runtime.rs`: STATUS_GADGET_HIT_NAMES + QuitMenu/PopupSaveLoad gadgets (drive aid, kept).
- `Code/Main/src/cnc_game_engine/dispatch.rs`: GENERALS_SLUI_PROBE roster line (kept, documented; delete SLUIROSTER static+block when GAP 1/2 close).
No git writes; no formatters; serial cargo coordinated with 5 sibling lanes (two shared release builds carried my fixes).
- RRT handoff (17:2x): binary with this lane + the impl_portrait:36 RefCell fix
  (OverlayTagDrive) is at target/release/generals. rrtdrive1.sh is staged; first driver
  with a free window runs it (~4 min, control-driven) and appends rrt_ingame_*.png
  verdicts here. rrt_menu.png + rrt_skirmish_menu.png already captured (16:1x binary):
  menus render; skirmish-menu Starting-Cash red box persists via the ShellMenuScheme.ini
  parse failure — shell-lane residual, NOT control-bar.

## OverlayTagDrive — white-blob writer NARROWED to the terrain EXTRA-BLEND pass: occlusion + shadow overlay lanes and ALL THREE terrain decal families (tree, road/overlay, water) exonerated by one tagged + three null-lane drives (2026-09-03 14:35-16:45)

### 1. Tagged discriminator (tag1, 15:03-15:05, binary mtime 15:00)
Drive /tmp/wsmoke/tagdrive1.sh (control-driven, 640x480, caffeinate) with GENERALS_UTBOVERLAYTAG=1 + GENERALS_UTBSHADOWTAG=1 against the staged probes. Verdict: **0 green px, 0 blue px** (2 blue HUD-icon noise px in every frame); blob-region white census blobA 757→730, blobB 469→469 vs baseline utb6. `occlusion_bridge.rs` enqueue_occluded_player_color_pass (green tag) and `shadow_pass.rs` record_shadow_and_occlusion_passes overlay billboards (blue tag) are **EXONERATED** — those passes never fire on-screen here (both only emit overlays for occluded/heat-vision units; spawn area has no occluded units).

### 2. Null-lane discriminators (A1/B1/C1, 16:26-16:40, binary mtime 16:4x — carries the impl_portrait.rs:36 `RefCell already borrowed` crash fix, which had aborted treenull1; the panic was a live-path P1 blocking every sibling's drive, fixed by copying the image out of the owned draw data before `borrow_mut`)
- A1 GENERALS_UTBTREENULL=1 (`record_tree_draws` skip): white blobs PERSIST (swept triangles, slivers, rect-notch) → tree billboards not the writer.
- B1 GENERALS_UTBROADNULL=1 (`record_road_draws` + `record_overlay_draws` skip = road/bridge/scorch/bib/tank-track/custom-edge/flat-LOD/smudge/snow all off): blobs PERSIST unchanged → road decal family exonerated.
- C1 GENERALS_UTBWATERNULL=1 (`record_water_draws` + `record_extra_water_draws` + main water pass skip): blobs PERSIST unchanged → water family exonerated.
Captures: /tmp/wsmoke/{treenull_A1,roadnull_B1,waternull_C1}_dozer.png vs baseline /tmp/wsmoke/tag_tag1_dozer.png. Cross-check: snow lane never initializes (zero "snow" lines in stderr; its white 1x1 flake fallback therefore irrelevant).

### 3. Remaining suspect (next lane-owner's target): `TerrainVisualImpl::record_extra_blend_pass` — GameClient `terrain/terrain_visual/impl_gpu.rs` (~line 2270 post-edit; "Second extra-blend pass over the base terrain (alpha overlay, no Z write)")
It is the ONLY remaining un-gated terrain-pass draw, and the blob silhouettes (irregular jagged polygon patches, terrain-tile-sized) match extra-blend tile geometry. Red flags for the white-out:
- `extra_blend_pipeline` falls back to the plain TERRAIN pipeline (`terrain.wgsl`) which has NO alpha-blend of its own for a second overlay pass;
- bind group 1 is `chunk_texture_bindings.values().next()` — an ARBITRARY first chunk texture, not the tile's authored blend texture (C++ W3D HeightMap extra-blend draws each tile with ITS OWN texture + vertex modulate);
- if `upload_extra_blend_overlay` (~line 2197) bakes white (255,255,255) vertex modulate or the pipeline blend is opaque-replace, the second pass paints flat-white terrain-tile polygons = exactly the observed blobs (exact 255 white, hard edges, static world anchors, camera-foreshortened).
Suggested discriminating fix for the closer: env-gate `record_extra_blend_pass` (UTBEXTRABLENDNULL) — expect blobs to vanish — then fix per C++ (per-tile texture binding + authored modulate/blend), remove ALL staged probes.

### 4. Probe inventory / hygiene for the closer (ALL env-gated, warn-level, inert without their env vars, `cargo check -p game-client-rust --lib` + `-p generals_main --lib` clean)
- occlusion_bridge.rs UTBOVERLAYTAG block (~line 486); shadow_pass.rs UTBSHADOWTAG block (~line 980) — UnitLightFix's staged tags.
- impl_gpu.rs: UTBWATERNULL in `record_water_draws`, UTBROADNULL in `record_road_draws`, UTBTREENULL in `record_tree_draws`, UTBTREEDUMP block + `utb_tree_dump_latch()` in `update_tree_meshes` (relocated to fire only once atlas levels are non-empty; never observed in drives because the tree lane was exonerated first).
- overlay_gpu.rs: UTBROADNULL in `record_overlay_draws`, UTBWATERNULL in `record_extra_water_draws`.
- pipeline_collect.rs: UTBWATERNULL early-return in the main-water-pass pre-scene callback (~line 1812).
- KEEP (documented diagnostic precedent): ZPROBE in pipeline_execute.rs. Drive scripts (outside repo): /tmp/wsmoke/tagdrive1.sh, nullDrive_{tree,road,water}.sh, logs nullabc.log.
- Guards untouched: combat filter 966/0, world_tests catalog, gameworld_shadow 302/302 — no GameLogic files modified; my kept change is ONLY the impl_portrait.rs borrow fix (GUI, live-path crash).

### 5. Screen truth at hand-off
Blobs persist exact-255 white in all five tagged/nulled captures; units remain white-silhouette (mesh lane exonerated earlier by UTBMAGENTA). The fixer should screenshot textured units after the extra-blend fix (acceptance for UnitLightFix2's ticket).

### 3b. ADDENDUM (16:50) — second candidate, UnitLightFix2's render_decals decal lane (probe STAGED, not yet driven)
The decal lane in GameClient `particle_renderer.rs` (render_decal_queue) is NOT covered by any of my five probes and CAN paint exact-white hard-edged unit-shaped quads: the generic else-branch pipeline is ALPHA_BLENDING with the default WHITE 1x1 bind group, fed by (a) SHADOW_ALPHA_DECAL(0x20) items (not handled explicitly, falls to else) and (b) smudge + DecalManager items whose texture_name is EMPTY (texture load skipped silently, default white texture stays bound). SHADOW_DECAL(0x01) modulate cannot go white (out=dst*src). GENERALS_UTBDECALTAG=1 is staged in-tree by UnitLightFix2 (tints every decal vertex pure RED + per-group warn logging of texture_name/shadow_type/vertex_count/default_tex_used). NEXT DRIVE after a rebuild that includes it: UTBDECALTAG=1 alone — red blobs ⇒ decal lane writer (the warn lines give the exact texture_name/shadow_type for the fix); unchanged white ⇒ terrain extra-blend pass (section 3) is the writer — then gate UTBEXTRABLENDNULL to confirm before fixing. Both probes + my five nulls + two tags come OUT after the fix lands.
- RRT VERIFIED (16:4x binary, rrtdrive1 EXIT=0): /tmp/wsmoke/rrt/rrt_ingame_{initial,
  selected,order}.png — ZERO red pixels in all three in-match states (control bar/HUD
  acceptance CLOSED). Scheme lane PROVEN live: left-stack buttons render at scheme rects
  (options 147,392 / worker 147,413 / chat 147,454 scaled) and ButtonLarge at MinMax rect
  (517,346-574,370) — positions moved by apply_scheme_button_art exactly per
  ControlBarScheme.cpp; art slots show the renderer's honest missing-atlas slate fill
  (SAControlBar512_001.tga absent) and will show retail art on re-extraction. Cameo strip,
  money, right-HUD slots, exp bar all render. Beacon hidden (skirmish parity) — left stack
  shows options/worker/chat. No regressions observed in the three states.

---

## CmdSetFix — command-grid zero-bind root-caused x2 + FIXED (2026-09-03, probe-driven; drive verification staged)

### CBGRID probe verdict (16:25 binary, cbdrive1, 16:52-16:54, Defcon6 skirmish USA)
904x `CBGRID: obj=464/465 NOT in OBJECT_REGISTRY -> presentation-residual branch`, ZERO `add_object_commands` lines, portrait visible, grid empty: the ControlBar never reached button binding at all.

### Root cause 1 — duplicate-CommandSet hard abort killed the ControlBar bridge
Recovered repack's `INIZH/Data/INI/MappedImages/TextureSize_512/SAControlBar512.INI` embeds ALL 472 CommandSet blocks (+CommandButton/AudioSettings/Animation fragments). Tolerant mapped-image lane parses it first → manager populated → shell.rs strict `Data/INI/CommandSet.ini` load dups on set #1 → `command_sets_parsed=false` (buttons=820, sets=472 "catalog unavailable" warn) → `refresh_control_bar_bridge_from_common()` never ran → `get_control_bar_bridge()`=None → add_object_commands early-out. C++ parity: ControlBar.cpp:1949-1981 re-parses into the existing set on duplicate (KM note: "nuke the old button with the new one"; DEBUG_CRASH debug-only). Fix: ini_command_set.rs duplicate branch = find_command_set_mut + parse_command_set_fields (overwrite), debug-level log.

### Root cause 2 — locomotor-without-AIUpdate guard skipped 604 Object blocks
Live boot stderr: 606x `Skipping object '<X>': Attempted to specify a locomotor ... without an AIUpdate block` (AmericaInfantryRanger, AmericaVehicleDozer, AmericaTankCrusader, all Tank_/SupW_/CINE_ variants) → templates left as name-only shells → CommandSet/BuildCost/KindOf lookups empty (kills ranger cameo lane + presentation freeze). C++ AIUpdate.cpp:141-149 reads the EMBEDDED m_aiModuleInfo (never null; guard is dead code in retail). Fix: thing_template.rs write_locomotor_set_into_ai_module returns Ok when no AI module exists yet; stored locomotor_sets replay via apply_stored_locomotors_to_ai_module (thing_factory.rs:994).

### State
CBGRID probes REMOVED (impl_buttons.rs + impl_lifecycle.rs; grep CBGRID = 0). `cargo check -p game_engine -p game-client-rust --lib` clean. NOT yet driven: next `cargo build --release --bin generals` then `/tmp/wsmoke/cbdrive2.sh` (select dozer → ButtonCommand03 Barracks cameo → placement → under_construction 1→0 → select barracks → ButtonCommand01 Ranger cameo → local_mobile_units +1; screenshots /tmp/wsmoke/cbd2_*.png). Residual same-class skips (one object each, pre-existing, out of lane): DefaultThingTemplate `OverrideableByLikeKind.Behavior` key leak; FireHydrantRed EditorSorting `MISC_MAN_MADE CLEARED_BY_BUILD` token. Evidence: /tmp/wsmoke/cbdrive1.log, cbd1_{cc_sel.png,child_stderr.log,skips}, /tmp/cbprobe.2ztK/{out,err}.log, bead hq-9udt7 notes.

---

## UnitLightFix2 — white-units blocker ROOT-CAUSED (white shards = prop light quads; REAL blocker = ALL unit/building bodies invisible despite valid geometry/transforms): 6 env-gated probes landed, zero drives contradicted, fix NOT landed (2026-09-03 17:00-19:50)

Method: 8 windowed 640x480 control-driven skirmish drives (utbdrive5-13.sh → /tmp/wsmoke/utb5..utb13_*_{dozer.png,stderr.log}) against release binaries carrying env-gated warn-level probes (all inert without env; no git writes; no formatters). Inherited stale context: UTBLIGHT had already fired (prior UnitLightFix section) — lighting 0-1 correct, so the mesh-lane-lighting hypothesis was dead on arrival.

### 1. White shards IDENTIFIED (the visible artifact)
- UTBDECALTAG (red tint of every `ParticleRenderer::render_decals` draw): 0 red px, 0 UTBDECAL lines ⇒ decal lane drew NOTHING all session — exonerated (also proves the alpha-decal pipeline + white-default-bind-group hole in `render_decals` never fires in-match).
- UTBEXTRABLENDNULL (skip `TerrainVisualImpl::record_extra_blend_pass`): blobs unchanged ⇒ extra-blend second pass exonerated (probe verified live via display.rs:1035 → record_chunk_draws:872).
- UTBMESHNULL (early-return `MeshRenderManager::draw_material_pass`): **shards VANISH** ⇒ they ARE ww3d mesh-lane pixels; the earlier UTBMAGENTA "mesh lane exonerated" conclusion was WRONG (override demonstrably didn't tint them).
- UTBDRAWLOG/UTBDRAWSKIP (per-draw log + surgical `start:count` skip): shards = **2-triangle (4-vert, range (0,6)) sub-mesh quads of prop models** — `Coplight.tga` (11 sites), `Lightbeam.tga`, `PMredlight.tga`, `Housecolor.tga/2`, `ATFan.tga`, `UBSnkAtak_01` + 7× `__missing_texture__` (Main `forward_materials.rs:680` fallback). shaderbits 0x24881b/0x34881b = SRCALPHA:ZERO cutout + texturing; rendered WHITE because alpha.wgsl `if !has_diffuse { layers.diffuse = vec3(1.0) }` — no-enabled-diffuse-stage passes paint white, and white-art quads paint white when enabled. Retail renders these as colored/soft light glows via vertex-material + authored alpha.
- UTBPASSTINT (per-pass material_diffuse override): shards stayed white ⇒ their shader path ignores material_diffuse color (consistent with stage-mask-off white default).

### 2. THE REAL BLOCKER: unit/building BODIES are invisible outright
`sample_unit_pos=3068.0,0.0,2241.3:AmericaCommandCenter` — camera_look_at targets the player CC correctly; screen shows terrain only. Evidence chain (all on correct-camera drives):
- UTBVERTS (geometry dump at `PreparedMeshModel::frommodel`): CC body `ABBtCmdHQ::7::ATHQSlab.tga` 894 verts bbox (-53.7..54.4, -0.1..51.9, -65.1..64.8) VALID; dozer `AVCONSTDOZ_A::0..10` all valid; quads flat y=0.00. No collapse, no NaN.
- UTBDRAW world= dump: body transforms CORRECT (CC body at exactly (3068.0,0.0,2241.3); dozer parts (3100-3119,120-131,2201); supply docks at both start bases). Textures bound with real pixels.
- UTBNODEPTH (mesh pipelines with depth_format=None): still invisible ⇒ NOT depth occlusion/order.
- GENERALS_FORCE_TWO_SIDED=1 (existing env; honored at wgpu_pipeline_manager.rs:547): still invisible ⇒ NOT winding/culling.
- UTBMESHVIS (per-model FOW + camera): `alpha_override=1.000 pres_opacity=1.000 fow=(a=1.000 f=1.000 exp=1.000)` ⇒ NOT FOW/alpha discard.
Remaining candidate space (narrow, next driver): (a) mesh-lane camera view-projection matrix content (get_position correct ≠ VP correct — dump the VP vs terrain lane's), (b) index buffer CONTENT (ranges/counts valid but `model.triangles` content never verified — all-zero indices = degenerate = invisible; ZPROBE readback precedent applies), (c) bodies rasterizing into a frame-graph target that never presents while static-sort quads present.

### 3. Probe inventory (ALL env-gated, warn-level, compile-clean; REMOVE AFTER fix)
render_manager.rs (ww3d-renderer-3d): UTBMESHNULL, UTBPASSTINT+UTBRANGE, UTBDRAWLOG/UTBDRAWSKIP(+deep dump; also feeds «shroud» caller marker), UTBVERTS, UTBMESHVIS, UTBNODEPTH. particle_renderer.rs (game-client-rust): UTBDECALTAG. terrain_visual/impl_gpu.rs: UTBEXTRABLENDNULL. Prior lanes' UTBLIGHT/UTBUVCENTER/UTBMAT/UTBOVERLAYTAG/UTBSHADOWTAG also still in-tree. `cargo check -p ww3d-renderer-3d --lib` and `-p game-client-rust --lib` clean after each edit; guard strings (frozen_fow_* tests, ww3d tests.rs draw_material_pass contract) untouched.
NOTE for drivers: `target/release/generals` (19:4x) is MINE and carries ALL probes; Main-pipeline lanes (CmdSetFix) must rebuild for their own verification. Guards at hand-off (EngineStores2, independent): combat 965/966 (chinook residual, pre-existing), gameworld_shadow 304/304, world_tests 8/8.
Artifacts: /tmp/wsmoke/utbdrive5-13.sh, utb5..utb13_* captures + stderr logs; key captures: utb5_utbdisc1 (shards present), utb6_utbmeshnull1 (shards gone), utb9_utbworld1 (shards gone + world dump), utb12_utbtwosided1 (units still gone).

---

## UnitBodyFix — 3-suspect discrimination COMPLETE: VP/indices/viewport/frame-target ALL EXONERATED; failure isolated to GPU-side execution of body draws; headless lane repro PASSES (2026-09-03 20:00-24:00)

Method: 7 windowed 640x480 drives (utbdrive14-19 + reruns → /tmp/wsmoke/utb14..utb19_*_{dozer.png,stderr.log}) against release binaries carrying NEW env-gated probes: UTBVP (project mesh world pos through the exact uploaded VP at bind time + transform basis + transformed-bbox NDC extents), UTBIDX (index-buffer CONTENT stats), UTBVPVPORT (mesh-pass viewport/scissor, change-latched), UTBTERRVP (terrain-lane VP rows at install), UTBPIX (frame color-texture readback — proved wrong target, all-zero), UTBMAT re-keyed per texture (mesh.name is empty live, old key latched after one draw), UTBNONIDX (bodies drawn non-indexed), UTBVREAD (buffer usage flags staged for GPU buffer readback, readback call NOT yet wired).

### Verdicts on the 3 handed-down suspects (all measured, none indicted)
- (a) VP CONTENT: EXONERATED. UTBVP: CC body (ATHQSlab, world 3068,0,2241.3) ndc=(0.000,-0.584,0.984) clipw=582; dozer parts ndc≈(-0.24..-0.09,-0.23..-0.06) — on-screen. VP rows identical to the terrain lane's (UTBTERRVP terr_vp0==cam_vp0=-2.1445...). Visible shards render at their exact projected pixels (PNG sample (110,70)=white matches Lightbeam bbox px153-176/rows75-125 cluster).
- (b) INDEX BUFFER CONTENT: EXONERATED (CPU side). UTBIDX: CC 1422 indices min=0 max=893(=verts-1) zeros=2 first=[1,0,2, 0,3,2...] — real triangles; all 123 dumped models in-range. GPU-side content still unverified (UTBVREAD staged, readback call pending).
- (c) FRAME-GRAIN COLOR TARGET: EXONERATED in its handed-down form. UTBVPVPORT in-match: viewport=(0,0,640,384) scissor=(0,0,640,384) on the SAME attachment the terrain pre-scene pass and UI use (one ww3d frame: terrain pre-scene → WW3D Main Render Pass meshes → post-frame UI). The dozer (rows 254-295) is INSIDE that rect.

### NEW measured facts (whoever picks this up, start here)
1. Presented-frame pixels are pure terrain at every projected body location (offline PNG decode of utb14_utbvp1_dozer.png: CC interior (320,300)/(320,340)/(330,260)/(280,330) ≈ (117,106,81) tan = terrain; dozer (260,272) same; shard calibration (110,70)=(255,255,255)). With UTBNODEPTH (no depth test) bodies stayed invisible ⇒ body fragments NEVER REACH THE COLOR TARGET — this is not blend/alpha/depth-fail.
2. Transform content clean: UTBVP basis=(1.000,1.000,1.000), skinned=false, CC transformed bbox covers NDC x -0.21..0.22 y -0.91..-0.11 (≈275x220 px on-screen at px 253..390 rows 218..437; rows>384 are viewport-clipped, rows 218..384 are NOT).
3. UTBMAT (re-keyed by texture): EVERY pass incl. CC slab + dozer: mat_diff=(1,1,1,1), overrides=(1,1,1,0), stage_mask=01, textures Rgba8 with px alpha=ff (ATHQSlab px0=[ff,fb,ef,ff]). CC sub-mesh shaders: 0x0024881b (main slab; IDENTICAL to the visible shard shader → same pipeline cache key), 0x0034881b (alphatest bit set), 0x0020881b (untextured). One CC 128x128 sub-texture has alpha=00 at px0 (shadow-like pass).
4. UTBNONIDX: drawing bodies NON-INDEXED (draw(0..894), same vertex buffer, same pipeline) STILL paints nothing ⇒ the failure is upstream of index fetch — vertex data as seen by the GPU or the draw execution itself.
5. GENERALS_DISC_NOTERRAIN=1: world area fully BLACK incl. shards ⇒ the terrain pre-scene pass owns the depth (and effectively color) clears; without it all meshes fail depth on garbage. Consistent with, but not explanatory of, the body invisibility.
6. HEADLESS ISOLATION TEST PASSES: ww3d-renderer-3d `utb_headless_body_mesh_paints_pixels` (render_manager.rs tests, 894-vert body, default opaque pass, live-equivalent camera/pipeline code path, offscreen readback) paints — the lane's core draw machinery is sound. The defect lives in the live frame's per-draw inputs or assembly order, NOT in the pipeline/shader/ABI.
7. Shader semantics note for the eventual fix pass: alpha.wgsl:630-637 discards UNCONDITIONALLY (final_alpha < 96/255*alpha_override) while C++ W3D applies alpha test only when ShaderClass bit20 (SHIFT_ALPHATEST) is set (render_state.rs:140 parity). CC main pass routes to opaque.wgsl (no discard) so this is NOT today's blocker, but every SRCALPHA:INVSRCALPHA mesh in the game is one bad uniform away from a full-discard; alpha-test gating belongs in the fix pass.

### Probe inventory added this pass (env-gated, inert without env; REMOVE AFTER fix like the others)
render_manager.rs: UTBVP, UTBIDX, UTBNONIDX, UTBVREAD usage flags, UTBMAT re-key. lib.rs (ww3d-renderer-3d): UTBVPVPORT. forward_render.rs (Main): UTBTERRVP. pipeline_execute.rs (Main): UTBPIX (+ww3d-engine `color_texture_arc()` accessor; note the frame's internal color texture is NOT the presented target — all-zero readback; the real capture path is `ww3d_engine::make_screenshot` → surface texture). Plus test `utb_headless_body_mesh_paints_pixels` (KEEP until fix lands, then keep or convert — it is a real regression harness).

### Exact next steps for the closer (in order, each ~1 drive)
1. Wire the UTBVREAD readback in `preparemodel` (queue IS available there via gpu_device.queue()): copy the first ~132 floats of the CC's vertex_buffer + first 16 indices from the GPU and log them next to the CPU values (the ZPROBE synchronous pattern). This is the ONLY unverified GPU-side input left; UTBNONIDX failing points squarely at it.
2. If GPU vertex data == CPU: the draw executes with correct data but no fragments — then diff the LIVE draw against the passing headless test by bisection inside draw_material_pass (lighting env Some→None, texture binds → fallback white, one-mesh-only frame via skipping all other queue_mesh calls) — one env flag per run.
3. If GPU vertex data != CPU: the upload path (create_buffer_init) or a buffer lifecycle bug (arena/cache) is indicted; check `PreparedMeshModel` pointer-keyed caches (`Arc::as_ptr` key, revision starts at 0 for every fresh model — collision-prone) and whether the CC's prepared entry was rebuilt after its model Arc was re-created.
4. Then fix, rebuild, drive, screenshot CC+dozer textured, and remove ALL UTB* probes (prior lanes' + this pass's).

Guards at hand-off: ww3d-renderer-3d --lib 357/357 (356 prior + new headless test); `cargo check -p ww3d-renderer-3d --lib --tests` and `-p generals_main --bin generals` clean. No git writes; no formatters. NOTE: target/release/generals (21:48) carries this pass's probes too.
Artifacts: /tmp/wsmoke/utbdrive14-19.sh, utb14..utb19 captures+stderr logs, utb14_utbvp1_dozer.png (pixel-decoded evidence), utb19_noterrain1_dozer.png (black world with no terrain), drive logs utb14_utbvport2 (in-match viewport 384).

---

## UnitGpuBisect — UTBVREAD wired + 8-drive live bisection COMPLETE: every per-draw input EXONERATED, body draws EXECUTE (magenta proof), fragments still absent; defect isolated between vertex-fetch+vertex-shader and fragment output; NOT fixed (2026-09-04 00:00-04:10)

Method: wired the staged UTBVREAD readback in `preparemodel` (render_manager.rs, staging MAP_READ buffer, copy_buffer_to_buffer of first 132 floats + 16 indices, synchronous poll+map — the ZPROBE pattern) and drove 640x480 in-match windows (utbdrive20.sh, utbbisect.sh, utbtrace.sh → /tmp/wsmoke/utb20_*, utbbisect_*, utbtrace_*).

### 1. UTBVREAD verdict (step 1 of hand-down plan)
GPU vertex/index buffer HEADS == CPU source on **all 460 prepared models** (0 `match_pos=false`), including the CC body `ABBtCmdHQ::7` (894 verts, 1422 idx, pos3/idx15 byte-identical). The buffer-lifecycle/pointer-cache theory (plan step 3) is DEAD: no GPU-side content mismatch exists.

### 2. Live bisection matrix (one env per drive, all pixel-decoded against the tan terrain reference)
- `GENERALS_UTBNOFOW=1` (FOW visibility forced 1/0/1): invisible.
- `GENERALS_UTBNOTEX=1` (stage masks zeroed, fallback-white route): invisible; NO wgpu validation errors (draws stayed valid).
- `GENERALS_UTBNOLIGHT=1` (lighting+fog env stripped, unlit pipeline variant): invisible.
- `GENERALS_UTBONLYBODY=1` (skip all meshes <256 verts; bright-shard pixels dropped 514→204, gate proven active): invisible.
- `GENERALS_DISC_DEPTH=1` (the EXISTING disc_depth_always probe — never driven before): invisible ⇒ depth rejection is NOT the mechanism. (The prior pass's UTBNODEPTH was a NO-OP: `depth_format.unwrap_or(Depth32Float)` at wgpu_pipeline_manager.rs:504 turned `None` back into Depth32Float, so it never changed any pipeline.)
- `GENERALS_UTBFORCEDRAW=1` (new: fullscreen magenta triangle from `vertex_index` only, drawn in every ≥256-vert body draw slot through RenderPassResources::set_pipeline): **SOLID MAGENTA FRAME** ⇒ body draw slots EXECUTE, the pass rasterizes, and the attachment reaches the screen. The defect is strictly inside the real pipeline's vertex-fetch/vertex-shader→fragment path.
- `GENERALS_UTBNOALPHATEST=1` (new: BASE_ALPHA_REF 96/255→0.0 via shader-source replace, kill the unconditional alpha.wgsl discard): invisible ⇒ alpha-test discard is NOT the whole mechanism either.
- `GENERALS_UTBARENAREAD=1` (new: stash first ≥256-vert draw's arena slice coords in CameraBinds/ModelBinds (new `offset/size` fields), then copy+map the EXECUTED arena bytes next frame in update_and_fill_live_cascade): 233 dumps incl. in-match CC `ATHQSlab.tga` frames: `vp_match=true mdl_match=true` byte-for-byte (gpu_vp3/exp, gpu_mdl translation (3067.989,0.000,2241.339,1.000)) ⇒ the camera VP and model matrix the GPU reads are EXACTLY the CPU values at draw time.

### 3. wgpu API trace attempt (GENERALS_UTBTRACE)
Added `wgpu-core` trace-feature wiring (workspace Cargo.toml + ww3d-gpu/Cargo.toml; `ww3d_gpu::device_authority::diagnostic_trace()` forced into every `request_device`). wgpu-core 27 traces record creations/writes/submits but **NOT render-pass commands** — dead end; trace dir deleted (was 1.8 GB).

### 4. State of the elimination (for the closer)
EXONERATED with in-match measurements: VP/viewport/frame-target (prior pass), GPU buffer content, uniform content at execution time, depth test, culling, alpha-test discard, textures, lighting, FOW, cross-draw interference, draw execution itself. PROVEN working: draw encode+execute, rasterizer, pass assembly, present.
THE ONLY UNMEASURED LINK LEFT: the vertex shader's computed clip positions for body draws (alpha.wgsl/opaque.wgsl `vs_main` output). Suggested next probe: write gl_Position/clip coords of a body draw to a small storage buffer from `vs_main` (one u32 flag in the model uniform selects the debug branch) and read back — this directly measures what NDC the body vertices land at, splitting "vertex fetch/stide mismatch at draw time" from "vertex math".

### New probe inventory added this pass (ALL env-gated, REMOVE AFTER fix)
render_manager.rs: UTBVREAD (readback), UTBONLYBODY, UTBNOLIGHT (render_mesh), UTBFORCEDRAW (+`debug_draw_pipeline`/`utb_debug_draw_pipeline`), UTBNOTEX, UTBNOFOW (draw_material_pass), UTBARENAREAD (+`UtbArenaProbe` struct, `utb_arena_probe` field, drain in update_and_fill_live_cascade). wgpu_material_binds.rs: CameraBinds/ModelBinds gained `offset/size` fields (inert). frame_uniform_arena.rs: pages granted COPY_SRC (inert otherwise). wgpu_pipeline_manager.rs: UTBNOALPHATEST shader-source replace. Cargo.toml (workspace + ww3d-gpu): wgpu-core trace feature + `diagnostic_trace()` (inert without GENERALS_UTBTRACE). wgpu/Cargo.toml briefly had `features=["trace"]` on the facade — reverted (feature does not exist in wgpu 27).
Drive scripts: /tmp/wsmoke/utbdrive20.sh, utbbisect.sh (generic TAG ENV), utbtrace.sh; captures utb20_vread1_*, utbbisect_{nofow1,notex1,nolight1,onlybody1,forcedraw1,depthalways1,arenaread1,arenaread2,noalphatest1}_dozer.png + stderr logs.

Guards at hand-off: `cargo check -p ww3d-renderer-3d --lib --tests` clean; `cargo test -p ww3d-renderer-3d --lib` **357/357** (headless `utb_headless_body_mesh_paints_pixels` still green); `cargo check -p ww3d-gpu --lib` + `-p generals_main --bin generals` clean. No git writes. target/release/generals (04:05) carries ALL probes above.

---

## VsClipDump — UTBCLIP wired + vertex-stage output MEASURED CORRECT + ROOT-CAUSED & FIXED: shader-state bit decode divergence from C++ (blend enums inverted + shift-table drift) made opaque materials decode as (Zero,One) write-dst = invisible-while-fully-drawn; CC + dozer now VISIBLE TEXTURED in-match (2026-09-04 02:00-03:40)

### 1. UTBCLIP probe (GENERALS_UTBCLIP=1): the last unmeasured link is now measured
Instrumented alpha/opaque/decal/additive `vs_main` (env-gated source replace in wgpu_pipeline_manager.rs, gated on `vertex.position` so skinned.wgsl is untouched) to write per-vertex fetched position + computed clip into the group-7 illumination slot, flipped read/write under env; dedicated dump buffer bound in draw_material_pass for the first ≥256-vert pass-0 body draw (8→64 dumps/frame budget), drained next frame in `update_and_fill_live_cascade` (UTBARENAREAD pattern). Required `Features::VERTEX_WRITABLE_STORAGE` env-gated in ww3d-gpu device_authority.
VERDICT (8 menu dumps + 56 in-match dumps incl. `ATHQSlab.tga` verts=894 dumped=894 untouched=0): **gpu clip == CPU projection EXACTLY, full-mesh NDC bbox identical** (`x -0.211..0.217 y -0.908..-0.112` = on-screen). Vertex fetch + vs_main math EXONERATED on the GPU.

### 2. Elimination drives with the probe binary
- `GENERALS_UTBFSMAG=1` (fs returns solid magenta α=1): ONLY the known shard quads turned magenta — body primitives die between VS and FS.
- `GENERALS_DISC_CULL=1`: shards turned BLACK (probe live on every pipeline) — bodies still gone ⇒ cull exonerated on current binary.
- `GENERALS_UTBFSMAG=1 GENERALS_DISC_CULL=1 GENERALS_DISC_DEPTH=1` (all rasterizer rejection removed): bodies STILL paint nothing ⇒ defect is NOT vertex math, NOT cull, NOT depth, NOT fragment math.
- UTBDRAW (clipdump3): CC slab draws `start=0 count=1422` at (3068,0,2241.3) in-match — full-range draw executes.

### 3. ROOT CAUSE (C++ parity bug in ww3d-renderer-3d shader.rs)
`W3dShaderStruct` carries C++-numbered fields; `from_w3d_shader` re-encodes them at the RUST shift table, and getters decode raw values with INVERTED blend enums. C++ GeneralsMD shader.h truth: `SRCBLEND_ZERO=0, ONE=1, SRC_ALPHA=2, ONE_MINUS_SRC_ALPHA=3` (2 bits @14), `DSTBLEND_ZERO=0, ONE=1, SRC_COLOR=2, INV=3, SRC_ALPHA=4, INV=5` (3 bits @5), FOG@8(2), PRI@10(3), SEC@13(1), TEXTURING@16, NPATCH@17, ALPHATEST@18, CULL@19, POSTDETAILCOLOR@20(4), POSTDETAILALPHA@24. Rust had SHIFT_SRCBLEND=15, FOG=9, PRI=11(2b), SEC(2b), TEXTURING=18, NPATCH=19, ALPHATEST=20, CULL=21, POSTDETAIL 22/26 AND enums `One=0, Zero=1, ...`. Net: the CC slab's authored (SRC_ALPHA, ZERO) cutout decoded as (Zero, One) → `create_blend_state_from_shader` produced wgpu (Zero, One) → `out = dst` — every opaque/world mesh fragment wrote the destination unchanged (invisible while fully drawn, all probes upstream dead). Explains: bodies+buildings+dozer invisible, white shard quads visible (their passes decode differently), UTBNOTEX/FOW/depth/cull/magenta all inconclusive.

### 4. FIX (ww3d-renderer-3d only; assets/gpu crates were already C++-correct)
- `shader_system/shader.rs`: shift+mask table → C++ layout; `SrcBlendFuncType{Zero=0,One=1,SrcAlpha=2,InvSrcAlpha=3}`; `DstBlendFuncType{Zero=0,One=1,SrcColor=2,InvSrcColor=3,SrcAlpha=4,InvSrcAlpha=5}`; `SecGradientType{Disable,Enable}`; `PriGradientType` + BumpEnvMap/BumpEnvMapLuminance/Modulate2x; all getter/setter tables + widths; `blend_mode()` Multiply=(Zero,SrcColor), Screen=(One,InvSrcColor); `set_src_blend/set_dest_blend` DX8 maps; internal wgpu converters.
- `render2d/gpu_context.rs` map_src/dst_factor: removed-variant arms.
- `wgpu_pipeline_manager.rs` create_blend_state_from_shader: same arms; UTBNOBLEND probe (kept, env-gated).
- **alpha.wgsl:632 / decal.wgsl:636**: `let mut final_alpha` — invalid WGSL that had NEVER compiled (no material routed to alpha/decal under the broken decode); `mut` removed. First-ever compile now clean.
- Side effect of correct decode: (SrcAlpha,InvSrcAlpha) materials NOW route to alpha.wgsl for the first time; its unconditional `discard` (BASE_ALPHA_REF) is gated on materials that author ALPHATEST — parity note from UnitBodyFix §7 still stands for a follow-up pass.

### 5. VERIFICATION
- `utbbisect_blendfix2` in-match 640x480, camera at CC (3068,0,2241.3): **CC fully visible textured + construction dozer textured; white shard blobs GONE** (`/tmp/wsmoke/utbbisect_blendfix2_dozer.png`). No wgpu validation errors.
- Guards: `cargo test -p ww3d-renderer-3d --lib` **357/357**; `--tests` suites green except `headless_smoke::headless_wrapper_supports_basic_lifecycle` which is a PRE-EXISTING wgpu device-singleton contention between sibling tests (passes alone: `--test headless_smoke headless_wrapper_supports_basic_lifecycle` ok).
- Guards NOT re-run this pass (worker scope): combat filter 966/0, world_tests catalog, gameworld_shadow 302/302 — decode change is confined to ww3d-renderer-3d material-pass state; main-agent project-wide validation should re-run them.

### 6. PROBE STATUS — NOT YET REMOVED (env-gated, inert without env)
All UTB*/DISC_* probes remain in-tree (this pass added UTBCLIP + UTBFSMAG + UTBNOBLEND; UTBNOALPHATEST/UTBARENAREAD/UTBVREAD/UTBFORCEDRAW/etc. from prior passes; DISC_DEPTH/DISC_CULL pre-existing; UTBCLIP's group-7 read_write flip is env-gated; wgpu-core trace feature + diagnostic_trace() from UnitGpuBisect still in Cargo.tomls/device_authority). Removal = mechanical sweep across render_manager.rs, wgpu_pipeline_manager.rs, wgpu_material_binds.rs (offset/size fields inert), frame_uniform_arena.rs, device_authority.rs, particle_renderer.rs, terrain_visual/impl_gpu.rs, forward_render.rs, pipeline_execute.rs, Cargo.toml trace features + blend_state_tests adjustments — recommended as its own pass with a fresh release build + the same utbbisect drives as regression check.
Artifacts: /tmp/wsmoke/utbbisect_{clipdump2,clipdump3,fsmag1,cullnone1,allkill1,blendfix1,blendfix2}_{dozer.png,stderr.log}; blendfix1 stderr shows the alpha.wgsl `mut` parse error that exposed the dead-shader lane. No git writes. target/release/generals (03:2x) carries fix + all probes.

---

## VisualGap2 — fresh full-state visual gap inventory on the blend-fix binary + 3 fixes landed (2026-09-04 03:30-05:00)

### Method
HEAD 98bb6b20f release binary (built 03:27). Single windowed drive
`/tmp/wsmoke/vg2drive.sh` (control-command only; OS synthetic clicks still dead),
log `/tmp/wsmoke/vg2.log`, captures `/tmp/wsmoke/vg2_*.png` (+`.txt` status
snapshots), child stderr under `$(cat /tmp/wsmoke/vg2_dir)`. Pixel decode via
`/tmp/wsmoke/vg2_decode.py` + PIL crops (`vg2_crop_*.png`). Chain: Menu → SP
flyout → Skirmish options → InGame initial → selection → move order → build
attempt (ButtonCommand03) → placement → combat attempt → ESC QuitMenu → pause →
diplomacy → options → save/load. Every prior UiGapInventory item re-assessed
against THIS binary.

### Re-assessment of prior items (fixed / still-open)

FIXED this drive:
- Unit/building invisibility (UnitLightFix/VsClipDump lane): CONFIRMED FIXED —
  CC complex + dozer render fully textured (`vg2_ingame_initial.png`,
  `vg2_esc_quitmenu.png`); no white blobs, no red command grid, no giant caption
  band (GiantTextFix/CtrlBarTexFix/RedRectsTriage residuals gone from the bar).

STILL OPEN (with fresh evidence):
- **Minimap/radar black** (`vg2_crop_mm.png`): 640x480 (10..175, 385..470) is a
  pure-black bordered rect. Prior "fixed on HEAD" claim NOT observed on this
  drive. Retail start shows live radar terrain around the start position
  (C++ RadarActor/W3DRadar draws from first frame). P1-class: orientation +
  radar-dependent commands unusable.
- **MoneyDisplay amount missing** (`vg2_crop_money.png`): "$$$" glyphs at the
  money slot; retail "$" + live amount (C++ ControlBar updateMoney). Prior
  InGame-2 confirmed. P2.
- **Selection visuals** (`vg2_crop_unit_sel.png`): only a small grey fleck left
  of the unit; no player-color ring, no health pips (C++ InGameUI draw
  selection + InGameUI.cpp health grid). Prior Select-1 confirmed. P1-class for
  play feel, P2 render-wise.
- **Command grid empty with selection** — NEW precision on prior Select-2: the
  no-selection grid shows cameos + observer-stat labels (Units/Buildings/
  Destroyed/Lost); on select_local_unit the cameos CLEAR and the grid renders
  EMPTY (9,255 px change in the bar band between initial/selected). The map
  dozer's command set never binds → `winit_click_named` on
  `ControlBar.wnd:ButtonCommand03` MISSED and the build chain aborted
  (`vg2.log` 03:42:29, `vg2_uc_failed.png`). Note: cbdrive2's acceptance ran the
  golden-fixture dozer (command-set override stamped by SimSystemsFix); the REAL
  map-spawned dozer has no bound set. Retail: dozer selection shows its build
  shortcut grid (C++ ControlBar::setCommandSet from
  `Object::getCommandSetString`). P1: build gameplay impossible from the real map.
- **Combat impossible in skirmish**: `attack_nearest_enemy` →
  `attack_fail_no_enemy`; `match_kills=0` throughout — the skirmish map spawns
  NO enemy player/units (retail Defcon6 skirmish spawns an AI opponent).
  Gameplay-population gap, not purely visual. P1 for combat visuals (nothing to
  fight, no projectiles/death visuals reachable).
- **Shell map absent** (MainMenu interior black behind the dropdown,
  `vg2_sp_flyout.png`). Prior MainMenu-1 confirmed. P2.
- **Skirmish options** (`vg2_skirmish_options.png`): map preview absent +
  unnumbered start dots (Skirmish-3), `TextEntryMapDisplay` shows literal
  "Static Text" (Skirmish-4), `TextEntryPlayerName` shows "Entry" (Skirmish-5),
  slider thumb invisible (Skirmish-2). All confirmed on HEAD. P2/P3.
- **Skybox faces still fail**: 12 WARNs each for
  TSMorning{E,S,W,N,T}.tga in child stderr — sky still missing in-match
  (asset-extraction class). P2.
- **"HUD" literal** bottom-left + faint tile lattice on terrain zoom
  (`vg2_crop_order.png`). P3.
- Menu/SP flyout minors from prior inventory unchanged (GreenDot/Clock/posters/
  RecentSave absent; EarthMap art; "CHALLENGE" label). P3.

REGRESSED/CONTRADICTED claim:
- RedRectsTriage's "0 red rects can leak" does NOT hold on the shell path:
  `vg2_crop_cash.png` — the anonymous Starting-Cash STATICTEXT
  (`SkirmishGameOptionsMenu.wnd:` rect 360,334-453,358) paints PURE
  `255,0,0,255` fill (pixel samples (300,270)=(255,0,0,255) etc.). ROOT CAUSE
  (this pass): `draw_window_image_or_fallback` no-image branch fills
  `visible_enabled_color()` which accepts the authored draw-data color — and
  retail WNDs author `255 0 0 255` as the NoImage placeholder sentinel. C++
  paints NO back fill when the image is absent
  (GeneralsMD W3DStaticText.cpp `W3DGadgetStaticTextImageDraw`: image-if-present
  + text only). **FIXED this pass** (below).

### Fixes landed this pass (all compile-clean `cargo check -p generals_main --bin generals`)

1. **ESC/QuitMenu paints nothing (prior PauseMenu-1 P1)** — root cause: the
   runtime-host command called ONLY the residual latch
   `simulate_quit_menu_toggle_show()` (flips an atomic; never loads the WND),
   while the real C++-parity toggle
   `game_client::gui::callbacks::toggle_quit_menu_with_result()` +
   Main's `host_toggle_retail_quit_menu()` bridge existed unused on that path.
   Fix: `runtime_host` `campaign_menus.rs` `runtime_host_cmd_toggle_quit_menu`
   now runs `host_toggle_retail_quit_menu()` first (C++ MSG_META_OPTIONS →
   ToggleQuitMenu parity), residual latch only as fallback.
2. **open_diplomacy stranded the match in the shell (NEW P1, worse than prior
   Diplomacy-1)**: mid-match `open_diplomacy` called
   `enter_shell_menu_from_runtime_host(Some("Diplomacy"))` → pushed the shell
   MainMenu over the live world AND flipped `state=Menu` (vg2.log 03:45:30 →
   `state=Menu ui_screen=Some(Diplomacy)` forever; save/quickload/options
   silently drained no-ops afterward — the save/load leg of the chain was
   unreachable). Fix: `gameplay_select.rs` `runtime_host_cmd_open_diplomacy`
   now toggles the live in-match `DiplomacySystem`
   (`game_client::gui::callbacks::toggle_diplomacy(false)` — creates/shows
   Diplomacy.wnd per C++ Diplomacy.cpp) when InGame/Paused; shell push retained
   for Menu state only.
3. **Placeholder-red sentinel fills** (contradicted claim above): `common.rs`
   `visible_enabled_color` now treats packed `0xFFFF_0000`
   (authored `255 0 0 255`) as undefined → all 8 fallback-fill call sites
   (static text, main menu, push button, power, progress, hud, command bar)
   render their honest slate fallback instead of the sentinel red.

Verification drive for 1+2: `/tmp/wsmoke/vg2verify.sh` (results appended below
once driven).

### Fresh gap inventory (severity-tagged; coordinates 640x480 actual)

| id | severity | item | evidence |
|---|---|---|---|
| G1 | P1 | minimap/radar fully black in-match | vg2_crop_mm.png |
| G2 | P1 | dozer command grid empty on selection → build impossible from real map (ButtonCommand03 miss) | vg2.log 03:42:29, vg2_uc_failed.png |
| G3 | P1 | no enemy in skirmish → no combat visuals reachable (attack_fail_no_enemy) | vg2.log 03:44:10, vg2_combat_attempt.png |
| G4 | P1 | pause screen renders nothing (paused=true, no dim/panel) | vg2_pause_screen.png (ui_screen stays GameHUD; legacy PauseMenu never paints in windowed host) |
| G5 | P2 | no selection ring/health pips (grey fleck only) | vg2_crop_unit_sel.png |
| G6 | P2 | money amount never rendered ("$$$" only) | vg2_crop_money.png |
| G7 | P2 | skirmish map preview absent; "Static Text"/"Entry" literals; slider thumb invisible | vg2_skirmish_options.png |
| G8 | P2 | skybox assets missing (TSMorning* WARNs) | child stderr |
| G9 | P2 | shell-map menu background absent | vg2_menu.png, vg2_sp_flyout.png |
| G10 | P2 | red placeholder fills (Starting-Cash label) | FIXED this pass; re-verify |
| G11 | P3 | "HUD" literal bottom-left; faint terrain tile lattice; two unidentified slate rects above bar right (~515-575, 345-370); ProductionQueue empty cells dark-maroon; menu minors (GreenDot/Clock/posters/EarthMap/CHALLENGE label) | vg2_crop_*.png |

Escaped P1s (ESC menu + diplomacy) have fixes 1+2 above; G4 (pause) is the
remaining unpainted overlay: `toggle_pause` keeps `ui_screen=GameHUD` — the
legacy Rust PauseMenu screen is never entered in the windowed host path
(`input.rs:1142` enters it on GameState::Paused transitions; the host pause path
does not). Next driver: route host pause through the same screen projection or
adopt the C++ pattern (retail ESC = QuitMenu; the separate pause screen is only
the fall-back surface).

Artifacts: `/tmp/wsmoke/vg2_*.png|txt`, `/tmp/wsmoke/vg2.log`,
`/tmp/wsmoke/vg2_dir`, `/tmp/wsmoke/vg2decode.py` → `vg2_decode.py`,
`/tmp/wsmoke/vg2drive.sh`, `/tmp/wsmoke/vg2verify.sh`. Serial run, caffeinate
-dis, no git writes, no formatters. Guards untouched (runtime-host command
lanes + one draw-color helper; residual test pins
(`source_scan_tests` "open_diplomacy"/"diplomacy_ok",
`host_quit_menu_residual_wave123` cmd names) verified still satisfied by grep.

---

## ProbeSweep — ALL UTB/DISC/ZPROBE render probes removed + alpha-test gated on the authored ShaderClass ALPHATEST bit; guards green; units still textured in-match (2026-09-04 04:00-04:35)

### 1. Probe sweep (Task A) — tree clean, acceptance grep 0 hits
Removed every env-gated render probe from the UnitTexBind/UnitLightFix/UnitLightFix2/
UnitBodyFix/UnitGpuBisect/VsClipDump inventories: render_manager.rs (UTBVIEW/UTBMAT/
UTBUVCENTER/UTBVREAD/UTBIDX/UTBVERTS statics+blocks, UtbArenaProbe/UtbClipProbe structs
+ fields + drains, UTBFORCEDRAW + `utb_debug_draw_pipeline`, UTBONLYBODY/UTBNOLIGHT/
UTBMESHNULL/UTBLIGHT/UTBNOTEX/UTBNODEPTH/UTBVP/UTBNOFOW/UTBMESHVIS/UTBPASSTINT+UTBRANGE/
UTBARENAREAD stash/UTBCLIP rebind, UTBDRAWLOG/UTBDRAWSKIP/UTBNONIDX incl. the now-unused
`issue_draw_call` mesh_translation param, UTBVIEW in stage_resources_for incl. the
branch-tuple plumbing); lib.rs UTBVPVPORT; wgpu_material_binds.rs UTBMAGENTA + the inert
CameraBinds/ModelBinds offset/size fields; frame_uniform_arena.rs COPY_SRC; ww3d-gpu
device_authority.rs UTBTRACE (`diagnostic_trace`) + UTBCLIP VERTEX_WRITABLE_STORAGE gate;
wgpu-core trace feature out of workspace Cargo.toml + ww3d-gpu/Cargo.toml; ww3d-engine
`color_texture_arc()` (UTBPIX) + the ZPROBE COPY_SRC depth usage; Main occlusion_bridge
UTBOVERLAYTAG, forward_render UTBTERRVP, pipeline_collect UTBWATERNULL, pipeline_execute
ZPROBE + UTBPIX (whole blocks), pipeline_prewarm DISC_NOTERRAIN; GameClient shadow_pass
UTBSHADOWTAG, particle_renderer UTBDECALTAG (incl. the tinted-decal clone branch),
terrain impl_gpu UTBWATERNULL/UTBROADNULL/UTBTREENULL/UTBTREEDUMP + `utb_tree_dump_latch`
/UTBEXTRABLENDNULL, overlay_gpu UTBROADNULL/UTBWATERNULL. `GENERALS_CBGRID_PROBE`
verified already 0 hits. KEPT: headless regression test
`utb_headless_body_mesh_paints_pixels` (still green inside the 357). /tmp/wsmoke drive
scripts untouched (outside repo). Acceptance: `grep -rE "GENERALS_UTB|GENERALS_DISC|
GENERALS_CBGRID" --include=*.rs --include=*.toml Code` → **0 hits**; `strings
target/release/generals | grep -cE "GENERALS_UTB|GENERALS_DISC|GENERALS_ZPROBE"` → **0**.

### 2. AlphaTest parity (Task B) — discard gated on the authored bit
`alpha.wgsl`/`decal.wgsl` discarded unconditionally when `BASE_ALPHA_REF>0`; C++ W3D
applies alpha test only when the ShaderClass ALPHATEST bit is authored: `ShaderClass::
Apply` drives `D3DRS_ALPHATESTENABLE` from `BOOL(Get_Alpha_Test())` (GeneralsMD
shader.cpp:998) with alphareference 0x60 (shader.cpp:427); the default device state is
`ALPHATESTENABLE=FALSE` / `ALPHAREF=0` (dx8wrapper.cpp:3682/3688). Fix:
`wgpu_pipeline_manager::gate_alpha_test_discard(&shader, source)` (pub, unit-testable) —
materials WITHOUT the bit compile the `96.0 / 255.0` threshold to `0.0` so
`final_alpha < alpha_threshold` can never fire (default no-discard); materials WITH the
bit keep 96/255. Wired at the single shader-module creation site in `get_or_create`,
alongside blend state (per-material, `PipelineKey` already carries shader_bits so
enabled/disabled variants cache separately; skinned/line sources carry no constant and
pass through unchanged). Focused test
`blend_state_tests::test_alpha_test_discard_gated_on_shader_bit` (blend_state_tests.rs
pattern): default (SrcAlpha,InvSrcAlpha) material zeroes the threshold; authored bit
keeps 96/255; raw `W3dShaderStruct` decode (C++ shift 18) drives the gate both ways;
non-carrying sources pass through. **20/20 blend_state_tests green.**

### 3. Guards (serial cargo, idle machine)
ww3d-renderer-3d `--lib` **357/357** (incl. the kept headless test);
`--tests` suites green except the documented pre-existing `headless_smoke`
device-singleton contention (each test passes alone; same as VsClipDump reported);
`--test blend_state_tests` **20/20**; combat filter **966/0** (one contended run showed
963-965 with a VARYING failure set — overlord_bunker/chinook flakes that all pass in
isolation; idle rerun 966/0); world_tests catalog **8/8**; gameworld_shadow **304/304**
(suite grew from the 302 hand-off).

### 4. Windowed drive verification (sweepdrive1.sh, utbdrive20 pattern, env-FREE launch)
Release binary (04:15, probe-free by strings scan): Menu → `start_game
mode=skirmish faction=USA` → InGame → `camera_look_at 3108,120,2201.3` → request_capture.
EXIT=0, no wgpu validation errors, **zero UTB/ZPROBE/DISC log lines** in child stderr.
Pixel census of `/tmp/wsmoke/sweep_sweep1_ingame.png` vs the VsClipDump known-good
`utbbisect_blendfix2_dozer.png`: white 0.2% vs 0.2%, tan 77.4% vs 77.2%, world-area
white 79 px vs 77 px (defect-era utb14: 409 px), top color buckets identical — CC +
dozer render TEXTURED, no white shards, HUD intact (visually confirmed).

Changed files: ww3d-renderer-3d (render_manager.rs, lib.rs, wgpu_material_binds.rs,
frame_uniform_arena.rs, wgpu_pipeline_manager.rs), ww3d-gpu (device_authority.rs,
Cargo.toml), workspace Cargo.toml, ww3d-engine/src/lib.rs, Main graphics
(occlusion_bridge/forward_render/pipeline_collect/pipeline_execute/pipeline_prewarm),
GameClient (shadow_pass.rs, particle_renderer.rs, terrain impl_gpu.rs, overlay_gpu.rs),
tests/blend_state_tests.rs. No git writes; no formatters. New drive script
`/tmp/wsmoke/sweepdrive1.sh`; evidence `/tmp/wsmoke/sweep_sweep1_{ingame.png,stderr.log,log}`,
`/tmp/wsmoke/combat_guard_post_sweep.log`, `/tmp/wsmoke/combat_guard_run2.log`.
