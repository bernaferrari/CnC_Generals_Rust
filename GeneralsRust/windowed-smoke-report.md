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
